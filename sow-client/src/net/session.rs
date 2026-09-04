use crate::app::SowApp;
use crate::get_build_version;
use sow_ui_kit::ClientPhase;

impl SowApp {
    pub(crate) fn make_ready_message(
        &self,
        lobby_id: u64,
        player_id: u16,
    ) -> sow_core::protocol::ClientMessage {
        match self.sim.relay_ticket.clone() {
            Some(ticket) => sow_core::protocol::ClientMessage::ReadyWithTicket {
                lobby_id,
                player_id,
                ticket,
            },
            None => sow_core::protocol::ClientMessage::Ready {
                lobby_id,
                player_id,
            },
        }
    }

    pub(crate) fn make_initial_relay_ready_message(
        &self,
        lobby_id: u64,
        player_id: u16,
    ) -> sow_core::protocol::ClientMessage {
        self.make_ready_message(lobby_id, player_id)
    }

    pub(crate) fn make_reconnect_message(
        &self,
        lobby_id: u64,
        player_id: u16,
    ) -> sow_core::protocol::ClientMessage {
        if let Some(ticket) = self.sim.relay_reconnect_ticket.clone() {
            sow_core::protocol::ClientMessage::ReconnectWithTicket {
                lobby_id,
                player_id,
                ticket,
            }
        } else {
            // A reconnect can race the relay's capability frame. Keep the
            // initial ticket as a compatibility fallback; production logs the
            // replay if that race loses, rather than silently authenticating.
            self.make_ready_message(lobby_id, player_id)
        }
    }

    pub(crate) fn make_join_message(
        &self,
        target_lobby_id: Option<u64>,
        host_private: bool,
        host_config: Option<Box<sow_core::game_config::GameConfig>>,
        password: Option<String>,
    ) -> sow_core::protocol::ClientMessage {
        let payload = sow_core::protocol::JoinPayload {
            name: self.ui.app.main_menu_state.player_name.clone(),
            is_observer: false,
            target_lobby_id,
            host_private,
            build_version: get_build_version(),
            clan_tag: self.ui.app.main_menu_state.clan_tag.clone(),
            civilization: self.ui.app.main_menu_state.selected_civilization,
            leader: self.ui.app.main_menu_state.selected_leader,
            database_account_id: self.progress_account_id.clone(),
            host_config,
            password,
        };
        match self.make_auth_proof() {
            Some(auth) => sow_core::protocol::ClientMessage::JoinWithAuth {
                join: Box::new(payload),
                auth,
            },
            None => {
                let sow_core::protocol::JoinPayload {
                    name,
                    is_observer,
                    target_lobby_id,
                    host_private,
                    build_version,
                    clan_tag,
                    civilization,
                    leader,
                    database_account_id,
                    host_config,
                    password,
                } = payload;
                sow_core::protocol::ClientMessage::Join {
                    name,
                    is_observer,
                    target_lobby_id,
                    host_private,
                    build_version,
                    clan_tag,
                    civilization,
                    leader,
                    database_account_id,
                    host_config,
                    password,
                }
            }
        }
    }

    /// Identity proof for JoinWithAuth: the CrazyGames platform token for
    /// signed-in portal users, or the anonymous account secret. None means a
    /// plain guest join (offline mode, or no account yet).
    fn make_auth_proof(&self) -> Option<sow_core::protocol::AuthProof> {
        if self.net.is_offline {
            return None;
        }
        let identity = crate::store_portals::load_identity("Player");
        if identity.provider == "wou" {
            let token = identity.auth_token.clone().filter(|t| !t.is_empty())?;
            return Some(sow_core::protocol::AuthProof {
                provider: "wou".to_string(),
                account_id: identity.external_id.clone(),
                token,
            });
        }
        if identity.provider == "crazygames" {
            let token = identity.auth_token.clone().filter(|t| !t.is_empty())?;
            return Some(sow_core::protocol::AuthProof {
                provider: "crazygames".to_string(),
                account_id: None,
                token,
            });
        }
        let account_id = crate::anonymous_identity::load_account_id()?;
        let token = crate::anonymous_identity::load_account_secret()?;
        Some(sow_core::protocol::AuthProof {
            provider: "anonymous".to_string(),
            account_id: Some(account_id),
            token,
        })
    }

    pub(crate) fn sync_portal_room(&self, joinable: bool) {
        if let Some(id) = self.ui.app.main_menu_state.joined_lobby_id {
            crate::store_portals::update_room(id, joinable, &get_build_version());
        } else if !joinable {
            crate::store_portals::left_room();
        }
    }

    /// Tear down an online match and run the existing ExitGame splash → MainMenu flow.
    pub(crate) fn begin_exit_to_main_menu(&mut self, use_loader: bool) {
        let was_playing = self.ui.app.phase == sow_ui_kit::ClientPhase::Playing;
        if self.ui.tutorial_active && !self.progress.intro_completed.unwrap_or(false) {
            crate::analytics::track("tutorial_exit_early");
        }
        if was_playing {
            crate::store_portals::gameplay_stop();
        }
        crate::store_portals::left_room();
        self.net.is_offline = false;
        self.net.ws_url = self.net.orchestrator_url.clone();
        self.ui.app.main_menu_state.server_address = self.net.ws_url.clone();

        // Drop relay connection and force orchestrator reconnect
        self.net.client = None;
        self.ui.app.main_menu_state.is_connected = false;
        self.ui.app.main_menu_state.is_connecting = false;
        while self.net.connect_rx.try_recv().is_ok() {}
        self.net.ws_connect_not_before = web_time::Instant::now();

        self.ui.app.main_menu_state.is_waiting = false;
        self.ui.app.main_menu_state.go_home();
        self.ui.app.main_menu_state.pending_join_lobby_id = None;
        self.ui.app.main_menu_state.joined_lobby_id = None;
        self.join_waiting_for_identity = false;
        self.join_matchmaking = false;
        self.ui.app.hud_state.sync_state = None;
        self.sim.my_lobby_id = None;
        self.sim.my_player_id = None;
        self.sim.relay_ticket = None;
        self.sim.relay_reconnect_ticket = None;
        if use_loader {
            self.ui.app.phase = ClientPhase::Splash;
            let lang = self.ui.app.settings_state.language;
            self.ui
                .app
                .splash_state
                .reset_anim(sow_ui::ui::loading_screen::SplashJob::ExitGame, lang);
        } else {
            self.ui.app.phase = ClientPhase::MainMenu;
            if was_playing {
                self.gfx.pending_session_cleanup = true;
            }
        }
        self.ui.is_spectating = false;
        self.ui.endgame_cache = None;
        self.reset_progress_session();

        if was_playing
            && self.progress_account_id.is_some()
            && crate::store_portals::should_fetch_cloud_profile()
        {
            self.fetch_cloud_progress();
        }
    }

    /// Enter the EnterGame splash (fade-in, progress bar, fade-out to Playing).
    pub(crate) fn begin_enter_game_loader(&mut self) {
        self.ui.app.phase = sow_ui_kit::ClientPhase::Splash;
        let lang = self.ui.app.settings_state.language;
        self.ui
            .app
            .splash_state
            .reset_anim(sow_ui::ui::loading_screen::SplashJob::EnterGame, lang);
    }

    /// Whether the map/mover GPU path should paint this frame (hidden during splash loads).
    pub(crate) fn should_draw_world(&self) -> bool {
        use sow_ui::ui::loading_screen::SplashJob;
        use sow_ui_kit::ClientPhase;

        match self.ui.app.phase {
            ClientPhase::Playing => true,
            ClientPhase::Splash => {
                let s = &self.ui.app.splash_state;
                matches!(s.job, SplashJob::EnterGame) && s.done && s.fadeout_start.is_some()
            }
            ClientPhase::MainMenu => false,
        }
    }

    #[inline]
    pub(crate) fn ws_on_relay(&self) -> bool {
        self.net.relay_handoff_done
            || self.net.ws_url.contains("/relay/")
            || self.net.ws_url.contains("relay.shadowsofwar.io")
            || self.net.ws_url.contains("2557")
    }
}
