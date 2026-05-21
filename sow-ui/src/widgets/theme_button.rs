use egui::{Button, Color32, Response, Ui, Widget};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemeButtonStyle {
    Primary,
    Secondary,
    Tertiary,
    Danger,
}

pub struct ThemeButton {
    text: String,
    style: ThemeButtonStyle,
    min_size: egui::Vec2,
    text_size: f32,
    custom_fill: Option<Color32>,
    custom_text_color: Option<Color32>,
}

impl ThemeButton {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: ThemeButtonStyle::Primary,
            min_size: egui::vec2(0.0, 0.0),
            text_size: 18.0,
            custom_fill: None,
            custom_text_color: None,
        }
    }

    pub fn style(mut self, style: ThemeButtonStyle) -> Self {
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

    pub fn custom_fill(mut self, color: Color32) -> Self {
        self.custom_fill = Some(color);
        self
    }

    pub fn custom_text_color(mut self, color: Color32) -> Self {
        self.custom_text_color = Some(color);
        self
    }
}

impl Widget for ThemeButton {
    fn ui(self, ui: &mut Ui) -> Response {
        let text_color = self.custom_text_color.unwrap_or(Color32::WHITE);
        
        let mut btn = Button::new("").min_size(self.min_size);

        if let Some(fill) = self.custom_fill {
            btn = btn.fill(fill);
        } else {
            match self.style {
                ThemeButtonStyle::Primary => {
                    btn = btn.fill(crate::ui::theme::accent_solo_cyan());
                }
                ThemeButtonStyle::Secondary => {
                    btn = btn.fill(crate::ui::theme::accent_ranked_gold());
                }
                ThemeButtonStyle::Tertiary => {
                    btn = btn.fill(crate::ui::theme::palette::button_inactive());
                }
                ThemeButtonStyle::Danger => {
                    btn = btn.fill(crate::ui::theme::accent_danger());
                }
            }
        }

        // We use an empty button to handle the interaction and background,
        // then overlay our custom outlined text perfectly centered over it.
        let response = ui.add(btn);
        
        if ui.is_rect_visible(response.rect) {
            crate::ui::theme::outlined_text(
                ui.painter(),
                response.rect.center(),
                egui::Align2::CENTER_CENTER,
                &self.text,
                egui::FontId::proportional(self.text_size),
                text_color,
                Color32::BLACK,
            );
        }
        
        response
    }
}
