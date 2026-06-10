use crate::bitset::DenseBitSet;
use crate::rng::NextIntExt;
use serde::{Deserialize, Serialize};
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
            Leader::Caesar => "Legions of Rome: Armies fight 10% stronger (lower losses, faster conquest).",
            Leader::Cleopatra => "Gift of the Nile: Factory districts generate +50% Gold.",
            Leader::Ragnar => "Longship Raid: Ports generate +50% Gold.",
            Leader::SunTzu => "Art of War: Factory districts produce troops 20% faster.",
            Leader::Alexander => "Great Conquest: Territory-conquering troops expand 15% faster.",
            Leader::GenghisKhan => "Horde Momentum: Gain 10% of gold spent by defeated enemies.",
            Leader::RichardTheLionheart => {
                "Crusader Fortresses: City districts grant +50% max troop capacity."
            }
            Leader::Vercingetorix => {
                "Hillfort Gaul: City districts generate +50% troop income."
            }
            Leader::Boudica => {
                "Iceni Revolt: City districts generate +50% Gold."
            }
            Leader::LadySixSky => {
                "Temple of the Sky: Factory districts generate +50% Gold."
            }
            Leader::Leonidas => {
                "Spartan Phalanx: Armory districts grant +50% max troop capacity."
            }
            Leader::Napoleon => {
                "Grande Armée: Territory-conquering troops expand 20% faster."
            }
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
            nameplate_x: 0.0,
            nameplate_y: 0.0,
            nameplate_size: 0.0,
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
        let is_smart_tribe = id.is_multiple_of(100);
        let starting_troops = config.starting_troops;
        let starting_gold = if is_smart_tribe {
            config.starting_gold
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
        let final_color = if config.game_mode == "Teams" {
            color
        } else {
            leader.filler_rgb()
        };
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
    pub fn calculate_nameplate(&mut self, map: &crate::map::GameMap) {
        if self.tile_count == 0 || self.border_tiles.is_empty() {
            self.nameplate_x = 0.0;
            self.nameplate_y = 0.0;
            self.nameplate_size = 0.0;
            return;
        }

        let mut min_x = u32::MAX;
        let mut min_y = u32::MAX;
        let mut max_x = 0;
        let mut max_y = 0;

        for idx in self.border_tiles.ones() {
            let x = idx % map.width;
            let y = idx / map.width;
            if x < min_x { min_x = x; }
            if y < min_y { min_y = y; }
            if x > max_x { max_x = x; }
            if y > max_y { max_y = y; }
        }

        let cx = if self.tile_count > 0 {
            (self.sum_x / self.tile_count as u64) as f32
        } else {
            0.0
        };
        let cy = if self.tile_count > 0 {
            (self.sum_y / self.tile_count as u64) as f32
        } else {
            0.0
        };

        if min_x == u32::MAX {
            self.nameplate_x = cx;
            self.nameplate_y = cy;
            self.nameplate_size = 12.0;
            return;
        }

        let width = max_x.saturating_sub(min_x) + 1;
        let height = max_y.saturating_sub(min_y) + 1;
        let size = width.min(height);

        let scaling_factor = if size < 25 {
            1
        } else if size < 50 {
            2
        } else if size < 100 {
            4
        } else if size < 250 {
            8
        } else if size < 500 {
            16
        } else {
            32
        };

        let scaled_min_x = min_x / scaling_factor;
        let scaled_min_y = min_y / scaling_factor;
        let scaled_max_x = max_x / scaling_factor;
        let scaled_max_y = max_y / scaling_factor;

        let grid_width = (scaled_max_x.saturating_sub(scaled_min_x) + 1) as usize;
        let grid_height = (scaled_max_y.saturating_sub(scaled_min_y) + 1) as usize;

        if grid_width == 0 || grid_height == 0 || grid_width > 1000 || grid_height > 1000 {
            self.nameplate_x = cx;
            self.nameplate_y = cy;
            self.nameplate_size = 12.0;
            return;
        }

        let mut grid = vec![vec![false; grid_height]; grid_width];

        for gx in 0..grid_width {
            for gy in 0..grid_height {
                let map_x = (scaled_min_x + gx as u32) * scaling_factor;
                let map_y = (scaled_min_y + gy as u32) * scaling_factor;

                if map_x < map.width && map_y < map.height {
                    let r = map.ref_id(map_x, map_y);
                    let tile = map.terrain[r];
                    let is_lake = tile.terrain_type() == crate::map::TerrainType::Lake;
                    let is_shore = tile.is_shoreline();
                    let is_owned = map.owner_id(map_x, map_y) == self.id;
                    grid[gx][gy] = is_owned || is_lake || is_shore;
                }
            }
        }

        let mut largest_rect = find_largest_inscribed_rectangle(&grid);
        if largest_rect.width == 0 || largest_rect.height == 0 {
            self.nameplate_x = cx;
            self.nameplate_y = cy;
            self.nameplate_size = 12.0;
            return;
        }

        largest_rect.x *= scaling_factor;
        largest_rect.y *= scaling_factor;
        largest_rect.width *= scaling_factor;
        largest_rect.height *= scaling_factor;

        let center_x = largest_rect.x + largest_rect.width / 2 + min_x;
        let center_y = largest_rect.y + largest_rect.height / 2 + min_y;

        let name_len = self.name.chars().count().max(1) as f32;
        let width_constrained = (largest_rect.width as f32 / name_len) * 2.0;
        let height_constrained = largest_rect.height as f32 / 3.0;
        let font_size = width_constrained.min(height_constrained).max(4.0);

        let nameplate_x = center_x as f32;
        let nameplate_y = center_y as f32 - (font_size / 3.0);

        self.nameplate_x = nameplate_x;
        self.nameplate_y = nameplate_y;
        self.nameplate_size = font_size;
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Rectangle {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
}

fn largest_rectangle_in_histogram(widths: &[u32]) -> Rectangle {
    let mut stack = Vec::new();
    let mut max_area = 0;
    let mut largest_rect = Rectangle::default();

    for i in 0..=widths.len() {
        let h = if i == widths.len() { 0 } else { widths[i] };

        while !stack.is_empty() && h < widths[*stack.last().unwrap()] {
            let height = widths[stack.pop().unwrap()];
            let width = if stack.is_empty() {
                i as u32
            } else {
                (i - *stack.last().unwrap() - 1) as u32
            };

            let area = height * width;
            if area > max_area {
                max_area = area;
                largest_rect = Rectangle {
                    x: if stack.is_empty() { 0 } else { (*stack.last().unwrap() + 1) as u32 },
                    y: 0,
                    width,
                    height,
                };
            }
        }
        stack.push(i);
    }

    largest_rect
}

fn find_largest_inscribed_rectangle(grid: &[Vec<bool>]) -> Rectangle {
    if grid.is_empty() || grid[0].is_empty() {
        return Rectangle::default();
    }
    let cols = grid.len();
    let rows = grid[0].len();
    let mut heights = vec![0u32; cols];
    let mut largest_rect = Rectangle::default();

    for row in 0..rows {
        for col in 0..cols {
            if grid[col][row] {
                heights[col] += 1;
            } else {
                heights[col] = 0;
            }
        }

        let rect_for_row = largest_rectangle_in_histogram(&heights);

        if rect_for_row.width * rect_for_row.height > largest_rect.width * largest_rect.height {
            largest_rect = Rectangle {
                x: rect_for_row.x,
                y: (row as u32).saturating_sub(rect_for_row.height).saturating_add(1),
                width: rect_for_row.width,
                height: rect_for_row.height,
            };
        }
    }

    largest_rect
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

pub fn premium_color(index: usize) -> [f32; 3] {
    let signature_colors: [[f32; 3]; 12] = [
        [0.75, 0.15, 0.18], // Caesar (Rome Crimson)
        [0.85, 0.65, 0.15], // Cleopatra (Egypt Gold)
        [0.15, 0.35, 0.65], // Ragnar (Vikings Blue)
        [0.15, 0.55, 0.42], // Sun Tzu (China Jade)
        [0.22, 0.45, 0.78], // Alexander (Macedon Blue)
        [0.55, 0.42, 0.22], // Genghis Khan (Mongol Bronze)
        [0.72, 0.18, 0.15], // Richard (Angevin Crimson)
        [0.28, 0.52, 0.22], // Vercingetorix (Gallic Green)
        [0.88, 0.42, 0.12], // Boudica (Iceni Orange)
        [0.12, 0.58, 0.52], // Lady Six Sky (Maya Teal)
        [0.62, 0.42, 0.22], // Leonidas (Sparta Bronze)
        [0.18, 0.28, 0.68], // Napoleon (France Blue)
    ];

    if index < 12 {
        return signature_colors[index];
    }

    // High density premium spectrum (18 hues x 6 variations)
    let hue_idx = (index - 12) % 18;
    let variant_idx = ((index - 12) / 18) % 6;
    let h = hue_idx as f32 / 18.0;

    let (s, v) = match variant_idx {
        0 => (0.85, 0.80), // Super Vibrant / Neon
        1 => (0.90, 0.55), // Deep / Rich
        2 => (0.95, 0.40), // Royal / Midnight
        3 => (0.65, 0.85), // Bright / Warm-Neon
        4 => (0.75, 0.65), // Rich Earthy / Jewel
        _ => (0.80, 0.45), // Deep Velvet / Wine
    };

    hsv_to_rgb(h, s, v)
}

/// RGB used for human-owned territory in the sow-render map shader (`map.wgsl`).
/// Matches WGSL `owner_id <= 16` branch so UI (nameplates) matches the map tint.
#[inline]
pub fn human_shader_territory_rgb(player_id: u16) -> [f32; 3] {
    if player_id >= 1 && player_id <= 120 {
        premium_color((player_id as usize) - 1)
    } else {
        // Fallback for massive games beyond 120 major actors:
        // Ensures strong, deep, and interesting tones (no pastel, no dull colors)
        let hue = (player_id as f32 * 0.618033988749895).fract();
        let s = 0.60 + ((player_id as f32 * 1.6180339887).fract() * 0.35); // 0.60 to 0.95
        let v = 0.40 + ((player_id as f32 * 2.6180339887).fract() * 0.40); // 0.40 to 0.80
        hsv_to_rgb(hue, s, v)
    }
}

pub fn bot_territory_color(game_seed: u64, bot_id: u16) -> [f32; 3] {
    let mix = game_seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (bot_id as u64).wrapping_shl(32)
        ^ (bot_id as u64);
    let mut rng = WyRand::new(mix);
    let h = rng.next_int(0, 10_000) as f32 / 10_000.0;
    
    // For major city states below ID 120, use premium colors directly
    if bot_id >= 1 && bot_id <= 120 {
        return premium_color((bot_id as usize) - 1);
    }
    
    let is_high_iq = bot_id % 100 == 0;
    let (s, v) = if is_high_iq {
        // Darker, richer colors — visually distinct as apex predators
        let s = 0.65 + (rng.next_int(0, 1000) as f32 / 1000.0 * 0.25); // 0.65 to 0.90
        let v = 0.25 + (rng.next_int(0, 1000) as f32 / 1000.0 * 0.20); // 0.25 to 0.45
        (s, v)
    } else {
        // Tribes use strong, rich, deep colors:
        // s is 0.60 to 0.95 (completely avoiding pastels/light washes)
        // v is 0.40 to 0.80 (solid and rich, preventing overly white/washed tones)
        let s = 0.60 + (rng.next_int(0, 1000) as f32 / 1000.0 * 0.35);
        let v = 0.40 + (rng.next_int(0, 1000) as f32 / 1000.0 * 0.40);
        (s, v)
    };
    let [r, g, b] = hsv_to_rgb(h, s, v);
    [
        r.clamp(0.05, 0.95),
        g.clamp(0.05, 0.95),
        b.clamp(0.05, 0.95),
    ]
}

pub fn tribe_animal(id: u16) -> &'static str {
    const ANIMALS: [&str; 40] = [
        "🦁", "🐯", "🐆", "🐺", "🦊", "🦝", "🐻", "🐨", "🐼", "🐗",
        "🦄", "🦅", "🦉", "🐊", "🦖", "🐉", "🦈", "🦂", "🐃", "🐏",
        "🐘", "🦏", "🦍", "🐎", "🦌", "🦇", "🦢", "🦩", "🐍", "🐢",
        "🐙", "🐬", "🐝", "🦋", "🕷️", "🦦", "🦫", "🐫", "🦘", "🦡",
    ];
    ANIMALS[(id as usize) % ANIMALS.len()]
}

pub fn display_name(id: u16, name: &str) -> String {
    if name.is_empty() {
        if id >= 200 {
            format!("{} Tribe {}", tribe_animal(id), id - 199)
        } else if id >= 103 {
            format!("Nation {}", id - 103)
        } else {
            format!("Player {}", id)
        }
    } else {
        if id >= 200 {
            format!("{} {}", tribe_animal(id), name)
        } else {
            name.to_string()
        }
    }
}

