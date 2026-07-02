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
    if zoom_scaled < 5.0 {
        return;
    }

    if let Some(snap) = &sim.current_snapshot {
        let alpha = crate::render::world::movers::interp_alpha(time, web_time::Instant::now());

        for fleet in &snap.fleets {
            let (wx_curr, wy_curr) =
                crate::render::world::movers::tile_to_world(fleet.current_tile, sim.map_w);

            let mut wx = wx_curr;
            let mut wy = wy_curr;

            if fleet.path_cursor > 0 && !fleet.path.is_empty() {
                let prev_idx = fleet
                    .path_cursor
                    .saturating_sub(2)
                    .min(fleet.path.len().saturating_sub(1));
                let prev_tile = fleet.path[prev_idx];
                let (wx_prev, wy_prev) =
                    crate::render::world::movers::tile_to_world(prev_tile, sim.map_w);

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

            if fleet.retreating
                && (time.start_time.elapsed().as_millis() / 500).is_multiple_of(2)
                && sow_ui_kit::theme::dev_config::DevConfig::get().vfx_fleet_blink
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
