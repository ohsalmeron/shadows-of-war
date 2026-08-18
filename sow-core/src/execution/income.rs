use crate::building::aggregate_buildings_per_player;
use crate::engine::SowEngine;
use crate::execution::income_rates::{gold_income_per_second, troop_income_per_second};
use crate::game::GamePhase;

impl SowEngine {
    pub fn execute_income(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }

        if self.building_aggregates_dirty {
            let max_pid = self
                .state
                .players
                .iter()
                .map(|p| p.id as usize)
                .max()
                .unwrap_or(0);
            self.building_aggregates =
                aggregate_buildings_per_player(self.buildings.iter().copied(), max_pid);
            self.building_aggregates_dirty = false;
        }

        if self.sea_lanes_dirty {
            crate::sea_lane::update_sea_lanes(self);
            self.sea_lanes_dirty = false;
        }
        let aggs = &self.building_aggregates;

        let config = self.state.config.clone();
        let num_players = self.state.players.len();
        for idx in 0..num_players {
            if !self.state.players[idx].alive {
                continue;
            }
            let tiles_owned = self.state.players[idx].tile_count;
            if tiles_owned == 0 {
                self.state.players[idx].alive = false;
                continue;
            }

            let player_id = self.state.players[idx].id;
            let agg = aggs.get(player_id as usize).copied().unwrap_or_default();
            let safe_troops = self.state.players[idx].troops.max(0.0);

            let t_f64 = tiles_owned as f64;
            let t_half = t_f64.sqrt();
            let t_quarter = t_half.sqrt();
            let t_eighth = t_quarter.sqrt();
            let max_troops_bonus = t_half * t_eighth;

            // Tribes (PlayerType::Bot) always eat the standard-bot handicap —
            // no id-based carve-out (that used to let a handful of "élite"
            // tribes dodge it by id%100, inverting the food chain).
            let is_standard_bot = self.state.players[idx].player_type
                == crate::player::PlayerType::Bot;

            let leader = self.state.players[idx].leader;
            let richard_mult = if leader == crate::player::Leader::RichardTheLionheart {
                1.50
            } else {
                1.0
            };
            let leonidas_mult = if leader == crate::player::Leader::Leonidas {
                1.50
            } else {
                1.0
            };

            let mut max_tr = config.max_troops_base
                + max_troops_bonus * config.max_troops_scale
                + agg.city_levels as f64 * config.city_max_troops * richard_mult
                + agg.armory_levels as f64 * 500.0 * leonidas_mult;
            if is_standard_bot {
                max_tr /= 1.5;
            }
            if let Some(cap) = self.state.players[idx].max_troops_cap {
                max_tr = max_tr.min(cap);
            }
            self.state.players[idx].max_troops = max_tr;

            let troop_ps = troop_income_per_second(tiles_owned, agg, leader, &config);
            let mut troop_income = config.per_tick(troop_ps);

            if is_standard_bot {
                troop_income *= 0.75;
            }
            self.state.players[idx].troops = (safe_troops + troop_income).min(max_tr);

            let safe_gold = self.state.players[idx].gold.max(0.0);

            let gold_ps = gold_income_per_second(tiles_owned, agg, leader, &config);
            let mut gold_income = config.per_tick(gold_ps);

            if is_standard_bot {
                gold_income *= 0.75;
            }
            self.state.players[idx].gold = safe_gold + gold_income;

            let iq_gain = config.per_tick(self.state.players[idx].iq as f64 / 100.0);
            self.state.players[idx].iq_points =
                (self.state.players[idx].iq_points + iq_gain).min(500.0);
        }

        let mut tribes_needing_city = Vec::new();
        for player in self
            .state
            .players
            .iter()
            .filter(|p| p.alive && self.state.config.buildings_enabled)
        {
            let has_city = aggs
                .get(player.id as usize)
                .is_some_and(|a| a.city_levels > 0);
            let is_standard_bot = player.player_type == crate::player::PlayerType::Bot
                && !player.id.is_multiple_of(100);
            let needs_city = if is_standard_bot {
                player.cities == 0
            } else {
                !has_city
            };
            if player.player_type == crate::player::PlayerType::Bot
                && needs_city
                && player.tile_count >= 150
                && (self.state.tick + player.id as u64).is_multiple_of(30)
            {
                tribes_needing_city.push((
                    player.id,
                    player.sum_x,
                    player.sum_y,
                    player.tile_count,
                ));
            }
        }

        for (tid, sum_x, sum_y, tile_count) in tribes_needing_city {
            let cx = (sum_x / tile_count as u64) as u32;
            let cy = (sum_y / tile_count as u64) as u32;
            let w = self.state.map.width;
            let mut found_tile = None;
            for dy in -5..=5 {
                for dx in -5..=5 {
                    let nx = cx as i32 + dx;
                    let ny = cy as i32 + dy;
                    if self.state.map.is_valid_coord(nx, ny) {
                        let (ux, uy) = (nx as u32, ny as u32);
                        if self.state.map.owner_id(ux, uy) == tid
                            && self.state.map.terrain[self.state.map.ref_id(ux, uy)].is_land()
                        {
                            found_tile = Some(uy * w + ux);
                            break;
                        }
                    }
                }
                if found_tile.is_some() {
                    break;
                }
            }
            if let Some(tile_idx) = found_tile {
                self.refresh_building_grid();
                let spawn_ok = crate::building::resolve_structure_spawn_tile(
                    &self.state.map,
                    tid,
                    crate::game::BuildingKind::City,
                    tile_idx,
                    &self.building_grid,
                    &self.buildings,
                    &mut self.placement_scratch,
                );
                if let Some(spawn_idx) = spawn_ok {
                    let building_id = self.state.next_building_id;
                    self.state.next_building_id =
                        self.state.next_building_id.wrapping_add(1).max(1);

                    self.add_building(crate::building::Building {
                        id: building_id,
                        owner_id: tid,
                        tile_idx: spawn_idx,
                        kind: crate::game::BuildingKind::City,
                        level: 1,
                        under_construction: false,
                        ticks_until_complete: 0,
                        modules: crate::building::CityModules::default(),
                    });
                    if let Some(p) = self.state.player_mut(tid) {
                        p.cities += 1;
                    }
                }
            }
        }
    }
}
