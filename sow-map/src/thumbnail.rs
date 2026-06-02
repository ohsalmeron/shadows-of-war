//! Center-cropped square map thumbnails for lobby previews.

use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ExtendedColorType, RgbaImage};
use std::path::Path;

/// Lobby / catalog preview edge length (keep small for WASM/CDN).
pub const THUMBNAIL_SIZE: u32 = 512;

/// Target lossy WebP quality when re-encoding via `cwebp` (see `reencode_thumbnail_file`).
pub const THUMBNAIL_WEBP_QUALITY: u8 = 80;

pub fn encode_square_thumbnail_webp(img: &DynamicImage) -> Result<Vec<u8>, String> {
    let cropped = center_crop_square(img);
    let resized = cropped.resize(THUMBNAIL_SIZE, THUMBNAIL_SIZE, FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    encode_lossless_webp(&rgba)
}

pub fn write_square_thumbnail(img: &DynamicImage, path: &Path) -> Result<(), String> {
    let bytes = encode_square_thumbnail_webp(img)?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())?;
    if let Err(e) = reencode_thumbnail_file(path) {
        log::warn!("thumbnail cwebp pass skipped ({}): {}", path.display(), e);
    }
    Ok(())
}

pub fn write_square_thumbnail_from_rgba(rgba: &RgbaImage, path: &Path) -> Result<(), String> {
    write_square_thumbnail(&DynamicImage::ImageRgba8(rgba.clone()), path)
}

pub fn write_square_thumbnail_from_pixels(
    width: u32,
    height: u32,
    pixels: &[[u8; 4]],
    path: &Path,
) -> Result<(), String> {
    let rgba = RgbaImage::from_raw(width, height, pixels.iter().flat_map(|p| p.iter().copied()).collect())
        .ok_or_else(|| "thumbnail pixel buffer size mismatch".to_string())?;
    write_square_thumbnail_from_rgba(&rgba, path)
}

/// Render packed terrain bytes into an RGBA preview (downscaled if very large).
pub fn terrain_preview_image(width: u32, height: u32, terrain: &[u8]) -> DynamicImage {
    let max_dim = 2048u32;
    let longest = width.max(height).max(1);
    let scale = if longest > max_dim {
        max_dim as f64 / longest as f64
    } else {
        1.0
    };
    let pw = ((width as f64 * scale).ceil() as u32).max(1);
    let ph = ((height as f64 * scale).ceil() as u32).max(1);
    let mut rgba = vec![106u8; (pw * ph * 4) as usize];
    rgba.chunks_mut(4).for_each(|px| px[3] = 255);

    for py in 0..ph {
        for px in 0..pw {
            let sx = ((px as f64 / scale).floor() as u32).min(width.saturating_sub(1));
            let sy = ((py as f64 / scale).floor() as u32).min(height.saturating_sub(1));
            let idx = (sy * width + sx) as usize;
            let color = terrain
                .get(idx)
                .copied()
                .map(color_from_terrain_byte)
                .unwrap_or([70, 132, 180, 255]);
            let o = ((py * pw + px) * 4) as usize;
            rgba[o..o + 4].copy_from_slice(&color);
        }
    }

    DynamicImage::ImageRgba8(
        RgbaImage::from_raw(pw, ph, rgba).expect("terrain preview buffer size"),
    )
}

fn center_crop_square(img: &DynamicImage) -> DynamicImage {
    let (w, h) = (img.width(), img.height());
    let side = w.min(h);
    let x = (w - side) / 2;
    let y = (h - side) / 2;
    img.crop_imm(x, y, side, side)
}

fn encode_lossless_webp(rgba: &RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    WebPEncoder::new_lossless(&mut out)
        .encode(rgba, rgba.width(), rgba.height(), ExtendedColorType::Rgba8)
        .map_err(|e| format!("webp encode: {e}"))?;
    Ok(out)
}

/// Lossy pass with system `cwebp` when available (smaller committed assets).
pub fn reencode_thumbnail_file(path: &Path) -> Result<(), String> {
    let cwebp = which_cwebp()?;
    let tmp = path.with_extension("webp.tmp");
    let status = std::process::Command::new(&cwebp)
        .args([
            "-q",
            &THUMBNAIL_WEBP_QUALITY.to_string(),
            "-resize",
            &THUMBNAIL_SIZE.to_string(),
            &THUMBNAIL_SIZE.to_string(),
            path.to_str().ok_or_else(|| "invalid thumbnail path".to_string())?,
            "-o",
            tmp.to_str().ok_or_else(|| "invalid tmp path".to_string())?,
        ])
        .status()
        .map_err(|e| format!("cwebp: {e}"))?;
    if !status.success() {
        return Err(format!("cwebp failed for {}", path.display()));
    }
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn which_cwebp() -> Result<String, String> {
    if let Ok(p) = std::process::Command::new("cwebp")
        .arg("-version")
        .output()
    {
        if p.status.success() {
            return Ok("cwebp".to_string());
        }
    }
    for p in ["/usr/bin/cwebp", "/usr/local/bin/cwebp"] {
        if std::path::Path::new(p).is_executable() {
            return Ok(p.to_string());
        }
    }
    Err("cwebp not found (install libwebp-utils)".to_string())
}

trait PathExt {
    fn is_executable(&self) -> bool;
}

impl PathExt for std::path::Path {
    fn is_executable(&self) -> bool {
        std::fs::metadata(self)
            .map(|m| m.is_file())
            .unwrap_or(false)
    }
}

fn color_from_terrain_byte(byte: u8) -> [u8; 4] {
    let is_land = (byte & 0x80) != 0;
    let shoreline = (byte & 0x40) != 0;
    let magnitude = (byte & 0x1f) as f64;

    if !is_land {
        if shoreline {
            return [100, 143, 255, 255];
        }
        let water_adj = (11.0 - (magnitude / 2.0).min(10.0) - 10.0) as i32;
        return [
            (70 + water_adj).clamp(0, 255) as u8,
            (132 + water_adj).clamp(0, 255) as u8,
            (180 + water_adj).clamp(0, 255) as u8,
            255,
        ];
    }

    if shoreline {
        return [204, 203, 158, 255];
    }

    if magnitude < 10.0 {
        let adj = 220.0 - 2.0 * magnitude;
        [190, adj.clamp(0.0, 255.0) as u8, 138, 255]
    } else if magnitude < 20.0 {
        let adj = 2.0 * magnitude;
        [
            (200.0 + adj).clamp(0.0, 255.0) as u8,
            (183.0 + adj).clamp(0.0, 255.0) as u8,
            (138.0 + adj).clamp(0.0, 255.0) as u8,
            255,
        ]
    } else {
        let adj = (230.0 + magnitude / 2.0).floor().clamp(0.0, 255.0) as u8;
        [adj, adj, adj, 255]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::GenericImageView;

    #[test]
    fn square_thumbnail_is_512_and_compact() {
        let img = DynamicImage::new_rgba8(2800, 1448);
        let bytes = encode_square_thumbnail_webp(&img).unwrap();
        let decoded = image::load_from_memory(&bytes).unwrap();
        assert_eq!(decoded.dimensions(), (THUMBNAIL_SIZE, THUMBNAIL_SIZE));
        // Lossless 512² should stay well under legacy 1MB 1024² blobs.
        assert!(
            bytes.len() < 400_000,
            "thumbnail too large: {} bytes",
            bytes.len()
        );
    }
}
