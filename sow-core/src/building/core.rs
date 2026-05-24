use super::placement::{idx_xy, manhattan};
use crate::config;
use crate::game::BuildingKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum ModuleKind {
    Port,
    Foundry,
    Armory,
    Intel,
    Arsenal,
    Shield,
}

impl ModuleKind {
    pub const ALL: [ModuleKind; 6] = [
        ModuleKind::Port,
        ModuleKind::Foundry,
        ModuleKind::Armory,
        ModuleKind::Intel,
        ModuleKind::Arsenal,
        ModuleKind::Shield,
    ];

    pub fn name(self) -> &'static str {
        match self {
            ModuleKind::Port => "Port",
            ModuleKind::Foundry => "Foundry",
            ModuleKind::Armory => "Armory",
            ModuleKind::Intel => "Intel",
            ModuleKind::Arsenal => "Arsenal",
            ModuleKind::Shield => "Shield",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct CityModules {
    pub port: u8,
    pub foundry: u8,
    pub armory: u8,
    pub intel: u8,
    pub arsenal: u8,
    pub shield: u8,
}

impl CityModules {
    pub fn get_level(&self, kind: ModuleKind) -> u8 {
        match kind {
            ModuleKind::Port => self.port,
            ModuleKind::Foundry => self.foundry,
            ModuleKind::Armory => self.armory,
            ModuleKind::Intel => self.intel,
            ModuleKind::Arsenal => self.arsenal,
            ModuleKind::Shield => self.shield,
        }
    }

    pub fn set_level(&mut self, kind: ModuleKind, level: u8) {
        match kind {
            ModuleKind::Port => self.port = level,
            ModuleKind::Foundry => self.foundry = level,
            ModuleKind::Armory => self.armory = level,
            ModuleKind::Intel => self.intel = level,
            ModuleKind::Arsenal => self.arsenal = level,
            ModuleKind::Shield => self.shield = level,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Building {
    pub id: u64,
    pub owner_id: u16,
    /// Linear index `y * width + x`.
    pub tile_idx: u32,
    pub kind: BuildingKind,
    pub level: u8,
    pub under_construction: bool,
    pub ticks_until_complete: u32,
    pub modules: CityModules,
}

/// Per-player totals for income / fleet gates (only **ready** structures count).
#[derive(Clone, Copy, Debug, Default)]
pub struct BuildingAggregate {
    pub city_levels: u32,
    pub bunker_levels: u32,
    pub port_levels: u32,
    pub foundry_levels: u32,
    pub armory_levels: u32,
    pub intel_levels: u32,
    pub arsenal_levels: u32,
    pub shield_levels: u32,
    pub has_completed_port: bool,
    /// Ready cities only (for bot `city_equivalent` base).
    pub ready_city_count: u32,
    /// Total instances per kind, **including** under construction (for bot build quotas).
    pub count_city: u32,
    pub count_bunker: u32,
}

impl BuildingAggregate {
    #[inline]
    pub fn total_structures_of_kind(self, kind: BuildingKind) -> u32 {
        match kind {
            BuildingKind::City => self.count_city,
            BuildingKind::Bunker => self.count_bunker,
        }
    }
}

/// `out[v]` is aggregate for owner id `v` (resize to `max_player_id + 1`).
pub fn aggregate_buildings_per_player(
    buildings: impl Iterator<Item = Building>,
    max_player_id: usize,
) -> Vec<BuildingAggregate> {
    let mut out = vec![BuildingAggregate::default(); max_player_id.saturating_add(1)];
    for b in buildings {
        let i = b.owner_id as usize;
        if i >= out.len() {
            continue;
        }
        let a = &mut out[i];
        match b.kind {
            BuildingKind::City => a.count_city += 1,
            BuildingKind::Bunker => a.count_bunker += 1,
        }
        if b.under_construction {
            continue;
        }
        let a = &mut out[i];
        match b.kind {
            BuildingKind::City => {
                a.city_levels += b.level as u32;
                a.ready_city_count += 1;
                a.port_levels += b.modules.port as u32;
                if b.modules.port > 0 {
                    a.has_completed_port = true;
                }
                a.foundry_levels += b.modules.foundry as u32;
                a.armory_levels += b.modules.armory as u32;
                a.intel_levels += b.modules.intel as u32;
                a.arsenal_levels += b.modules.arsenal as u32;
                a.shield_levels += b.modules.shield as u32;
            }
            BuildingKind::Bunker => {
                a.bunker_levels += b.level as u32;
            }
        }
    }
    out
}

/// Extra frontier priority when conquering a tile near an enemy DefensePost (defender `target_owner`).
#[inline]
pub fn defense_post_priority_bonus(
    buildings: &[Building],
    tile_x: u32,
    tile_y: u32,
    map_width: u32,
) -> i64 {
    let mut bonus: i64 = 0;
    for b in buildings {
        let (bx, by) = idx_xy(b.tile_idx, map_width);
        let d = manhattan(tile_x as i32, tile_y as i32, bx as i32, by as i32);
        if d <= config::DEFENSE_POST_RANGE {
            bonus += b.level as i64 * config::DEFENSE_POST_PRIORITY_PER_LEVEL;
        }
    }
    bonus
}

/// A spatial grid specifically designed to optimize `defense_post_priority_bonus` from O(N) to O(1).
/// This should be cached in a `Local` within the combat execution system to avoid allocations.
#[derive(Default, Clone)]
pub struct DefenseGrid {
    pub cells: Vec<Vec<Building>>,
    pub grid_w: u32,
    pub grid_h: u32,
    pub cell_size: u32,
}

impl DefenseGrid {
    /// Rebuild the grid with the specified player's defense posts.
    /// This is allocation-free after the first few calls because the `cells` array
    /// simply clears its internal `Vec`s without dropping capacity.
    pub fn rebuild(
        &mut self,
        buildings: &[Building],
        map_width: u32,
        map_height: u32,
        cell_size: u32,
    ) {
        let grid_w = map_width.div_ceil(cell_size);
        let grid_h = map_height.div_ceil(cell_size);

        self.grid_w = grid_w;
        self.grid_h = grid_h;
        self.cell_size = cell_size;

        let num_cells = (grid_w * grid_h) as usize;
        if self.cells.len() < num_cells {
            self.cells.resize(num_cells, Vec::new());
        }
        for cell in self.cells.iter_mut() {
            cell.clear();
        }

        for &b in buildings {
            if b.kind == BuildingKind::Bunker && !b.under_construction {
                let bx = b.tile_idx % map_width;
                let by = b.tile_idx / map_width;
                let cx = bx / cell_size;
                let cy = by / cell_size;
                if cx < grid_w && cy < grid_h {
                    self.cells[(cy * grid_w + cx) as usize].push(b);
                }
            }
        }
    }

    /// Calculate priority bonus querying only cells within `DEFENSE_POST_RANGE`.
    #[inline]
    pub fn priority_bonus(
        &self,
        tile_x: u32,
        tile_y: u32,
        map_width: u32,
        target_owner: u16,
    ) -> i64 {
        let mut bonus: i64 = 0;
        let range = config::DEFENSE_POST_RANGE as u32;

        let cx_min = tile_x.saturating_sub(range) / self.cell_size;
        let cx_max = (tile_x + range) / self.cell_size;
        let cy_min = tile_y.saturating_sub(range) / self.cell_size;
        let cy_max = (tile_y + range) / self.cell_size;

        let cx_max = cx_max.min(self.grid_w.saturating_sub(1));
        let cy_max = cy_max.min(self.grid_h.saturating_sub(1));

        for cy in cy_min..=cy_max {
            for cx in cx_min..=cx_max {
                let idx = (cy * self.grid_w + cx) as usize;
                for b in &self.cells[idx] {
                    if b.owner_id != target_owner {
                        continue;
                    }
                    let bx = b.tile_idx % map_width;
                    let by = b.tile_idx / map_width;
                    let d = manhattan(tile_x as i32, tile_y as i32, bx as i32, by as i32);
                    if d <= config::DEFENSE_POST_RANGE {
                        bonus += b.level as i64 * config::DEFENSE_POST_PRIORITY_PER_LEVEL;
                    }
                }
            }
        }
        bonus
    }
}

/// Cell side length for spatial indexing of structure centers. Must match `STRUCTURE_MIN_DIST` in `placement.rs`.
pub const BUILDING_GRID_CELL_SIZE: u32 = 15;

/// Spatial grid of structure tile coordinates `(x, y)` for O(local) minimum-distance checks during placement.
#[derive(Clone)]
pub struct BuildingGrid {
    pub cells: Vec<Vec<(u32, u32)>>,
    pub grid_w: u32,
    pub grid_h: u32,
    pub cell_size: u32,
    /// When false and `grid_w > 0`, [`DarkRiftEngine::refresh_building_grid`] may skip work.
    pub dirty: bool,
}

impl Default for BuildingGrid {
    fn default() -> Self {
        Self {
            cells: Vec::new(),
            grid_w: 0,
            grid_h: 0,
            cell_size: BUILDING_GRID_CELL_SIZE,
            dirty: true,
        }
    }
}

impl BuildingGrid {
    /// Grid with no buildings: dimensions and cells initialized, [`Self::dirty`] false.
    pub fn rebuild_empty(map_w: u32, map_h: u32) -> Self {
        let mut g = Self::default();
        g.rebuild_from_pairs(map_w, map_h, &[]);
        g
    }

    #[inline]
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Fill grid from all buildings (including under construction), matching legacy `existing_structure_positions_with_width`.
    pub fn rebuild<'a>(
        &mut self,
        buildings: impl Iterator<Item = &'a Building>,
        map_w: u32,
        map_h: u32,
    ) {
        let cell_size = BUILDING_GRID_CELL_SIZE;
        let grid_w = map_w.div_ceil(cell_size);
        let grid_h = map_h.div_ceil(cell_size);

        self.grid_w = grid_w;
        self.grid_h = grid_h;
        self.cell_size = cell_size;

        let num_cells = (grid_w * grid_h) as usize;
        if self.cells.len() < num_cells {
            self.cells.resize(num_cells, Vec::new());
        }
        for cell in self.cells.iter_mut() {
            cell.clear();
        }

        for b in buildings {
            if b.kind != BuildingKind::City {
                continue;
            }
            let bx = b.tile_idx % map_w;
            let by = b.tile_idx / map_w;
            let cx = bx / cell_size;
            let cy = by / cell_size;
            if cx < grid_w && cy < grid_h {
                self.cells[(cy * grid_w + cx) as usize].push((bx, by));
            }
        }
        self.dirty = false;
    }

    /// Rebuild from raw tile coordinates (tests / tooling; no `Building` structs).
    pub fn rebuild_from_pairs(&mut self, map_w: u32, map_h: u32, pairs: &[(u32, u32)]) {
        let cell_size = BUILDING_GRID_CELL_SIZE;
        let grid_w = map_w.div_ceil(cell_size);
        let grid_h = map_h.div_ceil(cell_size);
        self.grid_w = grid_w;
        self.grid_h = grid_h;
        self.cell_size = cell_size;
        let num_cells = (grid_w * grid_h) as usize;
        if self.cells.len() < num_cells {
            self.cells.resize(num_cells, Vec::new());
        }
        for cell in self.cells.iter_mut() {
            cell.clear();
        }
        for &(bx, by) in pairs {
            let cx = bx / cell_size;
            let cy = by / cell_size;
            if cx < grid_w && cy < grid_h {
                self.cells[(cy * grid_w + cx) as usize].push((bx, by));
            }
        }
        self.dirty = false;
    }

    /// All stored structure positions in cells overlapping the Euclidean disk of radius `range` around `(tile_x, tile_y)`.
    pub fn iter_in_range(
        &self,
        tile_x: u32,
        tile_y: u32,
        range: u32,
    ) -> impl Iterator<Item = (u32, u32)> + '_ {
        let cx_min = tile_x.saturating_sub(range) / self.cell_size;
        let cx_max = (tile_x + range) / self.cell_size;
        let cy_min = tile_y.saturating_sub(range) / self.cell_size;
        let cy_max = (tile_y + range) / self.cell_size;
        let cx_max = cx_max.min(self.grid_w.saturating_sub(1));
        let cy_max = cy_max.min(self.grid_h.saturating_sub(1));

        (cy_min..=cy_max).flat_map(move |cy| {
            (cx_min..=cx_max).flat_map(move |cx| {
                let idx = (cy * self.grid_w + cx) as usize;
                self.cells[idx].iter().copied()
            })
        })
    }
}
