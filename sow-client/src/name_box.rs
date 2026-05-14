#[derive(Clone, Copy, Debug)]
pub struct NameBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

pub fn calculate_name_box(player_id: u16, map_w: u32, map_h: u32, owners: &[u16], terrain: &[u8]) -> Option<NameBox> {
    // 1. Calculate the initial bounding box for the player
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    let mut found = false;

    for y in 0..map_h as i32 {
        for x in 0..map_w as i32 {
            let idx = (y * map_w as i32 + x) as usize;
            if owners[idx] == player_id {
                if x < min_x { min_x = x; }
                if y < min_y { min_y = y; }
                if x > max_x { max_x = x; }
                if y > max_y { max_y = y; }
                found = true;
            }
        }
    }

    if !found {
        return None;
    }

    let bb_min = Point { x: min_x, y: min_y };
    let bb_max = Point { x: max_x, y: max_y };

    let width = bb_max.x - bb_min.x + 1;
    let height = bb_max.y - bb_min.y + 1;
    let size = width.min(height);

    let scaling_factor = if size < 25 {
        1
    } else if size < 50 {
        2
    } else if size < 100 {
        4
    } else if size < 250 {
        8
    } else if size < 500 {
        16
    } else {
        32
    };

    let grid = create_grid(player_id, bb_min, bb_max, scaling_factor, map_w, map_h, owners, terrain);
    let mut largest_rect = find_largest_inscribed_rectangle(&grid);

    largest_rect.x *= scaling_factor as f32;
    largest_rect.y *= scaling_factor as f32;
    largest_rect.width *= scaling_factor as f32;
    largest_rect.height *= scaling_factor as f32;

    Some(NameBox {
        x: largest_rect.x + bb_min.x as f32,
        y: largest_rect.y + bb_min.y as f32,
        width: largest_rect.width,
        height: largest_rect.height,
    })
}

fn create_grid(
    player_id: u16,
    bb_min: Point,
    bb_max: Point,
    scaling_factor: i32,
    map_w: u32,
    map_h: u32,
    owners: &[u16],
    terrain: &[u8],
) -> Vec<Vec<bool>> {
    let scaled_min_x = bb_min.x / scaling_factor;
    let scaled_min_y = bb_min.y / scaling_factor;
    let scaled_max_x = bb_max.x / scaling_factor;
    let scaled_max_y = bb_max.y / scaling_factor;

    let width = (scaled_max_x - scaled_min_x + 1) as usize;
    let height = (scaled_max_y - scaled_min_y + 1) as usize;

    let mut grid = vec![vec![false; height]; width];

    for x in scaled_min_x..=scaled_max_x {
        for y in scaled_min_y..=scaled_max_y {
            let cx = x * scaling_factor;
            let cy = y * scaling_factor;

            if cx >= 0 && cx < map_w as i32 && cy >= 0 && cy < map_h as i32 {
                let idx = (cy * map_w as i32 + cx) as usize;
                let owner = owners[idx];
                let t_byte = terrain[idx];
                // In SOW, terrain: 0x80 bit is land.
                let is_water = (t_byte & 0x80) == 0;
                
                // Solid blocks inside territory include owned land or water completely surrounded
                // OpenFront NameBoxCalculator treats water/lake as safe to draw text over if bounded by empire.
                grid[(x - scaled_min_x) as usize][(y - scaled_min_y) as usize] = owner == player_id || is_water;
            }
        }
    }

    grid
}

fn find_largest_inscribed_rectangle(grid: &[Vec<bool>]) -> NameBox {
    if grid.is_empty() || grid[0].is_empty() {
        return NameBox { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };
    }

    let cols = grid.len();
    let rows = grid[0].len();
    let mut heights = vec![0; cols];
    let mut largest_rect = NameBox { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };

    for row in 0..rows {
        for col in 0..cols {
            if grid[col][row] {
                heights[col] += 1;
            } else {
                heights[col] = 0;
            }
        }

        let rect_for_row = largest_rectangle_in_histogram(&heights);

        if rect_for_row.width * rect_for_row.height > largest_rect.width * largest_rect.height {
            largest_rect = NameBox {
                x: rect_for_row.x,
                y: (row as f32 - rect_for_row.height + 1.0),
                width: rect_for_row.width,
                height: rect_for_row.height,
            };
        }
    }

    largest_rect
}

fn largest_rectangle_in_histogram(widths: &[i32]) -> NameBox {
    let mut stack: Vec<usize> = Vec::new();
    let mut max_area = 0;
    let mut largest_rect = NameBox { x: 0.0, y: 0.0, width: 0.0, height: 0.0 };

    let n = widths.len();
    for i in 0..=n {
        let h = if i == n { 0 } else { widths[i] };

        while !stack.is_empty() && h < widths[*stack.last().unwrap()] {
            let top_idx = stack.pop().unwrap();
            let height = widths[top_idx];
            let width = if stack.is_empty() {
                i as i32
            } else {
                (i - *stack.last().unwrap() - 1) as i32
            };

            if height * width > max_area {
                max_area = height * width;
                largest_rect = NameBox {
                    x: if stack.is_empty() { 0.0 } else { *stack.last().unwrap() as f32 + 1.0 },
                    y: 0.0,
                    width: width as f32,
                    height: height as f32,
                };
            }
        }
        stack.push(i);
    }

    largest_rect
}
