//! HISTORICAL SPIKE — bincode protocol client test.
//!
//! This example targets the retired fixed-port, unticketed relay experiment.
//! It is not a production client; current clients receive a dynamic TLS relay
//! port and use `ReadyWithTicket`/`ReconnectWithTicket`.
//!
//! Connects to the relay_bincode server over the data PIP, sends a real
//! `ClientMessage::Ready { lobby_id: 42, player_id: 7 }` (bincode-serialized,
//! same encoding the game uses), and verifies the `ServerMessage::Start`
//! answer field by field. Exit 0 = all fields matched; 1 = mismatch; 2 = timeout.
//!
//! Usage:
//!   ./relay_client [ws://<data-pip>:80]

use futures_util::{SinkExt, StreamExt};
use sow_core::protocol::{ClientMessage, ServerMessage};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

const DEFAULT_URL: &str = "ws://20.122.128.185:80";

#[tokio::main]
async fn main() {
    let url = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_URL.to_string());

    let (mut ws, _resp) = match connect_async(&url).await {
        Ok(x) => x,
        Err(e) => {
            println!("connect err: {}", e);
            std::process::exit(1);
        }
    };

    let ready = ClientMessage::Ready {
        lobby_id: 42,
        player_id: 7,
    };
    let bytes = match bincode::serialize(&ready) {
        Ok(b) => b,
        Err(e) => {
            println!("serialize err: {}", e);
            std::process::exit(1);
        }
    };
    if ws.send(Message::Binary(bytes)).await.is_err() {
        println!("send err");
        std::process::exit(1);
    }

    match tokio::time::timeout(std::time::Duration::from_secs(10), ws.next()).await {
        Ok(Some(Ok(Message::Binary(b)))) => match bincode::deserialize::<ServerMessage>(&b) {
            Ok(ServerMessage::Start(s)) => {
                if s.lobby_id == Some(42) && s.my_player_id == Some(7) && s.seed == 42 {
                    println!("Start OK lobby=42 player=7 seed=42");
                    std::process::exit(0);
                } else {
                    println!(
                        "Start MISMATCH lobby={:?} player={:?} seed={}",
                        s.lobby_id, s.my_player_id, s.seed
                    );
                    std::process::exit(1);
                }
            }
            Ok(other) => {
                println!("unexpected ServerMessage: {:?}", other);
                std::process::exit(1);
            }
            Err(e) => {
                println!("deserialize err: {}", e);
                std::process::exit(1);
            }
        },
        Ok(Some(Ok(Message::Close(_)))) => {
            println!("closed by server");
            std::process::exit(1);
        }
        Ok(Some(Ok(_))) => {
            println!("non-binary frame");
            std::process::exit(1);
        }
        Ok(Some(Err(e))) => {
            println!("ws err: {}", e);
            std::process::exit(1);
        }
        Ok(None) => {
            println!("eof");
            std::process::exit(1);
        }
        Err(_) => {
            println!("timeout");
            std::process::exit(2);
        }
    }
}
