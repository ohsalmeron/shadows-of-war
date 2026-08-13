use crate::bitset::DenseBitSet;
use crate::rng::NextIntExt;
use serde::{Deserialize, Serialize};
use sow_data::leader_for_civilization;

mod colors;

pub use colors::{bot_territory_color, human_shader_territory_rgb, premium_color, team_territory_rgb};
pub use sow_data::{Civilization, Leader, NamedColor, PREMIUM_COLORS};

use wyrand::WyRand;

pub type PlayerId = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerType {
    Human,
    Bot,
    Nation,
}

fn default_player_gold() -> f64 {
    crate::game_config::GameConfig::default().starting_gold
}

fn default_iq() -> u32 {
    100
}

fn default_iq_points() -> f64 {
    0.0
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Player {
    pub id: PlayerId,
    pub name: String,
    pub player_type: PlayerType,
    pub troops: f64,
    pub max_troops: f64,
    #[serde(default)]
    pub max_troops_cap: Option<f64>,
    #[serde(default = "default_player_gold")]
    pub gold: f64,
    pub color: [f32; 3],
    pub alive: bool,
    pub has_spawned: bool,
    pub sum_x: u64,
    pub sum_y: u64,
    pub tile_count: u32,
    pub border_tiles: DenseBitSet,
    #[serde(skip, default = "default_wyrand")]
    pub bot_rng: WyRand,
    pub factories: u32,
    pub cities: u32,
    pub team: Option<crate::protocol::Team>,
    #[serde(default = "default_iq")]
    pub iq: u32,
    #[serde(default = "default_iq_points")]
    pub iq_points: f64,
    #[serde(default)]
    pub alliances: Vec<PlayerId>,
    #[serde(default)]
    pub alliance_timers: std::collections::HashMap<PlayerId, u32>,
    #[serde(default)]
    pub disconnected: bool,
    /// Fictional humans: appear as Human players but the core auto-plays
    /// them via `execute_ai_think`, deterministically on every client. No
    /// sockets, no relay involvement — each client's engine simulates them.
    #[serde(default)]
    pub is_ai_controlled: bool,
    #[serde(default)]
    pub active_emoji: Option<String>,
    #[serde(default)]
    pub emoji_timer: u32,
    #[serde(default)]
    pub emoji_pinned: bool,
    #[serde(default)]
    pub traitor: bool,
    #[serde(default)]
    pub traitor_tick: u32,
    #[serde(default)]
    pub civilization: Civilization,
    #[serde(default)]
    pub leader: Leader,
    #[serde(default)]
    pub kills: u32,
    #[serde(default)]
    pub deaths: u32,
    #[serde(default)]
    pub assists: u32,
    #[serde(default)]
    pub tile_conquests: std::collections::BTreeMap<PlayerId, u32>,
}

fn default_wyrand() -> WyRand {
    WyRand::new(0)
}

impl Player {
    pub fn new_human(
        id: u16,
        name: String,
        color: [f32; 3],
        config: &crate::game_config::GameConfig,
    ) -> Self {
        Self {
            id,
            alive: true,
            player_type: PlayerType::Human,
            name,
            color,
            troops: config.starting_troops,
            max_troops: config.max_troops_base,
            max_troops_cap: None,
            gold: config.starting_gold,
            has_spawned: false,
            sum_x: 0,
            sum_y: 0,
            tile_count: 0,
            border_tiles: DenseBitSet::new(),
            bot_rng: WyRand::new(id as u64),
            factories: 0,
            cities: 0,
            team: None,
            iq: 100,
            iq_points: 0.0,
            alliances: Vec::new(),
            alliance_timers: std::collections::HashMap::new(),
            disconnected: false,
            is_ai_controlled: false,
            active_emoji: None,
            emoji_timer: 0,
            emoji_pinned: false,
            traitor: false,
            traitor_tick: 0,
            civilization: Civilization::Rome,
            leader: Leader::Caesar,
            kills: 0,
            deaths: 0,
            assists: 0,
            tile_conquests: std::collections::BTreeMap::new(),
        }
    }
    pub fn new_bot(
        id: u16,
        name: String,
        color: [f32; 3],
        config: &crate::game_config::GameConfig,
    ) -> Self {
        let mut rng = WyRand::new(id as u64);
        let iq = if id.is_multiple_of(100) {
            rng.next_int(130, 181) as u32
        } else if id.is_multiple_of(10) {
            rng.next_int(100, 121) as u32
        } else if id % 10 == 1 {
            rng.next_int(60, 81) as u32
        } else {
            rng.next_int(85, 106) as u32
        };
        let civ = Civilization::ALL[rng.next_int(0, Civilization::ALL.len() as i32) as usize];
        let leader = leader_for_civilization(civ);
        let starting_troops = config.starting_troops * 0.5; // ponytail: tribes get half
        let starting_gold = if iq >= 130 {
            config.starting_gold
        } else if iq >= 100 {
            config.starting_gold * 0.5
        } else {
            0.0
        };
        Self {
            id,
            alive: true,
            player_type: PlayerType::Bot,
            name,
            color,
            troops: starting_troops,
            max_troops: config.max_troops_base,
            max_troops_cap: None,
            gold: starting_gold,
            has_spawned: false,
            sum_x: 0,
            sum_y: 0,
            tile_count: 0,
            border_tiles: DenseBitSet::new(),
            bot_rng: WyRand::new(id as u64),
            factories: 0,
            cities: 0,
            team: None,
            iq,
            iq_points: 0.0,
            alliances: Vec::new(),
            alliance_timers: std::collections::HashMap::new(),
            disconnected: false,
            is_ai_controlled: false,
            active_emoji: None,
            emoji_timer: 0,
            emoji_pinned: false,
            traitor: false,
            traitor_tick: 0,
            civilization: civ,
            leader,
            kills: 0,
            deaths: 0,
            assists: 0,
            tile_conquests: std::collections::BTreeMap::new(),
        }
    }
    pub fn new_nation(
        id: u16,
        name: String,
        color: [f32; 3],
        config: &crate::game_config::GameConfig,
    ) -> Self {
        let mut rng = WyRand::new(id as u64);
        let iq = rng.next_int(130, 181) as u32;
        let civ = Civilization::ALL[rng.next_int(0, Civilization::ALL.len() as i32) as usize];
        let leader = leader_for_civilization(civ);
        let final_color = color;
        Self {
            id,
            alive: true,
            player_type: PlayerType::Nation,
            name,
            color: final_color,
            troops: config.starting_troops,
            max_troops: config.max_troops_base,
            max_troops_cap: None,
            gold: config.starting_gold,
            has_spawned: false,
            sum_x: 0,
            sum_y: 0,
            tile_count: 0,
            border_tiles: DenseBitSet::new(),
            bot_rng: WyRand::new(id as u64),
            factories: 0,
            cities: 0,
            team: None,
            iq,
            iq_points: 0.0,
            alliances: Vec::new(),
            alliance_timers: std::collections::HashMap::new(),
            disconnected: false,
            is_ai_controlled: false,
            active_emoji: None,
            emoji_timer: 0,
            emoji_pinned: false,
            traitor: false,
            traitor_tick: 0,
            civilization: civ,
            leader,
            kills: 0,
            deaths: 0,
            assists: 0,
            tile_conquests: std::collections::BTreeMap::new(),
        }
    }
    pub fn is_human(&self) -> bool {
        self.player_type == PlayerType::Human
    }
    pub fn border_coords(&self, map_width: u32) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.border_tiles
            .ones()
            .map(move |idx| (idx % map_width, idx / map_width))
    }
    #[inline]
    pub fn border_insert(&mut self, idx: u32) {
        self.border_tiles.insert(idx);
    }
    #[inline]
    pub fn border_remove(&mut self, idx: u32) {
        self.border_tiles.remove(idx);
    }
}

pub fn tribe_animal(id: u16, name: &str) -> &'static str {
    if name.is_empty() {
        sow_data::animal_for_id(id)
    } else {
        sow_data::animal_for_name(name)
    }
}

/// Empire/nation emoji — same selection scheme as [`tribe_animal`] but a distinct category.
pub fn empire_emoji(id: u16, name: &str) -> &'static str {
    if name.is_empty() {
        sow_data::empire_emoji_for_id(id)
    } else {
        sow_data::empire_emoji_for_name(name)
    }
}

pub fn display_name(id: u16, name: &str, player_type: PlayerType) -> String {
    if name.is_empty() {
        match player_type {
            PlayerType::Bot => format!(
                "{} Tribe {}",
                tribe_animal(id, name),
                id.saturating_sub(199)
            ),
            PlayerType::Nation => format!("Nation {}", id.saturating_sub(103)),
            PlayerType::Human => format!("Player {}", id),
        }
    } else {
        match player_type {
            PlayerType::Bot => format!("{} {}", tribe_animal(id, name), name),
            _ => name.to_string(),
        }
    }
}
