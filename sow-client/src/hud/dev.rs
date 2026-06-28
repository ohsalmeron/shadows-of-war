use web_time::Instant;

use crate::app::SowApp;
use sow_ui_kit::ClientPhase;

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

    /// Sync snapshot attacks/fleets/players into hud_state when the sim tick advances.
    pub(crate) fn sync_hud_combat_state(&mut self) {
        let my_pid = self.sim.my_player_id.unwrap_or(0);
        self.ui.app.hud_state.my_player_id = my_pid;
        self.ui.app.hud_state.map_w = self.sim.map_w;

        if let Some(snap) = &self.sim.current_snapshot {
            if self.ui.hud_combat_sync_tick != snap.tick {
                self.ui.hud_combat_sync_tick = snap.tick;
                self.ui.app.hud_state.attacks = snap.attacks.clone();
                self.ui.app.hud_state.fleets = snap.fleets.clone();
                self.ui.app.hud_state.players = snap.players.clone();
            }
        } else if self.ui.hud_combat_sync_tick != 0 {
            self.ui.hud_combat_sync_tick = 0;
            self.ui.app.hud_state.attacks.clear();
            self.ui.app.hud_state.fleets.clear();
            self.ui.app.hud_state.players.clear();
        }
    }

    pub(crate) fn render_attacks_panel(&mut self, ctx: &egui::Context, local_cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>) {
        if self.ui.app.phase == ClientPhase::Playing {
            if let Some(snap) = &self.sim.current_snapshot {
                let my_pid = self.sim.my_player_id.unwrap_or(0);
                if my_pid > 0 && (!snap.attacks.is_empty() || !snap.fleets.is_empty()) {
                    egui::Window::new("Attacks")
                        .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -140.0))
                        .title_bar(false)
                        .resizable(false)
                        .collapsible(false)
                        .frame(egui::Frame::window(&ctx.global_style()).fill(egui::Color32::from_black_alpha(200)))
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

    pub(crate) fn render_stats_overlay(&mut self, _ctx: &egui::Context) {
        if let Some(ref mut tr) = self.gfx.text_renderer {
            let mut stats = String::new();
            if let Some(ping) = self.net.current_ping_ms {
                stats.push_str(&format!("{ping}ms · {} fps", self.time.current_fps));
            } else {
                stats.push_str(&format!("{} fps", self.time.current_fps));
            }
            stats.push_str(&format!(" · {:.2}x", self.input.camera_zoom));

            // Render stats overlay in the bottom right corner using the GPU TextRenderer
            let right_inset = 12.0;
            let bottom_inset = 12.0;
            let font_size = 11.0;
            let x = self.input.screen_w - right_inset;
            let y = self.input.screen_h - bottom_inset;

            tr.push_string(
                &stats,
                [x, y],
                font_size,
                [1.0, 1.0, 1.0, 1.0], // color white
                [0.0, 0.0, 0.0, 1.0], // outline color black
                sow_render::TmpFontSettings::default(),
                1.0, // align right (align_x = 1.0)
                1.0, // char_spacing
                1.0, // emoji_scale
            );
        }
    }
}
