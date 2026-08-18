use egui::{Color32, CornerRadius, Frame, Margin, Stroke, Ui};
use sow_ui_kit::theme::palette;
use sow_ui_kit::theme::radius;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CardVariant {
    #[default]
    Surface,
    Glass,
    Inset,
    Accent,
}

pub struct Card {
    variant: CardVariant,
    padding: Margin,
    radius: CornerRadius,
    stroke: Option<Stroke>,
    min_width: Option<f32>,
    min_height: Option<f32>,
}

impl Default for Card {
    fn default() -> Self {
        Self {
            variant: CardVariant::default(),
            padding: Margin::same(16),
            radius: radius::md(),
            stroke: None,
            min_width: None,
            min_height: None,
        }
    }
}

impl Card {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn surface() -> Self {
        Self::new().variant(CardVariant::Surface)
    }

    pub fn glass() -> Self {
        Self::new().variant(CardVariant::Glass)
    }

    pub fn inset() -> Self {
        Self::new().variant(CardVariant::Inset)
    }

    pub fn accent() -> Self {
        Self::new().variant(CardVariant::Accent)
    }

    pub fn variant(mut self, variant: CardVariant) -> Self {
        self.variant = variant;
        self
    }

    pub fn padding(mut self, padding: Margin) -> Self {
        self.padding = padding;
        self
    }

    pub fn radius(mut self, radius: CornerRadius) -> Self {
        self.radius = radius;
        self
    }

    pub fn stroke(mut self, stroke: Stroke) -> Self {
        self.stroke = Some(stroke);
        self
    }

    pub fn min_width(mut self, min_width: f32) -> Self {
        self.min_width = Some(min_width);
        self
    }

    pub fn min_height(mut self, min_height: f32) -> Self {
        self.min_height = Some(min_height);
        self
    }

    pub fn show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> egui::InnerResponse<R> {
        let (fill, default_stroke) = match self.variant {
            CardVariant::Surface => (
                palette::surface(),
                Stroke::new(1.0_f32, palette::field_border()),
            ),
            CardVariant::Glass => (
                palette::backdrop(),
                Stroke::new(1.0_f32, Color32::from_white_alpha(30)),
            ),
            CardVariant::Inset => (
                palette::field_bg(),
                Stroke::new(1.0_f32, palette::field_border()),
            ),
            CardVariant::Accent => (
                palette::surface(),
                Stroke::new(1.5_f32, palette::neon_cyan()),
            ),
        };

        let stroke = self.stroke.unwrap_or(default_stroke);

        let frame = Frame::NONE
            .fill(fill)
            .stroke(stroke)
            .corner_radius(self.radius)
            .inner_margin(self.padding);

        frame.show(ui, |ui| {
            if let Some(w) = self.min_width {
                ui.set_min_width(w);
            }
            if let Some(h) = self.min_height {
                ui.set_min_height(h);
            }
            add_contents(ui)
        })
    }
}
