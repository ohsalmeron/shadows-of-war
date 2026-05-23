use super::*;
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
            // --- Layer 2: Fleets & Trails ---
            let now = web_time::Instant::now();
            let sim_dt = now.duration_since(time.last_tick).as_secs_f32();
            let tick_dur = time.tick_interval.as_secs_f32().max(0.01);
            let mut t = (sim_dt / tick_dur).clamp(0.0, 1.0);
            t = t * t * (3.0 - 2.0 * t); // Smoothstep curve

            // S8: Reuse a single trail points vec across all fleets
            let mut points = Vec::with_capacity(64);

            for fleet in &snap.fleets {
                let mut r = 0.5;
                let mut g = 0.5;
                let mut b = 0.5;
                if let Some(owner) = snap.players.iter().find(|p| p.id == fleet.owner_id) {
                    let rgb = if owner.player_type == sow_core::player::PlayerType::Human {
                        sow_core::player::human_shader_territory_rgb(owner.id)
                    } else {
                        owner.color
                    };
                    r = rgb[0];
                    g = rgb[1];
                    b = rgb[2];
                }
                let color = egui::Color32::from_rgb(
                    (r * 255.0) as u8,
                    (g * 255.0) as u8,
                    (b * 255.0) as u8,
                );
                let trail_color = egui::Color32::from_rgba_premultiplied(
                    (r * 255.0) as u8,
                    (g * 255.0) as u8,
                    (b * 255.0) as u8,
                    150,
                );

                // Render trail as a single line shape for massive performance boost
                let trail_len = fleet.path_cursor.min(fleet.path.len());
                points.clear();
                for &tile in &fleet.path[..trail_len] {
                    let wx = (tile % sim.map_w) as f32;
                    let wy = (tile / sim.map_w) as f32;
                    // Center the points in the tile
                    let screen_x = (input.camera_x + (wx + 0.5) * input.camera_zoom) / sf;
                    let screen_y = (input.camera_y + (wy + 0.5) * input.camera_zoom) / sf;
                    points.push(egui::pos2(screen_x, screen_y));
                }
                if points.len() > 1 {
                    let line_points = std::mem::take(&mut points);
                    painter.add(egui::Shape::line(
                        line_points,
                        egui::Stroke::new(zoom_scaled * 0.4, trail_color),
                    ));
                } else if points.len() == 1 {
                    painter.circle_filled(points[0], zoom_scaled * 0.2, trail_color);
                }

                // Render boat with smooth visual interpolation
                let wx_curr = (fleet.current_tile % sim.map_w) as f32;
                let wy_curr = (fleet.current_tile / sim.map_w) as f32;

                let mut wx = wx_curr;
                let mut wy = wy_curr;

                if fleet.path_cursor > 0 && !fleet.path.is_empty() {
                    let prev_idx = fleet
                        .path_cursor
                        .saturating_sub(2)
                        .min(fleet.path.len().saturating_sub(1));
                    let prev_tile = fleet.path[prev_idx];
                    let wx_prev = (prev_tile % sim.map_w) as f32;
                    let wy_prev = (prev_tile / sim.map_w) as f32;

                    wx = wx_prev + (wx_curr - wx_prev) * t;
                    wy = wy_prev + (wy_curr - wy_prev) * t;
                }

                let center_x = (input.camera_x + (wx + 0.5) * input.camera_zoom) / sf;
                let center_y = (input.camera_y + (wy + 0.5) * input.camera_zoom) / sf;
                let center = egui::pos2(center_x, center_y);

                let base_size = (zoom_scaled * 0.7).clamp(12.0, 64.0);
                let margin = base_size * 0.2;
                let rect = egui::Rect::from_center_size(center, egui::vec2(base_size, base_size));

                let uri = fleet.unit_type.asset().uri();

                let size_hint = egui::load::SizeHint::Size {
                    width: 64,
                    height: 64,
                    maintain_aspect_ratio: true,
                };

                let load_res = painter.ctx().try_load_texture(
                    uri,
                    egui::TextureOptions::LINEAR,
                    size_hint,
                );

                if let Ok(egui::load::TexturePoll::Ready { texture }) = load_res {
                    painter.image(
                        texture.id,
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        color,
                    );
                }

                if input.selected_warships.contains(&fleet.id) {
                    painter.rect_stroke(
                        rect.expand(2.0),
                        0.0,
                        egui::Stroke::new(2.0_f32, egui::Color32::YELLOW),
                        egui::StrokeKind::Middle,
                    );
                }

                if fleet.retreating
                    && (time.start_time.elapsed().as_millis() / 500).is_multiple_of(2)
                {
                    let center = rect.center();
                    painter.line_segment(
                        [
                            egui::pos2(center.x - margin, center.y - margin),
                            egui::pos2(center.x + margin, center.y + margin),
                        ],
                        egui::Stroke::new(2.0_f32, egui::Color32::BLACK),
                    );
                    painter.line_segment(
                        [
                            egui::pos2(center.x + margin, center.y - margin),
                            egui::pos2(center.x - margin, center.y + margin),
                        ],
                        egui::Stroke::new(2.0_f32, egui::Color32::BLACK),
                    );
                }
            }


    }
}
