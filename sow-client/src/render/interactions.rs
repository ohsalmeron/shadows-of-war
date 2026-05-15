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
    pub(crate) fn handle_map_interactions(&mut self, ctx: &egui::Context) {
                                    // Check long press
                                    if let Some((start, mx, my)) = self.map_touch_start {
                                        if start.elapsed().as_millis() > 500 {
                                            let world_x = (mx as f32 - self.camera_x) / self.camera_zoom;
                                            let world_y = (my as f32 - self.camera_y) / self.camera_zoom;
                                            let col = world_x.floor() as i32;
                                            let row = world_y.floor() as i32;
                                            if col >= 0 && row >= 0 && col < self.map_w as i32 && row < self.map_h as i32 {
                                                let idx = (row * self.map_w as i32 + col) as u32;
                                                self.map_context_menu = Some((mx as f32, my as f32, idx));
                                            }
                                            self.map_touch_start = None; // clear it so it doesn't re-trigger
                                        }
                                    }

                                    if let Some((mx, my, tile_idx)) = self.map_context_menu {
                                        let terrain_byte = self.map_renderer.as_ref().map(|mr| mr.terrain[tile_idx as usize]).unwrap_or(0);
                                        let is_land = (terrain_byte & 0x80) != 0;
                                        
                                        egui::Area::new(egui::Id::new("map_context_menu"))
                                            .anchor(egui::Align2::LEFT_TOP, egui::vec2(mx, my))
                                            .order(egui::Order::Foreground)
                                            .show(ctx, |ui| {
                                                egui::Frame::menu(&ctx.style()).show(ui, |ui| {
                                                    if is_land {
                                                        ui.label("Land Tile");
                                                    } else {
                                                        if ui.button("★ Send Fleet").clicked() {
                                                            let troops = Some(self.app.hud_state.troops * (self.app.hud_state.attack_ratio as f64));
                                                            let intent = sow_core::protocol::GameplayIntent::LaunchFleet {
                                                                target_tile: tile_idx,
                                                                troops,
                                                            };
                                                            if let Some(c) = self.net_client.as_ref() {
                                                                if let Ok(json) = bincode::serialize(&sow_core::protocol::ClientMessage::Gameplay { intent: intent.clone() }) {
                                                                    c.send(json);
                                                                }
                                                            } else {
                                                                let stamped = sow_core::protocol::StampedIntent { player_id: self.my_player_id.unwrap_or(1), intent };
                                                                self.bridge.send_command(sow_core::protocol::SimCommand::Turn(sow_core::protocol::Turn { turn_number: 0, intents: vec![stamped] }));
                                                            }
                                                            self.map_context_menu = None;
                                                        }
                                                    }
                                                    if ui.button("[X] Cancel").clicked() {
                                                        self.map_context_menu = None;
                                                    }
                                                });
                                            });
                                            
                                        // Auto-close if clicked elsewhere
                                        if ctx.input(|i| i.pointer.any_pressed()) && !ctx.egui_wants_pointer_input() {
                                            self.map_context_menu = None;
                                        }
                                    }

    }

    pub(crate) fn process_ui_actions(&mut self, ctx: &egui::Context, _sf: f32, _local_cancel_intents: &mut Vec<sow_core::protocol::GameplayIntent>) {
                                if let Some(action) = self.app.draw(ctx) {
                                    match action {
                                        UiAction::StartSinglePlayer => {
                                            self.app.phase = ClientPhase::Playing;
                                        }
                                        UiAction::ConnectToServer(addr) => {
                                            self.app.main_menu_state.is_connecting = true;
                                            let url = addr.clone();
                                            #[cfg(target_arch = "wasm32")]
                                            spawn_sow_client_connect(url, &self.connect_tx);
                                            #[cfg(not(target_arch = "wasm32"))]
                                            spawn_sow_client_connect(url, &self.connect_tx, &self.tokio_rt);
                                        }
                                        UiAction::JoinLobby(id) => {
                                            let join_msg = sow_core::protocol::ClientMessage::Join {
                                                name: self.app.main_menu_state.player_name.clone(),
                                                is_observer: false,
                                                target_lobby_id: Some(id),
                                                build_version: get_build_version(),
                                            };
                                            self.app.main_menu_state.pending_join_lobby_id = Some(id);
                                            if let Ok(json) = bincode::serialize(&join_msg) {
                                                if let Some(c) = self.net_client.as_ref() {
                                                    c.send(json);
                                                }
                                            }
                                            self.app.main_menu_state.is_waiting = true;
                                        }
                                        UiAction::LeaveLobby => {
                                            if let Some(c) = self.net_client.as_ref() {
                                                let leave = sow_core::protocol::ClientMessage::Leave {};
                                                if let Ok(json) = bincode::serialize(&leave) {
                                                    c.send(json);
                                                }
                                            }
                                            self.app.hud_state.connection_lost = false;
                                            self.app.main_menu_state.is_waiting = false;
                                            self.app.main_menu_state.pending_join_lobby_id = None;
                                            self.app.main_menu_state.joined_lobby_id = None;
                                            self.my_lobby_id = None;
                                            self.my_player_id = None;
                                            self.camera_x = 0.0;
                                            self.camera_y = 0.0;
                                            self.camera_zoom = 2.0;
                                            self.app.phase = ClientPhase::Splash;
                                            self.app.splash_state.job = sow_ui::ui::loading_screen::SplashJob::ExitGame;
                                            self.app.splash_state.gpu_load_step = 0;
                                            self.app.splash_state.frames_drawn = 0;
                                        }
                                        UiAction::SetAttackRatio(r) => {
                                            self.app.hud_state.attack_ratio = r;
                                        }
                                        UiAction::CenterCamera => {
                                            let pid = self.my_player_id.unwrap_or(1);
                                            if let Some(player) =
                                                self.current_snapshot.as_ref().and_then(|s| s.players.iter().find(|p| p.id == pid))
                                            {
                                                if player.tile_count > 0 && player.alive {
                                                    let cx = player.centroid_x;
                                                    let cy = player.centroid_y;
                                                    
                                                    let world_cx = cx + 0.5;
                                                    let world_cy = cy + 0.5;

                                                    self.camera_x = self.screen_w * 0.5 - world_cx * self.camera_zoom;
                                                    self.camera_y = self.screen_h * 0.5 - world_cy * self.camera_zoom;
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }

    }
}
