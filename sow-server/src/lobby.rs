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
pub const BOT_COUNT: u32 = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyPhase {
    Waiting,
    CountingDown,
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
) -> Result<(u64, u16), String> {
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
    Ok((lobby_id, player_id))
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
    lobby.phase = LobbyPhase::Active;
    lobby.seed = rand::random();
    let config = GameConfig::default();
    lobby.config = config.clone();

    let state = GameState::new(lobby.seed, 800, 600, config);
    let water = WaterComponents::compute(&state.map);
    let mut engine = SowEngine::new(state, water);

    let mut player_infos: Vec<PlayerInfo> = Vec::new();
    for p in &lobby.players {
        engine.spawn_human(p.player_id, p.name.clone(), [1.0, 0.0, 0.0]);
        player_infos.push(PlayerInfo {
            id: p.player_id,
            name: p.name.clone(),
            player_type: PlayerType::Human,
            color: [1.0, 0.0, 0.0],
            spawn_x: 0,
            spawn_y: 0,
        });
    }
    engine.spawn_random_bots(BOT_COUNT);
    lobby.engine = Some(engine);

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
            map_data: None,
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
