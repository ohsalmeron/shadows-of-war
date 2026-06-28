use super::super::*;
use super::render::seed_hash;

pub(crate) fn render_death_nameplates(
    ui: &mut crate::app::UiState,
    input: &crate::app::InputState,
    gfx: &mut crate::app::GraphicsState,
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

    ui.death_nameplates.retain_mut(|anim| {
        let elapsed = now.duration_since(anim.start_time).as_secs_f32();
        let duration = anim.duration.as_secs_f32();
        if elapsed >= duration {
            return false;
        }

        let t = elapsed / duration;

        // --- Layout Coordinates (Subtle Rise, No Sway) ---
        let rise_dist = 2.5 * t * (2.0 - t); // rise by max 2.5 units (reduced from 6.0)
        let rise_screen = rise_dist * input.camera_zoom / sf;

        let nx = (input.camera_x + anim.world_x * input.camera_zoom) / sf;
        let ny = (input.camera_y + anim.world_y * input.camera_zoom) / sf - rise_screen;
        let center = egui::pos2(nx, ny);

        // Frustum cull
        if nx < -300.0 || nx > input.screen_w + 300.0 || ny < -300.0 || ny > input.screen_h + 300.0
        {
            return true;
        }

        // --- Typography & Size Calculations ---
        let base_premium_size = visual_config.death_nameplate_font_size;

        // Cache the PreparedName to avoid per-frame layout overhead and egui font generation.
        // We use a constant base font size.
        if anim.prepared_name.is_none() {
            let font_size = (base_premium_size * ui_text_scale).round().max(1.0);
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
            anim.prepared_name = Some(sow_ui_kit::widgets::prepare_name(painter, &display_name, &font_id));
        }

        let prepared = anim.prepared_name.as_ref().unwrap();
        let name_size = prepared.size;

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

        // --- Dove/skull layout (shared between GPU and egui fallback) ---
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

        let start_bird_y = center.y - name_size.y / 2.0 - bird_size / 2.0 - 4.0;
        let bird_center_x = center.x + flight_x;
        let bird_center_y = start_bird_y + flight_y;

        let emoji = if anim.by_nuke { "☢️" } else { "🕊️" };

        // --- GPU path: text + emoji via unified pipeline ---
        let mut gpu_rendered = false;
        if let Some(ref mut tr) = gfx.text_renderer {
            gpu_rendered = true;
            let color_arr = vibrant_color.to_array().map(|v| v as f32 / 255.0);
            let outline_color_arr = [0.0f32, 0.0, 0.0, alpha as f32 / 255.0];

            let ctx_ref = painter.ctx();
            let face_dilate = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_face_dilate")).unwrap_or(-0.6f32)) * sf;
            let outline_thickness = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_outline_thickness")).unwrap_or(1.0f32)) * sf;
            let shadow_y = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_shadow_y")).unwrap_or(1.5f32)) * sf;
            let underlay_softness = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_underlay_softness")).unwrap_or(0.0f32)) * sf;
            let char_spacing = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_char_spacing")).unwrap_or(0.95f32));
            let font_size_scale = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_font_size_scale")).unwrap_or(1.67f32));
            let emoji_scale = ctx_ref.data(|d| d.get_temp::<f32>(egui::Id::new("dev_emoji_size_scale")).unwrap_or(1.4f32));

            let settings = crate::render::gpu::TmpFontSettings {
                face_dilate,
                outline_thickness,
                underlay_offset_y: shadow_y,
                underlay_softness,
            };

            let display_name = if anim.player_type == sow_core::player::PlayerType::Bot {
                if anim.name.is_empty() {
                    format!("Tribe {}", anim.player_id.saturating_sub(199))
                } else {
                    anim.name.clone()
                }
            } else {
                sow_core::player::display_name(anim.player_id, &anim.name, anim.player_type)
            };

            tr.push_string(
                &display_name,
                [center.x * sf, (center.y + name_size.y * 0.35) * sf],
                base_premium_size * ui_text_scale * font_size_scale * sf,
                color_arr,
                outline_color_arr,
                settings,
                0.5,
                char_spacing,
                emoji_scale,
            );

            tr.push_emoji(
                emoji,
                [bird_center_x * sf, bird_center_y * sf],
                bird_size * 0.5 * sf,
                [1.0, 1.0, 1.0, alpha as f32 / 255.0],
                outline_color_arr,
                outline_thickness,
                shadow_y,
            );
        }

        if !gpu_rendered {
            // --- 1. Draw Glow Name Centered ---
            let name_x = center.x - name_size.x / 2.0;
            let name_y = center.y - name_size.y / 2.0;
            sow_ui_kit::widgets::paint_prepared_name(
                painter,
                egui::pos2(name_x, name_y),
                egui::Align2::LEFT_TOP,
                prepared,
                vibrant_color,
                true,
            );

            // --- 2. Draw Dove (Soul) Flying Upward Separately ---
            let dove_rect = egui::Rect::from_center_size(
                egui::pos2(bird_center_x, bird_center_y),
                egui::vec2(bird_size, bird_size),
            );
            let soul_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, alpha);
            if !sow_ui_kit::widgets::try_paint_emoji(painter, emoji, dove_rect, soul_color) {
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
        }

        true
    });
}