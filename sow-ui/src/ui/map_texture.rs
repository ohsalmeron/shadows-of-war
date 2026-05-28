//! Map thumbnail decode/draw for main-menu lobby previews.

use egui::{Color32, Painter, TextureId};
use image::RgbaImage;

/// Turn near-black backdrop pixels transparent; keep original RGB and edge alpha intact.
pub fn key_black_backdrop_rgba8(pixels: &mut [u8]) {
    for px in pixels.chunks_exact_mut(4) {
        if px[0] < 18 && px[1] < 18 && px[2] < 18 {
            px[3] = 0;
        }
    }
}

pub fn key_black_backdrop_image(image: &mut RgbaImage) {
    key_black_backdrop_rgba8(image.as_mut());
}

pub fn color_image_from_map_thumbnail_bytes(bytes: &[u8]) -> Option<egui::ColorImage> {
    let mut image = image::load_from_memory(bytes).ok()?.to_rgba8();
    key_black_backdrop_image(&mut image);
    let size = [image.width() as _, image.height() as _];
    let pixels = image.as_flat_samples();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        size,
        pixels.as_slice(),
    ))
}

/// Standard alpha-blended map thumbnail (no fake additive / luminance keying).
pub fn draw_map_thumbnail(
    painter: &Painter,
    texture: TextureId,
    rect: egui::Rect,
    brightness: f32,
) {
    let tint = if brightness > 1.01 {
        Color32::WHITE.gamma_multiply(brightness.clamp(1.0, 1.12))
    } else {
        Color32::WHITE
    };
    painter.image(
        texture,
        rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        tint,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn black_backdrop_becomes_transparent() {
        let mut px = [0u8, 0, 0, 255];
        key_black_backdrop_rgba8(&mut px);
        assert_eq!(px[3], 0);
    }

    #[test]
    fn colored_pixels_keep_alpha() {
        let mut px = [40u8, 200, 60, 200];
        key_black_backdrop_rgba8(&mut px);
        assert_eq!(px[3], 200);
    }
}
