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
    let rect_aspect = rect_size.x / rect_size.y;
    let tex_aspect = tex_size.x / tex_size.y;

    if tex_aspect > rect_aspect {
        let u_width = rect_aspect / tex_aspect;
        let u_start = (1.0 - u_width) / 2.0;
        egui::Rect::from_min_max(egui::pos2(u_start, 0.0), egui::pos2(u_start + u_width, 1.0))
    } else {
        let v_height = tex_aspect / rect_aspect;
        let v_start = (1.0 - v_height) / 2.0;
        egui::Rect::from_min_max(
            egui::pos2(0.0, v_start),
            egui::pos2(1.0, v_start + v_height),
        )
    }
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
pub fn thumbnail_square_side_bounded(
    available_width: f32,
    max_height: f32,
    compact: bool,
) -> f32 {
    let width_side = thumbnail_square_side(available_width, compact);
    if max_height <= 0.0 {
        return width_side;
    }
    width_side.min(max_height * 0.92)
}

/// Full-opaque map thumbnail (albedo only — no alpha compositing tricks).
pub fn draw_map_thumbnail(
    painter: &Painter,
    texture: TextureId,
    rect: egui::Rect,
    brightness: f32,
) {
    draw_map_thumbnail_uv(
        painter,
        texture,
        rect,
        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        brightness,
    );
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
