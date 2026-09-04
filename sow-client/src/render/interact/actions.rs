use crate::app::SowApp;
use crate::spawn_sow_client_connect;
use sow_ui::UiAction;

#[cfg(target_os = "ios")]
unsafe extern "C" {
    fn sow_revenuecat_open_store(
        app_user_id: *const std::ffi::c_char,
        host_view_controller: *mut std::ffi::c_void,
    );
}

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
                    self.ui.app.main_menu_state.notice = None;
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
                    self.request_join(Some(id), false, None, None);
                    self.ui.app.main_menu_state.go_home();
                }
                UiAction::HostPrivateLobby => {
                    self.request_join(None, true, None, None);
                }
                UiAction::OpenCreateGame => {
                    self.ui
                        .app
                        .main_menu_state
                        .open_route(sow_ui::ui::main_menu::MainMenuRoute::Create);
                    self.ui.app.main_menu_state.custom_game_is_sp = false;
                    self.ui.app.main_menu_state.error_message = None;
                }
                UiAction::OpenJoinBrowser => {
                    self.ui
                        .app
                        .main_menu_state
                        .open_route(sow_ui::ui::main_menu::MainMenuRoute::Browser);
                    self.ui.app.main_menu_state.error_message = None;
                }
                UiAction::CloseOverlay => {
                    self.ui.app.main_menu_state.go_home();
                }
                UiAction::CreateGame {
                    config,
                    is_private,
                    password,
                } => {
                    self.request_join(None, is_private, Some(config), password);
                    self.ui.app.main_menu_state.go_home();
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
                        self.request_join(Some(lobby_id), false, None, None);
                        self.ui.app.main_menu_state.go_home();
                    } else {
                        self.ui.app.main_menu_state.error_message =
                            Some("Enter a valid lobby code".to_string());
                    }
                }
                UiAction::JoinWithPassword(lobby_id) => {
                    let password = self.ui.app.main_menu_state.join_password_input.clone();
                    let pw = if password.is_empty() {
                        None
                    } else {
                        Some(password)
                    };
                    self.request_join(Some(lobby_id), false, None, pw);
                    self.ui.app.main_menu_state.go_home();
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

                            self.input.camera_focus_target = Some((world_cx, world_cy));
                            self.input.target_zoom = 10.0;
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
                    self.clamp_camera_to_map();
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
                UiAction::SetFullscreen(fullscreen) => {
                    self.ui.app.settings_state.is_fullscreen = fullscreen;
                    #[cfg(not(target_arch = "wasm32"))]
                    if let Some(win) = self.gfx.window.as_ref() {
                        let mode = if fullscreen {
                            Some(winit::monitor::Fullscreen::Borderless(None))
                        } else {
                            None
                        };
                        win.set_fullscreen(mode);
                    }
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
                UiAction::ToggleShowcase => {
                    self.ui.app.is_showcase_open = !self.ui.app.is_showcase_open;
                }
                UiAction::OpenStorePage => {
                    self.ui
                        .app
                        .main_menu_state
                        .open_route(sow_ui::ui::main_menu::MainMenuRoute::Store);
                    self.ui.app.main_menu_state.error_message = None;
                }
                UiAction::OpenProfilePage => {
                    self.ui
                        .app
                        .main_menu_state
                        .open_route(sow_ui::ui::main_menu::MainMenuRoute::Profile);
                    self.ui.app.main_menu_state.profile.public_id = self.profile_public_id.clone();
                    self.ui.app.main_menu_state.profile.error = None;
                    self.ui.app.main_menu_state.profile.view = None;
                    self.ui.app.main_menu_state.profile.history.clear();
                    self.ui.app.main_menu_state.profile.ratings.clear();
                    self.ui.app.main_menu_state.profile.search_results.clear();
                    self.ui.app.main_menu_state.profile.history_cursor = 0;
                    self.ui.app.main_menu_state.profile.history_has_next = false;
                    self.ui.app.main_menu_state.profile.ratings_loaded = false;
                    self.ui.app.main_menu_state.profile.match_detail = None;
                    self.ui.app.main_menu_state.profile.active_tab =
                        sow_ui::ui::main_menu::profile::NativeProfileTab::Overview;
                    self.ui.app.main_menu_state.profile.loading = false;
                }
                UiAction::OpenStore => {
                    #[cfg(target_os = "ios")]
                    self.open_ios_store();
                }
                UiAction::LoadOwnProfile => {
                    self.load_native_profile();
                }
                UiAction::OpenPublicProfilePage(public_id) => {
                    self.ui
                        .app
                        .main_menu_state
                        .open_route(sow_ui::ui::main_menu::MainMenuRoute::Profile);
                    self.ui.app.main_menu_state.profile.public_id = Some(public_id);
                    self.ui.app.main_menu_state.profile.view = None;
                    self.ui.app.main_menu_state.profile.history.clear();
                    self.ui.app.main_menu_state.profile.ratings.clear();
                    self.ui.app.main_menu_state.profile.history_cursor = 0;
                    self.ui.app.main_menu_state.profile.history_has_next = false;
                    self.ui.app.main_menu_state.profile.ratings_loaded = false;
                    self.ui.app.main_menu_state.profile.match_detail = None;
                    self.ui.app.main_menu_state.profile.error = None;
                    self.ui.app.main_menu_state.profile.loading = false;
                }
                UiAction::LoadProfileHistory => {
                    self.load_native_profile_history();
                }
                UiAction::LoadProfileRatings => {
                    self.load_native_profile_ratings();
                }
                UiAction::SearchProfiles(query) => {
                    self.search_native_profiles(query);
                }
                UiAction::LoadMatchDetail(match_id) => {
                    self.load_native_match_detail(match_id);
                }
                UiAction::CloseMatchDetail => {
                    self.ui.app.main_menu_state.profile.match_detail = None;
                }
                UiAction::BuyGems(product_id) => {
                    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "ios")))]
                    self.open_desktop_purchase(&product_id);
                    #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
                    let _ = product_id;
                }
                UiAction::UnlockLeader {
                    leader_id,
                    currency,
                } => {
                    self.unlock_native_leader(leader_id, currency);
                }
                UiAction::UnlockSkin(skin_id) => {
                    self.unlock_native_skin(skin_id);
                }
                UiAction::EquipSkin(skin_id) => {
                    self.equip_native_skin(skin_id);
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
                UiAction::PortalShowAuthPrompt => {
                    crate::store_portals::show_auth_prompt();
                }
                UiAction::SaveDisplayName(display_name) => {
                    self.save_display_name(display_name);
                }
            }
        }
    }

    #[cfg(target_os = "ios")]
    fn open_ios_store(&mut self) {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};

        let Some(app_user_id) = self.profile_public_id.clone() else {
            self.ui.app.main_menu_state.error_message =
                Some("Your player profile is still loading. Try Store again in a moment.".into());
            return;
        };
        let Ok(app_user_id) = std::ffi::CString::new(app_user_id) else {
            self.ui.app.main_menu_state.error_message =
                Some("The player profile could not open the store.".into());
            return;
        };
        let Some(window) = self.active_window() else {
            self.ui.app.main_menu_state.error_message =
                Some("The game window is not ready to open the store.".into());
            return;
        };
        let Ok(window_handle) = window.window_handle() else {
            self.ui.app.main_menu_state.error_message =
                Some("The game window could not open the store.".into());
            return;
        };
        let RawWindowHandle::UiKit(handle) = window_handle.as_raw() else {
            self.ui.app.main_menu_state.error_message =
                Some("The current platform cannot open the Apple store.".into());
            return;
        };
        let Some(view_controller) = handle.ui_view_controller else {
            self.ui.app.main_menu_state.error_message =
                Some("The Apple store host is not ready.".into());
            return;
        };
        unsafe {
            sow_revenuecat_open_store(app_user_id.as_ptr(), view_controller.as_ptr());
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), not(target_os = "ios")))]
    fn open_desktop_purchase(&mut self, product_id: &str) {
        let Some(public_id) = self.profile_public_id.as_deref() else {
            self.ui.app.main_menu_state.error_message =
                Some("Your player profile is still loading. Try again in a moment.".into());
            return;
        };
        let Some(base) = std::env::var("SOW_REVENUECAT_WEB_PURCHASE_LINK")
            .ok()
            .filter(|value| !value.trim().is_empty())
        else {
            self.ui.app.main_menu_state.error_message =
                Some("Online purchases are not configured for this desktop build.".into());
            return;
        };
        let encoded_user = url::form_urlencoded::byte_serialize(public_id.as_bytes()).collect::<String>();
        let encoded_product = url::form_urlencoded::byte_serialize(product_id.as_bytes()).collect::<String>();
        let url = format!(
            "{}/{}/?package_id={}",
            base.trim_end_matches('/'),
            encoded_user,
            encoded_product
        );
        if let Err(error) = open::that(&url) {
            log::error!("[store] failed to open RevenueCat checkout: {error}");
            self.ui.app.main_menu_state.error_message =
                Some("Could not open the online checkout.".into());
        } else {
            log::info!("[store] opened RevenueCat checkout for gem bundle");
            self.ui.app.main_menu_state.error_message = None;
        }
    }

    pub(crate) fn request_join(
        &mut self,
        lobby_id: Option<u64>,
        is_private: bool,
        config: Option<Box<sow_core::game_config::GameConfig>>,
        password: Option<String>,
    ) {
        self.ui.app.main_menu_state.error_message = None;
        self.ui.app.main_menu_state.notice = None;
        let matchmaking_join = lobby_id.is_none() && !is_private && config.is_none();
        self.join_matchmaking = matchmaking_join;
        if let Some(cfg) = config {
            self.ui.app.main_menu_state.custom_game_config = cfg;
        }
        self.ui.app.main_menu_state.custom_game_is_private = is_private;
        self.ui.app.main_menu_state.custom_game_password = password.clone().unwrap_or_default();
        self.ui.app.main_menu_state.pending_join_lobby_id = lobby_id;
        self.ui.app.main_menu_state.is_waiting = true;
        self.ui.app.main_menu_state.is_lobby_host = if lobby_id.is_some() {
            false
        } else {
            !self.join_matchmaking
        };
        let identity_ready = self.progress_account_id.is_some() || self.net.is_offline;
        self.join_waiting_for_identity = !identity_ready || self.net.client.is_none();

        if let Some(c) = self.net.client.as_ref() {
            if identity_ready {
                let config_opt = (!self.join_matchmaking)
                    .then(|| self.ui.app.main_menu_state.custom_game_config.clone());
                let join_msg = self.make_join_message(lobby_id, is_private, config_opt, password);
                if let Ok(json) = bincode::serialize(&join_msg) {
                    c.send(json);
                }
                self.join_waiting_for_identity = false;
            } else {
                // Identity must settle before Join; otherwise the server sees a
                // second anonymous player on every refresh/race.
                log::info!("Queueing Join until the canonical account is loaded");
            }
        } else {
            log::info!(
                "No active connection, spawning lazy connection to {}",
                self.net.ws_url
            );
            self.ui.app.main_menu_state.is_connecting = true;
            let url = self.net.ws_url.clone();
            #[cfg(target_arch = "wasm32")]
            spawn_sow_client_connect(url, &self.net.connect_tx);
            #[cfg(not(target_arch = "wasm32"))]
            spawn_sow_client_connect(url, &self.net.connect_tx, &self.tokio_rt);
        }

    }
}
