//! Map catalog helpers and bundled terrain bytes.

pub use crate::map_file::{MapCatalog, MapCatalogEntry, MapFile, MapHeader, MapSpawn};

/// Default map key when catalog is empty or name is unknown.
pub const DEFAULT_MAP_KEY: &str = "northamerica";

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
        "northamerica" => Some(include_bytes!("../../assets/maps/northamerica/map.bin.br")),
        _ => None,
    }
}

/// Read `map.bin.br` from `assets/maps/<key>/` when running native from the repo root.
#[cfg(feature = "std")]
#[inline]
pub fn read_map_br_from_repo(key: &str) -> Option<Vec<u8>> {
    let path = std::path::Path::new("assets/maps")
        .join(map_key(key))
        .join("map.bin.br");
    std::fs::read(path).ok()
}

/// Read `thumbnail.webp` from `assets/maps/<key>/` when running native from the repo root.
#[cfg(feature = "std")]
#[inline]
pub fn read_thumbnail_webp_from_repo(key: &str) -> Option<Vec<u8>> {
    let path = std::path::Path::new("assets/maps")
        .join(map_key(key))
        .join("thumbnail.webp");
    std::fs::read(path).ok()
}

/// Repo `assets/maps`, valid cached download, then compile-time bundled bytes.
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
