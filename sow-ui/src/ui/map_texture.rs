//! Map thumbnail decode/draw for main-menu lobby previews.

use egui::{Color32, Painter, TextureId};
use image::RgbaImage;

/// Force every pixel fully opaque — thumbnails are drawn as solid albedo, no alpha keying.
pub fn force_opaque_rgba8(pixels: &mut [u8]) {
    for px in pixels.chunks_exact_mut(4) {
        px[3] = 255;
    }
}

pub fn force_opaque_image(image: &mut RgbaImage) {
    force_opaque_rgba8(image.as_mut());
}

pub fn color_image_from_map_thumbnail_bytes(bytes: &[u8]) -> Option<egui::ColorImage> {
    let mut image = image::load_from_memory(bytes).ok()?.to_rgba8();
    force_opaque_image(&mut image);
    let size = [image.width() as _, image.height() as _];
    let pixels = image.as_flat_samples();
    Some(egui::ColorImage::from_rgba_unmultiplied(
        size,
        pixels.as_slice(),
    ))
}

/// Center-crop UV for fitting a texture into a destination rect without stretching.
pub fn cover_uv(rect_size: egui::Vec2, tex_size: egui::Vec2) -> egui::Rect {
    crate::widgets::avatar_picker::calculate_cover_uv(
        rect_size,
        tex_size,
        crate::widgets::avatar_picker::CoverAnchor::Center,
    )
}

/// Map thumbnails are always displayed as 1:1 squares (side length in logical pixels).
pub fn thumbnail_square_side(available_width: f32, compact: bool) -> f32 {
    if compact {
        available_width
    } else {
        available_width.clamp(120.0, 200.0)
    }
}

/// Square thumbnail side capped by vertical budget so lobby cards never overflow the flex middle.
pub fn thumbnail_square_side_bounded(available_width: f32, max_height: f32, compact: bool) -> f32 {
    let width_side = thumbnail_square_side(available_width, compact);
    if max_height <= 0.0 {
        return width_side;
    }
    // Only shrink when vertical budget is tight; never grow past width-based size.
    width_side.min((max_height - 2.0).max(48.0))
}


/// Map thumbnail with explicit UV (use [`cover_uv`] for center-cropped fit).
pub fn draw_map_thumbnail_uv(
    painter: &Painter,
    texture: TextureId,
    rect: egui::Rect,
    uv: egui::Rect,
    brightness: f32,
) {
    let tint = if brightness > 1.01 {
        Color32::WHITE.gamma_multiply(brightness.clamp(1.0, 1.2))
    } else {
        Color32::WHITE
    };
    painter.image(texture, rect, uv, tint);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_forces_full_opacity() {
        let mut px = [10u8, 20, 30, 0];
        force_opaque_rgba8(&mut px);
        assert_eq!(px[3], 255);
    }

    #[test]
    fn colored_pixels_keep_rgb() {
        let mut px = [40u8, 200, 60, 128];
        force_opaque_rgba8(&mut px);
        assert_eq!(px[0..3], [40, 200, 60]);
        assert_eq!(px[3], 255);
    }
}
