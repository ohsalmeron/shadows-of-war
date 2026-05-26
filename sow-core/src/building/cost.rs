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

/// Gold price for a new structure.
#[inline]
pub fn structure_build_cost_gold() -> f64 {
    1000.0
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
