//! Shared asset filenames for dist build, CDN sync, and verify.
//! Keep [`sow_ui::ui_font`] in sync when changing the UI font.

/// Canonical UI font under `assets/static/fonts/` (WASM embed + marketing CDN).
pub const UI_FONT_FILE: &str = "WorkSans-Black.ttf";
