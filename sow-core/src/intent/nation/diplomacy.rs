use crate::diplomacy::{
    alliance_propose_roll_cap, is_valid_alliance_target, should_reject_traitor_request,
    ALLIANCE_RENEWAL_WINDOW_TICKS,
};
use crate::engine::SowEngine;
use crate::player::PlayerType;
use crate::protocol::GameplayIntent;
use crate::rng::NextIntExt;
use wyrand::WyRand;

use super::profile::{BotDecision, BotDecisionKind};

impl SowEngine {
    pub(super) fn nation_scan_neighbors(&mut self, bot_id: u16) -> (Vec<u16>, bool) {
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

        (neighbor_players, has_neutral)
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn nation_run_diplomacy_for_slot(
        &mut self,
        bot_id: u16,
        bot_iq: u32,
        alliance_cost: f64,
        send_cost: f64,
        neighbor_players: &[u16],
        has_neutral: bool,
        decisions: &mut Vec<BotDecision>,
    ) {
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
            if self.state.tick > share_interval && self.state.tick.is_multiple_of(share_interval) {
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

            for &neighbor in neighbor_players {
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
    }
}
