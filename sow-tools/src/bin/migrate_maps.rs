//! One-shot migration: legacy raw terrain + manifest.json → SOWM map.bin + catalog.bin

use serde_json::Value;
use sow_core::map_file::{self, MapFile, MapSpawn};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

fn slug(name: &str) -> String {
    sow_core::maps::map_key(name)
}

fn brotli_compress(input: &[u8]) -> std::io::Result<Vec<u8>> {
    let mut out = Vec::new();
    let mut writer = brotli::CompressorWriter::new(&mut out, 4096, 11, 22);
    writer.write_all(input)?;
    writer.flush()?;
    drop(writer);
    Ok(out)
}

fn read_map_payload(map_dir: &Path) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let bin_path = map_dir.join("map.bin");
    let br_path = map_dir.join("map.bin.br");
    if bin_path.exists() {
        return Ok(fs::read(&bin_path)?);
    }
    if br_path.exists() {
        let br = fs::read(&br_path)?;
        return map_file::decompress_map_payload(&br).map_err(|e| e.to_string().into());
    }
    Err("missing map.bin and map.bin.br".into())
}

fn load_legacy(
    key: &str,
    map_dir: &Path,
) -> Result<MapFile, Box<dyn std::error::Error>> {
    let raw = read_map_payload(map_dir)?;

    if raw.len() >= 4 && &raw[0..4] == map_file::MAP_MAGIC {
        return Ok(map_file::parse(&raw)?);
    }

    let manifest_path = map_dir.join("manifest.json");
    if !manifest_path.exists() {
        return Err("legacy raw map without manifest.json".into());
    }

    let manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    let map_info = manifest
        .get("map")
        .ok_or("manifest missing map section")?;
    let width = map_info.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let height = map_info.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let num_land = map_info
        .get("num_land_tiles")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let display_name = manifest
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(key)
        .to_string();

    let mut width = width;
    let mut height = height;
    let expected = (width as usize) * (height as usize);
    if expected == 0 || raw.len() != expected {
        if width > 0 && raw.len().is_multiple_of(width as usize) {
            height = (raw.len() / width as usize) as u32;
        } else if height > 0 && raw.len().is_multiple_of(height as usize) {
            width = (raw.len() / height as usize) as u32;
        } else {
            return Err(format!(
                "terrain size mismatch for {key}: manifest {width}x{height}, got {} bytes",
                raw.len()
            )
            .into());
        }
    }

    let mut spawns = Vec::new();
    if let Some(nations) = manifest.get("nations").and_then(|v| v.as_array()) {
        for entry in nations {
            let name = entry
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("Unknown")
                .to_string();
            let flag = entry
                .get("flag")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let coords = entry.get("coordinates").and_then(|v| v.as_array());
            let x = coords.and_then(|c| c.first()).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            let y = coords.and_then(|c| c.get(1)).and_then(|v| v.as_u64()).unwrap_or(0) as u32;
            spawns.push(MapSpawn { name, flag, x, y });
        }
    }

    Ok(MapFile {
        display_name,
        width,
        height,
        num_land_tiles: num_land,
        spawns,
        terrain: raw,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let maps_root = PathBuf::from("assets/maps");
    if !maps_root.is_dir() {
        eprintln!("Run from repo root; missing {}", maps_root.display());
        std::process::exit(1);
    }

    let mut catalog_items: Vec<(String, map_file::MapHeader)> = Vec::new();
    let mut migrated = 0usize;
    let mut skipped = 0usize;

    for entry in fs::read_dir(&maps_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let map_dir = entry.path();
        let key = entry.file_name().to_string_lossy().to_string();
        if key.starts_with('.') {
            continue;
        }
        if !map_dir.join("map.bin").exists() && !map_dir.join("map.bin.br").exists() {
            continue;
        }

        match load_legacy(&key, &map_dir) {
            Ok(map_file) => {
                let encoded = map_file::encode(&map_file);
                let header = map_file::parse_header(&encoded)?;
                fs::write(map_dir.join("map.bin"), &encoded)?;
                let compressed = brotli_compress(&encoded)?;
                fs::write(map_dir.join("map.bin.br"), compressed)?;

                let _ = fs::remove_file(map_dir.join("manifest.json"));
                let _ = fs::remove_file(map_dir.join("mini_map.bin"));

                catalog_items.push((slug(&key), header));
                migrated += 1;
                if migrated % 10 == 0 {
                    println!("... {migrated} maps");
                }
            }
            Err(e) => {
                eprintln!("SKIP {key}: {e}");
                skipped += 1;
            }
        }
    }

    let catalog = map_file::catalog_from_headers(catalog_items);
    let catalog_bytes = map_file::encode_catalog(&catalog);
    fs::write(maps_root.join("catalog.bin"), catalog_bytes)?;
    let _ = fs::remove_file(maps_root.join("maps.json"));

    println!("Migrated {migrated} maps, skipped {skipped}");
    println!(
        "Wrote {} ({} entries)",
        maps_root.join("catalog.bin").display(),
        catalog.entries.len()
    );
    Ok(())
}
