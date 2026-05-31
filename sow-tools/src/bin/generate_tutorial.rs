use sow_core::map::MapTile;
use sow_core::map_file::{self, MapFile, MapSpawn};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

const WIDTH: u32 = 1000;
const HEIGHT: u32 = 750;
const SCALE: f32 = 1000.0 / 800.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let width = WIDTH;
    let height = HEIGHT;
    let size = (width * height) as usize;
    let mut terrain = vec![MapTile::from_byte(32); size];

    // Player Island: center (750, 900), radius 450
    // Enemy Island: center (1650, 900), radius 330
    // Neutral islands: (1245, 480) and (1245, 1320), radius 105

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let fx = x as f32;
            let fy = y as f32;

            let dx_p = fx - 250.0 * SCALE;
            let dy_p = fy - 300.0 * SCALE;
            let dist_p = (dx_p * dx_p + dy_p * dy_p).sqrt();

            let dx_e = fx - 550.0 * SCALE;
            let dy_e = fy - 300.0 * SCALE;
            let dist_e = (dx_e * dx_e + dy_e * dy_e).sqrt();

            let dx_i1 = fx - 415.0 * SCALE;
            let dy_i1 = fy - 160.0 * SCALE;
            let dist_i1 = (dx_i1 * dx_i1 + dy_i1 * dy_i1).sqrt();

            let dx_i2 = fx - 415.0 * SCALE;
            let dy_i2 = fy - 440.0 * SCALE;
            let dist_i2 = (dx_i2 * dx_i2 + dy_i2 * dy_i2).sqrt();

            if dist_p < 150.0 * SCALE {
                let on_river = is_near_line(
                    fx,
                    fy,
                    180.0 * SCALE,
                    180.0 * SCALE,
                    320.0 * SCALE,
                    380.0 * SCALE,
                    5.0 * SCALE,
                );

                if on_river {
                    terrain[idx] = MapTile::from_byte(32);
                } else {
                    let dx_m = fx - 160.0 * SCALE;
                    let dy_m = fy - 180.0 * SCALE;
                    let dist_m = (dx_m * dx_m + dy_m * dy_m).sqrt();

                    if dist_m < 35.0 * SCALE {
                        terrain[idx] = MapTile::from_byte(0b10011000);
                    } else if dist_m < 60.0 * SCALE {
                        terrain[idx] = MapTile::from_byte(0b10001100);
                    } else {
                        terrain[idx] = MapTile::from_byte(0b10000000);
                    }
                }
            } else if dist_e < 110.0 * SCALE {
                let dx_em = fx - 550.0 * SCALE;
                let dy_em = fy - 300.0 * SCALE;
                let dist_em = (dx_em * dx_em + dy_em * dy_em).sqrt();

                if dist_em < 25.0 * SCALE {
                    terrain[idx] = MapTile::from_byte(0b10011000);
                } else if dist_em < 45.0 * SCALE {
                    terrain[idx] = MapTile::from_byte(0b10001100);
                } else {
                    terrain[idx] = MapTile::from_byte(0b10000000);
                }
            } else if dist_i1 < 35.0 * SCALE || dist_i2 < 35.0 * SCALE {
                terrain[idx] = MapTile::from_byte(0b10000000);
            }
        }
    }

    let mut terrain_final = terrain.clone();
    for y in 1..height.saturating_sub(1) {
        for x in 1..width.saturating_sub(1) {
            let idx = (y * width + x) as usize;
            if terrain[idx].is_land() {
                let mut near_water = false;
                for dy in -1i32..=1 {
                    for dx in -1i32..=1 {
                        let ny = y as i32 + dy;
                        let nx = x as i32 + dx;
                        let n_idx = (ny as u32 * width + nx as u32) as usize;
                        if terrain[n_idx].is_water() {
                            near_water = true;
                            break;
                        }
                    }
                    if near_water {
                        break;
                    }
                }
                if near_water {
                    let byte = terrain[idx].as_byte() | 0b01000000;
                    terrain_final[idx] = MapTile::from_byte(byte);
                }
            }
        }
    }

    let output_dir = Path::new("assets/maps/northamerica");
    fs::create_dir_all(output_dir)?;

    let terrain_bytes: Vec<u8> = terrain_final.iter().map(|t| t.as_byte()).collect();
    let num_land = terrain_bytes.iter().filter(|b| (*b & 0x80) != 0).count() as u32;
    let map_file = MapFile {
        display_name: "North America".to_string(),
        width,
        height,
        num_land_tiles: num_land,
        spawns: vec![
            MapSpawn {
                name: "Korinthal".to_string(),
                flag: String::new(),
                x: (250.0 * SCALE as f32).round() as u32,
                y: (350.0 * SCALE as f32).round() as u32,
            },
            MapSpawn {
                name: "Lunareth".to_string(),
                flag: String::new(),
                x: (550.0 * SCALE as f32).round() as u32,
                y: (300.0 * SCALE as f32).round() as u32,
            },
        ],
        terrain: terrain_bytes,
    };
    let encoded = map_file::encode(&map_file);
    fs::write(output_dir.join("map.bin"), &encoded)?;
    println!("Wrote {} bytes to map.bin", encoded.len());

    let br_path = output_dir.join("map.bin.br");
    let mut compressor = brotli::CompressorWriter::new(File::create(&br_path)?, 4096, 11, 22);
    compressor.write_all(&encoded)?;
    compressor.flush()?;
    println!("Compressed map.bin to map.bin.br");

    let preview = sow_map::terrain_preview_image(width, height, &map_file.terrain);
    sow_map::write_square_thumbnail(&preview, &output_dir.join("thumbnail.webp"))?;
    println!("Wrote 1024x1024 thumbnail.webp");

    refresh_catalog(output_dir.parent().unwrap())?;
    println!("Refreshed assets/maps/catalog.bin");

    Ok(())
}

fn refresh_catalog(maps_root: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut items = Vec::new();
    for entry in std::fs::read_dir(maps_root)? {
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
        let bytes = std::fs::read(&map_path)?;
        let header = map_file::parse_header(&bytes)?;
        items.push((sow_core::maps::map_key(&key), header));
    }
    let catalog = map_file::catalog_from_headers(items);
    std::fs::write(
        maps_root.join("catalog.bin"),
        map_file::encode_catalog(&catalog),
    )?;
    Ok(())
}

fn is_near_line(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32, threshold: f32) -> bool {
    let l2 = (x2 - x1) * (x2 - x1) + (y2 - y1) * (y2 - y1);
    if l2 == 0.0 {
        let dx = px - x1;
        let dy = py - y1;
        return (dx * dx + dy * dy).sqrt() < threshold;
    }
    let t = ((px - x1) * (x2 - x1) + (py - y1) * (y2 - y1)) / l2;
    let t = t.clamp(0.0, 1.0);
    let proj_x = x1 + t * (x2 - x1);
    let proj_y = y1 + t * (y2 - y1);
    let dx = px - proj_x;
    let dy = py - proj_y;
    (dx * dx + dy * dy).sqrt() < threshold
}
