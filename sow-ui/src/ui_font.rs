// Embed the TTF at compile time
pub static UI_FONT_TTF: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/fonts/StackSansNotch-Light.ttf"));
