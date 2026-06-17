use crate::building::{
    aggregate_buildings_per_player, resolve_structure_spawn_tile, structure_build_cost_gold,
    structure_kind_enabled,
};
use crate::diplomacy::{
    alliance_propose_roll_cap, is_valid_alliance_target, maybe_betray_for_attack,
    should_reject_traitor_request, ALLIANCE_RENEWAL_WINDOW_TICKS,
};
use crate::engine::SowEngine;
use crate::player::PlayerType;

use crate::game::{BuildingKind, NukeKind};
use crate::protocol::{AttackIntent, GameplayIntent, StampedIntent};
use crate::rng::NextIntExt;
use wyrand::WyRand;

fn bot_structure_target_count(kind: BuildingKind, city_equivalent: u32, bot_iq: u32) -> u32 {
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
        BuildingKind::Factory => ((city_equivalent as f64) * 0.65 * factor).floor() as u32,
        BuildingKind::Port => ((city_equivalent as f64) * 0.30 * factor).floor() as u32,
    }
}

fn iq_build_interval_base(iq: u32, bot_id: u16) -> u64 {
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

fn pick_stack_click_tile(
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

fn stack_build_decision(
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

const PLACEMENT_ATTEMPTS: i32 = 8;

struct StructureCandidates<'a> {
    border: &'a [u32],
    interior: &'a [(i32, i32)],
}

fn resolve_structure_from_candidates(
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
fn cheapest_gold_cost(cfg: &crate::game_config::GameConfig) -> f64 {
    cfg.cost_city
        .min(cfg.cost_bunker)
        .min(cfg.cost_factory)
        .min(cfg.cost_port)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
enum BotDecisionKind {
    Build = 0,
    Attack = 1,
}

#[derive(Clone, Debug)]
struct BotDecision {
    bot_id: u16,
    kind: BotDecisionKind,
    intent: GameplayIntent,
}

/// Describes which behaviours an AI entity should run this tick.
struct AiSlot {
    bot_id: u16,
    is_nation: bool,
    do_attack: bool,
    do_structures: bool,
    profile: BotAiProfile,
}

#[derive(Clone, Copy)]
struct BotAiProfile {
    trigger_ratio: f64,
    reserve_ratio: f64,
    expand_ratio: f64,
    refuse_human_chance: i32,
}

fn get_bot_ai_profile(bot_id: u16, is_nation: bool) -> BotAiProfile {
    let is_smart = is_nation || bot_id.is_multiple_of(100);
    if is_smart && bot_id.is_multiple_of(8) {
        BotAiProfile {
            trigger_ratio: 0.10,
            reserve_ratio: 0.02,
            expand_ratio: 0.02,
            refuse_human_chance: 0,
        }
    } else if is_smart {
        match bot_id % 4 {
            0 => BotAiProfile {
                trigger_ratio: 0.45,
                reserve_ratio: 0.15,
                expand_ratio: 0.15,
                refuse_human_chance: 20,
            },
            1 => BotAiProfile {
                trigger_ratio: 0.65,
                reserve_ratio: 0.40,
                expand_ratio: 0.20,
                refuse_human_chance: 60,
            },
            2 => BotAiProfile {
                trigger_ratio: 0.55,
                reserve_ratio: 0.30,
                expand_ratio: 0.15,
                refuse_human_chance: 40,
            },
            _ => BotAiProfile {
                trigger_ratio: 0.75,
                reserve_ratio: 0.50,
                expand_ratio: 0.25,
                refuse_human_chance: 80,
            },
        }
    } else if bot_id.is_multiple_of(20) {
        BotAiProfile {
            trigger_ratio: 0.45,
            reserve_ratio: 0.15,
            expand_ratio: 0.10,
            refuse_human_chance: 0,
        }
    } else {
        match bot_id % 3 {
            0 => BotAiProfile {
                trigger_ratio: 0.75,
                reserve_ratio: 0.50,
                expand_ratio: 0.20,
                refuse_human_chance: 90,
            },
            1 => BotAiProfile {
                trigger_ratio: 0.65,
                reserve_ratio: 0.35,
                expand_ratio: 0.15,
                refuse_human_chance: 75,
            },
            _ => BotAiProfile {
                trigger_ratio: 0.70,
                reserve_ratio: 0.30,
                expand_ratio: 0.10,
                refuse_human_chance: 85,
            },
        }
    }
}

impl SowEngine {
    /// Unified AI pipeline for both Tribes (`Bot`) and Nations.
    ///
    /// - Builds one combined schedule of all AI entities.
    /// - Every scheduled bot acts each tick (no global budget cap).
    /// - Each bot self-throttles via `iq_build_interval_base` keyed on IQ.
    /// - Uses `placement_scratch.border_scratch` for zero-allocation border scanning.
    pub fn execute_ai_think(&mut self) {
        if self.state.phase != crate::game::GamePhase::Playing {
            return;
        }

        let tick = self.state.tick;

        // ── Build unified schedule ──────────────────────────────────────────
        let mut schedule: Vec<AiSlot> = Vec::new();
        let mut any_structures = false;

        for p in self.state.players.iter() {
            let is_nation = p.player_type == crate::player::PlayerType::Nation;
            let is_bot = p.player_type == crate::player::PlayerType::Bot;
            if (!is_nation && !is_bot) || !p.alive {
                continue;
            }
            let bot_id = p.id;

            let profile = get_bot_ai_profile(bot_id, is_nation);

            // Fast defensive reaction if under attack by a non-ally
            let is_under_attack = p.iq >= 100
                && self
                    .attacks
                    .iter()
                    .any(|att| att.target_owner == bot_id && !p.alliances.contains(&att.owner_id));

            let interval_base = if is_under_attack {
                if p.iq >= 130 {
                    5 // Elite: react in 0.5s - 1.0s (5 - 10 ticks)
                } else {
                    10 // Advanced: react in 1.0s - 2.0s (10 - 20 ticks)
                }
            } else {
                iq_build_interval_base(p.iq, bot_id)
            };

            let mut sched_rng = WyRand::new(
                self.state
                    .seed
                    .wrapping_add(bot_id as u64)
                    .wrapping_add(interval_base),
            );
            let interval = sched_rng
                .next_int(interval_base as i32, (interval_base as i32 * 2).max(1))
                .max(1) as u64;
            let offset = sched_rng.next_int(0, interval as i32) as u64;

            let phase = tick % interval;
            let do_attack = phase == offset;

            let do_structures = if p.iq >= 100 {
                let one_third = (offset + interval / 3) % interval;
                let two_thirds = (offset + (interval * 2) / 3) % interval;
                do_attack || phase == one_third || phase == two_thirds
            } else {
                do_attack
            };

            if !do_attack && !do_structures {
                continue; // Nothing to do this tick for this entity
            }

            if do_structures && p.gold >= cheapest_gold_cost(&self.state.config) {
                any_structures = true;
            }

            schedule.push(AiSlot {
                bot_id,
                is_nation,
                do_attack,
                do_structures,
                profile,
            });
        }

        schedule.sort_unstable_by_key(|s| s.bot_id);
        if schedule.is_empty() {
            return;
        }

        // ── Pre-compute aggregates if any nation wants to build ─────────────
        if any_structures {
            self.refresh_building_grid();
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
        }

        // ── Process all scheduled bots (no global cap) ─────
        let mut decisions: Vec<BotDecision> = Vec::new();

        for slot in &schedule {
            let bot_id = slot.bot_id;

            let bot_iq = {
                let Some(player) = self.state.player(bot_id) else {
                    continue;
                };
                player.iq
            };

            // Smarter AIs are highly efficient and have much lower action costs!
            let (attack_cost, build_cost, alliance_cost, send_cost) = if bot_iq >= 130 {
                (5.0, 5.0, 5.0, 5.0)
            } else if bot_iq >= 100 {
                (5.0, 5.0, 5.0, 999.0)
            } else {
                (10.0, 10.0, 10.0, 999.0)
            };

            // ── Alliance Proposal Evaluation ───────────────────────────────
            let mut proposals_to_accept = Vec::new();
            for prop in &self.alliances_proposed {
                let proposer = prop.proposer;
                let target = prop.target;
                if target == bot_id {
                    let proposer_ok = self
                        .state
                        .player(proposer)
                        .map(|p| p.alive)
                        .unwrap_or(false);
                    if proposer_ok {
                        let is_teammate = {
                            let p_me = self.state.player(bot_id).unwrap();
                            let p_prop = self.state.player(proposer).unwrap();
                            p_me.team.is_some() && p_me.team == p_prop.team
                        };
                        if is_teammate {
                            continue;
                        }
                        let current_points = self.state.player(bot_id).unwrap().iq_points;
                        if current_points >= alliance_cost {
                            let mut accept = false;
                            let tick = self.current_tick_u32();
                            let traitor_roll = self
                                .state
                                .player_mut(bot_id)
                                .unwrap()
                                .bot_rng
                                .next_int(0, 100);
                            if bot_iq >= 130 {
                                if let (Some(p_me), Some(p_prop)) =
                                    (self.state.player(bot_id), self.state.player(proposer))
                                {
                                    if should_reject_traitor_request(p_prop, tick, traitor_roll) {
                                        accept = false;
                                    } else {
                                        let me_troops = p_me.troops.max(1.0);
                                        let me_tiles = p_me.tile_count.max(1);
                                        if p_prop.troops >= me_troops * 0.8
                                            && p_prop.tile_count >= (me_tiles as f64 * 0.8) as u32
                                        {
                                            accept = true;
                                        }
                                    }
                                }
                            } else if bot_iq >= 100 {
                                if let (Some(p_me), Some(p_prop)) =
                                    (self.state.player(bot_id), self.state.player(proposer))
                                {
                                    if should_reject_traitor_request(p_prop, tick, traitor_roll) {
                                        accept = false;
                                    } else {
                                        let me_troops = p_me.troops.max(1.0);
                                        let me_tiles = p_me.tile_count.max(1);
                                        if p_prop.troops >= me_troops * 0.5
                                            && p_prop.tile_count >= (me_tiles as f64 * 0.5) as u32
                                        {
                                            accept = true;
                                        }
                                    }
                                }
                            } else if let Some(p_prop) = self.state.player(proposer) {
                                accept = !should_reject_traitor_request(p_prop, tick, traitor_roll);
                            } else {
                                accept = false;
                            }

                            if accept {
                                proposals_to_accept.push(proposer);
                                if let Some(p_me) = self.state.player_mut(bot_id) {
                                    p_me.iq_points -= alliance_cost;
                                }
                            }
                        }
                    }
                }
            }
            for proposer in proposals_to_accept {
                decisions.push(BotDecision {
                    bot_id,
                    kind: BotDecisionKind::Build,
                    intent: GameplayIntent::AcceptAlliance {
                        target_player: proposer,
                    },
                });
            }

            // ── Respond to Resource Requests ─────────────────────────
            // Same cooldown window as proactive sharing — no spam
            {
                let share_interval = {
                    let mut rng = WyRand::new(
                        self.state
                            .seed
                            .wrapping_add(bot_id as u64)
                            .wrapping_add(7919),
                    );
                    rng.next_int(400, 1200) as u64
                };
                if self.state.tick > share_interval && self.state.tick % share_interval < 2 {
                    for req in &self.resource_requests_proposed {
                        if req.target != bot_id {
                            continue;
                        }
                        let requester = req.proposer;
                        let is_ally = self
                            .state
                            .player(bot_id)
                            .map(|p| p.alliances.contains(&requester))
                            .unwrap_or(false);
                        if !is_ally {
                            continue;
                        }

                        let accept = if bot_iq >= 130 {
                            if let (Some(p_me), Some(p_req)) =
                                (self.state.player(bot_id), self.state.player(requester))
                            {
                                p_me.troops >= p_req.troops * 2.0
                            } else {
                                false
                            }
                        } else if bot_iq >= 100 {
                            self.state
                                .player(bot_id)
                                .map(|p| p.troops > p.max_troops * 0.4 || p.gold > 100_000.0)
                                .unwrap_or(false)
                        } else {
                            // Low IQ: always accept
                            true
                        };

                        if accept {
                            let current_points = self.state.player(bot_id).unwrap().iq_points;
                            if current_points >= send_cost {
                                if let Some(p_me) = self.state.player_mut(bot_id) {
                                    p_me.iq_points -= send_cost;
                                }
                                decisions.push(BotDecision {
                                    bot_id,
                                    kind: BotDecisionKind::Build,
                                    intent: GameplayIntent::AcceptResourceRequest {
                                        target_player: requester,
                                    },
                                });
                            }
                        } else {
                            // Explicit rejection!
                            decisions.push(BotDecision {
                                bot_id,
                                kind: BotDecisionKind::Build,
                                intent: GameplayIntent::RejectResourceRequest {
                                    target_player: requester,
                                },
                            });
                        }
                        break;
                    }
                }
            }

            // ── Proactive Resource Requesting (Bots requesting help, prioritized human first, 10% default) ──
            {
                let req_interval = {
                    let mut rng = WyRand::new(
                        self.state
                            .seed
                            .wrapping_add(bot_id as u64)
                            .wrapping_add(1337),
                    );
                    rng.next_int(600, 1200) as u64 // every 30-60 seconds
                };
                if self.state.tick > req_interval && self.state.tick.is_multiple_of(req_interval) {
                    if let Some(p_me) = self.state.player(bot_id) {
                        let is_weak = p_me.troops < p_me.max_troops * 0.25 || p_me.gold < 30_000.0;
                        if is_weak && !p_me.alliances.is_empty() {
                            // Find target to ask: Prioritize human allies first
                            let mut target_id = None;
                            for &ally_id in &p_me.alliances {
                                if let Some(p_ally) = self.state.player(ally_id) {
                                    if p_ally.alive
                                        && p_ally.player_type == crate::player::PlayerType::Human
                                    {
                                        target_id = Some(ally_id);
                                        break;
                                    }
                                }
                            }
                            // Fallback to bot allies
                            if target_id.is_none() {
                                for &ally_id in &p_me.alliances {
                                    if let Some(p_ally) = self.state.player(ally_id) {
                                        if p_ally.alive {
                                            target_id = Some(ally_id);
                                            break;
                                        }
                                    }
                                }
                            }

                            if let Some(target) = target_id {
                                if let Some(p_target) = self.state.player(target) {
                                    // Ask for 10% of what they have
                                    let ask_gold = (p_target.gold * 0.10).floor().max(0.0);
                                    let ask_troops = (p_target.troops * 0.10).floor().max(0.0);
                                    if ask_gold > 0.0 || ask_troops > 0.0 {
                                        decisions.push(BotDecision {
                                            bot_id,
                                            kind: BotDecisionKind::Build,
                                            intent: GameplayIntent::RequestResources {
                                                target_player: target,
                                                gold: ask_gold,
                                                troops: ask_troops,
                                            },
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ── Proactive Resource Sharing (IQ 120+, every 20-60s) ────
            if bot_iq >= 120 {
                let share_interval = {
                    let mut rng = WyRand::new(
                        self.state
                            .seed
                            .wrapping_add(bot_id as u64)
                            .wrapping_add(7919),
                    );
                    rng.next_int(400, 1200) as u64
                };
                if self.state.tick > share_interval
                    && self.state.tick.is_multiple_of(share_interval)
                {
                    let share_chance = (60i32 - bot_iq as i32 / 3).clamp(10, 50);
                    let roll = self
                        .state
                        .player_mut(bot_id)
                        .unwrap()
                        .bot_rng
                        .next_int(0, 100);
                    if roll < share_chance {
                        let current_points = self.state.player(bot_id).unwrap().iq_points;
                        if current_points >= send_cost {
                            let mut ally_to_help = None;
                            if let Some(p_me) = self.state.player(bot_id) {
                                let has_surplus =
                                    p_me.gold > 100_000.0 || p_me.troops > p_me.max_troops * 0.4;
                                if has_surplus {
                                    for &ally_id in &p_me.alliances {
                                        if let Some(p_ally) = self.state.player(ally_id) {
                                            if p_ally.alive
                                                && (p_ally.troops < p_ally.max_troops * 0.3
                                                    || p_ally.troops <= 500.0)
                                            {
                                                ally_to_help = Some(ally_id);
                                                break;
                                            }
                                        }
                                    }
                                }
                            }
                            if let Some(ally_id) = ally_to_help {
                                // Random % of troops (5-15%) and gold (10-25%)
                                let p_me = self.state.player_mut(bot_id).unwrap();
                                let troop_pct = p_me.bot_rng.next_int(5, 15) as f64 / 100.0;
                                let gold_pct = p_me.bot_rng.next_int(10, 25) as f64 / 100.0;
                                let troops_avail = p_me.troops;
                                let gold_avail = p_me.gold;
                                p_me.iq_points -= send_cost;
                                let send_troops = (troops_avail * troop_pct).floor().max(50.0);
                                let send_gold = (gold_avail * gold_pct).floor();
                                decisions.push(BotDecision {
                                    bot_id,
                                    kind: BotDecisionKind::Build,
                                    intent: GameplayIntent::SendResources {
                                        target_player: ally_id,
                                        gold: send_gold,
                                        troops: send_troops,
                                    },
                                });
                            }
                        }
                    }
                }
            }

            // ── Zero-allocation border and neighbor scanning ──────────────
            self.placement_scratch.border_scratch.clear();
            if let Some(player) = self.state.player(bot_id) {
                self.placement_scratch
                    .border_scratch
                    .extend(player.border_coords(self.state.map.width));
            }

            let mut neighbor_players = Vec::new();
            let mut has_neutral = false;

            if !self.placement_scratch.border_scratch.is_empty() {
                let border_len = self.placement_scratch.border_scratch.len();
                let max_scan = border_len.min(256);
                let start_idx = {
                    let p_mut = self.state.player_mut(bot_id).unwrap();
                    p_mut.bot_rng.next_int(0, border_len as i32) as usize
                };
                for si in 0..max_scan {
                    let b_idx = (start_idx + si) % border_len;
                    let (bx, by) = self.placement_scratch.border_scratch[b_idx];
                    self.state.map.for_each_neighbor(bx, by, |nx, ny| {
                        let owner = self.state.map.owner_id(nx, ny);
                        if owner != bot_id {
                            if owner == 0 {
                                if self.state.map.terrain[self.state.map.ref_id(nx, ny)].is_land() {
                                    has_neutral = true;
                                }
                            } else {
                                neighbor_players.push(owner);
                            }
                        }
                    });
                }
                neighbor_players.sort_unstable();
                neighbor_players.dedup();
            }

            // ── Propose Alliances ──────────────────────────────────────────
            let current_points = self.state.player(bot_id).unwrap().iq_points;
            let is_under_attack = self.attacks.iter().any(|att| {
                att.target_owner == bot_id
                    && self
                        .state
                        .player(bot_id)
                        .map(|p| !p.alliances.contains(&att.owner_id))
                        .unwrap_or(true)
            });
            let expand_first = has_neutral && !is_under_attack;

            if current_points >= alliance_cost && !neighbor_players.is_empty() && !expand_first {
                let mut proposed_target = None;
                let (me_alliances, me_troops, me_tile_count) = {
                    let p_me = self.state.player(bot_id).unwrap();
                    (p_me.alliances.clone(), p_me.troops, p_me.tile_count)
                };

                let max_alliances = if bot_iq >= 130 {
                    1
                } else if bot_iq >= 100 {
                    3
                } else {
                    5
                };

                for &neighbor in &neighbor_players {
                    let (neigh_alive, neigh_troops, neigh_tile_count) =
                        match self.state.player(neighbor) {
                            Some(pn) => (pn.alive, pn.troops, pn.tile_count),
                            None => continue,
                        };
                    if neigh_alive {
                        let is_teammate = {
                            let p_me = self.state.player(bot_id).unwrap();
                            let p_neigh = self.state.player(neighbor).unwrap();
                            p_me.team.is_some() && p_me.team == p_neigh.team
                        };
                        let (is_allied, can_renew) = {
                            let p_me = self.state.player(bot_id).unwrap();
                            let allied = p_me.alliances.contains(&neighbor);
                            let timer = p_me.alliance_timers.get(&neighbor).copied().unwrap_or(0);
                            (allied, allied && timer <= ALLIANCE_RENEWAL_WINDOW_TICKS)
                        };
                        let neigh_type = self
                            .state
                            .player(neighbor)
                            .map(|p| p.player_type)
                            .unwrap_or(PlayerType::Bot);
                        let valid_target = is_valid_alliance_target(bot_id, neighbor, neigh_type);
                        let can_send = self.can_send_alliance_request(bot_id, neighbor);

                        let has_room = me_alliances.len() < max_alliances;
                        let should_propose = (has_room && !is_allied && !is_teammate) || can_renew;

                        if should_propose && valid_target && can_send {
                            let mut meets_threshold = false;
                            let roll = {
                                let p_me = self.state.player_mut(bot_id).unwrap();
                                p_me.bot_rng.next_int(0, 100)
                            };
                            let roll_cap = alliance_propose_roll_cap(bot_id, bot_iq, can_renew);

                            if can_renew {
                                meets_threshold = roll < roll_cap;
                            } else if bot_iq >= 130 {
                                let me_troops_val = me_troops.max(1.0);
                                let me_tiles_val = me_tile_count.max(1);
                                if neigh_troops >= me_troops_val * 0.8
                                    && neigh_tile_count >= (me_tiles_val as f64 * 0.8) as u32
                                {
                                    meets_threshold = roll < roll_cap;
                                }
                            } else if bot_iq >= 100 {
                                let me_troops_val = me_troops.max(1.0);
                                let me_tiles_val = me_tile_count.max(1);
                                if neigh_troops >= me_troops_val * 0.5
                                    && neigh_tile_count >= (me_tiles_val as f64 * 0.5) as u32
                                {
                                    meets_threshold = roll < roll_cap;
                                }
                            } else {
                                meets_threshold = roll < roll_cap;
                            }

                            if meets_threshold {
                                proposed_target = Some(neighbor);
                                break;
                            }
                        }
                    }
                }

                if let Some(neighbor) = proposed_target {
                    if let Some(p_me) = self.state.player_mut(bot_id) {
                        p_me.iq_points -= alliance_cost;
                    }
                    decisions.push(BotDecision {
                        bot_id,
                        kind: BotDecisionKind::Build,
                        intent: GameplayIntent::ProposeAlliance {
                            target_player: neighbor,
                        },
                    });
                }
            }

            // ── Structure building (IQ-scaled, all bots) ─────────────────
            if slot.do_structures {
                let current_points = self.state.player(bot_id).unwrap().iq_points;
                if current_points >= build_cost {
                    let (player_gold, player_tile_count) = {
                        if let Some(player) = self.state.player(bot_id) {
                            (player.gold, player.tile_count)
                        } else {
                            continue;
                        }
                    };
                    if player_gold >= cheapest_gold_cost(&self.state.config) {
                        let agg = self
                            .building_aggregates
                            .get(bot_id as usize)
                            .copied()
                            .unwrap_or_default();
                        let city_equivalent =
                            agg.ready_city_count.max((player_tile_count / 1000).max(1));
                        let build_order = [
                            BuildingKind::Bunker,
                            BuildingKind::City,
                            BuildingKind::Factory,
                            BuildingKind::Port,
                        ];
                        for kind in build_order {
                            if !structure_kind_enabled(kind) {
                                continue;
                            }
                            let owned = agg.total_structures_of_kind(kind);
                            let mut target_count =
                                bot_structure_target_count(kind, city_equivalent, bot_iq);
                            if kind == BuildingKind::Bunker && bot_iq >= 110 {
                                let mut under_attack = false;
                                for att in &self.attacks {
                                    if att.target_owner == bot_id {
                                        under_attack = true;
                                        break;
                                    }
                                }
                                if under_attack {
                                    target_count += 3;
                                }
                            }
                            let total_owned = agg.count_city
                                + agg.count_bunker
                                + agg.count_factory
                                + agg.count_port;
                            let density = total_owned as f32 / player_tile_count.max(1) as f32;
                            let is_density_high = bot_iq >= 110 && density > 1.0 / 600.0;
                            let structure_floor = player_tile_count / 800;
                            let under_structure_floor = total_owned < structure_floor;
                            let wants_new = owned < target_count || under_structure_floor;
                            let cost = structure_build_cost_gold(kind, owned, &self.state.config);

                            if !wants_new || is_density_high {
                                if let Some(d) = stack_build_decision(
                                    &self.buildings,
                                    bot_id,
                                    kind,
                                    player_gold,
                                    cost,
                                ) {
                                    if let Some(p_me) = self.state.player_mut(bot_id) {
                                        p_me.iq_points -= build_cost;
                                    }
                                    decisions.push(d);
                                    break;
                                }
                                if !wants_new {
                                    continue;
                                }
                            }

                            if !wants_new {
                                continue;
                            }
                            if player_gold < cheapest_gold_cost(&self.state.config) {
                                continue;
                            }
                            if self.state.player(bot_id).is_none_or(|p| p.gold < cost) {
                                continue;
                            }
                            let (border_candidates, interior_candidates) = {
                                let p = self.state.player_mut(bot_id).unwrap();
                                let mut border = Vec::new();
                                let border_len = p.border_tiles.count_ones();
                                for _ in 0..PLACEMENT_ATTEMPTS {
                                    if border_len > 0 {
                                        let pick =
                                            p.bot_rng.next_int(0, border_len as i32) as usize;
                                        if let Some(idx) = p.border_tiles.ones().nth(pick) {
                                            border.push(idx);
                                        }
                                    }
                                }
                                let mut interior = Vec::new();
                                if p.tile_count > 500 {
                                    let cx = (p.sum_x / p.tile_count as u64) as i32;
                                    let cy = (p.sum_y / p.tile_count as u64) as i32;
                                    for _ in 0..PLACEMENT_ATTEMPTS {
                                        let dx = p.bot_rng.next_int(-40, 41);
                                        let dy = p.bot_rng.next_int(-40, 41);
                                        interior.push((cx + dx, cy + dy));
                                    }
                                }
                                (border, interior)
                            };
                            if let Some(target_tile) = resolve_structure_from_candidates(
                                &self.state.map,
                                bot_id,
                                kind,
                                StructureCandidates {
                                    border: &border_candidates,
                                    interior: &interior_candidates,
                                },
                                &self.building_grid,
                                &self.buildings,
                                &mut self.placement_scratch,
                            ) {
                                if let Some(p_me) = self.state.player_mut(bot_id) {
                                    p_me.iq_points -= build_cost;
                                }
                                decisions.push(BotDecision {
                                    bot_id,
                                    kind: BotDecisionKind::Build,
                                    intent: GameplayIntent::BuildStructure { kind, target_tile },
                                });
                                break;
                            }

                            // Boxed in: stack on existing (player rules via apply_build_structure_intent)
                            if let Some(d) = stack_build_decision(
                                &self.buildings,
                                bot_id,
                                kind,
                                player_gold,
                                cost,
                            ) {
                                if let Some(p_me) = self.state.player_mut(bot_id) {
                                    p_me.iq_points -= build_cost;
                                }
                                decisions.push(d);
                                break;
                            }
                        }
                    }
                }
            }

            // ── Attack logic (both Bots and Nations) ────────────────────
            if slot.do_attack {
                let current_points = self.state.player(bot_id).unwrap().iq_points;
                if current_points >= attack_cost {
                    let tick = self.current_tick_u32();
                    let betray_cd = self.alliance_betray_cooldown_until.get(&bot_id).copied();
                    let bordering_count = neighbor_players.len();
                    let allied_on_border: Vec<u16> = {
                        let p_me = self.state.player(bot_id).unwrap();
                        p_me.alliances
                            .iter()
                            .copied()
                            .filter(|id| neighbor_players.contains(id))
                            .collect()
                    };
                    let mut betray_then_attack: Option<u16> = None;
                    for ally_id in allied_on_border {
                        let should_betray = {
                            let p_me = self.state.player(bot_id).unwrap();
                            let Some(p_ally) = self.state.player(ally_id) else {
                                continue;
                            };
                            let mut rng = WyRand::new(
                                self.state
                                    .seed
                                    .wrapping_add(bot_id as u64)
                                    .wrapping_add(ally_id as u64)
                                    .wrapping_add(tick as u64),
                            );
                            maybe_betray_for_attack(
                                p_me,
                                p_ally,
                                bordering_count,
                                tick,
                                betray_cd,
                                &mut rng,
                            )
                        };
                        if should_betray {
                            betray_then_attack = Some(ally_id);
                            break;
                        }
                    }
                    if let Some(ally_id) = betray_then_attack {
                        if let Some(p_me) = self.state.player_mut(bot_id) {
                            if p_me.iq_points >= alliance_cost {
                                p_me.iq_points -= alliance_cost;
                            }
                        }
                        decisions.push(BotDecision {
                            bot_id,
                            kind: BotDecisionKind::Build,
                            intent: GameplayIntent::BreakAlliance {
                                target_player: ally_id,
                            },
                        });
                    }

                    let trigger_ratio = slot.profile.trigger_ratio;
                    let reserve_ratio = slot.profile.reserve_ratio;
                    let expand_ratio = slot.profile.expand_ratio;
                    let refuse_human_chance = slot.profile.refuse_human_chance;

                    let (troops, max_troops) = {
                        let player = self.state.player(bot_id).unwrap();
                        (player.troops, player.max_troops)
                    };

                    let targets: Vec<u16> = neighbor_players
                        .iter()
                        .copied()
                        .filter(|&id| {
                            if betray_then_attack == Some(id) {
                                return true;
                            }
                            if let Some(p_me) = self.state.player(bot_id) {
                                !p_me.alliances.contains(&id)
                            } else {
                                true
                            }
                        })
                        .collect();

                    let is_mfo = slot.is_nation && bot_id.is_multiple_of(8);
                    let has_port =
                        crate::building::cost::player_has_completed_port(&self.buildings, bot_id);
                    let mut revenge_choice = None;
                    if is_mfo {
                        let mut max_attacker_troops = -1.0;
                        for att in &self.attacks {
                            if att.target_owner == bot_id && targets.contains(&att.owner_id) {
                                if let Some(p_att) = self.state.player(att.owner_id) {
                                    if let Some(p_me) = self.state.player(bot_id) {
                                        if p_me.troops > p_att.troops
                                            && p_att.troops > max_attacker_troops
                                        {
                                            max_attacker_troops = p_att.troops;
                                            revenge_choice = Some(att.owner_id);
                                        }
                                    }
                                }
                            }
                        }
                    }

                    let mut launched_fleet = false;
                    if is_mfo
                        && has_port
                        && troops >= max_troops * 0.20
                        && (targets.is_empty() || self.state.tick.is_multiple_of(12))
                    {
                        let mut best_target_p_id = None;
                        let mut min_troops = f64::MAX;
                        for p in &self.state.players {
                            if p.alive && p.id != bot_id {
                                let is_friendly = {
                                    let p_me = self.state.player(bot_id).unwrap();
                                    p_me.alliances.contains(&p.id)
                                        || (p_me.team.is_some() && p_me.team == p.team)
                                };
                                if !is_friendly
                                    && p.troops < min_troops
                                    && !p.border_tiles.is_empty()
                                {
                                    min_troops = p.troops;
                                    best_target_p_id = Some(p.id);
                                }
                            }
                        }
                        if let Some(target_p_id) = best_target_p_id {
                            let mut route_resolved = false;
                            let mut target_tile_opt = None;
                            {
                                let target_p = self.state.player(target_p_id).unwrap();
                                let border_len = target_p.border_tiles.count_ones();
                                if border_len > 0 {
                                    let pick_idx = (self.state.tick as usize) % border_len;
                                    if let Some(t_tile) = target_p.border_tiles.ones().nth(pick_idx)
                                    {
                                        let border_tiles =
                                            &self.state.player(bot_id).unwrap().border_tiles;
                                        if let Ok(_route) = crate::warp_fleet::resolve_fleet_route(
                                            &self.state.map,
                                            &self.water,
                                            &mut self.path_scratch,
                                            bot_id,
                                            target_p_id,
                                            t_tile,
                                            border_tiles,
                                            Some(&target_p.border_tiles),
                                        ) {
                                            route_resolved = true;
                                            target_tile_opt = Some(t_tile);
                                        }
                                    }
                                }
                            }
                            if route_resolved {
                                let p_send = (troops - (max_troops * 0.05)).max(0.0);
                                if p_send >= self.state.config.attack_cost_neutral {
                                    if let Some(p_me) = self.state.player_mut(bot_id) {
                                        p_me.iq_points = (p_me.iq_points - attack_cost).max(0.0);
                                    }
                                    decisions.push(BotDecision {
                                        bot_id,
                                        kind: BotDecisionKind::Attack,
                                        intent: GameplayIntent::LaunchFleet {
                                            target_tile: target_tile_opt.unwrap(),
                                            troops: Some(p_send),
                                        },
                                    });
                                    launched_fleet = true;
                                }
                            }
                        }
                    }

                    if launched_fleet {
                        continue;
                    }

                    let mut defender_target = None;
                    if bot_iq >= 100 {
                        let mut largest_attack = 0.0;
                        for att in &self.attacks {
                            // targets already excludes allies and teammates
                            if att.target_owner == bot_id
                                && targets.contains(&att.owner_id)
                                && att.troops > largest_attack
                            {
                                largest_attack = att.troops;
                                defender_target = Some(att.owner_id);
                            }
                        }
                    }

                    let (target_owner, is_neutral) = if let Some(attacker_id) = defender_target {
                        (attacker_id, false)
                    } else if is_mfo && !targets.is_empty() {
                        let chosen_target = if let Some(att_id) = revenge_choice {
                            att_id
                        } else {
                            let mut best_target = targets[0];
                            let mut min_troops = f64::MAX;
                            for &t_id in &targets {
                                if let Some(p_t) = self.state.player(t_id) {
                                    if p_t.troops < min_troops {
                                        min_troops = p_t.troops;
                                        best_target = t_id;
                                    }
                                }
                            }
                            best_target
                        };
                        (chosen_target, false)
                    } else if has_neutral {
                        (0, true)
                    } else if targets.is_empty() {
                        if slot.is_nation {
                            self.maybe_launch_nuke(bot_id, &mut decisions, bot_iq, &targets);
                        }
                        continue;
                    } else {
                        let target_owner;
                        if bot_iq >= 130 {
                            let mut best_target = targets[0];
                            for &t_id in &targets {
                                if let (Some(p_t), Some(p_b)) =
                                    (self.state.player(t_id), self.state.player(best_target))
                                {
                                    let t_is_tribe = p_t.player_type
                                        == crate::player::PlayerType::Bot
                                        && t_id % 100 != 0;
                                    let b_is_tribe = p_b.player_type
                                        == crate::player::PlayerType::Bot
                                        && !best_target.is_multiple_of(100);

                                    if (t_is_tribe && !b_is_tribe)
                                        || (t_is_tribe == b_is_tribe && p_t.troops < p_b.troops)
                                    {
                                        best_target = t_id;
                                    }
                                }
                            }
                            target_owner = best_target;
                        } else if bot_iq >= 100 {
                            let mut weakest = targets[0];
                            for &t_id in &targets {
                                if let (Some(p_t), Some(p_w)) =
                                    (self.state.player(t_id), self.state.player(weakest))
                                {
                                    if p_t.troops < p_w.troops {
                                        weakest = t_id;
                                    }
                                }
                            }
                            target_owner = weakest;
                        } else {
                            let p_mut = self.state.player_mut(bot_id).unwrap();
                            let roll = p_mut.bot_rng.next_int(0, targets.len() as i32) as usize;
                            target_owner = targets[roll];
                        }

                        let is_target_human = self
                            .state
                            .player(target_owner)
                            .map(|pl| pl.is_human())
                            .unwrap_or(false);
                        let refuse_roll = self
                            .state
                            .player_mut(bot_id)
                            .unwrap()
                            .bot_rng
                            .next_int(0, 100);
                        if is_target_human && refuse_roll < refuse_human_chance {
                            continue;
                        }

                        (target_owner, false)
                    };

                    let is_defending = defender_target.is_some();
                    if is_neutral || is_defending || troops >= max_troops * trigger_ratio {
                        let reserve = max_troops
                            * if is_neutral {
                                expand_ratio
                            } else if is_defending {
                                // Desperate defense: keep only half of standard reserve ratio
                                reserve_ratio * 0.5
                            } else {
                                reserve_ratio
                            };
                        let is_standard_bot = !slot.is_nation && !bot_id.is_multiple_of(100);
                        let p_send = if is_standard_bot && !is_neutral && !is_defending {
                            (troops / 4.0).max(0.0)
                        } else {
                            (troops - reserve).max(0.0)
                        };
                        if p_send >= self.state.config.attack_cost_neutral {
                            if let Some(p_me) = self.state.player_mut(bot_id) {
                                p_me.iq_points -= attack_cost;
                            }
                            decisions.push(BotDecision {
                                bot_id,
                                kind: BotDecisionKind::Attack,
                                intent: GameplayIntent::Attack(AttackIntent {
                                    target_owner,
                                    troops: Some(p_send),
                                }),
                            });
                        }
                    }
                    if slot.is_nation {
                        self.maybe_launch_nuke(bot_id, &mut decisions, bot_iq, &targets);
                    }
                }
            }
        }

        // ── Apply decisions deterministically ───────────────────────────────
        decisions.sort_by_key(|d| (d.bot_id, d.kind));
        for (intent_index, d) in decisions.into_iter().enumerate() {
            let stamped = StampedIntent {
                player_id: d.bot_id,
                intent: d.intent,
            };
            self.apply_stamped_intent(&stamped, intent_index as u32);
        }
    }

    fn maybe_launch_nuke(
        &mut self,
        bot_id: u16,
        decisions: &mut Vec<BotDecision>,
        bot_iq: u32,
        targets: &[u16],
    ) {
        if bot_iq < 100 {
            return;
        }

        if self.building_aggregates_dirty {
            self.building_aggregates = crate::building::core::aggregate_buildings_per_player(
                self.buildings.iter().copied(),
                self.state.players.len(),
            );
            self.building_aggregates_dirty = false;
        }

        let agg = self
            .building_aggregates
            .get(bot_id as usize)
            .copied()
            .unwrap_or_default();
        if agg.arsenal_levels == 0 {
            return;
        }

        let mut has_silo = false;
        for b in &self.buildings {
            if b.owner_id == bot_id
                && b.kind == BuildingKind::City
                && b.modules.arsenal > 0
                && !b.under_construction
                && self.silo_cooldowns.get(&b.id).copied().unwrap_or(0) == 0
            {
                has_silo = true;
            }
        }
        if !has_silo {
            return;
        }

        let Some(player) = self.state.player(bot_id) else {
            return;
        };
        let cost = self.state.config.nuke_cost;
        let perceived_cost = cost;

        if player.gold < perceived_cost {
            return;
        }
        let kind = NukeKind::AtomBomb;

        // Find crown leader
        let mut leader = 0;
        let mut leader_tiles = 0;
        for p in &self.state.players {
            if p.alive && p.tile_count > leader_tiles {
                leader = p.id;
                leader_tiles = p.tile_count;
            }
        }

        let mut primary_target = targets.first().copied().unwrap_or(0);
        if targets.contains(&leader) {
            primary_target = leader;
        }

        if primary_target == 0 || primary_target == bot_id {
            return;
        }

        // Find best structure to nuke
        let mut best_score = -1.0;
        let mut best_tile = 0;

        for b in &self.buildings {
            if b.owner_id != primary_target || b.under_construction {
                continue;
            }
            let mut score = match b.kind {
                BuildingKind::City => {
                    let mut val = 25000.0;
                    if b.modules.arsenal > 0 {
                        val += 25000.0;
                    }
                    if b.modules.shield > 0 {
                        val += 15000.0;
                    }
                    val
                }
                BuildingKind::Bunker => 5000.0 * (b.level as f64),
                BuildingKind::Factory => 15000.0 * (b.level as f64),
                BuildingKind::Port => 10000.0 * (b.level as f64),
            };

            let bx = b.tile_idx % self.state.map.width;
            let by = b.tile_idx / self.state.map.width;

            // SAM avoidance
            let mut sam_covered = false;
            for b2 in &self.buildings {
                if b2.kind == crate::game::BuildingKind::City
                    && b2.modules.shield > 0
                    && !b2.under_construction
                    && b2.owner_id != bot_id
                {
                    let mut is_ally = false;
                    if let Some(p1) = self.state.player(bot_id) {
                        if p1.alliances.contains(&b2.owner_id) {
                            is_ally = true;
                        }
                    }
                    if !is_ally {
                        let (sx, sy) = (
                            b2.tile_idx % self.state.map.width,
                            b2.tile_idx / self.state.map.width,
                        );
                        if (bx as i32 - sx as i32).abs() + (by as i32 - sy as i32).abs() <= 48 {
                            sam_covered = true;
                            break;
                        }
                    }
                }
            }
            if sam_covered {
                score -= 100000.0;
            }

            // Target dedup
            for (to, tt, _) in &self.recent_nuke_targets {
                if *to == primary_target && *tt == b.tile_idx {
                    score -= 50000.0;
                }
            }

            if score > best_score {
                best_score = score;
                best_tile = b.tile_idx;
            }
        }

        if best_score > 0.0 {
            self.recent_nuke_targets
                .push((primary_target, best_tile, self.state.tick));
            decisions.push(BotDecision {
                bot_id,
                kind: BotDecisionKind::Attack,
                intent: GameplayIntent::LaunchNuke {
                    kind,
                    target_tile: best_tile,
                },
            });
            let p_me = self.state.player_mut(bot_id).unwrap();
            p_me.iq_points -= 15.0; // Assume 15 points
        }
    }
}

#[cfg(test)]
mod bot_iq_alliance_tests {
    use super::bot_structure_target_count;
    use crate::engine::SowEngine;
    use crate::game::{BuildingKind, GamePhase, GameState};
    use crate::player::Player;
    use crate::water_components::WaterComponents;

    fn test_engine_two_players(seed: u64) -> SowEngine {
        let mut game = GameState::new(seed, 8, 8, crate::game_config::GameConfig::default());
        game.phase = GamePhase::Playing;

        // Player 1 (Bot, IQ 135 - High IQ)
        let mut p1 = Player::new_bot(
            1,
            "Bot1".into(),
            [1.0, 0.0, 0.0],
            &crate::game_config::GameConfig::default(),
        );
        p1.iq = 135;
        p1.iq_points = 50.0;
        p1.troops = 1000.0;
        p1.max_troops = 1500.0;
        p1.gold = 300_000.0;
        p1.tile_count = 10;
        p1.border_insert(0); // Tile (0, 0)
        game.players.push(p1);

        // Player 2 (Bot, IQ 85 - Low IQ)
        let mut p2 = Player::new_bot(
            2,
            "Bot2".into(),
            [0.0, 1.0, 0.0],
            &crate::game_config::GameConfig::default(),
        );
        p2.iq = 85;
        p2.iq_points = 50.0;
        p2.troops = 100.0;
        p2.max_troops = 200.0;
        p2.gold = 10_000.0;
        p2.tile_count = 5;
        p2.border_insert(1); // Tile (1, 0)
        game.players.push(p2);

        game.player_lookup = vec![None, Some(0), Some(1)];

        // Set map ownerships to make them neighbors
        game.map.set_owner_id(0, 0, 1);
        game.map.set_owner_id(1, 0, 2);

        // Make both land tiles
        let idx0 = game.map.ref_id(0, 0);
        game.map.terrain[idx0] = crate::map::MapTile::from_byte(0b1000_0000);
        let idx1 = game.map.ref_id(1, 0);
        game.map.terrain[idx1] = crate::map::MapTile::from_byte(0b1000_0000);

        SowEngine::new(game, WaterComponents::default())
    }

    #[test]
    fn test_execute_income_iq_points_accumulation() {
        let mut engine = test_engine_two_players(42);
        engine.state.config.global_speed_multiplier = 1.0;
        engine.state.config.tick_rate_ms = 100.0;

        // Prior to income
        assert_eq!(engine.state.player(1).unwrap().iq_points, 50.0);
        assert_eq!(engine.state.player(2).unwrap().iq_points, 50.0);

        // Tick income
        engine.execute_income();

        // High IQ (135): per_tick(1.35) = 1.35 * 0.1 * 1.0 = 0.135
        assert_eq!(engine.state.player(1).unwrap().iq_points, 50.135);
        // Low IQ (85): per_tick(0.85) = 0.85 * 0.1 * 1.0 = 0.085
        assert_eq!(engine.state.player(2).unwrap().iq_points, 50.085);
    }

    #[test]
    fn test_alliance_proposal_threshold_high_iq() {
        let mut engine = test_engine_two_players(42);
        // Ensure bot 1 can afford alliance
        engine.state.player_mut(1).unwrap().iq_points = 100.0;
        for _ in 0..30 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
        // Since bot 1 has IQ 135, it only proposes if target troops > 0.8 * me_troops.
        // Bot 2 has 100 troops, Bot 1 has 1000. It should NOT propose an alliance.
        assert!(
            engine.alliances_proposed.is_empty(),
            "High IQ bot should not propose to weak neighbor"
        );
    }

    #[test]
    fn test_attack_context_betrayal_not_timer_driven() {
        let mut engine = test_engine_two_players(42);
        let p1 = engine.state.player_mut(1).unwrap();
        p1.iq_points = 100.0;
        p1.player_type = crate::player::PlayerType::Nation;
        p1.alliances.push(2);
        p1.alliance_timers.insert(2, 100);
        p1.troops = 5000.0;
        let p2 = engine.state.player_mut(2).unwrap();
        p2.alliances.push(1);
        p2.alliance_timers.insert(1, 100);
        p2.troops = 500.0;
        // No neutral land — boxed in with ally only.
        let mut broke_alliance = false;
        for _ in 0..120 {
            engine.state.tick += 1;
            engine.execute_ai_think();
            if !engine.state.player(1).unwrap().alliances.contains(&2) {
                broke_alliance = true;
                break;
            }
        }
        assert!(
            broke_alliance,
            "strong nation should betray weak bordering ally via attack-context logic"
        );
    }

    #[test]
    fn test_proactive_two_x_betrayal_removed() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 100.0;
        engine.state.player_mut(1).unwrap().alliances.push(2);
        engine.state.player_mut(2).unwrap().alliances.push(1);
        engine
            .state
            .player_mut(1)
            .unwrap()
            .alliance_timers
            .insert(2, 500);
        engine
            .state
            .player_mut(2)
            .unwrap()
            .alliance_timers
            .insert(1, 500);
        engine.state.player_mut(1).unwrap().troops = 2500.0;
        engine.state.player_mut(2).unwrap().troops = 1000.0;
        // Give bot 1 neutral expansion option so diplomacy propose is skipped; 2x should not auto-break.
        engine.state.map.set_owner_id(2, 0, 0);
        let idx = engine.state.map.ref_id(2, 0);
        engine.state.map.terrain[idx] = crate::map::MapTile::from_byte(0b1000_0000);
        for _ in 0..20 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
        assert!(
            engine.state.player(1).unwrap().alliances.contains(&2),
            "2x troop advantage alone must not trigger timer betrayal"
        );
    }

    #[test]
    fn test_density_upgrade_logic() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().gold = 10_000_000.0;
        engine.state.player_mut(1).unwrap().tile_count = 100; // Small area
        engine.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Nation;

        // Add max structures to force upgrade
        for i in 0..15 {
            engine.buildings.push(crate::building::Building {
                id: i,
                owner_id: 1,
                tile_idx: 0,
                kind: crate::game::BuildingKind::City,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
                modules: crate::building::CityModules::default(),
            });
        }
        engine.refresh_building_grid();
        for _ in 0..30 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
        // As long as this executes without panic we're good
    }

    #[test]
    fn test_frontline_defense_post_prioritization() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Nation;

        // Simulate under attack
        engine.attacks.push(crate::execution::AttackExecution {
            id: 1,
            owner_id: 2,
            target_owner: 1,
            troops: 5000.0,
            to_conquer: Default::default(),
            insert_seq_counter: 0,
            rng: wyrand::WyRand::new(42),
            retreating: false,
        });

        for _ in 0..30 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
    }

    #[test]
    fn test_nuke_launch_sam_avoidance() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().gold = 100_000_000.0;
        engine.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Nation;

        // Give bot 1 a silo
        let m1 = crate::building::CityModules {
            arsenal: 1,
            ..Default::default()
        };
        engine.buildings.push(crate::building::Building {
            id: 100,
            owner_id: 1,
            tile_idx: 0,
            kind: crate::game::BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: m1,
        });

        // Give bot 2 a city
        engine.buildings.push(crate::building::Building {
            id: 101,
            owner_id: 2,
            tile_idx: 10,
            kind: crate::game::BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: crate::building::CityModules::default(),
        });

        // Give bot 2 a SAM covering the city
        let m2 = crate::building::CityModules {
            shield: 1,
            ..Default::default()
        };
        engine.buildings.push(crate::building::Building {
            id: 102,
            owner_id: 2,
            tile_idx: 10,
            kind: crate::game::BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: m2,
        });

        for _ in 0..30 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
        // Since the only target is covered by SAM, it shouldn't launch.
        assert!(engine.recent_nuke_targets.is_empty());
    }

    #[test]
    fn test_alliance_cap_enforced() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        // Bot 1 (IQ 135) allows max 1 alliance. Give it 1 alliance already.
        engine.state.player_mut(1).unwrap().alliances.push(3);

        for _ in 0..30 {
            engine.state.tick += 1;
            engine.execute_ai_think();
        }
        assert!(
            engine.alliances_proposed.is_empty(),
            "High IQ bot should respect alliance cap of 1"
        );
    }

    #[test]
    fn test_nuke_launch_target_centroid() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().gold = 100_000_000.0;
        engine.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Nation;

        // Give bot 1 a silo
        let m1 = crate::building::CityModules {
            arsenal: 1,
            ..Default::default()
        };
        engine.buildings.push(crate::building::Building {
            id: 100,
            owner_id: 1,
            tile_idx: 0,
            kind: crate::game::BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: m1,
        });

        // Give bot 2 a city
        engine.buildings.push(crate::building::Building {
            id: 101,
            owner_id: 2,
            tile_idx: 1,
            kind: crate::game::BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: crate::building::CityModules::default(),
        });

        // Make sure bot 2 actually owns tile 1 so they are neighbors!
        engine.state.map.set_owner_id(1, 0, 2);

        engine.refresh_building_grid();
        let mut decisions = Vec::new();
        engine.maybe_launch_nuke(1, &mut decisions, 135, &[2]);
        assert!(!decisions.is_empty());
        assert_eq!(engine.recent_nuke_targets[0].1, 1);
    }

    #[test]
    fn test_team_alliance_prohibited() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().team = Some(crate::protocol::Team::Red);
        engine.state.player_mut(2).unwrap().team = Some(crate::protocol::Team::Red);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;

        let stamped = crate::protocol::StampedIntent {
            player_id: 1,
            intent: crate::protocol::GameplayIntent::ProposeAlliance { target_player: 2 },
        };
        engine.apply_stamped_intent(&stamped, 0);
        assert!(
            engine.alliances_proposed.is_empty(),
            "Teammates should not be allowed to propose alliance"
        );
    }

    fn test_engine_nation_mid_game() -> SowEngine {
        let w = 64u32;
        let h = 64u32;
        let config = crate::game_config::GameConfig::default();
        let mut game = GameState::new(42, w, h, config.clone());
        game.phase = GamePhase::Playing;

        for t in game.map.terrain.iter_mut() {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }

        let owner = 1u16;
        let mut sum_x = 0u64;
        let mut sum_y = 0u64;
        let mut count = 0u32;
        for y in 10..50 {
            for x in 10..50 {
                game.map.set_owner_id(x, y, owner);
                sum_x += x as u64;
                sum_y += y as u64;
                count += 1;
            }
        }

        let mut nation = Player::new_nation(1, "Nation1".into(), [1.0, 0.0, 0.0], &config);
        nation.iq_points = 500.0;
        nation.gold = 100_000.0;
        nation.troops = 10_000.0;
        nation.tile_count = count;
        nation.sum_x = sum_x;
        nation.sum_y = sum_y;
        for x in 10..50 {
            nation.border_insert(10 * w + x);
            nation.border_insert(49 * w + x);
        }
        for y in 11..49 {
            nation.border_insert(y * w + 10);
            nation.border_insert(y * w + 49);
        }
        game.players.push(nation);
        game.player_lookup = vec![None, Some(0)];

        let mut engine = SowEngine::new(game, WaterComponents::default());
        let city_positions = [(15, 15), (15, 35), (35, 15), (25, 25), (20, 40), (40, 20)];
        for (i, (cx, cy)) in city_positions.iter().enumerate() {
            let tile_idx = cy * w + cx;
            engine.buildings.push(crate::building::Building {
                id: (i as u64) + 1,
                owner_id: 1,
                tile_idx,
                kind: crate::game::BuildingKind::City,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
                modules: crate::building::CityModules::default(),
            });
        }
        engine.refresh_building_grid();
        engine.building_aggregates_dirty = true;
        engine
    }

    fn test_engine_advanced_tribe() -> SowEngine {
        let w = 48u32;
        let h = 48u32;
        let config = crate::game_config::GameConfig::default();
        let mut game = GameState::new(42, w, h, config.clone());
        game.phase = GamePhase::Playing;

        for t in game.map.terrain.iter_mut() {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }

        let owner = 10u16;
        let mut sum_x = 0u64;
        let mut sum_y = 0u64;
        let mut count = 0u32;
        for y in 4..28 {
            for x in 4..28 {
                game.map.set_owner_id(x, y, owner);
                sum_x += x as u64;
                sum_y += y as u64;
                count += 1;
            }
        }

        let mut tribe = Player::new_bot(10, "Tribe10".into(), [0.0, 1.0, 0.0], &config);
        tribe.iq = 110;
        tribe.iq_points = 500.0;
        tribe.gold = 50_000.0;
        tribe.troops = 5_000.0;
        tribe.tile_count = count;
        tribe.sum_x = sum_x;
        tribe.sum_y = sum_y;
        for x in 4..28 {
            tribe.border_insert(4 * w + x);
            tribe.border_insert(27 * w + x);
        }
        for y in 5..27 {
            tribe.border_insert(y * w + 4);
            tribe.border_insert(y * w + 27);
        }
        game.players.push(tribe);
        let mut lookup = vec![None; 11];
        lookup[10] = Some(0);
        game.player_lookup = lookup;

        let mut engine = SowEngine::new(game, WaterComponents::default());
        engine.refresh_building_grid();
        engine.building_aggregates_dirty = true;
        engine
    }

    #[test]
    fn test_nation_keeps_building_mid_game() {
        let mut engine = test_engine_nation_mid_game();
        let initial_count = engine.buildings.len();
        engine.state.config.global_speed_multiplier = 1.0;
        for _ in 0..2000 {
            engine.state.tick += 1;
            engine.execute_income();
            engine.execute_ai_think();
        }
        assert!(
            engine.buildings.len() > initial_count,
            "nation should keep placing structures mid-game (had {initial_count}, now {})",
            engine.buildings.len()
        );
    }

    #[test]
    fn test_advanced_tribe_can_build() {
        let mut engine = test_engine_advanced_tribe();
        let initial_count = engine.buildings.len();
        engine.state.config.global_speed_multiplier = 1.0;
        for _ in 0..1000 {
            engine.state.tick += 1;
            engine.execute_income();
            engine.execute_ai_think();
        }
        assert!(
            engine.buildings.len() > initial_count,
            "advanced tribe (id % 10) should build structures (had {initial_count}, now {})",
            engine.buildings.len()
        );
    }

    fn run_building_sim_ticks(engine: &mut SowEngine, ticks: u64) {
        engine.state.config.global_speed_multiplier = 1.0;
        for _ in 0..ticks {
            engine.state.tick += 1;
            engine.execute_income();
            engine.execute_ai_think();
        }
    }

    type BuildingSimFingerprint = (usize, u64, f64, f64, Vec<(u64, u32, u8, u8)>);

    fn building_sim_fingerprint(engine: &SowEngine, player_id: u16) -> BuildingSimFingerprint {
        let mut snaps: Vec<(u64, u32, u8, u8)> = engine
            .buildings
            .iter()
            .filter(|b| b.owner_id == player_id)
            .map(|b| (b.id, b.tile_idx, b.kind as u8, b.level))
            .collect();
        snaps.sort_by_key(|s| s.0);
        let level_sum: u64 = snaps.iter().map(|s| s.3 as u64).sum();
        let gold = engine
            .state
            .player(player_id)
            .map(|p| p.gold)
            .unwrap_or(0.0);
        let iq_pts = engine
            .state
            .player(player_id)
            .map(|p| p.iq_points)
            .unwrap_or(0.0);
        (snaps.len(), level_sum, gold, iq_pts, snaps)
    }

    #[test]
    fn test_ai_building_simulation_is_deterministic() {
        let mut a = test_engine_nation_mid_game();
        run_building_sim_ticks(&mut a, 500);
        let fp_a = building_sim_fingerprint(&a, 1);

        let mut b = test_engine_nation_mid_game();
        run_building_sim_ticks(&mut b, 500);
        let fp_b = building_sim_fingerprint(&b, 1);

        assert_eq!(
            fp_a, fp_b,
            "identical seed/setup must produce identical building state after 500 ticks"
        );
    }

    #[test]
    fn test_bot_structure_target_count_floor_is_stable() {
        // Low IQ: factor 0.1 caps non-city kinds at 0, city at least 1
        assert_eq!(bot_structure_target_count(BuildingKind::City, 10, 85), 1);
        assert_eq!(bot_structure_target_count(BuildingKind::Bunker, 10, 85), 0);
        // Mid IQ: 50% of high-IQ quotas, deterministic floor
        assert_eq!(bot_structure_target_count(BuildingKind::Factory, 8, 110), 2);
        // High IQ: full quotas
        assert_eq!(bot_structure_target_count(BuildingKind::Port, 10, 140), 3);
    }

    #[test]
    fn test_bot_build_stacks_like_player() {
        let w = 32u32;
        let config = crate::game_config::GameConfig::default();
        let mut game = GameState::new(42, w, w, config.clone());
        game.phase = GamePhase::Playing;
        for t in game.map.terrain.iter_mut() {
            *t = crate::map::MapTile::from_byte(0b1000_0000);
        }
        for y in 0..w {
            for x in 0..w {
                game.map.set_owner_id(x, y, 1);
            }
        }
        let mut nation = Player::new_nation(1, "N".into(), [1.0, 0.0, 0.0], &config);
        nation.gold = 1_000_000.0;
        nation.iq = 140;
        nation.iq_points = 500.0;
        nation.tile_count = w * w;
        game.players.push(nation);
        game.player_lookup = vec![None, Some(0)];

        let mut engine = SowEngine::new(game, WaterComponents::default());
        engine.buildings.push(crate::building::Building {
            id: 1,
            owner_id: 1,
            tile_idx: 16 * w + 16,
            kind: BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: crate::building::CityModules::default(),
        });
        engine.refresh_building_grid();

        let city_tile = 16 * w + 16;
        engine.apply_stamped_intent(
            &crate::protocol::StampedIntent {
                player_id: 1,
                intent: crate::protocol::GameplayIntent::BuildStructure {
                    kind: BuildingKind::City,
                    target_tile: city_tile,
                },
            },
            0,
        );

        assert_eq!(
            engine.buildings.len(),
            1,
            "stack must not spawn a second city"
        );
        assert_eq!(engine.buildings[0].level, 2);
    }

    #[test]
    fn test_tribe_buildings_not_purged() {
        let mut engine = test_engine_two_players(42);
        engine.buildings.push(crate::building::Building {
            id: 50,
            owner_id: 2,
            tile_idx: 1,
            kind: crate::game::BuildingKind::City,
            level: 1,
            under_construction: false,
            ticks_until_complete: 0,
            modules: crate::building::CityModules::default(),
        });
        engine.building_aggregates_dirty = true;
        engine.execute_income();
        assert!(
            engine
                .buildings
                .iter()
                .any(|b| b.id == 50 && b.owner_id == 2),
            "standard tribe buildings must not be deleted by income tick"
        );
    }
}
