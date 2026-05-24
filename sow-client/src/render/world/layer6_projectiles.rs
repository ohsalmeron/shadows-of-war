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
            let sim_dt = web_time::Instant::now().duration_since(time.last_tick).as_secs_f32();
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
                sow_core::game::ProjectileKind::Nuke { .. }
            );

            let center = egui::pos2(screen_x, screen_y);

            // 1. Draw glowing flight trajectory trail curve for Flying Nukes!
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

                let level = match proj.kind {
                    sow_core::game::ProjectileKind::Nuke { level } => level,
                    _ => 1,
                };

                let trail_color = if level >= 3 {
                    egui::Color32::from_rgba_unmultiplied(255, 170, 0, 160)
                } else if level == 2 {
                    egui::Color32::from_rgba_unmultiplied(255, 50, 0, 150)
                } else {
                    egui::Color32::from_rgba_unmultiplied(255, 90, 0, 140)
                };

                for win in curve_points.windows(2) {
                    painter.line_segment(
                        [win[0], win[1]],
                        egui::Stroke::new(1.8f32 + (level as f32 * 0.4), trail_color),
                    );
                }

                // 2. Draw glowing rocket exhaust engine flame tail at the back of the missile!
                if curve_points.len() >= 2 {
                    let tip = curve_points[curve_points.len() - 1];
                    let prev = curve_points[curve_points.len() - 2];
                    let dir = tip - prev;
                    let dir_len = (dir.x * dir.x + dir.y * dir.y).sqrt().max(0.1);
                    let dir_norm = dir / dir_len;

                    let flame_len = 6.0 + (level as f32) * 4.0;
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
                sow_core::game::ProjectileKind::Nuke { .. } => sow_core::assets::Asset::AtomBomb.uri(),
                sow_core::game::ProjectileKind::SAMMissile => sow_core::assets::Asset::SamMissile.uri(),
                sow_core::game::ProjectileKind::Shell => continue,
            };

            let base_size = match proj.kind {
                sow_core::game::ProjectileKind::Nuke { level } => 10.0 + (level as f32) * 6.0,
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
            let my_id = sim.my_player_id.unwrap_or(0);
            let is_outgoing = attack.owner_id == my_id && my_id != 0;
            let is_incoming = attack.target_owner == my_id && my_id != 0;

            if !is_outgoing && !is_incoming {
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
                let rgb = attacker.color;
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

            let mut color = egui::Color32::from_rgb(
                (r * 255.0) as u8,
                (g * 255.0) as u8,
                (b * 255.0) as u8,
            );

            if is_incoming {
                color = egui::Color32::from_rgb(255, 60, 60); // Bright warning red for incoming
            }

            let start_pos = egui::pos2(start_x, start_y);
            let end_pos = egui::pos2(end_x, end_y);

            // 1. Draw a soft, elegant background guide rail/shadow line
            painter.line_segment(
                [start_pos, end_pos],
                egui::Stroke::new(3.0_f32, egui::Color32::from_black_alpha(30)),
            );
            painter.line_segment(
                [start_pos, end_pos],
                egui::Stroke::new(1.0_f32, color.linear_multiply(0.35)),
            );

            // 2. Render beautiful flowing dots sliding towards the target
            let elapsed = time.start_time.elapsed().as_secs_f32();
            let num_dots = 4;
            for i in 0..num_dots {
                let t_offset = (i as f32) / (num_dots as f32);
                let t = (t_offset + elapsed * 0.45) % 1.0;

                // Interpolated position along the ray
                let dot_pos = egui::pos2(
                    start_pos.x + (end_pos.x - start_pos.x) * t,
                    start_pos.y + (end_pos.y - start_pos.y) * t,
                );

                // Fade out as it reaches the target destination
                let alpha = ((1.0 - t) * 255.0) as u8;
                let outer_glow = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha / 3);
                let inner_core = egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);

                // Draw soft outer halo and solid inner core
                painter.circle_filled(dot_pos, 4.5_f32, outer_glow);
                painter.circle_filled(dot_pos, 1.8_f32, inner_core);
            }

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

        // --- Render Attack Troop Count Badges at the frontier centroids ---
        let middle_painter = painter.ctx().layer_painter(egui::LayerId::new(
            egui::Order::Middle,
            egui::Id::new("attack_badges"),
        ));

        for attack in &snap.attacks {
            if attack.troops <= 0.0 {
                continue;
            }

            let my_id = sim.my_player_id.unwrap_or(0);
            let is_outgoing = attack.owner_id == my_id && my_id != 0;
            let is_incoming = attack.target_owner == my_id && my_id != 0;

            // Only show labels for outgoing or incoming attacks involving the player
            if !is_outgoing && !is_incoming {
                continue;
            }

            let cx = attack.front_cx;
            let cy = attack.front_cy;
            if cx == 0.0 && cy == 0.0 {
                continue;
            }

            // Convert centroid column/row to world coordinates
            let wx = cx + 0.5 + ((cy as i32 % 2) as f32 * 0.5);
            let wy = (cy + 0.5) * 0.8660254_f32;

            // Convert to screen coordinates
            let screen_x = (input.camera_x + wx * input.camera_zoom) / sf;
            let screen_y = (input.camera_y + wy * input.camera_zoom) / sf;

            // Frustum cull
            if screen_x < -80.0 || screen_x > input.screen_w / sf + 80.0
                || screen_y < -40.0 || screen_y > input.screen_h / sf + 40.0 {
                continue;
            }

            let label = format!("⚔ {}", sow_ui::utils::format_number(attack.troops));
            let color = if is_incoming {
                egui::Color32::from_rgb(255, 90, 90) // Red for incoming
            } else {
                sow_ui::ui::theme::accent_solo_cyan_hover() // Cyan for outgoing
            };

            let pos = egui::pos2(screen_x, screen_y);

            // Supercell style text: solid black outline + heavy bottom shadow
            sow_ui::ui::theme::outlined_text(
                &middle_painter,
                pos,
                egui::Align2::CENTER_CENTER,
                &label,
                egui::FontId::proportional(13.0),
                color,
                egui::Color32::BLACK,
            );
        }
    }
}
