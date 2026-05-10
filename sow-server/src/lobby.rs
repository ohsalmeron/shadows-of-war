//! OpenFront / Dark Rift style master lobby: dynamic queue, single countdown promotion,
//! broadcasts only joinable lobbies, Active GC when no humans remain.

use sow_core::engine::SowEngine;
use sow_core::game::{GamePhase, GameState};
use sow_core::game_config::GameConfig;
use sow_core::player::PlayerType;
use sow_core::protocol::{
    LobbyInfo, PlayerInfo, ServerLobbyClosedMessage, ServerStartMessage, ServerTurnMessage,
    StampedIntent, Turn,
};
use sow_core::water_components::WaterComponents;
use tokio::sync::mpsc;

pub const LOBBY_COUNTDOWN_SECS: f32 = 15.0;
pub const ACTIVE_EMPTY_SECS: f32 = 30.0;
pub const TICK_SECS: f32 = 0.1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyPhase {
    Waiting,
    CountingDown,
    Loading,
    Active,
}

pub struct PlayerConnection {
    pub name: String,
    pub player_id: u16,
    pub tx: mpsc::Sender<String>,
}

pub struct ServerLobby {
    pub id: u64,
    pub phase: LobbyPhase,
    /// Remaining seconds while CountingDown.
    pub countdown_secs: f32,
    /// Counts down while Active and there are zero humans in `players`.
    pub active_empty_secs: f32,
    pub players: Vec<PlayerConnection>,
    pub ready_players: std::collections::HashSet<u16>,
    pub engine: Option<SowEngine>,
    pub pending_intents: Vec<StampedIntent>,
    pub seed: u64,
    pub config: GameConfig,
}

impl ServerLobby {
    pub fn joinable(&self) -> bool {
        matches!(self.phase, LobbyPhase::Waiting | LobbyPhase::CountingDown)
    }
}

fn spawn_waiting_lobby(games: &mut Vec<ServerLobby>, next_id: &mut u64) {
    let id = *next_id;
    *next_id += 1;
    games.push(ServerLobby {
        id,
        phase: LobbyPhase::Waiting,
        countdown_secs: 0.0,
        active_empty_secs: 0.0,
        players: Vec::new(),
        ready_players: std::collections::HashSet::new(),
        engine: None,
        pending_intents: Vec::new(),
        seed: 0,
        config: GameConfig::default(),
    });
}

fn ensure_queue_depth(games: &mut Vec<ServerLobby>, next_id: &mut u64) {
    while games.iter().filter(|g| g.joinable()).count() < 1 {
        spawn_waiting_lobby(games, next_id);
    }
}

fn promote_countdown(games: &mut [ServerLobby]) {
    let has_counting = games
        .iter()
        .any(|g| matches!(g.phase, LobbyPhase::CountingDown));
    if has_counting {
        return;
    }
    if let Some(lobby) = games
        .iter_mut()
        .find(|g| matches!(g.phase, LobbyPhase::Waiting))
    {
        lobby.phase = LobbyPhase::CountingDown;
        lobby.countdown_secs = LOBBY_COUNTDOWN_SECS;
        log::info!("Lobby {} promoted to CountingDown", lobby.id);
    }
}

/// Prefer counting-down lobby with lowest id, else lowest waiting id (matches DR client `primary_lobby_for_browser`).
pub fn primary_lobby_id(games: &[ServerLobby]) -> Option<u64> {
    let mut counting: Vec<u64> = games
        .iter()
        .filter(|g| g.joinable() && matches!(g.phase, LobbyPhase::CountingDown))
        .map(|g| g.id)
        .collect();
    if !counting.is_empty() {
        counting.sort_unstable();
        return Some(counting[0]);
    }
    let mut waiting: Vec<u64> = games
        .iter()
        .filter(|g| g.joinable() && matches!(g.phase, LobbyPhase::Waiting))
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
        games
            .iter()
            .find(|g| g.id == id && g.joinable())
            .map(|g| g.id)
    } else {
        primary_lobby_id(games)
    }
}

pub fn join_player(
    games: &mut Vec<ServerLobby>,
    name: String,
    client_tx: mpsc::Sender<String>,
    target_lobby_id: Option<u64>,
    preferred_map: Option<String>,
) -> Result<(u64, u16, String), String> {
    let lobby_id = resolve_join_target(target_lobby_id, games).ok_or_else(|| {
        "No joinable lobby available (try again)".to_string()
    })?;
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

    if lobby.players.is_empty() {
        if let Some(map_name) = preferred_map {
            lobby.config.map_name = map_name;
        }
    }

    let player_id = lobby
        .players
        .iter()
        .map(|p| p.player_id)
        .max()
        .unwrap_or(0)
        .saturating_add(1);

    lobby.players.push(PlayerConnection {
        name,
        player_id,
        tx: client_tx,
    });

    log::info!("Player {} joined lobby {}", player_id, lobby_id);
    Ok((lobby_id, player_id, lobby.config.map_name.clone()))
}

pub fn leave_player(games: &mut Vec<ServerLobby>, lobby_id: u64, player_id: u16) {
    if let Some(lobby) = games.iter_mut().find(|g| g.id == lobby_id) {
        let before = lobby.players.len();
        lobby.players.retain(|p| p.player_id != player_id);
        if before != lobby.players.len() {
            log::info!("Player {} left lobby {}", player_id, lobby_id);
            if lobby.phase == LobbyPhase::Active && lobby.players.is_empty() {
                lobby.active_empty_secs = ACTIVE_EMPTY_SECS;
            }
        }
    }
}

fn start_match(lobby: &mut ServerLobby) {
    lobby.phase = LobbyPhase::Loading;
    lobby.ready_players.clear();
    lobby.seed = rand::random();

    let root = std::env::var("SOW_MAPS_ROOT").unwrap_or_else(|_| "../OpenFrontIO/resources/maps".to_string());
    let map_dir = std::path::Path::new(&root).join(&lobby.config.map_name);
    let manifest_path = map_dir.join("manifest.json");
    let bin_path = map_dir.join("map.bin");

    let mut map_bytes = None;
    if let (Ok(m_data), Ok(b_data)) = (std::fs::read_to_string(&manifest_path), std::fs::read(&bin_path)) {
        if let Ok(manifest) = serde_json::from_str::<sow_core::map_openfront::MapManifest>(&m_data) {
            lobby.config.map_width = manifest.map.width;
            lobby.config.map_height = manifest.map.height;
            map_bytes = Some(b_data);
        } else {
            log::error!("Failed to parse map manifest at {:?}", manifest_path);
        }
    } else {
        log::warn!("Could not load map {:?}, falling back to defaults", map_dir);
        lobby.config.map_width = 800;
        lobby.config.map_height = 600;
    }

    let mut state = GameState::new(lobby.seed, lobby.config.map_width, lobby.config.map_height, lobby.config.clone());
    
    if let Some(ref bytes) = map_bytes {
        if bytes.len() == state.map.terrain.len() {
            let dest_ptr = state.map.terrain.as_mut_ptr() as *mut u8;
            unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), dest_ptr, bytes.len()); }
        } else {
            for (i, &b) in bytes.iter().enumerate() {
                if i < state.map.terrain.len() {
                    state.map.terrain[i] = sow_core::map::MapTile::from_byte(b);
                }
            }
        }
    }

    let water = WaterComponents::compute(&state.map);
    let mut engine = SowEngine::new(state, water);

    let mut player_infos: Vec<PlayerInfo> = Vec::new();
    for p in &lobby.players {
        engine.spawn_human(p.player_id, p.name.clone(), [1.0, 0.0, 0.0]);
        let (mut sx, mut sy) = (0, 0);
        if let Some(player) = engine.state.player(p.player_id) {
            if player.tile_count > 0 {
                sx = (player.sum_x / player.tile_count as u64) as u32;
                sy = (player.sum_y / player.tile_count as u64) as u32;
            }
        }
        player_infos.push(PlayerInfo {
            id: p.player_id,
            name: p.name.clone(),
            player_type: PlayerType::Human,
            color: [1.0, 0.0, 0.0],
            spawn_x: sx,
            spawn_y: sy,
        });
    }
    engine.spawn_ai(lobby.config.nation_count, lobby.config.bot_count);
    lobby.engine = Some(engine);
    lobby.countdown_secs = 10.0; // Max 10 seconds wait for clients to load
    lobby.phase = LobbyPhase::Loading;

    lobby.active_empty_secs = if lobby.players.is_empty() {
        ACTIVE_EMPTY_SECS
    } else {
        0.0
    };

    log::info!(
        "Lobby {} is Active (seed {}, {} humans)",
        lobby.id,
        lobby.seed,
        lobby.players.len()
    );

    for p in &lobby.players {
        let start_msg = ServerStartMessage {
            config: lobby.config.clone(),
            my_player_id: Some(p.player_id),
            seed: lobby.seed,
            players: player_infos.clone(),
            missed_turns: vec![],
            map_data: None, // clients load locally via config.map_name
        };
        let json = serde_json::to_string(&start_msg).expect("serialize ServerStartMessage");
        let _ = p.tx.try_send(json);
    }
}

fn close_lobby(lobby: &ServerLobby, reason: &str) {
    let msg = ServerLobbyClosedMessage {
        lobby_id: lobby.id,
        reason: reason.to_string(),
    };
    let json = serde_json::to_string(&msg).expect("serialize ServerLobbyClosedMessage");
    for p in &lobby.players {
        let _ = p.tx.try_send(json.clone());
    }
}

fn tick_active(lobby: &mut ServerLobby) -> bool {
    let humans = lobby.players.len();
    if humans == 0 {
        lobby.active_empty_secs -= TICK_SECS;
        if lobby.active_empty_secs <= 0.0 {
            close_lobby(lobby, "empty_timeout");
            return true;
        }
    } else {
        lobby.active_empty_secs = 0.0;
    }

    let Some(engine) = lobby.engine.as_mut() else {
        return false;
    };

    let turn = Turn {
        turn_number: engine.state.tick,
        intents: lobby.pending_intents.clone(),
    };
    lobby.pending_intents.clear();

    for intent in &turn.intents {
        engine.apply_stamped_intent(intent, 0);
    }
    engine.tick();

    if engine.state.phase == GamePhase::GameOver {
        close_lobby(lobby, "game_over");
        return true;
    }

    let msg = ServerTurnMessage { turn };
    let json = serde_json::to_string(&msg).expect("serialize ServerTurnMessage");
    for p in &lobby.players {
        let _ = p.tx.try_send(json.clone());
    }

    false
}

pub fn master_tick(games: &mut Vec<ServerLobby>, next_id: &mut u64) {
    ensure_queue_depth(games, next_id);
    promote_countdown(games);

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
                    if lobby.countdown_secs <= 0.0 || (lobby.ready_players.len() >= lobby.players.len() && !lobby.players.is_empty()) {
                        if lobby.countdown_secs <= 0.0 && lobby.ready_players.len() < lobby.players.len() {
                            log::warn!("Lobby {} loading phase timed out! Starting match anyway to let slow clients catch up.", lobby.id);
                        } else {
                            log::info!("Lobby {} all clients ready, starting active match!", lobby.id);
                        }
                        lobby.phase = LobbyPhase::Active;
                    }
                    false
                }
                LobbyPhase::Active => tick_active(lobby),
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

pub fn build_lobby_broadcast(games: &[ServerLobby]) -> Vec<LobbyInfo> {
    let mut infos: Vec<LobbyInfo> = games
        .iter()
        .filter(|g| g.joinable())
        .map(|g| LobbyInfo {
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
            player_names: g.players.iter().map(|p| p.name.clone()).collect(),
        })
        .collect();
    infos.sort_by_key(|l| l.id);
    infos
}
