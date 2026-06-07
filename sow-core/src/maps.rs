//! Map catalog helpers and bundled terrain bytes.

pub use crate::map_file::{MapCatalog, MapCatalogEntry, MapFile, MapHeader, MapSpawn};

/// Default map key when catalog is empty or name is unknown.
pub const DEFAULT_MAP_KEY: &str = "world";

/// Repo path for the online map library (authoring + server deploy). Not embedded in the client.
pub const SERVER_MAPS_ROOT: &str = "assets/maps";

/// Repo path for the single offline-bundled map (compile-time embed only).
pub const BUNDLED_MAPS_ROOT: &str = "assets/static/maps";

/// Mobile-safe upper bound (~OpenFront Pangaea: 1000×1000).
pub const MAX_MAP_PIXELS: u32 = 1_000_000;
/// Max width or height when targeting [`MAX_MAP_PIXELS`].
pub const MAX_MAP_AXIS: u32 = 1_000;

/// Round dimensions down to multiples of 4 (OSM rasterizer convention).
#[inline]
pub fn align_map_dim(v: u32) -> u32 {
    let v = v.max(4);
    v - (v % 4)
}

/// Compute map pixel dimensions for a bbox at the given scale (lon pixels per degree).
#[inline]
pub fn map_dims_for_bbox(min_lon: f64, min_lat: f64, max_lon: f64, max_lat: f64, scale: f64) -> (u32, u32) {
    let width = align_map_dim(((max_lon - min_lon) * scale).ceil() as u32);
    let height = align_map_dim(((max_lat - min_lat) * scale).ceil() as u32);
    (width, height)
}

/// Largest scale that keeps width×height ≤ [`MAX_MAP_PIXELS`].
pub fn max_scale_for_bbox(min_lon: f64, min_lat: f64, max_lon: f64, max_lat: f64) -> f64 {
    let lon_span = (max_lon - min_lon).max(0.001);
    let lat_span = (max_lat - min_lat).max(0.001);
    let area_deg = lon_span * lat_span;
    let scale = (MAX_MAP_PIXELS as f64 / area_deg).sqrt();
    // Verify aligned dims fit; shrink slightly if rounding pushed us over.
    let mut s = scale;
    for _ in 0..8 {
        let (w, h) = map_dims_for_bbox(min_lon, min_lat, max_lon, max_lat, s);
        if (w as u64) * (h as u64) <= MAX_MAP_PIXELS as u64 {
            return s;
        }
        s *= 0.98;
    }
    s
}

/// Clamp requested scale to the mobile-safe pixel budget.
#[inline]
pub fn clamp_scale_for_bbox(
    min_lon: f64,
    min_lat: f64,
    max_lon: f64,
    max_lat: f64,
    requested: f64,
) -> f64 {
    requested.min(max_scale_for_bbox(min_lon, min_lat, max_lon, max_lat))
}

/// Scale width/height down proportionally until width×height ≤ [`MAX_MAP_PIXELS`].
pub fn clamp_map_dimensions(width: u32, height: u32) -> (u32, u32) {
    let mut w = align_map_dim(width);
    let mut h = align_map_dim(height);
    while (w as u64) * (h as u64) > MAX_MAP_PIXELS as u64 {
        if w >= h && w > 4 {
            w -= 4;
        } else if h > 4 {
            h -= 4;
        } else {
            break;
        }
    }
    (w, h)
}

#[inline]
pub fn map_pixel_count(width: u32, height: u32) -> u64 {
    width as u64 * height as u64
}

#[inline]
pub fn map_within_pixel_budget(width: u32, height: u32) -> bool {
    map_pixel_count(width, height) <= MAX_MAP_PIXELS as u64
}

/// Maps shipped inside the client WASM for offline single-player boot.
pub const BUNDLED_MAP_KEYS: &[&str] = &[DEFAULT_MAP_KEY];

/// Normalize map folder / config name to catalog key (e.g. `"Europe"` → `"europe"`).
#[inline]
pub fn map_key(name: &str) -> String {
    name.trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

#[inline]
pub fn catalog_lookup<'a>(
    catalog: &'a [MapCatalogEntry],
    name: &str,
) -> Option<&'a MapCatalogEntry> {
    let key = map_key(name);
    catalog
        .iter()
        .find(|e| e.key == key || e.display_name.eq_ignore_ascii_case(name))
}

#[inline]
pub fn catalog_entry_at<'a>(
    catalog: &'a [MapCatalogEntry],
    index: usize,
) -> Option<&'a MapCatalogEntry> {
    if catalog.is_empty() {
        None
    } else {
        Some(&catalog[index % catalog.len()])
    }
}

/// Return normalized catalog key if `name` is in the catalog, else first entry or [`DEFAULT_MAP_KEY`].
#[inline]
pub fn resolve_map_name(catalog: &[MapCatalogEntry], name: &str) -> String {
    if let Some(entry) = catalog_lookup(catalog, name) {
        return entry.key.clone();
    }
    catalog
        .first()
        .map(|e| e.key.clone())
        .unwrap_or_else(|| DEFAULT_MAP_KEY.to_string())
}

#[inline]
pub fn apply_catalog_dimensions(
    catalog: &[MapCatalogEntry],
    map_name: &mut String,
    width: &mut u32,
    height: &mut u32,
) {
    if let Some(entry) = catalog_lookup(catalog, map_name) {
        *map_name = entry.key.clone();
        *width = entry.width;
        *height = entry.height;
    }
}

/// Decompress (if needed) and parse a full `map.bin` / `map.bin.br` payload.
#[inline]
pub fn load_map_from_payload(bytes: &[u8]) -> Result<MapFile, crate::map_file::MapFileError> {
    let raw = crate::map_file::decompress_map_payload(bytes)?;
    crate::map_file::parse(&raw)
}

/// Embedded brotli-compressed map for offline play (WASM / release builds).
#[inline]
pub fn bundled_map_br(key: &str) -> Option<&'static [u8]> {
    match map_key(key).as_str() {
        "world" => Some(crate::repo_asset_bytes!("maps/world/map.bin.br")),
        _ => None,
    }
}

/// Read `map.bin.br` from the server map library when running native from the repo root.
#[cfg(feature = "std")]
#[inline]
pub fn read_map_br_from_repo(key: &str) -> Option<Vec<u8>> {
    let path = std::path::Path::new(SERVER_MAPS_ROOT)
        .join(map_key(key))
        .join("map.bin.br");
    std::fs::read(path).ok()
}

/// Read `thumbnail.webp` from the server map library when running native from the repo root.
#[cfg(feature = "std")]
#[inline]
pub fn read_thumbnail_webp_from_repo(key: &str) -> Option<Vec<u8>> {
    let path = std::path::Path::new(SERVER_MAPS_ROOT)
        .join(map_key(key))
        .join("thumbnail.webp");
    std::fs::read(path).ok()
}

/// Repo server map library, valid cached download, then compile-time bundled offline map.
#[inline]
pub fn load_map_br_payload(key: &str, cached: Option<Vec<u8>>) -> Option<Vec<u8>> {
    #[cfg(feature = "std")]
    if let Some(bytes) = read_map_br_from_repo(key) {
        if load_map_from_payload(&bytes).is_ok() {
            return Some(bytes);
        }
    }
    if let Some(bytes) = cached {
        if load_map_from_payload(&bytes).is_ok() {
            return Some(bytes);
        }
    }
    bundled_map_br(key)
        .filter(|b| load_map_from_payload(b).is_ok())
        .map(|b| b.to_vec())
}

#[inline]
pub fn map_payload_available(key: &str) -> bool {
    load_map_br_payload(key, None).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn us_bbox_scale_clamps_under_one_million_pixels() {
        let max = max_scale_for_bbox(-125.0, 24.0, -66.0, 50.0);
        let (w, h) = map_dims_for_bbox(-125.0, 24.0, -66.0, 50.0, max);
        assert!(map_within_pixel_budget(w, h));
        assert!(max < 40.0);
    }

    #[test]
    fn clamp_map_dimensions_shrinks_oversized() {
        let (w, h) = clamp_map_dimensions(2400, 1800);
        assert!(map_within_pixel_budget(w, h));
    }
}
