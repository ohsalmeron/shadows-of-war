use crate::building::BuildingAggregate;
use crate::game_config::GameConfig;
use crate::player::Leader;

const ARMORY_TROOP_INCOME: f64 = 80.0;
const FOUNDRY_GOLD_INCOME: f64 = 100.0;

#[inline]
fn territory_rate(tiles: u32, amount: f64, interval: u32) -> f64 {
    if interval == 0 {
        return 0.0;
    }
    tiles as f64 / interval as f64 * amount
}

/// Per-second troop income before bot penalty and before `per_tick()` scaling.
pub fn troop_income_per_second(
    tiles_owned: u32,
    agg: BuildingAggregate,
    leader: Leader,
    cfg: &GameConfig,
) -> f64 {
    let sun_tzu_mult = if leader == Leader::SunTzu { 1.20 } else { 1.0 };
    let ragnar_mult = if leader == Leader::Ragnar { 1.50 } else { 1.0 };
    let vercingetorix_mult = if leader == Leader::Vercingetorix {
        1.50
    } else {
        1.0
    };

    cfg.troop_base_income
        + cfg.city_troop_income * agg.city_levels as f64 * vercingetorix_mult
        + ARMORY_TROOP_INCOME * agg.armory_levels as f64 * sun_tzu_mult
        + cfg.port_troop_income * agg.port_levels as f64 * ragnar_mult
        + territory_rate(
            tiles_owned,
            cfg.territory_troop_amount,
            cfg.territory_troop_tiles,
        )
}

/// Per-second gold income before bot penalty and before `per_tick()` scaling.
pub fn gold_income_per_second(
    tiles_owned: u32,
    agg: BuildingAggregate,
    leader: Leader,
    cfg: &GameConfig,
) -> f64 {
    let cleo_mult = if leader == Leader::Cleopatra {
        1.50
    } else {
        1.0
    };
    let boudica_mult = if leader == Leader::Boudica { 1.50 } else { 1.0 };
    let ragnar_mult = if leader == Leader::Ragnar { 1.50 } else { 1.0 };
    let lady_six_sky_mult = if leader == Leader::LadySixSky {
        1.50
    } else {
        1.0
    };

    let factory_gold = agg.factory_levels as f64 * cfg.factory_gold_income * lady_six_sky_mult;

    cfg.gold_base_income
        + cfg.city_gold_income * agg.city_levels as f64 * boudica_mult
        + FOUNDRY_GOLD_INCOME * agg.foundry_levels as f64 * cleo_mult
        + cfg.port_gold_income * agg.port_levels as f64 * ragnar_mult
        + factory_gold
        + territory_rate(
            tiles_owned,
            cfg.territory_gold_amount,
            cfg.territory_gold_tiles,
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::building::BuildingAggregate;

    fn default_cfg() -> GameConfig {
        GameConfig::default()
    }

    #[test]
    fn zero_tiles_base_only() {
        let cfg = default_cfg();
        let agg = BuildingAggregate::default();
        assert_eq!(
            troop_income_per_second(0, agg, Leader::Caesar, &cfg),
            cfg.troop_base_income
        );
        assert_eq!(
            gold_income_per_second(0, agg, Leader::Caesar, &cfg),
            cfg.gold_base_income
        );
    }

    #[test]
    fn territory_scaling_at_defaults() {
        let cfg = default_cfg();
        let agg = BuildingAggregate::default();
        assert_eq!(
            gold_income_per_second(400, agg, Leader::Caesar, &cfg),
            cfg.gold_base_income + 100.0
        );
        assert_eq!(
            troop_income_per_second(400, agg, Leader::Caesar, &cfg),
            cfg.troop_base_income + 50.0
        );
    }

    #[test]
    fn city_troop_income_from_config() {
        let mut cfg = default_cfg();
        cfg.city_troop_income = 25.0;
        let agg = BuildingAggregate {
            city_levels: 2,
            ..Default::default()
        };
        let rate = troop_income_per_second(0, agg, Leader::Caesar, &cfg);
        assert_eq!(rate, cfg.troop_base_income + 50.0);
    }

    #[test]
    fn vercingetorix_multiplies_city_troop_only() {
        let cfg = default_cfg();
        let agg = BuildingAggregate {
            city_levels: 2,
            port_levels: 2,
            ..Default::default()
        };
        let base = troop_income_per_second(0, agg, Leader::Caesar, &cfg);
        let verc = troop_income_per_second(0, agg, Leader::Vercingetorix, &cfg);
        let city_bonus = cfg.city_troop_income * 2.0;
        let port_bonus = cfg.port_troop_income * 2.0;
        assert_eq!(base, cfg.troop_base_income + city_bonus + port_bonus);
        assert_eq!(verc, cfg.troop_base_income + city_bonus * 1.5 + port_bonus);
    }
}
