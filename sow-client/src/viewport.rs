//! Unified viewport sizing — WASM and native mobile share one model.
//!
//! Physical pixels come from the GPU swapchain after reconfigure (`configured_physical`).
//! Window `surface_size()` is the resize *request*; layout and drawing use swapchain extent.

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
        Self::from_physical(physical, scale_factor.max(0.01))
    }

    pub fn from_physical(physical: PhysicalSize<u32>, scale_factor: f32) -> Self {
        let sf = scale_factor.max(0.01);
        Self {
            physical,
            scale_factor: sf,
            logical: Vec2::new(physical.width as f32 / sf, physical.height as f32 / sf),
        }
    }

    pub fn from_configured(app: &SowApp, scale_factor: f32) -> Self {
        Self::from_physical(app.gfx.configured_physical, scale_factor)
    }

    /// Window size differs from the last successful swapchain reconfigure.
    pub fn wants_reconfigure(&self, app: &SowApp) -> bool {
        if self.physical.width == 0 || self.physical.height == 0 {
            return false;
        }
        self.physical.width != app.gfx.configured_physical.width
            || self.physical.height != app.gfx.configured_physical.height
    }

    pub fn sync_to_app(&self, app: &mut SowApp) {
        app.input.screen_w = self.physical.width as f32;
        app.input.screen_h = self.physical.height as f32;
        apply_to_egui(app, self);
    }

    pub fn orientation_flipped(&self, app: &SowApp) -> bool {
        let cfg = app.gfx.configured_physical;
        if cfg.width == 0 || cfg.height == 0 {
            return false;
        }
        let was_portrait = cfg.width <= cfg.height;
        let now_portrait = self.physical.width <= self.physical.height;
        was_portrait != now_portrait
    }
}



#[cfg(target_arch = "wasm32")]
pub fn sync_wasm_window(app: &SowApp, win: &dyn winit::window::Window) {
    let (w, h) = crate::web_canvas::canvas_logical_size();
    let sf = win.scale_factor();
    let expected_w = (w * sf) as u32;
    let expected_h = (h * sf) as u32;

    if app.gfx.configured_physical.width != expected_w
        || app.gfx.configured_physical.height != expected_h
    {
        let _ = win.request_surface_size(winit::dpi::LogicalSize::new(w, h).into());
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
