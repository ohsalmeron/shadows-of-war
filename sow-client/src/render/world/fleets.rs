use super::*;
#[allow(unused_variables)]
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
    let zoom_scaled = ctx.zoom_scaled;

    if let Some(snap) = &sim.current_snapshot {
        let alpha = crate::render::world::movers::interp_alpha(time, web_time::Instant::now());

        for fleet in &snap.fleets {
            let tile_x_curr = (fleet.current_tile % sim.map_w) as f32;
            let tile_y_curr = (fleet.current_tile / sim.map_w) as f32;
            let wx_curr = tile_x_curr + 0.5 + (tile_y_curr as i32 % 2) as f32 * 0.5;
            let wy_curr = (tile_y_curr + 0.5) * 0.8660254_f32;

            let mut wx = wx_curr;
            let mut wy = wy_curr;

            if fleet.path_cursor > 0 && !fleet.path.is_empty() {
                let prev_idx = fleet
                    .path_cursor
                    .saturating_sub(2)
                    .min(fleet.path.len().saturating_sub(1));
                let prev_tile = fleet.path[prev_idx];
                let tile_x_prev = (prev_tile % sim.map_w) as f32;
                let tile_y_prev = (prev_tile / sim.map_w) as f32;
                let wx_prev = tile_x_prev + 0.5 + (tile_y_prev as i32 % 2) as f32 * 0.5;
                let wy_prev = (tile_y_prev + 0.5) * 0.8660254_f32;

                wx = wx_prev + (wx_curr - wx_prev) * alpha;
                wy = wy_prev + (wy_curr - wy_prev) * alpha;
            }

            let center_x = (input.camera_x + wx * input.camera_zoom) / sf;
            let center_y = (input.camera_y + wy * input.camera_zoom) / sf;
            let center = egui::pos2(center_x, center_y);

            let base_size = (zoom_scaled * 0.7).clamp(12.0, 64.0);
            let margin = base_size * 0.2;
            let rect = egui::Rect::from_center_size(center, egui::vec2(base_size, base_size));

            if input.selected_warships.contains(&fleet.id) {
                painter.rect_stroke(
                    rect.expand(2.0),
                    0.0,
                    egui::Stroke::new(2.0_f32, egui::Color32::YELLOW),
                    egui::StrokeKind::Middle,
                );
            }

            if fleet.retreating && (time.start_time.elapsed().as_millis() / 500).is_multiple_of(2) {
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
