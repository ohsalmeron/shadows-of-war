use sow_core::map_file::{self, MapCatalog, MapCatalogEntry};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static CATALOG: OnceLock<Vec<MapCatalogEntry>> = OnceLock::new();
static CATALOG_BYTES: OnceLock<Vec<u8>> = OnceLock::new();

fn slug(name: &str) -> String {
    sow_core::maps::map_key(name)
}

pub fn scan_maps_root(root: &Path) -> MapCatalog {
    let mut items = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(root) else {
        return MapCatalog { entries: vec![] };
    };
    for entry in read_dir.flatten() {
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let map_dir = entry.path();
        let key = entry.file_name().to_string_lossy().to_string();
        if key.starts_with('.') {
            continue;
        }
        // Try map.bin first (raw), fall back to map.bin.br (brotli-compressed, what deploy syncs).
        let bytes = if let Ok(data) = std::fs::read(&map_dir.join("map.bin")) {
            data
        } else if let Ok(br) = std::fs::read(&map_dir.join("map.bin.br")) {
            map_file::decompress_map_payload(&br).unwrap_or(br)
        } else {
            continue;
        };
        let Ok(header) = map_file::parse_header(&bytes) else {
            log::warn!("Skipping {key}: invalid map.bin header");
            continue;
        };
        items.push((slug(&key), header));
    }
    map_file::catalog_from_headers(items)
}

pub fn init(maps_root: &Path) {
    let catalog = scan_maps_root(maps_root);
    let bytes = map_file::encode_catalog(&catalog);
    let catalog_path = std::env::var("SOW_MAPS_CATALOG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| maps_root.join("catalog.bin"));
    if let Err(e) = std::fs::write(&catalog_path, &bytes) {
        log::error!("Failed to write {}: {}", catalog_path.display(), e);
    } else {
        log::info!(
            "Map catalog: {} entries → {}",
            catalog.entries.len(),
            catalog_path.display()
        );
    }
    let _ = CATALOG.set(catalog.entries);
    let _ = CATALOG_BYTES.set(bytes);
}

pub fn entries() -> &'static [MapCatalogEntry] {
    CATALOG.get().map(|v| v.as_slice()).unwrap_or(&[])
}

pub fn catalog_bytes() -> &'static [u8] {
    CATALOG_BYTES.get().map(|v| v.as_slice()).unwrap_or(&[])
}

pub fn lookup(name: &str) -> Option<MapCatalogEntry> {
    sow_core::maps::catalog_lookup(entries(), name).cloned()
}

pub fn maps_root() -> PathBuf {
    std::env::var("SOW_MAPS_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(sow_core::maps::SERVER_MAPS_ROOT))
}

#[derive(serde::Serialize, Debug, Clone)]
pub struct MapCatalogJsonEntry {
    pub key: String,
    pub display_name: String,
    pub width: u32,
    pub height: u32,
    pub thumbnail: String,
}

pub fn catalog_json() -> Vec<MapCatalogJsonEntry> {
    entries()
        .iter()
        .map(|entry| MapCatalogJsonEntry {
            key: entry.key.clone(),
            display_name: entry.display_name.clone(),
            width: entry.width,
            height: entry.height,
            thumbnail: format!("/maps/{}/thumbnail.webp", entry.key),
        })
        .collect()
}
