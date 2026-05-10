use blade_graphics as gpu;
use sow_render::{MapGlobals, MapRenderer, RenderContext};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use bytemuck;

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("Shadows of War - Map Viewer")
        .with_inner_size(winit::dpi::LogicalSize::new(800.0, 600.0))
        .build(&event_loop)
        .unwrap();

    let mut render_ctx = RenderContext::new();
    let mut surface = render_ctx.create_surface(&window, 800, 600);
    
    let mut map_renderer = MapRenderer::new(&render_ctx, 800, 600, surface.info().format);

    // Initialize map texture with dummy data
    let mut map_data = vec![0u32; 800 * 600];
    for y in 0..600 {
        for x in 0..800 {
            let owner = if x < 200 { 1 } else if x > 600 { 2 } else { 0 };
            map_data[y * 800 + x] = owner;
        }
    }

    let upload_buffer = render_ctx.context.create_buffer(gpu::BufferDesc {
        name: "map_upload",
        size: (map_data.len() * 4) as u64,
        memory: gpu::Memory::Upload,
    });
    
    unsafe {
        std::ptr::copy_nonoverlapping(
            map_data.as_ptr(),
            upload_buffer.data() as *mut u32,
            map_data.len(),
        );
    }
    render_ctx.context.sync_buffer(upload_buffer);

    render_ctx.command_encoder.start();
    render_ctx.command_encoder.init_texture(map_renderer.texture);
    if let mut transfer = render_ctx.command_encoder.transfer("upload_map") {
        transfer.copy_buffer_to_texture(
            upload_buffer.into(),
            800 * 4, // bytes per row
            map_renderer.texture.into(),
            gpu::Extent { width: 800, height: 600, depth: 1 },
        );
    }
    let sync_point = render_ctx.context.submit(&mut render_ctx.command_encoder);
    let _ = render_ctx.context.wait_for(&sync_point, !0);
    render_ctx.context.destroy_buffer(upload_buffer);

    let mut globals = MapGlobals {
        camera_pos: [0.0, 0.0],
        zoom: 1.0,
        time: 0.0,
        screen_size: [1280.0, 720.0],
        map_size: [map.width as f32, map.height as f32],
        visual_terrain_sharpness: 0.05,
        visual_interior_alpha: 0.35,
        visual_border_alpha: 1.0,
        padding: 0.0,
    };

    let mut prev_sync_point: Option<gpu::SyncPoint> = None;
    let mut state = Some((render_ctx, surface, map_renderer, prev_sync_point));

    event_loop.run(move |event, elwt| {
        elwt.set_control_flow(ControlFlow::Wait);

        if let Event::WindowEvent { event: WindowEvent::CloseRequested, .. } = &event {
            if let Some((render_ctx, mut surface, mut map_renderer, mut prev_sync_point)) = state.take() {
                if let Some(sp) = prev_sync_point.take() {
                    let _ = render_ctx.context.wait_for(&sp, !0);
                }
                map_renderer.destroy(&render_ctx);
                render_ctx.context.destroy_surface(&mut surface);
            }
            elwt.exit();
            return;
        }

        if let Some((render_ctx, surface, map_renderer, prev_sync_point)) = &mut state {
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::Resized(size) => {
                        let config = gpu::SurfaceConfig {
                            size: gpu::Extent {
                                width: size.width,
                                height: size.height,
                                depth: 1,
                            },
                            usage: gpu::TextureUsage::TARGET,
                            display_sync: gpu::DisplaySync::Recent,
                            ..Default::default()
                        };
                        render_ctx.context.reconfigure_surface(surface, config);
                        globals.screen_size = [size.width as f32, size.height as f32];
                        window.request_redraw();
                    }
                    WindowEvent::RedrawRequested => {
                        let frame = surface.acquire_frame();
                        render_ctx.command_encoder.start();
                        render_ctx.command_encoder.init_texture(frame.texture());
                        
                        map_renderer.draw(&mut render_ctx.command_encoder, frame.texture_view(), globals);
                        
                        render_ctx.command_encoder.present(frame);
                        let sync_point = render_ctx.context.submit(&mut render_ctx.command_encoder);
                        if let Some(sp) = prev_sync_point.take() {
                            let _ = render_ctx.context.wait_for(&sp, !0);
                        }
                        *prev_sync_point = Some(sync_point);
                    }
                    _ => {}
                },
                Event::AboutToWait => {
                    window.request_redraw();
                }
                _ => {}
            }
        }
    }).unwrap();
}
