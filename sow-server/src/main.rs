mod lobby;
mod map_catalog;

use futures_util::{SinkExt, StreamExt};
use lobby::{
    build_lobby_broadcast, force_start, is_host_teardown, join_player, kick_player, leave_player,
    lobby_to_info, master_tick, notify_lobby_closed, set_player_team, sync_host_lobby_to_members,
    ServerLobby,
};
use redis::Commands;
use sow_core::protocol::{
    ServerJoinAckMessage, ServerJoinFailedMessage, ServerLobbiesBroadcastMessage,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_tungstenite::tungstenite::protocol::Message;

const REDIS_PORTS_KEY: &str = "sow:ports";

fn relay_bin() -> String {
    std::env::var("SOW_RELAY_BIN").unwrap_or_else(|_| "./sow-relay".to_string())
}
const RELAY_PORT_MIN: u16 = 25590;
const RELAY_PORT_MAX: u16 = 26500;

fn find_free_port(redis_con: &mut redis::Connection) -> Option<u16> {
    let occupied: std::collections::HashSet<u16> =
        redis_con.smembers(REDIS_PORTS_KEY).unwrap_or_default();
    (RELAY_PORT_MIN..=RELAY_PORT_MAX).find(|p| !occupied.contains(p))
}

/// All server events, comming from client
enum ServerEvent {
    Join {
        client_tx: mpsc::Sender<Vec<u8>>,
        name: String,
        clan_tag: String,
        civilization: sow_core::player::Civilization,
        leader: sow_core::player::Leader,
        target_lobby_id: Option<u64>,
        host_private: bool,
        build_version: String,
        database_account_id: Option<String>,
        host_config: Option<Box<sow_core::game_config::GameConfig>>,
        password: Option<String>,
        ip: String,
    },

    Leave {
        lobby_id: u64,
        player_id: u16,
    },
    Ready {
        lobby_id: u64,
        player_id: u16,
    },
    MapDownloadProgress {
        lobby_id: u64,
        player_id: u16,
        progress: u8,
    },
    ForceStart {
        lobby_id: u64,
        player_id: u16,
    },
    Kick {
        lobby_id: u64,
        requester_id: u16,
        target_id: u16,
        ban: bool,
    },
    SetTeam {
        lobby_id: u64,
        requester_id: u16,
        target_id: u16,
    },
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let redis_url =
        std::env::var("SOW_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
    let redis_client = redis::Client::open(redis_url).expect("Failed to connect to Redis");
    let redis_con = Arc::new(std::sync::Mutex::new(
        redis_client
            .get_connection()
            .expect("Failed to get Redis connection"),
    ));
    {
        let mut con = redis_con.lock().unwrap();
        let _: () = con.del(REDIS_PORTS_KEY).unwrap_or_default();
        log::info!("Wiped stale relay port allocations from Redis. Ready for clean start.");
        let occupied: std::collections::HashSet<u16> =
            con.smembers(REDIS_PORTS_KEY).unwrap_or_default();
        log::info!("Redis connected. Occupied relay ports: {:?}", occupied);
    }

    let mut games: Vec<ServerLobby> = Vec::new();
    let mut next_lobby_id: u64 = 1;

    let maps_root = map_catalog::maps_root();
    map_catalog::init(&maps_root);

    master_tick(&mut games, &mut next_lobby_id);

    let games_state = Arc::new(Mutex::new(games));
    let next_id_state = Arc::new(Mutex::new(next_lobby_id));

    let (global_tx, _rx) = broadcast::channel::<Vec<u8>>(100);
    let (event_tx, mut event_rx) = mpsc::channel::<ServerEvent>(1000);

    let games_clone = Arc::clone(&games_state);
    let next_id_clone = Arc::clone(&next_id_state);
    let global_tx_clone = global_tx.clone();
    let redis_clone = Arc::clone(&redis_con);

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        let mut tick_count: u64 = 0;
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let tick_start = tokio::time::Instant::now();
                    tick_count += 1;

                    let mut games = games_clone.lock().await;
                    let mut nid = next_id_clone.lock().await;
                    master_tick(&mut games, &mut nid);

                    for lobby in &mut *games {
                        if lobby.phase == lobby::LobbyPhase::Loading && lobby.countdown_secs <= 3.0 && lobby.relay_port.is_none() {
                            let mut rcon = redis_clone.lock().unwrap();
                            let relay_port = match find_free_port(&mut rcon) {
                                Some(p) => p,
                                None => {
                                    log::error!("No available relay ports in {}-{}", RELAY_PORT_MIN, RELAY_PORT_MAX);
                                    continue;
                                }
                            };
                            let _: () = rcon.sadd(REDIS_PORTS_KEY, relay_port).unwrap_or_default();
                            drop(rcon);

                            let mut players_json = Vec::new();
                            let mut player_ids = Vec::new();
                            for p in &lobby.players {
                                players_json.push(serde_json::json!({
                                    "player_id": p.player_id,
                                    "name": p.name,
                                    "database_account_id": p.database_account_id,
                                }));
                                if let Some(acc_id) = &p.database_account_id {
                                    player_ids.push(acc_id.clone());
                                }
                            }

                            if !player_ids.is_empty() {
                                let match_id = lobby.id.to_string();
                                tokio::spawn(async move {
                                    let db_base_url = std::env::var("SOW_DB_URL")
                                        .unwrap_or_else(|_| "http://127.0.0.1:25585".to_string());
                                    let secret_token = std::env::var("SOW_DB_SECRET")
                                        .unwrap_or_else(|_| "sow_db_dev_secret_123_change_me_in_prod".to_string());

                                    let url = format!("{}/match/start", db_base_url.trim_end_matches('/'));
                                    let client = reqwest::Client::new();

                                    let payload = serde_json::json!({
                                        "match_id": match_id,
                                        "player_ids": player_ids,
                                    });

                                    match client.post(&url)
                                        .header("Authorization", format!("Bearer {}", secret_token))
                                        .json(&payload)
                                        .send()
                                        .await
                                    {
                                        Ok(res) => {
                                            if res.status().is_success() {
                                                log::info!("Successfully registered starting match {} with database.", match_id);
                                            } else {
                                                log::warn!("Database match/start endpoint returned status: {}", res.status());
                                            }
                                        }
                                        Err(e) => {
                                            log::error!("Failed to register starting match {} with database: {}", match_id, e);
                                        }
                                    }
                                });
                            }

                            let relay_config = serde_json::json!({
                                "lobby_id": lobby.id,
                                "tick_number": 0,
                                "active_empty_secs": lobby.active_empty_secs,
                                "players": players_json,
                                "tick_rate_ms": lobby.config.tick_rate_ms,
                            });

                            let log_path = format!("relay_{}.log", relay_port);
                            let log_file = match std::fs::File::create(&log_path) {
                                Ok(f) => f,
                                Err(e) => {
                                    log::error!("[RELAY] Cannot create log file {}: {}", log_path, e);
                                    let mut rcon = redis_clone.lock().unwrap();
                                    let _: () = rcon.srem(REDIS_PORTS_KEY, relay_port).unwrap_or_default();
                                    continue;
                                }
                            };
                            let log_file2 = match log_file.try_clone() {
                                Ok(f) => f,
                                Err(e) => {
                                    log::error!("[RELAY] Cannot clone log file handle: {}", e);
                                    let mut rcon = redis_clone.lock().unwrap();
                                    let _: () = rcon.srem(REDIS_PORTS_KEY, relay_port).unwrap_or_default();
                                    continue;
                                }
                            };
                            log::info!("[RELAY] Spawning relay for lobby {} on port {}", lobby.id, relay_port);
                            let mut cmd = tokio::process::Command::new(relay_bin());
                            cmd.arg("--port").arg(relay_port.to_string())
                               .arg("--lobby-json").arg(relay_config.to_string())
                               .stdin(std::process::Stdio::null())
                               .stdout(std::process::Stdio::from(log_file2))
                               .stderr(std::process::Stdio::from(log_file));

                            match cmd.spawn() {
                                Ok(_) => {
                                    log::info!("Spawned sow-relay for lobby {} on port {}", lobby.id, relay_port);
                                    lobby.relay_port = Some(relay_port);
                                }
                                Err(e) => {
                                    log::error!("Failed to spawn relay for lobby {}: {}", lobby.id, e);
                                    let mut rcon = redis_clone.lock().unwrap();
                                    let _: () = rcon.srem(REDIS_PORTS_KEY, relay_port).unwrap_or_default();
                                }
                            }
                        }
                    }

                    // Extract lobbies ready for relay
                    let mut ready_lobbies = Vec::new();
                    let mut i = 0;
                    while i < games.len() {
                        if games[i].phase == lobby::LobbyPhase::ReadyForRelay {
                            ready_lobbies.push(games.remove(i));
                        } else {
                            i += 1;
                        }
                    }

                    // Throttle the global lobbies broadcast to run once every 1,000ms (10 ticks)
                    if tick_count % 10 == 0 {
                        let lobbies_info = build_lobby_broadcast(&games);
                        log::debug!("[BROADCAST] {} lobbies in broadcast", lobbies_info.len());

                        let broadcast_msg = ServerLobbiesBroadcastMessage { lobbies: lobbies_info };
                        match bincode::serialize(&sow_core::protocol::ServerMessage::LobbiesBroadcast(broadcast_msg)) {
                            Ok(json) => { let _ = global_tx_clone.send(json); }
                            Err(e) => { log::error!("[BROADCAST] Failed to serialize LobbiesBroadcast: {}", e); }
                        }
                    }

                    // Precise latency performance metric logger
                    let elapsed = tick_start.elapsed().as_millis();
                    if elapsed > 10 {
                        log::warn!("[PERF] Event loop lag detected! Master tick execution took {}ms", elapsed);
                    }

                    for lobby in ready_lobbies {
                        if let Some(relay_port) = lobby.relay_port {
                            let mut player_infos = Vec::new();
                            for (i, p) in lobby.players.iter().enumerate() {
                                let (team, color) = if lobby.game_mode == "Teams" {
                                    // Honor the lobby-stage assignment the host set; fall back to
                                    // alternating by join index if a player was never assigned.
                                    let team = p.team.unwrap_or(if i % 2 == 0 {
                                        sow_core::protocol::Team::Red
                                    } else {
                                        sow_core::protocol::Team::Blue
                                    });
                                    let color = match team {
                                        sow_core::protocol::Team::Red => [1.0, 0.2, 0.2],
                                        sow_core::protocol::Team::Blue => [0.2, 0.5, 1.0],
                                    };
                                    (Some(team), color)
                                } else {
                                    (None, p.leader.filler_rgb())
                                };

                                player_infos.push(sow_core::protocol::PlayerInfo {
                                    id: p.player_id,
                                    name: if p.clan_tag.is_empty() { p.name.clone() } else { format!("[{}] {}", p.clan_tag, p.name) },
                                    player_type: sow_core::player::PlayerType::Human,
                                    color,
                                    team,
                                    spawn_x: 0,
                                    spawn_y: 0,
                                    civilization: p.civilization,
                                    leader: p.leader,
                                });
                            }

                            let start_msg = sow_core::protocol::ServerStartMessage {
                                config: lobby.config.clone(),
                                my_player_id: None,
                                lobby_id: Some(lobby.id),
                                seed: lobby.seed,
                                players: player_infos,
                                missed_turns: vec![],
                                map_data: None,
                                relay_port: Some(relay_port),
                            };

                            log::info!("[RELAY] Handing off lobby {} to relay port {}", lobby.id, relay_port);
                            for p in &lobby.players {
                                let mut player_start = start_msg.clone();
                                player_start.my_player_id = Some(p.player_id);
                                match bincode::serialize(&sow_core::protocol::ServerMessage::Start(Box::new(player_start))) {
                                    Ok(json) => { let _ = p.tx.try_send(json); }
                                    Err(e) => { log::error!("[RELAY] Failed to serialize Start for player {} in lobby {}: {}", p.player_id, lobby.id, e); }
                                }
                            }
                        } else {
                            log::error!("[RELAY] Lobby {} has no relay_port — cannot hand off. Sending LobbyClosed.", lobby.id);
                            let msg = sow_core::protocol::ServerLobbyClosedMessage {
                                lobby_id: lobby.id,
                                reason: "Server failed to allocate relay".to_string(),
                                rematch_lobby_id: None,
                            };
                            if let Ok(json) = bincode::serialize(&sow_core::protocol::ServerMessage::LobbyClosed(msg)) {
                                for p in &lobby.players {
                                    let _ = p.tx.try_send(json.clone());
                                }
                            }
                        }
                    }
                }

                Some(event) = event_rx.recv() => {
                    let mut games = games_clone.lock().await;
                    let mut nid = next_id_clone.lock().await;
                    match event {
                        ServerEvent::Join { client_tx, name, clan_tag, civilization, leader, target_lobby_id, host_private, build_version, database_account_id, host_config, password, ip } => {
                            log::info!("Player {} (clan: {}, ip: {}) joining with version: {}", name, clan_tag, ip, build_version);
                            match join_player(&mut games, &mut nid, name, clan_tag, civilization, leader, client_tx.clone(), target_lobby_id, host_private, database_account_id, host_config, password, ip) {
                                Ok((lobby_id, player_id, map_name, is_private)) => {
                                    let lobby_info = games.iter().find(|g| g.id == lobby_id).map(lobby_to_info);
                                    let ack = ServerJoinAckMessage { lobby_id, player_id, map_name, is_private, lobby_info };
                                    match bincode::serialize(&sow_core::protocol::ServerMessage::JoinAck(ack)) {
                                        Ok(json) => { let _ = client_tx.try_send(json); }
                                        Err(e) => { log::error!("[JOIN] Failed to serialize JoinAck for player {} in lobby {}: {}", player_id, lobby_id, e); }
                                    }
                                    if let Some(lobby) = games.iter().find(|g| g.id == lobby_id) {
                                        sync_host_lobby_to_members(lobby);
                                    }
                                }
                                Err(reason) => {
                                    log::warn!("[JOIN] Join rejected: {}", reason);
                                    let fail = ServerJoinFailedMessage { reason };
                                    match bincode::serialize(&sow_core::protocol::ServerMessage::JoinFailed(fail)) {
                                        Ok(json) => { let _ = client_tx.try_send(json); }
                                        Err(e) => { log::error!("[JOIN] Failed to serialize JoinFailed: {}", e); }
                                    }
                                }
                            }
                        }

                        ServerEvent::Leave { lobby_id, player_id } => {
                            if is_host_teardown(&games, lobby_id, player_id) {
                                if let Some(lobby) = games.iter().find(|g| g.id == lobby_id) {
                                    notify_lobby_closed(lobby, "HOST_LEFT");
                                }
                                games.retain(|g| g.id != lobby_id);
                                log::info!("[LOBBY] Host {} left Custom lobby {} — lobby dropped, members returned to menu", player_id, lobby_id);
                            } else {
                                leave_player(&mut games, lobby_id, player_id);
                                if let Some(lobby) = games.iter().find(|g| g.id == lobby_id) {
                                    sync_host_lobby_to_members(lobby);
                                }
                            }
                        }
                        ServerEvent::Kick { lobby_id, requester_id, target_id, ban } => {
                            kick_player(&mut games, lobby_id, requester_id, target_id, ban);
                        }
                        ServerEvent::SetTeam { lobby_id, requester_id, target_id } => {
                            set_player_team(&mut games, lobby_id, requester_id, target_id);
                        }
                        ServerEvent::Ready { lobby_id, player_id } => {
                            if let Some(lobby) = games.iter_mut().find(|g| g.id == lobby_id) {
                                lobby.ready_players.insert(player_id);
                                sync_host_lobby_to_members(lobby);
                            } else {
                                log::warn!("[READY] Player {} sent Ready for unknown lobby {}", player_id, lobby_id);
                            }
                        }
                        ServerEvent::MapDownloadProgress { lobby_id, player_id, progress } => {
                            if let Some(lobby) = games.iter_mut().find(|g| g.id == lobby_id) {
                                if let Some(p) = lobby.players.iter_mut().find(|p| p.player_id == player_id) {
                                    p.download_progress = progress;
                                } else {
                                    log::warn!("[PROGRESS] Player {} not found in lobby {} for download progress update", player_id, lobby_id);
                                }
                                sync_host_lobby_to_members(lobby);
                            } else {
                                log::warn!("[PROGRESS] Lobby {} not found for player {} progress update", lobby_id, player_id);
                            }
                        }
                        ServerEvent::ForceStart { lobby_id, player_id } => {
                            log::info!("[EVENT] ForceStart received lobby={} player={}", lobby_id, player_id);
                            force_start(&mut games, lobby_id, player_id);
                        }
                    }
                }
            }
        }
    });

    let addr = std::env::var("SOW_WS_LISTEN").unwrap_or_else(|_| "0.0.0.0:25565".to_string());

    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");
    log::info!("SOW-SERVER listening on ws://{}", addr);

    // HTTP Static File Server for maps and Admin Dashboard
    let games_for_axum = Arc::clone(&games_state);
    let redis_client_for_axum = redis_client.clone();
    tokio::spawn(async move {
        let root = maps_root.clone();
        let state = AppState {
            games: games_for_axum,
            redis_client: redis_client_for_axum,
        };
        let catalog_route = axum::Router::new()
            .route(
                "/maps/catalog.bin",
                axum::routing::get(|| async {
                    axum::response::Response::builder()
                        .header("Content-Type", "application/octet-stream")
                        .header("Cache-Control", "public, max-age=60")
                        .body(axum::body::Body::from(
                            map_catalog::catalog_bytes().to_vec(),
                        ))
                        .unwrap()
                }),
            )
            .route(
                "/maps/catalog.json",
                axum::routing::get(catalog_json_handler),
            )
            .route(
                "/lobbies.json",
                axum::routing::get(lobbies_json_handler),
            )
            .route(
                "/admin/dashboard",
                axum::routing::get(|| async {
                    axum::response::Html(include_str!("admin_dashboard.html"))
                }),
            )
            .route(
                "/admin/api/status",
                axum::routing::get(admin_status),
            )
            .with_state(state);

        let app = catalog_route
            .nest_service(
                "/maps",
                tower_http::services::ServeDir::new(root).precompressed_br(),
            )
            .layer(tower_http::cors::CorsLayer::permissive());
        let http_addr =
            std::env::var("SOW_MAPS_HTTP_LISTEN").unwrap_or_else(|_| "0.0.0.0:25566".to_string());
        log::info!("SOW-SERVER HTTP serving maps and admin on http://{}", http_addr);
        let listener = tokio::net::TcpListener::bind(&http_addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });

    while let Ok((stream, addr)) = listener.accept().await {
        let mut global_rx = global_tx.subscribe();
        let ev_tx = event_tx.clone();
        let games_state_conn = Arc::clone(&games_state);
        tokio::spawn(async move {
            let ip_cell = Arc::new(std::sync::Mutex::new(addr.ip().to_string()));
            let ip_cell_clone = Arc::clone(&ip_cell);
            let ws_stream = match tokio_tungstenite::accept_hdr_async(stream, move |req: &tokio_tungstenite::tungstenite::handshake::server::Request, response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                if let Some(real_ip) = req.headers().get("X-Real-IP") {
                    if let Ok(ip) = real_ip.to_str() {
                        if let Ok(mut guard) = ip_cell_clone.lock() {
                            *guard = ip.to_string();
                        }
                    }
                } else if let Some(forwarded) = req.headers().get("X-Forwarded-For") {
                    if let Ok(ip) = forwarded.to_str() {
                        if let Some(first_ip) = ip.split(',').next() {
                            if let Ok(mut guard) = ip_cell_clone.lock() {
                                *guard = first_ip.trim().to_string();
                            }
                        }
                    }
                }
                Ok(response)
            }).await {
                Ok(ws) => ws,
                Err(e) => {
                    log::error!("Handshake failed: {}", e);
                    return;
                }
            };
            let ip_str = if let Ok(guard) = ip_cell.lock() {
                guard.clone()
            } else {
                addr.ip().to_string()
            };
            let (mut write, mut read) = ws_stream.split();
            log::info!("Client connected from IP: {}", ip_str);

            let (direct_tx, mut direct_rx) = mpsc::channel::<Vec<u8>>(100);

            // Send immediate single-cast LobbiesBroadcast snapshot on connection so the home menu loads instantly
            {
                let games_guard = games_state_conn.lock().await;
                let lobbies_info = build_lobby_broadcast(&games_guard);
                let broadcast_msg = ServerLobbiesBroadcastMessage { lobbies: lobbies_info };
                if let Ok(json) = bincode::serialize(&sow_core::protocol::ServerMessage::LobbiesBroadcast(broadcast_msg)) {
                    let _ = direct_tx.try_send(json);
                }
            }

            let mut my_lobby_id: Option<u64> = None;
            let mut my_player_id: Option<u16> = None;

            loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(msg)) => {
                                if msg.is_binary() {
                                    let data = msg.into_data();

                                    if let Ok(msg) = bincode::deserialize::<sow_core::protocol::ClientMessage>(&data) {
                                        match msg {
                                            sow_core::protocol::ClientMessage::Join { name, is_observer: _, target_lobby_id, host_private, build_version, clan_tag, civilization, leader, database_account_id, host_config, password } => {
                                                let server_version = std::env::var("SOW_BUILD_VERSION")
                                                    .unwrap_or_else(|_| std::fs::read_to_string(".version").unwrap_or_default().trim().to_string());

                                                if !server_version.is_empty() && build_version != server_version {
                                                    log::warn!("Client version mismatch: expected {}, got {}", server_version, build_version);
                                                    let fail = sow_core::protocol::ServerJoinFailedMessage { reason: "VERSION_MISMATCH".to_string() };
                                                    let json = bincode::serialize(&sow_core::protocol::ServerMessage::JoinFailed(fail)).unwrap();
                                                    let _ = direct_tx.try_send(json);
                                                    continue;
                                                }

                                                let _ = ev_tx.send(ServerEvent::Join {
                                                    name,
                                                    clan_tag,
                                                    civilization,
                                                    leader,
                                                    client_tx: direct_tx.clone(),
                                                    target_lobby_id,
                                                    host_private,
                                                    build_version,
                                                    database_account_id,
                                                    host_config,
                                                    password,
                                                    ip: ip_str.clone(),
                                                }).await;
                                            }
                                            sow_core::protocol::ClientMessage::Gameplay { .. } => {
                                                // Orchestrator ignores gameplay intents
                                            }
                                            sow_core::protocol::ClientMessage::Leave {} => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                                    let _ = ev_tx.send(ServerEvent::Leave {
                                                        lobby_id: l_id,
                                                        player_id: p_id,
                                                    }).await;
                                                }
                                                my_lobby_id = None;
                                                my_player_id = None;
                                            }
                                            sow_core::protocol::ClientMessage::Ready { lobby_id, player_id } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                                    if lobby_id == l_id && player_id == p_id {
                                                        let _ = ev_tx.send(ServerEvent::Ready {
                                                            lobby_id: l_id,
                                                            player_id: p_id,
                                                        }).await;
                                                    }
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::MapDownloadProgress { lobby_id, player_id, progress } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                                    if lobby_id == l_id && player_id == p_id {
                                                        let _ = ev_tx.send(ServerEvent::MapDownloadProgress {
                                                            lobby_id: l_id,
                                                            player_id: p_id,
                                                            progress,
                                                        }).await;
                                                    }
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::RematchRequest { lobby_id: _ } => {
                                                // Orchestrator ignores RematchRequest, it is handled by the relay server
                                            }
                                            sow_core::protocol::ClientMessage::ForceStart { lobby_id, player_id } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                                    if lobby_id == l_id && player_id == p_id {
                                                        let _ = ev_tx.send(ServerEvent::ForceStart {
                                                            lobby_id: l_id,
                                                            player_id: p_id,
                                                        }).await;
                                                    }
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::Kick { lobby_id, target_player_id } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                                    if lobby_id == l_id {
                                                        let _ = ev_tx.send(ServerEvent::Kick {
                                                            lobby_id: l_id,
                                                            requester_id: p_id,
                                                            target_id: target_player_id,
                                                            ban: false,
                                                        }).await;
                                                    }
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::Ban { lobby_id, target_player_id } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                                    if lobby_id == l_id {
                                                        let _ = ev_tx.send(ServerEvent::Kick {
                                                            lobby_id: l_id,
                                                            requester_id: p_id,
                                                            target_id: target_player_id,
                                                            ban: true,
                                                        }).await;
                                                    }
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::SetPlayerTeam { lobby_id, target_player_id } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                                    if lobby_id == l_id {
                                                        let _ = ev_tx.send(ServerEvent::SetTeam {
                                                            lobby_id: l_id,
                                                            requester_id: p_id,
                                                            target_id: target_player_id,
                                                        }).await;
                                                    }
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::Ping { client_time } => {
                                                let pong = sow_core::protocol::ServerMessage::Pong { client_time };
                                                let json = bincode::serialize(&pong).unwrap();
                                                let _ = direct_tx.try_send(json);
                                            }
                                            sow_core::protocol::ClientMessage::SubmitStats { .. } => {}
                                        }
                                        continue;
                                    }

                                    log::warn!("[SERVER] Unrecognized message");
                                }
                            }
                            _ => break,
                        }
                    }
                    Ok(broadcast_data) = global_rx.recv() => {
                        // Drop lobbies broadcast packets if the client is already in a lobby/match
                        if my_lobby_id.is_none() {
                            if write.send(Message::Binary(broadcast_data)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Some(direct_data) = direct_rx.recv() => {
                        if let Ok(server_msg) = bincode::deserialize::<sow_core::protocol::ServerMessage>(&direct_data) {
                            match server_msg {
                                sow_core::protocol::ServerMessage::JoinAck(ack) => {
                                    my_lobby_id = Some(ack.lobby_id);
                                    my_player_id = Some(ack.player_id);
                                }
                                sow_core::protocol::ServerMessage::Start(start) => {
                                    if let Some(pid) = start.my_player_id {
                                        my_player_id = Some(pid);
                                    }
                                }
                                _ => {}
                            }
                        }

                        if write.send(Message::Binary(direct_data)).await.is_err() {
                            break;
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {
                        break;
                    }
                }
            }

            if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                let _ = ev_tx
                    .send(ServerEvent::Leave {
                        lobby_id: l_id,
                        player_id: p_id,
                    })
                    .await;
            }
        });
    }
}

#[derive(Clone)]
struct AppState {
    games: Arc<Mutex<Vec<lobby::ServerLobby>>>,
    redis_client: redis::Client,
}

async fn admin_status(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> axum::response::Json<serde_json::Value> {
    let games = state.games.lock().await;
    let mut lobbies = Vec::new();
    for lobby in &*games {
        let mut players = Vec::new();
        for p in &lobby.players {
            players.push(serde_json::json!({
                "player_id": p.player_id,
                "name": p.name,
                "clan_tag": p.clan_tag,
                "civilization": format!("{:?}", p.civilization),
                "leader": format!("{:?}", p.leader),
                "download_progress": p.download_progress,
                "ip": p.ip,
                "database_account_id": p.database_account_id,
            }));
        }
        lobbies.push(serde_json::json!({
            "id": lobby.id,
            "kind": format!("{:?}", lobby.kind),
            "is_private": lobby.is_private,
            "phase": format!("{:?}", lobby.phase),
            "countdown_secs": lobby.countdown_secs,
            "relay_port": lobby.relay_port,
            "players": players,
            "map_name": lobby.config.map_name,
            "game_mode": lobby.game_mode,
        }));
    }
    drop(games);

    // Query valkey INFO
    let valkey_info = tokio::task::spawn_blocking({
        let client = state.redis_client.clone();
        move || -> serde_json::Value {
            let mut conn = match client.get_connection() {
                Ok(c) => c,
                Err(_) => return serde_json::json!({"error": "cannot connect"}),
            };
            match redis::cmd("INFO").query::<String>(&mut conn) {
                Ok(info) => {
                    let mut result = serde_json::Map::new();
                    for line in info.lines() {
                        if line.contains(':') && !line.starts_with('#') {
                            let parts: Vec<&str> = line.splitn(2, ':').collect();
                            let key = parts[0].trim();
                            let val = parts[1].trim();
                            if ["used_memory_human", "used_memory_peak_human",
                                "connected_clients", "blocked_clients",
                                "keyspace_hits", "keyspace_misses",
                                "uptime_in_seconds", "instantaneous_ops_per_sec",
                                "instantaneous_input_kbps", "instantaneous_output_kbps",
                                "total_connections_received", "total_commands_processed",
                                "expired_keys", "evicted_keys"].contains(&key) {
                                result.insert(key.to_string(), serde_json::Value::String(val.to_string()));
                            }
                        }
                    }
                    serde_json::Value::Object(result)
                }
                Err(_) => serde_json::json!({"error": "info failed"}),
            }
        }
    }).await.unwrap_or(serde_json::json!({"error": "task failed"}));

    // Query sow-database stats
    let db_stats = match reqwest::get("http://127.0.0.1:25585/internal/stats").await {
        Ok(r) => r.json::<serde_json::Value>().await.unwrap_or(serde_json::json!({"error": "db unreachable"})),
        Err(_) => serde_json::json!({"error": "db unreachable"}),
    };

    axum::response::Json(serde_json::json!({
        "lobbies": lobbies,
        "valkey": valkey_info,
        "database": db_stats,
    }))
}

async fn lobbies_json_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl axum::response::IntoResponse {
    let games = state.games.lock().await;
    let lobbies_info = lobby::build_lobby_broadcast(&games);
    axum::response::Response::builder()
        .header("Content-Type", "application/json")
        .header("Cache-Control", "no-store")
        .body(axum::body::Body::from(serde_json::to_string(&lobbies_info).unwrap()))
        .unwrap()
}

async fn catalog_json_handler() -> impl axum::response::IntoResponse {
    let catalog = map_catalog::catalog_json();
    axum::response::Response::builder()
        .header("Content-Type", "application/json")
        .header("Cache-Control", "public, max-age=60")
        .body(axum::body::Body::from(serde_json::to_string(&catalog).unwrap()))
        .unwrap()
}

