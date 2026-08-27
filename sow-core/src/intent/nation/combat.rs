use crate::diplomacy::maybe_betray_for_attack;
use crate::engine::SowEngine;
use crate::game::{BuildingKind, NukeKind};
use crate::protocol::{AttackIntent, GameplayIntent};
use crate::rng::NextIntExt;
use wyrand::WyRand;

use super::profile::{AiSlot, AiTier, BotDecision, BotDecisionKind};

impl SowEngine {
    pub(super) fn nation_run_combat_for_slot(
        &mut self,
        slot: &AiSlot,
        bot: (u16, u32),
        costs: (f64, f64),
        neighbor_players: &[u16],
        has_neutral: bool,
        decisions: &mut Vec<BotDecision>,
    ) {
        let (bot_id, bot_iq) = bot;
        let (attack_cost, alliance_cost) = costs;
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

                // Build candidate targets. Exclude allies AND teammates so a
                // bot never wastes its (rare) action deciding to hit a friend —
                // `apply_attack_intent` would silently block it anyway.
                // Passive tribes (attacks_players=false) additionally exclude
                // all player targets up front: they never initiate vs anyone.
                let targets: Vec<u16> = neighbor_players
                    .iter()
                    .copied()
                    .filter(|&id| {
                        if betray_then_attack == Some(id) {
                            return true;
                        }
                        if let Some(p_me) = self.state.player(bot_id) {
                            let is_ally = p_me.alliances.contains(&id);
                            let is_teammate = p_me.team.is_some()
                                && p_me.team == self.state.player(id).and_then(|t| t.team);
                            if is_ally || is_teammate {
                                return false;
                            }
                            if !slot.profile.attacks_players {
                                if let Some(t) = self.state.player(id) {
                                    if t.player_type != crate::player::PlayerType::Bot {
                                        return false; // passive tribe: skip players
                                    }
                                }
                            }
                            true
                        } else {
                            true
                        }
                    })
                    .collect();

                // Every Nation shares the same mid-tier capability set. The
                // action phase below remains seed/id-jittered, but the ID no
                // longer decides which Nation gets fleet behavior. Ghosts
                // (is_ai_controlled humans) get the same naval breakout so a
                // teammate fully enclosed by allies keeps advancing instead
                // of idling when its border has no enemy contact.
                let is_mfo = slot.tier == AiTier::Nation;
                let can_fleet = is_mfo || slot.tier == AiTier::Ghost;
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
                // The engine's fleet rule needs no port — the AI self-restricts to
                // ports. An enclosed ghost (allies/teammates on every border tile,
                // no neutral left) has no land move at all, so it may launch
                // portless: idle "comfortable" ghosts must never happen.
                let enclosed = !has_neutral && targets.is_empty();
                if can_fleet
                    && (has_port || (slot.tier == AiTier::Ghost && enclosed))
                    && troops >= max_troops * 0.20
                    && (self.state.tick + bot_id as u64).is_multiple_of(24)
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
                            if !is_friendly && p.troops < min_troops && !p.border_tiles.is_empty() {
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
                                if let Some(t_tile) = target_p.border_tiles.ones().nth(pick_idx) {
                                    let border_tiles =
                                        &self.state.player(bot_id).unwrap().border_tiles;
                                    if let Ok(_route) = crate::warp_fleet::resolve_fleet_route(
                                        &self.state.map,
                                        &self.water,
                                        &mut self.path_scratch,
                                        bot_id,
                                        (target_p_id, t_tile),
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
                    return;
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

                // Target precedence:
                //   1. Defend the biggest inbound attack.
                //   2. Nations: revenge / weakest bordering player.
                //   3. Attack-armed tiers with a player on their border keep
                //      pressing it — leftover neutral pockets must never stall
                //      a war, but they must also never block expansion when no
                //      player is reachable (that regression froze whole lobbies).
                //   4. Everyone: expand into neutral land.
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
                } else if slot.profile.attacks_players
                    && !targets.is_empty()
                    && troops >= max_troops * trigger_ratio
                {
                    let target_owner;
                    if bot_iq >= 130 {
                        let mut best_target = targets[0];
                        for &t_id in &targets {
                            if let (Some(p_t), Some(p_b)) =
                                (self.state.player(t_id), self.state.player(best_target))
                            {
                                let t_is_tribe = p_t.player_type == crate::player::PlayerType::Bot;
                                let b_is_tribe = p_b.player_type == crate::player::PlayerType::Bot;

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
                        return;
                    }

                    (target_owner, false)
                } else if has_neutral {
                    (0, true)
                } else if targets.is_empty() {
                    if slot.tier == AiTier::Nation {
                        self.maybe_launch_nuke(bot_id, decisions, bot_iq, &targets);
                    }
                    return;
                } else {
                    // Below trigger, enemy neighbors only, no neutral left:
                    // bank this tick and keep accumulating for the war push.
                    return;
                };

                let is_defending = defender_target.is_some();
                // Initiation against players is gated on `attacks_players`
                // (Vanilla tribes are passive food: they expand into neutral
                // land but never target another player). Active tiers may
                // initiate once their trigger threshold is reached.
                let can_initiate = slot.profile.attacks_players;
                if is_neutral
                    || is_defending
                    || (can_initiate && troops >= max_troops * trigger_ratio)
                {
                    let reserve = max_troops
                        * if is_neutral {
                            expand_ratio
                        } else if is_defending {
                            // Desperate defense: keep only half of standard reserve ratio
                            reserve_ratio * 0.5
                        } else {
                            reserve_ratio
                        };
                    let is_standard_bot = slot.tier == AiTier::Tribe;
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
                if slot.tier == AiTier::Nation {
                    self.maybe_launch_nuke(bot_id, decisions, bot_iq, &targets);
                }
            }
        }
    }

    pub(super) fn maybe_launch_nuke(
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
