/// Closest same-kind building within stack range of the click (matches server logic).
pub fn find_stack_target_tile(
    kind: sow_core::game::BuildingKind,
    click_x: i32,
    click_y: i32,
    map_w: u32,
    my_id: u16,
    buildings: &[sow_core::protocol::BuildingSnapshot],
) -> Option<u32> {
    let stack_dist = sow_core::building::placement::STRUCTURE_MIN_DIST;
    let mut best: Option<(i32, u64, u32)> = None;
    for b in buildings {
        if b.owner_id != my_id || b.kind != kind {
            continue;
        }
        let bx = (b.tile_idx % map_w) as i32;
        let by = (b.tile_idx / map_w) as i32;
        let d = (click_x - bx).abs() + (click_y - by).abs();
        if d > stack_dist {
            continue;
        }
        let cand = (d, b.id, b.tile_idx);
        match best {
            None => best = Some(cand),
            Some((bd, bid, _)) => {
                if d < bd || (d == bd && b.id < bid) {
                    best = Some(cand);
                }
            }
        }
    }
    best.map(|(_, _, tile)| tile)
}

pub struct PlacementQuery<'a> {
    pub kind: sow_core::game::BuildingKind,
    pub click_x: i32,
    pub click_y: i32,
    pub map_w: u32,
    pub map_h: u32,
    pub owners: &'a [u16],
    pub terrain: &'a [u8],
    pub my_id: u16,
    pub buildings: &'a [sow_core::protocol::BuildingSnapshot],
}

pub fn resolve_build_target_tile(query: &PlacementQuery) -> Result<u32, &'static str> {
    if let Some(tile) = find_stack_target_tile(
        query.kind,
        query.click_x,
        query.click_y,
        query.map_w,
        query.my_id,
        query.buildings,
    ) {
        return Ok(tile);
    }
    resolve_building_placement_tile(query)
}

pub fn resolve_building_placement_tile(query: &PlacementQuery) -> Result<u32, &'static str> {
    let kind = query.kind;
    let click_x = query.click_x;
    let click_y = query.click_y;
    let map_w = query.map_w;
    let map_h = query.map_h;
    let owners = query.owners;
    let terrain = query.terrain;
    let my_id = query.my_id;
    let buildings = query.buildings;
    let pokayoke_dist = 25;
    let pokayoke_dist_sq = pokayoke_dist * pokayoke_dist;

    let mut found_any_owned = false;
    let mut found_any_land = false;
    let mut found_any_far_enough = false;

    let max_check_dist = pokayoke_dist + 6;
    let relevant_buildings: Vec<&sow_core::protocol::BuildingSnapshot> = buildings
        .iter()
        .filter(|b| {
            let bx = (b.tile_idx % map_w) as i32;
            let by = (b.tile_idx / map_w) as i32;
            (bx - click_x).abs() <= max_check_dist && (by - click_y).abs() <= max_check_dist
        })
        .collect();

    let mut valid_land_tiles = Vec::new();
    for dy in -pokayoke_dist..=pokayoke_dist {
        for dx in -pokayoke_dist..=pokayoke_dist {
            let tx = click_x + dx;
            let ty = click_y + dy;
            if tx < 0 || tx >= map_w as i32 || ty < 0 || ty >= map_h as i32 {
                continue;
            }
            if (dx * dx + dy * dy) >= pokayoke_dist_sq {
                continue;
            }
            let tile_idx = (ty * map_w as i32 + tx) as u32;

            if owners.get(tile_idx as usize).copied().unwrap_or(0) != my_id {
                continue;
            }
            found_any_owned = true;

            let tile_terrain = terrain.get(tile_idx as usize).copied().unwrap_or(0);
            let is_land = (tile_terrain & 0x80) != 0;
            if !is_land {
                continue;
            }
            found_any_land = true;

            let mut too_close = false;
            for rule in kind.spacing_rules() {
                let min_d = rule.min_distance;
                let min_d_sq = min_d * min_d;
                for b in &relevant_buildings {
                    if b.kind == rule.target_kind {
                        let bx = (b.tile_idx % map_w) as i32;
                        let by = (b.tile_idx / map_w) as i32;
                        let bdx = tx - bx;
                        let bdy = ty - by;
                        if (bdx * bdx + bdy * bdy) < min_d_sq {
                            too_close = true;
                            break;
                        }
                    }
                }
                if too_close {
                    break;
                }
            }

            if too_close {
                continue;
            }
            found_any_far_enough = true;

            if kind == sow_core::game::BuildingKind::Port {
                let mut near_water = false;
                for wdy in -2..=2 {
                    for wdx in -2..=2 {
                        let nx = tx + wdx;
                        let ny = ty + wdy;
                        if nx >= 0 && nx < map_w as i32 && ny >= 0 && ny < map_h as i32 {
                            let n_idx = (ny * map_w as i32 + nx) as usize;
                            let n_terr = terrain.get(n_idx).copied().unwrap_or(0);
                            let n_is_land = (n_terr & 0x80) != 0;
                            if !n_is_land {
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
                    continue;
                }
            }

            valid_land_tiles.push((tx, ty, tile_idx));
        }
    }

    if valid_land_tiles.is_empty() {
        if !found_any_owned {
            return Err("Target area must be inside your owned territory!");
        }
        if !found_any_land {
            return Err("Structures can only be built on land territory!");
        }
        if !found_any_far_enough {
            if kind == sow_core::game::BuildingKind::City {
                return Err("Too close to another City! Minimum spacing is 6 tiles.");
            } else {
                return Err(
                    "Too close to another structure! Spacing rules: City requires 6, other structures require 4.",
                );
            }
        }
        return Err("No space nearby!");
    }

    valid_land_tiles.sort_unstable_by(|a, b| {
        let da = (a.0 - click_x) * (a.0 - click_x) + (a.1 - click_y) * (a.1 - click_y);
        let db = (b.0 - click_x) * (b.0 - click_x) + (b.1 - click_y) * (b.1 - click_y);
        da.cmp(&db).then_with(|| a.2.cmp(&b.2))
    });
    Ok(valid_land_tiles.first().map(|&(_, _, idx)| idx).unwrap())
}
