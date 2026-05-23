pub fn resolve_building_placement_tile(
    kind: sow_core::game::BuildingKind,
    click_x: i32,
    click_y: i32,
    map_w: u32,
    map_h: u32,
    owners: &[u16],
    terrain: &[u8],
    my_id: u16,
    buildings: &[sow_core::protocol::BuildingSnapshot],
) -> Option<u32> {
    let min_dist = sow_core::building::STRUCTURE_MIN_DIST;
    let min_dist_sq = min_dist * min_dist;
    
    let pokayoke_dist = 25; // Large search radius to auto-snap placements (mistake-proofing)
    let pokayoke_dist_sq = pokayoke_dist * pokayoke_dist;
    let max_search_dist = pokayoke_dist + min_dist;
    let max_search_dist_sq = max_search_dist * max_search_dist;

    // Filter buildings to those close to the click target to optimize distance checks
    let nearby_buildings: Vec<_> = buildings
        .iter()
        .filter(|b| {
            let bx = (b.tile_idx % map_w) as i32;
            let by = (b.tile_idx / map_w) as i32;
            let bdx = click_x - bx;
            let bdy = click_y - by;
            (bdx * bdx + bdy * bdy) < max_search_dist_sq
        })
        .collect();

    // 1. Gather valid land structure tiles within pokayoke_dist of click target
    let mut valid_land_tiles = Vec::new();
    for dy in -pokayoke_dist..=pokayoke_dist {
        for dx in -pokayoke_dist..=pokayoke_dist {
            let tx = click_x + dx;
            let ty = click_y + dy;
            if tx < 0 || tx >= map_w as i32 || ty < 0 || ty >= map_h as i32 {
                continue;
            }
            if (dx * dx + dy * dy) >= pokayoke_dist_sq { // Euclidean distance limit
                continue;
            }
            let tile_idx = (ty * map_w as i32 + tx) as u32;
            
            // Check ownership
            if owners.get(tile_idx as usize).copied().unwrap_or(0) != my_id {
                continue;
            }
            
            // Check land (bit 7: is_land)
            let tile_terrain = terrain.get(tile_idx as usize).copied().unwrap_or(0);
            let is_land = (tile_terrain & 0x80) != 0;
            if !is_land {
                continue;
            }
            
            // Check minimum distance from existing buildings
            let mut too_close = false;
            for b in &nearby_buildings {
                let bx = (b.tile_idx % map_w) as i32;
                let by = (b.tile_idx / map_w) as i32;
                let bdx = tx - bx;
                let bdy = ty - by;
                if (bdx * bdx + bdy * bdy) < min_dist_sq {
                    too_close = true;
                    break;
                }
            }
            if too_close {
                continue;
            }
            
            valid_land_tiles.push((tx, ty, tile_idx));
        }
    }
    
    if valid_land_tiles.is_empty() {
        return None;
    }
    
    match kind {
        sow_core::game::BuildingKind::Port => {
            let mut candidates = Vec::new();
            for &(tx, ty, tile_idx) in &valid_land_tiles {
                let tile_terrain = terrain.get(tile_idx as usize).copied().unwrap_or(0);
                let is_shoreline = (tile_terrain & 0x40) != 0;
                if is_shoreline {
                    let dist = (tx - click_x).abs() + (ty - click_y).abs();
                    candidates.push((tx, ty, tile_idx, dist));
                }
            }
            
            // Sort candidates by Manhattan distance, then by tile index
            candidates.sort_by(|a, b| {
                a.3.cmp(&b.3).then_with(|| a.2.cmp(&b.2))
            });
            candidates.first().map(|&(_, _, idx, _)| idx)
        }
        _ => {
            // For other structures, find the closest valid land tile to click target by Euclidean distance
            valid_land_tiles.sort_by(|a, b| {
                let da = (a.0 - click_x) * (a.0 - click_x) + (a.1 - click_y) * (a.1 - click_y);
                let db = (b.0 - click_x) * (b.0 - click_x) + (b.1 - click_y) * (b.1 - click_y);
                da.cmp(&db).then_with(|| a.2.cmp(&b.2))
            });
            valid_land_tiles.first().map(|&(_, _, idx)| idx)
        }
    }
}
