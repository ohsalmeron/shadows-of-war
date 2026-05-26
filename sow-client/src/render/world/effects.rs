use super::*;

#[inline]
fn hash_xorshift(val: u32) -> f32 {
    let mut x = val;
    if x == 0 {
        x = 1;
    }
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    (x as f32) / (u32::MAX as f32)
}

#[allow(unused_variables)]
pub(crate) fn render(
    ui: &mut crate::app::UiState,
    sim: &crate::app::SimState,
    input: &crate::app::InputState,
    time: &crate::app::TimeState,
    gfx: &crate::app::GraphicsState,
    ctx: &RenderContext,
) {
    let painter = ctx.painter;
    let sf = ctx.sf;
    let zoom_scaled = ctx.zoom_scaled;
    let player_colors = ctx.player_colors;
    let dot_r = ctx.dot_r;
    let current_tick = ctx.current_tick;
    let wall_secs = ctx.wall_secs;
    let visible_players = ctx.visible_players;
    let terrain = ctx.terrain;

    let is_water = |tile_idx: u32| {
        let t = terrain.get(tile_idx as usize).copied().unwrap_or(0);
        (t & 0x80) == 0
    };

    if let Some(snap) = &sim.current_snapshot {
        let current_time = web_time::Instant::now();

        // --- Layer 5: Fallout Zones & Explosions ---
        ui.fallout_zones.retain(|fz| {
            let elapsed = current_time.duration_since(fz.start_time).as_secs_f32();
            let duration = 15.0; // Contamination duration
            if elapsed >= duration {
                return false;
            }

            let p = elapsed / duration;
            let alpha_p = (1.0 - p).max(0.0);

            let pulse = (wall_secs * 3.0).sin() as f32 * 0.15 + 0.85;
            let base_alpha = 45.0 * alpha_p * pulse;

            let fz_world_x = fz.x + 0.5 + (fz.y as i32 % 2) as f32 * 0.5;
            let fz_world_y = (fz.y + 0.5) * 0.8660254_f32;
            let screen_x = (input.camera_x + fz_world_x * input.camera_zoom) / sf;
            let screen_y = (input.camera_y + fz_world_y * input.camera_zoom) / sf;
            let center = egui::pos2(screen_x, screen_y);
            let radius = fz.radius * zoom_scaled;

            // Glowing green contaminated aura
            painter.circle_filled(
                center,
                radius,
                egui::Color32::from_rgba_unmultiplied(60, 220, 90, base_alpha as u8),
            );

            // CRISP high-contrast radioactive outer border
            let border_color =
                egui::Color32::from_rgba_unmultiplied(100, 255, 140, (base_alpha * 2.0) as u8);
            painter.circle_stroke(center, radius, egui::Stroke::new(1.0f32, border_color));

            // Deterministic floating glowing radioactive green dust particles!
            let seed = (fz.x * 123.45 + fz.y * 678.9) as i32;
            let particle_count = (fz.radius * 0.5) as i32;
            for i in 0..particle_count {
                let h1 =
                    hash_xorshift(seed as u32 ^ (i as u32).wrapping_mul(2654435761)) * 2.0 - 1.0;
                let h2 = hash_xorshift(
                    (seed as u32)
                        .wrapping_add(i as u32)
                        .wrapping_mul(3405691582),
                ) * 2.0
                    - 1.0;
                let h3 =
                    hash_xorshift((seed as u32).wrapping_add(i as u32).wrapping_mul(123456789));

                let mut dx = h1;
                let mut dy = h2;
                let len_sq = dx * dx + dy * dy;
                if len_sq > 1.0 {
                    let len = len_sq.sqrt();
                    dx /= len;
                    dy /= len;
                }

                let px = fz.x + dx * fz.radius;
                let py = fz.y + dy * fz.radius;

                let speed = 0.4 + h3 * 0.8;
                let drift_y = (wall_secs as f32 * speed) % 6.0;
                let py_drifted = py - drift_y;

                let p_world_x = px + 0.5 + (py_drifted as i32 % 2) as f32 * 0.5;
                let p_world_y = (py_drifted + 0.5) * 0.8660254_f32;
                let p_screen_x = (input.camera_x + p_world_x * input.camera_zoom) / sf;
                let p_screen_y = (input.camera_y + p_world_y * input.camera_zoom) / sf;

                let particle_alpha = (base_alpha * (1.0 - drift_y / 6.0)).max(0.0) as u8;

                painter.circle_filled(
                    egui::pos2(p_screen_x, p_screen_y),
                    (1.2_f32 * zoom_scaled).max(1.0_f32),
                    egui::Color32::from_rgba_unmultiplied(120, 255, 150, particle_alpha),
                );
            }

            true
        });

        ui.active_explosions.retain(|exp| {
            let elapsed = current_time.duration_since(exp.start_time).as_secs_f32();
            let duration = match exp.kind {
                crate::app::ExplosionKind::Hydrogen => 1.2,
                crate::app::ExplosionKind::Atom => 0.7,
                crate::app::ExplosionKind::MIRVWarhead => 0.4,
            };
            if elapsed >= duration {
                return false;
            }

            let p = elapsed / duration;

            let exp_world_x = exp.x + 0.5 + (exp.y as i32 % 2) as f32 * 0.5;
            let exp_world_y = (exp.y + 0.5) * 0.8660254_f32;
            let screen_x = (input.camera_x + exp_world_x * input.camera_zoom) / sf;
            let screen_y = (input.camera_y + exp_world_y * input.camera_zoom) / sf;
            let center = egui::pos2(screen_x, screen_y); // 1. Blinding white-hot flash (first 12% of duration, soft and clean)
            if p < 0.12 {
                let flash_alpha = ((1.0 - p / 0.12) * 160.0) as u8;
                painter.circle_filled(
                    center,
                    exp.max_radius * 2.0 * zoom_scaled,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, flash_alpha),
                );
            }

            // 2. Expanding Shockwave Circle (fast ease-out, thin and clean)
            let shockwave_max = exp.max_radius * 1.5;
            let shockwave_radius = (1.0 - (1.0 - p).powi(3)) * shockwave_max * zoom_scaled;
            let shockwave_alpha = 1.0 - p;
            let shockwave_color = egui::Color32::from_rgba_unmultiplied(
                255,
                255,
                255,
                (shockwave_alpha * 150.0) as u8,
            );
            painter.circle_stroke(
                center,
                shockwave_radius,
                egui::Stroke::new(1.2_f32, shockwave_color),
            );

            true
        });
    }
}
