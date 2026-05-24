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
                                egui::Color32::from_rgba_unmultiplied(239, 68, 68, 12),
                            );
                            painter.circle_stroke(
                                s_pos,
                                s_radius,
                                egui::Stroke::new(
                                    1.2_f32,
                                    egui::Color32::from_rgba_unmultiplied(239, 68, 68, 150),
                                ),
                            );
                        }
                    }
                }

                // --- Render Organizational Hex Grid around hover target ---
                let grid_radius = 12;
                for dy in -grid_radius..=grid_radius {
                    for dx in -grid_radius..=grid_radius {
                        let tx = col + dx;
                        let ty = row + dy;
                        if tx < 0 || tx >= map_w as i32 || ty < 0 || ty >= map_h as i32 {
                            continue;
                        }
                        let tile_idx = (ty * map_w as i32 + tx) as u32;
                        let tile_owner = owners.get(tile_idx as usize).copied().unwrap_or(0);
                        let tile_terrain = terrain.get(tile_idx as usize).copied().unwrap_or(0);
                        let is_land = (tile_terrain & 0x80) != 0;
                        if !is_land {
                            continue;
                        }

                        // Color depending on ownership / hover
                        let border_color = if tile_owner == my_id {
                            if tx == col && ty == row {
                                egui::Color32::from_rgb(34, 211, 238) // cyan highlight for hovered tile
                            } else {
                                egui::Color32::from_rgba_unmultiplied(74, 222, 128, 60)
                                // soft green for owned territory
                            }
                        } else {
                            egui::Color32::from_rgba_unmultiplied(156, 163, 175, 20)
                            // very faint gray for others
                        };

                        let thickness = if tx == col && ty == row {
                            1.5_f32
                        } else {
                            0.8_f32
                        };

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
                                egui::Color32::TRANSPARENT,
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
                    egui::Color32::from_rgba_unmultiplied(74, 222, 128, 80)
                } else {
                    egui::Color32::from_rgba_unmultiplied(239, 68, 68, 80)
                };
                let stroke_color = if is_valid {
                    egui::Color32::from_rgb(74, 222, 128)
                } else {
                    egui::Color32::from_rgb(239, 68, 68)
                };

                let tile_size = input.camera_zoom / sf;
                let tile_rect = egui::Rect::from_center_size(
                    egui::pos2(tile_screen_x, tile_screen_y),
                    egui::vec2(tile_size, tile_size),
                );
                painter.rect(
                    tile_rect,
                    0.0,
                    fill_color,
                    egui::Stroke::new(1.0_f32, stroke_color),
                    egui::StrokeKind::Inside,
                );

                // Draw ghost SVG
                {
                    let uri = kind.asset().uri();
                    let base_size = get_building_icon_size(tile_size);
                    let size_hint = egui::load::SizeHint::Size {
                        width: 64,
                        height: 64,
                        maintain_aspect_ratio: true,
                    };
                    if let Ok(egui::load::TexturePoll::Ready { texture }) = painter
                        .ctx()
                        .try_load_texture(uri, egui::TextureOptions::LINEAR, size_hint)
                    {
                        let tint = egui::Color32::from_white_alpha(180);
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
            }
        }
    }
}
