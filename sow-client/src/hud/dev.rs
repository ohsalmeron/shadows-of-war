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
        self.ui.app.hud_state.map_w = self.sim.map_w;

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
        let compact = rect.width() < 768.0 || rect.width() < rect.height() * 1.25;
        let text_size = if compact { 10.0 } else { 11.0 };
        let inset = if compact { 8.0 } else { 12.0 };

        let stats = if let Some(ping) = self.net.current_ping_ms {
            format!(
                "{ping}ms · {} fps · Zoom: {:.2}",
                self.time.current_fps, self.input.camera_zoom
            )
        } else {
            format!(
                "{} fps · Zoom: {:.2}",
                self.time.current_fps, self.input.camera_zoom
            )
        };

        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("dev_stats"),
        ));
        let font = egui::FontId::monospace(text_size);
        let color = egui::Color32::from_gray(165);
        let galley = painter.layout_no_wrap(stats, font, color);
        let size = galley.size();
        let pos = egui::pos2(rect.max.x - inset - size.x, rect.max.y - inset - size.y);
        painter.galley(pos, galley, color);
    }
}
