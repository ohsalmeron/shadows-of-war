use crate::config;
use crate::map::GameMap;
use crate::player::{Player, PlayerId};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum GamePhase {
    Lobby,
    Spawning { end_tick: u64 },
    Playing,
    GameOver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum BuildingKind {
    City,
    Bunker,
    Factory,
    Port,
}

impl BuildingKind {
    pub const ALL: [BuildingKind; 4] = [
        BuildingKind::City,
        BuildingKind::Bunker,
        BuildingKind::Factory,
        BuildingKind::Port,
    ];
    #[inline]
    pub fn as_str(self) -> &'static str {
        match self {
            BuildingKind::City => "City",
            BuildingKind::Bunker => "Defense Tower",
            BuildingKind::Factory => "Factory",
            BuildingKind::Port => "Port",
        }
    }
    pub fn construction_duration_ticks(self) -> u32 {
        match self {
            BuildingKind::City => 20,
            BuildingKind::Bunker => 50,
            BuildingKind::Factory => 35,
            BuildingKind::Port => 30,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum UnitType {
    TransportShip,
    TradeShip,
    Warship,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum NukeKind {
    AtomBomb,
}

impl NukeKind {
    pub fn gold_cost(self, _prev_launches: u32) -> f64 {
        0.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectileKind {
    Nuke { level: u8 },
    SAMMissile,
    Shell,
}

/// Tiles cleared to wilderness (ownership wipe + building destruction).
pub fn nuke_inner_radius(level: u8) -> u32 {
    12 + (level.saturating_sub(1) as u32) * 10
}

/// Tiles receiving troop damage only (ownership unchanged).
pub fn nuke_outer_radius(level: u8) -> u32 {
    30 + (level.saturating_sub(1) as u32) * 25
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Projectile {
    pub id: u64,
    pub kind: ProjectileKind,
    pub owner_id: u16,
    pub src_tile: u32,
    pub dst_tile: u32,
    pub path: Vec<u32>,
    pub path_cursor: usize,
    pub steps_per_tick: u8,
    pub active: bool,
}

impl UnitType {
    pub fn gold_cost(self) -> f64 {
        match self {
            UnitType::TransportShip => 0.0, // Free, converted from land troops
            UnitType::TradeShip => 10_000.0,
            UnitType::Warship => 100_000.0,
        }
    }

    pub fn build_duration_ticks(self) -> u32 {
        match self {
            UnitType::TransportShip => 0, // Instant conversion
            UnitType::TradeShip => 50,
            UnitType::Warship => 150,
        }
    }

    pub fn max_health(self) -> f64 {
        match self {
            UnitType::TransportShip => 50.0,
            UnitType::TradeShip => 100.0,
            UnitType::Warship => 1000.0,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ShipProduction {
    pub kind: UnitType,
    pub ticks_until_complete: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum GameEvent {
    TileExpanded {
        x: u32,
        y: u32,
        owner: u16,
    },
    TileCaptured {
        x: u32,
        y: u32,
        new_owner: u16,
        #[serde(default)]
        previous_owner: u16,
        #[serde(default)]
        troops: f64,
    },
    PlayerEliminated {
        player_id: u16,
        conqueror_id: u16,
        gold_bounty: u32,
        elimination_x: u32,
        elimination_y: u32,
        #[serde(default)]
        assists: Vec<(u16, u32)>,
        #[serde(default)]
        by_nuke: bool,
    },
    GameOver {
        winner_id: u16,
        winning_team: Option<crate::protocol::Team>,
    },
    StructureSpawned {
        id: u64,
        owner_id: u16,
        tile_idx: u32,
        kind: BuildingKind,
        level: u8,
    },
    StructureReady {
        id: u64,
        tile_idx: u32,
        kind: BuildingKind,
    },
    StructureUpgraded {
        id: u64,
        tile_idx: u32,
        kind: BuildingKind,
        level: u8,
    },
    NukeDetonated {
        tile_x: u32,
        tile_y: u32,
        inner_radius: u32,
        outer_radius: u32,
        owner_id: u16,
    },
    TileUpgraded {
        tile_idx: u32,
        level: u32,
    },
    ResourceTransferred {
        sender_id: u16,
        receiver_id: u16,
        gold: f64,
        troops: f64,
    },
    ResourceRequestRejected {
        rejector_id: u16,
        requester_id: u16,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GameState {
    pub seed: u64,
    pub config: crate::game_config::GameConfig,
    pub phase: GamePhase,
    pub map: GameMap,
    pub players: Vec<Player>,
    #[serde(skip)]
    pub player_lookup: Vec<Option<usize>>,
    pub tick: u64,
    pub winner: Option<u16>,
    #[serde(default)]
    pub winning_team: Option<crate::protocol::Team>,
    pub events: Vec<GameEvent>,
    #[serde(default = "default_one")]
    pub next_fleet_id: u64,
    #[serde(default = "default_one")]
    pub next_building_id: u64,
    #[serde(default = "default_one")]
    pub next_attack_id: u64,
    #[serde(default = "default_one")]
    pub next_projectile_id: u64,
    #[serde(default)]
    pub total_land_tiles: u32,
    #[serde(default)]
    pub sea_lanes: std::sync::Arc<Vec<crate::sea_lane::SeaLane>>,
    /// City-state spawn anchors from the loaded `map.bin` (not serialized over the wire).
    #[serde(skip)]
    pub map_spawns: Vec<crate::map_file::MapSpawn>,
}
fn default_one() -> u64 {
    1
}

impl GameState {
    pub fn new(seed: u64, width: u32, height: u32, config: crate::game_config::GameConfig) -> Self {
        let phase = if !config.random_spawn {
            let ticks = (15.0 * 1000.0 / config.tick_rate_ms) as u64;
            GamePhase::Spawning { end_tick: ticks }
        } else {
            GamePhase::Playing
        };
        Self {
            seed,
            config,
            phase,
            map: GameMap::new(width, height),
            players: Vec::new(),
            player_lookup: Vec::new(),
            tick: 0,
            winner: None,
            winning_team: None,
            events: Vec::new(),
            next_fleet_id: 1,
            next_building_id: 1,
            next_attack_id: 1,
            next_projectile_id: 1,
            total_land_tiles: 0,
            sea_lanes: std::sync::Arc::new(Vec::new()),
            map_spawns: Vec::new(),
        }
    }
    pub fn register_player(&mut self, player: Player) {
        let pid = player.id;
        let index = self.players.len();
        self.players.push(player);
        let pid_usize = pid as usize;
        if pid_usize >= self.player_lookup.len() {
            self.player_lookup.resize(pid_usize + 1, None);
        }
        self.player_lookup[pid_usize] = Some(index);
    }

    pub fn place_spawn(&mut self, pid: u16, cx: u32, cy: u32) {
        let r = config::SPAWN_RADIUS as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy > r * r {
                    continue;
                }
                let nx = cx as i32 + dx;
                let ny = cy as i32 + dy;
                if self.map.is_valid_coord(nx, ny) {
                    let (ux, uy) = (nx as u32, ny as u32);
                    if self.map.owner_id(ux, uy) == 0
                        && self.map.terrain[self.map.ref_id(ux, uy)].is_land()
                    {
                        self.set_tile_owner(ux, uy, pid);
                    }
                }
            }
        }
        if let Some(p) = self.player_mut(pid) {
            p.has_spawned = true;
        }
    }

    pub fn spawn_player(&mut self, player: Player, cx: u32, cy: u32) {
        let pid = player.id;
        self.register_player(player);
        self.place_spawn(pid, cx, cy);
    }
    pub fn tick(&mut self) {
        if self.phase != GamePhase::Playing {
            return;
        }
        self.tick += 1;
    }
    pub fn player(&self, id: PlayerId) -> Option<&Player> {
        let i = id as usize;
        if i < self.player_lookup.len() {
            self.player_lookup[i].and_then(|idx| self.players.get(idx))
        } else {
            None
        }
    }
    pub fn player_mut(&mut self, id: PlayerId) -> Option<&mut Player> {
        let i = id as usize;
        if i < self.player_lookup.len() {
            self.player_lookup[i].and_then(|idx| self.players.get_mut(idx))
        } else {
            None
        }
    }
    pub fn set_tile_owner(&mut self, x: u32, y: u32, new_owner: u16) {
        let old_owner = self.map.owner_id(x, y);
        if old_owner == new_owner {
            return;
        }
        let linear_idx = y * self.map.width + x;
        if old_owner != 0 {
            if let Some(p) = self.player_mut(old_owner) {
                p.sum_x = p.sum_x.saturating_sub(x as u64);
                p.sum_y = p.sum_y.saturating_sub(y as u64);
                p.tile_count = p.tile_count.saturating_sub(1);
                p.border_remove(linear_idx);
            }
        }
        if new_owner != 0 {
            if let Some(p) = self.player_mut(new_owner) {
                p.sum_x += x as u64;
                p.sum_y += y as u64;
                p.tile_count += 1;
                if old_owner != 0 && old_owner != new_owner {
                    *p.tile_conquests.entry(old_owner).or_insert(0) += 1;
                }
            }
        }
        self.map.set_owner_id(x, y, new_owner);
        if new_owner != 0 {
            let is_border = self.map.is_border_tile(x, y, new_owner);
            if is_border {
                if let Some(p) = self.player_mut(new_owner) {
                    p.border_insert(linear_idx);
                }
            }
        }

        let mut neighbors = [(0, 0); 8];
        let mut n_count = 0;
        self.map.for_each_neighbor(x, y, |nx, ny| {
            neighbors[n_count] = (nx, ny);
            n_count += 1;
        });

        for &(nx, ny) in neighbors.iter().take(n_count) {
            let n_owner = self.map.owner_id(nx, ny);
            let n_idx = ny * self.map.width + nx;
            if n_owner == old_owner && old_owner != 0 {
                if let Some(p) = self.player_mut(old_owner) {
                    p.border_insert(n_idx);
                }
            } else if n_owner == new_owner && new_owner != 0 {
                let ib = self.map.is_border_tile(nx, ny, new_owner);
                if !ib {
                    if let Some(p) = self.player_mut(new_owner) {
                        p.border_remove(n_idx);
                    }
                }
            }
        }

        if new_owner != 0 {
            let mut to_capture = [(0, 0); 6];
            let mut capture_count = 0;

            for &(nx, ny) in neighbors.iter().take(n_count) {
                let n_owner = self.map.owner_id(nx, ny);
                if n_owner != new_owner && self.map.terrain[self.map.ref_id(nx, ny)].is_land() {
                    let mut surrounded = true;
                    self.map.for_each_neighbor(nx, ny, |nnx, nny| {
                        if self.map.owner_id(nnx, nny) != new_owner {
                            surrounded = false;
                        }
                    });
                    if surrounded {
                        to_capture[capture_count] = (nx, ny);
                        capture_count += 1;
                    }
                }
            }

            for &(cx, cy) in to_capture.iter().take(capture_count) {
                self.set_tile_owner(cx, cy, new_owner);
            }
        }
    }
}
