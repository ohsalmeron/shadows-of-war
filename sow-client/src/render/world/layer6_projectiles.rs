use super::*;

pub(crate) fn render(_ui: &mut crate::app::UiState, sim: &crate::app::SimState, input: &crate::app::InputState, time: &crate::app::TimeState, _gfx: &crate::app::GraphicsState, ctx: &RenderContext) {
    let painter = ctx.painter;
    let sf = ctx.sf;

    // Tile index → world position (hex stagger)
    let tile_to_world = |tile: u32, map_w: u32| -> (f32, f32) {
        let tx = (tile % map_w) as f32;
        let ty = (tile / map_w) as f32;
        let wx = tx + 0.5 + (ty as i32 % 2) as f32 * 0.5;
        let wy = (ty + 0.5) * 0.8660254_f32;
        (wx, wy)
    };

    if let Some(snap) = &sim.current_snapshot {
        let map_w = snap.players.first().map(|_| sim.map_w).unwrap_or(1) as u32;

        // --- Layer 6: Projectiles (Nukes, SAM Missiles) ---
        for proj in &snap.projectiles {
            if proj.path.is_empty() {
                continue;
            }

            // Compute interpolated world position from path tiles
            let cursor = proj.path_cursor.min(proj.path.len() - 1);
            let prev_cursor = cursor.saturating_sub(1);
            let (wx_curr, wy_curr) = tile_to_world(proj.path[cursor], map_w);
            let (wx_prev, wy_prev) = tile_to_world(proj.path[prev_cursor], map_w);

            // Smooth interpolation between prev and current tile
            let sim_dt = std::time::Instant::now().duration_since(time.last_tick).as_secs_f32();
            let tick_dur = time.tick_interval.as_secs_f32().max(0.01);
            let mut t = (sim_dt / tick_dur).clamp(0.0, 1.0);
            t = t * t * (3.0 - 2.0 * t); // smoothstep

            let cur_x = wx_prev + (wx_curr - wx_prev) * t;
            let cur_y = wy_prev + (wy_curr - wy_prev) * t;

            // Parabolic height based on overall flight progress
            let progress = if proj.path.len() > 1 {
                cursor as f32 / (proj.path.len() - 1) as f32
            } else {
                1.0
            };
            let height = 4.0 * progress * (1.0 - progress);

            let screen_x = (input.camera_x + cur_x * input.camera_zoom) / sf;
            let screen_y = (input.camera_y + (cur_y - height * 20.0) * input.camera_zoom) / sf;

            // Frustum cull
            if screen_x < -50.0 || screen_x > input.screen_w / sf + 50.0
                || screen_y < -50.0 || screen_y > input.screen_h / sf + 50.0 {
                continue;
            }

            let is_nuke = matches!(
                proj.kind,
                sow_core::game::ProjectileKind::Nuke(_)
                    | sow_core::game::ProjectileKind::MIRVWarhead
            );

            let center = egui::pos2(screen_x, screen_y);

            // 1. Draw glowing flight trajectory trail curve for Flying Nukes & MIRV Warheads!
            if is_nuke {
                let trail_end = cursor.min(proj.path.len() - 1);
                let trail_step = (trail_end / 15).max(1);
                let mut curve_points = Vec::with_capacity(16);
                for i in (0..=trail_end).step_by(trail_step) {
                    let p = i as f32 / (proj.path.len() - 1).max(1) as f32;
                    let (t_wx, t_wy) = tile_to_world(proj.path[i], map_w);
                    let t_h = 4.0 * p * (1.0 - p);

                    let sc_x = (input.camera_x + t_wx * input.camera_zoom) / sf;
                    let sc_y = (input.camera_y + (t_wy - t_h * 20.0) * input.camera_zoom) / sf;
                    curve_points.push(egui::pos2(sc_x, sc_y));
                }
                // Always include the current interpolated position as the last point
                curve_points.push(center);

                let trail_color = match proj.kind {
                    sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::HydrogenBomb) => {
                        egui::Color32::from_rgba_unmultiplied(255, 50, 0, 150)
                    }
                    sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::MIRV) => {
                        egui::Color32::from_rgba_unmultiplied(255, 170, 0, 150)
                    }
                    sow_core::game::ProjectileKind::MIRVWarhead => {
                        egui::Color32::from_rgba_unmultiplied(255, 140, 0, 110)
                    }
                    _ => {
                        egui::Color32::from_rgba_unmultiplied(255, 90, 0, 140)
                    }
                };

                for win in curve_points.windows(2) {
                    painter.line_segment(
                        [win[0], win[1]],
                        egui::Stroke::new(1.8f32, trail_color),
                    );
                }

                // 2. Draw glowing rocket exhaust engine flame tail at the back of the missile!
                if curve_points.len() >= 2 {
                    let tip = curve_points[curve_points.len() - 1];
                    let prev = curve_points[curve_points.len() - 2];
                    let dir = tip - prev;
                    let dir_len = (dir.x * dir.x + dir.y * dir.y).sqrt().max(0.1);
                    let dir_norm = dir / dir_len;

                    let flame_len = match proj.kind {
                        sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::HydrogenBomb) => 14.0,
                        sow_core::game::ProjectileKind::MIRVWarhead => 6.0,
                        _ => 10.0,
                    };
                    let flame_back = tip - dir_norm * flame_len;
                    let perp = egui::vec2(-dir_norm.y, dir_norm.x) * (flame_len * 0.28);

                    let flame_left = flame_back - perp;
                    let flame_right = flame_back + perp;
                    painter.add(egui::Shape::convex_polygon(
                        vec![tip, flame_left, flame_right],
                        egui::Color32::from_rgb(255, 140, 0),
                        egui::Stroke::NONE,
                    ));

                    let core_back = tip - dir_norm * (flame_len * 0.45);
                    let core_perp = perp * 0.45;
                    painter.add(egui::Shape::convex_polygon(
                        vec![tip, core_back - core_perp, core_back + core_perp],
                        egui::Color32::from_rgb(255, 255, 200),
                        egui::Stroke::NONE,
                    ));
                }
            }

            let uri = match proj.kind {
                sow_core::game::ProjectileKind::Nuke(nk) => nk.asset().uri(),
                sow_core::game::ProjectileKind::MIRVWarhead => sow_core::assets::Asset::AtomBomb.uri(),
                sow_core::game::ProjectileKind::SAMMissile => sow_core::assets::Asset::SamMissile.uri(),
                sow_core::game::ProjectileKind::Shell => continue,
            };

            let base_size = match proj.kind {
                sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::HydrogenBomb) => 24.0,
                sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::MIRV) => 20.0,
                sow_core::game::ProjectileKind::Nuke(_) => 16.0,
                sow_core::game::ProjectileKind::MIRVWarhead => 8.0,
                sow_core::game::ProjectileKind::SAMMissile => 10.0,
                _ => 12.0,
            };

            let scale = (1.0_f32 + height * 0.5_f32).min(2.0_f32);
            let size = base_size * scale;
            let rect = egui::Rect::from_center_size(center, egui::vec2(size, size));

            let size_hint = egui::load::SizeHint::Size { width: 64, height: 64, maintain_aspect_ratio: true };
            if let Ok(egui::load::TexturePoll::Ready { texture }) = painter.ctx().try_load_texture(uri, egui::TextureOptions::LINEAR, size_hint) {
                painter.image(
                    texture.id,
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }

            // SAM missile trail
            if matches!(proj.kind, sow_core::game::ProjectileKind::SAMMissile) {
                let (src_wx, src_wy) = tile_to_world(proj.src_tile, map_w);
                let trail_x = (input.camera_x + src_wx * input.camera_zoom) / sf;
                let trail_y = (input.camera_y + src_wy * input.camera_zoom) / sf;
                painter.line_segment(
                    [egui::pos2(trail_x, trail_y), center],
                    egui::Stroke::new(1.0_f32, egui::Color32::from_rgba_unmultiplied(100, 200, 255, 100)),
                );
            }
        }

        for attack in &snap.attacks {
            if attack.target_owner == 0 {
                continue;
            }
            if attack.owner_id != sim.my_player_id.unwrap_or(0) {
                continue;
            }

            let mut rx = 0.5;
            let mut ry = 0.5;
            let mut tx = 0.5;
            let mut ty = 0.5;
            let mut r = 0.5;
            let mut g = 0.5;
            let mut b = 0.5;

            if let Some(attacker) = snap.players.iter().find(|p| p.id == attack.owner_id) {
                rx = attacker.centroid_x + 0.5 + (attacker.centroid_y as i32 % 2) as f32 * 0.5;
                ry = (attacker.centroid_y + 0.5) * 0.8660254_f32;
                let rgb = if attacker.player_type == sow_core::player::PlayerType::Human {
                    sow_core::player::human_shader_territory_rgb(attacker.id)
                } else {
                    attacker.color
                };
                r = rgb[0];
                g = rgb[1];
                b = rgb[2];
            }
            if let Some(target) = snap.players.iter().find(|p| p.id == attack.target_owner) {
                tx = target.centroid_x + 0.5 + (target.centroid_y as i32 % 2) as f32 * 0.5;
                ty = (target.centroid_y + 0.5) * 0.8660254_f32;
            }

            let start_x = (input.camera_x + rx * input.camera_zoom) / sf;
            let start_y = (input.camera_y + ry * input.camera_zoom) / sf;
            let end_x = (input.camera_x + tx * input.camera_zoom) / sf;
            let end_y = (input.camera_y + ty * input.camera_zoom) / sf;

            let color = egui::Color32::from_rgb(
                (r * 255.0) as u8,
                (g * 255.0) as u8,
                (b * 255.0) as u8,
            );
            let start_pos = egui::pos2(start_x, start_y);
            let end_pos = egui::pos2(end_x, end_y);

            // Simple thick line to represent attack
            painter.line_segment(
                [start_pos, end_pos],
                egui::Stroke::new(3.0_f32, egui::Color32::from_black_alpha(150)),
            );
            painter.line_segment([start_pos, end_pos], egui::Stroke::new(1.5_f32, color));

            if attack.retreating
                && (time.start_time.elapsed().as_millis() / 500).is_multiple_of(2)
            {
                let center = start_pos.lerp(end_pos, 0.5);
                painter.text(
                    center,
                    egui::Align2::CENTER_CENTER,
                    "[X]",
                    egui::FontId::proportional(20.0),
                    egui::Color32::RED,
                );
            }
        }


    }
}
