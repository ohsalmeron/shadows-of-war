mod names;
use clap::Parser;
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use sow_core::engine::SowEngine;
use sow_core::protocol::{AttackIntent, ClientMessage, GameplayIntent, ServerMessage};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tokio::sync::RwLock;
use tokio::time::{Duration, Instant, sleep};
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Default)]
struct RelayMetrics {
    turn_timestamps: Vec<(u64, f64)>, // (turn_number, seconds_since_epoch)
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Spawn bot clients for load testing the game server",
    long_about = None
)]
pub struct Args {
    #[arg(short, long, default_value = "wss://shadowsofwar.io/ws/")]
    pub url: String,
    #[arg(short, long, default_value_t = 30)]
    pub count: usize,
    #[arg(long)]
    pub lobby_id: Option<u64>,
    #[arg(long, default_value_t = true)]
    pub active: bool,
}

#[derive(Clone, Default)]
struct LobbySharedState {
    engine: Arc<RwLock<Option<SowEngine>>>,
    map_bytes: Arc<RwLock<Option<Vec<u8>>>>,
    map_downloaded: Arc<RwLock<bool>>,
}

#[derive(Clone)]
struct Coordinator {
    lobbies: Arc<RwLock<HashMap<u64, LobbySharedState>>>,
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

    let coordinator = Coordinator {
        lobbies: Arc::new(RwLock::new(HashMap::new())),
    };

    for i in 0..args.count {
        let url = args.url.clone();
        let version = version.clone();
        let lobby_id = args.lobby_id;
        let active = args.active;
        let active_bots_ref = Arc::clone(&active_bots);
        let coordinator = coordinator.clone();

        let delay_ms = rand::thread_rng().gen_range(10..2000);

        let handle = tokio::spawn(async move {
            sleep(Duration::from_millis(delay_ms)).await;
            match run_bot(i, url, version, lobby_id, active, coordinator).await {
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
    coordinator: Coordinator,
) -> Result<(), String> {
    let name = {
        let mut rng = rand::thread_rng();
        names::BOT_NAMES[rng.gen_range(0..names::BOT_NAMES.len())].to_string()
    };

    let (ws, _) = tokio_tungstenite::connect_async(&url)
        .await
        .map_err(|e| format!("Connect failed: {}", e))?;
    let (mut write, mut read) = ws.split();

    let mut resolved_lobby_id = target_lobby_id;
    if resolved_lobby_id.is_none() {
        println!(
            "[Bot {}] Querying lobby list to find the active homepage lobby...",
            bot_index
        );
        loop {
            let msg = recv_msg(&mut read, 10).await?;
            if let ServerMessage::LobbiesBroadcast(broadcast) = msg {
                let matchmaking_lobbies: Vec<_> = broadcast
                    .lobbies
                    .iter()
                    .filter(|l| matches!(l.kind, sow_core::protocol::LobbyKind::Matchmaking))
                    .collect();

                let mut counting: Vec<_> = matchmaking_lobbies
                    .iter()
                    .filter(|l| l.is_counting_down)
                    .copied()
                    .collect();
                if !counting.is_empty() {
                    counting.sort_by_key(|l| l.id);
                    resolved_lobby_id = Some(counting[0].id);
                } else {
                    let mut waiting: Vec<_> = matchmaking_lobbies
                        .iter()
                        .filter(|l| !l.is_counting_down)
                        .copied()
                        .collect();
                    if !waiting.is_empty() {
                        waiting.sort_by_key(|l| l.id);
                        resolved_lobby_id = Some(waiting[0].id);
                    }
                }

                if let Some(id) = resolved_lobby_id {
                    println!(
                        "[Bot {}] Targeted active matchmaking lobby ID: {}",
                        bot_index, id
                    );
                } else {
                    println!(
                        "[Bot {}] No active matchmaking lobby found! Falling back to server default.",
                        bot_index
                    );
                }
                break;
            }
        }
    }

    let (leader, civilization) = {
        let mut rng = rand::thread_rng();
        let l =
            sow_core::player::Leader::ALL[rng.gen_range(0..sow_core::player::Leader::ALL.len())];
        (l, l.civilization())
    };

    let join = ClientMessage::Join {
        name: name.clone(),
        is_observer: false,
        target_lobby_id: resolved_lobby_id,
        host_private: false,
        build_version: version,
        clan_tag: "".to_string(),
        civilization,
        leader,
        database_account_id: None,
        host_config: None,
        password: None,
    };
    write
        .send(Message::Binary(bincode::serialize(&join).unwrap()))
        .await
        .map_err(|e| format!("Failed to send join: {}", e))?;

    let lobby_id: u64;
    let player_id: u16;
    let map_name: String;

    loop {
        let msg = recv_msg(&mut read, 15).await?;
        match msg {
            ServerMessage::JoinAck(ack) => {
                lobby_id = ack.lobby_id;
                player_id = ack.player_id;
                map_name = ack.map_name;
                println!(
                    "[Bot {}] '{}' ({:?}/{:?}) joined lobby {} as player {}",
                    bot_index, name, leader, civilization, lobby_id, player_id
                );
                break;
            }
            ServerMessage::JoinFailed(f) => return Err(format!("Join failed: {}", f.reason)),
            _ => continue,
        }
    }

    let shared = {
        let mut lobbies = coordinator.lobbies.write().await;
        lobbies.entry(lobby_id).or_default().clone()
    };

    let mut map_is_mine_to_download = false;
    {
        let mut map_lock = shared.map_downloaded.write().await;
        if !*map_lock {
            *map_lock = true;
            map_is_mine_to_download = true;
        }
    }

    if map_is_mine_to_download {
        println!("[Bot {}] Downloading map {}...", bot_index, map_name);
        // Derive maps base from WS URL, same as the real client.
        let rest = url
            .strip_prefix("wss://")
            .or_else(|| url.strip_prefix("ws://"))
            .unwrap_or(&url);
        let host = rest
            .split('/')
            .next()
            .and_then(|s| s.split(':').next())
            .unwrap_or("shadowsofwar.io");
        let maps_base = if host == "127.0.0.1" || host == "localhost" {
            format!("http://{host}:25566/maps")
        } else {
            format!("https://{host}/maps")
        };
        let map_url = format!("{}/{}/map.bin.br", maps_base, map_name);

        let client = reqwest::Client::new();
        if let Ok(resp) = client.get(&map_url).send().await {
            if let Ok(compressed) = resp.bytes().await {
                let compressed = compressed.to_vec();
                let mut uncompressed = Vec::new();
                let mut decompressor = brotli::Decompressor::new(compressed.as_slice(), 4096);
                if std::io::Read::read_to_end(&mut decompressor, &mut uncompressed).is_ok() {
                    println!(
                        "[Bot {}] Map decompressed ({} bytes)",
                        bot_index,
                        uncompressed.len()
                    );
                    *shared.map_bytes.write().await = Some(uncompressed);
                } else {
                    println!("[Bot {}] Map was not brotli compressed", bot_index);
                    *shared.map_bytes.write().await = Some(compressed);
                }
            } else {
                return Err("Failed to download map body".into());
            }
        } else {
            return Err("Failed to request map".into());
        }
    } else {
        loop {
            if shared.map_bytes.read().await.is_some() {
                break;
            }
            sleep(Duration::from_millis(100)).await;
        }
    }

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

    let relay_port: u16;
    loop {
        let msg = recv_msg(&mut read, 60).await?;
        match msg {
            ServerMessage::Start(start) => {
                relay_port = start.relay_port.unwrap_or(0);
                if relay_port == 0 {
                    return Err("Start message has no relay port".into());
                }
                println!(
                    "[Bot {}] Received Start, relay port: {}",
                    bot_index, relay_port
                );

                let mut eng = shared.engine.write().await;
                if eng.is_none() {
                    println!("[Bot {}] Initializing shared engine...", bot_index);
                    let map_bytes = shared.map_bytes.read().await.clone().unwrap();
                    let parsed_map = sow_core::maps::load_map_from_payload(&map_bytes).ok();

                    let (w, h) = if let Some(ref m) = parsed_map {
                        (m.width, m.height)
                    } else {
                        (start.config.map_width, start.config.map_height)
                    };

                    let mut state =
                        sow_core::game::GameState::new(start.seed, w, h, start.config.clone());

                    if let Some(ref map_file) = parsed_map {
                        state.total_land_tiles = map_file.num_land_tiles;
                        state.map_spawns = map_file.spawns.clone();
                        state.geo_bounds = map_file.geo_bounds;
                        if map_file.terrain.len() == state.map.terrain.len() {
                            let dest_ptr = state.map.terrain.as_mut_ptr() as *mut u8;
                            unsafe {
                                std::ptr::copy_nonoverlapping(
                                    map_file.terrain.as_ptr(),
                                    dest_ptr,
                                    map_file.terrain.len(),
                                );
                            }
                        } else {
                            eprintln!(
                                "[Bot {}] Warning: terrain length mismatch ({} vs {})",
                                bot_index,
                                map_file.terrain.len(),
                                state.map.terrain.len()
                            );
                        }
                    } else {
                        eprintln!(
                            "[Bot {}] Warning: map parse failed, using blank terrain",
                            bot_index
                        );
                    }

                    for p in &start.players {
                        let mut new_player = sow_core::player::Player::new_human(
                            p.id,
                            p.name.clone(),
                            p.color,
                            &start.config,
                        );
                        new_player.player_type = p.player_type;
                        new_player.team = p.team;
                        state.register_player(new_player);
                    }

                    let water =
                        sow_core::water_components::WaterComponents::compute(&state.map, |_| {});
                    let mut engine = SowEngine::new(state, water);

                    for turn in &start.missed_turns {
                        engine.apply_intents(&turn.intents);
                        engine.tick();
                    }
                    *eng = Some(engine);
                    println!("[Bot {}] Shared engine ready!", bot_index);
                }
                break;
            }
            _ => continue,
        }
    }

    drop(write);
    drop(read);

    let relay_url = if url.contains("shadowsofwar.io") {
        format!("wss://relay.shadowsofwar.io:{}/ws/", relay_port)
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

    let ready = ClientMessage::Ready {
        lobby_id,
        player_id,
    };
    r_write
        .send(Message::Binary(bincode::serialize(&ready).unwrap()))
        .await
        .map_err(|e| e.to_string())?;

    let mut metrics = RelayMetrics::default();
    let relay_start = Instant::now();
    let mut last_turn_at: Option<Instant> = None;
    let mut turn_gaps: Vec<f64> = Vec::with_capacity(4096);

    loop {
        let msg = match recv_msg(&mut r_read, 15).await {
            Ok(m) => m,
            Err(_) => break,
        };

        if let ServerMessage::Turn(turn) = msg {
            let now = Instant::now();
            if let Some(last) = last_turn_at {
                let gap = now.saturating_duration_since(last).as_secs_f64();
                turn_gaps.push(gap);
            }
            last_turn_at = Some(now);
            metrics
                .turn_timestamps
                .push((turn.turn.turn_number, relay_start.elapsed().as_secs_f64()));

            if metrics.turn_timestamps.len() % 100 == 0 {
                let mut min_gap = f64::MAX;
                let mut max_gap = 0.0f64;
                let mut total = 0.0f64;
                for g in &turn_gaps[turn_gaps.len().saturating_sub(100)..] {
                    min_gap = min_gap.min(*g);
                    max_gap = max_gap.max(*g);
                    total += *g;
                }
                let avg = total / turn_gaps[turn_gaps.len().saturating_sub(100)..].len() as f64;
                println!(
                    "[Bot {}] turn#{} gap_window(100) min={:.1}ms avg={:.1}ms max={:.1}ms total_turns={}",
                    bot_index,
                    turn.turn.turn_number,
                    min_gap * 1000.0,
                    avg * 1000.0,
                    max_gap * 1000.0,
                    metrics.turn_timestamps.len()
                );
            }
            let intent = if active {
                let mut intent_to_send = None;

                let mut eng_guard = shared.engine.write().await;
                if let Some(engine) = eng_guard.as_mut() {
                    if engine.state.tick < turn.turn.turn_number {
                        engine.apply_intents(&turn.turn.intents);
                        engine.tick();
                    }

                    let phase = engine.state.phase.clone();
                    let mut rng = rand::thread_rng();

                    if rng.gen_bool(0.15) {
                        if let sow_core::game::GamePhase::Spawning { .. } = phase {
                            let w = engine.state.map.width;
                            let h = engine.state.map.height;
                            let mut tries = 0;
                            while tries < 100 {
                                let x = rng.gen_range(0..w);
                                let y = rng.gen_range(0..h);
                                let tile = engine.state.map.terrain[engine.state.map.ref_id(x, y)];
                                if tile.is_land() && engine.state.map.owner_id(x, y) == 0 {
                                    intent_to_send = Some(GameplayIntent::Spawn { x, y });
                                    break;
                                }
                                tries += 1;
                            }
                        } else if phase == sow_core::game::GamePhase::Playing {
                            if let Some(player) =
                                engine.state.players.iter().find(|p| p.id == player_id)
                            {
                                if player.alive && player.border_tiles.count_ones() > 0 {
                                    let border_count = player.border_tiles.count_ones();
                                    let mut target_owner = None;
                                    let mut ones = player.border_tiles.ones();
                                    let r_idx = rng.gen_range(0..border_count) as usize;
                                    if let Some(chosen_idx) = ones.nth(r_idx) {
                                        let bx = chosen_idx % engine.state.map.width;
                                        let by = chosen_idx / engine.state.map.width;
                                        let neighbors = engine.state.map.neighbors(bx, by);
                                        let mut targets = Vec::new();
                                        for (nx, ny) in neighbors {
                                            let owner = engine.state.map.owner_id(nx, ny);
                                            if owner != player_id {
                                                let is_land = engine.state.map.terrain
                                                    [engine.state.map.ref_id(nx, ny)]
                                                .is_land();
                                                if is_land {
                                                    targets.push(owner);
                                                }
                                            }
                                        }
                                        if !targets.is_empty() {
                                            target_owner =
                                                Some(targets[rng.gen_range(0..targets.len())]);
                                        }
                                    }

                                    if let Some(target) = target_owner {
                                        let required_cost = if target == 0 {
                                            engine.state.config.attack_cost_neutral
                                        } else {
                                            engine.state.config.attack_cost_enemy
                                        };
                                        if player.troops >= required_cost {
                                            intent_to_send =
                                                Some(GameplayIntent::Attack(AttackIntent {
                                                    target_owner: target,
                                                    troops: None,
                                                }));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                intent_to_send
            } else {
                None
            };

            if let Some(intent) = intent {
                let gm = ClientMessage::Gameplay { intent };
                let _ = r_write
                    .send(Message::Binary(bincode::serialize(&gm).unwrap()))
                    .await;
            }
        }
    }

    if !turn_gaps.is_empty() {
        let total = turn_gaps.len() as f64;
        let sum: f64 = turn_gaps.iter().sum();
        let avg = sum / total;
        let mut sorted = turn_gaps.clone();
        sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p50 = sorted[(sorted.len() as f64 * 0.50) as usize];
        let p95 = sorted[(sorted.len() as f64 * 0.95) as usize];
        let p99 = sorted[(sorted.len() as f64 * 0.99) as usize];
        println!(
            "[Bot {}] RELAY STATS: {} turns, gaps(ms) avg={:.1} p50={:.1} p95={:.1} p99={:.1}",
            bot_index,
            turn_gaps.len(),
            avg * 1000.0,
            p50 * 1000.0,
            p95 * 1000.0,
            p99 * 1000.0
        );
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
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
    loop {
        let now = tokio::time::Instant::now();
        if now >= deadline {
            return Err(format!("Timeout waiting for {}s", timeout_secs));
        }

        match tokio::time::timeout(deadline - now, read.next()).await {
            Ok(Some(Ok(Message::Binary(data)))) => {
                if let Ok(msg) = bincode::deserialize::<ServerMessage>(&data) {
                    return Ok(msg);
                }
            }
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(e))) => return Err(format!("WS error: {}", e)),
            Ok(None) => return Err("Stream ended".to_string()),
            Err(_) => return Err(format!("Timeout waiting for {}s", timeout_secs)),
        }
    }
}
