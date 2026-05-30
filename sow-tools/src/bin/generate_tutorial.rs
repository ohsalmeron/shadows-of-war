use sow_core::map::MapTile;
use sow_core::map_file::{self, MapFile, MapSpawn};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let width: u32 = 800;
    let height: u32 = 600;
    let size = (width * height) as usize;
    let mut terrain = vec![MapTile::from_byte(32); size]; // Start with all Ocean (value 32)

    // Player Island: center at (250, 300), radius 150
    // Enemy Island: center at (550, 300), radius 110
    // Small neutral island 1: center (415, 160), radius 35
    // Small neutral island 2: center (415, 440), radius 35

    for y in 0..height {
        for x in 0..width {
            let idx = (y * width + x) as usize;
            let fx = x as f32;
            let fy = y as f32;

            // Player Island
            let dx_p = fx - 250.0;
            let dy_p = fy - 300.0;
            let dist_p = (dx_p * dx_p + dy_p * dy_p).sqrt();

            // Enemy Island
            let dx_e = fx - 550.0;
            let dy_e = fy - 300.0;
            let dist_e = (dx_e * dx_e + dy_e * dy_e).sqrt();

            // Island 1
            let dx_i1 = fx - 415.0;
            let dy_i1 = fy - 160.0;
            let dist_i1 = (dx_i1 * dx_i1 + dy_i1 * dy_i1).sqrt();

            // Island 2
            let dx_i2 = fx - 415.0;
            let dy_i2 = fy - 440.0;
            let dist_i2 = (dx_i2 * dx_i2 + dy_i2 * dy_i2).sqrt();

            if dist_p < 150.0 {
                // Determine if this is on the river
                // River starting near mountain range (180, 180) and flowing south-east (320, 380)
                let on_river = is_near_line(fx, fy, 180.0, 180.0, 320.0, 380.0, 5.0);

                if on_river {
                    terrain[idx] = MapTile::from_byte(32); // Water/Ocean
                } else {
                    // Place a mountain range on the player's island
                    let dx_m = fx - 160.0;
                    let dy_m = fy - 180.0;
                    let dist_m = (dx_m * dx_m + dy_m * dy_m).sqrt();

                    if dist_m < 35.0 {
                        terrain[idx] = MapTile::from_byte(0b10011000); // Mountain (magnitude 24)
                    } else if dist_m < 60.0 {
                        terrain[idx] = MapTile::from_byte(0b10001100); // Highland (magnitude 12)
                    } else {
                        terrain[idx] = MapTile::from_byte(0b10000000); // Standard land
                    }
                }
            } else if dist_e < 110.0 {
                // Enemy Island: plain land with a mountain in the center
                let dx_em = fx - 550.0;
                let dy_em = fy - 300.0;
                let dist_em = (dx_em * dx_em + dy_em * dy_em).sqrt();

                if dist_em < 25.0 {
                    terrain[idx] = MapTile::from_byte(0b10011000); // Mountain
                } else if dist_em < 45.0 {
                    terrain[idx] = MapTile::from_byte(0b10001100); // Highland
                } else {
                    terrain[idx] = MapTile::from_byte(0b10000000); // Standard land
                }
            } else if dist_i1 < 35.0 || dist_i2 < 35.0 {
                terrain[idx] = MapTile::from_byte(0b10000000); // Small neutral islands
            }
        }
    }

    // Mark shorelines
    // Let's iterate and identify any land tile adjacent to a water tile.
    // If so, set its shore bit (bit 6).
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
                    // Set bit 6: shoreline
                    let byte = terrain[idx].as_byte() | 0b01000000;
                    terrain_final[idx] = MapTile::from_byte(byte);
                }
            }
        }
    }

    // Export to assets/maps/northamerica/
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
                x: 250,
                y: 350,
            },
            MapSpawn {
                name: "Lunareth".to_string(),
                flag: String::new(),
                x: 550,
                y: 300,
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

    // Placeholder thumbnail (generate via scripts/generate_placeholder_assets.py if missing)
    let dst_thumb = output_dir.join("thumbnail.webp");
    if !dst_thumb.exists() {
        println!("Note: add thumbnail.webp to {}", output_dir.display());
    }

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
