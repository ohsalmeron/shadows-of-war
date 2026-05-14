// Client-only visual configuration.
// Tweak these values and `cargo run --bin sow-client` to see changes without recompiling the server.
pub struct ClientVisualConfig {
    pub ui_lod_dot_radius: f32,
    
    // Master volume for nameplate text sizes.
    // 1.0 = Original size, 0.5 = Half size, 2.0 = Double size.
    pub ui_text_scale: f32,
}

impl Default for ClientVisualConfig {
    fn default() -> Self {
        Self {
            ui_lod_dot_radius: 2.0,
            
            // Nameplates
            ui_text_scale: 1.0,
        }
    }
}
