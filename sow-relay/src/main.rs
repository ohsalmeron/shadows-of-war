use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use redis::Commands;
use sow_core::protocol::{
    ClientMessage, GameplayIntent, ServerMessage, ServerTurnMessage, StampedIntent, Turn,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc};
use tokio::time::{Duration, interval};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;

const REDIS_PORTS_KEY: &str = "sow:ports";

fn redis_connect() -> Option<redis::Connection> {
    let url = std::env::var("SOW_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
    redis::Client::open(url)
        .ok()
        .and_then(|c| c.get_connection().ok())
}

#[derive(serde::Deserialize)]
struct RelayConfig {
    lobby_id: u64,
    tick_number: u64,
    active_empty_secs: f32,
    players: Vec<PlayerEntry>,
    tick_rate_ms: f32,
}

#[derive(serde::Deserialize)]
struct PlayerEntry {
    player_id: u16,
    name: String,
}

#[derive(Clone)]
enum RelayEvent {
    Gameplay {
        player_id: u16,
        intent: GameplayIntent,
    },
    Leave {
        player_id: u16,
    },
    RematchRequest {
        player_id: u16,
    },
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let mut port = 0;
    let mut lobby_json = String::new();

    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--port" && i + 1 < args.len() {
            port = args[i + 1].parse().unwrap_or(0);
            i += 2;
        } else if args[i] == "--lobby-json" && i + 1 < args.len() {
            lobby_json = args[i + 1].clone();
            i += 2;
        } else {
            i += 1;
        }
    }

    if port == 0 || lobby_json.is_empty() {
        error!("Usage: sow-relay --port <PORT> --lobby-json <JSON>");
        std::process::exit(1);
    }

    let config: RelayConfig = match serde_json::from_str(&lobby_json) {
        Ok(c) => c,
        Err(e) => {
            error!("Failed to parse lobby JSON: {}", e);
            std::process::exit(1);
        }
    };

    let lobby_id = config.lobby_id;
    let mut tick_number = config.tick_number;
    let mut active_empty_secs = config.active_empty_secs;
    let mut pending_intents = Vec::new();
    let valid_players: HashMap<u16, String> = config
        .players
        .into_iter()
        .map(|p| (p.player_id, p.name))
        .collect();

    // player_id -> Sender
    let connected_clients: Arc<Mutex<HashMap<u16, mpsc::UnboundedSender<Vec<u8>>>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let connected_clients_clone = connected_clients.clone();

    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<RelayEvent>();
    let event_tx_clone = event_tx.clone();

    // Store match history so late-joiners (or slower loading WASM clients) can catch up
    let match_history: Arc<Mutex<Vec<Turn>>> = Arc::new(Mutex::new(Vec::new()));
    let match_history_clone = match_history.clone();

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr)
        .await
        .expect("Failed to bind relay port");
    info!("Relay for lobby {} listening on ws://{}", lobby_id, addr);

    // Register in Redis
    let redis_con: Arc<std::sync::Mutex<Option<redis::Connection>>> =
        Arc::new(std::sync::Mutex::new(redis_connect()));
    {
        let mut guard = redis_con.lock().unwrap();
        if let Some(ref mut con) = *guard {
            let _: () = con.sadd(REDIS_PORTS_KEY, port).unwrap_or_default();
            let key = format!("sow:relay:{}", port);
            let _: () = con
                .set_ex(&key, lobby_id.to_string(), 60)
                .unwrap_or_default();
            info!("Registered port {} in Redis", port);
        }
    }
    let redis_cleanup = Arc::clone(&redis_con);
    let cleanup_port = port;

    // Main Tick Loop
    let tick_interval_ms = config.tick_rate_ms as u64;
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_millis(tick_interval_ms));
        let mut last_status = std::time::Instant::now();
        let mut generated_rematch_id: Option<u64> = None;

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let mut clients = connected_clients_clone.lock().await;
                    let humans = clients.len();

                    if humans == 0 {
                        active_empty_secs -= 0.05;
                        if active_empty_secs <= 0.0 {
                            info!("Relay {} shutting down (empty timeout)", lobby_id);
                            let mut guard = redis_cleanup.lock().unwrap();
                            if let Some(ref mut con) = *guard {
                                let _: () = con.srem(REDIS_PORTS_KEY, cleanup_port).unwrap_or_default();
                                let _: () = con.del(format!("sow:relay:{}", cleanup_port)).unwrap_or_default();
                                info!("Cleaned up port {} from Redis", cleanup_port);
                            }
                            std::process::exit(0);
                        }
                    } else {
                        active_empty_secs = 30.0;
                    }

                    let intents = std::mem::take(&mut pending_intents);
                    let turn = Turn {
                        turn_number: tick_number,
                        intents,
                    };
                    tick_number += 1;

                    match_history_clone.lock().await.push(turn.clone());

                    let msg = ServerTurnMessage { turn };
                    let json = bincode::serialize(&ServerMessage::Turn(msg)).expect("serialize ServerTurnMessage");

                    for tx in clients.values_mut() {
                        let _ = tx.send(json.clone());
                    }

                    if last_status.elapsed().as_secs() >= 10 {
                        println!("STATUS|{}|{}|{}|{}", lobby_id, std::process::id(), port, humans);
                        last_status = std::time::Instant::now();
                        // Heartbeat: refresh Redis TTL
                        let mut guard = redis_cleanup.lock().unwrap();
                        if let Some(ref mut con) = *guard {
                            let key = format!("sow:relay:{}", cleanup_port);
                            let _: () = con.set_ex(&key, lobby_id.to_string(), 60).unwrap_or_default();
                        }
                    }
                }
                Some(event) = event_rx.recv() => {
                    match event {
                        RelayEvent::Gameplay { player_id, intent } => {
                            pending_intents.push(StampedIntent { player_id, intent });
                        }
                        RelayEvent::Leave { player_id } => {
                            connected_clients_clone.lock().await.remove(&player_id);
                            info!("Player {} left relay {}", player_id, lobby_id);
                            pending_intents.push(StampedIntent {
                                player_id,
                                intent: GameplayIntent::MarkDisconnected { is_disconnected: true },
                            });
                        }
                        RelayEvent::RematchRequest { player_id } => {
                            let rematch_id = if let Some(id) = generated_rematch_id {
                                info!("Player {} requested a rematch. Reusing cached rematch_lobby_id {}", player_id, id);
                                id
                            } else {
                                let id = (rand::random::<u64>() % 100_000) + 100_000_000;
                                info!("Player {} requested a rematch. Generating rematch_lobby_id {}...", player_id, id);
                                generated_rematch_id = Some(id);
                                id
                            };
                            let msg = sow_core::protocol::ServerLobbyClosedMessage {
                                lobby_id,
                                reason: "Rematch Requested".to_string(),
                                rematch_lobby_id: Some(rematch_id),
                            };
                            let json = bincode::serialize(&sow_core::protocol::ServerMessage::LobbyClosed(msg)).expect("serialize LobbyClosed");
                            let mut clients = connected_clients_clone.lock().await;
                            for tx in clients.values_mut() {
                                let _ = tx.send(json.clone());
                            }
                        }
                    }
                }
            }
        }
    });

    // Accept incoming connections
    while let Ok((stream, _)) = listener.accept().await {
        let ev_tx = event_tx_clone.clone();
        let clients_map = connected_clients.clone();
        let valid_map = valid_players.clone();
        let history_arc = match_history.clone();

        tokio::spawn(async move {
            let ws_stream = match accept_async(stream).await {
                Ok(ws) => ws,
                Err(e) => {
                    warn!("Handshake failed: {}", e);
                    return;
                }
            };
            let (mut write, mut read) = ws_stream.split();
            let (direct_tx, mut direct_rx) = mpsc::unbounded_channel::<Vec<u8>>();

            let mut my_player_id: Option<u16> = None;

            loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(msg)) => {
                                if msg.is_binary() {
                                    if let Ok(cmsg) = bincode::deserialize::<ClientMessage>(&msg.into_data()) {
                                        match cmsg {
                                            ClientMessage::Ready { lobby_id: l_id, player_id } => {
                                                if l_id == lobby_id && valid_map.contains_key(&player_id) {
                                                    my_player_id = Some(player_id);
                                                    clients_map.lock().await.insert(player_id, direct_tx.clone());
                                                    info!("Player {} reconnected to relay", player_id);

                                                    let _ = ev_tx.send(RelayEvent::Gameplay {
                                                        player_id,
                                                        intent: GameplayIntent::MarkDisconnected { is_disconnected: false },
                                                    });

                                                    // Send all missed turns so they can catch up!
                                                    let hist = history_arc.lock().await;
                                                    for past_turn in hist.iter() {
                                                        let msg = ServerTurnMessage { turn: past_turn.clone() };
                                                        if let Ok(json) = bincode::serialize(&ServerMessage::Turn(msg)) {
                                                            let _ = direct_tx.send(json);
                                                        }
                                                    }
                                                } else {
                                                    warn!("Invalid Ready request for lobby {} player {}", l_id, player_id);
                                                }
                                            }
                                            ClientMessage::Gameplay { intent } => {
                                                if let Some(pid) = my_player_id {
                                                    let _ = ev_tx.send(RelayEvent::Gameplay { player_id: pid, intent });
                                                }
                                            }
                                            ClientMessage::Leave {} => {
                                                if let Some(pid) = my_player_id {
                                                    let _ = ev_tx.send(RelayEvent::Leave { player_id: pid });
                                                }
                                                my_player_id = None;
                                            }
                                            ClientMessage::RematchRequest { lobby_id: _ } => {
                                                if let Some(pid) = my_player_id {
                                                    let _ = ev_tx.send(RelayEvent::RematchRequest { player_id: pid });
                                                }
                                            }
                                            ClientMessage::Ping { client_time } => {
                                                let pong = ServerMessage::Pong { client_time };
                                                let json = bincode::serialize(&pong).unwrap();
                                                let _ = direct_tx.send(json);
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            _ => break,
                        }
                    }
                    Some(direct_data) = direct_rx.recv() => {
                        if write.send(Message::Binary(direct_data)).await.is_err() {
                            break;
                        }
                    }
                }
            }

            if let Some(pid) = my_player_id {
                let _ = ev_tx.send(RelayEvent::Leave { player_id: pid });
            }
        });
    }
}
