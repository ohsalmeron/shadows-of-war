use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Coord {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainType {
    Land,
    Water,
}

#[derive(Clone, Debug)]
pub struct TerrainTile {
    pub tile_type: TerrainType,
    pub shoreline: bool,
    pub magnitude: f64,
    pub ocean: bool,
}

pub struct MapResult {
    pub map_data: Vec<u8>,
    pub mini_map_data: Vec<u8>,
    pub thumbnail_data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub num_land_tiles: u32,
    pub mini_width: u32,
    pub mini_height: u32,
    pub mini_num_land_tiles: u32,
}

pub struct GeneratorArgs {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<[u8; 4]>, // RGBA pixels
    pub remove_small: bool,
}

pub fn generate_map(args: GeneratorArgs) -> Result<MapResult, String> {
    let mut width = args.width;
    let mut height = args.height;

    // Ensure dimensions are even for mini-map downscaling
    width = width - (width % 2);
    height = height - (height % 2);

    if width == 0 || height == 0 || args.pixels.len() < (width * height) as usize {
        return Err("Invalid map dimensions or pixel buffer size".to_string());
    }

    log::info!("Processing Map: dimensions {}x{}", width, height);

    let mut grid = vec![
        vec![
            TerrainTile {
                tile_type: TerrainType::Water,
                shoreline: false,
                magnitude: 0.0,
                ocean: false,
            };
            height as usize
        ];
        width as usize
    ];

    // Convert pixel colors to terrain types and magnitudes (matching Go / OpenFrontIO)
    for x in 0..width as usize {
        for y in 0..height as usize {
            let idx = y * args.width as usize + x;
            let rgba = args.pixels[idx];
            let blue = rgba[2];
            let alpha = rgba[3];

            if alpha < 20 || blue == 106 {
                grid[x][y].tile_type = TerrainType::Water;
            } else {
                grid[x][y].tile_type = TerrainType::Land;
                let mag = (blue as f64).clamp(140.0, 200.0) - 140.0;
                grid[x][y].magnitude = mag / 2.0;
            }
        }
    }

    if args.remove_small {
        remove_small_islands(&mut grid);
    }
    process_water(&mut grid, args.remove_small);

    let mini_grid = create_mini_map(&grid);
    let thumbnail_rgba = create_map_thumbnail(&mini_grid, 0.5);

    let mut thumbnail_data = Vec::new();
    if let Err(e) = image::codecs::webp::WebPEncoder::new_lossless(&mut thumbnail_data).encode(
        &thumbnail_rgba,
        (mini_grid.len() as u32 / 2).max(1),
        (mini_grid[0].len() as u32 / 2).max(1),
        image::ExtendedColorType::Rgba8,
    ) {
        log::error!("Failed to encode thumbnail WebP: {:?}", e);
    }

    let (map_data, num_land_tiles) = pack_terrain(&grid);
    let (mini_map_data, mini_num_land_tiles) = pack_terrain(&mini_grid);

    Ok(MapResult {
        map_data,
        mini_map_data,
        thumbnail_data,
        width,
        height,
        num_land_tiles,
        mini_width: width / 2,
        mini_height: height / 2,
        mini_num_land_tiles,
    })
}

fn remove_small_islands(grid: &mut Vec<Vec<TerrainTile>>) {
    let width = grid.len();
    let height = grid[0].len();
    let mut visited = vec![vec![false; height]; width];

    for x in 0..width {
        for y in 0..height {
            if grid[x][y].tile_type == TerrainType::Land && !visited[x][y] {
                let coords = get_area(x, y, grid, &mut visited, TerrainType::Land);
                if coords.len() < 30 {
                    for coord in coords {
                        grid[coord.x as usize][coord.y as usize].tile_type = TerrainType::Water;
                        grid[coord.x as usize][coord.y as usize].magnitude = 0.0;
                    }
                }
            }
        }
    }
    log::info!("Small island removal completed.");
}

fn process_water(grid: &mut Vec<Vec<TerrainTile>>, remove_small: bool) {
    let width = grid.len();
    let height = grid[0].len();
    let mut visited = vec![vec![false; height]; width];
    let mut water_bodies = Vec::new();

    for x in 0..width {
        for y in 0..height {
            if grid[x][y].tile_type == TerrainType::Water && !visited[x][y] {
                let coords = get_area(x, y, grid, &mut visited, TerrainType::Water);
                water_bodies.push(coords);
            }
        }
    }

    // Sort by size (largest first)
    water_bodies.sort_by(|a, b| b.len().cmp(&a.len()));

    if !water_bodies.is_empty() {
        // Mark largest water body as ocean
        for coord in &water_bodies[0] {
            grid[coord.x as usize][coord.y as usize].ocean = true;
        }

        if remove_small {
            let mut small_lakes = 0;
            for body in water_bodies.iter().skip(1) {
                if body.len() < 200 {
                    small_lakes += 1;
                    for coord in body {
                        grid[coord.x as usize][coord.y as usize].tile_type = TerrainType::Land;
                        grid[coord.x as usize][coord.y as usize].magnitude = 0.0;
                    }
                }
            }
            log::info!("Removed {} small lakes (< 200 tiles).", small_lakes);
        }

        let shoreline_waters = process_shore(grid);
        process_dist_to_land(shoreline_waters, grid);
    }
}

fn get_area(
    start_x: usize,
    start_y: usize,
    grid: &Vec<Vec<TerrainTile>>,
    visited: &mut Vec<Vec<bool>>,
    target_type: TerrainType,
) -> Vec<Coord> {
    let width = grid.len() as i32;
    let height = grid[0].len() as i32;
    let mut area = Vec::new();
    let mut queue = VecDeque::new();

    queue.push_back(Coord {
        x: start_x as i32,
        y: start_y as i32,
    });
    visited[start_x][start_y] = true;

    let directions = [(0, 1), (1, 0), (0, -1), (-1, 0)];

    while let Some(coord) = queue.pop_front() {
        area.push(coord);

        for &(dx, dy) in &directions {
            let nx = coord.x + dx;
            let ny = coord.y + dy;

            if nx >= 0 && ny >= 0 && nx < width && ny < height {
                let ux = nx as usize;
                let uy = ny as usize;
                if !visited[ux][uy] && grid[ux][uy].tile_type == target_type {
                    visited[ux][uy] = true;
                    queue.push_back(Coord { x: nx, y: ny });
                }
            }
        }
    }

    area
}

fn process_shore(grid: &mut Vec<Vec<TerrainTile>>) -> Vec<Coord> {
    let width = grid.len();
    let height = grid[0].len();
    let mut shoreline_waters = Vec::new();
    let directions = [(0, 1), (1, 0), (0, -1), (-1, 0)];

    for x in 0..width {
        for y in 0..height {
            let tile_type = grid[x][y].tile_type;
            let mut is_shore = false;

            for &(dx, dy) in &directions {
                let nx = x as i32 + dx;
                let ny = y as i32 + dy;

                if nx >= 0 && ny >= 0 && nx < width as i32 && ny < height as i32 {
                    let neighbor_type = grid[nx as usize][ny as usize].tile_type;
                    if neighbor_type != tile_type {
                        is_shore = true;
                        break;
                    }
                }
            }

            if is_shore {
                grid[x][y].shoreline = true;
                if tile_type == TerrainType::Water {
                    shoreline_waters.push(Coord {
                        x: x as i32,
                        y: y as i32,
                    });
                }
            }
        }
    }

    shoreline_waters
}

fn process_dist_to_land(shoreline_waters: Vec<Coord>, grid: &mut Vec<Vec<TerrainTile>>) {
    let width = grid.len();
    let height = grid[0].len();
    let mut visited = vec![vec![false; height]; width];
    let mut queue = VecDeque::new();

    for coord in shoreline_waters {
        queue.push_back((coord, 0));
        visited[coord.x as usize][coord.y as usize] = true;
        grid[coord.x as usize][coord.y as usize].magnitude = 0.0;
    }

    let directions = [(0, 1), (1, 0), (0, -1), (-1, 0)];

    while let Some((coord, dist)) = queue.pop_front() {
        for &(dx, dy) in &directions {
            let nx = coord.x + dx;
            let ny = coord.y + dy;

            if nx >= 0 && ny >= 0 && nx < width as i32 && ny < height as i32 {
                let ux = nx as usize;
                let uy = ny as usize;

                if !visited[ux][uy] && grid[ux][uy].tile_type == TerrainType::Water {
                    visited[ux][uy] = true;
                    grid[ux][uy].magnitude = (dist + 1) as f64;
                    queue.push_back((Coord { x: nx, y: ny }, dist + 1));
                }
            }
        }
    }
}

fn create_mini_map(grid: &Vec<Vec<TerrainTile>>) -> Vec<Vec<TerrainTile>> {
    let width = grid.len();
    let height = grid[0].len();
    let mini_width = width / 2;
    let mini_height = height / 2;

    let mut mini_grid = vec![
        vec![
            TerrainTile {
                tile_type: TerrainType::Land,
                shoreline: false,
                magnitude: 0.0,
                ocean: false,
            };
            mini_height
        ];
        mini_width
    ];

    for x in 0..width {
        for y in 0..height {
            let mx = x / 2;
            let my = y / 2;

            if mx < mini_width && my < mini_height {
                if grid[x][y].tile_type == TerrainType::Water {
                    mini_grid[mx][my].tile_type = TerrainType::Water;
                    mini_grid[mx][my].ocean = grid[x][y].ocean || mini_grid[mx][my].ocean;
                } else if mini_grid[mx][my].tile_type != TerrainType::Water {
                    mini_grid[mx][my] = grid[x][y].clone();
                }
            }
        }
    }

    mini_grid
}

fn create_map_thumbnail(terrain: &Vec<Vec<TerrainTile>>, quality: f64) -> Vec<u8> {
    let src_width = terrain.len();
    let src_height = terrain[0].len();

    let target_width = ((src_width as f64 * quality).floor() as usize).max(1);
    let target_height = ((src_height as f64 * quality).floor() as usize).max(1);

    let mut pixels = vec![0u8; target_width * target_height * 4];

    for x in 0..target_width {
        for y in 0..target_height {
            let src_x = (((x as f64) / quality).floor() as usize).min(src_width - 1);
            let src_y = (((y as f64) / quality).floor() as usize).min(src_height - 1);

            let tile = &terrain[src_x][src_y];
            let color = get_thumbnail_color(tile);

            let idx = (y * target_width + x) * 4;
            pixels[idx] = color[0];
            pixels[idx + 1] = color[1];
            pixels[idx + 2] = color[2];
            pixels[idx + 3] = color[3];
        }
    }

    pixels
}

fn get_thumbnail_color(t: &TerrainTile) -> [u8; 4] {
    if t.tile_type == TerrainType::Water {
        if t.shoreline {
            return [100, 143, 255, 255];
        }
        let water_adj = (11.0 - (t.magnitude / 2.0).min(10.0) - 10.0) as i32;
        return [
            (70 + water_adj).max(0).min(255) as u8,
            (132 + water_adj).max(0).min(255) as u8,
            (180 + water_adj).max(0).min(255) as u8,
            255,
        ];
    }

    if t.shoreline {
        return [204, 203, 158, 255];
    }

    if t.magnitude < 10.0 {
        // Plains
        let adj = 220.0 - 2.0 * t.magnitude;
        return [190, adj.max(0.0).min(255.0) as u8, 138, 255];
    } else if t.magnitude < 20.0 {
        // Highlands
        let adj = 2.0 * t.magnitude;
        return [
            (200.0 + adj).max(0.0).min(255.0) as u8,
            (183.0 + adj).max(0.0).min(255.0) as u8,
            (138.0 + adj).max(0.0).min(255.0) as u8,
            255,
        ];
    } else {
        // Mountains
        let adj = (230.0 + t.magnitude / 2.0).floor();
        let adj_val = adj.max(0.0).min(255.0) as u8;
        return [adj_val, adj_val, adj_val, 255];
    }
}

fn pack_terrain(terrain: &Vec<Vec<TerrainTile>>) -> (Vec<u8>, u32) {
    let width = terrain.len();
    let height = terrain[0].len();
    let mut packed = vec![0u8; width * height];
    let mut num_land = 0;

    for x in 0..width {
        for y in 0..height {
            let tile = &terrain[x][y];
            let mut byte = 0u8;

            if tile.tile_type == TerrainType::Land {
                byte |= 0b10000000;
                num_land += 1;
            }
            if tile.shoreline {
                byte |= 0b01000000;
            }
            if tile.ocean {
                byte |= 0b00100000;
            }

            let mag_val = if tile.tile_type == TerrainType::Land {
                tile.magnitude.ceil().min(31.0) as u8
            } else {
                (tile.magnitude / 2.0).ceil().min(31.0) as u8
            };

            byte |= mag_val & 0b00011111;

            packed[y * width + x] = byte;
        }
    }

    (packed, num_land)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_map_generation() {
        let width = 10;
        let height = 10;
        let mut pixels = vec![[106, 106, 106, 255]; (width * height) as usize];

        for x in 3..7 {
            for y in 3..7 {
                let idx = y * width + x;
                pixels[idx] = [0, 0, 180, 255];
            }
        }

        let args = GeneratorArgs {
            width: width as u32,
            height: height as u32,
            pixels,
            remove_small: false,
        };

        let res = generate_map(args).unwrap();
        assert_eq!(res.width, 10);
        assert_eq!(res.height, 10);
        assert!(res.num_land_tiles > 0);

        let center_tile = res.map_data[5 * 10 + 5];
        assert_ne!(center_tile & 0b10000000, 0);
    }
}
