use crate::app::SowApp;
use egui::{Align2, Color32, FontId, RichText};

impl SowApp {
    #[allow(deprecated)]
    pub(crate) fn render_endgame_ui(&mut self, ctx: &egui::Context) {
        let mut show_endgame = false;
        let mut is_victory = false;
        let mut text_title = "";
        let mut text_subtitle = String::new();
        let my_id = self.sim.my_player_id.unwrap_or(0);

        if let Some(snap) = &self.sim.current_snapshot {
            if let Some(winner) = snap.winner {
                show_endgame = true;
                if winner == my_id {
                    is_victory = true;
                    text_title = "VICTORY";
                    text_subtitle = "You have conquered the world.".to_string();
                } else {
                    is_victory = false;
                    text_title = "DEFEAT";
                    let winner_name = snap.players.iter().find(|p| p.id == winner).map(|p| p.name.clone()).unwrap_or_else(|| "Unknown".to_string());
                    text_subtitle = format!("{} emerged victorious.", winner_name);
                }
            } else {
                if let Some(me) = snap.players.iter().find(|p| p.id == my_id) {
                    if !me.alive && me.has_spawned {
                        show_endgame = true;
                        is_victory = false;
                        text_title = "DEFEAT";
                        text_subtitle = "Your empire has fallen.".to_string();
                    }
                }
            }
        }

        if show_endgame {
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
                        let title_color = if is_victory { Color32::GOLD } else { Color32::RED };
                        ui.label(RichText::new(text_title).color(title_color).font(FontId::proportional(64.0)).strong());
                        ui.add_space(10.0);
                        ui.label(RichText::new(&text_subtitle).color(Color32::LIGHT_GRAY).font(FontId::proportional(24.0)));
                        
                        ui.add_space(30.0);
                        
                        let btn_color = if is_victory { Color32::from_rgb(40, 140, 40) } else { Color32::from_rgb(140, 40, 40) };
                        if ui.add_sized([200.0, 50.0], egui::Button::new(RichText::new("Return to Lobby").color(Color32::WHITE).font(FontId::proportional(20.0))).fill(btn_color)).clicked() {
                            // Disconnect and return to main menu
                            self.net.client = None;
                            self.begin_exit_to_main_menu();
                        }
                    });
                });
        }
    }
}
