use egui::{Color32, FontId, Response, Sense, Ui};
use sow_ui_kit::theme::palette;

pub struct Heading<'a> {
    text: &'a str,
    size: f32,
    color: Color32,
}

impl<'a> Heading<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            size: 28.0,
            color: Color32::WHITE,
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    pub fn cyan(self) -> Self {
        self.color(palette::neon_cyan())
    }

    pub fn gold(self) -> Self {
        self.color(palette::neon_gold())
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let font_id = FontId::proportional(self.size);
        let galley = ui
            .painter()
            .layout_no_wrap(self.text.to_uppercase(), font_id, self.color);
        let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
        if ui.is_rect_visible(rect) {
            sow_ui_kit::theme::paint_premium_glow_galley(
                ui.painter(),
                rect.left_top(),
                galley,
                self.color,
                Color32::BLACK,
            );
        }
        response
    }
}

pub struct Subtitle<'a> {
    text: &'a str,
    size: f32,
    color: Color32,
}

impl<'a> Subtitle<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            size: 18.0,
            color: Color32::WHITE,
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    pub fn cyan(self) -> Self {
        self.color(palette::neon_cyan())
    }

    pub fn gold(self) -> Self {
        self.color(palette::neon_gold())
    }

    pub fn muted(self) -> Self {
        self.color(palette::text_muted())
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let font_id = FontId::proportional(self.size);
        let galley = ui
            .painter()
            .layout_no_wrap(self.text.to_uppercase(), font_id, self.color);
        let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
        if ui.is_rect_visible(rect) {
            sow_ui_kit::theme::paint_premium_glow_galley(
                ui.painter(),
                rect.left_top(),
                galley,
                self.color,
                Color32::BLACK,
            );
        }
        response
    }
}

pub struct BodyText<'a> {
    text: &'a str,
    size: f32,
    color: Color32,
    wrap_width: Option<f32>,
}

impl<'a> BodyText<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            size: 14.0,
            color: Color32::WHITE,
            wrap_width: None,
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    pub fn muted(self) -> Self {
        self.color(palette::text_muted())
    }

    pub fn wrap(mut self, width: f32) -> Self {
        self.wrap_width = Some(width);
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let font_id = FontId::proportional(self.size);
        let wrap_w = self.wrap_width.unwrap_or_else(|| ui.available_width());
        let galley = ui
            .painter()
            .layout(self.text.to_owned(), font_id, self.color, wrap_w);
        let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
        if ui.is_rect_visible(rect) {
            sow_ui_kit::theme::paint_premium_glow_galley(
                ui.painter(),
                rect.left_top(),
                galley,
                self.color,
                Color32::BLACK,
            );
        }
        response
    }
}

pub struct Caption<'a> {
    text: &'a str,
    size: f32,
    color: Color32,
}

impl<'a> Caption<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            size: 12.0,
            color: palette::text_muted(),
        }
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn color(mut self, color: Color32) -> Self {
        self.color = color;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let font_id = FontId::proportional(self.size);
        let galley = ui
            .painter()
            .layout_no_wrap(self.text.to_owned(), font_id, self.color);
        let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
        if ui.is_rect_visible(rect) {
            ui.painter().galley(rect.left_top(), galley, self.color);
        }
        response
    }
}
