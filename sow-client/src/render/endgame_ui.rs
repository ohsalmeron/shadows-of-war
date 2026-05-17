use crate::app::SowApp;
use egui::{Align2, Color32, FontId, RichText};
use sow_ui::app::ClientPhase;

impl SowApp {
    #[allow(deprecated)]
    pub(crate) fn render_endgame_ui(&mut self, ctx: &egui::Context) {
        let winner_id = match &self.sim.current_snapshot {
            Some(snap) => snap.winner,
            None => None,
        };

        if let Some(winner) = winner_id {
            let my_id = self.sim.my_player_id.unwrap_or(0);
            let is_victory = winner == my_id;
            
            // Dim background
            egui::Area::new(egui::Id::new("endgame_dimmer"))
                .order(egui::Order::Background)
                .fixed_pos(egui::Pos2::ZERO)
                .show(ctx, |ui| {
                    let rect = ctx.screen_rect();
                    ui.painter().rect_filled(rect, 0.0, Color32::from_black_alpha(150));
                });
            
            egui::Window::new("Endgame")
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .frame(egui::Frame::window(&ctx.global_style()).fill(Color32::from_rgb(20, 20, 25)).inner_margin(30.0))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        if is_victory {
                            ui.label(RichText::new("VICTORY").color(Color32::GOLD).font(FontId::proportional(64.0)).strong());
                            ui.add_space(10.0);
                            ui.label(RichText::new("You have conquered the world.").color(Color32::LIGHT_GRAY).font(FontId::proportional(24.0)));
                        } else {
                            ui.label(RichText::new("DEFEAT").color(Color32::RED).font(FontId::proportional(64.0)).strong());
                            ui.add_space(10.0);
                            let winner_name = self.sim.current_snapshot.as_ref().unwrap().players.iter().find(|p| p.id == winner).map(|p| p.name.clone()).unwrap_or_else(|| "Unknown".to_string());
                            ui.label(RichText::new(format!("{} emerged victorious.", winner_name)).color(Color32::LIGHT_GRAY).font(FontId::proportional(24.0)));
                        }
                        
                        ui.add_space(30.0);
                        
                        let btn_color = if is_victory { Color32::from_rgb(40, 140, 40) } else { Color32::from_rgb(140, 40, 40) };
                        if ui.add_sized([200.0, 50.0], egui::Button::new(RichText::new("Return to Lobby").color(Color32::WHITE).font(FontId::proportional(20.0))).fill(btn_color)).clicked() {
                            // Disconnect and return to main menu
                            self.net.client = None;
                            self.sim.current_snapshot = None;
                            self.sim.my_lobby_id = None;
                            self.app.phase = ClientPhase::MainMenu;
                            self.app.main_menu_state.is_waiting = false;
                        }
                    });
                });
        }
    }
}
