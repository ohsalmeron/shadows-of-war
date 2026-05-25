use super::*;

use crate::render::world::utils::*;

#[allow(unused_variables)]
pub(crate) fn render(
    ui: &mut crate::app::UiState,
    sim: &crate::app::SimState,
    input: &crate::app::InputState,
    time: &crate::app::TimeState,
    gfx: &crate::app::GraphicsState,
    ctx: &RenderContext,
) {
    let painter = ctx.painter.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("world_buildings"),
    ));
    let sf = ctx.sf;
    let zoom_scaled = ctx.zoom_scaled;
    let player_colors = ctx.player_colors;
    let building_scale = ctx.painter.ctx().data(|d| {
        d.get_temp::<f32>(egui::Id::new("dev_building_scale"))
            .unwrap_or(3.0)
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
            level: u32,
            under_construction: bool,
            ticks_until_complete: u32,
            count: usize,
            owner_id: u16,
            id: Option<u64>,
            modules: Option<sow_core::building::CityModules>,
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
                    1
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
                rendered_buildings.push(RenderedBuilding {
                    bx: sum_bx / count as f32,
                    by: sum_by / count as f32,
                    kind: final_kind,
                    level: sum_level,
                    under_construction: false,
                    ticks_until_complete: 0,
                    count,
                    owner_id: key.owner_id,
                    id: None,
                    modules: None,
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
                    level: b.level as u32,
                    under_construction: b.under_construction,
                    ticks_until_complete: b.ticks_until_complete,
                    count: 1,
                    owner_id: b.owner_id,
                    id: Some(b.id),
                    modules: Some(b.modules),
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

                let r = ((player_color.r() as f32 * 0.20) + (255.0 * 0.80)) as u8;
                let g = ((player_color.g() as f32 * 0.20) + (255.0 * 0.80)) as u8;
                let b_val = ((player_color.b() as f32 * 0.20) + (255.0 * 0.80)) as u8;

                let alpha = if b.under_construction { 110 } else { 166 }; // 0.65 opacity

                let tint = egui::Color32::from_rgba_unmultiplied(r, g, b_val, alpha);
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

                                let r = ((player_color.r() as f32 * 0.20) + (255.0 * 0.80)) as u8;
                                let g = ((player_color.g() as f32 * 0.20) + (255.0 * 0.80)) as u8;
                                let b_val =
                                    ((player_color.b() as f32 * 0.20) + (255.0 * 0.80)) as u8;
                                let tint = egui::Color32::from_rgba_unmultiplied(r, g, b_val, 166); // 0.65 opacity

                                // Draw connector line
                                painter.line_segment(
                                    [center, dist_center],
                                    egui::Stroke::new(
                                        1.2_f32,
                                        egui::Color32::from_rgba_unmultiplied(r, g, b_val, 60),
                                    ),
                                );

                                painter.image(
                                    texture.id,
                                    dist_rect,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    tint,
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

                // Render active Bunker range circle & glowing anchor border
                if b.kind == sow_core::game::BuildingKind::Bunker && !b.under_construction {
                    let radius_world = 8.0_f32; // config::DEFENSE_POST_RANGE
                    let elapsed = time.start_time.elapsed().as_secs_f32();
                    let pulse = (elapsed * 2.0).sin() * 0.04 + 0.96; // soft continuous pulse
                    let s_radius = radius_world * input.camera_zoom / sf * pulse;
                    let player_color = if b.owner_id != 0 {
                        player_colors
                            .get(b.owner_id as usize)
                            .copied()
                            .unwrap_or(egui::Color32::from_rgb(0, 220, 255))
                    } else {
                        egui::Color32::from_rgb(0, 220, 255)
                    };

                    // 1. Draw dynamic range scanning forcefield
                    let alpha = (90.0 * (1.0 - (zoom_scaled / 12.0).clamp(0.0, 0.7))).round() as u8;
                    let stroke_color = egui::Color32::from_rgba_unmultiplied(
                        player_color.r(),
                        player_color.g(),
                        player_color.b(),
                        alpha,
                    );
                    let fill_color = egui::Color32::from_rgba_unmultiplied(
                        player_color.r(),
                        player_color.g(),
                        player_color.b(),
                        alpha / 4,
                    );
                    painter.circle_stroke(
                        center,
                        s_radius,
                        egui::Stroke::new(1.5_f32, stroke_color),
                    );
                    painter.circle_filled(center, s_radius, fill_color);

                    // 2. Draw solid glowing hex outline around the Bunker itself for visual confirmation
                    let hex_r = (0.577_350_26_f32 * input.camera_zoom) / sf;
                    let points: Vec<egui::Pos2> = (0..6)
                        .map(|i| {
                            let angle = (i as f32 * 60.0 + 30.0).to_radians();
                            egui::pos2(
                                center.x + hex_r * angle.cos(),
                                center.y + hex_r * angle.sin(),
                            )
                        })
                        .collect();
                    painter.add(egui::Shape::convex_polygon(
                        points,
                        egui::Color32::from_rgba_unmultiplied(player_color.r(), player_color.g(), player_color.b(), 35),
                        egui::Stroke::new(2.0_f32, player_color),
                    ));
                }
            }

            if b.under_construction && b.ticks_until_complete > 0 {
                let total_ticks = b.kind.construction_duration_ticks();
                if total_ticks > 0 {
                    let progress =
                        1.0 - (b.ticks_until_complete as f32 / total_ticks as f32).clamp(0.0, 1.0);

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
                        painter.add(egui::Shape::line(
                            arc_points,
                            egui::Stroke::new(2.5_f32, egui::Color32::from_rgb(0, 220, 255)),
                        ));
                    }
                }
            }

            // Level badge (no white plate background, no frame, larger text in black)
            if b.level != 1 && zoom_scaled >= 0.6 {
                let text_val = get_level_str(b.level as u8);
                let font_size = (zoom_scaled * 0.65 * final_scale).clamp(8.0, 18.0).round();
                let bg_center =
                    egui::pos2(center.x + base_size * 0.45, center.y - base_size * 0.45);

                let font_id = egui::FontId::proportional(font_size);
                let galley = painter.layout_no_wrap(
                    text_val.to_owned(),
                    font_id.clone(),
                    egui::Color32::WHITE,
                );
                let pos = bg_center - galley.rect.size() / 2.0;

                crate::hud::nameplate::paint_glow_text(
                    &painter,
                    pos,
                    text_val,
                    font_id,
                    egui::Color32::WHITE,
                    galley.rect.size(),
                    false,
                );
            }

            // Render Bunker floating stats tooltip on hover
            if b.kind == sow_core::game::BuildingKind::Bunker
                && !b.under_construction
                && b.count == 1
            {
                let is_hovered = if let Some(snap_b) =
                    snap.buildings.iter().find(|sb| sb.id == b.id.unwrap_or(0))
                {
                    hovered_tile_idx == Some(snap_b.tile_idx)
                } else {
                    false
                };

                if is_hovered {
                    let penalty_prio = b.level * 4;
                    let extra_loss = b.level * 40;
                    let title = format!("🛡️ Defense Bunker (Lvl {})", b.level);
                    let stat1 = "Coverage: 8 Hex Radius";
                    let stat2 = format!("Atk Delay Penalty: +{}", penalty_prio);
                    let stat3 = format!("Atk Loss Penalty: +{}%", extra_loss);

                    let tooltip_text = format!("{}\n{}\n{}\n{}", title, stat1, stat2, stat3);

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
}
