//! Master lobby: dynamic queue, single countdown promotion,
//! broadcasts only joinable lobbies, Active GC when no humans remain.

use sow_core::game_config::GameConfig;
use sow_core::protocol::LobbyInfo;
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
}

pub struct ServerLobby {
    pub id: u64,
    pub is_private: bool,
    pub phase: LobbyPhase,
    /// Remaining seconds while CountingDown.
    pub countdown_secs: f32,
    /// Counts down while Active and there are zero humans in `players`.
    pub active_empty_secs: f32,
    pub players: Vec<PlayerConnection>,
    pub ready_players: std::collections::HashSet<u16>,
    pub seed: u64,
    pub config: GameConfig,
    pub game_mode: String,
    pub relay_port: Option<u16>,
}

impl ServerLobby {
    pub fn joinable(&self) -> bool {
        matches!(self.phase, LobbyPhase::Waiting | LobbyPhase::CountingDown)
    }
}

fn spawn_waiting_lobby(
    games: &mut Vec<ServerLobby>,
    next_id: &mut u64,
    game_mode: &str,
    is_private: bool,
) {
    let id = *next_id;
    *next_id += 1;
    let mut config = GameConfig::default();
    static NEXT_MAP_INDEX: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let map_idx = NEXT_MAP_INDEX.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let pool = crate::map_catalog::entries();
    if let Some(entry) = sow_core::maps::catalog_entry_at(pool, map_idx) {
        config.map_name = entry.key.clone();
        config.map_width = entry.width;
        config.map_height = entry.height;
    } else {
        log::error!("No maps in catalog; using default config map_name");
    }

    config.game_mode = game_mode.to_string();

    games.push(ServerLobby {
        id,
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
    });
}

fn ensure_queue_depth(games: &mut Vec<ServerLobby>, next_id: &mut u64) {
    if games
        .iter()
        .filter(|g| g.joinable() && !g.is_private && g.game_mode == "FFA")
        .count()
        < 1
    {
        spawn_waiting_lobby(games, next_id, "FFA", false);
    }
    if games
        .iter()
        .filter(|g| g.joinable() && !g.is_private && g.game_mode == "Teams")
        .count()
        < 1
    {
        spawn_waiting_lobby(games, next_id, "Teams", false);
    }
}

fn promote_countdown(games: &mut [ServerLobby]) {
    let has_counting = games
        .iter()
        .any(|g| matches!(g.phase, LobbyPhase::CountingDown));
    if has_counting {
        return;
    }

    // Pick the first waiting public lobby, or a private lobby with at least two players.
    let target = games.iter_mut().find(|g| {
        matches!(g.phase, LobbyPhase::Waiting) && (!g.is_private || g.players.len() >= 2)
    });

    if let Some(lobby) = target {
        lobby.phase = LobbyPhase::CountingDown;
        lobby.countdown_secs = LOBBY_COUNTDOWN_SECS;
        log::info!("Lobby {} promoted to CountingDown", lobby.id);
    }
}

/// Prefer counting-down lobby with lowest id, else lowest waiting id (matches DR client `primary_lobby_for_browser`).
pub fn primary_lobby_id(games: &[ServerLobby], game_mode: &str) -> Option<u64> {
    let mut counting: Vec<u64> = games
        .iter()
        .filter(|g| {
            g.joinable()
                && !g.is_private
                && g.game_mode == game_mode
                && matches!(g.phase, LobbyPhase::CountingDown)
        })
        .map(|g| g.id)
        .collect();
    if !counting.is_empty() {
        counting.sort_unstable();
        return Some(counting[0]);
    }
    let mut waiting: Vec<u64> = games
        .iter()
        .filter(|g| {
            g.joinable()
                && !g.is_private
                && g.game_mode == game_mode
                && matches!(g.phase, LobbyPhase::Waiting)
        })
        .map(|g| g.id)
        .collect();
    if waiting.is_empty() {
        return None;
    }
    waiting.sort_unstable();
    Some(waiting[0])
}

fn resolve_join_target(requested: Option<u64>, games: &[ServerLobby]) -> Option<u64> {
    if let Some(id) = requested {
        if games.iter().any(|g| g.id == id && g.joinable()) {
            return Some(id);
        }
        return None;
    }
    primary_lobby_id(games, "FFA")
}

#[allow(clippy::too_many_arguments)]
pub fn join_player(
    games: &mut Vec<ServerLobby>,
    next_id: &mut u64,
    name: String,
    clan_tag: String,
    civilization: sow_core::player::Civilization,
    leader: sow_core::player::Leader,
    client_tx: mpsc::Sender<Vec<u8>>,
    target_lobby_id: Option<u64>,
    host_private: bool,
) -> Result<(u64, u16, String, bool), String> {
    let lobby_id = if host_private {
        if target_lobby_id.is_some() {
            return Err("Cannot host private room with a target lobby".to_string());
        }
        spawn_waiting_lobby(games, next_id, "FFA", true);
        games.last().unwrap().id
    } else if let Some(req) = target_lobby_id {
        match resolve_join_target(Some(req), games) {
            Some(id) => id,
            None => {
                if req >= 100000000 {
                    // Rematch room doesn't exist yet, we must be the first to arrive! Create it.
                    log::info!("Creating rematch private lobby {}", req);
                    spawn_waiting_lobby(games, next_id, "FFA", true);
                    let new_lobby = games.last_mut().unwrap();
                    new_lobby.id = req; // Override the ID to match the rematch ID
                    req
                } else {
                    return Err("Lobby not found or not joinable".to_string());
                }
            }
        }
    } else {
        match resolve_join_target(None, games) {
            Some(id) => id,
            None => {
                spawn_waiting_lobby(games, next_id, "FFA", false);
                games.last().unwrap().id
            }
        }
    };

    let lobby = games
        .iter_mut()
        .find(|g| g.id == lobby_id)
        .expect("lobby must exist");

    if !lobby.joinable() {
        return Err("Lobby is not accepting joins".to_string());
    }
    let max = lobby.config.max_players as usize;
    if lobby.players.len() >= max {
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

    lobby.players.push(PlayerConnection {
        name,
        clan_tag,
        player_id,
        tx: client_tx,
        download_progress: 0,
        civilization,
        leader,
    });

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
        "Lobby {} is Loading (seed {}, {} humans)",
        lobby.id,
        lobby.seed,
        lobby.players.len()
    );
}

pub fn master_tick(games: &mut Vec<ServerLobby>, next_id: &mut u64) {
    ensure_queue_depth(games, next_id);
    promote_countdown(games);

    for lobby in games.iter() {
        sync_private_lobby_to_members(lobby);
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
                    let has_humans = !lobby.players.is_empty();
                    if (lobby.countdown_secs <= 0.0 || lobby.players.len() >= cap) && has_humans {
                        start_match(lobby);
                    } else if lobby.countdown_secs <= 0.0 && !has_humans {
                        // No human players, just reset the countdown
                        lobby.countdown_secs = LOBBY_COUNTDOWN_SECS;
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
                        })
                        .collect();

                    let all_ready = players.iter().all(|p| p.is_ready) && !players.is_empty();

                    // If everyone is ready, we force the countdown to jump to 1.5s if it was higher,
                    // serving as the "Stabilizing..." delay.
                    if all_ready && lobby.countdown_secs > 1.5 {
                        lobby.countdown_secs = 1.5;
                    }

                    let is_starting = all_ready;

                    let sync_msg = sow_core::protocol::ServerSyncStateMessage {
                        time_remaining: lobby.countdown_secs.max(0.0),
                        players,
                        is_starting,
                    };
                    let sync_json =
                        bincode::serialize(&sow_core::protocol::ServerMessage::SyncState(sync_msg))
                            .unwrap();
                    for p in &lobby.players {
                        let _ = p.tx.try_send(sync_json.clone());
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
                            let closed_json = bincode::serialize(
                                &sow_core::protocol::ServerMessage::LobbyClosed(closed_msg),
                            )
                            .unwrap();
                            lobby.players.retain(|p| {
                                if lobby.ready_players.contains(&p.player_id) {
                                    true
                                } else {
                                    let _ = p.tx.try_send(closed_json.clone());
                                    false
                                }
                            });
                        } else {
                            log::info!(
                                "Lobby {} all clients ready, starting active match!",
                                lobby.id
                            );
                        }
                        if lobby.players.is_empty() {
                            log::warn!("[SERVER ORCHESTRATOR] Lobby {} aborted relay spawn: No validated human players remaining (they disconnected or failed map sync).", lobby.id);
                            // If everyone dropped, just remove the lobby
                            true
                        } else {
                            log::info!("[SERVER ORCHESTRATOR] Lobby {} marked ReadyForRelay with {} validated human players.", lobby.id, lobby.players.len());
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
            })
            .collect(),
    }
}

/// Private lobbies are omitted from the global LobbiesBroadcast; push state to members directly.
pub fn sync_private_lobby_to_members(lobby: &ServerLobby) {
    if !lobby.is_private || !lobby.joinable() {
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
    let sync_json =
        bincode::serialize(&sow_core::protocol::ServerMessage::SyncState(sync_msg)).unwrap();
    for p in &lobby.players {
        let _ = p.tx.try_send(sync_json.clone());
    }
}

pub fn build_lobby_broadcast(games: &[ServerLobby]) -> Vec<LobbyInfo> {
    let mut infos: Vec<LobbyInfo> = games
        .iter()
        .filter(|g| g.joinable() && !g.is_private)
        .map(lobby_to_info)
        .collect();
    infos.sort_by_key(|l| l.id);
    infos
}
