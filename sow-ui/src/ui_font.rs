// Embed the TTF at compile time. Filename must match sow-dist `UI_FONT_FILE`.
pub static UI_FONT_TTF: &[u8] = sow_core::repo_asset_bytes!("fonts/WorkSans-Black.ttf");
