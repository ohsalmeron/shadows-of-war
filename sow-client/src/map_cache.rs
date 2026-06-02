//! Persist downloaded map payloads for offline replay after reload.

use sow_core::maps::{self, MapCatalogEntry};

#[cfg(target_arch = "wasm32")]
const STORAGE_KEY_PREFIX: &str = "sow_map_br_v1:";

#[cfg(target_arch = "wasm32")]
fn storage_key(map_key: &str) -> String {
    format!("{STORAGE_KEY_PREFIX}{}", maps::map_key(map_key))
}

fn validate_payload(bytes: &[u8]) -> bool {
    maps::load_map_from_payload(bytes).is_ok()
}

/// Load a previously cached map payload, if any.
pub fn load(map_key: &str) -> Option<Vec<u8>> {
    let key = maps::map_key(map_key);
    #[cfg(target_arch = "wasm32")]
    {
        let window = web_sys::window()?;
        let storage = window.local_storage().ok()??;
        let stored = storage.get_item(&storage_key(&key)).ok()??;
        let bytes = base64_decode(&stored)?;
        if validate_payload(&bytes) {
            return Some(bytes);
        }
        let _ = storage.remove_item(&storage_key(&key));
        return None;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let path = crate::paths::map_cache_dir().join(format!("{key}.bin.br"));
        let bytes = std::fs::read(path).ok()?;
        if validate_payload(&bytes) {
            Some(bytes)
        } else {
            None
        }
    }
}

/// Store a map payload after a successful download or SP session.
pub fn persist(map_key: &str, bytes: &[u8]) {
    if !validate_payload(bytes) {
        log::warn!("map_cache: skip persist for invalid payload ({map_key})");
        return;
    }
    let key = maps::map_key(map_key);
    #[cfg(target_arch = "wasm32")]
    {
        let Some(window) = web_sys::window() else {
            return;
        };
        let Ok(Some(storage)) = window.local_storage() else {
            return;
        };
        let Some(encoded) = base64_encode(bytes) else {
            log::warn!("map_cache: base64 encode failed for {key}");
            return;
        };
        if let Err(e) = storage.set_item(&storage_key(&key), &encoded) {
            log::warn!("map_cache: localStorage set failed for {key}: {e:?}");
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let dir = crate::paths::map_cache_dir();
        if let Err(e) = std::fs::create_dir_all(&dir) {
            log::warn!("map_cache: mkdir {}: {e}", dir.display());
            return;
        }
        let path = dir.join(format!("{key}.bin.br"));
        if let Err(e) = std::fs::write(path, bytes) {
            log::warn!("map_cache: write failed for {key}: {e}");
        }
    }
}

/// Map keys with a valid cached payload.
pub fn list_cached_keys() -> Vec<String> {
    #[cfg(target_arch = "wasm32")]
    {
        let Some(window) = web_sys::window() else {
            return Vec::new();
        };
        let Ok(Some(storage)) = window.local_storage() else {
            return Vec::new();
        };
        let len = storage.length().unwrap_or(0);
        let mut keys = Vec::new();
        for i in 0..len {
            let Ok(Some(name)) = storage.key(i) else {
                continue;
            };
            let Some(slug) = name.strip_prefix(STORAGE_KEY_PREFIX) else {
                continue;
            };
            if load(slug).is_some() {
                keys.push(slug.to_string());
            }
        }
        keys.sort();
        keys
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let dir = crate::paths::map_cache_dir();
        let Ok(read_dir) = std::fs::read_dir(dir) else {
            return Vec::new();
        };
        let mut keys = Vec::new();
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("br") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            if load(stem).is_some() {
                keys.push(stem.to_string());
            }
        }
        keys.sort();
        keys
    }
}

/// Build catalog entries from cached map headers (offline fallback).
pub fn catalog_from_cache() -> Vec<MapCatalogEntry> {
    let mut entries = Vec::new();
    for key in list_cached_keys() {
        let Some(bytes) = load(&key) else {
            continue;
        };
        let Ok(map) = maps::load_map_from_payload(&bytes) else {
            continue;
        };
        entries.push(MapCatalogEntry {
            key: key.clone(),
            display_name: map.display_name,
            width: map.width,
            height: map.height,
        });
    }
    entries.sort_by(|a, b| {
        a.display_name
            .to_lowercase()
            .cmp(&b.display_name.to_lowercase())
    });
    entries
}

/// Hydrate in-memory map cache from persistent storage.
pub fn hydrate_asset_maps(maps: &mut std::collections::HashMap<String, Vec<u8>>) {
    for key in list_cached_keys() {
        if let Some(bytes) = load(&key) {
            maps.entry(key).or_insert(bytes);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn base64_encode(bytes: &[u8]) -> Option<String> {
    Some(encode_base64(bytes))
}

#[cfg(target_arch = "wasm32")]
fn base64_decode(encoded: &str) -> Option<Vec<u8>> {
    decode_base64(encoded).ok()
}

#[cfg(target_arch = "wasm32")]
fn encode_base64(data: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((n >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(target_arch = "wasm32")]
fn decode_base64(input: &str) -> Result<Vec<u8>, ()> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' => Some(62),
            b'/' => Some(63),
            _ => None,
        }
    }
    let bytes: Vec<u8> = input.bytes().filter(|&b| b != b'=').collect();
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        if chunk.len() < 2 {
            return Err(());
        }
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            let v = val(c).ok_or(())?;
            n |= (v as u32) << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Ok(out)
}
