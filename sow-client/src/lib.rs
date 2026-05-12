use winit::event::{Event, WindowEvent, MouseButton, ElementState, MouseScrollDelta};
use sow_render::{RenderContext, MapRenderer, MapGlobals};
use sow_core::engine::SowEngine;
use sow_core::game::GameState;
use sow_core::game_config::GameConfig;
use sow_core::water_components::WaterComponents;
use blade_graphics as gpu;
use blade_egui::GuiPainter;
use egui::{Context, RawInput, Pos2, Rect, Vec2};
use sow_ui::{ClientApp, app::ClientPhase, UiAction};
use web_time::{Instant, Duration};
use sow_net::client::SowClient;
use std::collections::HashMap;
use std::sync::Arc;

struct CachedNameplate {
    name_galley: Arc<egui::Galley>,
    troops_galley: Arc<egui::Galley>,
    last_formatted_troops: String,
    last_font_size: f32,
    last_update_time: web_time::Instant,
}

fn render_troops(mut num: f64) -> String {
    num = num.max(0.0);
    if num >= 10_000_000.0 {
        let value = (num / 100_000.0).floor() / 10.0;
        format!("{:.1}M", value)
    } else if num >= 1_000_000.0 {
        let value = (num / 10_000.0).floor() / 100.0;
        format!("{:.2}M", value)
    } else if num >= 100_000.0 {
        format!("{}K", (num / 1000.0).floor())
    } else if num >= 10_000.0 {
        let value = (num / 100.0).floor() / 10.0;
        format!("{:.1}K", value)
    } else if num >= 1_000.0 {
        let value = (num / 10.0).floor() / 100.0;
        format!("{:.2}K", value)
    } else {
        format!("{:.0}", num.floor())
    }
}

const CAMERA_MIN_ZOOM: f32 = 0.25;
const CAMERA_MAX_ZOOM: f32 = 20.0;

fn player_label_scale(tiles_owned: u32, ref_tiles: f32, max_scale: f32) -> f32 {
    let t = tiles_owned.max(1) as f32;
    let r = ref_tiles.max(1.0);
    (t / r).sqrt().max(1.0).min(max_scale.max(1.0))
}

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
    let state = GameState::new(12345, map_w, map_h, config);
    let water = WaterComponents::compute(&state.map);
    let mut engine = SowEngine::new(state, water);

    engine.spawn_human(1, "Commander".to_string(), [0.1, 0.5, 0.9]);
    engine.spawn_ai(0, 4);

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
    let (map_tx, map_rx) = crossbeam_channel::unbounded::<Result<Vec<u8>, String>>();
    type EngineInitData = (sow_core::game::GameState, sow_core::water_components::WaterComponents, sow_core::protocol::ServerStartMessage);
    let (engine_init_tx, engine_init_rx) = crossbeam_channel::unbounded::<EngineInitData>();
    let mut pending_start_msg: Option<sow_core::protocol::ServerStartMessage> = None;

    let mut nameplate_cache: HashMap<u16, CachedNameplate> = HashMap::new();

    let (connect_tx, connect_rx) = crossbeam_channel::unbounded();

    // Reconnect scheduling (idle drop / resume / failed handshake).
    let mut ws_connect_fail_backoff_ms: u64 = 400;
    let mut ws_connect_not_before: Instant = Instant::now();
    let mut ws_reconnect_after_resume: bool = false;
    #[cfg(target_arch = "wasm32")]
    let mut wasm_doc_was_visible: bool = true;

    let ws_url = std::env::var("SOW_WS_URL").unwrap_or_else(|_| "wss://darkrift.ai/ws/".to_string());
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

    let mut prev_sync_point: Option<gpu::SyncPoint> = None;
    let mut last_tick = Instant::now();
    let start_time = Instant::now();
    let tick_interval = Duration::from_millis(100);
    let mut needs_first_upload = true;
    let mut needs_map_upload = true;
    let mut frame_count = 0;
    let mut last_fps_time = Instant::now();
    let mut current_fps = 0;

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
                    #[cfg(target_os = "android")]
                    let mut builder = winit::window::WindowBuilder::new()
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

                    #[cfg(not(any(target_os = "android", target_family = "wasm")))]
                    let builder = winit::window::WindowBuilder::new()
                        .with_title("Shadows of War — Native")
                        .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));

                    let win = builder.build(elwt).unwrap();
                    win
                });
                
                if surface.is_none() {
                    let sz = win.inner_size();
                    let s = render_ctx.create_surface(win, sz.width.max(1), sz.height.max(1));
                    screen_w = sz.width as f32;
                    screen_h = sz.height as f32;
                    raw_input.screen_rect = Some(Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(screen_w, screen_h)
                    ));
                    let format = s.info().format;
                    
                    if let Some(sp) = prev_sync_point.take() {
                        let _ = render_ctx.context.wait_for(&sp, !0);
                    }
                    render_ctx.command_encoder.start();
                    map_renderer = Some(MapRenderer::new(&render_ctx.context, &mut render_ctx.command_encoder, map_w, map_h, format));
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
                    WindowEvent::MouseInput { state: btn_state, button, .. } => {
                        let pressed = btn_state == ElementState::Pressed;
                        if button == MouseButton::Left {
                            dragging = pressed;

                            // If it's a click (pressed) and not intercepted by egui UI
                            if pressed && !egui_ctx.egui_wants_pointer_input() && app.phase == ClientPhase::Playing {
                                // Project mouse to map tile!
                                let world_x = (last_mouse_x as f32 - camera_x) / camera_zoom;
                                let world_y = (last_mouse_y as f32 - camera_y) / camera_zoom;
                                
                                let q_f = world_x - world_y * 0.577350269;
                                let r_f = world_y * 1.154700538;
                                let s_f = -q_f - r_f;

                                let mut rq = q_f.round();
                                let mut rr = r_f.round();
                                let rs = s_f.round();

                                let q_diff = (rq - q_f).abs();
                                let r_diff = (rr - r_f).abs();
                                let s_diff = (rs - s_f).abs();

                                if q_diff > r_diff && q_diff > s_diff {
                                    rq = -rr - rs;
                                } else if r_diff > s_diff {
                                    rr = -rq - rs;
                                }

                                let col = rq as i32 + (rr as i32 - (rr as i32 & 1)) / 2;
                                let row = rr as i32;

                                if col >= 0 && row >= 0 && col < map_w as i32 && row < map_h as i32 {
                                    let owner = engine.state.map.owner_id(col as u32, row as u32);
                                    
                                    // Apply intent locally instantly for single player responsiveness!
                                    let attack = sow_core::protocol::AttackIntent {
                                        target_owner: owner,
                                        troops: Some(app.hud_state.troops * (app.hud_state.attack_ratio as f64)),
                                    };
                                    let intent = sow_core::protocol::GameplayIntent::Attack(attack);
                                    
                                    if let Some(c) = net_client.as_ref() {
                                        // Multiplayer: send intent to server
                                        let msg = sow_core::protocol::ClientMessage::Gameplay {
                                            intent: intent.clone(),
                                        };
                                        if let Ok(json) = serde_json::to_string(&msg) {
                                            c.send(json);
                                        }
                                    } else {
                                        // Singleplayer: apply directly
                                        let stamped = sow_core::protocol::StampedIntent {
                                            player_id: my_player_id.unwrap_or(1),
                                            intent,
                                        };
                                        engine.apply_stamped_intent(&stamped, 0);
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
                                camera_zoom = camera_zoom.clamp(CAMERA_MIN_ZOOM, CAMERA_MAX_ZOOM);

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

                                if !egui_ctx.egui_wants_pointer_input() && app.phase == ClientPhase::Playing {
                                    let world_x = (last_mouse_x as f32 - camera_x) / camera_zoom;
                                    let world_y = (last_mouse_y as f32 - camera_y) / camera_zoom;
                                    
                                    let q_f = world_x - world_y * 0.577350269;
                                    let r_f = world_y * 1.154700538;
                                    let s_f = -q_f - r_f;

                                    let mut rq = q_f.round();
                                    let mut rr = r_f.round();
                                    let rs = s_f.round();

                                    let q_diff = (rq - q_f).abs();
                                    let r_diff = (rr - r_f).abs();
                                    let s_diff = (rs - s_f).abs();

                                    if q_diff > r_diff && q_diff > s_diff {
                                        rq = -rr - rs;
                                    } else if r_diff > s_diff {
                                        rr = -rq - rs;
                                    }

                                    let col = rq as i32 + (rr as i32 - (rr as i32 & 1)) / 2;
                                    let row = rr as i32;

                                    if col >= 0 && row >= 0 && col < map_w as i32 && row < map_h as i32 {
                                        let owner = engine.state.map.owner_id(col as u32, row as u32);
                                        let attack = sow_core::protocol::AttackIntent {
                                            target_owner: owner,
                                            troops: Some(app.hud_state.troops * (app.hud_state.attack_ratio as f64)),
                                        };
                                        let intent = sow_core::protocol::GameplayIntent::Attack(attack);
                                        if let Some(c) = net_client.as_ref() {
                                            let msg = sow_core::protocol::ClientMessage::Gameplay { intent: intent.clone() };
                                            if let Ok(json) = serde_json::to_string(&msg) {
                                                c.send(json);
                                            }
                                        } else {
                                            let stamped = sow_core::protocol::StampedIntent { player_id: my_player_id.unwrap_or(1), intent };
                                            engine.apply_stamped_intent(&stamped, 0);
                                        }
                                    }
                                }
                            }
                        }
                    }
                    WindowEvent::MouseWheel { delta, .. } => {
                        let scroll = match delta {
                            MouseScrollDelta::LineDelta(_, y) => y,
                            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 50.0,
                        };
                        let old_zoom = camera_zoom;
                        camera_zoom *= 1.0 + scroll * 0.15;
                        camera_zoom = camera_zoom.clamp(CAMERA_MIN_ZOOM, CAMERA_MAX_ZOOM);

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
                                if needs_map_upload {
                                    mr.update(&mut render_ctx.command_encoder, &render_ctx.context, &engine.state.map);
                                    needs_map_upload = false;
                                }

                                let globals = MapGlobals {
                                    camera_pos: [camera_x, camera_y],
                                    zoom: camera_zoom,
                                    time: start_time.elapsed().as_secs_f32(),
                                    screen_size: [screen_w, screen_h],
                                    map_size: [map_w as f32, map_h as f32],
                                    visual_terrain_sharpness: engine.state.config.shader_terrain_sharpness,
                                    visual_interior_alpha: engine.state.config.shader_interior_alpha,
                                    visual_border_alpha: engine.state.config.shader_border_alpha,
                                    lod_2_zoom: engine.state.config.ui_lod_2_zoom,
                                    lod_3_zoom: engine.state.config.ui_lod_3_zoom,
                                    local_player_id: my_player_id.unwrap_or(1) as u32,
                                    padding1: 0.0,
                                    padding2: 0.0,
                                };
                                mr.draw(&mut render_ctx.command_encoder, frame.texture_view(), globals);
                            }

                            // ── UI UPDATE ───────────────────────────────────────
                            let mut sf = window.as_ref().map_or(1.0, |w| w.scale_factor() as f32);
                            if cfg!(target_os = "android") && sf < 1.5 && screen_h > 800.0 {
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
                            
                            raw_input.predicted_dt = 1.0 / 60.0;
                            let egui_output = egui_ctx.run_ui(raw_input.clone(), |ctx| {
                                if app.phase == ClientPhase::Playing {
                                    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("world_overlays")));
                                    
                                    let cfg = &engine.state.config;
                                    let dot_r = cfg.ui_lod_dot_radius;
                                    
                                    // Determine LOD tier from zoom.
                                    // LOD 1: far/simplified
                                    // LOD 2: normal/full plates
                                    // LOD 3: max zoom
                                    let lod = if camera_zoom >= cfg.ui_lod_3_zoom {
                                        3u8
                                    } else if camera_zoom >= cfg.ui_lod_2_zoom {
                                        2u8
                                    } else {
                                        1u8
                                    };
                                    
                                    for player in &engine.state.players {
                                        if player.tile_count == 0 || !player.alive { continue; }
                                        
                                        let avg_col = player.sum_x as f32 / player.tile_count as f32;
                                        let avg_row = player.sum_y as f32 / player.tile_count as f32;
                                        
                                        let target_cx = avg_col + 0.5 + (avg_row as i32 % 2) as f32 * 0.5;
                                        let target_cy = (avg_row + 0.5) * 0.86602540378;
                                        
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
                                        
                                        let is_nation_or_human = player.player_type != sow_core::player::PlayerType::Bot;
                                        let show_full = lod >= 2;
                                        
                                        let center = egui::pos2(screen_x, screen_y);
                                        let pc = egui::Color32::from_rgb(
                                            (player.color[0] * 255.0) as u8,
                                            (player.color[1] * 255.0) as u8,
                                            (player.color[2] * 255.0) as u8,
                                        );
                                        
                                        if show_full {
                                            // Full nameplate
                                            let scale = player_label_scale(player.tile_count, cfg.ui_label_ref_tiles, cfg.ui_label_max_scale);
                                            let font_size = cfg.ui_label_base_size * scale;
                                            
                                            // 10 Hz Cache Logic
                                            let now = Instant::now();
                                            let cache_entry = nameplate_cache.entry(player.id).or_insert_with(|| {
                                                let font_id = egui::FontId::proportional(font_size);
                                                let troops_str = render_troops(player.troops);
                                                
                                                let display_name = if player.player_type == sow_core::player::PlayerType::Human {
                                                    format!("★ {}", player.name)
                                                } else {
                                                    player.name.clone()
                                                };
                                                
                                                CachedNameplate {
                                                    name_galley: painter.layout_no_wrap(display_name, font_id.clone(), egui::Color32::WHITE),
                                                    troops_galley: painter.layout_no_wrap(format!("⚔ {}", troops_str), font_id, egui::Color32::WHITE),
                                                    last_formatted_troops: troops_str,
                                                    last_font_size: font_size,
                                                    last_update_time: now,
                                                }
                                            });
                                            
                                            if now.duration_since(cache_entry.last_update_time).as_millis() >= 100 {
                                                let new_troops_str = render_troops(player.troops);
                                                if new_troops_str != cache_entry.last_formatted_troops {
                                                    let font_id = egui::FontId::proportional(font_size);
                                                    cache_entry.troops_galley = painter.layout_no_wrap(format!("⚔ {}", new_troops_str), font_id, egui::Color32::WHITE);
                                                    cache_entry.last_formatted_troops = new_troops_str;
                                                }
                                                // Dynamic font size scaling updates
                                                let current_font_size = cache_entry.last_font_size;
                                                if (current_font_size - font_size).abs() > 0.5 {
                                                    let font_id = egui::FontId::proportional(font_size);
                                                    let display_name = if player.player_type == sow_core::player::PlayerType::Human {
                                                        format!("★ {}", player.name)
                                                    } else {
                                                        player.name.clone()
                                                    };
                                                    cache_entry.name_galley = painter.layout_no_wrap(display_name, font_id.clone(), egui::Color32::WHITE);
                                                    cache_entry.troops_galley = painter.layout_no_wrap(format!("⚔ {}", cache_entry.last_formatted_troops), font_id, egui::Color32::WHITE);
                                                    cache_entry.last_font_size = font_size;
                                                }
                                                cache_entry.last_update_time = now;
                                            }
                                            
                                            let name_galley = &cache_entry.name_galley;
                                            let troops_galley = &cache_entry.troops_galley;
                                            
                                            let w = name_galley.rect.width().max(troops_galley.rect.width());
                                            let h = name_galley.rect.height() + troops_galley.rect.height() + 2.0;
                                            
                                            let bg_rect = egui::Rect::from_center_size(center, egui::vec2(w, h)).expand(6.0);
                                            painter.rect_filled(bg_rect, 4.0, egui::Color32::from_black_alpha(200));
                                            
                                            if is_nation_or_human {
                                                // Thin colored accent line at top
                                                let accent = egui::Rect::from_min_size(bg_rect.left_top(), egui::vec2(bg_rect.width(), 2.0));
                                                painter.rect_filled(accent, 2.0, pc);
                                            }
                                            
                                            let name_pos = egui::pos2(center.x - name_galley.rect.width() / 2.0, center.y - h / 2.0);
                                            let troops_pos = egui::pos2(center.x - troops_galley.rect.width() / 2.0, center.y - h / 2.0 + name_galley.rect.height() + 2.0);
                                            painter.galley(name_pos, name_galley.clone(), egui::Color32::WHITE);
                                            painter.galley(troops_pos, troops_galley.clone(), egui::Color32::WHITE);
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
                                
                                if app.phase == ClientPhase::Playing {
                                    egui::Area::new(egui::Id::new("fps_counter"))
                                        .fixed_pos(egui::pos2(10.0, 10.0))
                                        .show(ctx, |ui| {
                                            ui.label(
                                                egui::RichText::new(format!("FPS: {}", current_fps))
                                                    .color(egui::Color32::YELLOW)
                                                    .strong()
                                            );
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
                                                preferred_map: Some(app.main_menu_state.selected_map.clone()),
                                            };
                                            app.main_menu_state.pending_join_lobby_id = Some(id);
                                            if let Ok(json) = serde_json::to_string(&join_msg) {
                                                if let Some(c) = net_client.as_ref() {
                                                    c.send(json);
                                                }
                                            }
                                            app.main_menu_state.is_waiting = true;
                                        }
                                        UiAction::LeaveLobby => {
                                            if let Some(c) = net_client.as_ref() {
                                                let leave = sow_core::protocol::ClientMessage::Leave {};
                                                if let Ok(json) = serde_json::to_string(&leave) {
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
                                            app.phase = ClientPhase::MainMenu;
                                            
                                            // ── Proper Memory Cleanup ──
                                            // Reset the engine to a clean preview state so the background menu is clean
                                            // and the next singleplayer/multiplayer game doesn't inherit old state.
                                            let config = GameConfig::default();
                                            let state = GameState::new(12345, map_w, map_h, config);
                                            let water = WaterComponents::compute(&state.map);
                                            engine = SowEngine::new(state, water);
                                            engine.spawn_human(1, "Commander".to_string(), [0.1, 0.5, 0.9]);
                                            engine.spawn_ai(0, 4);
                                            turn_queue.clear();
                                            label_positions.clear();
                                            nameplate_cache.clear();
                                            needs_first_upload = true;
                                            needs_map_upload = true;
                                        }
                                        UiAction::SetAttackRatio(r) => {
                                            app.hud_state.attack_ratio = r;
                                        }
                                        UiAction::CenterCamera => {
                                            let pid = my_player_id.unwrap_or(1);
                                            if let Some(player) =
                                                engine.state.players.iter().find(|p| p.id == pid)
                                            {
                                                if player.tile_count > 0 && player.alive {
                                                    let cx = player.sum_x as f32
                                                        / player.tile_count as f32;
                                                    let cy = player.sum_y as f32
                                                        / player.tile_count as f32;
                                                    
                                                    let world_cx = cx + 0.5 + (cy as i32 % 2) as f32 * 0.5;
                                                    let world_cy = (cy + 0.5) * 0.86602540378;

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

                        if let Ok(start_msg) =
                            serde_json::from_str::<sow_core::protocol::ServerStartMessage>(&msg)
                        {
                            log::info!("Received ServerStartMessage; queuing for engine init until map finishes downloading");
                            pending_start_msg = Some(start_msg);
                            continue;
                        }
                        
                        if let Ok(turn_msg) =
                            serde_json::from_str::<sow_core::protocol::ServerTurnMessage>(&msg)
                        {
                            turn_queue.push_back(turn_msg.turn);
                            continue;
                        }

                        if let Ok(broadcast) =
                            serde_json::from_str::<sow_core::protocol::ServerLobbiesBroadcastMessage>(
                                &msg,
                            )
                        {
                            app.main_menu_state.lobbies = broadcast.lobbies.clone();

                            if app.main_menu_state.is_waiting {
                                let key = my_lobby_id
                                    .or(app.main_menu_state.joined_lobby_id)
                                    .or(app.main_menu_state.pending_join_lobby_id);
                                if let Some(l_id) = key {
                                    if let Some(lobby) =
                                        broadcast.lobbies.iter().find(|l| l.id == l_id)
                                    {
                                        if lobby.is_counting_down {
                                            app.main_menu_state.wait_timer_secs = lobby.timer_secs;
                                        }
                                    }
                                }
                            }
                            continue;
                        }

                        if let Ok(closed) =
                            serde_json::from_str::<sow_core::protocol::ServerLobbyClosedMessage>(&msg)
                        {
                            log::warn!("Lobby {} closed: {}", closed.lobby_id, closed.reason);
                            app.phase = ClientPhase::MainMenu;
                            app.main_menu_state.is_waiting = false;
                            app.main_menu_state.pending_join_lobby_id = None;
                            app.main_menu_state.joined_lobby_id = None;
                            my_lobby_id = None;
                            my_player_id = None;
                            let config = GameConfig::default();
                            let state = GameState::new(12345, map_w, map_h, config);
                            let water = WaterComponents::compute(&state.map);
                            engine = SowEngine::new(state, water);
                            engine.spawn_human(1, "Commander".to_string(), [0.1, 0.5, 0.9]);
                            engine.spawn_ai(0, 4);
                            turn_queue.clear();
                            label_positions.clear();
                            needs_first_upload = true;
                            continue;
                        }

                        if let Ok(fail) =
                            serde_json::from_str::<sow_core::protocol::ServerJoinFailedMessage>(&msg)
                        {
                            log::warn!("Join failed: {}", fail.reason);
                            app.main_menu_state.is_waiting = false;
                            app.main_menu_state.pending_join_lobby_id = None;
                            app.main_menu_state.joined_lobby_id = None;
                            continue;
                        }

                        if let Ok(ack) =
                            serde_json::from_str::<sow_core::protocol::ServerJoinAckMessage>(&msg)
                        {
                            my_lobby_id = Some(ack.lobby_id);
                            my_player_id = Some(ack.player_id);
                            app.main_menu_state.joined_lobby_id = Some(ack.lobby_id);
                            
                            // Start downloading the map via HTTP
                            let map_name = ack.map_name.clone();
                            let tx = map_tx.clone();
                            app.main_menu_state.is_downloading_map = true;
                            app.main_menu_state.cached_map = None;
                            
                            let maps_base = std::env::var("SOW_MAPS_URL").unwrap_or_else(|_| "https://darkrift.ai/assets/maps".to_string());
                            let url = format!("{}/{}/map.bin", maps_base.trim_end_matches('/'), map_name);
                            log::info!("Downloading map from: {}", url);
                            
                            let request = ehttp::Request::get(&url);
                            ehttp::fetch(request, move |result| {
                                match result {
                                    Ok(response) => {
                                        if response.ok {
                                            log::info!("Downloaded {} bytes", response.bytes.len());
                                            let _ = tx.send(Ok(response.bytes));
                                        } else {
                                            log::error!("Failed to fetch map, HTTP {}", response.status);
                                            let _ = tx.send(Err(format!("HTTP Error: {}", response.status)));
                                        }
                                    }
                                    Err(err) => {
                                        log::error!("Failed to fetch map: {}", err);
                                        let _ = tx.send(Err(format!("Fetch error: {}", err)));
                                    }
                                }
                            });
                            
                            continue;
                        }
                        }
                    }
                }
                if ws_disconnected {
                    log::warn!("WebSocket disconnected; will reconnect.");
                    net_client = None;
                    app.main_menu_state.is_connected = false;
                    app.main_menu_state.is_connecting = false;
                    ws_connect_not_before =
                        ws_connect_not_before.min(now + Duration::from_millis(200));
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
                if let Ok(res) = map_rx.try_recv() {
                    match res {
                        Ok(bytes) => {
                            log::info!("Map download completed successfully.");
                            app.main_menu_state.cached_map = Some(bytes);
                            app.main_menu_state.is_downloading_map = false;
                        }
                        Err(e) => {
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

                if let Some(start_msg) = pending_start_msg.take() {
                    if app.main_menu_state.is_downloading_map {
                        pending_start_msg = Some(start_msg);
                    } else {
                        log::info!("Received ServerStartMessage; entering match");
                        log::info!("Received ServerStartMessage; computing heavy init in background");
                        app.phase = sow_ui::app::ClientPhase::Loading;
                        app.main_menu_state.is_waiting = false;
                        app.main_menu_state.pending_join_lobby_id = None;
                        app.main_menu_state.joined_lobby_id = None;
                        my_player_id = start_msg.my_player_id;
                        
                        app.loading_state.status_text = "Computing terrain and water geometry...".to_string();
                        app.loading_state.progress = 0.1;

                        let cached_map = app.main_menu_state.cached_map.take();
                        let start_msg_clone = start_msg.clone();
                        let tx = engine_init_tx.clone();

                        let init_logic = move || {
                            let w = start_msg_clone.config.map_width;
                            let h = start_msg_clone.config.map_height;
                            let mut state = sow_core::game::GameState::new(
                                start_msg_clone.seed,
                                w,
                                h,
                                start_msg_clone.config.clone(),
                            );
                            
                            if let Some(bytes) = cached_map {
                                if bytes.len() == state.map.terrain.len() {
                                    let dest_ptr = state.map.terrain.as_mut_ptr() as *mut u8;
                                    unsafe {
                                        std::ptr::copy_nonoverlapping(bytes.as_ptr(), dest_ptr, bytes.len());
                                    }
                                } else {
                                    for (i, &b) in bytes.iter().enumerate() {
                                        if i < state.map.terrain.len() {
                                            state.map.terrain[i] = sow_core::map::MapTile::from_byte(b);
                                        }
                                    }
                                }
                            } else {
                                log::error!("Cached map data not found! Terrain will be empty.");
                            }

                            let water = sow_core::water_components::WaterComponents::compute(&state.map);
                            let _ = tx.send((state, water, start_msg_clone));
                        };

                        #[cfg(target_arch = "wasm32")]
                        init_logic();

                        #[cfg(not(target_arch = "wasm32"))]
                        std::thread::spawn(init_logic);

                        turn_queue.clear();
                        needs_first_upload = true;
                    }
                }
                // Poll engine init channel
                if app.phase == sow_ui::app::ClientPhase::Loading {
                    app.loading_state.progress = (app.loading_state.progress + 0.05).min(0.95);
                    if let Ok((state, water, start_msg)) = engine_init_rx.try_recv() {
                        log::info!("Engine initialization complete in background thread.");
                        app.loading_state.status_text = "Uploading assets to GPU...".to_string();
                        app.loading_state.progress = 1.0;
                        
                        engine = SowEngine::new(state, water);

                        for p in start_msg.players {
                            engine.spawn_human(p.id, p.name.clone(), p.color);
                        }
                        engine.spawn_ai(engine.state.config.nation_count, engine.state.config.bot_count);

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
                            map_renderer = Some(sow_render::map_renderer::MapRenderer::new(&render_ctx.context, &mut render_ctx.command_encoder, map_w, map_h, s.info().format));
                            let sync_point = render_ctx.context.submit(&mut render_ctx.command_encoder);
                            prev_sync_point = Some(sync_point);
                            
                            needs_first_upload = true;
                            needs_map_upload = true;
                        }
                        
                        app.phase = sow_ui::app::ClientPhase::Playing;
                        if let Some(pid) = my_player_id {
                            if let Some(player) = engine.state.players.iter().find(|p| p.id == pid) {
                                if player.tile_count > 0 && player.alive {
                                    let cx = player.sum_x as f32 / player.tile_count as f32;
                                    let cy = player.sum_y as f32 / player.tile_count as f32;
                                    camera_zoom = 1.5;
                                    camera_x = screen_w * 0.5 - cx * camera_zoom;
                                    camera_y = screen_h * 0.5 - cy * camera_zoom;
                                }
                            }
                        }

                        if let Some(c) = net_client.as_ref() {
                            if let (Some(lid), Some(pid)) = (my_lobby_id, my_player_id) {
                                let ready_msg = sow_core::protocol::ClientMessage::Ready { lobby_id: lid, player_id: pid };
                                let json = serde_json::to_string(&ready_msg).unwrap();
                                c.send(json);
                            }
                        }
                    }
                }

                app.hud_state.is_mobile = screen_w < 900.0;
                
                if app.phase == sow_ui::app::ClientPhase::Playing {
                    if net_client.is_some() {
                        // Multiplayer: lockstep execution dictated by server
                        let mut ticks_processed = 0;
                        while let Some(turn) = turn_queue.pop_front() {
                            for stamped in &turn.intents {
                                engine.apply_stamped_intent(stamped, 0);
                            }
                            engine.tick();
                            needs_map_upload = true;
                            
                            // Update UI HUD State from my player id
                            if let Some(player) = engine.state.players.iter().find(|p| p.id == my_player_id.unwrap_or(1)) {
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
                            engine.tick();
                            needs_map_upload = true;
                            last_tick = now;
                            
                            if let Some(player) = engine.state.players.iter().find(|p| p.id == my_player_id.unwrap_or(1)) {
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
