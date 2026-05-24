use crate::game_config::max_tiles_cap_for_troops;
use crate::game::{GameEvent, GamePhase};
use crate::map::TerrainType;
use crate::warp_fleet::best_shore_spawn_for_transport;
use crate::rng::NextIntExt;
use super::{fractional_extra_tiles_milli, refund_fleet_troops_to_player, PrioritizedTile, RETREAT_PENALTY_VS_PLAYER};
use crate::engine::SowEngine;

// System analogous to AttackExecution.tick() in OpenFront
impl SowEngine {
    pub fn execute_combat(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }

        // Poka-Yoke: Collections are now strictly maintained sorted on insertion.
        let map_w = self.state.map.width;
        let map_h = self.state.map.height;

        let tick_now = self.state.tick;

        // Pre-filter all active defense posts globally ONCE per tick when grid is stale.
        if !self.attacks.is_empty()
            && (self.defense_grid_dirty || self.defense_grid.grid_w == 0)
        {
            self.defense_grid.rebuild(&self.buildings, map_w, map_h, 16);
            self.defense_grid_dirty = false;
        }

        // attacks are sorted on insertion

        let mut to_remove = Vec::new();

        for i in 0..self.attacks.len() {
            // Need to borrow state and buildings mutably but not the entire self array
            // Actually, we can borrow state and defense grid and attacks[i] simultaneously
            let execution = &mut self.attacks[i];

        if execution.retreating {
            let refund = execution.troops.max(0.0);
            if refund > 0.0 && refund.is_finite() {
                let factor = if execution.target_owner == 0 {
                    1.0
                } else {
                    1.0 - RETREAT_PENALTY_VS_PLAYER
                };
                let back = refund * factor;
                if let Some(player) = self.state.player_mut(execution.owner_id) {
                    player.troops = (player.troops + back).min(player.max_troops);
                }
            }
            to_remove.push(i);
            continue;
        }

        if execution.troops < self.state.config.attack_cost_neutral || execution.troops.is_nan() {
            to_remove.push(i);
            continue;
        }

        // Fast approximation of active frontier size without scanning the entire empire border
        let adjacent = (execution.to_conquer.len() as f64).max(1.0);

        let max_cap = max_tiles_cap_for_troops(execution.troops, &self.state.config);

        let mut max_tiles_f64 = if execution.target_owner == 0 {
            // Neutral expansion speed: proportional to true border size
            (adjacent * 2.0).max(5.0).min(max_cap)
        } else {
            let defender_troops = self.state
                .player(execution.target_owner)
                .map(|p| p.troops.max(0.0))
                .unwrap_or(1.0)
                .max(1.0);
            let ratio = execution.troops / defender_troops;
            let power = (ratio * 2.0).clamp(0.02, 0.5); 
            (power * adjacent * 3.0).max(1.0).min(max_cap)
        };

        // Speed scales with remaining troops. Higher momentum_divisor = slower ramp.
        let momentum = (execution.troops / self.state.config.momentum_divisor).clamp(0.1, 5.0);
        max_tiles_f64 *= momentum;

        // Alexander (Macedon) perk: +15% expansion speed
        if let Some(player) = self.state.player(execution.owner_id) {
            if player.leader == crate::player::Leader::Alexander {
                max_tiles_f64 *= 1.15;
            }
        }

        // Scale expansion rate to real time (per_tick semantics: tick_rate × speed multiplier)
        max_tiles_f64 *= self.state.config.tick_rate_ms as f64 / 1000.0;
        max_tiles_f64 *= self.state.config.global_speed_multiplier;

        // Determine actual integer number of tiles to process this tick (Fractional determinism)
        let mut tiles_to_conquer = max_tiles_f64.floor() as u32;
        // Unconditional RNG advancement preserves deterministic PRNG consumption order.
        let roll_milli = execution.rng.next_int(0, 1000) as u32; // 0..=999
        tiles_to_conquer += fractional_extra_tiles_milli(max_tiles_f64, roll_milli);

        let mut expanded_this_tick = 0u32;
        let mut stale_pops = 0u32;
        let max_stale_pops = (tiles_to_conquer * 4).max(64);

        loop {
            if expanded_this_tick >= tiles_to_conquer || stale_pops > max_stale_pops {
                break;
            }

            if execution.troops < self.state.config.attack_cost_neutral || execution.troops.is_nan() {
                to_remove.push(i);
                break;
            }

            if let Some(target_tile) = execution.to_conquer.pop() {
                // Check if target tile still belongs to the exact target
                if self.state.map.owner_id(target_tile.x, target_tile.y) != execution.target_owner {
                    stale_pops += 1;
                    continue; // Skip, no longer controlled by target
                }

                // Terrain Check: Water and Mountains are impassable for basic ground attacks
                let terrain_type = self.state.map.terrain_type(target_tile.x, target_tile.y);
                if terrain_type == TerrainType::Water {
                    stale_pops += 1;
                    continue; // Skip, impassable terrain
                }

                // Check adjacency: ensure it still borders the attacker
                if !self.state.map.is_adjacent_to_player(target_tile.x, target_tile.y, execution.owner_id) {
                    stale_pops += 1;
                    continue; // Skip, edge was severed
                }

                let terrain_multiplier = match terrain_type {
                    TerrainType::Land => 1.0,
                    TerrainType::Highland => self.state.config.terrain_multiplier_highland,
                    TerrainType::Mountain => self.state.config.terrain_multiplier_mountain,
                    _ => 1.0,
                };

                if execution.target_owner == 0 {
                    // Neutral: attacker pays constant base cost scaled by terrain multiplier
                    execution.troops -= self.state.config.attack_cost_neutral * terrain_multiplier;
                } else {
                    // PvP: Combat resolution (OpenFront Parity)
                    let mut def_loss = 0.0;
                    if let Some(target_player) = self.state.player(execution.target_owner) {
                        if target_player.tile_count > 0 {
                            // Defender loses proportional to their troop density.
                            // Clamp to 0: if troops went negative from multi-attack drain,
                            // a negative def_loss would ADD troops to the defender on
                            // subtract, compounding per tick and diverging across clients.
                            def_loss = target_player.troops.max(0.0) / target_player.tile_count as f64;
                        }
                    }

                    // Defense posts increase attacker losses slightly
                    let dp_bonus = self.defense_grid.priority_bonus(
                        target_tile.x,
                        target_tile.y,
                        map_w,
                        execution.target_owner,
                    );
                    let dp_multiplier = 1.0 + (dp_bonus as f64 / 10.0); // Approximation of defense buff

                    let atk_loss = self.state.config.attack_cost_enemy * terrain_multiplier * dp_multiplier;

                    execution.troops -= atk_loss;

                    // Deduct from defender
                    if let Some(target_player) = self.state.player_mut(execution.target_owner) {
                        target_player.troops = (target_player.troops - def_loss).max(0.0);
                    }
                }
                
                execution.troops = execution.troops.max(0.0);
                expanded_this_tick += 1;

                // Enqueue new neutral/enemy neighbors that touch our newly acquired tile
                let is_odd = (target_tile.y % 2) != 0;
                let deltas = if is_odd {
                    [(1, 0), (-1, 0), (0, -1), (1, -1), (0, 1), (1, 1)]
                } else {
                    [(1, 0), (-1, 0), (-1, -1), (0, -1), (-1, 1), (0, 1)]
                };
                for &(dx, dy) in &deltas {
                    let nx = target_tile.x as i32 + dx;
                    let ny = target_tile.y as i32 + dy;
                    if nx >= 0 && nx < map_w as i32 && ny >= 0 && ny < map_h as i32 {
                        let nx = nx as u32;
                        let ny = ny as u32;
                        if self.state.map.owner_id(nx, ny) != execution.target_owner {
                            continue;
                        }

                        // How many tiles bordering (nx, ny) are owned by the attacker?
                        let mut num_owned_by_me = 0;
                        let n_is_odd = (ny % 2) != 0;
                        let n_deltas = if n_is_odd {
                            [(1, 0), (-1, 0), (0, -1), (1, -1), (0, 1), (1, 1)]
                        } else {
                            [(1, 0), (-1, 0), (-1, -1), (0, -1), (-1, 1), (0, 1)]
                        };
                        for &(ndx, ndy) in &n_deltas {
                            let nnx = nx as i32 + ndx;
                            let nny = ny as i32 + ndy;
                            if nnx >= 0 && nnx < map_w as i32 && nny >= 0 && nny < map_h as i32 {
                                if self.state.map.owner_id(nnx as u32, nny as u32) == execution.owner_id {
                                    num_owned_by_me += 1;
                                }
                            }
                        }

                        let terrain = self.state.map.terrain_type(nx, ny);
                        let mut prio = execution.calc_priority(num_owned_by_me, terrain, tick_now);
                        prio += self.defense_grid.priority_bonus(
                            nx,
                            ny,
                            map_w,
                            execution.target_owner,
                        );
                        let seq = execution.insert_seq_counter;
                        execution.insert_seq_counter = execution.insert_seq_counter.wrapping_add(1);

                        execution.to_conquer.push(PrioritizedTile {
                            priority: prio,
                            insert_seq: seq,
                            x: nx,
                            y: ny,
                        });
                    }
                }

                // VALID CONQUEST
                // Apply change AFTER enqueuing neighbors, mimicking OpenFront's `this._owner.conquer`
                // This ensures new neighbors don't artificially lower their priority by counting this tile as friendly yet!
                // Guaranteed BFS spread without DFS spikes!
                self.state.set_tile_owner(target_tile.x, target_tile.y, execution.owner_id);

                // Send event
                self.state.events.push(GameEvent::TileCaptured {
                    x: target_tile.x,
                    y: target_tile.y,
                    new_owner: execution.owner_id,
                });
            } else {
                let refund = execution.troops.max(0.0);
                if refund > 0.0 && refund.is_finite() {
                    if let Some(player) = self.state.player_mut(execution.owner_id) {
                        player.troops = (player.troops + refund).min(player.max_troops);
                    }
                }
                to_remove.push(i);
                break;
            }
        }

        // Conquer Gold Mechanic: Check elimination ONCE per attack, outside the tile loop
        let execution_ref = &self.attacks[i];
        if execution_ref.target_owner != 0 {
            let mut defeated_gold = 0.0;
            let mut is_eliminated = false;
            let mut transfer_ratio = 1.0;

            if let Some(target_player) = self.state.player(execution_ref.target_owner) {
                if target_player.tile_count == 0 && target_player.alive {
                    is_eliminated = true;
                    defeated_gold = target_player.gold.max(0.0);
                    if target_player.player_type == crate::player::PlayerType::Bot {
                        defeated_gold += 25.0; // Fixed bounty for eating a Tribe
                    }
                    if target_player.is_human() {
                        transfer_ratio = 0.5;
                    }
                }
            }

            if is_eliminated {
                // 1. Zero out defeated player
                if let Some(target_player) = self.state.player_mut(execution_ref.target_owner) {
                    target_player.gold = 0.0;
                    target_player.alive = false;
                }
                // 2. Transfer gold to conqueror
                if let Some(attacker) = self.state.player_mut(execution_ref.owner_id) {
                    let mut bounty_mult = 1.0;
                    if attacker.leader == crate::player::Leader::GenghisKhan {
                        bounty_mult = 1.5; // +50% bounty gold!
                    }
                    attacker.gold += defeated_gold * transfer_ratio * bounty_mult;
                }
            }
        }
    }

    // Remove dead attacks in O(1) and re-sort to preserve deterministic order
    let has_removals = !to_remove.is_empty();
    for i in to_remove.into_iter().rev() {
        self.attacks.swap_remove(i);
    }
    if has_removals {
        self.attacks.sort_unstable_by_key(|a| a.id);
    }
}

/// OpenFront `TransportShipExecution`: 1 tile/tick over water, retreat, landing → conquer + `AttackExecution`.
pub fn execute_fleets(&mut self) {
    if self.state.phase != GamePhase::Playing {
        return;
    }

    // Spatial hash of fleets: current_tile -> fleet indices
    let mut tile_to_fleets: std::collections::HashMap<u32, Vec<usize>> = std::collections::HashMap::new();
    for (idx, fleet) in self.fleets.iter().enumerate() {
        if fleet.troops > 0.0 {
            tile_to_fleets.entry(fleet.current_tile).or_default().push(idx);
        }
    }

    // Simple Naval Combat: Warships damage enemy fleets on the same tile or adjacent tiles
    let mut damages = Vec::new();
    let w = self.state.map.width;
    for i in 0..self.fleets.len() {
        if self.fleets[i].unit_type == crate::game::UnitType::Warship && self.fleets[i].troops > 0.0 {
            let mut neighbors = [0u32; 7];
            neighbors[0] = self.fleets[i].current_tile;
            let mut n_count = 1;
            let fx = self.fleets[i].current_tile % w;
            let fy = self.fleets[i].current_tile / w;
            self.state.map.for_each_neighbor(fx, fy, |nx, ny| {
                neighbors[n_count] = ny * w + nx;
                n_count += 1;
            });

            'outer: for &t in neighbors.iter().take(n_count) {
                if let Some(indices) = tile_to_fleets.get(&t) {
                    for &j in indices {
                        if i != j && self.fleets[j].owner_id != self.fleets[i].owner_id && self.fleets[j].troops > 0.0 {
                            damages.push((j, 100.0));
                            break 'outer;
                        }
                    }
                }
            }
        }
    }
    for (target_idx, dmg) in damages {
        self.fleets[target_idx].troops -= dmg;
    }

    // fleets are sorted on insertion
    let mut to_remove = Vec::new();

    for i in 0..self.fleets.len() {
        let fleet = &mut self.fleets[i];

        if fleet.troops <= 0.0 {
            to_remove.push(i);
            continue;
        }

        if fleet.retreating && fleet.retreat_dst.is_none() {
            if let Some(player) = self.state.player(fleet.owner_id) {
                let here_comp = self.water.component_of(fleet.current_tile);
                let comp_filter = if here_comp == 0 {
                    None
                } else {
                    Some((&self.water, here_comp))
                };
                if let Some(r_dst) = best_shore_spawn_for_transport(
                    &self.state.map,
                    fleet.owner_id,
                    &player.border_tiles,
                    fleet.current_tile,
                    comp_filter,
                ) {
                    fleet.retreat_dst = Some(r_dst);
                    if let Some(path) = self.path_scratch.astar.find_path(&self.state.map, &[fleet.current_tile], r_dst) {
                        fleet.path = std::sync::Arc::new(path);
                        fleet.path_cursor = 0;
                    } else {
                        refund_fleet_troops_to_player(&mut self.state, fleet.owner_id, fleet.troops);
                        to_remove.push(i);
                        continue;
                    }
                } else {
                    refund_fleet_troops_to_player(&mut self.state, fleet.owner_id, fleet.troops);
                    to_remove.push(i);
                    continue;
                }
            } else {
                to_remove.push(i);
                continue;
            }
        }

        if fleet.flow_target.is_none() && fleet.path.is_empty() {
            super::refund_fleet_troops_to_player(&mut self.state, fleet.owner_id, fleet.troops);
            to_remove.push(i);
            continue;
        }

        if let Some(target) = fleet.flow_target {
            let map = &self.state.map;
            let flow_field = self.flow_field_cache.get_or_compute(target, map);
            let dir = flow_field.directions[fleet.current_tile as usize];
            if dir < 8 {
                let w = map.width;
                let cx = fleet.current_tile % w;
                let cy = fleet.current_tile / w;
                let dx = [0, 1, 1, 1, 0, -1, -1, -1];
                let dy = [-1, -1, 0, 1, 1, 1, 0, -1];
                let nx = cx as i32 + dx[dir as usize];
                let ny = cy as i32 + dy[dir as usize];
                fleet.current_tile = ny as u32 * w + nx as u32;
            } else if dir == 8 {
                // Reached destination!
                fleet.path = std::sync::Arc::new(Vec::new());
                fleet.path_cursor = 0;
            } else {
                // Unreachable via FlowField
                super::refund_fleet_troops_to_player(&mut self.state, fleet.owner_id, fleet.troops);
                to_remove.push(i);
                continue;
            }
        } else if fleet.path_cursor < fleet.path.len() {
            fleet.current_tile = fleet.path[fleet.path_cursor];
            fleet.path_cursor += 1;
        }

        if fleet.unit_type == crate::game::UnitType::TradeShip {
            if let Some(p) = self.state.player_mut(fleet.owner_id) {
                p.gold += 15.0; // Passive gold generation
            }
            if fleet.path_cursor >= fleet.path.len() && !fleet.path.is_empty() {
                // Loop back
                let mut p = (*fleet.path).clone();
                p.reverse();
                fleet.path = std::sync::Arc::new(p);
                fleet.path_cursor = 0;
            }
            continue;
        }

        if fleet.unit_type == crate::game::UnitType::Warship {
            // Stop at destination
            if fleet.path_cursor >= fleet.path.len() {
                fleet.path = std::sync::Arc::new(Vec::new());
                fleet.path_cursor = 0;
            }
            continue;
        }

        if fleet.path_cursor < fleet.path.len() {
            continue;
        }

        let w = self.state.map.width;
        let lx = fleet.current_tile % w;
        let ly = fleet.current_tile / w;
        let owner_here = self.state.map.owner_id(lx, ly);

        if fleet.retreating {
            if owner_here == fleet.owner_id {
                let deaths = fleet.troops * crate::warp_fleet::FLEET_RETREAT_SHORE_MALUS;
                let survivors = fleet.troops - deaths;
                if let Some(p) = self.state.player_mut(fleet.owner_id) {
                    p.troops = (p.troops + survivors).min(p.max_troops);
                }
            } else {
                refund_fleet_troops_to_player(&mut self.state, fleet.owner_id, fleet.troops);
            }
            to_remove.push(i);
            continue;
        }

        if owner_here == fleet.owner_id {
            let deaths = fleet.troops * crate::warp_fleet::FLEET_RETREAT_SHORE_MALUS;
            let survivors = fleet.troops - deaths;
            if let Some(p) = self.state.player_mut(fleet.owner_id) {
                p.troops = (p.troops + survivors).min(p.max_troops);
            }
            to_remove.push(i);
            continue;
        }

        let prev_owner = owner_here;
        let f_owner = fleet.owner_id;
        let f_troops = fleet.troops;
        let f_id = fleet.id;
        
        self.state.set_tile_owner(lx, ly, f_owner);
        self.state.events.push(GameEvent::TileCaptured {
            x: lx,
            y: ly,
            new_owner: f_owner,
        });

        // Drop mutable borrow of self.fleets before calling method on self
        crate::intent::spawn_or_merge_attack_for_fleet_arrival_pure(
            self,
            f_owner,
            prev_owner,
            f_troops,
            f_id,
        );
        to_remove.push(i);
    }

    // Remove finished fleets in O(1) and re-sort to preserve deterministic order
    let has_removals = !to_remove.is_empty();
    for i in to_remove.into_iter().rev() {
        self.fleets.swap_remove(i);
    }
    if has_removals {
        self.fleets.sort_unstable_by_key(|f| f.id);
    }
}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fractional_extra_tile_milli_threshold_is_stable() {
        // 12.499 -> threshold 499 (roll 498 adds one; 499 does not)
        assert_eq!(fractional_extra_tiles_milli(12.499, 498), 1);
        assert_eq!(fractional_extra_tiles_milli(12.499, 499), 0);
        // Very small fraction below one milli should never add.
        assert_eq!(fractional_extra_tiles_milli(7.0009, 0), 0);
        // Near-one fraction clamps to 999 max threshold.
        assert_eq!(fractional_extra_tiles_milli(3.999_999_999, 998), 1);
        assert_eq!(fractional_extra_tiles_milli(3.999_999_999, 999), 0);
    }
}
