// Embed the TTF at compile time
pub static UI_FONT_TTF: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/fonts/StackSansNotch-Medium.ttf"));
pub static UI_FONT_BOLD_TTF: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/fonts/StackSansNotch-Bold.ttf"));
pub static UI_FONT_THIN_TTF: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../assets/fonts/StackSansNotch-ExtraLight.ttf"));
