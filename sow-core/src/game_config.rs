use serde::{Deserialize, Serialize};

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
    // ==========================================
    /// Zoom levels >= this value will render FULL nameplates for EVERY entity on the map.
    pub ui_lod_zoom_full: f32,
    /// Zoom levels >= this value will render FULL nameplates for Nations/Humans, but DOTS for tribes.
    /// Zoom levels below this value will render ONLY DOTS for everyone to declutter the map.
    pub ui_lod_zoom_nations: f32,
    /// Radius of the minimalist dot icon used when zooming out.
    pub ui_lod_dot_radius: f32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            // Lobby & Match Setup
            max_players: 12,
            bot_count: 200,      // Tribes (Simple, static filler AI)
            nation_count: 100,   // Nations (Dynamic expanding AI)
            bot_difficulty: BotDifficulty::Vanilla,

            // Map Generation & Spawning
            map_name: "europe".to_string(),
            map_width: 2904,
            map_height: 1672,
            random_spawn: false,

            // Core Simulation Pacing
            tick_rate_ms: 250.0, // Server clock ticks 4 times per second
            global_speed_multiplier: 0.5, // 0.5 = Game plays at exactly half speed (attacks last 2x longer, income 2x slower)
            
            // Combat & Expansion Mechanics
            attack_cost_enemy: 2.0,   // Extremely low: 100 troops = 50 enemy tiles bursts
            attack_cost_neutral: 0.5, // Extremely low: 100 troops = 200 neutral tiles bursts
            terrain_multiplier_highland: 1.75, // Highlands cost 75% more to conquer
            terrain_multiplier_mountain: 3.5,  // Mountains cost 3.5x more to conquer
            bot_attack_interval_ticks: 256,    // Nerfed aggression: bots wait ~64 seconds between waves
            max_tiles_per_tick: 40.0,          // Hard cap per attack (was 100). Visible, paced spread.
            momentum_divisor: 2000.0,          // Troops needed for 1x momentum (was 1000). Halved speed.

            // Economy & Income Rates (Halved to pace the game down 2x)
            starting_troops: 100.0,
            starting_gold: 100.0,
            gold_base_income: 4.0,   // Reduced from 8.0: Forces slower macro progression
            troop_base_income: 50.0, // Reduced from 100.0: Takes twice as long to prep an attack
            troop_per_tile: 2.0,     // Reduced from 4.0: Expansion snowballs slower
            max_troops_base: 1000.0,
            max_troops_scale: 500.0,
            city_max_troops_per_level: 2000.0,
            factory_income_bonus_per_level: 0.15, // 15% income boost per factory level
            factory_income_bonus_cap: 2.00,       // Max 200% bonus from factories
            gold_income_per_city_level: 1.0,      // +1 flat gold per city level
            
            // Rendering & Shaders (Visual Adjustments)
            shader_terrain_sharpness: 0.005, // Soft topographical bump map
            shader_interior_alpha: 0.75,     // High opacity, terrain shows through slightly
            shader_border_alpha: 0.75,       // High opacity solid border lines
            
            // User Interface & HUD
            ui_font: "Rajdhani-Medium.ttf".to_string(), // Cyber/RTS theme font
            ui_label_base_size: 12.0,                    // Min point size for nameplates
            ui_label_max_scale: 2.0,                    // Nameplates grow up to 4x size
            ui_label_ref_tiles: 400.0,                  // Reach 4x size when owning 400 tiles
            
            // Level-Of-Detail (LOD) Camera Thresholds
            ui_lod_zoom_full: 2.0,     // Zoom >= 3.0: High clutter (all labels)
            ui_lod_zoom_nations: 1.0,  // Zoom >= 1.5: Medium clutter (Nation/Human labels, Tribe dots)
            ui_lod_dot_radius: 4.0,    // Zoom < 1.5: Minimalist (4px Dots only)
        }
    }
}
