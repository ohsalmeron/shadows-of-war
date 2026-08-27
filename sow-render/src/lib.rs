pub mod context;
pub mod map_renderer;
pub mod mover_renderer;
pub mod sprite_atlas;
pub mod text;

pub use context::*;
pub use map_renderer::*;
pub use mover_renderer::*;
pub use sprite_atlas::*;
pub use text::*;

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
