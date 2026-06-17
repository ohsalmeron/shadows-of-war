use super::*;

use crate::render::world::movers::{tile_to_world, world_to_tile};
use crate::render::world::utils::*;

pub(crate) fn upgrade_level_label(level: u8) -> String {
    format!("Lvl {} -> {}", level, level + 1)
}

pub(crate) struct BuildingUpgradePlateLine {
    pub text: String,
    pub color: egui::Color32,
    pub scale: f32, // 1.0 main, 0.85 secondary
}

pub(crate) struct BuildingUpgradePlate {
    pub anchor: egui::Pos2,
    pub base_size: f32,
    pub bobbing: f32,
    pub border_color: egui::Color32,
    pub lines: Vec<BuildingUpgradePlateLine>,
}

pub(crate) fn paint_building_upgrade_plate(
    painter: &egui::Painter,
    plate: BuildingUpgradePlate,
    camera_zoom: f32,
    sf: f32,
) {
    let font_size = (8.0_f32 * camera_zoom / sf).clamp(7.0, 10.0).round();

    let padding_x = 10.0_f32;
    let padding_y = 6.0_f32;
    let column_gap = 6.0_f32;
    let line_gap = 3.0_f32;

    let emoji_size = font_size * 1.4;

    let mut text_w = 0.0_f32;
    let mut text_h = 0.0_f32;
    let mut line_sizes = Vec::new();

    for (i, line) in plate.lines.iter().enumerate() {
        let line_font_size = (font_size * line.scale).round();
        let font_id = egui::FontId::proportional(line_font_size);
        let size = sow_ui::widgets::measure_emoji_text(painter, &line.text, &font_id);
        text_w = text_w.max(size.x);
        if i > 0 {
            text_h += line_gap;
        }
        text_h += size.y;
        line_sizes.push(size);
    }

    let box_w = padding_x * 2.0 + emoji_size + column_gap + text_w;
    let box_h = padding_y * 2.0 + text_h.max(emoji_size);

    let building_top = plate.anchor.y - plate.base_size * 0.5;
    let gap = 4.0_f32; // small air between icon and plate
    let plate_center_y = building_top - gap - box_h * 0.5 + plate.bobbing;
    let badge_rect = egui::Rect::from_center_size(
        egui::pos2(plate.anchor.x, plate_center_y),
        egui::vec2(box_w, box_h),
    );

    painter.rect(
        badge_rect,
        6.0_f32,
        egui::Color32::from_rgba_unmultiplied(15, 23, 42, 210), // Glass slate dark
        egui::Stroke::new(
            1.2_f32,
            egui::Color32::from_rgba_unmultiplied(
                plate.border_color.r(),
                plate.border_color.g(),
                plate.border_color.b(),
                200,
            ),
        ),
        egui::StrokeKind::Inside,
    );

    // Left column: 🏗️ emoji centered
    let emoji_center_x = badge_rect.left() + padding_x + emoji_size * 0.5;
    let emoji_center_y = badge_rect.center().y;
    let emoji_center = egui::pos2(emoji_center_x, emoji_center_y);

    if !sow_ui::widgets::paint_emoji_centered(
        painter,
        "🏗️",
        emoji_center,
        emoji_size,
        egui::Color32::WHITE,
    ) {
        painter.text(
            emoji_center,
            egui::Align2::CENTER_CENTER,
            "🏗️",
            egui::FontId::proportional(emoji_size * 0.7),
            egui::Color32::WHITE,
        );
    }

    // Right column: left-aligned lines
    let text_start_x = badge_rect.left() + padding_x + emoji_size + column_gap;
    let text_start_y = badge_rect.center().y - text_h * 0.5;

    let mut current_y = text_start_y;
    for (i, line) in plate.lines.iter().enumerate() {
        let line_font_size = (font_size * line.scale).round();
        let font_id = egui::FontId::proportional(line_font_size);
        let size = line_sizes[i];

        let line_pos = egui::pos2(text_start_x, current_y + size.y * 0.5);

        sow_ui::widgets::paint_emoji_text_at(
            painter,
            line_pos,
            egui::Align2::LEFT_CENTER,
            &line.text,
            font_id,
            line.color,
            false,
        );

        current_y += size.y + line_gap;
    }
}

pub(crate) fn building_kind_emoji(kind: sow_core::game::BuildingKind) -> &'static str {
    match kind {
        sow_core::game::BuildingKind::City => "🏛️",
        sow_core::game::BuildingKind::Factory => "🏭",
        sow_core::game::BuildingKind::Port => "⚓",
        sow_core::game::BuildingKind::Bunker => "🛡️",
    }
}

pub(crate) fn paint_new_build_ghost(
    painter: &egui::Painter,
    kind: sow_core::game::BuildingKind,
    center: egui::Pos2,
    base_size: f32,
) {
    let rect = egui::Rect::from_center_size(center, egui::vec2(base_size, base_size));
    let emoji = building_kind_emoji(kind);
    if !sow_ui::widgets::try_paint_emoji(painter, emoji, rect, egui::Color32::WHITE) {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            emoji,
            egui::FontId::proportional(base_size * 0.7),
            egui::Color32::WHITE,
        );
    }
}

pub(crate) fn paint_gold_preview_indicator(
    painter: &egui::Painter,
    center: egui::Pos2,
    base_size: f32,
    amount_text: &str,
    text_color: egui::Color32,
    zoom_scaled: f32,
    final_scale: f32,
) {
    let font_size = (zoom_scaled * 0.65 * final_scale).clamp(10.0, 20.0).round();
    let font_id = egui::FontId::proportional(font_size);
    let emoji_size = font_size * 1.4;
    let amount_size = sow_ui::widgets::measure_emoji_text(painter, amount_text, &font_id);
    let gap = 1.0_f32;
    let total_w = emoji_size + gap + amount_size.x;
    let start_x = center.x - total_w * 0.5;
    let indicator_y = center.y + base_size * 0.4;

    sow_ui::widgets::paint_emoji_centered(
        painter,
        "🪙",
        egui::pos2(start_x + emoji_size * 0.5, indicator_y),
        emoji_size,
        egui::Color32::WHITE,
    );

    sow_ui::widgets::paint_emoji_text_at(
        painter,
        egui::pos2(start_x + emoji_size + gap, indicator_y),
        egui::Align2::LEFT_CENTER,
        amount_text,
        font_id,
        text_color,
        true,
    );
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
    let default_config;
    let config = if let Some(e) = sim.engine.as_ref() {
        &e.state.config
    } else {
        default_config = sow_core::game_config::GameConfig::default();
        &default_config
    };
    let edge_cache_stale = sim
        .current_snapshot
        .as_ref()
        .is_some_and(|s| !s.dirty_tiles.is_empty());
    let painter = ctx.painter.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("world_buildings"),
    ));
    let sf = ctx.sf;
    let zoom_scaled = ctx.zoom_scaled;
    let player_colors = ctx.player_colors;
    let building_scale = ctx.painter.ctx().data(|d| {
        d.get_temp::<f32>(egui::Id::new("dev_building_scale"))
            .unwrap_or(1.0)
    });
    let far_zoom_threshold = ClientVisualConfig::default().far_zoom_lod_threshold;
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
        let (h_col, h_row) = world_to_tile(world_x, world_y);
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
        } else if zoom_scaled < far_zoom_threshold {
            24.0 // LOD 1: Close clustering
        } else {
            1.0 // No clustering
        };

        let building_count = snap.buildings.len();
        let mut rendered_buildings = Vec::with_capacity(building_count);

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
            > = std::collections::HashMap::with_capacity(building_count / 4);

            for b in &snap.buildings {
                if zoom_scaled < 0.6 && b.kind != sow_core::game::BuildingKind::City {
                    continue;
                }

                let (bx, by) = tile_to_world(b.tile_idx, sim.map_w);
                let tile_x = (b.tile_idx % sim.map_w) as f32;
                let tile_y = (b.tile_idx / sim.map_w) as f32;

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
                let (bx, by) = tile_to_world(b.tile_idx, sim.map_w);
                let tile_x = (b.tile_idx % sim.map_w) as f32;
                let tile_y = (b.tile_idx / sim.map_w) as f32;
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

            // LOD 3: Draw "City Lights" Heatmap Nodes
            if zoom_scaled < 0.6 {
                let player_color = if b.owner_id != 0 {
                    player_colors
                        .get(b.owner_id as usize)
                        .copied()
                        .unwrap_or(egui::Color32::WHITE)
                } else {
                    egui::Color32::WHITE
                };

                let dot_radius = if b.count > 1 {
                    (0.5 + (b.count as f32).sqrt().min(3.0)) * final_scale
                } else {
                    0.5 * final_scale
                };

                let glow_alpha = (b.active_level as f32 / 10.0).clamp(0.2, 1.0) * 150.0;
                let color_glow = egui::Color32::from_rgba_unmultiplied(
                    player_color.r(),
                    player_color.g(),
                    player_color.b(),
                    glow_alpha as u8,
                );
                let color_core = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200);

                painter.circle_filled(center, dot_radius * 2.0, color_glow);
                painter.circle_filled(center, dot_radius * 0.8, color_core);
                continue;
            }

            let uri = b.kind.asset().uri();

            let base_size = if b.count > 1 {
                28.0_f32.max(get_building_icon_size(zoom_scaled) * 1.2)
            } else {
                get_building_icon_size(zoom_scaled)
            } * final_scale;
            let rect = egui::Rect::from_center_size(center, egui::vec2(base_size, base_size));

            // Icon sprite rendering

            let show_building = true;

            if show_building {
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

                let emoji = building_kind_emoji(b.kind);

                if !sow_ui::widgets::try_paint_emoji(&painter, emoji, rect, tint) {
                    painter.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        emoji,
                        egui::FontId::proportional(base_size * 0.7),
                        tint,
                    );
                }

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

                        let draw_district = |emoji: &str, dir_idx: usize| {
                            let (dx, dy) = neighbors_offsets[dir_idx % 6];
                            let dist_cx = screen_x + dx * input.camera_zoom / sf;
                            let dist_cy = screen_y + dy * input.camera_zoom / sf;
                            let dist_center = egui::pos2(dist_cx, dist_cy);
                            let dist_rect = egui::Rect::from_center_size(
                                dist_center,
                                egui::vec2(district_size, district_size),
                            );

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

                            if !sow_ui::widgets::try_paint_emoji(
                                &painter,
                                emoji,
                                dist_rect,
                                player_color,
                            ) {
                                painter.text(
                                    dist_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    emoji,
                                    egui::FontId::proportional(district_size * 0.7),
                                    player_color,
                                );
                            }
                        };

                        if mods.arsenal > 0 {
                            draw_district("🚀", (b_id % 6) as usize);
                        }
                        if mods.port > 0 {
                            draw_district("⚓", ((b_id + 2) % 6) as usize);
                        }
                        if mods.foundry > 0 {
                            draw_district("🏭", ((b_id + 4) % 6) as usize);
                        }
                    }
                }

                // ── Bunkers Firing Back VFX (LOD2 & LOD1) ──
                if zoom_scaled >= 0.8
                    && b.kind == sow_core::game::BuildingKind::Bunker
                    && b.active_level > 0
                {
                    let radius_world = config.bunker_range as f32;
                    let elapsed = time.start_time.elapsed().as_secs_f32();
                    let laser_opts = bunker_laser_vfx_opts(painter.ctx());
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
                            let hex_dist = sow_core::building::hex_distance(
                                b_col, b_row, attack_col, attack_row,
                            );
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

                            let atk_screen_x =
                                (input.camera_x + target_wx * input.camera_zoom) / sf;
                            let atk_screen_y =
                                (input.camera_y + target_wy * input.camera_zoom) / sf;
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
                                &painter,
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
                                        sow_audio::SpatialSoundParams {
                                            wx: b.bx,
                                            wy: b.by,
                                            camera_x: input.camera_x,
                                            camera_y: input.camera_y,
                                            camera_zoom: input.camera_zoom,
                                            screen_w: input.screen_w,
                                            screen_h: input.screen_h,
                                        },
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
                {
                    let radius_world = config.bunker_range as f32;
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
                            &painter,
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
                    if wave_range > 0.5 {
                        if let Some(t_idx) = b.tile_idx {
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
                                &painter,
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
                                            let cell_t =
                                                (current_range - dist as f32).clamp(0.0, 1.0);
                                            let mask = edge_mask_cache[tile_idx];
                                            for dir in 0..4 {
                                                let is_border_edge = (mask & (1 << dir)) != 0;

                                                if is_border_edge {
                                                    let hex_w_cx = c as f32 + 0.5;
                                                    let hex_w_cy = r as f32 + 0.5;
                                                    let edge_center_x = (input.camera_x
                                                        + hex_w_cx * input.camera_zoom)
                                                        / sf;
                                                    let edge_center_y = (input.camera_y
                                                        + hex_w_cy * input.camera_zoom)
                                                        / sf;
                                                    let edge_center =
                                                        egui::pos2(edge_center_x, edge_center_y);

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

                let main_text = upgrade_level_label(active_l);
                let text_color = egui::Color32::from_rgb(254, 240, 138); // Very soft warm golden text

                let mut lines = vec![BuildingUpgradePlateLine {
                    text: main_text,
                    color: text_color,
                    scale: 1.0,
                }];

                if queued_count > 1 {
                    lines.push(BuildingUpgradePlateLine {
                        text: format!("(+{} queued)", queued_count - 1),
                        color: text_color,
                        scale: 0.85,
                    });
                }

                let border_color = egui::Color32::from_rgb(250, 204, 21); // Amber / Gold
                let elapsed = time.start_time.elapsed().as_secs_f32();
                let bobbing = (elapsed * 3.0).sin() * 1.5;

                let plate = BuildingUpgradePlate {
                    anchor: center,
                    base_size,
                    bobbing,
                    border_color,
                    lines,
                };

                paint_building_upgrade_plate(&painter, plate, input.camera_zoom, sf);
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
                    if ui.cached_hovered_building_id != Some(b_id)
                        || ui.cached_hovered_building_level != b.active_level
                    {
                        ui.cached_hovered_building_id = Some(b_id);
                        ui.cached_hovered_building_level = b.active_level;
                        let lang = ui.app.settings_state.language;
                        let s = &sow_i18n::get(lang).hud;
                        ui.cached_hovered_building_tooltip = match b.kind {
                            sow_core::game::BuildingKind::Bunker => {
                                let stat1 = s
                                    .build_bunker_coverage
                                    .replace("{}", &config.bunker_range.round().to_string());
                                format!("{}\n{}", s.build_bunker_title, stat1)
                            }
                            sow_core::game::BuildingKind::Factory => {
                                let stat1 = s
                                    .build_factory_gold
                                    .replace("{}", &format!("{:.1}", config.factory_gold_income));
                                format!("{}\n{}", s.build_factory_title, stat1)
                            }
                            sow_core::game::BuildingKind::Port => {
                                let title = s
                                    .build_port_title
                                    .replace("{}", &b.active_level.to_string());
                                let stat2 = s
                                    .build_port_troops
                                    .replace("{}", &format!("{:.1}", b.active_level as f64 * 25.0));
                                let stat3 = s
                                    .build_port_gold
                                    .replace("{}", &format!("{:.1}", b.active_level as f64 * 50.0));
                                format!("{}\n{}\n{}\n{}", title, s.build_port_fleet, stat2, stat3)
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
                            tooltip_text,
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

                let terrain = gfx
                    .map_renderer
                    .as_ref()
                    .map(|mr| mr.terrain.as_slice())
                    .unwrap_or(&[]);

                let target_res = crate::input::resolve_build_target_tile(
                    kind,
                    h_col,
                    h_row,
                    sim.map_w,
                    sim.map_h,
                    owners,
                    terrain,
                    my_id,
                    &snap.buildings,
                );

                let stack_target = crate::input::find_stack_target_tile(
                    kind,
                    h_col,
                    h_row,
                    sim.map_w,
                    my_id,
                    &snap.buildings,
                )
                .and_then(|tile| {
                    snap.buildings
                        .iter()
                        .find(|b| b.tile_idx == tile && b.owner_id == my_id && b.kind == kind)
                });

                let preview_tile = target_res.unwrap_or(hovered_t);
                let is_valid = target_res.is_ok();

                let cost = {
                    let i = sow_core::game::BuildingKind::ALL
                        .iter()
                        .position(|&k| k == kind)
                        .unwrap_or(0);
                    ui.app.hud_state.building_costs[i]
                };

                let has_gold = ui.app.hud_state.gold >= cost;
                let tx = (preview_tile % sim.map_w) as f32;
                let ty = (preview_tile / sim.map_w) as f32;
                let hex_w_cx = tx + 0.5;
                let hex_w_cy = ty + 0.5;
                let center_x = (input.camera_x + hex_w_cx * input.camera_zoom) / sf;
                let center_y = (input.camera_y + hex_w_cy * input.camera_zoom) / sf;
                let preview_center = egui::pos2(center_x, center_y);

                // Draw SNAPPED outline
                let square_half = 0.5 * input.camera_zoom / sf;
                const SQUARE_OFFSETS: [egui::Vec2; 4] = [
                    egui::vec2(1.0, -1.0),
                    egui::vec2(1.0, 1.0),
                    egui::vec2(-1.0, 1.0),
                    egui::vec2(-1.0, -1.0),
                ];
                let points = [
                    preview_center + SQUARE_OFFSETS[0] * square_half,
                    preview_center + SQUARE_OFFSETS[1] * square_half,
                    preview_center + SQUARE_OFFSETS[2] * square_half,
                    preview_center + SQUARE_OFFSETS[3] * square_half,
                ];

                let is_stack = stack_target.is_some();
                let can_place = is_valid && has_gold;
                let outline_color = if can_place {
                    egui::Color32::from_rgb(34, 211, 238) // Cyan — new build and upgrade
                } else {
                    egui::Color32::from_rgb(239, 68, 68) // Red — invalid / broke
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

                let base_size = get_building_icon_size(zoom_scaled) * final_scale;

                if is_stack {
                    // 1. Upgrade hover highlight: glow only, no duplicate sprite
                    let glow_color = if has_gold {
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 45)
                    // Soft white highlight glow
                    } else {
                        egui::Color32::from_rgba_unmultiplied(239, 68, 68, 30) // Soft red warning glow
                    };
                    painter.circle_filled(preview_center, base_size * 0.55, glow_color);

                    // 2. Upgrade plate
                    if let Some(sb) = stack_target {
                        let current_lvl = sb.active_level();
                        let target_lvl = sb.level;
                        let elapsed = time.start_time.elapsed().as_secs_f32();
                        let bobbing = (elapsed * 3.0).sin() * 1.5;

                        let border_color = if has_gold {
                            egui::Color32::from_rgb(250, 204, 21) // Gold
                        } else {
                            egui::Color32::from_rgb(239, 68, 68) // Red
                        };

                        let main_color = egui::Color32::from_rgb(254, 240, 138); // soft gold

                        let mut lines = vec![BuildingUpgradePlateLine {
                            text: upgrade_level_label(current_lvl),
                            color: main_color,
                            scale: 1.0,
                        }];

                        if target_lvl > current_lvl {
                            lines.push(BuildingUpgradePlateLine {
                                text: format!("(+{} queued)", target_lvl - current_lvl),
                                color: main_color,
                                scale: 0.85,
                            });
                        }

                        let plate = BuildingUpgradePlate {
                            anchor: preview_center,
                            base_size,
                            bobbing,
                            border_color,
                            lines,
                        };

                        paint_building_upgrade_plate(&painter, plate, input.camera_zoom, sf);
                    }
                } else {
                    // New placement: ghost emoji and range circle
                    if kind == sow_core::game::BuildingKind::Bunker {
                        let current_range = config.bunker_range as f32;
                        let range_color = egui::Color32::from_rgba_unmultiplied(239, 68, 68, 120);
                        let range_fill = egui::Color32::from_rgba_unmultiplied(239, 68, 68, 10);
                        let (preview_col, preview_row) = world_to_tile(
                            (preview_center.x * sf - input.camera_x) / input.camera_zoom,
                            (preview_center.y * sf - input.camera_y) / input.camera_zoom,
                        );
                        paint_bunker_hex_range(
                            &painter,
                            preview_col,
                            preview_row,
                            current_range,
                            WorldPaintCamera {
                                camera_x: input.camera_x,
                                camera_y: input.camera_y,
                                camera_zoom: input.camera_zoom,
                                sf,
                            },
                            range_fill,
                            range_color,
                        );
                    }

                    paint_new_build_ghost(&painter, kind, preview_center, base_size);
                }

                // 3. Gold surplus/deficit indicator below
                let (amount_text, text_color) = if has_gold {
                    let leftover = ui.app.hud_state.gold - cost;
                    (
                        format!("+{}", sow_ui::utils::format_number(leftover)),
                        egui::Color32::from_rgb(74, 222, 128), // Green
                    )
                } else {
                    let deficit = cost - ui.app.hud_state.gold;
                    (
                        format!("-{}", sow_ui::utils::format_number(deficit)),
                        egui::Color32::from_rgb(248, 113, 113), // Red
                    )
                };

                paint_gold_preview_indicator(
                    &painter,
                    preview_center,
                    base_size,
                    &amount_text,
                    text_color,
                    zoom_scaled,
                    final_scale,
                );
            }
        }
    }
}
