use super::core::Building;
use crate::config;
use crate::game::BuildingKind;

/// Total effective building count (sum of levels) owned by `owner` of exact `kind`.
/// A stacked level-3 building counts as 3 for cost scaling.
pub fn count_kind(buildings: &[Building], owner_id: u16, kind: BuildingKind) -> u32 {
    buildings
        .iter()
        .filter(|b| b.owner_id == owner_id && b.kind == kind)
        .map(|b| b.level as u32)
        .sum()
}

/// Gold price for a new structure.
#[inline]
pub fn structure_build_cost_gold(
    kind: BuildingKind,
    count: u32,
    cfg: &crate::game_config::GameConfig,
) -> f64 {
    let base_cost = match kind {
        BuildingKind::City => cfg.cost_city,
        BuildingKind::Bunker => cfg.cost_bunker,
        BuildingKind::Factory => cfg.cost_factory,
        BuildingKind::Port => cfg.cost_port,
    };
    let cap_mult = cfg.cost_scale_cap_multiplier.max(1.0);
    let scaled = base_cost * 1.1f64.powi(count as i32);
    scaled.min(base_cost * cap_mult)
}

#[inline]
pub fn structure_kind_enabled(_kind: BuildingKind) -> bool {
    true
}

/// At least one **ready** Port owned by `player_id` (fleet launches require this).
#[inline]
pub fn player_has_completed_port(buildings: &[Building], player_id: u16) -> bool {
    buildings.iter().any(|b| {
        b.owner_id == player_id
            && ((b.kind == BuildingKind::City && !b.under_construction && b.modules.port > 0)
                || (b.kind == BuildingKind::Port && !b.under_construction))
    })
}

pub fn module_upgrade_cost_gold(kind: crate::building::ModuleKind, level: u8) -> f64 {
    let s = config::GOLD_SCALE.max(1.0);
    let base = match kind {
        crate::building::ModuleKind::Port => match level {
            1 => 100_000.0,
            2 => 200_000.0,
            3 => 400_000.0,
            4 => 800_000.0,
            _ => 1_600_000.0,
        },
        crate::building::ModuleKind::Foundry => match level {
            1 => 75_000.0,
            2 => 150_000.0,
            3 => 300_000.0,
            4 => 600_000.0,
            _ => 1_200_000.0,
        },
        crate::building::ModuleKind::Armory => match level {
            1 => 75_000.0,
            2 => 150_000.0,
            3 => 300_000.0,
            4 => 600_000.0,
            _ => 1_200_000.0,
        },
        crate::building::ModuleKind::Intel => match level {
            1 => 50_000.0,
            2 => 100_000.0,
            3 => 200_000.0,
            4 => 400_000.0,
            _ => 800_000.0,
        },
        crate::building::ModuleKind::Arsenal => match level {
            1 => 500_000.0,
            2 => 1_000_000.0,
            _ => 2_000_000.0,
        },
        crate::building::ModuleKind::Shield => match level {
            1 => 100_000.0,
            2 => 200_000.0,
            3 => 400_000.0,
            4 => 800_000.0,
            _ => 1_600_000.0,
        },
    };
    base / s
}
