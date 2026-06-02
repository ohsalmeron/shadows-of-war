//! Native preference / cache directories (no CWD litter).

#![cfg(not(target_arch = "wasm32"))]

use std::path::PathBuf;

/// Base directory for Shadows of War local data on native targets.
pub fn native_data_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("SOW_DATA_DIR") {
        let p = PathBuf::from(dir);
        if !p.as_os_str().is_empty() {
            return p;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join(".local/share/shadows-of-war");
    }
    PathBuf::from(".shadows-of-war")
}

/// Directory for cached `map.bin.br` payloads (native).
pub fn map_cache_dir() -> PathBuf {
    native_data_dir().join("maps")
}
