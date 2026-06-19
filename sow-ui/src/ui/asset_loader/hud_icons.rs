use super::*;

impl AssetLoader {
    pub fn ensure_hud_icons_loaded(&mut self, ctx: &egui::Context) {
        use crate::ui::hud::icons::HudIcon;

        if self.hud_icons.len() == HudIcon::ALL.len() {
            return;
        }

        for icon in HudIcon::ALL {
            if self.hud_icons.contains_key(&icon) {
                continue;
            }
            let image = image::load_from_memory(icon.bytes())
                .unwrap_or_else(|e| panic!("Failed to load {}: {e}", icon.file_name()))
                .to_rgba8();
            let size = [image.width() as _, image.height() as _];
            let pixels = image.as_flat_samples();
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
            let texture = ctx.load_texture(
                icon.texture_name(),
                color_image,
                egui::TextureOptions::LINEAR,
            );
            self.hud_icons.insert(icon, texture);
        }
    }

    #[inline]
    pub fn hud_icon(&self, icon: crate::ui::hud::icons::HudIcon) -> Option<&TextureHandle> {
        self.hud_icons.get(&icon)
    }
}
