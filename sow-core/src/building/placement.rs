use super::core::{Building, BuildingGrid};
use crate::game::BuildingKind;
use crate::map::{GameMap, TerrainType};

/// Min spacing distance between Cities.
pub const STRUCTURE_MIN_DIST: i32 = 6;
const STRUCTURE_MIN_DIST_SQ: i64 = (STRUCTURE_MIN_DIST as i64) * (STRUCTURE_MIN_DIST as i64);
const STRUCTURE_SEARCH_RADIUS_SQ: i64 = STRUCTURE_MIN_DIST_SQ;

/// Universal minimum spacing between any two buildings.
const BUILDING_MIN_DIST: i32 = 4;
/// Extra spacing from cities (cities are large anchors).
const CITY_MIN_DIST: i32 = STRUCTURE_MIN_DIST;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpacingRule {
    pub target_kind: BuildingKind,
    pub min_distance: i32,
}

impl BuildingKind {
    pub fn spacing_rules(self) -> &'static [SpacingRule] {
        match self {
            BuildingKind::City => &[
                SpacingRule { target_kind: BuildingKind::City,    min_distance: CITY_MIN_DIST },
                SpacingRule { target_kind: BuildingKind::Bunker,  min_distance: CITY_MIN_DIST },
                SpacingRule { target_kind: BuildingKind::Factory, min_distance: CITY_MIN_DIST },
                SpacingRule { target_kind: BuildingKind::Port,    min_distance: CITY_MIN_DIST },
            ],
            BuildingKind::Bunker => &[
                SpacingRule { target_kind: BuildingKind::City,    min_distance: CITY_MIN_DIST },
                SpacingRule { target_kind: BuildingKind::Bunker,  min_distance: BUILDING_MIN_DIST },
                SpacingRule { target_kind: BuildingKind::Factory, min_distance: BUILDING_MIN_DIST },
                SpacingRule { target_kind: BuildingKind::Port,    min_distance: BUILDING_MIN_DIST },
            ],
            BuildingKind::Factory => &[
                SpacingRule { target_kind: BuildingKind::City,    min_distance: CITY_MIN_DIST },
                SpacingRule { target_kind: BuildingKind::Bunker,  min_distance: BUILDING_MIN_DIST },
                SpacingRule { target_kind: BuildingKind::Factory, min_distance: BUILDING_MIN_DIST },
                SpacingRule { target_kind: BuildingKind::Port,    min_distance: BUILDING_MIN_DIST },
            ],
            BuildingKind::Port => &[
                SpacingRule { target_kind: BuildingKind::City,    min_distance: CITY_MIN_DIST },
                SpacingRule { target_kind: BuildingKind::Bunker,  min_distance: BUILDING_MIN_DIST },
                SpacingRule { target_kind: BuildingKind::Factory, min_distance: BUILDING_MIN_DIST },
                SpacingRule { target_kind: BuildingKind::Port,    min_distance: BUILDING_MIN_DIST },
            ],
        }
    }
}

#[inline]
pub fn idx_xy(idx: u32, w: u32) -> (u32, u32) {
    (idx % w, idx / w)
}

#[inline]
pub fn xy_idx(x: u32, y: u32, w: u32) -> u32 {
    y * w + x
}

#[inline]
pub fn euclid_sq(ax: i64, ay: i64, bx: i64, by: i64) -> i64 {
    let dx = ax - bx;
    let dy = ay - by;
    dx * dx + dy * dy
}

#[inline]
pub fn manhattan(ax: i32, ay: i32, bx: i32, by: i32) -> i32 {
    (ax - bx).abs() + (ay - by).abs()
}

/// Land shoreline tile: land terrain with shoreline bit.
pub fn is_shore_land_tile(map: &GameMap, x: u32, y: u32) -> bool {
    let t = map.terrain[map.ref_id(x, y)];
    t.is_land() && t.is_shoreline()
}

fn is_land_structure_tile(map: &GameMap, x: u32, y: u32) -> bool {
    matches!(
        map.terrain_type(x, y),
        TerrainType::Land | TerrainType::Highland | TerrainType::Mountain
    )
}

/// Tiles within Euclidean 12 of `click_idx`, 4-connected, owned by `owner_id`,
/// excluding tiles too close to existing cities if building a City.
pub fn valid_land_structure_indices(
    map: &GameMap,
    owner_id: u16,
    click_idx: u32,
    kind: BuildingKind,
    existing: &BuildingGrid,
    buildings: &[Building],
    scratch: &mut crate::engine::PlacementScratch,
) -> Vec<u32> {
    let w = map.width;
    let (cx, cy) = idx_xy(click_idx, w);
    if !is_land_structure_tile(map, cx, cy) {
        return Vec::new();
    }
    if map.owner_id(cx, cy) != owner_id {
        return Vec::new();
    }

    let cx_i = cx as i64;
    let cy_i = cy as i64;

    let stamp = scratch.stamp.wrapping_add(1);
    scratch.stamp = if stamp == 0 {
        scratch.visited_stamp.fill(0);
        1
    } else {
        stamp
    };
    let stamp = scratch.stamp;

    scratch.queue.clear();
    scratch.visited_stamp[480] = stamp;
    scratch.queue.push(click_idx);

    let mut out: Vec<u32> = Vec::new();

    let mut qi = 0usize;
    while qi < scratch.queue.len() {
        let idx = scratch.queue[qi];
        qi += 1;
        let (x, y) = idx_xy(idx, w);
        let xi = x as i64;
        let yi = y as i64;
        if euclid_sq(xi, yi, cx_i, cy_i) >= STRUCTURE_SEARCH_RADIUS_SQ {
            continue;
        }
        if !is_land_structure_tile(map, x, y) {
            continue;
        }
        if map.owner_id(x, y) != owner_id {
            continue;
        }

        let mut too_close = false;
        for rule in kind.spacing_rules() {
            let min_d = rule.min_distance;
            let min_d_sq = (min_d as i64) * (min_d as i64);

            if rule.target_kind == BuildingKind::City {
                for (bx, by) in existing.iter_in_range(x, y, min_d as u32) {
                    if euclid_sq(xi, yi, bx as i64, by as i64) < min_d_sq {
                        too_close = true;
                        break;
                    }
                }
            } else {
                for b in buildings {
                    if b.kind == rule.target_kind {
                        let bx = b.tile_idx % w;
                        let by = b.tile_idx / w;
                        if euclid_sq(xi, yi, bx as i64, by as i64) < min_d_sq {
                            too_close = true;
                            break;
                        }
                    }
                }
            }

            if too_close {
                break;
            }
        }

        if !too_close && kind == BuildingKind::Port {
            let mut near_water = false;
            for dy in -2..=2 {
                for dx in -2..=2 {
                    let nx = xi + dx;
                    let ny = yi + dy;
                    if nx >= 0 && nx < w as i64 && ny >= 0 && ny < map.height as i64 {
                        let t = map.terrain[map.ref_id(nx as u32, ny as u32)];
                        if !t.is_land() {
                            near_water = true;
                            break;
                        }
                    }
                }
                if near_water {
                    break;
                }
            }
            if !near_water {
                too_close = true;
            }
        }

        if !too_close {
            out.push(idx);
        }

        map.for_each_neighbor(x, y, |nx, ny| {
            let nxi = nx as i64;
            let nyi = ny as i64;
            if euclid_sq(nxi, nyi, cx_i, cy_i) >= STRUCTURE_SEARCH_RADIUS_SQ {
                return;
            }
            if map.owner_id(nx, ny) != owner_id {
                return;
            }
            let lx = (nx as i32 - cx as i32 + 15) as usize;
            let ly = (ny as i32 - cy as i32 + 15) as usize;
            let lidx = ly * 31 + lx;
            if scratch.visited_stamp[lidx] == stamp {
                return;
            }
            scratch.visited_stamp[lidx] = stamp;
            scratch.queue.push(xy_idx(nx, ny, w));
        });
    }

    out.sort_by(|&a, &b| {
        let (ax, ay) = idx_xy(a, w);
        let (bx, by) = idx_xy(b, w);
        let da = euclid_sq(ax as i64, ay as i64, cx_i, cy_i);
        let db = euclid_sq(bx as i64, by as i64, cx_i, cy_i);
        da.cmp(&db).then_with(|| a.cmp(&b))
    });
    out
}

/// Resolve final spawn tile index for `kind` at `click_idx`, or `None` if illegal.
pub fn resolve_structure_spawn_tile(
    map: &GameMap,
    owner_id: u16,
    kind: BuildingKind,
    click_idx: u32,
    existing: &BuildingGrid,
    buildings: &[Building],
    scratch: &mut crate::engine::PlacementScratch,
) -> Option<u32> {
    let w = map.width;
    let h = map.height;
    let max_idx = w.saturating_mul(h);
    if click_idx >= max_idx {
        return None;
    }

    let valid =
        valid_land_structure_indices(map, owner_id, click_idx, kind, existing, buildings, scratch);
    valid.first().copied()
}
