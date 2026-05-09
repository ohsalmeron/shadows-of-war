use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use futures_util::StreamExt;
use std::sync::Arc;
use tokio::sync::Mutex;
use sow_core::engine::SowEngine;
use sow_core::game::GameState;
use sow_core::game_config::GameConfig;
use sow_core::water_components::WaterComponents;

#[tokio::main]
async fn main() {
    env_logger::init();
    
    let config = GameConfig::default();
    let state = GameState::new(12345, 800, 600, config);
    let water = WaterComponents::compute(&state.map);
    let engine = Arc::new(Mutex::new(SowEngine::new(state, water)));
    
    let addr = "0.0.0.0:25565";
    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");
    log::info!("SOW-SERVER Authoritative Relay listening on ws://{}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let _engine_clone = Arc::clone(&engine);
        tokio::spawn(async move {
            let ws_stream = match accept_async(stream).await {
                Ok(ws) => ws,
                Err(e) => {
                    log::error!("Handshake failed: {}", e);
                    return;
                }
            };
            let (_write, mut read) = ws_stream.split();
            
            log::info!("Player connected!");
            
            while let Some(msg) = read.next().await {
                if let Ok(msg) = msg {
                    if msg.is_text() {
                        log::info!("Received intent: {}", msg);
                    }
                }
            }
        });
    }
}
