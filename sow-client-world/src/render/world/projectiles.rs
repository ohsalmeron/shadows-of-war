use super::*;
use crate::render::world::movers::{tile_to_world, world_to_tile};

const MIN_PREVIEW_STROKE_PX: f32 = 2.5;
const MIN_PREVIEW_ICON_PX: f32 = 14.0;
const MIN_PREVIEW_RING_PX: f32 = 10.0;

#[inline]
fn preview_px(world_size: f32, zoom: f32, sf: f32, min_px: f32) -> f32 {
    (world_size * zoom / sf).max(min_px / sf)
}

pub(crate) fn render(
    ui: &mut crate::app::UiState,
    sim: &crate::app::SimState,
    input: &crate::app::InputState,
    time: &crate::app::TimeState,
    _gfx: &crate::app::GraphicsState,
    ctx: &RenderContext,
) {
    let painter = ctx.painter;
    let sf = ctx.sf;

    if let Some(snap) = &sim.current_snapshot {
        let map_w = sim.map_w;

        if ctx.zoom_scaled >= 0.6 {
            for attack in &snap.attacks {
                if attack.target_owner == 0 {
                    continue;
                }
                let my_id = sim.my_player_id.unwrap_or(0);
                let is_incoming = attack.target_owner == my_id && my_id != 0;

                // Only draw threat lines and pointing fingers for incoming attacks on us
                if !is_incoming || attack.troops <= 0.0 {
                    continue;
                }

                let mut rx = 0.5;
                let mut ry = 0.5;
                let mut tx = 0.5;
                let mut ty = 0.5;

                if let Some(attacker) = snap.players.iter().find(|p| p.id == attack.owner_id) {
                    rx = attacker.centroid_x + 0.5;
                    ry = attacker.centroid_y + 0.5;
                }
                if let Some(target) = snap.players.iter().find(|p| p.id == attack.target_owner) {
                    tx = target.centroid_x + 0.5;
                    ty = target.centroid_y + 0.5;
                }

                // Start at the battlefront (or fall back to target centroid if front_cx/front_cy is 0)
                let mut fx = tx;
                let mut fy = ty;
                if attack.front_cx != 0.0 || attack.front_cy != 0.0 {
                    fx = attack.front_cx + 0.5;
                    fy = attack.front_cy + 0.5;
                }

                let start_x = (input.camera_x + fx * input.camera_zoom) / sf;
                let start_y = (input.camera_y + fy * input.camera_zoom) / sf;
                let end_x = (input.camera_x + rx * input.camera_zoom) / sf;
                let end_y = (input.camera_y + ry * input.camera_zoom) / sf;

                let color = egui::Color32::from_rgb(255, 60, 60); // Bright warning red for incoming threat source

                let start_pos = egui::pos2(start_x, start_y);
                let end_pos = egui::pos2(end_x, end_y);

                // 1. Draw a soft, elegant background guide rail/shadow line
                painter.line_segment(
                    [start_pos, end_pos],
                    egui::Stroke::new(3.0_f32, egui::Color32::from_black_alpha(30)),
                );
                painter.line_segment(
                    [start_pos, end_pos],
                    egui::Stroke::new(1.0_f32, color.linear_multiply(0.35)),
                );

                // 2. Render beautiful flowing dots sliding from the battlefront to the aggressor
                let elapsed = time.start_time.elapsed().as_secs_f32();
                let num_dots = 4;
                for i in 0..num_dots {
                    let t_offset = (i as f32) / (num_dots as f32);
                    let t = (t_offset + elapsed * 0.45) % 1.0;

                    // Interpolated position along the ray
                    let dot_pos = egui::pos2(
                        start_pos.x + (end_pos.x - start_pos.x) * t,
                        start_pos.y + (end_pos.y - start_pos.y) * t,
                    );

                    // Fade out as it reaches the aggressor destination
                    let alpha = ((1.0 - t) * 255.0) as u8;
                    let outer_glow = egui::Color32::from_rgba_unmultiplied(
                        color.r(),
                        color.g(),
                        color.b(),
                        alpha / 3,
                    );
                    let inner_core = egui::Color32::from_rgba_unmultiplied(
                        color.r(),
                        color.g(),
                        color.b(),
                        alpha,
                    );

                    // Draw soft outer halo and solid inner core
                    painter.circle_filled(dot_pos, 4.5_f32, outer_glow);
                    painter.circle_filled(dot_pos, 1.8_f32, inner_core);
                }

                if attack.retreating
                    && (time.start_time.elapsed().as_millis() / 500).is_multiple_of(2)
                {
                    let center = start_pos.lerp(end_pos, 0.5);
                    painter.text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        "[X]",
                        egui::FontId::proportional(20.0),
                        egui::Color32::RED,
                    );
                }
            }

            // --- Render Attack Troop Count Badges at the frontier centroids ---
            let middle_painter = painter.ctx().layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("attack_badges"),
            ));

            for attack in &snap.attacks {
                if attack.troops <= 0.0 {
                    continue;
                }

                let my_id = sim.my_player_id.unwrap_or(0);
                let is_outgoing = attack.owner_id == my_id && my_id != 0;
                let is_incoming = attack.target_owner == my_id && my_id != 0;

                // Only show labels for outgoing or incoming attacks involving the player
                if !is_outgoing && !is_incoming {
                    continue;
                }

                let cx = attack.front_cx;
                let cy = attack.front_cy;
                if cx == 0.0 && cy == 0.0 {
                    continue;
                }

                // Convert centroid column/row to world coordinates
                let wx = cx + 0.5;
                let wy = cy + 0.5;

                // Convert to screen coordinates
                let screen_x = (input.camera_x + wx * input.camera_zoom) / sf;
                let screen_y = (input.camera_y + wy * input.camera_zoom) / sf;

                // Frustum cull
                if screen_x < -80.0
                    || screen_x > input.screen_w / sf + 80.0
                    || screen_y < -40.0
                    || screen_y > input.screen_h / sf + 40.0
                {
                    continue;
                }

                let troops_val = attack.troops;
                let entry = ui
                    .attack_troop_labels
                    .entry(attack.id)
                    .or_insert_with(|| (troops_val, sow_ui_kit::utils::format_number(troops_val)));
                if (entry.0 - troops_val).abs() > 0.0001 {
                    *entry = (troops_val, sow_ui_kit::utils::format_number(troops_val));
                }
                let troops_str = &entry.1;
                let color = if is_incoming {
                    egui::Color32::from_rgb(255, 90, 90) // Red for incoming
                } else {
                    sow_ui_kit::theme::accent_solo_cyan_hover() // Cyan for outgoing
                };

                let font_id = egui::FontId::proportional(13.0);
                let galley =
                    middle_painter.layout_no_wrap(troops_str.to_owned(), font_id.clone(), color);
                let row_w = crate::hud::nameplate::troops_row_width(&galley, &font_id);
                let anchor = egui::pos2(
                    screen_x - row_w / 2.0,
                    screen_y - galley.rect.height() / 2.0,
                );
                crate::hud::nameplate::paint_glow_troops_row(
                    &middle_painter,
                    anchor,
                    galley,
                    &font_id,
                    color,
                    None,
                );
            }
        }

        // ── Nuke Placement Preview (visible at all zoom levels) ─────────
        if ui.app.hud_state.selected_nuke_kind.is_some() {
            // Resolve hovered tile from mouse (same hex math as buildings.rs)
            let mx = input.last_mouse_x as f32;
            let my = input.last_mouse_y as f32;
            let world_x = (mx - input.camera_x) / input.camera_zoom;
            let world_y = (my - input.camera_y) / input.camera_zoom;
            let (h_col, h_row) = world_to_tile(world_x, world_y);

            if h_col >= 0 && h_row >= 0 && h_col < sim.map_w as i32 && h_row < sim.map_h as i32 {
                let target_tile = (h_row * sim.map_w as i32 + h_col) as u32;
                let my_id = sim.my_player_id.unwrap_or(0);
                let current_tick = snap.tick;

                // Find nearest owned completed City not on cooldown
                let best_silo = snap
                    .buildings
                    .iter()
                    .filter(|b| {
                        b.kind == sow_core::game::BuildingKind::City
                            && b.owner_id == my_id
                            && !b.under_construction
                            && ui
                                .silo_cooldowns
                                .get(&b.id)
                                .is_none_or(|&exp| current_tick >= exp)
                    })
                    .min_by_key(|b| {
                        let bx = (b.tile_idx % sim.map_w) as i32;
                        let by = (b.tile_idx / sim.map_w) as i32;
                        let tx = target_tile as i32 % sim.map_w as i32;
                        let ty = target_tile as i32 / sim.map_w as i32;
                        (bx - tx).abs().max((by - ty).abs())
                    });

                // Show cooldown arc loaders on cities that recently fired
                for b in snap.buildings.iter().filter(|b| {
                    b.kind == sow_core::game::BuildingKind::City
                        && b.owner_id == my_id
                        && ui
                            .silo_cooldowns
                            .get(&b.id)
                            .is_some_and(|&exp| current_tick < exp)
                }) {
                    let (bwx, bwy) = tile_to_world(b.tile_idx, map_w);
                    let bsx = (input.camera_x + bwx * input.camera_zoom) / sf;
                    let bsy = (input.camera_y + bwy * input.camera_zoom) / sf;
                    let center = egui::pos2(bsx, bsy);
                    let radius = preview_px(0.35, input.camera_zoom, sf, MIN_PREVIEW_RING_PX);

                    let expires = ui.silo_cooldowns[&b.id];
                    let remaining = expires.saturating_sub(current_tick) as f32;
                    let progress = (remaining / 90.0).clamp(0.0, 1.0);

                    // Black transparent circle panel behind
                    painter.circle_filled(
                        center,
                        radius + 2.0_f32,
                        egui::Color32::from_black_alpha(150),
                    );

                    // Dim track outline
                    painter.circle_stroke(
                        center,
                        radius,
                        egui::Stroke::new(
                            2.5_f32,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 35),
                        ),
                    );

                    if progress > 0.0 {
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
                            egui::Stroke::new(
                                2.5_f32,
                                egui::Color32::from_rgb(239, 68, 68), // Red arc
                            ),
                        ));
                    }
                }

                if let Some(silo) = best_silo {
                    let level = silo.modules.arsenal.max(1);
                    let silo_tile = silo.tile_idx;
                    let path =
                        sow_core::pathfinding::bresenham_line(silo_tile, target_tile, sim.map_w);
                    let path_len = path.len().max(1);

                    // ── 1. Source city pulsing highlight ─────────────────
                    let (silo_wx, silo_wy) = tile_to_world(silo_tile, map_w);
                    let silo_sx = (input.camera_x + silo_wx * input.camera_zoom) / sf;
                    let silo_sy = (input.camera_y + silo_wy * input.camera_zoom) / sf;
                    let silo_center = egui::pos2(silo_sx, silo_sy);

                    let pulse = ((time.start_time.elapsed().as_secs_f32() * 3.0).sin() * 0.3 + 0.7)
                        .clamp(0.4, 1.0);
                    let pulse_alpha = (pulse * 120.0) as u8;
                    let silo_ring_r = preview_px(0.8, input.camera_zoom, sf, MIN_PREVIEW_RING_PX);
                    let preview_stroke =
                        (input.camera_zoom * 0.015 + 2.0).clamp(MIN_PREVIEW_STROKE_PX, 4.0);
                    painter.circle_stroke(
                        silo_center,
                        silo_ring_r,
                        egui::Stroke::new(
                            preview_stroke,
                            egui::Color32::from_rgba_unmultiplied(34, 211, 238, pulse_alpha),
                        ),
                    );

                    // ── 2. Trajectory arc ────────────────────────────────
                    let num_samples = 40.min(path_len);
                    let step = if num_samples > 1 {
                        (path_len - 1) as f32 / (num_samples - 1) as f32
                    } else {
                        1.0
                    };

                    let mut arc_points = Vec::with_capacity(num_samples + 1);
                    for i in 0..num_samples {
                        let idx = (i as f32 * step) as usize;
                        let idx = idx.min(path_len - 1);
                        let p = idx as f32 / (path_len - 1).max(1) as f32;
                        let (twx, twy) = tile_to_world(path[idx], map_w);
                        let height = 4.0 * p * (1.0 - p) * 20.0;
                        let sx = (input.camera_x + twx * input.camera_zoom) / sf;
                        let sy = (input.camera_y + (twy - height) * input.camera_zoom) / sf;
                        arc_points.push(egui::pos2(sx, sy));
                    }

                    // Draw dark outline behind for contrast
                    for win in arc_points.windows(2) {
                        painter.line_segment(
                            [win[0], win[1]],
                            egui::Stroke::new(
                                preview_stroke + 1.0,
                                egui::Color32::from_black_alpha(80),
                            ),
                        );
                    }

                    // Draw dashed arc segments (alternating visible/gap)
                    let arc_color = if level >= 2 {
                        egui::Color32::from_rgba_unmultiplied(255, 170, 50, 220)
                    } else {
                        egui::Color32::from_rgba_unmultiplied(255, 220, 130, 220)
                    };
                    for (seg_i, win) in arc_points.windows(2).enumerate() {
                        if seg_i % 3 != 2 {
                            painter.line_segment(
                                [win[0], win[1]],
                                egui::Stroke::new(preview_stroke, arc_color),
                            );
                        }
                    }

                    // ── 3. Wilderness-clearing blast radius at target ────
                    let (tgt_wx, tgt_wy) = tile_to_world(target_tile, map_w);
                    let tgt_sx = (input.camera_x + tgt_wx * input.camera_zoom) / sf;
                    let tgt_sy = (input.camera_y + tgt_wy * input.camera_zoom) / sf;
                    let tgt_center = egui::pos2(tgt_sx, tgt_sy);

                    let inner_tiles = sow_core::game::nuke_inner_radius(level) as f32;
                    let inner_r = preview_px(
                        inner_tiles,
                        input.camera_zoom,
                        sf,
                        MIN_PREVIEW_RING_PX * 1.5,
                    );
                    painter.circle_filled(
                        tgt_center,
                        inner_r,
                        egui::Color32::from_rgba_unmultiplied(255, 80, 30, 15),
                    );
                    painter.circle_stroke(
                        tgt_center,
                        inner_r,
                        egui::Stroke::new(
                            preview_stroke,
                            egui::Color32::from_rgba_unmultiplied(255, 90, 40, 140),
                        ),
                    );

                    // ── 4. Ghost nuke icon at target ─────────────────────
                    let ghost_size = (18.0 + level as f32 * 4.0).max(MIN_PREVIEW_ICON_PX);
                    let rect = egui::Rect::from_center_size(
                        tgt_center,
                        egui::vec2(ghost_size, ghost_size),
                    );
                    let tint = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 140);
                    if !sow_ui_kit::widgets::try_paint_emoji(painter, "☢️", rect, tint) {
                        painter.text(
                            tgt_center,
                            egui::Align2::CENTER_CENTER,
                            "☢️",
                            egui::FontId::proportional(ghost_size * 0.7),
                            tint,
                        );
                    }
                } else {
                    // No city available (all on cooldown or none owned)
                    let (tgt_wx, tgt_wy) = tile_to_world(target_tile, map_w);
                    let tgt_sx = (input.camera_x + tgt_wx * input.camera_zoom) / sf;
                    let tgt_sy = (input.camera_y + tgt_wy * input.camera_zoom) / sf;
                    painter.text(
                        egui::pos2(tgt_sx, tgt_sy),
                        egui::Align2::CENTER_CENTER,
                        "⏳ Cooldown",
                        egui::FontId::proportional(14.0),
                        egui::Color32::from_rgb(255, 160, 60),
                    );
                }
            }
        }
    }
}
