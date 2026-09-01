mod bot_fill;
mod lobby;
mod map_catalog;
mod map_playlist;

use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use lobby::{
    JoinPlayerOpts, ServerLobby, build_lobby_broadcast, force_start, is_host_teardown, join_player, kick_player,
    leave_player, lobby_to_info, master_tick, notify_lobby_closed, set_player_team,
    sync_host_lobby_to_members,
};
use sow_core::game_config::GameConfig;
use sow_core::protocol::{
    PlayerInfo, ServerJoinAckMessage, ServerJoinFailedMessage, ServerLobbiesBroadcastMessage,
};
use std::collections::HashSet;
use std::env;
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use rand::{RngCore, rngs::OsRng};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::sync::{Mutex, Semaphore, broadcast, mpsc};
use tokio_tungstenite::tungstenite::protocol::Message;

const DEFAULT_RELAY_WORKER_COUNT: usize = 4;
const RELAY_PORT_MIN: u16 = 25592;
const RELAY_PORT_MAX: u16 = 26500;
const RELAY_TICKET_TTL_SECS: u64 = 900;
const RELAY_HANDOFF_SEND_TIMEOUT_SECS: u64 = 10;
const DEFAULT_MAX_CONNECTIONS: usize = 32_768;
const HANDSHAKE_TIMEOUT_SECS: u64 = 10;
type HmacSha256 = Hmac<Sha256>;

static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(1);
static REJECTED_CONNECTIONS: AtomicU64 = AtomicU64::new(0);

fn next_session_id() -> u64 {
    NEXT_SESSION_ID.fetch_add(1, Ordering::Relaxed)
}

fn max_server_connections() -> Result<usize, String> {
    let value = std::env::var("SOW_SERVER_MAX_CONNECTIONS")
        .unwrap_or_else(|_| DEFAULT_MAX_CONNECTIONS.to_string());
    let parsed = value
        .parse::<usize>()
        .map_err(|_| "SOW_SERVER_MAX_CONNECTIONS must be a positive integer".to_string())?;
    if parsed == 0 {
        return Err("SOW_SERVER_MAX_CONNECTIONS must be a positive integer".to_string());
    }
    Ok(parsed)
}

#[derive(Clone, Debug)]
struct RelayWorker {
    id: usize,
    host: String,
    mgmt_url: String,
}

fn relay_worker_count() -> usize {
    std::env::var("SOW_RELAY_WORKER_COUNT")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|count| (1..=64).contains(count))
        .unwrap_or(DEFAULT_RELAY_WORKER_COUNT)
}

fn relay_mgmt_scheme() -> String {
    match std::env::var("SOW_RELAY_MGMT_SCHEME")
        .unwrap_or_else(|_| "https".to_string())
        .to_ascii_lowercase()
        .as_str()
    {
        "http" => {
            log::error!("SOW_RELAY_MGMT_SCHEME=http is forbidden; using https");
            "https".to_string()
        }
        "https" => "https".to_string(),
        other => {
            log::warn!("Unsupported SOW_RELAY_MGMT_SCHEME={other}; using https");
            "https".to_string()
        }
    }
}

fn new_relay_ticket() -> (String, String) {
    let mut raw = [0u8; 32];
    OsRng.fill_bytes(&mut raw);
    let ticket = hex::encode(raw);
    let digest = hex::encode(Sha256::digest(ticket.as_bytes()));
    (ticket, digest)
}

fn relay_mgmt_headers(
    secret: &str,
    method: &str,
    path: &str,
    body: &[u8],
) -> Result<(String, String, String), String> {
    if secret.trim().is_empty() {
        return Err("SOW_RELAY_CONTROL_SECRET missing".to_string());
    }
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| format!("system clock before UNIX epoch: {e}"))?
        .as_secs()
        .to_string();
    let mut nonce_bytes = [0u8; 16];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = hex::encode(nonce_bytes);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|_| "invalid relay control secret".to_string())?;
    mac.update(method.as_bytes());
    mac.update(b"\n");
    mac.update(path.as_bytes());
    mac.update(b"\n");
    mac.update(timestamp.as_bytes());
    mac.update(b"\n");
    mac.update(nonce.as_bytes());
    mac.update(b"\n");
    mac.update(body);
    Ok((timestamp, nonce, hex::encode(mac.finalize().into_bytes())))
}

/// Keep the TLS hostname from the management URL for certificate validation,
/// while allowing the production host to pin DNS resolution to the relay's
/// management NIC. The game hostname and management route intentionally use
/// different network interfaces on the two-NIC relay VM.
fn relay_mgmt_client(mgmt_url: &str) -> Result<reqwest::Client, String> {
    let mut builder = reqwest::Client::builder();
    let parsed = reqwest::Url::parse(mgmt_url)
        .map_err(|e| format!("invalid relay management URL {mgmt_url}: {e}"))?;
    if parsed.scheme() != "https" {
        return Err(format!("relay management URL must use https: {mgmt_url}"));
    }
    if let Ok(raw_ip) = std::env::var("SOW_RELAY_MGMT_RESOLVE_IP") {
        let ip = raw_ip
            .parse::<IpAddr>()
            .map_err(|e| format!("invalid SOW_RELAY_MGMT_RESOLVE_IP={raw_ip}: {e}"))?;
        let host = parsed
            .host_str()
            .ok_or_else(|| format!("relay management URL has no host: {mgmt_url}"))?;
        if host.parse::<IpAddr>().is_err() {
            let port = parsed.port().unwrap_or(443);
            builder = builder.resolve(host, SocketAddr::new(ip, port));
        }
    }
    builder
        .build()
        .map_err(|e| format!("build relay management client: {e}"))
}

fn validate_runtime_security() -> Result<(), String> {
    let scheme = std::env::var("SOW_RELAY_MGMT_SCHEME")
        .unwrap_or_else(|_| "https".to_string())
        .to_ascii_lowercase();
    if scheme != "https" {
        return Err("SOW_RELAY_MGMT_SCHEME must be https".to_string());
    }
    let tickets = std::env::var("SOW_RELAY_TICKETS_REQUIRED")
        .unwrap_or_else(|_| "1".to_string());
    if tickets != "1" {
        return Err("SOW_RELAY_TICKETS_REQUIRED must be 1".to_string());
    }
    if let Ok(url) = std::env::var("SOW_RELAY_MGMT_URL") {
        let parsed = reqwest::Url::parse(&url)
            .map_err(|e| format!("invalid SOW_RELAY_MGMT_URL={url}: {e}"))?;
        if parsed.scheme() != "https" {
            return Err("SOW_RELAY_MGMT_URL must use https".to_string());
        }
    }
    Ok(())
}

/// Parse one worker as `game_host:mgmt_host:mgmt_port`.
///
/// `SOW_RELAY_WORKERS` is a comma-separated catalog in this format. Hosts are
/// intentionally plain DNS/IP tokens here; IPv6 literals should be supplied
/// through a front door/DNS name until the client URL contract is upgraded.
fn parse_relay_worker(spec: &str, id: usize) -> Option<RelayWorker> {
    let fields: Vec<_> = spec.trim().split(':').collect();
    let (host, mgmt_host, mgmt_port) = match fields.as_slice() {
        [host, mgmt_host, mgmt_port] => (*host, *mgmt_host, *mgmt_port),
        _ => return None,
    };
    let mgmt_port = mgmt_port.parse::<u16>().ok()?;
    if host.is_empty()
        || host.contains('/')
        || host.contains('[')
        || host.contains(']')
        || mgmt_host.is_empty()
        || mgmt_host.contains('/')
        || mgmt_host.contains('[')
        || mgmt_host.contains(']')
    {
        return None;
    }
    let scheme = relay_mgmt_scheme();
    Some(RelayWorker {
        id,
        host: host.to_string(),
        mgmt_url: format!("{scheme}://{mgmt_host}:{mgmt_port}"),
    })
}

fn relay_workers() -> Result<Vec<RelayWorker>, String> {
    let specs = std::env::var("SOW_RELAY_WORKERS")
        .map_err(|_| "SOW_RELAY_WORKERS must configure every relay worker".to_string())?;
    let workers: Vec<_> = specs
        .split(',')
        .enumerate()
        .map(|(id, spec)| {
            parse_relay_worker(spec, id).ok_or_else(|| {
                format!("SOW_RELAY_WORKERS contains an invalid worker entry at index {id}")
            })
        })
        .collect::<Result<_, _>>()?;
    if workers.is_empty() {
        return Err("SOW_RELAY_WORKERS must contain at least one worker".to_string());
    }
    Ok(workers)
}

struct RelayPortAllocator {
    next: u32,
    used: HashSet<u16>,
}

impl RelayPortAllocator {
    fn new() -> Self {
        Self {
            next: RELAY_PORT_MIN as u32,
            used: HashSet::new(),
        }
    }

    fn allocate(&mut self) -> Option<u16> {
        let capacity = (RELAY_PORT_MAX as u32 - RELAY_PORT_MIN as u32) + 1;
        for _ in 0..capacity {
            if self.next > RELAY_PORT_MAX as u32 {
                self.next = RELAY_PORT_MIN as u32;
            }
            let candidate = self.next as u16;
            self.next += 1;
            if self.used.insert(candidate) {
                return Some(candidate);
            }
        }
        None
    }

    fn release(&mut self, port: u16) {
        self.used.remove(&port);
    }
}

/// Register a lobby with its assigned relay worker over mgmt HTTP. The worker
/// confirms with 200 OK before the server broadcasts Start to the clients.
/// Retries with exponential backoff (same resilience as the old spawn path).
async fn register_relay(rc: &RelayCandidate, worker: &RelayWorker) -> Result<(), String> {
    let url = format!(
        "{}/internal/lobby/register",
        worker.mgmt_url.trim_end_matches('/')
    );

    let active_empty_secs = if rc.active_empty_secs <= 0.0 {
        30.0
    } else {
        rc.active_empty_secs
    };

    let relay_config = serde_json::json!({
        "lobby_id": rc.lobby_id,
        "relay_port": rc.relay_port,
        "ticket_expires_at": rc.ticket_expires_at,
        "tick_number": 0,
        "active_empty_secs": active_empty_secs,
        "players": rc.relay_players_json,
        "tick_rate_ms": rc.tick_rate_ms,
        "config": rc.config,
        "kind": rc.kind,
    });

    let secret = std::env::var("SOW_RELAY_CONTROL_SECRET")
        .map_err(|_| "SOW_RELAY_CONTROL_SECRET missing while registering relay".to_string())?;
    let path = "/internal/lobby/register";
    let body = serde_json::to_vec(&relay_config)
        .map_err(|e| format!("serialize relay registration: {e}"))?;
    let client = relay_mgmt_client(&url)?;
    for attempt in 1..=5 {
        let (timestamp, nonce, signature) = relay_mgmt_headers(&secret, "POST", path, &body)?;
        match client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("X-SOW-Timestamp", timestamp)
            .header("X-SOW-Nonce", nonce)
            .header("X-SOW-Signature", signature)
            .body(body.clone())
            .send()
            .await
        {
            Ok(res) if res.status().is_success() => {
                log::info!(
                    "[RELAY] Lobby {} accepted by relay ({} OK)",
                    rc.lobby_id,
                    url
                );
                return Ok(());
            }
            Ok(res) => {
                log::warn!(
                    "[RELAY] register lobby {}: relay returned HTTP {} (attempt {}/5)",
                    rc.lobby_id,
                    res.status(),
                    attempt
                );
            }
            Err(e) => {
                log::warn!(
                    "[RELAY] register lobby {}: relay unreachable: {e} (attempt {}/5)",
                    rc.lobby_id,
                    attempt
                );
            }
        }
        if attempt < 5 {
            tokio::time::sleep(Duration::from_secs(2u64.pow(attempt as u32))).await;
        }
    }
    Err(format!("relay registration failed for lobby {}", rc.lobby_id))
}

/// Deliver the capability before Start, preserving wire order and refusing to
/// silently continue when the client's bounded handoff queue is unavailable.
async fn send_relay_handoff_frame(
    tx: &mpsc::Sender<Vec<u8>>,
    frame: Vec<u8>,
    lobby_id: u64,
    player_id: u16,
    label: &str,
) -> Result<(), String> {
    tokio::time::timeout(
        Duration::from_secs(RELAY_HANDOFF_SEND_TIMEOUT_SECS),
        tx.send(frame),
    )
    .await
    .map_err(|_| {
        format!(
            "timed out sending {label} for lobby {lobby_id} player {player_id}"
        )
    })?
    .map_err(|_| format!("client channel closed before {label} for lobby {lobby_id} player {player_id}"))
}

/// Register the account-backed match before the relay is exposed to clients.
/// This ordering prevents a fast match from finalizing before Valkey contains
/// its player list, which otherwise loses statistics as `Match not registered`.
/// Resolve a `JoinWithAuth` identity proof and server-authorized leader via
/// sow-data's /internal/verify.
struct VerifiedIdentity {
    account_id: String,
    leader: sow_core::player::Leader,
}

async fn verify_identity(
    auth: &sow_core::protocol::AuthProof,
    requested_leader: sow_core::player::Leader,
) -> Result<VerifiedIdentity, String> {
    let db_base_url = std::env::var("SOW_DB_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:25585".to_string());
    let secret_token = std::env::var("SOW_DB_SECRET")
        .map_err(|_| "SOW_DB_SECRET missing".to_string())?;
    let url = format!("{}/internal/verify", db_base_url.trim_end_matches('/'));
    let body = serde_json::json!({
        "provider": auth.provider.trim(),
        "account_id": auth.account_id.as_deref(),
        "token": auth.token,
        "requested_leader": sow_core::commerce::leader_wire_id(requested_leader),
    });
    let client = reqwest::Client::new();
    let res = client
        .post(&url)
        .header("Authorization", format!("Bearer {secret_token}"))
        .json(&body)
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("verify request failed: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("verify returned HTTP {}", res.status()));
    }
    let value: serde_json::Value = res
        .json()
        .await
        .map_err(|e| format!("verify response unreadable: {e}"))?;
    let account_id = value
        .get("account_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "verify response missing account_id".to_string())?;
    let leader = value
        .get("leader")
        .and_then(|v| v.as_str())
        .and_then(sow_core::commerce::leader_from_id)
        .ok_or_else(|| "verify response missing authorized leader".to_string())?;
    Ok(VerifiedIdentity { account_id, leader })
}

async fn register_match_start(rc: &RelayCandidate) -> Result<(), String> {
    if rc.player_ids.is_empty() {
        return Ok(());
    }
    let db_base_url = std::env::var("SOW_DB_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:25585".to_string());
    let secret_token = std::env::var("SOW_DB_SECRET")
        .map_err(|_| "SOW_DB_SECRET missing while registering match".to_string())?;
    let url = format!("{}/match/start", db_base_url.trim_end_matches('/'));
    let payload = serde_json::json!({
        "match_id": rc.lobby_id.to_string(),
        "player_ids": rc.player_ids,
    });
    let client = reqwest::Client::new();

    for attempt in 1..=5 {
        match client
            .post(&url)
            .header("Authorization", format!("Bearer {secret_token}"))
            .json(&payload)
            .send()
            .await
        {
            Ok(res) if res.status().is_success() => {
                log::info!("[DB] Match {} registered before relay handoff", rc.lobby_id);
                return Ok(());
            }
            Ok(res) => {
                log::warn!(
                    "[DB] match {} registration returned HTTP {} (attempt {attempt}/5)",
                    rc.lobby_id,
                    res.status()
                );
            }
            Err(e) => {
                log::warn!(
                    "[DB] match {} registration failed: {e} (attempt {attempt}/5)",
                    rc.lobby_id
                );
            }
        }
        if attempt < 5 {
            tokio::time::sleep(Duration::from_secs(2u64.pow(attempt as u32))).await;
        }
    }
    Err(format!("database match registration failed for lobby {}", rc.lobby_id))
}

#[derive(serde::Deserialize)]
struct BotPoolSeedResponse {
    account_ids: Vec<String>,
}

/// Resolve (or lazily create) the persistent bot-account pool at boot.
///
/// Builds a deterministic spec of `SOW_BOT_POOL_SIZE` (default 10000)
/// identities where `external_id = "bot_{:05}"` and `display_name` cycles
/// through `bot_fill::names::BOT_NAMES`. Calls
/// `POST {SOW_DB_URL}/internal/bot-pool/seed` with those external_ids — the
/// db get-or-creates each one and returns the stable account_ids in order.
/// The resulting `(account_id, display_name)` pairs are installed as the
/// process-wide `BotPool`.
///
/// MENTAL MODEL (organic): the pool is identities only — it never dictates
/// timing. Entry is a chaotic drip in `bot_fill` (see its module header).
///
/// A lobby server without real Ghost identities is not healthy. Fail startup
/// instead of installing an empty pool and silently serving human-only lobbies.
async fn init_bot_pool() -> Result<(), String> {
    let pool_size: usize = env::var("SOW_BOT_POOL_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10_000);

    if pool_size == 0 {
        return Err("SOW_BOT_POOL_SIZE must be greater than zero".to_string());
    }

    let name_pool = bot_fill::names::BOT_NAMES;
    if name_pool.is_empty() {
        return Err("names::BOT_NAMES is empty — cannot seed pool".to_string());
    }

    let external_ids: Vec<String> = (0..pool_size).map(|i| format!("bot_{:05}", i)).collect();
    let display_names: Vec<String> = (0..pool_size)
        .map(|i| name_pool[i % name_pool.len()].to_string())
        .collect();

    let db_base_url = env::var("SOW_DB_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:25585".to_string());
    let secret = env::var("SOW_DB_SECRET")
        .map_err(|_| "SOW_DB_SECRET missing — cannot seed pool".to_string())?;
    let url = format!("{}/internal/bot-pool/seed", db_base_url.trim_end_matches('/'));
    let body = serde_json::json!({ "external_ids": external_ids });

    let client = reqwest::Client::new();
    for attempt in 1..=5 {
        match client
            .post(&url)
            .header("Authorization", format!("Bearer {secret}"))
            .json(&body)
            .send()
            .await
        {
            Ok(res) if res.status().is_success() => {
                match res.json::<BotPoolSeedResponse>().await {
                    Ok(resp) if resp.account_ids.len() == pool_size => {
                        let entries: Vec<bot_fill::BotPoolEntry> = resp
                            .account_ids
                            .into_iter()
                            .zip(display_names.iter().cloned())
                            .map(|(account_id, display_name)| bot_fill::BotPoolEntry {
                                account_id,
                                display_name,
                            })
                            .collect();
                        log::info!("[BOT_POOL] resolved {} identities", entries.len());
                        bot_fill::BotPool::new(entries).install();
                        return Ok(());
                    }
                    Ok(resp) => log::warn!(
                        "[BOT_POOL] seed returned {} ids, expected {}",
                        resp.account_ids.len(),
                        pool_size
                    ),
                    Err(e) => log::warn!("[BOT_POOL] seed response parse error: {e}"),
                }
            }
            Ok(res) => log::warn!(
                "[BOT_POOL] seed HTTP {} (attempt {attempt}/5)",
                res.status()
            ),
            Err(e) => log::warn!(
                "[BOT_POOL] seed unreachable: {e} (attempt {attempt}/5)"
            ),
        }
        if attempt < 5 {
            tokio::time::sleep(Duration::from_secs(2u64.pow(attempt as u32))).await;
        }
    }

    Err(format!("seed failed after 5 attempts: {url}"))
}

// =============================================================================
// RELAY INTEGRATION — worker-per-queue DPDK relay (no per-lobby processes)
// =============================================================================
//
// Each worker process owns dynamic lobby ports selected by `port % 4` and one
// kernel management port. The server registers the lobby and ONLY broadcasts
// Start{relay_port, relay_host} after the owning worker answers 200 OK.
//
// Dynamic ports are bound by the long-lived worker process through the bridge
// command ring, so no relay subprocess is created for an individual match.



/// All server events, comming from client
enum ServerEvent {
    Join {
        client_tx: mpsc::Sender<Vec<u8>>,
        name: String,
        clan_tag: String,
        civilization: sow_core::player::Civilization,
        leader: sow_core::player::Leader,
        target_lobby_id: Option<u64>,
        host_private: bool,
        build_version: String,
        database_account_id: Option<String>,
        host_config: Option<Box<sow_core::game_config::GameConfig>>,
        password: Option<String>,
        ip: String,
        session_id: u64,
    },

    Leave {
        lobby_id: u64,
        player_id: u16,
    },
    Ready {
        lobby_id: u64,
        player_id: u16,
    },
    MapDownloadProgress {
        lobby_id: u64,
        player_id: u16,
        progress: u8,
    },
    ForceStart {
        lobby_id: u64,
        player_id: u16,
    },
    Kick {
        lobby_id: u64,
        requester_id: u16,
        target_id: u16,
        ban: bool,
    },
    SetTeam {
        lobby_id: u64,
        requester_id: u16,
        target_id: u16,
    },
}

/// Data collected inside the lock for a lobby that needs dynamic relay binding.
/// All blocking I/O (Redis, disk, process) happens *outside* the games lock in a dedicated worker task.
struct RelayCandidate {
    lobby_id: u64,
    relay_port: u16,
    ticket_expires_at: u64,
    worker_index: usize,
    active_empty_secs: f32,
    tick_rate_ms: f32,
    config: GameConfig,
    kind: String,
    seed: u64,
    start_players: Vec<PlayerInfo>,
    relay_players_json: Vec<serde_json::Value>,
    player_tickets: std::collections::HashMap<u16, String>,
    player_ids: Vec<String>,
    players_tx: Vec<(u16, mpsc::Sender<Vec<u8>>)>,
}

// Content-derived build identity (see build.rs). Proves which sources the
// running jail executes; printed once at startup as [SERVER-BOOT].
include!(concat!(env!("OUT_DIR"), "/build_epoch.rs"));

#[tokio::main]
async fn main() {
    env_logger::init();
    log::info!("[SERVER-BOOT] build_epoch={BUILD_EPOCH:#018x}");

    if std::env::var("SOW_DB_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_none()
    {
        log::error!("SOW_DB_SECRET must be set; refusing insecure default");
        return;
    }
    if let Err(e) = validate_runtime_security() {
        log::error!("{e}");
        return;
    }

    let redis_url =
        std::env::var("SOW_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1/".to_string());
    let redis_client = redis::Client::open(redis_url).expect("Failed to connect to Redis");
    // Establish one connection at boot as a hard dependency check.
    let _redis_connection = redis_client
        .get_connection()
        .expect("Failed to get Redis connection");
    log::info!("Redis connected");

    // Resolve (or lazily create) the persistent bot-account pool before any
    // lobby can be promoted. Idempotent — the seed endpoint get-or-creates
    // each account, so a restart reuses the same identities and their
    // accumulated stats.
    if let Err(error) = init_bot_pool().await {
        panic!("[BOT_POOL] startup failed: {error}");
    }

    let mut games: Vec<ServerLobby> = Vec::new();
    let mut next_lobby_id: u64 = 1;

    let maps_root = map_catalog::maps_root();
    map_catalog::init(&maps_root);

    master_tick(&mut games, &mut next_lobby_id);

    let games_state = Arc::new(Mutex::new(games));
    let next_id_state = Arc::new(Mutex::new(next_lobby_id));

    let (global_tx, _rx) = broadcast::channel::<Vec<u8>>(100);
    let (event_tx, mut event_rx) = mpsc::channel::<ServerEvent>(100000);
    let (relay_tx, mut relay_rx) = mpsc::unbounded_channel::<RelayCandidate>();
    let relay_workers = match relay_workers() {
        Ok(workers) => workers,
        Err(error) => {
            log::error!("Relay worker catalog is invalid: {error}");
            return;
        }
    };
    let relay_worker_count = relay_worker_count();
    if relay_workers.len() != relay_worker_count {
        log::error!(
            "Dynamic relay routing requires exactly {} workers; configured {}",
            relay_worker_count,
            relay_workers.len()
        );
        return;
    }
    let relay_ports = Arc::new(Mutex::new(RelayPortAllocator::new()));
    log::info!(
        "Relay worker catalog: {} worker(s) {:?}",
        relay_worker_count,
        relay_workers
    );

    let games_clone = Arc::clone(&games_state);
    let next_id_clone = Arc::clone(&next_id_state);
    let global_tx_clone = global_tx.clone();
    let relay_tx_clone = relay_tx.clone();

    // ── DEDICATED ASYNC RELAY WORKER TASK ──
    // A worker owns many lobbies. The destination port is the ownership key;
    // the client receives it only after the owning worker confirms that it
    // successfully bound the port.
    let relay_workers_for_task = relay_workers.clone();
    let relay_ports_for_task = relay_ports.clone();
    let relay_ports_for_tick = relay_ports.clone();
    tokio::spawn(async move {
        while let Some(rc) = relay_rx.recv().await {
            let workers = relay_workers_for_task.clone();
            let relay_ports = relay_ports_for_task.clone();
            tokio::spawn(async move {
                if let Err(e) = register_match_start(&rc).await {
                    log::error!(
                        "[DB] lobby {} will not start without match registration: {}",
                        rc.lobby_id, e
                    );
                    relay_ports.lock().await.release(rc.relay_port);
                    let closed_msg = sow_core::protocol::ServerLobbyClosedMessage {
                        lobby_id: rc.lobby_id,
                        reason: format!("DB_REGISTRATION_FAILED: {e}"),
                        rematch_lobby_id: None,
                    };
                    if let Ok(closed_json) = bincode::serialize(
                        &sow_core::protocol::ServerMessage::LobbyClosed(closed_msg),
                    ) {
                        for (_player_id, tx) in &rc.players_tx {
                            let _ = tx.try_send(closed_json.clone());
                        }
                    }
                    return;
                }

                let Some(worker) = workers.get(rc.worker_index) else {
                    log::error!(
                        "[RELAY] worker index {} missing for lobby {} port {}",
                        rc.worker_index, rc.lobby_id, rc.relay_port
                    );
                    relay_ports.lock().await.release(rc.relay_port);
                    return;
                };

                match register_relay(&rc, worker).await {
                    Ok(()) => {
                        log::info!(
                            "[RELAY] Lobby {} registered with worker {} on dynamic port {}",
                            rc.lobby_id, worker.id, rc.relay_port
                        );

                        // Broadcast Start message to each player with their specific my_player_id.
                        for (player_id, tx) in &rc.players_tx {
                            let start_msg = sow_core::protocol::ServerStartMessage {
                                config: rc.config.clone(),
                                my_player_id: Some(*player_id),
                                lobby_id: Some(rc.lobby_id),
                                seed: rc.seed,
                                players: rc.start_players.clone(),
                                missed_turns: vec![],
                                map_data: None,
                                relay_port: Some(rc.relay_port),
                                relay_host: Some(worker.host.clone()),
                            };
                            let handoff_result = async {
                                let ticket = rc.player_tickets.get(player_id).ok_or_else(|| {
                                    format!(
                                        "missing relay ticket for network player {} in lobby {}",
                                        player_id, rc.lobby_id
                                    )
                                })?;
                                let ticket_json = bincode::serialize(
                                    &sow_core::protocol::ServerMessage::RelayTicket {
                                        lobby_id: rc.lobby_id,
                                        player_id: *player_id,
                                        ticket: ticket.clone(),
                                    },
                                )
                                .map_err(|e| format!("serialize relay ticket: {e}"))?;
                                send_relay_handoff_frame(
                                    tx,
                                    ticket_json,
                                    rc.lobby_id,
                                    *player_id,
                                    "RelayTicket",
                                )
                                .await?;

                                let start_json = bincode::serialize(
                                    &sow_core::protocol::ServerMessage::Start(Box::new(start_msg)),
                                )
                                .map_err(|e| format!("serialize Start: {e}"))?;
                                send_relay_handoff_frame(
                                    tx,
                                    start_json,
                                    rc.lobby_id,
                                    *player_id,
                                    "Start",
                                )
                                .await
                            }
                            .await;
                            if let Err(e) = handoff_result {
                                log::error!(
                                    "[RELAY] lobby {} player {} handoff failed; Start not sent: {}",
                                    rc.lobby_id, player_id, e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        log::error!(
                            "[RELAY] lobby {} port {} registration failed: {}",
                            rc.lobby_id, rc.relay_port, e
                        );
                        relay_ports.lock().await.release(rc.relay_port);

                        let closed_msg = sow_core::protocol::ServerLobbyClosedMessage {
                            lobby_id: rc.lobby_id,
                            reason: format!("RELAY_REGISTRATION_FAILED: {}", e),
                            rematch_lobby_id: None,
                        };
                        if let Ok(closed_json) = bincode::serialize(
                            &sow_core::protocol::ServerMessage::LobbyClosed(closed_msg),
                        ) {
                            for (_player_id, tx) in &rc.players_tx {
                                let _ = tx.try_send(closed_json.clone());
                            }
                        }
                    }
                }
            });
        }
    });

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        let mut tick_count: u64 = 0;
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let tick_start = tokio::time::Instant::now();
                    tick_count += 1;

                    // ── PHASE 1: In-memory work under the lock (microseconds) ──
                    let (ready_candidates, broadcast_json) = {
                        let mut games = games_clone.lock().await;
                        let mut nid = next_id_clone.lock().await;
                        master_tick(&mut games, &mut nid);

                        // Extract lobbies that completed Loading and are ready for relay
                        let mut ready_candidates = Vec::new();
                        let mut i = 0;
                        while i < games.len() {
                            if games[i].phase == lobby::LobbyPhase::ReadyForRelay {
                                let Some(relay_port) = relay_ports_for_tick.lock().await.allocate() else {
                                    log::error!("dynamic relay port pool exhausted; lobby {} remains pending", games[i].id);
                                    i += 1;
                                    continue;
                                };
                                let mut lobby = games.remove(i);
                                let mut relay_players_json = Vec::new();
                                let mut player_tickets = std::collections::HashMap::new();
                                let mut start_players = Vec::new();
                                let mut player_ids = Vec::new();
                                let mut players_tx = Vec::new();
                                for p in &lobby.players {
                                    let origin = if p.is_internal_bot {
                                        "internal_bot"
                                    } else {
                                        "external_network"
                                    };
                                    log::info!(
                                        "[MATCH_AUDIT_PLAYER] lobby={} player={} origin={} ip={} session_id={:?} relay_ticket_expected={}",
                                        lobby.id,
                                        p.player_id,
                                        origin,
                                        p.ip,
                                        p.session_id,
                                        !p.is_internal_bot
                                    );
                                    // Every network participant must be present in Start so each
                                    // lockstep client can register the same player ids. Internal
                                    // backfillers go as Human + is_ai_controlled — the core
                                    // auto-plays them on every client (zero sockets).
                                    start_players.push(PlayerInfo {
                                        id: p.player_id,
                                        name: p.name.clone(),
                                        player_type: sow_core::player::PlayerType::Human,
                                        color: p.leader.filler_rgb(),
                                        team: p.team,
                                        spawn_x: 0,
                                        spawn_y: 0,
                                        civilization: p.civilization,
                                        leader: p.leader,
                                        is_ai_controlled: p.is_internal_bot,
                                    });
                                    let relay_ticket_digest = if p.is_internal_bot {
                                        None
                                    } else {
                                        let (ticket, digest) = new_relay_ticket();
                                        player_tickets.insert(p.player_id, ticket);
                                        Some(digest)
                                    };
                                    relay_players_json.push(serde_json::json!({
                                        "player_id": p.player_id,
                                        "name": p.name,
                                        "database_account_id": p.database_account_id,
                                        "leader": p.leader,
                                        "team": p.team,
                                        "is_internal": p.is_internal_bot,
                                        "session_id": p.session_id,
                                        "relay_ticket_digest": relay_ticket_digest,
                                    }));
                                    if let Some(acc_id) = &p.database_account_id {
                                        player_ids.push(acc_id.clone());
                                    }
                                    // Internal bots have no socket — exclude from Start delivery.
                                    if !p.is_internal_bot {
                                        players_tx.push((p.player_id, p.tx.clone()));
                                    }
                                }
                                let roster_total = lobby.players.len();
                                let internal_bot_count = lobby
                                    .players
                                    .iter()
                                    .filter(|p| p.is_internal_bot)
                                    .count();
                                let validated_external_count = roster_total - internal_bot_count;
                                log::info!(
                                    "[MATCH_AUDIT] lobby={} roster_total={} internal_bot_count={} validated_external_count={} relay_ticket_count={}",
                                    lobby.id,
                                    roster_total,
                                    internal_bot_count,
                                    validated_external_count,
                                    player_tickets.len()
                                );
                                // HumansVsNations autobalance: the AI nation side
                                // must match the human side so the match is fair.
                                // nation_count is set here (server-authoritative)
                                // so every client's deterministic engine spawns
                                // exactly that many Blue nations.
                                if lobby.config.game_mode == "HumansVsNations" {
                                    let human_side_players = lobby.players.len() as u32;
                                    lobby.config.nation_count = human_side_players;
                                    log::info!(
                                        "[HVN] Lobby {} autobalanced: human_side_players={} vs nations={}",
                                        lobby.id, human_side_players, human_side_players
                                    );
                                }
                                ready_candidates.push(RelayCandidate {
                                    lobby_id: lobby.id,
                                    relay_port,
                                    ticket_expires_at: std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .unwrap_or_default()
                                        .as_secs()
                                        .saturating_add(RELAY_TICKET_TTL_SECS),
                                    worker_index: relay_port as usize % relay_worker_count,
                                    active_empty_secs: lobby.active_empty_secs,
                                    tick_rate_ms: lobby.config.tick_rate_ms,
                                    config: lobby.config.clone(),
                                    kind: format!("{:?}", lobby.kind),
                                    seed: lobby.seed,
                                    start_players,
                                    relay_players_json,
                                    player_tickets,
                                    player_ids,
                                    players_tx,
                                });
                            } else {
                                i += 1;
                            }
                        }

                        // Build broadcast data (serialization is in-memory)
                        let broadcast_json = if tick_count % 10 == 0 {
                            let lobbies_info = build_lobby_broadcast(&games);
                            let broadcast_msg = ServerLobbiesBroadcastMessage { lobbies: lobbies_info };
                            match bincode::serialize(&sow_core::protocol::ServerMessage::LobbiesBroadcast(broadcast_msg)) {
                                Ok(json) => Some(json),
                                Err(e) => { log::error!("[BROADCAST] Failed to serialize LobbiesBroadcast: {}", e); None }
                            }
                        } else {
                            None
                        };

                        (ready_candidates, broadcast_json)
                    }; // ── LOCK RELEASED ──

                    // Enqueue relay candidates to background worker task (zero I/O in tick)
                    for rc in ready_candidates {
                        let _ = relay_tx_clone.send(rc);
                    }

                    // ── PHASE 3: Broadcast + perf (no lock needed) ──
                    // Note: Initial LobbiesBroadcast is sent on connection & HTTP endpoint.
                    // Global tick broadcast is disabled at high scale to preserve WS throughput.
                    if let Some(json) = broadcast_json {
                        let _ = global_tx_clone.send(json);
                    }

                    // Precise latency performance metric logger
                    let elapsed = tick_start.elapsed().as_millis();
                    if elapsed > 10 {
                        log::warn!("[PERF] Event loop lag detected! Master tick execution took {}ms", elapsed);
                    }
                }
                Some(event) = event_rx.recv() => {
                    let mut games = games_clone.lock().await;
                    let mut nid = next_id_clone.lock().await;
                    match event {
                        ServerEvent::Join { client_tx, name, clan_tag, civilization, leader, target_lobby_id, host_private, build_version, database_account_id, host_config, password, ip, session_id } => {
                            log::info!("Player {} (clan: {}, ip: {}, session_id: {}) joining with version: {}", name, clan_tag, ip, session_id, build_version);
                            match join_player(&mut games, &mut nid, JoinPlayerOpts {
                                name,
                                clan_tag,
                                civilization,
                                leader,
                                client_tx: client_tx.clone(),
                                target_lobby_id,
                                host_private,
                                database_account_id,
                                host_config,
                                password,
                                ip,
                                session_id: Some(session_id),
                            }) {
                                Ok((lobby_id, player_id, map_name, is_private)) => {
                                    let lobby_info = games.iter().find(|g| g.id == lobby_id).map(lobby_to_info);
                                    let ack = ServerJoinAckMessage { lobby_id, player_id, map_name, is_private, lobby_info };
                                    match bincode::serialize(&sow_core::protocol::ServerMessage::JoinAck(ack)) {
                                        Ok(json) => { let _ = client_tx.try_send(json); }
                                        Err(e) => { log::error!("[JOIN] Failed to serialize JoinAck for player {} in lobby {}: {}", player_id, lobby_id, e); }
                                    }
                                    if let Some(lobby) = games.iter().find(|g| g.id == lobby_id) {
                                        sync_host_lobby_to_members(lobby);
                                    }
                                }
                                Err(reason) => {
                                    log::warn!("[JOIN] Join rejected: {}", reason);
                                    let fail = ServerJoinFailedMessage { reason };
                                    match bincode::serialize(&sow_core::protocol::ServerMessage::JoinFailed(fail)) {
                                        Ok(json) => { let _ = client_tx.try_send(json); }
                                        Err(e) => { log::error!("[JOIN] Failed to serialize JoinFailed: {}", e); }
                                    }
                                }
                            }
                        }

                        ServerEvent::Leave { lobby_id, player_id } => {
                            if is_host_teardown(&games, lobby_id, player_id) {
                                if let Some(lobby) = games.iter().find(|g| g.id == lobby_id) {
                                    notify_lobby_closed(lobby, "HOST_LEFT");
                                }
                                games.retain(|g| g.id != lobby_id);
                                log::info!("[LOBBY] Host {} left Custom lobby {} — lobby dropped, members returned to menu", player_id, lobby_id);
                            } else {
                                leave_player(&mut games, lobby_id, player_id);
                                if let Some(lobby) = games.iter().find(|g| g.id == lobby_id) {
                                    sync_host_lobby_to_members(lobby);
                                }
                            }
                        }
                        ServerEvent::Kick { lobby_id, requester_id, target_id, ban } => {
                            kick_player(&mut games, lobby_id, requester_id, target_id, ban);
                        }
                        ServerEvent::SetTeam { lobby_id, requester_id, target_id } => {
                            set_player_team(&mut games, lobby_id, requester_id, target_id);
                        }
                        ServerEvent::Ready { lobby_id, player_id } => {
                            if let Some(lobby) = games.iter_mut().find(|g| g.id == lobby_id) {
                                lobby.ready_players.insert(player_id);
                                sync_host_lobby_to_members(lobby);
                            } else {
                                log::warn!("[READY] Player {} sent Ready for unknown lobby {}", player_id, lobby_id);
                            }
                        }
                        ServerEvent::MapDownloadProgress { lobby_id, player_id, progress } => {
                            if let Some(lobby) = games.iter_mut().find(|g| g.id == lobby_id) {
                                if let Some(p) = lobby.players.iter_mut().find(|p| p.player_id == player_id) {
                                    p.download_progress = progress;
                                } else {
                                    log::warn!("[PROGRESS] Player {} not found in lobby {} for download progress update", player_id, lobby_id);
                                }
                                sync_host_lobby_to_members(lobby);
                            } else {
                                log::warn!("[PROGRESS] Lobby {} not found for player {} progress update", lobby_id, player_id);
                            }
                        }
                        ServerEvent::ForceStart { lobby_id, player_id } => {
                            log::info!("[EVENT] ForceStart received lobby={} player={}", lobby_id, player_id);
                            force_start(&mut games, lobby_id, player_id);
                        }
                    }
                }
            }
        }
    });

    let addr = std::env::var("SOW_WS_LISTEN").unwrap_or_else(|_| "0.0.0.0:25565".to_string());

    let listener = TcpListener::bind(&addr).await.expect("Failed to bind");
    let max_connections = match max_server_connections() {
        Ok(value) => value,
        Err(e) => {
            log::error!("{e}");
            return;
        }
    };
    let connection_slots = Arc::new(Semaphore::new(max_connections));
    log::info!(
        "SOW-SERVER listening on ws://{} max_connections={} handshake_timeout={}s",
        addr, max_connections, HANDSHAKE_TIMEOUT_SECS
    );

    // HTTP Static File Server for maps and Admin Dashboard
    let games_for_axum = Arc::clone(&games_state);
    let redis_client_for_axum = redis_client.clone();
    tokio::spawn(async move {
        let root = maps_root.clone();
        let state = AppState {
            games: games_for_axum,
            redis_client: redis_client_for_axum,
        };
        let catalog_route = axum::Router::new()
            .route(
                "/maps/catalog.bin",
                axum::routing::get(|| async {
                    axum::response::Response::builder()
                        .header("Content-Type", "application/octet-stream")
                        .header("Cache-Control", "public, max-age=60")
                        .body(axum::body::Body::from(
                            map_catalog::catalog_bytes().to_vec(),
                        ))
                        .unwrap()
                }),
            )
            .route(
                "/maps/catalog.json",
                axum::routing::get(catalog_json_handler),
            )
            .route("/lobbies.json", axum::routing::get(lobbies_json_handler))
            .route("/admin/api/status", axum::routing::get(admin_status))
            .with_state(state);

        let app = catalog_route
            .nest_service(
                "/maps",
                tower_http::services::ServeDir::new(root).precompressed_br(),
            )
            .layer(tower_http::cors::CorsLayer::permissive());
        let http_addr =
            std::env::var("SOW_MAPS_HTTP_LISTEN").unwrap_or_else(|_| "127.0.0.1:25566".to_string());
        log::info!(
            "SOW-SERVER HTTP serving maps and admin on http://{}",
            http_addr
        );
        let listener = tokio::net::TcpListener::bind(&http_addr).await.unwrap();
        axum::serve(listener, app).await.unwrap();
    });

    while let Ok((stream, addr)) = listener.accept().await {
        let permit = match connection_slots.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(_) => {
                let rejected = REJECTED_CONNECTIONS.fetch_add(1, Ordering::Relaxed) + 1;
                if rejected.is_power_of_two() {
                    log::warn!("Connection cap rejected orchestrator peer={} count={rejected}", addr);
                }
                drop(stream);
                continue;
            }
        };
        let mut global_rx = global_tx.subscribe();
        let ev_tx = event_tx.clone();
        let games_state_conn = Arc::clone(&games_state);
        tokio::spawn(async move {
            let _connection_permit = permit;
            let session_id = next_session_id();
            let ip_cell = Arc::new(std::sync::Mutex::new(addr.ip().to_string()));
            let ip_cell_clone = Arc::clone(&ip_cell);
            let ws_stream = match tokio::time::timeout(
                Duration::from_secs(HANDSHAKE_TIMEOUT_SECS),
                tokio_tungstenite::accept_hdr_async(stream, move |req: &tokio_tungstenite::tungstenite::handshake::server::Request, response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                if let Some(real_ip) = req.headers().get("X-Real-IP") {
                    if let Ok(ip) = real_ip.to_str() {
                        if let Ok(mut guard) = ip_cell_clone.lock() {
                            *guard = ip.to_string();
                        }
                    }
                } else if let Some(forwarded) = req.headers().get("X-Forwarded-For") {
                    if let Ok(ip) = forwarded.to_str() {
                        if let Some(first_ip) = ip.split(',').next() {
                            if let Ok(mut guard) = ip_cell_clone.lock() {
                                *guard = first_ip.trim().to_string();
                            }
                        }
                    }
                }
                Ok(response)
            }),
            )
            .await
            {
                Ok(Ok(ws)) => ws,
                Ok(Err(e)) => {
                    log::error!("Handshake failed: {}", e);
                    return;
                }
                Err(_) => {
                    log::warn!("Handshake timeout peer={} session_id={}", addr, session_id);
                    return;
                }
            };
            let ip_str = if let Ok(guard) = ip_cell.lock() {
                guard.clone()
            } else {
                addr.ip().to_string()
            };
            let (mut write, mut read) = ws_stream.split();
            log::info!("Client connected from IP: {} session_id={}", ip_str, session_id);

            let (direct_tx, mut direct_rx) = mpsc::channel::<Vec<u8>>(4096);

            // Send immediate single-cast LobbiesBroadcast snapshot on connection so the home menu loads instantly
            {
                let games_guard = games_state_conn.lock().await;
                let lobbies_info = build_lobby_broadcast(&games_guard);
                let broadcast_msg = ServerLobbiesBroadcastMessage {
                    lobbies: lobbies_info,
                };
                if let Ok(json) = bincode::serialize(
                    &sow_core::protocol::ServerMessage::LobbiesBroadcast(broadcast_msg),
                ) {
                    let _ = direct_tx.try_send(json);
                }
            }

            let mut my_lobby_id: Option<u64> = None;
            let mut my_player_id: Option<u16> = None;

            loop {
                tokio::select! {
                    msg = read.next() => {
                        match msg {
                            Some(Ok(msg)) => {
                                if msg.is_binary() {
                                    let data = msg.into_data();

                                    if let Ok(msg) = bincode::deserialize::<sow_core::protocol::ClientMessage>(&data) {
                                        match msg {
                                            sow_core::protocol::ClientMessage::Join { name, is_observer: _, target_lobby_id, host_private, build_version, clan_tag, civilization, leader, database_account_id, host_config, password } => {
                                                let server_version = std::env::var("SOW_BUILD_VERSION")
                                                    .unwrap_or_else(|_| std::fs::read_to_string(".version").unwrap_or_default().trim().to_string());

                                                if !server_version.is_empty() && build_version != server_version {
                                                    log::warn!("Client version mismatch: expected {}, got {}", server_version, build_version);
                                                    let fail = sow_core::protocol::ServerJoinFailedMessage { reason: "VERSION_MISMATCH".to_string() };
                                                    let json = bincode::serialize(&sow_core::protocol::ServerMessage::JoinFailed(fail)).unwrap();
                                                    let _ = direct_tx.try_send(json);
                                                    continue;
                                                }

                                                // Legacy join carries no identity proof: a
                                                // client-asserted account id binds nothing
                                                // (no stats attribution, no reconnect
                                                // takeover). Only JoinWithAuth verifies.
                                                if database_account_id.is_some() {
                                                    log::info!(
                                                        "[AUTH] legacy Join with client-asserted account id from {} — bound as unverified guest",
                                                        ip_str
                                                    );
                                                }

                                                let free_leaders = sow_core::commerce::current_leader_rotation();
                                                let leader = if free_leaders.contains(&leader) {
                                                    leader
                                                } else {
                                                    free_leaders[0]
                                                };

                                                let _ = ev_tx.send(ServerEvent::Join {
                                                    name,
                                                    clan_tag,
                                                    civilization,
                                                    leader,
                                                    client_tx: direct_tx.clone(),
                                                    target_lobby_id,
                                                    host_private,
                                                    build_version,
                                                    database_account_id: None,
                                                    host_config,
                                                    password,
                                                    ip: ip_str.clone(),
                                                    session_id,
                                                }).await;
                                            }
                                            sow_core::protocol::ClientMessage::JoinWithAuth { join, auth } => {
                                                let payload = *join;
                                                let server_version = std::env::var("SOW_BUILD_VERSION")
                                                    .unwrap_or_else(|_| std::fs::read_to_string(".version").unwrap_or_default().trim().to_string());

                                                if !server_version.is_empty() && payload.build_version != server_version {
                                                    log::warn!("Client version mismatch: expected {}, got {}", server_version, payload.build_version);
                                                    let fail = sow_core::protocol::ServerJoinFailedMessage { reason: "VERSION_MISMATCH".to_string() };
                                                    let json = bincode::serialize(&sow_core::protocol::ServerMessage::JoinFailed(fail)).unwrap();
                                                    let _ = direct_tx.try_send(json);
                                                    continue;
                                                }

                                                let (database_account_id, leader) = match verify_identity(&auth, payload.leader).await {
                                                    Ok(identity) => {
                                                        log::info!(
                                                            "[AUTH] join verified provider={} account={}",
                                                            auth.provider,
                                                            identity.account_id
                                                        );
                                                        (Some(identity.account_id), identity.leader)
                                                    }
                                                    Err(e) => {
                                                        log::warn!(
                                                            "[AUTH] join verification failed provider={}: {}",
                                                            auth.provider,
                                                            e
                                                        );
                                                        let fail = sow_core::protocol::ServerJoinFailedMessage {
                                                            reason: "IDENTITY_OR_LEADER_UNAVAILABLE".to_string(),
                                                        };
                                                        if let Ok(json) = bincode::serialize(
                                                            &sow_core::protocol::ServerMessage::JoinFailed(fail),
                                                        ) {
                                                            let _ = direct_tx.try_send(json);
                                                        }
                                                        continue;
                                                    }
                                                };

                                                let _ = ev_tx.send(ServerEvent::Join {
                                                    name: payload.name,
                                                    clan_tag: payload.clan_tag,
                                                    civilization: payload.civilization,
                                                    leader,
                                                    client_tx: direct_tx.clone(),
                                                    target_lobby_id: payload.target_lobby_id,
                                                    host_private: payload.host_private,
                                                    build_version: payload.build_version,
                                                    database_account_id,
                                                    host_config: payload.host_config,
                                                    password: payload.password,
                                                    ip: ip_str.clone(),
                                                    session_id,
                                                }).await;
                                            }
                                            sow_core::protocol::ClientMessage::Gameplay { .. } => {
                                                // Orchestrator ignores gameplay intents
                                            }
                                            sow_core::protocol::ClientMessage::Leave {} => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                                    let _ = ev_tx.send(ServerEvent::Leave {
                                                        lobby_id: l_id,
                                                        player_id: p_id,
                                                    }).await;
                                                }
                                                my_lobby_id = None;
                                                my_player_id = None;
                                            }
                                            sow_core::protocol::ClientMessage::Ready { lobby_id, player_id } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                                    if lobby_id == l_id && player_id == p_id {
                                                        let _ = ev_tx.send(ServerEvent::Ready {
                                                            lobby_id: l_id,
                                                            player_id: p_id,
                                                        }).await;
                                                    }
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::ReadyWithTicket { .. } => {
                                                // Relay tickets are valid only on the direct relay
                                                // connection, never on the orchestrator socket.
                                            }
                                            sow_core::protocol::ClientMessage::ReconnectWithTicket { .. } => {
                                                // Reconnect capabilities are valid only on the direct
                                                // relay connection, never on the orchestrator socket.
                                            }
                                            sow_core::protocol::ClientMessage::MapDownloadProgress { lobby_id, player_id, progress } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                                    if lobby_id == l_id && player_id == p_id {
                                                        let _ = ev_tx.send(ServerEvent::MapDownloadProgress {
                                                            lobby_id: l_id,
                                                            player_id: p_id,
                                                            progress,
                                                        }).await;
                                                    }
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::RematchRequest { lobby_id: _ } => {
                                                // Orchestrator ignores RematchRequest, it is handled by the relay server
                                            }
                                            sow_core::protocol::ClientMessage::ForceStart { lobby_id, player_id } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                                    if lobby_id == l_id && player_id == p_id {
                                                        let _ = ev_tx.send(ServerEvent::ForceStart {
                                                            lobby_id: l_id,
                                                            player_id: p_id,
                                                        }).await;
                                                    }
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::Kick { lobby_id, target_player_id } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                                    if lobby_id == l_id {
                                                        let _ = ev_tx.send(ServerEvent::Kick {
                                                            lobby_id: l_id,
                                                            requester_id: p_id,
                                                            target_id: target_player_id,
                                                            ban: false,
                                                        }).await;
                                                    }
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::Ban { lobby_id, target_player_id } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                                    if lobby_id == l_id {
                                                        let _ = ev_tx.send(ServerEvent::Kick {
                                                            lobby_id: l_id,
                                                            requester_id: p_id,
                                                            target_id: target_player_id,
                                                            ban: true,
                                                        }).await;
                                                    }
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::SetPlayerTeam { lobby_id, target_player_id } => {
                                                if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                                                    if lobby_id == l_id {
                                                        let _ = ev_tx.send(ServerEvent::SetTeam {
                                                            lobby_id: l_id,
                                                            requester_id: p_id,
                                                            target_id: target_player_id,
                                                        }).await;
                                                    }
                                                }
                                            }
                                            sow_core::protocol::ClientMessage::Ping { client_time } => {
                                                let pong = sow_core::protocol::ServerMessage::Pong { client_time };
                                                let json = bincode::serialize(&pong).unwrap();
                                                let _ = direct_tx.try_send(json);
                                            }
                                            sow_core::protocol::ClientMessage::SubmitStats { .. } => {}
                                            sow_core::protocol::ClientMessage::SubmitStatsWithLeader { .. } => {}
                                            sow_core::protocol::ClientMessage::SubmitMatchReport { .. } => {}
                                        }
                                        continue;
                                    }

                                    log::warn!("[SERVER] Unrecognized message");
                                }
                            }
                            _ => break,
                        }
                    }
                    Ok(broadcast_data) = global_rx.recv() => {
                        // Drop lobbies broadcast packets if the client is already in a lobby/match
                        if my_lobby_id.is_none() {
                            if write.send(Message::Binary(broadcast_data)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Some(direct_data) = direct_rx.recv() => {
                        if let Ok(server_msg) = bincode::deserialize::<sow_core::protocol::ServerMessage>(&direct_data) {
                            match server_msg {
                                sow_core::protocol::ServerMessage::JoinAck(ack) => {
                                    my_lobby_id = Some(ack.lobby_id);
                                    my_player_id = Some(ack.player_id);
                                }
                                sow_core::protocol::ServerMessage::Start(start) => {
                                    if let Some(pid) = start.my_player_id {
                                        my_player_id = Some(pid);
                                    }
                                }
                                _ => {}
                            }
                        }

                        if write.send(Message::Binary(direct_data)).await.is_err() {
                            break;
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_secs(60)) => {
                        break;
                    }
                }
            }

            if let (Some(l_id), Some(p_id)) = (my_lobby_id, my_player_id) {
                let _ = ev_tx
                    .send(ServerEvent::Leave {
                        lobby_id: l_id,
                        player_id: p_id,
                    })
                    .await;
            }
        });
    }
}

#[derive(Clone)]
struct AppState {
    games: Arc<Mutex<Vec<lobby::ServerLobby>>>,
    redis_client: redis::Client,
}

async fn admin_status(
    axum::extract::State(state): axum::extract::State<AppState>,
    headers: axum::http::HeaderMap,
) -> axum::response::Response {
    // The status payload discloses player IPs and account ids — it is
    // localhost tooling only and requires the deployment bearer secret.
    let secret = std::env::var("SOW_DB_SECRET").unwrap_or_default();
    let presented = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim);
    if secret.is_empty() || presented != Some(secret.as_str()) {
        log::warn!("[ADMIN] unauthorized /admin/api/status request");
        return axum::response::IntoResponse::into_response((
            axum::http::StatusCode::UNAUTHORIZED,
            axum::response::Json(serde_json::json!({"error": "Unauthorized"})),
        ));
    }

    let games = state.games.lock().await;
    let mut lobbies = Vec::new();
    for lobby in &*games {
        let mut players = Vec::new();
        for p in &lobby.players {
            players.push(serde_json::json!({
                "player_id": p.player_id,
                "name": p.name,
                "clan_tag": p.clan_tag,
                "civilization": format!("{:?}", p.civilization),
                "leader": format!("{:?}", p.leader),
                "download_progress": p.download_progress,
                "ip": p.ip,
                "database_account_id": p.database_account_id,
            }));
        }
        lobbies.push(serde_json::json!({
            "id": lobby.id,
            "kind": format!("{:?}", lobby.kind),
            "is_private": lobby.is_private,
            "phase": format!("{:?}", lobby.phase),
            "countdown_secs": lobby.countdown_secs,
            "relay_port": lobby.relay_port,
            "players": players,
            "map_name": lobby.config.map_name,
            "game_mode": lobby.game_mode,
        }));
    }
    drop(games);

    // Query valkey INFO
    let valkey_info = tokio::task::spawn_blocking({
        let client = state.redis_client.clone();
        move || -> serde_json::Value {
            let mut conn = match client.get_connection() {
                Ok(c) => c,
                Err(_) => return serde_json::json!({"error": "cannot connect"}),
            };
            match redis::cmd("INFO").query::<String>(&mut conn) {
                Ok(info) => {
                    let mut result = serde_json::Map::new();
                    for line in info.lines() {
                        if line.contains(':') && !line.starts_with('#') {
                            let parts: Vec<&str> = line.splitn(2, ':').collect();
                            let key = parts[0].trim();
                            let val = parts[1].trim();
                            if [
                                "used_memory_human",
                                "used_memory_peak_human",
                                "connected_clients",
                                "blocked_clients",
                                "keyspace_hits",
                                "keyspace_misses",
                                "uptime_in_seconds",
                                "instantaneous_ops_per_sec",
                                "instantaneous_input_kbps",
                                "instantaneous_output_kbps",
                                "total_connections_received",
                                "total_commands_processed",
                                "expired_keys",
                                "evicted_keys",
                            ]
                            .contains(&key)
                            {
                                result.insert(
                                    key.to_string(),
                                    serde_json::Value::String(val.to_string()),
                                );
                            }
                        }
                    }
                    serde_json::Value::Object(result)
                }
                Err(_) => serde_json::json!({"error": "info failed"}),
            }
        }
    })
    .await
    .unwrap_or(serde_json::json!({"error": "task failed"}));

    // Query sow-database stats
    let db_base = std::env::var("SOW_DB_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:25585".to_string());
    let db_stats_url = format!(
        "{}/internal/stats",
        db_base.trim_end_matches('/')
    );
    let db_stats = match reqwest::Client::new()
        .get(&db_stats_url)
        .header("Authorization", format!("Bearer {secret}"))
        .send()
        .await
    {
        Ok(r) => r
            .json::<serde_json::Value>()
            .await
            .unwrap_or(serde_json::json!({"error": "db unreachable"})),
        Err(_) => serde_json::json!({"error": "db unreachable"}),
    };

    axum::response::IntoResponse::into_response(axum::response::Json(serde_json::json!({
        "lobbies": lobbies,
        "valkey": valkey_info,
        "database": db_stats,
    })))
}

async fn lobbies_json_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl axum::response::IntoResponse {
    let games = state.games.lock().await;
    let lobbies_info = lobby::build_lobby_broadcast(&games);
    axum::response::Response::builder()
        .header("Content-Type", "application/json")
        .header("Cache-Control", "no-store")
        .body(axum::body::Body::from(
            serde_json::to_string(&lobbies_info).unwrap(),
        ))
        .unwrap()
}

async fn catalog_json_handler() -> impl axum::response::IntoResponse {
    let catalog = map_catalog::catalog_json();
    axum::response::Response::builder()
        .header("Content-Type", "application/json")
        .header("Cache-Control", "public, max-age=60")
        .body(axum::body::Body::from(
            serde_json::to_string(&catalog).unwrap(),
        ))
        .unwrap()
}

#[cfg(test)]
mod relay_worker_tests {
    use super::{
        parse_relay_worker, send_relay_handoff_frame, RelayPortAllocator,
        DEFAULT_RELAY_WORKER_COUNT, RELAY_PORT_MAX, RELAY_PORT_MIN,
    };

    #[test]
    fn parses_game_and_management_hosts() {
        let worker = parse_relay_worker("relay-a.example:mgmt.example:8083", 2)
            .expect("valid worker");
        assert_eq!(worker.id, 2);
        assert_eq!(worker.host, "relay-a.example");
        assert_eq!(worker.mgmt_url, "https://mgmt.example:8083");
    }

    #[test]
    fn parses_separate_game_and_management_hosts() {
        let worker = parse_relay_worker("data.example:mgmt.example:8080", 0)
            .expect("valid worker");
        assert_eq!(worker.host, "data.example");
        assert_eq!(worker.mgmt_url, "https://mgmt.example:8080");
    }

    #[test]
    fn allocates_ports_in_range_and_by_worker_modulo() {
        let mut allocator = RelayPortAllocator::new();
        let first = allocator.allocate().expect("first dynamic port");
        let second = allocator.allocate().expect("second dynamic port");
        assert!((RELAY_PORT_MIN..=RELAY_PORT_MAX).contains(&first));
        assert!((RELAY_PORT_MIN..=RELAY_PORT_MAX).contains(&second));
        assert_eq!(first % DEFAULT_RELAY_WORKER_COUNT as u16, 0);
        assert_eq!(second % DEFAULT_RELAY_WORKER_COUNT as u16, 1);
        allocator.release(first);
        assert!(!allocator.used.contains(&first));
    }

    #[test]
    fn rejects_malformed_worker_entries() {
        assert!(parse_relay_worker("relay-a.example:8080", 0).is_none());
        assert!(parse_relay_worker("relay-a.example:mgmt.example:not-a-port", 0).is_none());
        assert!(parse_relay_worker("http://relay-a.example:mgmt.example:8080", 0).is_none());
        assert!(parse_relay_worker("[::1]:mgmt.example:8080", 0).is_none());
    }

    #[tokio::test]
    async fn handoff_frames_are_delivered_in_order() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        send_relay_handoff_frame(&tx, b"ticket".to_vec(), 7, 2, "RelayTicket")
            .await
            .expect("ticket delivery");
        let ticket = rx.recv().await.expect("ticket frame");
        assert_eq!(ticket, b"ticket");

        send_relay_handoff_frame(&tx, b"start".to_vec(), 7, 2, "Start")
            .await
            .expect("start delivery");
        let start = rx.recv().await.expect("start frame");
        assert_eq!(start, b"start");
    }
}
