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
    pub fn calculate_nameplate(&mut self, map: &crate::map::GameMap) {
        if self.tile_count == 0 || self.border_tiles.is_empty() {
            self.nameplate_x = 0.0;
            self.nameplate_y = 0.0;
            self.nameplate_size = 0.0;
            self.nameplate_dirty = false;
            return;
        }

        let Some((min_x, min_y, max_x, max_y)) =
            largest_owned_cluster_bbox(map, self.id, &self.border_tiles)
        else {
            self.nameplate_x = 0.0;
            self.nameplate_y = 0.0;
            self.nameplate_size = 0.0;
            self.nameplate_dirty = false;
            return;
        };

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
            let prefer_x = min_x + (max_x.saturating_sub(min_x)) / 2;
            let prefer_y = min_y + (max_y.saturating_sub(min_y)) / 2;
            let (cx, cy) =
                nearest_owned_land(map, self.id, prefer_x, prefer_y, min_x, min_y, max_x, max_y);
            self.nameplate_x = cx as f32;
            self.nameplate_y = cy as f32;
            self.nameplate_size = 1.5;
            self.nameplate_dirty = false;
            return;
        }

        let mut grid = vec![vec![false; grid_height]; grid_width];

        for (gx, column) in grid.iter_mut().enumerate().take(grid_width) {
            for (gy, item) in column.iter_mut().enumerate().take(grid_height) {
                let map_x = (scaled_min_x + gx as u32) * scaling_factor;
                let map_y = (scaled_min_y + gy as u32) * scaling_factor;

                if map_x < map.width && map_y < map.height {
                    *item = nameplate_grid_valid(map, self.id, map_x, map_y);
                }
            }
        }

        let mut largest_rect = find_largest_inscribed_rectangle(&grid);
        if largest_rect.width == 0 || largest_rect.height == 0 {
            let prefer_x = min_x + (max_x.saturating_sub(min_x)) / 2;
            let prefer_y = min_y + (max_y.saturating_sub(min_y)) / 2;
            let (cx, cy) =
                nearest_owned_land(map, self.id, prefer_x, prefer_y, min_x, min_y, max_x, max_y);
            self.nameplate_x = cx as f32;
            self.nameplate_y = cy as f32;
            self.nameplate_size = 1.5;
            self.nameplate_dirty = false;
            return;
        }

        largest_rect.x *= scaling_factor;
        largest_rect.y *= scaling_factor;
        largest_rect.width *= scaling_factor;
        largest_rect.height *= scaling_factor;

        let center_x = largest_rect.x + largest_rect.width / 2 + min_x;
        let center_y = largest_rect.y + largest_rect.height / 2 + min_y;
        let (land_x, land_y) =
            nearest_owned_land(map, self.id, center_x, center_y, min_x, min_y, max_x, max_y);

        let name_len = self.name.chars().count().max(1) as f32;
        let width_constrained = (largest_rect.width as f32 / name_len) * 2.0;
        let height_constrained = largest_rect.height as f32 / 3.0;
        let font_size = width_constrained.min(height_constrained).clamp(0.2, 24.0);

        self.nameplate_x = land_x as f32;
        self.nameplate_y = land_y as f32 - (font_size / 3.0);
        self.nameplate_size = font_size;
        self.nameplate_dirty = false;
    }
}

fn largest_owned_cluster_bbox(
    map: &crate::map::GameMap,
    player_id: u16,
    border_tiles: &crate::bitset::DenseBitSet,
) -> Option<(u32, u32, u32, u32)> {
    let map_size = (map.width * map.height) as usize;
    let mut visited = vec![false; map_size];

    let mut largest_min_x = u32::MAX;
    let mut largest_min_y = u32::MAX;
    let mut largest_max_x = 0;
    let mut largest_max_y = 0;
    let mut max_cluster_size = 0;

    for start_idx in border_tiles.ones() {
        if visited[start_idx as usize] {
            continue;
        }
        let sx = start_idx % map.width;
        let sy = start_idx / map.width;
        if map.owner_id(sx, sy) != player_id {
            continue;
        }

        let mut queue = vec![start_idx];
        visited[start_idx as usize] = true;

        let mut cluster_min_x = u32::MAX;
        let mut cluster_min_y = u32::MAX;
        let mut cluster_max_x = 0;
        let mut cluster_max_y = 0;
        let mut count = 0;

        let mut q_idx = 0;
        while q_idx < queue.len() {
            let curr = queue[q_idx];
            q_idx += 1;

            let cx = curr % map.width;
            let cy = curr / map.width;

            if cx < cluster_min_x {
                cluster_min_x = cx;
            }
            if cy < cluster_min_y {
                cluster_min_y = cy;
            }
            if cx > cluster_max_x {
                cluster_max_x = cx;
            }
            if cy > cluster_max_y {
                cluster_max_y = cy;
            }
            count += 1;

            map.for_each_neighbor(cx, cy, |nx, ny| {
                let n_idx = (ny * map.width + nx) as usize;
                if map.owner_id(nx, ny) == player_id && !visited[n_idx] {
                    visited[n_idx] = true;
                    queue.push(ny * map.width + nx);
                }
            });
        }

        if count > max_cluster_size {
            max_cluster_size = count;
            largest_min_x = cluster_min_x;
            largest_min_y = cluster_min_y;
            largest_max_x = cluster_max_x;
            largest_max_y = cluster_max_y;
        }
    }

    if largest_min_x == u32::MAX {
        None
    } else {
        Some((largest_min_x, largest_min_y, largest_max_x, largest_max_y))
    }
}

fn nameplate_grid_valid(map: &crate::map::GameMap, player_id: u16, map_x: u32, map_y: u32) -> bool {
    let r = map.ref_id(map_x, map_y);
    let tile = map.terrain[r];
    let is_owned = map.owner_id(map_x, map_y) == player_id;
    let is_lake = tile.terrain_type() == crate::map::TerrainType::Lake;
    let is_shore = tile.is_land() && tile.is_shoreline();
    is_owned || is_lake || is_shore
}

#[allow(clippy::too_many_arguments)]
fn nearest_owned_land(
    map: &crate::map::GameMap,
    player_id: u16,
    prefer_x: u32,
    prefer_y: u32,
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
) -> (u32, u32) {
    let px = prefer_x.min(map.width.saturating_sub(1));
    let py = prefer_y.min(map.height.saturating_sub(1));
    let r = map.ref_id(px, py);
    if map.terrain[r].is_land() && map.owner_id(px, py) == player_id {
        return (px, py);
    }

    let mut best = (px, py);
    let mut best_dist = u32::MAX;
    for y in min_y..=max_y.min(map.height.saturating_sub(1)) {
        for x in min_x..=max_x.min(map.width.saturating_sub(1)) {
            if map.owner_id(x, y) != player_id {
                continue;
            }
            if !map.terrain[map.ref_id(x, y)].is_land() {
                continue;
            }
            let dist = px.abs_diff(x) + py.abs_diff(y);
            if dist < best_dist {
                best_dist = dist;
                best = (x, y);
            }
        }
    }
    best
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
                    x: if stack.is_empty() {
                        0
                    } else {
                        (*stack.last().unwrap() + 1) as u32
                    },
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

    #[allow(clippy::needless_range_loop)]
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
                y: (row as u32)
                    .saturating_sub(rect_for_row.height)
                    .saturating_add(1),
                width: rect_for_row.width,
                height: rect_for_row.height,
            };
        }
    }

    largest_rect
}

const fn hex_to_rgb(hex: u32) -> [f32; 3] {
    let r = ((hex >> 16) & 0xFF) as f32 / 255.0;
    let g = ((hex >> 8) & 0xFF) as f32 / 255.0;
    let b = (hex & 0xFF) as f32 / 255.0;
    [r, g, b]
}

fn rgb_to_hsv(r: f32, g: f32, b: f32) -> [f32; 3] {
    let min = r.min(g).min(b);
    let max = r.max(g).max(b);
    let delta = max - min;
    let v = max;
    let s = if max == 0.0 { 0.0 } else { delta / max };
    let mut h = 0.0;
    if delta > 0.00001 {
        if r == max {
            h = (g - b) / delta;
        } else if g == max {
            h = 2.0 + (b - r) / delta;
        } else {
            h = 4.0 + (r - g) / delta;
        }
        h /= 6.0;
        if h < 0.0 {
            h += 1.0;
        }
    }
    [h, s, v]
}

pub struct NamedColor {
    pub name: &'static str,
    pub rgb: [f32; 3],
}

pub static PREMIUM_COLORS: [NamedColor; 300] = [
    // --- The 12 Major Leaders (Signature Colors) ---
    NamedColor {
        name: "Rome Crimson",
        rgb: [0.75, 0.15, 0.18],
    },
    NamedColor {
        name: "Egypt Gold",
        rgb: [0.85, 0.65, 0.15],
    },
    NamedColor {
        name: "Vikings Blue",
        rgb: [0.15, 0.35, 0.65],
    },
    NamedColor {
        name: "China Jade",
        rgb: [0.15, 0.55, 0.42],
    },
    NamedColor {
        name: "Macedon Blue",
        rgb: [0.22, 0.45, 0.78],
    },
    NamedColor {
        name: "Mongol Bronze",
        rgb: [0.55, 0.42, 0.22],
    },
    NamedColor {
        name: "Angevin Crimson",
        rgb: [0.72, 0.18, 0.15],
    },
    NamedColor {
        name: "Gallic Green",
        rgb: [0.28, 0.52, 0.22],
    },
    NamedColor {
        name: "Iceni Orange",
        rgb: [0.88, 0.42, 0.12],
    },
    NamedColor {
        name: "Maya Teal",
        rgb: [0.12, 0.58, 0.52],
    },
    NamedColor {
        name: "Sparta Bronze",
        rgb: [0.62, 0.42, 0.22],
    },
    NamedColor {
        name: "France Blue",
        rgb: [0.18, 0.28, 0.68],
    },
    // --- The Green Sanctuary (70 Colors) ---
    NamedColor {
        name: "Cyber Neon Green",
        rgb: hex_to_rgb(0x00FF66),
    },
    NamedColor {
        name: "Neon Lime",
        rgb: hex_to_rgb(0x39FF14),
    },
    NamedColor {
        name: "Teal Wave",
        rgb: hex_to_rgb(0x1FCECB),
    },
    NamedColor {
        name: "Classic Teal",
        rgb: hex_to_rgb(0x008080),
    },
    NamedColor {
        name: "Sea Green",
        rgb: hex_to_rgb(0x2E8B57),
    },
    NamedColor {
        name: "Spring Green",
        rgb: hex_to_rgb(0x00FF7F),
    },
    NamedColor {
        name: "Chartreuse",
        rgb: hex_to_rgb(0x7FFF00),
    },
    NamedColor {
        name: "Lawn Green",
        rgb: hex_to_rgb(0x7CFC00),
    },
    NamedColor {
        name: "Green Yellow",
        rgb: hex_to_rgb(0xADFF2F),
    },
    NamedColor {
        name: "Pale Green",
        rgb: hex_to_rgb(0x98FB98),
    },
    NamedColor {
        name: "Electric Lime",
        rgb: hex_to_rgb(0x00FF00),
    },
    NamedColor {
        name: "Lime Green",
        rgb: hex_to_rgb(0x32CD32),
    },
    NamedColor {
        name: "Forest Green",
        rgb: hex_to_rgb(0x228B22),
    },
    NamedColor {
        name: "Dark Green",
        rgb: hex_to_rgb(0x006400),
    },
    NamedColor {
        name: "Dark Olive Green",
        rgb: hex_to_rgb(0x556B2F),
    },
    NamedColor {
        name: "Olive Drab",
        rgb: hex_to_rgb(0x6B8E23),
    },
    NamedColor {
        name: "Yellow Green",
        rgb: hex_to_rgb(0x9ACD32),
    },
    NamedColor {
        name: "Light Sea Green",
        rgb: hex_to_rgb(0x20B2AA),
    },
    NamedColor {
        name: "Medium Sea Green",
        rgb: hex_to_rgb(0x3CB371),
    },
    NamedColor {
        name: "Dark Sea Green",
        rgb: hex_to_rgb(0x8FBC8F),
    },
    NamedColor {
        name: "Medium Aquamarine",
        rgb: hex_to_rgb(0x66CDAA),
    },
    NamedColor {
        name: "Deep Sea Green",
        rgb: hex_to_rgb(0x458B74),
    },
    NamedColor {
        name: "Reptile Green",
        rgb: hex_to_rgb(0x00C78C),
    },
    NamedColor {
        name: "Jade",
        rgb: hex_to_rgb(0x00A86B),
    },
    NamedColor {
        name: "Emerald Isle",
        rgb: hex_to_rgb(0x50C878),
    },
    NamedColor {
        name: "Deep Forest Moss",
        rgb: hex_to_rgb(0x1F807C),
    },
    NamedColor {
        name: "Rich Racing Green",
        rgb: hex_to_rgb(0x286D49),
    },
    NamedColor {
        name: "Fern Green",
        rgb: hex_to_rgb(0x4F7942),
    },
    NamedColor {
        name: "Sagebrush",
        rgb: hex_to_rgb(0x8A9A5B),
    },
    NamedColor {
        name: "Tea Matcha",
        rgb: hex_to_rgb(0xD0F0C0),
    },
    NamedColor {
        name: "Pistachio",
        rgb: hex_to_rgb(0x93C572),
    },
    NamedColor {
        name: "Persian Green",
        rgb: hex_to_rgb(0x17B890),
    },
    NamedColor {
        name: "Viridian",
        rgb: hex_to_rgb(0x40826D),
    },
    NamedColor {
        name: "Laurel Green",
        rgb: hex_to_rgb(0x00A572),
    },
    NamedColor {
        name: "Clover",
        rgb: hex_to_rgb(0x0B6623),
    },
    NamedColor {
        name: "Eucalyptus",
        rgb: hex_to_rgb(0x4A5D4E),
    },
    NamedColor {
        name: "Dark Slate",
        rgb: hex_to_rgb(0x2F4F4F),
    },
    NamedColor {
        name: "Olive",
        rgb: hex_to_rgb(0x808000),
    },
    NamedColor {
        name: "Amazon Green",
        rgb: hex_to_rgb(0x3B7A57),
    },
    NamedColor {
        name: "Earthy Sage",
        rgb: hex_to_rgb(0x5F8575),
    },
    // Expanded Greens
    NamedColor {
        name: "Vibrant Scale Green",
        rgb: hex_to_rgb(0x567D48),
    },
    NamedColor {
        name: "Amazonia",
        rgb: hex_to_rgb(0x3A5F0B),
    },
    NamedColor {
        name: "Valhalla Moss",
        rgb: hex_to_rgb(0x445533),
    },
    NamedColor {
        name: "Kyoto Tea",
        rgb: hex_to_rgb(0xB9C406),
    },
    NamedColor {
        name: "Bamboo Shoot",
        rgb: hex_to_rgb(0x889F22),
    },
    NamedColor {
        name: "Lush Jungle Green",
        rgb: hex_to_rgb(0x358D5A),
    },
    NamedColor {
        name: "Mint Mojito",
        rgb: hex_to_rgb(0x77DD77),
    },
    NamedColor {
        name: "Algae Bloom",
        rgb: hex_to_rgb(0x93DFB8),
    },
    NamedColor {
        name: "Green Apple",
        rgb: hex_to_rgb(0x8DB600),
    },
    NamedColor {
        name: "Artichoke",
        rgb: hex_to_rgb(0x8F9779),
    },
    NamedColor {
        name: "Asparagus",
        rgb: hex_to_rgb(0x87A96B),
    },
    NamedColor {
        name: "Avocado",
        rgb: hex_to_rgb(0x568203),
    },
    NamedColor {
        name: "Basil",
        rgb: hex_to_rgb(0x828E84),
    },
    NamedColor {
        name: "Celadon",
        rgb: hex_to_rgb(0xACE1AF),
    },
    NamedColor {
        name: "Feldgrau",
        rgb: hex_to_rgb(0x4D5D53),
    },
    NamedColor {
        name: "Hooker's Green",
        rgb: hex_to_rgb(0x49796B),
    },
    NamedColor {
        name: "Hunter Green",
        rgb: hex_to_rgb(0x355E3B),
    },
    NamedColor {
        name: "Itten's Green",
        rgb: hex_to_rgb(0x00A859),
    },
    NamedColor {
        name: "Kelly Green",
        rgb: hex_to_rgb(0x4CBB17),
    },
    NamedColor {
        name: "Malachite",
        rgb: hex_to_rgb(0x0BDA51),
    },
    NamedColor {
        name: "Mantis",
        rgb: hex_to_rgb(0x74C365),
    },
    NamedColor {
        name: "Middle Green",
        rgb: hex_to_rgb(0x4D8C57),
    },
    NamedColor {
        name: "Brighter Pine",
        rgb: hex_to_rgb(0x437E2E),
    },
    NamedColor {
        name: "Myrtle Green",
        rgb: hex_to_rgb(0x317873),
    },
    NamedColor {
        name: "Neon Carrot Green",
        rgb: hex_to_rgb(0x80FF00),
    },
    NamedColor {
        name: "Bright Spruce",
        rgb: hex_to_rgb(0x446B4E),
    },
    NamedColor {
        name: "Warm Olive Clay",
        rgb: hex_to_rgb(0x73603D),
    },
    NamedColor {
        name: "Parrot Green",
        rgb: hex_to_rgb(0x12AD2B),
    },
    NamedColor {
        name: "Brighter Pine Green",
        rgb: hex_to_rgb(0x5E6B50),
    },
    NamedColor {
        name: "Reseda Green",
        rgb: hex_to_rgb(0x6C7C59),
    },
    // --- The Cosmic Abyss (50 Colors) ---
    NamedColor {
        name: "Classic Purple",
        rgb: hex_to_rgb(0x800080),
    },
    NamedColor {
        name: "Blue Violet",
        rgb: hex_to_rgb(0x8A2BE2),
    },
    NamedColor {
        name: "Dark Violet",
        rgb: hex_to_rgb(0x9400D3),
    },
    NamedColor {
        name: "Dark Orchid",
        rgb: hex_to_rgb(0x9932CC),
    },
    NamedColor {
        name: "Medium Orchid",
        rgb: hex_to_rgb(0xBA55D3),
    },
    NamedColor {
        name: "Orchid Pink",
        rgb: hex_to_rgb(0xDA70D6),
    },
    NamedColor {
        name: "Lavender Blush",
        rgb: hex_to_rgb(0xEE82EE),
    },
    NamedColor {
        name: "Thistle",
        rgb: hex_to_rgb(0xD8BFD8),
    },
    NamedColor {
        name: "Electric Fuchsia",
        rgb: hex_to_rgb(0xFF00FF),
    },
    NamedColor {
        name: "Medium Violet Red",
        rgb: hex_to_rgb(0xC71585),
    },
    NamedColor {
        name: "Deep Hot Pink",
        rgb: hex_to_rgb(0xFF1493),
    },
    NamedColor {
        name: "Bubblegum Pink",
        rgb: hex_to_rgb(0xFF69B4),
    },
    NamedColor {
        name: "Indigo Abyss",
        rgb: hex_to_rgb(0x4B0082),
    },
    NamedColor {
        name: "Dark Slate Blue",
        rgb: hex_to_rgb(0x483D8B),
    },
    NamedColor {
        name: "Medium Slate Blue",
        rgb: hex_to_rgb(0x7B68EE),
    },
    NamedColor {
        name: "Royal Violet",
        rgb: hex_to_rgb(0x6A0DAD),
    },
    NamedColor {
        name: "Burgundy Wine",
        rgb: hex_to_rgb(0x800020),
    },
    NamedColor {
        name: "Wine Rose",
        rgb: hex_to_rgb(0x722F37),
    },
    NamedColor {
        name: "Cyberpunk Magenta",
        rgb: hex_to_rgb(0xDA1884),
    },
    NamedColor {
        name: "Ruby Velvet",
        rgb: hex_to_rgb(0xE0115F),
    },
    NamedColor {
        name: "Cerise Cherry",
        rgb: hex_to_rgb(0xDE3163),
    },
    NamedColor {
        name: "Wisteria",
        rgb: hex_to_rgb(0x7A4F80),
    },
    NamedColor {
        name: "Rose Gold",
        rgb: hex_to_rgb(0xB76E79),
    },
    NamedColor {
        name: "Lavender Mist",
        rgb: hex_to_rgb(0xE6E6FA),
    },
    NamedColor {
        name: "Plum Jam",
        rgb: hex_to_rgb(0xDDA0DD),
    },
    NamedColor {
        name: "Royal Purple",
        rgb: hex_to_rgb(0x6A3573),
    },
    NamedColor {
        name: "Cosmic Violet",
        rgb: hex_to_rgb(0x512888),
    },
    NamedColor {
        name: "Neon Purple",
        rgb: hex_to_rgb(0xA32CC4),
    },
    NamedColor {
        name: "Amethyst",
        rgb: hex_to_rgb(0x602F6B),
    },
    NamedColor {
        name: "Deep Velvet Rose",
        rgb: hex_to_rgb(0xD10056),
    },
    // Expanded Cosmic
    NamedColor {
        name: "Pulsar Purple",
        rgb: hex_to_rgb(0xBF00FF),
    },
    NamedColor {
        name: "Supernova Pink",
        rgb: hex_to_rgb(0xFF007F),
    },
    NamedColor {
        name: "Nebula Twilight",
        rgb: hex_to_rgb(0x4C4366),
    },
    NamedColor {
        name: "Nebula Gas",
        rgb: hex_to_rgb(0x452C63),
    },
    NamedColor {
        name: "Star Cluster",
        rgb: hex_to_rgb(0x76616B),
    },
    NamedColor {
        name: "Galaxy Swirl",
        rgb: hex_to_rgb(0x58427C),
    },
    NamedColor {
        name: "Star Astral Purple",
        rgb: hex_to_rgb(0x6E448F),
    },
    NamedColor {
        name: "Void Purple",
        rgb: hex_to_rgb(0x59288C),
    },
    NamedColor {
        name: "Comet Tail",
        rgb: hex_to_rgb(0x99AAB5),
    },
    NamedColor {
        name: "Bright Meteorite Grey",
        rgb: hex_to_rgb(0x7D7D7D),
    },
    NamedColor {
        name: "Quantum Pink",
        rgb: hex_to_rgb(0xFF1493),
    },
    NamedColor {
        name: "Gamma Ray",
        rgb: hex_to_rgb(0xFFFF33),
    },
    NamedColor {
        name: "Binary Star",
        rgb: hex_to_rgb(0xFFA500),
    },
    NamedColor {
        name: "Solar Flare",
        rgb: hex_to_rgb(0xFF4500),
    },
    NamedColor {
        name: "Lunar Dust",
        rgb: hex_to_rgb(0xC0C0C0),
    },
    NamedColor {
        name: "Cosmic Dust",
        rgb: hex_to_rgb(0x4B3621),
    },
    NamedColor {
        name: "Stardust",
        rgb: hex_to_rgb(0x9F8170),
    },
    NamedColor {
        name: "Zenith Blue",
        rgb: hex_to_rgb(0x007FFF),
    },
    NamedColor {
        name: "Nadire Purple",
        rgb: hex_to_rgb(0x4B0082),
    },
    NamedColor {
        name: "Cosmic Slate Blue",
        rgb: hex_to_rgb(0x3D5A80),
    },
    // --- The Ocean Depths (50 Colors) ---
    NamedColor {
        name: "Electric Blue",
        rgb: hex_to_rgb(0x0000FF),
    },
    NamedColor {
        name: "Rich Royal Navy",
        rgb: hex_to_rgb(0x294BB0),
    },
    NamedColor {
        name: "Dodger Blue",
        rgb: hex_to_rgb(0x1E90FF),
    },
    NamedColor {
        name: "Deep Sky Blue",
        rgb: hex_to_rgb(0x00BFFF),
    },
    NamedColor {
        name: "Sky Blue",
        rgb: hex_to_rgb(0x87CEEB),
    },
    NamedColor {
        name: "Steel Blue",
        rgb: hex_to_rgb(0x4682B4),
    },
    NamedColor {
        name: "Cadet Blue",
        rgb: hex_to_rgb(0x5F9EA0),
    },
    NamedColor {
        name: "Aqua Cyan",
        rgb: hex_to_rgb(0x00FFFF),
    },
    NamedColor {
        name: "Pale Turquoise",
        rgb: hex_to_rgb(0xE0FFFF),
    },
    NamedColor {
        name: "Pale Blue-Green",
        rgb: hex_to_rgb(0xAFEEEE),
    },
    NamedColor {
        name: "Aquamarine",
        rgb: hex_to_rgb(0x7FFFD4),
    },
    NamedColor {
        name: "Turquoise Wave",
        rgb: hex_to_rgb(0x40E0D0),
    },
    NamedColor {
        name: "Dark Turquoise",
        rgb: hex_to_rgb(0x00CED1),
    },
    NamedColor {
        name: "Deep Sea Teal",
        rgb: hex_to_rgb(0x088F8F),
    },
    NamedColor {
        name: "Cobalt Blue",
        rgb: hex_to_rgb(0x0047AB),
    },
    NamedColor {
        name: "Royal Blue",
        rgb: hex_to_rgb(0x4169E1),
    },
    NamedColor {
        name: "Classic Royal Blue",
        rgb: hex_to_rgb(0x385ED0),
    },
    NamedColor {
        name: "Midnight Purple-Blue",
        rgb: hex_to_rgb(0x3F3FB0),
    },
    NamedColor {
        name: "Cornflower",
        rgb: hex_to_rgb(0x6495ED),
    },
    NamedColor {
        name: "Ice Blue",
        rgb: hex_to_rgb(0xB0C4DE),
    },
    NamedColor {
        name: "Slate Grey-Blue",
        rgb: hex_to_rgb(0x708090),
    },
    NamedColor {
        name: "Egyptian Blue",
        rgb: hex_to_rgb(0x1034A6),
    },
    NamedColor {
        name: "Luminous Abyss",
        rgb: hex_to_rgb(0x334BCB),
    },
    NamedColor {
        name: "Majorelle Blue",
        rgb: hex_to_rgb(0x6050DC),
    },
    NamedColor {
        name: "Neon Blue",
        rgb: hex_to_rgb(0x4D4DFF),
    },
    // Expanded Ocean
    NamedColor {
        name: "Slate Ink Blue",
        rgb: hex_to_rgb(0x324D73),
    },
    NamedColor {
        name: "Bioluminescent Blue",
        rgb: hex_to_rgb(0x00F5FF),
    },
    NamedColor {
        name: "Siren's Song",
        rgb: hex_to_rgb(0x40E0D0),
    },
    NamedColor {
        name: "Coral Reef Pink",
        rgb: hex_to_rgb(0xFF7F50),
    },
    NamedColor {
        name: "Anemone Purple",
        rgb: hex_to_rgb(0x800080),
    },
    NamedColor {
        name: "Manta Ray Grey",
        rgb: hex_to_rgb(0x2F4F4F),
    },
    NamedColor {
        name: "Luminous Abyssal",
        rgb: hex_to_rgb(0x2B4EBF),
    },
    NamedColor {
        name: "Pelagic Blue",
        rgb: hex_to_rgb(0x0000FF),
    },
    NamedColor {
        name: "Benthic Steel Blue",
        rgb: hex_to_rgb(0x3F5B73),
    },
    NamedColor {
        name: "Deep Trench Blue",
        rgb: hex_to_rgb(0x324E6E),
    },
    NamedColor {
        name: "Oceanic Cobalt",
        rgb: hex_to_rgb(0x0047AB),
    },
    NamedColor {
        name: "Sea Salt Blue",
        rgb: hex_to_rgb(0xF0F8FF),
    },
    NamedColor {
        name: "Marine Teal",
        rgb: hex_to_rgb(0x008080),
    },
    NamedColor {
        name: "Surf Spray",
        rgb: hex_to_rgb(0x00FFFF),
    },
    NamedColor {
        name: "Tropical Lagoon",
        rgb: hex_to_rgb(0x20B2AA),
    },
    NamedColor {
        name: "Pacific Blue",
        rgb: hex_to_rgb(0x1CA9C9),
    },
    NamedColor {
        name: "Atlantic Navy",
        rgb: hex_to_rgb(0x002366),
    },
    NamedColor {
        name: "Indian Azure",
        rgb: hex_to_rgb(0x007FFF),
    },
    NamedColor {
        name: "Arctic Ice",
        rgb: hex_to_rgb(0xE0FFFF),
    },
    NamedColor {
        name: "Deep Sea Kelp",
        rgb: hex_to_rgb(0x2E8B57),
    },
    NamedColor {
        name: "Viking Fjord",
        rgb: hex_to_rgb(0x00A86B),
    },
    NamedColor {
        name: "Mediterranean Mist",
        rgb: hex_to_rgb(0x87CEEB),
    },
    NamedColor {
        name: "Baltic Steel",
        rgb: hex_to_rgb(0x4682B4),
    },
    NamedColor {
        name: "Caribbean Cyan",
        rgb: hex_to_rgb(0x00FFFF),
    },
    NamedColor {
        name: "Coral Sea",
        rgb: hex_to_rgb(0xFF7F50),
    },
    // --- The Sun & Hearth (30 Colors) ---
    NamedColor {
        name: "Crimson Fire",
        rgb: hex_to_rgb(0xFF0000),
    },
    NamedColor {
        name: "Blaze Orange",
        rgb: hex_to_rgb(0xFF4500),
    },
    NamedColor {
        name: "Dark Amber",
        rgb: hex_to_rgb(0xFF8C00),
    },
    NamedColor {
        name: "Sun Yellow",
        rgb: hex_to_rgb(0xFFA500),
    },
    NamedColor {
        name: "Neon Yellow",
        rgb: hex_to_rgb(0xFFFF00),
    },
    NamedColor {
        name: "Imperial Gold",
        rgb: hex_to_rgb(0xFFD700),
    },
    NamedColor {
        name: "Dark Goldenrod",
        rgb: hex_to_rgb(0xB8860B),
    },
    NamedColor {
        name: "Peru Tan",
        rgb: hex_to_rgb(0xCD853F),
    },
    NamedColor {
        name: "Chocolate Earth",
        rgb: hex_to_rgb(0xD2691E),
    },
    NamedColor {
        name: "Saddle Brown",
        rgb: hex_to_rgb(0x8B4513),
    },
    NamedColor {
        name: "Sienna Dust",
        rgb: hex_to_rgb(0xA0522D),
    },
    NamedColor {
        name: "Rosy Gold",
        rgb: hex_to_rgb(0xBC8F8F),
    },
    NamedColor {
        name: "Dark Salmon",
        rgb: hex_to_rgb(0xE9967A),
    },
    // Expanded Sun/Hearth
    NamedColor {
        name: "Phoenix Feather",
        rgb: hex_to_rgb(0xFF4F00),
    },
    NamedColor {
        name: "Molten Core",
        rgb: hex_to_rgb(0xFF3300),
    },
    NamedColor {
        name: "Hearth Fire",
        rgb: hex_to_rgb(0xFF6600),
    },
    NamedColor {
        name: "Cinder Ash",
        rgb: hex_to_rgb(0x543D37),
    },
    NamedColor {
        name: "Ember Warmth",
        rgb: hex_to_rgb(0x6D4E45),
    },
    NamedColor {
        name: "Dragon Breath",
        rgb: hex_to_rgb(0xFF2400),
    },
    NamedColor {
        name: "Solar Flare Red",
        rgb: hex_to_rgb(0xED2939),
    },
    NamedColor {
        name: "Afterglow",
        rgb: hex_to_rgb(0xF28500),
    },
    NamedColor {
        name: "Autumn Leaf",
        rgb: hex_to_rgb(0xD94D1A),
    },
    NamedColor {
        name: "Burnt Sienna",
        rgb: hex_to_rgb(0xE97451),
    },
    NamedColor {
        name: "Campfire Ember",
        rgb: hex_to_rgb(0xFF8C00),
    },
    NamedColor {
        name: "Copper Penny",
        rgb: hex_to_rgb(0xAD6F44),
    },
    NamedColor {
        name: "Desert Rose",
        rgb: hex_to_rgb(0xEDC9AF),
    },
    NamedColor {
        name: "Fired Brick",
        rgb: hex_to_rgb(0xB22222),
    },
    NamedColor {
        name: "Garnet",
        rgb: hex_to_rgb(0x733632),
    },
    NamedColor {
        name: "Ignited Gold",
        rgb: hex_to_rgb(0xFFD700),
    },
    NamedColor {
        name: "Lava Flow",
        rgb: hex_to_rgb(0xCF1020),
    },
    // --- Desert Mirage (25 Colors) ---
    NamedColor {
        name: "Sahara Sand",
        rgb: hex_to_rgb(0xF4A460),
    },
    NamedColor {
        name: "Mojave Dust",
        rgb: hex_to_rgb(0xC2B280),
    },
    NamedColor {
        name: "Canyon Shadow",
        rgb: hex_to_rgb(0xA0522D),
    },
    NamedColor {
        name: "Mesa Sun",
        rgb: hex_to_rgb(0xFF8C00),
    },
    NamedColor {
        name: "Oasis Palm",
        rgb: hex_to_rgb(0x228B22),
    },
    NamedColor {
        name: "Dune Crest",
        rgb: hex_to_rgb(0xD2B48C),
    },
    NamedColor {
        name: "Scorpion Brown",
        rgb: hex_to_rgb(0x6D4E4B),
    },
    NamedColor {
        name: "Mirage Blue",
        rgb: hex_to_rgb(0xADD8E6),
    },
    NamedColor {
        name: "Sandstorm",
        rgb: hex_to_rgb(0xE2CA76),
    },
    NamedColor {
        name: "Badlands Red",
        rgb: hex_to_rgb(0xB22222),
    },
    NamedColor {
        name: "Arid Bone",
        rgb: hex_to_rgb(0xF5F5DC),
    },
    NamedColor {
        name: "Cactus Flower",
        rgb: hex_to_rgb(0xFF00FF),
    },
    NamedColor {
        name: "Prickly Pear",
        rgb: hex_to_rgb(0x568203),
    },
    NamedColor {
        name: "Dry Wash",
        rgb: hex_to_rgb(0x808080),
    },
    NamedColor {
        name: "Tumbleweed",
        rgb: hex_to_rgb(0xDEB887),
    },
    NamedColor {
        name: "Sedona Sunset",
        rgb: hex_to_rgb(0xCC7722),
    },
    NamedColor {
        name: "Painted Desert",
        rgb: hex_to_rgb(0xCD5C5C),
    },
    NamedColor {
        name: "Gila Monster",
        rgb: hex_to_rgb(0xFF8C00),
    },
    NamedColor {
        name: "Yucca",
        rgb: hex_to_rgb(0x2E8B57),
    },
    NamedColor {
        name: "Sage Scrub",
        rgb: hex_to_rgb(0x8A9A5B),
    },
    NamedColor {
        name: "Heat Haze",
        rgb: hex_to_rgb(0xE6E6FA),
    },
    NamedColor {
        name: "Adobe Clay",
        rgb: hex_to_rgb(0xD2691E),
    },
    NamedColor {
        name: "Copper Canyon",
        rgb: hex_to_rgb(0xB87333),
    },
    NamedColor {
        name: "Dust Devil",
        rgb: hex_to_rgb(0xBC8F8F),
    },
    NamedColor {
        name: "Golden Oasis",
        rgb: hex_to_rgb(0xFFD700),
    },
    // --- Frozen Tundra (25 Colors) ---
    NamedColor {
        name: "Arctic Fox",
        rgb: hex_to_rgb(0xF0F8FF),
    },
    NamedColor {
        name: "Glacial Blue",
        rgb: hex_to_rgb(0xAFEEEE),
    },
    NamedColor {
        name: "Aurora Teal",
        rgb: hex_to_rgb(0x00FFFF),
    },
    NamedColor {
        name: "Permafrost",
        rgb: hex_to_rgb(0xB0C4DE),
    },
    NamedColor {
        name: "Snow Drift",
        rgb: hex_to_rgb(0xFFFAFA),
    },
    NamedColor {
        name: "Icy Peak",
        rgb: hex_to_rgb(0xE0FFFF),
    },
    NamedColor {
        name: "Polar Bear",
        rgb: hex_to_rgb(0xFFFFFF),
    },
    NamedColor {
        name: "Frozen Lake",
        rgb: hex_to_rgb(0x87CEEB),
    },
    NamedColor {
        name: "Midnight Sun",
        rgb: hex_to_rgb(0xFFD700),
    },
    NamedColor {
        name: "Winter Twilight",
        rgb: hex_to_rgb(0x483D8B),
    },
    NamedColor {
        name: "Hoarfrost",
        rgb: hex_to_rgb(0xDCDCDC),
    },
    NamedColor {
        name: "Blizzard White",
        rgb: hex_to_rgb(0xF8F8FF),
    },
    NamedColor {
        name: "Crystal Ice",
        rgb: hex_to_rgb(0xB0E0E6),
    },
    NamedColor {
        name: "Boreal Green",
        rgb: hex_to_rgb(0x006400),
    },
    NamedColor {
        name: "Tundra Moss",
        rgb: hex_to_rgb(0x8FBC8F),
    },
    NamedColor {
        name: "Frostbite",
        rgb: hex_to_rgb(0xADD8E6),
    },
    NamedColor {
        name: "Sleet Grey",
        rgb: hex_to_rgb(0x708090),
    },
    NamedColor {
        name: "Hailstone",
        rgb: hex_to_rgb(0xD3D3D3),
    },
    NamedColor {
        name: "Avalanche",
        rgb: hex_to_rgb(0xFFFFFF),
    },
    NamedColor {
        name: "Northern Lights Purple",
        rgb: hex_to_rgb(0x8A2BE2),
    },
    NamedColor {
        name: "Frozen Cave Blue",
        rgb: hex_to_rgb(0x385BD0),
    },
    NamedColor {
        name: "Glacier Grey",
        rgb: hex_to_rgb(0xC0C0C0),
    },
    NamedColor {
        name: "Powder Snow",
        rgb: hex_to_rgb(0xFFFAFA),
    },
    NamedColor {
        name: "Shivering Sky",
        rgb: hex_to_rgb(0x87CEFA),
    },
    NamedColor {
        name: "Fir Evergreen",
        rgb: hex_to_rgb(0x426B4E),
    },
    // --- Earthy Core (20 Colors) ---
    NamedColor {
        name: "Obsidian Jade Green",
        rgb: hex_to_rgb(0x3F5448),
    },
    NamedColor {
        name: "Basalt Slate Grey",
        rgb: hex_to_rgb(0x757575),
    },
    NamedColor {
        name: "Pyrite Shine",
        rgb: hex_to_rgb(0x967117),
    },
    NamedColor {
        name: "Bright Hematite Red",
        rgb: hex_to_rgb(0x9E3B3B),
    },
    NamedColor {
        name: "Magma Chamber",
        rgb: hex_to_rgb(0x8B0000),
    },
    NamedColor {
        name: "Limestone",
        rgb: hex_to_rgb(0xD3D3D3),
    },
    NamedColor {
        name: "Granite",
        rgb: hex_to_rgb(0x676767),
    },
    NamedColor {
        name: "Slate Shale",
        rgb: hex_to_rgb(0x828285),
    },
    NamedColor {
        name: "Coal Grey",
        rgb: hex_to_rgb(0x545459),
    },
    NamedColor {
        name: "Earthen Clay",
        rgb: hex_to_rgb(0x8B4513),
    },
    NamedColor {
        name: "Copper Ore",
        rgb: hex_to_rgb(0xB87333),
    },
    NamedColor {
        name: "Silver Vein",
        rgb: hex_to_rgb(0xC0C0C0),
    },
    NamedColor {
        name: "Iron Oxide",
        rgb: hex_to_rgb(0x8D4024),
    },
    NamedColor {
        name: "Lighter Tectonic Slate",
        rgb: hex_to_rgb(0x5A7D7D),
    },
    NamedColor {
        name: "Gravel Path",
        rgb: hex_to_rgb(0x808080),
    },
    NamedColor {
        name: "Boulder",
        rgb: hex_to_rgb(0x7B7B7B),
    },
    NamedColor {
        name: "Mudstone",
        rgb: hex_to_rgb(0x70543E),
    },
    NamedColor {
        name: "Sandstone",
        rgb: hex_to_rgb(0xF4A460),
    },
    NamedColor {
        name: "Pumice",
        rgb: hex_to_rgb(0xDCDCDC),
    },
    NamedColor {
        name: "Core Fire",
        rgb: hex_to_rgb(0xFF4500),
    },
    // --- Floral Bloom (18 Colors) ---
    NamedColor {
        name: "Hibiscus Red",
        rgb: hex_to_rgb(0xFF2400),
    },
    NamedColor {
        name: "Cherry Blossom",
        rgb: hex_to_rgb(0xFFB7C5),
    },
    NamedColor {
        name: "Orchid Bloom",
        rgb: hex_to_rgb(0xDA70D6),
    },
    NamedColor {
        name: "Lavender Mist",
        rgb: hex_to_rgb(0xE6E6FA),
    },
    NamedColor {
        name: "Daffodil Yellow",
        rgb: hex_to_rgb(0xFFFF31),
    },
    NamedColor {
        name: "Peony Pink",
        rgb: hex_to_rgb(0xFFC1CC),
    },
    NamedColor {
        name: "Rose Petal",
        rgb: hex_to_rgb(0xFF033E),
    },
    NamedColor {
        name: "Sunflower",
        rgb: hex_to_rgb(0xFFDA03),
    },
    NamedColor {
        name: "Iris Purple",
        rgb: hex_to_rgb(0x5A4FCF),
    },
    NamedColor {
        name: "Tulip Orange",
        rgb: hex_to_rgb(0xFF8C00),
    },
    NamedColor {
        name: "Bluebell",
        rgb: hex_to_rgb(0xA2A2D0),
    },
    NamedColor {
        name: "Poppy Red",
        rgb: hex_to_rgb(0xE35D6A),
    },
    NamedColor {
        name: "Violet",
        rgb: hex_to_rgb(0x8F00FF),
    },
    NamedColor {
        name: "Lily White",
        rgb: hex_to_rgb(0xFFFFF0),
    },
    NamedColor {
        name: "Marigold",
        rgb: hex_to_rgb(0xEAA221),
    },
    NamedColor {
        name: "Cornflower Blue",
        rgb: hex_to_rgb(0x6495ED),
    },
    NamedColor {
        name: "Hydrangea",
        rgb: hex_to_rgb(0x8396D1),
    },
    NamedColor {
        name: "Snapdragon",
        rgb: hex_to_rgb(0xFFD700),
    },
];

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
    PREMIUM_COLORS[index % 300].rgb
}

/// RGB used for human-owned territory in the sow-render map shader (`map.wgsl`).
/// Matches WGSL `owner_id <= 16` branch so UI (nameplates) matches the map tint.
#[inline]
pub fn human_shader_territory_rgb(player_id: u16) -> [f32; 3] {
    let base_color = &PREMIUM_COLORS[(player_id as usize).saturating_sub(1) % 300];
    if (1..=300).contains(&player_id) {
        base_color.rgb
    } else {
        // Fallback with subtle jittering for massive games
        let [r, g, b] = base_color.rgb;
        let [h, s, v] = rgb_to_hsv(r, g, b);

        let seed = (player_id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let mut rng = WyRand::new(seed);

        // Jitter: +/- 0.015 Hue, +/- 0.04 Sat, +/- 0.04 Val
        let hj = h + (rng.next_int(0, 1000) as f32 / 1000.0 - 0.5) * 0.03;
        let sj = (s + (rng.next_int(0, 1000) as f32 / 1000.0 - 0.5) * 0.08).clamp(0.4, 1.0);
        let mut vj = (v + (rng.next_int(0, 1000) as f32 / 1000.0 - 0.5) * 0.08).clamp(0.45, 1.0);

        // Vibrancy guard for warm colors
        if hj.fract().abs() >= 0.03 && hj.fract().abs() <= 0.15 && vj < 0.60 {
            vj = 0.65 + (vj * 0.3);
        }

        hsv_to_rgb(hj, sj, vj)
    }
}

pub fn bot_territory_color(game_seed: u64, bot_id: u16) -> [f32; 3] {
    let base_color = &PREMIUM_COLORS[(bot_id as usize).saturating_sub(1) % 300];
    if (1..=300).contains(&bot_id) {
        base_color.rgb
    } else {
        let [r, g, b] = base_color.rgb;
        let [h, s, v] = rgb_to_hsv(r, g, b);

        let mix = game_seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (bot_id as u64).wrapping_shl(32)
            ^ (bot_id as u64);
        let mut rng = WyRand::new(mix);

        let hj = h + (rng.next_int(0, 1000) as f32 / 1000.0 - 0.5) * 0.03;
        let sj = (s + (rng.next_int(0, 1000) as f32 / 1000.0 - 0.5) * 0.08).clamp(0.4, 1.0);
        let mut vj = (v + (rng.next_int(0, 1000) as f32 / 1000.0 - 0.5) * 0.08).clamp(0.45, 1.0);

        // Vibrancy guard for warm colors
        if hj.fract().abs() >= 0.03 && hj.fract().abs() <= 0.15 && vj < 0.60 {
            vj = 0.65 + (vj * 0.3);
        }

        let [r_res, g_res, b_res] = hsv_to_rgb(hj, sj, vj);
        [
            r_res.clamp(0.05, 0.95),
            g_res.clamp(0.05, 0.95),
            b_res.clamp(0.05, 0.95),
        ]
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
