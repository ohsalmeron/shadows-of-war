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
    pub max_players: u32,
    pub bot_count: u32,
    pub nation_count: u32,
    pub bot_difficulty: BotDifficulty,

    // Map Config
    pub map_name: String,
    pub map_width: u32,
    pub map_height: u32,
    pub random_spawn: bool,
    // Gameplay Modifiers
    pub tick_rate_ms: f32,
    pub attack_cost_enemy: f64,
    pub attack_cost_neutral: f64,
    pub terrain_multiplier_highland: f64,
    pub terrain_multiplier_mountain: f64,
    pub bot_attack_interval_ticks: u64,
    
    // Economy Modifiers
    pub starting_troops: f64,
    pub starting_gold: f64,
    pub gold_base_income: f64,
    pub troop_base_income: f64,
    pub troop_per_tile: f64,
    pub max_troops_base: f64,
    pub max_troops_scale: f64,
    pub city_max_troops_per_level: f64,
    pub factory_income_bonus_per_level: f64,
    pub factory_income_bonus_cap: f64,
    pub gold_income_per_city_level: f64,
    
    // Visual Shader Settings
    pub shader_terrain_sharpness: f32,
    pub shader_interior_alpha: f32,
    pub shader_border_alpha: f32,
}

impl Default for GameConfig {
    fn default() -> Self {
        Self {
            max_players: 12,
            bot_count: 400, // Tribes (Bots)
            nation_count: 50, // Nations (Complex AI)
            bot_difficulty: BotDifficulty::Vanilla,
            map_name: "europe".to_string(),
            map_width: 2904,
            map_height: 1672,
            random_spawn: false,
            tick_rate_ms: 100.0,
            attack_cost_enemy: 4.0,
            attack_cost_neutral: 1.5,
            terrain_multiplier_highland: 1.75,
            terrain_multiplier_mountain: 3.5,
            bot_attack_interval_ticks: 64,
            starting_troops: 100.0,
            starting_gold: 100.0,
            gold_base_income: 8.0,
            troop_base_income: 100.0,
            troop_per_tile: 4.0,
            max_troops_base: 1000.0,
            max_troops_scale: 500.0,
            city_max_troops_per_level: 2000.0,
            factory_income_bonus_per_level: 0.15,
            factory_income_bonus_cap: 2.00,
            gold_income_per_city_level: 1.0,
            
            // Visual Defaults (mimicking OpenFront style for readability)
            shader_terrain_sharpness: 0.005, // Much softer topographical bump map
            shader_interior_alpha: 0.75, // Brighter interior (terrain shows through more)
            shader_border_alpha: 0.75, // Solid border
        }
    }
}
