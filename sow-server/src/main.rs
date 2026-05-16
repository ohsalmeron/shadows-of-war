mod lobby;

use futures_util::{SinkExt, StreamExt};
use lobby::{master_tick, ServerLobby, build_lobby_broadcast, join_player, leave_player};
use redis::Commands;
use sow_core::protocol::{
    ServerJoinAckMessage,
    ServerJoinFailedMessage, ServerLobbiesBroadcastMessage,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;

const REDIS_PORTS_KEY: &str = "sow:ports";
const RELAY_PORT_MIN: u16 = 25570;
const RELAY_PORT_MAX: u16 = 25600;

fn find_free_port(redis_con: &mut redis::Connection) -> Option<u16> {
    let occupied: std::collections::HashSet<u16> = redis_con.smembers(REDIS_PORTS_KEY).unwrap_or_default();
    (RELAY_PORT_MIN..=RELAY_PORT_MAX).find(|p| !occupied.contains(p))
}

enum ServerEvent {
    Join {
        client_tx: mpsc::Sender<Vec<u8>>,
        name: String,
        target_lobby_id: Option<u64>,
        build_version: String,
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
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let redis_url = std::env::var("SOW_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
    let redis_client = redis::Client::open(redis_url).expect("Failed to connect to Redis");
    let redis_con = Arc::new(std::sync::Mutex::new(
        redis_client.get_connection().expect("Failed to get Redis connection"),
    ));
    {
        let mut con = redis_con.lock().unwrap();
        let occupied: std::collections::HashSet<u16> = con.smembers(REDIS_PORTS_KEY).unwrap_or_default();
        log::info!("Redis connected. Occupied relay ports: {:?}", occupied);
    }

    let mut games: Vec<ServerLobby> = Vec::new();
    let mut next_lobby_id: u64 = 1;
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
        loop {
            tokio::select! {
                _ = interval.tick() => {
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
                            for p in &lobby.players {
                                players_json.push(serde_json::json!({
                                    "player_id": p.player_id,
                                    "name": p.name,
                                }));
                            }
                            
                            let relay_config = serde_json::json!({
                                "lobby_id": lobby.id,
                                "tick_number": 0,
                                "active_empty_secs": lobby.active_empty_secs,
                                "players": players_json
                            });
                            
                            let log_file = std::fs::File::create(format!("relay_{}.log", relay_port)).unwrap();
                            let mut cmd = tokio::process::Command::new("./sow-relay");
                            cmd.arg("--port").arg(relay_port.to_string())
                               .arg("--lobby-json").arg(relay_config.to_string())
                               .stdin(std::process::Stdio::null())
                               .stdout(std::process::Stdio::from(log_file.try_clone().unwrap()))
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
                    
                    let lobbies_info = build_lobby_broadcast(&games);
                    
                    let mut broadcast_msg = ServerLobbiesBroadcastMessage { lobbies: lobbies_info };
                    // Hard cap to 1 lobby for the UI
                    if broadcast_msg.lobbies.len() > 1 {
                        broadcast_msg.lobbies.truncate(1);
                    }
                    
                    let json = bincode::serialize(&sow_core::protocol::ServerMessage::LobbiesBroadcast(broadcast_msg)).unwrap();
                    let _ = global_tx_clone.send(json);
                    
                    for lobby in ready_lobbies {
                        if let Some(relay_port) = lobby.relay_port {
                            let mut player_infos = Vec::new();
                            for p in &lobby.players {
                                player_infos.push(sow_core::protocol::PlayerInfo {
                                    id: p.player_id,
                                    name: p.name.clone(),
                                    player_type: sow_core::player::PlayerType::Human,
                                    color: [1.0, 0.0, 0.0],
                                    spawn_x: 0,
                                    spawn_y: 0,
                                });
                            }
                            
                            let start_msg = sow_core::protocol::ServerStartMessage {
                                config: lobby.config.clone(),
                                my_player_id: None,
                                seed: lobby.seed,
                                players: player_infos,
                                missed_turns: vec![],
                                map_data: None,
                                relay_port: Some(relay_port),
                            };
                            
                            for p in &lobby.players {
                                let mut player_start = start_msg.clone();
                                player_start.my_player_id = Some(p.player_id);
                                let json = bincode::serialize(&sow_core::protocol::ServerMessage::Start(Box::new(player_start))).unwrap();
                                let _ = p.tx.try_send(json);
                            }
                        } else {
                            log::error!("Failed to handoff relay for lobby {}: no relay_port assigned", lobby.id);
                            let msg = sow_core::protocol::ServerLobbyClosedMessage {
                                lobby_id: lobby.id,
                                reason: "Server failed to allocate relay".to_string(),
                            };
                            let json = bincode::serialize(&sow_core::protocol::ServerMessage::LobbyClosed(msg)).unwrap();
                            for p in &lobby.players {
                                let _ = p.tx.try_send(json.clone());
                            }
                        }
                    }
                }

                Some(event) = event_rx.recv() => {
                    let mut games = games_clone.lock().await;
                    let mut nid = next_id_clone.lock().await;
                    match event {
                        ServerEvent::Join { client_tx, name, target_lobby_id, build_version } => {
                            log::info!("Player {} joining with version: {}", name, build_version);
                            match join_player(&mut games, &mut nid, name, client_tx.clone(), target_lobby_id) {
                                Ok((lobby_id, player_id, map_name)) => {
                                    let ack = ServerJoinAckMessage { lobby_id, player_id, map_name };
                                    let json = bincode::serialize(&sow_core::protocol::ServerMessage::JoinAck(ack)).unwrap();
                                    let _ = client_tx.try_send(json);
                                }
                                Err(reason) => {
                                    let fail = ServerJoinFailedMessage { reason };
                                    let json = bincode::serialize(&sow_core::protocol::ServerMessage::JoinFailed(fail)).unwrap();
                                    let _ = client_tx.try_send(json);
                                }
                            }
                        }

                        ServerEvent::Leave { lobby_id, player_id } => {
                            leave_player(&mut games, lobby_id, player_id);
                        }
                        ServerEvent::Ready { lobby_id, player_id } => {
                            if let Some(lobby) = games.iter_mut().find(|g| g.id == lobby_id) {
                                lobby.ready_players.insert(player_id);
                            }
                        }
                        ServerEvent::MapDownloadProgress { lobby_id, player_id, progress } => {
                            if let Some(lobby) = games.iter_mut().find(|g| g.id == lobby_id) {
                                if let Some(p) = lobby.players.iter_mut().find(|p| p.player_id == player_id) {
                                    p.download_progress = progress;
                                }
                            }
                        }
                    }
                }
            }
        }
    });

    let addr = std::env::var("SOW_WS_LISTEN").unwrap_or_else(|_| "0.0.0.0:25565".to_string());
    
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");
    log::info!("SOW-SERVER listening on ws://{}", addr);

    // HTTP Static File Server for maps
        tokio::spawn(async move {
            let root = std::env::var("SOW_MAPS_ROOT").unwrap_or_else(|_| "assets/maps".to_string());
            let app = axum::Router::new()
                .nest_service("/maps", tower_http::services::ServeDir::new(root).precompressed_br())
                .layer(tower_http::cors::CorsLayer::permissive());
            let http_addr = std::env::var("SOW_MAPS_HTTP_LISTEN").unwrap_or_else(|_| "0.0.0.0:25566".to_string());
            log::info!("SOW-SERVER HTTP serving maps on http://{}", http_addr);
            let listener = tokio::net::TcpListener::bind(&http_addr).await.unwrap();
            axum::serve(listener, app).await.unwrap();
        });


    while let Ok((stream, _)) = listener.accept().await {
        let mut global_rx = global_tx.subscribe();
        let ev_tx = event_tx.clone();
        tokio::spawn(async move {
            let ws_stream = match accept_async(stream).await {
                Ok(ws) => ws,
                Err(e) => {
                    log::error!("Handshake failed: {}", e);
                    return;
                }
            };
            let (mut write, mut read) = ws_stream.split();
            log::info!("Client connected");

            let (direct_tx, mut direct_rx) = mpsc::channel::<Vec<u8>>(100);

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
                                            sow_core::protocol::ClientMessage::Join { name, is_observer: _, target_lobby_id, build_version } => {
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
                                                    client_tx: direct_tx.clone(),
                                                    target_lobby_id,
                                                    build_version,
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
                                            sow_core::protocol::ClientMessage::Ping { client_time } => {
                                                let pong = sow_core::protocol::ServerMessage::Pong { client_time };
                                                let json = bincode::serialize(&pong).unwrap();
                                                let _ = direct_tx.try_send(json);
                                            }
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
                        if write.send(Message::Binary(broadcast_data)).await.is_err() {
                            break;
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
                }
            }

            if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                let _ = ev_tx.send(ServerEvent::Leave {
                    lobby_id: l_id,
                    player_id: p_id,
                }).await;
            }
        });
    }
}
