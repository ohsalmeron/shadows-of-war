use crate::app::SowApp;
use crate::{CAMERA_MIN_ZOOM, camera_zoom_upper_bound};

impl SowApp {
    pub fn check_surface(&mut self) {
        if self.gfx.surface.is_none() && self.gfx.window.is_some() {
            let win = self.gfx.window.as_ref().unwrap();
            let sz = win.surface_size();
            match self.gfx.render_ctx.create_surface(win, sz.width.max(1), sz.height.max(1)) {
                Ok(s) => {
                    self.input.screen_w = sz.width as f32;
                    self.input.screen_h = sz.height as f32;
                    let zmax = camera_zoom_upper_bound(self.input.screen_w, self.input.screen_h);
                    self.input.camera_zoom = self.input.camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);
                    self.raw_input.screen_rect = Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::Vec2::new(self.input.screen_w, self.input.screen_h)
                    ));
                    let format = s.info().format;
                    
                    if let Some(sp) = self.gfx.prev_sync_point.take() {
                        let _ = self.gfx.render_ctx.context.wait_for(&sp, !0);
                    }
                    let mut old_terrain = vec![128; (self.sim.map_w * self.sim.map_h) as usize];
                    if let Some(mut old_mr) = self.gfx.map_renderer.take() {
                        old_terrain = old_mr.terrain.clone();
                        old_mr.destroy(&self.gfx.render_ctx);
                    }
                    self.gfx.map_renderer = Some(sow_render::MapRenderer::new(&self.gfx.render_ctx.context, self.sim.map_w, self.sim.map_h, format, &old_terrain));
                    self.gfx.needs_first_upload = true;
                    
                    self.gfx.gui_painter = Some(blade_egui::GuiPainter::new(s.info(), &self.gfx.render_ctx.context));
                    self.gfx.surface = Some(s);
                    
                    self.egui_ctx = egui::Context::default();
                    sow_ui::ui::theme::apply_theme(&self.egui_ctx);
                    log::info!("Successfully created surface on retry.");
                }
                Err(_) => {
                    // Still unavailable
                }
            }
        }

    }
}
