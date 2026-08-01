use crate::building::aggregate_buildings_per_player;
use crate::engine::SowEngine;
use crate::protocol::StampedIntent;
use crate::rng::NextIntExt;
use wyrand::WyRand;

mod build;
mod combat;
mod diplomacy;
mod profile;
mod structures;

use profile::{AiSlot, BotDecision, get_bot_ai_profile};
use structures::{cheapest_gold_cost, iq_build_interval_base};

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

            let (neighbor_players, has_neutral) = self.nation_scan_neighbors(bot_id);

            self.nation_run_diplomacy_for_slot(
                (bot_id, bot_iq),
                (alliance_cost, send_cost),
                &neighbor_players,
                has_neutral,
                &mut decisions,
            );

            self.nation_run_structure_build_for_slot(
                slot,
                bot_id,
                bot_iq,
                build_cost,
                &mut decisions,
            );

            self.nation_run_combat_for_slot(
                slot,
                (bot_id, bot_iq),
                (attack_cost, alliance_cost),
                &neighbor_players,
                has_neutral,
                &mut decisions,
            );
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
}

#[cfg(test)]
mod tests;
