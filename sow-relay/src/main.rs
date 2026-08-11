//! sow-relay — one F-Stack worker process over the DPDK bridge.
//!
//! Each lobby owns one dynamic TCP port in 1024..=65535. The destination port
//! modulo the number of F-Stack queues selects the owning worker. Lobbies are
//! registered by sow-server via that worker's kernel management HTTP port:
//!
//!     POST /internal/lobby/register   {lobby_id, tick_number, tick_rate_ms,
//!                                      active_empty_secs, players, config?}
//!     GET  /internal/lobbies          active lobby roster (ops/validation)
//!
//! `SOW_RELAY_BASE_MGMT_PORT` (legacy `SOW_RELAY_MGMT_PORT` remains accepted)
//! selects the management port; game listeners are created dynamically when
//! the owning worker accepts a lobby registration.
//!
//! Connections arrive via RSS on the game port. The first frame decides the
//! role: `ClientMessage::Ready` → relay player routed to its registered lobby;
//! silence for ORCHESTRATOR_GRACE_SECS → orchestrator WS (LobbiesBroadcast
//! feed for the backfill daemon / ops tooling).
//!
//! The per-lobby game loop (ticks, intents, missed-tick drop, MatchTracker,
//! finalize upload) is the sow-relay logic, now keyed by registry lookup.

use fstack_bridge::bridge::{self, Ev};
use futures_util::{SinkExt, StreamExt};
use hmac::{Hmac, Mac};
use libc::{c_int, c_void};
use log::{error, info, warn};
use redis::Commands;
use sow_core::game_config::{BotDifficulty, GameConfig};
use sow_core::player::Leader;
use sow_core::protocol::{
    ClientMessage, GameplayIntent, LobbyInfo, LobbyKind, LobbyPlayerSyncState,
    ServerLobbiesBroadcastMessage, ServerLobbyClosedMessage, ServerMessage, ServerTurnMessage,
    StampedIntent, Turn,
};
use rustls::{Certificate, PrivateKey, ServerConfig};
use sha2::Sha256;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ffi::CString;
use std::io::BufReader;
use std::path::PathBuf;
use std::pin::Pin;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio_rustls::TlsAcceptor;
use tokio::net::TcpListener;
use tokio::sync::mpsc::error::TrySendError;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock, Semaphore};
use tokio::io::AsyncWriteExt;
use tokio::time::{interval, Duration};
use tokio_tungstenite::tungstenite::Message;

const DEFAULT_MGMT_PORT: u16 = 8080;
const DYNAMIC_PORT_MIN: u16 = 1024;
const DEFAULT_EXPECTED_WORKER_COUNT: u16 = 4;
const MAX_MGMT_BODY_BYTES: usize = 2 * 1024 * 1024;
const MGMT_CLOCK_SKEW_SECS: u64 = 30;
const MGMT_NONCE_TTL_SECS: u64 = 120;
type HmacSha256 = Hmac<Sha256>;

fn expected_worker_count() -> u16 {
    std::env::var("SOW_RELAY_WORKER_COUNT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|count| (1..=64).contains(count))
        .unwrap_or(DEFAULT_EXPECTED_WORKER_COUNT)
}

static WORKER_QUEUE_ID: AtomicU16 = AtomicU16::new(0);
static WORKER_QUEUE_COUNT: AtomicU16 = AtomicU16::new(1);
static LISTENER_COUNT: AtomicU64 = AtomicU64::new(0);
static DISPATCH_LOCAL: AtomicU64 = AtomicU64::new(0);
static DISPATCH_REDIRECTED: AtomicU64 = AtomicU64::new(0);
static DISPATCH_UNMATCHED: AtomicU64 = AtomicU64::new(0);

/// TLS acceptor for direct browser connections. `None` when no cert is
/// configured (dev mode, plain ws://). Set once at boot from
/// `SOW_RELAY_TLS_CERT` / `SOW_RELAY_TLS_KEY` file paths.
static TLS_ACCEPTOR: OnceLock<Option<TlsAcceptor>> = OnceLock::new();

fn load_tls_acceptor() -> Option<TlsAcceptor> {
    let cert_path = std::env::var("SOW_RELAY_TLS_CERT").ok()?;
    let key_path = std::env::var("SOW_RELAY_TLS_KEY").ok()?;

    let cert_bytes = std::fs::read(&cert_path).ok()?;
    let key_bytes = std::fs::read(&key_path).ok()?;

    // Parse certificates
    let mut cert_reader = BufReader::new(cert_bytes.as_slice());
    let raw_certs = match rustls_pemfile::certs(&mut cert_reader) {
        Ok(v) => v,
        Err(e) => {
            error!("[tls] cert parse error: {e}");
            return None;
        }
    };
    let certs: Vec<Certificate> = raw_certs.into_iter().map(Certificate).collect();
    if certs.is_empty() {
        error!("[tls] no certificates found in {}", cert_path);
        return None;
    }

    // Parse private key (try PKCS8, then RSA)
    let mut key_reader = BufReader::new(key_bytes.as_slice());
    let mut key_data: Option<Vec<u8>> = None;
    if let Ok(keys) = rustls_pemfile::pkcs8_private_keys(&mut key_reader) {
        if let Some(d) = keys.into_iter().next() {
            key_data = Some(d);
        }
    }
    if key_data.is_none() {
        let mut r2 = BufReader::new(key_bytes.as_slice());
        if let Ok(keys) = rustls_pemfile::rsa_private_keys(&mut r2) {
            if let Some(d) = keys.into_iter().next() {
                key_data = Some(d);
            }
        }
    }
    let key = match key_data {
        Some(k) => PrivateKey(k),
        None => {
            error!("[tls] no private key found in {}", key_path);
            return None;
        }
    };

    match ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
    {
        Ok(config) => {
            info!("[tls] cert loaded from {}, key from {}", cert_path, key_path);
            Some(TlsAcceptor::from(Arc::new(config)))
        }
        Err(e) => {
            error!("[tls] config error: {e}");
            None
        }
    }
}

fn base_mgmt_port() -> u16 {
    std::env::var("SOW_RELAY_BASE_MGMT_PORT")
        .or_else(|_| std::env::var("SOW_RELAY_MGMT_PORT"))
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_MGMT_PORT)
}

fn worker_port(base: u16, queue_id: u16) -> Result<u16, String> {
    base.checked_add(queue_id)
        .ok_or_else(|| format!("worker port overflow: base={base} queue={queue_id}"))
}

/// Advertised relay address (data PIP) written to `sow:relay:{lobby_id}`.
fn relay_host() -> String {
    if let Ok(host) = std::env::var("SOW_RELAY_HOST") {
        if !host.trim().is_empty() {
            return host.trim().to_string();
        }
    }
    let addr = std::env::var("SOW_RELAY_ADDR").unwrap_or_else(|_| "127.0.0.1:80".to_string());
    addr.rsplit_once(':')
        .map(|(host, _)| host.to_string())
        .unwrap_or(addr)
}

fn relay_addr(port: u16) -> String {
    format!("{}:{port}", relay_host())
}

unsafe extern "C" fn relay_packet_dispatcher(
    data: *mut c_void,
    len: *mut u16,
    queue_id: u16,
    nb_queues: u16,
) -> c_int {
    if data.is_null() || len.is_null() {
        return queue_id as c_int;
    }
    let bytes = unsafe { std::slice::from_raw_parts(data as *const u8, *len as usize) };
    match fstack_bridge::tcp_destination_queue(bytes, nb_queues, DYNAMIC_PORT_MIN) {
        Some(target) if target == queue_id => {
            DISPATCH_LOCAL.fetch_add(1, Ordering::Relaxed);
            target as c_int
        }
        Some(target) => {
            DISPATCH_REDIRECTED.fetch_add(1, Ordering::Relaxed);
            target as c_int
        }
        None => {
            DISPATCH_UNMATCHED.fetch_add(1, Ordering::Relaxed);
            queue_id as c_int
        }
    }
}

/// Seconds without any frame before a connection is classified as the
/// orchestrator WS (backfill daemon sends nothing, players Ready within ~2s).
const ORCHESTRATOR_GRACE_SECS: u64 = 3;

/// Max consecutive missed ticks before dropping a slow client.
/// At 10 ticks/s, 40 = 4 seconds of silence — fast enough that a zombie
/// connection (backfill saturated, not draining its socket) cannot pin
/// megabytes of turns in its per-client channel and OOM a relay worker.
const MAX_MISSED_TICKS: u32 = 40;

/// Per-connection outbound channel capacity. A smaller queue keeps the global
/// slot budget finite at 100k connections; slow clients are counted as missed
/// ticks and removed instead of accumulating an unbounded turn backlog.
const PER_CLIENT_CHANNEL: usize = 32;

/// Per-connection bridge RX capacity (inbound DPDK mbuf guards). At 100k
/// connections, a capacity of 256 would reserve up to 25.6M guard slots before
/// any payload exists. Keep this small; a stalled peer drops frames and the
/// guard is recycled immediately.
const BRIDGE_RX_CAP: usize = 16;

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

/// Number of append commands allowed to wait behind the replay writer. The
/// tick loop awaits when this queue is full, preserving every turn without
/// allowing replay backlog to grow with the match duration.
const REPLAY_QUEUE_CAP: usize = 256;
const LIVE_HISTORY_CAP: usize = 512;
/// Hard ceiling for one replay artifact. The current production replays are
/// below 1 MiB; this keeps a malformed or runaway match from becoming a RAM
/// or disk amplification vector.
const MAX_REPLAY_BYTES: u64 = 16 * 1024 * 1024;

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
    sender: mpsc::Sender<Arc<Vec<u8>>>,
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

enum ReplayCommand {
    Append(Turn),
    Finalize {
        reply: oneshot::Sender<Result<ReplayArtifact, String>>,
    },
}

struct ReplayArtifact {
    bytes: Vec<u8>,
    path: PathBuf,
}

#[derive(serde::Serialize)]
struct ReplayFinalizePayload<'a> {
    match_id: &'a str,
    lobby_json: &'a str,
    replay_data: &'a [u8],
}

struct ReplayJournal {
    live: Arc<Mutex<VecDeque<Turn>>>,
    tx: mpsc::Sender<ReplayCommand>,
    finalized: AtomicBool,
    failed: Arc<AtomicBool>,
}

impl ReplayJournal {
    fn new(match_id: u64) -> Arc<Self> {
        let spool_dir = std::env::var("SOW_REPLAY_SPOOL_DIR")
            .or_else(|_| std::env::var("SOW_REPLAY_DIR"))
            .unwrap_or_else(|_| "replays".to_string());
        let journal_path = PathBuf::from(spool_dir).join(format!("{match_id}.journal"));
        let (tx, rx) = mpsc::channel(REPLAY_QUEUE_CAP);
        let failed = Arc::new(AtomicBool::new(false));
        let journal = Arc::new(Self {
            live: Arc::new(Mutex::new(VecDeque::with_capacity(LIVE_HISTORY_CAP))),
            tx,
            finalized: AtomicBool::new(false),
            failed: failed.clone(),
        });
        tokio::spawn(replay_writer(journal_path, rx, failed));
        journal
    }

    async fn append(&self, turn: Turn) -> Result<(), String> {
        if self.finalized.load(Ordering::Acquire) {
            return Err("replay journal already finalized".to_string());
        }
        if self.failed.load(Ordering::Acquire) {
            return Err("replay writer failed".to_string());
        }
        // Backpressure is intentional: dropping an append would silently
        // corrupt the replay. The bounded channel prevents an unbounded RAM
        // queue while the writer catches up.
        self.tx
            .send(ReplayCommand::Append(turn.clone()))
            .await
            .map_err(|_| "replay writer stopped".to_string())?;
        let mut live = self.live.lock().await;
        live.push_back(turn);
        while live.len() > LIVE_HISTORY_CAP {
            live.pop_front();
        }
        Ok(())
    }

    async fn snapshot(&self) -> Vec<Turn> {
        self.live.lock().await.iter().cloned().collect()
    }

    async fn finalize(&self) -> Result<ReplayArtifact, String> {
        if self.finalized.swap(true, Ordering::AcqRel) {
            return Err("replay journal already finalized".to_string());
        }
        let (reply, result) = oneshot::channel();
        if self.tx.send(ReplayCommand::Finalize { reply }).await.is_err() {
            return Err("replay writer stopped".to_string());
        }
        let replay = result
            .await
            .map_err(|_| "replay writer dropped finalize response".to_string())??;
        self.live.lock().await.clear();
        Ok(replay)
    }
}

async fn replay_writer(
    journal_path: PathBuf,
    mut rx: mpsc::Receiver<ReplayCommand>,
    failed: Arc<AtomicBool>,
) {
    if let Some(parent) = journal_path.parent() {
        if let Err(e) = tokio::fs::create_dir_all(parent).await {
            failed.store(true, Ordering::Release);
            error!("[replay] create journal directory {:?} failed: {}", parent, e);
            return;
        }
    }
    let mut file = match tokio::fs::File::create(&journal_path).await {
        Ok(file) => file,
        Err(e) => {
            failed.store(true, Ordering::Release);
            error!("[replay] create journal {:?} failed: {}", journal_path, e);
            return;
        }
    };

    let mut journal_bytes = 0u64;
    while let Some(command) = rx.recv().await {
        match command {
            ReplayCommand::Append(turn) => {
                let encoded = match bincode::serialize(&turn) {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        failed.store(true, Ordering::Release);
                        error!("[replay] serialize turn failed: {}", e);
                        break;
                    }
                };
                let len = encoded.len() as u32;
                let record_bytes = 4u64.saturating_add(encoded.len() as u64);
                if journal_bytes.saturating_add(record_bytes) > MAX_REPLAY_BYTES {
                    failed.store(true, Ordering::Release);
                    error!(
                        "[replay] journal {:?} exceeded {} byte limit",
                        journal_path, MAX_REPLAY_BYTES
                    );
                    break;
                }
                if file.write_all(&len.to_le_bytes()).await.is_err()
                    || file.write_all(&encoded).await.is_err()
                {
                    failed.store(true, Ordering::Release);
                    error!("[replay] append journal {:?} failed", journal_path);
                    break;
                }
                journal_bytes = journal_bytes.saturating_add(record_bytes);
            }
            ReplayCommand::Finalize { reply } => {
                let result = async {
                    file.flush().await.map_err(|e| e.to_string())?;
                    file.sync_all().await.map_err(|e| e.to_string())?;
                    let size = tokio::fs::metadata(&journal_path)
                        .await
                        .map_err(|e| e.to_string())?
                        .len();
                    if size > MAX_REPLAY_BYTES {
                        return Err(format!(
                            "replay journal exceeds {} byte limit",
                            MAX_REPLAY_BYTES
                        ));
                    }
                    let bytes = tokio::fs::read(&journal_path)
                        .await
                        .map_err(|e| e.to_string())?;
                    let history = decode_replay_journal(&bytes)?;
                    let canonical = bincode::serialize(&history).map_err(|e| e.to_string())?;
                    let replay_path = journal_path.with_extension("replay");
                    let temp_path = journal_path.with_extension("replay.tmp");
                    tokio::fs::write(&temp_path, &canonical)
                        .await
                        .map_err(|e| e.to_string())?;
                    tokio::fs::rename(&temp_path, &replay_path)
                        .await
                        .map_err(|e| e.to_string())?;
                    let _ = tokio::fs::remove_file(&journal_path).await;
                    Ok(ReplayArtifact {
                        bytes: canonical,
                        path: replay_path,
                    })
                }
                .await;
                if result.is_err() {
                    failed.store(true, Ordering::Release);
                }
                let _ = reply.send(result);
                break;
            }
        }
    }
}

fn decode_replay_journal(bytes: &[u8]) -> Result<Vec<Turn>, String> {
    let mut offset = 0usize;
    let mut history = Vec::new();
    while offset < bytes.len() {
        if bytes.len().saturating_sub(offset) < 4 {
            return Err("truncated replay record length".to_string());
        }
        let len = u32::from_le_bytes(
            bytes[offset..offset + 4]
                .try_into()
                .map_err(|_| "invalid replay record length".to_string())?,
        ) as usize;
        offset += 4;
        if len > bytes.len().saturating_sub(offset) {
            return Err("truncated replay record".to_string());
        }
        let turn = bincode::deserialize(&bytes[offset..offset + len])
            .map_err(|e| format!("decode replay record: {e}"))?;
        history.push(turn);
        offset += len;
    }
    Ok(history)
}

fn finalize_gate() -> Arc<Semaphore> {
    static GATE: OnceLock<Arc<Semaphore>> = OnceLock::new();
    GATE.get_or_init(|| Arc::new(Semaphore::new(1))).clone()
}

fn trigger_match_finalize(match_id: u64, lobby_json: String, journal: Arc<ReplayJournal>) {
    tokio::spawn(async move {
        let _permit = match finalize_gate().acquire_owned().await {
            Ok(permit) => permit,
            Err(_) => return,
        };
        let artifact = match journal.finalize().await {
            Ok(artifact) => artifact,
            Err(e) => {
                error!("Failed to finalize replay journal for {match_id}: {e}");
                return;
            }
        };
        let replay_path = artifact.path.clone();
        let metadata_path = replay_path.with_extension("json");
        let metadata_tmp = replay_path.with_extension("json.tmp");
        if let Err(e) = async {
            tokio::fs::write(&metadata_tmp, lobby_json.as_bytes())
                .await
                .map_err(|e| e.to_string())?;
            tokio::fs::rename(&metadata_tmp, &metadata_path)
                .await
                .map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        }
        .await
        {
            error!("Failed to write replay metadata for {match_id}: {e}");
            let _ = tokio::fs::remove_file(&metadata_tmp).await;
            return;
        }

        let db_url =
            std::env::var("SOW_DB_URL").unwrap_or_else(|_| "http://127.0.0.1:25585".to_string());
        let secret = std::env::var("SOW_DB_SECRET")
            .expect("SOW_DB_SECRET validated at relay startup");
        let url = format!("{}/internal/match-finalize", db_url.trim_end_matches('/'));

        let match_id_string = match_id.to_string();
        let payload = ReplayFinalizePayload {
            match_id: &match_id_string,
            lobby_json: &lobby_json,
            replay_data: &artifact.bytes,
        };

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

        if success {
            if let Err(e) = tokio::fs::remove_file(&replay_path).await {
                if e.kind() != std::io::ErrorKind::NotFound {
                    warn!("[replay] cleanup {:?} failed: {}", replay_path, e);
                }
            }
            if let Err(e) = tokio::fs::remove_file(&metadata_path).await {
                if e.kind() != std::io::ErrorKind::NotFound {
                    warn!("[replay] cleanup {:?} failed: {}", metadata_path, e);
                }
            }
        } else {
            error!("[CRITICAL] Failed to upload match {match_id} to database after 5 attempts.");

            // Keep only a small pointer in Valkey. The replay and metadata
            // remain in the bounded local spool for recovery; placing the raw
            // bytes in Valkey duplicates the payload in RAM and caused the
            // historical relay OOM.
            let url = std::env::var("SOW_VALKEY_URL")
                .or_else(|_| std::env::var("SOW_REDIS_URL"))
                .unwrap_or_else(|_| "redis://127.0.0.1/".to_string());

            let mut valkey_success = false;
            if let Ok(client) = redis::Client::open(url) {
                if let Ok(mut con) = client.get_connection() {
                    let key = "sow:match_history:dead_letter";
                    let fallback_payload = serde_json::to_vec(&serde_json::json!({
                        "match_id": match_id,
                        "replay_path": replay_path.to_string_lossy(),
                        "metadata_path": metadata_path.to_string_lossy(),
                    }))
                    .unwrap_or_default();
                    if let Ok(()) = con.lpush::<_, _, ()>(key, fallback_payload) {
                        warn!(
                            "[FALLBACK] Saved replay pointer in local Valkey queue under key '{}' for match {match_id}",
                            key
                        );
                        valkey_success = true;
                    }
                }
            }

            if !valkey_success {
                error!(
                    "[ALERT] Valkey fallback failed; replay remains at {:?} and metadata at {:?}",
                    replay_path, metadata_path
                );
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
        }
    }
}

// ---- registry / lobby state -------------------------------------------------

/// Register body — the orchestrator's lobby shape plus the dynamic game port
/// that this worker must bind before Start is broadcast.
#[derive(serde::Deserialize, serde::Serialize)]
struct RegisterBody {
    lobby_id: u64,
    relay_port: u16,
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
    relay_port: u16,
    valid_players: HashMap<u16, String>,
    clients: Arc<Mutex<HashMap<u16, ClientChannel>>>,
    journal: Arc<ReplayJournal>,
    tracker: Arc<std::sync::Mutex<MatchTracker>>,
    ev_tx: mpsc::Sender<RelayEvent>,
}

type Registry = Arc<RwLock<HashMap<u64, Arc<LobbyState>>>>;

// ---- main (bridge boot — same shape as the fstack-bridge examples) ----------

fn main() {
    if std::env::var("SOW_DB_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_none()
    {
        eprintln!("SOW_DB_SECRET must be set; refusing insecure default");
        std::process::exit(78);
    }
    if std::env::var("SOW_RELAY_CONTROL_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .is_none()
    {
        eprintln!("SOW_RELAY_CONTROL_SECRET must be set; refusing unauthenticated management");
        std::process::exit(78);
    }

    let prog_args: Vec<CString> = std::env::args()
        .filter(|a| !a.starts_with("--fstack-"))
        .map(|a| CString::new(a).unwrap())
        .collect();
    env_logger::init();

    let base_mgmt = base_mgmt_port();

    // TLS: load cert if configured, otherwise plain ws://.
    TLS_ACCEPTOR.get_or_init(|| {
        let acceptor = load_tls_acceptor();
        if acceptor.is_some() {
            info!("[BOOT] TLS enabled — browser connects via wss:// directly");
        } else {
            info!("[BOOT] TLS disabled — plain ws:// (dev mode)");
        }
        acceptor
    });
    if std::env::var("SOW_MGMT_TLS_REQUIRED").ok().as_deref() == Some("1")
        && TLS_ACCEPTOR.get().and_then(|acceptor| acceptor.as_ref()).is_none()
    {
        error!("[sow-relay] management TLS is required but the relay certificate is unavailable");
        std::process::exit(78);
    }

    // TAP dev loop: inject --no-pci + net_tap0 vdev (FSTACK_TAP=1). Empty for the physical VF.
    let mut extra_eal: Vec<&str> = Vec::new();
    if std::env::var("FSTACK_TAP").ok().as_deref() == Some("1") {
        extra_eal.push("--no-pci");
        extra_eal.push("--iova-mode=va"); // TAP vdev needs VA IOVA (no physical NIC for PA)
        extra_eal.push("--vdev=net_tap0,iface=tap0");
        info!("[sow-relay] TAP mode (--no-pci + --iova-mode=va + net_tap0)");
    } else {
        info!("[sow-relay] physical VF mode (config.ini [dpdk] allow=)");
    }

    unsafe {
        if let Err(code) = fstack_bridge::init(&prog_args, &extra_eal) {
            error!("[sow-relay] init failed (code={})", code);
            std::process::exit(1);
        }

        let (mut pid, mut qid, mut nbq, mut reta) = (0u16, 0u16, 0u16, 0u16);
        if fstack_bridge::ff_rss_self_queue_info(&mut pid, &mut qid, &mut nbq, &mut reta) != 0 {
            error!("[sow-relay] unable to inspect F-Stack worker queue");
            std::process::exit(1);
        }
        if qid >= nbq || nbq == 0 {
            error!("[sow-relay] invalid F-Stack queue assignment: proc_id={pid} queue_id={qid} nb_queues={nbq}");
            std::process::exit(1);
        }
        let expected_workers = expected_worker_count();
        if nbq != expected_workers {
            error!(
                "[sow-relay] dynamic routing requires {} F-Stack queues, got {}",
                expected_workers, nbq
            );
            std::process::exit(1);
        }
        if pid != qid {
            error!(
                "[sow-relay] F-Stack proc_id must equal queue_id for worker ownership: proc_id={pid} queue_id={qid}"
            );
            std::process::exit(1);
        }
        let mport = match worker_port(base_mgmt, qid) {
            Ok(port) => port,
            Err(e) => {
                error!("[sow-relay] {e}");
                std::process::exit(1);
            }
        };
        WORKER_QUEUE_ID.store(qid, Ordering::Relaxed);
        WORKER_QUEUE_COUNT.store(nbq, Ordering::Relaxed);
        info!(
            "[BOOT] proc_id={} queue_id={} nb_queues={} reta_size={} dynamic_ports={}..65535 mgmt_port={}",
            pid, qid, nbq, reta, DYNAMIC_PORT_MIN, mport
        );
        fstack_bridge::ff_regist_packet_dispatcher(relay_packet_dispatcher);

        bridge::setup();
        bridge::KQ = fstack_bridge::ff_kqueue();
        if bridge::KQ < 0 {
            error!("[sow-relay] ff_kqueue failed");
            std::process::exit(1);
        }

        let registry: Registry = Arc::new(RwLock::new(HashMap::new()));

        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tokio runtime");
        rt.spawn(mgmt_http(registry.clone(), mport));
        rt.spawn(bridge_worker(registry.clone()));

        info!("[BOOT] dynamic game listeners, mgmt :{}, entering ff_run", mport);
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

type MgmtNonceCache = Arc<Mutex<HashMap<String, u64>>>;

fn mgmt_header<'a>(headers: &'a HashMap<String, String>, name: &str) -> Option<&'a str> {
    headers.get(&name.to_ascii_lowercase()).map(String::as_str)
}

async fn verify_mgmt_auth(
    method: &str,
    path: &str,
    body: &[u8],
    headers: &HashMap<String, String>,
    nonce_cache: &MgmtNonceCache,
) -> bool {
    let Some(secret) = std::env::var("SOW_RELAY_CONTROL_SECRET")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        error!("[mgmt] SOW_RELAY_CONTROL_SECRET is missing");
        return false;
    };
    let Some(timestamp) = mgmt_header(headers, "x-sow-timestamp") else {
        return false;
    };
    let Some(nonce) = mgmt_header(headers, "x-sow-nonce") else {
        return false;
    };
    let Some(signature) = mgmt_header(headers, "x-sow-signature") else {
        return false;
    };
    if nonce.is_empty() || nonce.len() > 128 {
        return false;
    }
    let Ok(timestamp) = timestamp.parse::<u64>() else {
        return false;
    };
    let Ok(now) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) else {
        return false;
    };
    if now.as_secs().abs_diff(timestamp) > MGMT_CLOCK_SKEW_SECS {
        return false;
    }
    let Ok(provided) = hex::decode(signature) else {
        return false;
    };
    let Ok(mut mac) = HmacSha256::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(method.as_bytes());
    mac.update(b"\n");
    mac.update(path.as_bytes());
    mac.update(b"\n");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b"\n");
    mac.update(nonce.as_bytes());
    mac.update(b"\n");
    mac.update(body);
    if mac.verify_slice(&provided).is_err() {
        return false;
    }

    let mut cache = nonce_cache.lock().await;
    let cutoff = now.as_secs().saturating_sub(MGMT_NONCE_TTL_SECS);
    cache.retain(|_, seen| *seen >= cutoff);
    if cache.contains_key(nonce) {
        return false;
    }
    if cache.len() >= 4096 {
        if let Some(oldest) = cache
            .iter()
            .min_by_key(|(_, seen)| **seen)
            .map(|(nonce, _)| nonce.clone())
        {
            cache.remove(&oldest);
        }
    }
    cache.insert(nonce.to_string(), now.as_secs());
    true
}

async fn bind_game_port(port: u16) -> Result<(), String> {
    if !(DYNAMIC_PORT_MIN..=u16::MAX).contains(&port) {
        return Err(format!("relay port {port} outside dynamic range"));
    }
    let newly_bound = bridge::listen_port(port).await?;
    if !newly_bound {
        return Ok(());
    }
    LISTENER_COUNT.fetch_add(1, Ordering::Relaxed);

    let redis_con = redis_shared();
    tokio::task::spawn_blocking(move || {
        let mut guard = redis_con.lock().unwrap();
        if let Some(ref mut con) = *guard {
            if let Err(e) = con.sadd::<_, _, ()>("sow:ports", port) {
                error!("[REDIS] SADD sow:ports {port} FAILED: {e}");
            }
        }
    });
    Ok(())
}

async fn release_game_port(port: u16) {
    let removed = match bridge::unlisten_port(port).await {
        Ok(removed) => removed,
        Err(e) => {
            error!("[relay] failed to close dynamic listener {port}: {e}");
            return;
        }
    };
    if !removed {
        return;
    }
    LISTENER_COUNT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
        Some(n.saturating_sub(1))
    }).ok();

    let redis_con = redis_shared();
    tokio::task::spawn_blocking(move || {
        let mut guard = redis_con.lock().unwrap();
        if let Some(ref mut con) = *guard {
            if let Err(e) = con.srem::<_, _, ()>("sow:ports", port) {
                error!("[REDIS] SREM sow:ports {port} FAILED: {e}");
            }
        }
    });
}

async fn mgmt_http(registry: Registry, mport: u16) {
    let listen = mgmt_listen();
    let listener = match TcpListener::bind((listen.as_str(), mport)).await {
        Ok(l) => l,
        Err(e) => {
            error!("[mgmt] bind {}:{} failed: {}", listen, mport, e);
            return;
        }
    };
    info!("[mgmt] http listening on {}:{}", listen, mport);
    let nonce_cache: MgmtNonceCache = Arc::new(Mutex::new(HashMap::new()));

    loop {
        let (sock, _) = match listener.accept().await {
            Ok(x) => x,
            Err(_) => continue,
        };
        let reg = registry.clone();
        let nonce_cache = nonce_cache.clone();
        let tls_acceptor = TLS_ACCEPTOR.get().and_then(|acceptor| acceptor.clone());
        tokio::spawn(async move {
            if let Some(acceptor) = tls_acceptor {
                match acceptor.accept(sock).await {
                    Ok(tls) => mgmt_connection(tls, reg, nonce_cache).await,
                    Err(e) => warn!("[mgmt] TLS handshake failed: {e}"),
                }
            } else {
                mgmt_connection(sock, reg, nonce_cache).await;
            }
        });
    }
}

async fn mgmt_connection<S>(
    mut sock: S,
    registry: Registry,
    nonce_cache: MgmtNonceCache,
)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
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
    let headers: HashMap<String, String> = lines
        .filter_map(|line| {
            let (key, value) = line.split_once(':')?;
            Some((key.trim().to_ascii_lowercase(), value.trim().to_string()))
        })
        .collect();
    let clen: usize = headers
        .get("content-length")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    if clen > MAX_MGMT_BODY_BYTES {
        let _ = sock
            .write_all(b"HTTP/1.1 413 Payload Too Large\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await;
        return;
    }

    while buf.len() < header_end + clen && clen > 0 {
        match sock.read(&mut tmp).await {
            Ok(0) | Err(_) => return,
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
        }
    }
    if buf.len() < header_end + clen {
        return;
    }

    let body = &buf[header_end..header_end + clen];
    let authorized = path == "/healthz"
        || verify_mgmt_auth(&method, &path, body, &headers, &nonce_cache).await;
    let (status, resp) = if authorized {
        handle_http(&registry, &method, &path, body).await
    } else {
        (
            "401 Unauthorized",
            serde_json::json!({ "error": "unauthorized" }),
        )
    };
    let resp_body = serde_json::to_string(&resp).unwrap_or_else(|_| "{}".to_string());
    let out = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        resp_body.len(),
        resp_body
    );
    let _ = sock.write_all(out.as_bytes()).await;
}

async fn handle_http(
    registry: &Registry,
    method: &str,
    path: &str,
    body: &[u8],
) -> (&'static str, serde_json::Value) {
    match (method, path) {
        ("GET", "/healthz") => ("200 OK", serde_json::json!({ "ok": true })),
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
            if !(DYNAMIC_PORT_MIN..=u16::MAX).contains(&rb.relay_port) {
                return (
                    "400 Bad Request",
                    serde_json::json!({ "error": "relay_port must be in 1024..=65535" }),
                );
            }
            let queue_id = WORKER_QUEUE_ID.load(Ordering::Relaxed);
            let queue_count = WORKER_QUEUE_COUNT.load(Ordering::Relaxed);
            if queue_count == 0 || rb.relay_port % queue_count != queue_id {
                return (
                    "409 Conflict",
                    serde_json::json!({
                        "error": "relay_port belongs to another worker",
                        "relay_port": rb.relay_port,
                        "worker": rb.relay_port % queue_count.max(1),
                        "this_worker": queue_id,
                    }),
                );
            }
            if let Err(e) = bind_game_port(rb.relay_port).await {
                return (
                    "409 Conflict",
                    serde_json::json!({ "error": e }),
                );
            }
            let existing = registry.read().await.get(&rb.lobby_id).cloned();
            if let Some(existing) = existing {
                if existing.relay_port != rb.relay_port {
                    return (
                        "409 Conflict",
                        serde_json::json!({ "error": "lobby already owns another relay port" }),
                    );
                }
                return (
                    "200 OK",
                    serde_json::json!({ "ok": true, "existing": true }),
                );
            }
            let port = rb.relay_port;
            spawn_lobby(registry, rb).await;
            (
                "200 OK",
                serde_json::json!({ "ok": true, "existing": false, "relay_port": port }),
            )
        }
        ("GET", "/internal/lobbies") => {
            let reg = registry.read().await;
            let lobbies: Vec<serde_json::Value> = reg
                .iter()
                .map(|(id, st)| {
                    serde_json::json!({
                        "lobby_id": id,
                        "relay_port": st.relay_port,
                        "humans": st.clients.try_lock().map(|c| c.len()).unwrap_or(0),
                    })
                })
                .collect();
            ("200 OK", serde_json::json!({ "lobbies": lobbies }))
        }
        ("GET", "/internal/metrics") => (
            "200 OK",
            serde_json::json!({
                "proc_id": std::process::id(),
                "listener_count": LISTENER_COUNT.load(Ordering::Relaxed),
                "queue_id": WORKER_QUEUE_ID.load(Ordering::Relaxed),
                "queue_count": WORKER_QUEUE_COUNT.load(Ordering::Relaxed),
                "dispatch_local": DISPATCH_LOCAL.load(Ordering::Relaxed),
                "dispatch_redirected": DISPATCH_REDIRECTED.load(Ordering::Relaxed),
                "dispatch_unmatched": DISPATCH_UNMATCHED.load(Ordering::Relaxed),
            }),
        ),
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
            let removed_state = registry.write().await.remove(&lobby_id);
            let removed = removed_state.is_some();
            if let Some(state) = removed_state {
                release_game_port(state.relay_port).await;
            }
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
    let journal = ReplayJournal::new(body.lobby_id);
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
    }));

    let state = Arc::new(LobbyState {
        id: body.lobby_id,
        relay_port: body.relay_port,
        valid_players,
        clients,
        journal,
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
        let val = relay_addr(body.relay_port);
        tokio::task::spawn_blocking(move || {
            let mut guard = redis_con.lock().unwrap();
            if let Some(ref mut con) = *guard {
                if let Err(e) = con.set_ex::<_, _, ()>(&key, val, 60) {
                    error!("[REDIS] SETEX {key} FAILED: {e}");
                }
            }
        });
    }

    let initial_empty_secs = if body.active_empty_secs <= 0.0 {
        30.0
    } else {
        body.active_empty_secs
    };

    tokio::spawn(tick_task(
        state.clone(),
        registry.clone(),
        ev_rx,
        body.tick_rate_ms as u64,
        body.tick_number,
        initial_empty_secs,
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
    // Lifecycle: a lobby is removed from the registry and its live replay
    // dropped when the match ends (GameOver / all clients leave) OR when it
    // outlives the maximum match duration. Backfill bots play indefinitely, so
    // without this ceiling a filled lobby keeps pushing to live history forever
    // and a relay worker OOMs (5.5GB in ~4min under 50k bots).
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
                let already_finalized = state.tracker.lock().unwrap().finalized;

                if humans == 0 {
                    active_empty_secs -= 0.05;
                    if already_finalized || active_empty_secs <= 0.0 {
                        info!("[relay] lobby {} empty for too long, GC", state.id);
                        drop(clients);
                        let lobby_json = state.tracker.lock().unwrap().lobby_json.clone();
                        trigger_match_finalize(state.id, lobby_json, state.journal.clone());
                        registry.write().await.remove(&state.id);
                        release_game_port(state.relay_port).await;
                        break;
                    }
                    // No clients: skip turn serialization/broadcast entirely.
                    continue;
                } else {
                    active_empty_secs = 30.0;
                }

                // Lifecycle: enforce the maximum match duration. Broadcast
                // LobbyClosed so clients exit cleanly, finalize the replay to
                // the database, then drop the lobby from the registry. The
                // complete replay is already on disk; only live history is RAM.
                let is_finalized = already_finalized;
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
                        let frame = Arc::new(json);
                        for (_, client) in clients.iter() {
                            let _ = client.sender.try_send(frame.clone());
                        }
                    }
                    drop(clients);
                    let lobby_json = state.tracker.lock().unwrap().lobby_json.clone();
                    trigger_match_finalize(state.id, lobby_json, state.journal.clone());
                    registry.write().await.remove(&state.id);
                    release_game_port(state.relay_port).await;
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

                if let Err(e) = state.journal.append(turn.clone()).await {
                    error!("[replay] lobby {} append failed: {}", state.id, e);
                }

                let msg = ServerTurnMessage { turn };
                let json = Arc::new(
                    bincode::serialize(&ServerMessage::Turn(msg))
                        .expect("serialize ServerTurnMessage"),
                );

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
                        state.id, std::process::id(), state.relay_port, humans, total_ticks, total_intents
                    );
                    last_status = std::time::Instant::now();
                    // Heartbeat: refresh per-lobby Redis TTL.
                    let redis_con = redis_shared();
                    let key = format!("sow:relay:{}", state.id);
                    let val = relay_addr(state.relay_port);
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
                        let json = Arc::new(
                            bincode::serialize(&ServerMessage::LobbyClosed(msg))
                                .expect("serialize LobbyClosed"),
                        );
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
                        // If the ws_task is stalled writing to a saturated peer
                        // socket, drop the guard so the DPDK mbuf is recycled
                        // immediately instead of pinning per-connection memory.
                        match tx.try_send(guard) {
                            Ok(()) => {}
                            Err(TrySendError::Full(guard)) => {
                                warn!("[bridge] per-connection RX budget exhausted fd={fd}; dropping frame");
                                drop(guard);
                            }
                            Err(TrySendError::Closed(guard)) => drop(guard),
                        }
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

/// Unified stream: either plain Conn or TLS-wrapped Conn.
/// tokio-tungstenite needs AsyncRead + AsyncWrite; we delegate both.
enum MaybeTlsConn {
    Plain(bridge::Conn),
    Tls(tokio_rustls::server::TlsStream<bridge::Conn>),
}

impl AsyncRead for MaybeTlsConn {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            MaybeTlsConn::Plain(c) => Pin::new(c).poll_read(cx, buf),
            MaybeTlsConn::Tls(c) => Pin::new(c).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for MaybeTlsConn {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        match self.get_mut() {
            MaybeTlsConn::Plain(c) => Pin::new(c).poll_write(cx, buf),
            MaybeTlsConn::Tls(c) => Pin::new(c).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            MaybeTlsConn::Plain(c) => Pin::new(c).poll_flush(cx),
            MaybeTlsConn::Tls(c) => Pin::new(c).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        match self.get_mut() {
            MaybeTlsConn::Plain(c) => Pin::new(c).poll_shutdown(cx),
            MaybeTlsConn::Tls(c) => Pin::new(c).poll_shutdown(cx),
        }
    }
}

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

    // TLS handshake (if cert configured), then WebSocket upgrade.
    let stream = if let Some(acceptor) = TLS_ACCEPTOR.get().and_then(|o| o.as_ref()) {
        match acceptor.accept(conn).await {
            Ok(tls) => MaybeTlsConn::Tls(tls),
            Err(e) => {
                warn!("[tls] handshake fail fd={} err={}", fd, e);
                return;
            }
        }
    } else {
        MaybeTlsConn::Plain(conn)
    };

    let ws = match tokio_tungstenite::accept_async(stream).await {
        Ok(ws) => ws,
        Err(e) => {
            warn!("[ws] handshake fail fd={} err={}", fd, e);
            return;
        }
    };

    let (mut write, mut read) = ws.split();
    let (direct_tx, mut direct_rx) = mpsc::channel::<Arc<Vec<u8>>>(PER_CLIENT_CHANNEL);

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

    // A connection that sent a player frame but did not resolve to a lobby
    // owned by this worker is never a valid game session. Close it explicitly
    // instead of leaving an unowned socket in the worker's event loop. This is
    // the cross-worker ownership guard: the client must reconnect using the
    // relay endpoint carried by ServerStartMessage.
    if role.is_none() {
        warn!("[relay] rejecting unowned/invalid player connection fd={fd}");
        let _ = write.send(Message::Close(None)).await;
        return;
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
                                            let _ = direct_tx.try_send(Arc::new(json));
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
                match tokio::time::timeout(
                    Duration::from_millis(200),
                    write.send(Message::Binary((*direct_data).clone())),
                )
                .await
                {
                    Ok(Ok(())) => {}
                    _ => break,
                }
            }
            _ = ping_interval.tick() => {
                // Idle path (no ticks): keepalive ping + reap check.
                // Lifecycle: if the lobby was finalized/GC'd (removed from the
                // registry) or this client was dropped by the tick task, end
                // this connection so the Arc<LobbyState> (and its live replay
                // window) is released instead of being pinned alive forever.
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
    direct_tx: &mpsc::Sender<Arc<Vec<u8>>>,
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

    let history = lobby.journal.snapshot().await;
    for past_turn in history {
        let msg = ServerTurnMessage { turn: past_turn };
        if let Ok(json) = bincode::serialize(&ServerMessage::Turn(msg)) {
            let _ = tokio::time::timeout(
                Duration::from_millis(500),
                direct_tx.send(Arc::new(json)),
            )
            .await;
        }
    }

    Some(Role::RelayPlayer { lobby, player_id })
}

/// Orchestrator connection: periodic LobbiesBroadcast of registered lobbies,
/// so the backfill daemon keeps spawning until slots fill.
async fn orchestrator_task(
    write: &mut futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<MaybeTlsConn>,
        Message,
    >,
    read: &mut futures_util::stream::SplitStream<
        tokio_tungstenite::WebSocketStream<MaybeTlsConn>,
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

#[cfg(test)]
mod dispatcher_tests {
    use super::{try_ready_register, Registry};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::{mpsc, RwLock};

    #[tokio::test]
    async fn rejects_ready_for_lobby_not_owned_by_worker() {
        let registry: Registry = Arc::new(RwLock::new(HashMap::new()));
        let (tx, _rx) = mpsc::channel(1);
        assert!(try_ready_register(&registry, 42, 7, &tx).await.is_none());
    }
}
