//! Native preference / cache directories (no CWD litter).

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

/// Persisted tutorial completion flag (native).
pub fn tutorial_completed_path() -> PathBuf {
    native_data_dir().join("tutorial_completed")
}

/// Directory for cached `map.bin.br` payloads (native).
pub fn map_cache_dir() -> PathBuf {
    native_data_dir().join("maps")
}
