//! OpenFront-style world heightmap sampling for OSM hybrid classify.
//!
//! Reads `giantworldmap`-style `image.png` where the blue channel encodes elevation
//! (MapGenerator rules: water `blue == 106` or `alpha < 20`; land `blue` 140–200).

use image::RgbaImage;
use std::path::PathBuf;
use std::sync::OnceLock;

static WORLD_HEIGHTMAP: OnceLock<Result<WorldHeightmap, String>> = OnceLock::new();

/// Equirectangular world heightmap (OpenFront `giantworldmap` asset).
#[derive(Clone)]
pub struct WorldHeightmap {
    img: RgbaImage,
}

impl WorldHeightmap {
    /// Load from the first available path (see [`heightmap_search_paths`]).
    pub fn load() -> Result<Self, String> {
        WORLD_HEIGHTMAP.get_or_init(Self::load_from_disk).clone()
    }

    /// Build from an in-memory image (tests).
    pub fn from_image(img: RgbaImage) -> Self {
        Self { img }
    }

    fn load_from_disk() -> Result<Self, String> {
        let tried = heightmap_search_paths();
        for path in &tried {
            if path.is_file() {
                let img = image::open(path)
                    .map_err(|e| format!("failed to open {}: {e}", path.display()))?
                    .to_rgba8();
                log::info!(
                    "Loaded world heightmap {} ({}x{})",
                    path.display(),
                    img.width(),
                    img.height()
                );
                return Ok(Self { img });
            }
        }
        Err(format!(
            "world heightmap not found. Tried:\n{}\n\
             Set SOW_HEIGHTMAP_PATH, clone MapGenerator at the repo root, or add \
             assets/heightmaps/world.png (see README).",
            tried
                .iter()
                .map(|p| format!("  - {}", p.display()))
                .collect::<Vec<_>>()
                .join("\n")
        ))
    }

    pub fn width(&self) -> u32 {
        self.img.width()
    }

    pub fn height(&self) -> u32 {
        self.img.height()
    }

    /// Sample MapGenerator blue value at WGS84 lon/lat (equirectangular projection).
    pub fn sample_openfront_blue(&self, lon: f64, lat: f64) -> u8 {
        let w = self.img.width().max(1) as f64;
        let h = self.img.height().max(1) as f64;
        let x = ((lon + 180.0) / 360.0 * w).clamp(0.0, w - 1.0) as u32;
        let y = ((90.0 - lat.clamp(-90.0, 90.0)) / 180.0 * h).clamp(0.0, h - 1.0) as u32;
        openfront_blue_from_rgba(self.img.get_pixel(x, y).0)
    }
}

/// MapGenerator pixel → encoded blue channel.
pub fn openfront_blue_from_rgba(px: [u8; 4]) -> u8 {
    let [_r, _g, b, a] = px;
    if a < 20 || b == 106 { 106 } else { b }
}

pub fn heightmap_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Ok(p) = std::env::var("SOW_HEIGHTMAP_PATH") {
        paths.push(PathBuf::from(p));
    }
    if let Some(ws) = workspace_root() {
        paths.push(ws.join("MapGenerator/assets/maps/giantworldmap/image.png"));
        paths.push(ws.join("OpenFrontIO/map-generator/assets/maps/giantworldmap/image.png"));
        paths.push(ws.join("assets/heightmaps/world.png"));
    }
    paths
}

fn workspace_root() -> Option<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest.parent().map(|p| p.to_path_buf())
}

/// Histogram of packed `MapTile` bytes after `generate_from_rgba`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TerrainStats {
    pub land: u32,
    pub water: u32,
    pub ocean: u32,
    pub shoreline: u32,
    pub plains: u32,
    pub highlands: u32,
    pub mountains: u32,
}

pub fn terrain_stats_from_packed(map_data: &[u8]) -> TerrainStats {
    let mut s = TerrainStats::default();
    for &byte in map_data {
        let land = (byte & 0b1000_0000) != 0;
        let mag = byte & 0b0001_1111;
        if land {
            s.land += 1;
            if mag < 10 {
                s.plains += 1;
            } else if mag < 20 {
                s.highlands += 1;
            } else {
                s.mountains += 1;
            }
        } else {
            s.water += 1;
        }
        if (byte & 0b0100_0000) != 0 {
            s.shoreline += 1;
        }
        if (byte & 0b0010_0000) != 0 {
            s.ocean += 1;
        }
    }
    s
}

impl TerrainStats {
    pub fn log_summary(&self) {
        log::info!(
            "Terrain: {} land (plains {}, highlands {}, mountains {}), {} water ({} ocean, {} shoreline tiles)",
            self.land,
            self.plains,
            self.highlands,
            self.mountains,
            self.water,
            self.ocean,
            self.shoreline
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openfront_blue_water_and_land() {
        assert_eq!(openfront_blue_from_rgba([0, 0, 106, 255]), 106);
        assert_eq!(openfront_blue_from_rgba([0, 0, 106, 0]), 106);
        assert_eq!(openfront_blue_from_rgba([0, 0, 180, 255]), 180);
    }

    #[test]
    fn equirect_sample_rockies_elevation() {
        let path = heightmap_search_paths().into_iter().find(|p| p.is_file());
        let Some(_path) = path else {
            eprintln!("skip equirect_sample_rockies_elevation: no heightmap on disk");
            return;
        };
        let hm = WorldHeightmap::load().expect("heightmap");
        let blue = hm.sample_openfront_blue(-110.0, 45.0);
        assert_ne!(blue, 106, "Rockies should not be water");
        let mag = (blue.clamp(140, 200) as i32 - 140) / 2;
        assert!((0..=30).contains(&mag), "magnitude {mag} from blue {blue}");
    }
}
