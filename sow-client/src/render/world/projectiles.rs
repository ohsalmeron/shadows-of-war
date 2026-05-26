use super::*;

pub(crate) fn render(
    ui: &mut crate::app::UiState,
    sim: &crate::app::SimState,
    input: &crate::app::InputState,
    time: &crate::app::TimeState,
    _gfx: &crate::app::GraphicsState,
    ctx: &RenderContext,
) {
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
        let map_w = snap.players.first().map(|_| sim.map_w).unwrap_or(1);

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
            let sim_dt = web_time::Instant::now()
                .duration_since(time.last_tick)
                .as_secs_f32();
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
            if screen_x < -50.0
                || screen_x > input.screen_w / sf + 50.0
                || screen_y < -50.0
                || screen_y > input.screen_h / sf + 50.0
            {
                continue;
            }

            let is_nuke = matches!(proj.kind, sow_core::game::ProjectileKind::Nuke { .. });

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
                sow_core::game::ProjectileKind::Nuke { .. } => {
                    sow_core::assets::Asset::AtomBomb.uri()
                }
                sow_core::game::ProjectileKind::SAMMissile => {
                    sow_core::assets::Asset::SamMissile.uri()
                }
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

            let size_hint = egui::load::SizeHint::Size {
                width: 64,
                height: 64,
                maintain_aspect_ratio: true,
            };
            if let Ok(egui::load::TexturePoll::Ready { texture }) =
                painter
                    .ctx()
                    .try_load_texture(uri, egui::TextureOptions::LINEAR, size_hint)
            {
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
                    egui::Stroke::new(
                        1.0_f32,
                        egui::Color32::from_rgba_unmultiplied(100, 200, 255, 100),
                    ),
                );
            }
        }

        for attack in &snap.attacks {
            if attack.target_owner == 0 {
                continue;
            }
            let my_id = sim.my_player_id.unwrap_or(0);
            let is_incoming = attack.target_owner == my_id && my_id != 0;

            // Only draw threat lines and pointing fingers for incoming attacks on us
            if !is_incoming || attack.troops <= 0.0 {
                continue;
            }

            let mut rx = 0.5;
            let mut ry = 0.5;
            let mut tx = 0.5;
            let mut ty = 0.5;

            if let Some(attacker) = snap.players.iter().find(|p| p.id == attack.owner_id) {
                rx = attacker.centroid_x + 0.5 + (attacker.centroid_y as i32 % 2) as f32 * 0.5;
                ry = (attacker.centroid_y + 0.5) * 0.8660254_f32;
            }
            if let Some(target) = snap.players.iter().find(|p| p.id == attack.target_owner) {
                tx = target.centroid_x + 0.5 + (target.centroid_y as i32 % 2) as f32 * 0.5;
                ty = (target.centroid_y + 0.5) * 0.8660254_f32;
            }

            // Start at the battlefront (or fall back to target centroid if front_cx/front_cy is 0)
            let mut fx = tx;
            let mut fy = ty;
            if attack.front_cx != 0.0 || attack.front_cy != 0.0 {
                fx = attack.front_cx + 0.5 + (attack.front_cy as i32 % 2) as f32 * 0.5;
                fy = (attack.front_cy + 0.5) * 0.8660254_f32;
            }

            let start_x = (input.camera_x + fx * input.camera_zoom) / sf;
            let start_y = (input.camera_y + fy * input.camera_zoom) / sf;
            let end_x = (input.camera_x + rx * input.camera_zoom) / sf;
            let end_y = (input.camera_y + ry * input.camera_zoom) / sf;

            let color = egui::Color32::from_rgb(255, 60, 60); // Bright warning red for incoming threat source

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

            // 2. Render beautiful flowing dots sliding from the battlefront to the aggressor
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

                // Fade out as it reaches the aggressor destination
                let alpha = ((1.0 - t) * 255.0) as u8;
                let outer_glow = egui::Color32::from_rgba_unmultiplied(
                    color.r(),
                    color.g(),
                    color.b(),
                    alpha / 3,
                );
                let inner_core =
                    egui::Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha);

                // Draw soft outer halo and solid inner core
                painter.circle_filled(dot_pos, 4.5_f32, outer_glow);
                painter.circle_filled(dot_pos, 1.8_f32, inner_core);
            }

            if attack.retreating && (time.start_time.elapsed().as_millis() / 500).is_multiple_of(2)
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
            if screen_x < -80.0
                || screen_x > input.screen_w / sf + 80.0
                || screen_y < -40.0
                || screen_y > input.screen_h / sf + 40.0
            {
                continue;
            }

            let troops_val = attack.troops;
            let entry = ui.attack_troop_labels.entry(attack.id).or_insert_with(|| {
                (troops_val, format!("⚔ {}", sow_ui::utils::format_number(troops_val)))
            });
            if (entry.0 - troops_val).abs() > 0.0001 {
                *entry = (troops_val, format!("⚔ {}", sow_ui::utils::format_number(troops_val)));
            }
            let label = &entry.1;
            let color = if is_incoming {
                egui::Color32::from_rgb(255, 90, 90) // Red for incoming
            } else {
                sow_ui::ui::theme::accent_solo_cyan_hover() // Cyan for outgoing
            };

            // Layout once, paint 7 passes with zero additional layout cost
            let font_id = egui::FontId::proportional(13.0);
            let galley = middle_painter.layout_no_wrap(label.to_owned(), font_id, color);
            let half = galley.size() / 2.0;
            let anchor = egui::pos2(screen_x, screen_y) - half;
            crate::hud::nameplate::paint_glow_nameplate_galley(
                &middle_painter, anchor, galley, color, false,
            );
        }

        // ── Nuke Placement Preview ──────────────────────────────────────
        if ui.app.hud_state.selected_nuke_kind.is_some() {
            // Resolve hovered tile from mouse (same hex math as buildings.rs)
            let mx = input.last_mouse_x as f32;
            let my = input.last_mouse_y as f32;
            let world_x = (mx - input.camera_x) / input.camera_zoom;
            let world_y = (my - input.camera_y) / input.camera_zoom;
            let q_f = world_x - world_y * 0.577_350_26_f32;
            let r_f = world_y * 1.154_700_5_f32;
            let s_f = -q_f - r_f;
            let mut rq = q_f.round();
            let mut rr = r_f.round();
            let rs = s_f.round();
            let q_diff = (rq - q_f).abs();
            let r_diff = (rr - r_f).abs();
            let s_diff = (rs - s_f).abs();
            if q_diff > r_diff && q_diff > s_diff {
                rq = -rr - rs;
            } else if r_diff > s_diff {
                rr = -rq - rs;
            }
            let h_col = rq as i32 + (rr as i32 - (rr as i32 & 1)) / 2;
            let h_row = rr as i32;

            if h_col >= 0
                && h_row >= 0
                && h_col < sim.map_w as i32
                && h_row < sim.map_h as i32
            {
                let target_tile = (h_row * sim.map_w as i32 + h_col) as u32;
                let my_id = sim.my_player_id.unwrap_or(0);
                let current_tick = snap.tick;

                // Find nearest owned completed City not on cooldown
                let best_silo = snap
                    .buildings
                    .iter()
                    .filter(|b| {
                        b.kind == sow_core::game::BuildingKind::City
                            && b.owner_id == my_id
                            && !b.under_construction
                            && ui.silo_cooldowns.get(&b.id).map_or(true, |&exp| current_tick >= exp)
                    })
                    .min_by_key(|b| {
                        let bx = (b.tile_idx % sim.map_w) as i32;
                        let by = (b.tile_idx / sim.map_w) as i32;
                        let tx = target_tile as i32 % sim.map_w as i32;
                        let ty = target_tile as i32 / sim.map_w as i32;
                        (bx - tx).abs() + (by - ty).abs()
                    });

                // Show cooldown rings on cities that recently fired
                for b in snap.buildings.iter().filter(|b| {
                    b.kind == sow_core::game::BuildingKind::City
                        && b.owner_id == my_id
                        && ui.silo_cooldowns.get(&b.id).map_or(false, |&exp| current_tick < exp)
                }) {
                    let (bwx, bwy) = tile_to_world(b.tile_idx, map_w);
                    let bsx = (input.camera_x + bwx * input.camera_zoom) / sf;
                    let bsy = (input.camera_y + bwy * input.camera_zoom) / sf;
                    let ring_r = (0.7 * input.camera_zoom) / sf;
                    // Cooldown progress: 1.0 = just fired, 0.0 = ready
                    let expires = ui.silo_cooldowns[&b.id];
                    let remaining = expires.saturating_sub(current_tick) as f32;
                    let progress = (remaining / 90.0).clamp(0.0, 1.0);
                    let alpha = (progress * 160.0) as u8;
                    painter.circle_stroke(
                        egui::pos2(bsx, bsy),
                        ring_r,
                        egui::Stroke::new(
                            2.0_f32,
                            egui::Color32::from_rgba_unmultiplied(239, 68, 68, alpha),
                        ),
                    );
                }

                if let Some(silo) = best_silo {
                    let level = silo.modules.arsenal.max(1);
                    let silo_tile = silo.tile_idx;
                    let path =
                        sow_core::pathfinding::bresenham_line(silo_tile, target_tile, sim.map_w);
                    let path_len = path.len().max(1);

                    // ── 1. Source city pulsing highlight ─────────────────
                    let (silo_wx, silo_wy) = tile_to_world(silo_tile, map_w);
                    let silo_sx = (input.camera_x + silo_wx * input.camera_zoom) / sf;
                    let silo_sy = (input.camera_y + silo_wy * input.camera_zoom) / sf;
                    let silo_center = egui::pos2(silo_sx, silo_sy);

                    let pulse = ((time.start_time.elapsed().as_secs_f32() * 3.0).sin() * 0.3 + 0.7)
                        .clamp(0.4, 1.0);
                    let pulse_alpha = (pulse * 120.0) as u8;
                    let silo_ring_r = (0.8 * input.camera_zoom) / sf;
                    painter.circle_stroke(
                        silo_center,
                        silo_ring_r,
                        egui::Stroke::new(
                            2.0_f32,
                            egui::Color32::from_rgba_unmultiplied(34, 211, 238, pulse_alpha),
                        ),
                    );

                    // ── 2. Trajectory arc ────────────────────────────────
                    let num_samples = 24.min(path_len);
                    let step = if num_samples > 1 {
                        (path_len - 1) as f32 / (num_samples - 1) as f32
                    } else {
                        1.0
                    };

                    let mut arc_points = Vec::with_capacity(num_samples + 1);
                    for i in 0..num_samples {
                        let idx = (i as f32 * step) as usize;
                        let idx = idx.min(path_len - 1);
                        let p = idx as f32 / (path_len - 1).max(1) as f32;
                        let (twx, twy) = tile_to_world(path[idx], map_w);
                        let height = 4.0 * p * (1.0 - p) * 20.0;
                        let sx = (input.camera_x + twx * input.camera_zoom) / sf;
                        let sy = (input.camera_y + (twy - height) * input.camera_zoom) / sf;
                        arc_points.push(egui::pos2(sx, sy));
                    }

                    // Draw dark outline behind for contrast
                    for win in arc_points.windows(2) {
                        painter.line_segment(
                            [win[0], win[1]],
                            egui::Stroke::new(3.0_f32, egui::Color32::from_black_alpha(80)),
                        );
                    }

                    // Draw dashed arc segments (alternating visible/gap)
                    let arc_color = if level >= 2 {
                        egui::Color32::from_rgba_unmultiplied(255, 170, 50, 200)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(255, 220, 130, 200)
                    };
                    for (seg_i, win) in arc_points.windows(2).enumerate() {
                        if seg_i % 3 != 2 {
                            painter.line_segment(
                                [win[0], win[1]],
                                egui::Stroke::new(1.8_f32, arc_color),
                            );
                        }
                    }

                    // ── 3. Blast radius circles at target ────────────────
                    let (tgt_wx, tgt_wy) = tile_to_world(target_tile, map_w);
                    let tgt_sx = (input.camera_x + tgt_wx * input.camera_zoom) / sf;
                    let tgt_sy = (input.camera_y + tgt_wy * input.camera_zoom) / sf;
                    let tgt_center = egui::pos2(tgt_sx, tgt_sy);

                    let max_radius = 45.0 + (level.saturating_sub(1) as f32) * 33.0;
                    let fallout_radius = 30.0 + (level.saturating_sub(1) as f32) * 22.5;

                    // Inner destruction zone
                    let inner_r = (max_radius * input.camera_zoom) / sf;
                    painter.circle_filled(
                        tgt_center,
                        inner_r,
                        egui::Color32::from_rgba_unmultiplied(255, 80, 30, 15),
                    );
                    painter.circle_stroke(
                        tgt_center,
                        inner_r,
                        egui::Stroke::new(
                            1.5_f32,
                            egui::Color32::from_rgba_unmultiplied(255, 90, 40, 140),
                        ),
                    );

                    // Outer fallout zone
                    let outer_r = (fallout_radius * input.camera_zoom) / sf;
                    painter.circle_filled(
                        tgt_center,
                        outer_r,
                        egui::Color32::from_rgba_unmultiplied(120, 200, 60, 10),
                    );
                    painter.circle_stroke(
                        tgt_center,
                        outer_r,
                        egui::Stroke::new(
                            1.0_f32,
                            egui::Color32::from_rgba_unmultiplied(120, 200, 60, 80),
                        ),
                    );

                    // ── 4. Ghost nuke icon at target ─────────────────────
                    let size_hint = egui::load::SizeHint::Size {
                        width: 64,
                        height: 64,
                        maintain_aspect_ratio: true,
                    };
                    let uri = sow_core::assets::Asset::AtomBomb.uri();
                    if let Ok(egui::load::TexturePoll::Ready { texture }) = painter
                        .ctx()
                        .try_load_texture(uri, egui::TextureOptions::LINEAR, size_hint)
                    {
                        let ghost_size = 18.0 + level as f32 * 4.0;
                        let rect = egui::Rect::from_center_size(
                            tgt_center,
                            egui::vec2(ghost_size, ghost_size),
                        );
                        painter.image(
                            texture.id,
                            rect,
                            egui::Rect::from_min_max(
                                egui::pos2(0.0, 0.0),
                                egui::pos2(1.0, 1.0),
                            ),
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 140),
                        );
                    }
                } else {
                    // No city available (all on cooldown or none owned)
                    let (tgt_wx, tgt_wy) = tile_to_world(target_tile, map_w);
                    let tgt_sx = (input.camera_x + tgt_wx * input.camera_zoom) / sf;
                    let tgt_sy = (input.camera_y + tgt_wy * input.camera_zoom) / sf;
                    painter.text(
                        egui::pos2(tgt_sx, tgt_sy),
                        egui::Align2::CENTER_CENTER,
                        "⏳ Cooldown",
                        egui::FontId::proportional(14.0),
                        egui::Color32::from_rgb(255, 160, 60),
                    );
                }
            }
        }
    }
}

