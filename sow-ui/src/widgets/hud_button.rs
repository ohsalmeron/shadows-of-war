use egui::{Align2, Color32, CursorIcon, FontId, Response, Sense, Ui, Widget};

pub struct HudButton {
    text: String,
    size: f32,
    color: Color32,
    dim: Option<f32>,
}

impl HudButton {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            size: crate::ui::theme::hud_button_text_size(),
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

impl Widget for HudButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let size = self.dim.unwrap_or(if cfg!(target_os = "android") {
            48.0
        } else {
            32.0
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

        let font_id = FontId::proportional(self.size);
        crate::widgets::outlined_emoji_text(
            ui.painter(),
            rect.center(),
            Align2::CENTER_CENTER,
            &self.text,
            font_id,
            self.color,
            Color32::BLACK,
        );

        response.on_hover_cursor(CursorIcon::PointingHand)
    }
}
