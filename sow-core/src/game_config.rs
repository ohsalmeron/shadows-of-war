use serde::{Deserialize, Serialize};

fn default_troop_income_pace() -> f64 {
    1.0
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
    /// Hard cap on tiles conquered per attack per tick. Controls visual expansion speed.
    /// Lower = slower, more cinematic spread. Higher = faster blitz.
    pub max_tiles_per_tick: f64,
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
    
    // ==========================================
    // Rendering & Shaders (Visual Adjustments)
    // ==========================================
    /// Controls how dramatic the heightmap topology looks in the pixel shader.
    pub shader_terrain_sharpness: f32,
    /// Opacity of the filling inside a player's territory borders.
    pub shader_interior_alpha: f32,
    /// Opacity of the solid borders outlining a player's territory.
    pub shader_border_alpha: f32,
    
    // ==========================================
    // User Interface & HUD
    // ==========================================
    /// Font file embedded to draw the UI.
    pub ui_font: String,
    /// Minimum font size for drawing player nameplates.
    pub ui_label_base_size: f32,
    /// Maximum scale multiplier applied to the nameplate based on territory size.
    pub ui_label_max_scale: f32,
    /// Number of tiles required to reach the maximum label scale.
    pub ui_label_ref_tiles: f32,
    
    // ==========================================
    // Level-Of-Detail (LOD) Camera Thresholds
    // Camera zoom is clamped in sow-client to [0.25, 20.0].
    // ==========================================
    /// LOD 1 threshold (farthest / simplified): below LOD 2 threshold, use minimal labels.
    pub ui_lod_1_zoom: f32,
    /// LOD 2 threshold (normal view): zoom levels >= this value render normal full plates.
    pub ui_lod_2_zoom: f32,
    /// LOD 3 threshold (max zoom view): zoom levels >= this value are in LOD 3.
    pub ui_lod_3_zoom: f32,
    /// Radius of the minimalist dot icon used when zooming out.
    pub ui_lod_dot_radius: f32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            // Lobby & Match Setup
            max_players: 12,
            bot_count: 2000,      // Tribes (Simple, static filler AI)
            nation_count: 250,   // Nations (Dynamic expanding AI)
            bot_difficulty: BotDifficulty::Vanilla,

            // Map Generation & Spawning
            map_name: "europe".to_string(),
            map_width: 2904,
            map_height: 1672,
            random_spawn: false,

            // Core Simulation Pacing
            tick_rate_ms: 50.0, // Server clock ticks every 50ms (20 ticks per second)
            // Scales combat expansion, gold, and troop income broadly; use `troop_income_pace` to tune troop refill alone.
            global_speed_multiplier: 0.85, // 0.85 = Slightly slower, more tactical pace
            
            // Combat & Expansion Mechanics
            attack_cost_enemy: 4.0,   // Balanced: harder to melt through enemy territory
            attack_cost_neutral: 1.5, // Standard neutral cost
            terrain_multiplier_highland: 1.5,
            terrain_multiplier_mountain: 3.0,
            bot_attack_interval_ticks: 120,    // Strategic waves: bots wait ~6 seconds (120 ticks) between decisions
            max_tiles_per_tick: 20.0,          // Cap per attack to make expansion flow like a frontline
            momentum_divisor: 5000.0,          // Troops needed for 1x momentum

            // Economy & Income Rates
            starting_troops: 500.0,  // Initial burst to allow early expansion
            starting_gold: 100.0,
            gold_base_income: 6.0,
            troop_base_income: 75.0, // Smooth baseline troop recovery
            troop_per_tile: 3.0,     // Rewards map control, but doesn't instantly snowball
            max_troops_base: 100.0,
            max_troops_scale: 50.0,
            city_max_troops_per_level: 2000.0,
            factory_income_bonus_per_level: 0.15, // 15% income boost per factory level
            factory_income_bonus_cap: 2.00,       // Max 200% bonus from factories
            gold_income_per_city_level: 1.0,      // +1 flat gold per city level
            troop_income_pace: 1.0, // Designer-only troop refill multiplier (see field doc)
            
            // Rendering & Shaders (Visual Adjustments)
            shader_terrain_sharpness: 0.0001, // Soft topographical bump map
            shader_interior_alpha: 0.95,     // High opacity, terrain shows through slightly
            shader_border_alpha: 0.95,       // High opacity solid border lines
            
            // User Interface & HUD
            ui_font: "Rajdhani-Medium.ttf".to_string(), // Cyber/RTS theme font
            ui_label_base_size: 12.0,                    // Min point size for nameplates
            ui_label_max_scale: 2.0,                    // Nameplates grow up to 4x size
            ui_label_ref_tiles: 400.0,                  // Reach 4x size when owning 400 tiles
            
            // Level-Of-Detail (LOD) Camera Thresholds (camera zoom clamp: 0.25..20.0)
            // Desired order: far/simplified -> normal/full -> max zoom.
            ui_lod_1_zoom: 0.25, // Farthest zoom band
            ui_lod_2_zoom: 10.0,  // Normal gameplay zoom starts full plates
            ui_lod_3_zoom: 20.0, // Max zoom band
            ui_lod_dot_radius: 2.0,
        }
    }
}
