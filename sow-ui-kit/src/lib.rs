pub mod hud;
pub mod theme;
pub mod utils;
pub mod widgets;

pub mod ui_font;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClientPhase {
    Splash,
    MainMenu,
    Playing,
}

pub use sow_assets::repo_asset_bytes;
pub use sow_assets_ui::{
    atlas_texture, atlas_uv, register_emoji_atlas, register_game_assets, texture_options,
};
