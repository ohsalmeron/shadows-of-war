use super::super::*;
use super::cluster::RenderedBuilding;
use crate::render::world::utils::*;

pub(super) struct BunkerPaintOpts<'a> {
    pub painter: &'a egui::Painter,
    pub snap: &'a sow_core::protocol::SimSnapshot,
    pub config: &'a sow_core::game_config::GameConfig,
    pub b: &'a RenderedBuilding,
    pub center: egui::Pos2,
    pub zoom_scaled: f32,
    pub sf: f32,
    pub edge_cache_stale: bool,
    pub player_colors: &'a [egui::Color32],
}

pub(super) fn paint_bunker_effects(
    ui: &mut crate::app::UiState,
    sim: &crate::app::SimState,
    input: &crate::app::InputState,
    time: &crate::app::TimeState,
    gfx: &crate::app::GraphicsState,
    ctx: &RenderContext,
    opts: &BunkerPaintOpts,
) {
    let painter = opts.painter;
    let snap = opts.snap;
    let config = opts.config;
    let b = opts.b;
    let center = opts.center;
    let zoom_scaled = opts.zoom_scaled;
    let sf = opts.sf;
    let edge_cache_stale = opts.edge_cache_stale;
    let player_colors = opts.player_colors;

    if zoom_scaled < 2.5 {
        return;
    }

    if b.kind == sow_core::game::BuildingKind::Bunker
        && b.active_level > 0
        && sow_ui_kit::theme::dev_config::DevConfig::get().vfx_tower
    {
        let radius_world = config.bunker_range as f32;
        let elapsed = time.start_time.elapsed().as_secs_f32();
        let laser_opts = bunker_laser_vfx_opts();
        let low_detail = input.screen_w < 900.0 || sf > 1.5 || zoom_scaled < 1.0;

        let player_color = if b.owner_id != 0 {
            player_colors
                .get(b.owner_id as usize)
                .copied()
                .unwrap_or(egui::Color32::from_rgb(0, 220, 255))
        } else {
            egui::Color32::from_rgb(0, 220, 255)
        };

        let b_col = (b.bx - 0.5).floor() as i32;
        let b_row = (b.by - 0.5).floor() as i32;

        for (attack_idx, attack) in snap.attacks.iter().enumerate() {
            if attack.target_owner == b.owner_id && attack.troops > 0.0 {
                if attack.front_cx == 0.0 && attack.front_cy == 0.0 {
                    continue;
                }

                let attack_col = attack.front_cx.floor() as i32;
                let attack_row = attack.front_cy.floor() as i32;
                let hex_dist =
                    sow_core::building::hex_distance(b_col, b_row, attack_col, attack_row);
                if hex_dist as f32 > radius_world {
                    continue;
                }

                let attack_wx = attack.front_cx + 0.5;
                let attack_wy = attack.front_cy + 0.5;

                let (target_wx, target_wy) = if laser_opts.target_seeking {
                    (attack_wx, attack_wy)
                } else {
                    let dx = attack_wx - b.bx;
                    let dy = attack_wy - b.by;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist > 0.0 {
                        (
                            b.bx + (dx / dist) * radius_world,
                            b.by + (dy / dist) * radius_world,
                        )
                    } else {
                        (b.bx, b.by)
                    }
                };

                let atk_screen_x = (input.camera_x + target_wx * input.camera_zoom) / sf;
                let atk_screen_y = (input.camera_y + target_wy * input.camera_zoom) / sf;
                let atk_center = egui::pos2(atk_screen_x, atk_screen_y);

                let glow_color = egui::Color32::from_rgba_unmultiplied(
                    player_color.r(),
                    player_color.g(),
                    player_color.b(),
                    180,
                );
                let core_color = egui::Color32::from_rgba_unmultiplied(
                    player_color.r().saturating_add(120),
                    player_color.g().saturating_add(120),
                    player_color.b().saturating_add(120),
                    255,
                );

                let scatter_seed = b.id.unwrap_or(0);
                paint_bunker_laser(
                    painter,
                    BunkerLaserPaint {
                        center,
                        atk_center,
                        elapsed,
                        glow_color,
                        core_color,
                        low_detail,
                        opts: laser_opts,
                        scatter_seed,
                        scatter_slot: attack_idx as u32,
                    },
                );

                if let Some(b_id) = b.id {
                    let mut play = false;
                    let now = web_time::Instant::now();
                    if let Some(&last_time) = ui.bunker_last_sound_time.get(&b_id) {
                        if now.duration_since(last_time).as_millis() >= 300 {
                            play = true;
                        }
                    } else {
                        play = true;
                    }

                    if play {
                        ui.bunker_last_sound_time.insert(b_id, now);
                        let seed = (b_id as u32)
                            .wrapping_mul(31)
                            .wrapping_add(attack_idx as u32);
                        sow_audio::play_bunker_defense_sound(
                            seed,
                            crate::app::audio::SpatialAudioCtx::from_input(input)
                                .params(b.bx, b.by),
                        );
                    }
                }

                let muzzle_pulse = (elapsed * 30.0).sin().abs() * 3.5 + 6.0;
                painter.circle_filled(center, muzzle_pulse, egui::Color32::WHITE);
                painter.circle_filled(center, muzzle_pulse + 5.0, glow_color);
            }
        }
    }

    // Render active Bunker range circle & glowing anchor border
    if b.kind == sow_core::game::BuildingKind::Bunker
        && b.active_level > 0
        && painter.ctx().input(|i| i.modifiers.alt)
        && sow_ui_kit::theme::dev_config::DevConfig::get().vfx_tower_range
    {
        let radius_world = config.bunker_range as f32;
        let elapsed = time.start_time.elapsed().as_secs_f32();
        let pulse = (elapsed * 2.0).sin() * 0.04 + 0.96; // soft continuous pulse

        let bunker_start = painter.ctx().data_mut(|d| {
            let map = d.get_temp_mut_or_insert_with::<std::collections::HashMap<u64, f32>>(
                egui::Id::new("bunker_activation_times"),
                std::collections::HashMap::new,
            );
            if let Some(b_id) = b.id {
                *map.entry(b_id).or_insert(elapsed)
            } else {
                elapsed
            }
        });
        let age = elapsed - bunker_start;

        let current_range = if age < 1.5 {
            let t = age / 1.5;
            let ease = 1.0 - (1.0 - t).powi(3);
            ease * radius_world
        } else {
            radius_world
        } * pulse;
        let player_color = if b.owner_id != 0 {
            player_colors
                .get(b.owner_id as usize)
                .copied()
                .unwrap_or(egui::Color32::from_rgb(0, 220, 255))
        } else {
            egui::Color32::from_rgb(0, 220, 255)
        };

        // 1. Draw hex-aligned range forcefield
        let stroke_color = egui::Color32::from_rgba_unmultiplied(
            player_color.r(),
            player_color.g(),
            player_color.b(),
            180,
        );
        let fill_color = egui::Color32::from_rgba_unmultiplied(
            player_color.r(),
            player_color.g(),
            player_color.b(),
            35,
        );
        if let Some(t_idx) = b.tile_idx {
            let map_w = sim.map_w as i32;
            let b_col = (t_idx as i32) % map_w;
            let b_row = (t_idx as i32) / map_w;
            paint_bunker_hex_range(
                painter,
                b_col,
                b_row,
                current_range,
                WorldPaintCamera {
                    camera_x: input.camera_x,
                    camera_y: input.camera_y,
                    camera_zoom: input.camera_zoom,
                    sf,
                },
                fill_color,
                stroke_color,
            );
        }

        // 1b. Draw expanding scanning radar pulse wave ripple (hex ring)
        let wave_t = (elapsed * 0.8) % 1.0;
        let wave_range = current_range * wave_t;
        let wave_alpha = ((1.0 - wave_t) * 140.0) as u8;
        if wave_range > 0.5
            && let Some(t_idx) = b.tile_idx
        {
            let map_w = sim.map_w as i32;
            let b_col = (t_idx as i32) % map_w;
            let b_row = (t_idx as i32) / map_w;
            let wave_stroke = egui::Color32::from_rgba_unmultiplied(
                player_color.r(),
                player_color.g(),
                player_color.b(),
                wave_alpha,
            );
            paint_bunker_hex_range(
                painter,
                b_col,
                b_row,
                wave_range,
                WorldPaintCamera {
                    camera_x: input.camera_x,
                    camera_y: input.camera_y,
                    camera_zoom: input.camera_zoom,
                    sf,
                },
                egui::Color32::TRANSPARENT,
                wave_stroke,
            );
        }

        // 2. Draw solid glowing square outline around the Bunker itself for visual confirmation
        let square_half = 0.5 * input.camera_zoom / sf;
        const SQUARE_OFFSETS: [egui::Vec2; 4] = [
            egui::vec2(1.0, -1.0),
            egui::vec2(1.0, 1.0),
            egui::vec2(-1.0, 1.0),
            egui::vec2(-1.0, -1.0),
        ];
        let points = vec![
            center + SQUARE_OFFSETS[0] * square_half,
            center + SQUARE_OFFSETS[1] * square_half,
            center + SQUARE_OFFSETS[2] * square_half,
            center + SQUARE_OFFSETS[3] * square_half,
        ];
        painter.add(egui::Shape::convex_polygon(
            points,
            egui::Color32::from_rgba_unmultiplied(
                player_color.r(),
                player_color.g(),
                player_color.b(),
                35,
            ),
            egui::Stroke::new(2.5_f32, player_color),
        ));

        // 3. Draw glowing defended borders within range 8
        if let Some(t_idx) = b.tile_idx {
            let map_w = sim.map_w as i32;
            let map_h = sim.map_h as i32;
            let b_col = (t_idx as i32) % map_w;
            let b_row = (t_idx as i32) / map_w;
            let b_owner = b.owner_id;

            let owners = gfx
                .map_renderer
                .as_ref()
                .map(|mr| mr.owners.as_slice())
                .unwrap_or(&[]);
            let terrain = ctx.terrain;

            let hex_dist = |c1: i32, r1: i32, c2: i32, r2: i32| -> i32 {
                sow_core::building::hex_distance(c1, r1, c2, r2)
            };

            let border_pulse = (elapsed * 3.5).sin() * 0.15 + 0.85;

            if ui.edge_mask_cache.is_empty() || edge_cache_stale {
                ui.edge_mask_cache.resize((map_w * map_h) as usize, 0u8);
                ui.edge_mask_cache.fill(0);
                for row_idx in 0..map_h {
                    for col_idx in 0..map_w {
                        let tile_idx = (row_idx * map_w + col_idx) as usize;
                        let owner = owners.get(tile_idx).copied().unwrap_or(0);
                        if owner == 0 {
                            continue;
                        }
                        let mut mask = 0u8;
                        for dir in 0..4 {
                            let (nc, nr) = match dir {
                                0 => (col_idx + 1, row_idx), // East
                                1 => (col_idx - 1, row_idx), // West
                                2 => (col_idx, row_idx - 1), // North
                                3 => (col_idx, row_idx + 1), // South
                                _ => (col_idx, row_idx),
                            };
                            let is_border = if nc < 0 || nr < 0 || nc >= map_w || nr >= map_h {
                                true
                            } else {
                                let n_idx = (nr * map_w + nc) as usize;
                                let n_terr = terrain.get(n_idx).copied().unwrap_or(0);
                                let n_is_land = (n_terr & 0x80) != 0;
                                if !n_is_land {
                                    true
                                } else {
                                    owners.get(n_idx).copied().unwrap_or(0) != owner
                                }
                            };
                            if is_border {
                                mask |= 1 << dir;
                            }
                        }
                        ui.edge_mask_cache[tile_idx] = mask;
                    }
                }
            }
            let edge_mask_cache = &ui.edge_mask_cache;

            let max_range = radius_world.ceil() as i32;
            for r_offset in -max_range..=max_range {
                for c_offset in -max_range..=max_range {
                    let c = b_col + c_offset;
                    let r = b_row + r_offset;
                    if c >= 0 && r >= 0 && c < map_w && r < map_h {
                        let dist = hex_dist(c, r, b_col, b_row);
                        if dist as f32 <= current_range {
                            let tile_idx = (r * map_w + c) as usize;
                            let owner = owners.get(tile_idx).copied().unwrap_or(0);
                            if owner == b_owner {
                                let cell_t = (current_range - dist as f32).clamp(0.0, 1.0);
                                let mask = edge_mask_cache[tile_idx];
                                for dir in 0..4 {
                                    let is_border_edge = (mask & (1 << dir)) != 0;

                                    if is_border_edge {
                                        let hex_w_cx = c as f32 + 0.5;
                                        let hex_w_cy = r as f32 + 0.5;
                                        let edge_center_x =
                                            (input.camera_x + hex_w_cx * input.camera_zoom) / sf;
                                        let edge_center_y =
                                            (input.camera_y + hex_w_cy * input.camera_zoom) / sf;
                                        let edge_center = egui::pos2(edge_center_x, edge_center_y);

                                        const SQUARE_OFFSETS: [egui::Vec2; 4] = [
                                            egui::vec2(1.0, -1.0),  // v0: Top-Right
                                            egui::vec2(1.0, 1.0),   // v1: Bottom-Right
                                            egui::vec2(-1.0, 1.0),  // v2: Bottom-Left
                                            egui::vec2(-1.0, -1.0), // v3: Top-Left
                                        ];
                                        let square_half = 0.5 * input.camera_zoom / sf;
                                        let get_vertex = |v_idx: usize| -> egui::Pos2 {
                                            let offset = SQUARE_OFFSETS[v_idx % 4];
                                            egui::pos2(
                                                edge_center.x + square_half * offset.x,
                                                edge_center.y + square_half * offset.y,
                                            )
                                        };

                                        let (v1, v2) = match dir {
                                            0 => (get_vertex(0), get_vertex(1)), // East: v0 to v1
                                            1 => (get_vertex(2), get_vertex(3)), // West: v2 to v3
                                            2 => (get_vertex(3), get_vertex(0)), // North: v3 to v0
                                            3 => (get_vertex(1), get_vertex(2)), // South: v1 to v2
                                            _ => (get_vertex(0), get_vertex(1)),
                                        };

                                        // Background neon glow line
                                        let glow_alpha = (90.0 * border_pulse * cell_t) as u8;
                                        painter.line_segment(
                                            [v1, v2],
                                            egui::Stroke::new(
                                                5.5_f32,
                                                egui::Color32::from_rgba_unmultiplied(
                                                    player_color.r(),
                                                    player_color.g(),
                                                    player_color.b(),
                                                    glow_alpha,
                                                ),
                                            ),
                                        );

                                        // Crisp core foreground neon line
                                        let core_alpha = (255.0 * cell_t) as u8;
                                        painter.line_segment(
                                            [v1, v2],
                                            egui::Stroke::new(
                                                2.0_f32,
                                                egui::Color32::from_rgba_unmultiplied(
                                                    player_color.r(),
                                                    player_color.g(),
                                                    player_color.b(),
                                                    core_alpha,
                                                ),
                                            ),
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
