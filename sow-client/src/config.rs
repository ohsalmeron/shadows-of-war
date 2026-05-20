// Client-only visual configuration.
// Tweak these values and `cargo run --bin sow-client` to see changes without recompiling the server.
pub struct ClientVisualConfig {
    pub ui_lod_dot_radius: f32,

    // Master volume for nameplate text sizes.
    // 1.0 = Original size, 0.5 = Half size, 2.0 = Double size.
    pub ui_text_scale: f32,

    // Android UI Theme Settings (Main Menu)
    pub top_bar_color: [u8; 4],
    pub bottom_bar_color: [u8; 4],
}

impl Default for ClientVisualConfig {
    fn default() -> Self {
        Self {
            ui_lod_dot_radius: 2.0,

            // Nameplates
            ui_text_scale: 1.0,

            // Android UI Theme Settings (Main Menu)
            top_bar_color: [15, 15, 20, 255],    // Dark gray
            bottom_bar_color: [15, 15, 20, 255], // Dark gray
        }
    }
}
