use super::super::*;
use super::render::seed_hash;

pub(crate) fn render_death_nameplates(
    ui: &mut crate::app::UiState,
    input: &crate::app::InputState,
    tr: &mut crate::render::gpu::TextRenderer,
    sf: f32,
    now: web_time::Instant,
) {
    if ui.death_nameplates.is_empty() {
        return;
    }

    // Keep repainting while death animations are running, to hold high FPS.
    ui.egui_ctx.request_repaint();

    let visual_config = ClientVisualConfig::default();
    let ui_text_scale = visual_config.ui_text_scale;
    let base_premium_size = visual_config.death_nameplate_font_size;

    // Frame-constant SDF text/emoji tuning — read once per frame, not per nameplate.
    let ctx_ref = ui.egui_ctx.clone();
    let face_dilate = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_face_dilate")).unwrap_or(-0.6f32)) * sf;
    let outline_thickness = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_outline_thickness")).unwrap_or(1.0f32)) * sf;
    let shadow_y = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_shadow_y")).unwrap_or(1.5f32)) * sf;
    let underlay_softness = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_underlay_softness")).unwrap_or(0.0f32)) * sf;
    let char_spacing = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_char_spacing")).unwrap_or(0.95f32));
    let font_size_scale = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_size_scale")).unwrap_or(1.67f32));
    let emoji_scale = visual_config.emoji_scale;

    let text_settings = crate::render::gpu::TmpFontSettings {
        face_dilate,
        outline_thickness,
        underlay_offset_y: shadow_y,
        underlay_softness,
    };

    // The logical size the name is actually drawn at; doubles as its layout height (no egui galley).
    let render_font_size = base_premium_size * ui_text_scale * font_size_scale;

    ui.death_nameplates.retain_mut(|anim| {
        let elapsed = now.duration_since(anim.start_time).as_secs_f32();
        let duration = anim.duration.as_secs_f32();
        if elapsed >= duration {
            return false;
        }

        let t = elapsed / duration;

        // --- Layout Coordinates (Subtle Rise, No Sway) ---
        let rise_dist = 2.5 * t * (2.0 - t); // rise by max 2.5 units
        let rise_screen = rise_dist * input.camera_zoom / sf;

        let nx = (input.camera_x + anim.world_x * input.camera_zoom) / sf;
        let ny = (input.camera_y + anim.world_y * input.camera_zoom) / sf - rise_screen;
        let center = egui::pos2(nx, ny);

        // Frustum cull
        if nx < -300.0 || nx > input.screen_w + 300.0 || ny < -300.0 || ny > input.screen_h + 300.0
        {
            return true;
        }

        // Bright visibility curve: rapid fade-in, solid middle, smooth late fade-out.
        let alpha = if t < 0.10 {
            ((t / 0.10) * 255.0) as u8
        } else if t > 0.60 {
            (((1.0 - t) / 0.40) * 255.0).clamp(0.0, 255.0) as u8
        } else {
            255
        };

        if alpha == 0 {
            return true;
        }

        let vibrant_color = egui::Color32::from_rgba_unmultiplied(
            anim.color.r(),
            anim.color.g(),
            anim.color.b(),
            alpha,
        );

        // --- Dove/skull layout ---
        let avatar_size = (base_premium_size * 2.2 * ui_text_scale).round().max(2.0);
        let scale_var = 0.8 + seed_hash(anim.seed, 5) * 0.6;
        let bird_scale = scale_var * (1.0 - t * 0.2);
        let bird_size = (avatar_size * bird_scale).round().max(2.0);

        let angle_offset = (seed_hash(anim.seed, 1) - 0.5) * 0.2;
        let flight_angle = -std::f32::consts::FRAC_PI_2 + angle_offset;

        let base_dist = 15.0 + seed_hash(anim.seed, 2) * 15.0;
        let scale_factor = (input.camera_zoom / sf).clamp(0.2, 3.0);
        let fly_dist = base_dist * scale_factor * t;

        let flight_x = flight_angle.cos() * fly_dist;
        let flight_y = flight_angle.sin() * fly_dist;

        let start_bird_y = center.y - render_font_size / 2.0 - bird_size / 2.0 - 4.0;
        let bird_center_x = center.x + flight_x;
        let bird_center_y = start_bird_y + flight_y;

        let emoji = if anim.by_nuke { "☢️" } else { "🕊️" };

        let display_name = if anim.player_type == sow_core::player::PlayerType::Bot {
            if anim.name.is_empty() {
                format!("Tribe {}", anim.player_id.saturating_sub(199))
            } else {
                anim.name.clone()
            }
        } else {
            sow_core::player::display_name(anim.player_id, &anim.name, anim.player_type)
        };

        let color_arr = vibrant_color.to_array().map(|v| v as f32 / 255.0);
        let outline_color_arr = [0.0f32, 0.0, 0.0, alpha as f32 / 255.0];

        // --- 1. Name, centered ---
        tr.push_string(
            &display_name,
            [center.x * sf, (center.y + render_font_size * 0.35) * sf],
            render_font_size * sf,
            color_arr,
            outline_color_arr,
            text_settings,
            0.5,
            char_spacing,
            emoji_scale,
        );

        // --- 2. Dove (soul) flying upward ---
        tr.push_emoji(
            emoji,
            [bird_center_x * sf, bird_center_y * sf],
            bird_size * 0.5 * sf,
            [1.0, 1.0, 1.0, alpha as f32 / 255.0],
            outline_color_arr,
            outline_thickness,
            shadow_y,
        );

        true
    });
}
