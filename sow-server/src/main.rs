mod lobby;
mod map_catalog;

use futures_util::{SinkExt, StreamExt};
use lobby::{
    JoinPlayerOpts, ServerLobby, build_lobby_broadcast, force_start, is_host_teardown, join_player, kick_player,
    leave_player, lobby_to_info, master_tick, notify_lobby_closed, set_player_team,
    sync_host_lobby_to_members,
};
use redis::Commands;
use sow_core::game_config::GameConfig;
use sow_core::protocol::{
    PlayerInfo, ServerJoinAckMessage, ServerJoinFailedMessage, ServerLobbiesBroadcastMessage,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio_tungstenite::tungstenite::protocol::Message;

const REDIS_PORTS_KEY: &str = "sow:ports";

/// The monolithic DPDK relay's fixed game port (SOW_RELAY_PORT, default 80).
fn relay_port() -> u16 {
    std::env::var("SOW_RELAY_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(80)
}

/// Host the clients should reach the relay at (data PIP on Azure). None means
/// "use the orchestrator host" (legacy client behavior).
fn relay_host() -> Option<String> {
    std::env::var("SOW_RELAY_HOST")
        .ok()
        .filter(|host| !host.trim().is_empty())
        .map(|host| host.trim().to_string())
}

/// Register a lobby with the monolithic relay over mgmt HTTP. The relay
/// confirms with 200 OK before the server broadcasts Start to the clients.
/// Retries with exponential backoff (same resilience as the old spawn path).
async fn register_relay(rc: &RelayCandidate) -> Result<(), String> {
    let mgmt_url = std::env::var("SOW_RELAY_MGMT_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let url = format!("{}/internal/lobby/register", mgmt_url.trim_end_matches('/'));

    let active_empty_secs = if rc.active_empty_secs <= 0.0 {
        30.0
    } else {
        rc.active_empty_secs
    };

    let relay_config = serde_json::json!({
        "lobby_id": rc.lobby_id,
        "tick_number": 0,
        "active_empty_secs": active_empty_secs,
        "players": rc.players_json,
        "tick_rate_ms": rc.tick_rate_ms,
    });

    let client = reqwest::Client::new();
    for attempt in 1..=5 {
        match client.post(&url).json(&relay_config).send().await {
            Ok(res) if res.status().is_success() => {
                log::info!(
                    "[RELAY] Lobby {} accepted by relay ({} OK)",
                    rc.lobby_id,
                    url
                );
                return Ok(());
            }
            Ok(res) => {
                log::warn!(
                    "[RELAY] register lobby {}: relay returned HTTP {} (attempt {}/5)",
                    rc.lobby_id,
                    res.status(),
                    attempt
                );
            }
            Err(e) => {
                log::warn!(
                    "[RELAY] register lobby {}: relay unreachable: {e} (attempt {}/5)",
                    rc.lobby_id,
                    attempt
                );
            }
        }
        if attempt < 5 {
            tokio::time::sleep(Duration::from_secs(2u64.pow(attempt as u32))).await;
        }
    }
    Err(format!("relay registration failed for lobby {}", rc.lobby_id))
}

// =============================================================================
// RELAY INTEGRATION — monolithic DPDK relay (no per-lobby processes)
// =============================================================================
//
// The relay is a single process (sow-relay + fstack-bridge) listening on a
// fixed game port (SOW_RELAY_PORT, default 80) on the data NIC. There is no
// dynamic port allocation, no per-lobby spawn, no nginx /relay/{port}/ws/
// reverse proxy, and no port-claiming race: the server registers each lobby
// with the relay over mgmt HTTP (SOW_RELAY_MGMT_URL, default
// http://127.0.0.1:8080) and ONLY broadcasts Start{relay_port, relay_host}
// after the relay answers 200 OK — the relay accepts the lobby before any
// client is told where to connect.
//
// History (why the old code is gone): the previous design gave each match its
// own `sow-relay` process on a dynamic port. Port liveness was tracked via
// Redis (`sow:ports` set + `sow:relay:{port}` TTL), and a race between relay
// startup, TTL expiry and lazy-cleanup could hand a still-bound port to a new
// lobby → "Address already in use" → clients routed to the OLD relay of a
// DIFFERENT lobby → "Invalid Ready request" spam and stalled matches
// (relay_25623.log, 2026-08-01). The probe-bind gate fixed the symptom; the
// monolithic relay removes the failure mode entirely.



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

/// Data collected inside the lock for a lobby that needs a relay spawned.
/// All blocking I/O (Redis, disk, process) happens *outside* the games lock in a dedicated worker task.
struct RelayCandidate {
    lobby_id: u64,
    active_empty_secs: f32,
    tick_rate_ms: f32,
    config: GameConfig,
    seed: u64,
    start_players: Vec<PlayerInfo>,
    players_json: Vec<serde_json::Value>,
    player_ids: Vec<String>,
    players_tx: Vec<(u16, mpsc::Sender<Vec<u8>>)>,
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
        let occupied: std::collections::HashSet<u16> = match con.smembers(REDIS_PORTS_KEY) {
            Ok(s) => s,
            Err(e) => {
                log::error!("[REDIS] SMEMBERS {REDIS_PORTS_KEY} FAILED at startup: {e}");
                std::collections::HashSet::new()
            }
        };
        log::info!(
            "Redis connected. Active relay ports preserved: {:?}",
            occupied
        );
    }

    let mut games: Vec<ServerLobby> = Vec::new();
    let mut next_lobby_id: u64 = 1;

    let maps_root = map_catalog::maps_root();
    map_catalog::init(&maps_root);

    master_tick(&mut games, &mut next_lobby_id);

    let games_state = Arc::new(Mutex::new(games));
    let next_id_state = Arc::new(Mutex::new(next_lobby_id));

    let (global_tx, _rx) = broadcast::channel::<Vec<u8>>(100);
    let (event_tx, mut event_rx) = mpsc::channel::<ServerEvent>(100000);
    let (relay_tx, mut relay_rx) = mpsc::unbounded_channel::<RelayCandidate>();

    let games_clone = Arc::clone(&games_state);
    let next_id_clone = Arc::clone(&next_id_state);
    let global_tx_clone = global_tx.clone();
    let relay_tx_clone = relay_tx.clone();

    // ── DEDICATED ASYNC RELAY WORKER TASK ──
    // The relay is a monolithic DPDK process (fstack-bridge) listening on a
    // fixed game port. There is no per-lobby process and no dynamic port: the
    // worker registers the lobby over mgmt HTTP and only broadcasts
    // Start{relay_port, relay_host} AFTER the relay confirms with 200 OK.
    // This closes the old spawn/bind race ("Failed to bind... Address already
    // in use", wrong-relay routing) for good: the relay accepts the lobby
    // before any client is told where to connect.
    tokio::spawn(async move {
        while let Some(rc) = relay_rx.recv().await {
            let relay_port = relay_port();

            // DB match registration
            if !rc.player_ids.is_empty() {
                let match_id = rc.lobby_id.to_string();
                let pids = rc.player_ids.clone();
                tokio::spawn(async move {
                    let db_base_url = std::env::var("SOW_DB_URL")
                        .unwrap_or_else(|_| "http://127.0.0.1:25585".to_string());
                    let secret_token = std::env::var("SOW_DB_SECRET").unwrap_or_else(|_| {
                        "sow_db_dev_secret_123_change_me_in_prod".to_string()
                    });
                    let url = format!("{}/match/start", db_base_url.trim_end_matches('/'));
                    let _ = reqwest::Client::new()
                        .post(&url)
                        .header("Authorization", format!("Bearer {}", secret_token))
                        .json(&serde_json::json!({ "match_id": match_id, "player_ids": pids }))
                        .send()
                        .await;
                });
            }

            // Register the lobby with the monolithic relay (mgmt HTTP, kernel
            // 127.0.0.1). Retries with backoff; the relay confirms before any
            // client is told the port. Each registration runs in its own task
            // so a slow relay never serializes the whole queue behind one
            // lobby's backoff (that backlog was starving bots of Start).
            let relay_port_for_task = relay_port;
            tokio::spawn(async move {
                match register_relay(&rc).await {
                    Ok(()) => {
                        log::info!(
                            "[RELAY] Lobby {} registered with relay on port {}",
                            rc.lobby_id,
                            relay_port_for_task
                        );

                        // Broadcast Start message to each player with their specific my_player_id
                        for (player_id, tx) in &rc.players_tx {
                            let start_msg = sow_core::protocol::ServerStartMessage {
                                config: rc.config.clone(),
                                my_player_id: Some(*player_id),
                                lobby_id: Some(rc.lobby_id),
                                seed: rc.seed,
                                players: rc.start_players.clone(),
                                missed_turns: vec![],
                                map_data: None,
                                relay_port: Some(relay_port_for_task),
                                relay_host: relay_host(),
                            };
                            if let Ok(start_json) = bincode::serialize(
                                &sow_core::protocol::ServerMessage::Start(Box::new(start_msg)),
                            ) {
                                let _ = tx.try_send(start_json);
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("[RELAY] {e}; lobby {} lost this tick", rc.lobby_id);
                    }
                }
            });
        }
    });

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        let mut tick_count: u64 = 0;
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let tick_start = tokio::time::Instant::now();
                    tick_count += 1;

                    // ── PHASE 1: In-memory work under the lock (microseconds) ──
                    let (ready_candidates, broadcast_json) = {
                        let mut games = games_clone.lock().await;
                        let mut nid = next_id_clone.lock().await;
                        master_tick(&mut games, &mut nid);

                        // Extract lobbies that completed Loading and are ready for relay
                        let mut ready_candidates = Vec::new();
                        let mut i = 0;
                        while i < games.len() {
                            if games[i].phase == lobby::LobbyPhase::ReadyForRelay {
                                let lobby = games.remove(i);
                                let mut players_json = Vec::new();
                                let mut start_players = Vec::new();
                                let mut player_ids = Vec::new();
                                let mut players_tx = Vec::new();
                                for p in &lobby.players {
                                    // Every network participant must be present in Start so each
                                    // lockstep client/backfill can register the same player ids.
                                    // PlayerType::Bot is reserved for local AI spawned by the
                                    // client engine; backfill bots are network-controlled players.
                                    start_players.push(PlayerInfo {
                                        id: p.player_id,
                                        name: p.name.clone(),
                                        player_type: sow_core::player::PlayerType::Human,
                                        color: p.leader.filler_rgb(),
                                        team: p.team,
                                        spawn_x: 0,
                                        spawn_y: 0,
                                        civilization: p.civilization,
                                        leader: p.leader,
                                    });
                                    players_json.push(serde_json::json!({
                                        "player_id": p.player_id,
                                        "name": p.name,
                                        "database_account_id": p.database_account_id,
                                    }));
                                    if let Some(acc_id) = &p.database_account_id {
                                        player_ids.push(acc_id.clone());
                                    }
                                    players_tx.push((p.player_id, p.tx.clone()));
                                }
                                ready_candidates.push(RelayCandidate {
                                    lobby_id: lobby.id,
                                    active_empty_secs: lobby.active_empty_secs,
                                    tick_rate_ms: lobby.config.tick_rate_ms,
                                    config: lobby.config.clone(),
                                    seed: lobby.seed,
                                    start_players,
                                    players_json,
                                    player_ids,
                                    players_tx,
                                });
                            } else {
                                i += 1;
                            }
                        }

                        // Build broadcast data (serialization is in-memory)
                        let broadcast_json = if tick_count % 10 == 0 {
                            let lobbies_info = build_lobby_broadcast(&games);
                            let broadcast_msg = ServerLobbiesBroadcastMessage { lobbies: lobbies_info };
                            match bincode::serialize(&sow_core::protocol::ServerMessage::LobbiesBroadcast(broadcast_msg)) {
                                Ok(json) => Some(json),
                                Err(e) => { log::error!("[BROADCAST] Failed to serialize LobbiesBroadcast: {}", e); None }
                            }
                        } else {
                            None
                        };

                        (ready_candidates, broadcast_json)
                    }; // ── LOCK RELEASED ──

                    // Enqueue relay candidates to background worker task (zero I/O in tick)
                    for rc in ready_candidates {
                        let _ = relay_tx_clone.send(rc);
                    }

                    // ── PHASE 3: Broadcast + perf (no lock needed) ──
                    if let Some(json) = broadcast_json {
                        let _ = global_tx_clone.send(json);
                    }

                    // Precise latency performance metric logger
                    let elapsed = tick_start.elapsed().as_millis();
                    if elapsed > 10 {
                        log::warn!("[PERF] Event loop lag detected! Master tick execution took {}ms", elapsed);
                    }
                }
                Some(event) = event_rx.recv() => {
                    let mut games = games_clone.lock().await;
                    let mut nid = next_id_clone.lock().await;
                    match event {
                        ServerEvent::Join { client_tx, name, clan_tag, civilization, leader, target_lobby_id, host_private, build_version, database_account_id, host_config, password, ip } => {
                            log::info!("Player {} (clan: {}, ip: {}) joining with version: {}", name, clan_tag, ip, build_version);
                            match join_player(&mut games, &mut nid, JoinPlayerOpts {
                                name,
                                clan_tag,
                                civilization,
                                leader,
                                client_tx: client_tx.clone(),
                                target_lobby_id,
                                host_private,
                                database_account_id,
                                host_config,
                                password,
                                ip,
                            }) {
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
            .route("/lobbies.json", axum::routing::get(lobbies_json_handler))
            .route(
                "/admin/dashboard",
                axum::routing::get(|| async {
                    axum::response::Html(include_str!("admin_dashboard.html"))
                }),
            )
            .route("/admin/api/status", axum::routing::get(admin_status))
            .with_state(state);

        let app = catalog_route
            .nest_service(
                "/maps",
                tower_http::services::ServeDir::new(root).precompressed_br(),
            )
            .layer(tower_http::cors::CorsLayer::permissive());
        let http_addr =
            std::env::var("SOW_MAPS_HTTP_LISTEN").unwrap_or_else(|_| "0.0.0.0:25566".to_string());
        log::info!(
            "SOW-SERVER HTTP serving maps and admin on http://{}",
            http_addr
        );
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

            let (direct_tx, mut direct_rx) = mpsc::channel::<Vec<u8>>(4096);

            // Send immediate single-cast LobbiesBroadcast snapshot on connection so the home menu loads instantly
            {
                let games_guard = games_state_conn.lock().await;
                let lobbies_info = build_lobby_broadcast(&games_guard);
                let broadcast_msg = ServerLobbiesBroadcastMessage {
                    lobbies: lobbies_info,
                };
                if let Ok(json) = bincode::serialize(
                    &sow_core::protocol::ServerMessage::LobbiesBroadcast(broadcast_msg),
                ) {
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
                            if [
                                "used_memory_human",
                                "used_memory_peak_human",
                                "connected_clients",
                                "blocked_clients",
                                "keyspace_hits",
                                "keyspace_misses",
                                "uptime_in_seconds",
                                "instantaneous_ops_per_sec",
                                "instantaneous_input_kbps",
                                "instantaneous_output_kbps",
                                "total_connections_received",
                                "total_commands_processed",
                                "expired_keys",
                                "evicted_keys",
                            ]
                            .contains(&key)
                            {
                                result.insert(
                                    key.to_string(),
                                    serde_json::Value::String(val.to_string()),
                                );
                            }
                        }
                    }
                    serde_json::Value::Object(result)
                }
                Err(_) => serde_json::json!({"error": "info failed"}),
            }
        }
    })
    .await
    .unwrap_or(serde_json::json!({"error": "task failed"}));

    // Query sow-database stats
    let db_stats = match reqwest::get("http://127.0.0.1:25585/internal/stats").await {
        Ok(r) => r
            .json::<serde_json::Value>()
            .await
            .unwrap_or(serde_json::json!({"error": "db unreachable"})),
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
        .body(axum::body::Body::from(
            serde_json::to_string(&lobbies_info).unwrap(),
        ))
        .unwrap()
}

async fn catalog_json_handler() -> impl axum::response::IntoResponse {
    let catalog = map_catalog::catalog_json();
    axum::response::Response::builder()
        .header("Content-Type", "application/json")
        .header("Cache-Control", "public, max-age=60")
        .body(axum::body::Body::from(
            serde_json::to_string(&catalog).unwrap(),
        ))
        .unwrap()
}
