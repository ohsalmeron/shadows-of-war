use winit::{
    event::{Event, WindowEvent, MouseButton, ElementState, MouseScrollDelta},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use sow_render::{RenderContext, MapRenderer, MapGlobals};
use sow_core::engine::SowEngine;
use sow_core::game::GameState;
use sow_core::game_config::GameConfig;
use sow_core::water_components::WaterComponents;
use blade_graphics as gpu;
use blade_egui::GuiPainter;
use egui::{Context, RawInput, Pos2, Rect, Vec2};
use sow_ui::{ClientApp, app::ClientPhase, UiAction};
use std::time::{Instant, Duration};
use sow_net::client::SowClient;

fn format_troops(troops: f64) -> String {
    if troops < 1000.0 {
        format!("{:.0}", troops)
    } else if troops < 1_000_000.0 {
        format!("{:.1}K", troops / 1000.0)
    } else {
        format!("{:.1}M", troops / 1_000_000.0)
    }
}

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);

    let window = WindowBuilder::new()
        .with_title("Shadows of War — Native")
        .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0))
        .build(&event_loop)
        .unwrap();

    // ── Simulation ──────────────────────────────────────────────────────────
    let map_w: u32 = 800;
    let map_h: u32 = 600;
    let config = GameConfig::default();
    let state = GameState::new(12345, map_w, map_h, config);
    let water = WaterComponents::compute(&state.map);
    let mut engine = SowEngine::new(state, water);

    engine.spawn_human(1);
    engine.spawn_random_bots(4);

    // ── Renderer ────────────────────────────────────────────────────────────
    let mut render_ctx = RenderContext::new();
    let mut surface: Option<gpu::Surface> = None;
    let mut map_renderer: Option<MapRenderer> = None;
    let mut gui_painter: Option<GuiPainter> = None;

    // ── UI State ────────────────────────────────────────────────────────────
    let mut app = ClientApp::new();
    let egui_ctx = Context::default();
    let mut raw_input = RawInput::default();

    // ── Network State ───────────────────────────────────────────────────────
    let tokio_rt = tokio::runtime::Runtime::new().unwrap();
    let mut net_client: Option<SowClient> = None;
    let mut turn_queue = std::collections::VecDeque::new();
    let mut my_player_id: Option<u16> = None;
    let mut my_lobby_id: Option<u64> = None;

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

    let mut prev_sync_point: Option<gpu::SyncPoint> = None;
    let mut last_tick = Instant::now();
    let tick_interval = Duration::from_millis(100);
    let mut needs_first_upload = true;

    event_loop.run(move |event, elwt| {
        match event {
            Event::Resumed => {
                if surface.is_none() {
                    let s = render_ctx.create_surface(&window, 1280, 720);
                    let format = s.info().format;
                    map_renderer = Some(MapRenderer::new(&render_ctx, map_w, map_h, format));
                    gui_painter = Some(GuiPainter::new(s.info(), &render_ctx.context));
                    surface = Some(s);
                }
            }
            Event::WindowEvent { event, window_id } if window_id == window.id() => {
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
                            window.request_redraw();
                        }
                    }
                    WindowEvent::MouseInput { state: btn_state, button, .. } => {
                        let pressed = btn_state == ElementState::Pressed;
                        if button == MouseButton::Left {
                            dragging = pressed;

                            // If it's a click (pressed) and not intercepted by egui UI
                            if pressed && !egui_ctx.egui_wants_pointer_input() && app.phase == ClientPhase::Playing {
                                // Project mouse to map tile!
                                let map_x = (last_mouse_x as f32 - camera_x) / camera_zoom;
                                let map_y = (last_mouse_y as f32 - camera_y) / camera_zoom;

                                if map_x >= 0.0 && map_y >= 0.0 && map_x < map_w as f32 && map_y < map_h as f32 {
                                    let owner = engine.state.map.owner_id(map_x as u32, map_y as u32);
                                    
                                    // Apply intent locally instantly for single player responsiveness!
                                    let attack = sow_core::protocol::AttackIntent {
                                        target_owner: owner,
                                        troops: Some(app.hud_state.troops * (app.hud_state.attack_ratio as f64)),
                                    };
                                    let intent = sow_core::protocol::GameplayIntent::Attack(attack);
                                    
                                    if let Some(c) = net_client.as_ref() {
                                        // Multiplayer: send intent to server
                                        let msg = sow_core::protocol::ClientGameplayMessage {
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
                    WindowEvent::MouseWheel { delta, .. } => {
                        let scroll = match delta {
                            MouseScrollDelta::LineDelta(_, y) => y,
                            MouseScrollDelta::PixelDelta(pos) => pos.y as f32 / 50.0,
                        };
                        let old_zoom = camera_zoom;
                        camera_zoom *= 1.0 + scroll * 0.15;
                        camera_zoom = camera_zoom.clamp(0.25, 20.0);

                        // Zoom towards cursor
                        let factor = camera_zoom / old_zoom;
                        camera_x = last_mouse_x as f32 - factor * (last_mouse_x as f32 - camera_x);
                        camera_y = last_mouse_y as f32 - factor * (last_mouse_y as f32 - camera_y);
                    }
                    WindowEvent::RedrawRequested => {
                        if let Some(ref mut s) = surface {
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
                                mr.update(&mut render_ctx.command_encoder, &engine.state.map);

                                let globals = MapGlobals {
                                    camera_pos: [camera_x, camera_y],
                                    zoom: camera_zoom,
                                    _pad0: 0.0,
                                    screen_size: [screen_w, screen_h],
                                    map_size: [map_w as f32, map_h as f32],
                                };
                                mr.draw(&mut render_ctx.command_encoder, frame.texture_view(), globals);
                            }

                            // ── UI UPDATE ───────────────────────────────────────
                            raw_input.predicted_dt = 1.0 / 60.0;
                            let egui_output = egui_ctx.run_ui(raw_input.clone(), |ctx| {
                                if let Some(action) = app.draw(ctx) {
                                    match action {
                                        UiAction::StartSinglePlayer => {
                                            app.phase = ClientPhase::Playing;
                                        }
                                        UiAction::ConnectToServer(addr) => {
                                            app.lobby_state.is_waiting = true;
                                            app.lobby_state.is_connected = true;
                                            match tokio_rt.block_on(async { SowClient::connect(&addr).await }) {
                                                Ok(client) => {
                                                    log::info!("Connected to server!");
                                                    net_client = Some(client);
                                                    
                                                    // Send join message
                                                    let join_msg = sow_core::protocol::ClientJoinMessage {
                                                        name: "NativePlayer".into(),
                                                        is_observer: false,
                                                        target_lobby_id: None,
                                                    };
                                                    my_lobby_id = Some(1); // Server assigns None to 1
                                                    if let Ok(json) = serde_json::to_string(&join_msg) {
                                                        if let Some(c) = net_client.as_ref() {
                                                            c.send(json);
                                                        }
                                                    }
                                                }
                                                Err(e) => {
                                                    log::error!("Failed to connect: {}", e);
                                                    app.lobby_state.is_connected = false;
                                                    app.lobby_state.is_waiting = false;
                                                }
                                            }
                                        }
                                        UiAction::JoinLobby(id) => {
                                            let join_msg = sow_core::protocol::ClientJoinMessage {
                                                name: "NativePlayer".into(),
                                                is_observer: false,
                                                target_lobby_id: Some(id),
                                            };
                                            my_lobby_id = Some(id);
                                            if let Ok(json) = serde_json::to_string(&join_msg) {
                                                if let Some(c) = net_client.as_ref() {
                                                    c.send(json);
                                                }
                                            }
                                            app.lobby_state.is_waiting = true;
                                        }
                                        UiAction::LeaveLobby => {
                                            net_client = None;
                                            app.lobby_state.is_connected = false;
                                            app.lobby_state.is_waiting = false;
                                            app.phase = ClientPhase::MainMenu;
                                        }
                                        UiAction::SetAttackRatio(r) => {
                                            app.hud_state.attack_ratio = r;
                                        }
                                        _ => {}
                                    }
                                }

                                if app.phase == ClientPhase::Playing {
                                    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, egui::Id::new("nameplates")));
                                    
                                    for p in &engine.state.players {
                                        if !p.alive || p.tile_count == 0 { continue; }
                                        
                                        let world_x = p.sum_x as f32 / p.tile_count as f32;
                                        let world_y = p.sum_y as f32 / p.tile_count as f32;
                                        
                                        let screen_x = world_x * camera_zoom + camera_x;
                                        let screen_y = world_y * camera_zoom + camera_y;
                                        
                                        // Viewport culling
                                        if screen_x < -100.0 || screen_x > screen_w + 100.0 || 
                                           screen_y < -100.0 || screen_y > screen_h + 100.0 {
                                            continue;
                                        }

                                        let label = format!("{}\n{}", p.name, format_troops(p.troops));
                                        
                                        let font_id = egui::FontId::proportional((14.0 * camera_zoom).clamp(8.0, 48.0));
                                        let text_color = egui::Color32::from_rgb(
                                            (p.color[0] * 255.0) as u8,
                                            (p.color[1] * 255.0) as u8,
                                            (p.color[2] * 255.0) as u8,
                                        );

                                        painter.text(
                                            egui::pos2(screen_x + 1.0, screen_y + 1.0),
                                            egui::Align2::CENTER_CENTER,
                                            &label,
                                            font_id.clone(),
                                            egui::Color32::BLACK,
                                        );

                                        painter.text(
                                            egui::pos2(screen_x, screen_y),
                                            egui::Align2::CENTER_CENTER,
                                            &label,
                                            font_id,
                                            text_color,
                                        );
                                    }
                                }
                            });
                            raw_input.events.clear();

                            // ── DRAWING UI ──────────────────────────────────────────
                            if let Some(ref mut gp) = gui_painter {
                                let screen_desc = blade_egui::ScreenDescriptor {
                                    physical_size: (screen_w as u32, screen_h as u32),
                                    scale_factor: 1.0,
                                };
                                let paint_jobs = egui_ctx.tessellate(egui_output.shapes, 1.0);
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
                
                // Process network messages
                if let Some(c) = net_client.as_ref() {
                    while let Ok(msg) = c.rx.try_recv() {
                        // Check if it's a ServerLobbiesBroadcastMessage
                        if let Ok(broadcast) = serde_json::from_str::<sow_core::protocol::ServerLobbiesBroadcastMessage>(&msg) {
                            app.lobby_state.lobbies = broadcast.lobbies.clone();
                            
                            // If we're waiting for a lobby to start, find our lobby to update wait_timer_secs
                            if app.lobby_state.is_waiting {
                                if let Some(l_id) = my_lobby_id {
                                    if let Some(lobby) = broadcast.lobbies.iter().find(|l| l.id == l_id) {
                                        if lobby.is_counting_down {
                                            app.lobby_state.wait_timer_secs = lobby.timer_secs;
                                        }
                                    }
                                }
                            }
                        }
                        // Check if it's a ServerTurnMessage
                        if let Ok(turn_msg) = serde_json::from_str::<sow_core::protocol::ServerTurnMessage>(&msg) {
                            turn_queue.push_back(turn_msg.turn);
                        }
                        
                        // Check if it's a ServerStartMessage
                        if let Ok(start_msg) = serde_json::from_str::<sow_core::protocol::ServerStartMessage>(&msg) {
                            log::info!("Received ServerStartMessage! Transitioning to Playing!");
                            app.phase = ClientPhase::Playing;
                            app.lobby_state.is_waiting = false;
                            my_player_id = start_msg.my_player_id;
                            
                            // Load multiplayer state!
                            let state = sow_core::game::GameState::new(start_msg.seed, 800, 600, start_msg.config);
                            let water = sow_core::water_components::WaterComponents::compute(&state.map);
                            engine = SowEngine::new(state, water);
                            
                            // Let the engine spawn players in the exact deterministic order as the server
                            // (Since server currently just calls spawn_human for each player, then spawn_random_bots)
                            for p in start_msg.players {
                                engine.spawn_human(p.id);
                            }
                            engine.spawn_random_bots(4); // the server spawns 4 bots right now
                            
                            turn_queue.clear();
                            needs_first_upload = true;
                        }
                    }
                }
                
                if app.phase == ClientPhase::Playing {
                    if net_client.is_some() {
                        // Multiplayer: lockstep execution dictated by server
                        while let Some(turn) = turn_queue.pop_front() {
                            for stamped in &turn.intents {
                                engine.apply_stamped_intent(stamped, 0);
                            }
                            engine.tick();
                            
                            // Update UI HUD State from my player id
                            if let Some(player) = engine.state.players.iter().find(|p| p.id == my_player_id.unwrap_or(1)) {
                                app.hud_state.gold = player.gold;
                                app.hud_state.troops = player.troops;
                                let owned_tiles = engine.state.map.tiles_owned_by(player.id) as f64;
                                app.hud_state.max_troops = owned_tiles * 50.0;
                            }
                        }
                        last_tick = now;
                    } else {
                        // Singleplayer: run freely based on local timer
                        if now.duration_since(last_tick) >= tick_interval {
                            engine.tick();
                            last_tick = now;
                            
                            if let Some(player) = engine.state.players.iter().find(|p| p.id == my_player_id.unwrap_or(1)) {
                                app.hud_state.gold = player.gold;
                                app.hud_state.troops = player.troops;
                                let owned_tiles = engine.state.map.tiles_owned_by(player.id) as f64;
                                app.hud_state.max_troops = owned_tiles * 50.0;
                            }
                        }
                    }
                } else {
                    last_tick = now;
                }
                window.request_redraw();
            }
            _ => {}
        }
    }).unwrap();
}
