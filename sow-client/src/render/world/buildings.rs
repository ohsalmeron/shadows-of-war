use super::*;

use crate::render::world::utils::*;

pub(crate) fn get_upgrade_str(level: u8) -> &'static str {
    match level {
        1 => "🏗️ Lvl 1 ➔ 2",
        2 => "🏗️ Lvl 2 ➔ 3",
        3 => "🏗️ Lvl 3 ➔ 4",
        4 => "🏗️ Lvl 4 ➔ 5",
        5 => "🏗️ Lvl 5 ➔ 6",
        6 => "🏗️ Lvl 6 ➔ 7",
        7 => "🏗️ Lvl 7 ➔ 8",
        8 => "🏗️ Lvl 8 ➔ 9",
        9 => "🏗️ Lvl 9 ➔ 10",
        10 => "🏗️ Lvl 10 ➔ 11",
        11 => "🏗️ Lvl 11 ➔ 12",
        12 => "🏗️ Lvl 12 ➔ 13",
        13 => "🏗️ Lvl 13 ➔ 14",
        14 => "🏗️ Lvl 14 ➔ 15",
        15 => "🏗️ Lvl 15 ➔ 16",
        16 => "🏗️ Lvl 16 ➔ 17",
        17 => "🏗️ Lvl 17 ➔ 18",
        18 => "🏗️ Lvl 18 ➔ 19",
        19 => "🏗️ Lvl 19 ➔ 20",
        20 => "🏗️ Lvl 20 ➔ 21",
        21 => "🏗️ Lvl 21 ➔ 22",
        22 => "🏗️ Lvl 22 ➔ 23",
        23 => "🏗️ Lvl 23 ➔ 24",
        24 => "🏗️ Lvl 24 ➔ 25",
        25 => "🏗️ Lvl 25 ➔ 26",
        26 => "🏗️ Lvl 26 ➔ 27",
        27 => "🏗️ Lvl 27 ➔ 28",
        28 => "🏗️ Lvl 28 ➔ 29",
        29 => "🏗️ Lvl 29 ➔ 30",
        30 => "🏗️ Lvl 30 ➔ 31",
        _ => "🏗️ Lvl Upgrade",
    }
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
    let config = sim
        .engine
        .as_ref()
        .map(|e| e.state.config.clone())
        .unwrap_or_default();
    let mut edge_cache: Option<Vec<u8>> = None;
    let painter = ctx.painter.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("world_buildings"),
    ));
    let sf = ctx.sf;
    let zoom_scaled = ctx.zoom_scaled;
    let player_colors = ctx.player_colors;
    let building_scale = ctx.painter.ctx().data(|d| {
        d.get_temp::<f32>(egui::Id::new("dev_building_scale"))
            .unwrap_or(2.0)
    });
    let zoom_factor = ((zoom_scaled - 0.6) / 9.4).clamp(0.0, 1.0);
    let min_lod_scale = 0.5; // Scale when fully zoomed out
    let max_lod_scale = 1.0; // Scale when fully zoomed in
    let lod_scale = min_lod_scale + (max_lod_scale - min_lod_scale) * zoom_factor;
    let mut final_scale = building_scale * lod_scale;
    if zoom_scaled < 0.6 {
        final_scale *= 0.35; // scale down LOD 3 so it doesn't get too big
    }

    if let Some(snap) = &sim.current_snapshot {
        // S2: Restore zoom LOD gate — at zoom < 0.25, buildings are sub-pixel, skip entirely
        if zoom_scaled < 0.25 {
            return;
        }

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
        let hovered_tile_idx =
            if h_col >= 0 && h_row >= 0 && h_col < sim.map_w as i32 && h_row < sim.map_h as i32 {
                Some((h_row * sim.map_w as i32 + h_col) as u32)
            } else {
                None
            };

        struct RenderedBuilding {
            bx: f32,
            by: f32,
            kind: sow_core::game::BuildingKind,
            active_level: u8,
            target_level: u8,
            under_construction: bool,
            ticks_until_complete: u32,
            count: usize,
            owner_id: u16,
            id: Option<u64>,
            modules: Option<sow_core::building::CityModules>,
            tile_idx: Option<u32>,
        }

        let cell_size = if zoom_scaled < 0.6 {
            128.0 // LOD 3: Major sector-level grouping
        } else if zoom_scaled < 1.2 {
            64.0 // LOD 2: Intermediate grid grouping
        } else if zoom_scaled < 2.5 {
            24.0 // LOD 1: Close clustering
        } else {
            1.0 // No clustering
        };

        let mut rendered_buildings = Vec::new();

        if cell_size > 1.0 {
            #[derive(Hash, PartialEq, Eq)]
            struct ClusterKey {
                grid_x: i32,
                grid_y: i32,
                owner_id: u16,
                kind: Option<sow_core::game::BuildingKind>,
                level: Option<u8>,
            }
            let mut clusters: std::collections::HashMap<
                ClusterKey,
                (f32, f32, usize, u32, Option<sow_core::game::BuildingKind>),
            > = std::collections::HashMap::new();

            for b in &snap.buildings {
                let tile_x = (b.tile_idx % sim.map_w) as f32;
                let tile_y = (b.tile_idx / sim.map_w) as f32;
                let bx = tile_x + 0.5 + (tile_y as i32 % 2) as f32 * 0.5;
                let by = (tile_y + 0.5) * 0.8660254_f32;

                let grid_x = (tile_x / cell_size) as i32;
                let grid_y = (tile_y / cell_size) as i32;

                let (kind_key, level_key) = if zoom_scaled < 0.6 {
                    (None, None)
                } else if zoom_scaled < 1.2 {
                    (Some(b.kind), None)
                } else {
                    (Some(b.kind), Some(b.level))
                };

                let key = ClusterKey {
                    grid_x,
                    grid_y,
                    owner_id: b.owner_id,
                    kind: kind_key,
                    level: level_key,
                };

                let b_level = if b.under_construction {
                    b.active_level() as u32
                } else {
                    b.level as u32
                };

                let entry = clusters
                    .entry(key)
                    .or_insert((0.0, 0.0, 0, 0, Some(b.kind)));
                entry.0 += bx;
                entry.1 += by;
                entry.2 += 1;
                entry.3 += b_level;
            }

            for (key, (sum_bx, sum_by, count, sum_level, cluster_kind)) in clusters {
                let final_kind = key
                    .kind
                    .or(cluster_kind)
                    .unwrap_or(sow_core::game::BuildingKind::City);
                let avg_level = (sum_level / count as u32) as u8;
                rendered_buildings.push(RenderedBuilding {
                    bx: sum_bx / count as f32,
                    by: sum_by / count as f32,
                    kind: final_kind,
                    active_level: avg_level,
                    target_level: avg_level,
                    under_construction: false,
                    ticks_until_complete: 0,
                    count,
                    owner_id: key.owner_id,
                    id: None,
                    modules: None,
                    tile_idx: None,
                });
            }
        } else {
            for b in &snap.buildings {
                let tile_x = (b.tile_idx % sim.map_w) as f32;
                let tile_y = (b.tile_idx / sim.map_w) as f32;
                let bx = tile_x + 0.5 + (tile_y as i32 % 2) as f32 * 0.5;
                let by = (tile_y + 0.5) * 0.8660254_f32;
                rendered_buildings.push(RenderedBuilding {
                    bx,
                    by,
                    kind: b.kind,
                    active_level: b.active_level(),
                    target_level: b.level,
                    under_construction: b.under_construction,
                    ticks_until_complete: b.ticks_until_complete,
                    count: 1,
                    owner_id: b.owner_id,
                    id: Some(b.id),
                    modules: Some(b.modules),
                    tile_idx: Some(b.tile_idx),
                });
            }
        }

        // Depth sort bottom-to-top (and left-to-right) to make overlaps completely stable and prevent flickering
        rendered_buildings.sort_by(|a, b| {
            a.by.partial_cmp(&b.by)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.bx.partial_cmp(&b.bx).unwrap_or(std::cmp::Ordering::Equal))
                .then_with(|| a.count.cmp(&b.count))
        });

        for b in rendered_buildings {
            let screen_x = (input.camera_x + b.bx * input.camera_zoom) / sf;
            let screen_y = (input.camera_y + b.by * input.camera_zoom) / sf;

            // Frustum cull
            let margin = zoom_scaled * 2.0;
            if screen_x < -margin
                || screen_x > input.screen_w / sf + margin
                || screen_y < -margin
                || screen_y > input.screen_h / sf + margin
            {
                continue;
            }

            let center = egui::pos2(screen_x, screen_y);

            let uri = b.kind.asset().uri();

            let base_size = if b.count > 1 {
                28.0_f32.max(get_building_icon_size(zoom_scaled) * 1.2)
            } else {
                get_building_icon_size(zoom_scaled)
            } * final_scale;
            let rect = egui::Rect::from_center_size(center, egui::vec2(base_size, base_size));

            // Icon sprite rendering

            let size_hint = egui::load::SizeHint::Size {
                width: 64,
                height: 64,
                maintain_aspect_ratio: true,
            };

            let load_res =
                painter
                    .ctx()
                    .try_load_texture(uri, egui::TextureOptions::LINEAR, size_hint);

            if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res {
                let player_color = if b.owner_id != 0 {
                    player_colors
                        .get(b.owner_id as usize)
                        .copied()
                        .unwrap_or(egui::Color32::WHITE)
                } else {
                    egui::Color32::WHITE
                };

                let tint = if b.owner_id != 0 {
                    let mut color = player_color;
                    if b.under_construction {
                        color = color.gamma_multiply(0.5);
                    }
                    color
                } else {
                    if b.under_construction {
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 128)
                    } else {
                        egui::Color32::WHITE
                    }
                };

                painter.image(
                    texture.id,
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    tint,
                );

                // Render Automated City Districts (Port, Silo, Foundry)
                if b.kind == sow_core::game::BuildingKind::City && b.count == 1 {
                    if let (Some(b_id), Some(mods)) = (b.id, b.modules) {
                        let district_size = base_size * 0.75;
                        let neighbors_offsets = [
                            (0.85_f32, 0.0_f32),
                            (0.425_f32, 0.736_f32),
                            (-0.425_f32, 0.736_f32),
                            (-0.85_f32, 0.0_f32),
                            (-0.425_f32, -0.736_f32),
                            (0.425_f32, -0.736_f32),
                        ];

                        let draw_district = |uri: &str, dir_idx: usize| {
                            let (dx, dy) = neighbors_offsets[dir_idx % 6];
                            let dist_cx = screen_x + dx * input.camera_zoom / sf;
                            let dist_cy = screen_y + dy * input.camera_zoom / sf;
                            let dist_center = egui::pos2(dist_cx, dist_cy);
                            let dist_rect = egui::Rect::from_center_size(
                                dist_center,
                                egui::vec2(district_size, district_size),
                            );

                            let size_hint = egui::load::SizeHint::Size {
                                width: 48,
                                height: 48,
                                maintain_aspect_ratio: true,
                            };
                            let load_res = painter.ctx().try_load_texture(
                                uri,
                                egui::TextureOptions::LINEAR,
                                size_hint,
                            );
                            if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res {
                                let player_color = if b.owner_id != 0 {
                                    player_colors
                                        .get(b.owner_id as usize)
                                        .copied()
                                        .unwrap_or(egui::Color32::WHITE)
                                } else {
                                    egui::Color32::WHITE
                                };

                                // Draw connector line
                                painter.line_segment(
                                    [center, dist_center],
                                    egui::Stroke::new(
                                        1.2_f32,
                                        egui::Color32::from_rgba_unmultiplied(
                                            player_color.r(),
                                            player_color.g(),
                                            player_color.b(),
                                            60,
                                        ),
                                    ),
                                );

                                painter.image(
                                    texture.id,
                                    dist_rect,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                     ),
                                    player_color,
                                );
                            }
                        };

                        if mods.arsenal > 0 {
                            draw_district(
                                sow_core::assets::Asset::MissileSilo.uri(),
                                (b_id % 6) as usize,
                            );
                        }
                        if mods.port > 0 {
                            draw_district(
                                sow_core::assets::Asset::Port.uri(),
                                ((b_id + 2) % 6) as usize,
                            );
                        }
                        if mods.foundry > 0 {
                            draw_district(
                                sow_core::assets::Asset::Factory.uri(),
                                ((b_id + 4) % 6) as usize,
                            );
                        }
                    }
                }

                // ── Bunkers Firing Back VFX (LOD2 & LOD1) ──
                if zoom_scaled >= 0.8
                    && b.kind == sow_core::game::BuildingKind::Bunker
                    && b.active_level > 0
                {
                    let radius_world = config.bunker_base_range as f32
                        + (b.active_level as f32 - 1.0) * config.bunker_range_scale as f32;
                    let elapsed = time.start_time.elapsed().as_secs_f32();

                    let player_color = if b.owner_id != 0 {
                        player_colors
                            .get(b.owner_id as usize)
                            .copied()
                            .unwrap_or(egui::Color32::from_rgb(0, 220, 255))
                    } else {
                        egui::Color32::from_rgb(0, 220, 255)
                    };

                    for attack in &snap.attacks {
                        if attack.target_owner == b.owner_id && attack.troops > 0.0 {
                            if attack.front_cx == 0.0 && attack.front_cy == 0.0 {
                                continue;
                            }

                            // Convert attack front hex-grid centroid to world-space coordinates
                            let attack_wx =
                                attack.front_cx + 0.5 + ((attack.front_cy as i32) % 2) as f32 * 0.5;
                            let attack_wy = (attack.front_cy + 0.5) * 0.8660254_f32;

                            let dx = attack_wx - b.bx;
                            let dy = attack_wy - b.by;
                            let dist = (dx * dx + dy * dy).sqrt();

                            if dist <= radius_world {
                                // Zero abstraction: shoot directly towards the bunker's own radius boundary in the direction of the attack
                                let target_wx = if dist > 0.0 {
                                    b.bx + (dx / dist) * radius_world
                                } else {
                                    b.bx
                                };
                                let target_wy = if dist > 0.0 {
                                    b.by + (dy / dist) * radius_world
                                } else {
                                    b.by
                                };

                                let atk_screen_x =
                                    (input.camera_x + target_wx * input.camera_zoom) / sf;
                                let atk_screen_y =
                                    (input.camera_y + target_wy * input.camera_zoom) / sf;
                                let atk_center = egui::pos2(atk_screen_x, atk_screen_y);

                                // Laser colors
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

                                if input.screen_w < 900.0 || sf > 1.5 || zoom_scaled < 1.0 {
                                    // High-performance clean laser line for low-end / zoomed out
                                    painter.line_segment(
                                        [center, atk_center],
                                        egui::Stroke::new(5.0_f32, glow_color),
                                    );
                                    painter.line_segment(
                                        [center, atk_center],
                                        egui::Stroke::new(2.0_f32, egui::Color32::WHITE),
                                    );
                                    painter.circle_filled(
                                        atk_center,
                                        6.0_f32,
                                        egui::Color32::WHITE,
                                    );
                                    painter.circle_filled(atk_center, 9.0_f32, glow_color);
                                } else {
                                    // 1. Heavy crackling electrical/lightning conduit (jagged segments)
                                    let steps = 8;
                                    let dir = atk_center - center;
                                    let length = dir.length();
                                    if length > 1.0 {
                                        let perp = egui::vec2(-dir.y, dir.x) / length;
                                        let mut prev_pt = center;

                                        for step in 1..=steps {
                                            let t = step as f32 / steps as f32;
                                            let mut pt = center + dir * t;
                                            if step < steps {
                                                // Crackle offset using high-frequency sine waves
                                                let offset_mag = (elapsed * 45.0
                                                    + step as f32 * 1.6)
                                                    .sin()
                                                    * 5.0
                                                    + (elapsed * 95.0 - step as f32 * 2.3).cos()
                                                        * 2.5;
                                                pt += perp * offset_mag;
                                            }

                                            // Glow outer layer
                                            painter.line_segment(
                                                [prev_pt, pt],
                                                egui::Stroke::new(
                                                    8.0_f32,
                                                    glow_color.linear_multiply(0.55),
                                                ),
                                            );
                                            // Intense plasma beam core
                                            painter.line_segment(
                                                [prev_pt, pt],
                                                egui::Stroke::new(3.5_f32, core_color),
                                            );
                                            // White hot electric filament
                                            painter.line_segment(
                                                [prev_pt, pt],
                                                egui::Stroke::new(1.2_f32, egui::Color32::WHITE),
                                            );
                                            prev_pt = pt;
                                        }
                                    }

                                    // 2. Animated firing projectile stream (3 plasma bolts spaced apart)
                                    let angle =
                                        (atk_center.y - center.y).atan2(atk_center.x - center.x);
                                    let trail_len = 20.0_f32;
                                    for p_idx in 0..3 {
                                        let t = (elapsed * 3.0 + p_idx as f32 * 0.33) % 1.0;
                                        let proj_pos = egui::pos2(
                                            center.x + (atk_center.x - center.x) * t,
                                            center.y + (atk_center.y - center.y) * t,
                                        );
                                        let trail_start = egui::pos2(
                                            proj_pos.x - angle.cos() * trail_len,
                                            proj_pos.y - angle.sin() * trail_len,
                                        );

                                        // High-velocity projectile tail
                                        painter.line_segment(
                                            [trail_start, proj_pos],
                                            egui::Stroke::new(4.5_f32, glow_color),
                                        );
                                        painter.circle_filled(
                                            proj_pos,
                                            5.0_f32,
                                            egui::Color32::WHITE,
                                        );
                                        painter.circle_filled(proj_pos, 7.5_f32, glow_color);
                                    }

                                    // 3. Exploding impact sparks + shockwaves
                                    let ring_t = (elapsed * 4.0) % 1.0;
                                    painter.circle(
                                        atk_center,
                                        ring_t * 26.0,
                                        egui::Color32::TRANSPARENT,
                                        egui::Stroke::new(
                                            2.5_f32,
                                            egui::Color32::from_rgba_unmultiplied(
                                                255,
                                                255,
                                                255,
                                                ((1.0 - ring_t) * 230.0) as u8,
                                            ),
                                        ),
                                    );

                                    let spark_t = (elapsed * 6.0) % 1.0;
                                    for i in 0..8 {
                                        let angle =
                                            (i as f32 * 45.0 + elapsed * 280.0).to_radians();
                                        let spark_len = spark_t * 20.0;
                                        let spark_start = atk_center
                                            + egui::vec2(angle.cos(), angle.sin())
                                                * (spark_len * 0.25);
                                        let spark_end = atk_center
                                            + egui::vec2(angle.cos(), angle.sin()) * spark_len;
                                        painter.line_segment(
                                            [spark_start, spark_end],
                                            egui::Stroke::new(
                                                2.2_f32,
                                                egui::Color32::from_rgba_unmultiplied(
                                                    255,
                                                    235,
                                                    130,
                                                    ((1.0 - spark_t) * 255.0) as u8,
                                                ),
                                            ),
                                        );
                                    }
                                }

                                // 4. Pulse muzzle flash at bunker center
                                let muzzle_pulse = (elapsed * 30.0).sin().abs() * 3.5 + 6.0;
                                painter.circle_filled(center, muzzle_pulse, egui::Color32::WHITE);
                                painter.circle_filled(center, muzzle_pulse + 5.0, glow_color);
                            }
                        }
                    }
                }

                // Render active Bunker range circle & glowing anchor border
                if b.kind == sow_core::game::BuildingKind::Bunker
                    && b.active_level > 0
                    && painter.ctx().input(|i| i.modifiers.alt)
                {
                    let radius_world = config.bunker_base_range as f32
                        + (b.active_level as f32 - 1.0) * config.bunker_range_scale as f32;
                    let elapsed = time.start_time.elapsed().as_secs_f32();
                    let pulse = (elapsed * 2.0).sin() * 0.04 + 0.96; // soft continuous pulse

                    let bunker_start = painter.ctx().data_mut(|d| {
                        let map = d
                            .get_temp_mut_or_insert_with::<std::collections::HashMap<u64, f32>>(
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
                        // Smooth cubic ease-out for a premium, heavy cybernetic launch feel
                        let t = age / 1.5;
                        let ease = 1.0 - (1.0 - t).powi(3);
                        ease * radius_world
                    } else {
                        radius_world
                    };

                    let s_radius = current_range * input.camera_zoom / sf * pulse;
                    let player_color = if b.owner_id != 0 {
                        player_colors
                            .get(b.owner_id as usize)
                            .copied()
                            .unwrap_or(egui::Color32::from_rgb(0, 220, 255))
                    } else {
                        egui::Color32::from_rgb(0, 220, 255)
                    };

                    // 1. Draw dynamic range scanning forcefield (zero-fade solid premium contrast)
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
                    painter.circle_stroke(
                        center,
                        s_radius,
                        egui::Stroke::new(1.5_f32, stroke_color),
                    );
                    painter.circle_filled(center, s_radius, fill_color);

                    // 1b. Draw expanding scanning radar pulse wave ripple
                    let wave_t = (elapsed * 0.8) % 1.0;
                    let wave_radius = s_radius * wave_t;
                    let wave_alpha = ((1.0 - wave_t) * 140.0) as u8;
                    painter.circle_stroke(
                        center,
                        wave_radius,
                        egui::Stroke::new(
                            1.5_f32,
                            egui::Color32::from_rgba_unmultiplied(
                                player_color.r(),
                                player_color.g(),
                                player_color.b(),
                                wave_alpha,
                            ),
                        ),
                    );

                    // 2. Draw solid glowing hex outline around the Bunker itself for visual confirmation
                    let hex_r = (0.577_350_26_f32 * input.camera_zoom) / sf;
                    const HEX_OFFSETS: [egui::Vec2; 6] = [
                        egui::vec2(0.8660254, 0.5),
                        egui::vec2(0.0, 1.0),
                        egui::vec2(-0.8660254, 0.5),
                        egui::vec2(-0.8660254, -0.5),
                        egui::vec2(0.0, -1.0),
                        egui::vec2(0.8660254, -0.5),
                    ];
                    let points = vec![
                        center + HEX_OFFSETS[0] * hex_r,
                        center + HEX_OFFSETS[1] * hex_r,
                        center + HEX_OFFSETS[2] * hex_r,
                        center + HEX_OFFSETS[3] * hex_r,
                        center + HEX_OFFSETS[4] * hex_r,
                        center + HEX_OFFSETS[5] * hex_r,
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

                        let get_owner = |col: i32, row: i32| -> u16 {
                            if col >= 0 && row >= 0 && col < map_w && row < map_h {
                                owners[(row as usize * map_w as usize) + col as usize]
                            } else {
                                0
                            }
                        };

                        let get_is_land = |col: i32, row: i32| -> bool {
                            if col >= 0 && row >= 0 && col < map_w && row < map_h {
                                let t_byte =
                                    terrain[(row as usize * map_w as usize) + col as usize];
                                (t_byte & 0x80) != 0
                            } else {
                                false
                            }
                        };

                        let hex_dist = |c1: i32, r1: i32, c2: i32, r2: i32| -> i32 {
                            let q1 = c1 - (r1 - (r1 & 1)) / 2;
                            let r1 = r1;
                            let q2 = c2 - (r2 - (r2 & 1)) / 2;
                            let r2 = r2;
                            let dq = q2 - q1;
                            let dr = r2 - r1;
                            (dq.abs() + dr.abs() + (dq + dr).abs()) / 2
                        };

                        let border_pulse = (elapsed * 3.5).sin() * 0.15 + 0.85;

                        let edge_mask_cache = edge_cache.get_or_insert_with(|| {
                            let mut c_cache = vec![0u8; (map_w * map_h) as usize];
                            for row_idx in 0..map_h {
                                for col_idx in 0..map_w {
                                    let tile_idx = (row_idx * map_w + col_idx) as usize;
                                    let owner = owners.get(tile_idx).copied().unwrap_or(0);
                                    if owner == 0 {
                                        continue;
                                    }
                                    let is_odd = (row_idx % 2) != 0;
                                    let mut mask = 0u8;
                                    for dir in 0..6 {
                                        let (nc, nr) = match dir {
                                            0 => (col_idx + 1, row_idx), // East
                                            1 => (col_idx - 1, row_idx), // West
                                            2 => {
                                                if is_odd {
                                                    (col_idx, row_idx - 1)
                                                } else {
                                                    (col_idx - 1, row_idx - 1)
                                                }
                                            } // Northwest
                                            3 => {
                                                if is_odd {
                                                    (col_idx + 1, row_idx - 1)
                                                } else {
                                                    (col_idx, row_idx - 1)
                                                }
                                            } // Northeast
                                            4 => {
                                                if is_odd {
                                                    (col_idx, row_idx + 1)
                                                } else {
                                                    (col_idx - 1, row_idx + 1)
                                                }
                                            } // Southwest
                                            5 => {
                                                if is_odd {
                                                    (col_idx + 1, row_idx + 1)
                                                } else {
                                                    (col_idx, row_idx + 1)
                                                }
                                            } // Southeast
                                            _ => (col_idx, row_idx),
                                        };
                                        let is_border =
                                            if nc < 0 || nr < 0 || nc >= map_w || nr >= map_h {
                                                true
                                            } else {
                                                let n_idx = (nr * map_w + nc) as usize;
                                                let n_terr =
                                                    terrain.get(n_idx).copied().unwrap_or(0);
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
                                    c_cache[tile_idx] = mask;
                                }
                            }
                            c_cache
                        });

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
                                            let cell_t =
                                                (current_range - dist as f32).clamp(0.0, 1.0);
                                            let mask = edge_mask_cache[tile_idx];
                                            for dir in 0..6 {
                                                let is_border_edge = (mask & (1 << dir)) != 0;

                                                if is_border_edge {
                                                    let hex_w_cx =
                                                        c as f32 + 0.5 + (r % 2) as f32 * 0.5;
                                                    let hex_w_cy = (r as f32 + 0.5) * 0.8660254_f32;
                                                    let edge_center_x = (input.camera_x
                                                        + hex_w_cx * input.camera_zoom)
                                                        / sf;
                                                    let edge_center_y = (input.camera_y
                                                        + hex_w_cy * input.camera_zoom)
                                                        / sf;
                                                    let edge_center =
                                                        egui::pos2(edge_center_x, edge_center_y);

                                                    let hex_r =
                                                        (0.577_350_26_f32 * input.camera_zoom) / sf;
                                                    const HEX_OFFSETS: [egui::Vec2; 6] = [
                                                        egui::vec2(0.8660254, 0.5),
                                                        egui::vec2(0.0, 1.0),
                                                        egui::vec2(-0.8660254, 0.5),
                                                        egui::vec2(-0.8660254, -0.5),
                                                        egui::vec2(0.0, -1.0),
                                                        egui::vec2(0.8660254, -0.5),
                                                    ];
                                                    let get_vertex = |v_idx: usize| -> egui::Pos2 {
                                                        let offset = HEX_OFFSETS[v_idx % 6];
                                                        egui::pos2(
                                                            edge_center.x + hex_r * offset.x,
                                                            edge_center.y + hex_r * offset.y,
                                                        )
                                                    };

                                                    let (v1, v2) = match dir {
                                                        0 => (get_vertex(5), get_vertex(0)),
                                                        1 => (get_vertex(2), get_vertex(3)),
                                                        2 => (get_vertex(3), get_vertex(4)),
                                                        3 => (get_vertex(4), get_vertex(5)),
                                                        4 => (get_vertex(1), get_vertex(2)),
                                                        5 => (get_vertex(0), get_vertex(1)),
                                                        _ => (get_vertex(0), get_vertex(1)),
                                                    };

                                                    // Background neon glow line
                                                    let glow_alpha =
                                                        (90.0 * border_pulse * cell_t) as u8;
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

            if b.under_construction && b.ticks_until_complete > 0 {
                let active_l = b.active_level;
                let target_l = b.target_level;

                let progress = if active_l > 0 {
                    let mut queued_above_ticks = 0;
                    for lvl in (active_l + 2)..=target_l {
                        queued_above_ticks +=
                            sow_core::building::core::upgrade_duration_ticks(b.kind, lvl);
                    }
                    let ticks_current = b.ticks_until_complete.saturating_sub(queued_above_ticks);
                    let dur_current =
                        sow_core::building::core::upgrade_duration_ticks(b.kind, active_l + 1);
                    1.0 - (ticks_current as f32 / dur_current as f32).clamp(0.0, 1.0)
                } else {
                    let total_ticks = b.kind.construction_duration_ticks();
                    if total_ticks > 0 {
                        1.0 - (b.ticks_until_complete as f32 / total_ticks as f32).clamp(0.0, 1.0)
                    } else {
                        0.0
                    }
                };

                let radius = base_size * 0.28;

                // Black transparent circle panel behind the circle loader
                painter.circle_filled(
                    center,
                    radius + 2.0_f32,
                    egui::Color32::from_black_alpha(150),
                );

                // Track outline
                painter.circle_stroke(
                    center,
                    radius,
                    egui::Stroke::new(
                        2.5_f32,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 35),
                    ),
                );

                if progress > 0.0 {
                    // Sharp glowing outer progress arc
                    let num_points = (32.0 * progress).ceil().max(2.0) as usize;
                    let mut arc_points = Vec::with_capacity(num_points);
                    for i in 0..num_points {
                        let t = i as f32 / (num_points - 1) as f32;
                        let angle =
                            -std::f32::consts::FRAC_PI_2 + t * progress * std::f32::consts::TAU;
                        arc_points.push(egui::pos2(
                            center.x + radius * angle.cos(),
                            center.y + radius * angle.sin(),
                        ));
                    }

                    // Golden glowing arc for upgrade, cyan for initial construction
                    let arc_color = if active_l > 0 {
                        egui::Color32::from_rgb(250, 204, 21) // Amber / Gold
                    } else {
                        egui::Color32::from_rgb(0, 220, 255) // Cyan
                    };

                    painter.add(egui::Shape::line(
                        arc_points,
                        egui::Stroke::new(2.5_f32, arc_color),
                    ));
                }
            }

            // Level badge (no white plate background, no frame, larger text in black)
            if b.active_level != 1 && b.active_level != 0 && zoom_scaled >= 0.6 {
                let text_val = get_level_str(b.active_level);
                let font_size = (zoom_scaled * 0.65 * final_scale).clamp(8.0, 18.0).round();
                let bg_center =
                    egui::pos2(center.x + base_size * 0.45, center.y - base_size * 0.45);

                let font_id = egui::FontId::proportional(font_size);
                let galley =
                    painter.layout_no_wrap(text_val.to_owned(), font_id, egui::Color32::WHITE);
                let pos = bg_center - galley.rect.size() / 2.0;

                crate::hud::nameplate::paint_glow_nameplate_galley(
                    &painter,
                    pos,
                    galley,
                    egui::Color32::WHITE,
                    false,
                );
            }

            // Render premium golden glassmorphic floating egui badge above upgrading building
            if b.under_construction
                && b.ticks_until_complete > 0
                && b.active_level > 0
                && b.count == 1
            {
                let active_l = b.active_level;
                let target_l = b.target_level;
                let queued_count = (target_l as i32 - active_l as i32).max(0) as u32;

                let text = if queued_count > 1 {
                    std::borrow::Cow::Owned(format!(
                        "🏗️ Lvl {} ➔ {} (+{} queued)",
                        active_l,
                        active_l + 1,
                        queued_count - 1
                    ))
                } else {
                    std::borrow::Cow::Borrowed(get_upgrade_str(active_l))
                };

                let elapsed = time.start_time.elapsed().as_secs_f32();
                let bobbing = (elapsed * 3.0).sin() * 1.5;

                let font_size = (10.0_f32 * input.camera_zoom / sf).clamp(9.0, 13.0).round();
                let font_id = egui::FontId::proportional(font_size);
                let galley =
                    painter.layout_no_wrap(text.clone().into_owned(), font_id.clone(), egui::Color32::WHITE);

                let padding_x = 8.0_f32;
                let padding_y = 4.0_f32;
                let rect_w = galley.rect.width() + padding_x * 2.0;
                let rect_h = galley.rect.height() + padding_y * 2.0;

                let badge_y = center.y - base_size * 0.7 + bobbing;
                let badge_rect = egui::Rect::from_center_size(
                    egui::pos2(center.x, badge_y),
                    egui::vec2(rect_w, rect_h),
                );

                let border_color = egui::Color32::from_rgb(250, 204, 21); // Amber / Gold
                painter.rect(
                    badge_rect,
                    6.0_f32,
                    egui::Color32::from_rgba_unmultiplied(15, 23, 42, 210), // Glass slate dark
                    egui::Stroke::new(
                        1.2_f32,
                        egui::Color32::from_rgba_unmultiplied(
                            border_color.r(),
                            border_color.g(),
                            border_color.b(),
                            180,
                        ),
                    ),
                    egui::StrokeKind::Inside,
                );

                painter.text(
                    egui::pos2(center.x, badge_y),
                    egui::Align2::CENTER_CENTER,
                    &text,
                    font_id,
                    egui::Color32::from_rgb(254, 240, 138), // Very soft warm golden text
                );
            }

            // Render floating stats tooltip on hover
            if b.active_level > 0 && b.count == 1 {
                let is_hovered = if let Some(snap_b) =
                    snap.buildings.iter().find(|sb| sb.id == b.id.unwrap_or(0))
                {
                    hovered_tile_idx == Some(snap_b.tile_idx)
                } else {
                    false
                };

                if is_hovered {
                    let b_id = b.id.unwrap_or(0);
                    if ui.cached_hovered_building_id != Some(b_id) || ui.cached_hovered_building_level != b.active_level {
                        ui.cached_hovered_building_id = Some(b_id);
                        ui.cached_hovered_building_level = b.active_level;
                        ui.cached_hovered_building_tooltip = match b.kind {
                            sow_core::game::BuildingKind::Bunker => {
                                let penalty_prio = b.active_level * 4;
                                let extra_loss = b.active_level * 40;
                                let title = format!("🛡️ Defense Tower (Lvl {})", b.active_level);
                                let stat1 = format!(
                                    "Coverage: {} Hex Radius",
                                    (config.bunker_base_range
                                        + (b.active_level as f64 - 1.0) * config.bunker_range_scale)
                                        .round() as i32
                                );
                                let stat2 = format!("Atk Delay Penalty: +{}", penalty_prio);
                                let stat3 = format!("Atk Loss Penalty: +{}%", extra_loss);
                                format!("{}\n{}\n{}\n{}", title, stat1, stat2, stat3)
                            }
                            sow_core::game::BuildingKind::Factory => {
                                let title = format!("🏭 Industrial Factory (Lvl {})", b.active_level);
                                let income_val = config.factory_base_income
                                    + (b.active_level as f64 - 1.0) * config.factory_income_scale;
                                let stat1 = format!("Gold Generation: +{:.1}/s", income_val);
                                format!("{}\n{}", title, stat1)
                            }
                            sow_core::game::BuildingKind::Port => {
                                let title = format!("⚓ Maritime Port (Lvl {})", b.active_level);
                                let stat1 = "Fleet Support: Enabled".to_string();
                                let stat2 =
                                    format!("Troop Income: +{:.1}/s", b.active_level as f64 * 25.0);
                                let stat3 =
                                    format!("Gold Income: +{:.1}/s", b.active_level as f64 * 50.0);
                                format!("{}\n{}\n{}\n{}", title, stat1, stat2, stat3)
                            }
                            _ => String::new(),
                        };
                    }

                    let tooltip_text = &ui.cached_hovered_building_tooltip;

                    if !tooltip_text.is_empty() {
                        let font_size = (9.0_f32 * input.camera_zoom / sf).clamp(9.0, 12.0).round();
                        let font_id = egui::FontId::proportional(font_size);
                        let galley = painter.layout_no_wrap(
                            tooltip_text.clone(),
                            font_id.clone(),
                            egui::Color32::WHITE,
                        );

                        let padding_x = 8.0_f32;
                        let padding_y = 5.0_f32;
                        let rect_w = galley.rect.width() + padding_x * 2.0;
                        let rect_h = galley.rect.height() + padding_y * 2.0;

                        let tooltip_y = center.y - base_size * 0.8;
                        let tooltip_rect = egui::Rect::from_center_size(
                            egui::pos2(center.x, tooltip_y),
                            egui::vec2(rect_w, rect_h),
                        );

                        // Premium slate blue dark glassmorphic container with owner's tint outline
                        let player_color = if b.owner_id != 0 {
                            player_colors
                                .get(b.owner_id as usize)
                                .copied()
                                .unwrap_or(egui::Color32::from_rgb(34, 211, 238))
                        } else {
                            egui::Color32::from_rgb(34, 211, 238)
                        };
                        painter.rect(
                            tooltip_rect,
                            4.0_f32,
                            egui::Color32::from_rgba_unmultiplied(15, 23, 42, 220),
                            egui::Stroke::new(
                                1.0_f32,
                                egui::Color32::from_rgba_unmultiplied(
                                    player_color.r(),
                                    player_color.g(),
                                    player_color.b(),
                                    180,
                                ),
                            ),
                            egui::StrokeKind::Inside,
                        );

                        painter.text(
                            egui::pos2(center.x, tooltip_y),
                            egui::Align2::CENTER_CENTER,
                            &tooltip_text,
                            font_id,
                            egui::Color32::WHITE,
                        );
                    }
                }
            }
        }

        // Building Placement Snap Preview
        if let Some(kind) = ui.app.hud_state.selected_building_kind {
            if let Some(hovered_t) = hovered_tile_idx {
                let h_col = (hovered_t as i32) % sim.map_w as i32;
                let h_row = (hovered_t as i32) / sim.map_w as i32;
                let my_id = sim.my_player_id.unwrap_or(0);
                let owners = gfx
                    .map_renderer
                    .as_ref()
                    .map(|mr| mr.owners.as_slice())
                    .unwrap_or(&[]);

                let snapped_res = crate::input::resolve_building_placement_tile(
                    kind,
                    h_col,
                    h_row,
                    sim.map_w,
                    sim.map_h,
                    owners,
                    ctx.terrain,
                    my_id,
                    &snap.buildings,
                );

                let (target_tile, is_valid) = match snapped_res {
                    Ok(t) => (t, true),
                    Err(_) => (hovered_t, false),
                };

                let cost = {
                    let i = sow_core::game::BuildingKind::ALL
                        .iter()
                        .position(|&k| k == kind)
                        .unwrap_or(0);
                    ui.app.hud_state.building_costs[i]
                };

                let has_gold = ui.app.hud_state.gold >= cost;

                let tx = (target_tile % sim.map_w) as f32;
                let ty = (target_tile / sim.map_w) as f32;
                let hex_w_cx = tx + 0.5 + (ty as i32 % 2) as f32 * 0.5;
                let hex_w_cy = (ty + 0.5) * 0.8660254_f32;
                let center_x = (input.camera_x + hex_w_cx * input.camera_zoom) / sf;
                let center_y = (input.camera_y + hex_w_cy * input.camera_zoom) / sf;
                let preview_center = egui::pos2(center_x, center_y);

                // Draw SNAPPED outline
                let hex_r = (0.577_350_26_f32 * input.camera_zoom) / sf;
                const HEX_OFFSETS: [egui::Vec2; 6] = [
                    egui::vec2(0.8660254, 0.5),
                    egui::vec2(0.0, 1.0),
                    egui::vec2(-0.8660254, 0.5),
                    egui::vec2(-0.8660254, -0.5),
                    egui::vec2(0.0, -1.0),
                    egui::vec2(0.8660254, -0.5),
                ];
                let points = [
                    preview_center + HEX_OFFSETS[0] * hex_r,
                    preview_center + HEX_OFFSETS[1] * hex_r,
                    preview_center + HEX_OFFSETS[2] * hex_r,
                    preview_center + HEX_OFFSETS[3] * hex_r,
                    preview_center + HEX_OFFSETS[4] * hex_r,
                    preview_center + HEX_OFFSETS[5] * hex_r,
                ];

                let outline_color = if is_valid && has_gold {
                    egui::Color32::from_rgb(34, 211, 238) // Glowing Cyan
                } else {
                    egui::Color32::from_rgb(239, 68, 68) // Glowing Red
                };

                painter.add(egui::Shape::convex_polygon(
                    points.to_vec(),
                    egui::Color32::from_rgba_unmultiplied(
                        outline_color.r(),
                        outline_color.g(),
                        outline_color.b(),
                        25,
                    ),
                    egui::Stroke::new(3.0_f32, outline_color),
                ));

                // Draw Bunker range circle preview if Bunker
                if kind == sow_core::game::BuildingKind::Bunker {
                    let current_range = config.bunker_base_range as f32;
                    let s_radius = (current_range * input.camera_zoom) / sf;
                    let range_color = egui::Color32::from_rgba_unmultiplied(239, 68, 68, 120);
                    let range_fill = egui::Color32::from_rgba_unmultiplied(239, 68, 68, 10);
                    painter.circle_filled(preview_center, s_radius, range_fill);
                    painter.circle_stroke(
                        preview_center,
                        s_radius,
                        egui::Stroke::new(1.5_f32, range_color),
                    );
                }

                // Draw preview ghost sprite
                let size_hint = egui::load::SizeHint::Size {
                    width: 64,
                    height: 64,
                    maintain_aspect_ratio: true,
                };
                let uri = kind.asset().uri();
                let load_res =
                    painter
                        .ctx()
                        .try_load_texture(uri, egui::TextureOptions::LINEAR, size_hint);
                let base_size = get_building_icon_size(zoom_scaled) * final_scale;
                let rect =
                    egui::Rect::from_center_size(preview_center, egui::vec2(base_size, base_size));

                if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res {
                    let ghost_alpha = if is_valid && has_gold { 140 } else { 80 };
                    let tint = egui::Color32::from_rgba_unmultiplied(255, 255, 255, ghost_alpha);
                    painter.image(
                        texture.id,
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        tint,
                    );
                }

                // If player cannot afford the structure, render the unified red deficit circular panel
                if !has_gold {
                    let deficit = cost - ui.app.hud_state.gold;
                    let radius = base_size * 0.28;

                    // Black transparent circular backing panel with red deficit tint
                    painter.circle_filled(
                        preview_center,
                        radius + 2.0_f32,
                        egui::Color32::from_black_alpha(180),
                    );
                    painter.circle_filled(
                        preview_center,
                        radius + 2.0_f32,
                        egui::Color32::from_rgba_unmultiplied(220, 38, 38, 35),
                    );

                    // Red glowing border
                    painter.circle_stroke(
                        preview_center,
                        radius,
                        egui::Stroke::new(
                            2.5_f32,
                            egui::Color32::from_rgba_unmultiplied(239, 68, 68, 180),
                        ),
                    );

                    // Render negative cost text formatted centered inside circle
                    let text_val = format!("-{}", deficit);
                    let font_size = (zoom_scaled * 0.65 * final_scale).clamp(10.0, 20.0).round();
                    let font_id = egui::FontId::proportional(font_size);
                    let galley = painter.layout_no_wrap(
                        text_val,
                        font_id,
                        egui::Color32::from_rgb(248, 113, 113),
                    );
                    let text_pos = preview_center - galley.rect.size() / 2.0;
                    crate::hud::nameplate::paint_glow_nameplate_galley(
                        &painter,
                        text_pos,
                        galley,
                        egui::Color32::from_rgba_unmultiplied(239, 68, 68, 180),
                        false,
                    );
                }
            }
        }
    }
}
