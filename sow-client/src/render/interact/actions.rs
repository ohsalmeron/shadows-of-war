use crate::app::SowApp;
use crate::{get_build_version, spawn_sow_client_connect};
use sow_ui::UiAction;

impl SowApp {
    pub(crate) fn process_ui_actions(
        &mut self,
        _ctx: &egui::Context,
        action: Option<sow_ui::UiAction>,
    ) {
        if let Some(action) = action {
            match action {
                UiAction::StartSinglePlayer(config) => {
                    self.net.is_offline = true;
                    self.sim.offline_tick_timer = 0.0;
                    self.sim.offline_last_update = web_time::Instant::now();
                    self.net.client = None;
                    self.begin_enter_game_loader();
                    self.sim.my_player_id = Some(1);
                    self.sim.my_lobby_id = Some(0);

                    let map_id =
                        sow_ui::ui::asset_loader::AssetLoader::map_key(&config.map_name);
                    self.ui.app.main_menu_state.downloading_map_name = Some(map_id.clone());

                    let mut config = *config;
                    config.map_name = map_id.clone();
                    if let Some(catalog) = &self.ui.app.asset_loader.map_catalog {
                        if let Some(entry) = sow_core::maps::catalog_lookup(catalog, &map_id) {
                            config.map_width = entry.width;
                            config.map_height = entry.height;
                            config.map_name = entry.key.clone();
                        } else {
                            log::warn!("Map '{}' not in catalog.bin", map_id);
                        }
                    }
                    if let Some(payload) = sow_core::maps::load_map_br_payload(
                        &map_id,
                        crate::map_cache::load(&map_id),
                    ) {
                        if let Ok(map_file) =
                            sow_core::maps::load_map_from_payload(&payload)
                        {
                            config.map_width = map_file.width;
                            config.map_height = map_file.height;
                            self.ui
                                .app
                                .asset_loader
                                .maps
                                .insert(map_id.clone(), payload);
                        }
                    }

                    let start_msg = sow_core::protocol::ServerStartMessage {
                        lobby_id: None,
                        config: config.clone(),
                        my_player_id: Some(1),
                        seed: config.seed,
                        players: vec![sow_core::protocol::PlayerInfo {
                            id: 1,
                            name: {
                                let name = &self.ui.app.main_menu_state.player_name;
                                let tag = &self.ui.app.main_menu_state.clan_tag;
                                if tag.is_empty() {
                                    name.clone()
                                } else {
                                    format!("[{}] {}", tag, name)
                                }
                            },
                            color: self
                                .ui
                                .app
                                .main_menu_state
                                .selected_leader
                                .filler_rgb(),
                            player_type: sow_core::player::PlayerType::Human,
                            team: None,
                            spawn_x: 0,
                            spawn_y: 0,
                            civilization: self.ui.app.main_menu_state.selected_civilization,
                            leader: self.ui.app.main_menu_state.selected_leader,
                        }],
                        missed_turns: vec![],
                        map_data: None,
                        relay_port: None,
                    };
                    self.tasks.engine_init_queued_msg = Some(start_msg);

                    if self.ui.app.asset_loader.has_map(&map_id) {
                        self.ui.app.main_menu_state.cached_map =
                            self.ui.app.asset_loader.take_map(&map_id);
                        self.ui.app.main_menu_state.cached_map_key = Some(map_id.clone());
                        self.ui.app.main_menu_state.is_downloading_map = false;
                    } else {
                        self.ui.app.main_menu_state.is_downloading_map = true;
                        self.ui.app.main_menu_state.cached_map = None;
                        self.ui.app.main_menu_state.cached_map_key = None;
                        let maps_base = self.asset_config.maps_base.clone();
                        let url =
                            format!("{}/{}/map.bin.br", maps_base.trim_end_matches('/'), map_id);
                        let tx = self.tasks.map_tx.clone();
                        let request = ehttp::Request::get(&url);
                        let map_name_for_closure = map_id.clone();
                        let accumulated = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
                        let total_bytes = std::sync::Arc::new(std::sync::Mutex::new(0usize));

                        ehttp::streaming::fetch(
                            request,
                            move |result: ehttp::Result<ehttp::streaming::Part>| match result {
                                Ok(ehttp::streaming::Part::Response(res)) => {
                                    if !res.ok {
                                        let _ = tx.send(crate::MapDownloadEvent::Error(format!(
                                            "HTTP Error: {}",
                                            res.status
                                        )));
                                        return std::ops::ControlFlow::Break(());
                                    }
                                    let cl = res
                                        .headers
                                        .get("content-length")
                                        .or_else(|| res.headers.get("Content-Length"));
                                    if let Some(cl_str) = cl {
                                        if let Ok(b) = cl_str.parse::<usize>() {
                                            *total_bytes.lock().unwrap() = b;
                                        }
                                    }
                                    std::ops::ControlFlow::Continue(())
                                }
                                Ok(ehttp::streaming::Part::Chunk(chunk)) => {
                                    if chunk.is_empty() {
                                        let final_bytes =
                                            std::mem::take(&mut *accumulated.lock().unwrap());
                                        let _ = tx.send(crate::MapDownloadEvent::MapReady(
                                            map_name_for_closure.clone(),
                                            final_bytes,
                                        ));
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
                                    let _ = tx.send(crate::MapDownloadEvent::Progress(
                                        map_name_for_closure.clone(),
                                        pct,
                                    ));
                                    std::ops::ControlFlow::Continue(())
                                }
                                Err(e) => {
                                    let _ = tx.send(crate::MapDownloadEvent::Error(e.to_string()));
                                    std::ops::ControlFlow::Break(())
                                }
                            },
                        );
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
                UiAction::RetryConnection => {
                    self.ui.app.main_menu_state.error_message = None;
                    self.ui.app.main_menu_state.is_connecting = true;
                    let url = self.ui.app.main_menu_state.server_address.clone();
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
                    if let Some(player) = self
                        .sim
                        .current_snapshot
                        .as_ref()
                        .and_then(|s| s.players.iter().find(|p| p.id == pid))
                    {
                        if player.tile_count > 0 && player.alive {
                            let cx = player.centroid_x;
                            let cy = player.centroid_y;

                            let world_cx = cx + 0.5 + (cy as i32 % 2) as f32 * 0.5;
                            let world_cy = (cy + 0.5) * 0.8660254_f32;

                            self.input.camera_x =
                                self.input.screen_w * 0.5 - world_cx * self.input.camera_zoom;
                            self.input.camera_y =
                                self.input.screen_h * 0.5 - world_cy * self.input.camera_zoom;
                        }
                    }
                }
                UiAction::FocusTile(col, row) => {
                    let world_cx = col + 0.5 + (row as i32 % 2) as f32 * 0.5;
                    let world_cy = (row + 0.5) * 0.8660254_f32;

                    // Zoom in to a comfortable battle-focus level
                    let target_zoom = 3.0_f32.max(self.input.camera_zoom);
                    self.input.camera_zoom = target_zoom;
                    self.input.camera_x = self.input.screen_w * 0.5 - world_cx * target_zoom;
                    self.input.camera_y = self.input.screen_h * 0.5 - world_cy * target_zoom;
                }
                UiAction::ToggleDevSidebar => {
                    self.ui.show_dev_sidebar = !self.ui.show_dev_sidebar;
                }
                UiAction::ToggleSettings => {
                    // Handle settings toggle if it's there
                }
                UiAction::ToggleCredits => {
                    self.ui.app.is_credits_open = !self.ui.app.is_credits_open;
                }
                UiAction::TogglePrivacy => {
                    self.ui.app.is_privacy_open = !self.ui.app.is_privacy_open;
                }
                UiAction::ToggleTerms => {
                    self.ui.app.is_terms_open = !self.ui.app.is_terms_open;
                }
                UiAction::ZoomIn => {
                    self.process_camera_zoom(
                        1.25,
                        self.input.screen_w * 0.5,
                        self.input.screen_h * 0.5,
                    );
                }
                UiAction::ZoomOut => {
                    self.process_camera_zoom(
                        0.8,
                        self.input.screen_w * 0.5,
                        self.input.screen_h * 0.5,
                    );
                }
            }
        }
    }
}
