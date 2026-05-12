// Client-only visual configuration.
// Tweak these values and `cargo run --bin sow-client` to see changes without recompiling the server.
pub struct ClientVisualConfig {
    pub shader_terrain_sharpness: f32,
    pub shader_interior_alpha: f32,
    pub shader_border_alpha: f32,
    pub shader_border_thickness: f32,
    
    pub ui_lod_2_zoom: f32,
    pub ui_lod_3_zoom: f32,
    pub ui_lod_dot_radius: f32,
    
    // Master volume for nameplate text sizes.
    // 1.0 = Original size, 0.5 = Half size, 2.0 = Double size.
    pub ui_text_scale: f32,
    
    // Algorithmic Visual Effects (1.0 = Standard, 0.0 = Disabled)
    pub effect_shockwave_intensity: f32,
    pub effect_border_breathe: f32,
    pub effect_energy_flow: f32,
}

impl Default for ClientVisualConfig {
    fn default() -> Self {
        Self {
            // Shaders
            shader_terrain_sharpness: 0.0001,
            shader_interior_alpha: 0.95,
            shader_border_alpha: 0.95,
            shader_border_thickness: 0.1,
            
            // LODs & Visibility
            ui_lod_2_zoom: 10.0,
            ui_lod_3_zoom: 20.0,
            ui_lod_dot_radius: 2.0,
            
            // Nameplates
            ui_text_scale: 0.25,
            
            // Effects
            effect_shockwave_intensity: 1.0,
            effect_border_breathe: 1.0,
            effect_energy_flow: 1.0,
        }
    }
}
