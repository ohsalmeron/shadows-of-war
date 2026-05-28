use crate::app::SowApp;
use crate::{get_build_version, get_maps_url, spawn_sow_client_connect, MapDownloadEvent};
use sow_ui::app::ClientPhase;
use web_time::{Duration, Instant};

impl SowApp {
    fn fetch_map_catalog_if_needed(&mut self) {
        if self.ui.app.asset_loader.map_catalog.is_some()
            || self.ui.app.asset_loader.catalog_in_flight
        {
            return;
        }
        self.ui.app.asset_loader.catalog_in_flight = true;
        let url = format!(
            "{}/catalog.bin",
            get_maps_url().trim_end_matches('/')
        );
        let tx = self.tasks.map_tx.clone();
        let request = ehttp::Request::get(&url);
        ehttp::fetch(request, move |result: ehttp::Result<ehttp::Response>| {
            if let Ok(res) = result {
                if res.ok {
                    if let Ok(catalog) = sow_core::map_file::parse_catalog(&res.bytes) {
                        let _ = tx.send(MapDownloadEvent::CatalogReady(catalog.entries));
                        return;
                    }
                }
            }
            log::warn!("Failed to fetch map catalog.bin");
            let _ = tx.send(MapDownloadEvent::CatalogReady(Vec::new()));
        });
    }

    pub fn update_net(&mut self, now: Instant) {
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
            sow_ui::app::ClientPhase::MainMenu | sow_ui::app::ClientPhase::Splash
        ) {
            self.fetch_map_catalog_if_needed();
        }

        // 3-second relay timeout check
        if self.ws_on_relay() && self.net.client.is_none() && !self.net.is_offline {
            if self.net.relay_connect_start.is_none() {
                self.net.relay_connect_start = Some(now);
                self.net.relay_retry_count = 0;
            }
            if let Some(start) = self.net.relay_connect_start {
                if now.duration_since(start) >= Duration::from_secs(3) {
                    log::error!("Relay connection/reconnection timed out after 3 seconds");
                    self.net.relay_connect_start = None;
                    self.net.relay_retry_count = 0;
                    self.ui.app.main_menu_state.error_message = Some("Failed to connect to the game server. Please check your internet connection.".to_string());
                    self.begin_exit_to_main_menu(true);
                }
            }
        } else {
            self.net.relay_connect_start = None;
        }

        // No fake map download simulation! Progress is real!

        while let Ok(res) = self.net.connect_rx.try_recv() {
            match res {
                Ok(client) => {
                    log::warn!("[CLIENT NET] ✅ Received successfully connected WebSocket client from channel!");
                    self.ui.app.main_menu_state.is_connected = true;
                    self.ui.app.main_menu_state.is_connecting = false;
                    self.net.ws_connect_fail_backoff_ms = 400;
                    if self.ws_on_relay() {
                        self.net.relay_connect_start = None;
                        self.net.relay_retry_count = 0;
                    }

                    if self.ui.app.phase == sow_ui::app::ClientPhase::Playing {
                        if let (Some(lid), Some(pid)) =
                            (self.sim.my_lobby_id, self.sim.my_player_id)
                        {
                            log::info!("Sent Ready to Relay server on reconnect/playing!");
                            client.send(
                                bincode::serialize(&sow_core::protocol::ClientMessage::Ready {
                                    lobby_id: lid,
                                    player_id: pid,
                                })
                                .unwrap(),
                            );
                        }
                    } else if self.net.pending_lobby_rejoin {
                        log::info!("Re-sending Join to lobby after hop");
                        let join_msg = sow_core::protocol::ClientMessage::Join {
                            name: self.ui.app.main_menu_state.player_name.clone(),
                            is_observer: false,
                            target_lobby_id: self.sim.my_lobby_id.or(self
                                .ui
                                .app
                                .main_menu_state
                                .pending_join_lobby_id),
                            build_version: get_build_version(),
                            clan_tag: self.ui.app.main_menu_state.clan_tag.clone(),
                            civilization: self.ui.app.main_menu_state.selected_civilization,
                            leader: self.ui.app.main_menu_state.selected_leader,
                        };
                        if let Ok(json) = bincode::serialize(&join_msg) {
                            client.send(json);
                        }
                        self.net.pending_lobby_rejoin = false;
                    }
                    self.net.client = Some(client);
                }
                Err(e) => {
                    log::debug!("Failed to connect: {}", e);
                    self.ui.app.main_menu_state.is_connected = false;
                    self.ui.app.main_menu_state.is_connecting = false;
                    if self.ws_on_relay() && !self.net.is_offline {
                        self.net.relay_retry_count += 1;
                        if self.net.relay_retry_count >= 3 {
                            log::error!("Relay connection failed after 3 attempts");
                            self.net.relay_connect_start = None;
                            self.net.relay_retry_count = 0;
                            self.ui.app.main_menu_state.error_message = Some(
                                "Failed to connect to the game server after 3 attempts."
                                    .to_string(),
                            );
                            self.begin_exit_to_main_menu(true);
                        } else {
                            log::warn!(
                                "Relay connection failed; retrying rapid connection attempt {}/3",
                                self.net.relay_retry_count + 1
                            );
                            self.net.ws_connect_fail_backoff_ms = 100;
                            self.net.ws_connect_not_before = now + Duration::from_millis(100);
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

        let mut ws_disconnected = false;
        #[cfg(target_arch = "wasm32")]
        if let Some(c) = self.net.client.as_ref() {
            if c.is_socket_closed() {
                ws_disconnected = true;
            }
        }

        let mut switch_to_relay = None;
        let mut exit_to_menu_after_net = false;

        // Process network messages
        if let Some(c) = self.net.client.as_ref() {
            if !ws_disconnected {
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
                                "[NET] Failed to deserialize server message ({} bytes): {}",
                                msg.len(),
                                e
                            );
                            continue;
                        }
                    };

                    match server_msg {
                        ServerMessage::Start(start_msg) => {
                            log::info!(
                                "Received ServerStartMessage; entering Splash phase immediately"
                            );
                            if self.ui.app.phase != sow_ui::app::ClientPhase::Splash {
                                self.ui.app.phase = sow_ui::app::ClientPhase::Splash;
                                let lang = self.ui.app.settings_state.language;
                                self.ui.app.splash_state.reset_anim(
                                    sow_ui::ui::loading_screen::SplashJob::EnterGame,
                                    lang,
                                );
                            }
                            self.ui.app.main_menu_state.is_waiting = false;
                            self.ui.app.main_menu_state.pending_join_lobby_id = None;
                            self.ui.app.main_menu_state.joined_lobby_id = None;
                            self.ui.app.hud_state.sync_state = None; // REMOVE the modal overlay
                            self.sim.my_player_id = start_msg.my_player_id;

                            if let Some(relay_port) = start_msg.relay_port {
                                switch_to_relay = Some(relay_port);
                            }

                            self.tasks.engine_init_queued_msg = Some(*start_msg);
                            if switch_to_relay.is_some() {
                                break;
                            }
                        }
                        ServerMessage::Turn(turn_msg) => {
                            self.sim.turn_queue.push_back(turn_msg.turn);
                            self.ui.app.hud_state.sync_state = None;
                        }
                        ServerMessage::SyncState(sync_msg) => {
                            self.ui.app.hud_state.sync_state = Some(sync_msg.clone());
                            if self.ui.app.main_menu_state.is_waiting {
                                self.ui.app.main_menu_state.wait_timer_secs =
                                    sync_msg.time_remaining;

                                // All clients ready: go to loader immediately
                                if sync_msg.is_starting {
                                    log::info!(
                                        "[LOBBY] All ready (is_starting), entering loader screen"
                                    );
                                    if self.ui.app.phase != sow_ui::app::ClientPhase::Splash {
                                        self.ui.app.phase = sow_ui::app::ClientPhase::Splash;
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
                                            self.ui.app.main_menu_state.lobbies.push(
                                                sow_core::protocol::LobbyInfo {
                                                    id,
                                                    num_players: sync_msg.players.len() as u32,
                                                    max_players: 8,
                                                    is_counting_down: sync_msg.time_remaining > 0.0
                                                        && sync_msg.time_remaining < 30.0,
                                                    timer_secs: sync_msg.time_remaining,
                                                    map_name: "Loading...".to_string(),
                                                    game_mode: "FFA".to_string(),
                                                    players: sync_msg.players.clone(),
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
                            if self.ui.app.phase != sow_ui::app::ClientPhase::Splash {
                                self.ui.app.main_menu_state.lobbies = broadcast.lobbies.clone();
                            }

                            let maps_base = get_maps_url();
                            let (_, maps_to_fetch) = self
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
                                    move |result: ehttp::Result<ehttp::streaming::Part>| {
                                        match result {
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
                                                    let final_bytes = std::mem::take(
                                                        &mut *accumulated.lock().unwrap(),
                                                    );
                                                    let _ = tx.send(MapDownloadEvent::MapReady(
                                                        map_name_for_closure.clone(),
                                                        final_bytes,
                                                    ));
                                                    return std::ops::ControlFlow::Break(());
                                                }
                                                accumulated
                                                    .lock()
                                                    .unwrap()
                                                    .extend_from_slice(&chunk);
                                                std::ops::ControlFlow::Continue(())
                                            }
                                            Err(_) => std::ops::ControlFlow::Break(()),
                                        }
                                    },
                                );
                            }

                            if self.ui.app.main_menu_state.is_waiting {
                                let key = self
                                    .sim
                                    .my_lobby_id
                                    .or(self.ui.app.main_menu_state.joined_lobby_id)
                                    .or(self.ui.app.main_menu_state.pending_join_lobby_id);
                                if let Some(l_id) = key {
                                    if let Some(lobby) =
                                        broadcast.lobbies.iter().find(|l| l.id == l_id)
                                    {
                                        if lobby.is_counting_down {
                                            self.ui.app.main_menu_state.wait_timer_secs =
                                                lobby.timer_secs;
                                        }
                                    }
                                }
                            }
                        }
                        ServerMessage::VersionUpdate { version } => {
                            log::info!("Received version update: {}", version);
                        }
                        ServerMessage::LobbyClosed(closed) => {
                            log::warn!("Lobby {} closed: {}", closed.lobby_id, closed.reason);
                            self.ui.app.hud_state.sync_state = None;
                            self.sim.my_lobby_id = None;
                            self.sim.my_player_id = None;

                            if closed.reason.contains("Requeueing") {
                                log::info!("Auto-requeueing to a new lobby...");
                                self.ui.app.phase = ClientPhase::MainMenu;
                                self.ui.app.main_menu_state.is_waiting = true;
                                let join_msg = sow_core::protocol::ClientMessage::Join {
                                    name: self.ui.app.main_menu_state.player_name.clone(),
                                    is_observer: false,
                                    target_lobby_id: None,
                                    build_version: get_build_version(),
                                    clan_tag: self.ui.app.main_menu_state.clan_tag.clone(),
                                    civilization: self.ui.app.main_menu_state.selected_civilization,
                                    leader: self.ui.app.main_menu_state.selected_leader,
                                };
                                c.send(bincode::serialize(&join_msg).unwrap());
                            } else {
                                exit_to_menu_after_net = true;
                            }
                        }
                        ServerMessage::JoinFailed(fail) => {
                            log::warn!("Join failed: {}", fail.reason);
                            if fail.reason == "VERSION_MISMATCH" {
                                log::info!("Version mismatch — prompting user to update...");
                                self.ui.update_available = true;
                            }
                            self.ui.app.main_menu_state.is_waiting = false;
                            self.ui.app.main_menu_state.pending_join_lobby_id = None;
                            self.ui.app.main_menu_state.joined_lobby_id = None;
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
                            self.ui.app.main_menu_state.joined_lobby_id = Some(ack.lobby_id);

                            let map_name = ack.map_name.clone();
                            self.ui.app.main_menu_state.downloading_map_name =
                                Some(map_name.clone());

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
                                    bincode::serialize(&sow_core::protocol::ClientMessage::Ready {
                                        lobby_id: ack.lobby_id,
                                        player_id: ack.player_id,
                                    })
                                    .unwrap(),
                                );
                            } else {
                                let tx = self.tasks.map_tx.clone();
                                self.ui.app.main_menu_state.is_downloading_map = true;
                                self.ui.app.main_menu_state.cached_map = None;

                                let maps_base = get_maps_url();
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
                                let total_bytes =
                                    std::sync::Arc::new(std::sync::Mutex::new(0usize));

                                ehttp::streaming::fetch(
                                    request,
                                    move |result: ehttp::Result<ehttp::streaming::Part>| {
                                        match result {
                                            Ok(ehttp::streaming::Part::Response(res)) => {
                                                if !res.ok {
                                                    log::error!(
                                                        "Failed to fetch map, HTTP {}",
                                                        res.status
                                                    );
                                                    let _ = tx.send(MapDownloadEvent::Error(
                                                        format!("HTTP Error: {}", res.status),
                                                    ));
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
                                                        log::info!(
                                                            "Map content-length parsed as: {}",
                                                            len
                                                        );
                                                    } else {
                                                        log::warn!(
                                                            "Failed to parse content-length: {}",
                                                            cl
                                                        );
                                                    }
                                                } else {
                                                    log::warn!(
                                                        "No content-length header received!"
                                                    );
                                                }
                                                std::ops::ControlFlow::Continue(())
                                            }
                                            Ok(ehttp::streaming::Part::Chunk(chunk)) => {
                                                if chunk.is_empty() {
                                                    let final_bytes = std::mem::take(
                                                        &mut *accumulated.lock().unwrap(),
                                                    );
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
                                                    let progress = ((downloaded as f64
                                                        / total as f64)
                                                        * 100.0)
                                                        as u8;
                                                    let _ = tx.send(MapDownloadEvent::Progress(
                                                        map_name_for_closure.clone(),
                                                        progress.min(99),
                                                    ));
                                                    if downloaded % 524288 < chunk.len() {
                                                        log::info!("Downloading map... {} / {} bytes ({}%)", downloaded, total, progress.min(99));
                                                    }
                                                } else if downloaded % 524288 < chunk.len() {
                                                    log::info!("Downloading map... {} bytes (unknown total)", downloaded);
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
                                        }
                                    },
                                );
                            }
                        }
                    }
                } // end loop
            } // end if !ws_disconnected
        } // end if let Some(c)

        if exit_to_menu_after_net {
            self.begin_exit_to_main_menu(true);
        }

        if let Some(relay_port) = switch_to_relay {
            log::info!(
                "[CLIENT NET] Handoff from Master Orchestrator -> Game Relay on port {}",
                relay_port
            );
            if let Ok(mut url) = url::Url::parse(&self.net.ws_url) {
                if url.scheme() == "wss" || self.net.ws_url.contains("shadowsofwar.io") {
                    let new_path = format!("/relay/{}/ws/", relay_port);
                    url.set_path(&new_path);
                } else {
                    let _ = url.set_port(Some(relay_port));
                }
                self.net.ws_url = url.to_string();
                self.net.client = None; // Drop orchestrator connection
                self.ui.app.main_menu_state.server_address = self.net.ws_url.clone();
                self.ui.app.main_menu_state.is_connecting = true; // PREVENT DUPLICATE CONNECTIONS
                ws_disconnected = false;

                // Clear stale connections
                while self.net.connect_rx.try_recv().is_ok() {
                    log::warn!("[CLIENT NET] 🗑️  Purged stale connection from channel during handoff to relay!");
                }

                self.net.relay_connect_start = Some(now);
                self.net.relay_retry_count = 0;

                log::warn!(
                    "[CLIENT NET] 🚀 Spawning WS connection task to RELAY: {}",
                    self.net.ws_url
                );
                #[cfg(target_arch = "wasm32")]
                crate::spawn_sow_client_connect(self.net.ws_url.clone(), &self.net.connect_tx);
                #[cfg(not(target_arch = "wasm32"))]
                crate::spawn_sow_client_connect(
                    self.net.ws_url.clone(),
                    &self.net.connect_tx,
                    &self.tokio_rt,
                );
            }
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
                log::warn!("[CLIENT NET] Disconnected outside match; reconnecting to orchestrator");
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
            log::warn!(
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
