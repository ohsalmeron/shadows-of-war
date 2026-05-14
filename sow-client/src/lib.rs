use winit::event::{Event, WindowEvent, MouseButton, ElementState, MouseScrollDelta};
use sow_render::{RenderContext, MapRenderer, MapGlobals};
use crate::sim_bridge::{SimBridge, PlatformSimBridge};
use sow_core::protocol::{SimCommand, SimSnapshot};

use sow_core::game_config::GameConfig;

use blade_graphics as gpu;
use blade_egui::GuiPainter;
use egui::{Context, RawInput, Pos2, Rect, Vec2};
use sow_ui::{ClientApp, app::ClientPhase, UiAction};
use web_time::{Instant, Duration};
use sow_net::client::SowClient;
use std::collections::HashMap;

fn get_build_version() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(val) = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("SOW_BUILD_VERSION")) {
                if let Some(s) = val.as_string() {
                    if s != "__BUILD_TS__" {
                        return s;
                    }
                }
            }
        }
        "unknown".to_string()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read_to_string(".version").unwrap_or_else(|_| "unknown".to_string()).trim().to_string()
    }
}

fn get_maps_url() -> String {
    #[allow(unused_mut)]
    let mut url = std::env::var("SOW_MAPS_URL").unwrap_or_else(|_| "https://darkrift.ai/assets/maps".to_string());
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(val) = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("SOW_MAPS_URL")) {
                if let Some(s) = val.as_string() {
                    url = s;
                }
            }
        }
    }
    url
}

mod nameplates;
use nameplates::*;

mod client_config;
use client_config::ClientVisualConfig;

/// Allow very wide map views (scroll / pinch clamp to this minimum).
const CAMERA_MIN_ZOOM: f32 = 0.001;
/// Hard ceiling so zoom stays finite and GPU paths stay well-behaved.
const CAMERA_MAX_ZOOM_CAP: f32 = 8192.0;

/// Pixels-per-world-unit zoom max scales with window size so you can fill ~one hex tile
/// across the long screen axis (hex neighbor spacing ≈ 1 world unit in the map shader).
fn camera_zoom_upper_bound(screen_w: f32, screen_h: f32) -> f32 {
    let longest = screen_w.max(screen_h).max(1.0);
    (longest * 3.0).clamp(768.0, CAMERA_MAX_ZOOM_CAP)
}

/// Zoom level used only for nameplate **font** sizing (not LOD). Matches default `camera_zoom` so
/// first-frame text size matches the old formula, while zooming no longer churns egui glyph atlas sizes.
const NAMEPLATE_REFERENCE_ZOOM: f32 = 2.0;



fn spawn_sow_client_connect(
    url: String,
    connect_tx: &crossbeam_channel::Sender<Result<SowClient, String>>,
    #[cfg(not(target_arch = "wasm32"))] tokio_rt: &tokio::runtime::Runtime,
) {
    let tx = connect_tx.clone();
    let fut = async move {
        match SowClient::connect(&url).await {
            Ok(c) => {
                let _ = tx.send(Ok(c));
            }
            Err(e) => {
                let _ = tx.send(Err(e.to_string()));
            }
        }
    };
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(fut);
    #[cfg(not(target_arch = "wasm32"))]
    tokio_rt.spawn(fut);
}


pub enum MapDownloadEvent {
    MapReady(String, Vec<u8>),
    ThumbnailReady(String, Vec<u8>),
    Progress(String, u8),
    Error(String),
}

pub enum EngineInitEvent {
    Status(String),
    Progress(f32),
    Complete(Box<sow_core::game::GameState>, sow_core::water_components::WaterComponents, Box<sow_core::protocol::ServerStartMessage>),
}

pub fn run_game(event_loop: winit::event_loop::EventLoop<()>) {
    #[cfg(target_os = "android")]
    {
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(log::LevelFilter::Debug)
                .with_tag("sow-client")
        );
        std::panic::set_hook(Box::new(|info| {
            log::error!("PANIC: {}", info);
        }));
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = env_logger::builder().filter_level(log::LevelFilter::Info).try_init();
    }
    // ── Simulation ──────────────────────────────────────────────────────────
    let mut map_w: u32 = 800;
    let mut map_h: u32 = 600;
    let config = GameConfig::default();
    
    let bridge = PlatformSimBridge::spawn();
    bridge.send_command(SimCommand::Init {
        config,
        seed: 12345,
        map_bytes: vec![],
        players: vec![],
    });
    
    let mut current_snapshot: Option<SimSnapshot> = None;

    // ── Renderer ────────────────────────────────────────────────────────────
    let mut render_ctx = RenderContext::new();
    let mut surface: Option<gpu::Surface> = None;
    let mut map_renderer: Option<MapRenderer> = None;
    let mut gui_painter: Option<GuiPainter> = None;
    let mut window: Option<winit::window::Window> = None;

    // ── UI State ────────────────────────────────────────────────────────────
    let mut app = ClientApp::new();
    let mut egui_ctx = Context::default();
    sow_ui::ui::theme::apply_theme(&egui_ctx);
    let mut raw_input = RawInput::default();

    // ── Network State ───────────────────────────────────────────────────────
    #[cfg(not(target_arch = "wasm32"))]
    let tokio_rt = tokio::runtime::Runtime::new().unwrap();
    let mut net_client: Option<SowClient> = None;
    let mut turn_queue = std::collections::VecDeque::new();
    let mut my_player_id: Option<u16> = None;
    let mut my_lobby_id: Option<u64> = None;
    let (map_tx, map_rx) = crossbeam_channel::unbounded::<MapDownloadEvent>();
    type EngineInitData = (sow_core::game::GameState, sow_core::water_components::WaterComponents, sow_core::protocol::ServerStartMessage);
    let (engine_init_tx, engine_init_rx) = crossbeam_channel::unbounded::<EngineInitEvent>();
    let mut pending_engine_init_data: Option<EngineInitData> = None;
    let mut engine_init_queued_msg: Option<sow_core::protocol::ServerStartMessage> = None;

    let mut nameplate_cache: HashMap<u16, CachedNameplate> = HashMap::new();
    let mut troop_label_throttle = TroopLabelThrottle::default();

    let (connect_tx, connect_rx) = crossbeam_channel::unbounded();

    // Reconnect scheduling (idle drop / resume / failed handshake).
    let mut ws_connect_fail_backoff_ms: u64 = 400;
    let mut ws_connect_not_before: Instant = Instant::now();
    let mut ws_reconnect_after_resume: bool = false;
    #[cfg(target_arch = "wasm32")]
    let mut wasm_doc_was_visible: bool = true;

    #[allow(unused_mut)]
    let mut ws_url = std::env::var("SOW_WS_URL").unwrap_or_else(|_| "wss://darkrift.ai/ws/".to_string());
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(window) = web_sys::window() {
            if let Ok(val) = js_sys::Reflect::get(&window, &wasm_bindgen::JsValue::from_str("SOW_WS_URL")) {
                if let Some(s) = val.as_string() {
                    ws_url = s;
                }
            }
        }
    }
    app.main_menu_state.server_address = ws_url.clone();
    
    log::info!("Auto-connecting to {}...", ws_url);
    app.main_menu_state.is_connecting = true;
    #[cfg(target_arch = "wasm32")]
    spawn_sow_client_connect(ws_url.clone(), &connect_tx);
    #[cfg(not(target_arch = "wasm32"))]
    spawn_sow_client_connect(ws_url.clone(), &connect_tx, &tokio_rt);

    // ── Camera state ────────────────────────────────────────────────────────
    let mut camera_x: f32 = 0.0;
    let mut camera_y: f32 = 0.0;
    let mut camera_zoom: f32 = 2.0;
    let mut screen_w: f32 = 1280.0;
    let mut screen_h: f32 = 720.0;

    // Mouse drag state
    let mut dragging = false;
    let mut last_mouse_x: f64 = 0.0;
    let mut last_mouse_y: f64 = 0.0;

    let mut label_positions: HashMap<u16, (f32, f32)> = HashMap::new();
    
    // Touch state for pinch-to-zoom
    let mut active_touches: HashMap<u64, (f64, f64)> = HashMap::new();
    let mut last_pinch_distance: Option<f64> = None;

    // Tracks last `Window::set_ime_allowed` value (mirrors egui-winit debounce).
    let mut ime_allowed_state = false;
    // Last physical-pixel IME area for `set_ime_cursor_area`, for debouncing.
    let mut ime_cursor_rect_px: Option<Rect> = None;

    let mut prev_sync_point: Option<gpu::SyncPoint> = None;
    let mut last_tick = Instant::now();
    let start_time = Instant::now();
    let tick_interval = Duration::from_millis(100);
    let mut needs_first_upload = true;

    let mut frame_count = 0;
    let mut last_fps_time = Instant::now();
    let mut current_fps = 0;
    let mut current_ping_ms: Option<u32> = None;
    let mut last_ping_time = Instant::now();
    let mut last_frame_time = Instant::now();

    event_loop.run(move |event, elwt| {
        match event {
            Event::Suspended => {
                if let Some(sp) = prev_sync_point.take() {
                    let _ = render_ctx.context.wait_for(&sp, !0);
                }
                if let Some(mut s) = surface.take() {
                    if let Some(mut gp) = gui_painter.take() {
                        gp.destroy(&render_ctx.context);
                    }
                    if let Some(mut mr) = map_renderer.take() {
                        mr.destroy(&render_ctx);
                    }
                    render_ctx.context.destroy_surface(&mut s);
                }
            }
            Event::Resumed => {
                // App or tab foregrounded — retry WS soon if the socket died in the background.
                ws_reconnect_after_resume = true;
                let win = window.get_or_insert_with(|| {
                    #[cfg(any(target_os = "android", target_os = "ios"))]
                    let builder = winit::window::WindowBuilder::new()
                        .with_title("Shadows of War")
                        .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));

                    #[cfg(target_arch = "wasm32")]
                    let mut builder = {
                        let win = web_sys::window().unwrap();
                        let w = win.inner_width().unwrap().as_f64().unwrap();
                        let h = win.inner_height().unwrap().as_f64().unwrap();
                        winit::window::WindowBuilder::new()
                            .with_title("Shadows of War")
                            .with_inner_size(winit::dpi::LogicalSize::new(w, h))
                    };

                    #[cfg(target_arch = "wasm32")]
                    {
                        use winit::platform::web::WindowBuilderExtWebSys;
                        use wasm_bindgen::JsCast;
                        let window = web_sys::window().unwrap();
                        let document = window.document().unwrap();
                        let canvas = document.get_element_by_id("blade")
                            .unwrap()
                            .dyn_into::<web_sys::HtmlCanvasElement>()
                            .unwrap();
                        builder = builder.with_canvas(Some(canvas));
                    }

                    #[cfg(not(any(target_os = "android", target_os = "ios", target_family = "wasm")))]
                    let builder = winit::window::WindowBuilder::new()
                        .with_title("Shadows of War — Native")
                        .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));

                    
                    builder.build(elwt).unwrap()
                });
                
                if surface.is_none() {
                    let sz = win.inner_size();
                    let s = render_ctx.create_surface(win, sz.width.max(1), sz.height.max(1));
                    screen_w = sz.width as f32;
                    screen_h = sz.height as f32;
                    let zmax = camera_zoom_upper_bound(screen_w, screen_h);
                    camera_zoom = camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);
                    raw_input.screen_rect = Some(Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(screen_w, screen_h)
                    ));
                    let format = s.info().format;
                    
                    if let Some(sp) = prev_sync_point.take() {
                        let _ = render_ctx.context.wait_for(&sp, !0);
                    }
                    let mut old_terrain = vec![128; (map_w * map_h) as usize];
                    if let Some(mut old_mr) = map_renderer.take() {
                        old_terrain = old_mr.terrain.clone();
                        old_mr.destroy(&render_ctx);
                    }
                    render_ctx.command_encoder.start();
                    map_renderer = Some(MapRenderer::new(&render_ctx.context, &mut render_ctx.command_encoder, map_w, map_h, format, &old_terrain));
                    let sync_point = render_ctx.context.submit(&mut render_ctx.command_encoder);
                    prev_sync_point = Some(sync_point);
                    
                    gui_painter = Some(GuiPainter::new(s.info(), &render_ctx.context));
                    surface = Some(s);
                    
                    // Re-create egui context to force it to re-upload its font texture!
                    egui_ctx = Context::default();
                    sow_ui::ui::theme::apply_theme(&egui_ctx);
                }
            }
            Event::WindowEvent { event, window_id } => {
                if window.is_none() || window.as_ref().unwrap().id() != window_id {
                    return;
                }
                match event {
                    WindowEvent::CloseRequested => {
                        // ── Clean shutdown: wait for GPU, destroy resources ──
                        if let Some(sp) = prev_sync_point.take() {
                            let _ = render_ctx.context.wait_for(&sp, !0);
                        }
                        if let Some(mut s) = surface.take() {
                            if let Some(mut gp) = gui_painter.take() {
                                gp.destroy(&render_ctx.context);
                            }
                            if let Some(mut mr) = map_renderer.take() {
                                mr.destroy(&render_ctx);
                            }
                            render_ctx.context.destroy_command_encoder(&mut render_ctx.command_encoder);
                            render_ctx.context.destroy_surface(&mut s);
                        }
                        elwt.exit()
                    }
                    WindowEvent::Resized(physical_size) => {
                        if physical_size.width > 0 && physical_size.height > 0 {
                            if let Some(sp) = prev_sync_point.take() {
                                let _ = render_ctx.context.wait_for(&sp, !0);
                            }
                            if let Some(ref mut s) = surface {
                                render_ctx.context.reconfigure_surface(s, gpu::SurfaceConfig {
                                    size: gpu::Extent {
                                        width: physical_size.width,
                                        height: physical_size.height,
                                        depth: 1,
                                    },
                                    usage: gpu::TextureUsage::TARGET,
                                    display_sync: gpu::DisplaySync::Recent,
                                    ..Default::default()
                                });
                            }
                            screen_w = physical_size.width as f32;
                            screen_h = physical_size.height as f32;
                            let zmax = camera_zoom_upper_bound(screen_w, screen_h);
                            camera_zoom = camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);
                            raw_input.screen_rect = Some(Rect::from_min_size(
                                Pos2::ZERO,
                                Vec2::new(screen_w, screen_h)
                            ));
                            if let Some(win) = window.as_ref() {
                                win.request_redraw();
                            }
                        }
                    }
                    WindowEvent::KeyboardInput { event, .. } => {
                        if event.state == ElementState::Pressed {
                            if let winit::keyboard::Key::Character(text) = &event.logical_key {
                                raw_input.events.push(egui::Event::Text(text.to_string()));
                            } else if let winit::keyboard::Key::Named(named) = &event.logical_key {
                                if *named == winit::keyboard::NamedKey::Backspace {
                                    raw_input.events.push(egui::Event::Key {
                                        key: egui::Key::Backspace,
                                        physical_key: None,
                                        pressed: true,
                                        repeat: false,
                                        modifiers: Default::default(),
                                    });
                                }
                            }
                        }
                    }
                    WindowEvent::Ime(ime) => {
                        use winit::event::Ime;
                        match ime {
                            Ime::Enabled | Ime::Disabled => {}
                            Ime::Preedit(text, _) => {
                                raw_input
                                    .events
                                    .push(egui::Event::Ime(egui::ImeEvent::Preedit(text.clone())));
                            }
                            Ime::Commit(text) => {
                                raw_input
                                    .events
                                    .push(egui::Event::Ime(egui::ImeEvent::Commit(text.clone())));
                            }
                        }
                    }
                    WindowEvent::MouseInput { state: btn_state, button, .. } => {
                        let pressed = btn_state == ElementState::Pressed;
                        if button == MouseButton::Left {
                            dragging = pressed;

                            // If it's a click (pressed) and not intercepted by egui UI
                            if pressed && !egui_ctx.egui_wants_pointer_input() && app.phase == ClientPhase::Playing && app.hud_state.sync_state.is_none() {
                                // Project mouse to map tile!
                                let world_x = (last_mouse_x as f32 - camera_x) / camera_zoom;
                                let world_y = (last_mouse_y as f32 - camera_y) / camera_zoom;
                                
                                let col = world_x.floor() as i32;
                                let row = world_y.floor() as i32;

                                if col >= 0 && row >= 0 && col < map_w as i32 && row < map_h as i32 {
                                    let phase = current_snapshot.as_ref().map(|s| &s.phase).unwrap_or(&sow_core::game::GamePhase::Lobby);
                                    let intent = if matches!(phase, sow_core::game::GamePhase::Spawning { .. }) {
                                        sow_core::protocol::GameplayIntent::Spawn { x: col as u32, y: row as u32 }
                                    } else {
                                        let owner = map_renderer.as_ref().map(|mr| (mr.cached_pixels[(row * map_w as i32 + col) as usize] & 0xFFFF) as u16).unwrap_or(0);
                                        let attack = sow_core::protocol::AttackIntent {
                                            target_owner: owner,
                                            troops: Some(app.hud_state.troops * (app.hud_state.attack_ratio as f64)),
                                        };
                                        sow_core::protocol::GameplayIntent::Attack(attack)
                                    };
                                    
                                    if let Some(c) = net_client.as_ref() {
                                        // Multiplayer: send intent to server
                                        let msg = sow_core::protocol::ClientMessage::Gameplay {
                                            intent: intent.clone(),
                                        };
                                        if let Ok(json) = bincode::serialize(&msg) {
                                            c.send(json);
                                        }
                                    } else {
                                        // Singleplayer: apply directly
                                        let stamped = sow_core::protocol::StampedIntent {
                                            player_id: my_player_id.unwrap_or(1),
                                            intent,
                                        };
                                        bridge.send_command(SimCommand::Turn(sow_core::protocol::Turn { turn_number: 0, intents: vec![stamped] }));
                                    }
                                }
                            }
                        }

                        raw_input.events.push(egui::Event::PointerButton {
                            pos: Pos2::new(last_mouse_x as f32, last_mouse_y as f32),
                            button: match button {
                                MouseButton::Left => egui::PointerButton::Primary,
                                MouseButton::Right => egui::PointerButton::Secondary,
                                MouseButton::Middle => egui::PointerButton::Middle,
                                _ => egui::PointerButton::Primary,
                            },
                            pressed,
                            modifiers: Default::default(),
                        });
                    }
                    WindowEvent::CursorMoved { position, .. } => {
                        if dragging {
                            let dx = position.x - last_mouse_x;
                            let dy = position.y - last_mouse_y;
                            camera_x += dx as f32;
                            camera_y += dy as f32;
                        }
                        last_mouse_x = position.x;
                        last_mouse_y = position.y;
                        raw_input.events.push(egui::Event::PointerMoved(Pos2::new(last_mouse_x as f32, last_mouse_y as f32)));
                    }
                    
                    WindowEvent::Touch(touch) => {
                        use winit::event::TouchPhase;
                        let pressed = touch.phase == TouchPhase::Started;
                        let released = touch.phase == TouchPhase::Ended || touch.phase == TouchPhase::Cancelled;

                        if pressed || touch.phase == TouchPhase::Moved {
                            active_touches.insert(touch.id, (touch.location.x, touch.location.y));
                        }
                        if released {
                            active_touches.remove(&touch.id);
                        }

                        if active_touches.len() >= 2 {
                            dragging = false; // Cancel map drag while pinching
                            let mut it = active_touches.values();
                            let p1 = *it.next().unwrap();
                            let p2 = *it.next().unwrap();
                            let dx = p1.0 - p2.0;
                            let dy = p1.1 - p2.1;
                            let distance = (dx * dx + dy * dy).sqrt();

                            if let Some(last_dist) = last_pinch_distance {
                                let delta = distance - last_dist;
                                let old_zoom = camera_zoom;
                                camera_zoom *= 1.0 + (delta as f32 * 0.005);
                                let zmax = camera_zoom_upper_bound(screen_w, screen_h);
                                camera_zoom = camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);

                                let pinch_cx = (p1.0 + p2.0) / 2.0;
                                let pinch_cy = (p1.1 + p2.1) / 2.0;
                                let map_x = (pinch_cx as f32 - camera_x) / old_zoom;
                                let map_y = (pinch_cy as f32 - camera_y) / old_zoom;
                                camera_x = pinch_cx as f32 - map_x * camera_zoom;
                                camera_y = pinch_cy as f32 - map_y * camera_zoom;
                            }
                            last_pinch_distance = Some(distance);
                        } else {
                            last_pinch_distance = None;
                            
                            if active_touches.len() == 1 {
                                if pressed {
                                    last_mouse_x = touch.location.x;
                                    last_mouse_y = touch.location.y;
                                    dragging = true;
                                    raw_input.events.push(egui::Event::PointerMoved(Pos2::new(last_mouse_x as f32, last_mouse_y as f32)));
                                    raw_input.events.push(egui::Event::PointerButton {
                                        pos: Pos2::new(last_mouse_x as f32, last_mouse_y as f32),
                                        button: egui::PointerButton::Primary,
                                        pressed: true,
                                        modifiers: Default::default(),
                                    });
                                } else if touch.phase == TouchPhase::Moved {
                                    let dx = touch.location.x - last_mouse_x;
                                    let dy = touch.location.y - last_mouse_y;
                                    if dragging && !egui_ctx.egui_wants_pointer_input() {
                                        camera_x += dx as f32;
                                        camera_y += dy as f32;
                                    }
                                    last_mouse_x = touch.location.x;
                                    last_mouse_y = touch.location.y;
                                    raw_input.events.push(egui::Event::PointerMoved(Pos2::new(last_mouse_x as f32, last_mouse_y as f32)));
                                }
                            }
                            
                            if released {
                                dragging = false;
                                raw_input.events.push(egui::Event::PointerButton {
                                    pos: Pos2::new(last_mouse_x as f32, last_mouse_y as f32),
                                    button: egui::PointerButton::Primary,
                                    pressed: false,
                                    modifiers: Default::default(),
                                });

                                if !egui_ctx.egui_wants_pointer_input() && app.phase == ClientPhase::Playing && app.hud_state.sync_state.is_none() {
                                    let world_x = (last_mouse_x as f32 - camera_x) / camera_zoom;
                                    let world_y = (last_mouse_y as f32 - camera_y) / camera_zoom;
                                    
                                    let col = world_x.floor() as i32;
                                    let row = world_y.floor() as i32;

                                    if col >= 0 && row >= 0 && col < map_w as i32 && row < map_h as i32 {
                                        let phase = current_snapshot.as_ref().map(|s| &s.phase).unwrap_or(&sow_core::game::GamePhase::Lobby);
                                        let intent = if matches!(phase, sow_core::game::GamePhase::Spawning { .. }) {
                                            sow_core::protocol::GameplayIntent::Spawn { x: col as u32, y: row as u32 }
                                        } else {
                                            let owner = map_renderer.as_ref().map(|mr| (mr.cached_pixels[(row * map_w as i32 + col) as usize] & 0xFFFF) as u16).unwrap_or(0);
                                            let attack = sow_core::protocol::AttackIntent {
                                                target_owner: owner,
                                                troops: Some(app.hud_state.troops * (app.hud_state.attack_ratio as f64)),
                                            };
                                            sow_core::protocol::GameplayIntent::Attack(attack)
                                        };

                                        if let Some(c) = net_client.as_ref() {
                                            let msg = sow_core::protocol::ClientMessage::Gameplay { intent: intent.clone() };
                                            if let Ok(json) = bincode::serialize(&msg) {
                                                c.send(json);
                                            }
                                        } else {
                                            let stamped = sow_core::protocol::StampedIntent { player_id: my_player_id.unwrap_or(1), intent };
                                            bridge.send_command(SimCommand::Turn(sow_core::protocol::Turn { turn_number: 0, intents: vec![stamped] }));
                                        }
                                    }
                                }
                            }
                        }
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        let scroll = match delta {
                            MouseScrollDelta::LineDelta(x, y) => {
                                if y.abs() >= x.abs() {
                                    y
                                } else {
                                    x
                                }
                            }
                            MouseScrollDelta::PixelDelta(pos) => {
                                let x = pos.x as f32 / 50.0;
                                let y = pos.y as f32 / 50.0;
                                if y.abs() >= x.abs() {
                                    y
                                } else {
                                    x
                                }
                            }
                        };
                        let old_zoom = camera_zoom;
                        camera_zoom *= 1.0 + scroll * 0.15;
                        let zmax = camera_zoom_upper_bound(screen_w, screen_h);
                        camera_zoom = camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);

                        // Zoom towards cursor
                        let factor = camera_zoom / old_zoom;
                        camera_x = last_mouse_x as f32 - factor * (last_mouse_x as f32 - camera_x);
                        camera_y = last_mouse_y as f32 - factor * (last_mouse_y as f32 - camera_y);
                    }
                    WindowEvent::RedrawRequested => {
                        #[cfg(target_arch = "wasm32")]
                        if let Some(win) = window.as_ref() {
                            let web_win = web_sys::window().unwrap();
                            let w = web_win.inner_width().unwrap().as_f64().unwrap();
                            let h = web_win.inner_height().unwrap().as_f64().unwrap();
                            
                            // Use the logical size and sf to calculate current physical size
                            let sf = win.scale_factor();
                            let expected_w = (w * sf) as u32;
                            let expected_h = (h * sf) as u32;
                            
                            if expected_w.abs_diff(screen_w as u32) > 1 || expected_h.abs_diff(screen_h as u32) > 1 {
                                let _ = win.request_inner_size(winit::dpi::LogicalSize::new(w, h));
                            }
                        }

                        if let Some(ref mut s) = surface {
                            if let Some(win) = window.as_ref() {
                                win.pre_present_notify();
                            }
                            let frame = s.acquire_frame();

                            if let Some(sp) = prev_sync_point.take() {
                                let _ = render_ctx.context.wait_for(&sp, !0);
                            }

                            render_ctx.command_encoder.start();
                            render_ctx.command_encoder.init_texture(frame.texture());

                            if let Some(ref mut mr) = map_renderer {
                                // Upload map state on first frame or after each tick
                                if needs_first_upload {
                                    render_ctx.command_encoder.init_texture(mr.texture);
                                    needs_first_upload = false;
                                }
                                mr.update(&mut render_ctx.command_encoder, &render_ctx.context, &current_snapshot.as_ref().map(|s| &s.dirty_tiles).unwrap_or(&vec![]));

                                let globals = MapGlobals {
                                    camera_pos: [camera_x, camera_y],
                                    zoom: camera_zoom,
                                    time: start_time.elapsed().as_secs_f32(),
                                    screen_size: [screen_w, screen_h],
                                    map_size: [map_w as f32, map_h as f32],
                                };
                                mr.draw(&mut render_ctx.command_encoder, frame.texture_view(), globals);
                            }

                            // ── UI UPDATE ───────────────────────────────────────
                            let mut sf = window.as_ref().map_or(1.0, |w| w.scale_factor() as f32);
                            if cfg!(any(target_os = "android", target_os = "ios"))
                                && sf < 1.5
                                && screen_h > 800.0
                            {
                                sf = 2.0; // Force higher scale on dense mobile displays if OS reports 1.0
                            }
                            
                            egui_ctx.set_pixels_per_point(sf);
                            raw_input.screen_rect = Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::Vec2::new(screen_w / sf, screen_h / sf)
                            ));
                            
                            for ev in &mut raw_input.events {
                                match ev {
                                    egui::Event::PointerMoved(pos) | egui::Event::PointerButton { pos, .. } => {
                                        pos.x /= sf;
                                        pos.y /= sf;
                                    }
                                    _ => {}
                                }
                            }
                            
                            let frame_now = Instant::now();
                            let dt = frame_now.duration_since(last_frame_time).as_secs_f32();
                            last_frame_time = frame_now;
                            raw_input.predicted_dt = dt.min(0.1);
                            
                            if app.main_menu_state.is_waiting && app.main_menu_state.wait_timer_secs > 0.0 {
                                app.main_menu_state.wait_timer_secs = (app.main_menu_state.wait_timer_secs - raw_input.predicted_dt).max(0.0);
                            }
                            if let Some(ref mut secs) = app.hud_state.spawn_timer_secs {
                                *secs = (*secs - raw_input.predicted_dt).max(0.0);
                            }
                            if let Some(ref mut sync) = app.hud_state.sync_state {
                                sync.time_remaining = (sync.time_remaining - raw_input.predicted_dt).max(0.0);
                            }
                            
                            let egui_output = egui_ctx.run_ui(raw_input.clone(), |ctx| {
                                if app.phase == ClientPhase::Playing {
                                    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("world_overlays")));
                                    let wall_secs = start_time.elapsed().as_secs_f64();

                                    // Configuration variables removed from GameConfig
                                    let dot_r = ClientVisualConfig::default().ui_lod_dot_radius;
                                    
                                    struct VisPlayer<'a> {
                                        player: &'a sow_core::protocol::PlayerSnapshot,
                                        center: egui::Pos2,
                                        pc: egui::Color32,
                                        sizing_presence: f32,
                                        lod_presence: f32,
                                    }
                                    let mut visible_players = Vec::new();
                                    if let Some(snap) = &current_snapshot {
                                        for player in &snap.players {
                                            if player.tile_count == 0 || !player.alive { continue; }
                                            
                                            let avg_col = player.centroid_x;
                                            let avg_row = player.centroid_y;
                                            
                                            let target_cx = avg_col + 0.5;
                                            let target_cy = avg_row + 0.5;
                                            
                                            // Smooth position interpolation
                                            let pos = label_positions.entry(player.id).or_insert((target_cx, target_cy));
                                            let dx = target_cx - pos.0;
                                            let dy = target_cy - pos.1;
                                            let dist = (dx * dx + dy * dy).sqrt();
                                            if dist > 50.0 {
                                                pos.0 = target_cx;
                                                pos.1 = target_cy;
                                            } else if dist > 0.1 {
                                                pos.0 += dx * 0.2;
                                                pos.1 += dy * 0.2;
                                            } else {
                                                pos.0 = target_cx;
                                                pos.1 = target_cy;
                                            }
                                            
                                            let screen_x = (pos.0 * camera_zoom + camera_x) / sf;
                                            let screen_y = (pos.1 * camera_zoom + camera_y) / sf;
                                            
                                            // Frustum cull
                                            if screen_x < -100.0 || screen_x > screen_w + 100.0 || screen_y < -100.0 || screen_y > screen_h + 100.0 { continue; }
                                            
                                            let center = egui::pos2(screen_x, screen_y);
                                            // Map shader derives human tint from id, not `player.color`; match that for dots + ★.
                                            let rgb = if player.player_type == sow_core::player::PlayerType::Human {
                                                sow_core::player::human_shader_territory_rgb(player.id)
                                            } else {
                                                player.color
                                            };
                                            let pc = nameplate_matte_player_rgb(rgb);
                                            
                                            // `lod_presence` uses zoom (when zoomed out, dots only). `sizing_presence`
                                            // does not, so nameplate font sizes stay stable and egui's glyph atlas is not
                                            // invalidated every scroll step (fixes garbled glyphs). Font size is rounded
                                            // to whole points for fewer distinct `FontId`s.
                                            let importance = (player.tile_count as f32).sqrt().max(1.0);
                                            let lod_presence = importance * (camera_zoom / sf);
                                            let sizing_presence = importance * (NAMEPLATE_REFERENCE_ZOOM / sf);

                                            visible_players.push(VisPlayer {
                                                player, center, pc, sizing_presence, lod_presence
                                            });
                                        }
                                    }

                                    visible_players.sort_unstable_by(|a, b| {
                                        let a_is_human = a.player.player_type == sow_core::player::PlayerType::Human;
                                        let b_is_human = b.player.player_type == sow_core::player::PlayerType::Human;
                                        if a_is_human != b_is_human {
                                            return b_is_human.cmp(&a_is_human); // true > false
                                        }
                                        
                                        let a_is_nation = a.player.id < 200;
                                        let b_is_nation = b.player.id < 200;
                                        if a_is_nation != b_is_nation {
                                            return b_is_nation.cmp(&a_is_nation); // true > false
                                        }
                                        
                                        b.lod_presence.partial_cmp(&a.lod_presence).unwrap_or(std::cmp::Ordering::Equal)
                                    });

                                    let mut full_labels_drawn = 0;

                                    for vp in visible_players {
                                        let player = vp.player;
                                        let center = vp.center;
                                        let pc = vp.pc;
                                        let sizing_presence = vp.sizing_presence;
                                        let lod_presence = vp.lod_presence;

                                        // Small nations require zooming in to appear.
                                        let threshold = if player.id >= 200 {
                                            24.0 // Tribes need to be much closer/bigger to show text
                                        } else {
                                            8.0 // Nations can show text further away
                                        };
                                        let show_full = lod_presence >= threshold && full_labels_drawn < 100;

                                        if show_full {
                                            full_labels_drawn += 1;
                                            let ui_text_scale = ClientVisualConfig::default().ui_text_scale;

                                            // 1. Bounding box for font fitting (reference zoom, not current zoom)
                                            let empire_width_px = sizing_presence * 2.5; // Hexagons spread out
                                            let empire_height_px = sizing_presence * 1.5;

                                            // 2. Constrain font size so the text fits INSIDE those pixels
                                            let name_len = player.name.len().max(1) as f32;
                                            let max_by_width = empire_width_px / (name_len * 0.6); // Avg char width is ~60% of height
                                            let max_by_height = empire_height_px / 2.5; // Need space for 2 lines of text (name + troops)

                                            // 3. Raw font size that inscribes the territory at reference zoom
                                            let raw_font_size = max_by_width.min(max_by_height);

                                            // 4. Integer pt sizes → stable galley cache, stable atlas entries
                                            let target_font_size = raw_font_size * ui_text_scale;
                                            // Quantize to 2pt steps so float jitter does not rebuild galleys every frame.
                                            let font_size = (((target_font_size.round() as i32).clamp(14, 64) + 1) / 2 * 2) as f32;

                                            let is_human = player.player_type == sow_core::player::PlayerType::Human;
                                            let troops_for_label = troop_label_throttle
                                                .displayed_troops(wall_secs, player.id, player.troops);
                                            let new_troops_str = render_troops(troops_for_label);
                                            let cache_entry = nameplate_cache.entry(player.id).or_insert_with(|| {
                                                let font_id = egui::FontId::proportional(font_size);
                                                let troops_str = new_troops_str.clone();
                                                
                                                CachedNameplate {
                                                    name_galley: layout_nameplate_name_galley(
                                                        &painter,
                                                        font_id.clone(),
                                                        &player.name,
                                                        is_human,
                                                        pc,
                                                    ),
                                                    troops_galley: painter.layout_no_wrap(format!("⚔ {}", troops_str), font_id, NAMEPLATE_FILL),
                                                    last_formatted_troops: troops_str,
                                                    last_font_size: font_size,
                                                }
                                            });
                                            
                                            if cache_entry.last_font_size != font_size {
                                                let font_id = egui::FontId::proportional(font_size);
                                                cache_entry.name_galley = layout_nameplate_name_galley(
                                                    &painter,
                                                    font_id.clone(),
                                                    &player.name,
                                                    is_human,
                                                    pc,
                                                );
                                                cache_entry.troops_galley = crate::nameplates::layout_nameplate_troops_galley(
                                                    &painter,
                                                    font_id,
                                                    &new_troops_str,
                                                );
                                                cache_entry.last_formatted_troops = new_troops_str.clone();
                                                cache_entry.last_font_size = font_size;
                                            } else if new_troops_str != cache_entry.last_formatted_troops {
                                                let font_id = egui::FontId::proportional(font_size);
                                                cache_entry.troops_galley = crate::nameplates::layout_nameplate_troops_galley(
                                                    &painter,
                                                    font_id,
                                                    &new_troops_str,
                                                );
                                                cache_entry.last_formatted_troops = new_troops_str;
                                            }
                                            
                                            let name_galley = &cache_entry.name_galley;
                                            let troops_galley = &cache_entry.troops_galley;
                                            
                                            let h = name_galley.rect.height() + troops_galley.rect.height() + 2.0;
                                            
                                            let name_pos = egui::pos2(center.x - name_galley.rect.width() / 2.0, center.y - h / 2.0);
                                            let troops_pos = egui::pos2(center.x - troops_galley.rect.width() / 2.0, center.y - h / 2.0 + name_galley.rect.height() + 2.0);
                                            crate::nameplates::paint_nameplate_galley(&painter, name_pos, name_galley.clone());
                                            crate::nameplates::paint_nameplate_galley(&painter, troops_pos, troops_galley.clone());
                                        } else {
                                            // Dot only — zero text layout, bare metal fast
                                            painter.circle_filled(center, dot_r, pc);
                                            painter.circle_stroke(center, dot_r, egui::Stroke::new(1.0_f32, egui::Color32::from_black_alpha(180)));
                                        }
                                    }
                                }
                                frame_count += 1;
                                if last_fps_time.elapsed().as_secs_f64() >= 1.0 {
                                    current_fps = frame_count;
                                    frame_count = 0;
                                    last_fps_time = Instant::now();
                                }

                                if last_ping_time.elapsed().as_secs_f64() >= 1.0 {
                                    if let Some(c) = net_client.as_ref() {
                                        let ping_msg = sow_core::protocol::ClientMessage::Ping {
                                            client_time: start_time.elapsed().as_secs_f64(),
                                        };
                                        if let Ok(json) = bincode::serialize(&ping_msg) {
                                            c.send(json);
                                        }
                                    }
                                    last_ping_time = Instant::now();
                                }
                                
                                if app.phase == ClientPhase::Playing {
                                    egui::Area::new(egui::Id::new("fps_counter"))
                                        .anchor(egui::Align2::LEFT_TOP, egui::vec2(10.0, 10.0))
                                        .show(ctx, |ui| {
                                            ui.horizontal(|ui| {
                                                if let Some(ping) = current_ping_ms {
                                                    ui.label(
                                                        egui::RichText::new(format!("Ping: {}ms", ping))
                                                            .color(egui::Color32::WHITE)
                                                            .strong()
                                                    );
                                                }
                                                ui.label(
                                                    egui::RichText::new(format!("FPS: {}", current_fps))
                                                        .color(egui::Color32::YELLOW)
                                                        .strong()
                                                );
                                            });
                                        });
                                }

                                if let Some(action) = app.draw(ctx) {
                                    match action {
                                        UiAction::StartSinglePlayer => {
                                            app.phase = ClientPhase::Playing;
                                        }
                                        UiAction::ConnectToServer(addr) => {
                                            app.main_menu_state.is_connecting = true;
                                            let url = addr.clone();
                                            #[cfg(target_arch = "wasm32")]
                                            spawn_sow_client_connect(url, &connect_tx);
                                            #[cfg(not(target_arch = "wasm32"))]
                                            spawn_sow_client_connect(url, &connect_tx, &tokio_rt);
                                        }
                                        UiAction::JoinLobby(id) => {
                                            let join_msg = sow_core::protocol::ClientMessage::Join {
                                                name: app.main_menu_state.player_name.clone(),
                                                is_observer: false,
                                                target_lobby_id: Some(id),
                                                build_version: get_build_version(),
                                            };
                                            app.main_menu_state.pending_join_lobby_id = Some(id);
                                            if let Ok(json) = bincode::serialize(&join_msg) {
                                                if let Some(c) = net_client.as_ref() {
                                                    c.send(json);
                                                }
                                            }
                                            app.main_menu_state.is_waiting = true;
                                        }
                                        UiAction::LeaveLobby => {
                                            if let Some(c) = net_client.as_ref() {
                                                let leave = sow_core::protocol::ClientMessage::Leave {};
                                                if let Ok(json) = bincode::serialize(&leave) {
                                                    c.send(json);
                                                }
                                            }
                                            app.main_menu_state.is_waiting = false;
                                            app.main_menu_state.pending_join_lobby_id = None;
                                            app.main_menu_state.joined_lobby_id = None;
                                            my_lobby_id = None;
                                            my_player_id = None;
                                            camera_x = 0.0;
                                            camera_y = 0.0;
                                            camera_zoom = 2.0;
                                            app.phase = ClientPhase::Splash;
                                            app.splash_state.job = sow_ui::ui::loading_screen::SplashJob::ExitGame;
                                            app.splash_state.frames_drawn = 0;
                                        }
                                        UiAction::SetAttackRatio(r) => {
                                            app.hud_state.attack_ratio = r;
                                        }
                                        UiAction::CenterCamera => {
                                            let pid = my_player_id.unwrap_or(1);
                                            if let Some(player) =
                                                current_snapshot.as_ref().and_then(|s| s.players.iter().find(|p| p.id == pid))
                                            {
                                                if player.tile_count > 0 && player.alive {
                                                    let cx = player.centroid_x;
                                                    let cy = player.centroid_y;
                                                    
                                                    let world_cx = cx + 0.5;
                                                    let world_cy = cy + 0.5;

                                                    camera_x = screen_w * 0.5 - world_cx * camera_zoom;
                                                    camera_y = screen_h * 0.5 - world_cy * camera_zoom;
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }

                                // The new nameplates are rendered before app.draw()
                            });

                            if let Some(win) = window.as_ref() {
                                let ime_opt = egui_output.platform_output.ime;
                                let allow_ime = ime_opt.is_some();
                                let toggling = ime_allowed_state != allow_ime;
                                if toggling {
                                    ime_allowed_state = allow_ime;
                                    win.set_ime_allowed(allow_ime);
                                }
                                if let Some(ime_out) = ime_opt {
                                    let ppp = egui_output.pixels_per_point;
                                    let ime_rect_px = ppp * ime_out.rect;
                                    let had_input_events = !raw_input.events.is_empty();
                                    if ime_cursor_rect_px != Some(ime_rect_px) || had_input_events {
                                        ime_cursor_rect_px = Some(ime_rect_px);
                                        win.set_ime_cursor_area(
                                            winit::dpi::PhysicalPosition::new(
                                                ime_rect_px.min.x.round() as i32,
                                                ime_rect_px.min.y.round() as i32,
                                            ),
                                            winit::dpi::PhysicalSize::new(
                                                ime_rect_px.width().round().max(1.0) as u32,
                                                ime_rect_px.height().round().max(1.0) as u32,
                                            ),
                                        );
                                    }
                                } else {
                                    ime_cursor_rect_px = None;
                                }
                            }

                            raw_input.events.clear();

                            // ── DRAWING UI ──────────────────────────────────────────
                            if let Some(ref mut gp) = gui_painter {
                                let screen_desc = blade_egui::ScreenDescriptor {
                                    physical_size: (screen_w as u32, screen_h as u32),
                                    scale_factor: sf,
                                };
                                let paint_jobs = egui_ctx.tessellate(egui_output.shapes, sf);
                                gp.update_textures(
                                    &mut render_ctx.command_encoder,
                                    &egui_output.textures_delta,
                                    &render_ctx.context,
                                );

                                let mut pass = render_ctx.command_encoder.render("ui_pass", gpu::RenderTargetSet {
                                    colors: &[gpu::RenderTarget {
                                        view: frame.texture_view(),
                                        init_op: gpu::InitOp::Load,
                                        finish_op: gpu::FinishOp::Store,
                                    }],
                                    depth_stencil: None,
                                });

                                gp.paint(&mut pass, &paint_jobs, &screen_desc, &render_ctx.context);
                                drop(pass);
                            }
                            if let Some(ref mut gp) = gui_painter {
                                gp.sync(&render_ctx.context);
                            }
                            render_ctx.command_encoder.present(frame);
                            let sync_point = render_ctx.context.submit(&mut render_ctx.command_encoder);
                            
                            if let Some(ref mut gp) = gui_painter {
                                gp.after_submit(&sync_point);
                            }
                            
                            prev_sync_point = Some(sync_point);
                        }
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => {
                let now = Instant::now();

                #[cfg(target_arch = "wasm32")]
                {
                    let doc_visible = web_sys::window()
                        .and_then(|w| w.document())
                        .map(|d| d.visibility_state() == web_sys::VisibilityState::Visible)
                        .unwrap_or(true);
                    if doc_visible && !wasm_doc_was_visible {
                        ws_reconnect_after_resume = true;
                    }
                    wasm_doc_was_visible = doc_visible;
                }

                if ws_reconnect_after_resume {
                    ws_reconnect_after_resume = false;
                    ws_connect_not_before = ws_connect_not_before.min(now);
                }
                // No fake map download simulation! Progress is real!

                while let Ok(res) = connect_rx.try_recv() {
                    match res {
                        Ok(client) => {
                            log::info!("Connected to server!");
                            net_client = Some(client);
                            app.main_menu_state.is_connected = true;
                            app.main_menu_state.is_connecting = false;
                            ws_connect_fail_backoff_ms = 400;
                        }
                        Err(e) => {
                            log::error!("Failed to connect: {}", e);
                            app.main_menu_state.is_connected = false;
                            app.main_menu_state.is_connecting = false;
                            ws_connect_fail_backoff_ms =
                                (ws_connect_fail_backoff_ms.saturating_mul(2)).min(30_000);
                            ws_connect_not_before =
                                now + Duration::from_millis(ws_connect_fail_backoff_ms);
                        }
                    }
                }

                let mut ws_disconnected = false;
                #[cfg(target_arch = "wasm32")]
                if let Some(c) = net_client.as_ref() {
                    if c.is_socket_closed() {
                        ws_disconnected = true;
                    }
                }

                // Process network messages
                if let Some(c) = net_client.as_ref() {
                    if !ws_disconnected {
                        loop {
                            let msg = match c.rx.try_recv() {
                                Ok(msg) => msg,
                                Err(std::sync::mpsc::TryRecvError::Empty) => break,
                                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                    ws_disconnected = true;
                                    break;
                                }
                            };

                        use sow_core::protocol::ServerMessage;
                        let server_msg = match bincode::deserialize::<ServerMessage>(&msg) {
                            Ok(m) => m,
                            Err(e) => {
                                log::warn!("[NET] Failed to deserialize server message ({} bytes): {}", msg.len(), e);
                                continue;
                            }
                        };

                        match server_msg {
                            ServerMessage::Start(start_msg) => {
                                log::info!("Received ServerStartMessage; entering Splash phase immediately");
                                app.phase = sow_ui::app::ClientPhase::Splash;
                                app.splash_state.job = sow_ui::ui::loading_screen::SplashJob::EnterGame;
                                app.splash_state.frames_drawn = 0;
                                app.main_menu_state.is_waiting = false;
                                app.main_menu_state.pending_join_lobby_id = None;
                                app.main_menu_state.joined_lobby_id = None;
                                my_player_id = start_msg.my_player_id;
                                engine_init_queued_msg = Some(*start_msg);
                            }
                            ServerMessage::Turn(turn_msg) => {
                                turn_queue.push_back(turn_msg.turn);
                                app.hud_state.sync_state = None;
                            }
                            ServerMessage::SyncState(sync_msg) => {
                                app.hud_state.sync_state = Some(sync_msg);
                            }
                            ServerMessage::Pong { client_time } => {
                                let rtt = start_time.elapsed().as_secs_f64() - client_time;
                                current_ping_ms = Some((rtt * 1000.0) as u32);
                            }
                            ServerMessage::LobbiesBroadcast(broadcast) => {

                                app.main_menu_state.lobbies = broadcast.lobbies.clone();

                                let maps_base = get_maps_url();
                                let (thumbs_to_fetch, maps_to_fetch) = app.asset_loader.get_assets_to_fetch(&app.main_menu_state.lobbies);
                                
                                for map_name in thumbs_to_fetch {
                                    let url = format!("{}/{}/thumbnail.webp", maps_base.trim_end_matches('/'), map_name);
                                    let tx = map_tx.clone();
                                    let map_name_for_closure = map_name.clone();
                                    let request = ehttp::Request::get(&url);
                                    ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
                                        if let Ok(res) = result {
                                            if res.ok {
                                                let _ = tx.send(MapDownloadEvent::ThumbnailReady(map_name_for_closure, res.bytes));
                                            }
                                        }
                                    });
                                }
                                
                                for map_name in maps_to_fetch {
                                    let url = format!("{}/{}/map.bin.br", maps_base.trim_end_matches('/'), map_name);
                                    let tx = map_tx.clone();
                                    let map_name_for_closure = map_name.clone();
                                    let request = ehttp::Request::get(&url);
                                    let accumulated = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
                                    ehttp::streaming::fetch(request, move |result: ehttp::Result<ehttp::streaming::Part>| {
                                        match result {
                                            Ok(ehttp::streaming::Part::Response(res)) => {
                                                if !res.ok {
                                                    log::warn!("Prefetch failed for {}", map_name_for_closure);
                                                    return std::ops::ControlFlow::Break(());
                                                }
                                                std::ops::ControlFlow::Continue(())
                                            }
                                            Ok(ehttp::streaming::Part::Chunk(chunk)) => {
                                                if chunk.is_empty() {
                                                    let final_bytes = std::mem::take(&mut *accumulated.lock().unwrap());
                                                    let _ = tx.send(MapDownloadEvent::MapReady(map_name_for_closure.clone(), final_bytes));
                                                    return std::ops::ControlFlow::Break(());
                                                }
                                                accumulated.lock().unwrap().extend_from_slice(&chunk);
                                                std::ops::ControlFlow::Continue(())
                                            }
                                            Err(_) => std::ops::ControlFlow::Break(()),
                                        }
                                    });
                                }

                                if app.main_menu_state.is_waiting {
                                    let key = my_lobby_id
                                        .or(app.main_menu_state.joined_lobby_id)
                                        .or(app.main_menu_state.pending_join_lobby_id);
                                    if let Some(l_id) = key {
                                        if let Some(lobby) = broadcast.lobbies.iter().find(|l| l.id == l_id) {
                                            if lobby.is_counting_down {

                                                app.main_menu_state.wait_timer_secs = lobby.timer_secs;
                                            }
                                        }
                                    }
                                }
                            }
                            ServerMessage::LobbyClosed(closed) => {
                                log::warn!("Lobby {} closed: {}", closed.lobby_id, closed.reason);
                                app.hud_state.sync_state = None;
                                my_lobby_id = None;
                                my_player_id = None;

                                if closed.reason.contains("Requeueing") {
                                    log::info!("Auto-requeueing to a new lobby...");
                                    app.phase = ClientPhase::MainMenu;
                                    app.main_menu_state.is_waiting = true;
                                    let join_msg = sow_core::protocol::ClientMessage::Join {
                                        name: app.main_menu_state.player_name.clone(),
                                        is_observer: false,
                                        target_lobby_id: None,
                                        build_version: get_build_version(),
                                    };
                                    c.send(bincode::serialize(&join_msg).unwrap());
                                } else {
                                    app.phase = ClientPhase::Splash;
                                    app.splash_state.job = sow_ui::ui::loading_screen::SplashJob::ExitGame;
                                    app.splash_state.frames_drawn = 0;
                                    app.main_menu_state.is_waiting = false;
                                    app.main_menu_state.pending_join_lobby_id = None;
                                    app.main_menu_state.joined_lobby_id = None;
                                }
                            }
                            ServerMessage::JoinFailed(fail) => {
                                log::warn!("Join failed: {}", fail.reason);
                                if fail.reason == "VERSION_MISMATCH" {
                                    log::info!("Version mismatch — reloading...");
                                    #[cfg(target_arch = "wasm32")]
                                    if let Some(window) = web_sys::window() {
                                        let _ = window.location().reload();
                                    }
                                }
                                app.main_menu_state.is_waiting = false;
                                app.main_menu_state.pending_join_lobby_id = None;
                                app.main_menu_state.joined_lobby_id = None;
                            }
                            ServerMessage::JoinAck(ack) => {
                                log::info!("[LOBBY] Joined lobby {} as player {} (map: {})", ack.lobby_id, ack.player_id, ack.map_name);
                                my_lobby_id = Some(ack.lobby_id);
                                my_player_id = Some(ack.player_id);
                                app.main_menu_state.joined_lobby_id = Some(ack.lobby_id);
                                
                                let map_name = ack.map_name.clone();
                                app.main_menu_state.downloading_map_name = Some(map_name.clone());
                                
                                if let Some(texture) = app.asset_loader.thumbnail(&map_name) {
                                    app.splash_state.thumbnail = Some(texture.clone());
                                } else {
                                    app.splash_state.thumbnail = None;
                                }
                                
                                if app.asset_loader.has_map(&map_name) {
                                    log::info!("Map already cached, skipping download.");
                                    app.main_menu_state.cached_map = app.asset_loader.take_map(&map_name);
                                    app.main_menu_state.is_downloading_map = false;
                                    app.main_menu_state.map_download_progress = 100;
                                    c.send(bincode::serialize(&sow_core::protocol::ClientMessage::MapDownloadProgress {
                                        lobby_id: ack.lobby_id,
                                        player_id: ack.player_id,
                                        progress: 100,
                                    }).unwrap());
                                } else {
                                    let tx = map_tx.clone();
                                    app.main_menu_state.is_downloading_map = true;
                                    app.main_menu_state.cached_map = None;
                                    
                                    let maps_base = get_maps_url();
                                    let url = format!("{}/{}/map.bin.br", maps_base.trim_end_matches('/'), map_name);
                                    log::info!("Downloading map from: {}", url);
                                    
                                    c.send(bincode::serialize(&sow_core::protocol::ClientMessage::MapDownloadProgress {
                                        lobby_id: ack.lobby_id,
                                        player_id: ack.player_id,
                                        progress: 0,
                                    }).unwrap());
                                
                                    let request = ehttp::Request::get(&url);
                                    let map_name_for_closure = map_name.clone();
                                    let accumulated = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
                                    let total_bytes = std::sync::Arc::new(std::sync::Mutex::new(0usize));
                                    
                                    ehttp::streaming::fetch(request, move |result: ehttp::Result<ehttp::streaming::Part>| {
                                        match result {
                                            Ok(ehttp::streaming::Part::Response(res)) => {
                                                if !res.ok {
                                                    log::error!("Failed to fetch map, HTTP {}", res.status);
                                                    let _ = tx.send(MapDownloadEvent::Error(format!("HTTP Error: {}", res.status)));
                                                    return std::ops::ControlFlow::Break(());
                                                }
                                                log::info!("Server map response ok! headers: {:?}", res.headers);
                                                let cl = res.headers.get("content-length").or_else(|| res.headers.get("Content-Length"));
                                                if let Some(cl) = cl {
                                                    if let Ok(len) = cl.parse::<usize>() {
                                                        *total_bytes.lock().unwrap() = len;
                                                        log::info!("Map content-length parsed as: {}", len);
                                                    } else {
                                                        log::warn!("Failed to parse content-length: {}", cl);
                                                    }
                                                } else {
                                                    log::warn!("No content-length header received!");
                                                }
                                                std::ops::ControlFlow::Continue(())
                                            }
                                            Ok(ehttp::streaming::Part::Chunk(chunk)) => {
                                                if chunk.is_empty() {
                                                    let final_bytes = std::mem::take(&mut *accumulated.lock().unwrap());
                                                    log::info!("Map fully downloaded: {} bytes", final_bytes.len());
                                                    let _ = tx.send(MapDownloadEvent::MapReady(map_name_for_closure.clone(), final_bytes));
                                                    return std::ops::ControlFlow::Break(());
                                                }
                                                let mut acc = accumulated.lock().unwrap();
                                                acc.extend_from_slice(&chunk);
                                                let downloaded = acc.len();
                                                let total = *total_bytes.lock().unwrap();
                                                if total > 0 {
                                                    let progress = ((downloaded as f64 / total as f64) * 100.0) as u8;
                                                    let _ = tx.send(MapDownloadEvent::Progress(map_name_for_closure.clone(), progress.min(99)));
                                                    if downloaded % 524288 < chunk.len() {
                                                        log::info!("Downloading map... {} / {} bytes ({}%)", downloaded, total, progress.min(99));
                                                    }
                                                } else if downloaded % 524288 < chunk.len() {
                                                    log::info!("Downloading map... {} bytes (unknown total)", downloaded);
                                                }
                                                std::ops::ControlFlow::Continue(())
                                            }
                                            Err(err) => {
                                                log::error!("Failed to fetch map: {}", err);
                                                let _ = tx.send(MapDownloadEvent::Error(format!("Fetch error: {}", err)));
                                                std::ops::ControlFlow::Break(())
                                            }
                                        }
                                    });
                                }
                            }
                        }
                        } // end loop
                    } // end if !ws_disconnected
                } // end if let Some(c)
                
                if ws_disconnected {
                    log::warn!("WebSocket disconnected; will reconnect.");
                    net_client = None;
                    app.main_menu_state.is_connected = false;
                    app.main_menu_state.is_connecting = false;
                    ws_connect_not_before =
                        ws_connect_not_before.min(now + Duration::from_millis(200));
                        
                    // Recover: Send the user back to the loader
                    if app.phase != sow_ui::app::ClientPhase::Splash {
                        #[cfg(target_arch = "wasm32")]
                        {
                            if let Some(window) = web_sys::window() {
                                let _ = window.location().reload();
                            }
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        {
                            app.splash_state.job = sow_ui::ui::loading_screen::SplashJob::Reconnect;
                            app.splash_state.status_text = "Connection lost. Reconnecting...".to_string();
                            app.splash_state.progress = 0.0;
                            app.phase = sow_ui::app::ClientPhase::Splash;
                        }
                    }
                }

                #[cfg(target_arch = "wasm32")]
                let allow_ws_spawn = wasm_doc_was_visible;
                #[cfg(not(target_arch = "wasm32"))]
                let allow_ws_spawn = true;

                if allow_ws_spawn
                    && net_client.is_none()
                    && !app.main_menu_state.is_connecting
                    && now >= ws_connect_not_before
                {
                    app.main_menu_state.is_connecting = true;
                    let url = app.main_menu_state.server_address.clone();
                    #[cfg(target_arch = "wasm32")]
                    spawn_sow_client_connect(url, &connect_tx);
                    #[cfg(not(target_arch = "wasm32"))]
                    spawn_sow_client_connect(url, &connect_tx, &tokio_rt);
                }

                // Poll map download channel
                while let Ok(res) = map_rx.try_recv() {
                    match res {
                        MapDownloadEvent::Progress(downloaded_map_name, progress) => {
                            if Some(downloaded_map_name.clone()) == app.main_menu_state.downloading_map_name {
                                app.main_menu_state.map_download_progress = progress;
                                if let (Some(lid), Some(pid)) = (my_lobby_id, my_player_id) {
                                    if let Some(c) = net_client.as_ref() {
                                        c.send(bincode::serialize(&sow_core::protocol::ClientMessage::MapDownloadProgress {
                                            lobby_id: lid,
                                            player_id: pid,
                                            progress,
                                        }).unwrap());
                                    }
                                }
                            }
                        }
                        MapDownloadEvent::ThumbnailReady(map_name, bytes) => {
                            if let Ok(img) = image::load_from_memory(&bytes) {
                                let size = [img.width() as _, img.height() as _];
                                let image_buffer = img.to_rgba8();
                                let pixels = image_buffer.as_flat_samples();
                                let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
                                let texture = egui_ctx.load_texture(&map_name, color_image, egui::TextureOptions::LINEAR);
                                app.asset_loader.thumbnails.insert(map_name.clone(), texture);
                            } else {
                                log::warn!("Failed to decode thumbnail for {}", map_name);
                            }
                            app.asset_loader.thumbnails_in_flight.remove(&map_name);
                        }
                        MapDownloadEvent::MapReady(map_name, bytes) => {
                            let mut valid = true;
                            if let Some(expected_md5) = app.asset_loader.expected_md5s.get(&map_name) {
                                let digest = md5::compute(&bytes);
                                let actual_md5 = format!("{:x}", digest);
                                if actual_md5 != *expected_md5 {
                                    log::error!("MD5 mismatch for map {}: expected {}, got {}", map_name, expected_md5, actual_md5);
                                    valid = false;
                                } else {
                                    log::info!("MD5 verified for map {}", map_name);
                                }
                            }
                            
                            app.asset_loader.maps_in_flight.remove(&map_name);
                            if valid {
                                app.asset_loader.maps.insert(map_name.clone(), bytes.clone());
                            } else {
                                // Optionally handle failure, we just drop it so it can be retried later
                                continue;
                            }
                            
                            if Some(map_name.clone()) == app.main_menu_state.downloading_map_name {
                                log::info!("Map download completed successfully.");
                                app.main_menu_state.cached_map = Some(bytes);
                                app.main_menu_state.is_downloading_map = false;
                                app.main_menu_state.map_download_progress = 100;
                                
                                if let (Some(lid), Some(pid)) = (my_lobby_id, my_player_id) {
                                    if let Some(c) = net_client.as_ref() {
                                        c.send(bincode::serialize(&sow_core::protocol::ClientMessage::MapDownloadProgress {
                                            lobby_id: lid,
                                            player_id: pid,
                                            progress: 100,
                                        }).unwrap());
                                    }
                                }
                            }
                        }
                        MapDownloadEvent::Error(e) => {
                            log::error!("Map download aborted: {}", e);
                            app.main_menu_state.is_downloading_map = false;
                            // Optionally return to main menu or show error
                            app.phase = ClientPhase::MainMenu;
                            app.main_menu_state.is_waiting = false;
                            app.main_menu_state.pending_join_lobby_id = None;
                            app.main_menu_state.joined_lobby_id = None;
                        }
                    }
                }

                if let Some(start_msg) = engine_init_queued_msg.take() {
                    if app.main_menu_state.is_downloading_map {
                        engine_init_queued_msg = Some(start_msg);
                    } else {
                        log::info!("Map downloaded, computing heavy init in background");
                        
                        app.splash_state.status_text = "Computing terrain and water geometry...".to_string();
                        app.splash_state.progress = 0.1;

                        let cached_map = app.main_menu_state.cached_map.take();
                        let start_msg_clone = start_msg.clone();
                        let tx = engine_init_tx.clone();

                        let init_logic = move || {
                            let _ = tx.send(EngineInitEvent::Status("Decompressing map...".to_string()));
                            
                            let mut uncompressed_map = None;
                            if let Some(bytes) = cached_map {
                                let mut uncompressed = Vec::new();
                                let mut decompressor = brotli::Decompressor::new(bytes.as_slice(), 4096);
                                if std::io::Read::read_to_end(&mut decompressor, &mut uncompressed).is_ok() {
                                    uncompressed_map = Some(uncompressed);
                                } else {
                                    log::error!("Failed to decompress map.bin.br payload");
                                }
                            } else {
                                log::error!("Cached map data not found! Terrain will be empty.");
                            }
                            
                            let _ = tx.send(EngineInitEvent::Status("Computing terrain and water geometry...".to_string()));

                            let w = start_msg_clone.config.map_width;
                            let h = start_msg_clone.config.map_height;
                            let mut state = sow_core::game::GameState::new(
                                start_msg_clone.seed,
                                w,
                                h,
                                start_msg_clone.config.clone(),
                            );
                            
                            if let Some(bytes) = uncompressed_map {
                                if bytes.len() == state.map.terrain.len() {
                                    let dest_ptr = state.map.terrain.as_mut_ptr() as *mut u8;
                                    unsafe {
                                        std::ptr::copy_nonoverlapping(bytes.as_ptr(), dest_ptr, bytes.len());
                                    }
                                } else {
                                    log::error!("Map size mismatch! Expected {} bytes but decompressed {} bytes. Map will be randomly generated.", state.map.terrain.len(), bytes.len());
                                    for (i, &b) in bytes.iter().enumerate() {
                                        if i < state.map.terrain.len() {
                                            state.map.terrain[i] = sow_core::map::MapTile::from_byte(b);
                                        }
                                    }
                                }
                            }

                            let tx_prog = tx.clone();
                            let water = sow_core::water_components::WaterComponents::compute(&state.map, move |prog| {
                                let _ = tx_prog.send(EngineInitEvent::Progress(prog));
                            });
                            let _ = tx.send(EngineInitEvent::Complete(Box::new(state), water, Box::new(start_msg_clone)));
                        };

                        #[cfg(target_arch = "wasm32")]
                        init_logic();

                        #[cfg(not(target_arch = "wasm32"))]
                        std::thread::spawn(init_logic);

                        turn_queue.clear();
                        nameplate_cache.clear();
                        troop_label_throttle.clear();
                        needs_first_upload = true;
                    }
                }
                // Poll engine init channel
                if app.phase == sow_ui::app::ClientPhase::Splash {
                    match app.splash_state.job {
                        sow_ui::ui::loading_screen::SplashJob::Boot | sow_ui::ui::loading_screen::SplashJob::Reconnect => {
                            if app.main_menu_state.is_connected {
                                app.phase = ClientPhase::MainMenu;
                            } else {
                                app.splash_state.status_text = "Connecting to Server...".to_string();
                            }
                        }
                        sow_ui::ui::loading_screen::SplashJob::ExitGame => {
                            // Clean the engine state
                            let config = GameConfig::default();
                            bridge.send_command(SimCommand::Init {
                                config,
                                seed: 12345,
                                map_bytes: vec![],
                                players: vec![],
                            });
                            turn_queue.clear();
                            label_positions.clear();
                            nameplate_cache.clear();
                            troop_label_throttle.clear();
                            needs_first_upload = true;

                            
                            app.phase = ClientPhase::MainMenu;
                        }
                        sow_ui::ui::loading_screen::SplashJob::EnterGame => {
                            while let Ok(event) = engine_init_rx.try_recv() {
                                match event {
                                    EngineInitEvent::Status(msg) => {
                                        app.splash_state.status_text = msg;
                                    }
                                    EngineInitEvent::Progress(prog) => {
                                        app.splash_state.progress = prog;
                                    }
                                    EngineInitEvent::Complete(state, water, start_msg) => {
                                        log::info!("Engine initialization complete in background thread.");
                                        app.splash_state.status_text = "Uploading assets to GPU...".to_string();
                                        app.splash_state.progress = 1.0;
                                        app.splash_state.frames_drawn = 0; // Reset to ensure we draw the new text
                                        pending_engine_init_data = Some((*state, water, *start_msg));
                                    }
                                }
                            }
                        }
                    }

                    if app.splash_state.job == sow_ui::ui::loading_screen::SplashJob::EnterGame && pending_engine_init_data.is_some() && app.splash_state.frames_drawn > 1 {
                        // We have drawn the "Uploading assets to GPU..." screen, now block the main thread!
                        let (state, _, start_msg) = pending_engine_init_data.take().unwrap();
                            
                            let map_bytes: Vec<u8> = state.map.terrain.iter().map(|t| t.as_byte()).collect();
                            bridge.send_command(SimCommand::Init {
                                config: start_msg.config.clone(),
                                seed: start_msg.seed,
                                map_bytes: map_bytes.clone(),
                                players: start_msg.players.clone(),
                            });

                            map_w = start_msg.config.map_width;
                            map_h = start_msg.config.map_height;
                            if let Some(sp) = prev_sync_point.take() {
                                let _ = render_ctx.context.wait_for(&sp, !0);
                            }
                            if let Some(mut mr) = map_renderer.take() {
                                mr.destroy(&render_ctx);
                            }
                            if let Some(ref s) = surface {
                                render_ctx.command_encoder.start();
                                map_renderer = Some(sow_render::map_renderer::MapRenderer::new(&render_ctx.context, &mut render_ctx.command_encoder, map_w, map_h, s.info().format, &map_bytes));
                                let sync_point = render_ctx.context.submit(&mut render_ctx.command_encoder);
                                prev_sync_point = Some(sync_point);
                                
                                needs_first_upload = true;
    
                            }
                            
                            app.phase = sow_ui::app::ClientPhase::Playing;
                            if let Some(pid) = my_player_id {
                                if let Some(snap) = &current_snapshot {
                                    if let Some(player) = snap.players.iter().find(|p| p.id == pid) {
                                        if player.tile_count > 0 && player.alive {
                                            let cx = player.centroid_x;
                                            let cy = player.centroid_y;
                                            camera_zoom = 1.5;
                                            camera_x = screen_w * 0.5 - cx * camera_zoom;
                                            camera_y = screen_h * 0.5 - cy * camera_zoom;
                                        }
                                    }
                                }
                            }

                            if let Some(c) = net_client.as_ref() {
                                if let (Some(lid), Some(pid)) = (my_lobby_id, my_player_id) {
                                    let ready_msg = sow_core::protocol::ClientMessage::Ready { lobby_id: lid, player_id: pid };
                                    let json = bincode::serialize(&ready_msg).unwrap();
                                    c.send(json);
                                }
                            }
                        }
                    }
                app.hud_state.is_mobile = screen_w < 900.0;
                if let Some(snap) = &current_snapshot {
                    if let sow_core::game::GamePhase::Spawning { end_tick } = snap.phase {
                        let rem_ticks = end_tick.saturating_sub(snap.tick);
                        let target_secs = rem_ticks as f32 * 0.1; // assume 100ms
                        if let Some(ref mut current) = app.hud_state.spawn_timer_secs {
                            if (*current - target_secs).abs() > 0.3 {
                                *current = target_secs;
                            }
                        } else {
                            app.hud_state.spawn_timer_secs = Some(target_secs);
                        }
                    } else {
                        app.hud_state.spawn_timer_secs = None;
                    }
                } else {
                    app.hud_state.spawn_timer_secs = None;
                }
                if app.phase == sow_ui::app::ClientPhase::Playing {
                    if net_client.is_some() {
                        // Multiplayer: lockstep execution dictated by server
                        let mut ticks_processed = 0;
                        while let Some(turn) = turn_queue.pop_front() {
                            bridge.send_command(SimCommand::Turn(turn));
                            
                            // Update UI HUD State from my player id
                            if let Some(player) = current_snapshot.as_ref().and_then(|s| s.players.iter().find(|p| p.id == my_player_id.unwrap_or(1))) {
                                app.hud_state.gold = player.gold;
                                app.hud_state.troops = player.troops;
                                let owned_tiles = player.tile_count as f64;
                                app.hud_state.max_troops = owned_tiles * 50.0;
                            }

                            ticks_processed += 1;
                            if ticks_processed >= 10 {
                                break;
                            }
                        }
                        last_tick = now;
                    } else {
                        // Singleplayer: run freely based on local timer
                        if now.duration_since(last_tick) >= tick_interval {
                            bridge.send_command(SimCommand::Turn(sow_core::protocol::Turn { turn_number: 0, intents: vec![] }));

                            last_tick = now;
                            
                            if let Some(player) = current_snapshot.as_ref().and_then(|s| s.players.iter().find(|p| p.id == my_player_id.unwrap_or(1))) {
                                app.hud_state.gold = player.gold;
                                app.hud_state.troops = player.troops;
                                let owned_tiles = player.tile_count as f64;
                                app.hud_state.max_troops = owned_tiles * 50.0;
                            }
                        }
                    }
                } else {
                    last_tick = now;
                }
                if let Some(snap) = bridge.try_recv_snapshot() {
                    if let Some(mr) = &mut map_renderer {
                        mr.update(&mut render_ctx.command_encoder, &render_ctx.context, &snap.dirty_tiles);
                    }
                    current_snapshot = Some(snap);
                }
                
                if let Some(win) = window.as_ref() {
                    win.request_redraw();
                }
            }
            _ => {}
        }
    }).unwrap();
}


#[cfg(target_os = "android")]
#[no_mangle]
pub fn android_main(app: winit::platform::android::activity::AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;
    use winit::event_loop::EventLoopBuilder;

    // Redirect all crashes and logs to a physical file so we can see what's failing without ADB!
    if let Some(ext_path) = app.external_data_path() {
        let _ = std::fs::create_dir_all(&ext_path);
        let log_file = ext_path.join("sow_crash.txt");
        if let Ok(file) = std::fs::File::create(&log_file) {
            use std::os::unix::io::AsRawFd;
            let fd = file.as_raw_fd();
            unsafe {
                libc::dup2(fd, libc::STDERR_FILENO);
                libc::dup2(fd, libc::STDOUT_FILENO);
            }
        }
    }

    // Now env_logger will write to the redirected stderr instead of logcat!
    env_logger::builder().filter_level(log::LevelFilter::Info).init();

    log::info!("SOW ENGINE STARTING...");

    let event_loop = EventLoopBuilder::default().with_android_app(app).build().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

    run_game(event_loop);
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn wasm_main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Info).expect("error initializing logger");
    log::info!("SOW ENGINE WASM STARTING...");

    let event_loop = winit::event_loop::EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

    run_game(event_loop);
}
pub mod sim_bridge;
