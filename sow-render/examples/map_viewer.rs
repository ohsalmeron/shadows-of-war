use blade_graphics as gpu;
use sow_core::map::GameMap;
use sow_render::{MapGlobals, MapRenderer, RenderContext};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

const MW: u32 = 800;
const MH: u32 = 600;

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("Shadows of War - Map Viewer")
        .with_inner_size(winit::dpi::LogicalSize::new(MW as f64, MH as f64))
        .build(&event_loop)
        .unwrap();

    let mut render_ctx = RenderContext::new();
    let surface = render_ctx.create_surface(&window, MW, MH);
    let format = surface.info().format;

    render_ctx.command_encoder.start();
    let mut map_renderer = MapRenderer::new(
        &render_ctx.context,
        &mut render_ctx.command_encoder,
        MW,
        MH,
        format,
    );
    let sp_new = render_ctx.context.submit(&mut render_ctx.command_encoder);
    let _ = render_ctx.context.wait_for(&sp_new, !0);

    let mut game_map = GameMap::new(MW, MH);
    for y in 0..MH {
        for x in 0..MW {
            let owner = if x < 200 {
                1u16
            } else if x > 600 {
                2
            } else {
                0
            };
            game_map.set_owner_id(x, y, owner);
        }
    }

    render_ctx.command_encoder.start();
    map_renderer.update(&mut render_ctx.command_encoder, &render_ctx.context, &game_map);
    let sp_up = render_ctx.context.submit(&mut render_ctx.command_encoder);
    let _ = render_ctx.context.wait_for(&sp_up, !0);

    let globals = MapGlobals {
        camera_pos: [0.0, 0.0],
        zoom: 1.0,
        _pad0: 0.0,
        screen_size: [MW as f32, MH as f32],
        map_size: [MW as f32, MH as f32],
        local_player_id: 1,
        _pad1: 0,
        _pad2: 0,
        _pad3: 0,
    };

    let prev_sync_point: Option<gpu::SyncPoint> = None;
    let mut state = Some((render_ctx, surface, map_renderer, prev_sync_point, globals));

    event_loop
        .run(move |event, elwt| {
            elwt.set_control_flow(ControlFlow::Wait);

            if let Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } = &event
            {
                if let Some((mut render_ctx, mut surface, mut map_renderer, mut prev_sync_point, _)) =
                    state.take()
                {
                    if let Some(sp) = prev_sync_point.take() {
                        let _ = render_ctx.context.wait_for(&sp, !0);
                    }
                    map_renderer.destroy(&render_ctx);
                    render_ctx
                        .context
                        .destroy_command_encoder(&mut render_ctx.command_encoder);
                    render_ctx.context.destroy_surface(&mut surface);
                }
                elwt.exit();
                return;
            }

            if let Some((render_ctx, surface, map_renderer, prev_sync_point, globals)) =
                &mut state
            {
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

                            map_renderer.draw(
                                &mut render_ctx.command_encoder,
                                frame.texture_view(),
                                *globals,
                            );

                            render_ctx.command_encoder.present(frame);
                            let sync_point =
                                render_ctx.context.submit(&mut render_ctx.command_encoder);
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
        })
        .unwrap();
}
