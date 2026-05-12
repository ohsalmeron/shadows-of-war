use serde::{Deserialize, Serialize};
use wyrand::WyRand;
use crate::rng::NextIntExt;
use crate::bitset::DenseBitSet;

pub type PlayerId = u16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerType { Human, Bot, Nation }

fn default_player_gold() -> f64 { crate::config::STARTING_GOLD }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Player {
    pub id: PlayerId, pub name: String, pub player_type: PlayerType,
    pub troops: f64, pub max_troops: f64,
    #[serde(default = "default_player_gold")]
    pub gold: f64,
    pub color: [f32; 3], pub alive: bool, pub has_spawned: bool,
    pub sum_x: u64, pub sum_y: u64, pub tile_count: u32,
    pub border_tiles: DenseBitSet,
    #[serde(skip, default = "default_wyrand")] pub bot_rng: WyRand,
    pub factories: u32, pub cities: u32,
}

fn default_wyrand() -> WyRand { WyRand::new(0) }

impl Player {
    pub fn new_human(id: u16, name: String, color: [f32; 3], config: &crate::game_config::GameConfig) -> Self {
        Self { id, alive: true, player_type: PlayerType::Human, name, color,
            troops: config.starting_troops, max_troops: config.max_troops_base,
            gold: config.starting_gold, has_spawned: false,
            sum_x: 0, sum_y: 0, tile_count: 0,
            border_tiles: DenseBitSet::new(), bot_rng: WyRand::new(id as u64),
            factories: 0, cities: 0,
        }
    }
    pub fn new_bot(id: u16, name: String, color: [f32; 3], config: &crate::game_config::GameConfig) -> Self {
        Self { id, alive: true, player_type: PlayerType::Bot, name, color,
            troops: config.starting_troops, max_troops: config.max_troops_base / 3.0,
            gold: 0.0, has_spawned: false,
            sum_x: 0, sum_y: 0, tile_count: 0,
            border_tiles: DenseBitSet::new(), bot_rng: WyRand::new(id as u64),
            factories: 0, cities: 0,
        }
    }
    pub fn new_nation(id: u16, name: String, color: [f32; 3], config: &crate::game_config::GameConfig) -> Self {
        Self { id, alive: true, player_type: PlayerType::Nation, name, color,
            troops: config.starting_troops, max_troops: config.max_troops_base * 0.75,
            gold: config.starting_gold, has_spawned: false,
            sum_x: 0, sum_y: 0, tile_count: 0,
            border_tiles: DenseBitSet::new(), bot_rng: WyRand::new(id as u64),
            factories: 0, cities: 0,
        }
    }
    pub fn is_human(&self) -> bool { self.player_type == PlayerType::Human }
    pub fn border_coords(&self, map_width: u32) -> impl Iterator<Item = (u32, u32)> + '_ {
        self.border_tiles.ones().map(move |idx| (idx % map_width, idx / map_width))
    }
    #[inline] pub fn border_insert(&mut self, idx: u32) { self.border_tiles.insert(idx); }
    #[inline] pub fn border_remove(&mut self, idx: u32) { self.border_tiles.remove(idx); }
}

pub fn player_colors() -> Vec<[f32; 3]> {
    vec![ [0.0,1.0,1.0],[1.0,0.02,0.08],[0.02,1.0,0.08],[1.0,1.0,0.0],
          [1.0,0.05,0.95],[0.45,0.08,1.0],[1.0,0.32,0.0],[0.0,0.98,0.88] ]
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    let h = h.clamp(0.0, 1.0).fract(); let s = s.clamp(0.0, 1.0); let v = v.clamp(0.0, 1.0);
    let i = (h * 6.0).floor() as i32; let f = h * 6.0 - i as f32;
    let p = v * (1.0 - s); let q = v * (1.0 - f * s); let t = v * (1.0 - (1.0 - f) * s);
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (v, t, p), 1 => (q, v, p), 2 => (p, v, t),
        3 => (p, q, v), 4 => (t, p, v), _ => (v, p, q),
    };
    [r, g, b]
}

/// RGB used for human-owned territory in the sow-render map shader (`map.wgsl`).
/// Matches WGSL `owner_id <= 16` branch so UI (nameplates) matches the map tint.
#[inline]
pub fn human_shader_territory_rgb(player_id: u16) -> [f32; 3] {
    let hue = player_id as f32 * 0.618_033_99;
    let fract = |x: f32| x - x.floor();
    let r = (fract(hue) * 2.0 - 1.0).abs();
    let g = (fract(hue + 0.333) * 2.0 - 1.0).abs();
    let b = (fract(hue + 0.666) * 2.0 - 1.0).abs();
    [r, g, b]
}

pub fn bot_territory_color(game_seed: u64, bot_id: u16) -> [f32; 3] {
    let mix = game_seed.wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (bot_id as u64).wrapping_shl(32) ^ (bot_id as u64);
    let mut rng = WyRand::new(mix);
    let h = rng.next_int(0, 10_000) as f32 / 10_000.0;
    let s = 0.28 + rng.next_int(0, 1000) as f32 / 1000.0 * 0.18;
    let v = 0.52 + rng.next_int(0, 1000) as f32 / 1000.0 * 0.18;
    let [r, g, b] = hsv_to_rgb(h, s, v);
    [r.clamp(0.16, 0.88), g.clamp(0.16, 0.88), b.clamp(0.16, 0.88)]
}
