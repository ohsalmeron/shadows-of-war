use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use sow_core::protocol::{ClientMessage, ServerMessage, GameplayIntent, AttackIntent};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::time::{sleep, Duration};
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Orchestrator URL
    #[arg(short, long, default_value = "wss://shadowsofwar.io/ws/")]
    pub url: String,

    /// Number of bots to spawn
    #[arg(short, long, default_value_t = 30)]
    pub count: usize,

    /// Optional target lobby ID to join
    #[arg(long)]
    pub lobby_id: Option<u64>,

    /// Should bots send random intents to stress the engine?
    #[arg(long, default_value_t = true)]
    pub active: bool,
}

#[tokio::main]
async fn main() {
    env_logger::init();
    let args = Args::parse();

    let version = std::fs::read_to_string(".version")
        .unwrap_or_else(|_| "unknown".to_string())
        .trim()
        .to_string();

    println!("🤖 Starting Bot Manager with {} bots", args.count);
    println!("📡 Target: {}", args.url);

    let active_bots = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    for i in 0..args.count {
        let url = args.url.clone();
        let version = version.clone();
        let lobby_id = args.lobby_id;
        let active = args.active;
        let active_bots_ref = Arc::clone(&active_bots);

        let delay_ms = rand::thread_rng().gen_range(10..2000);

        let handle = tokio::spawn(async move {
            // Slight jitter so they don't all slam exactly on the same millisecond
            sleep(Duration::from_millis(delay_ms)).await;
            match run_bot(i, url, version, lobby_id, active).await {
                Ok(_) => {
                    println!("[Bot {}] Clean exit", i);
                }
                Err(e) => {
                    eprintln!("[Bot {}] Failed: {}", i, e);
                }
            }
            active_bots_ref.fetch_sub(1, Ordering::SeqCst);
        });

        handles.push(handle);
        active_bots.fetch_add(1, Ordering::SeqCst);
    }

    // Monitor
    loop {
        let count = active_bots.load(Ordering::SeqCst);
        println!("... {} bots active ...", count);
        if count == 0 {
            break;
        }
        sleep(Duration::from_secs(5)).await;
    }

    for handle in handles {
        let _ = handle.await;
    }

    println!("✅ All bots finished");
}

async fn run_bot(
    bot_index: usize,
    url: String,
    version: String,
    target_lobby_id: Option<u64>,
    active: bool,
) -> Result<(), String> {
    let name = format!("StressBot_{}", bot_index);

    let (ws, _) = tokio_tungstenite::connect_async(&url)
        .await
        .map_err(|e| format!("Connect failed: {}", e))?;
    let (mut write, mut read) = ws.split();

    // 1. Join
    let join = ClientMessage::Join {
        name,
        is_observer: false,
        target_lobby_id,
        build_version: version,
    };
    write
        .send(Message::Binary(bincode::serialize(&join).unwrap()))
        .await
        .map_err(|e| format!("Failed to send join: {}", e))?;

    let lobby_id: u64;
    let player_id: u16;

    loop {
        let msg = recv_msg(&mut read, 15).await?;
        match msg {
            ServerMessage::JoinAck(ack) => {
                lobby_id = ack.lobby_id;
                player_id = ack.player_id;
                break;
            }
            ServerMessage::JoinFailed(f) => return Err(format!("Join failed: {}", f.reason)),
            _ => continue,
        }
    }

    // 2. Ready
    write
        .send(Message::Binary(
            bincode::serialize(&ClientMessage::MapDownloadProgress {
                lobby_id,
                player_id,
                progress: 100,
            })
            .unwrap(),
        ))
        .await
        .unwrap();

    write
        .send(Message::Binary(
            bincode::serialize(&ClientMessage::Ready {
                lobby_id,
                player_id,
            })
            .unwrap(),
        ))
        .await
        .unwrap();

    // 3. Wait for start
    let relay_port: u16;
    loop {
        let msg = recv_msg(&mut read, 60).await?;
        match msg {
            ServerMessage::Start(start) => {
                relay_port = start.relay_port.unwrap_or(0);
                if relay_port == 0 {
                    return Err("Start message has no relay port".into());
                }
                break;
            }
            _ => continue,
        }
    }

    // Disconnect orchestrator
    drop(write);
    drop(read);

    // 4. Connect to relay
    let relay_url = if url.contains("shadowsofwar.io") {
        format!("wss://shadowsofwar.io/relay/{}/ws/", relay_port)
    } else {
        let mut parsed = reqwest::Url::parse(&url).unwrap();
        let _ = parsed.set_port(Some(relay_port));
        parsed.to_string()
    };

    let mut relay_ws = None;
    for _ in 1..=10 {
        if let Ok((ws, _)) = tokio_tungstenite::connect_async(&relay_url).await {
            relay_ws = Some(ws);
            break;
        }
        sleep(Duration::from_millis(500)).await;
    }

    let relay_ws = relay_ws.ok_or_else(|| "Could not connect to relay".to_string())?;
    let (mut r_write, mut r_read) = relay_ws.split();

    // 5. Send ready to relay
    let ready = ClientMessage::Ready {
        lobby_id,
        player_id,
    };
    r_write
        .send(Message::Binary(bincode::serialize(&ready).unwrap()))
        .await
        .map_err(|e| e.to_string())?;

    // 6. Listen loop
    loop {
        let msg = match recv_msg(&mut r_read, 15).await {
            Ok(m) => m,
            Err(_) => break, // Timeout or disconnected, end bot cleanly
        };

        if let ServerMessage::Turn(_t) = msg {
            if active {
                let intent = {
                    let mut rng = rand::thread_rng();
                    // 10% chance to send an intent per turn to be "playful"
                    // without immediately crushing the server socket
                    if rng.gen_bool(0.1) {
                        if rng.gen_bool(0.5) {
                            Some(GameplayIntent::Spawn { 
                                x: rng.gen_range(0..256), 
                                y: rng.gen_range(0..256) 
                            })
                        } else {
                            Some(GameplayIntent::Attack(AttackIntent {
                                target_owner: rng.gen_range(1..100),
                                troops: None, // All troops
                            }))
                        }
                    } else {
                        None
                    }
                };

                if let Some(intent) = intent {
                    let gm = ClientMessage::Gameplay { intent };
                    let _ = r_write.send(Message::Binary(bincode::serialize(&gm).unwrap())).await;
                }
            }
        }
    }

    let leave = ClientMessage::Leave {};
    let _ = r_write
        .send(Message::Binary(bincode::serialize(&leave).unwrap()))
        .await;

    Ok(())
}

async fn recv_msg(
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    timeout_secs: u64,
) -> Result<ServerMessage, String> {
    let deadline = tokio::time::timeout(Duration::from_secs(timeout_secs), async {
        loop {
            match read.next().await {
                Some(Ok(Message::Binary(data))) => {
                    if let Ok(msg) = bincode::deserialize::<ServerMessage>(&data) {
                        return Ok(msg);
                    }
                }
                Some(Ok(_)) => continue,
                Some(Err(e)) => return Err(format!("WS error: {}", e)),
                None => return Err("Stream ended".to_string()),
            }
        }
    });

    match deadline.await {
        Ok(res) => res,
        Err(_) => Err(format!("Timeout waiting for {}s", timeout_secs)),
    }
}
