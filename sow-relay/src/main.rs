use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use sow_core::protocol::{ClientMessage, GameplayIntent, ServerMessage, ServerTurnMessage, StampedIntent, Turn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{interval, Duration};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(serde::Deserialize)]
struct RelayConfig {
    lobby_id: u64,
    tick_number: u64,
    active_empty_secs: f32,
    players: Vec<PlayerEntry>,
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
    let valid_players: HashMap<u16, String> = config.players.into_iter().map(|p| (p.player_id, p.name)).collect();
    
    // player_id -> Sender
    let connected_clients: Arc<Mutex<HashMap<u16, mpsc::Sender<Vec<u8>>>>> = Arc::new(Mutex::new(HashMap::new()));
    let connected_clients_clone = connected_clients.clone();

    let (event_tx, mut event_rx) = mpsc::channel::<RelayEvent>(1000);
    let event_tx_clone = event_tx.clone();

    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind relay port");
    info!("Relay for lobby {} listening on ws://{}", lobby_id, addr);

    // Main Tick Loop
    tokio::spawn(async move {
        let mut ticker = interval(Duration::from_millis(50)); // 20 ticks per second (Server config tick time = 0.05)
        let mut last_status = std::time::Instant::now();
        
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let mut clients = connected_clients_clone.lock().await;
                    let humans = clients.len();

                    if humans == 0 {
                        active_empty_secs -= 0.05;
                        if active_empty_secs <= 0.0 {
                            info!("Relay {} shutting down (empty timeout)", lobby_id);
                            // TODO: Add logic to receive endgame signal to immediately shut down instance upon declaring a winner
                            std::process::exit(0);
                        }
                    } else {
                        active_empty_secs = 30.0; // Reset
                    }

                    let intents = std::mem::take(&mut pending_intents);
                    let turn = Turn {
                        turn_number: tick_number,
                        intents,
                    };
                    tick_number += 1;

                    let msg = ServerTurnMessage { turn };
                    let json = bincode::serialize(&ServerMessage::Turn(msg)).expect("serialize ServerTurnMessage");

                    for tx in clients.values_mut() {
                        let _ = tx.try_send(json.clone());
                    }
                    
                    if last_status.elapsed().as_secs() >= 10 {
                        println!("STATUS|{}|{}|{}|{}", lobby_id, std::process::id(), port, humans);
                        last_status = std::time::Instant::now();
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
                                intent: GameplayIntent::Resign,
                            });
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

        tokio::spawn(async move {
            let ws_stream = match accept_async(stream).await {
                Ok(ws) => ws,
                Err(e) => {
                    warn!("Handshake failed: {}", e);
                    return;
                }
            };
            let (mut write, mut read) = ws_stream.split();
            let (direct_tx, mut direct_rx) = mpsc::channel::<Vec<u8>>(100);
            
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
                                                } else {
                                                    warn!("Invalid Ready request for lobby {} player {}", l_id, player_id);
                                                }
                                            }
                                            ClientMessage::Gameplay { intent } => {
                                                if let Some(pid) = my_player_id {
                                                    let _ = ev_tx.send(RelayEvent::Gameplay { player_id: pid, intent }).await;
                                                }
                                            }
                                            ClientMessage::Leave {} => {
                                                if let Some(pid) = my_player_id {
                                                    let _ = ev_tx.send(RelayEvent::Leave { player_id: pid }).await;
                                                }
                                                my_player_id = None;
                                            }
                                            ClientMessage::Ping { client_time } => {
                                                let pong = ServerMessage::Pong { client_time };
                                                let json = bincode::serialize(&pong).unwrap();
                                                let _ = direct_tx.try_send(json);
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
                let _ = ev_tx.send(RelayEvent::Leave { player_id: pid }).await;
            }
        });
    }
}
