//! M3-FaseB — multi-lobby game relay over the bridge + mgmt HTTP register.
//!
//! One process handles every lobby assigned to it (no per-match processes):
//!
//! - VF :80 (F-Stack, DPDK): game WebSockets. The first binary frame decides
//!   the role of the connection:
//!     - `ClientMessage::Join`     → stub-server phase (validation only): answers
//!       JoinAck / Start so real `sow-backfill` bots can drive the relay.
//!     - `ClientMessage::Ready`    → relay phase: `lobby_id` lookup in the
//!       in-memory `LobbyRegistry`, per-lobby tick loop broadcasts `Turn`s at
//!       `tick_rate_ms`, `Gameplay` intents flow back through the same conn.
//!   Connections that send nothing in 3s are treated as the orchestrator WS:
//!   they receive periodic `LobbiesBroadcast` (so the backfill daemon spawns
//!   bots against this relay).
//! - mgmt :8080 (kernel, 127.0.0.1 — mgmt NIC): internal HTTP for the
//!   orchestrator — `POST /internal/lobby/register`, `GET /internal/lobbies`.
//!
//! The tick loop / MatchTracker / finalize / stats logic is ported verbatim
//! from sow-relay/src/main.rs; the only architectural change is the registry.
//! `bridge.rs` is untouched.
//!
//! Run (physical VF):
//!   ./relay_full --conf echo-vf.ini --proc-type=primary --proc-id=0 --stub
//!
//! Test:
//!   curl -s -X POST 127.0.0.1:8080/internal/lobby/register -d @lobby.json
//!   sow-backfill --url ws://<data-pip>:80/ws/ --maps-root ~/maps

use fstack_bridge::bridge::{self, Ev};
use fstack_bridge::ffi::{ev_set, kevent, EV_ADD, EVFILT_READ};
use futures_util::{SinkExt, StreamExt};
use libc::{
    c_int, c_void, sockaddr_in, socklen_t, AF_INET, INADDR_ANY, SOCK_STREAM, SOL_SOCKET,
    SO_REUSEADDR, FIONBIO,
};
use log::{error, info, warn};
use redis::Commands;
use sow_core::game_config::{BotDifficulty, GameConfig};
use sow_core::player::PlayerType;
use sow_core::protocol::{
    ClientMessage, GameplayIntent, LobbyInfo, LobbyKind, LobbyPlayerSyncState, PlayerInfo,
    ServerJoinAckMessage, ServerJoinFailedMessage, ServerLobbiesBroadcastMessage,
    ServerLobbyClosedMessage, ServerMessage, ServerStartMessage, ServerTurnMessage,
    StampedIntent, Turn,
};
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicU16, Ordering};
use std::sync::{Arc, OnceLock};
use tokio::net::TcpListener;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{interval, Duration};
use tokio_tungstenite::tungstenite::Message;

const GAME_PORT: u16 = 80;
const MGMT_PORT: u16 = 8080;
/// Advertised relay address (data PIP) — what bots connect to for the relay.
const DATA_RELAY_ADDR: &str = "20.122.128.185:80";
/// Stub server (validation): lobby id + slot cap for backfill bots.
const STUB_LOBBY_ID: u64 = 42;
const STUB_MAX_PLAYERS: u16 = 30;
const STUB_MAP: &str = "world";
/// Seconds without any frame before a connection is classified as the
/// orchestrator WS (backfill daemon sends nothing, bots Join within ~2s).
const ORCHESTRATOR_GRACE_SECS: u64 = 3;

const REDIS_PORTS_KEY: &str = "sow:ports";
const MAX_MISSED_TICKS: u32 = 120;
const PER_CLIENT_CHANNEL: usize = 4096;
const EVENT_CHANNEL: usize = 1024;

// ---- relay event plumbing (verbatim from sow-relay) ------------------------

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

struct ClientChannel {
    sender: mpsc::Sender<Vec<u8>>,
    missed_ticks: u32,
}

// ---- redis helpers (verbatim from sow-relay) --------------------------------

fn redis_connect() -> Option<redis::Connection> {
    let url = std::env::var("SOW_VALKEY_URL")
        .or_else(|_| std::env::var("SOW_REDIS_URL"))
        .unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
    redis::Client::open(url)
        .ok()
        .and_then(|c| c.get_connection().ok())
}

static REDIS_CON: OnceLock<Arc<std::sync::Mutex<Option<redis::Connection>>>> = OnceLock::new();

fn redis_shared() -> Arc<std::sync::Mutex<Option<redis::Connection>>> {
    REDIS_CON
        .get_or_init(|| Arc::new(std::sync::Mutex::new(redis_connect())))
        .clone()
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

fn trigger_match_finalize(match_id: u64, lobby_json: String, match_history: Arc<Mutex<Vec<Turn>>>) {
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

        let payload = serde_json::json!({
            "match_id": match_id.to_string(),
            "lobby_json": Some(lobby_json.clone()),
            "replay_data": Some(replay_bytes.clone()),
        });

        let mut success = false;
        let client = reqwest::Client::new();

        for attempt in 1..=5 {
            info!(
                "Attempting raw upload/finalize to database for match {match_id} (Attempt {attempt}/5)..."
            );
            match client
                .post(&url)
                .header("Authorization", format!("Bearer {secret}"))
                .json(&payload)
                .send()
                .await
            {
                Ok(res) if res.status().is_success() => {
                    info!("Match {match_id} successfully finalized and archived!");
                    success = true;
                    break;
                }
                Ok(res) => {
                    warn!(
                        "Attempt {attempt}/5: database returned HTTP status {} for match {match_id}",
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
            error!("[CRITICAL] Failed to upload match {match_id} to database after 5 attempts.");

            let url = std::env::var("SOW_VALKEY_URL")
                .or_else(|_| std::env::var("SOW_REDIS_URL"))
                .unwrap_or_else(|_| "redis://127.0.0.1/".to_string());

            let mut valkey_success = false;
            if let Ok(client) = redis::Client::open(url) {
                if let Ok(mut con) = client.get_connection() {
                    let key = "sow:match_history:dead_letter";
                    let fallback_payload =
                        bincode::serialize(&(lobby_json.clone(), replay_bytes.clone()))
                            .unwrap_or_default();
                    if let Ok(()) = con.lpush::<_, _, ()>(key, fallback_payload) {
                        warn!(
                            "[FALLBACK] Saved raw replay backup in local Valkey queue under key '{}' for match {match_id}",
                            key
                        );
                        valkey_success = true;
                    }
                }
            }

            if !valkey_success {
                let backup_dir = "/tmp/sow_crash_replays";
                error!(
                    "[ALERT] Valkey local fallback also failed! Dumping raw payload directly to local disk at {} for match {match_id}",
                    backup_dir
                );
                let _ = std::fs::create_dir_all(backup_dir);
                let _ = std::fs::write(format!("{}/{}.json", backup_dir, match_id), &lobby_json);
                let _ =
                    std::fs::write(format!("{}/{}.replay", backup_dir, match_id), &replay_bytes);
            }
        }
    });
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

// ---- registry / lobby state -------------------------------------------------

/// Register body — the orchestrator's lobby shape (RelayConfig from sow-relay)
/// plus the optional GameConfig for the coming simulation phase.
#[derive(serde::Deserialize, serde::Serialize)]
struct RegisterBody {
    lobby_id: u64,
    tick_number: u64,
    tick_rate_ms: f32,
    active_empty_secs: f32,
    players: Vec<PlayerEntry>,
    #[serde(default)]
    config: Option<GameConfig>,
}

#[derive(serde::Deserialize, serde::Serialize, Clone)]
struct PlayerEntry {
    player_id: u16,
    name: String,
    database_account_id: Option<String>,
}

struct LobbyState {
    id: u64,
    valid_players: HashMap<u16, String>,
    clients: Arc<Mutex<HashMap<u16, ClientChannel>>>,
    match_history: Arc<Mutex<Vec<Turn>>>,
    tracker: Arc<std::sync::Mutex<MatchTracker>>,
    ev_tx: mpsc::Sender<RelayEvent>,
    config: Option<GameConfig>,
}

type Registry = Arc<RwLock<HashMap<u64, Arc<LobbyState>>>>;

/// Stub server state (validation): assigns bot player_ids and reports the
/// lobby roster so the backfill daemon keeps spawning until full.
struct StubState {
    next_player: AtomicU16,
    joined: Mutex<Vec<(u16, String, sow_core::player::Civilization, sow_core::player::Leader)>>,
}

impl StubState {
    fn new() -> Arc<Self> {
        Arc::new(StubState {
            next_player: AtomicU16::new(1),
            joined: Mutex::new(Vec::new()),
        })
    }

    async fn lobby_info(&self) -> LobbyInfo {
        let joined = self.joined.lock().await.clone();
        let mut players = vec![LobbyPlayerSyncState {
            name: "HumanHost".to_string(),
            is_ready: false,
            download_progress: 0,
            leader: sow_core::player::Leader::Cleopatra,
            player_id: 0,
            team: None,
        }];
        for (pid, name, _, leader) in &joined {
            players.push(LobbyPlayerSyncState {
                name: name.clone(),
                is_ready: false,
                download_progress: 100,
                leader: *leader,
                player_id: *pid,
                team: None,
            });
        }
        LobbyInfo {
            id: STUB_LOBBY_ID,
            num_players: players.len() as u32,
            max_players: STUB_MAX_PLAYERS as u32,
            is_counting_down: false,
            timer_secs: 0.0,
            map_name: STUB_MAP.to_string(),
            game_mode: "FFA".to_string(),
            players,
            has_password: false,
            host_name: "HumanHost".to_string(),
            bot_count: joined.len() as u32,
            nation_count: 0,
            bot_difficulty: BotDifficulty::Vanilla,
            kind: LobbyKind::Matchmaking,
        }
    }

    /// Full roster as Start players — every joined bot, so each bot's local
    /// engine simulates an N-player match (1-player matches end immediately).
    async fn start_players(&self, self_id: u16) -> Vec<PlayerInfo> {
        fn color_for(id: u16) -> [f32; 3] {
            let c = (id as f32) * 0.6180339887;
            [c.fract(), (c + 0.33).fract(), (c + 0.66).fract()]
        }
        let joined = self.joined.lock().await.clone();
        let mut out: Vec<PlayerInfo> = joined
            .iter()
            .map(|(pid, name, civ, leader)| PlayerInfo {
                id: *pid,
                name: name.clone(),
                player_type: PlayerType::Human,
                color: color_for(*pid),
                team: None,
                spawn_x: 0,
                spawn_y: 0,
                civilization: *civ,
                leader: *leader,
            })
            .collect();
        if !out.iter().any(|p| p.id == self_id) {
            out.push(PlayerInfo {
                id: self_id,
                name: format!("Bot{self_id}"),
                player_type: PlayerType::Human,
                color: color_for(self_id),
                team: None,
                spawn_x: 0,
                spawn_y: 0,
                civilization: sow_core::player::Civilization::Rome,
                leader: sow_core::player::Leader::Caesar,
            });
        }
        out
    }
}

// ---- main (bridge boot — identical shape to relay_bincode) -------------------

fn main() {
    let prog_args: Vec<CString> = std::env::args()
        .filter(|a| !a.starts_with("--fstack-") && a != "--stub")
        .map(|a| CString::new(a).unwrap())
        .collect();
    let stub_enabled = std::env::args().any(|a| a == "--stub");

    unsafe {
        if let Err(code) = fstack_bridge::init(&prog_args, &[]) {
            eprintln!("[relay-full] init failed (code={})", code);
            std::process::exit(1);
        }

        let (mut pid, mut qid, mut nbq, mut reta) = (0u16, 0u16, 0u16, 0u16);
        if fstack_bridge::ff_rss_self_queue_info(&mut pid, &mut qid, &mut nbq, &mut reta) == 0 {
            eprintln!(
                "[BOOT] proc_id={} queue_id={} nb_queues={} reta_size={}",
                pid, qid, nbq, reta
            );
        }

        bridge::setup();
        bridge::KQ = fstack_bridge::ff_kqueue();
        if bridge::KQ < 0 {
            eprintln!("[relay-full] ff_kqueue failed");
            std::process::exit(1);
        }

        let lfd = fstack_bridge::ff_socket(AF_INET, SOCK_STREAM, 0);
        if lfd < 0 {
            eprintln!("[relay-full] ff_socket failed");
            std::process::exit(1);
        }
        bridge::LISTEN_FD = lfd;

        let on: c_int = 1;
        fstack_bridge::ff_setsockopt(
            lfd,
            SOL_SOCKET,
            SO_REUSEADDR,
            &on as *const _ as *const c_void,
            mem::size_of::<c_int>() as socklen_t,
        );
        fstack_bridge::ff_ioctl(lfd, FIONBIO as libc::c_ulong, &on);

        let mut addr: sockaddr_in = mem::zeroed();
        addr.sin_family = AF_INET as u16;
        addr.sin_port = GAME_PORT.to_be();
        addr.sin_addr.s_addr = INADDR_ANY;
        if fstack_bridge::ff_bind(lfd, &addr, mem::size_of::<sockaddr_in>() as socklen_t) < 0 {
            eprintln!("[relay-full] ff_bind :{} failed", GAME_PORT);
            std::process::exit(1);
        }
        if fstack_bridge::ff_listen(lfd, 512) < 0 {
            eprintln!("[relay-full] ff_listen failed");
            std::process::exit(1);
        }

        let mut kev: kevent = mem::zeroed();
        ev_set(&mut kev, lfd as usize, EVFILT_READ, EV_ADD, 0, 512, ptr::null_mut());
        fstack_bridge::ff_kevent(bridge::KQ, &kev, 1, ptr::null_mut(), 0, ptr::null());

        // Redis: advertise the single game port once at boot.
        {
            let redis_con = redis_shared();
            let mut guard = redis_con.lock().unwrap();
            if let Some(ref mut con) = *guard {
                if let Err(e) = con.sadd::<_, _, ()>(REDIS_PORTS_KEY, GAME_PORT) {
                    error!("[REDIS] SADD {REDIS_PORTS_KEY} {} FAILED: {e}", GAME_PORT);
                }
                info!("Registered port {} in Redis", GAME_PORT);
            }
        }

        let registry: Registry = Arc::new(RwLock::new(HashMap::new()));
        let stub = stub_enabled.then(StubState::new);

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.spawn(mgmt_http(registry.clone()));
        rt.spawn(bridge_worker(registry.clone(), stub));

        eprintln!(
            "[BOOT] listening on :{}, mgmt :{} (stub={}), entering ff_run",
            GAME_PORT,
            MGMT_PORT,
            stub_enabled
        );
        fstack_bridge::ff_run(bridge::driver_cb, ptr::null_mut());
    }
}

// ---- mgmt HTTP (kernel, 127.0.0.1 — never touches the bridge) ----------------

async fn mgmt_http(registry: Registry) {
    let listener = match TcpListener::bind(("127.0.0.1", MGMT_PORT)).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("[mgmt] bind 127.0.0.1:{} failed: {}", MGMT_PORT, e);
            return;
        }
    };
    eprintln!("[mgmt] http listening on 127.0.0.1:{}", MGMT_PORT);

    loop {
        let (mut sock, _) = match listener.accept().await {
            Ok(x) => x,
            Err(_) => continue,
        };
        let reg = registry.clone();
        tokio::spawn(async move {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};

            let mut buf: Vec<u8> = Vec::new();
            let mut tmp = [0u8; 4096];
            let header_end;
            loop {
                match sock.read(&mut tmp).await {
                    Ok(0) | Err(_) => return,
                    Ok(n) => buf.extend_from_slice(&tmp[..n]),
                }
                if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                    header_end = i + 4;
                    break;
                }
                if buf.len() > 65536 {
                    return;
                }
            }

            let head = String::from_utf8_lossy(&buf[..header_end]).to_string();
            let mut lines = head.lines();
            let mut parts = lines.next().unwrap_or("").split_whitespace();
            let method = parts.next().unwrap_or("").to_string();
            let path = parts.next().unwrap_or("").to_string();
            let clen: usize = head
                .lines()
                .skip(1)
                .find_map(|l| {
                    let (k, v) = l.split_once(':')?;
                    if k.trim().eq_ignore_ascii_case("content-length") {
                        v.trim().parse().ok()
                    } else {
                        None
                    }
                })
                .unwrap_or(0);

            while buf.len() < header_end + clen && clen > 0 {
                match sock.read(&mut tmp).await {
                    Ok(0) | Err(_) => break,
                    Ok(n) => buf.extend_from_slice(&tmp[..n]),
                }
            }

            let body = &buf[header_end..(header_end + clen).min(buf.len())];
            let (status, resp) = handle_http(&reg, &method, &path, body).await;
            let resp_body = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string());
            let out = format!(
                "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                resp_body.len(),
                resp_body
            );
            let _ = sock.write_all(out.as_bytes()).await;
        });
    }
}

async fn handle_http(
    registry: &Registry,
    method: &str,
    path: &str,
    body: &[u8],
) -> (&'static str, serde_json::Value) {
    match (method, path) {
        ("POST", "/internal/lobby/register") => {
            let rb: RegisterBody = match serde_json::from_slice(body) {
                Ok(b) => b,
                Err(e) => {
                    return (
                        "400 Bad Request",
                        serde_json::json!({ "error": format!("bad body: {e}") }),
                    )
                }
            };
            if rb.lobby_id == 0 {
                return ("400 Bad Request", serde_json::json!({ "error": "lobby_id required" }));
            }
            let exists = registry.read().await.contains_key(&rb.lobby_id);
            if !exists {
                spawn_lobby(registry, rb).await;
            }
            (
                "200 OK",
                serde_json::json!({ "ok": true, "existing": exists }),
            )
        }
        ("GET", "/internal/lobbies") => {
            let reg = registry.read().await;
            let lobbies: Vec<serde_json::Value> = reg
                .iter()
                .map(|(id, st)| {
                    serde_json::json!({
                        "lobby_id": id,
                        "humans": st.clients.try_lock().map(|c| c.len()).unwrap_or(0),
                    })
                })
                .collect();
            ("200 OK", serde_json::json!({ "lobbies": lobbies }))
        }
        _ => ("404 Not Found", serde_json::json!({ "error": "not found" })),
    }
}

async fn spawn_lobby(registry: &Registry, body: RegisterBody) -> Arc<LobbyState> {
    let lobby_json = serde_json::to_string(&body).unwrap_or_default();
    let mut valid_players: HashMap<u16, String> = HashMap::new();
    let mut player_accounts: HashMap<u16, String> = HashMap::new();
    for p in &body.players {
        valid_players.insert(p.player_id, p.name.clone());
        if let Some(acc) = &p.database_account_id {
            player_accounts.insert(p.player_id, acc.clone());
        }
    }

    let clients = Arc::new(Mutex::new(HashMap::new()));
    let match_history = Arc::new(Mutex::new(Vec::new()));
    let (ev_tx, ev_rx) = mpsc::channel::<RelayEvent>(EVENT_CHANNEL);
    let redis_con = redis_shared();
    let tracked = !player_accounts.is_empty();
    let tracker = Arc::new(std::sync::Mutex::new(MatchTracker {
        lobby_id: body.lobby_id,
        player_accounts,
        in_match: valid_players.keys().copied().collect(),
        logged_exits: HashSet::new(),
        finalized: false,
        tracked,
        redis_con: redis_con.clone(),
        lobby_json: lobby_json.clone(),
        match_history: match_history.clone(),
    }));

    let state = Arc::new(LobbyState {
        id: body.lobby_id,
        valid_players,
        clients,
        match_history,
        tracker,
        ev_tx,
        config: body.config,
    });

    eprintln!(
        "[registry] lobby {} registered (players={} tick_rate_ms={} tracked={})",
        state.id,
        state.valid_players.len(),
        body.tick_rate_ms,
        tracked
    );

    registry.write().await.insert(state.id, state.clone());

    // Per-lobby Redis registration: sow:relay:{lobby_id} -> data PIP (TTL 60,
    // refreshed by the tick loop heartbeat every 10s).
    {
        let redis_con = redis_shared();
        let key = format!("sow:relay:{}", state.id);
        let val = DATA_RELAY_ADDR.to_string();
        tokio::task::spawn_blocking(move || {
            let mut guard = redis_con.lock().unwrap();
            if let Some(ref mut con) = *guard {
                if let Err(e) = con.set_ex::<_, _, ()>(&key, val, 60) {
                    error!("[REDIS] SETEX {key} FAILED: {e}");
                }
            }
        });
    }

    tokio::spawn(tick_task(
        state.clone(),
        registry.clone(),
        ev_rx,
        body.tick_rate_ms as u64,
        body.tick_number,
        body.active_empty_secs,
    ));
    state
}

// ---- per-lobby tick loop (verbatim from sow-relay) ---------------------------

async fn tick_task(
    state: Arc<LobbyState>,
    registry: Registry,
    mut ev_rx: mpsc::Receiver<RelayEvent>,
    tick_rate_ms: u64,
    mut tick_number: u64,
    mut active_empty_secs: f32,
) {
    let mut ticker = interval(Duration::from_millis(tick_rate_ms));
    let mut last_status = std::time::Instant::now();
    let mut generated_rematch_id: Option<u64> = None;
    let mut pending_intents = Vec::new();
    let mut total_ticks: u64 = 0;
    let mut total_intents: u64 = 0;

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let mut clients = state.clients.lock().await;
                let humans = clients.len();

                if humans == 0 {
                    active_empty_secs -= 0.05;
                    if active_empty_secs <= 0.0 {
                        eprintln!("[relay] lobby {} empty for too long, GC", state.id);
                        registry.write().await.remove(&state.id);
                        break;
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
                total_ticks += 1;
                total_intents += turn.intents.len() as u64;

                state.match_history.lock().await.push(turn.clone());

                let msg = ServerTurnMessage { turn };
                let json = bincode::serialize(&ServerMessage::Turn(msg))
                    .expect("serialize ServerTurnMessage");

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
                    eprintln!(
                        "STATUS|{}|{}|{}|{}|{}|{}",
                        state.id, std::process::id(), GAME_PORT, humans, total_ticks, total_intents
                    );
                    last_status = std::time::Instant::now();
                    // Heartbeat: refresh per-lobby Redis TTL.
                    let redis_con = redis_shared();
                    let key = format!("sow:relay:{}", state.id);
                    let val = DATA_RELAY_ADDR.to_string();
                    tokio::task::spawn_blocking(move || {
                        let mut guard = redis_con.lock().unwrap();
                        if let Some(ref mut con) = *guard {
                            if let Err(e) = con.set_ex::<_, _, ()>(&key, val, 60) {
                                error!("[REDIS] SETEX {key} FAILED: {e}");
                            }
                        }
                    });
                }
            }
            Some(event) = ev_rx.recv() => {
                match event {
                    RelayEvent::Gameplay { player_id, intent } => {
                        if matches!(intent, GameplayIntent::Resign) {
                            state.tracker.lock().unwrap().record_exit(player_id);
                        }
                        pending_intents.push(StampedIntent { player_id, intent });
                    }
                    RelayEvent::Leave { player_id } => {
                        state.clients.lock().await.remove(&player_id);
                        info!("Player {} left relay {}", player_id, state.id);
                        state.tracker.lock().unwrap().record_exit(player_id);
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
                        let msg = ServerLobbyClosedMessage {
                            lobby_id: state.id,
                            reason: "Rematch Requested".to_string(),
                            rematch_lobby_id: Some(rematch_id),
                        };
                        let json = bincode::serialize(&ServerMessage::LobbyClosed(msg))
                            .expect("serialize LobbyClosed");
                        let mut clients = state.clients.lock().await;
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
    eprintln!("[relay] lobby {} tick task ended", state.id);
}

// ---- bridge worker (dispatch — identical to relay_bincode) -------------------

async fn bridge_worker(registry: Registry, stub: Option<Arc<StubState>>) {
    let rx = bridge::rx_ring();
    let notify = bridge::notify();
    let mut conns: HashMap<c_int, mpsc::UnboundedSender<bridge::ZcRxGuard>> = HashMap::new();

    loop {
        while let Some(ev) = rx.pop() {
            match ev {
                Ev::Accept { fd, generation } => {
                    let (tx, rx_conn) = mpsc::unbounded_channel();
                    conns.insert(fd, tx);
                    tokio::spawn(ws_task(
                        fd,
                        generation,
                        rx_conn,
                        registry.clone(),
                        stub.clone(),
                    ));
                }
                Ev::Data { fd, guard } => match conns.get(&fd) {
                    Some(tx) => {
                        let _ = tx.send(guard);
                    }
                    None => drop(guard),
                },
                Ev::Closed { fd } => {
                    conns.remove(&fd);
                }
            }
        }
        notify.notified().await;
    }
}

// ---- per-connection protocol task -------------------------------------------

#[derive(Clone)]
enum Role {
    StubBot {
        player_id: u16,
        name: String,
        civilization: sow_core::player::Civilization,
        leader: sow_core::player::Leader,
    },
    RelayPlayer {
        lobby: Arc<LobbyState>,
        player_id: u16,
    },
}

async fn ws_task(
    fd: c_int,
    generation: u64,
    rx: mpsc::UnboundedReceiver<bridge::ZcRxGuard>,
    registry: Registry,
    stub: Option<Arc<StubState>>,
) {
    let conn = bridge::Conn::new(fd, generation, rx);
    let ws = match tokio_tungstenite::accept_async(conn).await {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("[ws] handshake fail fd={} err={}", fd, e);
            return;
        }
    };
    eprintln!("[ws] handshake OK fd={}", fd);

    let (mut write, mut read) = ws.split();
    let (direct_tx, mut direct_rx) = mpsc::channel::<Vec<u8>>(PER_CLIENT_CHANNEL);

    // First frame decides the role. The backfill daemon (orchestrator) sends
    // nothing: classify it after a short grace period.
    let role: Option<Role>;
    match tokio::time::timeout(
        Duration::from_secs(ORCHESTRATOR_GRACE_SECS),
        read.next(),
    )
    .await
    {
        Ok(Some(Ok(Message::Binary(b)))) => {
            match bincode::deserialize::<ClientMessage>(&b) {
                Ok(ClientMessage::Join {
                    name,
                    civilization,
                    leader,
                    ..
                }) => {
                    if let Some(stub_state) = &stub {
                        let pid = stub_state.next_player.fetch_add(1, Ordering::SeqCst);
                        if pid > STUB_MAX_PLAYERS {
                            let fail = ServerMessage::JoinFailed(ServerJoinFailedMessage {
                                reason: "lobby full".to_string(),
                            });
                            let _ = direct_tx.try_send(bincode::serialize(&fail).unwrap_or_default());
                            eprintln!("[stub] join full fd={}", fd);
                            role = None;
                        } else {
                            stub_state
                                .joined
                                .lock()
                                .await
                                .push((pid, name.clone(), civilization, leader));
                            let ack = ServerMessage::JoinAck(ServerJoinAckMessage {
                                lobby_id: STUB_LOBBY_ID,
                                player_id: pid,
                                map_name: STUB_MAP.to_string(),
                                is_private: false,
                                lobby_info: None,
                            });
                            let _ = direct_tx.try_send(bincode::serialize(&ack).unwrap_or_default());
                            eprintln!("[stub] join pid={} name={} fd={}", pid, name, fd);
                            role = Some(Role::StubBot {
                                player_id: pid,
                                name,
                                civilization,
                                leader,
                            });
                        }
                    } else {
                        eprintln!("[stub] Join ignored (stub disabled) fd={}", fd);
                        role = None;
                    }
                }
                Ok(ClientMessage::Ready { lobby_id, player_id }) => {
                    role = try_ready_register(&registry, lobby_id, player_id, &direct_tx).await;
                    eprintln!(
                        "[bincode] first-frame Ready lobby={} player={} role={} fd={}",
                        lobby_id,
                        player_id,
                        role.is_some(),
                        fd
                    );
                }
                Ok(_) => {
                    eprintln!("[ws] ignored first frame fd={}", fd);
                    role = None;
                }
                Err(e) => {
                    eprintln!("[bincode] deserialize err {} fd={}", e, fd);
                    role = None;
                }
            }
        }
        Ok(Some(Ok(_))) => role = None,
        Ok(Some(Err(e))) => {
            eprintln!("[ws] recv err fd={} err={}", fd, e);
            std::mem::forget((write, read));
            return;
        }
        Ok(None) => {
            std::mem::forget((write, read));
            return;
        }
        Err(_) => {
            // Nothing in the grace window → orchestrator (backfill daemon) WS.
            eprintln!("[ws] orchestrator role fd={}", fd);
            orchestrator_task(&mut write, &mut read, &stub).await;
            std::mem::forget((write, read));
            return;
        }
    }

    // Stub bot: answer Ready with Start, then wait for the client to leave.
    if let Some(Role::StubBot { player_id, .. }) = &role
    {
        let pid = *player_id;
        loop {
            tokio::select! {
                msg = read.next() => {
                    match msg {
                        Some(Ok(Message::Binary(b))) => {
                            if let Ok(ClientMessage::Ready { .. }) = bincode::deserialize(&b) {
                                let start_players = match &stub {
                                    Some(s) => s.start_players(pid).await,
                                    None => Vec::new(),
                                };
                                let start = ServerMessage::Start(Box::new(ServerStartMessage {
                                    config: GameConfig::default(),
                                    my_player_id: Some(pid),
                                    lobby_id: Some(STUB_LOBBY_ID),
                                    seed: 42,
                                    players: start_players,
                                    missed_turns: Vec::new(),
                                    map_data: None,
                                    relay_port: Some(GAME_PORT),
                                    relay_host: Some(DATA_RELAY_ADDR.split(':').next().unwrap_or("").to_string()),
                                }));
                                let _ = direct_tx.try_send(bincode::serialize(&start).unwrap_or_default());
                                eprintln!("[stub] start pid={} fd={}", pid, fd);
                            }
                        }
                        Some(Ok(_)) => {}
                        _ => break,
                    }
                }
                Some(data) = direct_rx.recv() => {
                    match tokio::time::timeout(Duration::from_secs(1), write.send(Message::Binary(data))).await {
                        Ok(Ok(())) => {}
                        _ => break,
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(15)) => { break; }
            }
        }
        eprintln!("[stub] bot pid={} done fd={}", pid, fd);
        return;
    }

    // Relay player: the sow-relay per-connection loop, routed to its lobby.
    let my_lobby: Option<Arc<LobbyState>> = match &role {
        Some(Role::RelayPlayer { lobby, .. }) => Some(lobby.clone()),
        _ => None,
    };
    let my_player_id: Option<u16> = match &role {
        Some(Role::RelayPlayer { player_id, .. }) => Some(*player_id),
        _ => None,
    };

    if let (Some(lobby), Some(pid)) = (&my_lobby, &my_player_id) {
        let _ = lobby.ev_tx.try_send(RelayEvent::Gameplay {
            player_id: *pid,
            intent: GameplayIntent::MarkDisconnected { is_disconnected: false },
        });
        let hist = lobby.match_history.lock().await;
        for past_turn in hist.iter() {
            let msg = ServerTurnMessage {
                turn: past_turn.clone(),
            };
            if let Ok(json) = bincode::serialize(&ServerMessage::Turn(msg)) {
                let _ =
                    tokio::time::timeout(Duration::from_millis(500), direct_tx.send(json)).await;
            }
        }
        eprintln!("[bincode] Ready lobby={} player={} registered fd={}", lobby.id, pid, fd);
    }

    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(msg)) => {
                        if msg.is_binary() {
                            if let Ok(cmsg) = bincode::deserialize::<ClientMessage>(&msg.into_data()) {
                                match cmsg {
                                    ClientMessage::Ready { lobby_id: l_id, player_id } => {
                                        // Re-ready (reconnect) mid-session.
                                        if let (Some(lobby), Some(pid)) = (&my_lobby, &my_player_id) {
                                            if lobby.id == l_id && *pid == player_id
                                                && lobby.valid_players.contains_key(&player_id)
                                            {
                                                lobby.clients.lock().await.insert(player_id, ClientChannel { sender: direct_tx.clone(), missed_ticks: 0 });
                                                eprintln!("[bincode] Ready lobby={} player={} fd={}", l_id, player_id, fd);
                                                let _ = lobby.ev_tx.try_send(RelayEvent::Gameplay {
                                                    player_id,
                                                    intent: GameplayIntent::MarkDisconnected { is_disconnected: false },
                                                });
                                            }
                                        }
                                    }
                                    ClientMessage::Gameplay { intent } => {
                                        if let (Some(lobby), Some(pid)) = (&my_lobby, &my_player_id) {
                                            let _ = lobby.ev_tx.try_send(RelayEvent::Gameplay { player_id: *pid, intent });
                                        }
                                    }
                                    ClientMessage::Leave {} => {
                                        if let (Some(lobby), Some(pid)) = (&my_lobby, &my_player_id) {
                                            let _ = lobby.ev_tx.try_send(RelayEvent::Leave { player_id: *pid });
                                        }
                                    }
                                    ClientMessage::RematchRequest { .. } => {
                                        if let (Some(lobby), Some(pid)) = (&my_lobby, &my_player_id) {
                                            let _ = lobby.ev_tx.try_send(RelayEvent::RematchRequest { player_id: *pid });
                                        }
                                    }
                                    ClientMessage::Ping { client_time } => {
                                        let pong = ServerMessage::Pong { client_time };
                                        if let Ok(json) = bincode::serialize(&pong) {
                                            let _ = direct_tx.try_send(json);
                                        }
                                    }
                                    ClientMessage::SubmitStats { kills, deaths, assists, players_defeated, empires_defeated, tribes_defeated } => {
                                        if let (Some(lobby), Some(pid)) = (&my_lobby, &my_player_id) {
                                            if let Some(acc) = lobby.tracker.lock().unwrap().player_accounts.get(pid).cloned() {
                                                let guard = redis_shared();
                                                let mut guard = guard.lock().unwrap();
                                                if let Some(ref mut con) = *guard {
                                                    log_player_stats(con, lobby.id, &acc, MatchPlayerStats {
                                                        kills, deaths, assists, players_defeated, empires_defeated, tribes_defeated,
                                                    });
                                                    info!("Logged stats for player {pid} (account {acc}): K/D/A {kills}/{deaths}/{assists}");
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

    if let (Some(lobby), Some(pid)) = (my_lobby, my_player_id) {
        let _ = lobby.ev_tx.try_send(RelayEvent::Leave { player_id: pid });
    }
    eprintln!("[ws] done fd={}", fd);
}

/// Resolve a `Ready` against the registry and register the client channel.
/// Returns `Some(Role::RelayPlayer)` on success (mirrors the sow-relay Ready
/// handler: valid lobby + valid player + history replay).
async fn try_ready_register(
    registry: &Registry,
    lobby_id: u64,
    player_id: u16,
    direct_tx: &mpsc::Sender<Vec<u8>>,
) -> Option<Role> {
    let lobby = {
        let reg = registry.read().await;
        reg.get(&lobby_id).cloned()
    };
    let lobby = match lobby {
        Some(l) => l,
        None => {
            eprintln!("[bincode] invalid Ready lobby={} player={} (no such lobby)", lobby_id, player_id);
            return None;
        }
    };
    if !lobby.valid_players.contains_key(&player_id) {
        eprintln!("[bincode] invalid Ready lobby={} player={} (not a valid player)", lobby_id, player_id);
        return None;
    }

    lobby
        .clients
        .lock()
        .await
        .insert(player_id, ClientChannel { sender: direct_tx.clone(), missed_ticks: 0 });
    eprintln!(
        "[bincode] Ready lobby={} player={} registered",
        lobby_id, player_id
    );

    let _ = lobby.ev_tx.try_send(RelayEvent::Gameplay {
        player_id,
        intent: GameplayIntent::MarkDisconnected { is_disconnected: false },
    });

    {
        let hist = lobby.match_history.lock().await;
        for past_turn in hist.iter() {
            let msg = ServerTurnMessage {
                turn: past_turn.clone(),
            };
            if let Ok(json) = bincode::serialize(&ServerMessage::Turn(msg)) {
                let _ =
                    tokio::time::timeout(Duration::from_millis(500), direct_tx.send(json)).await;
            }
        }
    }

    Some(Role::RelayPlayer { lobby, player_id })
}

/// Orchestrator connection: periodic LobbiesBroadcast for the backfill daemon.
async fn orchestrator_task(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<bridge::Conn>,
        Message,
    >,
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<bridge::Conn>,
    >,
    stub: &Option<Arc<StubState>>,
) {
    let Some(stub_state) = stub else { return };
    let mut ticker = interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let info = stub_state.lobby_info().await;
                let msg = ServerMessage::LobbiesBroadcast(ServerLobbiesBroadcastMessage {
                    lobbies: vec![info],
                });
                if let Ok(json) = bincode::serialize(&msg) {
                    match tokio::time::timeout(Duration::from_secs(1), write.send(Message::Binary(json))).await {
                        Ok(Ok(())) => {}
                        _ => break,
                    }
                }
            }
            msg = read.next() => {
                match msg {
                    Some(Ok(_)) => {}
                    _ => break,
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(15)) => { break; }
        }
    }
    eprintln!("[ws] orchestrator done");
}
