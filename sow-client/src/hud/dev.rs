

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
        egui::Window::new("Dev Toggle")
            .title_bar(false)
            .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-10.0, 10.0))
            .auto_sized()
            .frame(egui::Frame::window(&ctx.global_style()).fill(sow_ui::ui::theme::panel_bg()).stroke(egui::Stroke::new(1.0_f32, sow_ui::ui::theme::nickname_field_border())).corner_radius(12.0))
            .show(ctx, |ui| {
                let icon = if self.ui.show_dev_sidebar { "▼" } else { "▶" };
                let btn = egui::Button::new(egui::RichText::new(format!("{} Dev Info", icon))).min_size(egui::vec2(80.0, 30.0));
                if ui.add(btn).clicked() {
                    self.ui.show_dev_sidebar = !self.ui.show_dev_sidebar;
                }
            });

        if self.ui.show_dev_sidebar {
            egui::Window::new("HUD Sidebar")
                .title_bar(false)
                .anchor(egui::Align2::LEFT_TOP, egui::vec2(10.0, 10.0))
                .auto_sized()
                .frame(egui::Frame::window(&ctx.global_style()).fill(sow_ui::ui::theme::panel_bg()).stroke(egui::Stroke::new(1.0_f32, sow_ui::ui::theme::nickname_field_border())).corner_radius(12.0))
                .show(ctx, |ui| {
                    let mut is_expanded = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(egui::Id::new("dev_utils_expanded"), || false));
                    ui.vertical(|ui| {
                        // 1. Stats Row
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
                        
                        ui.add_space(4.0);

                        // 2. Buttons Row
                        ui.horizontal(|ui| {
                            let ld_icon = if self.ui.show_leaderboard { "▼" } else { "▶" };
                            let ld_btn = egui::Button::new(egui::RichText::new(format!("{} 🏆 Leaderboard", ld_icon))).min_size(egui::vec2(100.0, 30.0));
                            if ui.add(ld_btn).clicked() {
                                self.ui.show_leaderboard = !self.ui.show_leaderboard;
                            }

                            let dev_icon = if is_expanded { "▼" } else { "▶" };
                            let dev_btn = egui::Button::new(egui::RichText::new(format!("{} 🛠 Dev Utils", dev_icon))).min_size(egui::vec2(100.0, 30.0));
                            if ui.add(dev_btn).clicked() {
                                is_expanded = !is_expanded;
                                ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_utils_expanded"), is_expanded));
                            }
                        });

                        // 3. Dev Utils Expanded Panel
                        if is_expanded {
                            ui.separator();
                            ui.style_mut().spacing.slider_width = 100.0;
                            ui.style_mut().spacing.item_spacing = egui::vec2(4.0, 4.0);

                            let mut thick = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(egui::Id::new("dev_thickness"), || 0.4f32));
                            let mut dark = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(egui::Id::new("dev_darkness"), || 0.15f32));
                            let mut s_thick = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(egui::Id::new("dev_shore_thickness"), || 0.4f32));
                            let mut s_dark = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(egui::Id::new("dev_shore_darkness"), || 0.15f32));
                            let mut roundness = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(egui::Id::new("dev_roundness"), || 0.5f32));
                            
                            ui.add(egui::Slider::new(&mut thick, 0.0..=1.0).text("Border Thk"));
                            ui.add(egui::Slider::new(&mut dark, 0.0..=1.0).text("Border Drk"));
                            ui.add(egui::Slider::new(&mut s_thick, 0.0..=1.0).text("Shore Thk"));
                            ui.add(egui::Slider::new(&mut s_dark, 0.0..=1.0).text("Shore Drk"));
                            ui.add(egui::Slider::new(&mut roundness, 0.0..=1.0).text("Roundness"));
                            
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_thickness"), thick));
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_darkness"), dark));
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_shore_thickness"), s_thick));
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_shore_darkness"), s_dark));
                            ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_roundness"), roundness));
                        }
                    });
                });
        }

                                if self.ui.app.phase == ClientPhase::Playing {
                                    if let Some(snap) = &self.sim.current_snapshot {
                                        let my_pid = self.sim.my_player_id.unwrap_or(0);
                                        if my_pid > 0 && (!snap.attacks.is_empty() || !snap.fleets.is_empty()) {
                                            egui::Window::new("Attacks")
                                                .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -140.0))
                                                .title_bar(false)
                                                .resizable(false)
                                                .collapsible(false)
                                                .frame(egui::Frame::window(&ctx.global_style()).fill(sow_ui::ui::theme::panel_bg()).stroke(egui::Stroke::new(1.0_f32, sow_ui::ui::theme::nickname_field_border())).corner_radius(12.0))
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
