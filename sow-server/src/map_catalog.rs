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
        let map_path = map_dir.join("map.bin");
        if !map_path.exists() {
            continue;
        }
        let Ok(bytes) = std::fs::read(&map_path) else {
            continue;
        };
        let Ok(header) = map_file::parse_header(&bytes) else {
            log::warn!("Skipping {}: invalid map.bin header", key);
            continue;
        };
        items.push((slug(&key), header));
    }
    map_file::catalog_from_headers(items)
}

pub fn init(maps_root: &Path) {
    let catalog = scan_maps_root(maps_root);
    let bytes = map_file::encode_catalog(&catalog);
    let catalog_path = maps_root.join("catalog.bin");
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
