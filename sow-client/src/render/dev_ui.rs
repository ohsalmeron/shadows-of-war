#![allow(unused_imports)]
use sow_render::{RenderContext, MapRenderer, MapGlobals};
use crate::sim_bridge::{SimBridge, PlatformSimBridge};
use sow_core::protocol::{SimCommand, SimSnapshot};

use sow_core::game_config::GameConfig;

use blade_egui::GuiPainter;
use egui::{Context, RawInput, Pos2, Rect, Vec2};
use sow_ui::{ClientApp, app::ClientPhase, UiAction};
use web_time::{Instant, Duration};
use sow_net::client::SowClient;
use std::collections::HashMap;
use crate::{CAMERA_MIN_ZOOM, camera_zoom_upper_bound, NAMEPLATE_REFERENCE_ZOOM};
use crate::{spawn_sow_client_connect, get_build_version, get_maps_url};
use crate::nameplates::*;
use crate::client_config::ClientVisualConfig;
use crate::{MapDownloadEvent, EngineInitEvent};
use winit::event::{WindowEvent, MouseButton, ElementState, MouseScrollDelta};

use blade_graphics as gpu;
use crate::app_state::SowApp;
use std::io::Read;



impl SowApp {
    pub(crate) fn calculate_fps_and_ping(&mut self) {
                                self.frame_count += 1;
                                if self.last_fps_time.elapsed().as_secs_f64() >= 1.0 {
                                    self.current_fps = self.frame_count;
                                    self.frame_count = 0;
                                    self.last_fps_time = Instant::now();
                                }

                                if self.last_ping_time.elapsed().as_secs_f64() >= 1.0 {
                                    if let Some(c) = self.net_client.as_ref() {
                                        let ping_msg = sow_core::protocol::ClientMessage::Ping {
                                            client_time: self.start_time.elapsed().as_secs_f64(),
                                        };
                                        if let Ok(json) = bincode::serialize(&ping_msg) {
                                            c.send(json);
                                        }
                                    }
                                    self.last_ping_time = Instant::now();
                                }

    }

    pub(crate) fn render_dev_panels(&mut self, ctx: &egui::Context, local_cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>) {
                                    egui::Area::new(egui::Id::new("fps_counter"))
                                        .anchor(egui::Align2::LEFT_TOP, egui::vec2(10.0, 10.0))
                                        .show(ctx, |ui| {
                                            ui.horizontal(|ui| {
                                                if let Some(ping) = self.current_ping_ms {
                                                    ui.label(
                                                        egui::RichText::new(format!("Ping: {}ms", ping))
                                                            .color(egui::Color32::WHITE)
                                                            .strong()
                                                    );
                                                }
                                                ui.label(
                                                    egui::RichText::new(format!("FPS: {}", self.current_fps))
                                                        .color(egui::Color32::YELLOW)
                                                        .strong()
                                                );
                                                ui.label(
                                                    egui::RichText::new(format!("Zoom: {:.2}", self.camera_zoom))
                                                        .color(egui::Color32::LIGHT_BLUE)
                                                        .strong()
                                                );
                                            });
                                        });

                                    let mut is_expanded = ctx.data_mut(|d| *d.get_temp_mut_or_insert_with(egui::Id::new("dev_utils_expanded"), || false));

                                    egui::Window::new("LOD Dev Utils")
                                        .title_bar(false)
                                        .resizable(false)
                                        .show(ctx, |ui| {
                                            ui.horizontal(|ui| {
                                                let icon = if is_expanded { "▼" } else { "▶" };
                                                let btn = egui::Button::new(egui::RichText::new(icon).size(24.0)).min_size(egui::vec2(36.0, 36.0));
                                                if ui.add(btn).clicked() {
                                                    is_expanded = !is_expanded;
                                                    ctx.data_mut(|d| d.insert_temp(egui::Id::new("dev_utils_expanded"), is_expanded));
                                                }
                                                ui.label(egui::RichText::new("Dev Utils").strong().size(18.0));
                                            });

                                            if is_expanded {
                                                ui.separator();
                                            }
                                        });

                                if self.app.phase == ClientPhase::Playing {
                                    if let Some(snap) = &self.current_snapshot {
                                        let my_pid = self.my_player_id.unwrap_or(0);
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
}
