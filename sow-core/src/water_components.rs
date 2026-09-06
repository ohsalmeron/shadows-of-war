//! Connected-component labeling of the water plane. Two shoreline tiles share a `component`
//! iff they touch the same connected body of water — ocean, lake, or river.
//!
//! Computed once at map-load (terrain is static). Each client does this locally
//! off the deterministic terrain bytes, so no network payload required.

use std::collections::VecDeque;

use crate::map::{CARDINAL_NEIGHBOR_DELTAS, GameMap};

#[inline]
fn for_each_cardinal_neighbor(
    width: u32,
    height: u32,
    x: u32,
    y: u32,
    mut visit: impl FnMut(u32, u32),
) {
    for &(dx, dy) in &CARDINAL_NEIGHBOR_DELTAS {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
            visit(nx as u32, ny as u32);
        }
    }
}

/// Component labels indexed by linear tile index (`y * width + x`).
///
/// - Water tile → ID of its flood-fill group (`>= 1`)
/// - Land + shoreline tile → ID of the smallest-ID water component any 4-neighbor
///   water tile belongs to (deterministic tie-break). This lets callers ask
///   "does my shore and the target's shore touch the same water body?" in O(1).
/// - Any other tile → `0` (no water component, not launchable, not landable).
#[derive(Debug, Clone, Default)]
pub struct WaterComponents {
    pub components: Vec<u32>,
    pub count: u32,
}

impl WaterComponents {
    /// One-shot flood-fill (4-connectivity) over water tiles, followed by shore
    /// inheritance. `O(width * height)` time, `O(width * height)` memory.
    pub fn compute<F: FnMut(f32)>(map: &GameMap, mut on_progress: F) -> Self {
        let n = (map.width as usize) * (map.height as usize);
        if n == 0 {
            return Self::default();
        }
        let w = map.width;
        let mut components = vec![0u32; n];
        let mut count: u32 = 0;
        let mut queue: VecDeque<u32> = VecDeque::new();

        // ── 1. Label water bodies ────────────────────────────────────────
        let step1 = (n / 100).max(1);
        for start in 0..n {
            if start % step1 == 0 {
                on_progress((start as f32 / n as f32) * 0.8);
            }
            if map.terrain[start].is_land() || components[start] != 0 {
                continue;
            }
            count += 1;
            let id = count;
            components[start] = id;
            queue.clear();
            queue.push_back(start as u32);
            while let Some(t) = queue.pop_front() {
                let x = t % w;
                let y = t / w;
                for_each_cardinal_neighbor(w, map.height, x, y, |nx, ny| {
                    let ni = (ny * w + nx) as usize;
                    if !map.terrain[ni].is_land() && components[ni] == 0 {
                        components[ni] = id;
                        queue.push_back(ni as u32);
                    }
                });
            }
        }

        // ── 2. Shore tiles inherit the smallest adjacent water component ─
        //    (min-ID tie-break keeps this 100% deterministic across clients)
        let step2 = (n / 100).max(1);
        for idx in 0..n {
            if idx % step2 == 0 {
                on_progress(0.8 + (idx as f32 / n as f32) * 0.2);
            }
            let tile = map.terrain[idx];
            if !tile.is_land() || !tile.is_shoreline() {
                continue;
            }
            let x = (idx as u32) % w;
            let y = (idx as u32) / w;
            let mut best: u32 = 0;
            for_each_cardinal_neighbor(w, map.height, x, y, |nx, ny| {
                let ni = (ny * w + nx) as usize;
                if !map.terrain[ni].is_land() {
                    let c = components[ni];
                    if c > 0 && (best == 0 || c < best) {
                        best = c;
                    }
                }
            });
            components[idx] = best;
        }

        Self { components, count }
    }

    #[inline]
    pub fn component_of(&self, idx: u32) -> u32 {
        self.components.get(idx as usize).copied().unwrap_or(0)
    }
}

/// Incremental form of [`WaterComponents::compute`]. It performs bounded work
/// per [`Self::step`] call while preserving the exact scan/BFS order of the
/// one-shot implementation. This is used by the browser client to yield to the
/// renderer between chunks; native callers can continue using `compute`.
pub struct WaterComponentsBuilder {
    width: u32,
    height: u32,
    terrain: Vec<crate::map::MapTile>,
    components: Vec<u32>,
    count: u32,
    queue: VecDeque<u32>,
    scan_index: usize,
    shore_index: usize,
    phase: BuildPhase,
    result_taken: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BuildPhase {
    LabelWater,
    InheritShore,
    Complete,
}

impl WaterComponentsBuilder {
    pub fn new(map: &GameMap) -> Self {
        let n = (map.width as usize) * (map.height as usize);
        Self {
            width: map.width,
            height: map.height,
            terrain: map.terrain.clone(),
            components: vec![0; n],
            count: 0,
            queue: VecDeque::new(),
            scan_index: 0,
            shore_index: 0,
            phase: if n == 0 {
                BuildPhase::Complete
            } else {
                BuildPhase::LabelWater
            },
            result_taken: false,
        }
    }

    /// Run at most `budget` tile/queue operations and return `(progress,
    /// result)`. `result` is `Some` exactly once, when all components are ready.
    pub fn step(&mut self, budget: usize) -> (f32, Option<WaterComponents>) {
        if self.phase == BuildPhase::Complete {
            if self.result_taken {
                return (1.0, None);
            }
            self.result_taken = true;
            let components = std::mem::take(&mut self.components);
            return (
                1.0,
                Some(WaterComponents {
                    components,
                    count: self.count,
                }),
            );
        }

        let budget = budget.max(1);
        let n = self.terrain.len();
        let mut work = 0;

        while work < budget {
            match self.phase {
                BuildPhase::LabelWater => {
                    if let Some(t) = self.queue.pop_front() {
                        self.visit_water_tile(t);
                        work += 1;
                    } else if self.scan_index < n {
                        let start = self.scan_index;
                        self.scan_index += 1;
                        work += 1;
                        if !self.terrain[start].is_land() && self.components[start] == 0 {
                            self.count += 1;
                            self.components[start] = self.count;
                            self.queue.push_back(start as u32);
                        }
                    } else {
                        self.phase = BuildPhase::InheritShore;
                        self.shore_index = 0;
                    }
                }
                BuildPhase::InheritShore => {
                    if self.shore_index < n {
                        let idx = self.shore_index;
                        self.shore_index += 1;
                        work += 1;
                        self.inherit_shore(idx);
                    } else {
                        self.phase = BuildPhase::Complete;
                        break;
                    }
                }
                BuildPhase::Complete => break,
            }
        }

        if self.phase == BuildPhase::Complete {
            self.result_taken = true;
            let components = std::mem::take(&mut self.components);
            return (
                1.0,
                Some(WaterComponents {
                    components,
                    count: self.count,
                }),
            );
        }

        let progress = match self.phase {
            BuildPhase::LabelWater => (self.scan_index as f32 / n as f32) * 0.8,
            BuildPhase::InheritShore => 0.8 + (self.shore_index as f32 / n as f32) * 0.2,
            BuildPhase::Complete => 1.0,
        };
        (progress.min(0.999), None)
    }

    fn visit_water_tile(&mut self, t: u32) {
        let x = t % self.width;
        let y = t / self.width;
        let width = self.width;
        let height = self.height;
        let terrain = &self.terrain;
        let components = &mut self.components;
        let queue = &mut self.queue;
        let component_id = self.count;
        for_each_cardinal_neighbor(width, height, x, y, |nx, ny| {
            let ni = (ny * width + nx) as usize;
            if !terrain[ni].is_land() && components[ni] == 0 {
                components[ni] = component_id;
                queue.push_back(ni as u32);
            }
        });
    }

    fn inherit_shore(&mut self, idx: usize) {
        let tile = self.terrain[idx];
        if !tile.is_land() || !tile.is_shoreline() {
            return;
        }
        let x = (idx as u32) % self.width;
        let y = (idx as u32) / self.width;
        let mut best = 0;
        let width = self.width;
        let height = self.height;
        let terrain = &self.terrain;
        let components = &self.components;
        for_each_cardinal_neighbor(width, height, x, y, |nx, ny| {
            let ni = (ny * width + nx) as usize;
            if !terrain[ni].is_land() {
                let component = components[ni];
                if component > 0 && (best == 0 || component < best) {
                    best = component;
                }
            }
        });
        self.components[idx] = best;
    }
}

#[cfg(test)]
mod incremental_tests {
    use super::{WaterComponents, WaterComponentsBuilder};
    use crate::map::{GameMap, MapTile};

    fn sample_map() -> GameMap {
        let mut map = GameMap::new(11, 9);
        for (idx, tile) in map.terrain.iter_mut().enumerate() {
            if idx % 5 == 0 || idx / 11 == 4 {
                *tile = MapTile::from_byte(0);
            }
        }
        map
    }

    #[test]
    fn incremental_matches_one_shot_for_multiple_budgets() {
        let map = sample_map();
        let expected = WaterComponents::compute(&map, |_| {});
        for budget in [1, 2, 7, 31, 4096] {
            let mut builder = WaterComponentsBuilder::new(&map);
            let actual = loop {
                let (_, result) = builder.step(budget);
                if let Some(result) = result {
                    break result;
                }
            };
            assert_eq!(actual.components, expected.components, "budget={budget}");
            assert_eq!(actual.count, expected.count, "budget={budget}");
        }
    }

    #[test]
    fn diagonal_water_tiles_are_separate_bodies() {
        let mut map = GameMap::new(3, 3);
        map.terrain[0] = MapTile::from_byte(0);
        map.terrain[4] = MapTile::from_byte(0);

        let components = WaterComponents::compute(&map, |_| {});

        assert_eq!(components.count, 2);
        assert_ne!(components.component_of(0), components.component_of(4));
    }
}
