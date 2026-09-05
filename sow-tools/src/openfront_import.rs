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

    // Display name from OpenFront info.json ("Africa", "Europe", ...); fall back to slug.
    let display_name = read_info_json_name(&input).unwrap_or_else(|| slug.clone());

    let map_file = if input.join("image.png").exists() {
        import_from_png(&input, &display_name)?
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

    let thumb_path = out_dir.join("thumbnail.webp");
    if let Ok(img) = image::open(input.join("image.png")) {
        sow_map::write_square_thumbnail(&img, &thumb_path)
            .map_err(|e| format!("thumbnail: {e}"))?;
    } else {
        write_placeholder_thumbnail(&map_file, &thumb_path)?;
    }

    // Persist the OpenFront `multiplayer_frequency` as info.toml so the server's
    // catalog scan can build weighted playlists (0 = out of rotation).
    let frequency = read_info_json_frequency(&input);
    fs::write(
        out_dir.join("info.toml"),
        format!("# Written by sow-tools import-openfront\nfrequency = {frequency}\n"),
    )?;

    refresh_catalog(&args.maps_root)?;

    println!(
        "Imported '{}' → {}/ ({}x{}, {} spawns, {} land tiles, freq {})",
        slug,
        out_dir.display(),
        map_file.width,
        map_file.height,
        map_file.spawns.len(),
        map_file.num_land_tiles,
        frequency
    );
    Ok(())
}

/// `multiplayer_frequency` from an OpenFront `info.json` (absent → 0, matching
/// OpenFront: omitted/0 keeps the map out of the regular rotation).
fn read_info_json_frequency(dir: &Path) -> u32 {
    let Ok(blob) = fs::read_to_string(dir.join("info.json")) else {
        return 0;
    };
    let Ok(info) = serde_json::from_str::<Value>(&blob) else {
        return 0;
    };
    info.get("multiplayer_frequency")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32
}

/// `name` from an OpenFront `info.json` (used as the catalog display name).
fn read_info_json_name(dir: &Path) -> Option<String> {
    let blob = fs::read_to_string(dir.join("info.json")).ok()?;
    let info: Value = serde_json::from_str(&blob).ok()?;
    info.get("name").and_then(|v| v.as_str()).map(str::to_string)
}

fn import_from_png(dir: &Path, display_name: &str) -> Result<MapFile, Box<dyn std::error::Error>> {
    let png_path = dir.join("image.png");
    let original = image::open(&png_path)?;
    let (src_w, src_h) = (original.width(), original.height());
    // OpenFront source PNGs are huge (up to ~5M tiles). Clamp to the project's
    // MAX_MAP_PIXELS, preserving aspect, before generating the map.
    let (target_w, target_h) = clamp_map_dimensions_proportional(
        src_w,
        src_h,
        sow_core::maps::MAX_MAP_PIXELS,
    );
    let img = if target_w != src_w || target_h != src_h {
        original.resize(target_w, target_h, image::imageops::FilterType::Triangle)
    } else {
        original
    };
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

    let mut spawns = load_info_json_spawns(dir)?;
    // Spawn coordinates come from the native-resolution info.json — scale them
    // down with the image so they stay on land after clamping.
    if src_w != width || src_h != height {
        for s in &mut spawns {
            let rx = (s.x as f64 * (width as f64 / src_w as f64)).round() as u32;
            let ry = (s.y as f64 * (height as f64 / src_h as f64)).round() as u32;
            s.x = rx.min(width.saturating_sub(1));
            s.y = ry.min(height.saturating_sub(1));
        }
    }

    Ok(MapFile {
        display_name: display_name.to_string(),
        width: result.width,
        height: result.height,
        num_land_tiles: result.num_land_tiles,
        spawns,
        geo_bounds: None,
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

pub fn clamp_map_dimensions_proportional(width: u32, height: u32, max_pixels: u32) -> (u32, u32) {
    if (width as u64) * (height as u64) <= max_pixels as u64 {
        return (
            sow_core::maps::align_map_dim(width),
            sow_core::maps::align_map_dim(height),
        );
    }
    let aspect = width as f64 / height as f64;
    let h = (max_pixels as f64 / aspect).sqrt();
    let w = h * aspect;

    let mut target_w = sow_core::maps::align_map_dim(w.round() as u32);
    let mut target_h = sow_core::maps::align_map_dim(h.round() as u32);

    while (target_w as u64) * (target_h as u64) > max_pixels as u64 {
        if target_w > target_h {
            target_w = (target_w - 4).max(4);
        } else {
            target_h = (target_h - 4).max(4);
        }
    }
    (target_w, target_h)
}

fn import_from_bin_or_manifest(
    dir: &Path,
    key: &str,
) -> Result<MapFile, Box<dyn std::error::Error>> {
    let raw = read_map_payload(dir)?;
    if raw.len() >= 4 && &raw[0..4] == map_file::MAP_MAGIC {
        let mut map = map_file::parse(&raw)?;
        let width = map.width;
        let height = map.height;
        if (width as u64) * (height as u64) > sow_core::maps::MAX_MAP_PIXELS as u64 {
            let (target_w, target_h) =
                clamp_map_dimensions_proportional(width, height, sow_core::maps::MAX_MAP_PIXELS);
            let mut rescaled = Vec::with_capacity((target_w * target_h) as usize);
            for ty in 0..target_h {
                for tx in 0..target_w {
                    let sx = (tx as f64 * (width as f64 / target_w as f64)).floor() as u32;
                    let sy = (ty as f64 * (height as f64 / target_h as f64)).floor() as u32;
                    let sx = sx.min(width - 1);
                    let sy = sy.min(height - 1);
                    rescaled.push(map.terrain[(sy * width + sx) as usize]);
                }
            }
            for s in &mut map.spawns {
                let rx = (s.x as f64 * (target_w as f64 / width as f64)).round() as u32;
                let ry = (s.y as f64 * (target_h as f64 / height as f64)).round() as u32;
                s.x = rx.min(target_w - 1);
                s.y = ry.min(target_h - 1);
            }
            map.width = target_w;
            map.height = target_h;
            map.num_land_tiles = rescaled.iter().filter(|&&b| (b & 0x80) != 0).count() as u32;
            map.terrain = rescaled;
        }
        return Ok(map);
    }

    let manifest_path = dir.join("manifest.json");
    if !manifest_path.exists() {
        return Err("Need image.png, map.bin, or manifest.json + legacy terrain".into());
    }

    let manifest: Value = serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
    let map_info = manifest.get("map").ok_or("manifest missing map section")?;
    let mut width = map_info.get("width").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let mut height = map_info.get("height").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
    let display_name = manifest
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(key)
        .to_string();

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
            push_spawn(entry, &mut spawns);
        }
    }

    let mut terrain = raw;
    if (width as u64) * (height as u64) > sow_core::maps::MAX_MAP_PIXELS as u64 {
        let (target_w, target_h) =
            clamp_map_dimensions_proportional(width, height, sow_core::maps::MAX_MAP_PIXELS);
        let mut rescaled = Vec::with_capacity((target_w * target_h) as usize);
        for ty in 0..target_h {
            for tx in 0..target_w {
                let sx = (tx as f64 * (width as f64 / target_w as f64)).floor() as u32;
                let sy = (ty as f64 * (height as f64 / target_h as f64)).floor() as u32;
                let sx = sx.min(width - 1);
                let sy = sy.min(height - 1);
                rescaled.push(terrain[(sy * width + sx) as usize]);
            }
        }
        for s in &mut spawns {
            let rx = (s.x as f64 * (target_w as f64 / width as f64)).round() as u32;
            let ry = (s.y as f64 * (target_h as f64 / height as f64)).round() as u32;
            s.x = rx.min(target_w - 1);
            s.y = ry.min(target_h - 1);
        }
        width = target_w;
        height = target_h;
        terrain = rescaled;
    }

    let num_land = terrain.iter().filter(|&&b| (b & 0x80) != 0).count() as u32;

    Ok(MapFile {
        display_name,
        width,
        height,
        num_land_tiles: num_land,
        spawns,
        geo_bounds: None,
        terrain,
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
    sow_map::write_map_thumbnail(map_file, path).map_err(|e| e.into())
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
        let frequency = fs::read_to_string(entry.path().join("info.toml"))
            .map(|blob| map_file::parse_frequency_toml(&blob))
            .unwrap_or(1);
        items.push((sow_core::maps::map_key(&key), header, frequency));
    }
    let catalog = map_file::catalog_from_headers(items);
    let catalog_bytes = map_file::encode_catalog(&catalog);
    fs::write(maps_root.join("catalog.bin"), catalog_bytes)?;
    Ok(())
}
