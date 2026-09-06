use egui::{Color32, FontId, Response, Sense, Stroke, Ui, Vec2, vec2};
use sow_ui_kit::theme::palette;
use sow_ui_kit::theme::radius;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Secondary,
    Ghost,
    Danger,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ButtonSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl ButtonSize {
    pub fn min_size(self) -> Vec2 {
        match self {
            ButtonSize::Small => vec2(80.0, 28.0),
            ButtonSize::Medium => vec2(120.0, 38.0),
            ButtonSize::Large => vec2(180.0, 48.0),
        }
    }

    pub fn font_size(self) -> f32 {
        match self {
            ButtonSize::Small => 13.0,
            ButtonSize::Medium => 16.0,
            ButtonSize::Large => 20.0,
        }
    }

    pub fn padding(self) -> Vec2 {
        match self {
            ButtonSize::Small => vec2(10.0, 4.0),
            ButtonSize::Medium => vec2(16.0, 8.0),
            ButtonSize::Large => vec2(24.0, 12.0),
        }
    }
}

pub struct Button<'a> {
    text: &'a str,
    variant: ButtonVariant,
    size: ButtonSize,
    disabled: bool,
    custom_min_size: Option<Vec2>,
    full_width: bool,
}

impl<'a> Button<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            variant: ButtonVariant::default(),
            size: ButtonSize::default(),
            disabled: false,
            custom_min_size: None,
            full_width: false,
        }
    }

    pub fn primary(text: &'a str) -> Self {
        Self::new(text).variant(ButtonVariant::Primary)
    }

    pub fn secondary(text: &'a str) -> Self {
        Self::new(text).variant(ButtonVariant::Secondary)
    }

    pub fn ghost(text: &'a str) -> Self {
        Self::new(text).variant(ButtonVariant::Ghost)
    }

    pub fn danger(text: &'a str) -> Self {
        Self::new(text).variant(ButtonVariant::Danger)
    }

    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn size(mut self, size: ButtonSize) -> Self {
        self.size = size;
        self
    }

    pub fn small(self) -> Self {
        self.size(ButtonSize::Small)
    }

    pub fn large(self) -> Self {
        self.size(ButtonSize::Large)
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn min_size(mut self, min_size: Vec2) -> Self {
        self.custom_min_size = Some(min_size);
        self
    }

    pub fn full_width(mut self, full_width: bool) -> Self {
        self.full_width = full_width;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let font_id = FontId::proportional(self.size.font_size());
        let font_galley =
            ui.painter()
                .layout_no_wrap(self.text.to_uppercase(), font_id, Color32::WHITE);

        let padding = self.size.padding();
        let content_size = font_galley.size() + padding * 2.0;
        let min_size = self.custom_min_size.unwrap_or_else(|| self.size.min_size());

        let mut desired_size = content_size.max(min_size);
        if self.full_width {
            desired_size.x = ui.available_width();
        }

        let (rect, response) = ui.allocate_exact_size(
            desired_size,
            if self.disabled {
                Sense::hover()
            } else {
                Sense::click()
            },
        );

        if ui.is_rect_visible(rect) {
            let is_hovered = response.hovered() && !self.disabled;
            let is_pressed = response.is_pointer_button_down_on() && !self.disabled;

            let (fill, stroke, text_color, has_glow) = if self.disabled {
                (
                    palette::button_inactive().linear_multiply(0.4),
                    Stroke::new(1.0_f32, palette::field_border().linear_multiply(0.3)),
                    palette::text_muted(),
                    false,
                )
            } else {
                match self.variant {
                    ButtonVariant::Primary => {
                        if is_pressed {
                            (
                                palette::neon_cyan(),
                                Stroke::new(1.5_f32, palette::neon_cyan_hover()),
                                Color32::BLACK,
                                true,
                            )
                        } else if is_hovered {
                            (
                                palette::neon_cyan_hover(),
                                Stroke::new(1.5_f32, Color32::WHITE),
                                Color32::BLACK,
                                true,
                            )
                        } else {
                            (palette::neon_cyan(), Stroke::NONE, Color32::BLACK, false)
                        }
                    }
                    ButtonVariant::Secondary => {
                        if is_pressed {
                            (
                                palette::button_hovered(),
                                Stroke::new(1.5_f32, palette::neon_cyan()),
                                palette::neon_cyan(),
                                true,
                            )
                        } else if is_hovered {
                            (
                                palette::button_hovered(),
                                Stroke::new(1.0_f32, palette::neon_cyan_hover()),
                                Color32::WHITE,
                                true,
                            )
                        } else {
                            (
                                palette::surface(),
                                Stroke::new(1.0_f32, palette::field_border()),
                                Color32::WHITE,
                                false,
                            )
                        }
                    }
                    ButtonVariant::Ghost => {
                        if is_pressed {
                            (
                                palette::button_inactive(),
                                Stroke::new(1.0_f32, palette::neon_gold()),
                                palette::neon_gold(),
                                false,
                            )
                        } else if is_hovered {
                            (
                                palette::button_inactive(),
                                Stroke::NONE,
                                palette::neon_cyan(),
                                true,
                            )
                        } else {
                            (
                                Color32::TRANSPARENT,
                                Stroke::NONE,
                                palette::text_muted(),
                                false,
                            )
                        }
                    }
                    ButtonVariant::Danger => {
                        if is_pressed {
                            (
                                palette::danger(),
                                Stroke::new(1.5_f32, palette::danger_border()),
                                Color32::WHITE,
                                true,
                            )
                        } else if is_hovered {
                            (
                                palette::danger(),
                                Stroke::new(1.5_f32, Color32::WHITE),
                                Color32::WHITE,
                                true,
                            )
                        } else {
                            (
                                Color32::from_rgba_unmultiplied(80, 20, 20, 180),
                                Stroke::new(1.0_f32, palette::danger_border()),
                                Color32::WHITE,
                                false,
                            )
                        }
                    }
                }
            };

            let corner_radius = radius::sm();
            ui.painter()
                .rect(rect, corner_radius, fill, stroke, egui::StrokeKind::Inside);

            let text_pos = egui::pos2(
                rect.center().x - font_galley.size().x * 0.5,
                rect.center().y - font_galley.size().y * 0.5,
            );

            if text_color == Color32::WHITE || has_glow {
                sow_ui_kit::theme::paint_premium_glow_galley(
                    ui.painter(),
                    text_pos,
                    font_galley,
                    text_color,
                    Color32::BLACK,
                );
            } else {
                ui.painter().galley(text_pos, font_galley, text_color);
            }
        }

        response
    }
}
