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

fn get_hop_distance(adj: &[Vec<usize>], start: usize, goal: usize, max_hops: usize) -> Option<usize> {
    if start == goal {
        return Some(0);
    }
    let mut visited = vec![false; adj.len()];
    let mut queue = std::collections::VecDeque::new();
    visited[start] = true;
    queue.push_back((start, 0));

    while let Some((curr, dist)) = queue.pop_front() {
        if curr == goal {
            return Some(dist);
        }
        if dist >= max_hops {
            continue;
        }
        for &neighbor in &adj[curr] {
            if !visited[neighbor] {
                visited[neighbor] = true;
                queue.push_back((neighbor, dist + 1));
            }
        }
    }
    None
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct IncrementalRail {
    pub path: Vec<u32>,
    pub owner_id: u16,
    pub from_idx: usize,
    pub to_idx: usize,
}

pub fn update_railroads(engine: &mut SowEngine) {
    if engine.railroads_dirty {
        let mut stations = Vec::new();
        for b in &engine.buildings {
            if !b.under_construction && (b.kind == BuildingKind::City || b.kind == BuildingKind::Factory || b.kind == BuildingKind::Port) {
                stations.push(*b);
            }
        }
        // Sort by id to process deterministically in creation order
        stations.sort_by_key(|b| b.id);
        
        engine.railroad_calc = Some((0, Vec::new(), stations));
        engine.railroads_dirty = false;
    }

    let w = engine.state.map.width;
    if w == 0 {
        return;
    }

    if let Some((s_idx, mut rails, stations)) = engine.railroad_calc.take() {
        if s_idx < stations.len() {
            let s_tile = stations[s_idx].tile_idx;
            let (sx, sy) = (s_tile % w, s_tile / w);
            let mut split_occurred = false;
            let mut new_rails = Vec::new();
            let mut i = 0;

            while i < rails.len() {
                let mut min_dist_sq = i32::MAX;
                let mut closest_tile_idx = 0;

                for (tile_idx, &path_tile) in rails[i].path.iter().enumerate() {
                    let (tx, ty) = (path_tile % w, path_tile / w);
                    let dx = sx as i32 - tx as i32;
                    let dy = sy as i32 - ty as i32;
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq < min_dist_sq {
                        min_dist_sq = dist_sq;
                        closest_tile_idx = tile_idx;
                    }
                }

                let closest_tile = rails[i].path[closest_tile_idx];
                let (tx, ty) = (closest_tile % w, closest_tile / w);
                let dx = (sx as i32 - tx as i32).abs();
                let dy = (sy as i32 - ty as i32).abs();

                let is_near = dx <= 3 && dy <= 3;
                let is_endpoint = closest_tile_idx == 0 || closest_tile_idx == rails[i].path.len() - 1;

                if is_near && !is_endpoint {
                    let original_rail = rails.remove(i);

                    let mut path1 = Vec::new();
                    let mut path2 = Vec::new();
                    let mut pathfound = false;

                    if let Some(to_s_path) = find_rail_path(&engine.state.map, closest_tile, s_tile) {
                        if to_s_path.len() <= 480 {
                            path1 = original_rail.path[0..closest_tile_idx].to_vec();
                            path1.extend(to_s_path.clone());

                            let mut from_s_path = to_s_path;
                            from_s_path.reverse();
                            path2 = from_s_path;
                            path2.extend_from_slice(&original_rail.path[closest_tile_idx + 1..]);
                            pathfound = true;
                        }
                    }

                    if !pathfound {
                        path1 = original_rail.path[0..=closest_tile_idx].to_vec();
                        if path1.last() != Some(&s_tile) {
                            path1.push(s_tile);
                        }

                        path2 = original_rail.path[closest_tile_idx..].to_vec();
                        if path2.first() != Some(&s_tile) {
                            path2.insert(0, s_tile);
                        }
                    }

                    new_rails.push(IncrementalRail {
                        path: path1,
                        owner_id: original_rail.owner_id,
                        from_idx: original_rail.from_idx,
                        to_idx: s_idx,
                    });

                    new_rails.push(IncrementalRail {
                        path: path2,
                        owner_id: original_rail.owner_id,
                        from_idx: s_idx,
                        to_idx: original_rail.to_idx,
                    });

                    split_occurred = true;
                } else {
                    i += 1;
                }
            }
            rails.extend(new_rails);

            if !split_occurred {
                let mut neighbors = Vec::new();
                for other_idx in 0..s_idx {
                    let other = &stations[other_idx];

                    let id1 = stations[s_idx].owner_id;
                    let id2 = other.owner_id;
                    let friendly = if id1 == id2 {
                        true
                    } else if id1 == 0 || id2 == 0 {
                        true
                    } else {
                        let mut is_ally = false;
                        if let Some(p1) = engine.state.players.iter().find(|p| p.id == id1) {
                            if p1.alliances.contains(&id2) {
                                is_ally = true;
                            }
                        }
                        if !is_ally {
                            if let Some(p2) = engine.state.players.iter().find(|p| p.id == id2) {
                                if p2.alliances.contains(&id1) {
                                    is_ally = true;
                                }
                            }
                        }
                        is_ally
                    };

                    if !friendly {
                        continue;
                    }

                    let (x2, y2) = (other.tile_idx % w, other.tile_idx / w);
                    let dx = sx as i32 - x2 as i32;
                    let dy = sy as i32 - y2 as i32;
                    let dist_sq = dx * dx + dy * dy;

                    if dist_sq >= 225 && dist_sq <= 10000 {
                        neighbors.push((dist_sq, other_idx));
                    }
                }

                neighbors.sort_by_key(|n| n.0);

                for (_, other_idx) in neighbors {
                    let mut adj = vec![Vec::new(); s_idx + 1];
                    for rail in &rails {
                        if rail.from_idx <= s_idx && rail.to_idx <= s_idx {
                            adj[rail.from_idx].push(rail.to_idx);
                            adj[rail.to_idx].push(rail.from_idx);
                        }
                    }

                    if let Some(hops) = get_hop_distance(&adj, s_idx, other_idx, 4) {
                        if hops <= 4 {
                            continue;
                        }
                    }

                    let other = &stations[other_idx];
                    if let Some(path) = find_rail_path(&engine.state.map, s_tile, other.tile_idx) {
                        if path.len() <= 480 {
                            let owner_id = if stations[s_idx].owner_id != 0 {
                                stations[s_idx].owner_id
                            } else {
                                other.owner_id
                            };
                            rails.push(IncrementalRail {
                                path,
                                owner_id,
                                from_idx: s_idx,
                                to_idx: other_idx,
                            });
                        }
                    }
                }
            }

            // Save state back to continue on the next tick
            engine.railroad_calc = Some((s_idx + 1, rails, stations));
        } else {
            // Completed all steps! Re-compile final railroads and apply
            let mut final_railroads = Vec::new();
            for (idx, rail) in rails.into_iter().enumerate() {
                final_railroads.push(Railroad {
                    id: idx as u64,
                    path: rail.path,
                    owner_id: rail.owner_id,
                });
            }
            engine.state.railroads = final_railroads;
            engine.railroad_calc = None;
        }
    }
}
