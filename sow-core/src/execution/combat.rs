use super::{
    refund_fleet_troops_to_player, PrioritizedTile,
    RETREAT_PENALTY_VS_PLAYER,
};
use crate::engine::SowEngine;
use crate::game::{GameEvent, GamePhase};
// Removed max_tiles_cap_for_troops
use crate::map::TerrainType;
use crate::rng::NextIntExt;
use crate::warp_fleet::best_shore_spawn_for_transport;

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
        if !self.attacks.is_empty() && (self.defense_grid_dirty || self.defense_grid.grid_w == 0) {
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

            let attack_troops = execution.troops;
            let adjacent = (execution.to_conquer.len() as f64).max(1.0);
            
            // Add slight RNG to adjacent like OpenFront: borderSize + rand(0, 5)
            let rand_adj = execution.rng.next_int(0, 5) as f64;
            let num_adjacent = adjacent + rand_adj;

            let mut num_tiles_per_tick = if execution.target_owner == 0 {
                num_adjacent * 2.0
            } else {
                let defender_troops = self
                    .state
                    .player(execution.target_owner)
                    .map(|p| p.troops.max(0.0))
                    .unwrap_or(1.0)
                    .max(1.0);
                
                let ratio = (5.0 * attack_troops) / defender_troops;
                let clamped_ratio = (ratio * 2.0).clamp(0.01, 0.5);
                clamped_ratio * num_adjacent * 3.0
            };

            // Scale num_tiles_per_tick by tick rate ratio (OpenFront is 10 TPS, we are config.tick_rate_ms)
            let of_tick_ratio = self.state.config.tick_rate_ms as f64 / 100.0;
            num_tiles_per_tick *= of_tick_ratio * self.state.config.global_speed_multiplier;
            
            // Allow negatives to carry over so we strictly respect OpenFront's low-speed throttling across arbitrary tick rates
            num_tiles_per_tick += execution.tick_overflow;

            let mut stale_pops = 0u32;
            let max_stale_pops = 64;

            loop {
                if num_tiles_per_tick <= 0.0 || stale_pops > max_stale_pops {
                    execution.tick_overflow = num_tiles_per_tick;
                    break;
                }

                if execution.troops < self.state.config.attack_cost_neutral
                    || execution.troops.is_nan()
                {
                    to_remove.push(i);
                    break;
                }

                if let Some(target_tile) = execution.to_conquer.pop() {
                    // Check if target tile still belongs to the exact target
                    if self.state.map.owner_id(target_tile.x, target_tile.y)
                        != execution.target_owner
                    {
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
                    if !self.state.map.is_adjacent_to_player(
                        target_tile.x,
                        target_tile.y,
                        execution.owner_id,
                    ) {
                        stale_pops += 1;
                        continue; // Skip, edge was severed
                    }

                    // Compute Speed and Mag based on OpenFront formulas
                    let (speed, mag): (f64, f64) = match terrain_type {
                        TerrainType::Land => (16.5, 80.0),
                        TerrainType::Highland => (20.0, 100.0),
                        TerrainType::Mountain => (25.0, 120.0),
                        _ => (16.5, 80.0),
                    };

                    let mut tiles_per_tick_used = 1.0;

                    if execution.target_owner == 0 {
                        let is_bot = self.state.player(execution.owner_id).map(|p| p.player_type == crate::player::PlayerType::Bot).unwrap_or(false);
                        let cost = if is_bot { mag / 10.0 } else { mag / 5.0 };
                        execution.troops -= cost;
                        tiles_per_tick_used = ((2000.0 * speed.max(10.0)) / attack_troops).clamp(5.0, 100.0);
                    } else {
                        // PvP: Combat resolution (OpenFront Parity)
                        let mut def_loss = 0.0;
                        let mut defender_troops = 1.0;
                        let mut defender_tiles = 1.0;
                        
                        if let Some(target_player) = self.state.player(execution.target_owner) {
                            if target_player.tile_count > 0 {
                                defender_troops = target_player.troops.max(1.0);
                                defender_tiles = target_player.tile_count as f64;
                                def_loss = defender_troops / defender_tiles;
                            }
                        }

                        // Apply defense post bonuses
                        let dp_bonus = self.defense_grid.priority_bonus(
                            target_tile.x,
                            target_tile.y,
                            map_w,
                            execution.target_owner,
                        );
                        // In OpenFront: DefensePostDefenseBonus() = 5, SpeedBonus() = 3
                        let (final_mag, final_speed) = if dp_bonus > 0 {
                            (mag * 5.0, speed * 3.0)
                        } else {
                            (mag, speed)
                        };

                        // Attacker troop loss formula
                        let current_attacker_loss = (defender_troops / attack_troops).clamp(0.6, 2.0)
                            * final_mag * 0.8;
                        let alt_attacker_loss = 1.3 * def_loss * (final_mag / 100.0);
                        let atk_loss = 0.6 * current_attacker_loss + 0.4 * alt_attacker_loss;

                        execution.troops -= atk_loss;

                        if let Some(target_player) = self.state.player_mut(execution.target_owner) {
                            target_player.troops = (target_player.troops - def_loss).max(0.0);
                        }

                        tiles_per_tick_used = (defender_troops / (5.0 * attack_troops)).clamp(0.2, 1.5) * final_speed;
                    }

                    num_tiles_per_tick -= tiles_per_tick_used;
                    execution.troops = execution.troops.max(0.0);

                    // Enqueue new neutral/enemy neighbors that touch our newly acquired tile
                    self.state
                        .map
                        .for_each_neighbor(target_tile.x, target_tile.y, |nx, ny| {
                            // It must belong to target
                            if self.state.map.owner_id(nx, ny) != execution.target_owner {
                                return;
                            }

                            // How many tiles bordering (nx, ny) are owned by the attacker?
                            let mut num_owned_by_me = 0;
                            self.state.map.for_each_neighbor(nx, ny, |nnx, nny| {
                                if self.state.map.owner_id(nnx, nny) == execution.owner_id {
                                    num_owned_by_me += 1;
                                }
                            });

                            let terrain = self.state.map.terrain_type(nx, ny);
                            let mut prio =
                                execution.calc_priority(num_owned_by_me, terrain, tick_now);
                            prio += self.defense_grid.priority_bonus(
                                nx,
                                ny,
                                map_w,
                                execution.target_owner,
                            );
                            let seq = execution.insert_seq_counter;
                            execution.insert_seq_counter =
                                execution.insert_seq_counter.wrapping_add(1);

                            execution.to_conquer.push(PrioritizedTile {
                                priority: prio,
                                insert_seq: seq,
                                x: nx,
                                y: ny,
                            });
                        });

                    // VALID CONQUEST
                    // Apply change AFTER enqueuing neighbors, mimicking OpenFront's `this._owner.conquer`
                    // This ensures new neighbors don't artificially lower their priority by counting this tile as friendly yet!
                    // Guaranteed BFS spread without DFS spikes!
                    self.state
                        .set_tile_owner(target_tile.x, target_tile.y, execution.owner_id);

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
                        attacker.gold += defeated_gold * transfer_ratio;
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

        // fleets are sorted on insertion
        let mut to_remove = Vec::new();

        for i in 0..self.fleets.len() {
            let fleet = &mut self.fleets[i];

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
                        if let Some(path) = self.path_scratch.astar.find_path(
                            &self.state.map,
                            &[fleet.current_tile],
                            r_dst,
                        ) {
                            fleet.path = path;
                            fleet.path_cursor = 0;
                        } else {
                            refund_fleet_troops_to_player(
                                &mut self.state,
                                fleet.owner_id,
                                fleet.troops,
                            );
                            to_remove.push(i);
                            continue;
                        }
                    } else {
                        refund_fleet_troops_to_player(
                            &mut self.state,
                            fleet.owner_id,
                            fleet.troops,
                        );
                        to_remove.push(i);
                        continue;
                    }
                } else {
                    to_remove.push(i);
                    continue;
                }
            }

            if fleet.path.is_empty() {
                refund_fleet_troops_to_player(&mut self.state, fleet.owner_id, fleet.troops);
                to_remove.push(i);
                continue;
            }

            if fleet.path_cursor < fleet.path.len() {
                fleet.current_tile = fleet.path[fleet.path_cursor];
                fleet.path_cursor += 1;
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
                self, f_owner, prev_owner, f_troops, f_id,
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
