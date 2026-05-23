use super::*;

use crate::render::world::utils::*;
#[allow(unused_variables)]
pub(crate) fn render(ui: &mut crate::app::UiState, sim: &crate::app::SimState, input: &crate::app::InputState, time: &crate::app::TimeState, gfx: &crate::app::GraphicsState, ctx: &RenderContext) {
    let painter = ctx.painter;
    let sf = ctx.sf;
    let zoom_scaled = ctx.zoom_scaled;
    let player_colors = ctx.player_colors;
    let terrain = ctx.terrain;

    let is_water = |tile_idx: u32| {
        let t = terrain.get(tile_idx as usize).copied().unwrap_or(0);
        (t & 0x80) == 0
    };

    if let Some(snap) = &sim.current_snapshot {
            // --- Layer 7: Building Placement Preview (Ghost structures) ---
            if let Some(kind) = ui.app.hud_state.selected_building_kind {
                let mx = input.last_mouse_x as f32;
                let my = input.last_mouse_y as f32;

                let world_x = (mx - input.camera_x) / input.camera_zoom;
                let world_y = (my - input.camera_y) / input.camera_zoom;

                let col = world_x.floor() as i32;
                let row = world_y.floor() as i32;

                if col >= 0 && row >= 0 && col < sim.map_w as i32 && row < sim.map_h as i32 {
                    let map_w = sim.map_w;
                    let map_h = sim.map_h;
                    let owners = gfx.map_renderer.as_ref().map(|mr| mr.owners.as_slice()).unwrap_or(&[]);
                    let my_id = sim.my_player_id.unwrap_or(0);
                    let buildings = sim.current_snapshot.as_ref().map(|s| s.buildings.as_slice()).unwrap_or(&[]);

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
                                if d <= min_dist {
                                    if d < best_dist || (d == best_dist && upgrade_building.map_or(true, |old_b: &sow_core::protocol::BuildingSnapshot| b.id < old_b.id)) {
                                        best_dist = d;
                                        upgrade_building = Some(b);
                                    }
                                }
                            }
                        }
                    }

                    let snapped_idx = if let Some(b) = upgrade_building {
                        Some(b.tile_idx)
                    } else {
                        crate::input::resolve_building_placement_tile(
                            kind,
                            col,
                            row,
                            map_w,
                            map_h,
                            owners,
                            terrain,
                            my_id,
                            buildings,
                        ).ok()
                    };

                    let is_station_kind = (kind == sow_core::game::BuildingKind::City
                        || kind == sow_core::game::BuildingKind::Factory
                        || kind == sow_core::game::BuildingKind::Port)
                        && upgrade_building.is_none();

                    if let Some(start_idx) = snapped_idx {
                        // S1: Only recompute rail preview paths when snapped tile changes
                        if is_station_kind && ui.last_preview_tile != Some(start_idx) {
                            ui.last_preview_tile = Some(start_idx);
                            ui.cached_preview_paths.clear();

                            let is_friendly = |other_id: u16| -> bool {
                                if other_id == my_id || other_id == 0 {
                                    return true;
                                }
                                if let Some(snapshot) = &sim.current_snapshot {
                                    if let Some(my_p) = snapshot.players.iter().find(|p| p.id == my_id) {
                                        if my_p.alliances.contains(&other_id) {
                                            return true;
                                        }
                                    }
                                    if let Some(other_p) = snapshot.players.iter().find(|p| p.id == other_id) {
                                        if other_p.alliances.contains(&my_id) {
                                            return true;
                                        }
                                    }
                                }
                                false
                            };

                            let eligible_stations: Vec<_> = buildings.iter()
                                .filter(|b| !b.under_construction && is_friendly(b.owner_id) && (b.kind == sow_core::game::BuildingKind::City || b.kind == sow_core::game::BuildingKind::Factory || b.kind == sow_core::game::BuildingKind::Port))
                                .collect();

                            let temp_map = sow_core::map::GameMap {
                                width: map_w,
                                height: map_h,
                                terrain: terrain.iter().map(|&b| sow_core::map::MapTile::from_byte(b)).collect(),
                                state: vec![0; (map_w * map_h) as usize],
                                dirty_tiles: Vec::new(),
                            };

                            let mut adj: std::collections::HashMap<u32, Vec<u32>> = std::collections::HashMap::new();
                            if let Some(snapshot) = &sim.current_snapshot {
                                for rail in &snapshot.railroads {
                                    if is_friendly(rail.owner_id) {
                                        if let (Some(&s_node), Some(&e_node)) = (rail.path.first(), rail.path.last()) {
                                            adj.entry(s_node).or_default().push(e_node);
                                            adj.entry(e_node).or_default().push(s_node);
                                        }
                                    }
                                }
                            }

                            let distance_from = |start_tile: u32, dest_tile: u32, max_dist: usize| -> Option<usize> {
                                if start_tile == dest_tile {
                                    return Some(0);
                                }
                                let mut visited = std::collections::HashSet::new();
                                let mut queue = std::collections::VecDeque::new();
                                visited.insert(start_tile);
                                queue.push_back((start_tile, 0));

                                while let Some((curr, dist)) = queue.pop_front() {
                                    if curr == dest_tile {
                                        return Some(dist);
                                    }
                                    if dist >= max_dist {
                                        continue;
                                    }
                                    if let Some(neighbors) = adj.get(&curr) {
                                        for &neighbor in neighbors {
                                            if visited.insert(neighbor) {
                                                queue.push_back((neighbor, dist + 1));
                                            }
                                        }
                                    }
                                }
                                None
                            };

                            let mut candidates = Vec::new();
                            for station in &eligible_stations {
                                let x1 = (start_idx % map_w) as i32;
                                let y1 = (start_idx / map_w) as i32;
                                let x2 = (station.tile_idx % map_w) as i32;
                                let y2 = (station.tile_idx / map_w) as i32;
                                let dx = x1 - x2;
                                let dy = y1 - y2;
                                let dist_sq = dx * dx + dy * dy;
                                if dist_sq >= 225 && dist_sq <= 10000 {
                                    candidates.push((dist_sq, station));
                                }
                            }
                            candidates.sort_by_key(|c| c.0);

                            let mut paths_found = 0;
                            let mut connected_stations = Vec::new();

                            for &(_, station) in &candidates {
                                if paths_found >= 5 {
                                    break;
                                }

                                let already_reachable = connected_stations.iter().any(|&s| {
                                    distance_from(station.tile_idx, s, 3).is_some()
                                });
                                if already_reachable {
                                    continue;
                                }

                                if let Some(path) = sow_core::building::railroad::find_rail_path(&temp_map, start_idx, station.tile_idx) {
                                    if path.len() <= 480 {
                                        paths_found += 1;
                                        connected_stations.push(station.tile_idx);
                                        ui.cached_preview_paths.push(path);
                                    }
                                }
                            }
                        } else if !is_station_kind {
                            // Not a station kind — clear preview cache
                            ui.last_preview_tile = None;
                            ui.cached_preview_paths.clear();
                        }

                        // Render cached preview paths
                        for path in &ui.cached_preview_paths {
                            let rail_tiles = compute_rail_tiles(map_w, path);
                            for rt in &rail_tiles {
                                let tile_idx = rt.tile_idx;
                                let c = (tile_idx % map_w) as f32;
                                let r = (tile_idx / map_w) as f32;

                                if is_water(tile_idx) {
                                    let bridge_rects = get_bridge_rects(rt.rail_type);
                                    let bridge_color = egui::Color32::from_rgba_unmultiplied(197, 69, 72, 102);
                                    for &[dx, dy, w, h] in bridge_rects {
                                        let world_x = c + 0.5 + (dx as f32) / 2.0;
                                        let world_y = r + 0.5 + (dy as f32) / 2.0;
                                        let world_w = w as f32 / 2.0;
                                        let world_h = h as f32 / 2.0;

                                        let screen_x = (input.camera_x + world_x * input.camera_zoom) / sf;
                                        let screen_y = (input.camera_y + world_y * input.camera_zoom) / sf;
                                        let screen_w = world_w * input.camera_zoom / sf;
                                        let screen_h = world_h * input.camera_zoom / sf;

                                        painter.rect_filled(
                                            egui::Rect::from_min_size(egui::pos2(screen_x, screen_y), egui::vec2(screen_w, screen_h)),
                                            0.0,
                                            bridge_color,
                                        );
                                    }
                                }

                                let rail_rects = get_railroad_rects(rt.rail_type);
                                let track_color = egui::Color32::from_rgba_unmultiplied(0, 0, 0, 102);
                                for &[dx, dy, w, h] in rail_rects {
                                    let world_x = c + 0.5 + (dx as f32) / 2.0;
                                    let world_y = r + 0.5 + (dy as f32) / 2.0;
                                    let world_w = w as f32 / 2.0;
                                    let world_h = h as f32 / 2.0;

                                    let screen_x = (input.camera_x + world_x * input.camera_zoom) / sf;
                                    let screen_y = (input.camera_y + world_y * input.camera_zoom) / sf;
                                    let screen_w = world_w * input.camera_zoom / sf;
                                    let screen_h = world_h * input.camera_zoom / sf;

                                    painter.rect_filled(
                                        egui::Rect::from_min_size(egui::pos2(screen_x, screen_y), egui::vec2(screen_w, screen_h)),
                                        0.0,
                                        track_color,
                                    );
                                }
                            }
                        }
                    } else {
                        // No valid snap — clear cache
                        ui.last_preview_tile = None;
                        ui.cached_preview_paths.clear();
                    }

                    let can_afford = {
                        let i = sow_core::game::BuildingKind::ALL.iter().position(|&k| k == kind).unwrap_or(0);
                        ui.app.hud_state.gold >= ui.app.hud_state.building_costs[i]
                    };

                    let (draw_col, draw_row, is_valid) = if let Some(idx) = snapped_idx {
                        ((idx % map_w) as i32, (idx / map_w) as i32, can_afford)
                    } else {
                        (col, row, false)
                    };

                    let tile_screen_x = (input.camera_x + (draw_col as f32 + 0.5) * input.camera_zoom) / sf;
                    let tile_screen_y = (input.camera_y + (draw_row as f32 + 0.5) * input.camera_zoom) / sf;

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
                        egui::vec2(tile_size, tile_size)
                    );
                    painter.rect(tile_rect, 0.0, fill_color, egui::Stroke::new(1.0_f32, stroke_color), egui::StrokeKind::Inside);

                    // Draw ghost SVG
                    {
                        let uri = kind.asset().uri();
                        let base_size = get_building_icon_size(tile_size);
                        let size_hint = egui::load::SizeHint::Size { width: 64, height: 64, maintain_aspect_ratio: true };
                        if let Ok(egui::load::TexturePoll::Ready { texture }) = painter.ctx().try_load_texture(uri, egui::TextureOptions::LINEAR, size_hint) {
                            let tint = if kind.asset().is_svg() {
                                egui::Color32::BLACK
                            } else {
                                egui::Color32::from_white_alpha(180)
                            };
                            painter.image(
                                texture.id,
                                egui::Rect::from_center_size(egui::pos2(tile_screen_x, tile_screen_y), egui::vec2(base_size, base_size)),
                                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                                tint,
                            );
                        }
                    }
                }
            } else {
                // Placement mode exited — clear cache
                if ui.last_preview_tile.is_some() {
                    ui.last_preview_tile = None;
                    ui.cached_preview_paths.clear();
                }
            }
    }
}
