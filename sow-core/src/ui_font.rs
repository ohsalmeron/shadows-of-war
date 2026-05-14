pub const UI_FONT_FILENAME: &str = "Baloo2-Bold.ttf";
pub const UI_FONT_FAMILY: &str = "Baloo2";

// Embed the TTF at compile time
pub static UI_FONT_TTF: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/fonts/Baloo2-Bold.ttf"));
