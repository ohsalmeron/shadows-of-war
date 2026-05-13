//! Shadows of War networking protocol — Turn-Relay model.
//!
//! Pure data models for intents and messages.

use serde::{Deserialize, Serialize};
use crate::game::BuildingKind;

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
    CancelAttack { attack_id: u64 },
    Spawn { x: u32, y: u32 },
    /// Water transport: raw clicked tile (`y * width + x`); sim snaps to nearest owned shoreline.
    LaunchFleet {
        target_tile: u32,
        troops: Option<f64>,
    },
    RecallFleet { fleet_id: u64 },
    /// `target_tile` is the clicked tile index (`y * width + x`);
    BuildStructure {
        kind: BuildingKind,
        target_tile: u32,
    },
    /// pays gold cost again and increments `level`.
    UpgradeStructure { building_id: u64 },
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
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum ClientMessage {
    Join {
        name: String,
        is_observer: bool,
        target_lobby_id: Option<u64>,
        preferred_map: Option<String>,
    },
    Gameplay {
        intent: GameplayIntent,
    },
    Leave {},
    Ready {
        lobby_id: u64,
        player_id: u16,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LobbyInfo {
    pub id: u64,
    pub num_players: u32,
    pub max_players: u32,
    pub is_counting_down: bool,
    pub timer_secs: f32,
    pub map_name: String,
    pub player_names: Vec<String>,
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
    pub seed: u64,
    pub players: Vec<PlayerInfo>,
    pub missed_turns: Vec<Turn>,
    pub map_data: Option<Vec<u8>>, // deflate compressed map.bin data
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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct PlayerInfo {
    pub id: u16,
    pub name: String,
    pub player_type: crate::player::PlayerType,
    pub color: [f32; 3],
    pub spawn_x: u32,
    pub spawn_y: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ServerTurnMessage {
    pub turn: Turn,
}


#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ServerSyncStateMessage {
    pub time_remaining: f32,
    pub players: Vec<String>,
    pub ready_players: Vec<String>,
    pub is_starting: bool,
}

