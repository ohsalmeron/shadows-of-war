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

            let t_f64 = tiles_owned as f64;
            // OpenFront uses tiles^0.6 for max troops: 2 * (tiles^0.6 * 1000 + 50000)
            // Which is tiles^0.6 * 2000 + 100000. We map config variables to it.
            let max_troops_bonus = libm::pow(t_f64, 0.6);

            player.max_troops = config.max_troops_base
                + max_troops_bonus * config.max_troops_scale
                + agg.city_levels as f64 * config.city_max_troops_per_level;
            
            if player.player_type == crate::player::PlayerType::Bot {
                player.max_troops /= 3.0;
            } else if player.player_type == crate::player::PlayerType::Nation {
                player.max_troops *= 0.75; // Medium difficulty
            }

            // OpenFront 1:1 Income Rate
            // toAdd = 10 + Math.pow(troops, 0.73) / 4
            let mut to_add = 10.0 + libm::pow(safe_troops, 0.73) / 4.0;
            
            // ratio = 1 - troops / max
            let ratio = (1.0 - safe_troops / player.max_troops).max(0.0);
            to_add *= ratio;

            if player.player_type == crate::player::PlayerType::Bot {
                to_add *= 0.5;
            }

            // OpenFront runs at 10 TPS (100ms per tick)
            // Scale the per-tick addition by the actual tick rate to maintain real-time parity.
            let of_tick_ratio = config.tick_rate_ms as f64 / 100.0;
            let raw_income = to_add * of_tick_ratio;

            let factory_extra = (agg.factory_levels as f64 * config.factory_income_bonus_per_level)
                .min(config.factory_income_bonus_cap - 1.0);
            let factory_mult = 1.0 + factory_extra;
            
            // Note: OpenFront does not have global_speed_multiplier, but we keep it here to allow engine scaling.
            let income = raw_income * factory_mult * config.global_speed_multiplier;

            player.troops = (safe_troops + income).min(player.max_troops);

            let safe_gold = player.gold.max(0.0);

            let mut gold_base = config.gold_base_income;
            if player.player_type == crate::player::PlayerType::Bot {
                gold_base *= 0.5; // Tribes generate 50% less gold than Nations/Humans
            }

            let mut gold_income =
                gold_base + agg.city_levels as f64 * config.gold_income_per_city_level;
            gold_income *= config.global_speed_multiplier;
            player.gold = safe_gold + gold_income;
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
        let ticks_per_second = 1000.0_f64 / cfg.tick_rate_ms as f64;
        let base_income_per_tick = p.max_troops / (cfg.troop_fill_time_seconds.max(0.1) * ticks_per_second);
        
        let uncapped = base_income_per_tick * (1.0 + 20.0 * cfg.factory_income_bonus_per_level);
        let actual_gain = p.troops - low;
        assert!(
            actual_gain <= uncapped + 0.001,
            "income should be capped by FACTORY_INCOME_BONUS_CAP"
        );
        assert!(
            actual_gain <= base_income_per_tick * cfg.factory_income_bonus_cap + 0.02,
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
        let g = engine.state.config.gold_base_income * engine.state.config.global_speed_multiplier;
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
        let expected = (cfg.gold_base_income + 3.0 * cfg.gold_income_per_city_level)
            * cfg.global_speed_multiplier;
        assert!((p.gold - expected).abs() < 0.001, "gold={}", p.gold);
    }

    #[test]
    fn troop_fill_time_seconds_doubles_troop_gain_not_gold() {
        let mut engine_base = engine_one_player(46, 100, 50.0, 200.0);
        engine_base.execute_income();
        let p_base = engine_base.state.player(1).unwrap();
        let gain_base = p_base.troops - 50.0;
        let gold_base = p_base.gold;

        let mut cfg = crate::game_config::GameConfig::default();
        cfg.troop_fill_time_seconds /= 2.0; // Halving the fill time should double the income
        let mut game = GameState::new(46, 8, 8, cfg.clone());
        game.phase = GamePhase::Playing;
        game.players
            .push(Player::new_human(1, "p".into(), [1.0, 0.0, 0.0], &cfg));
        game.player_lookup = vec![None, Some(0)];
        if let Some(p) = game.player_mut(1) {
            p.tile_count = 100;
            p.troops = 50.0;
            p.gold = 200.0;
        }
        let mut engine_fast = SowEngine::new(game, WaterComponents::default());
        engine_fast.execute_income();
        let p_fast = engine_fast.state.player(1).unwrap();
        let gain_fast = p_fast.troops - 50.0;

        assert!(
            (gain_fast - 2.0 * gain_base).abs() < 0.02,
            "gain_fast={} gain_base={}",
            gain_fast,
            gain_base
        );
        assert!(
            (p_fast.gold - gold_base).abs() < 0.001,
            "gold should ignore troop_fill_time_seconds: {} vs {}",
            p_fast.gold,
            gold_base
        );
    }
}
