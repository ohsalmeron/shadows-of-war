use futures_util::{SinkExt, StreamExt};
use log::{error, info, warn};
use redis::Commands;
use sow_core::protocol::{
    ClientMessage, GameplayIntent, ServerMessage, ServerTurnMessage, StampedIntent, Turn,
};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, mpsc};
use tokio::sync::mpsc::error::TrySendError;
use tokio::time::{Duration, interval};
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::Message;

const REDIS_PORTS_KEY: &str = "sow:ports";

/// Max consecutive missed ticks before dropping a slow client.
/// At 20 ticks/s, 120 = 6 seconds of silence.
const MAX_MISSED_TICKS: u32 = 120;

/// Per-connection channel capacity: 4096 messages = ~200s buffer at 20 ticks/s.
const PER_CLIENT_CHANNEL: usize = 4096;

/// Event channel capacity.
const EVENT_CHANNEL: usize = 1024;

struct ClientChannel {
    sender: mpsc::Sender<Vec<u8>>,
    missed_ticks: u32,
}

fn redis_connect() -> Option<redis::Connection> {
    let url = std::env::var("SOW_VALKEY_URL")
        .or_else(|_| std::env::var("SOW_REDIS_URL"))
        .unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
    redis::Client::open(url)
        .ok()
        .and_then(|c| c.get_connection().ok())
}

fn log_player_exit(con: &mut redis::Connection, match_id: u64, account_id: &str) {
    let key = format!("sow:match:{match_id}:exits");
    let _: Result<(), _> = con
        .rpush(&key, account_id)
        .and_then(|()| con.expire(&key, 3600));
}

struct MatchPlayerStats {
    kills: u32,
    deaths: u32,
    assists: u32,
    players_defeated: u32,
    empires_defeated: u32,
    tribes_defeated: u32,
}

fn log_player_stats(
    con: &mut redis::Connection,
    match_id: u64,
    account_id: &str,
    stats: MatchPlayerStats,
) {
    let key = format!("sow:match:{match_id}:stats:{account_id}");
    let _: Result<(), _> = con
        .hset_multiple(
            &key,
            &[
                ("kills", stats.kills.to_string()),
                ("deaths", stats.deaths.to_string()),
                ("assists", stats.assists.to_string()),
                ("players_defeated", stats.players_defeated.to_string()),
                ("empires_defeated", stats.empires_defeated.to_string()),
                ("tribes_defeated", stats.tribes_defeated.to_string()),
            ],
        )
        .and_then(|()| con.expire(&key, 3600));
}

fn trigger_match_finalize(
    match_id: u64,
    lobby_json: String,
    match_history: Arc<Mutex<Vec<Turn>>>,
) {
    tokio::spawn(async move {
        let history = match_history.lock().await.clone();
        let replay_bytes = match bincode::serialize(&history) {
            Ok(bytes) => bytes,
            Err(e) => {
                error!("Failed to serialize match history for {match_id}: {e}");
                Vec::new()
            }
        };

        let db_url =
            std::env::var("SOW_DB_URL").unwrap_or_else(|_| "http://127.0.0.1:25585".to_string());
        let secret = std::env::var("SOW_DB_SECRET")
            .unwrap_or_else(|_| "sow_db_dev_secret_123_change_me_in_prod".to_string());
        let url = format!("{}/internal/match-finalize", db_url.trim_end_matches('/'));

        // ponytail: Raw replay bytes are passed directly to avoid any heavy relay processing
        let payload = serde_json::json!({
            "match_id": match_id.to_string(),
            "lobby_json": Some(lobby_json.clone()),
            "replay_data": Some(replay_bytes.clone()),
        });

        let mut success = false;
        let client = reqwest::Client::new();

        // ponytail: Resilient uploading with exponential backoff
        for attempt in 1..=5 {
            info!("Attempting raw upload/finalize to IONOS for match {match_id} (Attempt {attempt}/5)...");
            match client
                .post(&url)
                .header("Authorization", format!("Bearer {secret}"))
                .json(&payload)
                .send()
                .await
            {
                Ok(res) if res.status().is_success() => {
                    info!("Match {match_id} successfully finalized and archived on IONOS!");
                    success = true;
                    break;
                }
                Ok(res) => {
                    warn!(
                        "Attempt {attempt}/5: IONOS returned HTTP status {} for match {match_id}",
                        res.status()
                    );
                }
                Err(e) => {
                    warn!("Attempt {attempt}/5: Network error uploading match {match_id}: {e}");
                }
            }

            if attempt < 5 {
                let delay = Duration::from_secs(2u64.pow(attempt as u32));
                tokio::time::sleep(delay).await;
            }
        }

        if !success {
            error!("[CRITICAL] Failed to upload match {match_id} to IONOS after 5 attempts.");
            
            // Try local Valkey dead-letter fallback
            let url = std::env::var("SOW_VALKEY_URL")
                .or_else(|_| std::env::var("SOW_REDIS_URL"))
                .unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
            
            let mut valkey_success = false;
            if let Ok(client) = redis::Client::open(url) {
                if let Ok(mut con) = client.get_connection() {
                    let key = "sow:match_history:dead_letter";
                    let fallback_payload = bincode::serialize(&(lobby_json.clone(), replay_bytes.clone())).unwrap_or_default();
                    if let Ok(()) = con.lpush::<_, _, ()>(key, fallback_payload) {
                        warn!("[FALLBACK] Saved raw replay backup in local Valkey queue under key '{}' for match {match_id}", key);
                        valkey_success = true;
                    }
                }
            }

            if !valkey_success {
                // Extreme backup: save directly to host disk so DevOps can recover it easily
                let backup_dir = "/tmp/sow_crash_replays";
                error!("[ALERT] Valkey local fallback also failed! Dumping raw payload directly to local disk at {} for match {match_id}", backup_dir);
                let _ = std::fs::create_dir_all(backup_dir);
                let _ = std::fs::write(format!("{}/{}.json", backup_dir, match_id), &lobby_json);
                let _ = std::fs::write(format!("{}/{}.replay", backup_dir, match_id), &replay_bytes);
            }
        }
    });
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
    database_account_id: Option<String>,
}

struct MatchTracker {
    lobby_id: u64,
    player_accounts: HashMap<u16, String>,
    in_match: HashSet<u16>,
    logged_exits: HashSet<String>,
    finalized: bool,
    tracked: bool,
    redis_con: Arc<std::sync::Mutex<Option<redis::Connection>>>,
    lobby_json: String,
    match_history: Arc<Mutex<Vec<Turn>>>,
}

impl MatchTracker {
    fn record_exit(&mut self, player_id: u16) {
        if !self.tracked || self.finalized {
            self.in_match.remove(&player_id);
            return;
        }
        self.in_match.remove(&player_id);
        if let Some(account_id) = self.player_accounts.get(&player_id) {
            if self.logged_exits.insert(account_id.clone()) {
                let mut guard = self.redis_con.lock().unwrap();
                if let Some(ref mut con) = *guard {
                    log_player_exit(con, self.lobby_id, account_id);
                    info!(
                        "Logged exit for player {player_id} (account {account_id}) in match {}",
                        self.lobby_id
                    );
                }
            }
        }
        if self.in_match.len() <= 1 {
            if let Some(winner_id) = self.in_match.iter().copied().next() {
                if let Some(winner_acc) = self.player_accounts.get(&winner_id) {
                    if self.logged_exits.insert(winner_acc.clone()) {
                        let mut guard = self.redis_con.lock().unwrap();
                        if let Some(ref mut con) = *guard {
                            log_player_exit(con, self.lobby_id, winner_acc);
                            info!(
                                "Logged winner player {winner_id} (account {winner_acc}) in match {}",
                                self.lobby_id
                            );
                        }
                    }
                }
            }
            self.finalized = true;
            trigger_match_finalize(
                self.lobby_id,
                self.lobby_json.clone(),
                self.match_history.clone(),
            );
        }
    }
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
    let mut player_accounts: HashMap<u16, String> = HashMap::new();
    let mut valid_players: HashMap<u16, String> = HashMap::new();
    for p in config.players {
        valid_players.insert(p.player_id, p.name);
        if let Some(acc) = p.database_account_id {
            player_accounts.insert(p.player_id, acc);
        }
    }
    // player_id -> ClientChannel (bounded sender + missed tick counter)
    let connected_clients: Arc<Mutex<HashMap<u16, ClientChannel>>> =
        Arc::new(Mutex::new(HashMap::new()));
    let connected_clients_clone = connected_clients.clone();

    let (event_tx, mut event_rx) = mpsc::channel::<RelayEvent>(EVENT_CHANNEL);
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

    let tracked = !player_accounts.is_empty();
    let accounts_for_ws = player_accounts.clone();
    let match_tracker = Arc::new(std::sync::Mutex::new(MatchTracker {
        lobby_id,
        player_accounts,
        in_match: valid_players.keys().copied().collect(),
        logged_exits: HashSet::new(),
        finalized: false,
        tracked,
        redis_con: Arc::clone(&redis_con),
        lobby_json: lobby_json.clone(),
        match_history: Arc::clone(&match_history),
    }));
    let match_tracker_tick = Arc::clone(&match_tracker);

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

                    clients.retain(|player_id, client| {
                        match client.sender.try_send(json.clone()) {
                            Ok(()) => {
                                client.missed_ticks = 0;
                                true
                            }
                            Err(TrySendError::Full(_)) => {
                                client.missed_ticks += 1;
                                if client.missed_ticks >= MAX_MISSED_TICKS {
                                    warn!("Player {player_id} dropped: {}/{} consecutive missed ticks",
                                        client.missed_ticks, MAX_MISSED_TICKS);
                                    false
                                } else {
                                    if client.missed_ticks == 1 || client.missed_ticks % 10 == 0 {
                                        warn!("Player {player_id} slow: {} missed ticks",
                                            client.missed_ticks);
                                    }
                                    true
                                }
                            }
                            Err(TrySendError::Closed(_)) => {
                                info!("Player {player_id} channel closed, removing");
                                false
                            }
                        }
                    });

                    if last_status.elapsed().as_secs() >= 10 {
                        println!("STATUS|{}|{}|{}|{}", lobby_id, std::process::id(), port, humans);
                        last_status = std::time::Instant::now();
                        // Heartbeat: refresh Redis TTL (offloaded to blocking thread)
                        let rcon = redis_cleanup.clone();
                        let key = format!("sow:relay:{}", cleanup_port);
                        let val = lobby_id.to_string();
                        tokio::task::spawn_blocking(move || {
                            let mut guard = rcon.lock().unwrap();
                            if let Some(ref mut con) = *guard {
                                let _: () = con.set_ex(&key, val, 60).unwrap_or_default();
                            }
                        });
                    }
                }
                Some(event) = event_rx.recv() => {
                    match event {
                        RelayEvent::Gameplay { player_id, intent } => {
                            if matches!(intent, GameplayIntent::Resign) {
                                match_tracker_tick.lock().unwrap().record_exit(player_id);
                            }
                            pending_intents.push(StampedIntent { player_id, intent });
                        }
                        RelayEvent::Leave { player_id } => {
                            connected_clients_clone.lock().await.remove(&player_id);
                            info!("Player {} left relay {}", player_id, lobby_id);
                            match_tracker_tick.lock().unwrap().record_exit(player_id);
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
                            clients.retain(|player_id, client| {
                                match client.sender.try_send(json.clone()) {
                                    Ok(()) => true,
                                    Err(TrySendError::Full(_)) => {
                                        warn!("Player {player_id} slow during rematch broadcast, dropping");
                                        true
                                    }
                                    Err(TrySendError::Closed(_)) => {
                                        info!("Player {player_id} channel closed during rematch broadcast, removing");
                                        false
                                    }
                                }
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
        let accounts_map = accounts_for_ws.clone();
        let history_arc = match_history.clone();
        let redis_stats = Arc::clone(&redis_con);

        tokio::spawn(async move {
            let ws_stream = match accept_async(stream).await {
                Ok(ws) => ws,
                Err(e) => {
                    warn!("Handshake failed: {}", e);
                    return;
                }
            };
            let (mut write, mut read) = ws_stream.split();
            let (direct_tx, mut direct_rx) = mpsc::channel::<Vec<u8>>(PER_CLIENT_CHANNEL);

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
                                                    clients_map.lock().await.insert(player_id, ClientChannel { sender: direct_tx.clone(), missed_ticks: 0 });
                                                    info!("Player {} reconnected to relay", player_id);

                                                    let _ = ev_tx.try_send(RelayEvent::Gameplay {
                                                        player_id,
                                                        intent: GameplayIntent::MarkDisconnected { is_disconnected: false },
                                                    });

                                                    // Send all missed turns so they can catch up!
                                                    // Use send() with a short timeout instead of try_send
                                                    // so history is not silently dropped on a full buffer.
                                                    let hist = history_arc.lock().await;
                                                    for past_turn in hist.iter() {
                                                        let msg = ServerTurnMessage { turn: past_turn.clone() };
                                                        if let Ok(json) = bincode::serialize(&ServerMessage::Turn(msg)) {
                                                            let _ = tokio::time::timeout(Duration::from_millis(500), direct_tx.send(json)).await;
                                                        }
                                                    }
                                                } else {
                                                    warn!("Invalid Ready request for lobby {} player {}", l_id, player_id);
                                                }
                                            }
                                            ClientMessage::Gameplay { intent } => {
                                                if let Some(pid) = my_player_id {
                                                    let _ = ev_tx.try_send(RelayEvent::Gameplay { player_id: pid, intent });
                                                }
                                            }
                                            ClientMessage::Leave {} => {
                                                if let Some(pid) = my_player_id {
                                                    let _ = ev_tx.try_send(RelayEvent::Leave { player_id: pid });
                                                }
                                                my_player_id = None;
                                            }
                                            ClientMessage::RematchRequest { lobby_id: _ } => {
                                                if let Some(pid) = my_player_id {
                                                    let _ = ev_tx.try_send(RelayEvent::RematchRequest { player_id: pid });
                                                }
                                            }
                                            ClientMessage::Ping { client_time } => {
                                                let pong = ServerMessage::Pong { client_time };
                                                let json = bincode::serialize(&pong).unwrap();
                                                let _ = direct_tx.try_send(json);
                                            }
                                            ClientMessage::SubmitStats {
                                                kills,
                                                deaths,
                                                assists,
                                                players_defeated,
                                                empires_defeated,
                                                tribes_defeated,
                                            } => {
                                                if let Some(pid) = my_player_id {
                                                    if let Some(acc) = accounts_map.get(&pid) {
                                                        let mut guard = redis_stats.lock().unwrap();
                                                        if let Some(ref mut con) = *guard {
                                                            log_player_stats(
                                                                con,
                                                                lobby_id,
                                                                acc,
                                                                MatchPlayerStats {
                                                                    kills,
                                                                    deaths,
                                                                    assists,
                                                                    players_defeated,
                                                                    empires_defeated,
                                                                    tribes_defeated,
                                                                },
                                                            );
                                                            info!(
                                                                "Logged stats for player {pid} (account {acc}): K/D/A {kills}/{deaths}/{assists}"
                                                            );
                                                        }
                                                    }
                                                }
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
                        match tokio::time::timeout(Duration::from_secs(1), write.send(Message::Binary(direct_data))).await {
                            Ok(Ok(())) => {}
                            _ => break,
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_secs(15)) => {
                        break;
                    }
                }
            }

            if let Some(pid) = my_player_id {
                let _ = ev_tx.try_send(RelayEvent::Leave { player_id: pid });
            }
        });
    }
}
