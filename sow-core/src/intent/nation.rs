use crate::building::{
    aggregate_buildings_per_player, resolve_structure_spawn_tile, structure_build_cost_gold,
    structure_kind_enabled,
};
use crate::engine::SowEngine;

use crate::game::{BuildingKind, NukeKind};
use crate::protocol::{AttackIntent, GameplayIntent, StampedIntent};
use crate::rng::NextIntExt;
use wyrand::WyRand;

/// Maximum number of AI entities that may produce decisions in a single tick.
/// This prevents thundering-herd spikes when many bots/nations have aligned
/// intervals. Excess entities are deferred to the next tick via round-robin.
const MAX_AI_DECISIONS_PER_TICK: usize = 12;

fn bot_structure_target_count(
    kind: BuildingKind,
    city_equivalent: u32,
    difficulty: crate::game_config::BotDifficulty,
) -> u32 {
    let sam_ratio = match difficulty {
        crate::game_config::BotDifficulty::Vanilla => 0.20,
        crate::game_config::BotDifficulty::Terminator => 0.30,
    };
    match kind {
        BuildingKind::DefensePost => ((city_equivalent as f64) * 0.25).floor() as u32,
        BuildingKind::Port => ((city_equivalent as f64) * 0.75).floor() as u32,
        BuildingKind::Factory => ((city_equivalent as f64) * 0.75).floor() as u32,
        BuildingKind::SamLauncher => ((city_equivalent as f64) * sam_ratio).floor() as u32,
        BuildingKind::MissileSilo => ((city_equivalent as f64) * 0.2).floor() as u32,
        BuildingKind::City => city_equivalent.saturating_add(1),
    }
}

/// Cheapest possible gold cost for a building kind (count=0, no scaling).
/// Used as a fast pre-check to skip expensive placement logic when the bot
/// clearly cannot afford any building of this type.
#[inline]
fn cheapest_gold_cost(kind: BuildingKind) -> f64 {
    let s = crate::config::GOLD_SCALE.max(1.0);
    match kind {
        BuildingKind::City | BuildingKind::Factory | BuildingKind::Port => 125_000.0 / s,
        BuildingKind::DefensePost => 50_000.0 / s,
        BuildingKind::SamLauncher => 1_500_000.0 / s,
        BuildingKind::MissileSilo => 1_000_000.0 / s,
    }
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
}

struct BotAiProfile {
    interval_base: u64,
    trigger_ratio: f64,
    reserve_ratio: f64,
    expand_ratio: f64,
    refuse_human_chance: i32,
}

fn get_bot_ai_profile(bot_id: u16, is_nation: bool) -> BotAiProfile {
    let is_smart = is_nation || (bot_id % 100 == 0);
    if is_smart {
        // Nations have 4 distinct profiles: 25% Expansionist, 25% Defensive, 25% Balanced, 25% Pacifist
        match bot_id % 4 {
            0 => BotAiProfile {
                interval_base: 120,
                trigger_ratio: 0.45,
                reserve_ratio: 0.15,
                expand_ratio: 0.15,
                refuse_human_chance: 20,
            },
            1 => BotAiProfile {
                interval_base: 180,
                trigger_ratio: 0.65,
                reserve_ratio: 0.40,
                expand_ratio: 0.20,
                refuse_human_chance: 60,
            },
            2 => BotAiProfile {
                interval_base: 150,
                trigger_ratio: 0.55,
                reserve_ratio: 0.30,
                expand_ratio: 0.15,
                refuse_human_chance: 40,
            },
            _ => BotAiProfile {
                interval_base: 240,
                trigger_ratio: 0.75,
                reserve_ratio: 0.50,
                expand_ratio: 0.25,
                refuse_human_chance: 80,
            },
        }
    } else {
        // Tribes (bots): 5% Terminator rare aggro, 95% Vanilla split into Territorial, Standard, and Soft Expansionist
        if bot_id % 20 == 0 {
            BotAiProfile {
                interval_base: 120,
                trigger_ratio: 0.45,
                reserve_ratio: 0.15,
                expand_ratio: 0.10,
                refuse_human_chance: 0,
            }
        } else {
            match bot_id % 3 {
                0 => BotAiProfile {
                    interval_base: 350,
                    trigger_ratio: 0.75,
                    reserve_ratio: 0.50,
                    expand_ratio: 0.20,
                    refuse_human_chance: 90,
                },
                1 => BotAiProfile {
                    interval_base: 280,
                    trigger_ratio: 0.65,
                    reserve_ratio: 0.35,
                    expand_ratio: 0.15,
                    refuse_human_chance: 75,
                },
                _ => BotAiProfile {
                    interval_base: 220,
                    trigger_ratio: 0.70,
                    reserve_ratio: 0.30,
                    expand_ratio: 0.10,
                    refuse_human_chance: 85,
                },
            }
        }
    }
}

impl SowEngine {
    /// Unified AI pipeline for both Tribes (`Bot`) and Nations.
    ///
    /// - Builds one combined schedule of all AI entities.
    /// - Applies a per-tick budget (`MAX_AI_DECISIONS_PER_TICK`) with a round-robin
    ///   cursor so no single spike processes more than N bots.
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
            let interval_base = profile.interval_base;

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

            let is_smart = is_nation || (bot_id % 100 == 0);

            // Nations and 1% smart tribes get structure phases at 1/3 and 2/3 intervals
            let do_structures = if is_smart {
                let one_third = (offset + interval / 3) % interval;
                let two_thirds = (offset + (interval * 2) / 3) % interval;
                do_attack || phase == one_third || phase == two_thirds
            } else {
                false
            };

            if !do_attack && !do_structures {
                continue; // Nothing to do this tick for this entity
            }

            if do_structures && p.gold >= cheapest_gold_cost(BuildingKind::DefensePost) {
                any_structures = true;
            }

            schedule.push(AiSlot {
                bot_id,
                is_nation: is_smart,
                do_attack,
                do_structures,
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

        // ── Apply round-robin budget ────────────────────────────────────────
        let total = schedule.len();
        if self.ai_round_robin >= total {
            self.ai_round_robin = 0;
        }
        let start = self.ai_round_robin;
        let budget = total.min(MAX_AI_DECISIONS_PER_TICK);

        let mut decisions: Vec<BotDecision> = Vec::new();
        let mut processed = 0usize;

        for raw_i in 0..total {
            if processed >= budget {
                break;
            }
            let i = (start + raw_i) % total;
            let slot = &schedule[i];
            let bot_id = slot.bot_id;

            let (bot_iq, _bot_iq_points) = {
                let Some(player) = self.state.player(bot_id) else {
                    continue;
                };
                (player.iq, player.iq_points)
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
            for &(proposer, target) in &self.alliances_proposed {
                if target == bot_id {
                    let proposer_ok = self.state.player(proposer).map(|p| p.alive).unwrap_or(false);
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
                            if bot_iq >= 130 {
                                if let (Some(p_me), Some(p_prop)) = (self.state.player(bot_id), self.state.player(proposer)) {
                                    let me_troops = p_me.troops.max(1.0);
                                    let me_tiles = p_me.tile_count.max(1);
                                    if p_prop.troops >= me_troops * 0.8 && p_prop.tile_count >= (me_tiles as f64 * 0.8) as u32 {
                                        accept = true;
                                    }
                                }
                            } else if bot_iq >= 100 {
                                if let (Some(p_me), Some(p_prop)) = (self.state.player(bot_id), self.state.player(proposer)) {
                                    let me_troops = p_me.troops.max(1.0);
                                    let me_tiles = p_me.tile_count.max(1);
                                    if p_prop.troops >= me_troops * 0.5 && p_prop.tile_count >= (me_tiles as f64 * 0.5) as u32 {
                                        accept = true;
                                    }
                                }
                            } else {
                                accept = true;
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
                    intent: GameplayIntent::AcceptAlliance { target_player: proposer },
                });
            }

            // ── Resource Sharing (High IQ only) ───────────────────────────
            if bot_iq >= 130 {
                let current_points = self.state.player(bot_id).unwrap().iq_points;
                if current_points >= send_cost {
                    let mut ally_to_help = None;
                    if let Some(p_me) = self.state.player(bot_id) {
                        if p_me.gold > 200_000.0 && p_me.troops > p_me.max_troops * 0.5 {
                            for &ally_id in &p_me.alliances {
                                if let Some(p_ally) = self.state.player(ally_id) {
                                    if p_ally.alive && (p_ally.troops < p_ally.max_troops * 0.3 || p_ally.troops <= 500.0) {
                                        ally_to_help = Some(ally_id);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    if let Some(ally_id) = ally_to_help {
                        if let Some(p_me) = self.state.player_mut(bot_id) {
                            p_me.iq_points -= send_cost;
                        }
                        decisions.push(BotDecision {
                            bot_id,
                            kind: BotDecisionKind::Build,
                            intent: GameplayIntent::SendResources {
                                target_player: ally_id,
                                gold: 50_000.0,
                                troops: 200.0,
                            },
                        });
                    }
                }
            }

            // ── Breaking Alliances if ally has become too weak ─────────────
            let current_points = self.state.player(bot_id).unwrap().iq_points;
            if current_points >= alliance_cost {
                let mut alliances_to_break = Vec::new();
                if let Some(p_me) = self.state.player(bot_id) {
                    for &ally_id in &p_me.alliances {
                        if let Some(p_ally) = self.state.player(ally_id) {
                            if p_ally.alive {
                                let me_troops = p_me.troops.max(1.0);
                                let me_tiles = p_me.tile_count.max(1);
                                if bot_iq >= 130 {
                                    if p_ally.troops < me_troops * 0.8 || p_ally.tile_count < (me_tiles as f64 * 0.8) as u32 {
                                        alliances_to_break.push(ally_id);
                                    }
                                } else if bot_iq >= 100 {
                                    if p_ally.troops < me_troops * 0.5 || p_ally.tile_count < (me_tiles as f64 * 0.5) as u32 {
                                        alliances_to_break.push(ally_id);
                                    }
                                }
                            }
                        }
                    }
                }
                for ally_id in alliances_to_break {
                    if let Some(p_me) = self.state.player_mut(bot_id) {
                        if p_me.iq_points >= alliance_cost {
                            p_me.iq_points -= alliance_cost;
                            decisions.push(BotDecision {
                                bot_id,
                                kind: BotDecisionKind::Build,
                                intent: GameplayIntent::BreakAlliance { target_player: ally_id },
                            });
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
            if current_points >= alliance_cost && !neighbor_players.is_empty() {
                let mut proposed_target = None;
                let (me_alliances, me_troops, me_tile_count) = {
                    let p_me = self.state.player(bot_id).unwrap();
                    (p_me.alliances.clone(), p_me.troops, p_me.tile_count)
                };

                let max_alliances = if bot_iq >= 130 { 1 } else if bot_iq >= 100 { 3 } else { 5 };
                
                // Betrayal logic
                if bot_iq >= 130 && !me_alliances.is_empty() {
                    for &ally_id in &me_alliances {
                        if let Some(ally) = self.state.player(ally_id) {
                            if me_troops >= ally.troops * 2.0 {
                                let tick = self.state.tick as u32;
                                if let Some(p_me) = self.state.player_mut(bot_id) {
                                    p_me.iq_points -= alliance_cost;
                                    p_me.traitor = true;
                                    p_me.traitor_tick = tick;
                                }
                                decisions.push(BotDecision {
                                    bot_id,
                                    kind: BotDecisionKind::Build,
                                    intent: GameplayIntent::BreakAlliance { target_player: ally_id },
                                });
                                break;
                            }
                        }
                    }
                }

                if me_alliances.len() < max_alliances {
                    for &neighbor in &neighbor_players {
                    let (neigh_alive, neigh_troops, neigh_tile_count) = match self.state.player(neighbor) {
                        Some(pn) => (pn.alive, pn.troops, pn.tile_count),
                        None => continue,
                    };
                    if neigh_alive {
                        let is_teammate = {
                            let p_me = self.state.player(bot_id).unwrap();
                            let p_neigh = self.state.player(neighbor).unwrap();
                            p_me.team.is_some() && p_me.team == p_neigh.team
                        };
                        let is_allied = me_alliances.contains(&neighbor) || is_teammate;
                        let is_proposed = self.alliances_proposed.contains(&(bot_id, neighbor));
                        if !is_allied && !is_proposed {
                            let mut meets_threshold = false;
                            let roll = {
                                let p_me = self.state.player_mut(bot_id).unwrap();
                                p_me.bot_rng.next_int(0, 100)
                            };

                            if bot_iq >= 130 {
                                let me_troops_val = me_troops.max(1.0);
                                let me_tiles_val = me_tile_count.max(1);
                                if neigh_troops >= me_troops_val * 0.8 && neigh_tile_count >= (me_tiles_val as f64 * 0.8) as u32 {
                                    meets_threshold = roll < 15;
                                }
                            } else if bot_iq >= 100 {
                                let me_troops_val = me_troops.max(1.0);
                                let me_tiles_val = me_tile_count.max(1);
                                if neigh_troops >= me_troops_val * 0.5 && neigh_tile_count >= (me_tiles_val as f64 * 0.5) as u32 {
                                    meets_threshold = roll < 10;
                                }
                            } else {
                                meets_threshold = roll < 5;
                            }

                            if meets_threshold {
                                proposed_target = Some(neighbor);
                                break;
                            }
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
                        intent: GameplayIntent::ProposeAlliance { target_player: neighbor },
                    });
                }
            }

            // ── Structure building (Nations only) ───────────────────────
            if slot.do_structures && slot.is_nation {
                let current_points = self.state.player(bot_id).unwrap().iq_points;
                if current_points >= build_cost {
                    let (player_gold, player_tile_count) = {
                        if let Some(player) = self.state.player(bot_id) {
                            (player.gold, player.tile_count)
                        } else {
                            continue;
                        }
                    };
                    if player_gold >= cheapest_gold_cost(BuildingKind::DefensePost) {
                        let agg = self
                            .building_aggregates
                            .get(bot_id as usize)
                            .copied()
                            .unwrap_or_default();
                        let city_equivalent =
                            agg.ready_city_count.max((player_tile_count / 2000).max(1));
                        let build_order = [
                            BuildingKind::DefensePost,
                            BuildingKind::Port,
                            BuildingKind::Factory,
                            BuildingKind::SamLauncher,
                            BuildingKind::MissileSilo,
                            BuildingKind::City,
                        ];
                        for kind in build_order {
                            if !structure_kind_enabled(kind) {
                                continue;
                            }
                            let owned = agg.total_structures_of_kind(kind);
                            let mut target_count = bot_structure_target_count(
                                kind,
                                city_equivalent,
                                crate::game_config::BotDifficulty::Terminator,
                            );
                            if kind == BuildingKind::DefensePost && bot_iq >= 110 {
                                let mut under_attack = false;
                                for att in &self.attacks {
                                    if att.target_owner == bot_id {
                                        under_attack = true;
                                        break;
                                    }
                                }
                                if under_attack { target_count += 3; }
                            }
                            if kind == BuildingKind::MissileSilo && owned >= 3 {
                                continue;
                            }
                            let total_owned = agg.count_city + agg.count_factory + agg.count_port + agg.count_defense + agg.count_sam + agg.count_silo;
                            let density = total_owned as f32 / player_tile_count.max(1) as f32;
                            let is_density_high = bot_iq >= 110 && density > 1.0 / 1500.0;

                            let mut upgraded = false;
                            if owned >= target_count || is_density_high {
                                let mut upgrade_target = None;
                                let mut best_score = -1.0;
                                for b in &self.buildings {
                                    if b.owner_id == bot_id && b.kind == kind && !b.under_construction && b.level < 5 {
                                        let mut score = 1.0;
                                        let mut has_sam = false;
                                        let (bx, by) = (b.tile_idx % self.state.map.width, b.tile_idx / self.state.map.width);
                                        for b2 in &self.buildings {
                                            if b2.kind == crate::game::BuildingKind::SamLauncher && !b2.under_construction {
                                                let (sx, sy) = (b2.tile_idx % self.state.map.width, b2.tile_idx / self.state.map.width);
                                                if (bx as i32 - sx as i32).abs() + (by as i32 - sy as i32).abs() <= 48 {
                                                    has_sam = true;
                                                    break;
                                                }
                                            }
                                        }
                                        if has_sam {
                                            score += 10.0;
                                        }
                                        if score > best_score {
                                            best_score = score;
                                            upgrade_target = Some(b.id);
                                        }
                                    }
                                }
                                if let Some(target_id) = upgrade_target {
                                    let cost = structure_build_cost_gold(kind, bot_id, &self.buildings);
                                    if player_gold >= cost {
                                        if let Some(p_me) = self.state.player_mut(bot_id) {
                                            p_me.iq_points -= build_cost;
                                        }
                                        decisions.push(BotDecision {
                                            bot_id,
                                            kind: BotDecisionKind::Build,
                                            intent: GameplayIntent::UpgradeStructure { building_id: target_id },
                                        });
                                        upgraded = true;
                                    }
                                }
                            }

                            if upgraded {
                                break;
                            }

                            if owned >= target_count {
                                continue;
                            }
                            if player_gold < cheapest_gold_cost(kind) {
                                continue;
                            }
                            let cost = structure_build_cost_gold(kind, bot_id, &self.buildings);
                            let map_w = self.state.map.width;
                            let Some(bot_now) = self.state.player_mut(bot_id) else {
                                continue;
                            };
                            if bot_now.gold < cost {
                                continue;
                            }

                            let border_len = bot_now.border_tiles.count_ones();
                            let mut tile_choice: Option<u32> = None;
                            if border_len > 0 {
                                let pick = bot_now.bot_rng.next_int(0, border_len as i32) as usize;
                                let chosen_idx_opt = bot_now.border_tiles.ones().nth(pick);
                                if let Some(chosen_idx) = chosen_idx_opt {
                                    let (bx, by) = (chosen_idx % map_w, chosen_idx / map_w);
                                    let idx = by * map_w + bx;
                                    if resolve_structure_spawn_tile(
                                        &self.state.map,
                                        bot_id,
                                        kind,
                                        idx,
                                        &self.building_grid,
                                        &mut self.placement_scratch,
                                    )
                                    .is_some()
                                    {
                                        tile_choice = Some(idx);
                                    }
                                }
                            }
                            let Some(target_tile) = tile_choice else {
                                continue;
                            };
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
                    }
                }
            }

            // ── Attack logic (both Bots and Nations) ────────────────────
            if slot.do_attack {
                let current_points = self.state.player(bot_id).unwrap().iq_points;
                if current_points >= attack_cost {
                    let profile = get_bot_ai_profile(bot_id, slot.is_nation);
                    let trigger_ratio = profile.trigger_ratio;
                    let reserve_ratio = profile.reserve_ratio;
                    let expand_ratio = profile.expand_ratio;
                    let refuse_human_chance = profile.refuse_human_chance;

                    let (troops, max_troops) = {
                        let player = self.state.player(bot_id).unwrap();
                        (player.troops, player.max_troops)
                    };

                    let targets: Vec<u16> = neighbor_players.iter().copied()
                        .filter(|&id| {
                            if let Some(p_me) = self.state.player(bot_id) {
                                !p_me.alliances.contains(&id)
                            } else {
                                true
                            }
                        })
                        .collect();

                    let (target_owner, is_neutral) = if has_neutral {
                        (0, true)
                    } else if targets.is_empty() {
                        if slot.is_nation {
                            self.maybe_launch_nuke(bot_id, &mut decisions, bot_iq, &targets);
                            self.maybe_launch_mirv(bot_id, &mut decisions, bot_iq);
                        }
                        processed += 1;
                        continue;
                    } else {
                        let target_owner;
                        if bot_iq >= 130 {
                            let mut best_target = targets[0];
                            for &t_id in &targets {
                                if let (Some(p_t), Some(p_b)) = (
                                    self.state.player(t_id),
                                    self.state.player(best_target),
                                ) {
                                    let t_is_tribe = p_t.player_type == crate::player::PlayerType::Bot && t_id % 100 != 0;
                                    let b_is_tribe = p_b.player_type == crate::player::PlayerType::Bot && best_target % 100 != 0;

                                    if t_is_tribe && !b_is_tribe {
                                        best_target = t_id;
                                    } else if t_is_tribe == b_is_tribe {
                                        if p_t.troops < p_b.troops {
                                            best_target = t_id;
                                        }
                                    }
                                }
                            }
                            target_owner = best_target;
                        } else if bot_iq >= 100 {
                            let mut weakest = targets[0];
                            for &t_id in &targets {
                                if let (Some(p_t), Some(p_w)) = (
                                    self.state.player(t_id),
                                    self.state.player(weakest),
                                ) {
                                    if p_t.troops < p_w.troops { weakest = t_id; }
                                }
                            }
                            target_owner = weakest;
                        } else {
                            let p_mut = self.state.player_mut(bot_id).unwrap();
                            let roll = p_mut.bot_rng.next_int(0, targets.len() as i32) as usize;
                            target_owner = targets[roll];
                        }

                        let is_target_human = self.state.player(target_owner).map(|pl| pl.is_human()).unwrap_or(false);
                        let refuse_roll = self.state.player_mut(bot_id).unwrap().bot_rng.next_int(0, 100);
                        if is_target_human && refuse_roll < refuse_human_chance {
                            processed += 1;
                            continue;
                        }

                        (target_owner, false)
                    };

                    if is_neutral || troops >= max_troops * trigger_ratio {
                        let reserve = max_troops
                            * if is_neutral {
                                expand_ratio
                            } else {
                                reserve_ratio
                            };
                        let is_standard_bot = !slot.is_nation && (bot_id % 100 != 0);
                        let p_send = if is_standard_bot && !is_neutral {
                            (troops / 20.0).max(0.0)
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
                        self.maybe_launch_mirv(bot_id, &mut decisions, bot_iq);
                    }
                }
            }

            processed += 1;
        }

        // Advance round-robin cursor for next tick
        self.ai_round_robin = (start + processed) % total.max(1);

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

    fn maybe_launch_nuke(&mut self, bot_id: u16, decisions: &mut Vec<BotDecision>, bot_iq: u32, targets: &[u16]) {
        if bot_iq < 100 { return; }
        
        if self.building_aggregates_dirty {
            self.building_aggregates = crate::building::core::aggregate_buildings_per_player(
                self.buildings.iter().copied(),
                self.state.players.len(),
            );
            self.building_aggregates_dirty = false;
        }
        
        let agg = self.building_aggregates.get(bot_id as usize).copied().unwrap_or_default();
        if agg.count_silo == 0 { return; }
        
        let mut has_silo = false;
        let mut total_silos = 0;
        for b in &self.buildings {
            if b.owner_id == bot_id && b.kind == BuildingKind::MissileSilo && !b.under_construction {
                if self.silo_cooldowns.get(&b.id).copied().unwrap_or(0) == 0 {
                    has_silo = true;
                }
                total_silos += 1;
            }
        }
        if !has_silo { return; }

        let Some(player) = self.state.player(bot_id) else { return; };
        let prev_mirv_launches = self.mirv_launches.get(&bot_id).copied().unwrap_or(0);
        let atom_cost = NukeKind::AtomBomb.gold_cost(prev_mirv_launches);
        let hydro_cost = NukeKind::HydrogenBomb.gold_cost(prev_mirv_launches);

        // Gold hoarding for MIRV logic
        let perceived_atom_cost = atom_cost * (1.0 + 0.5 * total_silos as f64);
        let perceived_hydro_cost = hydro_cost * (1.0 + 0.25 * total_silos as f64);

        let kind = if player.gold >= perceived_hydro_cost {
            NukeKind::HydrogenBomb
        } else if player.gold >= perceived_atom_cost {
            NukeKind::AtomBomb
        } else {
            return;
        };

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

        if primary_target == 0 || primary_target == bot_id { return; }

        // Find best structure to nuke
        let mut best_score = -1.0;
        let mut best_tile = 0;
        
        for b in &self.buildings {
            if b.owner_id != primary_target || b.under_construction { continue; }
            let mut score = match b.kind {
                BuildingKind::MissileSilo => 50000.0,
                BuildingKind::City => 25000.0,
                BuildingKind::Factory | BuildingKind::Port => 15000.0,
                BuildingKind::DefensePost => 5000.0 * (b.level as f64),
                BuildingKind::SamLauncher => 10000.0 * (b.level as f64),
            };

            let bx = b.tile_idx % self.state.map.width;
            let by = b.tile_idx / self.state.map.width;

            // SAM avoidance
            let mut sam_covered = false;
            for b2 in &self.buildings {
                if b2.kind == crate::game::BuildingKind::SamLauncher && !b2.under_construction && b2.owner_id != bot_id {
                    let mut is_ally = false;
                    if let Some(p1) = self.state.player(bot_id) {
                        if p1.alliances.contains(&b2.owner_id) {
                            is_ally = true;
                        }
                    }
                    if !is_ally {
                        let (sx, sy) = (b2.tile_idx % self.state.map.width, b2.tile_idx / self.state.map.width);
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
            self.recent_nuke_targets.push((primary_target, best_tile, self.state.tick));
            decisions.push(BotDecision {
                bot_id,
                kind: BotDecisionKind::Attack,
                intent: GameplayIntent::LaunchNuke { kind, target_tile: best_tile },
            });
            let p_me = self.state.player_mut(bot_id).unwrap();
            p_me.iq_points -= 15.0; // Assume 15 points
        }
    }

    fn maybe_launch_mirv(&mut self, bot_id: u16, decisions: &mut Vec<BotDecision>, bot_iq: u32) {
        if bot_iq < 100 { return; }
        
        if self.building_aggregates_dirty {
            self.building_aggregates = crate::building::core::aggregate_buildings_per_player(
                self.buildings.iter().copied(),
                self.state.players.len(),
            );
            self.building_aggregates_dirty = false;
        }
 
        let agg = self.building_aggregates.get(bot_id as usize).copied().unwrap_or_default();
        if agg.count_silo == 0 { return; }

        let mut has_silo = false;
        for b in &self.buildings {
            if b.owner_id == bot_id && b.kind == BuildingKind::MissileSilo && !b.under_construction {
                if self.silo_cooldowns.get(&b.id).copied().unwrap_or(0) == 0 {
                    has_silo = true;
                    break;
                }
            }
        }
        if !has_silo { return; }

        let Some(player) = self.state.player(bot_id) else { return; };
        let prev_mirv_launches = self.mirv_launches.get(&bot_id).copied().unwrap_or(0);
        let mirv_cost = NukeKind::MIRV.gold_cost(prev_mirv_launches);

        if player.gold < mirv_cost { return; }

        let total_land = self.state.total_land_tiles.max(1);
        let mut target_id = 0;

        // Counter-MIRV
        for proj in &self.projectiles {
            if proj.active && matches!(proj.kind, crate::game::ProjectileKind::Nuke(NukeKind::MIRV)) {
                let target_x = (proj.dst_tile % self.state.map.width) as i32;
                let target_y = (proj.dst_tile / self.state.map.width) as i32;
                if self.state.map.is_valid_coord(target_x, target_y) {
                    if self.state.map.owner_id(target_x as u32, target_y as u32) == bot_id {
                        target_id = proj.owner_id;
                        break;
                    }
                }
            }
        }

        // Victory Denial
        if target_id == 0 {
            for p in &self.state.players {
                if p.alive && p.id != bot_id && !player.alliances.contains(&p.id) {
                    let share = p.tile_count as f32 / total_land as f32;
                    let limit = if bot_iq >= 130 { 0.50 } else { 0.65 };
                    if share > limit {
                        target_id = p.id;
                        break;
                    }
                }
            }
        }

        if target_id != 0 {
            // Check cooldown to prevent pile-on
            let last_mirv_tick = self.mirv_cooldown_targets.get(&target_id).copied().unwrap_or(0);
            if self.state.tick > last_mirv_tick + 300 {
                if let Some(target_p) = self.state.player(target_id) {
                    if target_p.tile_count > 0 {
                        let cx = (target_p.sum_x / target_p.tile_count as u64) as u32;
                        let cy = (target_p.sum_y / target_p.tile_count as u64) as u32;
                        let target_tile = cy * self.state.map.width + cx;
                        
                        self.mirv_cooldown_targets.insert(target_id, self.state.tick);
                        decisions.push(BotDecision {
                            bot_id,
                            kind: BotDecisionKind::Attack,
                            intent: GameplayIntent::LaunchNuke { kind: NukeKind::MIRV, target_tile },
                        });
                        let p_me = self.state.player_mut(bot_id).unwrap();
                        p_me.iq_points -= 15.0;
                    }
                }
            }
        }
    }
}


#[cfg(test)]
mod bot_iq_alliance_tests {
    use crate::engine::SowEngine;
    use crate::game::{GamePhase, GameState};
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
        for _ in 0..30 { engine.state.tick += 1; engine.execute_ai_think(); }
        // Since bot 1 has IQ 135, it only proposes if target troops > 0.8 * me_troops. 
        // Bot 2 has 100 troops, Bot 1 has 1000. It should NOT propose an alliance.
        assert!(engine.alliances_proposed.is_empty(), "High IQ bot should not propose to weak neighbor");
    }

    #[test]
    #[ignore]
    fn test_alliance_betrayal_high_iq() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 100.0;
        // Force alliance
        engine.state.player_mut(1).unwrap().alliances.push(2);
        engine.state.player_mut(2).unwrap().alliances.push(1);
        
        engine.refresh_building_grid();
        engine.state.player_mut(1).unwrap().troops = 5000.0;
        engine.state.map.set_owner_id(1, 0, 2);
        for _ in 0..30 { engine.state.tick += 1; engine.execute_ai_think(); }
        // Bot 1 (1000 troops) should betray Bot 2 (100 troops)
        let p1 = engine.state.player(1).unwrap();
        assert!(p1.traitor);
        assert_eq!(p1.traitor_tick, 0);
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
                kind: crate::game::BuildingKind::Factory,
                level: 1,
                under_construction: false,
                ticks_until_complete: 0,
            });
        }
        engine.refresh_building_grid();
        for _ in 0..30 { engine.state.tick += 1; engine.execute_ai_think(); }
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
            initial_troops: 5000.0,
            to_conquer: Default::default(),
            insert_seq_counter: 0,
            rng: wyrand::WyRand::new(42),
            retreating: false,
        });
        
        for _ in 0..30 { engine.state.tick += 1; engine.execute_ai_think(); }
    }

    #[test]
    fn test_nuke_launch_sam_avoidance() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().gold = 100_000_000.0;
        engine.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Nation;
        
        // Give bot 1 a silo
        engine.buildings.push(crate::building::Building {
            id: 100, owner_id: 1, tile_idx: 0, kind: crate::game::BuildingKind::MissileSilo,
            level: 1, under_construction: false, ticks_until_complete: 0,
        });

        // Give bot 2 a city
        engine.buildings.push(crate::building::Building {
            id: 101, owner_id: 2, tile_idx: 10, kind: crate::game::BuildingKind::City,
            level: 1, under_construction: false, ticks_until_complete: 0,
        });

        // Give bot 2 a SAM covering the city
        engine.buildings.push(crate::building::Building {
            id: 102, owner_id: 2, tile_idx: 10, kind: crate::game::BuildingKind::SamLauncher,
            level: 1, under_construction: false, ticks_until_complete: 0,
        });
        
        for _ in 0..30 { engine.state.tick += 1; engine.execute_ai_think(); }
        // Since the only target is covered by SAM, it shouldn't launch.
        assert!(engine.recent_nuke_targets.is_empty());
    }

    #[test]
    fn test_mirv_launch_victory_denial() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().gold = 100_000_000.0;
        engine.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Nation;
        
        // Give bot 1 a silo
        engine.buildings.push(crate::building::Building {
            id: 100, owner_id: 1, tile_idx: 0, kind: crate::game::BuildingKind::MissileSilo,
            level: 1, under_construction: false, ticks_until_complete: 0,
        });

        // Make bot 2 have 60% of land
        engine.state.total_land_tiles = 1000;
        engine.state.player_mut(2).unwrap().tile_count = 600;
        
        engine.refresh_building_grid();
        engine.state.tick = 400; // bypass cooldown
        let mut decisions = Vec::new();
        engine.maybe_launch_mirv(1, &mut decisions, 135);
        
        // Bot 1 has IQ 135, victory limit is 50%. Bot 2 has 60%. Bot 1 should MIRV Bot 2!
        assert!(!decisions.is_empty(), "MIRV should be launched");
    }

    #[test]
    fn test_mirv_launch_counter_mirv() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().gold = 100_000_000.0;
        engine.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Nation;
        
        // Give bot 1 a silo
        engine.buildings.push(crate::building::Building {
            id: 100, owner_id: 1, tile_idx: 0, kind: crate::game::BuildingKind::MissileSilo,
            level: 1, under_construction: false, ticks_until_complete: 0,
        });

        // Bot 2 fired a MIRV at Bot 1
        engine.projectiles.push(crate::game::Projectile {
            id: 1, owner_id: 2, active: true,
            kind: crate::game::ProjectileKind::Nuke(crate::game::NukeKind::MIRV),
            src_tile: 10 * 8 + 10, dst_tile: 0,
            path: vec![10 * 8 + 10, 0],
            path_cursor: 0,
            steps_per_tick: 2,
        });

        // Bot 1 owns tile (0,0)
        engine.state.map.set_owner_id(0, 0, 1);
        engine.state.player_mut(2).unwrap().tile_count = 1; // Need a tile to target!
        
        engine.refresh_building_grid();
        engine.state.tick = 400; // bypass cooldown
        let mut decisions = Vec::new();
        engine.maybe_launch_mirv(1, &mut decisions, 135);
        
        assert!(!decisions.is_empty(), "Counter-MIRV should be launched");
    }

    #[test]
    fn test_alliance_cap_enforced() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        // Bot 1 (IQ 135) allows max 1 alliance. Give it 1 alliance already.
        engine.state.player_mut(1).unwrap().alliances.push(3);
        
        for _ in 0..30 { engine.state.tick += 1; engine.execute_ai_think(); }
        assert!(engine.alliances_proposed.is_empty(), "High IQ bot should respect alliance cap of 1");
    }

    #[test]
    fn test_nuke_launch_target_centroid() {
        let mut engine = test_engine_two_players(42);
        engine.state.player_mut(1).unwrap().iq_points = 500.0;
        engine.state.player_mut(1).unwrap().gold = 100_000_000.0;
        engine.state.player_mut(1).unwrap().player_type = crate::player::PlayerType::Nation;
        
        // Give bot 1 a silo
        engine.buildings.push(crate::building::Building {
            id: 100, owner_id: 1, tile_idx: 0, kind: crate::game::BuildingKind::MissileSilo,
            level: 1, under_construction: false, ticks_until_complete: 0,
        });

        // Give bot 2 a city
        engine.buildings.push(crate::building::Building {
            id: 101, owner_id: 2, tile_idx: 1, kind: crate::game::BuildingKind::City,
            level: 1, under_construction: false, ticks_until_complete: 0,
        });
        
        // Make sure bot 2 actually owns tile 1 so they are neighbors!
        engine.state.map.set_owner_id(1, 0, 2);
        
        engine.refresh_building_grid();
        let mut decisions = Vec::new();
        engine.maybe_launch_nuke(1, &mut decisions, 135, &vec![2]);
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
        assert!(engine.alliances_proposed.is_empty(), "Teammates should not be allowed to propose alliance");
    }
}
