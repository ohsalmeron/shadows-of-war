use egui::{Align2, Color32, CursorIcon, Pos2, Rect, Response, Sense, Ui, Widget};

/// Draw a pixel emoji from the embedded atlas. Returns false if the glyph is not in the atlas.
pub fn try_paint_emoji(painter: &egui::Painter, emoji: &str, rect: Rect, tint: Color32) -> bool {
    let Some(uv) = sow_core::emoji::atlas_uv(emoji) else {
        return false;
    };
    let size_hint = egui::load::SizeHint::Size {
        width: rect.width().round().max(1.0) as u32,
        height: rect.height().round().max(1.0) as u32,
        maintain_aspect_ratio: true,
    };
    let load = painter.ctx().try_load_texture(
        sow_core::emoji::ATLAS_URI,
        sow_core::emoji::texture_options(),
        size_hint,
    );
    let Ok(egui::load::TexturePoll::Ready { texture }) = load else {
        return false;
    };
    painter.image(texture.id, rect, uv, tint);
    true
}

pub fn paint_emoji_centered(
    painter: &egui::Painter,
    emoji: &str,
    center: Pos2,
    size: f32,
    tint: Color32,
) -> bool {
    let rect = Rect::from_center_size(center, egui::vec2(size, size));
    try_paint_emoji(painter, emoji, rect, tint)
}

/// Clickable HUD square that renders a pixel emoji (falls back to glow text if missing).
pub struct HudEmojiButton {
    emoji: String,
    color: Color32,
    dim: Option<f32>,
}

impl HudEmojiButton {
    pub fn new(emoji: impl Into<String>) -> Self {
        Self {
            emoji: emoji.into(),
            color: Color32::WHITE,
            dim: None,
        }
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    pub fn dim(mut self, dim: f32) -> Self {
        self.dim = Some(dim);
        self
    }
}

impl Widget for HudEmojiButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let size = self.dim.unwrap_or_else(|| {
            if cfg!(target_os = "android") {
                48.0
            } else {
                32.0
            }
        });
        let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), Sense::click());

        let is_hovered = response.hovered();
        let is_active = response.is_pointer_button_down_on();
        let hover_t = ui.ctx().animate_bool(response.id.with("hover"), is_hovered);
        let active_t = ui.ctx().animate_bool(response.id.with("active"), is_active);
        let alpha = (hover_t * 25.0 + active_t * 25.0) as u8;
        if alpha > 0 {
            ui.painter()
                .rect_filled(rect, 6.0, Color32::from_white_alpha(alpha));
        }

        let emoji_size = size * 0.88;
        if !try_paint_emoji(
            ui.painter(),
            &self.emoji,
            Rect::from_center_size(rect.center(), egui::vec2(emoji_size, emoji_size)),
            self.color,
        ) {
            let font_id = egui::FontId::proportional(crate::ui::theme::hud_button_text_size());
            crate::ui::theme::paint_premium_glow_text(
                ui.painter(),
                rect.center(),
                Align2::CENTER_CENTER,
                &self.emoji,
                font_id,
                self.color,
                Color32::BLACK,
            );
        }

        response.on_hover_cursor(CursorIcon::PointingHand)
    }
}
