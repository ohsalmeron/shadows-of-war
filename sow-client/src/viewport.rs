//! Unified viewport sizing — WASM and native mobile share one model.
//!
//! Physical pixels come from `window.surface_size()` (the UIView / Metal drawable).
//! Logical size is `physical / scale_factor`. No per-platform safe-area shrink.

use egui::{Pos2, Rect, Vec2};
use winit::dpi::PhysicalSize;

use crate::app::SowApp;

#[derive(Clone, Copy, Debug)]
pub struct Viewport {
    pub physical: PhysicalSize<u32>,
    pub scale_factor: f32,
    pub logical: Vec2,
}

impl Viewport {
    pub fn measure(win: &dyn winit::window::Window) -> Self {
        let physical = win.surface_size();
        let scale_factor = win.scale_factor() as f32;
        let sf = scale_factor.max(0.01);
        Self {
            physical,
            scale_factor: sf,
            logical: Vec2::new(physical.width as f32 / sf, physical.height as f32 / sf),
        }
    }

    pub fn physical_changed(&self, app: &SowApp) -> bool {
        if self.physical.width == 0 || self.physical.height == 0 {
            return false;
        }
        (app.input.screen_w as u32).abs_diff(self.physical.width) > 1
            || (app.input.screen_h as u32).abs_diff(self.physical.height) > 1
    }

    pub fn orientation_flipped(&self, app: &SowApp) -> bool {
        if app.input.screen_w <= 0.0 || app.input.screen_h <= 0.0 {
            return false;
        }
        let was_portrait = app.input.screen_w <= app.input.screen_h;
        let now_portrait = self.physical.width <= self.physical.height;
        was_portrait != now_portrait
    }
}

pub fn apply_to_egui(app: &mut SowApp, vp: &Viewport) {
    app.ui.egui_ctx.set_pixels_per_point(vp.scale_factor);
    app.ui.raw_input.screen_rect = Some(Rect::from_min_size(Pos2::ZERO, vp.logical));
    app.ui.raw_input.safe_area_insets = None;
}

pub fn scale_pointer_events(raw_input: &mut egui::RawInput, scale_factor: f32) {
    for ev in &mut raw_input.events {
        match ev {
            egui::Event::PointerMoved(pos) | egui::Event::PointerButton { pos, .. } => {
                pos.x /= scale_factor;
                pos.y /= scale_factor;
            }
            _ => {}
        }
    }
}
