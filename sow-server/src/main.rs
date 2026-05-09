use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use futures_util::{StreamExt, SinkExt};
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc};
use sow_core::engine::SowEngine;
use sow_core::game::GameState;
use sow_core::game_config::GameConfig;
use sow_core::water_components::WaterComponents;
use sow_core::protocol::{LobbyInfo, ServerLobbiesBroadcastMessage, ClientGameplayMessage, ClientJoinMessage, ServerStartMessage, PlayerInfo, Turn, StampedIntent, ServerTurnMessage};
use std::time::Duration;
use tokio_tungstenite::tungstenite::protocol::Message;

struct PlayerConnection {
    name: String,
    player_id: u16,
    tx: mpsc::Sender<String>,
}

struct ServerGame {
    id: u64,
    engine: SowEngine,
    players: Vec<PlayerConnection>,
    is_counting_down: bool,
    timer_secs: f32,
    pending_intents: Vec<StampedIntent>,
}

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
    }
}

#[tokio::main]
async fn main() {
    env_logger::init();
    
    // Create 2 Active Lobbies
    let mut games = Vec::new();
    for i in 1..=2 {
        let config = GameConfig::default();
        let state = GameState::new(12345 + i, 800, 600, config);
        let water = WaterComponents::compute(&state.map);
        games.push(ServerGame {
            id: i,
            engine: SowEngine::new(state, water),
            players: Vec::new(),
            is_counting_down: true,
            timer_secs: 15.0,
            pending_intents: Vec::new(),
        });
    }

    let games_state = Arc::new(Mutex::new(games));
    
    // Global broadcast just for lobby info
    let (global_tx, _rx) = broadcast::channel::<String>(100);
    
    // Channel for incoming client events
    let (event_tx, mut event_rx) = mpsc::channel::<ServerEvent>(1000);

    let games_clone = Arc::clone(&games_state);
    let global_tx_clone = global_tx.clone();
    
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let mut lobbies_info = Vec::new();
                    let mut games = games_clone.lock().await;
                    
                    for game in games.iter_mut() {
                        if game.is_counting_down {
                            if !game.players.is_empty() {
                                game.timer_secs -= 0.1;
                                if game.timer_secs <= 0.0 {
                                    // Start game
                                    game.is_counting_down = false;
                                    
                                    // Spawn players
                                    let mut player_infos = Vec::new();
                                    for p in &game.players {
                                        game.engine.spawn_human(p.player_id);
                                        player_infos.push(PlayerInfo {
                                            id: p.player_id,
                                            name: p.name.clone(),
                                            player_type: sow_core::player::PlayerType::Human,
                                            color: [1.0, 0.0, 0.0],
                                            spawn_x: 0,
                                            spawn_y: 0,
                                        });
                                    }
                                    
                                    // Spawn bots if necessary
                                    game.engine.spawn_random_bots(4);
                                    
                                    // Send ServerStartMessage to all players in this game
                                    for p in &game.players {
                                        let start_msg = ServerStartMessage {
                                            config: GameConfig::default(),
                                            my_player_id: Some(p.player_id),
                                            seed: 12345 + game.id,
                                            players: player_infos.clone(),
                                            missed_turns: vec![],
                                            map_data: None,
                                        };
                                        let json = serde_json::to_string(&start_msg).unwrap();
                                        let _ = p.tx.try_send(json);
                                    }
                                }
                            } else {
                                game.timer_secs = 15.0; // Reset timer if empty
                            }
                        } else {
                            // Tick game
                            let turn = Turn {
                                turn_number: game.engine.state.tick,
                                intents: game.pending_intents.clone(),
                            };
                            game.pending_intents.clear();
                            
                            for intent in &turn.intents {
                                game.engine.apply_stamped_intent(intent, 0);
                            }
                            game.engine.tick();
                            
                            let msg = ServerTurnMessage { turn };
                            let json = serde_json::to_string(&msg).unwrap();
                            for p in &game.players {
                                let _ = p.tx.try_send(json.clone());
                            }
                            
                            if game.engine.state.phase == sow_core::game::GamePhase::GameOver {
                                game.is_counting_down = true;
                                game.timer_secs = 15.0;
                                game.players.clear();
                                let state = GameState::new(rand::random(), 800, 600, GameConfig::default());
                                let water = WaterComponents::compute(&state.map);
                                game.engine = SowEngine::new(state, water);
                            }
                        }
                        
                        lobbies_info.push(LobbyInfo {
                            id: game.id,
                            num_players: game.players.len() as u32,
                            max_players: 8,
                            is_counting_down: game.is_counting_down,
                            timer_secs: game.timer_secs,
                            map_name: format!("Map {}", game.id),
                            player_names: game.players.iter().map(|p| p.name.clone()).collect(),
                        });
                    }
                    
                    let broadcast_msg = ServerLobbiesBroadcastMessage { lobbies: lobbies_info };
                    let json = serde_json::to_string(&broadcast_msg).unwrap();
                    let _ = global_tx_clone.send(json);
                }
                
                Some(event) = event_rx.recv() => {
                    let mut games = games_clone.lock().await;
                    match event {
                        ServerEvent::Join { client_tx, name, target_lobby_id } => {
                            let lobby_id = target_lobby_id.unwrap_or(1);
                            if let Some(game) = games.iter_mut().find(|g| g.id == lobby_id) {
                                let player_id = (game.players.len() as u16) + 1;
                                game.players.push(PlayerConnection {
                                    name,
                                    player_id,
                                    tx: client_tx,
                                });
                                log::info!("Player joined lobby {}", lobby_id);
                            }
                        }
                        ServerEvent::Gameplay { lobby_id, player_id, intent } => {
                            if let Some(game) = games.iter_mut().find(|g| g.id == lobby_id) {
                                game.pending_intents.push(StampedIntent {
                                    player_id,
                                    intent,
                                });
                            }
                        }
                        ServerEvent::Leave { lobby_id, player_id } => {
                            if let Some(game) = games.iter_mut().find(|g| g.id == lobby_id) {
                                game.players.retain(|p| p.player_id != player_id);
                            }
                        }
                    }
                }
            }
        }
    });

    let addr = "0.0.0.0:25565";
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");
    log::info!("SOW-SERVER Authoritative Relay listening on ws://{}", addr);

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
            log::info!("Player connected!");
            
            // This channel receives direct messages from the server (ServerStartMessage, ServerTurnMessage)
            let (direct_tx, mut direct_rx) = mpsc::channel::<String>(100);
            
            let mut my_lobby_id = None;
            let mut my_player_id = None;
            
            loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(msg)) => {
                                if msg.is_text() {
                                    let text = msg.to_text().unwrap();
                                    // log::debug!("[SERVER] Received msg: {}", text);
                                    if let Ok(join) = serde_json::from_str::<ClientJoinMessage>(text) {
                                        log::info!("[SERVER] Parsed JoinMessage for {:?}", join.target_lobby_id);
                                        my_lobby_id = Some(join.target_lobby_id.unwrap_or(1));
                                        let _ = ev_tx.send(ServerEvent::Join {
                                            client_tx: direct_tx.clone(),
                                            name: join.name,
                                            target_lobby_id: my_lobby_id,
                                        }).await;
                                    } else if let Ok(gameplay) = serde_json::from_str::<ClientGameplayMessage>(text) {
                                        if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                            let _ = ev_tx.send(ServerEvent::Gameplay {
                                                lobby_id: l_id,
                                                player_id: p_id,
                                                intent: gameplay.intent,
                                            }).await;
                                        }
                                    } else {
                                        log::warn!("[SERVER] Failed to parse message: {}", text);
                                    }
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
                        // Extract my_player_id if this is a ServerStartMessage
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
            
            // Clean up
            if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                let _ = ev_tx.send(ServerEvent::Leave { lobby_id: l_id, player_id: p_id }).await;
            }
        });
    }
}
