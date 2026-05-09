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
use std::time::{Instant, Duration};

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
                            window.request_redraw();
                        }
                    }
                    // ── Mouse input for camera ──────────────────────────────
                    WindowEvent::MouseInput { state: btn_state, button: MouseButton::Left, .. } => {
                        dragging = btn_state == ElementState::Pressed;
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

                            render_ctx.command_encoder.present(frame);
                            let sync_point = render_ctx.context.submit(&mut render_ctx.command_encoder);
                            prev_sync_point = Some(sync_point);
                        }
                    }
                    _ => {}
                }
            }
            Event::AboutToWait => {
                let now = Instant::now();
                if now.duration_since(last_tick) >= tick_interval {
                    engine.tick();
                    last_tick = now;
                }
                window.request_redraw();
            }
            _ => {}
        }
    }).unwrap();
}
