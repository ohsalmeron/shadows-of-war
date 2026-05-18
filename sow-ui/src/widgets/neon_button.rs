use egui::{Button, Color32, Response, RichText, Ui, Widget};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NeonButtonStyle {
    Primary,
    Secondary,
    Success,
    Danger,
    Outline,
}

pub struct NeonButton {
    text: String,
    style: NeonButtonStyle,
    min_size: egui::Vec2,
    text_size: f32,
}

impl NeonButton {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: NeonButtonStyle::Primary,
            min_size: egui::vec2(0.0, 0.0),
            text_size: 18.0,
        }
    }

    pub fn style(mut self, style: NeonButtonStyle) -> Self {
        self.style = style;
        self
    }

    pub fn min_size(mut self, min_size: egui::Vec2) -> Self {
        self.min_size = min_size;
        self
    }

    pub fn text_size(mut self, text_size: f32) -> Self {
        self.text_size = text_size;
        self
    }
}

impl Widget for NeonButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let text_color = if self.style == NeonButtonStyle::Outline {
            crate::ui::theme::text_secondary()
        } else {
            Color32::WHITE
        };

        let rich_text = RichText::new(self.text)
            .size(self.text_size)
            .strong()
            .color(text_color);
        
        let mut btn = Button::new(rich_text).min_size(self.min_size);

        match self.style {
            NeonButtonStyle::Primary => {
                btn = btn.fill(crate::ui::theme::accent_solo_cyan());
            }
            NeonButtonStyle::Secondary => {
                btn = btn.fill(crate::ui::theme::accent_ranked_gold());
            }
            NeonButtonStyle::Success => {
                btn = btn.fill(crate::ui::theme::accent_solo_cyan());
            }
            NeonButtonStyle::Danger => {
                btn = btn.fill(crate::ui::theme::accent_danger());
            }
            NeonButtonStyle::Outline => {
                // Relies on the default inactive/hovered visuals
            }
        }

        ui.add(btn)
    }
}
