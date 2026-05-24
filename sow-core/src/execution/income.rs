use crate::building::aggregate_buildings_per_player;
use crate::engine::SowEngine;
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

            let is_standard_bot = self.state.players[idx].player_type
                == crate::player::PlayerType::Bot
                && !player_id.is_multiple_of(100);

            let mut max_tr = config.max_troops_base
                + max_troops_bonus * config.max_troops_scale
                + agg.city_levels as f64 * 1000.0
                + agg.armory_levels as f64 * 500.0;
            if is_standard_bot {
                max_tr /= 1.5;
            }
            self.state.players[idx].max_troops = max_tr;

            let leader = self.state.players[idx].leader;
            let sun_tzu_mult = if leader == crate::player::Leader::SunTzu {
                1.20
            } else {
                1.0
            };
            let ragnar_mult = if leader == crate::player::Leader::Ragnar {
                1.50
            } else {
                1.0
            };

            let mut troop_income = config.per_tick(config.troop_base_income)
                + config.per_tick(50.0) * agg.city_levels as f64
                + config.per_tick(80.0) * agg.armory_levels as f64 * sun_tzu_mult
                + config.per_tick(25.0) * agg.port_levels as f64 * ragnar_mult;

            if is_standard_bot {
                troop_income *= 0.75;
            }
            self.state.players[idx].troops = (safe_troops + troop_income).min(max_tr);

            let safe_gold = self.state.players[idx].gold.max(0.0);
            let cleo_mult = if leader == crate::player::Leader::Cleopatra {
                1.50
            } else {
                1.0
            };

            let mut gold_income = config.per_tick(config.gold_base_income)
                + config.per_tick(8.0) * agg.city_levels as f64
                + config.per_tick(100.0) * agg.foundry_levels as f64 * cleo_mult
                + config.per_tick(50.0) * agg.port_levels as f64 * ragnar_mult;

            if is_standard_bot {
                gold_income *= 0.75;
            }
            self.state.players[idx].gold = safe_gold + gold_income;

            let iq_gain = config.per_tick(self.state.players[idx].iq as f64 / 100.0);
            self.state.players[idx].iq_points =
                (self.state.players[idx].iq_points + iq_gain).min(500.0);
        }

        // Delete any captured structures owned by standard bots
        let mut buildings_deleted = false;
        let p_lookup = &self.state.player_lookup;
        let p_list = &self.state.players;
        self.buildings.retain(|b| {
            let pid_usize = b.owner_id as usize;
            let is_standard_bot = if pid_usize < p_lookup.len() {
                p_lookup[pid_usize]
                    .and_then(|idx| p_list.get(idx))
                    .is_some_and(|p| {
                        p.player_type == crate::player::PlayerType::Bot && p.id % 100 != 0
                    })
            } else {
                false
            };
            if is_standard_bot {
                buildings_deleted = true;
                false
            } else {
                true
            }
        });
        if buildings_deleted {
            self.building_aggregates_dirty = true;
        }

        let mut tribes_needing_city = Vec::new();
        for player in self.state.players.iter().filter(|p| p.alive) {
            if player.player_type == crate::player::PlayerType::Bot
                && player.cities == 0
                && player.tile_count >= 150
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
                let building_id = self.state.next_building_id;
                self.state.next_building_id = self.state.next_building_id.wrapping_add(1).max(1);

                self.add_building(crate::building::Building {
                    id: building_id,
                    owner_id: tid,
                    tile_idx,
                    kind: crate::game::BuildingKind::City,
                    level: 1,
                    under_construction: false,
                    ticks_until_complete: 0,
                    modules: crate::building::CityModules::default(),
                });
                if let Some(p) = self.state.player_mut(tid) {
                    p.cities += 1;
                }
                log::info!("Tribe {} established City Center at tile {}", tid, tile_idx);
            }
        }
    }
}
