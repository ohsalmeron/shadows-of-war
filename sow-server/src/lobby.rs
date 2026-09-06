//! Master lobby: dynamic queue, single countdown promotion,
//! broadcasts only joinable lobbies, Active GC when no external players remain.

use sow_core::game_config::GameConfig;
use sow_core::protocol::{LobbyInfo, LobbyKind, Team};
use tokio::sync::mpsc;

pub const LOBBY_COUNTDOWN_SECS: f32 = 15.0;
pub const TICK_SECS: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyPhase {
    Waiting,
    CountingDown,
    Loading,
    ReadyForRelay,
}

pub struct PlayerConnection {
    pub name: String,
    pub clan_tag: String,
    pub player_id: u16,
    pub tx: mpsc::Sender<Vec<u8>>,
    pub download_progress: u8,
    pub civilization: sow_core::player::Civilization,
    pub leader: sow_core::player::Leader,
    pub database_account_id: Option<String>,
    /// Lobby-stage team (Teams mode only; `None` in FFA). Carried into the match start.
    pub team: Option<Team>,
    pub ip: String,
    /// Monotonic server connection identifier. `None` means an internal bot
    /// with no network session.
    pub session_id: Option<u64>,
    /// Server-side internal bot filler: no socket and no relay ticket behind it.
    pub is_internal_bot: bool,
}

pub struct ServerLobby {
    pub id: u64,
    /// Matchmaking = server-spawned auto-queue; Custom = player-created, host-started.
    pub kind: LobbyKind,
    pub is_private: bool,
    pub phase: LobbyPhase,
    /// Remaining seconds while CountingDown.
    pub countdown_secs: f32,
    /// Counts down while Active and there are zero external players in `players`.
    pub active_empty_secs: f32,
    pub players: Vec<PlayerConnection>,
    pub ready_players: std::collections::HashSet<u16>,
    pub seed: u64,
    pub config: GameConfig,
    pub game_mode: String,
    pub relay_port: Option<u16>,
    /// Only set for Custom lobbies — the player_id of whoever created the lobby.
    pub host_player_id: Option<u16>,
    pub password: Option<String>,
    pub host_name: String,
    /// Identities (account id, or `name:<name>` fallback) banned from this lobby.
    pub banned: std::collections::HashSet<String>,
    /// Internal bot fill (fictional humans, ORGANIC entry — see
    /// `bot_fill/mod.rs` MENTAL MODEL: each ghost independently picks a random
    /// moment during the countdown. No mean, no drip, no perceptible rate —
    /// clusters and silences emerge from pure independence).
    /// `bot_fill_target`: total ghost count to reach (rolled once per lobby).
    /// `pending_bots`: reserved identities still waiting, as
    /// `(account_id, display_name, join_at_elapsed)` and sorted ASCENDING by
    /// `join_at_elapsed` (seconds into the countdown when this ghost shows up).
    pub bot_fill_target: Option<usize>,
    pub pending_bots: Vec<(String, String, f32)>,
}

/// Stable identity for ban tracking: prefer the account id, fall back to name.
fn ban_identity(database_account_id: &Option<String>, name: &str) -> String {
    database_account_id
        .clone()
        .unwrap_or_else(|| format!("name:{name}"))
}

/// Server-authoritative display-name normalization.  Names are presentation
/// data only; they are never used as the player's account identity.
pub fn normalize_player_name(input: &str) -> String {
    let name: String = input
        .trim()
        .chars()
        .filter(|character| !character.is_control())
        .take(16)
        .collect();
    if name.is_empty() {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            % 1000;
        format!("ANON{suffix:03}")
    } else {
        name
    }
}

impl ServerLobby {
    pub fn joinable(&self) -> bool {
        matches!(self.phase, LobbyPhase::Waiting | LobbyPhase::CountingDown)
    }
}

pub struct SpawnLobbyOpts {
    pub game_mode: String,
    pub kind: LobbyKind,
    pub is_private: bool,
    pub config_override: Option<GameConfig>,
    pub password: Option<String>,
    pub host_name: String,
}

fn spawn_waiting_lobby(games: &mut Vec<ServerLobby>, next_id: &mut u64, opts: SpawnLobbyOpts) {
    let game_mode = opts.game_mode;
    let kind = opts.kind;
    let is_private = opts.is_private;
    let config_override = opts.config_override;
    let password = opts.password;
    let host_name = opts.host_name;
    let id = *next_id;
    *next_id += 1;
    let mut config = if let Some(mut c) = config_override {
        // Resolve map dimensions from catalog when host provides a config.
        // Capacity is derived from the chosen map (OpenFront model) — the host
        // picks the map, the map picks the lobby size.
        if let Some(entry) = crate::map_catalog::lookup(&c.map_name) {
            c.map_width = entry.width;
            c.map_height = entry.height;
            c.map_name = entry.key.clone();
            let mut rng = rand::thread_rng();
            c.max_players = crate::map_playlist::lobby_max_players(&entry, &game_mode, &mut rng);
        }
        c
    } else {
        // Matchmaking: draw the next map from the mode's weighted playlist and
        // derive the lobby capacity from it.
        let mut c = GameConfig::default();
        let pool = crate::map_catalog::entries();
        if let Some(key) = crate::map_playlist::next_map_for_mode(&game_mode, pool) {
            if let Some(entry) = crate::map_catalog::lookup(&key) {
                c.map_name = entry.key.clone();
                c.map_width = entry.width;
                c.map_height = entry.height;
                let mut rng = rand::thread_rng();
                c.max_players =
                    crate::map_playlist::lobby_max_players(&entry, &game_mode, &mut rng);
            }
        } else {
            log::error!("spawn_waiting_lobby: no maps in catalog for lobby {}", id);
        }
        c
    };

    config.game_mode = game_mode.to_string();

    log::info!(
        "[LOBBY] Spawned lobby {} kind={:?} mode={} map={} private={}",
        id,
        kind,
        game_mode,
        config.map_name,
        is_private
    );

    games.push(ServerLobby {
        id,
        kind,
        is_private,
        phase: LobbyPhase::Waiting,
        countdown_secs: 0.0,
        active_empty_secs: 0.0,
        players: Vec::new(),
        ready_players: std::collections::HashSet::new(),
        seed: 0,
        config,
        game_mode: game_mode.to_string(),
        relay_port: None,
        host_player_id: None,
        password,
        host_name,
        banned: std::collections::HashSet::new(),
        bot_fill_target: None,
        pending_bots: Vec::new(),
    });
}

/// Matchmaking queue depth: exactly ONE joinable lobby at a time. When it
/// launches (or dies), the next one spawns and rotates to the next game mode,
/// so over time a single slot cycles through FFA / Teams with a fresh map and
/// derived capacity each cycle.
fn ensure_queue_depth(games: &mut Vec<ServerLobby>, next_id: &mut u64) {
    let has_joinable = games
        .iter()
        .any(|g| g.joinable() && g.kind == LobbyKind::Matchmaking);
    if has_joinable {
        return;
    }
    static ROTATION: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    const MODES: &[&str] = &["FFA", "Teams", "HumansVsNations"];
    let idx = ROTATION.fetch_add(1, std::sync::atomic::Ordering::SeqCst) % MODES.len();
    spawn_waiting_lobby(
        games,
        next_id,
        SpawnLobbyOpts {
            game_mode: MODES[idx].to_string(),
            kind: LobbyKind::Matchmaking,
            is_private: false,
            config_override: None,
            password: None,
            host_name: String::new(),
        },
    );
}

fn promote_countdown(games: &mut [ServerLobby]) {
    // Custom lobbies are NEVER auto-promoted — they start only when the host calls force_start().
    // Both passes below are strictly Matchmaking-only.

    // Pass 1: any non-empty Matchmaking waiting lobby starts immediately, unblocked.
    for g in games.iter_mut() {
        if matches!(g.phase, LobbyPhase::Waiting)
            && g.kind == LobbyKind::Matchmaking
            && !g.players.is_empty()
        {
            g.phase = LobbyPhase::CountingDown;
            g.countdown_secs = LOBBY_COUNTDOWN_SECS;
            log::info!("[LOBBY] {} Matchmaking→CountingDown (has players)", g.id);
        }
    }

    // Pass 2: promote the single empty Matchmaking waiting lobby to a beacon
    // so clients see a live timer. With one-lobby rotation there's at most one.
    let has_beacon = games.iter().any(|g| {
        matches!(g.phase, LobbyPhase::CountingDown)
            && g.kind == LobbyKind::Matchmaking
            && g.players.is_empty()
    });
    if !has_beacon
        && let Some(lobby) = games
            .iter_mut()
            .find(|g| matches!(g.phase, LobbyPhase::Waiting) && g.kind == LobbyKind::Matchmaking)
    {
        lobby.phase = LobbyPhase::CountingDown;
        lobby.countdown_secs = LOBBY_COUNTDOWN_SECS;
        log::info!(
            "[LOBBY] {} Matchmaking→CountingDown ({} beacon)",
            lobby.id,
            lobby.game_mode
        );
    }

    // Pass 3: drip fictional humans into Matchmaking lobbies.
    // Ghost players (is_internal_bot=true) — each client's engine auto-plays them
    // via execute_ai_think. Zero sockets, zero relay overhead.
    crate::bot_fill::inject_internal_bots(games);
}

pub fn primary_lobby_id(games: &[ServerLobby], game_mode: &str) -> Option<u64> {
    let mut joinable_lobbies: Vec<u64> = games
        .iter()
        .filter(|g| {
            g.joinable()
                && g.kind == LobbyKind::Matchmaking
                && g.game_mode == game_mode
                && g.players.len() < g.config.max_players as usize
        })
        .map(|g| g.id)
        .collect();
    if joinable_lobbies.is_empty() {
        return None;
    }
    joinable_lobbies.sort_unstable();
    Some(joinable_lobbies[0])
}

/// Returns the active server-spawned queue lobby regardless of its rotating mode.
/// Quick Match must follow the queue rotation instead of forcing every player into FFA.
fn primary_matchmaking_lobby_id(games: &[ServerLobby]) -> Option<u64> {
    games
        .iter()
        .filter(|g| {
            g.joinable()
                && g.kind == LobbyKind::Matchmaking
                && g.players.len() < g.config.max_players as usize
        })
        .min_by_key(|g| g.id)
        .map(|g| g.id)
}

fn resolve_join_target(requested: Option<u64>, games: &[ServerLobby]) -> Option<u64> {
    if let Some(id) = requested {
        if games
            .iter()
            .any(|g| g.id == id && g.joinable() && g.players.len() < g.config.max_players as usize)
        {
            return Some(id);
        }
        return None;
    }
    primary_matchmaking_lobby_id(games)
}

pub struct JoinPlayerOpts {
    pub name: String,
    pub clan_tag: String,
    pub civilization: sow_core::player::Civilization,
    pub leader: sow_core::player::Leader,
    pub client_tx: mpsc::Sender<Vec<u8>>,
    pub target_lobby_id: Option<u64>,
    pub host_private: bool,
    pub database_account_id: Option<String>,
    pub host_config: Option<Box<GameConfig>>,
    pub password: Option<String>,
    pub ip: String,
    pub session_id: Option<u64>,
}

pub fn join_player(
    games: &mut Vec<ServerLobby>,
    next_id: &mut u64,
    opts: JoinPlayerOpts,
) -> Result<(u64, u16, String, bool), String> {
    let name = normalize_player_name(&opts.name);
    let clan_tag = opts.clan_tag;
    let civilization = opts.civilization;
    let leader = opts.leader;
    let client_tx = opts.client_tx;
    let target_lobby_id = opts.target_lobby_id;
    let host_private = opts.host_private;
    let database_account_id = opts.database_account_id;
    let host_config = opts.host_config;
    let password = opts.password;
    let ip = opts.ip;
    let session_id = opts.session_id;
    let mut is_new_host = false;
    let lobby_id = if host_private {
        if target_lobby_id.is_some() {
            log::warn!(
                "[JOIN] {} tried to host private room with a target_lobby_id set — rejected",
                name
            );
            return Err("Cannot host private room with a target lobby".to_string());
        }
        let game_mode = host_config
            .as_ref()
            .map(|c| c.game_mode.clone())
            .unwrap_or_else(|| "FFA".to_string());
        log::info!(
            "[JOIN] {} creating private Custom lobby mode={}",
            name,
            game_mode
        );
        spawn_waiting_lobby(
            games,
            next_id,
            SpawnLobbyOpts {
                game_mode,
                kind: LobbyKind::Custom,
                is_private: true,
                config_override: host_config.map(|c| *c),
                password: password.clone(),
                host_name: name.clone(),
            },
        );
        is_new_host = true;
        games.last().unwrap().id
    } else if host_config.is_some() && target_lobby_id.is_none() {
        // Host-created public Custom lobby
        let config = host_config.unwrap();
        let game_mode = config.game_mode.clone();
        log::info!(
            "[JOIN] {} creating public Custom lobby mode={}",
            name,
            game_mode
        );
        spawn_waiting_lobby(
            games,
            next_id,
            SpawnLobbyOpts {
                game_mode,
                kind: LobbyKind::Custom,
                is_private: false,
                config_override: Some(*config),
                password: password.clone(),
                host_name: name.clone(),
            },
        );
        is_new_host = true;
        games.last().unwrap().id
    } else if let Some(req) = target_lobby_id {
        match resolve_join_target(Some(req), games) {
            Some(id) => id,
            None => {
                if req >= 100000000 {
                    // Rematch room doesn't exist yet, we must be the first to arrive! Create it.
                    log::info!("[JOIN] Creating rematch Custom lobby id={}", req);
                    spawn_waiting_lobby(
                        games,
                        next_id,
                        SpawnLobbyOpts {
                            game_mode: "FFA".to_string(),
                            kind: LobbyKind::Custom,
                            is_private: true,
                            config_override: None,
                            password: None,
                            host_name: String::new(),
                        },
                    );
                    let new_lobby = games.last_mut().unwrap();
                    new_lobby.id = req; // Override the ID to match the rematch ID
                    req
                } else {
                    // Explicit target (invite link / room join / browser click) that no
                    // longer resolves must NOT be silently rerouted into a fresh
                    // Matchmaking lobby — that fills the "new game" with ghosts while
                    // the player thinks they joined their friend's private room.
                    // Reject instead; the client shows a notice and stays in the menu.
                    log::warn!(
                        "[JOIN] Requested lobby {} unavailable — rejecting join from {} (target no longer joinable)",
                        req,
                        name
                    );
                    return Err("Lobby is not accepting joins".to_string());
                }
            }
        }
    } else {
        match resolve_join_target(None, games) {
            Some(id) => id,
            None => {
                log::info!(
                    "[JOIN] No Matchmaking lobby available for {}, spawning one",
                    name
                );
                spawn_waiting_lobby(
                    games,
                    next_id,
                    SpawnLobbyOpts {
                        game_mode: "FFA".to_string(),
                        kind: LobbyKind::Matchmaking,
                        is_private: false,
                        config_override: None,
                        password: None,
                        host_name: String::new(),
                    },
                );
                games.last().unwrap().id
            }
        }
    };

    let (is_joinable, is_full, is_matchmaking, game_mode) =
        match games.iter().find(|g| g.id == lobby_id) {
            Some(g) => (
                g.joinable(),
                g.players.len() >= g.config.max_players as usize,
                g.kind == LobbyKind::Matchmaking,
                g.game_mode.clone(),
            ),
            None => return Err("Lobby not found".to_string()),
        };

    if is_matchmaking && (!is_joinable || is_full) {
        log::info!(
            "[JOIN] Target lobby {} unavailable (joinable={}, full={}), directing {} to open lobby",
            lobby_id,
            is_joinable,
            is_full,
            name
        );
        let target_id = match primary_lobby_id(games, &game_mode) {
            Some(id) if id != lobby_id => id,
            _ => {
                spawn_waiting_lobby(
                    games,
                    next_id,
                    SpawnLobbyOpts {
                        game_mode: game_mode.clone(),
                        kind: LobbyKind::Matchmaking,
                        is_private: false,
                        config_override: None,
                        password: None,
                        host_name: String::new(),
                    },
                );
                games.last().unwrap().id
            }
        };
        let fallback_lobby = games.iter_mut().find(|g| g.id == target_id).unwrap();
        let player_id = fallback_lobby
            .players
            .iter()
            .map(|p| p.player_id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let team = if fallback_lobby.game_mode == "HumansVsNations" {
            // PvE: every human is on the Red team against AI nations (Blue).
            Some(Team::Red)
        } else if fallback_lobby.game_mode == "Teams" {
            let reds = fallback_lobby
                .players
                .iter()
                .filter(|p| p.team == Some(Team::Red))
                .count();
            let blues = fallback_lobby
                .players
                .iter()
                .filter(|p| p.team == Some(Team::Blue))
                .count();
            Some(if blues < reds { Team::Blue } else { Team::Red })
        } else {
            None
        };
        fallback_lobby.players.push(PlayerConnection {
            name,
            clan_tag,
            player_id,
            tx: client_tx,
            download_progress: 0,
            civilization,
            leader,
            database_account_id,
            team,
            ip,
            session_id,
            is_internal_bot: false,
        });
        return Ok((
            target_id,
            player_id,
            fallback_lobby.config.map_name.clone(),
            fallback_lobby.is_private,
        ));
    }

    let lobby = games.iter_mut().find(|g| g.id == lobby_id).unwrap();

    if !lobby.joinable() {
        log::warn!(
            "[JOIN] {} tried to join lobby {} which is no longer joinable (phase={:?})",
            name,
            lobby_id,
            lobby.phase
        );
        return Err("Lobby is not accepting joins".to_string());
    }
    if lobby
        .banned
        .contains(&ban_identity(&database_account_id, &name))
    {
        log::warn!(
            "[JOIN] {} is banned from lobby {} — rejected",
            name,
            lobby_id
        );
        return Err("BANNED".to_string());
    }
    if let Some(ref lobby_pw) = lobby.password.clone()
        && password.as_deref() != Some(lobby_pw.as_str())
    {
        log::warn!("[JOIN] {} gave wrong password for lobby {}", name, lobby_id);
        return Err("Wrong password".to_string());
    }
    let max = lobby.config.max_players as usize;

    // Reconnect matches by stable account id ONLY — never by display name.
    // Names are mutable presentation data and may collide; both sides must
    // carry a real Some(account_id), otherwise the player joins fresh.
    if let Some(existing_idx) = lobby.players.iter().position(|p| {
        matches!(
            (&p.database_account_id, &database_account_id),
            (Some(a), Some(b)) if a == b
        )
    }) {
        log::info!(
            "Player {} already in lobby {}, updating connection (reconnect)",
            name,
            lobby_id
        );
        let p = &mut lobby.players[existing_idx];
        p.tx = client_tx;
        p.clan_tag = clan_tag;
        p.civilization = civilization;
        p.leader = leader;
        p.ip = ip;
        let pid = p.player_id;
        return Ok((
            lobby_id,
            pid,
            lobby.config.map_name.clone(),
            lobby.is_private,
        ));
    }

    if lobby.players.len() >= max {
        log::warn!(
            "[JOIN] {} tried to join lobby {} but it is full ({}/{})",
            name,
            lobby_id,
            lobby.players.len(),
            max
        );
        return Err("Lobby is full".to_string());
    }

    // (Map is strictly server-assigned)

    let player_id = lobby
        .players
        .iter()
        .map(|p| p.player_id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);

    // Teams mode: drop the joiner into whichever team is smaller (Red on a tie).
    // HumansVsNations: every human joins Red (AI nations are Blue, spawned engine-side).
    let team = if lobby.game_mode == "HumansVsNations" {
        Some(Team::Red)
    } else if lobby.game_mode == "Teams" {
        let reds = lobby
            .players
            .iter()
            .filter(|p| p.team == Some(Team::Red))
            .count();
        let blues = lobby
            .players
            .iter()
            .filter(|p| p.team == Some(Team::Blue))
            .count();
        Some(if blues < reds { Team::Blue } else { Team::Red })
    } else {
        None
    };

    lobby.players.push(PlayerConnection {
        name,
        clan_tag,
        player_id,
        tx: client_tx,
        download_progress: 0,
        civilization,
        leader,
        database_account_id,
        team,
        ip,
        session_id,
        is_internal_bot: false,
    });

    if is_new_host {
        lobby.host_player_id = Some(player_id);
    }

    log::info!("Player {} joined lobby {}", player_id, lobby_id);
    Ok((
        lobby_id,
        player_id,
        lobby.config.map_name.clone(),
        lobby.is_private,
    ))
}

pub fn leave_player(games: &mut [ServerLobby], lobby_id: u64, player_id: u16) {
    if let Some(lobby) = games.iter_mut().find(|g| g.id == lobby_id) {
        let before = lobby.players.len();
        lobby.players.retain(|p| p.player_id != player_id);
        lobby.ready_players.remove(&player_id);
        if before != lobby.players.len() {
            log::info!("Player {} left lobby {}", player_id, lobby_id);
            // Lobbies in ReadyForRelay don't care, they are about to be dropped.
        }
    }
}

/// Serialize a `LobbyClosed` envelope for a lobby/reason, or `None` on failure.
fn lobby_closed_json(lobby_id: u64, reason: &str) -> Option<Vec<u8>> {
    let msg = sow_core::protocol::ServerLobbyClosedMessage {
        lobby_id,
        reason: reason.to_string(),
        rematch_lobby_id: None,
    };
    match bincode::serialize(&sow_core::protocol::ServerMessage::LobbyClosed(msg)) {
        Ok(json) => Some(json),
        Err(e) => {
            log::error!("[LOBBY] Failed to serialize LobbyClosed for {lobby_id}: {e}");
            None
        }
    }
}

/// Push a `LobbyClosed` to every member — used when the host abandons the lobby.
pub fn notify_lobby_closed(lobby: &ServerLobby, reason: &str) {
    if let Some(json) = lobby_closed_json(lobby.id, reason) {
        for p in &lobby.players {
            let _ = p.tx.try_send(json.clone());
        }
    }
}

/// True when `player_id` leaving should drop the whole Custom lobby: they are the
/// host and the match has not yet been handed off to the relay (poka-yoke — never
/// strand the remaining members behind a vanished host).
pub fn is_host_teardown(games: &[ServerLobby], lobby_id: u64, player_id: u16) -> bool {
    games.iter().any(|g| {
        g.id == lobby_id
            && g.kind == LobbyKind::Custom
            && g.host_player_id == Some(player_id)
            && g.phase != LobbyPhase::ReadyForRelay
    })
}

/// Host-initiated removal of a single player from a Custom lobby. When `ban` is set
/// the player's identity is recorded so they cannot rejoin this lobby. The target is
/// notified with a `LobbyClosed` (reason `KICKED`/`BANNED`) and the roster re-synced.
pub fn kick_player(
    games: &mut [ServerLobby],
    lobby_id: u64,
    requester_id: u16,
    target_id: u16,
    ban: bool,
) {
    let Some(lobby) = games.iter_mut().find(|g| g.id == lobby_id) else {
        return;
    };
    if lobby.kind != LobbyKind::Custom || lobby.host_player_id != Some(requester_id) {
        log::warn!(
            "[KICK] Player {requester_id} is not host of Custom lobby {lobby_id} — rejected"
        );
        return;
    }
    if target_id == requester_id {
        log::warn!("[KICK] Host {requester_id} tried to kick themselves from {lobby_id} — ignored");
        return;
    }
    let Some(target) = lobby.players.iter().find(|p| p.player_id == target_id) else {
        return;
    };
    if ban {
        let identity = ban_identity(&target.database_account_id, &target.name);
        lobby.banned.insert(identity);
    }
    let reason = if ban { "BANNED" } else { "KICKED" };
    if let Some(json) = lobby_closed_json(lobby_id, reason) {
        let _ = target.tx.try_send(json);
    }
    lobby.players.retain(|p| p.player_id != target_id);
    lobby.ready_players.remove(&target_id);
    log::info!("[KICK] Host {requester_id} {reason} player {target_id} from lobby {lobby_id}");
    sync_host_lobby_to_members(lobby);
}

/// Host toggles a player's team (Red↔Blue) in a Teams Custom lobby, then re-syncs
/// the roster so everyone sees the new assignment.
pub fn set_player_team(
    games: &mut [ServerLobby],
    lobby_id: u64,
    requester_id: u16,
    target_id: u16,
) {
    let Some(lobby) = games.iter_mut().find(|g| g.id == lobby_id) else {
        return;
    };
    if lobby.kind != LobbyKind::Custom || lobby.host_player_id != Some(requester_id) {
        log::warn!(
            "[TEAM] Player {requester_id} is not host of Custom lobby {lobby_id} — rejected"
        );
        return;
    }
    if lobby.game_mode != "Teams" {
        log::warn!(
            "[TEAM] Lobby {lobby_id} mode '{}' is not host-toggleable Teams — ignored",
            lobby.game_mode
        );
        return;
    }
    if let Some(target) = lobby.players.iter_mut().find(|p| p.player_id == target_id) {
        target.team = match target.team {
            Some(Team::Red) => Some(Team::Blue),
            _ => Some(Team::Red),
        };
        log::info!(
            "[TEAM] Host {requester_id} moved player {target_id} to {:?} in lobby {lobby_id}",
            target.team
        );
    }
    sync_host_lobby_to_members(lobby);
}

fn start_match(lobby: &mut ServerLobby) {
    lobby.phase = LobbyPhase::Loading;
    lobby.seed = rand::random();

    if let Some(entry) = crate::map_catalog::lookup(&lobby.config.map_name) {
        lobby.config.map_width = entry.width;
        lobby.config.map_height = entry.height;
        lobby.config.map_name = entry.key.clone();
    } else {
        log::error!(
            "Unknown map '{}' in catalog; using config defaults",
            lobby.config.map_name
        );
    }

    // We no longer build map state or SowEngine here. That is handled by sow-relay.

    lobby.countdown_secs = 16.5; // 15s load + 1.5s stabilize
    lobby.phase = LobbyPhase::Loading;

    lobby.active_empty_secs = 30.0;
    log::info!(
        "Lobby {} is Loading (seed {}, roster={} external={} internal_bots={})",
        lobby.id,
        lobby.seed,
        lobby.players.len(),
        lobby.players.iter().filter(|p| !p.is_internal_bot).count(),
        lobby.players.iter().filter(|p| p.is_internal_bot).count()
    );
}

pub fn force_start(games: &mut [ServerLobby], lobby_id: u64, player_id: u16) {
    let Some(lobby) = games.iter_mut().find(|g| g.id == lobby_id) else {
        log::warn!(
            "[FORCE_START] Lobby {} not found (player_id={})",
            lobby_id,
            player_id
        );
        return;
    };
    if lobby.kind != LobbyKind::Custom {
        log::warn!(
            "[FORCE_START] Player {} tried to force-start Matchmaking lobby {} — ignored",
            player_id,
            lobby_id
        );
        return;
    }
    if lobby.host_player_id != Some(player_id) {
        log::warn!(
            "[FORCE_START] Player {} is not host of lobby {} (host={:?}) — rejected",
            player_id,
            lobby_id,
            lobby.host_player_id
        );
        return;
    }
    if lobby.players.is_empty() {
        log::warn!(
            "[FORCE_START] Host {} tried to start empty lobby {} — ignored",
            player_id,
            lobby_id
        );
        return;
    }
    match lobby.phase {
        LobbyPhase::Waiting => {
            lobby.phase = LobbyPhase::CountingDown;
            lobby.countdown_secs = 3.0;
            log::info!(
                "[FORCE_START] Lobby {} started by host {} ({} players)",
                lobby_id,
                player_id,
                lobby.players.len()
            );
        }
        LobbyPhase::CountingDown if lobby.countdown_secs > 3.0 => {
            lobby.countdown_secs = 3.0;
            log::info!(
                "[FORCE_START] Lobby {} countdown snapped to 3s by host {}",
                lobby_id,
                player_id
            );
        }
        other => {
            log::info!(
                "[FORCE_START] Lobby {} already in phase {:?}, no-op",
                lobby_id,
                other
            );
        }
    }
}

pub fn master_tick(games: &mut Vec<ServerLobby>, next_id: &mut u64) {
    ensure_queue_depth(games, next_id);
    promote_countdown(games);

    for lobby in games.iter() {
        sync_host_lobby_to_members(lobby);
    }

    let mut i = 0;
    while i < games.len() {
        let remove = {
            let lobby = &mut games[i];
            match lobby.phase {
                LobbyPhase::Waiting => false,
                LobbyPhase::CountingDown => {
                    lobby.countdown_secs -= TICK_SECS;
                    let cap = lobby.config.max_players as usize;
                    if lobby.countdown_secs <= 0.0 || lobby.players.len() >= cap {
                        start_match(lobby);
                    }
                    false
                }
                LobbyPhase::Loading => {
                    lobby.countdown_secs -= TICK_SECS;

                    let players: Vec<sow_core::protocol::LobbyPlayerSyncState> = lobby
                        .players
                        .iter()
                        .map(|p| sow_core::protocol::LobbyPlayerSyncState {
                            name: p.name.clone(),
                            is_ready: lobby.ready_players.contains(&p.player_id),
                            download_progress: p.download_progress,
                            leader: p.leader,
                            player_id: p.player_id,
                            team: p.team,
                        })
                        .collect();

                    let all_ready = players.iter().all(|p| p.is_ready) && !players.is_empty();

                    // If everyone is ready, we force the countdown to jump to 1.5s if it was higher,
                    // serving as the "Stabilizing..." delay to allow the server to spawn the relay.
                    if all_ready && lobby.countdown_secs > 1.5 {
                        lobby.countdown_secs = 1.5;
                    }

                    let is_starting = all_ready;

                    let sync_msg = sow_core::protocol::ServerSyncStateMessage {
                        time_remaining: lobby.countdown_secs.max(0.0),
                        players,
                        is_starting,
                    };
                    match bincode::serialize(&sow_core::protocol::ServerMessage::SyncState(
                        sync_msg,
                    )) {
                        Ok(sync_json) => {
                            for p in &lobby.players {
                                if let Err(e) = p.tx.try_send(sync_json.clone()) {
                                    log::debug!(
                                        "[LOADING] SyncState send failed for player {} in lobby {}: {}",
                                        p.player_id,
                                        lobby.id,
                                        e
                                    );
                                }
                            }
                        }
                        Err(e) => {
                            log::error!(
                                "[LOADING] Failed to serialize SyncState for lobby {}: {}",
                                lobby.id,
                                e
                            );
                        }
                    }

                    if lobby.countdown_secs <= 0.0 {
                        if !all_ready {
                            log::warn!(
                                "Lobby {} loading phase timed out! Removing slow clients.",
                                lobby.id
                            );
                            let closed_msg = sow_core::protocol::ServerLobbyClosedMessage {
                                lobby_id: lobby.id,
                                reason: "Sync timeout. Requeueing...".to_string(),
                                rematch_lobby_id: None,
                            };
                            if let Ok(closed_json) = bincode::serialize(
                                &sow_core::protocol::ServerMessage::LobbyClosed(closed_msg),
                            ) {
                                lobby.players.retain(|p| {
                                    if lobby.ready_players.contains(&p.player_id) {
                                        true
                                    } else {
                                        let _ = p.tx.try_send(closed_json.clone());
                                        false
                                    }
                                });
                            } else {
                                log::error!(
                                    "[LOADING] Failed to serialize LobbyClosed for lobby {} — dropping all slow clients",
                                    lobby.id
                                );
                                lobby
                                    .players
                                    .retain(|p| lobby.ready_players.contains(&p.player_id));
                            }
                        } else {
                            log::info!(
                                "Lobby {} all clients ready, starting active match!",
                                lobby.id
                            );
                        }
                        let has_external_players = lobby.players.iter().any(|p| !p.is_internal_bot);
                        if !has_external_players {
                            log::warn!(
                                "[SERVER ORCHESTRATOR] Lobby {} aborted relay spawn: no validated external players remaining (they disconnected or failed map sync).",
                                lobby.id
                            );
                            // No external players left — drop the lobby so it doesn't waste a relay worker.
                            true
                        } else {
                            let external_players =
                                lobby.players.iter().filter(|p| !p.is_internal_bot).count();
                            log::info!(
                                "[SERVER ORCHESTRATOR] Lobby {} marked ReadyForRelay with {} validated external players ({} total roster).",
                                lobby.id,
                                external_players,
                                lobby.players.len()
                            );
                            lobby.phase = LobbyPhase::ReadyForRelay;
                            false
                        }
                    } else {
                        false
                    }
                }
                LobbyPhase::ReadyForRelay => {
                    // Handled async by the orchestrator main loop
                    false
                }
            }
        };

        if remove {
            games.remove(i);
            ensure_queue_depth(games, next_id);
            continue;
        }
        i += 1;
    }
}

pub fn lobby_to_info(g: &ServerLobby) -> LobbyInfo {
    LobbyInfo {
        id: g.id,
        kind: g.kind,
        num_players: g.players.len() as u32,
        max_players: g.config.max_players,
        is_counting_down: matches!(g.phase, LobbyPhase::CountingDown),
        timer_secs: if matches!(g.phase, LobbyPhase::CountingDown) {
            g.countdown_secs.max(0.0)
        } else {
            0.0
        },
        map_name: g.config.map_name.clone(),
        game_mode: g.game_mode.clone(),
        players: g
            .players
            .iter()
            .map(|p| sow_core::protocol::LobbyPlayerSyncState {
                name: p.name.clone(),
                is_ready: g.ready_players.contains(&p.player_id),
                download_progress: p.download_progress,
                leader: p.leader,
                player_id: p.player_id,
                team: p.team,
            })
            .collect(),
        has_password: g.password.is_some(),
        host_name: g.host_name.clone(),
        bot_count: g.config.bot_count,
        nation_count: g.config.nation_count,
        bot_difficulty: g.config.bot_difficulty,
    }
}

/// All joinable lobbies (Matchmaking or Custom) push roster/timer updates to all members.
/// Called every tick so all players see joiners and countdown in real time.
pub fn sync_host_lobby_to_members(lobby: &ServerLobby) {
    if !lobby.joinable() {
        return;
    }
    let players: Vec<sow_core::protocol::LobbyPlayerSyncState> = lobby
        .players
        .iter()
        .map(|p| sow_core::protocol::LobbyPlayerSyncState {
            name: p.name.clone(),
            is_ready: lobby.ready_players.contains(&p.player_id),
            download_progress: p.download_progress,
            leader: p.leader,
            player_id: p.player_id,
            team: p.team,
        })
        .collect();
    let time_remaining = if matches!(lobby.phase, LobbyPhase::CountingDown) {
        lobby.countdown_secs.max(0.0)
    } else {
        0.0
    };
    let sync_msg = sow_core::protocol::ServerSyncStateMessage {
        time_remaining,
        players,
        is_starting: false,
    };
    match bincode::serialize(&sow_core::protocol::ServerMessage::SyncState(sync_msg)) {
        Ok(sync_json) => {
            for p in &lobby.players {
                if let Err(e) = p.tx.try_send(sync_json.clone()) {
                    log::debug!(
                        "[SYNC] Failed to send SyncState to player {} in lobby {}: {}",
                        p.player_id,
                        lobby.id,
                        e
                    );
                }
            }
        }
        Err(e) => {
            log::error!(
                "[SYNC] Failed to serialize SyncState for lobby {}: {}",
                lobby.id,
                e
            );
        }
    }
}

pub fn build_lobby_broadcast(games: &[ServerLobby]) -> Vec<LobbyInfo> {
    // Matchmaking lobbies → always broadcast (main menu quick-join).
    // Custom public lobbies → broadcast (Game Browser).
    // Custom private lobbies → NEVER broadcast (code/invite only).
    let mut infos: Vec<LobbyInfo> = games
        .iter()
        .filter(|g| {
            g.joinable()
                && match g.kind {
                    LobbyKind::Matchmaking => true,
                    LobbyKind::Custom => !g.is_private,
                }
        })
        .map(lobby_to_info)
        .collect();
    infos.sort_by_key(|l| l.id);
    infos
}

#[cfg(test)]
mod name_tests {
    use super::{
        JoinPlayerOpts, LobbyKind, LobbyPhase, PlayerConnection, ServerLobby,
        build_lobby_broadcast, join_player, kick_player, normalize_player_name,
        resolve_join_target, set_player_team,
    };
    use sow_core::game_config::GameConfig;
    use sow_core::player::{Civilization, Leader};
    use sow_core::protocol::Team;

    fn queue_lobby(id: u64, kind: LobbyKind, mode: &str) -> ServerLobby {
        ServerLobby {
            id,
            kind,
            is_private: false,
            phase: LobbyPhase::Waiting,
            countdown_secs: 0.0,
            active_empty_secs: 0.0,
            players: Vec::new(),
            ready_players: std::collections::HashSet::new(),
            seed: 0,
            config: GameConfig {
                game_mode: mode.to_string(),
                max_players: 8,
                ..Default::default()
            },
            game_mode: mode.to_string(),
            relay_port: None,
            host_player_id: None,
            password: None,
            host_name: String::new(),
            banned: std::collections::HashSet::new(),
            bot_fill_target: None,
            pending_bots: Vec::new(),
        }
    }

    #[test]
    fn names_are_bounded_and_control_free() {
        assert_eq!(normalize_player_name("  A\nB  "), "AB");
        assert_eq!(
            normalize_player_name("0123456789abcdefghi"),
            "0123456789abcdef"
        );
        assert!(normalize_player_name("\n\t").starts_with("ANON"));
    }

    #[test]
    fn quick_match_follows_rotating_matchmaking_mode() {
        let games = vec![queue_lobby(10, LobbyKind::Matchmaking, "Teams")];
        assert_eq!(resolve_join_target(None, &games), Some(10));
    }

    #[test]
    fn quick_match_does_not_select_custom_lobbies() {
        let games = vec![queue_lobby(10, LobbyKind::Custom, "FFA")];
        assert_eq!(resolve_join_target(None, &games), None);
    }

    #[test]
    fn public_passworded_custom_lobbies_are_broadcast_without_the_secret() {
        let mut public = queue_lobby(10, LobbyKind::Custom, "FFA");
        public.password = Some("hidden".to_string());
        let mut private = queue_lobby(11, LobbyKind::Custom, "Teams");
        private.is_private = true;
        private.password = Some("also-hidden".to_string());

        let infos = build_lobby_broadcast(&[public, private]);
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].id, 10);
        assert!(infos[0].has_password);
    }

    #[test]
    fn custom_lobby_password_is_checked_before_joining() {
        let mut lobby = queue_lobby(10, LobbyKind::Custom, "FFA");
        lobby.password = Some("correct".to_string());
        let mut games = vec![lobby];
        let (client_tx, _client_rx) = tokio::sync::mpsc::channel(1);

        let result = join_player(
            &mut games,
            &mut 20,
            JoinPlayerOpts {
                name: "Commander".to_string(),
                clan_tag: String::new(),
                civilization: sow_core::player::Civilization::Rome,
                leader: sow_core::player::Leader::Caesar,
                client_tx,
                target_lobby_id: Some(10),
                host_private: false,
                database_account_id: None,
                host_config: None,
                password: Some("wrong".to_string()),
                ip: "127.0.0.1".to_string(),
                session_id: Some(1),
            },
        );

        assert_eq!(result.unwrap_err(), "Wrong password");
        assert!(games[0].players.is_empty());
    }

    fn lobby_player(
        player_id: u16,
        tx: tokio::sync::mpsc::Sender<Vec<u8>>,
        team: Option<Team>,
    ) -> PlayerConnection {
        PlayerConnection {
            name: format!("Player{player_id}"),
            clan_tag: String::new(),
            player_id,
            tx,
            download_progress: 0,
            civilization: Civilization::Rome,
            leader: Leader::Caesar,
            database_account_id: None,
            team,
            ip: "127.0.0.1".to_string(),
            session_id: Some(player_id as u64),
            is_internal_bot: false,
        }
    }

    #[test]
    fn custom_lobby_moderation_is_host_only() {
        let (host_tx, _host_rx) = tokio::sync::mpsc::channel(2);
        let (target_tx, mut target_rx) = tokio::sync::mpsc::channel(2);
        let mut lobby = queue_lobby(10, LobbyKind::Custom, "FFA");
        lobby.host_player_id = Some(1);
        lobby.players = vec![
            lobby_player(1, host_tx, None),
            lobby_player(2, target_tx, None),
        ];
        let mut games = vec![lobby];

        kick_player(&mut games, 10, 2, 1, false);
        assert_eq!(games[0].players.len(), 2);

        kick_player(&mut games, 10, 1, 2, true);
        assert_eq!(games[0].players.len(), 1);
        assert!(target_rx.try_recv().is_ok());
        assert!(games[0].banned.contains("name:Player2"));
    }

    #[test]
    fn teams_host_can_toggle_a_player_team() {
        let (host_tx, _host_rx) = tokio::sync::mpsc::channel(2);
        let (target_tx, _target_rx) = tokio::sync::mpsc::channel(2);
        let mut lobby = queue_lobby(10, LobbyKind::Custom, "Teams");
        lobby.host_player_id = Some(1);
        lobby.players = vec![
            lobby_player(1, host_tx, Some(Team::Red)),
            lobby_player(2, target_tx, Some(Team::Red)),
        ];
        let mut games = vec![lobby];

        set_player_team(&mut games, 10, 1, 2);
        assert_eq!(games[0].players[1].team, Some(Team::Blue));
    }
}
