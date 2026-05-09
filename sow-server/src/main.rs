use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use futures_util::{StreamExt, SinkExt};
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast};
use sow_core::engine::SowEngine;
use sow_core::game::GameState;
use sow_core::game_config::GameConfig;
use sow_core::water_components::WaterComponents;
use sow_core::protocol::{LobbyInfo, ServerLobbiesBroadcastMessage};
use std::time::Duration;

struct ServerGame {
    id: u64,
    engine: SowEngine,
    players: Vec<String>,
    is_counting_down: bool,
    timer_secs: f32,
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
            timer_secs: 15.0, // 15 second countdown
        });
    }

    let games_state = Arc::new(Mutex::new(games));
    let (tx, _rx) = broadcast::channel::<String>(100);
    
    // Background task to tick lobbies and broadcast state
    let games_clone = Arc::clone(&games_state);
    let tx_clone = tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        loop {
            interval.tick().await;
            let mut lobbies_info = Vec::new();
            
            {
                let mut games = games_clone.lock().await;
                for game in games.iter_mut() {
                    if game.is_counting_down {
                        game.timer_secs -= 0.1;
                        if game.timer_secs <= 0.0 {
                            // Start game
                            game.is_counting_down = false;
                            let start_msg = sow_core::protocol::ServerStartMessage {
                                config: GameConfig::default(),
                                my_player_id: Some(1),
                                seed: 12345,
                                players: vec![],
                                missed_turns: vec![],
                                map_data: None,
                            };
                            let json = serde_json::to_string(&start_msg).unwrap();
                            let _ = tx_clone.send(json);
                        }
                    } else {
                        // Tick game
                        game.engine.tick();
                        if game.engine.state.phase == sow_core::game::GamePhase::GameOver {
                            // Reset lobby
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
                        player_names: game.players.clone(),
                    });
                }
            }
            
            let broadcast_msg = ServerLobbiesBroadcastMessage { lobbies: lobbies_info };
            let json = serde_json::to_string(&broadcast_msg).unwrap();
            let _ = tx_clone.send(json);
        }
    });

    let addr = "0.0.0.0:25565";
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");
    log::info!("SOW-SERVER Authoritative Relay listening on ws://{}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let mut rx = tx.subscribe();
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
            
            loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(msg)) => {
                                if msg.is_text() {
                                    log::info!("Received msg: {}", msg);
                                }
                            }
                            _ => break,
                        }
                    }
                    Ok(broadcast_text) = rx.recv() => {
                        if write.send(tokio_tungstenite::tungstenite::protocol::Message::Text(broadcast_text.into())).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });
    }
}
