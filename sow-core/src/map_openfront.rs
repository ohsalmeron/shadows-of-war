use crate::map::{GameMap, MapTile};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MapSize {
    pub width: u32,
    pub height: u32,
    pub num_land_tiles: u32,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Nation {
    pub name: String,
    pub flag: Option<String>,
    pub coordinates: [u32; 2],
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct MapManifest {
    pub name: String,
    pub map: MapSize,
    pub map4x: Option<MapSize>,
    pub map16x: Option<MapSize>,
    pub nations: Option<Vec<Nation>>,
    pub map_md5: Option<String>,
}

pub fn game_map_from_openfront(manifest: &MapManifest, bin: &[u8]) -> Result<GameMap, String> {
    let width = manifest.map.width;
    let height = manifest.map.height;
    let expected_len = (width * height) as usize;

    if bin.len() != expected_len {
        return Err(format!(
            "Map bytes length mismatch. Expected {} ({}x{}), got {}",
            expected_len,
            width,
            height,
            bin.len()
        ));
    }

    let mut map = GameMap::new(width, height);
    for (i, &b) in bin.iter().enumerate() {
        map.terrain[i] = MapTile::from_byte(b);
    }

    Ok(map)
}
