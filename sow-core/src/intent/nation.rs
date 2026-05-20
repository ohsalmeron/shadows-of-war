use crate::building::{
    aggregate_buildings_per_player, resolve_structure_spawn_tile, structure_build_cost_gold,
    structure_kind_enabled,
};
use crate::engine::SowEngine;
use crate::game::BuildingKind;
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
    if is_nation {
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

            // Nations get structure phases at 1/3 and 2/3 intervals; Bots never build
            let do_structures = if is_nation {
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
                is_nation,
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

            // Define point costs and thresholds based on IQ
            let (attack_cost, build_cost, alliance_cost, send_cost) = if bot_iq >= 130 {
                (30.0, 30.0, 20.0, 10.0)
            } else if bot_iq >= 100 {
                (15.0, 15.0, 10.0, 999.0)
            } else {
                (5.0, 5.0, 2.0, 999.0)
            };

            // ── Alliance Proposal Evaluation ───────────────────────────────
            let mut proposals_to_accept = Vec::new();
            for &(proposer, target) in &self.alliances_proposed {
                if target == bot_id {
                    let proposer_ok = self.state.player(proposer).map(|p| p.alive).unwrap_or(false);
                    if proposer_ok {
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

                for &neighbor in &neighbor_players {
                    let (neigh_alive, neigh_troops, neigh_tile_count) = match self.state.player(neighbor) {
                        Some(pn) => (pn.alive, pn.troops, pn.tile_count),
                        None => continue,
                    };
                    if neigh_alive {
                        let is_allied = me_alliances.contains(&neighbor);
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
                    let Some(player) = self.state.player(bot_id) else {
                        continue;
                    };
                    let player_gold = player.gold;
                    if player_gold >= cheapest_gold_cost(BuildingKind::DefensePost) {
                        let agg = self
                            .building_aggregates
                            .get(bot_id as usize)
                            .copied()
                            .unwrap_or_default();
                        let city_equivalent =
                            agg.ready_city_count.max((player.tile_count / 2000).max(1));
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
                            let target_count = bot_structure_target_count(
                                kind,
                                city_equivalent,
                                crate::game_config::BotDifficulty::Terminator,
                            );
                            if kind == BuildingKind::MissileSilo && owned >= 3 {
                                continue;
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
                        processed += 1;
                        continue;
                    } else {
                        let mut target_owner = targets[0];
                        if bot_iq >= 130 {
                            let mut weakest = targets[0];
                            let mut strongest = targets[0];
                            for &t_id in &targets {
                                if let (Some(p_t), Some(p_w), Some(p_s)) = (
                                    self.state.player(t_id),
                                    self.state.player(weakest),
                                    self.state.player(strongest),
                                ) {
                                    if p_t.troops < p_w.troops { weakest = t_id; }
                                    if p_t.troops > p_s.troops { strongest = t_id; }
                                }
                            }
                            if let (Some(p_me), Some(p_s)) = (
                                self.state.player(bot_id),
                                self.state.player(strongest),
                            ) {
                                if p_me.troops >= p_s.troops * 1.5 {
                                    target_owner = strongest;
                                } else {
                                    target_owner = weakest;
                                }
                            }
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
                        let p_send = (troops - reserve).max(0.0);
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
}

#[cfg(test)]
mod bot_ratio_tests {
    use super::bot_structure_target_count;
    use crate::game::BuildingKind;
    use crate::game_config::BotDifficulty;

    #[test]
    fn bot_structure_ratio_targets_follow_legacy_engine_like_values() {
        let city_equivalent = 10;
        let sam_vanilla = bot_structure_target_count(
            BuildingKind::SamLauncher,
            city_equivalent,
            BotDifficulty::Vanilla,
        );
        let sam_terminator = bot_structure_target_count(
            BuildingKind::SamLauncher,
            city_equivalent,
            BotDifficulty::Terminator,
        );
        let ports = bot_structure_target_count(
            BuildingKind::Port,
            city_equivalent,
            BotDifficulty::Terminator,
        );
        let silos = bot_structure_target_count(
            BuildingKind::MissileSilo,
            city_equivalent,
            BotDifficulty::Terminator,
        );
        assert_eq!(sam_vanilla, 2);
        assert_eq!(sam_terminator, 3);
        assert_eq!(ports, 7);
        assert_eq!(silos, 2);
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
        
        // Prior to income
        assert_eq!(engine.state.player(1).unwrap().iq_points, 50.0);
        assert_eq!(engine.state.player(2).unwrap().iq_points, 50.0);

        // Tick income
        engine.execute_income();

        // High IQ (135) should gain 1.35 points * multiplier
        assert_eq!(engine.state.player(1).unwrap().iq_points, 51.35);
        // Low IQ (85) should gain 0.85 points * multiplier
        assert_eq!(engine.state.player(2).unwrap().iq_points, 50.85);
    }

    #[test]
    fn test_low_iq_bot_indiscriminate_alliance() {
        let mut engine = test_engine_two_players(42);
        
        // Set Player 1 IQ to 90 so it doesn't immediately break the alliance due to being high IQ
        engine.state.player_mut(1).unwrap().iq = 90;

        // Let Player 1 propose alliance to Player 2 (Low IQ)
        engine.alliances_proposed.push((1, 2));

        // Run think cycles to process proposals
        for _ in 0..500 {
            engine.execute_ai_think();
            engine.state.tick += 1;
        }

        // They should have become allied since Player 2 is low IQ and accepts indiscriminately.
        let p1 = engine.state.player(1).unwrap();
        let p2 = engine.state.player(2).unwrap();
        assert!(p1.alliances.contains(&2) || p2.alliances.contains(&1));
    }

    #[test]
    fn test_high_iq_resource_sharing() {
        let mut engine = test_engine_two_players(42);

        // Make Player 1 and Player 2 allied
        engine.state.player_mut(1).unwrap().alliances.push(2);
        engine.state.player_mut(2).unwrap().alliances.push(1);

        // Set Player 2 (ally) in deep trouble (troops < 30% of max, max = 200, troops = 10)
        engine.state.player_mut(2).unwrap().troops = 10.0;

        // Player 1 has plenty of gold (300,000) and troops (1000, max = 1500), iq_points = 50.
        // Let's trigger thinking
        for _ in 0..500 {
            engine.execute_ai_think();
            engine.state.tick += 1;
        }

        // Player 2 should have received resources (troops & gold increased)
        let p2 = engine.state.player(2).unwrap();
        assert!(p2.troops > 10.0 || p2.gold > 10_000.0);
    }
}
