use super::core::Building;
use crate::config;
use crate::game::BuildingKind;

/// Count buildings owned by `owner` of exact `kind`.
pub fn count_kind(buildings: &[Building], owner_id: u16, kind: BuildingKind) -> u32 {
    buildings
        .iter()
        .filter(|b| b.owner_id == owner_id && b.kind == kind)
        .count() as u32
}

#[inline]
pub fn scaled_pow2_cost(count: u32, base: u64, cap: u64, scale: f64) -> f64 {
    let steps = count.min(20);
    let mut val = base;
    for _ in 0..steps {
        val = val.saturating_mul(2);
        if val >= cap {
            val = cap;
            break;
        }
    }
    (val.min(cap)) as f64 / scale
}

/// Gold price for a new structure.
pub fn structure_build_cost_gold(kind: BuildingKind, owner_id: u16, buildings: &[Building]) -> f64 {
    let s = config::GOLD_SCALE.max(1.0);
    match kind {
        BuildingKind::City => {
            let n = count_kind(buildings, owner_id, BuildingKind::City);
            scaled_pow2_cost(n, 125_000, 1_000_000, s)
        }
        BuildingKind::Bunker => {
            let n = count_kind(buildings, owner_id, BuildingKind::Bunker);
            ((n as f64 + 1.0) * (50_000.0 / s)).min(250_000.0 / s)
        }
    }
}

#[inline]
pub fn structure_kind_enabled(_kind: BuildingKind) -> bool {
    true
}

/// Sum of `level` for ready (not under construction) buildings of `kind` for `owner_id`.
pub fn count_kind_levels(buildings: &[Building], owner_id: u16, kind: BuildingKind) -> u32 {
    buildings
        .iter()
        .filter(|b| b.owner_id == owner_id && b.kind == kind && !b.under_construction)
        .map(|b| b.level as u32)
        .sum()
}

/// At least one **ready** Port owned by `player_id` (fleet launches require this).
#[inline]
pub fn player_has_completed_port(buildings: &[Building], player_id: u16) -> bool {
    buildings
        .iter()
        .any(|b| b.owner_id == player_id && b.kind == BuildingKind::City && !b.under_construction && b.modules.port > 0)
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

pub fn city_upgrade_cost_gold(level: u8) -> f64 {
    let s = config::GOLD_SCALE.max(1.0);
    let base = match level {
        2 => 250_000.0,
        3 => 500_000.0,
        4 => 1_000_000.0,
        _ => 2_000_000.0,
    };
    base / s
}

pub fn bunker_upgrade_cost_gold(level: u8) -> f64 {
    let s = config::GOLD_SCALE.max(1.0);
    let base = match level {
        2 => 100_000.0,
        _ => 200_000.0,
    };
    base / s
}
