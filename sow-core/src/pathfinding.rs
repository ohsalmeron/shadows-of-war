//! Deterministic water A* (LegacyEngine `AStar.Water.ts` port). No HashMap iteration; integer costs only.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, VecDeque};

use crate::map::GameMap;

const LAND_BIT: u8 = 7;
const MAGNITUDE_MASK: u8 = 0x1f;
const COST_SCALE: u32 = 100;
const BASE_COST: u32 = COST_SCALE;

#[inline]
fn magnitude_penalty(magnitude: u8) -> u32 {
    if magnitude < 3 {
        10 * COST_SCALE
    } else if magnitude <= 10 {
        0
    } else {
        COST_SCALE
    }
}

#[inline]
fn cross_tie_breaker(
    nx: u32,
    ny: u32,
    goal_x: u32,
    goal_y: u32,
    start_x: u32,
    start_y: u32,
    cross_norm: u32,
) -> u32 {
    let dx_goal = goal_x as i64 - start_x as i64;
    let dy_goal = goal_y as i64 - start_y as i64;
    let dx_n = nx as i64 - goal_x as i64;
    let dy_n = ny as i64 - goal_y as i64;
    let cross = (dx_goal * dy_n - dy_goal * dx_n).wrapping_abs() as u64;
    let cn = cross_norm.max(1) as u64;
    ((cross * (COST_SCALE - 1) as u64) / cn / cn) as u32
}

/// Open heap node: max-heap by `Ord` pops smallest `f_score` first (see `cmp`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct AStarNode {
    f_score: u32,
    insert_seq: u32,
    idx: u32,
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        match other.f_score.cmp(&self.f_score) {
            Ordering::Equal => match other.insert_seq.cmp(&self.insert_seq) {
                Ordering::Equal => other.idx.cmp(&self.idx),
                o => o,
            },
            o => o,
        }
    }
}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Reusable buffers for water-only 4-neighbor A* (matches LegacyEngine defaults).
#[derive(Debug, Clone)]
pub struct WaterAStar {
    width: u32,
    height: u32,
    stamp: u32,
    pub(crate) closed_stamp: Vec<u32>,
    pub(crate) gscore_stamp: Vec<u32>,
    pub(crate) gscore: Vec<u32>,
    pub(crate) came_from: Vec<i32>,
    pub(crate) heap: BinaryHeap<AStarNode>,
    heuristic_weight: u32,
    max_iterations: u32,
    push_seq: u32,
    pub(crate) macro_closed_stamp: Vec<u32>,
    pub(crate) macro_gscore: Vec<u32>,
    pub(crate) macro_gscore_stamp: Vec<u32>,
    pub(crate) macro_came_from: Vec<i32>,
    pub(crate) allowed_chunks: Vec<bool>,
}

impl Default for WaterAStar {
    fn default() -> Self {
        Self::new()
    }
}

#[inline]
fn chunk_contains_water(
    map: &GameMap,
    cx: u32,
    cy: u32,
    goal_cx: u32,
    goal_cy: u32,
    starts: &[u32],
) -> bool {
    if cx == goal_cx && cy == goal_cy {
        return true;
    }
    for &s in starts {
        let sx = s % map.width;
        let sy = s / map.width;
        if sx / 16 == cx && sy / 16 == cy {
            return true;
        }
    }
    let start_x = cx * 16;
    let start_y = cy * 16;
    let end_x = (start_x + 16).min(map.width);
    let end_y = (start_y + 16).min(map.height);
    for y in start_y..end_y {
        for x in start_x..end_x {
            let idx = (y * map.width + x) as usize;
            if idx < map.terrain.len() && !map.terrain[idx].is_land() {
                return true;
            }
        }
    }
    false
}

impl WaterAStar {
    pub fn new() -> Self {
        Self {
            width: 0,
            height: 0,
            stamp: 0,
            closed_stamp: Vec::new(),
            gscore_stamp: Vec::new(),
            gscore: Vec::new(),
            came_from: Vec::new(),
            heap: BinaryHeap::new(),
            heuristic_weight: 5,
            max_iterations: 1_000_000,
            push_seq: 0,
            macro_closed_stamp: Vec::new(),
            macro_gscore: Vec::new(),
            macro_gscore_stamp: Vec::new(),
            macro_came_from: Vec::new(),
            allowed_chunks: Vec::new(),
        }
    }

    pub fn ensure_capacity(&mut self, map: &GameMap) {
        let n = (map.width * map.height) as usize;
        if self.closed_stamp.len() != n {
            self.closed_stamp.resize(n, 0);
            self.gscore_stamp.resize(n, 0);
            self.gscore.resize(n, 0);
            self.came_from.resize(n, -1);
            self.width = map.width;
            self.height = map.height;
        }
        let macro_cols = map.width.div_ceil(16) as usize;
        let macro_rows = map.height.div_ceil(16) as usize;
        let mn = macro_cols * macro_rows;
        if self.allowed_chunks.len() != mn {
            self.allowed_chunks.resize(mn, false);
            self.macro_closed_stamp.resize(mn, 0);
            self.macro_gscore.resize(mn, 0);
            self.macro_gscore_stamp.resize(mn, 0);
            self.macro_came_from.resize(mn, -1);
        }
    }

    fn find_macro_path(&mut self, map: &GameMap, starts: &[u32], goal: u32) -> bool {
        let width = map.width;
        let height = map.height;
        let macro_cols = width.div_ceil(16);
        let macro_rows = height.div_ceil(16);
        let num_macro_nodes = (macro_cols * macro_rows) as usize;
        if num_macro_nodes == 0 {
            return false;
        }

        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.macro_closed_stamp.fill(0);
            self.macro_gscore_stamp.fill(0);
            self.stamp = 1;
        }
        let stamp = self.stamp;
        self.push_seq = 0;

        let goal_x = goal % width;
        let goal_y = goal / width;
        let goal_cx = goal_x / 16;
        let goal_cy = goal_y / 16;
        let goal_chunk = goal_cy * macro_cols + goal_cx;

        self.heap.clear();

        for &s in starts {
            let sx = s % width;
            let sy = s / width;
            let scx = sx / 16;
            let scy = sy / 16;
            let chunk_idx = scy * macro_cols + scx;
            if chunk_idx as usize >= num_macro_nodes {
                continue;
            }
            let idx = chunk_idx as usize;
            self.macro_gscore[idx] = 0;
            self.macro_gscore_stamp[idx] = stamp;
            self.macro_came_from[idx] = -1;

            let h = manhattan(scx, scy, goal_cx, goal_cy) * 16 * COST_SCALE;
            self.push_seq = self.push_seq.wrapping_add(1);
            self.heap.push(AStarNode {
                f_score: h,
                insert_seq: self.push_seq,
                idx: chunk_idx,
            });
        }

        let mut found = false;
        let mut iterations = 10000;

        while let Some(node) = self.heap.pop() {
            if iterations == 0 {
                break;
            }
            iterations -= 1;

            let current = node.idx as usize;
            if self.macro_closed_stamp[current] == stamp {
                continue;
            }
            self.macro_closed_stamp[current] = stamp;

            if node.idx == goal_chunk {
                found = true;
                break;
            }

            let current_g = self.macro_gscore[current];
            let current_cx = node.idx % macro_cols;
            let current_cy = node.idx / macro_cols;

            let neighbors = [
                current_cy.checked_sub(1).map(|ny| (current_cx, ny)),
                if current_cy + 1 < macro_rows {
                    Some((current_cx, current_cy + 1))
                } else {
                    None
                },
                current_cx.checked_sub(1).map(|nx| (nx, current_cy)),
                if current_cx + 1 < macro_cols {
                    Some((current_cx + 1, current_cy))
                } else {
                    None
                },
            ];

            for opt in neighbors.into_iter().flatten() {
                let (nx, ny) = opt;
                let neighbor = (ny * macro_cols + nx) as usize;

                if !chunk_contains_water(map, nx, ny, goal_cx, goal_cy, starts) {
                    continue;
                }

                if self.macro_closed_stamp[neighbor] == stamp {
                    continue;
                }

                let cost = 16 * COST_SCALE;
                let tentative_g = current_g.saturating_add(cost);

                if self.macro_gscore_stamp[neighbor] != stamp
                    || tentative_g < self.macro_gscore[neighbor]
                {
                    self.macro_came_from[neighbor] = current as i32;
                    self.macro_gscore[neighbor] = tentative_g;
                    self.macro_gscore_stamp[neighbor] = stamp;
                    let h = manhattan(nx, ny, goal_cx, goal_cy) * 16 * COST_SCALE;
                    self.push_seq = self.push_seq.wrapping_add(1);
                    self.heap.push(AStarNode {
                        f_score: tentative_g.saturating_add(h),
                        insert_seq: self.push_seq,
                        idx: neighbor as u32,
                    });
                }
            }
        }

        if !found {
            return false;
        }

        self.allowed_chunks.fill(false);
        let mut curr = goal_chunk as i32;
        while curr >= 0 {
            let cx = curr as u32 % macro_cols;
            let cy = curr as u32 / macro_cols;
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if nx >= 0 && nx < macro_cols as i32 && ny >= 0 && ny < macro_rows as i32 {
                        let idx = (ny as u32 * macro_cols + nx as u32) as usize;
                        self.allowed_chunks[idx] = true;
                    }
                }
            }
            curr = self.macro_came_from[curr as usize];
        }

        true
    }

    /// Multi-source start (first start defines cross-tie line), single goal. Tile indices: `y * width + x`.
    pub fn find_path(&mut self, map: &GameMap, starts: &[u32], goal: u32) -> Option<Vec<u32>> {
        if starts.is_empty() {
            return None;
        }
        self.ensure_capacity(map);
        let width = map.width;
        let height = map.height;
        let num_nodes = (width * height) as usize;
        let land_mask = 1u8 << LAND_BIT;

        if !self.find_macro_path(map, starts, goal) {
            return None;
        }

        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.closed_stamp.fill(0);
            self.gscore_stamp.fill(0);
            self.stamp = 1;
        }
        let stamp = self.stamp;
        self.push_seq = 0;

        let goal_x = goal % width;
        let goal_y = goal / width;

        let s0 = starts[0];
        let start_x = s0 % width;
        let start_y = s0 / width;
        let dx_goal = goal_x as i32 - start_x as i32;
        let dy_goal = goal_y as i32 - start_y as i32;
        let cross_norm = (dx_goal.unsigned_abs() + dy_goal.unsigned_abs()).max(1);

        self.heap.clear();

        for &s in starts {
            if s as usize >= num_nodes {
                continue;
            }
            let sx = s % width;
            let sy = s / width;
            let idx = s as usize;
            self.gscore[idx] = 0;
            self.gscore_stamp[idx] = stamp;
            self.came_from[idx] = -1;
            let h = self
                .heuristic_weight
                .saturating_mul(BASE_COST)
                .saturating_mul(manhattan(sx, sy, goal_x, goal_y));
            self.push_seq = self.push_seq.wrapping_add(1);
            self.heap.push(AStarNode {
                f_score: h,
                insert_seq: self.push_seq,
                idx: s,
            });
        }

        let mut iterations = self.max_iterations;
        let macro_cols = width.div_ceil(16);

        while let Some(node) = self.heap.pop() {
            if iterations == 0 {
                return None;
            }
            iterations -= 1;

            let current = node.idx as usize;
            if self.closed_stamp[current] == stamp {
                continue;
            }
            self.closed_stamp[current] = stamp;

            if node.idx == goal {
                return Some(self.build_path(goal as usize));
            }

            let current_g = self.gscore[current];
            let current_x = node.idx % width;
            let current_y = node.idx / width;

            let is_odd = !current_y.is_multiple_of(2);
            let deltas = if is_odd {
                [
                    (1, 0),  // East (0)
                    (-1, 0), // West (1)
                    (0, -1), // Northwest (2)
                    (1, -1), // Northeast (3)
                    (0, 1),  // Southwest (4)
                    (1, 1),  // Southeast (5)
                ]
            } else {
                [
                    (1, 0),   // East (0)
                    (-1, 0),  // West (1)
                    (-1, -1), // Northwest (2)
                    (0, -1),  // Northeast (3)
                    (-1, 1),  // Southwest (4)
                    (0, 1),   // Southeast (5)
                ]
            };

            let mut neighbors = [None; 6];
            for (idx, &(dx, dy)) in deltas.iter().enumerate() {
                let nx = current_x as i32 + dx;
                let ny = current_y as i32 + dy;
                if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
                    neighbors[idx] = Some((nx as u32, ny as u32));
                }
            }

            for opt in neighbors.into_iter().flatten() {
                let (nx, ny) = opt;
                let neighbor = (ny * width + nx) as usize;

                let n_cx = nx / 16;
                let n_cy = ny / 16;
                let n_chunk = (n_cy * macro_cols + n_cx) as usize;
                if !self.allowed_chunks[n_chunk] {
                    continue;
                }

                let b = map.terrain[neighbor].as_byte();
                let is_land = (b & land_mask) != 0;
                if neighbor as u32 != goal && is_land {
                    continue;
                }

                if self.closed_stamp[neighbor] == stamp {
                    continue;
                }

                let magnitude = b & MAGNITUDE_MASK;
                let cost = BASE_COST.saturating_add(magnitude_penalty(magnitude));
                let tentative_g = current_g.saturating_add(cost);

                if self.gscore_stamp[neighbor] != stamp || tentative_g < self.gscore[neighbor] {
                    self.came_from[neighbor] = current as i32;
                    self.gscore[neighbor] = tentative_g;
                    self.gscore_stamp[neighbor] = stamp;
                    let h = self
                        .heuristic_weight
                        .saturating_mul(BASE_COST)
                        .saturating_mul(manhattan(nx, ny, goal_x, goal_y));
                    let cross =
                        cross_tie_breaker(nx, ny, goal_x, goal_y, start_x, start_y, cross_norm);
                    let f = tentative_g.saturating_add(h).saturating_add(cross);
                    self.push_seq = self.push_seq.wrapping_add(1);
                    self.heap.push(AStarNode {
                        f_score: f,
                        insert_seq: self.push_seq,
                        idx: neighbor as u32,
                    });
                }
            }
        }

        None
    }

    fn build_path(&self, goal: usize) -> Vec<u32> {
        let mut path = Vec::new();
        let mut current: i32 = goal as i32;
        while current >= 0 {
            let u = current as u32;
            path.push(u);
            current = self.came_from[u as usize];
        }
        path.reverse();
        path
    }
}

#[inline]
fn manhattan(x1: u32, y1: u32, x2: u32, y2: u32) -> u32 {
    let r1 = y1 as i32;
    let q1 = x1 as i32 - (r1 - (r1 & 1)) / 2;
    let r2 = y2 as i32;
    let q2 = x2 as i32 - (r2 - (r2 & 1)) / 2;

    let dq = q1 - q2;
    let dr = r1 - r2;
    ((dq.abs() + (dq + dr).abs() + dr.abs()) / 2) as u32
}

/// Shared scratch buffers for water A* + closest-shore BFS (insert as `Resource` on the Bevy app).
#[derive(Default, Clone)]
pub struct WaterPathfinderScratch {
    pub astar: WaterAStar,
    pub bfs_queue: VecDeque<u32>,
    pub bfs_visited: Vec<u32>,
    pub bfs_stamp: u32,
}
/// Bresenham line rasterization on an offset hex grid. Returns a Vec of tile
/// indices from `src` to `dst` (inclusive). Pure integer math — used for
/// projectile flight paths that ignore terrain.
pub fn bresenham_line(src: u32, dst: u32, width: u32) -> Vec<u32> {
    let sx = (src % width) as i32;
    let sy = (src / width) as i32;
    let ex = (dst % width) as i32;
    let ey = (dst / width) as i32;

    let dx = (ex - sx).abs();
    let dy = (ey - sy).abs();
    let sign_x: i32 = if ex > sx { 1 } else { -1 };
    let sign_y: i32 = if ey > sy { 1 } else { -1 };
    let mut err = dx - dy;

    let mut cx = sx;
    let mut cy = sy;

    let mut path = Vec::with_capacity((dx + dy + 1) as usize);
    loop {
        path.push(cy as u32 * width + cx as u32);
        if cx == ex && cy == ey {
            break;
        }
        let e2 = err * 2;
        if e2 > -dy {
            err -= dy;
            cx += sign_x;
        }
        if e2 < dx {
            err += dx;
            cy += sign_y;
        }
    }
    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{GameMap, MapTile};

    #[test]
    fn test_hierarchical_pathfinding_water_only() {
        let mut map = GameMap::new(32, 32);
        // Top row
        for x in 0..32 {
            let idx = x as usize;
            map.terrain[idx] = MapTile::from_byte(0b00100000);
        }
        // Rightmost column
        for y in 0..32 {
            let idx = (y * 32 + 31) as usize;
            map.terrain[idx] = MapTile::from_byte(0b00100000);
        }
        let mut pathfinder = WaterAStar::new();
        let path = pathfinder.find_path(&map, &[0], 32 * 31 + 31);
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path[0], 0);
        assert_eq!(path[path.len() - 1], 32 * 31 + 31);
    }
}

#[derive(Clone)]
pub struct FlowField {
    pub target: u32,
    pub width: u32,
    pub height: u32,
    pub directions: Vec<u8>, // 0-7: N, NE, E, SE, S, SW, W, NW. 255: Unreachable/Obstacle
}

impl FlowField {
    pub fn new(width: u32, height: u32, target: u32) -> Self {
        Self {
            target,
            width,
            height,
            directions: vec![255; (width * height) as usize],
        }
    }

    pub fn compute_from_target(&mut self, map: &crate::map::GameMap) {
        let n = (self.width * self.height) as usize;
        let mut distances = vec![u32::MAX; n];
        let mut queue = std::collections::VecDeque::new();

        let tx = self.target % self.width;
        let ty = self.target / self.width;

        distances[self.target as usize] = 0;
        self.directions[self.target as usize] = 6; // Reached
        queue.push_back((tx, ty));

        while let Some((cx, cy)) = queue.pop_front() {
            let curr_idx = (cy * self.width + cx) as usize;
            let current_dist = distances[curr_idx];

            let is_odd = (cy % 2) != 0;
            let deltas = if is_odd {
                [
                    (1, 0),  // East (0)
                    (-1, 0), // West (1)
                    (0, -1), // Northwest (2)
                    (1, -1), // Northeast (3)
                    (0, 1),  // Southwest (4)
                    (1, 1),  // Southeast (5)
                ]
            } else {
                [
                    (1, 0),   // East (0)
                    (-1, 0),  // West (1)
                    (-1, -1), // Northwest (2)
                    (0, -1),  // Northeast (3)
                    (-1, 1),  // Southwest (4)
                    (0, 1),   // Southeast (5)
                ]
            };

            for (i, delta) in deltas.iter().enumerate() {
                let nx = cx as i32 + delta.0;
                let ny = cy as i32 + delta.1;

                if nx >= 0 && nx < self.width as i32 && ny >= 0 && ny < self.height as i32 {
                    let n_idx = (ny as u32 * self.width + nx as u32) as usize;

                    let b = map.terrain[n_idx].as_byte();
                    let is_land = (b & (1 << 7)) != 0;
                    if is_land {
                        continue;
                    }

                    if distances[n_idx] > current_dist + 1 {
                        distances[n_idx] = current_dist + 1;
                        let opp = match i {
                            0 => 1,
                            1 => 0,
                            2 => 5,
                            3 => 4,
                            4 => 3,
                            5 => 2,
                            _ => 6,
                        };
                        self.directions[n_idx] = opp as u8;
                        queue.push_back((nx as u32, ny as u32));
                    }
                }
            }
        }
    }
}

#[derive(Default, Clone)]
pub struct FlowFieldCache {
    pub fields: std::collections::HashMap<u32, FlowField>,
    pub access_order: std::collections::VecDeque<u32>,
}

impl FlowFieldCache {
    pub fn get_or_compute(&mut self, target: u32, map: &crate::map::GameMap) -> &FlowField {
        if !self.fields.contains_key(&target) {
            if self.fields.len() >= 8 {
                if let Some(oldest) = self.access_order.pop_front() {
                    self.fields.remove(&oldest);
                }
            }
            let mut field = FlowField::new(map.width, map.height, target);
            field.compute_from_target(map);
            self.fields.insert(target, field);
            self.access_order.push_back(target);
        } else {
            if let Some(pos) = self.access_order.iter().position(|&x| x == target) {
                self.access_order.remove(pos);
            }
            self.access_order.push_back(target);
        }
        self.fields.get(&target).unwrap()
    }
}
