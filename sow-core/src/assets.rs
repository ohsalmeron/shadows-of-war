use egui::Context;

/// Embed a file from the workspace-root [`assets/`] tree (resolved via `sow-core` manifest).
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

/// Register all game assets (emoji atlas only) into egui.
pub fn register_game_assets(ctx: &Context) {
    crate::emoji::register_emoji_atlas(ctx);
}
