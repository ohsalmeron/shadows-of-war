use super::*;

impl AssetLoader {
    pub(crate) fn leader_retry_ready(&self, key: LeaderPortraitKey, now: Instant) -> bool {
        self.leader_retry_state
            .get(&key)
            .map(|state| !state.permanent && state.next_retry_at <= now)
            .unwrap_or(true)
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn decode_and_upload_webp(ctx: &egui::Context, name: &str, bytes: &[u8]) -> egui::TextureHandle {
    let mut image = image::load_from_memory(bytes).expect("Failed to load UI asset");

    if image.width() > 2048 || image.height() > 2048 {
        image = image.resize(2048, 2048, image::imageops::FilterType::Triangle);
    }

    let image_rgba = image.to_rgba8();
    let size = [image_rgba.width() as _, image_rgba.height() as _];
    let pixels = image_rgba.as_flat_samples();
    let color_image = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
    ctx.load_texture(name, color_image, egui::TextureOptions::LINEAR)
}

#[cfg(test)]
mod tests {
}
