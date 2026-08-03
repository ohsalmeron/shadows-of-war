//! sow-relay — monolithic game relay over the F-Stack DPDK bridge.
//!
//! One process, one game port (default :80) on the data NIC. Lobbies are
//! registered by the orchestrator (sow-server) via mgmt HTTP on 127.0.0.1:8080:
//!
//!     POST /internal/lobby/register   {lobby_id, tick_number, tick_rate_ms,
//!                                      active_empty_secs, players, config?}
//!     GET  /internal/lobbies          active lobby roster (ops/validation)
//!
//! Connections arrive via RSS on the game port. The first frame decides the
//! role: `ClientMessage::Ready` → relay player routed to its registered lobby;
//! silence for ORCHESTRATOR_GRACE_SECS → orchestrator WS (LobbiesBroadcast
//! feed for the backfill daemon / ops tooling).
//!
//! The per-lobby game loop (ticks, intents, missed-tick drop, MatchTracker,
//! finalize upload) is the sow-relay logic, now keyed by registry lookup.

use fstack_bridge::bridge::{self, Ev};
use fstack_bridge::ffi::{ev_set, kevent, EV_ADD, EVFILT_READ};
use futures_util::{SinkExt, StreamExt};
use libc::{
    c_int, c_void, sockaddr_in, socklen_t, AF_INET, FIONBIO, INADDR_ANY, SOCK_STREAM, SOL_SOCKET,
    SO_REUSEADDR,
};
use log::{error, info, warn};
use redis::Commands;
use sow_core::game_config::{BotDifficulty, GameConfig};
use sow_core::player::Leader;
use sow_core::protocol::{
    ClientMessage, GameplayIntent, LobbyInfo, LobbyKind, LobbyPlayerSyncState,
    ServerLobbiesBroadcastMessage, ServerLobbyClosedMessage, ServerMessage, ServerTurnMessage,
    StampedIntent, Turn,
};
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::mem;
use std::ptr;
use std::sync::{Arc, OnceLock};
use tokio::net::TcpListener;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio::time::{interval, Duration};
use tokio_tungstenite::tungstenite::Message;

fn game_port() -> u16 {
    std::env::var("SOW_RELAY_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(80)
}

fn mgmt_port() -> u16 {
    std::env::var("SOW_RELAY_MGMT_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080)
}

/// Advertised relay address (data PIP) written to `sow:relay:{lobby_id}`.
fn relay_addr() -> String {
    std::env::var("SOW_RELAY_ADDR").unwrap_or_else(|_| "127.0.0.1:80".to_string())
}

/// Seconds without any frame before a connection is classified as the
/// orchestrator WS (backfill daemon sends nothing, players Ready within ~2s).
const ORCHESTRATOR_GRACE_SECS: u64 = 3;

/// Max consecutive missed ticks before dropping a slow client.
/// At 10 ticks/s, 40 = 4 seconds of silence — fast enough that a zombie
/// connection (backfill saturated, not draining its socket) cannot pin
/// megabytes of turns in its per-client channel and OOM the monolithic relay.
const MAX_MISSED_TICKS: u32 = 40;

/// Per-connection channel capacity: 256 messages bounds worst-case zombie
/// memory to ~256 × turn_size (~1-4MB) instead of 4096 × turn_size (~40MB).
const PER_CLIENT_CHANNEL: usize = 256;

/// Per-connection bridge RX capacity (inbound DPDK mbuf guards). Bounded so a
/// stalled ws_task (peer socket saturated) drops guards instead of pinning
/// unbounded mbuf memory; guards are recycled immediately on drop.
const BRIDGE_RX_CAP: usize = 256;

/// Event channel capacity.
const EVENT_CHANNEL: usize = 1024;

/// Liveness: relay-initiated WS ping period. Browsers auto-Pong at the WS
/// layer; backfill/native clients Pong explicitly. A dead peer never does.
const WS_PING_SECS: u64 = 15;

/// Liveness: reap a player connection after this many seconds without a
/// single received frame (data, Ping, or Pong). A SIGKILLed peer whose FIN
/// was lost under DPDK load sends nothing, so it is reaped here instead of
/// lingering in the lobby forever.
const RX_DEADLINE_SECS: u64 = 45;

// ---- relay event plumbing ---------------------------------------------------

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

// ---- redis helpers ----------------------------------------------------------

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
        let history = {
            let mut guard = match_history.lock().await;
            let hist = guard.clone();
            guard.clear();
            guard.shrink_to_fit();
            hist
        };
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

            // Try local Valkey dead-letter fallback
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
                // Extreme backup: save directly to host disk so DevOps can recover it easily
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
}

type Registry = Arc<RwLock<HashMap<u64, Arc<LobbyState>>>>;

// ---- main (bridge boot — same shape as the fstack-bridge examples) ----------

fn main() {
    let prog_args: Vec<CString> = std::env::args()
        .filter(|a| !a.starts_with("--fstack-"))
        .map(|a| CString::new(a).unwrap())
        .collect();
    env_logger::init();

    let gport = game_port();
    let mport = mgmt_port();

    unsafe {
        if let Err(code) = fstack_bridge::init(&prog_args, &[]) {
            error!("[sow-relay] init failed (code={})", code);
            std::process::exit(1);
        }

        let (mut pid, mut qid, mut nbq, mut reta) = (0u16, 0u16, 0u16, 0u16);
        if fstack_bridge::ff_rss_self_queue_info(&mut pid, &mut qid, &mut nbq, &mut reta) == 0 {
            info!(
                "[BOOT] proc_id={} queue_id={} nb_queues={} reta_size={}",
                pid, qid, nbq, reta
            );
        }

        bridge::setup();
        bridge::KQ = fstack_bridge::ff_kqueue();
        if bridge::KQ < 0 {
            error!("[sow-relay] ff_kqueue failed");
            std::process::exit(1);
        }

        let lfd = fstack_bridge::ff_socket(AF_INET, SOCK_STREAM, 0);
        if lfd < 0 {
            error!("[sow-relay] ff_socket failed");
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
        addr.sin_port = gport.to_be();
        addr.sin_addr.s_addr = INADDR_ANY;
        if fstack_bridge::ff_bind(lfd, &addr, mem::size_of::<sockaddr_in>() as socklen_t) < 0 {
            error!("[sow-relay] ff_bind :{} failed", gport);
            std::process::exit(1);
        }
        if fstack_bridge::ff_listen(lfd, 512) < 0 {
            error!("[sow-relay] ff_listen failed");
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
                if let Err(e) = con.sadd::<_, _, ()>("sow:ports", gport) {
                    error!("[REDIS] SADD sow:ports {gport} FAILED: {e}");
                }
                info!("Registered port {} in Redis", gport);
            }
        }

        let registry: Registry = Arc::new(RwLock::new(HashMap::new()));

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.spawn(mgmt_http(registry.clone()));
        rt.spawn(bridge_worker(registry.clone()));

        info!(
            "[BOOT] listening on :{}, mgmt :{}, entering ff_run",
            gport, mport
        );
        fstack_bridge::ff_run(bridge::driver_cb, ptr::null_mut());
    }
}

// ---- mgmt HTTP (kernel — never touches the bridge) ----------------
//
// Bind address is SOW_MGMT_LISTEN (default 127.0.0.1). Production sets it to
// 0.0.0.0 so the orchestrator (other host) can register lobbies; access is
// then locked down by the cloud NSG to the orchestrator's IP only.

fn mgmt_listen() -> String {
    std::env::var("SOW_MGMT_LISTEN").unwrap_or_else(|_| "127.0.0.1".to_string())
}

async fn mgmt_http(registry: Registry) {
    let mport = mgmt_port();
    let listen = mgmt_listen();
    let listener = match TcpListener::bind((listen.as_str(), mport)).await {
        Ok(l) => l,
        Err(e) => {
            error!("[mgmt] bind {}:{} failed: {}", listen, mport, e);
            return;
        }
    };
    info!("[mgmt] http listening on {}:{}", listen, mport);

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
        ("POST", "/internal/lobby/close") => {
            let lobby_id: u64 = serde_json::from_slice::<serde_json::Value>(body)
                .ok()
                .and_then(|v| v.get("lobby_id").and_then(|x| x.as_u64()))
                .unwrap_or(0);
            if lobby_id == 0 {
                return (
                    "400 Bad Request",
                    serde_json::json!({ "error": "lobby_id required" }),
                );
            }
            let removed = registry.write().await.remove(&lobby_id).is_some();
            info!(
                "[relay] lobby {} closed via mgmt API (removed={})",
                lobby_id, removed
            );
            ("200 OK", serde_json::json!({ "ok": true, "removed": removed }))
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
    });

    info!(
        "[registry] lobby {} registered (players={} tick_rate_ms={} tracked={})",
        state.id,
        state.valid_players.len(),
        body.tick_rate_ms,
        tracked
    );

    registry.write().await.insert(state.id, state.clone());

    // Per-lobby Redis registration: sow:relay:{lobby_id} -> relay addr (TTL 60,
    // refreshed by the tick loop heartbeat every 10s).
    {
        let redis_con = redis_shared();
        let key = format!("sow:relay:{}", state.id);
        let val = relay_addr();
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

// ---- per-lobby tick loop -----------------------------------------------------

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
    // Lifecycle: a lobby is removed from the registry and its match_history
    // dropped when the match ends (GameOver / all clients leave) OR when it
    // outlives the maximum match duration. Backfill bots play indefinitely, so
    // without this ceiling a filled lobby keeps pushing to match_history forever
    // and the monolithic relay OOMs (5.5GB in ~4min under 50k bots).
    let match_started = std::time::Instant::now();
    let max_match_secs: u64 = std::env::var("SOW_RELAY_MATCH_MAX_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1800);

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let mut clients = state.clients.lock().await;
                let humans = clients.len();

                if humans == 0 {
                    active_empty_secs -= 0.05;
                    if active_empty_secs <= 0.0 {
                        info!("[relay] lobby {} empty for too long, GC", state.id);
                        registry.write().await.remove(&state.id);
                        break;
                    }
                    // No clients: skip turn serialization/broadcast entirely.
                    continue;
                } else {
                    active_empty_secs = 30.0;
                }

                // Lifecycle: enforce the maximum match duration. Broadcast
                // LobbyClosed so clients exit cleanly, finalize the replay to
                // the database, then drop the lobby from the registry so its
                // match_history Vec is freed (monolithic relay must not hold
                // unbounded per-lobby history).
                let is_finalized = state.tracker.lock().unwrap().finalized;
                let is_timed_out = match_started.elapsed().as_secs() >= max_match_secs;
                if is_finalized || is_timed_out {
                    let reason = if is_finalized {
                        "Match finalized".to_string()
                    } else {
                        format!("Match duration limit reached ({}s)", max_match_secs)
                    };
                    info!("[relay] lobby {} ending: {}, finalizing & cleaning up", state.id, reason);
                    if let Ok(json) = bincode::serialize(
                        &ServerMessage::LobbyClosed(ServerLobbyClosedMessage {
                            lobby_id: state.id,
                            reason,
                            rematch_lobby_id: None,
                        }),
                    ) {
                        for (_, client) in clients.iter() {
                            let _ = client.sender.try_send(json.clone());
                        }
                    }
                    drop(clients);
                    {
                        let tracker = state.tracker.lock().unwrap();
                        if tracker.tracked && !tracker.finalized {
                            trigger_match_finalize(
                                state.id,
                                tracker.lobby_json.clone(),
                                state.match_history.clone(),
                            );
                        }
                    }
                    state.match_history.lock().await.clear();
                    registry.write().await.remove(&state.id);
                    break;
                }

                let intents = std::mem::take(&mut pending_intents);
                let turn = Turn {
                    turn_number: tick_number,
                    intents,
                };
                tick_number += 1;
                total_ticks += 1;
                total_intents += turn.intents.len() as u64;

                {
                    let mut history = state.match_history.lock().await;
                    history.push(turn.clone());
                    if history.len() > 1000 {
                        let drain_amount = history.len() - 500;
                        history.drain(0..drain_amount);
                    }
                }

                let msg = ServerTurnMessage { turn };
                let json = bincode::serialize(&ServerMessage::Turn(msg))
                    .expect("serialize ServerTurnMessage");

                let mut dropped_players = Vec::new();
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
                                dropped_players.push(*player_id);
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
                            dropped_players.push(*player_id);
                            false
                        }
                    }
                });

                if !dropped_players.is_empty() {
                    let mut tracker = state.tracker.lock().unwrap();
                    for pid in dropped_players {
                        tracker.record_exit(pid);
                    }
                }

                if last_status.elapsed().as_secs() >= 10 {
                    info!(
                        "STATUS|{}|{}|{}|{}|{}|{}",
                        state.id, std::process::id(), game_port(), humans, total_ticks, total_intents
                    );
                    last_status = std::time::Instant::now();
                    // Heartbeat: refresh per-lobby Redis TTL.
                    let redis_con = redis_shared();
                    let key = format!("sow:relay:{}", state.id);
                    let val = relay_addr();
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
    info!("[relay] lobby {} tick task ended", state.id);
}

// ---- bridge worker (dispatch — same shape as the fstack-bridge examples) -----

async fn bridge_worker(registry: Registry) {
    let rx = bridge::rx_ring();
    let notify = bridge::notify();
    let mut conns: HashMap<c_int, mpsc::Sender<bridge::ZcRxGuard>> = HashMap::new();

    loop {
        while let Some(ev) = rx.pop() {
            match ev {
                Ev::Accept { fd, generation } => {
                    let (tx, rx_conn) = mpsc::channel(BRIDGE_RX_CAP);
                    conns.insert(fd, tx);
                    tokio::spawn(ws_task(fd, generation, rx_conn, registry.clone()));
                }
                Ev::Data { fd, guard } => match conns.get(&fd) {
                    Some(tx) => {
                        // Bounded per-connection RX: if the ws_task is stalled
                        // writing to a saturated peer socket, drop the guard so
                        // the DPDK mbuf is recycled immediately instead of
                        // pinning unbounded per-connection memory (the zombie is
                        // reaped by rx-silence shortly anyway).
                        let _ = tx.try_send(guard);
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
    RelayPlayer {
        lobby: Arc<LobbyState>,
        player_id: u16,
    },
}

async fn ws_task(
    fd: c_int,
    generation: u64,
    rx: mpsc::Receiver<bridge::ZcRxGuard>,
    registry: Registry,
) {
    let conn = bridge::Conn::new(fd, generation, rx);
    let ws = match tokio_tungstenite::accept_async(conn).await {
        Ok(ws) => ws,
        Err(e) => {
            warn!("[ws] handshake fail fd={} err={}", fd, e);
            return;
        }
    };

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
                Ok(ClientMessage::Ready { lobby_id, player_id }) => {
                    role = try_ready_register(&registry, lobby_id, player_id, &direct_tx).await;
                    info!(
                        "[relay] first-frame Ready lobby={} player={} registered={} fd={}",
                        lobby_id,
                        player_id,
                        role.is_some(),
                        fd
                    );
                }
                Ok(ClientMessage::Join { .. }) => {
                    // Real clients Join the orchestrator (sow-server), never the
                    // relay. Ignore stale/spike-era joins.
                    warn!("[relay] Join ignored (orchestrator handles joins) fd={}", fd);
                    role = None;
                }
                Ok(_) => {
                    warn!("[relay] ignored first frame fd={}", fd);
                    role = None;
                }
                Err(e) => {
                    warn!("[relay] deserialize err {} fd={}", e, fd);
                    role = None;
                }
            }
        }
        Ok(Some(Ok(_))) => role = None,
        Ok(Some(Err(e))) => {
            warn!("[ws] recv err fd={} err={}", fd, e);
            return;
        }
        Ok(None) => {
            return;
        }
        Err(_) => {
            // Nothing in the grace window → orchestrator (backfill daemon) WS.
            orchestrator_task(&mut write, &mut read, &registry).await;
            return;
        }
    }

    // Relay player: the per-connection loop, routed to its lobby.
    let my_lobby: Option<Arc<LobbyState>> = match &role {
        Some(Role::RelayPlayer { lobby, .. }) => Some(lobby.clone()),
        _ => None,
    };
    let my_player_id: Option<u16> = match &role {
        Some(Role::RelayPlayer { player_id, .. }) => Some(*player_id),
        _ => None,
    };

    if let (Some(lobby), Some(pid)) = (&my_lobby, &my_player_id) {
        info!("[relay] Ready lobby={} player={} registered fd={}", lobby.id, pid, fd);
    }

    let mut last_rx = std::time::Instant::now();
    let mut last_ping = std::time::Instant::now();
    let mut ping_interval = interval(Duration::from_secs(WS_PING_SECS));
    ping_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            msg = read.next() => {
                match msg {
                    Some(Ok(msg)) => {
                        last_rx = std::time::Instant::now();
                        if let Message::Ping(payload) = &msg {
                            let _ = tokio::time::timeout(
                                Duration::from_millis(200),
                                write.send(Message::Pong(payload.clone())),
                            )
                            .await;
                            continue;
                        }
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
                                                info!("[relay] Ready lobby={} player={} fd={}", l_id, player_id, fd);
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
                // Hot path (ticks flowing). select! may starve the interval
                // branch here, so ping + reap checks live on this branch too.
                if last_ping.elapsed() >= Duration::from_secs(WS_PING_SECS) {
                    last_ping = std::time::Instant::now();
                    if tokio::time::timeout(
                        Duration::from_millis(200),
                        write.send(Message::Ping(Vec::new())),
                    )
                    .await
                    .is_err()
                    {
                        break;
                    }
                }
                if last_rx.elapsed() > Duration::from_secs(RX_DEADLINE_SECS) {
                    warn!(
                        "[ws] fd={} rx silence {}s (ticks flowing) — reaping stale connection",
                        fd,
                        last_rx.elapsed().as_secs()
                    );
                    break;
                }
                match tokio::time::timeout(Duration::from_millis(200), write.send(Message::Binary(direct_data))).await {
                    Ok(Ok(())) => {}
                    _ => break,
                }
            }
            _ = ping_interval.tick() => {
                // Idle path (no ticks): keepalive ping + reap check.
                // Lifecycle: if the lobby was finalized/GC'd (removed from the
                // registry) or this client was dropped by the tick task, end
                // this connection so the Arc<LobbyState> (and its match_history)
                // is released instead of being pinned alive forever.
                if let (Some(lobby), Some(pid)) = (&my_lobby, &my_player_id) {
                    let still_registered = registry.read().await.contains_key(&lobby.id);
                    if !still_registered {
                        info!("[ws] fd={} lobby {} no longer registered — closing connection", fd, lobby.id);
                        break;
                    }
                    if !lobby.clients.lock().await.contains_key(pid) {
                        info!("[ws] fd={} player {} dropped from lobby {} — closing connection", fd, pid, lobby.id);
                        break;
                    }
                }
                last_ping = std::time::Instant::now();
                if last_rx.elapsed() > Duration::from_secs(RX_DEADLINE_SECS) {
                    warn!(
                        "[ws] fd={} rx silence {}s — reaping stale connection",
                        fd,
                        last_rx.elapsed().as_secs()
                    );
                    break;
                }
                if tokio::time::timeout(
                    Duration::from_secs(1),
                    write.send(Message::Ping(Vec::new())),
                )
                .await
                .is_err()
                {
                    break;
                }
            }
            _ = tokio::time::sleep(Duration::from_secs(15)) => {
                if last_rx.elapsed() > Duration::from_secs(RX_DEADLINE_SECS) {
                    break;
                }
            }
        }
    }

    if let (Some(lobby), Some(pid)) = (my_lobby, my_player_id) {
        let _ = lobby.ev_tx.try_send(RelayEvent::Leave { player_id: pid });
    }
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
            warn!("[relay] invalid Ready lobby={} player={} (no such lobby)", lobby_id, player_id);
            return None;
        }
    };
    if !lobby.valid_players.contains_key(&player_id) {
        warn!("[relay] invalid Ready lobby={} player={} (not a valid player)", lobby_id, player_id);
        return None;
    }

    lobby
        .clients
        .lock()
        .await
        .insert(player_id, ClientChannel { sender: direct_tx.clone(), missed_ticks: 0 });

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
                let _ = tokio::time::timeout(Duration::from_millis(500), direct_tx.send(json)).await;
            }
        }
    }

    Some(Role::RelayPlayer { lobby, player_id })
}

/// Orchestrator connection: periodic LobbiesBroadcast of registered lobbies,
/// so the backfill daemon keeps spawning until slots fill.
async fn orchestrator_task(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<bridge::Conn>,
        Message,
    >,
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<bridge::Conn>,
    >,
    registry: &Registry,
) {
    let mut ticker = interval(Duration::from_secs(1));
    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let lobbies: Vec<LobbyInfo> = {
                    let reg = registry.read().await;
                    reg.values().map(lobby_info).collect()
                };
                let msg = ServerMessage::LobbiesBroadcast(ServerLobbiesBroadcastMessage {
                    lobbies,
                });
                if let Ok(json) = bincode::serialize(&msg) {
                    match tokio::time::timeout(Duration::from_millis(200), write.send(Message::Binary(json))).await {
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
    info!("[ws] orchestrator done");
}

/// Registry lobby as the broadcast LobbyInfo (roster = valid players).
fn lobby_info(state: &Arc<LobbyState>) -> LobbyInfo {
    let mut players: Vec<LobbyPlayerSyncState> = state
        .valid_players
        .iter()
        .map(|(pid, name)| LobbyPlayerSyncState {
            name: name.clone(),
            is_ready: false,
            download_progress: 100,
            leader: Leader::Cleopatra,
            player_id: *pid,
            team: None,
        })
        .collect();
    players.sort_by_key(|p| p.player_id);
    LobbyInfo {
        id: state.id,
        num_players: players.len() as u32,
        max_players: players.len() as u32,
        is_counting_down: false,
        timer_secs: 0.0,
        map_name: "world".to_string(),
        game_mode: "FFA".to_string(),
        players,
        has_password: false,
        host_name: String::new(),
        bot_count: 0,
        nation_count: 0,
        bot_difficulty: BotDifficulty::Vanilla,
        kind: LobbyKind::Matchmaking,
    }
}
