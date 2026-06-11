use reqwest::Client;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use crate::osm_coast::CoastlineGeometry;
use crate::osm_tiles::OSM_USER_AGENT;

const OVERPASS_SERVERS: [&str; 2] = [
    "https://overpass.kumi.systems/api/interpreter",
    "https://overpass-api.de/api/interpreter",
];

pub const TILE_LON_DEG: f64 = 15.0;
pub const TILE_LAT_DEG: f64 = 10.0;
const REQUEST_TIMEOUT_SECS: u64 = 180;

/// How many Overpass grid cells a bbox spans (15°×10° per cell).
pub fn overpass_tile_count(min_lon: f64, min_lat: f64, max_lon: f64, max_lat: f64) -> usize {
    tile_bboxes(min_lon, min_lat, max_lon, max_lat).len()
}

/// Shrink bbox toward its center until it fits within `max_tiles` Overpass cells.
pub fn clamp_bbox_to_tile_budget(
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
    max_tiles: usize,
) -> (f64, f64, f64, f64, bool) {
    if overpass_tile_count(min_lon, min_lat, max_lon, max_lat) <= max_tiles {
        return (min_lon, min_lat, max_lon, max_lat, false);
    }
    let clon = (min_lon + max_lon) * 0.5;
    let clat = (min_lat + max_lat) * 0.5;
    let mut half_lon = (max_lon - min_lon).max(1e-9) * 0.5;
    let mut half_lat = (max_lat - min_lat).max(1e-9) * 0.5;
    for _ in 0..64 {
        let out_min_lon = clon - half_lon;
        let out_max_lon = clon + half_lon;
        let out_min_lat = clat - half_lat;
        let out_max_lat = clat + half_lat;
        if overpass_tile_count(out_min_lon, out_min_lat, out_max_lon, out_max_lat) <= max_tiles {
            return (out_min_lon, out_min_lat, out_max_lon, out_max_lat, true);
        }
        half_lon *= 0.85;
        half_lat *= 0.85;
    }
    (
        clon - TILE_LON_DEG * 0.5,
        clat - TILE_LAT_DEG * 0.5,
        clon + TILE_LON_DEG * 0.5,
        clat + TILE_LAT_DEG * 0.5,
        true,
    )
}

pub fn tile_bboxes(
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
) -> Vec<(f64, f64, f64, f64)> {
    let mut tiles = Vec::new();
    let mut lat = min_lat;
    while lat < max_lat {
        let tile_max_lat = (lat + TILE_LAT_DEG).min(max_lat);
        let mut lon = min_lon;
        while lon < max_lon {
            let tile_max_lon = (lon + TILE_LON_DEG).min(max_lon);
            tiles.push((lat, lon, tile_max_lat, tile_max_lon));
            lon = tile_max_lon;
        }
        lat = tile_max_lat;
    }
    tiles
}

/// Fetch coastlines one tile at a time, extract projected segments, drop raw JSON each iteration.
#[allow(clippy::too_many_arguments)]
pub async fn fetch_coastlines_tiled(
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
    scale: f64,
    map_width: u32,
    map_height: u32,
    cache_name: Option<&str>,
) -> Result<CoastlineGeometry, Box<dyn Error>> {
    let tiles = tile_bboxes(min_lon, min_lat, max_lon, max_lat);
    log::info!(
        "Overpass coastlines: {} tile(s) for bbox [{min_lon:.4}, {min_lat:.4}, {max_lon:.4}, {max_lat:.4}]",
        tiles.len()
    );

    let cache_dir = cache_name.map(|n| PathBuf::from("target/osm_cache").join(n));
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()?;

    let mut segments = Vec::new();
    let mut failed = 0usize;

    for (tile_index, (tile_min_lat, tile_min_lon, tile_max_lat, tile_max_lon)) in
        tiles.iter().enumerate()
    {
        log::info!(
            "  coastline tile {}/{}: [{tile_min_lon:.2}, {tile_min_lat:.2}, {tile_max_lon:.2}, {tile_max_lat:.2}]",
            tile_index + 1,
            tiles.len()
        );

        if let Some(ref dir) = cache_dir {
            if let Some(cached) = load_coastline_cache(dir, tile_index) {
                log::info!("    loaded {} segments from cache", cached.segments.len());
                segments.extend(cached.segments);
                continue;
            }
        }

        let query = coastline_query(*tile_min_lat, *tile_min_lon, *tile_max_lat, *tile_max_lon);
        let json = match run_overpass(&client, &query).await {
            Ok(j) => j,
            Err(e) => {
                log::warn!("    coastline fetch failed: {e}");
                failed += 1;
                continue;
            }
        };

        if let Some(remark) = json.get("remark").and_then(|r| r.as_str()) {
            if remark.contains("timed out") || remark.contains("runtime error") {
                log::warn!("    coastline query failed: {remark}");
                failed += 1;
                continue;
            }
        }

        let extracted = crate::osm_coast::extract_coastlines(
            &json, min_lon, min_lat, max_lon, max_lat, scale, map_width, map_height,
        );
        log::info!("    {} coastline segments", extracted.segments.len());

        if let Some(ref dir) = cache_dir {
            save_coastline_cache(dir, tile_index, &extracted)?;
        }

        segments.extend(extracted.segments);
    }

    log::info!(
        "Coastline fetch done: {} segments ({} tile failures)",
        segments.len(),
        failed
    );
    if segments.is_empty() {
        return Err(
            "No coastline geometry from Overpass (timeouts or rate limits). Retry later.".into(),
        );
    }

    Ok(CoastlineGeometry { segments })
}

/// Fetch lake/bay polygons one tile at a time and stamp them onto the grid immediately.
#[allow(clippy::too_many_arguments)]
pub async fn stamp_water_tiled(
    grid: &mut [sow_core::map::MapTile],
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
    scale: f64,
    map_width: u32,
    map_height: u32,
) -> Result<(), Box<dyn Error>> {
    let tiles = tile_bboxes(min_lon, min_lat, max_lon, max_lat);
    eprintln!("Overpass water bodies: {} tile(s)", tiles.len());

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()?;

    let mut stamped = 0usize;
    for (tile_index, (tile_min_lat, tile_min_lon, tile_max_lat, tile_max_lon)) in
        tiles.iter().enumerate()
    {
        eprintln!(
            "  water tile {}/{}: [{tile_min_lon:.2}, {tile_min_lat:.2}, {tile_max_lon:.2}, {tile_max_lat:.2}]",
            tile_index + 1,
            tiles.len()
        );

        let query = water_query(*tile_min_lat, *tile_min_lon, *tile_max_lat, *tile_max_lon);
        let json = match run_overpass(&client, &query).await {
            Ok(j) => j,
            Err(e) => {
                eprintln!("    water fetch skipped: {e}");
                continue;
            }
        };

        if let Some(remark) = json.get("remark").and_then(|r| r.as_str()) {
            if remark.contains("timed out") || remark.contains("runtime error") {
                eprintln!("    water query skipped: {remark}");
                continue;
            }
        }

        let before = grid.iter().filter(|t| t.is_water()).count();
        crate::osm_coast::stamp_water_polygons(
            grid, &json, min_lon, min_lat, max_lon, max_lat, scale, map_width, map_height,
        );
        let after = grid.iter().filter(|t| t.is_water()).count();
        if after > before {
            stamped += 1;
        }
        eprintln!(
            "    stamped {} new water tiles",
            after.saturating_sub(before)
        );
    }

    eprintln!("Water stamp done ({stamped} tiles added inland water)");
    Ok(())
}

pub async fn fetch_places(
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
) -> Result<Value, Box<dyn Error>> {
    let tiles = tile_bboxes(min_lon, min_lat, max_lon, max_lat);
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(90))
        .build()?;
    let mut all_elements = Vec::new();
    let mut seen = HashSet::new();

    for (tile_min_lat, tile_min_lon, tile_max_lat, tile_max_lon) in tiles {
        let query = format!(
            r#"[out:json][timeout:60];node["place"~"city|town|village|suburb|hamlet"]({tile_min_lat},{tile_min_lon},{tile_max_lat},{tile_max_lon});out body;"#
        );
        for server in OVERPASS_SERVERS {
            let res = client
                .post(server)
                .header("User-Agent", OSM_USER_AGENT)
                .form(&[("data", &query)])
                .send()
                .await;
            if let Ok(resp) = res {
                if resp.status().is_success() {
                    let json: Value = resp.json().await?;
                    if let Some(elements) = json.get("elements").and_then(|e| e.as_array()) {
                        for element in elements {
                            let key = element_key(element);
                            if seen.insert(key) {
                                all_elements.push(element.clone());
                            }
                        }
                    }
                    break;
                }
            }
        }
    }

    Ok(json!({ "elements": all_elements }))
}

fn coastline_query(min_lat: f64, min_lon: f64, max_lat: f64, max_lon: f64) -> String {
    format!(
        r#"[out:json][timeout:120];way["natural"="coastline"]({min_lat},{min_lon},{max_lat},{max_lon});out geom;"#
    )
}

fn water_query(min_lat: f64, min_lon: f64, max_lat: f64, max_lon: f64) -> String {
    format!(
        r#"[out:json][timeout:120];
        (
          way["natural"="water"]({min_lat},{min_lon},{max_lat},{max_lon});
          way["natural"="bay"]({min_lat},{min_lon},{max_lat},{max_lon});
          way["landuse"="water"]({min_lat},{min_lon},{max_lat},{max_lon});
        );
        out geom;"#
    )
    .replace('\n', " ")
}

async fn run_overpass(client: &Client, query: &str) -> Result<Value, Box<dyn Error>> {
    let mut last_err = String::new();
    for server in OVERPASS_SERVERS {
        let res = client
            .post(server)
            .header("User-Agent", OSM_USER_AGENT)
            .form(&[("data", query)])
            .send()
            .await;

        match res {
            Ok(resp) if resp.status().is_success() => {
                return Ok(resp.json().await?);
            }
            Ok(resp) => {
                last_err = format!("{server} HTTP {}", resp.status());
            }
            Err(e) => {
                last_err = format!("{server}: {e}");
            }
        }
    }
    Err(last_err.into())
}

fn element_key(element: &Value) -> (String, i64) {
    let elem_type = element
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("unknown")
        .to_string();
    let id = element.get("id").and_then(|i| i.as_i64()).unwrap_or(0);
    (elem_type, id)
}

fn load_coastline_cache(dir: &Path, tile_index: usize) -> Option<CoastlineGeometry> {
    let path = dir.join(format!("coast_{tile_index:02}.json"));
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn save_coastline_cache(
    dir: &Path,
    tile_index: usize,
    geo: &CoastlineGeometry,
) -> Result<(), Box<dyn Error>> {
    fs::create_dir_all(dir)?;
    let path = dir.join(format!("coast_{tile_index:02}.json"));
    fs::write(path, serde_json::to_string(geo)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tile_bboxes_cover_full_extent() {
        let tiles = tile_bboxes(-125.0, 24.0, -66.0, 50.0);
        assert_eq!(tiles.len(), 12);
        assert!((tiles[0].1 - (-125.0)).abs() < f64::EPSILON);
        assert!((tiles.last().unwrap().2 - 50.0).abs() < f64::EPSILON);
        assert!((tiles.last().unwrap().3 - (-66.0)).abs() < f64::EPSILON);
    }

    #[test]
    fn clamp_bbox_limits_overpass_tiles() {
        let huge = (-29.0, 25.0, 48.0, 71.0);
        assert!(overpass_tile_count(huge.0, huge.1, huge.2, huge.3) > 1);
        let (a, b, c, d, clamped) = clamp_bbox_to_tile_budget(huge.0, huge.1, huge.2, huge.3, 1);
        assert!(clamped);
        assert_eq!(overpass_tile_count(a, b, c, d), 1);
    }
}
