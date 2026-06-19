use crate::building::resolve_structure_spawn_tile;

use crate::game::BuildingKind;
use crate::protocol::GameplayIntent;

use super::profile::{BotDecision, BotDecisionKind};

pub(super) fn bot_structure_target_count(
    kind: BuildingKind,
    city_equivalent: u32,
    bot_iq: u32,
) -> u32 {
    let factor = if bot_iq >= 130 {
        1.0
    } else if bot_iq >= 100 {
        0.5
    } else {
        0.1
    };

    match kind {
        BuildingKind::Bunker => ((city_equivalent as f64) * 0.35 * factor).floor() as u32,
        BuildingKind::City => {
            let base = (city_equivalent.saturating_add(2) as f64) * factor;
            (base.floor() as u32).max(1)
        }
        BuildingKind::Factory => {
            let val = ((city_equivalent as f64) * 0.65 * factor).floor() as u32;
            if factor > 0.4 {
                val.max(1)
            } else {
                val
            }
        }
        BuildingKind::Port => {
            let val = ((city_equivalent as f64) * 0.30 * factor).floor() as u32;
            if factor > 0.4 {
                val.max(1)
            } else {
                val
            }
        }
    }
}

pub(super) fn iq_build_interval_base(iq: u32, bot_id: u16) -> u64 {
    if iq >= 130 {
        if bot_id.is_multiple_of(8) {
            10
        } else {
            match bot_id % 4 {
                0 => 40,
                1 => 60,
                2 => 50,
                _ => 80,
            }
        }
    } else if iq >= 100 {
        match bot_id % 2 {
            0 => 80,
            _ => 100,
        }
    } else {
        match bot_id % 3 {
            0 => 160,
            1 => 140,
            _ => 120,
        }
    }
}

pub(super) fn pick_stack_click_tile(
    buildings: &[crate::building::Building],
    bot_id: u16,
    kind: BuildingKind,
) -> Option<u32> {
    let mut best: Option<(u8, u64, u32)> = None;
    for b in buildings {
        if b.owner_id != bot_id || b.kind != kind || b.under_construction || b.level >= 5 {
            continue;
        }
        let cand = (b.level, b.id, b.tile_idx);
        match best {
            None => best = Some(cand),
            Some((bl, bid, _)) if b.level < bl || (b.level == bl && b.id < bid) => {
                best = Some(cand);
            }
            _ => {}
        }
    }
    best.map(|(_, _, tile)| tile)
}

pub(super) fn stack_build_decision(
    buildings: &[crate::building::Building],
    bot_id: u16,
    kind: BuildingKind,
    player_gold: f64,
    cost: f64,
) -> Option<BotDecision> {
    let stack_tile = pick_stack_click_tile(buildings, bot_id, kind)?;
    if player_gold < cost {
        return None;
    }
    Some(BotDecision {
        bot_id,
        kind: BotDecisionKind::Build,
        intent: GameplayIntent::BuildStructure {
            kind,
            target_tile: stack_tile,
        },
    })
}

pub(super) const PLACEMENT_ATTEMPTS: i32 = 8;

pub(super) struct StructureCandidates<'a> {
    pub(super) border: &'a [u32],
    pub(super) interior: &'a [(i32, i32)],
}

pub(super) fn resolve_structure_from_candidates(
    map: &crate::map::GameMap,
    owner_id: u16,
    kind: BuildingKind,
    candidates: StructureCandidates<'_>,
    existing: &crate::building::BuildingGrid,
    buildings: &[crate::building::Building],
    scratch: &mut crate::engine::PlacementScratch,
) -> Option<u32> {
    let map_w = map.width;
    for &idx in candidates.border {
        if let Some(spawn) =
            resolve_structure_spawn_tile(map, owner_id, kind, idx, existing, buildings, scratch)
        {
            return Some(spawn);
        }
    }
    for &(nx, ny) in candidates.interior {
        if !map.is_valid_coord(nx, ny) {
            continue;
        }
        let (ux, uy) = (nx as u32, ny as u32);
        if map.owner_id(ux, uy) != owner_id {
            continue;
        }
        let idx = uy * map_w + ux;
        if let Some(spawn) =
            resolve_structure_spawn_tile(map, owner_id, kind, idx, existing, buildings, scratch)
        {
            return Some(spawn);
        }
    }
    None
}

/// Cheapest possible gold cost for a building.
#[inline]
pub(super) fn cheapest_gold_cost(cfg: &crate::game_config::GameConfig) -> f64 {
    cfg.cost_city
        .min(cfg.cost_bunker)
        .min(cfg.cost_factory)
        .min(cfg.cost_port)
}
