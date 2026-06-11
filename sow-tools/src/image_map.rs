//! Image-based map generation.
//!
//! Ports the proven OpenFront `MapGenerator` technique: a source world-map PNG
//! encodes land/water by pixel color, so no network/API calls are needed.
//! Core pipeline lives in `sow_map::image_pipeline`.

use crate::poi_extractor::POISpawn;
use serde_json::Value;
use sow_core::map::MapTile;
use sow_map::image_pipeline::generate_from_rgba;
use std::error::Error;
use std::path::Path;

pub struct GeneratedMap {
    pub width: u32,
    pub height: u32,
    pub terrain: Vec<MapTile>,
    pub num_land_tiles: u32,
}

/// Decode a source PNG, classify terrain, and downscale to the mobile-safe budget.
pub fn generate_from_image(path: &Path) -> Result<GeneratedMap, Box<dyn Error>> {
    let img = image::open(path)?.to_rgba8();
    let src_w = img.width();
    let src_h = img.height();
    println!(
        "Source image: {src_w}x{src_h} ({} pixels)",
        src_w as u64 * src_h as u64
    );

    let result = generate_from_rgba(&img, None).map_err(|e| -> Box<dyn Error> { e.into() })?;

    let terrain = result
        .map_data
        .iter()
        .map(|&b| MapTile::from_byte(b))
        .collect();

    Ok(GeneratedMap {
        width: result.width,
        height: result.height,
        terrain,
        num_land_tiles: result.num_land_tiles,
    })
}

/// Map nation coordinates from `info.json` (source-image pixel space) to the
/// downscaled grid. Falls back to a land grid when no info file is given.
pub fn spawns_from_info(
    info_path: Option<&Path>,
    src_w: u32,
    src_h: u32,
    map: &GeneratedMap,
) -> Vec<POISpawn> {
    if let Some(path) = info_path {
        if let Ok(text) = std::fs::read_to_string(path) {
            if let Ok(json) = serde_json::from_str::<Value>(&text) {
                if let Some(spawns) = parse_nation_spawns(&json, src_w, src_h, map) {
                    if !spawns.is_empty() {
                        return spawns;
                    }
                }
            }
        }
        eprintln!("Warning: could not read nations from info.json; using land-grid spawns");
    }
    crate::poi_extractor::fallback_spawns_on_land(&map.terrain, map.width, map.height, 16)
}

fn parse_nation_spawns(
    json: &Value,
    src_w: u32,
    src_h: u32,
    map: &GeneratedMap,
) -> Option<Vec<POISpawn>> {
    let nations = json.get("nations")?.as_array()?;
    let sx = map.width as f64 / src_w as f64;
    let sy = map.height as f64 / src_h as f64;
    let mut spawns = Vec::new();
    for nation in nations {
        let coords = nation.get("coordinates").and_then(|c| c.as_array());
        let name = nation.get("name").and_then(|n| n.as_str());
        let (Some(coords), Some(name)) = (coords, name) else {
            continue;
        };
        if coords.len() < 2 {
            continue;
        }
        let raw_x = coords[0].as_f64().unwrap_or(0.0);
        let raw_y = coords[1].as_f64().unwrap_or(0.0);
        let x = ((raw_x * sx).round() as u32).min(map.width.saturating_sub(1));
        let y = ((raw_y * sy).round() as u32).min(map.height.saturating_sub(1));
        spawns.push(POISpawn {
            name: name.to_string(),
            x,
            y,
        });
    }
    Some(spawns)
}

#[cfg(test)]
mod tests {
    use sow_core::maps;
    use sow_map::image_pipeline::mobile_safe_dims;

    #[test]
    fn mobile_safe_dims_preserve_aspect_and_budget() {
        let (w, h) = mobile_safe_dims(2800, 1448);
        assert!(w <= maps::MAX_MAP_AXIS && h <= maps::MAX_MAP_AXIS);
        assert!((w as u64) * (h as u64) <= maps::MAX_MAP_PIXELS as u64);
        let src_ar = 2800.0 / 1448.0;
        let dst_ar = w as f64 / h as f64;
        assert!(
            (src_ar - dst_ar).abs() < 0.05,
            "aspect {src_ar} vs {dst_ar}"
        );
    }
}
