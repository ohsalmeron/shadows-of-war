use super::*;
#[allow(unused_variables)]
pub(crate) fn render(ui: &mut crate::app::UiState, sim: &crate::app::SimState, input: &crate::app::InputState, time: &crate::app::TimeState, gfx: &crate::app::GraphicsState, ctx: &RenderContext) {
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
            // --- Layer 6: Projectiles (Nukes, SAM Missiles) ---
            for proj in &snap.projectiles {
                let cur_x = proj.src_x + (proj.dst_x - proj.src_x) * proj.progress;
                let cur_y = proj.src_y + (proj.dst_y - proj.src_y) * proj.progress;

                // Parabolic height for nukes (peak at progress=0.5)
                let height = 4.0 * proj.progress * (1.0 - proj.progress);

                let screen_x = (input.camera_x + (cur_x + 0.5) * input.camera_zoom) / sf;
                let screen_y = (input.camera_y + (cur_y + 0.5 - height * 20.0) * input.camera_zoom) / sf;

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
                    let steps = 15;
                    let mut curve_points = Vec::with_capacity(steps + 1);
                    for i in 0..=steps {
                        let p = (i as f32 / steps as f32) * proj.progress;
                        let t_x = proj.src_x + (proj.dst_x - proj.src_x) * p;
                        let t_y = proj.src_y + (proj.dst_y - proj.src_y) * p;
                        let t_h = 4.0 * p * (1.0 - p);

                        let sc_x = (input.camera_x + (t_x + 0.5) * input.camera_zoom) / sf;
                        let sc_y = (input.camera_y + (t_y + 0.5 - t_h * 20.0) * input.camera_zoom) / sf;
                        curve_points.push(egui::pos2(sc_x, sc_y));
                    }

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
                    let trail_x = (input.camera_x + (proj.src_x + 0.5) * input.camera_zoom) / sf;
                    let trail_y = (input.camera_y + (proj.src_y + 0.5) * input.camera_zoom) / sf;
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
                    rx = attacker.centroid_x + 0.5;
                    ry = attacker.centroid_y + 0.5;
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
                    tx = target.centroid_x + 0.5;
                    ty = target.centroid_y + 0.5;
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
