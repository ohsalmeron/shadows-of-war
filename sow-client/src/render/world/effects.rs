use super::*;

#[allow(unused_variables)]
pub(crate) fn render(
    ui: &mut crate::app::UiState,
    sim: &crate::app::SimState,
    input: &crate::app::InputState,
    time: &crate::app::TimeState,
    gfx: &crate::app::GraphicsState,
    ctx: &RenderContext,
) {
    let sf = ctx.sf;
    let zoom_scaled = ctx.zoom_scaled;

    let current_time = web_time::Instant::now();

    if !sow_ui_kit::theme::dev_config::DevConfig::get().vfx_click_markers {
        return;
    }

    let marker_painter = ctx.painter.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Middle,
        egui::Id::new("click_markers"),
    ));

    if !ui.click_markers.is_empty() {
        marker_painter.ctx().request_repaint();
    }

    ui.click_markers.retain(|m| {
        let duration = 0.16_f32;
        let elapsed = current_time.duration_since(m.start_time).as_secs_f32();
        if elapsed > duration {
            return false;
        }
        let t = elapsed / duration;
        let screen_x = (input.camera_x + m.world_x * input.camera_zoom) / sf;
        let screen_y = (input.camera_y + m.world_y * input.camera_zoom) / sf;
        let center = egui::pos2(screen_x, screen_y);

        let radius = 24.0 * t;
        let alpha = 1.0 - t;
        let color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, (alpha * 200.0) as u8);

        marker_painter.circle_stroke(
            center,
            radius * zoom_scaled.min(1.0),
            egui::Stroke::new(1.5_f32, color),
        );

        let half = 4.0 * zoom_scaled.min(1.0) * (1.0 - t);
        marker_painter.line_segment(
            [
                center + egui::vec2(-half, -half),
                center + egui::vec2(half, half),
            ],
            egui::Stroke::new(1.0_f32, color),
        );
        marker_painter.line_segment(
            [
                center + egui::vec2(-half, half),
                center + egui::vec2(half, -half),
            ],
            egui::Stroke::new(1.0_f32, color),
        );

        true
    });
}
