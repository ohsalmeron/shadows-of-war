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
            // --- Layer 4: Track and Spawn Detonations ---
            let mut new_detonations = Vec::new();
            for (id, prev_proj) in &ui.last_projectiles {
                if !snap.projectiles.iter().any(|p| p.id == *id) {
                    if prev_proj.progress >= 0.9 {
                        new_detonations.push((prev_proj.dst_x, prev_proj.dst_y, prev_proj.kind));
                    }
                }
            }

            // Sync last_projectiles
            ui.last_projectiles.clear();
            for proj in &snap.projectiles {
                ui.last_projectiles.insert(proj.id, proj.clone());
            }

            // Spawn active explosions and fallout zones for new detonations
            let current_time = web_time::Instant::now();
            for (dx, dy, kind) in new_detonations {
                match kind {
                    sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::AtomBomb) => {
                        ui.active_explosions.push(crate::app::ActiveExplosion {
                            x: dx,
                            y: dy,
                            start_time: current_time,
                            max_radius: 45.0,
                            kind: crate::app::ExplosionKind::Atom,
                        });
                        ui.fallout_zones.push(crate::app::FalloutZone {
                            x: dx,
                            y: dy,
                            radius: 30.0,
                            start_time: current_time,
                        });
                    }
                    sow_core::game::ProjectileKind::Nuke(sow_core::game::NukeKind::HydrogenBomb) => {
                        ui.active_explosions.push(crate::app::ActiveExplosion {
                            x: dx,
                            y: dy,
                            start_time: current_time,
                            max_radius: 120.0,
                            kind: crate::app::ExplosionKind::Hydrogen,
                        });
                        ui.fallout_zones.push(crate::app::FalloutZone {
                            x: dx,
                            y: dy,
                            radius: 100.0,
                            start_time: current_time,
                        });
                    }
                    sow_core::game::ProjectileKind::MIRVWarhead => {
                        ui.active_explosions.push(crate::app::ActiveExplosion {
                            x: dx,
                            y: dy,
                            start_time: current_time,
                            max_radius: 20.0,
                            kind: crate::app::ExplosionKind::MIRVWarhead,
                        });
                        ui.fallout_zones.push(crate::app::FalloutZone {
                            x: dx,
                            y: dy,
                            radius: 18.0,
                            start_time: current_time,
                        });
                    }
                    _ => {}
                }
            }


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

                let screen_x = (input.camera_x + (fz.x + 0.5) * input.camera_zoom) / sf;
                let screen_y = (input.camera_y + (fz.y + 0.5) * input.camera_zoom) / sf;
                let center = egui::pos2(screen_x, screen_y);
                let radius = fz.radius * zoom_scaled;

                // Glowing green contaminated aura
                painter.circle_filled(
                    center,
                    radius,
                    egui::Color32::from_rgba_unmultiplied(60, 220, 90, base_alpha as u8),
                );

                // CRISP high-contrast radioactive outer border
                let border_color = egui::Color32::from_rgba_unmultiplied(100, 255, 140, (base_alpha * 2.0) as u8);
                painter.circle_stroke(
                    center,
                    radius,
                    egui::Stroke::new(1.0f32, border_color),
                );

                // Deterministic floating glowing radioactive green dust particles!
                let seed = (fz.x * 123.45 + fz.y * 678.9) as i32;
                let particle_count = (fz.radius * 0.5) as i32;
                for i in 0..particle_count {
                    let angle = ((seed + i * 37) as f32).sin() * std::f32::consts::TAU;
                    let dist_ratio = (((seed + i * 19) as f32).cos() * 0.5 + 0.5).sqrt();
                    let dist = dist_ratio * fz.radius;

                    let px = fz.x + angle.cos() * dist;
                    let py = fz.y + angle.sin() * dist;

                    let speed = 0.4 + ((seed + i * 13) as f32).sin().abs() * 0.8;
                    let drift_y = (wall_secs as f32 * speed) % 6.0;
                    let py_drifted = py - drift_y;

                    let p_screen_x = (input.camera_x + (px + 0.5) * input.camera_zoom) / sf;
                    let p_screen_y = (input.camera_y + (py_drifted + 0.5) * input.camera_zoom) / sf;

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
                    crate::app::ExplosionKind::Hydrogen => 3.5,
                    crate::app::ExplosionKind::Atom => 2.2,
                    crate::app::ExplosionKind::MIRVWarhead => 1.2,
                };
                if elapsed >= duration {
                    return false;
                }

                let p = elapsed / duration;

                let screen_x = (input.camera_x + (exp.x + 0.5) * input.camera_zoom) / sf;
                let screen_y = (input.camera_y + (exp.y + 0.5) * input.camera_zoom) / sf;
                let center = egui::pos2(screen_x, screen_y);

                // 1. Expanding Shockwave Circle
                let shockwave_max = exp.max_radius * 1.6;
                let shockwave_radius = p * shockwave_max * zoom_scaled;
                let shockwave_alpha = (1.0 - p).max(0.0);
                let shockwave_color = egui::Color32::from_rgba_unmultiplied(
                    255, 255, 255,
                    (shockwave_alpha * 190.0) as u8,
                );
                painter.circle_stroke(
                    center,
                    shockwave_radius,
                    egui::Stroke::new(1.5f32, shockwave_color),
                );

                // 2. Rising Mushroom Cloud / Fireball caps
                let cloud_scale = match exp.kind {
                    crate::app::ExplosionKind::Hydrogen => 1.0,
                    crate::app::ExplosionKind::Atom => 0.45,
                    crate::app::ExplosionKind::MIRVWarhead => 0.18,
                };

                let cap_rise = p * 45.0 * cloud_scale * zoom_scaled;
                let cap_center = egui::pos2(center.x, center.y - cap_rise);
                let cap_radius = (p * 2.0).min(1.0) * exp.max_radius * zoom_scaled;

                let smoke_alpha = ((1.0 - p) * 195.0) as u8;
                let fire_alpha = ((1.0 - p) * 240.0) as u8;
                let core_alpha = (((1.0 - p).powi(2)) * 255.0) as u8;

                // Cap layers:
                // Outer dark fire-smoke
                painter.circle_filled(
                    cap_center,
                    cap_radius,
                    egui::Color32::from_rgba_unmultiplied(225, 50, 0, smoke_alpha),
                );
                // Middle glowing orange
                painter.circle_filled(
                    cap_center,
                    cap_radius * 0.75,
                    egui::Color32::from_rgba_unmultiplied(255, 130, 0, fire_alpha),
                );
                // Inner white-hot blast core
                painter.circle_filled(
                    cap_center,
                    cap_radius * 0.45,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 190, core_alpha),
                );

                // Mushroom Stem
                let stem_w = cap_radius * 0.22;
                let stem_rect = egui::Rect::from_min_max(
                    egui::pos2(center.x - stem_w, cap_center.y),
                    egui::pos2(center.x + stem_w, center.y),
                );
                painter.rect_filled(
                    stem_rect,
                    2.0,
                    egui::Color32::from_rgba_unmultiplied(255, 90, 0, (smoke_alpha as f32 * 0.75) as u8),
                );

                true
            });


    }
}
