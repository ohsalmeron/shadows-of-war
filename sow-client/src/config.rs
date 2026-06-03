// Client-only visual configuration.
// Tweak these values and `cargo run --bin sow-client` to see changes without recompiling the server.

pub struct ClientVisualConfig {
    pub ui_lod_dot_radius: f32,

    // Master volume for nameplate text sizes.
    // 1.0 = Original size, 0.5 = Half size, 2.0 = Double size.
    pub ui_text_scale: f32,

    // Base nameplate sizes
    pub nameplate_my_size: f32,
    pub nameplate_nation_size: f32,
    pub nameplate_tribe_size: f32,
    pub nameplate_premium_size: f32,

    // Disconnect emoji scale relative to the base nameplate size
    pub nameplate_disconnected_emoji_scale: f32,

    // Death nameplate floater (defeated player name, desktop).
    // Base font size in points; spring shrink still applies at runtime.
    pub death_nameplate_font_size: f32,

    // Floating gold bounty text ("💰 +N" on conquer).
    // Base font size in points; bounce scale still applies at runtime.
    pub gold_reward_notice_font_size: f32,

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
            nameplate_my_size: 12.0,
            nameplate_nation_size: 12.0,
            nameplate_tribe_size: 12.0,
            nameplate_premium_size: 12.0,
            nameplate_disconnected_emoji_scale: 8.0,
            death_nameplate_font_size: 40.0,
            gold_reward_notice_font_size: 32.0,

            // Android UI Theme Settings (Main Menu)
            top_bar_color: [15, 15, 20, 255],    // Dark gray
            bottom_bar_color: [15, 15, 20, 0], // Fully transparent
        }
    }
}
