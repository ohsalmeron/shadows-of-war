use crate::app_state::SowApp;
use crate::{CAMERA_MIN_ZOOM, camera_zoom_upper_bound};

impl SowApp {
    pub fn check_surface(&mut self) {
        if self.surface.is_none() && self.window.is_some() {
            let win = self.window.as_ref().unwrap();
            let sz = win.surface_size();
            match self.render_ctx.create_surface(win, sz.width.max(1), sz.height.max(1)) {
                Ok(s) => {
                    self.screen_w = sz.width as f32;
                    self.screen_h = sz.height as f32;
                    let zmax = camera_zoom_upper_bound(self.screen_w, self.screen_h);
                    self.camera_zoom = self.camera_zoom.clamp(CAMERA_MIN_ZOOM, zmax);
                    self.raw_input.screen_rect = Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::Vec2::new(self.screen_w, self.screen_h)
                    ));
                    let format = s.info().format;
                    
                    if let Some(sp) = self.prev_sync_point.take() {
                        let _ = self.render_ctx.context.wait_for(&sp, !0);
                    }
                    let mut old_terrain = vec![128; (self.map_w * self.map_h) as usize];
                    if let Some(mut old_mr) = self.map_renderer.take() {
                        old_terrain = old_mr.terrain.clone();
                        old_mr.destroy(&self.render_ctx);
                    }
                    self.map_renderer = Some(sow_render::MapRenderer::new(&self.render_ctx.context, self.map_w, self.map_h, format, &old_terrain));
                    self.needs_first_upload = true;
                    
                    self.gui_painter = Some(blade_egui::GuiPainter::new(s.info(), &self.render_ctx.context));
                    self.surface = Some(s);
                    
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
