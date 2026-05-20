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
            self.building_aggregates_dirty = false;
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
                + agg.city_levels as f64 * config.city_max_troops_per_level;

            let ticks_per_second = 1000.0 / config.tick_rate_ms as f64;
            
            let raw_income = (config.troop_base_income / ticks_per_second) * config.global_speed_multiplier;

            let cap_extra = (config.factory_income_bonus_cap - 1.0).max(0.0);
            let factory_extra = (agg.factory_levels as f64 * config.factory_income_bonus_per_level)
                .min(cap_extra);
            let factory_mult = 1.0 + factory_extra;
            let income = raw_income * factory_mult;

            player.troops = (safe_troops + income).min(player.max_troops);

            let safe_gold = player.gold.max(0.0);

            let gold_base = config.gold_base_income;
            let gold_income =
                (gold_base + agg.city_levels as f64 * config.gold_income_per_city_level)
                * config.global_speed_multiplier;
            player.gold = safe_gold + gold_income;

            let iq_gain = (player.iq as f64 / 100.0) * config.global_speed_multiplier;
            player.iq_points = (player.iq_points + iq_gain).min(500.0);
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
        let ticks_per_second = 1000.0 / cfg.tick_rate_ms as f64;
        let raw_income = cfg.troop_base_income / ticks_per_second;
        
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
    fn gold_increments_each_income_tick() {
        let mut engine = engine_one_player(44, 10, 0.0, 100.0);
        engine.execute_income();
        let p = engine.state.player(1).unwrap();
        let delta = p.gold - 100.0;
        let g = engine.state.config.gold_base_income;
        assert!(
            (delta - g).abs() < 0.001,
            "gold delta {} expected {}",
            delta,
            g
        );
    }

    #[test]
    fn gold_income_scales_with_ready_city_levels() {
        let mut engine = engine_one_player(45, 20, 0.0, 0.0);
        engine.buildings.push(Building {
            id: 1,
            owner_id: 1,
            tile_idx: 0,
            kind: BuildingKind::City,
            level: 3,
            under_construction: false,
            ticks_until_complete: 0,
        });
        engine.execute_income();
        let p = engine.state.player(1).unwrap();
        let cfg = &engine.state.config;
        let expected =
            cfg.gold_base_income + 3.0 * cfg.gold_income_per_city_level;
        assert!((p.gold - expected).abs() < 0.001, "gold={}", p.gold);
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
        
        // ticks_per_second = 1000.0 / 100.0 = 10.0
        // raw_income = (100.0 / 10.0) * 4.0 = 40.0
        assert_eq!(p.troops, 40.0);
    }
}
