use crate::app::SowApp;
use crate::{camera_zoom_lower_bound, camera_zoom_upper_bound};

impl SowApp {
    pub fn check_surface(&mut self) {
        if !self.ensure_render_ctx() {
            return;
        }
        if self.gfx.surface.is_none() {
            if let Some(ref win) = self.gfx.window {
                #[cfg(target_arch = "wasm32")]
                let (pw, ph) = crate::web_canvas::physical_viewport_size();
                #[cfg(target_arch = "wasm32")]
                let sz = winit::dpi::PhysicalSize::new(pw.max(1), ph.max(1));
                #[cfg(not(target_arch = "wasm32"))]
                let sz = win.surface_size();

                let Some(render_ctx) = self.gfx.render_ctx.take() else {
                    return;
                };

                #[cfg(target_arch = "wasm32")]
                crate::web_canvas::set_canvas_backing_store_size(sz.width, sz.height);

                match render_ctx.create_surface(win, sz.width, sz.height) {
                    Ok(s) => {
                        self.gfx.configured_physical = sz;
                        // ponytail: query device_pixel_ratio directly as winit scale_factor is 1.0 initially
                        #[cfg(target_arch = "wasm32")]
                        let sf = web_sys::window()
                            .map(|window| window.device_pixel_ratio() as f32)
                            .unwrap_or(1.0);
                        #[cfg(not(target_arch = "wasm32"))]
                        let sf = win.scale_factor() as f32;

                        let vp = crate::viewport::Viewport::from_configured(self, sf);
                        vp.sync_to_app(self);
                        let zmin = camera_zoom_lower_bound(
                            self.input.screen_w,
                            self.input.screen_h,
                            self.sim.map_w,
                            self.sim.map_h,
                        );
                        let zmax =
                            camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h)
                                .max(zmin);
                        self.input.camera_zoom =
                            self.input.camera_zoom.clamp(zmin, zmax);
                        self.input.target_zoom = self.input.target_zoom.clamp(zmin, zmax);
                        self.clamp_camera_to_map();
                        let format = s.info().format;

                        if let Some(sp) = self.gfx.prev_sync_point.take() {
                            let _ = render_ctx.context.wait_for(&sp, !0);
                        }
                        if let Some(mut old_mr) = self.gfx.map_renderer.take() {
                            let old_terrain = old_mr.terrain.clone();
                            old_mr.destroy(&render_ctx);
                            self.gfx.map_renderer = Some(crate::render::gpu::MapRenderer::new(
                                &render_ctx.context,
                                self.sim.map_w,
                                self.sim.map_h,
                                format,
                                &old_terrain,
                            ));
                            self.gfx.needs_first_upload = true;
                        }
                        if let Some(mut old_mover) = self.gfx.mover_renderer.take() {
                            old_mover.destroy(&render_ctx);
                        }
                        self.gfx.mover_renderer = Some(crate::render::gpu::MoverRenderer::new(
                            &render_ctx.context,
                            format,
                        ));
                        if let Some(mut old_text) = self.gfx.text_renderer.take() {
                            old_text.destroy(&render_ctx);
                        }
                        self.gfx.text_renderer = Some(crate::render::gpu::TextRenderer::new(
                            &render_ctx.context,
                            format,
                        ));
                        if let Some(mut old_gp) = self.gfx.gui_painter.take() {
                            old_gp.destroy(&render_ctx.context);
                        }

                        self.gfx.gui_painter =
                            Some(blade_egui::GuiPainter::new(s.info(), &render_ctx.context));
                        self.gfx.surface = Some(s);
                        self.gfx.render_ctx = Some(render_ctx);

                        self.ui.egui_ctx = egui::Context::default();
                        sow_ui_kit::theme::apply_theme(&self.ui.egui_ctx);
                        self.ui.invalidate_egui_dependent_caches();
                        sow_ui_kit::register_game_assets(&self.ui.egui_ctx);
                        log::info!("Successfully created surface on retry.");
                    }
                    Err(e) => {
                        self.gfx.render_ctx = Some(render_ctx);
                        log::warn!("Surface creation failed: {:?}", e);
                    }
                }
            }
        }
    }
}
