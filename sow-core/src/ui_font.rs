pub const UI_FONT_FILENAME: &str = "CinzelDecorative-Black.ttf";
pub const UI_FONT_FAMILY: &str = "CinzelDecorative";

// Embed the TTF at compile time
pub static UI_FONT_TTF: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/fonts/CinzelDecorative-Black.ttf"));
