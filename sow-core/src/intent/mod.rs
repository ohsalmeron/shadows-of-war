pub mod buildings;
pub mod combat;
pub mod fleet;
pub mod nation;
pub mod nukes;

pub use combat::*;

use crate::diplomacy::{
    ALLIANCE_DURATION_TICKS, ALLIANCE_RENEWAL_WINDOW_TICKS, BOT_BETRAYAL_EMOJI_TICKS,
    HUMAN_BETRAYAL_EMOJI_TICKS, TRAITOR_STATUS_TICKS,
};
use crate::engine::SowEngine;
use crate::player::PlayerType;
use crate::protocol::{GameplayIntent, StampedIntent};

/// Merge two frontiers: same cell keeps the tile with lower `priority` (better for expansion order).
///
/// **Determinism**: We do NOT use a `HashMap` for dedup because `HashMap` iteration
/// order is randomised by SipHash.  When two tiles at the same `(x,y)` share the
/// exact same `priority`, the `HashMap` would non-deterministically choose which
/// `insert_seq` to keep, silently diverging the `BinaryHeap` across clients.
/// Instead we sort deterministically and `dedup_by_key` — the first entry per key
/// (lowest priority, then lowest insert_seq, then spatial) survives.
impl SowEngine {
    pub fn apply_intents(&mut self, intents: &[StampedIntent]) {
        for (i, stamped) in intents.iter().enumerate() {
            self.apply_stamped_intent(stamped, i as u32);
        }
    }

    pub fn retreat_mutual_aggression(&mut self, p1: u16, p2: u16) {
        for ex in &mut self.attacks {
            if (ex.owner_id == p1 && ex.target_owner == p2)
                || (ex.owner_id == p2 && ex.target_owner == p1)
            {
                ex.retreating = true;
            }
        }
        for wf in &mut self.fleets {
            if (wf.owner_id == p1 && wf.target_owner == p2)
                || (wf.owner_id == p2 && wf.target_owner == p1)
            {
                wf.retreating = true;
                wf.retreat_dst = None;
                wf.path = std::sync::Arc::new(Vec::new());
                wf.path_cursor = 0;
            }
        }
    }

    pub fn apply_stamped_intent(&mut self, stamped: &StampedIntent, intent_index: u32) {
        match &stamped.intent {
            GameplayIntent::RecallFleet { fleet_id } => {
                let pid = stamped.player_id;
                for wf in &mut self.fleets {
                    if wf.id != *fleet_id {
                        continue;
                    }
                    if wf.owner_id != pid {
                        continue;
                    }
                    wf.retreating = true;
                    wf.retreat_dst = None;
                    wf.path = std::sync::Arc::new(Vec::new());
                    wf.path_cursor = 0;
                    break;
                }
            }
            GameplayIntent::LaunchFleet {
                target_tile,
                troops,
            } => {
                let owner = self.state.map.state[*target_tile as usize];
                let is_betrayer = self
                    .state
                    .player(owner)
                    .map(|p| p.active_emoji.as_deref() == Some("🗡️"))
                    .unwrap_or(false);
                let is_allied_in_list = self
                    .state
                    .player(stamped.player_id)
                    .map(|p| p.alliances.contains(&owner))
                    .unwrap_or(false);

                if is_allied_in_list && is_betrayer {
                    // Silently break the alliance without any penalty for the attacker
                    let attacker = stamped.player_id;
                    if let Some(p1) = self.state.player_mut(attacker) {
                        p1.alliances.retain(|&id| id != owner);
                        p1.alliance_timers.remove(&owner);
                    }
                    if let Some(p2) = self.state.player_mut(owner) {
                        p2.alliances.retain(|&id| id != attacker);
                        p2.alliance_timers.remove(&attacker);
                    }
                }

                let is_allied = self
                    .state
                    .player(stamped.player_id)
                    .map(|p| p.alliances.contains(&owner))
                    .unwrap_or(false);
                if !is_allied {
                    self.apply_launch_fleet_intent(stamped.player_id, *target_tile, *troops);
                }
            }
            GameplayIntent::CancelAttack { attack_id } => {
                let pid = stamped.player_id;
                for ex in &mut self.attacks {
                    if ex.id == *attack_id && ex.owner_id == pid {
                        ex.retreating = true;
                        return;
                    }
                }
                log::debug!(
                    "apply_stamped_intent: cancel attack_id={} for player {} — not found or not owner",
                    attack_id, pid
                );
            }
            GameplayIntent::Attack(attack) => {
                let owner = attack.target_owner;
                let is_betrayer = self
                    .state
                    .player(owner)
                    .map(|p| p.active_emoji.as_deref() == Some("🗡️"))
                    .unwrap_or(false);
                let is_allied_in_list = self
                    .state
                    .player(stamped.player_id)
                    .map(|p| p.alliances.contains(&owner))
                    .unwrap_or(false);

                if is_allied_in_list && is_betrayer {
                    // Silently break the alliance without any penalty for the attacker
                    let attacker = stamped.player_id;
                    if let Some(p1) = self.state.player_mut(attacker) {
                        p1.alliances.retain(|&id| id != owner);
                        p1.alliance_timers.remove(&owner);
                    }
                    if let Some(p2) = self.state.player_mut(owner) {
                        p2.alliances.retain(|&id| id != attacker);
                        p2.alliance_timers.remove(&attacker);
                    }
                }

                let is_allied = self
                    .state
                    .player(stamped.player_id)
                    .map(|p| p.alliances.contains(&owner))
                    .unwrap_or(false);
                if !is_allied {
                    self.apply_attack_intent(stamped.player_id, attack, intent_index);
                }
            }
            GameplayIntent::BuildStructure { kind, target_tile } => {
                self.apply_build_structure_intent(stamped.player_id, *kind, *target_tile);
            }
            GameplayIntent::UpgradeStructure { .. } => {
                // Stacking is handled inside BuildStructure; this intent is unused.
            }
            GameplayIntent::UpgradeCityModule {
                building_id,
                module,
            } => {
                self.apply_upgrade_city_module_intent(stamped.player_id, *building_id, *module);
            }
            GameplayIntent::UpgradeTile { tile_idx } => {
                self.apply_upgrade_tile_intent(stamped.player_id, *tile_idx);
            }
            GameplayIntent::BuildShip { port_tile, kind } => {
                let pid = stamped.player_id;
                let cost = kind.gold_cost();
                let port_id = self
                    .buildings
                    .iter()
                    .find(|b| {
                        b.tile_idx == *port_tile
                            && b.kind == crate::game::BuildingKind::City
                            && b.modules.port > 0
                            && b.owner_id == pid
                            && !b.under_construction
                    })
                    .map(|b| b.id);

                if let Some(port_id) = port_id {
                    if let Some(player) = self.state.player_mut(pid) {
                        if player.gold >= cost {
                            player.gold -= cost;
                            let queue = self.port_queues.entry(port_id).or_default();
                            queue.push_back(crate::game::ShipProduction {
                                kind: *kind,
                                ticks_until_complete: kind.build_duration_ticks(),
                            });
                        }
                    }
                }
            }
            GameplayIntent::MoveWarships {
                unit_ids,
                target_tile,
            } => {
                let pid = stamped.player_id;
                let target = *target_tile;
                let w = self.state.map.width;
                let area = w.saturating_mul(self.state.map.height);
                if target >= area {
                    return;
                }
                for uid in unit_ids {
                    if let Some(fleet) = self.fleets.iter_mut().find(|f| {
                        f.id == *uid
                            && f.owner_id == pid
                            && f.unit_type == crate::game::UnitType::Warship
                    }) {
                        // Try sea lane routing first (Dijkstra on ~20 port nodes)
                        let lane_path = if !self.state.sea_lanes.is_empty() {
                            let src_comp = self.water.component_of(fleet.current_tile);
                            let dst_comp = self.water.component_of(target);
                            if src_comp > 0 && src_comp == dst_comp {
                                let src_port = crate::sea_lane::closest_port_on_component(
                                    &self.buildings,
                                    &self.state.map,
                                    &self.water,
                                    fleet.current_tile,
                                    src_comp,
                                );
                                let dst_port = crate::sea_lane::closest_port_on_component(
                                    &self.buildings,
                                    &self.state.map,
                                    &self.water,
                                    target,
                                    dst_comp,
                                );
                                match (src_port, dst_port) {
                                    (Some(sp), Some(dp)) => crate::sea_lane::route_through_lanes(
                                        &self.state.sea_lanes,
                                        sp,
                                        dp,
                                    ),
                                    _ => None,
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        let path = lane_path.or_else(|| {
                            self.path_scratch.astar.find_path(
                                &self.state.map,
                                &[fleet.current_tile],
                                target,
                            )
                        });

                        if let Some(path) = path {
                            fleet.path = std::sync::Arc::new(path);
                            fleet.path_cursor = 0;
                            fleet.retreating = false;
                        }
                    }
                }
            }
            GameplayIntent::LaunchNuke { target_tile, .. } => {
                self.apply_launch_nuke_intent(stamped.player_id, *target_tile);
            }
            GameplayIntent::Spawn { x, y } => {
                if let crate::game::GamePhase::Spawning { .. } = self.state.phase {
                    let x = *x;
                    let y = *y;
                    let pid = stamped.player_id;

                    if self.state.map.is_valid_coord(x as i32, y as i32)
                        && self.state.map.terrain[self.state.map.ref_id(x, y)].is_land()
                        && self.state.map.owner_id(x, y) == 0
                    {
                        // Clear old tiles and buildings for this player
                        let w = self.state.map.width;
                        let mut to_clear = Vec::new();
                        for (i, &owner) in self.state.map.state.iter().enumerate() {
                            if owner == pid {
                                to_clear.push(i as u32);
                            }
                        }
                        for i in to_clear {
                            self.state.set_tile_owner(i % w, i / w, 0);
                        }
                        self.buildings.retain(|b| b.owner_id != pid);

                        // Set new spawn
                        self.state.place_spawn(pid, x, y);
                    }
                }
            }
            GameplayIntent::Resign => {
                self.kill_player(stamped.player_id);
            }
            GameplayIntent::MarkDisconnected { is_disconnected } => {
                if let Some(player) = self.state.player_mut(stamped.player_id) {
                    player.disconnected = *is_disconnected;
                }
            }
            GameplayIntent::ExpressEmoji { emoji, pinned } => {
                if let Some(player) = self.state.player_mut(stamped.player_id) {
                    player.active_emoji = Some(emoji.clone());
                    player.emoji_pinned = *pinned;
                    player.emoji_timer = if *pinned { 0 } else { 150 };
                }
            }
            GameplayIntent::ProposeAlliance { target_player } => {
                let proposer = stamped.player_id;
                let target = *target_player;
                if proposer != target {
                    let proposer_alive = self
                        .state
                        .player(proposer)
                        .map(|p| p.alive)
                        .unwrap_or(false);
                    let target_alive = self.state.player(target).map(|p| p.alive).unwrap_or(false);
                    if proposer_alive && target_alive {
                        let is_teammate = {
                            let p_prop = self.state.player(proposer).unwrap();
                            let p_target = self.state.player(target).unwrap();
                            p_prop.team.is_some() && p_prop.team == p_target.team
                        };
                        let (is_allied, can_renew) = self
                            .state
                            .player(proposer)
                            .map(|p| {
                                let allied = p.alliances.contains(&target);
                                let timer = p.alliance_timers.get(&target).copied().unwrap_or(0);
                                (
                                    allied,
                                    allied && timer <= ALLIANCE_RENEWAL_WINDOW_TICKS,
                                )
                            })
                            .unwrap_or((false, false));

                        if is_teammate {
                            log::warn!("ABERRATION: Player {} tried to propose alliance to teammate/ally {} in team game", proposer, target);
                            return;
                        }

                        if is_allied && !can_renew {
                            // Benign race: another bot's AcceptAlliance in the same
                            // tick batch already created this alliance before our
                            // ProposeAlliance was applied. Silently skip.
                            return;
                        }

                        if !is_allied || can_renew {
                            if self.has_alliance_proposal(target, proposer) {
                                // Mutual request! Accept/Renew it immediately.
                                let idx = self
                                    .alliances_proposed
                                    .iter()
                                    .position(|p| p.proposer == target && p.target == proposer)
                                    .unwrap();
                                self.alliances_proposed.remove(idx);
                                if let Some(p1) = self.state.player_mut(proposer) {
                                    if !p1.alliances.contains(&target) {
                                        p1.alliances.push(target);
                                    }
                                    p1.alliance_timers
                                        .insert(target, ALLIANCE_DURATION_TICKS);
                                }
                                if let Some(p2) = self.state.player_mut(target) {
                                    if !p2.alliances.contains(&proposer) {
                                        p2.alliances.push(proposer);
                                    }
                                    p2.alliance_timers
                                        .insert(proposer, ALLIANCE_DURATION_TICKS);
                                }
                                self.retreat_mutual_aggression(proposer, target);
                            } else if self.can_send_alliance_request(proposer, target) {
                                self.push_alliance_proposal(proposer, target);
                            }
                        }
                    }
                }
            }
            GameplayIntent::AcceptAlliance { target_player } => {
                let acceptor = stamped.player_id;
                let target = *target_player;
                let prop_idx = self
                    .alliances_proposed
                    .iter()
                    .position(|p| p.proposer == target && p.target == acceptor);
                if let Some(idx) = prop_idx {
                    self.alliances_proposed.remove(idx);
                    if let Some(rev_idx) = self.alliances_proposed.iter().position(|p| {
                        p.proposer == acceptor && p.target == target
                    }) {
                        self.alliances_proposed.remove(rev_idx);
                    }
                    if let Some(p1) = self.state.player_mut(acceptor) {
                        if !p1.alliances.contains(&target) {
                            p1.alliances.push(target);
                        }
                        p1.alliance_timers
                            .insert(target, ALLIANCE_DURATION_TICKS);
                    }
                    if let Some(p2) = self.state.player_mut(target) {
                        if !p2.alliances.contains(&acceptor) {
                            p2.alliances.push(acceptor);
                        }
                        p2.alliance_timers
                            .insert(acceptor, ALLIANCE_DURATION_TICKS);
                    }
                    self.retreat_mutual_aggression(acceptor, target);
                }
            }
            GameplayIntent::RejectAlliance { target_player } => {
                let rejector = stamped.player_id;
                let target = *target_player;
                let prop_idx = self
                    .alliances_proposed
                    .iter()
                    .position(|p| p.proposer == target && p.target == rejector);
                if let Some(idx) = prop_idx {
                    self.alliances_proposed.remove(idx);
                    self.mark_alliance_request_cooldown(target, rejector);
                }
            }
            GameplayIntent::BreakAlliance { target_player } => {
                let breaker = stamped.player_id;
                let target = *target_player;
                let emoji_ticks = self
                    .state
                    .player(breaker)
                    .map(|p| {
                        if p.player_type == PlayerType::Human {
                            HUMAN_BETRAYAL_EMOJI_TICKS
                        } else {
                            BOT_BETRAYAL_EMOJI_TICKS
                        }
                    })
                    .unwrap_or(BOT_BETRAYAL_EMOJI_TICKS);
                let traitor_until = self
                    .current_tick_u32()
                    .saturating_add(TRAITOR_STATUS_TICKS);
                if let Some(p1) = self.state.player_mut(breaker) {
                    p1.alliances.retain(|&id| id != target);
                    p1.alliance_timers.remove(&target);
                    p1.traitor = true;
                    p1.traitor_tick = traitor_until;
                    p1.active_emoji = Some("🗡️".to_string());
                    p1.emoji_timer = emoji_ticks;
                }
                self.mark_betrayal_cooldown(breaker);
                if let Some(p2) = self.state.player_mut(target) {
                    p2.alliances.retain(|&id| id != breaker);
                    p2.alliance_timers.remove(&breaker);
                }
            }
            GameplayIntent::SendResources {
                target_player,
                gold,
                troops,
            } => {
                let sender = stamped.player_id;
                let target = *target_player;
                let g = *gold;
                let t = *troops;
                let is_allied = self
                    .state
                    .player(sender)
                    .map(|p| {
                        p.alliances.contains(&target)
                            || (p.team.is_some()
                                && p.team
                                    == self.state.player(target).and_then(|t| t.team))
                    })
                    .unwrap_or(false);
                if sender != target && is_allied && (g > 0.0 || t > 0.0) && !g.is_nan() && !t.is_nan() {
                    let mut actual_g = 0.0;
                    let mut actual_t = 0.0;
                    let mut sender_ok = false;
                    if let Some(s_player) = self.state.player_mut(sender) {
                        if s_player.alive {
                            actual_g = if g > 0.0 { g.min(s_player.gold) } else { 0.0 };
                            let max_t_to_send = (s_player.troops - 1.0).max(0.0);
                            actual_t = if t > 0.0 { t.min(max_t_to_send) } else { 0.0 };
                            s_player.gold -= actual_g;
                            s_player.troops -= actual_t;
                            sender_ok = true;
                        }
                    }
                    if sender_ok && (actual_g > 0.0 || actual_t > 0.0) {
                        if let Some(t_player) = self.state.player_mut(target) {
                            if t_player.alive {
                                t_player.gold += actual_g;
                                t_player.troops =
                                    (t_player.troops + actual_t).min(t_player.max_troops);
                            }
                        }
                        self.state.events.push(
                            crate::game::GameEvent::ResourceTransferred {
                                sender_id: sender,
                                receiver_id: target,
                                gold: actual_g,
                                troops: actual_t,
                            },
                        );
                    }
                }
            }
            GameplayIntent::RequestResources {
                target_player,
                gold,
                troops,
            } => {
                let proposer = stamped.player_id;
                let target = *target_player;
                let g = *gold;
                let t = *troops;
                if proposer != target && (g > 0.0 || t > 0.0) && !g.is_nan() && !t.is_nan() {
                    let proposer_alive = self.state.player(proposer).map(|p| p.alive).unwrap_or(false);
                    let target_alive = self.state.player(target).map(|p| p.alive).unwrap_or(false);
                    if proposer_alive && target_alive {
                        // Clear any existing request between these two
                        self.resource_requests_proposed.retain(|r| !(r.proposer == proposer && r.target == target));
                        self.resource_requests_proposed.push(crate::engine::ResourceRequestProposed {
                            proposer,
                            target,
                            gold: g,
                            troops: t,
                        });
                    }
                }
            }
            GameplayIntent::AcceptResourceRequest { target_player } => {
                let acceptor = stamped.player_id;
                let target = *target_player; // target here is the proposer
                if let Some(pos) = self.resource_requests_proposed.iter().position(|r| r.proposer == target && r.target == acceptor) {
                    let req = self.resource_requests_proposed.remove(pos);
                    let mut actual_g = 0.0;
                    let mut actual_t = 0.0;
                    let mut acceptor_ok = false;
                    // Acceptor pays the resources
                    if let Some(acc_player) = self.state.player_mut(acceptor) {
                        if acc_player.alive {
                            actual_g = if req.gold > 0.0 { req.gold.min(acc_player.gold) } else { 0.0 };
                            let max_t_to_send = (acc_player.troops - 1.0).max(0.0);
                            actual_t = if req.troops > 0.0 { req.troops.min(max_t_to_send) } else { 0.0 };
                            acc_player.gold -= actual_g;
                            acc_player.troops -= actual_t;
                            acceptor_ok = true;
                        }
                    }
                    // Proposer receives the resources
                    if acceptor_ok && (actual_g > 0.0 || actual_t > 0.0) {
                        if let Some(prop_player) = self.state.player_mut(target) {
                            if prop_player.alive {
                                prop_player.gold += actual_g;
                                prop_player.troops = (prop_player.troops + actual_t).min(prop_player.max_troops);
                            }
                        }
                    }
                }
            }
            GameplayIntent::RejectResourceRequest { target_player } => {
                let rejector = stamped.player_id;
                let target = *target_player; // target here is the proposer
                if let Some(pos) = self.resource_requests_proposed.iter().position(|r| r.proposer == target && r.target == rejector) {
                    self.resource_requests_proposed.remove(pos);
                    self.state.events.push(
                        crate::game::GameEvent::ResourceRequestRejected {
                            rejector_id: rejector,
                            requester_id: target,
                        }
                    );
                }
            }
        }
    }
}
