use sow_ui::{app::ClientPhase, UiAction};
use crate::{spawn_sow_client_connect, get_build_version};
use crate::app::SowApp;

impl SowApp {
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

                    let config = sow_core::game_config::GameConfig {
                        map_name: map_name.clone(),
                        map_width: 1000,
                        map_height: 800,
                        bot_count: 2,
                        nation_count: 1,
                        ..Default::default()
                    };

                    let start_msg = sow_core::protocol::ServerStartMessage {
                        lobby_id: None,
                        config,
                        my_player_id: Some(1),
                        seed: 42,
                        players: vec![
                            sow_core::protocol::PlayerInfo {
                                id: 1,
                                name: {
                                    let name = &self.ui.app.main_menu_state.player_name;
                                    let tag = &self.ui.app.main_menu_state.clan_tag;
                                    if tag.is_empty() { name.clone() } else { format!("[{}] {}", tag, name) }
                                },
                                color: sow_core::player::human_shader_territory_rgb(1),
                                player_type: sow_core::player::PlayerType::Human,
                                team: None,
                                spawn_x: 0,
                                spawn_y: 0,
                                civilization: self.ui.app.main_menu_state.selected_civilization,
                                leader: self.ui.app.main_menu_state.selected_leader,
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

                    let map_id = config.map_name.to_lowercase().replace("_", "");
                    self.ui.app.main_menu_state.downloading_map_name = Some(map_id.clone());

                    let mut config = *config;
                    config.map_name = map_id.clone();
                    if let Some(man) = self.ui.app.asset_loader.manifests.get(&map_id) {
                        config.map_width = man.map.width;
                        config.map_height = man.map.height;
                    } else if map_id == "world" {
                        config.map_width = 2000;
                        config.map_height = 1000;
                    } else if map_id == "giantworldmap" {
                        config.map_width = 4108;
                        config.map_height = 1948;
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
                                name: {
                                    let name = &self.ui.app.main_menu_state.player_name;
                                    let tag = &self.ui.app.main_menu_state.clan_tag;
                                    if tag.is_empty() { name.clone() } else { format!("[{}] {}", tag, name) }
                                },
                                color: sow_core::player::human_shader_territory_rgb(1),
                                player_type: sow_core::player::PlayerType::Human,
                                team: None,
                                spawn_x: 0,
                                spawn_y: 0,
                                civilization: self.ui.app.main_menu_state.selected_civilization,
                                leader: self.ui.app.main_menu_state.selected_leader,
                            }
                        ],
                        missed_turns: vec![],
                        map_data: None,
                        relay_port: None,
                        nations: None,
                    };
                    self.tasks.engine_init_queued_msg = Some(start_msg);

                    if self.ui.app.asset_loader.has_map(&map_id) {
                        self.ui.app.main_menu_state.cached_map = self.ui.app.asset_loader.take_map(&map_id);
                        self.ui.app.main_menu_state.is_downloading_map = false;
                    } else {
                        self.ui.app.main_menu_state.is_downloading_map = true;
                        self.ui.app.main_menu_state.cached_map = None;
                        let maps_base = crate::get_maps_url();
                        let map_name_clone = map_id.clone();
                        let tx_man = self.tasks.map_tx.clone();
                        
                        // 1. Fetch manifest.json
                        let manifest_url = format!("{}/{}/manifest.json", maps_base.trim_end_matches('/'), map_id);
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
                        let url = format!("{}/{}/map.bin.br", maps_base.trim_end_matches('/'), map_id);
                        let tx = self.tasks.map_tx.clone();
                        let request = ehttp::Request::get(&url);
                        let map_name_for_closure = map_id.clone();
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
                        clan_tag: self.ui.app.main_menu_state.clan_tag.clone(),
                        civilization: self.ui.app.main_menu_state.selected_civilization,
                        leader: self.ui.app.main_menu_state.selected_leader,
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
                    self.begin_exit_to_main_menu(false);
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
                            
                            let world_cx = cx + 0.5 + (cy as i32 % 2) as f32 * 0.5;
                            let world_cy = (cy + 0.5) * 0.8660254_f32;

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
                    let window = self.gfx.window.take().expect("No window to handoff to editor");
                    let surface = self.gfx.surface.take().expect("No surface to handoff to editor");
                    let render_ctx = std::mem::take(&mut self.gfx.render_ctx);
                    let gui_painter = self.gfx.gui_painter.take().expect("No gui_painter to handoff to editor");
                    let egui_ctx = self.ui.egui_ctx.clone();
                    let client_app = std::mem::take(&mut self.ui.app);

                    let session = sow_map::MapEditorSession::new(
                        window,
                        surface,
                        render_ctx,
                        gui_painter,
                        egui_ctx,
                        client_app,
                    );
                    self.map_editor = Some(session);
                }
            }
        }
    }
}
