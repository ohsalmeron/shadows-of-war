pub mod bot;
pub mod buildings;
pub mod combat;
pub mod fleet;
pub mod nation;

pub use combat::*;

use crate::engine::SowEngine;
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
            if (ex.owner_id == p1 && ex.target_owner == p2) || (ex.owner_id == p2 && ex.target_owner == p1) {
                ex.retreating = true;
            }
        }
        for wf in &mut self.fleets {
            if (wf.owner_id == p1 && wf.target_owner == p2) || (wf.owner_id == p2 && wf.target_owner == p1) {
                wf.retreating = true;
                wf.retreat_dst = None;
                wf.path.clear();
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
                    wf.path.clear();
                    wf.path_cursor = 0;
                    break;
                }
            }
            GameplayIntent::LaunchFleet {
                target_tile,
                troops,
            } => {
                let owner = self.state.map.state[*target_tile as usize];
                let is_allied = self.state.player(stamped.player_id).map(|p| p.alliances.contains(&owner)).unwrap_or(false);
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
                println!(
                "apply_stamped_intent: cancel attack_id={} for player {} — not found or not owner",
                attack_id, pid
            );
            }
            GameplayIntent::Attack(attack) => {
                let is_allied = self.state.player(stamped.player_id).map(|p| p.alliances.contains(&attack.target_owner)).unwrap_or(false);
                if !is_allied {
                    self.apply_attack_intent(stamped.player_id, attack, intent_index);
                }
            }
            GameplayIntent::BuildStructure { kind, target_tile } => {
                self.apply_build_structure_intent(stamped.player_id, *kind, *target_tile);
            }
            GameplayIntent::UpgradeStructure { building_id } => {
                self.apply_upgrade_structure_intent(stamped.player_id, *building_id);
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
                        // Clear old tiles for this player
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
            GameplayIntent::ExpressEmoji { emoji } => {
                if let Some(player) = self.state.player_mut(stamped.player_id) {
                    player.active_emoji = Some(emoji.clone());
                    player.emoji_timer = 30; // 3 seconds at 10 ticks per second
                }
            }
            GameplayIntent::ProposeAlliance { target_player } => {
                let proposer = stamped.player_id;
                let target = *target_player;
                if proposer != target {
                    let proposer_alive = self.state.player(proposer).map(|p| p.alive).unwrap_or(false);
                    let target_alive = self.state.player(target).map(|p| p.alive).unwrap_or(false);
                    if proposer_alive && target_alive {
                        let is_allied = self.state.player(proposer)
                            .map(|p| p.alliances.contains(&target))
                            .unwrap_or(false);
                        if !is_allied {
                            if self.alliances_proposed.contains(&(target, proposer)) {
                                // Mutual request! Accept it immediately.
                                let idx = self.alliances_proposed.iter().position(|&(p, t)| p == target && t == proposer).unwrap();
                                self.alliances_proposed.remove(idx);
                                if let Some(p1) = self.state.player_mut(proposer) {
                                    if !p1.alliances.contains(&target) { p1.alliances.push(target); }
                                }
                                if let Some(p2) = self.state.player_mut(target) {
                                    if !p2.alliances.contains(&proposer) { p2.alliances.push(proposer); }
                                }
                                self.retreat_mutual_aggression(proposer, target);
                            } else if !self.alliances_proposed.contains(&(proposer, target)) {
                                self.alliances_proposed.push((proposer, target));
                            }
                        }
                    }
                }
            }
            GameplayIntent::AcceptAlliance { target_player } => {
                let acceptor = stamped.player_id;
                let target = *target_player;
                let prop_idx = self.alliances_proposed.iter().position(|&(p, t)| p == target && t == acceptor);
                if let Some(idx) = prop_idx {
                    self.alliances_proposed.remove(idx);
                    if let Some(rev_idx) = self.alliances_proposed.iter().position(|&(p, t)| p == acceptor && t == target) {
                        self.alliances_proposed.remove(rev_idx);
                    }
                    if let Some(p1) = self.state.player_mut(acceptor) {
                        if !p1.alliances.contains(&target) {
                            p1.alliances.push(target);
                        }
                    }
                    if let Some(p2) = self.state.player_mut(target) {
                        if !p2.alliances.contains(&acceptor) {
                            p2.alliances.push(acceptor);
                        }
                    }
                    self.retreat_mutual_aggression(acceptor, target);
                }
            }
            GameplayIntent::RejectAlliance { target_player } => {
                let rejector = stamped.player_id;
                let target = *target_player;
                let prop_idx = self.alliances_proposed.iter().position(|&(p, t)| p == target && t == rejector);
                if let Some(idx) = prop_idx {
                    self.alliances_proposed.remove(idx);
                }
            }
            GameplayIntent::BreakAlliance { target_player } => {
                let breaker = stamped.player_id;
                let target = *target_player;
                if let Some(p1) = self.state.player_mut(breaker) {
                    p1.alliances.retain(|&id| id != target);
                    p1.active_emoji = Some("🗡️".to_string());
                    p1.emoji_timer = 50; // 5 seconds of betrayal icon
                }
                if let Some(p2) = self.state.player_mut(target) {
                    p2.alliances.retain(|&id| id != breaker);
                }
            }
            GameplayIntent::SendResources { target_player, gold, troops } => {
                let sender = stamped.player_id;
                let target = *target_player;
                let g = *gold;
                let t = *troops;
                if sender != target && g > 0.0 && t > 0.0 && !g.is_nan() && !t.is_nan() {
                    let mut actual_g = 0.0;
                    let mut actual_t = 0.0;
                    let mut sender_ok = false;
                    if let Some(s_player) = self.state.player_mut(sender) {
                        if s_player.alive {
                            actual_g = g.min(s_player.gold);
                            let max_t_to_send = (s_player.troops - 1.0).max(0.0);
                            actual_t = t.min(max_t_to_send);
                            s_player.gold -= actual_g;
                            s_player.troops -= actual_t;
                            sender_ok = true;
                        }
                    }
                    if sender_ok && (actual_g > 0.0 || actual_t > 0.0) {
                        if let Some(t_player) = self.state.player_mut(target) {
                            if t_player.alive {
                                t_player.gold += actual_g;
                                t_player.troops = (t_player.troops + actual_t).min(t_player.max_troops);
                            }
                        }
                    }
                }
            }
        }
    }
}
