/// Must match `sow-dist/src/assets.rs` `UI_FONT_FILE` and `sow-ui-kit/src/ui_font.rs`.
pub const UI_FONT_FILE: &str = "WorkSans-Black.ttf";

/// Embed a file from the workspace-root [`assets/static/`] tree.
#[macro_export]
macro_rules! repo_asset_bytes {
    ($path:expr) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../assets/static/",
            $path
        ))
    };
}

pub const EMOJI_ATLAS_PATH: &str = "emoji/atlas.webp";
pub const WORLD_MAP_PATH: &str = "maps/world/map.bin.br";
pub const WORLD_THUMBNAIL_PATH: &str = "maps/world/thumbnail.webp";

pub static EMOJI_ATLAS_BYTES: &[u8] = repo_asset_bytes!("emoji/atlas.webp");
pub static WORLD_MAP_BYTES: &[u8] = repo_asset_bytes!("maps/world/map.bin.br");
