use serde::{Deserialize, Serialize};

fn default_troop_fill_time() -> f64 {
    40.0
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

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum BotDifficulty {
    #[default]
    Vanilla,
    Terminator,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BotProfile {
    pub attack_interval_ticks: u64,
    pub trigger_ratio: f64,
    pub reserve_ratio: f64,
    pub expand_ratio: f64,
}

impl Default for BotProfile {
    fn default() -> Self {
        Self {
            attack_interval_ticks: 240, // 4 seconds by default
            trigger_ratio: 0.6,
            reserve_ratio: 0.3,
            expand_ratio: 0.15,
        }
    }
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
    /// Lowers expansion speed, income generation, and bot aggression proportionally.
    /// Example: 0.5 means attacks take twice as long to spread and income generates half as fast.
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
    /// Minimum ticks a bot must wait between launch attacks.
    /// Higher values = slower, less aggressive AI expansion.
    pub bot_attack_interval_ticks: u64,
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
    /// Flat gold income added per second. Halving this doubles the time it takes to afford structures.
    pub gold_base_income: f64,
    /// Base troop income added per tick (dark-rift: flat base + curve).
    #[serde(default = "default_troop_base_income")]
    pub troop_base_income: f64,
    /// Base maximum troop capacity cap before territory size is accounted for.
    pub max_troops_base: f64,
    /// How much extra troop capacity is gained based on total territory owned.
    pub max_troops_scale: f64,
    /// Additional maximum troop capacity provided per level of an owned city.
    pub city_max_troops_per_level: f64,
    /// Percentage multiplier bonus to overall income provided per factory level.
    pub factory_income_bonus_per_level: f64,
    /// Maximum allowable income multiplier from all factories combined.
    pub factory_income_bonus_cap: f64,
    /// Flat gold income generated per level of an owned city.
    pub gold_income_per_city_level: f64,
    /// How many seconds it takes to naturally fill your troop capacity from 0 to 100% (ignoring factories).
    #[serde(default = "default_troop_fill_time")]
    pub troop_fill_time_seconds: f64,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            // Lobby & Match Setup
            max_players: 120,
            bot_count: 650,    // Tribes (Simple, static filler AI)
            nation_count: 80, // Nations (Dynamic expanding AI)
            bot_difficulty: BotDifficulty::Vanilla,

            // Map Generation & Spawning
            map_name: "europe".to_string(),
            game_mode: "FFA".to_string(),
            map_width: 2904,
            map_height: 1672,
            random_spawn: false,
            map_control_win_percentage: 0.60,

            // Core Simulation Pacing
            tick_rate_ms: 100.0, // Server clock ticks every 100ms (10 ticks per second)
            // Scales combat expansion, gold, and troop income broadly
            global_speed_multiplier: 0.1, 

            // Combat & Expansion Mechanics
            attack_cost_enemy: 3.0,
            attack_cost_neutral: 0.05,
            terrain_multiplier_highland: 3.0,
            terrain_multiplier_mountain: 6.0,
            bot_attack_interval_ticks: 240,
            max_tiles_per_tick: 1024.0,
            max_tiles_per_tick_reference_troops: 1000.0,
            max_tiles_per_tick_at_reference: 12.0,
            momentum_divisor: 125.0,

            // Economy & Income Rates
            starting_troops: 100.0,
            starting_gold: 25.0,
            gold_base_income: 1.5,
            troop_base_income: 2.0,
            max_troops_base: 100.0,
            max_troops_scale: 50.0,
            city_max_troops_per_level: 2000.0,
            factory_income_bonus_per_level: 0.10,
            factory_income_bonus_cap: 1.50,
            gold_income_per_city_level: 0.5,
            troop_fill_time_seconds: 40.0,
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