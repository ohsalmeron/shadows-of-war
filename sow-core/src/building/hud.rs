use super::core::Building;
use super::cost::*;
use super::placement::*;
use crate::game::BuildingKind;
use crate::map::GameMap;
/// Max sampled tiles per structure kind when estimating HUD `can_build` (full scan is O(map²)).
pub const PLACEMENT_HUD_MAX_SAMPLES: usize = 4096;

/// Whether `kind` can likely be placed somewhere on owned land (sampled; false negatives possible).
pub fn player_has_valid_placement_sampled(
    map: &GameMap,
    owner_id: u16,
    kind: BuildingKind,
    grid: &super::core::BuildingGrid,
    buildings: &[Building],
    scratch: &mut crate::engine::PlacementScratch,
    max_samples: usize,
) -> bool {
    let w = map.width;
    let area = w.saturating_mul(map.height) as usize;
    if area == 0 {
        return false;
    }
    let n = area.min(max_samples.max(1));
    let divisor = n.max(1);
    for k in 0..n {
        let idx = ((k as u64 * area as u64) / divisor as u64) as u32;
        let (x, y) = idx_xy(idx, w);
        if map.owner_id(x, y) != owner_id {
            continue;
        }
        if resolve_structure_spawn_tile(map, owner_id, kind, idx, grid, buildings, scratch)
            .is_some()
        {
            return true;
        }
    }
    false
}

/// Exhaustive scan (tests / tiny maps only).
pub fn player_has_valid_placement_scan(
    map: &GameMap,
    owner_id: u16,
    kind: BuildingKind,
    grid: &super::core::BuildingGrid,
    buildings: &[Building],
    scratch: &mut crate::engine::PlacementScratch,
) -> bool {
    player_has_valid_placement_sampled(map, owner_id, kind, grid, buildings, scratch, usize::MAX)
}
#[derive(Clone, Copy, Debug)]
pub struct BuildableEntry {
    pub kind: BuildingKind,
    pub cost: f64,
    pub count: u32,
    /// Sampled map check — independent of current gold balance.
    pub placement_feasible: bool,
    pub can_build: bool,
}

/// One HUD row per structure kind.
pub fn compute_buildables_for_player(
    map: &GameMap,
    owner_id: u16,
    player_gold: f64,
    buildings: &[Building],
    grid: &super::core::BuildingGrid,
    scratch: &mut crate::engine::PlacementScratch,
) -> [BuildableEntry; 2] {
    let mut out = [BuildableEntry {
        kind: BuildingKind::City,
        cost: 0.0,
        count: 0,
        placement_feasible: false,
        can_build: false,
    }; 2];
    for (i, &kind) in BuildingKind::ALL.iter().enumerate() {
        let enabled = structure_kind_enabled(kind);
        let cost = structure_build_cost_gold();
        let count = count_kind(buildings, owner_id, kind);
        let placement_feasible = player_has_valid_placement_sampled(
            map,
            owner_id,
            kind,
            grid,
            buildings,
            scratch,
            PLACEMENT_HUD_MAX_SAMPLES,
        );
        let can_build = enabled && player_gold >= cost && cost.is_finite() && placement_feasible;
        out[i] = BuildableEntry {
            kind,
            cost,
            count,
            placement_feasible,
            can_build,
        };
    }
    out
}

/// Refresh costs and afford flags after gold or building-count changes without re-scanning the map.
pub fn patch_buildable_entries_gold_and_counts(
    entries: &mut [BuildableEntry],
    owner_id: u16,
    player_gold: f64,
    buildings: &[Building],
) {
    for e in entries.iter_mut() {
        let kind = e.kind;
        e.cost = structure_build_cost_gold();
        e.count = count_kind(buildings, owner_id, kind);
        let afford = player_gold >= e.cost && e.cost.is_finite();
        let enabled = structure_kind_enabled(kind);
        e.can_build = enabled && afford && e.placement_feasible;
    }
}

