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
                    let winner_name = snap
                        .players
                        .iter()
                        .find(|p| p.id == winner)
                        .map(|p| p.name.clone())
                        .unwrap_or_else(|| "Unknown".to_string());
                    text_subtitle = format!("{} emerged victorious.", winner_name);
                }
            } else {
                if let Some(me) = snap.players.iter().find(|p| p.id == my_id) {
                    if !me.alive && me.has_spawned {
                        if !self.ui.is_spectating {
                            show_endgame = true;
                        }
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
                .order(egui::Order::Foreground)
                .fixed_pos(egui::Pos2::ZERO)
                .show(ctx, |ui| {
                    let rect = ctx.content_rect();
                    ui.painter()
                        .rect_filled(rect, 0.0, Color32::from_black_alpha(180));
                });

            let screen_width = ctx.content_rect().width();
            let is_mobile = screen_width < 600.0;

            let (
                title_size,
                subtitle_size,
                space_top,
                space_mid,
                space_bot,
                btn_size,
                win_width,
                win_margin,
            ): (f32, f32, f32, f32, f32, egui::Vec2, f32, f32) = if is_mobile {
                (44.0, 16.0, 8.0, 12.0, 20.0, egui::vec2(180.0, 44.0), 300.0, 20.0)
            } else {
                (64.0, 22.0, 10.0, 15.0, 30.0, egui::vec2(220.0, 50.0), 400.0, 30.0)
            };

            let border_color = if is_victory {
                sow_ui::ui::theme::accent_ranked_gold()
            } else {
                sow_ui::ui::theme::palette::danger()
            };

            egui::Window::new("Endgame")
                .title_bar(false)
                .collapsible(false)
                .resizable(false)
                .order(egui::Order::Foreground)
                .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
                .frame(
                    egui::Frame::window(&ctx.global_style())
                        .fill(sow_ui::ui::theme::panel_bg())
                        .stroke(egui::Stroke::new(2.0_f32, border_color))
                        .corner_radius(12.0)
                        .inner_margin(win_margin),
                )
                .show(ctx, |ui| {
                    let actual_width = win_width.min(screen_width - 32.0);
                    ui.set_min_width(actual_width);
                    ui.set_max_width(actual_width);

                    ui.vertical_centered(|ui| {
                        let title_color = if is_victory {
                            sow_ui::ui::theme::accent_ranked_gold()
                        } else {
                            sow_ui::ui::theme::palette::danger()
                        };

                        let title_font = FontId::proportional(title_size);
                        let galley = ui.painter().layout_no_wrap(
                            text_title.to_string(),
                            title_font.clone(),
                            title_color,
                        );
                        let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
                        if ui.is_rect_visible(rect) {
                            sow_ui::ui::theme::outlined_text(
                                ui.painter(),
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                text_title,
                                title_font,
                                title_color,
                                Color32::BLACK,
                            );
                        }

                        ui.add_space(space_top);
                        ui.label(
                            RichText::new(&text_subtitle)
                                .color(Color32::LIGHT_GRAY)
                                .font(FontId::proportional(subtitle_size)),
                        );

                        ui.add_space(space_bot);

                        let btn_color = if is_victory {
                            Color32::from_rgb(40, 140, 40)
                        } else {
                            Color32::from_rgb(140, 40, 40)
                        };

                        let return_btn = sow_ui::widgets::ThemeButton::new("EXIT")
                            .min_size(btn_size)
                            .text_size(if is_mobile { 16.0 } else { 20.0 })
                            .custom_fill(btn_color);

                        if ui.add(return_btn).clicked() {
                            // Disconnect and return to main menu
                            self.net.client = None;
                            self.begin_exit_to_main_menu(true);
                        }

                        // Add SPECTATE button if defeated but the game has not officially ended
                        if !is_victory
                            && self
                                .sim
                                .current_snapshot
                                .as_ref()
                                .map_or(false, |s| s.winner.is_none())
                        {
                            ui.add_space(space_mid);

                            let spectate_btn = sow_ui::widgets::ThemeButton::new("SPECTATE")
                                .min_size(btn_size)
                                .text_size(if is_mobile { 16.0 } else { 20.0 })
                                .custom_fill(Color32::from_rgb(60, 60, 60));

                            if ui.add(spectate_btn).clicked() {
                                self.ui.is_spectating = true;
                            }
                        }
                    });
                });
        }
    }
}
