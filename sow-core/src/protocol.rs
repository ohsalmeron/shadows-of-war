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
    UpgradeCityModule {
        building_id: u64,
        module: crate::building::ModuleKind,
    },
    UpgradeTile {
        tile_idx: u32,
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
    RequestResources {
        target_player: crate::player::PlayerId,
        gold: f64,
        troops: f64,
    },
    AcceptResourceRequest {
        target_player: crate::player::PlayerId,
    },
    RejectResourceRequest {
        target_player: crate::player::PlayerId,
    },
    LaunchNuke {
        kind: crate::game::NukeKind,
        target_tile: u32,
    },
    /// INK TIDE (Wave-Racer-style) boat controls — lockstep input for the
    /// racer. The relay treats this opaquely: stamps, stores, broadcasts.
    /// Only racer clients decode it. Appended last so existing variant
    /// indices (and SoW clients) are untouched.
    RacerControls(RacerControlsIntent),
}

/// Boat control frame for INK TIDE lockstep turns.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct RacerControlsIntent {
    pub steer: f64,
    pub throttle: f64,
    pub brake: f64,
    pub drift: bool,
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
        /// Host a new private friend room (CrazyGames instant multiplayer / play again).
        host_private: bool,
        build_version: String,
        clan_tag: String,
        civilization: crate::player::Civilization,
        leader: crate::player::Leader,
        /// Client-declared account/progress metadata for the roster (CrazyGames
        /// or a persistent bot account). The server may use it to correlate a
        /// lobby reconnect or ban, but it is not proof of identity or relay
        /// authentication; the direct relay connection requires a short-lived
        /// ticket.
        database_account_id: Option<String>,
        /// When creating a custom lobby (public or private), the host's desired config.
        #[serde(default)]
        host_config: Option<Box<crate::game_config::GameConfig>>,
        /// Password for joining a password-protected lobby, or setting one when creating.
        #[serde(default)]
        password: Option<String>,
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
    /// First relay frame for an authenticated game session. The unticketed
    /// `Ready` variant remains decodable for wire compatibility, but production
    /// relay workers reject it when `SOW_RELAY_TICKETS_REQUIRED=1`.
    ReadyWithTicket {
        lobby_id: u64,
        player_id: u16,
        ticket: String,
    },
    /// First frame on a relay reconnect. The relay rotates this capability
    /// after every accepted reconnect; it is never sent to the orchestrator.
    ReconnectWithTicket {
        lobby_id: u64,
        player_id: u16,
        ticket: String,
    },
    ForceStart {
        lobby_id: u64,
        player_id: u16,
    },
    /// Host removes a player from a Custom lobby — the player can rejoin afterwards.
    Kick {
        lobby_id: u64,
        target_player_id: u16,
    },
    /// Host removes a player from a Custom lobby and blocks them from rejoining it.
    Ban {
        lobby_id: u64,
        target_player_id: u16,
    },
    /// Host toggles a player's team (Red↔Blue) in a Teams Custom lobby.
    SetPlayerTeam {
        lobby_id: u64,
        target_player_id: u16,
    },
    Ping {
        client_time: f64,
    },
    RematchRequest {
        lobby_id: u64,
    },
    SubmitStats {
        kills: u32,
        deaths: u32,
        assists: u32,
        #[serde(default)]
        players_defeated: u32,
        #[serde(default)]
        empires_defeated: u32,
        #[serde(default)]
        tribes_defeated: u32,
    },
    /// Identity-proving join. A separate variant (not a new field on `Join`)
    /// keeps the legacy `Join` encoding decodable for cached/portal bundles:
    /// bincode is not self-describing, so an in-struct field would make every
    /// old client's join frame unparseable on a newer server.
    JoinWithAuth {
        join: Box<JoinPayload>,
        auth: AuthProof,
    },
    /// Stats submission carrying the leader used for authoritative mastery
    /// and reward accounting. Appended for bincode compatibility with older
    /// clients that still send `SubmitStats`.
    SubmitStatsWithLeader {
        kills: u32,
        deaths: u32,
        assists: u32,
        #[serde(default)]
        players_defeated: u32,
        #[serde(default)]
        empires_defeated: u32,
        #[serde(default)]
        tribes_defeated: u32,
        leader: String,
    },
    /// Final deterministic snapshot used to verify a durable match result.
    /// Appended to preserve decoding of older clients.
    SubmitMatchReport {
        kills: u32,
        deaths: u32,
        assists: u32,
        #[serde(default)]
        players_defeated: u32,
        #[serde(default)]
        empires_defeated: u32,
        #[serde(default)]
        tribes_defeated: u32,
        leader: String,
        winner_player_id: Option<u16>,
        #[serde(default)]
        winning_team: Option<Team>,
        tick: u64,
    },
}

/// Fields of a `Join`, factored out so `JoinWithAuth` can carry them without
/// altering the legacy `Join` variant's wire encoding.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct JoinPayload {
    pub name: String,
    pub is_observer: bool,
    pub target_lobby_id: Option<u64>,
    pub host_private: bool,
    pub build_version: String,
    pub clan_tag: String,
    pub civilization: crate::player::Civilization,
    pub leader: crate::player::Leader,
    pub database_account_id: Option<String>,
    #[serde(default)]
    pub host_config: Option<Box<crate::game_config::GameConfig>>,
    #[serde(default)]
    pub password: Option<String>,
}

/// Proof that a join may bind to a `database_account_id`.
/// - `crazygames`: `token` is the platform JWT from `getUserToken`; the server
///   resolves the account from the VERIFIED token and ignores any client
///   assertion of the account id.
/// - `anonymous`: `account_id` + `token`, where token is the one-time account
///   secret minted by sow-data (only its BLAKE3 hash is stored).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct AuthProof {
    pub provider: String,
    pub account_id: Option<String>,
    pub token: String,
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
    Pong {
        client_time: f64,
    },
    VersionUpdate {
        version: String,
    },
    /// Separate capability frame keeps the existing Start struct wire shape
    /// compatible with cached clients during the ticket rollout.
    RelayTicket {
        lobby_id: u64,
        player_id: u16,
        ticket: String,
    },
    /// Capability returned by the relay for the next reconnect only.
    RelayReconnectTicket {
        lobby_id: u64,
        player_id: u16,
        ticket: String,
    },
}

/// Classifies a lobby so every code path can branch on it explicitly instead of
/// inferring kind from implicit flag combinations.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LobbyKind {
    /// Server-spawned rolling queue — auto-countdown, auto-start, main menu.
    #[default]
    Matchmaking,
    /// Player-created lobby — host decides when to start, shown in the Game Browser.
    Custom,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct LobbyInfo {
    pub id: u64,
    pub num_players: u32,
    pub max_players: u32,
    pub is_counting_down: bool,
    pub timer_secs: f32,
    pub map_name: String,
    pub game_mode: String,
    pub players: Vec<LobbyPlayerSyncState>,
    #[serde(default)]
    pub has_password: bool,
    #[serde(default)]
    pub host_name: String,
    #[serde(default)]
    pub bot_count: u32,
    #[serde(default)]
    pub nation_count: u32,
    #[serde(default)]
    pub bot_difficulty: crate::game_config::BotDifficulty,
    #[serde(default)]
    pub kind: LobbyKind,
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
    pub is_private: bool,
    /// Full server-authoritative lobby snapshot, so private lobbies (never broadcast)
    /// can be seeded with real config (mode, slots, bots, host) instead of placeholders.
    #[serde(default)]
    pub lobby_info: Option<LobbyInfo>,
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
    pub rematch_lobby_id: Option<u64>,
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
    /// Direct relay TLS hostname for the assigned DPDK worker. `None` is kept
    /// only for wire compatibility/dev fixtures; production always supplies
    /// the direct relay host and dynamic game port.
    #[serde(default)]
    pub relay_host: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Copy, Hash)]
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
    #[serde(default)]
    pub civilization: crate::player::Civilization,
    #[serde(default)]
    pub leader: crate::player::Leader,
    #[serde(default)]
    pub is_ai_controlled: bool,
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
    #[serde(default)]
    pub leader: crate::player::Leader,
    /// Per-lobby player id — lets the host target kick/ban actions at a roster entry.
    #[serde(default)]
    pub player_id: u16,
    /// Lobby-stage team assignment (Teams mode only; `None` in FFA).
    #[serde(default)]
    pub team: Option<Team>,
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
        /// Spawn anchors + geo bounds from the parsed map file. `map_bytes`
        /// is raw terrain (also the GPU upload), so these must ride along
        /// explicitly or they never reach the engine.
        map_spawns: Vec<crate::map_file::MapSpawn>,
        geo_bounds: Option<crate::map_file::GeoBounds>,
        num_land_tiles: u32,
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
    #[serde(default)]
    pub resource_requests: Vec<ResourceRequest>,
    pub disconnected: bool,
    pub active_emoji: Option<String>,
    #[serde(default)]
    pub traitor: bool,
    pub civilization: crate::player::Civilization,
    pub leader: crate::player::Leader,
    #[serde(default)]
    pub kills: u32,
    #[serde(default)]
    pub deaths: u32,
    #[serde(default)]
    pub assists: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ResourceRequest {
    pub requester: crate::player::PlayerId,
    pub gold: f64,
    pub troops: f64,
}

/// A single tile whose owner changed during a tick.
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct DirtyTile {
    pub index: u32,
    pub new_owner: u16,
    pub upgrade_level: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
pub struct BuildingSnapshot {
    pub id: u64,
    pub tile_idx: u32,
    pub owner_id: u16,
    pub kind: crate::game::BuildingKind,
    pub level: u8,
    pub under_construction: bool,
    pub ticks_until_complete: u32,
    pub modules: crate::building::CityModules,
}

impl BuildingSnapshot {
    #[inline]
    pub fn active_level(&self) -> u8 {
        if !self.under_construction {
            return self.level;
        }
        let mut ticks = self.ticks_until_complete;
        let mut lvl = self.level;
        while lvl > 1 {
            let dur = crate::building::core::upgrade_duration_ticks(self.kind, lvl);
            if ticks > 0 {
                ticks = ticks.saturating_sub(dur);
                lvl -= 1;
            } else {
                break;
            }
        }
        if lvl == 1 && ticks > 0 { 0 } else { lvl }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct FleetSnapshot {
    pub id: u64,
    pub owner_id: u16,
    pub unit_type: crate::game::UnitType,
    pub troops: f64,
    pub current_tile: u32,
    pub path: std::sync::Arc<Vec<u32>>,
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

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProjectileSnapshot {
    pub id: u64,
    pub kind: crate::game::ProjectileKind,
    pub owner_id: u16,
    pub src_tile: u32,
    pub dst_tile: u32,
    pub path: Vec<u32>,
    pub path_cursor: usize,
    pub steps_per_tick: u8,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NukeAlert {
    pub owner_id: u16,
    pub kind: crate::game::NukeKind,
    pub tile_x: u32,
    pub tile_y: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResourceTransfer {
    pub sender_id: u16,
    pub receiver_id: u16,
    pub gold: f64,
    pub troops: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ResourceRejection {
    pub rejector_id: u16,
    pub requester_id: u16,
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
    #[serde(default)]
    pub nuke_alerts: Vec<NukeAlert>,
    #[serde(default)]
    pub resource_transfers: Vec<ResourceTransfer>,
    #[serde(default)]
    pub resource_rejections: Vec<ResourceRejection>,
    pub winner: Option<u16>,
    #[serde(default)]
    pub winning_team: Option<Team>,
    pub defense_posts: Vec<u32>,
    pub defense_dirty: bool,
    pub total_land_tiles: u32,
    pub sea_lanes: std::sync::Arc<Vec<crate::sea_lane::SeaLane>>,
    pub debug_mem_info: String,
}

#[cfg(test)]
mod protocol_compat_tests {
    use super::{ServerStartMessage, Turn};
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize)]
    struct LegacyStartMessage {
        config: crate::game_config::GameConfig,
        my_player_id: Option<u16>,
        lobby_id: Option<u64>,
        seed: u64,
        players: Vec<super::PlayerInfo>,
        missed_turns: Vec<Turn>,
        map_data: Option<Vec<u8>>,
        relay_port: Option<u16>,
        relay_host: Option<String>,
    }

    fn current_start() -> ServerStartMessage {
        ServerStartMessage {
            config: crate::game_config::GameConfig::default(),
            my_player_id: Some(1),
            lobby_id: Some(42),
            seed: 7,
            players: Vec::new(),
            missed_turns: Vec::new(),
            map_data: None,
            relay_port: Some(25592),
            relay_host: Some("relay.example".to_string()),
        }
    }

    #[test]
    fn new_start_is_decodable_by_legacy_shape() {
        let current = current_start();
        let bytes = bincode::serialize(&current).expect("serialize current start");
        let legacy: LegacyStartMessage =
            bincode::deserialize(&bytes).expect("legacy decoder accepts trailing ticket");
        assert_eq!(legacy.lobby_id, Some(42));
        assert_eq!(legacy.relay_port, Some(25592));
    }

    #[test]
    fn start_wire_shape_roundtrips_without_ticket_field() {
        let current = current_start();
        let legacy = LegacyStartMessage {
            config: current.config,
            my_player_id: current.my_player_id,
            lobby_id: current.lobby_id,
            seed: current.seed,
            players: current.players,
            missed_turns: current.missed_turns,
            map_data: current.map_data,
            relay_port: current.relay_port,
            relay_host: current.relay_host,
        };
        let bytes = bincode::serialize(&legacy).expect("serialize legacy start");
        let decoded: ServerStartMessage = bincode::deserialize(&bytes).expect("start roundtrip");
        assert_eq!(decoded.lobby_id, Some(42));
    }
}
