
pub(super) fn largest_owned_cluster_bbox(
    map: &crate::map::GameMap,
    player_id: u16,
    border_tiles: &crate::bitset::DenseBitSet,
) -> Option<(u32, u32, u32, u32)> {
    let map_size = (map.width * map.height) as usize;
    let mut visited = vec![false; map_size];

    let mut largest_min_x = u32::MAX;
    let mut largest_min_y = u32::MAX;
    let mut largest_max_x = 0;
    let mut largest_max_y = 0;
    let mut max_cluster_size = 0;

    for start_idx in border_tiles.ones() {
        if visited[start_idx as usize] {
            continue;
        }
        let sx = start_idx % map.width;
        let sy = start_idx / map.width;
        if map.owner_id(sx, sy) != player_id {
            continue;
        }

        let mut queue = vec![start_idx];
        visited[start_idx as usize] = true;

        let mut cluster_min_x = u32::MAX;
        let mut cluster_min_y = u32::MAX;
        let mut cluster_max_x = 0;
        let mut cluster_max_y = 0;
        let mut count = 0;

        let mut q_idx = 0;
        while q_idx < queue.len() {
            let curr = queue[q_idx];
            q_idx += 1;

            let cx = curr % map.width;
            let cy = curr / map.width;

            if cx < cluster_min_x {
                cluster_min_x = cx;
            }
            if cy < cluster_min_y {
                cluster_min_y = cy;
            }
            if cx > cluster_max_x {
                cluster_max_x = cx;
            }
            if cy > cluster_max_y {
                cluster_max_y = cy;
            }
            count += 1;

            map.for_each_neighbor(cx, cy, |nx, ny| {
                let n_idx = (ny * map.width + nx) as usize;
                if map.owner_id(nx, ny) == player_id && !visited[n_idx] {
                    visited[n_idx] = true;
                    queue.push(ny * map.width + nx);
                }
            });
        }

        if count > max_cluster_size {
            max_cluster_size = count;
            largest_min_x = cluster_min_x;
            largest_min_y = cluster_min_y;
            largest_max_x = cluster_max_x;
            largest_max_y = cluster_max_y;
        }
    }

    if largest_min_x == u32::MAX {
        None
    } else {
        Some((largest_min_x, largest_min_y, largest_max_x, largest_max_y))
    }
}

pub(super) fn nameplate_grid_valid(map: &crate::map::GameMap, player_id: u16, map_x: u32, map_y: u32) -> bool {
    let r = map.ref_id(map_x, map_y);
    let tile = map.terrain[r];
    let is_owned = map.owner_id(map_x, map_y) == player_id;
    let is_lake = tile.terrain_type() == crate::map::TerrainType::Lake;
    let is_shore = tile.is_land() && tile.is_shoreline();
    is_owned || is_lake || is_shore
}

#[allow(clippy::too_many_arguments)]
pub(super) fn nearest_owned_land(
    map: &crate::map::GameMap,
    player_id: u16,
    prefer_x: u32,
    prefer_y: u32,
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
) -> (u32, u32) {
    let px = prefer_x.min(map.width.saturating_sub(1));
    let py = prefer_y.min(map.height.saturating_sub(1));
    let r = map.ref_id(px, py);
    if map.terrain[r].is_land() && map.owner_id(px, py) == player_id {
        return (px, py);
    }

    let mut best = (px, py);
    let mut best_dist = u32::MAX;
    for y in min_y..=max_y.min(map.height.saturating_sub(1)) {
        for x in min_x..=max_x.min(map.width.saturating_sub(1)) {
            if map.owner_id(x, y) != player_id {
                continue;
            }
            if !map.terrain[map.ref_id(x, y)].is_land() {
                continue;
            }
            let dist = px.abs_diff(x) + py.abs_diff(y);
            if dist < best_dist {
                best_dist = dist;
                best = (x, y);
            }
        }
    }
    best
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Rectangle {
    pub(super) x: u32,
    pub(super) y: u32,
    pub(super) width: u32,
    pub(super) height: u32,
}

pub(super) fn largest_rectangle_in_histogram(widths: &[u32]) -> Rectangle {
    let mut stack = Vec::new();
    let mut max_area = 0;
    let mut largest_rect = Rectangle::default();

    for i in 0..=widths.len() {
        let h = if i == widths.len() { 0 } else { widths[i] };

        while !stack.is_empty() && h < widths[*stack.last().unwrap()] {
            let height = widths[stack.pop().unwrap()];
            let width = if stack.is_empty() {
                i as u32
            } else {
                (i - *stack.last().unwrap() - 1) as u32
            };

            let area = height * width;
            if area > max_area {
                max_area = area;
                largest_rect = Rectangle {
                    x: if stack.is_empty() {
                        0
                    } else {
                        (*stack.last().unwrap() + 1) as u32
                    },
                    y: 0,
                    width,
                    height,
                };
            }
        }
        stack.push(i);
    }

    largest_rect
}

pub(super) fn find_largest_inscribed_rectangle(grid: &[Vec<bool>]) -> Rectangle {
    if grid.is_empty() || grid[0].is_empty() {
        return Rectangle::default();
    }
    let cols = grid.len();
    let rows = grid[0].len();
    let mut heights = vec![0u32; cols];
    let mut largest_rect = Rectangle::default();

    #[allow(clippy::needless_range_loop)]
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
            largest_rect = Rectangle {
                x: rect_for_row.x,
                y: (row as u32)
                    .saturating_sub(rect_for_row.height)
                    .saturating_add(1),
                width: rect_for_row.width,
                height: rect_for_row.height,
            };
        }
    }

    largest_rect
}

impl super::Player {
    pub fn calculate_nameplate(&mut self, map: &crate::map::GameMap) {
        if self.tile_count == 0 || self.border_tiles.is_empty() {
            self.nameplate_x = 0.0;
            self.nameplate_y = 0.0;
            self.nameplate_size = 0.0;
            self.nameplate_dirty = false;
            return;
        }

        let Some((min_x, min_y, max_x, max_y)) =
            largest_owned_cluster_bbox(map, self.id, &self.border_tiles)
        else {
            self.nameplate_x = 0.0;
            self.nameplate_y = 0.0;
            self.nameplate_size = 0.0;
            self.nameplate_dirty = false;
            return;
        };

        let width = max_x.saturating_sub(min_x) + 1;
        let height = max_y.saturating_sub(min_y) + 1;
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

        let scaled_min_x = min_x / scaling_factor;
        let scaled_min_y = min_y / scaling_factor;
        let scaled_max_x = max_x / scaling_factor;
        let scaled_max_y = max_y / scaling_factor;

        let grid_width = (scaled_max_x.saturating_sub(scaled_min_x) + 1) as usize;
        let grid_height = (scaled_max_y.saturating_sub(scaled_min_y) + 1) as usize;

        if grid_width == 0 || grid_height == 0 || grid_width > 1000 || grid_height > 1000 {
            let prefer_x = min_x + (max_x.saturating_sub(min_x)) / 2;
            let prefer_y = min_y + (max_y.saturating_sub(min_y)) / 2;
            let (cx, cy) =
                nearest_owned_land(map, self.id, prefer_x, prefer_y, min_x, min_y, max_x, max_y);
            self.nameplate_x = cx as f32;
            self.nameplate_y = cy as f32;
            self.nameplate_size = 1.5;
            self.nameplate_dirty = false;
            return;
        }

        let mut grid = vec![vec![false; grid_height]; grid_width];

        for (gx, column) in grid.iter_mut().enumerate().take(grid_width) {
            for (gy, item) in column.iter_mut().enumerate().take(grid_height) {
                let map_x = (scaled_min_x + gx as u32) * scaling_factor;
                let map_y = (scaled_min_y + gy as u32) * scaling_factor;

                if map_x < map.width && map_y < map.height {
                    *item = nameplate_grid_valid(map, self.id, map_x, map_y);
                }
            }
        }

        let mut largest_rect = find_largest_inscribed_rectangle(&grid);
        if largest_rect.width == 0 || largest_rect.height == 0 {
            let prefer_x = min_x + (max_x.saturating_sub(min_x)) / 2;
            let prefer_y = min_y + (max_y.saturating_sub(min_y)) / 2;
            let (cx, cy) =
                nearest_owned_land(map, self.id, prefer_x, prefer_y, min_x, min_y, max_x, max_y);
            self.nameplate_x = cx as f32;
            self.nameplate_y = cy as f32;
            self.nameplate_size = 1.5;
            self.nameplate_dirty = false;
            return;
        }

        largest_rect.x *= scaling_factor;
        largest_rect.y *= scaling_factor;
        largest_rect.width *= scaling_factor;
        largest_rect.height *= scaling_factor;

        let center_x = largest_rect.x + largest_rect.width / 2 + min_x;
        let center_y = largest_rect.y + largest_rect.height / 2 + min_y;
        let (land_x, land_y) =
            nearest_owned_land(map, self.id, center_x, center_y, min_x, min_y, max_x, max_y);

        let name_len = self.name.chars().count().max(1) as f32;
        let width_constrained = (largest_rect.width as f32 / name_len) * 2.0;
        let height_constrained = largest_rect.height as f32 / 3.0;
        let font_size = width_constrained.min(height_constrained).clamp(0.2, 24.0);

        self.nameplate_x = land_x as f32;
        self.nameplate_y = land_y as f32 - (font_size / 3.0);
        self.nameplate_size = font_size;
        self.nameplate_dirty = false;
    }
}

