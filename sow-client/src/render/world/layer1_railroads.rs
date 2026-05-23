use super::*;

use crate::render::world::utils::*;
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
            // --- Layer 1: Railroads & Bridges (Bottom-most) ---
            for rail in &snap.railroads {
                let owner_color = player_colors.get(rail.owner_id as usize).copied().unwrap_or(egui::Color32::GRAY);

                let (cached_path, cached_tiles) = ui.cached_railroads.entry(rail.id).or_insert_with(|| {
                    (rail.path.clone(), compute_rail_tiles(sim.map_w, &rail.path))
                });
                if cached_path != &rail.path {
                    *cached_path = rail.path.clone();
                    *cached_tiles = compute_rail_tiles(sim.map_w, &rail.path);
                }
                let rail_tiles = cached_tiles;

                for rt in rail_tiles {
                    let tile_idx = rt.tile_idx;
                    let col = (tile_idx % sim.map_w) as f32;
                    let row = (tile_idx / sim.map_w) as f32;

                    // S4: Frustum cull individual rail tiles
                    let scr_x = (input.camera_x + (col + 0.5) * input.camera_zoom) / sf;
                    let scr_y = (input.camera_y + (row + 0.5) * input.camera_zoom) / sf;
                    if scr_x < -zoom_scaled || scr_x > input.screen_w / sf + zoom_scaled
                        || scr_y < -zoom_scaled || scr_y > input.screen_h / sf + zoom_scaled {
                        continue;
                    }

                    if is_water(tile_idx) {
                        let bridge_rects = get_bridge_rects(rt.rail_type);
                        let bridge_color = egui::Color32::from_rgb(197, 69, 72); // rusty red
                        for &[dx, dy, w, h] in bridge_rects {
                            let world_x = col + 0.5 + (dx as f32) / 2.0;
                            let world_y = row + 0.5 + (dy as f32) / 2.0;
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
                    for &[dx, dy, w, h] in rail_rects {
                        let world_x = col + 0.5 + (dx as f32) / 2.0;
                        let world_y = row + 0.5 + (dy as f32) / 2.0;
                        let world_w = w as f32 / 2.0;
                        let world_h = h as f32 / 2.0;

                        let screen_x = (input.camera_x + world_x * input.camera_zoom) / sf;
                        let screen_y = (input.camera_y + world_y * input.camera_zoom) / sf;
                        let screen_w = world_w * input.camera_zoom / sf;
                        let screen_h = world_h * input.camera_zoom / sf;

                        painter.rect_filled(
                            egui::Rect::from_min_size(egui::pos2(screen_x, screen_y), egui::vec2(screen_w, screen_h)),
                            0.0,
                            owner_color,
                        );
                    }
                }

                // Render visual train carriages moving along completed railroads (100% client-side, zero overhead!)
                if rail.path.len() > 1 {
                    let elapsed = time.start_time.elapsed().as_secs_f32();
                    // Speed: 5.0 tiles per second
                    let speed = 5.0_f32;
                    let t_tiles = elapsed * speed;
                    
                    let n_segments = (rail.path.len() - 1) as f32;
                    
                    // We render an engine carriage and 2 trailing carriages for a beautiful chain visual!
                    let carriage_offsets = [0.0_f32, 0.35_f32, 0.7_f32];
                    
                    for (i, &offset) in carriage_offsets.iter().enumerate() {
                        let c_t = t_tiles - offset;
                        if c_t < 0.0 {
                            continue; // Carriage not yet spawned
                        }
                        
                        // Round trip: forward then backward
                        let progress = c_t % (2.0 * n_segments);
                        
                        let (segment_idx, segment_fract) = if progress < n_segments {
                            (progress.floor() as usize, progress.fract())
                        } else {
                            let back: f32 = 2.0 * n_segments - progress;
                            (back.floor() as usize, back.fract())
                        };
                        
                        let idx = segment_idx.min(rail.path.len() - 2);
                        let t1 = rail.path[idx];
                        let t2 = rail.path[idx + 1];
                        
                        let col1 = (t1 % sim.map_w) as f32;
                        let row1 = (t1 / sim.map_w) as f32;
                        let col2 = (t2 % sim.map_w) as f32;
                        let row2 = (t2 / sim.map_w) as f32;
                        
                        // Linear interpolation of world coordinates
                        let wx = col1 + (col2 - col1) * segment_fract + 0.5;
                        let wy = row1 + (row2 - row1) * segment_fract + 0.5;
                        
                        // Convert to screen position
                        let screen_x = (input.camera_x + wx * input.camera_zoom) / sf;
                        let screen_y = (input.camera_y + wy * input.camera_zoom) / sf;

                        // S4: Frustum cull train carriages
                        if screen_x < -20.0 || screen_x > input.screen_w / sf + 20.0
                            || screen_y < -20.0 || screen_y > input.screen_h / sf + 20.0 {
                            continue;
                        }

                        let screen_pos = egui::pos2(screen_x, screen_y);
                        
                        // Train Carriage Radius (scales with camera zoom!)
                        let is_engine = i == 0;
                        let radius = if is_engine {
                            zoom_scaled * 0.18
                        } else {
                            zoom_scaled * 0.13
                        };
                        
                        // Colors: Engine is gold/black, carriages are player/nation color
                        let (fill_col, stroke_col) = if is_engine {
                            (egui::Color32::from_rgb(251, 191, 36), egui::Color32::BLACK)
                        } else {
                            (owner_color, egui::Color32::from_black_alpha(200))
                        };
                        
                        painter.circle_filled(
                            screen_pos,
                            radius,
                            fill_col,
                        );
                        painter.circle_stroke(
                            screen_pos,
                            radius,
                            egui::Stroke::new(1.0_f32, stroke_col),
                        );

                        if is_engine {
                            let gold_str = get_train_gold_str(segment_idx.min(39));
                            let font_size = 9.0_f32;
                            let text_pos = egui::pos2(screen_pos.x, screen_pos.y - radius - 5.0);
                            
                            painter.text(
                                egui::pos2(text_pos.x + 1.0, text_pos.y + 1.0),
                                egui::Align2::CENTER_CENTER,
                                gold_str,
                                egui::FontId::proportional(font_size),
                                egui::Color32::from_black_alpha(220),
                            );
                            painter.text(
                                text_pos,
                                egui::Align2::CENTER_CENTER,
                                gold_str,
                                egui::FontId::proportional(font_size),
                                egui::Color32::from_rgb(251, 191, 36),
                            );
                        }
                    }
                }
            }

            // --- Sea Lanes: Dashed water paths between ports ---
            if zoom_scaled >= 0.3 {
                let lane_color = egui::Color32::from_rgba_unmultiplied(59, 130, 246, 100); // blue, translucent
                for lane in &snap.sea_lanes {
                    let cached_tiles = ui.cached_sea_lanes.entry(lane.id).or_insert_with(|| {
                        compute_rail_tiles(sim.map_w, &lane.path)
                    });

                    for (ti, rt) in cached_tiles.iter().enumerate() {
                        // Dashed pattern: skip every other tile
                        if ti % 2 == 1 {
                            continue;
                        }

                        let col = (rt.tile_idx % sim.map_w) as f32;
                        let row = (rt.tile_idx / sim.map_w) as f32;

                        // Frustum cull
                        let scr_x = (input.camera_x + (col + 0.5) * input.camera_zoom) / sf;
                        let scr_y = (input.camera_y + (row + 0.5) * input.camera_zoom) / sf;
                        if scr_x < -zoom_scaled || scr_x > input.screen_w / sf + zoom_scaled
                            || scr_y < -zoom_scaled || scr_y > input.screen_h / sf + zoom_scaled {
                            continue;
                        }

                        let rail_rects = get_railroad_rects(rt.rail_type);
                        for &[dx, dy, w, h] in rail_rects {
                            let world_x = col + 0.5 + (dx as f32) / 2.0;
                            let world_y = row + 0.5 + (dy as f32) / 2.0;
                            let world_w = w as f32 / 2.0;
                            let world_h = h as f32 / 2.0;

                            let screen_x = (input.camera_x + world_x * input.camera_zoom) / sf;
                            let screen_y = (input.camera_y + world_y * input.camera_zoom) / sf;
                            let screen_w = world_w * input.camera_zoom / sf;
                            let screen_h = world_h * input.camera_zoom / sf;

                            painter.rect_filled(
                                egui::Rect::from_min_size(egui::pos2(screen_x, screen_y), egui::vec2(screen_w, screen_h)),
                                0.0,
                                lane_color,
                            );
                        }
                    }
                }
            }

    }
}
