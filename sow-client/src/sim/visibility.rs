pub(crate) fn compute_visibility(
    map_w: u32,
    map_h: u32,
    my_id: u16,
    owners: &[u16],
    snap: &sow_core::protocol::SimSnapshot,
    _terrain: &[u8],
    fog_explored: &mut sow_core::bitset::DenseBitSet,
    fog_visible: &mut sow_core::bitset::DenseBitSet,
    fog_of_war_enabled: bool,
) {
    if map_w == 0 || map_h == 0 {
        return;
    }

    let my_player = snap.players.iter().find(|p| p.id == my_id);
    let is_alive = my_player.map_or(false, |p| p.alive);
    let game_over = matches!(snap.phase, sow_core::game::GamePhase::GameOver);

    if !fog_of_war_enabled || my_id == 0 || game_over || !is_alive {
        // SPECTATOR, GAME OVER, DEAD PLAYER or FOG DISABLED: Make everything fully visible and explored
        let total_tiles = map_w * map_h;
        let total_blocks = (total_tiles + 63) as usize / 64;
        fog_visible.blocks.resize(total_blocks, 0);
        fog_explored.blocks.resize(total_blocks, 0);
        for i in 0..total_blocks {
            fog_visible.blocks[i] = !0u64;
            fog_explored.blocks[i] = !0u64;
        }
        return;
    }

    if matches!(snap.phase, sow_core::game::GamePhase::Spawning { .. }) {
        // SPAWNING/DEPLOYMENT PHASE: Make everything fully visible, but unexplored
        let total_tiles = map_w * map_h;
        let total_blocks = (total_tiles + 63) as usize / 64;
        fog_visible.blocks.resize(total_blocks, 0);
        fog_explored.blocks.resize(total_blocks, 0);
        for i in 0..total_blocks {
            fog_visible.blocks[i] = !0u64;
            fog_explored.blocks[i] = 0u64;
        }
        return;
    }

    // Reset current visibility
    *fog_visible = sow_core::bitset::DenseBitSet::new();

    let territory_radius = 4;
    let building_radius = 8;
    let fleet_radius = 6;

    // Helper closure to check if owner is player or ally/teammate (optimized with lookup table)
    let mut ally_or_self = vec![false; 65536];
    let my_player = snap.players.iter().find(|p| p.id == my_id);
    for p in &snap.players {
        let pid = p.id;
        if pid == my_id {
            ally_or_self[pid as usize] = true;
            continue;
        }
        let is_allied = my_player.map_or(false, |mp| mp.alliances.contains(&pid));
        let is_teammate = {
            let my_team = my_player.and_then(|mp| mp.team);
            let other_team = p.team;
            my_team.is_some() && my_team == other_team
        };
        ally_or_self[pid as usize] = is_allied || is_teammate;
    }

    let is_ally_or_self = |other_owner: u16| -> bool { ally_or_self[other_owner as usize] };

    // Helper to add a radius of visibility (accepting fog_visible as parameter to avoid borrow checker errors)
    let add_vision = |tile_idx: u32, radius: i32, f_vis: &mut sow_core::bitset::DenseBitSet| {
        let cx = (tile_idx % map_w) as i32;
        let cy = (tile_idx / map_w) as i32;
        let r1 = cy;
        let q1 = cx - (cy - (cy & 1)) / 2;
        let s1 = -q1 - r1;

        for dy in -radius..=radius {
            let y = cy + dy;
            if y >= 0 && y < map_h as i32 {
                let r2 = y;
                let r_diff_abs = (r1 - r2).abs();
                let y_offset = (y - (y & 1)) / 2;
                for dx in -radius..=radius {
                    let x = cx + dx;
                    if x >= 0 && x < map_w as i32 {
                        let q2 = x - y_offset;
                        let q_diff = q1 - q2;
                        let s2 = -q2 - r2;
                        let s_diff = s1 - s2;
                        if (q_diff.abs() + r_diff_abs + s_diff.abs()) / 2 <= radius {
                            let idx = y as u32 * map_w + x as u32;
                            f_vis.insert(idx);
                        }
                    }
                }
            }
        }
    };

    thread_local! {
        static TEMP_OWNERS_CACHE: std::cell::RefCell<Vec<u16>> = std::cell::RefCell::new(Vec::new());
        static PLAYER_TILE_COUNTS_CACHE: std::cell::RefCell<Vec<usize>> = std::cell::RefCell::new(Vec::new());
    }

    // Pre-apply snapshot's dirty tiles to a temporary copy of owners to ensure
    // we calculate vision for newly claimed/spawned tiles immediately.
    let total_tiles = (map_w * map_h) as usize;
    let mut temp_owners = TEMP_OWNERS_CACHE.with(|cache| {
        let mut v = cache.borrow_mut();
        v.clear();
        v.resize(total_tiles, 0u16);
        std::mem::take(&mut *v)
    });
    let limit = owners.len().min(total_tiles);
    temp_owners[..limit].copy_from_slice(&owners[..limit]);
    for dt in &snap.dirty_tiles {
        let idx = dt.index as usize;
        if idx < temp_owners.len() {
            temp_owners[idx] = dt.new_owner;
        }
    }

    // Pre-calculate tile count for each player to support vision radius scaling
    let mut player_tile_counts = PLAYER_TILE_COUNTS_CACHE.with(|cache| {
        let mut v = cache.borrow_mut();
        v.clear();
        v.resize(65536, 0usize);
        std::mem::take(&mut *v)
    });
    for &owner in &temp_owners {
        player_tile_counts[owner as usize] += 1;
    }

    let get_bonus = |owner: u16| -> i32 {
        let count = player_tile_counts[owner as usize];
        ((count / 1000) as i32).min(8)
    };

    // Helper to check if a tile is on the edge of the player's territory (excluding allied borders)
    let is_border_tile = |tile_idx: u32, temp_owners: &[u16]| -> bool {
        let x = tile_idx % map_w;
        let y = tile_idx / map_w;
        let owner = temp_owners[tile_idx as usize];
        let deltas = [(1, 0), (-1, 0), (0, -1), (0, 1)];
        for &(dx, dy) in &deltas {
            let nx = x as i32 + dx;
            let ny = y as i32 + dy;
            if nx >= 0 && nx < map_w as i32 && ny >= 0 && ny < map_h as i32 {
                let n_idx = (ny as u32 * map_w + nx as u32) as usize;
                let n_owner = temp_owners[n_idx];
                // If the neighboring tile belongs to a different owner who is NOT an ally,
                // then this is a boundary tile that needs to project vision.
                if n_owner != owner && !ally_or_self[n_owner as usize] {
                    return true;
                }
            } else {
                return true; // Map edges always count as perimeter
            }
        }
        false
    };

    // 1. Vision from owned/allied tiles
    for tile_idx in 0..total_tiles {
        let owner = temp_owners[tile_idx];
        if is_ally_or_self(owner) {
            // Interior tiles are always visible to their owner.
            // We only run the radial projection (add_vision) for border tiles
            // to project vision outside the territory.
            fog_visible.insert(tile_idx as u32);
            if is_border_tile(tile_idx as u32, &temp_owners) {
                let bonus = get_bonus(owner);
                add_vision(tile_idx as u32, territory_radius + bonus, fog_visible);
            }
        }
    }

    // 2. Vision from owned/allied buildings
    for b in &snap.buildings {
        if is_ally_or_self(b.owner_id) {
            let bonus = get_bonus(b.owner_id);
            add_vision(b.tile_idx, building_radius + bonus, fog_visible);
        }
    }

    // 3. Vision from owned/allied fleets
    for f in &snap.fleets {
        if is_ally_or_self(f.owner_id) {
            let bonus = get_bonus(f.owner_id);
            add_vision(f.current_tile, fleet_radius + bonus, fog_visible);
        }
    }

    // 4. Merge visible into explored
    let total_blocks = fog_visible.blocks.len();
    if fog_explored.blocks.len() < total_blocks {
        fog_explored.blocks.resize(total_blocks, 0);
    }
    for i in 0..total_blocks {
        fog_explored.blocks[i] |= fog_visible.blocks[i];
    }

    // Restore cached vectors to the thread-local storage
    TEMP_OWNERS_CACHE.with(|cache| {
        *cache.borrow_mut() = temp_owners;
    });
    PLAYER_TILE_COUNTS_CACHE.with(|cache| {
        *cache.borrow_mut() = player_tile_counts;
    });
}
