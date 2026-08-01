use crate::app::SowApp;
use crate::render::world::movers::world_to_tile;
use crate::{CAMERA_MIN_ZOOM, camera_zoom_upper_bound};
use blade_graphics as gpu;
use egui::Vec2;

impl SowApp {
    pub(crate) fn mouse_to_tile(&self, x: f64, y: f64) -> Option<(i32, i32)> {
        let world_x = (x as f32 - self.input.camera_x) / self.input.camera_zoom;
        let world_y = (y as f32 - self.input.camera_y) / self.input.camera_zoom;

        let (col, row) = world_to_tile(world_x, world_y);

        if col >= 0 && row >= 0 && col < self.sim.map_w as i32 && row < self.sim.map_h as i32 {
            Some((col, row))
        } else {
            None
        }
    }

    pub(crate) fn apply_surface_resize(
        &mut self,
        physical_size: winit::dpi::PhysicalSize<u32>,
        force_reconfigure: bool,
    ) {
        if physical_size.width == 0 || physical_size.height == 0 {
            return;
        }
        // ponytail: bypass winit scale_factor on web to avoid initial 1.0 zoom mismatch.
        #[cfg(target_arch = "wasm32")]
        let sf = web_sys::window()
            .map(|window| window.device_pixel_ratio() as f32)
            .unwrap_or(1.0)
            .max(0.01);
        #[cfg(not(target_arch = "wasm32"))]
        let sf = self
            .gfx
            .window
            .as_ref()
            .map_or(1.0, |w| w.scale_factor() as f32)
            .max(0.01);
        let vp = crate::viewport::Viewport {
            physical: physical_size,
            scale_factor: sf,
            logical: Vec2::new(
                physical_size.width as f32 / sf,
                physical_size.height as f32 / sf,
            ),
        };
        let needs_reconfigure = vp.wants_reconfigure(self) || force_reconfigure;

        let recreate_surface = needs_reconfigure
            && cfg!(any(target_os = "android", target_os = "ios"))
            && self.gfx.surface.is_some()
            && vp.orientation_flipped(self);

        if recreate_surface {
            if let Some(render_ctx) = self.gfx.render_ctx.as_mut() {
                if let Some(sp) = self.gfx.prev_sync_point.take() {
                    let _ = render_ctx.context.wait_for(&sp, !0);
                }
                if let Some(mut s) = self.gfx.surface.take() {
                    render_ctx.context.destroy_surface(&mut s);
                }
            }
        } else if needs_reconfigure {
            if let Some(render_ctx) = self.gfx.render_ctx.as_mut() {
                if let Some(sp) = self.gfx.prev_sync_point.take() {
                    let _ = render_ctx.context.wait_for(&sp, !0);
                }
                if let Some(ref mut s) = self.gfx.surface {
                    let display_sync = if cfg!(any(target_os = "android", target_os = "ios")) {
                        gpu::DisplaySync::Block
                    } else {
                        gpu::DisplaySync::Tear
                    };
                    #[cfg(target_arch = "wasm32")]
                    crate::web_canvas::set_canvas_backing_store_size(
                        physical_size.width,
                        physical_size.height,
                    );
                    render_ctx.context.reconfigure_surface(
                        s,
                        gpu::SurfaceConfig {
                            size: gpu::Extent {
                                width: physical_size.width,
                                height: physical_size.height,
                                depth: 1,
                            },
                            usage: gpu::TextureUsage::TARGET,
                            display_sync,
                            color_space: gpu::ColorSpace::Linear,
                            ..Default::default()
                        },
                    );
                }
            }
        }

        if needs_reconfigure || recreate_surface {
            self.gfx.configured_physical = physical_size;
        }

        crate::viewport::Viewport::from_configured(self, sf).sync_to_app(self);
        let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h);
        self.input.camera_zoom = self.input.camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);
        self.input.target_zoom = self.input.camera_zoom;

        if recreate_surface {
            self.check_surface();
        }
        if needs_reconfigure {
            if let Some(win) = self.gfx.window.as_ref() {
                win.request_redraw();
            }
        }
    }
    pub(crate) fn process_camera_zoom(&mut self, zoom_factor: f32, cx: f32, cy: f32) {
        let old_zoom = self.input.camera_zoom;
        self.input.camera_zoom *= zoom_factor;
        let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h);
        self.input.camera_zoom = self.input.camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);
        self.input.target_zoom = self.input.camera_zoom;

        let map_x = (cx - self.input.camera_x) / old_zoom;
        let map_y = (cy - self.input.camera_y) / old_zoom;
        self.input.camera_x = cx - map_x * self.input.camera_zoom;
        self.input.camera_y = cy - map_y * self.input.camera_zoom;
    }
}
