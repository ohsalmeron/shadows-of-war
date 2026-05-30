//! Import OpenFront-style map folders (PNG + info.json, or legacy map.bin + manifest.json).

use serde_json::Value;
use sow_core::map_file::{self, MapFile, MapSpawn};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

pub struct ImportArgs {
    pub input: PathBuf,
    pub name: Option<String>,
    pub maps_root: PathBuf,
}

pub fn run_import(args: ImportArgs) -> Result<(), Box<dyn std::error::Error>> {
    let input = args.input.canonicalize().unwrap_or(args.input.clone());
    if !input.is_dir() {
        return Err(format!("Input is not a directory: {}", input.display()).into());
    }

    let slug = args
        .name
        .clone()
        .unwrap_or_else(|| input.file_name().unwrap().to_string_lossy().to_string());
    let slug = sow_core::maps::map_key(&slug);

    let map_file = if input.join("image.png").exists() {
        import_from_png(&input, &slug)?
    } else {
        import_from_bin_or_manifest(&input, &slug)?
    };

    fs::create_dir_all(&args.maps_root)?;
    let out_dir = args.maps_root.join(&slug);
    fs::create_dir_all(&out_dir)?;

    let encoded = map_file::encode(&map_file);
    fs::write(out_dir.join("map.bin"), &encoded)?;

    let mut brotli_out = Vec::new();
    let mut writer = brotli::CompressorWriter::new(&mut brotli_out, 4096, 11, 22);
    writer.write_all(&encoded)?;
    writer.flush()?;
    drop(writer);
    fs::write(out_dir.join("map.bin.br"), brotli_out)?;

    if let Ok(img) = image::open(&input.join("image.png")) {
        let thumb_path = out_dir.join("thumbnail.webp");
        if img.save(&thumb_path).is_err() {
            write_placeholder_thumbnail(&map_file, &thumb_path)?;
        }
    } else {
        write_placeholder_thumbnail(&map_file, &out_dir.join("thumbnail.webp"))?;
    }

    refresh_catalog(&args.maps_root)?;

    println!(
        "Imported '{}' → {}/ ({}x{}, {} spawns, {} land tiles)",
        slug,
        out_dir.display(),
        map_file.width,
        map_file.height,
        map_file.spawns.len(),
        map_file.num_land_tiles
    );
    Ok(())
}

fn import_from_png(dir: &Path, display_name: &str) -> Result<MapFile, Box<dyn std::error::Error>> {
    let png_path = dir.join("image.png");
    let img = image::open(&png_path)?;
    let rgba = img.to_rgba8();
    let width = rgba.width();
    let height = rgba.height();
    let pixels: Vec<[u8; 4]> = rgba.pixels().map(|p| [p[0], p[1], p[2], p[3]]).collect();

    let result = sow_map::generator::generate_map(sow_map::generator::GeneratorArgs {
        width,
        height,
        pixels,
        remove_small: true,
    })
    .map_err(|e| format!("generator: {e}"))?;

    let spawns = load_info_json_spawns(dir)?;

    Ok(MapFile {
        display_name: display_name.to_string(),
        width: result.width,
        height: result.height,
        num_land_tiles: result.num_land_tiles,
        spawns,
        terrain: result.map_data,
    })
}

fn load_info_json_spawns(dir: &Path) -> Result<Vec<MapSpawn>, Box<dyn std::error::Error>> {
    let info_path = dir.join("info.json");
    if !info_path.exists() {
        return Ok(Vec::new());
    }
    let info: Value = serde_json::from_str(&fs::read_to_string(&info_path)?)?;
    let mut spawns = Vec::new();
    if let Some(arr) = info.as_array() {
        for entry in arr {
            push_spawn(entry, &mut spawns);
        }
    } else if let Some(nations) = info.get("nations").and_then(|v| v.as_array()) {
        for entry in nations {
            push_spawn(entry, &mut spawns);
        }
    }
    Ok(spawns)
}

fn push_spawn(entry: &Value, spawns: &mut Vec<MapSpawn>) {
    let name = entry
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Nation")
        .to_string();
    let flag = entry
        .get("flag")
        .and_then(|v| v.as_str())
        .unwrap_or("🏳")
        .to_string();
    let coords = entry.get("coordinates").and_then(|v| v.as_array());
    let x = coords
        .and_then(|c| c.first())
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let y = coords
        .and_then(|c| c.get(1))
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    spawns.push(MapSpawn { name, flag, x, y });
}

fn import_from_bin_or_manifest(dir: &Path, key: &str) -> Result<MapFile, Box<dyn std::error::Error>> {
    let raw = read_map_payload(dir)?;
    if raw.len() >= 4 && &raw[0..4] == map_file::MAP_MAGIC {
        return Ok(map_file::parse(&raw)?);
    }

    let manifest_path = dir.join("manifest.json");
    if !manifest_path.exists() {
        return Err(
            "Need image.png, map.bin, or manifest.json + legacy terrain".into(),
        );
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

    let mut spawns = Vec::new();
    if let Some(nations) = manifest.get("nations").and_then(|v| v.as_array()) {
        for entry in nations {
            push_spawn(entry, &mut spawns);
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

fn write_placeholder_thumbnail(
    map_file: &MapFile,
    path: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let w = map_file.width.min(256).max(1);
    let h = map_file.height.min(256).max(1);
    let mut rgba = vec![106u8; (w * h * 4) as usize];
    for (i, &byte) in map_file.terrain.iter().enumerate() {
        if i as u32 >= w * h {
            break;
        }
        if (byte & 0x80) != 0 {
            let px = i * 4;
            rgba[px] = 190;
            rgba[px + 1] = 200;
            rgba[px + 2] = 138;
            rgba[px + 3] = 255;
        }
    }
    let img = image::RgbaImage::from_raw(w, h, rgba)
        .ok_or("thumbnail buffer size mismatch")?;
    img.save(path)?;
    Ok(())
}

pub fn refresh_catalog(maps_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut items = Vec::new();
    for entry in fs::read_dir(maps_root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let key = entry.file_name().to_string_lossy().to_string();
        if key.starts_with('.') {
            continue;
        }
        let map_path = entry.path().join("map.bin");
        if !map_path.exists() {
            continue;
        }
        let bytes = fs::read(&map_path)?;
        let header = map_file::parse_header(&bytes)?;
        items.push((sow_core::maps::map_key(&key), header));
    }
    let catalog = map_file::catalog_from_headers(items);
    let catalog_bytes = map_file::encode_catalog(&catalog);
    fs::write(maps_root.join("catalog.bin"), catalog_bytes)?;
    Ok(())
}
