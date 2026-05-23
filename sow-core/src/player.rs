use crate::bitset::DenseBitSet;
use crate::rng::NextIntExt;
use serde::{Deserialize, Serialize};
use wyrand::WyRand;

pub type PlayerId = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Civilization {
    Rome,
    Egypt,
    Vikings,
    China,
    Macedon,
    Mongols,
}

impl Default for Civilization {
    fn default() -> Self {
        Civilization::Rome
    }
}

impl Civilization {
    pub const ALL: [Civilization; 6] = [
        Civilization::Rome,
        Civilization::Egypt,
        Civilization::Vikings,
        Civilization::China,
        Civilization::Macedon,
        Civilization::Mongols,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Civilization::Rome => "Roman Empire",
            Civilization::Egypt => "Egyptian Empire",
            Civilization::Vikings => "Norse Kingdom",
            Civilization::China => "Chinese Empire",
            Civilization::Macedon => "Macedonian Empire",
            Civilization::Mongols => "Mongol Horde",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Leader {
    Caesar,
    Cleopatra,
    Ragnar,
    SunTzu,
    Alexander,
    GenghisKhan,
}

impl Default for Leader {
    fn default() -> Self {
        Leader::Caesar
    }
}

impl Leader {
    pub const ALL: [Leader; 6] = [
        Leader::Caesar,
        Leader::Cleopatra,
        Leader::Ragnar,
        Leader::SunTzu,
        Leader::Alexander,
        Leader::GenghisKhan,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Leader::Caesar => "Caesar",
            Leader::Cleopatra => "Cleopatra",
            Leader::Ragnar => "Ragnar",
            Leader::SunTzu => "Sun Tzu",
            Leader::Alexander => "Alexander",
            Leader::GenghisKhan => "Genghis Khan",
        }
    }

    pub fn perk_description(self) -> &'static str {
        match self {
            Leader::Caesar => "Pax Romana: Starts match with a free Military district.",
            Leader::Cleopatra => "Gift of the Nile: Industry districts generate +50% Gold.",
            Leader::Ragnar => "Longship Raid: Ports generate +50% Gold.",
            Leader::SunTzu => "Art of War: Military districts produce troops 20% faster.",
            Leader::Alexander => "Great Conquest: Territory-conquering troops expand 15% faster.",
            Leader::GenghisKhan => "Horde Momentum: Gain 10% of gold spent by defeated enemies.",
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
        }
    }
    pub fn new_bot(
        id: u16,
        name: String,
        color: [f32; 3],
        config: &crate::game_config::GameConfig,
    ) -> Self {
        let mut rng = WyRand::new(id as u64);
        let iq = rng.next_int(80, 131) as u32;
        let civ = Civilization::ALL[rng.next_int(0, Civilization::ALL.len() as i32) as usize];
        let leader = match civ {
            Civilization::Rome => Leader::Caesar,
            Civilization::Egypt => Leader::Cleopatra,
            Civilization::Vikings => Leader::Ragnar,
            Civilization::China => Leader::SunTzu,
            Civilization::Macedon => Leader::Alexander,
            Civilization::Mongols => Leader::GenghisKhan,
        };
        Self {
            id,
            alive: true,
            player_type: PlayerType::Bot,
            name,
            color,
            troops: config.starting_troops,
            max_troops: config.max_troops_base,
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
            active_emoji: None,
            emoji_timer: 0,
            emoji_pinned: false,
            traitor: false,
            traitor_tick: 0,
            civilization: civ,
            leader,
        }
    }
    pub fn new_nation(
        id: u16,
        name: String,
        color: [f32; 3],
        config: &crate::game_config::GameConfig,
    ) -> Self {
        let mut rng = WyRand::new(id as u64);
        let iq = rng.next_int(110, 161) as u32;
        let civ = Civilization::ALL[rng.next_int(0, Civilization::ALL.len() as i32) as usize];
        let leader = match civ {
            Civilization::Rome => Leader::Caesar,
            Civilization::Egypt => Leader::Cleopatra,
            Civilization::Vikings => Leader::Ragnar,
            Civilization::China => Leader::SunTzu,
            Civilization::Macedon => Leader::Alexander,
            Civilization::Mongols => Leader::GenghisKhan,
        };
        Self {
            id,
            alive: true,
            player_type: PlayerType::Nation,
            name,
            color,
            troops: config.starting_troops,
            max_troops: config.max_troops_base,
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
            active_emoji: None,
            emoji_timer: 0,
            emoji_pinned: false,
            traitor: false,
            traitor_tick: 0,
            civilization: civ,
            leader,
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

pub fn player_colors() -> Vec<[f32; 3]> {
    vec![
        [0.0, 1.0, 1.0],
        [1.0, 0.02, 0.08],
        [0.02, 1.0, 0.08],
        [1.0, 1.0, 0.0],
        [1.0, 0.05, 0.95],
        [0.45, 0.08, 1.0],
        [1.0, 0.32, 0.0],
        [0.0, 0.98, 0.88],
    ]
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = h.clamp(0.0, 1.0).fract();
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);
    let i = (h * 6.0).floor() as i32;
    let f = h * 6.0 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - f * s);
    let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [r, g, b]
}

/// RGB used for human-owned territory in the sow-render map shader (`map.wgsl`).
/// Matches WGSL `owner_id <= 16` branch so UI (nameplates) matches the map tint.
#[inline]
pub fn human_shader_territory_rgb(player_id: u16) -> [f32; 3] {
    let hue = player_id as f32 * 0.618_034;
    let fract = |x: f32| x - x.floor();
    let r = (fract(hue) * 2.0 - 1.0).abs();
    let g = (fract(hue + 0.333) * 2.0 - 1.0).abs();
    let b = (fract(hue + 0.666) * 2.0 - 1.0).abs();
    [r, g, b]
}

pub fn bot_territory_color(game_seed: u64, bot_id: u16) -> [f32; 3] {
    let mix = game_seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (bot_id as u64).wrapping_shl(32)
        ^ (bot_id as u64);
    let mut rng = WyRand::new(mix);
    let h = rng.next_int(0, 10_000) as f32 / 10_000.0;
    let s = 0.28 + rng.next_int(0, 1000) as f32 / 1000.0 * 0.18;
    let v = 0.52 + rng.next_int(0, 1000) as f32 / 1000.0 * 0.18;
    let [r, g, b] = hsv_to_rgb(h, s, v);
    [
        r.clamp(0.16, 0.88),
        g.clamp(0.16, 0.88),
        b.clamp(0.16, 0.88),
    ]
}
