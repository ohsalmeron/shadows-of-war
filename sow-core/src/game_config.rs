use serde::{Deserialize, Serialize};

fn default_seed() -> u64 {
    42
}

fn default_game_mode() -> String {
    "FFA".to_string()
}

fn default_max_tiles_per_tick_reference_troops() -> f64 {
    1000.0
}

fn default_max_tiles_per_tick_at_reference() -> f64 {
    4.0
}

fn default_troop_base_income() -> f64 {
    2.0
}

fn default_cost_scale_cap_multiplier() -> f64 {
    10.0
}

fn default_territory_gold_amount() -> f64 {
    1.0
}

fn default_territory_gold_tiles() -> u32 {
    4
}

fn default_territory_troop_amount() -> f64 {
    1.0
}

fn default_territory_troop_tiles() -> u32 {
    8
}

fn default_city_troop_income() -> f64 {
    50.0
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum BotDifficulty {
    #[default]
    Vanilla,
    Terminator,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct GameConfig {
    // ==========================================
    // Lobby & Match Setup
    // ==========================================
    /// The maximum number of human players allowed in the lobby.
    pub max_players: u32,
    /// Number of minor tribes (simple bot entities) spawned across the map.
    pub bot_count: u32,
    /// Number of major AI nations (complex AI) spawned across the map.
    pub nation_count: u32,
    /// Determines how aggressively the AI bots expand and behave.
    pub bot_difficulty: BotDifficulty,
    /// Seed used for procedural random number generation.
    #[serde(default = "default_seed")]
    pub seed: u64,

    // ==========================================
    // Map Generation & Spawning
    // ==========================================
    /// The directory name of the map to load (e.g., "europe").
    pub map_name: String,
    /// Whether this is "FFA", "Teams", etc.
    #[serde(default = "default_game_mode")]
    pub game_mode: String,
    /// Physical width of the map grid in tiles.
    pub map_width: u32,
    /// Physical height of the map grid in tiles.
    pub map_height: u32,
    /// If true, players spawn randomly. If false, they spawn based on preset locations.
    pub random_spawn: bool,
    /// Percentage of the map's total land tiles needed to trigger an automatic win (e.g. 0.10 for 10%).
    pub map_control_win_percentage: f32,

    // ==========================================
    // Core Simulation Pacing
    // ==========================================
    /// Determines how fast the server ticks. 250ms = 4 ticks per second.
    /// Increasing this slows down everything mechanically, including attack animations.
    pub tick_rate_ms: f32,
    /// Master speed dial for the entire game (1.0 = normal).
    /// Scales all per-second rates (expansion, income, IQ) proportionally via `per_tick()`.
    /// Example: 0.5 = half speed, 2.0 = double speed.
    pub global_speed_multiplier: f64,

    // ==========================================
    // Combat & Expansion Mechanics
    // ==========================================
    /// Number of troops consumed to conquer a single tile owned by another player.
    /// Lower values = faster, explosive conquest bursts.
    pub attack_cost_enemy: f64,
    /// Number of troops consumed to conquer a single unowned/neutral tile.
    /// Lower values = much faster early-game expansion.
    pub attack_cost_neutral: f64,
    /// Defense modifier for highland terrain tiles (multiplies attack cost).
    pub terrain_multiplier_highland: f64,
    /// Defense modifier for mountain terrain tiles (multiplies attack cost).
    pub terrain_multiplier_mountain: f64,

    /// Absolute ceiling on the troop-scaled per-tick tile cap (applied before momentum).
    /// The effective cap is `min(curve(troops), this value)`; see `max_tiles_cap_for_troops`.
    pub max_tiles_per_tick: f64,
    /// Troop count at which the scaled cap equals `max_tiles_per_tick_at_reference` (before ceiling).
    #[serde(default = "default_max_tiles_per_tick_reference_troops")]
    pub max_tiles_per_tick_reference_troops: f64,
    /// Tile cap at `max_tiles_per_tick_reference_troops`; doubles each 10× troops above that anchor.
    #[serde(default = "default_max_tiles_per_tick_at_reference")]
    pub max_tiles_per_tick_at_reference: f64,
    /// Troops needed to reach 1x momentum. At 2x this value you get 2x speed, etc.
    /// Higher = slower expansion for large armies. (default 2000 = half speed vs old 1000)
    pub momentum_divisor: f64,

    // ==========================================
    // Economy & Income Rates
    // ==========================================
    /// Amount of troops given to players at spawn.
    pub starting_troops: f64,
    /// Amount of gold given to players at spawn.
    pub starting_gold: f64,
    /// Gold earned per second at 1x speed (before city bonuses). Scaled by `per_tick()`.
    pub gold_base_income: f64,
    /// Troops regenerated per second at 1x speed (before factory bonuses). Scaled by `per_tick()`.
    #[serde(default = "default_troop_base_income")]
    pub troop_base_income: f64,
    /// Base maximum troop capacity cap before territory size is accounted for.
    pub max_troops_base: f64,
    /// How much extra troop capacity is gained based on total territory owned.
    pub max_troops_scale: f64,
    /// Gold per second per `territory_gold_tiles` owned (smooth fractional scaling).
    #[serde(default = "default_territory_gold_amount")]
    pub territory_gold_amount: f64,
    /// Tiles per territory gold interval (e.g. 4 → 1 gold/s per 4 tiles).
    #[serde(default = "default_territory_gold_tiles")]
    pub territory_gold_tiles: u32,
    /// Troops per second per `territory_troop_tiles` owned (smooth fractional scaling).
    #[serde(default = "default_territory_troop_amount")]
    pub territory_troop_amount: f64,
    /// Tiles per territory troop interval (e.g. 8 → 1 troop/s per 8 tiles).
    #[serde(default = "default_territory_troop_tiles")]
    pub territory_troop_tiles: u32,

    // ==========================================
    // Buildings (stacking model — each placed building adds its flat bonus)
    // ==========================================
    /// Max troop capacity added per City.
    pub city_max_troops: f64,
    /// Gold income per second added per City.
    pub city_gold_income: f64,
    /// Troop income per second added per City level.
    #[serde(default = "default_city_troop_income")]
    pub city_troop_income: f64,
    /// Defense range/radius of each Bunker.
    pub bunker_range: f64,
    /// Extra attack frontier priority per Bunker within range.
    pub bunker_priority: f64,
    /// Defensive troop loss multiplier per Bunker within range.
    pub bunker_strength: f64,
    /// Gold income per second added per Factory.
    pub factory_gold_income: f64,
    /// Troop income per second added per Port.
    pub port_troop_income: f64,
    /// Gold income per second added per Port.
    pub port_gold_income: f64,
    /// Gold cost to place a City.
    pub cost_city: f64,
    /// Gold cost to place a Bunker.
    pub cost_bunker: f64,
    /// Gold cost to place a Factory.
    pub cost_factory: f64,
    /// Gold cost to place a Port.
    pub cost_port: f64,
    /// Max multiplier applied to structure base cost as count grows (cost plateaus at base × this).
    #[serde(default = "default_cost_scale_cap_multiplier")]
    pub cost_scale_cap_multiplier: f64,
    /// Gold cost to launch a nuke.
    pub nuke_cost: f64,

    #[serde(default)]
    pub player_civilization: crate::player::Civilization,
    #[serde(default)]
    pub player_leader: crate::player::Leader,
}

impl GameConfig {
    /// Convert a per-second rate to a per-tick rate, applying the global speed multiplier.
    /// This is the **single source of truth** for time scaling. Every system uses this.
    #[inline]
    pub fn per_tick(&self, per_second: f64) -> f64 {
        per_second * (self.tick_rate_ms as f64 / 1000.0) * self.global_speed_multiplier
    }
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            // Lobby & Match Setup
            max_players: 128,
            bot_count: 420,    // Tribes (Simple, static filler AI)
            nation_count: 128, // Nations (Dynamic expanding AI)
            bot_difficulty: BotDifficulty::Vanilla,
            seed: 42,

            // Map Generation & Spawning
            map_name: crate::maps::DEFAULT_MAP_KEY.to_string(),
            game_mode: "FFA".to_string(),
            map_width: 1000,
            map_height: 800,
            random_spawn: false,
            map_control_win_percentage: 0.60,

            // Core Simulation Pacing
            tick_rate_ms: 100.0, // Server clock ticks every 100ms (10 ticks per second)
            global_speed_multiplier: 0.5,

            // Combat & Expansion Mechanics
            attack_cost_enemy: 3.0,
            attack_cost_neutral: 0.5,
            terrain_multiplier_highland: 1.25,
            terrain_multiplier_mountain: 2.0,

            max_tiles_per_tick: 4096.0,
            max_tiles_per_tick_reference_troops: 25000.0,
            max_tiles_per_tick_at_reference: 256.0,
            momentum_divisor: 1250.0,

            // Economy & Income Rates
            starting_troops: 5000.0,
            starting_gold: 100.0,
            gold_base_income: 4.0,
            troop_base_income: 250.0,
            max_troops_base: 10.0,
            max_troops_scale: 350.0,
            territory_gold_amount: 1.0,
            territory_gold_tiles: 4,
            territory_troop_amount: 1.0,
            territory_troop_tiles: 8,

            // Buildings (stacking)
            city_max_troops: 5000.0,
            city_gold_income: 4.0,
            city_troop_income: 50.0,
            bunker_range: 14.0,
            bunker_priority: 120.0,
            bunker_strength: 4.0,
            factory_gold_income: 8.0,
            port_troop_income: 50.0,
            port_gold_income: 4.0,
            cost_city: 200.0,
            cost_bunker: 75.0,
            cost_factory: 125.0,
            cost_port: 150.0,
            cost_scale_cap_multiplier: 10.0,
            nuke_cost: 5.0,

            player_civilization: crate::player::Civilization::Rome,
            player_leader: crate::player::Leader::Caesar,
        }
    }
}

/// Per-attack tile cap before momentum: doubles each 10× troops above `reference_troops`,
/// anchored at `at_reference` for stacks at or below that troop count (see field docs).
pub fn max_tiles_cap_for_troops(troops: f64, cfg: &GameConfig) -> f64 {
    let ceiling = cfg.max_tiles_per_tick;
    let at_ref = cfg.max_tiles_per_tick_at_reference;
    let t0 = cfg.max_tiles_per_tick_reference_troops;

    let sane_ceiling = if ceiling.is_finite() && ceiling > 0.0 {
        ceiling
    } else if at_ref.is_finite() && at_ref > 0.0 {
        at_ref
    } else {
        return 1.0;
    };

    if !at_ref.is_finite() || at_ref <= 0.0 {
        return sane_ceiling.max(1.0);
    }
    if !t0.is_finite() || t0 <= 0.0 {
        return at_ref.min(sane_ceiling).max(1.0);
    }
    if troops.is_nan() || troops < 0.0 {
        return at_ref.min(sane_ceiling).max(1.0);
    }
    if troops.is_infinite() {
        return sane_ceiling.max(1.0);
    }

    let t_eff = troops.max(t0);
    let ratio = t_eff / t0;
    let curve = at_ref * libm::pow(2.0, libm::log10(ratio));
    let curve = if curve.is_finite() {
        curve
    } else {
        sane_ceiling
    };
    curve.min(sane_ceiling).max(1.0)
}
