use crate::building::aggregate_buildings_per_player;
use crate::engine::SowEngine;
use crate::game::{GamePhase, GameState};

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
            self.region_grid.rebuild(self.state.map.width, self.state.map.height, &self.state.map.state, &self.buildings);
            self.building_aggregates_dirty = false;
        }

        if self.railroads_dirty {
            crate::building::railroad::update_railroads(self);
            self.railroads_dirty = false;
        }

        if self.sea_lanes_dirty {
            crate::sea_lane::update_sea_lanes(self);
            self.sea_lanes_dirty = false;
        }
        let aggs = &self.building_aggregates;

        let config = self.state.config.clone();
        let GameState {
            map: _, players, ..
        } = &mut self.state;

        for player in players.iter_mut().filter(|p| p.alive) {
            let tiles_owned = player.tile_count;
            if tiles_owned == 0 {
                player.alive = false; // Dead
                continue;
            }

            let agg = aggs.get(player.id as usize).copied().unwrap_or_default();

            // Defensive mathematical bound: if player troops violently drain below zero during massive intent execution bursts,
            // the calculation mathematically breaks and introduces desyncing NaN generations universally. We clamp to safely avoid this!
            let safe_troops = player.troops.max(0.0);

            // Use strictly deterministic IEEE-754 sqrt instead of libm powf!
            // tiles_owned^0.625 = tiles_owned^(1/2) * tiles_owned^(1/8)
            let t_f64 = tiles_owned as f64;
            let t_half = t_f64.sqrt();
            let t_quarter = t_half.sqrt();
            let t_eighth = t_quarter.sqrt();
            let max_troops_bonus = t_half * t_eighth;

            player.max_troops = config.max_troops_base
                + max_troops_bonus * config.max_troops_scale
                + agg.city_levels as f64 * config.city_max_troops_per_level
                + agg.factory_levels as f64 * 500.0;

            let raw_income = config.per_tick(config.troop_base_income);

            let cap_extra = (config.factory_income_bonus_cap - 1.0).max(0.0);
            let sun_tzu_mult = if player.leader == crate::player::Leader::SunTzu { 1.20 } else { 1.0 };
            let total_troop_boost_levels = (agg.factory_levels as f64 * sun_tzu_mult) + agg.port_levels as f64 * 0.5;

            let factory_extra = (total_troop_boost_levels * config.factory_income_bonus_per_level)
                .min(cap_extra);
            let factory_mult = 1.0 + factory_extra;
            let income = raw_income * factory_mult;

            player.troops = (safe_troops + income).min(player.max_troops);

            let safe_gold = player.gold.max(0.0);

            let cleo_mult = if player.leader == crate::player::Leader::Cleopatra { 1.50 } else { 1.0 };
            let ragnar_mult = if player.leader == crate::player::Leader::Ragnar { 1.50 } else { 1.0 };

            let industry_gold = agg.industry_levels as f64 * 100.0 * cleo_mult;
            let port_gold = agg.port_levels as f64 * 50.0 * ragnar_mult;
            let city_gold = agg.city_levels as f64 * config.gold_income_per_city_level;

            let gold_base = config.gold_base_income;
            let gold_income = config.per_tick(gold_base + city_gold + industry_gold + port_gold);
            player.gold = safe_gold + gold_income;

            let iq_gain = config.per_tick(player.iq as f64 / 100.0);
            player.iq_points = (player.iq_points + iq_gain).min(500.0);
        }

        let mut tribes_needing_city = Vec::new();
        for player in self.state.players.iter().filter(|p| p.alive) {
            if player.player_type == crate::player::PlayerType::Bot && player.cities == 0 && player.tile_count >= 150 {
                tribes_needing_city.push((player.id, player.sum_x, player.sum_y, player.tile_count));
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
                });
                if let Some(p) = self.state.player_mut(tid) {
                    p.cities += 1;
                }
                log::info!("Tribe {} established City Center at tile {}", tid, tile_idx);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::Building;
    use crate::game::BuildingKind;
    use crate::player::Player;
    use crate::water_components::WaterComponents;

    fn engine_one_player(seed: u64, tiles: u32, troops: f64, gold: f64) -> SowEngine {
        let mut game = GameState::new(seed, 8, 8, crate::game_config::GameConfig::default());
        game.phase = GamePhase::Playing;
        game.players.push(Player::new_human(
            1,
            "p".into(),
            [1.0, 0.0, 0.0],
            &crate::game_config::GameConfig::default(),
        ));
        game.player_lookup = vec![None, Some(0)];
        if let Some(p) = game.player_mut(1) {
            p.tile_count = tiles;
            p.troops = troops;
            p.gold = gold;
        }
        SowEngine::new(game, WaterComponents::default())
    }

    #[test]
    fn city_levels_raise_max_troops_via_income_system() {
        let mut engine = engine_one_player(42, 50, 10.0, 0.0);
        engine.buildings.push(Building {
            id: 1,
            owner_id: 1,
            tile_idx: 0,
            kind: BuildingKind::City,
            level: 4,
            under_construction: false,
            ticks_until_complete: 0,
        });
        engine.execute_income();
        let cfg = &engine.state.config;
        let p = engine.state.player(1).unwrap();
        let expected_floor = cfg.max_troops_base + 4.0 * cfg.city_max_troops_per_level;
        assert!(
            p.max_troops + 0.01 >= expected_floor,
            "max_troops={} expected>={}",
            p.max_troops,
            expected_floor
        );
    }

    #[test]
    fn factory_income_multiplier_respects_cap() {
        let mut engine = engine_one_player(43, 50, 5.0, 0.0);
        engine.state.config.factory_income_bonus_cap = 1.5;
        engine.state.config.troop_base_income = 100.0;
        engine.state.config.global_speed_multiplier = 1.0;
        for i in 0u32..20 {
            engine.buildings.push(Building {
                id: u64::from(i + 1),
                owner_id: 1,
                tile_idx: i,
                kind: BuildingKind::Factory,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            });
        }
        engine.execute_income();
        let p = engine.state.player(1).unwrap();
        let cfg = &engine.state.config;
        let low = 5.0_f64;
        let raw_income = cfg.per_tick(cfg.troop_base_income);
        
        let uncapped = raw_income
            * (1.0 + 20.0 * cfg.factory_income_bonus_per_level);
        let actual_gain = p.troops - low;
        assert!(
            actual_gain <= uncapped + 0.001,
            "income should be capped by FACTORY_INCOME_BONUS_CAP"
        );
        assert!(
            actual_gain <= raw_income * cfg.factory_income_bonus_cap + 0.02,
            "gain {} exceeds cap-scaled income",
            actual_gain
        );
    }

    #[test]
    fn troop_base_income_generates_correctly() {
        let mut engine = engine_one_player(46, 1, 0.0, 0.0);
        engine.state.config.troop_base_income = 100.0;
        engine.state.config.global_speed_multiplier = 4.0;
        engine.state.config.tick_rate_ms = 100.0;
        engine.state.config.max_troops_base = 100.0;
        engine.state.config.max_troops_scale = 100.0;
        
        engine.execute_income();
        let p = engine.state.player(1).unwrap();
        
        // per_tick(100.0) = 100.0 * (100.0/1000.0) * 4.0 = 40.0
        assert_eq!(p.troops, 40.0);
    }
}
