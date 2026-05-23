//! Shadows of War networking protocol — Turn-Relay model.
//!
//! Pure data models for intents and messages.

use crate::game::BuildingKind;
use serde::{Deserialize, Serialize};

// ─── Intents (Client → Server) ─────────────────────────────────────────────

/// A player wants to attack/expand toward a tile owner.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AttackIntent {
    pub target_owner: u16,
    pub troops: Option<f64>,
}

/// Attack or cancel — both are stamped and replayed in `Turn` for determinism.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum GameplayIntent {
    Attack(AttackIntent),
    CancelAttack {
        attack_id: u64,
    },
    Spawn {
        x: u32,
        y: u32,
    },
    /// Water transport: raw clicked tile (`y * width + x`); sim snaps to nearest owned shoreline.
    LaunchFleet {
        target_tile: u32,
        troops: Option<f64>,
    },
    RecallFleet {
        fleet_id: u64,
    },
    /// `target_tile` is the clicked tile index (`y * width + x`);
    BuildStructure {
        kind: BuildingKind,
        target_tile: u32,
    },
    BuildShip {
        port_tile: u32,
        kind: crate::game::UnitType,
    },
    MoveWarships {
        unit_ids: Vec<u64>,
        target_tile: u32,
    },
    /// pays gold cost again and increments `level`.
    UpgradeStructure {
        building_id: u64,
    },
    /// Informs the engine that the player has disconnected or resigned.
    Resign,
    MarkDisconnected {
        is_disconnected: bool,
    },
    ExpressEmoji {
        emoji: String,
        pinned: bool,
    },
    ProposeAlliance {
        target_player: crate::player::PlayerId,
    },
    AcceptAlliance {
        target_player: crate::player::PlayerId,
    },
    RejectAlliance {
        target_player: crate::player::PlayerId,
    },
    BreakAlliance {
        target_player: crate::player::PlayerId,
    },
    SendResources {
        target_player: crate::player::PlayerId,
        gold: f64,
        troops: f64,
    },
    LaunchNuke {
        kind: crate::game::NukeKind,
        target_tile: u32,
    },
}

/// Stamped intent bundled into a turn (attack or cancel).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct StampedIntent {
    pub player_id: u16,
    pub intent: GameplayIntent,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Turn {
    pub turn_number: u64,
    pub intents: Vec<StampedIntent>,
}
/// Envelope for all client → server messages (bincode-safe: has a discriminant).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ClientMessage {
    Join {
        name: String,
        is_observer: bool,
        target_lobby_id: Option<u64>,
        build_version: String,
    },
    Gameplay {
        intent: GameplayIntent,
    },
    MapDownloadProgress {
        lobby_id: u64,
        player_id: u16,
        progress: u8,
    },
    Leave {},
    Ready {
        lobby_id: u64,
        player_id: u16,
    },
    Ping {
        client_time: f64,
    },
}

/// Envelope for all server → client messages (bincode-safe: has a discriminant).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum ServerMessage {
    LobbiesBroadcast(ServerLobbiesBroadcastMessage),
    JoinAck(ServerJoinAckMessage),
    JoinFailed(ServerJoinFailedMessage),
    LobbyClosed(ServerLobbyClosedMessage),
    Start(Box<ServerStartMessage>),
    Turn(ServerTurnMessage),
    SyncState(ServerSyncStateMessage),
    Pong { client_time: f64 },
    VersionUpdate { version: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LobbyInfo {
    pub id: u64,
    pub num_players: u32,
    pub max_players: u32,
    pub is_counting_down: bool,
    pub timer_secs: f32,
    pub map_name: String,
    pub map_md5: Option<String>,
    pub game_mode: String,
    pub players: Vec<LobbyPlayerSyncState>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ServerLobbiesBroadcastMessage {
    pub lobbies: Vec<LobbyInfo>,
}

/// Sent immediately after the server accepts a player into a queue lobby (Waiting / CountingDown).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ServerJoinAckMessage {
    pub lobby_id: u64,
    pub player_id: u16,
    pub map_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ServerJoinFailedMessage {
    pub reason: String,
}

/// Sent when a match lobby is torn down (GC, game over); client should return to browser.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ServerLobbyClosedMessage {
    pub lobby_id: u64,
    pub reason: String,
}

// ─── Server Messages (Server → Client) ─────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ServerStartMessage {
    pub config: crate::game_config::GameConfig,
    pub my_player_id: Option<u16>,
    pub lobby_id: Option<u64>,
    pub seed: u64,
    pub players: Vec<PlayerInfo>,
    pub missed_turns: Vec<Turn>,
    pub map_data: Option<Vec<u8>>, // currently unused (maps fetched via HTTP)
    pub relay_port: Option<u16>,
    pub nations: Option<Vec<crate::map_legacy::Nation>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default)]
pub struct TurnLog {
    pub turns: Vec<Turn>,
    pub current: u64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ReplayFile {
    pub config: crate::game_config::GameConfig,
    pub seed: u64,
    pub players: Vec<PlayerInfo>,
    pub turns: Vec<Turn>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Copy)]
pub enum Team {
    Red,
    Blue,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PlayerInfo {
    pub id: u16,
    pub name: String,
    pub player_type: crate::player::PlayerType,
    pub color: [f32; 3],
    pub team: Option<Team>,
    pub spawn_x: u32,
    pub spawn_y: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ServerTurnMessage {
    pub turn: Turn,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LobbyPlayerSyncState {
    pub name: String,
    pub is_ready: bool,
    pub download_progress: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ServerSyncStateMessage {
    pub time_remaining: f32,
    pub players: Vec<LobbyPlayerSyncState>,
    pub is_starting: bool,
}

// ─── SimBridge Protocol (Main Thread ↔ Sim Thread) ──────────────────────────

/// Command sent from the main (render) thread to the simulation thread.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum SimCommand {
    /// Initialize the engine with map data and start config.
    Init {
        config: Box<crate::game_config::GameConfig>,
        seed: u64,
        map_bytes: Vec<u8>,
        players: Vec<PlayerInfo>,
        nations: Option<Vec<crate::map_legacy::Nation>>,
    },
    /// Apply a server turn (network intents + tick).
    Turn(Turn),
    /// Shutdown the sim thread.
    Shutdown,
}

/// Lightweight per-player summary for HUD/nameplates (no heavy state).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PlayerSnapshot {
    pub id: u16,
    pub name: String,
    pub troops: f64,
    pub max_troops: f64,
    pub gold: f64,
    pub tile_count: u32,
    pub centroid_x: f32,
    pub centroid_y: f32,
    pub player_type: crate::player::PlayerType,
    pub color: [f32; 3],
    pub team: Option<Team>,
    pub has_spawned: bool,
    pub alive: bool,
    pub iq: u32,
    pub alliances: Vec<crate::player::PlayerId>,
    #[serde(default)]
    pub alliance_timers: std::collections::HashMap<crate::player::PlayerId, u32>,
    pub alliance_requests: Vec<crate::player::PlayerId>,
    pub disconnected: bool,
    pub active_emoji: Option<String>,
}

/// A single tile whose owner changed during a tick.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct DirtyTile {
    pub index: u32,
    pub new_owner: u16,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct BuildingSnapshot {
    pub id: u64,
    pub tile_idx: u32,
    pub owner_id: u16,
    pub kind: crate::game::BuildingKind,
    pub level: u8,
    pub under_construction: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FleetSnapshot {
    pub id: u64,
    pub owner_id: u16,
    pub unit_type: crate::game::UnitType,
    pub troops: f64,
    pub current_tile: u32,
    pub path: Vec<u32>,
    pub path_cursor: usize,
    pub retreating: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AttackSnapshot {
    pub id: u64,
    pub owner_id: u16,
    pub target_owner: u16,
    pub troops: f64,
    pub retreating: bool,
    /// Front-line centroid (world-space tile coords).
    pub front_cx: f32,
    pub front_cy: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct ProjectileSnapshot {
    pub id: u64,
    pub kind: crate::game::ProjectileKind,
    pub owner_id: u16,
    pub src_x: f32,
    pub src_y: f32,
    pub dst_x: f32,
    pub dst_y: f32,
    pub progress: f32,
}

/// Snapshot sent from the simulation thread to the main thread every tick.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SimSnapshot {
    pub tick: u64,
    pub phase: crate::game::GamePhase,
    pub spawn_timer_secs: Option<f32>,
    pub players: Vec<PlayerSnapshot>,
    pub dirty_tiles: Vec<DirtyTile>,
    pub fleets: Vec<FleetSnapshot>,
    pub attacks: Vec<AttackSnapshot>,
    pub buildings: Vec<BuildingSnapshot>,
    pub projectiles: Vec<ProjectileSnapshot>,
    pub winner: Option<u16>,
    pub defense_posts: Vec<u32>,
    pub defense_dirty: bool,
    pub total_land_tiles: u32,
    pub railroads: Vec<crate::building::railroad::Railroad>,
    pub debug_mem_info: String,
}
