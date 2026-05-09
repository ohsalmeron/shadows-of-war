mod lobby;

use futures_util::{SinkExt, StreamExt};
use lobby::{master_tick, ServerLobby, build_lobby_broadcast, join_player, leave_player};
use sow_core::protocol::{
    ClientGameplayMessage, ClientJoinMessage, ClientLeaveMessage, ServerJoinAckMessage,
    ServerJoinFailedMessage, ServerLobbiesBroadcastMessage, ServerStartMessage,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;

enum ServerEvent {
    Join {
        client_tx: mpsc::Sender<String>,
        name: String,
        target_lobby_id: Option<u64>,
    },
    Gameplay {
        lobby_id: u64,
        player_id: u16,
        intent: sow_core::protocol::GameplayIntent,
    },
    Leave {
        lobby_id: u64,
        player_id: u16,
    },
}

#[tokio::main]
async fn main() {
    env_logger::init();

    let mut games: Vec<ServerLobby> = Vec::new();
    let mut next_lobby_id: u64 = 1;
    master_tick(&mut games, &mut next_lobby_id);

    let games_state = Arc::new(Mutex::new(games));
    let next_id_state = Arc::new(Mutex::new(next_lobby_id));

    let (global_tx, _rx) = broadcast::channel::<String>(100);
    let (event_tx, mut event_rx) = mpsc::channel::<ServerEvent>(1000);

    let games_clone = Arc::clone(&games_state);
    let next_id_clone = Arc::clone(&next_id_state);
    let global_tx_clone = global_tx.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let mut games = games_clone.lock().await;
                    let mut nid = next_id_clone.lock().await;
                    master_tick(&mut games, &mut *nid);
                    let lobbies_info = build_lobby_broadcast(&games);
                    let broadcast_msg = ServerLobbiesBroadcastMessage { lobbies: lobbies_info };
                    let json = serde_json::to_string(&broadcast_msg).unwrap();
                    let _ = global_tx_clone.send(json);
                }

                Some(event) = event_rx.recv() => {
                    let mut games = games_clone.lock().await;
                    match event {
                        ServerEvent::Join { client_tx, name, target_lobby_id } => {
                            match join_player(&mut games, name, client_tx.clone(), target_lobby_id) {
                                Ok((lobby_id, player_id)) => {
                                    let ack = ServerJoinAckMessage { lobby_id, player_id };
                                    let json = serde_json::to_string(&ack).unwrap();
                                    let _ = client_tx.try_send(json);
                                }
                                Err(reason) => {
                                    let fail = ServerJoinFailedMessage { reason };
                                    let json = serde_json::to_string(&fail).unwrap();
                                    let _ = client_tx.try_send(json);
                                }
                            }
                        }
                        ServerEvent::Gameplay { lobby_id, player_id, intent } => {
                            if let Some(lobby) = games.iter_mut().find(|g| g.id == lobby_id) {
                                if lobby.phase != lobby::LobbyPhase::Active {
                                    continue;
                                }
                                lobby.pending_intents.push(sow_core::protocol::StampedIntent {
                                    player_id,
                                    intent,
                                });
                            }
                        }
                        ServerEvent::Leave { lobby_id, player_id } => {
                            leave_player(&mut games, lobby_id, player_id);
                        }
                    }
                }
            }
        }
    });

    let addr = "0.0.0.0:25565";
    let listener = TcpListener::bind(addr).await.expect("Failed to bind");
    log::info!("SOW-SERVER listening on ws://{}", addr);

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

            let (direct_tx, mut direct_rx) = mpsc::channel::<String>(100);

            let mut my_lobby_id: Option<u64> = None;
            let mut my_player_id: Option<u16> = None;

            loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(msg)) => {
                                if msg.is_text() {
                                    let text = msg.to_text().unwrap();

                                    if let Ok(join) = serde_json::from_str::<ClientJoinMessage>(text) {
                                        let _ = ev_tx.send(ServerEvent::Join {
                                            client_tx: direct_tx.clone(),
                                            name: join.name,
                                            target_lobby_id: join.target_lobby_id,
                                        }).await;
                                        continue;
                                    }

                                    if let Ok(gameplay) = serde_json::from_str::<ClientGameplayMessage>(text) {
                                        if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                            let _ = ev_tx.send(ServerEvent::Gameplay {
                                                lobby_id: l_id,
                                                player_id: p_id,
                                                intent: gameplay.intent,
                                            }).await;
                                        }
                                        continue;
                                    }

                                    if serde_json::from_str::<ClientLeaveMessage>(text).is_ok() {
                                        if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                            let _ = ev_tx.send(ServerEvent::Leave {
                                                lobby_id: l_id,
                                                player_id: p_id,
                                            }).await;
                                            my_lobby_id = None;
                                            my_player_id = None;
                                        }
                                        continue;
                                    }

                                    log::warn!("[SERVER] Unrecognized message: {}", text);
                                }
                            }
                            _ => break,
                        }
                    }
                    Ok(broadcast_text) = global_rx.recv() => {
                        if write.send(Message::Text(broadcast_text.into())).await.is_err() {
                            break;
                        }
                    }
                    Some(direct_text) = direct_rx.recv() => {
                        if let Ok(ack) = serde_json::from_str::<ServerJoinAckMessage>(&direct_text) {
                            my_lobby_id = Some(ack.lobby_id);
                            my_player_id = Some(ack.player_id);
                        }
                        if let Ok(start) = serde_json::from_str::<ServerStartMessage>(&direct_text) {
                            if let Some(pid) = start.my_player_id {
                                my_player_id = Some(pid);
                            }
                        }

                        if write.send(Message::Text(direct_text.into())).await.is_err() {
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
