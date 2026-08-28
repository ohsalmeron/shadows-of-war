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
            // War still spends iq_points (clamped at zero below); growth and
            // defense never freeze for lack of budget.
            {
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

                // D1 — OpenFront `sendBoatAttackToNearbyTerraNullius` parity:
                // with no free land on our own frontier, cross water to the
                // nearest neutral shores. Free like land expansion (growth,
                // not war); every tier including passive tribes — neutral
                // never means combat.
                if !has_neutral && self.try_expansion_boat(bot_id, decisions) {
                    return;
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
                // Portless breakout is tier-blind: an enclosed Nation starves
                // exactly like an enclosed Ghost (islands = zero land actions).
                // Vanilla tribes stay excluded (passive by design).
                if can_fleet
                    && (has_port || (enclosed && slot.tier != AiTier::Tribe))
                    && troops >= max_troops * 0.20
                    && (self.state.tick + bot_id as u64).is_multiple_of(24)
                {
                    let boat_send = (troops - (max_troops * 0.05)).max(0.0);
                    let mut best_target_p_id = None;
                    let mut best_overall_p_id = None;
                    let mut min_troops = f64::MAX;
                    let mut min_overall = f64::MAX;
                    for p in &self.state.players {
                        if p.alive && p.id != bot_id {
                            let is_friendly = {
                                let p_me = self.state.player(bot_id).unwrap();
                                p_me.alliances.contains(&p.id)
                                    || (p_me.team.is_some() && p_me.team == p.team)
                            };
                            if !is_friendly && !p.border_tiles.is_empty() {
                                if p.troops < min_overall {
                                    min_overall = p.troops;
                                    best_overall_p_id = Some(p.id);
                                }
                                // Same odds discipline as land initiation: no
                                // boat suicide into a dwarfing target.
                                let p_troops = p.troops.max(0.0);
                                let p_is_tribe =
                                    p.player_type == crate::player::PlayerType::Bot;
                                let odds_ok = if p_is_tribe {
                                    boat_send >= p_troops * 2.0
                                } else {
                                    boat_send >= p_troops * 0.20
                                };
                                if odds_ok && p.troops < min_troops {
                                    min_troops = p.troops;
                                    best_target_p_id = Some(p.id);
                                }
                            }
                        }
                    }
                    if best_target_p_id.is_none() {
                        // Enclosed with no odds-passing target: a desperate
                        // breakout beats idling — idle enclosed bots must
                        // never happen (the D2 contract).
                        best_target_p_id = best_overall_p_id.filter(|_| enclosed);
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
                let (mut target_owner, mut is_neutral) = (target_owner, is_neutral);

                // OF odds discipline (AiAttackBehavior parity) — initiation
                // only; defense and retaliation stay exempt:
                //   · vs tribes (`calculateBotAttackTroops`): strike with 4×
                //     the tribe's troops; if we can't afford that, our
                //     affordable wave must still be ≥2× or we bank —
                //     half-hearted pokes at food are how tribes balloon.
                //   · vs real players (`isAttackTooWeak`): never initiate with
                //     less than 20% of the target's troops; bleeding into a
                //     prepared defense (tile cost scales with the defender's
                //     TOTAL troops) only feeds the pile-on.
                //   · blocked with free land adjacent: grow instead (OF
                //     expansions precede wars) — never frozen.
                // Affordability keys on the STANDING ARMY, not max_troops:
                // max grows with territory while troops trail it for most of
                // the match, so a max-based reserve zeroes out every war
                // decision mid-expansion (OF can key on max — its armies sit
                // near cap).
                let mut odds_send: Option<f64> = None;
                // Team games are exempt from the PLAYER-target odds gates —
                // OF parity: troopSendCap/isAttackTooWeak return
                // Infinity/false when teammates back the attack. The tribe
                // window applies everywhere (tribes are the map's food).
                let is_team_game = self.state.config.game_mode != "FFA";
                if !is_neutral && !is_defending && bot_iq >= 130 {
                    let (target_troops, target_is_tribe) = self
                        .state
                        .player(target_owner)
                        .map(|p| {
                            (
                                p.troops.max(0.0),
                                p.player_type == crate::player::PlayerType::Bot,
                            )
                        })
                        .unwrap_or((0.0, false));
                    // Tribe targets: NO affordability floor. Tribes sit AT
                    // their troop cap (they never spend) while nations sit
                    // far below theirs (sweeps drain troops and every
                    // conquest raises max), so any troops-ratio window — OF's
                    // 2× included — structurally favors the tribe and
                    // re-freezes the map mid-game. The 4× sizing caps the
                    // send; the conquest math (5:1 → max power at 1×) makes
                    // even parity waves grind territory. FFA discipline stays
                    // for PLAYER targets only, and team games are exempt
                    // (OF: troopSendCap/isAttackTooWeak are FFA-only).
                    let affordable =
                        troops * (1.0 - slot.profile.reserve_ratio);
                    let committed = if target_is_tribe {
                        Some((target_troops * 4.0).min(affordable))
                    } else if is_team_game
                        || (affordable >= target_troops * 0.20 && target_troops < troops)
                    {
                        Some(affordable)
                    } else if let Some(tribe_alt) = targets.iter().copied().find(|id| {
                        self.state
                            .player(*id)
                            .map(|p| p.player_type == crate::player::PlayerType::Bot)
                            .unwrap_or(false)
                    }) {
                        // Odds-locked player target (FFA): swing at a tribe
                        // neighbor instead of banking — the attrition war on
                        // tribes is always open, and picking a locked target
                        // used to stall the whole AI while an attackable
                        // tribe sat on the same border.
                        let tt = self
                            .state
                            .player(tribe_alt)
                            .map(|p| p.troops.max(0.0))
                            .unwrap_or(0.0);
                        target_owner = tribe_alt;
                        Some((tt * 4.0).min(affordable))
                    } else {
                        None
                    };
                    match committed {
                        Some(s) => odds_send = Some(s),
                        None if has_neutral => {
                            target_owner = 0;
                            is_neutral = true;
                        }
                        None => return, // bank and accumulate for the real push
                    }
                }

                // A committed odds decision IS the war trigger — the classic
                // trigger ratio keys on max_troops, which explodes with
                // territory while troops trail it, so mid-expansion nations
                // could never pass it (they only wared true-late).
                let odds_committed = odds_send.is_some();
                if is_neutral
                    || is_defending
                    || odds_committed
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
                    let mut p_send = if is_standard_bot && !is_neutral && !is_defending {
                        (troops / 4.0).max(0.0)
                    } else {
                        (troops - reserve).max(0.0)
                    };
                    if let Some(s) = odds_send {
                        p_send = s;
                    }
                    if p_send >= self.state.config.attack_cost_neutral {
                        // Neutral expansion is GROWTH, not war: it must stay
                        // free of the iq budget, or a high-cadence bot drains
                        // its points on contested frontiers and permanently
                        // freezes mid-game (bankruptcy = zero actions, troops
                        // piling at cap while free land sits next door).
                        if !is_neutral {
                            if let Some(p_me) = self.state.player_mut(bot_id) {
                                p_me.iq_points = (p_me.iq_points - attack_cost).max(0.0);
                            }
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

    /// OpenFront `sendBoatAttackToNearbyTerraNullius` parity: probe random
    /// neutral land tiles and sail an expansion wave to the first reachable
    /// one. No port, no player target, no iq cost — pure growth.
    pub(super) fn try_expansion_boat(
        &mut self,
        bot_id: u16,
        decisions: &mut Vec<BotDecision>,
    ) -> bool {
        use crate::rng::NextIntExt;
        use crate::warp_fleet::resolve_fleet_route;
        use wyrand::WyRand;

        let Some(p0) = self.state.player(bot_id) else {
            return false;
        };
        let (troops, max_troops) = (p0.troops, p0.max_troops);
        let (width, height) = (self.state.map.width, self.state.map.height);
        let reserve = max_troops * 0.10; // OF expandRatio band (10–20%)
        let send = (troops - reserve).max(0.0);
        if send < self.state.config.attack_cost_neutral {
            return false;
        }
        if std::env::var("SOW_AI_DEBUG").is_ok() {
            eprintln!("TNBOAT enter id={bot_id} troops={troops:.0}");
        }
        let border = p0.border_tiles.clone();
        let mut rng = WyRand::new(
            self.state
                .seed
                .wrapping_add(bot_id as u64)
                .wrapping_mul(0x9E3779B97F4A7C15)
                .wrapping_add(self.state.tick as u64),
        );
        for sample in 0..8 {
            let tx = rng.next_int(0, width as i32).max(0) as u32;
            let ty = rng.next_int(0, height as i32).max(0) as u32;
            let owner = self.state.map.owner_id(tx, ty);
            let is_land = self.state.map.terrain[self.state.map.ref_id(tx, ty)].is_land();
            if std::env::var("SOW_AI_DEBUG").is_ok() {
                eprintln!("SMP id={bot_id} tick={} s={sample} t=({tx},{ty}) owner={owner} land={is_land}", self.state.tick);
            }
            if owner != 0 {
                continue;
            }
            if !is_land {
                continue;
            }
            if std::env::var("SOW_AI_DEBUG").is_ok() {
                eprintln!("TNBOAT sample hit t={}", self.state.tick);
            }
            let route = resolve_fleet_route(
                &self.state.map,
                &self.water,
                &mut self.path_scratch,
                bot_id,
                (0, ty * width + tx),
                &border,
                None,
            );
            if std::env::var("SOW_AI_DEBUG").is_ok() {
                eprintln!(
                    "ROUTE t={} target=({tx},{ty}) ok={} err={:?}",
                    self.state.tick,
                    route.is_ok(),
                    route.as_ref().err()
                );
            }
            if route.is_ok() {
                if let Some(p) = self.state.player_mut(bot_id) {
                    p.troops -= send;
                }
                decisions.push(BotDecision {
                    bot_id,
                    kind: BotDecisionKind::Attack,
                    intent: GameplayIntent::LaunchFleet {
                        target_tile: ty * width + tx,
                        troops: Some(send),
                    },
                });
                return true;
            }
        }
        false
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
