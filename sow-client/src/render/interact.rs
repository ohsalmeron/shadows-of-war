


use sow_ui::{app::ClientPhase, UiAction};
use crate::{spawn_sow_client_connect, get_build_version};

use crate::app::SowApp;



impl SowApp {
    pub(crate) fn handle_map_interactions(&mut self, ctx: &egui::Context) {
        if self.sim.current_snapshot.as_ref().is_some_and(|s| s.winner.is_some()) {
            return;
        }

        if self.ui.app.main_menu_state.is_waiting {
            return;
        }

        // ── Hold-to-Attack pump: sends 10% of troops per second while held ──
        if let Some((target_owner, press_start, sx, sy, has_fired_initial)) = self.input.hold_attack_target {
            let held_ms = press_start.elapsed().as_millis();
            // Only start streaming after 300ms grace period (to distinguish from quick-click)
            if held_ms > 300 {
                // Check cursor hasn't drifted too far from press origin
                let dx = self.input.last_mouse_x - sx;
                let dy = self.input.last_mouse_y - sy;
                if dx * dx + dy * dy <= 2500.0 {
                    if !has_fired_initial {
                        // Mobile hold threshold reached -> fire initial burst
                        self.input.hold_attack_target = Some((target_owner, press_start, sx, sy, true));
                        self.input.hold_attack_accum = 0.0;
                        
                        let troops = self.ui.app.hud_state.troops * (self.ui.app.hud_state.attack_ratio as f64);
                        if troops > 0.0 {
                            let attack = sow_core::protocol::AttackIntent {
                                target_owner,
                                troops: Some(troops),
                            };
                            let intent = sow_core::protocol::GameplayIntent::Attack(attack);
                            if let Some(c) = self.net.client.as_ref() {
                                if let Ok(json) = bincode::serialize(&sow_core::protocol::ClientMessage::Gameplay { intent: intent.clone() }) {
                                    c.send(json);
                                }
                            } else {
                                self.sim.offline_intents.push(intent);
                            }
                        }
                    } else {
                        // Accumulate real time since last pump
                        let dt = ctx.input(|i| i.predicted_dt);
                        self.input.hold_attack_accum += dt;

                        // Send one attack every 250ms
                        while self.input.hold_attack_accum >= 0.25 {
                            self.input.hold_attack_accum -= 0.25;
                            // 25% of the bar settings (bar is attack_ratio)
                            let ratio_per_tick = (self.ui.app.hud_state.attack_ratio as f64) * 0.25;
                            let troops = self.ui.app.hud_state.troops * ratio_per_tick;
                            if troops > 0.0 {
                                let attack = sow_core::protocol::AttackIntent {
                                    target_owner,
                                    troops: Some(troops),
                                };
                                let intent = sow_core::protocol::GameplayIntent::Attack(attack);
                                if let Some(c) = self.net.client.as_ref() {
                                    if let Ok(json) = bincode::serialize(&sow_core::protocol::ClientMessage::Gameplay { intent: intent.clone() }) {
                                        c.send(json);
                                    }
                                } else {
                                    self.sim.offline_intents.push(intent);
                                }
                            }
                        }
                    }
                } else {
                    // Drifted too far, cancel hold
                    self.input.hold_attack_target = None;
                    self.input.hold_attack_accum = 0.0;
                }
            }
        }

        // ── Context menu (right-click on desktop, tap on mobile) ──
        if let Some((mx, my, tile_idx)) = self.input.map_context_menu {
            let terrain_byte = self.gfx.map_renderer.as_ref().map(|mr| mr.terrain[tile_idx as usize]).unwrap_or(0);
            let is_land = (terrain_byte & 0x80) != 0;
            
            egui::Area::new(egui::Id::new("map_context_menu"))
                .anchor(egui::Align2::LEFT_TOP, egui::vec2(mx, my))
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::menu(&ctx.global_style())
                        .fill(sow_ui::ui::theme::panel_bg())
                        .stroke(egui::Stroke::new(1.0_f32, sow_ui::ui::theme::nickname_field_border()))
                        .corner_radius(12.0)
                        .inner_margin(8.0)
                        .show(ui, |ui| {
                        if is_land {
                            ui.label("Land Tile");
                        } else {
                            if ui.button("★ Send Fleet").clicked() {
                                let troops = Some(self.ui.app.hud_state.troops * (self.ui.app.hud_state.attack_ratio as f64));
                                let intent = sow_core::protocol::GameplayIntent::LaunchFleet {
                                    target_tile: tile_idx,
                                    troops,
                                };
                                if let Some(c) = self.net.client.as_ref() {
                                    if let Ok(json) = bincode::serialize(&sow_core::protocol::ClientMessage::Gameplay { intent: intent.clone() }) {
                                        c.send(json);
                                    }
                                } else {
                                    self.sim.offline_intents.push(intent);
                                }
                                self.input.map_context_menu = None;
                            }
                        }
                        if ui.button("[X] Cancel").clicked() {
                            self.input.map_context_menu = None;
                        }
                    });
                });
                
            // Auto-close if clicked elsewhere
            if ctx.input(|i| i.pointer.any_pressed()) && !ctx.egui_wants_pointer_input() {
                self.input.map_context_menu = None;
            }
        }

    }

    pub(crate) fn process_ui_actions(&mut self, _ctx: &egui::Context, action: Option<sow_ui::UiAction>) {
                                if let Some(action) = action {
                                    match action {
                                        UiAction::StartTutorial => {
                                            self.net.is_offline = true;
                                            self.sim.offline_tick_timer = 0.0;
                                            self.net.client = None;
                                            self.ui.app.phase = ClientPhase::Splash;
                                            self.ui.app.splash_state.job = sow_ui::ui::loading_screen::SplashJob::EnterGame;
                                            self.ui.app.splash_state.frames_drawn = 0;
                                            self.ui.app.splash_state.gpu_load_step = 0;
                                            self.sim.my_player_id = Some(1);
                                            self.sim.my_lobby_id = Some(0);

                                            self.ui.tutorial_completed = false;
                                            self.ui.tutorial_step = crate::hud::tutorial::TutorialStep::Welcome;
                                            #[cfg(target_arch = "wasm32")]
                                            if let Some(window) = web_sys::window() {
                                                if let Ok(Some(storage)) = window.local_storage() {
                                                    let _ = storage.remove_item("sow_tutorial_completed");
                                                }
                                            }
                                            #[cfg(not(target_arch = "wasm32"))]
                                            let _ = std::fs::remove_file("sow_tutorial_completed.txt");

                                            let map_name = "tutorial".to_string();
                                            self.ui.app.main_menu_state.downloading_map_name = Some(map_name.clone());

                                            let mut config = sow_core::game_config::GameConfig::default();
                                            config.map_name = map_name.clone();
                                            config.map_width = 800;
                                            config.map_height = 600;
                                            config.bot_count = 2;
                                            config.nation_count = 1;

                                            let start_msg = sow_core::protocol::ServerStartMessage {
                                                lobby_id: None,
                                                config,
                                                my_player_id: Some(1),
                                                seed: 42,
                                                players: vec![
                                                    sow_core::protocol::PlayerInfo {
                                                        id: 1,
                                                        name: self.ui.app.main_menu_state.player_name.clone(),
                                                        color: sow_core::player::human_shader_territory_rgb(1),
                                                        player_type: sow_core::player::PlayerType::Human,
                                                        team: None,
                                                        spawn_x: 0,
                                                        spawn_y: 0,
                                                    }
                                                ],
                                                missed_turns: vec![],
                                                map_data: None,
                                                relay_port: None,
                                                nations: None,
                                            };
                                            self.tasks.engine_init_queued_msg = Some(start_msg);

                                            if self.ui.app.asset_loader.has_map(&map_name) {
                                                self.ui.app.main_menu_state.cached_map = self.ui.app.asset_loader.take_map(&map_name);
                                                self.ui.app.main_menu_state.is_downloading_map = false;
                                            } else {
                                                self.ui.app.main_menu_state.is_downloading_map = true;
                                                self.ui.app.main_menu_state.cached_map = None;
                                                let maps_base = crate::get_maps_url();
                                                let url = format!("{}/{}/map.bin.br", maps_base.trim_end_matches('/'), map_name);
                                                let tx = self.tasks.map_tx.clone();
                                                
                                                let request = ehttp::Request::get(&url);
                                                let map_name_for_closure = map_name.clone();
                                                let accumulated = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
                                                let total_bytes = std::sync::Arc::new(std::sync::Mutex::new(0usize));
                                                
                                                ehttp::streaming::fetch(request, move |result: ehttp::Result<ehttp::streaming::Part>| {
                                                    match result {
                                                        Ok(ehttp::streaming::Part::Response(res)) => {
                                                            if !res.ok {
                                                                let _ = tx.send(crate::MapDownloadEvent::Error(format!("HTTP Error: {}", res.status)));
                                                                return std::ops::ControlFlow::Break(());
                                                            }
                                                            let cl = res.headers.get("content-length").or_else(|| res.headers.get("Content-Length"));
                                                            if let Some(cl_str) = cl {
                                                                if let Ok(b) = cl_str.parse::<usize>() {
                                                                    *total_bytes.lock().unwrap() = b;
                                                                }
                                                            }
                                                            std::ops::ControlFlow::Continue(())
                                                        }
                                                        Ok(ehttp::streaming::Part::Chunk(chunk)) => {
                                                            if chunk.is_empty() {
                                                                let final_bytes = std::mem::take(&mut *accumulated.lock().unwrap());
                                                                let _ = tx.send(crate::MapDownloadEvent::MapReady(map_name_for_closure.clone(), final_bytes));
                                                                return std::ops::ControlFlow::Break(());
                                                            }
                                                            let mut acc = accumulated.lock().unwrap();
                                                            acc.extend_from_slice(&chunk);
                                                            let total = *total_bytes.lock().unwrap();
                                                            let pct = if total > 0 {
                                                                ((acc.len() as f64 / total as f64) * 100.0) as u8
                                                            } else {
                                                                0
                                                            };
                                                            let _ = tx.send(crate::MapDownloadEvent::Progress(map_name_for_closure.clone(), pct));
                                                            std::ops::ControlFlow::Continue(())
                                                        }
                                                        Err(e) => {
                                                            let _ = tx.send(crate::MapDownloadEvent::Error(e.to_string()));
                                                            std::ops::ControlFlow::Break(())
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                        UiAction::StartSinglePlayer(config) => {
                                            self.net.is_offline = true;
                                            self.sim.offline_tick_timer = 0.0;
                                            self.net.client = None;
                                            self.ui.app.phase = ClientPhase::Splash;
                                            self.ui.app.splash_state.job = sow_ui::ui::loading_screen::SplashJob::EnterGame;
                                            self.ui.app.splash_state.frames_drawn = 0;
                                            self.ui.app.splash_state.gpu_load_step = 0;
                                            self.sim.my_player_id = Some(1);
                                            self.sim.my_lobby_id = Some(0);

                                            let map_name = config.map_name.clone();
                                            self.ui.app.main_menu_state.downloading_map_name = Some(map_name.clone());

                                            // TODO: dynamically parse width/height if we had map metadata
                                            let mut config = *config;
                                            if config.map_name == "world" {
                                                config.map_width = 2000;
                                                config.map_height = 1000;
                                            } else {
                                                config.map_width = 800;
                                                config.map_height = 400;
                                            }

                                            let start_msg = sow_core::protocol::ServerStartMessage {
                                                lobby_id: None,
                                                config,
                                                my_player_id: Some(1),
                                                seed: 42,
                                                players: vec![
                                                    sow_core::protocol::PlayerInfo {
                                                        id: 1,
                                                        name: self.ui.app.main_menu_state.player_name.clone(),
                                                        color: sow_core::player::human_shader_territory_rgb(1),
                                                        player_type: sow_core::player::PlayerType::Human,
                                                        team: None,
                                                        spawn_x: 0,
                                                        spawn_y: 0,
                                                    }
                                                ],
                                                missed_turns: vec![],
                                                map_data: None,
                                                relay_port: None,
                                                nations: None,
                                            };
                                            self.tasks.engine_init_queued_msg = Some(start_msg);

                                            if self.ui.app.asset_loader.has_map(&map_name) {
                                                self.ui.app.main_menu_state.cached_map = self.ui.app.asset_loader.take_map(&map_name);
                                                self.ui.app.main_menu_state.is_downloading_map = false;
                                            } else {
                                                self.ui.app.main_menu_state.is_downloading_map = true;
                                                self.ui.app.main_menu_state.cached_map = None;
                                                let maps_base = crate::get_maps_url();
                                                let map_name_clone = map_name.clone();
                                                let tx_man = self.tasks.map_tx.clone();
                                                
                                                // 1. Fetch manifest.json
                                                let manifest_url = format!("{}/{}/manifest.json", maps_base.trim_end_matches('/'), map_name);
                                                let request_man = ehttp::Request::get(&manifest_url);
                                                ehttp::fetch(request_man, move |result| {
                                                    if let Ok(res) = result {
                                                        if res.ok {
                                                            if let Ok(manifest) = serde_json::from_slice::<sow_core::map_legacy::MapManifest>(&res.bytes) {
                                                                let _ = tx_man.send(crate::MapDownloadEvent::ManifestReady(map_name_clone, manifest));
                                                            }
                                                        }
                                                    }
                                                });

                                                // 2. Fetch map.bin.br
                                                let url = format!("{}/{}/map.bin.br", maps_base.trim_end_matches('/'), map_name);
                                                let tx = self.tasks.map_tx.clone();
                                                let request = ehttp::Request::get(&url);
                                                let map_name_for_closure = map_name.clone();
                                                let accumulated = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
                                                let total_bytes = std::sync::Arc::new(std::sync::Mutex::new(0usize));
                                                
                                                ehttp::streaming::fetch(request, move |result: ehttp::Result<ehttp::streaming::Part>| {
                                                    match result {
                                                        Ok(ehttp::streaming::Part::Response(res)) => {
                                                            if !res.ok {
                                                                let _ = tx.send(crate::MapDownloadEvent::Error(format!("HTTP Error: {}", res.status)));
                                                                return std::ops::ControlFlow::Break(());
                                                            }
                                                            let cl = res.headers.get("content-length").or_else(|| res.headers.get("Content-Length"));
                                                            if let Some(cl_str) = cl {
                                                                if let Ok(b) = cl_str.parse::<usize>() {
                                                                    *total_bytes.lock().unwrap() = b;
                                                                }
                                                            }
                                                            std::ops::ControlFlow::Continue(())
                                                        }
                                                        Ok(ehttp::streaming::Part::Chunk(chunk)) => {
                                                            if chunk.is_empty() {
                                                                let final_bytes = std::mem::take(&mut *accumulated.lock().unwrap());
                                                                let _ = tx.send(crate::MapDownloadEvent::MapReady(map_name_for_closure.clone(), final_bytes));
                                                                return std::ops::ControlFlow::Break(());
                                                            }
                                                            let mut acc = accumulated.lock().unwrap();
                                                            acc.extend_from_slice(&chunk);
                                                            let total = *total_bytes.lock().unwrap();
                                                            let pct = if total > 0 {
                                                                ((acc.len() as f64 / total as f64) * 100.0) as u8
                                                            } else {
                                                                0
                                                            };
                                                            let _ = tx.send(crate::MapDownloadEvent::Progress(map_name_for_closure.clone(), pct));
                                                            std::ops::ControlFlow::Continue(())
                                                        }
                                                        Err(e) => {
                                                            let _ = tx.send(crate::MapDownloadEvent::Error(e.to_string()));
                                                            std::ops::ControlFlow::Break(())
                                                        }
                                                    }
                                                });
                                            }
                                        }
                                        UiAction::ConnectToServer(addr) => {
                                            self.ui.app.main_menu_state.is_connecting = true;
                                            let url = addr.clone();
                                            #[cfg(target_arch = "wasm32")]
                                            spawn_sow_client_connect(url, &self.net.connect_tx);
                                            #[cfg(not(target_arch = "wasm32"))]
                                            spawn_sow_client_connect(url, &self.net.connect_tx, &self.tokio_rt);
                                        }
                                        UiAction::JoinLobby(id) => {
                                            let join_msg = sow_core::protocol::ClientMessage::Join {
                                                name: self.ui.app.main_menu_state.player_name.clone(),
                                                is_observer: false,
                                                target_lobby_id: Some(id),
                                                build_version: get_build_version(),
                                            };
                                            self.ui.app.main_menu_state.pending_join_lobby_id = Some(id);
                                            if let Ok(json) = bincode::serialize(&join_msg) {
                                                if let Some(c) = self.net.client.as_ref() {
                                                    c.send(json);
                                                }
                                            }
                                            self.ui.app.main_menu_state.is_waiting = true;
                                        }
                                        UiAction::LeaveLobby => {
                                            if let Some(c) = self.net.client.as_ref() {
                                                let leave = sow_core::protocol::ClientMessage::Leave {};
                                                if let Ok(json) = bincode::serialize(&leave) {
                                                    c.send(json);
                                                }
                                            }
                                            self.input.camera_x = 0.0;
                                            self.input.camera_y = 0.0;
                                            self.input.camera_zoom = 2.0;
                                            self.net.client = None;
                                            self.begin_exit_to_main_menu();
                                        }
                                        UiAction::SetAttackRatio(r) => {
                                            self.ui.app.hud_state.attack_ratio = r;
                                        }
                                        UiAction::CenterCamera => {
                                            let pid = self.sim.my_player_id.unwrap_or(1);
                                            if let Some(player) =
                                                self.sim.current_snapshot.as_ref().and_then(|s| s.players.iter().find(|p| p.id == pid))
                                            {
                                                if player.tile_count > 0 && player.alive {
                                                    let cx = player.centroid_x;
                                                    let cy = player.centroid_y;
                                                    
                                                    let world_cx = cx + 0.5;
                                                    let world_cy = cy + 0.5;

                                                    self.input.camera_x = self.input.screen_w * 0.5 - world_cx * self.input.camera_zoom;
                                                    self.input.camera_y = self.input.screen_h * 0.5 - world_cy * self.input.camera_zoom;
                                                }
                                            }
                                        }
                                        UiAction::ToggleDevSidebar => {
                                            self.ui.show_dev_sidebar = !self.ui.show_dev_sidebar;
                                        }
                                        UiAction::ToggleSettings => {
                                            // Handle settings toggle if it's there
                                        }
                                        UiAction::ZoomIn => {
                                            self.process_camera_zoom(1.25, self.input.screen_w * 0.5, self.input.screen_h * 0.5);
                                        }
                                        UiAction::ZoomOut => {
                                            self.process_camera_zoom(0.8, self.input.screen_w * 0.5, self.input.screen_h * 0.5);
                                        }
                                        UiAction::OpenMapEditor => {
                                            // Map editor is not available in the stable version
                                        }
                                    }
                                }

    }
}
