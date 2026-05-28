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

        // Fallout zones are rendered GPU-side in the map shader (fallout_slots).

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

            // 3. Volumetric Fireball & Rising Mushroom Cloud (Deterministic egui particle burst)
            let seed = (exp.x * 374.0 + exp.y * 668.0) as i32;
            let num_particles = match exp.kind {
                crate::app::ExplosionKind::Hydrogen => 60,
                crate::app::ExplosionKind::Atom => 36,
                crate::app::ExplosionKind::MIRVWarhead => 16,
            };

            for i in 0..num_particles {
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
                let h4 =
                    hash_xorshift((seed as u32).wrapping_add(i as u32).wrapping_mul(987654321));

                let mut dx = h1;
                let mut dy = h2;
                let len_sq = dx * dx + dy * dy;
                if len_sq > 1.0 {
                    let len = len_sq.sqrt();
                    dx /= len;
                    dy /= len;
                }

                // Easing for smooth expansion
                let expansion = 1.0 - (1.0 - p).powi(2);
                let speed = 0.4 + h3 * 1.2;
                let size_mult = 0.5 + h4 * 0.7;

                // Create stem vs cap particles for mushroom cloud silhouette
                let is_stem = i % 3 == 0;
                let (p_dist_x, p_dist_y, rise_mult) = if is_stem {
                    // Stem particles stay close to center horizontally, rise vertically
                    (
                        dx * expansion * speed * exp.max_radius * 0.25,
                        dy * expansion * speed * exp.max_radius * 0.1,
                        1.2,
                    )
                } else {
                    // Cap particles expand outward horizontally and vertically
                    (
                        dx * expansion * speed * exp.max_radius * 0.9,
                        dy * expansion * speed * exp.max_radius * 0.7
                            - (expansion * exp.max_radius * 0.25),
                        1.0,
                    )
                };

                let rise = p * (1.1 + h3 * 0.9) * exp.max_radius * 0.4 * rise_mult;

                let px = exp.x + p_dist_x;
                let py = exp.y + p_dist_y - rise;

                let p_world_x = px + 0.5 + (py as i32 % 2) as f32 * 0.5;
                let p_world_y = (py + 0.5) * 0.8660254_f32;
                let p_screen_x = (input.camera_x + p_world_x * input.camera_zoom) / sf;
                let p_screen_y = (input.camera_y + p_world_y * input.camera_zoom) / sf;

                // Fireball color stage transitions
                let (r, g, b, alpha) = if p < 0.15 {
                    let t = p / 0.15;
                    (
                        255,
                        (255.0 - t * 75.0) as u8,
                        (255.0 - t * 215.0) as u8,
                        (220.0 * (1.0 - p)) as u8,
                    )
                } else if p < 0.4 {
                    let t = (p - 0.15) / 0.25;
                    (
                        255,
                        (180.0 - t * 120.0) as u8,
                        40,
                        (200.0 * (1.0 - p)) as u8,
                    )
                } else if p < 0.7 {
                    let t = (p - 0.4) / 0.3;
                    (
                        (255.0 - t * 175.0) as u8,
                        (60.0 + t * 20.0) as u8,
                        (40.0 + t * 40.0) as u8,
                        (160.0 * (1.0 - p)) as u8,
                    )
                } else {
                    (80, 80, 80, (120.0 * (1.0 - p)) as u8)
                };

                // Radius starts small, swells up volumetric, and scales down slightly as it fades into smoke
                let particle_radius =
                    exp.max_radius * 0.25 * size_mult * (0.8 + expansion * 0.6) * zoom_scaled;
                painter.circle_filled(
                    egui::pos2(p_screen_x, p_screen_y),
                    particle_radius.max(1.5),
                    egui::Color32::from_rgba_unmultiplied(r, g, b, alpha),
                );
            }

            true
        });
    }
}
