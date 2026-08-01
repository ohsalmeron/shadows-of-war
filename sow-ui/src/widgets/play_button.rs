//! A compact "gaming" Play / GO button — hot-pink gradient body, single glow ring, glow text.
//! Comes in two forms:
//!
//! * [`paint_play_button`] — pure paint into a rect, for custom-painted surfaces;
//! * [`PlayButton`] — a standalone interactive widget that allocates, senses click, animates hover.

use egui::{
    Color32, CornerRadius, FontId, Painter, Rect, Response, Sense, Stroke, Ui, Vec2, Widget,
};

fn lerp_col(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

/// Paint a play button into `rect`. `hot` (0..1) drives hover brightness and glow.
pub fn paint_play_button(painter: &Painter, rect: Rect, hot: f32, pulse: f32, label: &str) {
    if !rect.is_positive() {
        return;
    }
    let hot = hot.clamp(0.0, 1.0);
    let r = (rect.height() * 0.28).clamp(6.0, 14.0) as u8;
    let radius = CornerRadius::same(r);

    // Hot-pink → bright, darkens toward bottom.
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

    // Single glow ring, breathes at rest, flares on hover.
    let glow_a = (0.15 + 0.25 * hot + 0.06 * pulse).clamp(0.0, 0.5);
    let grow = 3.0 + 2.0 * hot;
    painter.rect_stroke(
        rect.expand(grow),
        CornerRadius::same(r.saturating_add(grow as u8)),
        Stroke::new(
            2.0_f32,
            Color32::from_rgb(255, 70, 140).gamma_multiply(glow_a),
        ),
        egui::StrokeKind::Inside,
    );

    // Body: dark base + top gradient.
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

    // Text.
    let font = FontId::proportional((rect.height() * 0.45).clamp(13.0, 20.0));
    sow_ui_kit::theme::paint_premium_glow_text(
        painter,
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        font,
        Color32::WHITE,
        Color32::from_black_alpha(160),
    );
}

/// Standalone interactive play button.
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
            ui.ctx().request_repaint();
        }
        let pulse = (((ui.input(|i| i.time) * 3.0).sin() + 1.0) * 0.5) as f32;
        let scale = 1.0 + 0.03 * hot
            - if response.is_pointer_button_down_on() {
                0.04
            } else {
                0.0
            };
        let draw_rect = Rect::from_center_size(rect.center(), rect.size() * scale);
        if ui.is_rect_visible(rect) {
            paint_play_button(ui.painter(), draw_rect, hot, pulse, &self.label);
        }
        response
    }
}
