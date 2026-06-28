// Client-only visual configuration.
// Tweak these values and `cargo run --bin sow-client` to see changes without recompiling the server.

pub struct ClientVisualConfig {
    /// Below this `camera_zoom / sf`, world overlays use far LOD (dots, simplified buildings).
    pub far_zoom_lod_threshold: f32,

    pub ui_lod_dot_radius: f32,

    // Master volume for nameplate text sizes.
    // 1.0 = Original size, 0.5 = Half size, 2.0 = Double size.
    pub ui_text_scale: f32,

    // Base nameplate sizes
    pub nameplate_my_size: f32,
    pub nameplate_nation_size: f32,
    pub nameplate_tribe_size: f32,
    pub nameplate_premium_size: f32,
    pub nameplate_max_screen_font: f32,
    pub nameplate_size_grow_rate: f32,

    // Death nameplate floater (defeated player name, desktop).
    // Base font size in points; spring shrink still applies at runtime.
    pub death_nameplate_font_size: f32,

    // Floating gold bounty text ("🪙 +N" on conquer).
    // Base font size in points; bounce scale still applies at runtime.
    pub gold_reward_notice_font_size: f32,

    // Global scale multiplier for emojis relative to the text font size.
    pub emoji_scale: f32,
}

impl Default for ClientVisualConfig {
    fn default() -> Self {
        Self {
            far_zoom_lod_threshold: 2.5,
            ui_lod_dot_radius: 2.0,

            // Nameplates
            ui_text_scale: 1.0,
            nameplate_my_size: 14.0,
            nameplate_nation_size: 12.0,
            nameplate_tribe_size: 12.0,
            nameplate_premium_size: 12.0,
            nameplate_max_screen_font: 32.0,
            nameplate_size_grow_rate: 12.0,
            death_nameplate_font_size: 18.0,
            gold_reward_notice_font_size: 14.0,
            emoji_scale: 0.75,
        }
    }
}
