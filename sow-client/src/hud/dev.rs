

use sow_ui::app::ClientPhase;
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

    pub(crate) fn render_dev_panels(&mut self, ctx: &egui::Context, local_cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>) {


        egui::Area::new(egui::Id::new("ping_fps_zoom_area"))
            .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(12.0, -12.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(sow_ui::ui::theme::panel_bg_transparent())
                    .stroke(egui::Stroke::new(1.0_f32, sow_ui::ui::theme::nickname_field_border()))
                    .corner_radius(12.0)
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if let Some(ping) = self.net.current_ping_ms {
                                ui.label(
                                    egui::RichText::new(format!("Ping: {}ms", ping))
                                        .color(egui::Color32::WHITE)
                                        .strong()
                                );
                            }
                            ui.label(
                                egui::RichText::new(format!("FPS: {}", self.time.current_fps))
                                    .color(egui::Color32::YELLOW)
                                    .strong()
                            );
                            ui.label(
                                egui::RichText::new(format!("Zoom: {:.2}", self.input.camera_zoom))
                                    .color(egui::Color32::LIGHT_BLUE)
                                    .strong()
                            );
                        });
                    });
            });

                                if self.ui.app.phase == ClientPhase::Playing {
                                    if let Some(snap) = &self.sim.current_snapshot {
                                        let my_pid = self.sim.my_player_id.unwrap_or(0);
                                        if my_pid > 0 && (!snap.attacks.is_empty() || !snap.fleets.is_empty()) {
                                            egui::Window::new("Attacks")
                                                .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -140.0))
                                                .title_bar(false)
                                                .resizable(false)
                                                .collapsible(false)
                                                .frame(egui::Frame::window(&ctx.global_style()).fill(sow_ui::ui::theme::panel_bg_transparent()).stroke(egui::Stroke::new(1.0_f32, sow_ui::ui::theme::nickname_field_border())).corner_radius(12.0))
                                                .show(ctx, |ui| {
                                                    ui.set_max_height(150.0);
                                                    egui::ScrollArea::vertical().show(ui, |ui| {
                                                        for attack in &snap.attacks {
                                                            if attack.owner_id == my_pid {
                                                                ui.horizontal(|ui| {
                                                                    ui.label(egui::RichText::new(format!("⚔ OUT {:.0}", attack.troops)).color(egui::Color32::from_rgb(0, 200, 255)));
                                                                    if let Some(target) = snap.players.iter().find(|p| p.id == attack.target_owner) {
                                                                        ui.label(&target.name);
                                                                    } else {
                                                                        ui.label("Wilderness");
                                                                    }
                                                                    if attack.retreating {
                                                                        ui.label("(Retreating...)");
                                                                    } else {
                                                                        if ui.button("[X]").clicked() {
                                                                            local_cancel_intents.push(sow_core::protocol::GameplayIntent::CancelAttack { attack_id: attack.id });
                                                                        }
                                                                    }
                                                                });
                                                            }
                                                        }
                                                        for fleet in &snap.fleets {
                                                            if fleet.owner_id == my_pid {
                                                                ui.horizontal(|ui| {
                                                                    ui.label(egui::RichText::new(format!("★ NAVY {:.0}", fleet.troops)).color(egui::Color32::from_rgb(0, 200, 255)));
                                                                    ui.label("Naval Invasion");
                                                                    if fleet.retreating {
                                                                        ui.label("(Retreating...)");
                                                                    } else {
                                                                        if ui.button("[X]").clicked() {
                                                                            local_cancel_intents.push(sow_core::protocol::GameplayIntent::RecallFleet { fleet_id: fleet.id });
                                                                        }
                                                                    }
                                                                });
                                                            }
                                                        }
                                                        for attack in &snap.attacks {
                                                            if attack.target_owner == my_pid {
                                                                ui.horizontal(|ui| {
                                                                    ui.label(egui::RichText::new(format!("⚔ IN {:.0}", attack.troops)).color(egui::Color32::RED));
                                                                    if let Some(attacker) = snap.players.iter().find(|p| p.id == attack.owner_id) {
                                                                        ui.label(&attacker.name);
                                                                    }
                                                                });
                                                            }
                                                        }
                                                    });
                                                });
                                        }
                                    }
                                }

    }
}
