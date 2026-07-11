use std::sync::{LazyLock, RwLock};

#[derive(Clone, Debug)]
pub struct DevConfig {
    // Map & Borders
    pub thickness: f32,
    pub darkness: f32,
    pub shore_thickness: f32,
    pub shore_darkness: f32,
    pub conquest_duration: f32,
    pub territory_opacity: f32,
    pub blend_mode: f32,

    // Custom HUD Theme
    pub theme_roundness: f32,
    pub theme_color_top: [f32; 4],
    pub theme_color_bottom: [f32; 4],
    pub theme_color_outline: [f32; 4],
    pub theme_color_glow: [f32; 4],
    pub theme_outline_thickness: f32,
    pub theme_glow_spread: f32,
    pub theme_glow_thickness: f32,

    // Font SDF
    pub font_face_dilate: f32,
    pub font_outline_thickness: f32,
    pub font_shadow_y: f32,
    pub font_underlay_softness: f32,
    pub font_char_spacing: f32,
    pub font_size_scale: f32,
    pub font_offset_x: f32,

    // Objectives Bar
    pub obj_filler_top: [f32; 4],
    pub obj_filler_bottom: [f32; 4],
    pub obj_backplate_top: [f32; 4],
    pub obj_backplate_bottom: [f32; 4],

    // Building
    pub building_scale: f32,
    pub emoji_size_scale: f32,

    // Bunker Laser
    pub bunker_laser_target: bool,
    pub bunker_laser_arc: bool,
    pub bunker_laser_scatter: bool,

    // VFX Flags
    pub vfx_conquer: bool,
    pub vfx_border_breathe: bool,
    pub vfx_energy_flow: bool,
    pub vfx_heartbeat: bool,
    pub vfx_war_fog: bool,
    pub fog_of_war: bool,
    pub vfx_fallout: bool,
    pub vfx_ambient_grade: bool,
    pub vfx_holo_grid: bool,
    pub vfx_tower: bool,
    pub vfx_tower_range: bool,
    pub vfx_attack_lines: bool,
    pub vfx_attack_badges: bool,
    pub vfx_click_markers: bool,
    pub vfx_nuke_preview: bool,
    pub vfx_floating_notices: bool,
    pub vfx_status_emojis: bool,
    pub vfx_upgrade_plate: bool,
    pub vfx_placement_preview: bool,
    pub vfx_world_buildings: bool,
    pub vfx_mover_trails: bool,
    pub vfx_railways: bool,
    pub vfx_fleet_blink: bool,
    pub vfx_bot_avatars: bool,
    pub vfx_nameplate_names: bool,
    pub vfx_nameplate_troops: bool,
}

impl Default for DevConfig {
    fn default() -> Self {
        Self {
            thickness: 0.5,
            darkness: 0.35,
            shore_thickness: 0.35,
            shore_darkness: 1.0,
            conquest_duration: 1.5,
            territory_opacity: 1.0,
            blend_mode: 0.0,

            theme_roundness: 4.0,
            theme_color_top: [55.0 / 255.0, 68.0 / 255.0, 70.0 / 255.0, 240.0 / 255.0],
            theme_color_bottom: [35.0 / 255.0, 45.0 / 255.0, 48.0 / 255.0, 240.0 / 255.0],
            theme_color_outline: [1.0, 1.0, 1.0, 160.0 / 255.0],
            theme_color_glow: [0.0, 0.0, 0.0, 50.0 / 255.0],
            theme_outline_thickness: 0.75,
            theme_glow_spread: 0.0,
            theme_glow_thickness: 1.0,

            font_face_dilate: -0.2,
            font_outline_thickness: 1.4,
            font_shadow_y: 2.0,
            font_underlay_softness: 0.1,
            font_char_spacing: 0.95,
            font_size_scale: 2.0,
            font_offset_x: 16.0,

            obj_filler_top: [96.0 / 255.0, 240.0 / 255.0, 150.0 / 255.0, 1.0],
            obj_filler_bottom: [30.0 / 255.0, 168.0 / 255.0, 96.0 / 255.0, 1.0],
            obj_backplate_top: [13.0 / 255.0, 28.0 / 255.0, 20.0 / 255.0, 1.0],
            obj_backplate_bottom: [4.0 / 255.0, 10.0 / 255.0, 7.0 / 255.0, 1.0],

            building_scale: 0.5,
            emoji_size_scale: 1.4,

            bunker_laser_target: true,
            bunker_laser_arc: true,
            bunker_laser_scatter: false,

            vfx_conquer: true,
            vfx_border_breathe: true,
            vfx_energy_flow: true,
            vfx_heartbeat: true,
            vfx_war_fog: true,
            fog_of_war: true,
            vfx_fallout: true,
            vfx_ambient_grade: true,
            vfx_holo_grid: true,
            vfx_tower: true,
            vfx_tower_range: true,
            vfx_attack_lines: true,
            vfx_attack_badges: true,
            vfx_click_markers: true,
            vfx_nuke_preview: true,
            vfx_floating_notices: true,
            vfx_status_emojis: true,
            vfx_upgrade_plate: true,
            vfx_placement_preview: true,
            vfx_world_buildings: true,
            vfx_mover_trails: true,
            vfx_railways: true,
            vfx_fleet_blink: true,
            vfx_bot_avatars: true,
            vfx_nameplate_names: true,
            vfx_nameplate_troops: true,
        }
    }
}

static GLOBAL: LazyLock<RwLock<DevConfig>> = LazyLock::new(|| RwLock::new(DevConfig::default()));

impl DevConfig {
    pub fn get() -> Self {
        GLOBAL.read().unwrap().clone()
    }

    pub fn set(cfg: Self) {
        *GLOBAL.write().unwrap() = cfg;
    }

    pub fn update(f: impl FnOnce(&mut Self)) {
        let mut guard = GLOBAL.write().unwrap();
        f(&mut guard);
    }
}
