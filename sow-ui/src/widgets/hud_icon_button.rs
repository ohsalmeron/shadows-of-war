use egui::{Image, Response, Sense, TextureHandle, Ui, Widget};

/// Icon-only HUD control: no frame, no hover chrome — just the webp texture.
pub struct HudIconButton<'a> {
    texture: Option<&'a TextureHandle>,
    size: f32,
}

impl<'a> HudIconButton<'a> {
    pub fn new(texture: Option<&'a TextureHandle>, size: f32) -> Self {
        Self { texture, size }
    }
}

impl Widget for HudIconButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(self.size, self.size), Sense::click());

        if ui.is_rect_visible(rect) {
            if let Some(tex) = self.texture {
                ui.put(rect, Image::new(tex).fit_to_exact_size(rect.size()));
            }
        }

        response
    }
}
