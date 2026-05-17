use serde::{Deserialize, Serialize};

fn default_troop_income_pace() -> f64 {
    1.0
}

fn default_max_tiles_per_tick_reference_troops() -> f64 {
    1000.0
}

fn default_max_tiles_per_tick_at_reference() -> f64 {
    4.0
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum BotDifficulty {
    BrainDead,
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
    /// Flat troop income added per second. Halving this doubles the time to build an army.
    pub troop_base_income: f64,
    /// Troops generated per second for every tile owned by the player.
    pub troop_per_tile: f64,
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
    /// Designer dial for troop refill only: multiplied onto final per-tick troop income after
    /// `troop_base_income`, factories, bot penalties, and `global_speed_multiplier`.
    /// `1.0` matches prior behavior; values above 1 speed refill, below 1 slow it. Does not affect gold.
    #[serde(default = "default_troop_income_pace")]
    pub troop_income_pace: f64,
    
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            // Lobby & Match Setup
            max_players: 120,
            bot_count: 500,      // Tribes (Simple, static filler AI)
            nation_count: 120,   // Nations (Dynamic expanding AI)
            bot_difficulty: BotDifficulty::Vanilla,

            // Map Generation & Spawning
            map_name: "europe".to_string(),
            map_width: 2904,
            map_height: 1672,
            random_spawn: false,
            map_control_win_percentage: 0.10,

            // Core Simulation Pacing
            tick_rate_ms: 100.0, // Server clock ticks every 50ms (20 ticks per second)
            // Scales combat expansion, gold, and troop income broadly; use `troop_income_pace` to tune troop refill alone.
            global_speed_multiplier: 0.45, // 0.85 = Slightly slower, more tactical pace
            
            // Combat & Expansion Mechanics
            attack_cost_enemy: 1.0,   // Balanced: harder to melt through enemy territory
            attack_cost_neutral: 0.1, // Standard neutral cost
            terrain_multiplier_highland: 3.5,
            terrain_multiplier_mountain: 7.0,
            bot_attack_interval_ticks: 16,    // Strategic waves: bots wait ~6 seconds (120 ticks) between decisions
            max_tiles_per_tick: 64.0, // Ceiling on troop-scaled cap; curve ~4@1K → ~32@1M
            max_tiles_per_tick_reference_troops: 1000.0,
            max_tiles_per_tick_at_reference: 4.0,
            momentum_divisor: 1250.0,          // Troops needed for 1x momentum

            // Economy & Income Rates
            starting_troops: 100.0,  // Initial burst to allow early expansion
            starting_gold: 10.0,
            gold_base_income: 4.0,
            troop_base_income: 2.0, // Smooth baseline troop recovery
            troop_per_tile: 2.0,     // Rewards map control, but doesn't instantly snowball
            max_troops_base: 100.0,
            max_troops_scale: 50.0,
            city_max_troops_per_level: 2000.0,
            factory_income_bonus_per_level: 0.15, // 15% income boost per factory level
            factory_income_bonus_cap: 2.00,       // Max 200% bonus from factories
            gold_income_per_city_level: 1.0,      // +1 flat gold per city level
            troop_income_pace: 1.0, // Designer-only troop refill multiplier (see field doc)
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
    let curve = if curve.is_finite() { curve } else { sane_ceiling };
    curve.min(sane_ceiling).max(1.0)
}

#[cfg(test)]
mod max_tiles_cap_tests {
    use super::*;

    fn assert_close(a: f64, b: f64) {
        assert!(
            (a - b).abs() < 1e-9,
            "expected {b}, got {a} (diff {})",
            (a - b).abs()
        );
    }

    #[test]
    fn cap_at_reference_decades() {
        let c = GameConfig::default();
        assert_close(max_tiles_cap_for_troops(1000.0, &c), 4.0);
        assert_close(max_tiles_cap_for_troops(10_000.0, &c), 8.0);
        assert_close(max_tiles_cap_for_troops(100_000.0, &c), 16.0);
        assert_close(max_tiles_cap_for_troops(1_000_000.0, &c), 32.0);
    }

    #[test]
    fn cap_floors_small_stacks_to_reference_curve() {
        let c = GameConfig::default();
        assert_close(max_tiles_cap_for_troops(100.0, &c), 4.0);
        assert_close(max_tiles_cap_for_troops(0.0, &c), 4.0);
    }

    #[test]
    fn cap_respects_ceiling() {
        let mut c = GameConfig::default();
        c.max_tiles_per_tick = 6.0;
        assert_close(max_tiles_cap_for_troops(10_000.0, &c), 6.0);
    }

    #[test]
    fn cap_nan_and_negative_fall_back_to_at_reference_infinity_hits_ceiling() {
        let c = GameConfig::default();
        assert_close(max_tiles_cap_for_troops(f64::NAN, &c), 4.0);
        assert_close(max_tiles_cap_for_troops(f64::INFINITY, &c), 64.0);
        assert_close(max_tiles_cap_for_troops(-1.0, &c), 4.0);
    }

    #[test]
    fn serde_omitted_new_fields_use_defaults() {
        let json = r#"{"max_players":2,"bot_count":0,"nation_count":0,"bot_difficulty":"Vanilla","map_name":"m","map_width":8,"map_height":8,"random_spawn":false,"tick_rate_ms":50.0,"global_speed_multiplier":1.0,"attack_cost_enemy":1.0,"attack_cost_neutral":1.0,"terrain_multiplier_highland":1.0,"terrain_multiplier_mountain":1.0,"bot_attack_interval_ticks":1,"max_tiles_per_tick":8.0,"momentum_divisor":1.0,"starting_troops":1.0,"starting_gold":0.0,"gold_base_income":0.0,"troop_base_income":0.0,"troop_per_tile":0.0,"max_troops_base":1.0,"max_troops_scale":0.0,"city_max_troops_per_level":0.0,"factory_income_bonus_per_level":0.0,"factory_income_bonus_cap":0.0,"gold_income_per_city_level":0.0}"#;
        let cfg: GameConfig = serde_json::from_str(json).expect("deserialize");
        assert_close(cfg.max_tiles_per_tick_reference_troops, 1000.0);
        assert_close(cfg.max_tiles_per_tick_at_reference, 4.0);
        assert_close(cfg.max_tiles_per_tick, 8.0);
        assert_close(max_tiles_cap_for_troops(10_000.0, &cfg), 8.0);
    }
}
