use serde::{Deserialize, Serialize};
use crate::game::BuildingKind;
use crate::map::GameMap;
use crate::engine::SowEngine;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Ordering;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Railroad {
    pub id: u64,
    pub path: Vec<u32>,
    pub owner_id: u16,
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct State {
    cost: u32,
    position: u32,
}

impl Ord for State {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.cmp(&self.cost) // min-heap
    }
}

impl PartialOrd for State {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

pub fn is_shoreline(map: &GameMap, idx: u32) -> bool {
    let w = map.width;
    if w == 0 {
        return false;
    }
    let (x, y) = (idx % w, idx / w);
    let my_land = map.terrain[idx as usize].is_land();
    let mut is_shore = false;
    // Check cardinal neighbors
    let neighbors = [
        (x.wrapping_sub(1), y),
        (x + 1, y),
        (x, y.wrapping_sub(1)),
        (x, y + 1),
    ];
    for &(nx, ny) in &neighbors {
        if nx < map.width && ny < map.height {
            let n_idx = ny * map.width + nx;
            if map.terrain[n_idx as usize].is_land() != my_land {
                is_shore = true;
                break;
            }
        }
    }
    is_shore
}

pub fn is_traversable(map: &GameMap, from_idx: u32, to_idx: u32) -> bool {
    let to_tile = map.terrain[to_idx as usize];
    if to_tile.is_land() {
        true
    } else {
        is_shoreline(map, from_idx) || is_shoreline(map, to_idx)
    }
}

pub fn find_rail_path(map: &GameMap, start: u32, goal: u32) -> Option<Vec<u32>> {
    let w = map.width;
    if w == 0 {
        return None;
    }
    let mut dist = HashMap::new();
    let mut came_from = HashMap::new();
    let mut heap = BinaryHeap::new();

    dist.insert(start, 0);
    heap.push(State { cost: 0, position: start });

    let (gx, gy) = (goal % w, goal / w);

    while let Some(State { cost: _, position }) = heap.pop() {
        if position == goal {
            let mut path = Vec::new();
            let mut curr = goal;
            while curr != start {
                path.push(curr);
                curr = *came_from.get(&curr)?;
            }
            path.push(start);
            path.reverse();
            return Some(path);
        }

        let (cx, cy) = (position % w, position / w);
        let current_cost = dist[&position];

        let neighbors = [
            (cx.wrapping_sub(1), cy),
            (cx + 1, cy),
            (cx, cy.wrapping_sub(1)),
            (cx, cy + 1),
        ];

        for &(nx, ny) in &neighbors {
            if nx < map.width && ny < map.height {
                let next = ny * w + nx;
                if !is_traversable(map, position, next) {
                    continue;
                }

                let mut penalty = 1;
                if let Some(&prev) = came_from.get(&position) {
                    let (px, py) = (prev % w, prev / w);
                    let dx1 = cx as i32 - px as i32;
                    let dy1 = cy as i32 - py as i32;
                    let dx2 = nx as i32 - cx as i32;
                    let dy2 = ny as i32 - cy as i32;
                    if dx1 != dx2 || dy1 != dy2 {
                        penalty += 3;
                    }
                }

                let next_cost = current_cost + penalty;
                let current_next_cost = dist.get(&next).copied().unwrap_or(u32::MAX);

                if next_cost < current_next_cost {
                    dist.insert(next, next_cost);
                    came_from.insert(next, position);

                    let h = (nx as i32 - gx as i32).abs() + (ny as i32 - gy as i32).abs();
                    heap.push(State {
                        cost: next_cost + h as u32,
                        position: next,
                    });
                }
            }
        }
    }

    None
}

pub fn update_railroads(engine: &mut SowEngine) {
    let mut stations = Vec::new();
    for b in &engine.buildings {
        if !b.under_construction && (b.kind == BuildingKind::City || b.kind == BuildingKind::Factory || b.kind == BuildingKind::Port) {
            stations.push(*b);
        }
    }

    let mut new_railroads = Vec::new();
    let mut rail_id = 0u64;

    for i in 0..stations.len() {
        for j in (i + 1)..stations.len() {
            let s1 = &stations[i];
            let s2 = &stations[j];

            if s1.owner_id != s2.owner_id {
                continue;
            }

            let w = engine.state.map.width;
            if w == 0 {
                continue;
            }
            let (x1, y1) = (s1.tile_idx % w, s1.tile_idx / w);
            let (x2, y2) = (s2.tile_idx % w, s2.tile_idx / w);
            let dist = (x1 as i32 - x2 as i32).abs() + (y1 as i32 - y2 as i32).abs();
            if dist > 18 {
                continue;
            }

            if let Some(path) = find_rail_path(&engine.state.map, s1.tile_idx, s2.tile_idx) {
                if path.len() <= 25 {
                    new_railroads.push(Railroad {
                        id: rail_id,
                        path,
                        owner_id: s1.owner_id,
                    });
                    rail_id += 1;
                }
            }
        }
    }

    engine.state.railroads = new_railroads;
}
