#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;
use sow_render::{RenderContext, MapRenderer, MapGlobals};
use blade_graphics as gpu;

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let window = app.get_webview_window("main").unwrap();
            
            let window_clone = window.clone();
            
            std::thread::spawn(move || {
                let mut render_ctx = RenderContext::new();
                let mut surface = render_ctx.create_surface(&window_clone, 1280, 720);
                let map_renderer = MapRenderer::new(&render_ctx, 1280, 720, surface.info().format);
                
                let globals = MapGlobals {
                    camera_pos: [0.0, 0.0],
                    zoom: 1.0,
                    screen_size: [1280.0, 720.0],
                    pad: [0.0; 3],
                };
                
                let mut prev_sync_point: Option<gpu::SyncPoint> = None;
                
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(16));
                    let frame = surface.acquire_frame();
                    render_ctx.command_encoder.start();
                    render_ctx.command_encoder.init_texture(frame.texture());
                    
                    map_renderer.draw(&mut render_ctx.command_encoder, frame.texture_view(), globals);
                    
                    render_ctx.command_encoder.present(frame);
                    let sync_point = render_ctx.context.submit(&mut render_ctx.command_encoder);
                    if let Some(sp) = prev_sync_point.take() {
                        let _ = render_ctx.context.wait_for(&sp, !0);
                    }
                    prev_sync_point = Some(sync_point);
                }
            });
            
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
