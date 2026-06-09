use egui::{Context, Rect, TextureOptions};

pub mod manifest;

pub const ATLAS_FILE: &str = "emoji/atlas.webp";
pub const ATLAS_URI: &str = "bytes://emoji/atlas.webp";

static ATLAS_BYTES: &[u8] = crate::repo_asset_bytes!("emoji/atlas.webp");

/// Register the embedded emoji atlas into egui's `bytes://` loader.
pub fn register_emoji_atlas(ctx: &Context) {
    ctx.include_bytes(ATLAS_URI, ATLAS_BYTES);
}

/// Resolve atlas UV rect for a unicode emoji string (handles optional FE0F variants).
pub fn atlas_uv(emoji: &str) -> Option<Rect> {
    let rect = manifest::lookup(emoji).or_else(|| {
        let stripped = strip_fe0f(emoji);
        if stripped == emoji {
            None
        } else {
            manifest::lookup(stripped)
        }
    })?;
    Some(Rect::from_min_max(
        egui::pos2(
            rect.x as f32 / manifest::ATLAS_WIDTH as f32,
            rect.y as f32 / manifest::ATLAS_HEIGHT as f32,
        ),
        egui::pos2(
            (rect.x + rect.w) as f32 / manifest::ATLAS_WIDTH as f32,
            (rect.y + rect.h) as f32 / manifest::ATLAS_HEIGHT as f32,
        ),
    ))
}

fn strip_fe0f(emoji: &str) -> &str {
    if emoji.ends_with('\u{fe0f}') {
        &emoji[..emoji.len() - '\u{fe0f}'.len_utf8()]
    } else {
        emoji
    }
}

/// Nearest-neighbor atlas texture options for pixel emoji.
pub fn texture_options() -> TextureOptions {
    TextureOptions {
        magnification: egui::TextureFilter::Nearest,
        minification: egui::TextureFilter::Nearest,
        ..Default::default()
    }
}
