//! A bold, "gaming" Play / GO button — glossy gradient body, animated outer glow, a bright top
//! sheen and a light bevel rim, with glow text. Comes in two forms:
//!
//! * [`paint_play_button`] — pure paint into a rect, for custom-painted surfaces (e.g. the lobby
//!   card, where the whole card is the click target and the button only sells the affordance);
//! * [`PlayButton`] — a standalone interactive [`egui::Widget`] that allocates, senses the click,
//!   and animates its own hover.

use egui::{Color32, CornerRadius, FontId, Painter, Rect, Response, Sense, Stroke, Ui, Vec2, Widget};

fn lerp_col(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Paint a play button into `rect`.
///
/// * `hot` (0..1) — hover energy: brightens the body, flares the glow, lifts the sheen. Drive it
///   from the host surface's hover (animate it for a smooth ramp).
/// * `pulse` (0..1) — a slow breathing factor for the idle glow (e.g. `sin` of time, remapped).
pub fn paint_play_button(painter: &Painter, rect: Rect, hot: f32, pulse: f32, label: &str) {
    if !rect.is_positive() {
        return;
    }
    let hot = hot.clamp(0.0, 1.0);
    let r = (rect.height() * 0.28).clamp(6.0, 14.0) as u8;
    let radius = CornerRadius::same(r);

    // Hot-pink → crimson, brightening on hover.
    let top = lerp_col(
        Color32::from_rgb(244, 63, 132),
        Color32::from_rgb(255, 122, 170),
        hot,
    );
    let bottom = lerp_col(
        Color32::from_rgb(198, 24, 84),
        Color32::from_rgb(236, 64, 122),
        hot,
    );
    let glow_col = Color32::from_rgb(255, 70, 140);

    // Outer glow: a few concentric rounded strokes, breathing when idle, flaring on hover.
    let glow_a = (0.12 + 0.22 * hot + 0.05 * pulse).clamp(0.0, 0.6);
    for i in 0..3 {
        let grow = 2.0 + i as f32 * 3.0 + 4.0 * hot;
        let a = (glow_a * (1.0 - i as f32 * 0.3)).clamp(0.0, 1.0);
        painter.rect_stroke(
            rect.expand(grow),
            CornerRadius::same(r.saturating_add(grow as u8)),
            Stroke::new(2.0, glow_col.gamma_multiply(a)),
            egui::StrokeKind::Inside,
        );
    }

    // Body — darker base, then a top gloss band gives the vertical gradient with rounded corners.
    painter.rect_filled(rect, radius, bottom);
    let gloss = Rect::from_min_max(
        rect.min,
        egui::pos2(rect.max.x, rect.min.y + rect.height() * 0.55),
    );
    painter.rect_filled(
        gloss,
        CornerRadius {
            nw: r,
            ne: r,
            sw: 0,
            se: 0,
        },
        top,
    );
    // Bright sheen near the top.
    let sheen = Rect::from_min_max(
        rect.min + egui::vec2(3.0, 3.0),
        egui::pos2(rect.max.x - 3.0, rect.min.y + rect.height() * 0.22),
    );
    painter.rect_filled(
        sheen,
        CornerRadius::same((r as f32 * 0.6) as u8),
        Color32::from_white_alpha((38.0 + 34.0 * hot) as u8),
    );

    // Light bevel rim.
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.5, Color32::from_white_alpha((80.0 + 50.0 * hot) as u8)),
        egui::StrokeKind::Inside,
    );

    // Label — bold, centered, dark shadow + glow.
    let font = FontId::proportional((rect.height() * 0.44).clamp(13.0, 22.0));
    sow_ui_kit::theme::paint_premium_glow_text(
        painter,
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font,
        Color32::WHITE,
        Color32::from_black_alpha(170),
    );
}

/// Standalone interactive play button. Allocates `min_size`, senses the click, animates hover.
pub struct PlayButton {
    label: String,
    min_size: Vec2,
}

impl PlayButton {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            min_size: Vec2::new(96.0, 42.0),
        }
    }

    pub fn min_size(mut self, size: Vec2) -> Self {
        self.min_size = size;
        self
    }
}

impl Widget for PlayButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(self.min_size, Sense::click());
        let hot = ui
            .ctx()
            .animate_bool(response.id.with("play_hot"), response.hovered());
        if response.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            ui.ctx().request_repaint(); // breathe while hovered
        }
        let pulse = (((ui.input(|i| i.time) * 3.0).sin() + 1.0) * 0.5) as f32;
        // Press dip + hover lift.
        let scale = 1.0 + 0.04 * hot - if response.is_pointer_button_down_on() { 0.05 } else { 0.0 };
        let draw_rect = Rect::from_center_size(rect.center(), rect.size() * scale);
        if ui.is_rect_visible(rect) {
            paint_play_button(ui.painter(), draw_rect, hot, pulse, &self.label);
        }
        response
    }
}
