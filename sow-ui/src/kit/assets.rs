use egui::{Context, Rect, TextureHandle, TextureOptions};
use sow_data::emoji::{ATLAS_HEIGHT, ATLAS_WIDTH, lookup};

#[macro_export]
macro_rules! repo_asset_bytes {
    ($path:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../assets/",
            $path
        ))
    };
}

pub static EMOJI_ATLAS_BYTES: &[u8] = repo_asset_bytes!("gameplay/emoji/atlas.webp");

const ATLAS_TEX_ID: &str = "sow_emoji_atlas";

pub fn atlas_texture(ctx: &Context) -> Option<TextureHandle> {
    let id = egui::Id::new(ATLAS_TEX_ID);
    if let Some(tex) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(tex);
    }
    let rgba = image::load_from_memory(EMOJI_ATLAS_BYTES).ok()?.to_rgba8();
    let size = [rgba.width() as _, rgba.height() as _];
    let pixels = rgba.as_flat_samples();
    let color = egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
    let handle = ctx.load_texture(ATLAS_TEX_ID, color, texture_options());
    ctx.data_mut(|d| d.insert_temp(id, handle.clone()));
    Some(handle)
}

pub fn register_emoji_atlas(ctx: &Context) {
    let _ = atlas_texture(ctx);
}

pub fn register_game_assets(ctx: &Context) {
    register_emoji_atlas(ctx);
}

pub fn atlas_uv(emoji: &str) -> Option<Rect> {
    let rect = lookup(emoji).or_else(|| {
        let stripped = strip_fe0f(emoji);
        if stripped == emoji {
            None
        } else {
            lookup(stripped)
        }
    })?;
    Some(Rect::from_min_max(
        egui::pos2(
            rect.x as f32 / ATLAS_WIDTH as f32,
            rect.y as f32 / ATLAS_HEIGHT as f32,
        ),
        egui::pos2(
            (rect.x + rect.w) as f32 / ATLAS_WIDTH as f32,
            (rect.y + rect.h) as f32 / ATLAS_HEIGHT as f32,
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

pub fn texture_options() -> TextureOptions {
    TextureOptions::LINEAR
}
