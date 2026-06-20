use crate::app::SowApp;
use crate::spawn_sow_client_connect;
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
                    self.start_offline_match(*config, false);
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
                    if self.ui.app.main_menu_state.is_waiting {
                        return;
                    }
                    let join_msg = self.make_join_message(Some(id), false, None, None);
                    self.ui.app.main_menu_state.pending_join_lobby_id = Some(id);
                    if let Ok(json) = bincode::serialize(&join_msg) {
                        if let Some(c) = self.net.client.as_ref() {
                            c.send(json);
                        }
                    }
                    self.ui.app.main_menu_state.show_join_browser = false;
                    self.ui.app.main_menu_state.is_waiting = true;
                }
                UiAction::HostPrivateLobby => {
                    let join_msg = self.make_join_message(None, true, None, None);
                    if let Ok(json) = bincode::serialize(&join_msg) {
                        if let Some(c) = self.net.client.as_ref() {
                            c.send(json);
                        }
                    }
                    self.ui.app.main_menu_state.is_waiting = true;
                    self.ui.app.main_menu_state.is_lobby_host = true;
                }
                UiAction::OpenCreateGame => {
                    self.ui.app.main_menu_state.show_create_game = true;
                }
                UiAction::OpenJoinBrowser => {
                    self.ui.app.main_menu_state.show_join_browser = true;
                }
                UiAction::CloseOverlay => {
                    self.ui.app.main_menu_state.show_create_game = false;
                    self.ui.app.main_menu_state.show_join_browser = false;
                }
                UiAction::CreateGame {
                    config,
                    is_private,
                    password,
                } => {
                    let join_msg = self.make_join_message(None, is_private, Some(config), password);
                    if let Ok(json) = bincode::serialize(&join_msg) {
                        if let Some(c) = self.net.client.as_ref() {
                            c.send(json);
                        }
                    }
                    self.ui.app.main_menu_state.is_waiting = true;
                    self.ui.app.main_menu_state.is_lobby_host = true;
                    self.ui.app.main_menu_state.show_create_game = false;
                }
                UiAction::JoinWithCode => {
                    let code = self
                        .ui
                        .app
                        .main_menu_state
                        .join_lobby_code
                        .trim()
                        .to_string();
                    if let Ok(lobby_id) = code.parse::<u64>() {
                        let join_msg = self.make_join_message(Some(lobby_id), false, None, None);
                        self.ui.app.main_menu_state.pending_join_lobby_id = Some(lobby_id);
                        if let Ok(json) = bincode::serialize(&join_msg) {
                            if let Some(c) = self.net.client.as_ref() {
                                c.send(json);
                            }
                        }
                        self.ui.app.main_menu_state.is_waiting = true;
                        self.ui.app.main_menu_state.show_join_browser = false;
                    }
                }
                UiAction::JoinWithPassword(lobby_id) => {
                    let password = self.ui.app.main_menu_state.join_password_input.clone();
                    let pw = if password.is_empty() {
                        None
                    } else {
                        Some(password)
                    };
                    let join_msg = self.make_join_message(Some(lobby_id), false, None, pw);
                    self.ui.app.main_menu_state.pending_join_lobby_id = Some(lobby_id);
                    if let Ok(json) = bincode::serialize(&join_msg) {
                        if let Some(c) = self.net.client.as_ref() {
                            c.send(json);
                        }
                    }
                    self.ui.app.main_menu_state.is_waiting = true;
                    self.ui.app.main_menu_state.show_join_browser = false;
                    self.ui.app.main_menu_state.join_password_for_lobby = None;
                    self.ui.app.main_menu_state.join_password_input.clear();
                }
                UiAction::LeaveLobby => {
                    crate::store_portals::left_room();
                    if let Some(c) = self.net.client.as_ref() {
                        let leave = sow_core::protocol::ClientMessage::Leave {};
                        if let Ok(json) = bincode::serialize(&leave) {
                            c.send(json);
                        }
                    }
                    self.ui.app.main_menu_state.in_private_match = false;
                    self.ui.app.main_menu_state.is_lobby_host = false;
                    self.ui.app.main_menu_state.my_player_id = None;
                    self.input.camera_x = 0.0;
                    self.input.camera_y = 0.0;
                    self.input.camera_zoom = 2.0;
                    self.input.target_zoom = 2.0;
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

                            let world_cx = cx + 0.5;
                            let world_cy = cy + 0.5;

                            self.input.camera_x =
                                self.input.screen_w * 0.5 - world_cx * self.input.camera_zoom;
                            self.input.camera_y =
                                self.input.screen_h * 0.5 - world_cy * self.input.camera_zoom;
                        }
                    }
                }
                UiAction::FocusTile(col, row) => {
                    let world_cx = col + 0.5;
                    let world_cy = row + 0.5;

                    // Zoom in to a comfortable battle-focus level
                    let target_zoom = 3.0_f32.max(self.input.camera_zoom);
                    self.input.camera_zoom = target_zoom;
                    self.input.target_zoom = target_zoom;
                    self.input.camera_x = self.input.screen_w * 0.5 - world_cx * target_zoom;
                    self.input.camera_y = self.input.screen_h * 0.5 - world_cy * target_zoom;
                }
                #[cfg(any(feature = "dev", debug_assertions))]
                UiAction::ToggleDevSidebar => {
                    self.ui.show_dev_sidebar = !self.ui.show_dev_sidebar;
                    if self.ui.show_dev_sidebar {
                        self.ui.show_leaderboard = false;
                    }
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
                UiAction::CopyInviteLink(lobby_id) => {
                    let link =
                        crate::store_portals::invite_link(lobby_id, &crate::get_build_version())
                            .unwrap_or_else(|| {
                                let mut fallback =
                                    format!("https://play.shadowsofwar.io/?lobbyId={}", lobby_id);
                                #[cfg(target_arch = "wasm32")]
                                if let Some(w) = web_sys::window() {
                                    if let Ok(href) = w.location().href() {
                                        if let Ok(mut url) = url::Url::parse(&href) {
                                            url.set_query(Some(&format!("lobbyId={}", lobby_id)));
                                            fallback = url.to_string();
                                        }
                                    }
                                }
                                fallback
                            });
                    self.ui.egui_ctx.copy_text(link);
                    self.ui.app.main_menu_state.invite_copied_at =
                        Some(self.ui.egui_ctx.input(|i| i.time));
                    log::info!("Copied invite link to clipboard.");
                }
                UiAction::StartPrivateLobby(lobby_id) => {
                    if let (Some(c), Some(player_id)) =
                        (self.net.client.as_ref(), self.sim.my_player_id)
                    {
                        let msg = sow_core::protocol::ClientMessage::ForceStart {
                            lobby_id,
                            player_id,
                        };
                        if let Ok(json) = bincode::serialize(&msg) {
                            c.send(json);
                        }
                    }
                }
                UiAction::KickPlayer {
                    lobby_id,
                    target_player_id,
                } => {
                    if let Some(c) = self.net.client.as_ref() {
                        let msg = sow_core::protocol::ClientMessage::Kick {
                            lobby_id,
                            target_player_id,
                        };
                        if let Ok(json) = bincode::serialize(&msg) {
                            c.send(json);
                        }
                    }
                }
                UiAction::BanPlayer {
                    lobby_id,
                    target_player_id,
                } => {
                    if let Some(c) = self.net.client.as_ref() {
                        let msg = sow_core::protocol::ClientMessage::Ban {
                            lobby_id,
                            target_player_id,
                        };
                        if let Ok(json) = bincode::serialize(&msg) {
                            c.send(json);
                        }
                    }
                }
                UiAction::MovePlayerTeam {
                    lobby_id,
                    target_player_id,
                } => {
                    if let Some(c) = self.net.client.as_ref() {
                        let msg = sow_core::protocol::ClientMessage::SetPlayerTeam {
                            lobby_id,
                            target_player_id,
                        };
                        if let Ok(json) = bincode::serialize(&msg) {
                            c.send(json);
                        }
                    }
                }
                UiAction::ResolveLinkConflict { keep_account_id } => {
                    self.resolve_link_conflict(keep_account_id);
                }
                UiAction::PortalShowAuthPrompt => {
                    crate::store_portals::show_auth_prompt();
                }
            }
        }
    }
}
