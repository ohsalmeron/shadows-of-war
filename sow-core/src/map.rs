use serde::{Deserialize, Serialize};
use bitfield::bitfield;

bitfield! {
    #[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    #[repr(transparent)]
    pub struct MapTile(u8);
    impl Debug;
    pub is_land, _: 7;
    pub is_shoreline, _: 6;
    pub is_ocean, _: 5;
    pub u8, magnitude, _: 4, 0;
}

impl MapTile {
    pub fn from_byte(byte: u8) -> Self { MapTile(byte) }
    pub fn as_byte(&self) -> u8 { self.0 }
    pub fn is_water(&self) -> bool { !self.is_land() }
    pub fn terrain_type(&self) -> TerrainType {
        if self.is_land() {
            let m = self.magnitude();
            if m < 10 { TerrainType::Land }
            else if m < 20 { TerrainType::Highland }
            else { TerrainType::Mountain }
        } else if self.is_ocean() { TerrainType::Water }
        else { TerrainType::Lake }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainType { Water, Lake, Land, Highland, Mountain }

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct GameMap {
    pub width: u32, pub height: u32,
    pub terrain: Vec<MapTile>, pub state: Vec<u16>,
}

/// Delta `(dx, dy)` for `border_mask` bit `bit_index` (0..6). `odd_row` means `(cell_y % 2 != 0)`.
/// Matches `sow-render` `map.wgsl` edge directions and `map_compute.wgsl` neighbor iteration.
const HEX_NEIGHBOR_DELTA_ODD: [(i32, i32); 6] = [
    (1, 0),
    (-1, 0),
    (0, -1),
    (1, -1),
    (0, 1),
    (1, 1),
];
const HEX_NEIGHBOR_DELTA_EVEN: [(i32, i32); 6] = [
    (1, 0),
    (-1, 0),
    (-1, -1),
    (0, -1),
    (-1, 1),
    (0, 1),
];

#[inline]
pub fn hex_neighbor_delta(bit_index: u32, odd_row: bool) -> (i32, i32) {
    debug_assert!(bit_index < 6);
    let i = bit_index as usize;
    if odd_row {
        HEX_NEIGHBOR_DELTA_ODD[i]
    } else {
        HEX_NEIGHBOR_DELTA_EVEN[i]
    }
}

impl GameMap {
    pub const PLAYER_ID_MASK: u16 = 0x0FFF;
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        Self { width, height, terrain: vec![MapTile::from_byte(0b10000000); size], state: vec![0; size] }
    }
    pub fn ref_id(&self, x: u32, y: u32) -> usize { (y * self.width + x) as usize }
    pub fn terrain_type(&self, x: u32, y: u32) -> TerrainType { self.terrain[self.ref_id(x, y)].terrain_type() }
    pub fn is_valid_coord(&self, x: i32, y: i32) -> bool { x >= 0 && x < self.width as i32 && y >= 0 && y < self.height as i32 }
    pub fn owner_id(&self, x: u32, y: u32) -> u16 { self.state[self.ref_id(x, y)] & Self::PLAYER_ID_MASK }
    pub fn set_owner_id(&mut self, x: u32, y: u32, player_id: u16) {
        let r = self.ref_id(x, y);
        self.state[r] = (self.state[r] & !Self::PLAYER_ID_MASK) | (player_id & Self::PLAYER_ID_MASK);
    }
    pub fn for_each_neighbor(&self, x: u32, y: u32, mut f: impl FnMut(u32, u32)) {
        let is_odd = !y.is_multiple_of(2);
        for bit in 0u32..6 {
            let (dx, dy) = hex_neighbor_delta(bit, is_odd);
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx >= 0 && nx < self.width as i32 && ny >= 0 && ny < self.height as i32 {
                f(nx as u32, ny as u32);
            }
        }
    }
    pub fn neighbors(&self, x: u32, y: u32) -> Vec<(u32, u32)> {
        let mut r = Vec::with_capacity(6);
        self.for_each_neighbor(x, y, |nx, ny| r.push((nx, ny)));
        r
    }
    pub fn is_border_tile(&self, x: u32, y: u32, player_id: u16) -> bool {
        if self.owner_id(x, y) != player_id { return false; }
        let mut b = false;
        self.for_each_neighbor(x, y, |nx, ny| { if !b && self.owner_id(nx, ny) != player_id { b = true; } });
        b
    }
    pub fn tiles_owned_by(&self, player_id: u16) -> u32 {
        self.state.iter().filter(|&&s| (s & Self::PLAYER_ID_MASK) == player_id).count() as u32
    }
    pub fn is_adjacent_to_player(&self, x: u32, y: u32, player_id: u16) -> bool {
        let mut a = false;
        self.for_each_neighbor(x, y, |nx, ny| { if !a && self.owner_id(nx, ny) == player_id { a = true; } });
        a
    }
}

#[cfg(test)]
mod border_mask_tests {
    use super::{hex_neighbor_delta, GameMap};

    #[test]
    fn hex_neighbor_deltas_match_for_each_neighbor_at_interior() {
        let map = GameMap::new(32, 32);
        let x = 10u32;
        for y in [0u32, 1u32, 10u32, 21u32] {
            let is_odd = !y.is_multiple_of(2);
            let mut from_fe = std::collections::HashSet::new();
            map.for_each_neighbor(x, y, |nx, ny| {
                from_fe.insert((nx as i32 - x as i32, ny as i32 - y as i32));
            });
            let from_bits: std::collections::HashSet<_> = (0..6u32)
                .map(|bit| hex_neighbor_delta(bit, is_odd))
                .filter(|&(dx, dy)| {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    nx >= 0 && nx < 32 && ny >= 0 && ny < 32
                })
                .collect();
            assert_eq!(from_fe, from_bits, "y={y}");
        }
    }

    fn compute_border_mask_u32(raw: &[u32], w: u32, h: u32, x: u32, y: u32) -> u32 {
        let i = (y * w + x) as usize;
        let cell = raw[i];
        let owner = cell & 0x3FF;
        let terr = (cell >> 16) & 0xFF;
        let c_land = (terr & 0x80) != 0;
        let odd = !y.is_multiple_of(2);
        let mut mask = 0u32;
        for bit in 0..6u32 {
            let (dx, dy) = hex_neighbor_delta(bit, odd);
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                mask |= 1 << bit;
                continue;
            }
            let ni = (ny as u32 * w + nx as u32) as usize;
            let nraw = raw[ni];
            let no = nraw & 0x3FF;
            let nterr = (nraw >> 16) & 0xFF;
            let n_land = (nterr & 0x80) != 0;
            if no != owner || c_land != n_land {
                mask |= 1 << bit;
            }
        }
        mask
    }

    #[test]
    fn uniform_land_interior_has_zero_border_mask() {
        let w = 8u32;
        let h = 8u32;
        let terrain_land = 0x80u32;
        let pack = |owner: u32| owner | (terrain_land << 16);
        let raw: Vec<u32> = (0..(w * h)).map(|_| pack(5)).collect();
        assert_eq!(compute_border_mask_u32(&raw, w, h, 3, 3), 0);
        assert_eq!(compute_border_mask_u32(&raw, w, h, 4, 4), 0);
    }

    #[test]
    fn owner_step_sets_political_border_bit() {
        let w = 4u32;
        let h = 4u32;
        let land = 0x80u32;
        let mut raw: Vec<u32> = (0..(w * h)).map(|_| 1u32 | (land << 16)).collect();
        let i = (1 * w + 2) as usize;
        raw[i] = 2 | (land << 16);
        let m = compute_border_mask_u32(&raw, w, h, 1, 1);
        assert_ne!(m, 0, "expected border between owner 1 and 2");
    }
}
