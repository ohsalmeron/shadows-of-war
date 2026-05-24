use web_time::Instant;

use crate::app::SowApp;

impl SowApp {
    pub(crate) fn calculate_fps_and_ping(&mut self) {
        self.time.frame_count += 1;
        if self.time.last_fps_time.elapsed().as_secs_f64() >= 1.0 {
            self.time.current_fps = self.time.frame_count;
            self.time.frame_count = 0;
            self.time.last_fps_time = Instant::now();
        }

        if self.net.last_ping_time.elapsed().as_secs_f64() >= 1.0 {
            if let Some(c) = self.net.client.as_ref() {
                let ping_msg = sow_core::protocol::ClientMessage::Ping {
                    client_time: self.time.start_time.elapsed().as_secs_f64(),
                };
                if let Ok(json) = bincode::serialize(&ping_msg) {
                    c.send(json);
                }
            }
            self.net.last_ping_time = Instant::now();
        }
    }

    /// Sync snapshot attacks/fleets/players into hud_state so `hud::draw` can render them.
    pub(crate) fn sync_hud_combat_state(&mut self) {
        let my_pid = self.sim.my_player_id.unwrap_or(0);
        self.ui.app.hud_state.my_player_id = my_pid;

        if let Some(snap) = &self.sim.current_snapshot {
            self.ui.app.hud_state.attacks = snap.attacks.clone();
            self.ui.app.hud_state.fleets = snap.fleets.clone();
            self.ui.app.hud_state.players = snap.players.clone();
        } else {
            self.ui.app.hud_state.attacks.clear();
            self.ui.app.hud_state.fleets.clear();
            self.ui.app.hud_state.players.clear();
        }
    }

    pub(crate) fn render_dev_panels(&mut self, ctx: &egui::Context) {
        let rect = ctx.content_rect();
        let compact = rect.width() < 1024.0 || rect.width() < rect.height() * 1.25;
        let text_size = if compact { 10.0 } else { 12.0 };
        let padding = if compact {
            egui::Margin::symmetric(6, 3)
        } else {
            egui::Margin::symmetric(10, 5)
        };
        let corner_radius = if compact { 8.0 } else { 10.0 };

        egui::Area::new(egui::Id::new("ping_fps_zoom_area"))
            .anchor(egui::Align2::CENTER_TOP, egui::vec2(0.0, 28.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(sow_ui::ui::theme::panel_bg_transparent())
                    .stroke(egui::Stroke::new(1.0_f32, sow_ui::ui::theme::nickname_field_border()))
                    .corner_radius(corner_radius)
                    .inner_margin(padding)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = if compact { 6.0 } else { 10.0 };
                        ui.horizontal(|ui| {
                            if let Some(ping) = self.net.current_ping_ms {
                                ui.label(
                                    egui::RichText::new(format!("Ping: {}ms", ping))
                                        .color(egui::Color32::WHITE)
                                        .size(text_size)
                                        .strong()
                                );
                            }
                            ui.label(
                                egui::RichText::new(format!("FPS: {}", self.time.current_fps))
                                    .color(egui::Color32::YELLOW)
                                    .size(text_size)
                                    .strong()
                            );
                            ui.label(
                                egui::RichText::new(format!("Zoom: {:.2}", self.input.camera_zoom))
                                    .color(egui::Color32::LIGHT_BLUE)
                                    .size(text_size)
                                    .strong()
                            );
                        });
                    });
            });
    }
}
