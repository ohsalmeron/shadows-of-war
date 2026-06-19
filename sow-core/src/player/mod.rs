use crate::bitset::DenseBitSet;
use crate::rng::NextIntExt;
use serde::{Deserialize, Serialize};
mod colors;
mod nameplate;

pub use colors::{
    bot_territory_color, human_shader_territory_rgb, premium_color, NamedColor, PREMIUM_COLORS,
};

use wyrand::WyRand;

pub type PlayerId = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Civilization {
    #[default]
    Rome,
    Egypt,
    Vikings,
    China,
    Macedon,
    Mongols,
    Angevin,
    Gallic,
    Iceni,
    Maya,
    Sparta,
    France,
}

impl Civilization {
    pub const ALL: [Civilization; 12] = [
        Civilization::Rome,
        Civilization::Egypt,
        Civilization::Vikings,
        Civilization::China,
        Civilization::Macedon,
        Civilization::Mongols,
        Civilization::Angevin,
        Civilization::Gallic,
        Civilization::Iceni,
        Civilization::Maya,
        Civilization::Sparta,
        Civilization::France,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Civilization::Rome => "Roman Empire",
            Civilization::Egypt => "Egyptian Empire",
            Civilization::Vikings => "Norse Kingdom",
            Civilization::China => "Chinese Empire",
            Civilization::Macedon => "Macedonian Empire",
            Civilization::Mongols => "Mongol Horde",
            Civilization::Angevin => "Angevin Empire",
            Civilization::Gallic => "Gallic Tribes",
            Civilization::Iceni => "Iceni Kingdom",
            Civilization::Maya => "Maya Civilization",
            Civilization::Sparta => "Sparta",
            Civilization::France => "Kingdom of France",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Leader {
    #[default]
    Caesar,
    Cleopatra,
    Ragnar,
    SunTzu,
    Alexander,
    GenghisKhan,
    RichardTheLionheart,
    Vercingetorix,
    Boudica,
    LadySixSky,
    Leonidas,
    Napoleon,
}

impl Leader {
    pub const ALL: [Leader; 12] = [
        Leader::Caesar,
        Leader::Cleopatra,
        Leader::Ragnar,
        Leader::SunTzu,
        Leader::Alexander,
        Leader::GenghisKhan,
        Leader::RichardTheLionheart,
        Leader::Vercingetorix,
        Leader::Boudica,
        Leader::LadySixSky,
        Leader::Leonidas,
        Leader::Napoleon,
    ];

    /// Menu / HUD emoji for this leader.
    pub fn menu_emoji(self) -> &'static str {
        match self {
            Leader::Caesar => "🏛️",
            Leader::Cleopatra => "👑",
            Leader::Ragnar => "🪓",
            Leader::SunTzu => "📜",
            Leader::Alexander => "🛡️",
            Leader::GenghisKhan => "🐺",
            Leader::RichardTheLionheart => "🦁",
            Leader::Vercingetorix => "⚔️",
            Leader::Boudica => "🔥",
            Leader::LadySixSky => "🌙",
            Leader::Leonidas => "🪖",
            Leader::Napoleon => "🎖️",
        }
    }

    /// Brand / placeholder fill color (linear RGB 0..1). Used behind portraits and for human territory tint.
    pub fn filler_rgb(self) -> [f32; 3] {
        match self {
            Leader::Caesar => [0.75, 0.15, 0.18],
            Leader::Cleopatra => [0.85, 0.65, 0.15],
            Leader::Ragnar => [0.15, 0.35, 0.65],
            Leader::SunTzu => [0.15, 0.55, 0.42],
            Leader::Alexander => [0.22, 0.45, 0.78],
            Leader::GenghisKhan => [0.55, 0.42, 0.22],
            Leader::RichardTheLionheart => [0.72, 0.18, 0.15],
            Leader::Vercingetorix => [0.28, 0.52, 0.22],
            Leader::Boudica => [0.88, 0.42, 0.12],
            Leader::LadySixSky => [0.12, 0.58, 0.52],
            Leader::Leonidas => [0.62, 0.42, 0.22],
            Leader::Napoleon => [0.18, 0.28, 0.68],
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Leader::Caesar => "Caesar",
            Leader::Cleopatra => "Cleopatra",
            Leader::Ragnar => "Ragnar",
            Leader::SunTzu => "Sun Tzu",
            Leader::Alexander => "Alexander",
            Leader::GenghisKhan => "Genghis Khan",
            Leader::RichardTheLionheart => "Richard the Lionheart",
            Leader::Vercingetorix => "Vercingetorix",
            Leader::Boudica => "Boudica",
            Leader::LadySixSky => "Lady Six Sky",
            Leader::Leonidas => "Leonidas",
            Leader::Napoleon => "Napoleon",
        }
    }

    /// Combat multiplier for troop losses and expansion power (1.0 = normal).
    pub fn troop_strength_multiplier(self) -> f64 {
        match self {
            Leader::Caesar => 1.10,
            _ => 1.0,
        }
    }

    pub fn perk_description(self) -> &'static str {
        match self {
            Leader::Caesar => {
                "Legions of Rome: Armies fight 10% stronger (lower losses, faster conquest)."
            }
            Leader::Cleopatra => "Gift of the Nile: Factory districts generate +50% Gold.",
            Leader::Ragnar => "Longship Raid: Ports generate +50% Gold.",
            Leader::SunTzu => "Art of War: Factory districts produce troops 20% faster.",
            Leader::Alexander => "Great Conquest: Territory-conquering troops expand 15% faster.",
            Leader::GenghisKhan => "Horde Momentum: Gain 10% of gold spent by defeated enemies.",
            Leader::RichardTheLionheart => {
                "Crusader Fortresses: City districts grant +50% max troop capacity."
            }
            Leader::Vercingetorix => "Hillfort Gaul: City districts generate +50% troop income.",
            Leader::Boudica => "Iceni Revolt: City districts generate +50% Gold.",
            Leader::LadySixSky => "Temple of the Sky: Factory districts generate +50% Gold.",
            Leader::Leonidas => "Spartan Phalanx: Armory districts grant +50% max troop capacity.",
            Leader::Napoleon => "Grande Armée: Territory-conquering troops expand 20% faster.",
        }
    }

    pub fn civilization(self) -> Civilization {
        match self {
            Leader::Caesar => Civilization::Rome,
            Leader::Cleopatra => Civilization::Egypt,
            Leader::Ragnar => Civilization::Vikings,
            Leader::SunTzu => Civilization::China,
            Leader::Alexander => Civilization::Macedon,
            Leader::GenghisKhan => Civilization::Mongols,
            Leader::RichardTheLionheart => Civilization::Angevin,
            Leader::Vercingetorix => Civilization::Gallic,
            Leader::Boudica => Civilization::Iceni,
            Leader::LadySixSky => Civilization::Maya,
            Leader::Leonidas => Civilization::Sparta,
            Leader::Napoleon => Civilization::France,
        }
    }
}

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
    pub nameplate_x: f32,
    #[serde(default)]
    pub nameplate_y: f32,
    #[serde(default)]
    pub nameplate_size: f32,
    #[serde(default)]
    pub nameplate_dirty: bool,
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
            gold: config.starting_gold,
            has_spawned: false,
            sum_x: 0,
            sum_y: 0,
            tile_count: 0,
            nameplate_x: 0.0,
            nameplate_y: 0.0,
            nameplate_size: 0.0,
            nameplate_dirty: true,
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
            rng.next_int(130, 181) as u32 // 1% Smartest tribes (Nation disguise)
        } else if id.is_multiple_of(10) {
            rng.next_int(100, 121) as u32 // 9% Advanced tribes
        } else if id % 10 == 1 {
            rng.next_int(60, 81) as u32 // 10% Stupidest tribes
        } else {
            rng.next_int(85, 106) as u32 // 80% Standard baseline tribes
        };
        let civ = Civilization::ALL[rng.next_int(0, Civilization::ALL.len() as i32) as usize];
        let leader = match civ {
            Civilization::Rome => Leader::Caesar,
            Civilization::Egypt => Leader::Cleopatra,
            Civilization::Vikings => Leader::Ragnar,
            Civilization::China => Leader::SunTzu,
            Civilization::Macedon => Leader::Alexander,
            Civilization::Mongols => Leader::GenghisKhan,
            Civilization::Angevin => Leader::RichardTheLionheart,
            Civilization::Gallic => Leader::Vercingetorix,
            Civilization::Iceni => Leader::Boudica,
            Civilization::Maya => Leader::LadySixSky,
            Civilization::Sparta => Leader::Leonidas,
            Civilization::France => Leader::Napoleon,
        };
        let starting_troops = config.starting_troops;
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
            gold: starting_gold,
            has_spawned: false,
            sum_x: 0,
            sum_y: 0,
            tile_count: 0,
            nameplate_x: 0.0,
            nameplate_y: 0.0,
            nameplate_size: 0.0,
            nameplate_dirty: true,
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
        let leader = match civ {
            Civilization::Rome => Leader::Caesar,
            Civilization::Egypt => Leader::Cleopatra,
            Civilization::Vikings => Leader::Ragnar,
            Civilization::China => Leader::SunTzu,
            Civilization::Macedon => Leader::Alexander,
            Civilization::Mongols => Leader::GenghisKhan,
            Civilization::Angevin => Leader::RichardTheLionheart,
            Civilization::Gallic => Leader::Vercingetorix,
            Civilization::Iceni => Leader::Boudica,
            Civilization::Maya => Leader::LadySixSky,
            Civilization::Sparta => Leader::Leonidas,
            Civilization::France => Leader::Napoleon,
        };
        let final_color = color;
        Self {
            id,
            alive: true,
            player_type: PlayerType::Nation,
            name,
            color: final_color,
            troops: config.starting_troops,
            max_troops: config.max_troops_base,
            gold: config.starting_gold,
            has_spawned: false,
            sum_x: 0,
            sum_y: 0,
            tile_count: 0,
            nameplate_x: 0.0,
            nameplate_y: 0.0,
            nameplate_size: 0.0,
            nameplate_dirty: true,
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
        self.nameplate_dirty = true;
    }
    #[inline]
    pub fn border_remove(&mut self, idx: u32) {
        self.border_tiles.remove(idx);
        self.nameplate_dirty = true;
    }
}

pub fn tribe_animal(id: u16, name: &str) -> &'static str {
    if name.is_empty() {
        crate::tribes::animal_for_id(id)
    } else {
        crate::tribes::animal_for_name(name)
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
