use super::super::*;
use super::emoji::spring_overshoot;
use super::render::seed_hash;

pub(crate) fn render_death_nameplates(
    ui: &mut crate::app::UiState,
    input: &crate::app::InputState,
    sf: f32,
    now: web_time::Instant,
) {
    if ui.death_nameplates.is_empty() {
        return;
    }

    // Always request repaint if we have active death animations running to keep high FPS
    ui.egui_ctx.request_repaint();

    let painter = ui.egui_ctx.layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("death_nameplates"),
    ));
    let painter = &painter;

    let visual_config = ClientVisualConfig::default();
    let ui_text_scale = visual_config.ui_text_scale;

    ui.death_nameplates.retain(|anim| {
        let elapsed = now.duration_since(anim.start_time).as_secs_f32();
        let duration = anim.duration.as_secs_f32();
        if elapsed >= duration {
            return false;
        }

        let t = elapsed / duration;
        let s = anim.seed as f32;

        // --- Layout Coordinates (Smooth Rise & Damped Wobble) ---
        let rise_dist = 6.0 * t * (2.0 - t); // quadratic ease-out rise
        let rise_screen = rise_dist * input.camera_zoom / sf;
        let wobble_x = (elapsed * 5.0 + s).sin() * 15.0 * (1.0 - t); // gentle sway

        let nx = (input.camera_x + anim.world_x * input.camera_zoom) / sf + wobble_x;
        let ny = (input.camera_y + anim.world_y * input.camera_zoom) / sf - rise_screen;
        let center = egui::pos2(nx, ny);

        // Frustum cull
        if nx < -300.0 || nx > input.screen_w + 300.0 || ny < -300.0 || ny > input.screen_h + 300.0
        {
            return true;
        }

        // Spring entry scale (pops up smoothly from shrunk state)
        let entry_scale = spring_overshoot((t / 0.3).clamp(0.0, 1.0));

        // --- Typography & Size Calculations ---
        const NAMEPLATE_RENDER_SCALE: f32 = 0.4;
        let base_premium_size = if anim.player_type == sow_core::player::PlayerType::Bot {
            visual_config.death_nameplate_font_size
        } else if anim.nameplate_size > 0.1 {
            anim.nameplate_size * NAMEPLATE_RENDER_SCALE
        } else {
            visual_config.death_nameplate_font_size
        };

        // QUANTIZATION: Round sizes to nearest integers
        let font_size = (base_premium_size * ui_text_scale * entry_scale)
            .round()
            .max(1.0);
        let font_id = egui::FontId::proportional(font_size);

        let display_name = if anim.player_type == sow_core::player::PlayerType::Bot {
            if anim.name.is_empty() {
                format!("Tribe {}", anim.player_id.saturating_sub(199))
            } else {
                anim.name.clone()
            }
        } else {
            sow_core::player::display_name(anim.player_id, &anim.name, anim.player_type)
        };

        let name_size = crate::hud::nameplate::name_label_size(painter, &display_name, &font_id);

        // Bright visibility curve: rapid fade-in, solid middle, smooth late fade-out (no early muddy darks)
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

        // --- 1. Draw Glow Name Centered ---
        let name_x = center.x - name_size.x / 2.0;
        let name_y = center.y - name_size.y / 2.0;

        crate::hud::nameplate::paint_glow_name_label(
            painter,
            egui::pos2(name_x, name_y),
            &display_name,
            font_id,
            vibrant_color,
        );

        // --- 2. Draw Dove (Soul) Flying Upward Separately ---
        let avatar_size = (base_premium_size * 2.2 * ui_text_scale).round().max(2.0);
        let scale_var = 0.8 + seed_hash(anim.seed, 5) * 0.6;
        let bird_scale = entry_scale * scale_var * (1.0 - t * 0.2);
        let bird_size = (avatar_size * bird_scale).round().max(2.0);

        let angle_offset = (seed_hash(anim.seed, 1) - 0.5) * 1.0;
        let flight_angle = -std::f32::consts::FRAC_PI_2 + angle_offset;

        let base_dist = 60.0 + seed_hash(anim.seed, 2) * 120.0;
        let scale_factor = (input.camera_zoom / sf).clamp(0.2, 3.0);
        let fly_dist = base_dist * scale_factor * t.powf(1.2);

        let flight_x = flight_angle.cos() * fly_dist;
        let flight_y = flight_angle.sin() * fly_dist;

        let flutter_freq = 12.0 + seed_hash(anim.seed, 3) * 16.0;
        let flutter_amp = (4.0 + seed_hash(anim.seed, 4) * 8.0) * scale_factor;
        let flutter_x = (elapsed * flutter_freq).sin() * flutter_amp;

        let start_bird_y = center.y - name_size.y / 2.0 - bird_size / 2.0 - 4.0;
        let bird_center_x = center.x + flight_x + flutter_x;
        let bird_center_y = start_bird_y + flight_y;

        let dove_rect = egui::Rect::from_center_size(
            egui::pos2(bird_center_x, bird_center_y),
            egui::vec2(bird_size, bird_size),
        );
        let soul_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);

        let emoji = if anim.by_nuke { "☢️" } else { "🕊️" };
        if !sow_ui::widgets::try_paint_emoji(painter, emoji, dove_rect, soul_color) {
            let emoji_galley = painter.layout_no_wrap(
                emoji.to_owned(),
                egui::FontId::proportional(bird_size),
                soul_color,
            );
            let emoji_pos = egui::pos2(
                dove_rect.center().x - emoji_galley.size().x / 2.0,
                dove_rect.center().y - emoji_galley.size().y / 2.0,
            );
            painter.galley(emoji_pos, emoji_galley, soul_color);
        }

        true
    });
}
