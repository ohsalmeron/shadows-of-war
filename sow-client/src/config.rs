// Client-only visual configuration.
// Tweak these values and `cargo run --bin sow-client` to see changes without recompiling the server.

pub struct ClientVisualConfig {
    /// Below this `camera_zoom / sf`, world overlays use far LOD (dots, simplified buildings).
    pub far_zoom_lod_threshold: f32,

    pub ui_lod_dot_radius: f32,

    // Master volume for nameplate text sizes.
    // 1.0 = Original size, 0.5 = Half size, 2.0 = Double size.
    pub ui_text_scale: f32,

    pub nameplate_size_grow_rate: f32,

    // Nameplate font px per on-screen world px of territory side length.
    pub nameplate_world_scale: f32,

    // Screen-px ceiling for nameplate fonts (humans; bots cap at 60% of this).
    pub nameplate_max_font: f32,

    // Below this camera zoom (zoom_scaled = camera_zoom / sf), NON-human plates collapse to a
    // cheap disc to declutter the far view. At any closer zoom every plate is drawn full.
    pub nameplate_hide_zoom: f32,

    // Floating gold bounty text ("🪙 +N" on conquer).
    // Base font size in points; bounce scale still applies at runtime.
    pub gold_reward_notice_font_size: f32,
}

impl Default for ClientVisualConfig {
    fn default() -> Self {
        Self {
            far_zoom_lod_threshold: 2.5,
            ui_lod_dot_radius: 2.0,

            // Nameplates
            ui_text_scale: 1.0,
            nameplate_size_grow_rate: 12.0,
            nameplate_world_scale: 0.05,
            nameplate_max_font: 32.0,
            nameplate_hide_zoom: 1.5,
            gold_reward_notice_font_size: 14.0,
        }
    }
}
