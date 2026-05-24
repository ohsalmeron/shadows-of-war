//! test-relay: End-to-end integration test for the orchestrator → relay handoff.
//!
//! Simulates a real player:
//!   1. Connect to orchestrator WS
//!   2. Send Join, receive JoinAck
//!   3. Send MapDownloadProgress(100) + Ready
//!   4. Wait for ServerMessage::Start with relay_port
//!   5. Connect to relay WS (via NGINX proxy path)
//!   6. Send Ready to relay
//!   7. Receive at least one Turn
//!   8. Send Leave, disconnect
//!
//! Usage: cargo run --bin test-relay -- --url wss://shadowsofwar.io/ws/

use futures_util::{SinkExt, StreamExt};
use sow_core::protocol::{ClientMessage, ServerMessage};
use tokio_tungstenite::tungstenite::protocol::Message;

fn step(n: u8, label: &str) {
    eprintln!("\n\x1b[1;36m[STEP {n}]\x1b[0m {label}");
}

fn pass(msg: &str) {
    eprintln!("  \x1b[1;32m✅ {msg}\x1b[0m");
}

fn fail(msg: &str) -> ! {
    eprintln!("  \x1b[1;31m❌ {msg}\x1b[0m");
    std::process::exit(1);
}

async fn ws_send(
    ws: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
    msg: &ClientMessage,
) {
    let bytes = bincode::serialize(msg).unwrap();
    ws.send(Message::Binary(bytes)).await.unwrap();
}

async fn recv(
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    >,
    timeout_secs: u64,
) -> ServerMessage {
    let deadline = tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), async {
        loop {
            match read.next().await {
                Some(Ok(Message::Binary(data))) => {
                    if let Ok(msg) = bincode::deserialize::<ServerMessage>(&data) {
                        return msg;
                    }
                }
                Some(Ok(_)) => continue,
                Some(Err(e)) => fail(&format!("WS read error: {e}")),
                None => fail("WS stream ended unexpectedly"),
            }
        }
    });
    match deadline.await {
        Ok(msg) => msg,
        Err(_) => fail(&format!(
            "Timed out after {timeout_secs}s waiting for server message"
        )),
    }
}

#[tokio::main]
async fn main() {
    let url = std::env::args()
        .skip_while(|a| a != "--url")
        .nth(1)
        .unwrap_or_else(|| "wss://shadowsofwar.io/ws/".to_string());

    let version = std::fs::read_to_string(".version")
        .unwrap_or_else(|_| "unknown".to_string())
        .trim()
        .to_string();

    eprintln!("\x1b[1;33m═══ SOW Relay Integration Test ═══\x1b[0m");
    eprintln!("  Orchestrator: {url}");
    eprintln!("  Build version: {version}");

    // ── Step 1: Connect to orchestrator ─────────────────────────────────────
    step(1, "Connecting to orchestrator...");
    let (ws, _) = tokio_tungstenite::connect_async(&url)
        .await
        .unwrap_or_else(|e| fail(&format!("Connect failed: {e}")));
    let (mut write, mut read) = ws.split();
    pass("Connected to orchestrator");

    // ── Step 2: Join ────────────────────────────────────────────────────────
    step(2, "Sending Join...");
    let join = ClientMessage::Join {
        name: "TestBot".to_string(),
        is_observer: false,
        target_lobby_id: None,
        build_version: version,
        clan_tag: "".to_string(),
        civilization: sow_core::player::Civilization::Rome,
        leader: sow_core::player::Leader::Caesar,
    };
    ws_send(&mut write, &join).await;

    let lobby_id: u64;
    let player_id: u16;

    // Drain messages until we get JoinAck
    loop {
        let msg = recv(&mut read, 10).await;
        match msg {
            ServerMessage::JoinAck(ack) => {
                lobby_id = ack.lobby_id;
                player_id = ack.player_id;
                pass(&format!(
                    "JoinAck: lobby={lobby_id}, player={player_id}, map={}",
                    ack.map_name
                ));
                break;
            }
            ServerMessage::JoinFailed(f) => fail(&format!("JoinFailed: {}", f.reason)),
            ServerMessage::LobbiesBroadcast(_) => continue, // ignore broadcasts
            other => {
                eprintln!("  (ignoring {:?})", std::mem::discriminant(&other));
                continue;
            }
        }
    }

    // ── Step 3: Send Ready (skip map download) ──────────────────────────────
    step(
        3,
        "Sending MapDownloadProgress(100) + Ready to orchestrator...",
    );
    ws_send(
        &mut write,
        &ClientMessage::MapDownloadProgress {
            lobby_id,
            player_id,
            progress: 100,
        },
    )
    .await;
    ws_send(
        &mut write,
        &ClientMessage::Ready {
            lobby_id,
            player_id,
        },
    )
    .await;
    pass("Sent Ready to orchestrator");

    // ── Step 4: Wait for Start ──────────────────────────────────────────────
    step(
        4,
        "Waiting for ServerMessage::Start (up to 30s for countdown)...",
    );
    let relay_port: u16;
    let start_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);

    loop {
        let remaining = start_deadline.duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            fail("Timed out waiting for Start message");
        }

        let msg = tokio::time::timeout(remaining, async {
            loop {
                match read.next().await {
                    Some(Ok(Message::Binary(data))) => {
                        if let Ok(m) = bincode::deserialize::<ServerMessage>(&data) {
                            return m;
                        }
                    }
                    Some(Ok(_)) => continue,
                    Some(Err(e)) => fail(&format!("WS error: {e}")),
                    None => fail("WS closed before Start"),
                }
            }
        })
        .await;

        match msg {
            Ok(ServerMessage::Start(start)) => {
                relay_port = start.relay_port.unwrap_or(0);
                pass(&format!(
                    "Start received! relay_port={}, my_id={:?}, players={}, seed={}",
                    relay_port,
                    start.my_player_id,
                    start.players.len(),
                    start.seed
                ));
                if relay_port == 0 {
                    fail("Start message has no relay_port!");
                }
                break;
            }
            Ok(ServerMessage::SyncState(s)) => {
                eprintln!(
                    "  ⏳ SyncState: time_remaining={:.1}s, is_starting={}",
                    s.time_remaining, s.is_starting
                );
                continue;
            }
            Ok(ServerMessage::LobbiesBroadcast(_)) => continue,
            Ok(other) => {
                eprintln!("  (ignoring {:?})", std::mem::discriminant(&other));
                continue;
            }
            Err(_) => fail("Timed out waiting for Start"),
        }
    }

    // Drop orchestrator connection
    drop(write);
    drop(read);
    pass("Dropped orchestrator connection");

    // ── Step 5: Connect to relay ────────────────────────────────────────────
    step(5, &format!("Connecting to relay on port {relay_port}..."));

    let relay_url = if url.contains("shadowsofwar.io") {
        format!("wss://shadowsofwar.io/relay/{relay_port}/ws/")
    } else {
        // Local: replace port directly
        let mut parsed = reqwest::Url::parse(&url).unwrap();
        let _ = parsed.set_port(Some(relay_port));
        parsed.to_string()
    };
    eprintln!("  Relay URL: {relay_url}");

    // Retry connection for up to 5 seconds (relay may still be booting)
    let mut relay_ws = None;
    for attempt in 1..=10 {
        match tokio_tungstenite::connect_async(&relay_url).await {
            Ok((ws, _)) => {
                relay_ws = Some(ws);
                pass(&format!("Connected to relay (attempt {attempt})"));
                break;
            }
            Err(e) => {
                eprintln!("  ⏳ Attempt {attempt}/10 failed: {e}");
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
        }
    }
    let relay_ws = relay_ws.unwrap_or_else(|| fail("Could not connect to relay after 10 attempts"));
    let (mut r_write, mut r_read) = relay_ws.split();

    // ── Step 6: Send Ready to relay ─────────────────────────────────────────
    step(6, "Sending Ready to relay...");
    let ready = ClientMessage::Ready {
        lobby_id,
        player_id,
    };
    let bytes = bincode::serialize(&ready).unwrap();
    r_write.send(Message::Binary(bytes)).await.unwrap();
    pass("Sent Ready to relay");

    // ── Step 7: Receive turns ───────────────────────────────────────────────
    step(7, "Waiting for Turn messages from relay (5s window)...");
    let mut turn_count = 0u64;
    let turn_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);

    loop {
        let remaining = turn_deadline.duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }

        match tokio::time::timeout(remaining, r_read.next()).await {
            Ok(Some(Ok(Message::Binary(data)))) => {
                if let Ok(ServerMessage::Turn(t)) = bincode::deserialize::<ServerMessage>(&data) {
                    turn_count += 1;
                    if turn_count <= 3 || turn_count.is_multiple_of(20) {
                        eprintln!(
                            "  📦 Turn #{} (intents: {})",
                            t.turn.turn_number,
                            t.turn.intents.len()
                        );
                    }
                }
            }
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(e))) => {
                eprintln!("  ⚠️  Read error: {e}");
                break;
            }
            Ok(None) => {
                eprintln!("  ⚠️  Relay stream ended");
                break;
            }
            Err(_) => break, // timeout, that's fine
        }
    }

    if turn_count > 0 {
        pass(&format!("Received {turn_count} turns from relay"));
    } else {
        fail("Received ZERO turns from relay — relay is not sending data!");
    }

    // ── Step 8: Leave ───────────────────────────────────────────────────────
    step(8, "Sending Leave to relay...");
    let leave = bincode::serialize(&ClientMessage::Leave {}).unwrap();
    let _ = r_write.send(Message::Binary(leave)).await;
    pass("Sent Leave");

    // Done
    eprintln!("\n\x1b[1;32m═══ ALL STEPS PASSED ═══\x1b[0m");
    eprintln!("  Orchestrator handoff → Relay connection → Turn receive → Clean exit");
    eprintln!("  {turn_count} turns received in 5 seconds\n");
}
