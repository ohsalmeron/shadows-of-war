use crate::MapDownloadEvent;
use crate::app::SowApp;
use crate::net::lobby::{apply_lobbies_broadcast, seed_joined_lobby_entry};
use sow_ui_kit::ClientPhase;

pub(super) struct ProcessWsResult {
    pub(super) ws_disconnected: bool,
    pub(super) switch_to_relay: Option<(u16, Option<String>)>,
    pub(super) exit_to_menu_after_net: bool,
    pub(super) pending_rematch: Option<u64>,
}

impl SowApp {
    pub(super) fn process_ws_messages(&mut self) -> ProcessWsResult {
        let mut ws_disconnected = false;
        #[cfg(target_arch = "wasm32")]
        if let Some(c) = self.net.client.as_ref() {
            if c.is_socket_closed() {
                ws_disconnected = true;
            }
        }

        let mut switch_to_relay: Option<(u16, Option<String>)> = None;
        let mut exit_to_menu_after_net = false;
        let mut pending_rematch: Option<u64> = None;

        // Process network messages
        if let Some(c) = self.net.client.as_ref()
            && !ws_disconnected
        {
            loop {
                let msg = match c.rx.try_recv() {
                    Ok(msg) => msg,
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        ws_disconnected = true;
                        break;
                    }
                };

                use sow_core::protocol::ServerMessage;
                let server_msg = match bincode::deserialize::<ServerMessage>(&msg) {
                    Ok(m) => m,
                    Err(e) => {
                        log::warn!(
                            "[DIAG NET ERROR] Failed to deserialize server message ({} bytes): {}",
                            msg.len(),
                            e
                        );
                        continue;
                    }
                };

                match server_msg {
                    ServerMessage::RelayTicket {
                        lobby_id,
                        player_id,
                        ticket,
                    } => {
                        if self.sim.my_lobby_id == Some(lobby_id)
                            && self.sim.my_player_id == Some(player_id)
                        {
                            self.sim.relay_ticket = Some(ticket);
                            self.sim.relay_reconnect_ticket = None;
                        }
                    }
                    ServerMessage::RelayReconnectTicket {
                        lobby_id,
                        player_id,
                        ticket,
                    } => {
                        if self.sim.my_lobby_id == Some(lobby_id)
                            && self.sim.my_player_id == Some(player_id)
                        {
                            self.sim.relay_reconnect_ticket = Some(ticket);
                        }
                    }
                    ServerMessage::Start(start_msg) => {
                        log::info!(
                            "[CLIENT NET] 🎮 Received ServerStartMessage! lobby_id={:?}, player_id={:?}, relay_port={:?}, relay_host={:?}",
                            start_msg.lobby_id,
                            start_msg.my_player_id,
                            start_msg.relay_port,
                            start_msg.relay_host
                        );
                        self.sync_portal_room(false);
                        let not_splash = self.ui.app.phase != sow_ui_kit::ClientPhase::Splash;
                        let wrong_job = self.ui.app.splash_state.job
                            != sow_ui::ui::loading_screen::SplashJob::EnterGame;

                        if not_splash || wrong_job {
                            self.ui.app.phase = sow_ui_kit::ClientPhase::Splash;
                            let lang = self.ui.app.settings_state.language;
                            self.ui
                                .app
                                .splash_state
                                .reset_anim(sow_ui::ui::loading_screen::SplashJob::EnterGame, lang);
                        }
                        self.ui.app.main_menu_state.is_waiting = false;
                        self.ui.app.main_menu_state.pending_join_lobby_id = None;
                        self.ui.app.main_menu_state.joined_lobby_id = None;
                        self.ui.app.main_menu_state.wait_timer_secs = 0.0;
                        self.ui.app.hud_state.sync_state = None;
                        if start_msg.my_player_id.is_some() {
                            self.sim.my_player_id = start_msg.my_player_id;
                        }
                        self.sim.my_lobby_id = start_msg.lobby_id;

                        if let Some(relay_port) = start_msg.relay_port {
                            switch_to_relay = Some((relay_port, start_msg.relay_host.clone()));
                        }

                        self.tasks.engine_init_queued_msg = Some(*start_msg);
                        if switch_to_relay.is_some() {
                            break;
                        }
                    }
                    ServerMessage::Turn(turn_msg) => {
                        // NOTE(2026-08-22): per-turn [DIAG NET] logging
                        // disabled — added to diagnose the F-Stack relay
                        // lag report; at 10 Hz it logged EVERY turn and
                        // Chrome's console made DevTools-open sessions
                        // noticeably slower. Relay forensics exonerated
                        // the delivery path. Re-enable behind
                        // `crate::diag::enabled()` if needed.
                        self.sim.turn_queue.push_back(turn_msg.turn);
                        self.ui.app.hud_state.sync_state = None;
                    }
                    ServerMessage::SyncState(sync_msg) => {
                        self.ui.app.hud_state.sync_state = Some(sync_msg.clone());
                        if self.ui.app.main_menu_state.is_waiting {
                            self.ui.app.main_menu_state.wait_timer_secs = sync_msg.time_remaining;

                            // All clients ready: go to loader immediately
                            if sync_msg.is_starting {
                                log::info!(
                                    "[LOBBY] All ready (is_starting), entering loader screen"
                                );
                                if self.ui.app.phase != sow_ui_kit::ClientPhase::Splash {
                                    self.ui.app.phase = sow_ui_kit::ClientPhase::Splash;
                                    let lang = self.ui.app.settings_state.language;
                                    self.ui.app.splash_state.reset_anim(
                                        sow_ui::ui::loading_screen::SplashJob::EnterGame,
                                        lang,
                                    );
                                }
                                self.ui.app.main_menu_state.is_waiting = false;
                            } else {
                                // Update lobby player list in UI
                                let key = self
                                    .sim
                                    .my_lobby_id
                                    .or(self.ui.app.main_menu_state.joined_lobby_id)
                                    .or(self.ui.app.main_menu_state.pending_join_lobby_id);
                                if let Some(id) = key {
                                    if let Some(lobby) = self
                                        .ui
                                        .app
                                        .main_menu_state
                                        .lobbies
                                        .iter_mut()
                                        .find(|l| l.id == id)
                                    {
                                        lobby.timer_secs = sync_msg.time_remaining;
                                        lobby.is_counting_down = sync_msg.time_remaining > 0.0
                                            && sync_msg.time_remaining < 30.0;
                                        lobby.num_players = sync_msg.players.len() as u32;
                                        lobby.players = sync_msg.players.clone();
                                    } else {
                                        let kind = if self.ui.app.main_menu_state.in_private_match {
                                            sow_core::protocol::LobbyKind::Custom
                                        } else {
                                            sow_core::protocol::LobbyKind::Matchmaking
                                        };
                                        self.ui.app.main_menu_state.lobbies.push(
                                            sow_core::protocol::LobbyInfo {
                                                id,
                                                kind,
                                                num_players: sync_msg.players.len() as u32,
                                                max_players: 0, // unknown until the server broadcast arrives
                                                is_counting_down: sync_msg.time_remaining > 0.0
                                                    && sync_msg.time_remaining < 30.0,
                                                timer_secs: sync_msg.time_remaining,
                                                map_name: "Loading...".to_string(),
                                                game_mode: "FFA".to_string(),
                                                players: sync_msg.players.clone(),
                                                has_password: false,
                                                host_name: String::new(),
                                                bot_count: 0,
                                                nation_count: 0,
                                                bot_difficulty: Default::default(),
                                            },
                                        );
                                    }
                                }
                            }
                        }
                    }
                    ServerMessage::Pong { client_time } => {
                        let rtt = self.time.start_time.elapsed().as_secs_f64() - client_time;
                        self.net.current_ping_ms = Some((rtt * 1000.0) as u32);
                    }
                    ServerMessage::LobbiesBroadcast(broadcast) => {
                        // Don't clobber the lobby list once we've started loading into a game
                        if self.ui.app.phase != sow_ui_kit::ClientPhase::Splash {
                            apply_lobbies_broadcast(&mut self.ui.app.main_menu_state, &broadcast);
                        }

                        let maps_base = self.asset_config.maps_base.clone();
                        let maps_to_fetch = self
                            .ui
                            .app
                            .asset_loader
                            .get_assets_to_fetch(&self.ui.app.main_menu_state.lobbies);

                        for map_name in maps_to_fetch {
                            let url = format!(
                                "{}/{}/map.bin.br",
                                maps_base.trim_end_matches('/'),
                                map_name
                            );
                            let tx = self.tasks.map_tx.clone();
                            let map_name_for_closure = map_name.clone();
                            let request = ehttp::Request::get(&url);
                            let accumulated =
                                std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
                            ehttp::streaming::fetch(
                                request,
                                move |result: ehttp::Result<ehttp::streaming::Part>| match result {
                                    Ok(ehttp::streaming::Part::Response(res)) => {
                                        if !res.ok {
                                            log::warn!(
                                                "Prefetch failed for {}",
                                                map_name_for_closure
                                            );
                                            return std::ops::ControlFlow::Break(());
                                        }
                                        std::ops::ControlFlow::Continue(())
                                    }
                                    Ok(ehttp::streaming::Part::Chunk(chunk)) => {
                                        if chunk.is_empty() {
                                            let final_bytes =
                                                std::mem::take(&mut *accumulated.lock().unwrap());
                                            let _ = tx.send(MapDownloadEvent::MapReady(
                                                map_name_for_closure.clone(),
                                                final_bytes,
                                            ));
                                            return std::ops::ControlFlow::Break(());
                                        }
                                        accumulated.lock().unwrap().extend_from_slice(&chunk);
                                        std::ops::ControlFlow::Continue(())
                                    }
                                    Err(_) => std::ops::ControlFlow::Break(()),
                                },
                            );
                        }

                        if self.ui.app.main_menu_state.is_waiting {
                            let key = self
                                .sim
                                .my_lobby_id
                                .or(self.ui.app.main_menu_state.joined_lobby_id)
                                .or(self.ui.app.main_menu_state.pending_join_lobby_id);
                            if let Some(l_id) = key
                                && let Some(lobby) = broadcast.lobbies.iter().find(|l| l.id == l_id)
                                && lobby.is_counting_down
                            {
                                self.ui.app.main_menu_state.wait_timer_secs = lobby.timer_secs;
                            }
                        }
                    }
                    ServerMessage::VersionUpdate { version } => {
                        log::info!("Received version update: {}", version);
                        self.ui.update_available = true;
                    }
                    ServerMessage::LobbyClosed(closed) => {
                        log::warn!("Lobby {} closed: {}", closed.lobby_id, closed.reason);
                        self.ui.app.hud_state.sync_state = None;
                        self.sim.my_lobby_id = None;
                        self.sim.my_player_id = None;
                        self.sim.relay_ticket = None;
                        self.sim.relay_reconnect_ticket = None;

                        if let Some(rematch_id) = closed.rematch_lobby_id {
                            log::info!("Rematch lobby {} offered", rematch_id);
                            pending_rematch = Some(rematch_id);
                        } else if closed.reason.contains("Requeueing") {
                            log::info!("Auto-requeueing to a new lobby...");
                            self.ui.app.phase = ClientPhase::MainMenu;
                            self.ui.app.main_menu_state.is_waiting = true;
                            let join_msg = self.make_join_message(None, false, None, None);
                            c.send(bincode::serialize(&join_msg).unwrap());
                        } else if matches!(
                            closed.reason.as_str(),
                            "HOST_LEFT" | "KICKED" | "BANNED"
                        ) {
                            // Lobby-stage removal: keep the orchestrator connection so the
                            // browser stays live, drop straight back to the menu (no exit
                            // splash — they were only waiting), and surface a brief notice.
                            crate::store_portals::left_room();
                            let mm = &mut self.ui.app.main_menu_state;
                            mm.is_waiting = false;
                            mm.joined_lobby_id = None;
                            mm.pending_join_lobby_id = None;
                            mm.in_private_match = false;
                            mm.is_lobby_host = false;
                            mm.my_player_id = None;
                            mm.notice = Some(match closed.reason.as_str() {
                                "KICKED" => sow_ui::LobbyNotice::Kicked,
                                "BANNED" => sow_ui::LobbyNotice::Banned,
                                _ => sow_ui::LobbyNotice::HostLeft,
                            });
                            mm.notice_at = None;
                            self.ui.app.phase = ClientPhase::MainMenu;
                        } else {
                            crate::store_portals::left_room();
                            exit_to_menu_after_net = true;
                        }
                    }
                    ServerMessage::JoinFailed(fail) => {
                        log::warn!("[JOIN] Failed: {}", fail.reason);
                        crate::store_portals::left_room();
                        if fail.reason == "VERSION_MISMATCH" {
                            log::info!("[JOIN] Version mismatch — prompting user to update...");
                            self.ui.update_available = true;
                        }
                        self.ui.app.main_menu_state.is_waiting = false;
                        self.ui.app.main_menu_state.pending_join_lobby_id = None;
                        self.ui.app.main_menu_state.joined_lobby_id = None;
                        self.ui.app.main_menu_state.is_lobby_host = false;
                        self.ui.app.main_menu_state.my_player_id = None;
                        self.ui.app.main_menu_state.in_private_match = false;
                        self.ui.app.main_menu_state.error_message = Some(fail.reason.clone());
                        if fail.reason == "BANNED" {
                            // Banned from that lobby — show the banned notice instead of
                            // the raw connection-error modal.
                            self.ui.app.main_menu_state.notice = Some(sow_ui::LobbyNotice::Banned);
                            self.ui.app.main_menu_state.notice_at = None;
                        } else {
                            self.ui.app.main_menu_state.notice =
                                Some(sow_ui::LobbyNotice::ConnectionLost);
                            self.ui.app.main_menu_state.notice_at = None;
                        }
                    }
                    ServerMessage::JoinAck(ack) => {
                        log::info!(
                            "[LOBBY] Joined lobby {} as player {} (map: {})",
                            ack.lobby_id,
                            ack.player_id,
                            ack.map_name
                        );
                        self.sim.my_lobby_id = Some(ack.lobby_id);
                        self.sim.my_player_id = Some(ack.player_id);
                        self.ui.app.main_menu_state.my_player_id = Some(ack.player_id);
                        self.ui.app.main_menu_state.joined_lobby_id = Some(ack.lobby_id);
                        self.ui.app.main_menu_state.is_waiting = true;
                        self.ui.app.main_menu_state.go_home();
                        self.ui.app.main_menu_state.in_private_match = ack.is_private;
                        seed_joined_lobby_entry(&mut self.ui.app.main_menu_state, &ack);
                        crate::analytics::track_with(
                            "matchmaking_joined",
                            serde_json::json!({
                                "lobby_id": ack.lobby_id,
                                "map": ack.map_name,
                            }),
                        );
                        self.sync_portal_room(true);

                        let map_name = ack.map_name.clone();
                        self.ui.app.main_menu_state.downloading_map_name = Some(map_name.clone());

                        if let Some(texture) = self.ui.app.asset_loader.thumbnail(&map_name) {
                            self.ui.app.splash_state.thumbnail = Some(texture.clone());
                        } else {
                            self.ui.app.splash_state.thumbnail = None;
                        }

                        if self.ui.app.asset_loader.has_map(&map_name) {
                            log::info!("Map already cached, skipping download.");
                            self.ui.app.main_menu_state.cached_map =
                                self.ui.app.asset_loader.take_map(&map_name);
                            self.ui.app.main_menu_state.is_downloading_map = false;
                            self.ui.app.main_menu_state.map_download_progress = 100;
                            c.send(
                                bincode::serialize(
                                    &sow_core::protocol::ClientMessage::MapDownloadProgress {
                                        lobby_id: ack.lobby_id,
                                        player_id: ack.player_id,
                                        progress: 100,
                                    },
                                )
                                .unwrap(),
                            );
                            c.send(
                                bincode::serialize(
                                    &self.make_ready_message(ack.lobby_id, ack.player_id),
                                )
                                .unwrap(),
                            );
                        } else {
                            let tx = self.tasks.map_tx.clone();
                            self.ui.app.main_menu_state.is_downloading_map = true;
                            self.ui.app.main_menu_state.cached_map = None;

                            let maps_base = self.asset_config.maps_base.clone();
                            let url = format!(
                                "{}/{}/map.bin.br",
                                maps_base.trim_end_matches('/'),
                                map_name
                            );
                            log::info!("Downloading map from: {}", url);

                            c.send(
                                bincode::serialize(
                                    &sow_core::protocol::ClientMessage::MapDownloadProgress {
                                        lobby_id: ack.lobby_id,
                                        player_id: ack.player_id,
                                        progress: 0,
                                    },
                                )
                                .unwrap(),
                            );

                            let request = ehttp::Request::get(&url);
                            let map_name_for_closure = map_name.clone();
                            let accumulated =
                                std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
                            let total_bytes = std::sync::Arc::new(std::sync::Mutex::new(0usize));

                            ehttp::streaming::fetch(
                                request,
                                move |result: ehttp::Result<ehttp::streaming::Part>| match result {
                                    Ok(ehttp::streaming::Part::Response(res)) => {
                                        if !res.ok {
                                            log::error!("Failed to fetch map, HTTP {}", res.status);
                                            let _ = tx.send(MapDownloadEvent::Error(format!(
                                                "HTTP Error: {}",
                                                res.status
                                            )));
                                            return std::ops::ControlFlow::Break(());
                                        }
                                        log::info!(
                                            "Server map response ok! headers: {:?}",
                                            res.headers
                                        );
                                        let cl = res
                                            .headers
                                            .get("content-length")
                                            .or_else(|| res.headers.get("Content-Length"));
                                        if let Some(cl) = cl {
                                            if let Ok(len) = cl.parse::<usize>() {
                                                *total_bytes.lock().unwrap() = len;
                                                log::info!("Map content-length parsed as: {}", len);
                                            } else {
                                                log::warn!(
                                                    "Failed to parse content-length: {}",
                                                    cl
                                                );
                                            }
                                        } else {
                                            log::warn!("No content-length header received!");
                                        }
                                        std::ops::ControlFlow::Continue(())
                                    }
                                    Ok(ehttp::streaming::Part::Chunk(chunk)) => {
                                        if chunk.is_empty() {
                                            let final_bytes =
                                                std::mem::take(&mut *accumulated.lock().unwrap());
                                            log::info!(
                                                "Map fully downloaded: {} bytes",
                                                final_bytes.len()
                                            );
                                            let _ = tx.send(MapDownloadEvent::MapReady(
                                                map_name_for_closure.clone(),
                                                final_bytes,
                                            ));
                                            return std::ops::ControlFlow::Break(());
                                        }
                                        let mut acc = accumulated.lock().unwrap();
                                        acc.extend_from_slice(&chunk);
                                        let downloaded = acc.len();
                                        let total = *total_bytes.lock().unwrap();
                                        if total > 0 {
                                            let progress =
                                                ((downloaded as f64 / total as f64) * 100.0) as u8;
                                            let _ = tx.send(MapDownloadEvent::Progress(
                                                map_name_for_closure.clone(),
                                                progress.min(99),
                                            ));
                                            if downloaded % 524288 < chunk.len() {
                                                log::info!(
                                                    "Downloading map... {} / {} bytes ({}%)",
                                                    downloaded,
                                                    total,
                                                    progress.min(99)
                                                );
                                            }
                                        } else if downloaded % 524288 < chunk.len() {
                                            log::info!(
                                                "Downloading map... {} bytes (unknown total)",
                                                downloaded
                                            );
                                        }
                                        std::ops::ControlFlow::Continue(())
                                    }
                                    Err(err) => {
                                        log::error!("Failed to fetch map: {}", err);
                                        let _ = tx.send(MapDownloadEvent::Error(format!(
                                            "Fetch error: {}",
                                            err
                                        )));
                                        std::ops::ControlFlow::Break(())
                                    }
                                },
                            );
                        }
                    }
                }
            } // end loop
        } // end if !ws_disconnected
        // end if let Some(c)

        ProcessWsResult {
            ws_disconnected,
            switch_to_relay,
            exit_to_menu_after_net,
            pending_rematch,
        }
    }
}
