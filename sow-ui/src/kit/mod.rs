pub mod theme;
pub mod utils;
pub mod widgets;

pub mod ui_font;
pub mod assets;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientPhase {
    Splash,
    MainMenu,
    Playing,
}

pub use assets::{
    atlas_texture, atlas_uv, register_emoji_atlas, register_game_assets, texture_options,
    EMOJI_ATLAS_BYTES,
};
