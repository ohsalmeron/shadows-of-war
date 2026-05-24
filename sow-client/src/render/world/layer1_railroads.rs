use super::*;

use crate::render::world::utils::*;

#[derive(Clone)]
pub struct VisualTrain {
    pub id: u32,
    pub rail_id: u64,
    pub progress: f32,
    pub speed: f32,
    pub direction: f32,
}

#[derive(Clone)]
pub struct StationAnimation {
    pub tile_idx: u32,
    pub start_time: f32,
    pub duration: f32,
}

fn pseudo_rand(seed: u32) -> u32 {
    seed.wrapping_mul(1664525).wrapping_add(1013904223)
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
        for rail in snap.railroads.iter() {
            let owner_color = player_colors
                .get(rail.owner_id as usize)
                .copied()
                .unwrap_or(egui::Color32::GRAY);

            let (cached_path, cached_tiles) = ui
                .cached_railroads
                .entry(rail.id)
                .or_insert_with(|| (rail.path.clone(), compute_rail_tiles(sim.map_w, &rail.path)));
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
                let r_world_x = col + 0.5 + (row as i32 % 2) as f32 * 0.5;
                let r_world_y = (row + 0.5) * 0.8660254_f32;
                let scr_x = (input.camera_x + r_world_x * input.camera_zoom) / sf;
                let scr_y = (input.camera_y + r_world_y * input.camera_zoom) / sf;
                if scr_x < -zoom_scaled
                    || scr_x > input.screen_w / sf + zoom_scaled
                    || scr_y < -zoom_scaled
                    || scr_y > input.screen_h / sf + zoom_scaled
                {
                    continue;
                }

                if is_water(tile_idx) {
                    let bridge_rects = get_bridge_rects(rt.rail_type);
                    let bridge_color = egui::Color32::from_rgb(197, 69, 72); // rusty red
                    for &[dx, dy, w, h] in bridge_rects {
                        let world_x = col + 0.5 + (row as i32 % 2) as f32 * 0.5 + (dx as f32) / 2.0;
                        let world_y = (row + 0.5) * 0.8660254_f32 + (dy as f32) / 2.0;
                        let world_w = w as f32 / 2.0;
                        let world_h = h as f32 / 2.0;

                        let screen_x = (input.camera_x + world_x * input.camera_zoom) / sf;
                        let screen_y = (input.camera_y + world_y * input.camera_zoom) / sf;
                        let screen_w = world_w * input.camera_zoom / sf;
                        let screen_h = world_h * input.camera_zoom / sf;

                        painter.rect_filled(
                            egui::Rect::from_min_size(
                                egui::pos2(screen_x, screen_y),
                                egui::vec2(screen_w, screen_h),
                            ),
                            0.0,
                            bridge_color,
                        );
                    }
                }

                let rail_rects = get_railroad_rects(rt.rail_type);
                for &[dx, dy, w, h] in rail_rects {
                    let world_x = col + 0.5 + (row as i32 % 2) as f32 * 0.5 + (dx as f32) / 2.0;
                    let world_y = (row + 0.5) * 0.8660254_f32 + (dy as f32) / 2.0;
                    let world_w = w as f32 / 2.0;
                    let world_h = h as f32 / 2.0;

                    let screen_x = (input.camera_x + world_x * input.camera_zoom) / sf;
                    let screen_y = (input.camera_y + world_y * input.camera_zoom) / sf;
                    let screen_w = world_w * input.camera_zoom / sf;
                    let screen_h = world_h * input.camera_zoom / sf;

                    painter.rect_filled(
                        egui::Rect::from_min_size(
                            egui::pos2(screen_x, screen_y),
                            egui::vec2(screen_w, screen_h),
                        ),
                        0.0,
                        owner_color,
                    );
                }
            }
        }

        // --- GPU-Style Visual Train Simulation ---
        let train_id = egui::Id::new("sow_railroad_trains_v3");
        let anim_id = egui::Id::new("sow_railroad_anims_v3");
        let time_id = egui::Id::new("sow_railroad_time_v3");

        let current_time = wall_secs as f32;
        let last_time: Option<f32> = painter.ctx().data(|d| d.get_temp(time_id));
        let dt = if let Some(last) = last_time {
            (current_time - last).max(0.0).min(0.1)
        } else {
            0.0
        };
        painter
            .ctx()
            .data_mut(|d| d.insert_temp(time_id, current_time));

        let mut trains: Vec<VisualTrain> = painter
            .ctx()
            .data(|d| d.get_temp(train_id))
            .unwrap_or_default();
        let mut anims: Vec<StationAnimation> = painter
            .ctx()
            .data(|d| d.get_temp(anim_id))
            .unwrap_or_default();

        // 1. Prune/validate existing trains
        trains.retain(|t| snap.railroads.iter().any(|r| r.id == t.rail_id));

        // 2. Spawn trains up to capacity
        let max_trains = 30.min(snap.railroads.len());
        if trains.len() < max_trains {
            let mut next_train_id = trains.iter().map(|t| t.id).max().unwrap_or(0) + 1;
            for r in snap.railroads.iter() {
                if trains.len() >= max_trains {
                    break;
                }
                if r.path.len() > 1 && !trains.iter().any(|t| t.rail_id == r.id) {
                    let mut seed = next_train_id;
                    seed = pseudo_rand(seed);
                    let progress_fract = (seed % 100) as f32 / 100.0;
                    seed = pseudo_rand(seed);
                    let direction = if seed % 2 == 0 { 1.0 } else { -1.0 };

                    trains.push(VisualTrain {
                        id: next_train_id,
                        rail_id: r.id,
                        progress: progress_fract * (r.path.len() - 1) as f32,
                        speed: 4.5,
                        direction,
                    });
                    next_train_id += 1;
                }
            }
        }

        // 3. Update trains progress and handle arrivals
        for train in &mut trains {
            if let Some(rail) = snap.railroads.iter().find(|r| r.id == train.rail_id) {
                if rail.path.len() > 1 {
                    let n_segments = (rail.path.len() - 1) as f32;
                    train.progress += train.direction * train.speed * dt;

                    let mut arrived = false;
                    let mut arrival_tile = 0;
                    if train.direction > 0.0 {
                        if train.progress >= n_segments {
                            arrived = true;
                            arrival_tile = rail.path[rail.path.len() - 1];
                        }
                    } else {
                        if train.progress <= 0.0 {
                            arrived = true;
                            arrival_tile = rail.path[0];
                        }
                    }

                    if arrived {
                        // Spawn steam rings at station
                        anims.push(StationAnimation {
                            tile_idx: arrival_tile,
                            start_time: current_time,
                            duration: 1.2,
                        });

                        // Choose next rail pseudo-randomly
                        let connected_rails: Vec<&sow_core::building::railroad::Railroad> = snap
                            .railroads
                            .iter()
                            .filter(|r| {
                                r.path.len() > 1
                                    && (r.path[0] == arrival_tile
                                        || r.path[r.path.len() - 1] == arrival_tile)
                            })
                            .collect();

                        if !connected_rails.is_empty() {
                            let mut seed = train.id.wrapping_add((current_time * 1000.0) as u32);
                            seed = pseudo_rand(seed);
                            let choice = (seed as usize) % connected_rails.len();
                            let next_rail = connected_rails[choice];
                            train.rail_id = next_rail.id;
                            if next_rail.path[0] == arrival_tile {
                                train.progress = 0.0;
                                train.direction = 1.0;
                            } else {
                                train.progress = (next_rail.path.len() - 1) as f32;
                                train.direction = -1.0;
                            }
                        } else {
                            // Dead end: reverse
                            train.direction = -train.direction;
                            train.progress = train.progress.clamp(0.0, n_segments);
                        }
                    }
                }
            }
        }

        // 4. Render active trains
        for train in &trains {
            if let Some(rail) = snap.railroads.iter().find(|r| r.id == train.rail_id) {
                if rail.path.len() > 1 {
                    let owner_color = player_colors
                        .get(rail.owner_id as usize)
                        .copied()
                        .unwrap_or(egui::Color32::GRAY);
                    let n_segments = (rail.path.len() - 1) as f32;
                    let carriage_offsets = [0.0_f32, 0.35_f32, 0.7_f32];

                    for (i, &offset) in carriage_offsets.iter().enumerate() {
                        let p = (train.progress - train.direction * offset).clamp(0.0, n_segments);
                        let segment_idx = p.floor() as usize;
                        let segment_fract = p.fract();

                        let idx = segment_idx.min(rail.path.len() - 2);
                        let t1 = rail.path[idx];
                        let t2 = rail.path[idx + 1];

                        let col1 = (t1 % sim.map_w) as f32;
                        let row1 = (t1 / sim.map_w) as f32;
                        let col2 = (t2 % sim.map_w) as f32;
                        let row2 = (t2 / sim.map_w) as f32;

                        let wx1 = col1 + 0.5 + (row1 as i32 % 2) as f32 * 0.5;
                        let wy1 = (row1 + 0.5) * 0.8660254_f32;
                        let wx2 = col2 + 0.5 + (row2 as i32 % 2) as f32 * 0.5;
                        let wy2 = (row2 + 0.5) * 0.8660254_f32;

                        let wx = wx1 + (wx2 - wx1) * segment_fract;
                        let wy = wy1 + (wy2 - wy1) * segment_fract;

                        let screen_x = (input.camera_x + wx * input.camera_zoom) / sf;
                        let screen_y = (input.camera_y + wy * input.camera_zoom) / sf;

                        if screen_x < -20.0
                            || screen_x > input.screen_w / sf + 20.0
                            || screen_y < -20.0
                            || screen_y > input.screen_h / sf + 20.0
                        {
                            continue;
                        }

                        let screen_pos = egui::pos2(screen_x, screen_y);
                        let is_engine = i == 0;
                        let radius = if is_engine {
                            zoom_scaled * 0.18
                        } else {
                            zoom_scaled * 0.13
                        };

                        let (fill_col, stroke_col) = if is_engine {
                            (egui::Color32::from_rgb(251, 191, 36), egui::Color32::BLACK)
                        } else {
                            (owner_color, egui::Color32::from_black_alpha(200))
                        };

                        painter.circle_filled(screen_pos, radius, fill_col);
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
        }

        // 5. Render active station animations (steam circles)
        anims.retain(|anim| current_time - anim.start_time < anim.duration);
        for anim in &anims {
            let elapsed = current_time - anim.start_time;
            let t = elapsed / anim.duration;

            let col = (anim.tile_idx % sim.map_w) as f32;
            let row = (anim.tile_idx / sim.map_w) as f32;

            let wx = col + 0.5 + (row as i32 % 2) as f32 * 0.5;
            let wy = (row + 0.5) * 0.8660254_f32;

            let screen_x = (input.camera_x + wx * input.camera_zoom) / sf;
            let screen_y = (input.camera_y + wy * input.camera_zoom) / sf;

            if screen_x < -40.0
                || screen_x > input.screen_w / sf + 40.0
                || screen_y < -40.0
                || screen_y > input.screen_h / sf + 40.0
            {
                continue;
            }

            let screen_pos = egui::pos2(screen_x, screen_y);

            // Draw 3 expanding concentric circles
            for c in 0..3 {
                let circle_t = (t + (c as f32 / 3.0)) % 1.0;
                let alpha = ((1.0 - circle_t) * 160.0) as u8;
                let radius = zoom_scaled * 0.7 * circle_t;
                painter.circle_stroke(
                    screen_pos,
                    radius,
                    egui::Stroke::new(1.5_f32, egui::Color32::from_white_alpha(alpha)),
                );
            }
        }

        // 6. Save states back to egui temporary storage
        painter.ctx().data_mut(|d| {
            d.insert_temp(train_id, trains);
            d.insert_temp(anim_id, anims);
        });

        // --- Sea Lanes: Dashed water paths between ports ---
        if zoom_scaled >= 0.3 {
            let lane_color = egui::Color32::from_rgba_unmultiplied(59, 130, 246, 100); // blue, translucent
            for lane in snap.sea_lanes.iter() {
                let cached_tiles = ui
                    .cached_sea_lanes
                    .entry(lane.id)
                    .or_insert_with(|| compute_rail_tiles(sim.map_w, &lane.path));

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
                    if scr_x < -zoom_scaled
                        || scr_x > input.screen_w / sf + zoom_scaled
                        || scr_y < -zoom_scaled
                        || scr_y > input.screen_h / sf + zoom_scaled
                    {
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
                            egui::Rect::from_min_size(
                                egui::pos2(screen_x, screen_y),
                                egui::vec2(screen_w, screen_h),
                            ),
                            0.0,
                            lane_color,
                        );
                    }
                }
            }
        }
    }
}
