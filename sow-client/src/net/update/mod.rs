use crate::app::SowApp;
use crate::spawn_sow_client_connect;
use sow_ui_kit::ClientPhase;
use web_time::{Duration, Instant};

mod messages;

impl SowApp {
    pub fn update_net(&mut self, now: Instant) {
        self.poll_portal_intents();

        #[cfg(target_arch = "wasm32")]
        {
            let doc_visible = web_sys::window()
                .and_then(|w| w.document())
                .map(|d| d.visibility_state() == web_sys::VisibilityState::Visible)
                .unwrap_or(true);
            if doc_visible && !self.wasm_doc_was_visible {
                self.net.ws_reconnect_after_resume = true;
            }
            self.wasm_doc_was_visible = doc_visible;
        }

        if self.net.ws_reconnect_after_resume {
            self.net.ws_reconnect_after_resume = false;
            self.net.ws_connect_not_before = self.net.ws_connect_not_before.min(now);
        }

        if matches!(
            self.ui.app.phase,
            sow_ui_kit::ClientPhase::MainMenu | sow_ui_kit::ClientPhase::Splash
        ) {
            self.fetch_map_catalog_if_needed();
        }

        // No fake map download simulation! Progress is real!

        while let Ok(res) = self.net.connect_rx.try_recv() {
            match res {
                Ok(client) => {
                    log::info!(
                        "[CLIENT NET] ✅ Received successfully connected WebSocket client from channel!"
                    );
                    self.ui.app.main_menu_state.is_connected = true;
                    self.ui.app.main_menu_state.is_connecting = false;
                    self.net.ws_connect_fail_backoff_ms = 400;
                    if self.ws_on_relay() {
                        self.net.relay_connect_start = None;
                        self.net.relay_retry_count = 0;
                        self.net.load_telemetry.mark_relay_connected();
                    }

                    if self.ui.app.phase == sow_ui_kit::ClientPhase::Playing {
                        if let (Some(lid), Some(pid)) =
                            (self.sim.my_lobby_id, self.sim.my_player_id)
                        {
                            log::info!("Sent Ready to Relay server on reconnect/playing!");
                            client.send(
                                bincode::serialize(&self.make_reconnect_message(lid, pid)).unwrap(),
                            );
                        }
                    } else if self.net.pending_lobby_rejoin {
                        log::info!("Re-sending Join to lobby after hop");
                        let target = self.sim.my_lobby_id.or(self
                            .ui
                            .app
                            .main_menu_state
                            .pending_join_lobby_id);
                        let join_msg = self.make_join_message(target, false, None, None);
                        if let Ok(json) = bincode::serialize(&join_msg) {
                            client.send(json);
                        }
                        self.net.pending_lobby_rejoin = false;
                    } else if self.ui.app.main_menu_state.host_private_pending {
                        log::info!("Hosting private lobby (portal instant / play again)");
                        let join_msg = self.make_join_message(None, true, None, None);
                        if let Ok(json) = bincode::serialize(&join_msg) {
                            client.send(json);
                        }
                        self.ui.app.main_menu_state.host_private_pending = false;
                        self.ui.app.main_menu_state.is_waiting = true;
                    } else if let Some(id) = self.ui.app.main_menu_state.pending_join_lobby_id {
                        if self.ui.app.main_menu_state.is_waiting
                            && self.ui.app.main_menu_state.joined_lobby_id.is_none()
                            && (self.progress_account_id.is_some() || self.net.is_offline)
                        {
                            log::info!("Joining lobby {} from portal invite", id);
                            let join_msg = self.make_join_message(Some(id), false, None, None);
                            if let Ok(json) = bincode::serialize(&join_msg) {
                                client.send(json);
                            }
                            self.join_waiting_for_identity = false;
                        }
                    } else if self.join_waiting_for_identity
                        && self.ui.app.main_menu_state.is_waiting
                        && self.ui.app.main_menu_state.joined_lobby_id.is_none()
                        && (self.progress_account_id.is_some() || self.net.is_offline)
                    {
                        let join_msg = self.make_join_message(
                            None,
                            self.ui.app.main_menu_state.custom_game_is_private,
                            Some(self.ui.app.main_menu_state.custom_game_config.clone()),
                            Some(self.ui.app.main_menu_state.custom_game_password.clone())
                                .filter(|password| !password.is_empty()),
                        );
                        if let Ok(json) = bincode::serialize(&join_msg) {
                            client.send(json);
                        }
                        self.join_waiting_for_identity = false;
                    }
                    self.net.client = Some(client);
                }
                Err(e) => {
                    log::warn!("[CLIENT NET] Failed to connect: {}", e);
                    self.ui.app.main_menu_state.is_connected = false;
                    self.ui.app.main_menu_state.is_connecting = false;
                    if self.ws_on_relay() && !self.net.is_offline {
                        self.net.relay_retry_count += 1;
                        self.net.relay_connect_start = Some(now); // reset timer on each Err to extend per-attempt budget
                        if self.net.relay_retry_count >= 10 {
                            log::error!("Relay connection failed after 10 attempts");
                            self.net.relay_connect_start = None;
                            self.net.relay_retry_count = 0;
                            self.ui.app.main_menu_state.error_message = Some(
                                "Failed to connect to the game server after 10 attempts."
                                    .to_string(),
                            );
                            self.begin_exit_to_main_menu(true);
                        } else {
                            log::warn!(
                                "Relay connection failed: {}; retrying rapid connection attempt {}/10",
                                e,
                                self.net.relay_retry_count + 1
                            );
                            self.net.ws_connect_fail_backoff_ms = 500;
                            self.net.ws_connect_not_before = now + Duration::from_millis(500);
                        }
                    } else {
                        self.net.ws_connect_fail_backoff_ms =
                            (self.net.ws_connect_fail_backoff_ms.saturating_mul(2)).min(30_000);
                        self.net.ws_connect_not_before =
                            now + Duration::from_millis(self.net.ws_connect_fail_backoff_ms);
                    }
                }
            }
        }

        // 15-second total relay timeout check (as a last-resort safety net)
        if self.ws_on_relay() && self.net.client.is_none() && !self.net.is_offline {
            if self.net.relay_connect_start.is_none() {
                self.net.relay_connect_start = Some(now);
                self.net.relay_retry_count = 0;
            }
            if let Some(start) = self.net.relay_connect_start {
                if now.duration_since(start) >= Duration::from_secs(15) {
                    log::error!("Relay connection/reconnection timed out after 15 seconds total");
                    self.net.relay_connect_start = None;
                    self.net.relay_retry_count = 0;
                    self.ui.app.main_menu_state.error_message = Some(
                        "Failed to connect to the game server. Connection timed out.".to_string(),
                    );
                    self.begin_exit_to_main_menu(true);
                }
            }
        } else {
            self.net.relay_connect_start = None;
        }

        let (mut ws_disconnected, switch_to_relay, exit_to_menu_after_net, pending_rematch) =
            self.process_ws_messages(now);

        if let Some(rematch_id) = pending_rematch {
            crate::store_portals::gameplay_stop();
            self.cleanup_game_session_stub();
            self.net.pending_lobby_rejoin = true;
            self.ui.app.main_menu_state.pending_join_lobby_id = Some(rematch_id);
            self.ui.app.phase = ClientPhase::MainMenu;
            self.ui.app.main_menu_state.is_waiting = true;

            // Drop relay connection and force orchestrator reconnect for the rematch
            self.net.client = None;
            self.net.ws_url = self.net.orchestrator_url.clone();
            self.ui.app.main_menu_state.server_address = self.net.ws_url.clone();
            self.net.ws_connect_not_before = now;
        }

        if exit_to_menu_after_net {
            self.begin_exit_to_main_menu(true);
        }

        if let Some((relay_port, relay_host)) = switch_to_relay {
            self.net.relay_handoff_done = true;
            self.net.load_telemetry.reset_for_relay_handoff();
            log::info!(
                "[CLIENT NET] Handoff from Master Orchestrator -> Game Relay on port {} (host {:?})",
                relay_port,
                relay_host
            );

            // Connect directly to the relay host (F-Stack endpoint with TLS).
            // Both WASM and native clients use wss:// — the relay terminates TLS.
            if let Some(host) = relay_host {
                self.net.ws_url = format!("wss://{}:{}/ws/", host, relay_port);
            } else if let Ok(mut url) = url::Url::parse(&self.net.ws_url) {
                let _ = url.set_port(Some(relay_port));
                self.net.ws_url = url.to_string();
            }

            log::info!("[CLIENT NET] Handoff URL resolved to: {}", self.net.ws_url);
            self.net.client = None; // Drop orchestrator connection
            self.ui.app.main_menu_state.is_connected = false; // Reset connection status during handoff
            self.ui.app.main_menu_state.server_address = self.net.ws_url.clone();
            ws_disconnected = false;

            // Clear stale connections
            while self.net.connect_rx.try_recv().is_ok() {
                log::info!(
                    "Purged stale connection from channel during handoff to relay"
                );
            }

            self.net.relay_connect_start = Some(now);
            self.net.relay_retry_count = 0;
            self.net.ws_connect_not_before = now; // Ensure no backoff delays are active for retries
        }

        if ws_disconnected {
            log::warn!(
                "[CLIENT NET] WS disconnect observed: phase={:?}, waiting={}, splash_job={:?}, has_engine_init_queued={}, has_pending_init_data={}, on_relay={}, ws_url={}",
                self.ui.app.phase,
                self.ui.app.main_menu_state.is_waiting,
                self.ui.app.splash_state.job,
                self.tasks.engine_init_queued_msg.is_some(),
                self.tasks.pending_engine_init_data.is_some(),
                self.ws_on_relay(),
                self.net.ws_url
            );
            self.net.client = None;
            self.ui.app.main_menu_state.is_connected = false;
            self.ui.app.main_menu_state.is_connecting = false;
            if self.ws_on_relay() {
                self.net.ws_connect_not_before = now;
                self.net.relay_connect_start = Some(now);
                self.net.relay_retry_count = 0;
            } else {
                self.net.ws_connect_not_before = now + Duration::from_millis(2000);
            }

            if self.net.is_offline {
                log::debug!("[CLIENT NET] Offline match; ignoring disconnect recovery");
            } else if self.ui.app.phase == ClientPhase::Playing {
                // Relay can replay turns after ClientMessage::Ready (see sow-relay), but we do not
                // resume in-place: the socket drop may mean the relay died, and catch-up without a
                // full snapshot risks desync. Use the existing ExitGame loader → MainMenu.
                if self.ws_on_relay() {
                    log::warn!("[CLIENT NET] Relay lost during match — attempting reconnect");
                } else {
                    log::warn!(
                        "[CLIENT NET] Connection lost during match — returning to main menu"
                    );
                    self.ui.app.main_menu_state.error_message =
                        Some("Connection to the matchmaking server was lost.".to_string());
                    self.begin_exit_to_main_menu(true);
                }
            } else if self.ui.app.phase != ClientPhase::Splash {
                log::info!("[CLIENT NET] Disconnected outside match; reconnecting to orchestrator");
                self.net.ws_url = self.net.orchestrator_url.clone();
                self.ui.app.main_menu_state.server_address = self.net.ws_url.clone();
            }
        }

        #[cfg(target_arch = "wasm32")]
        let allow_ws_spawn = self.wasm_doc_was_visible;
        #[cfg(not(target_arch = "wasm32"))]
        let allow_ws_spawn = true;

        if allow_ws_spawn
            && self.net.client.is_none()
            && !self.ui.app.main_menu_state.is_connecting
            && now >= self.net.ws_connect_not_before
            && !self.net.is_offline
        {
            self.ui.app.main_menu_state.is_connecting = true;
            let url = self.ui.app.main_menu_state.server_address.clone();
            log::info!(
                "[CLIENT NET] 🔄 Auto-reconnect triggered: Spawning WS connection task to {}",
                url
            );
            #[cfg(target_arch = "wasm32")]
            spawn_sow_client_connect(url, &self.net.connect_tx);
            #[cfg(not(target_arch = "wasm32"))]
            spawn_sow_client_connect(url, &self.net.connect_tx, &self.tokio_rt);
        }
    }
}
