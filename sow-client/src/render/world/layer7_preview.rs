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
    let painter = ctx.painter;
    let sf = ctx.sf;
    let zoom_scaled = ctx.zoom_scaled;
    let player_colors = ctx.player_colors;
    let terrain = ctx.terrain;

    let building_scale = painter.ctx().data(|d| {
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
        // --- Layer 7: Building Placement Preview (Ghost structures) ---
        if let Some(kind) = ui.app.hud_state.selected_building_kind {
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

            let col = rq as i32 + (rr as i32 - (rr as i32 & 1)) / 2;
            let row = rr as i32;

            if col >= 0 && row >= 0 && col < sim.map_w as i32 && row < sim.map_h as i32 {
                let map_w = sim.map_w;
                let map_h = sim.map_h;
                let owners = gfx
                    .map_renderer
                    .as_ref()
                    .map(|mr| mr.owners.as_slice())
                    .unwrap_or(&[]);
                let my_id = sim.my_player_id.unwrap_or(0);
                let buildings = sim
                    .current_snapshot
                    .as_ref()
                    .map(|s| s.buildings.as_slice())
                    .unwrap_or(&[]);

                // Check if there is a valid upgrade target within Manhattan distance 8 of (col, row)
                let mut upgrade_building = None;
                if kind.upgradable() {
                    let min_dist = 8;
                    let mut best_dist = 999;
                    for b in buildings {
                        if b.owner_id == my_id && b.kind == kind && !b.under_construction {
                            let bx = (b.tile_idx % map_w) as i32;
                            let by = (b.tile_idx / map_w) as i32;
                            let d = (col - bx).abs() + (row - by).abs();
                            if d <= min_dist
                                && (d < best_dist
                                    || (d == best_dist
                                        && upgrade_building.is_none_or(
                                            |old_b: &sow_core::protocol::BuildingSnapshot| {
                                                b.id < old_b.id
                                            },
                                        )))
                            {
                                best_dist = d;
                                upgrade_building = Some(b);
                            }
                        }
                    }
                }

                let snapped_idx = if let Some(b) = upgrade_building {
                    Some(b.tile_idx)
                } else {
                    crate::input::resolve_building_placement_tile(
                        kind, col, row, map_w, map_h, owners, terrain, my_id, buildings,
                    )
                    .ok()
                };

                // --- Render Radius Signals (No-build zones) ---
                for b in buildings {
                    let (b_cx, b_cy) = (b.tile_idx % map_w, b.tile_idx / map_w);
                    let world_bcx = b_cx as f32 + 0.5 + (b_cy % 2) as f32 * 0.5;
                    let world_bcy = (b_cy as f32 + 0.5) * 0.8660254_f32;
                    let s_bcx = (input.camera_x + world_bcx * input.camera_zoom) / sf;
                    let s_bcy = (input.camera_y + world_bcy * input.camera_zoom) / sf;
                    let s_pos = egui::pos2(s_bcx, s_bcy);

                    let radius_tiles = if kind == sow_core::game::BuildingKind::City {
                        if b.kind == sow_core::game::BuildingKind::City {
                            Some(12.0_f32)
                        } else {
                            None
                        }
                    } else if kind == sow_core::game::BuildingKind::Bunker {
                        if b.kind == sow_core::game::BuildingKind::City {
                            Some(6.0_f32)
                        } else if b.kind == sow_core::game::BuildingKind::Bunker {
                            Some(4.0_f32)
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if let Some(r_val) = radius_tiles {
                        let s_radius = r_val * input.camera_zoom / sf;
                        if s_bcx + s_radius >= 0.0
                            && s_bcx - s_radius <= input.screen_w / sf
                            && s_bcy + s_radius >= 0.0
                            && s_bcy - s_radius <= input.screen_h / sf
                        {
                            painter.circle_filled(
                                s_pos,
                                s_radius,
                                egui::Color32::from_rgba_unmultiplied(239, 68, 68, 90),
                            );

                            painter.circle_stroke(
                                s_pos,
                                s_radius,
                                egui::Stroke::new(
                                    1.2_f32,
                                    egui::Color32::from_rgba_unmultiplied(239, 68, 68, 120),
                                ),
                            );
                        }
                    }
                }

                // --- Render Organizational Hex Grid around hover target ---
                let hovered_idx = (row * map_w as i32 + col) as u32;
                let grid_radius = 12;
                for dy in -grid_radius..=grid_radius {
                    for dx in -grid_radius..=grid_radius {
                        let tx = col + dx;
                        let ty = row + dy;
                        if tx < 0 || tx >= map_w as i32 || ty < 0 || ty >= map_h as i32 {
                            continue;
                        }

                        // Calculate standard hex distance to make the rendered grid circular
                        let q1 = col - (row - (row & 1)) / 2;
                        let r1 = row;
                        let q2 = tx - (ty - (ty & 1)) / 2;
                        let r2 = ty;
                        let dq = q2 - q1;
                        let dr = r2 - r1;
                        let dist = (dq.abs() + dr.abs() + (dq + dr).abs()) / 2;
                        if dist > grid_radius {
                            continue;
                        }
                        let tile_idx = (ty * map_w as i32 + tx) as u32;
                        let tile_owner = owners.get(tile_idx as usize).copied().unwrap_or(0);
                        let tile_terrain = terrain.get(tile_idx as usize).copied().unwrap_or(0);
                        let is_land = (tile_terrain & 0x80) != 0;
                        if !is_land {
                            continue;
                        }

                        let mut can_place_tile = false;
                        if tile_owner == my_id && is_land {
                            let mut too_close = false;
                            if kind == sow_core::game::BuildingKind::City {
                                for b in buildings {
                                    if b.kind == sow_core::game::BuildingKind::City {
                                        let bx = (b.tile_idx % map_w) as i32;
                                        let by = (b.tile_idx / map_w) as i32;
                                        let bdx = tx - bx;
                                        let bdy = ty - by;
                                        if (bdx * bdx + bdy * bdy) < 144 {
                                            too_close = true;
                                            break;
                                        }
                                    }
                                }
                            } else if kind == sow_core::game::BuildingKind::Bunker {
                                for b in buildings {
                                    if b.kind == sow_core::game::BuildingKind::City {
                                        let bx = (b.tile_idx % map_w) as i32;
                                        let by = (b.tile_idx / map_w) as i32;
                                        let bdx = tx - bx;
                                        let bdy = ty - by;
                                        if (bdx * bdx + bdy * bdy) < 36 {
                                            too_close = true;
                                            break;
                                        }
                                    } else if b.kind == sow_core::game::BuildingKind::Bunker {
                                        let bx = (b.tile_idx % map_w) as i32;
                                        let by = (b.tile_idx / map_w) as i32;
                                        let bdx = tx - bx;
                                        let bdy = ty - by;
                                        if (bdx * bdx + bdy * bdy) < 16 {
                                            too_close = true;
                                            break;
                                        }
                                    }
                                }
                            }
                            if !too_close {
                                can_place_tile = true;
                            }
                        }

                        let fill_color = if can_place_tile {
                            egui::Color32::from_rgba_unmultiplied(34, 211, 238, 35)
                        } else {
                            egui::Color32::from_rgba_unmultiplied(239, 68, 68, 30)
                        };

                        // Color depending on placement validity / hover
                        let is_mine = tile_owner == my_id;
                        let is_hovered = tx == col && ty == row;
                        let border_color = if is_hovered {
                            if snapped_idx == Some(hovered_idx) {
                                egui::Color32::from_rgb(34, 211, 238) // cyan = building goes here
                            } else {
                                egui::Color32::from_rgb(239, 68, 68) // red = can't build here
                            }
                        } else if is_mine {
                            egui::Color32::from_rgba_unmultiplied(74, 222, 128, 100)
                        } else {
                            egui::Color32::from_rgba_unmultiplied(156, 163, 175, 40)
                        };

                        let thickness = if is_hovered { 2.5_f32 } else { 1.2_f32 };

                        // Draw hex cell outline
                        let world_cx = tx as f32 + 0.5 + (ty % 2) as f32 * 0.5;
                        let world_cy = (ty as f32 + 0.5) * 0.8660254_f32;
                        let screen_cx = (input.camera_x + world_cx * input.camera_zoom) / sf;
                        let screen_cy = (input.camera_y + world_cy * input.camera_zoom) / sf;
                        let screen_r = (0.577_350_26_f32 * input.camera_zoom) / sf;

                        if screen_cx + screen_r >= 0.0
                            && screen_cx - screen_r <= input.screen_w / sf
                            && screen_cy + screen_r >= 0.0
                            && screen_cy - screen_r <= input.screen_h / sf
                        {
                            let points: Vec<egui::Pos2> = (0..6)
                                .map(|i| {
                                    let angle = (i as f32 * 60.0 + 30.0).to_radians();
                                    egui::pos2(
                                        screen_cx + screen_r * angle.cos(),
                                        screen_cy + screen_r * angle.sin(),
                                    )
                                })
                                .collect();

                            painter.add(egui::Shape::convex_polygon(
                                points,
                                fill_color,
                                egui::Stroke::new(thickness, border_color),
                            ));
                        }
                    }
                }

                let can_afford = {
                    let i = sow_core::game::BuildingKind::ALL
                        .iter()
                        .position(|&k| k == kind)
                        .unwrap_or(0);
                    ui.app.hud_state.gold >= ui.app.hud_state.building_costs[i]
                };

                let (draw_col, draw_row, is_valid) = if let Some(idx) = snapped_idx {
                    ((idx % map_w) as i32, (idx / map_w) as i32, can_afford)
                } else {
                    (col, row, false)
                };

                let world_cx = draw_col as f32 + 0.5 + (draw_row % 2) as f32 * 0.5;
                let world_cy = (draw_row as f32 + 0.5) * 0.8660254_f32;
                let tile_screen_x = (input.camera_x + world_cx * input.camera_zoom) / sf;
                let tile_screen_y = (input.camera_y + world_cy * input.camera_zoom) / sf;

                let fill_color = if is_valid {
                    egui::Color32::from_rgba_unmultiplied(74, 222, 128, 140)
                } else {
                    egui::Color32::from_rgba_unmultiplied(239, 68, 68, 140)
                };
                let stroke_color = if is_valid {
                    egui::Color32::from_rgb(74, 222, 128)
                } else {
                    egui::Color32::from_rgb(239, 68, 68)
                };

                let tile_size = input.camera_zoom / sf;
                let screen_r = 0.577_350_26_f32 * tile_size;
                let points: Vec<egui::Pos2> = (0..6)
                    .map(|i| {
                        let angle = (i as f32 * 60.0 + 30.0).to_radians();
                        egui::pos2(
                            tile_screen_x + screen_r * angle.cos(),
                            tile_screen_y + screen_r * angle.sin(),
                        )
                    })
                    .collect();
                painter.add(egui::Shape::convex_polygon(
                    points,
                    fill_color,
                    egui::Stroke::new(3.0_f32, stroke_color),
                ));

                // Draw active range circle indicator for Bunker preview
                if kind == sow_core::game::BuildingKind::Bunker {
                    let radius_world = 8.0_f32; // config::DEFENSE_POST_RANGE
                    let elapsed = time.start_time.elapsed().as_secs_f32();
                    let pulse = (elapsed * 2.5).sin() * 0.04 + 0.96; // beautiful rapid scan pulse
                    let s_radius = radius_world * input.camera_zoom / sf * pulse;
                    let player_color = if my_id != 0 {
                        player_colors
                            .get(my_id as usize)
                            .copied()
                            .unwrap_or(egui::Color32::from_rgb(0, 220, 255))
                    } else {
                        egui::Color32::from_rgb(0, 220, 255)
                    };
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
                        egui::pos2(tile_screen_x, tile_screen_y),
                        s_radius,
                        egui::Stroke::new(1.5_f32, stroke_color),
                    );
                    painter.circle_filled(
                        egui::pos2(tile_screen_x, tile_screen_y),
                        s_radius,
                        fill_color,
                    );
                }

                // Draw ghost SVG
                {
                    let uri = kind.asset().uri();
                    let base_size = get_building_icon_size(tile_size) * final_scale;
                    let size_hint = egui::load::SizeHint::Size {
                        width: 64,
                        height: 64,
                        maintain_aspect_ratio: true,
                    };
                    if let Ok(egui::load::TexturePoll::Ready { texture }) = painter
                        .ctx()
                        .try_load_texture(uri, egui::TextureOptions::LINEAR, size_hint)
                    {
                        let (r_t, g_t, b_t) = if is_valid {
                            (34, 211, 238)
                        } else {
                            (239, 68, 68)
                        };
                        let tint = egui::Color32::from_rgba_unmultiplied(
                            ((r_t as f32 * 0.20) + (255.0 * 0.80)) as u8,
                            ((g_t as f32 * 0.20) + (255.0 * 0.80)) as u8,
                            ((b_t as f32 * 0.20) + (255.0 * 0.80)) as u8,
                            166, // 0.65 opacity
                        );
                        painter.image(
                            texture.id,
                            egui::Rect::from_center_size(
                                egui::pos2(tile_screen_x, tile_screen_y),
                                egui::vec2(base_size, base_size),
                            ),
                            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            tint,
                        );
                    }
                }

                // Render Upgrade Tooltip if upgrading
                if let Some(b) = upgrade_building {
                    let next_lvl = b.level + 1;
                    let text = format!("Upgrade Lvl {}", next_lvl);
                    let font_size = (8.5_f32 * input.camera_zoom / sf).clamp(8.0, 12.0).round();
                    let font_id = egui::FontId::proportional(font_size);
                    let galley =
                        painter.layout_no_wrap(text.clone(), font_id.clone(), egui::Color32::WHITE);

                    let padding_x = 6.0_f32;
                    let padding_y = 3.0_f32;
                    let rect_w = galley.rect.width() + padding_x * 2.0;
                    let rect_h = galley.rect.height() + padding_y * 2.0;

                    let tooltip_y = tile_screen_y - tile_size * 0.65;
                    let tooltip_rect = egui::Rect::from_center_size(
                        egui::pos2(tile_screen_x, tooltip_y),
                        egui::vec2(rect_w, rect_h),
                    );

                    // Premium slate blue dark glassmorphic container with cyan outline
                    painter.rect(
                        tooltip_rect,
                        4.0_f32,
                        egui::Color32::from_rgba_unmultiplied(15, 23, 42, 220),
                        egui::Stroke::new(
                            1.0_f32,
                            egui::Color32::from_rgba_unmultiplied(34, 211, 238, 180),
                        ),
                        egui::StrokeKind::Inside,
                    );

                    // Glowing cyan text centered
                    painter.text(
                        egui::pos2(tile_screen_x, tooltip_y),
                        egui::Align2::CENTER_CENTER,
                        &text,
                        font_id,
                        egui::Color32::from_rgb(34, 211, 238),
                    );
                }
            }
        }
    }
}
