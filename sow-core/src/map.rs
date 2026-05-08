use serde::{Deserialize, Serialize};
use bitfield::bitfield;

bitfield! {
    #[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
        if x > 0 { f(x - 1, y); }
        if x + 1 < self.width { f(x + 1, y); }
        if y > 0 { f(x, y - 1); }
        if y + 1 < self.height { f(x, y + 1); }
    }
    pub fn neighbors(&self, x: u32, y: u32) -> Vec<(u32, u32)> {
        let mut r = Vec::with_capacity(4);
        if x > 0 { r.push((x - 1, y)); }
        if x + 1 < self.width { r.push((x + 1, y)); }
        if y > 0 { r.push((x, y - 1)); }
        if y + 1 < self.height { r.push((x, y + 1)); }
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
