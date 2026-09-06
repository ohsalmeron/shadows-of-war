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

use profile::{AiSlot, BotDecision, ai_profile_for, ai_tier};
use structures::{cheapest_gold_cost, iq_build_interval_base};

impl SowEngine {
    /// Unified AI pipeline for both Tribes (`Bot`) and Nations.
    ///
    /// - Builds one combined schedule of all AI entities.
    /// - Every scheduled bot acts each tick (no global budget cap).
    /// - Each bot self-throttles via `iq_build_interval_base` keyed on tier.
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
            let Some(tier) = ai_tier(p.player_type, p.is_ai_controlled) else {
                continue; // real human: no AI brain
            };
            if !p.alive {
                continue;
            }
            let bot_id = p.id;

            let profile = ai_profile_for(tier, self.state.config.bot_difficulty);

            // Unified metronomic scheduler for Nations, Tribes, AND ghost
            // (is_ai_controlled) Humans. Tier is resolved once by
            // `ai_tier(player_type, is_ai_controlled)` — the single source of
            // truth. IQ (assigned per-tier at spawn) drives cadence; RNG is
            // WyRand(seed, bot_id, interval) → lockstep-safe across clients.
            let is_under_attack = p.iq >= 100
                && self
                    .attacks
                    .iter()
                    .any(|att| att.target_owner == bot_id && !p.alliances.contains(&att.owner_id));

            let interval_base = if is_under_attack {
                if p.iq >= 130 {
                    5 // Top/smart: react in 0.5s - 1.0s (5 - 10 ticks)
                } else {
                    10 // Advanced: react in 1.0s - 2.0s (10 - 20 ticks)
                }
            } else {
                iq_build_interval_base(tier)
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

            #[cfg(test)]
            if std::env::var("SOW_AI_DEBUG").is_ok() && bot_id == 1 && tick < 250 {
                eprintln!(
                    "SCHED id={bot_id} tick={tick} interval={interval} offset={offset} phase={phase} do_attack={do_attack}"
                );
            }
            if !do_attack && !do_structures {
                continue; // Nothing to do this tick for this entity
            }

            if do_structures && p.gold >= cheapest_gold_cost(&self.state.config) {
                any_structures = true;
            }

            schedule.push(AiSlot {
                bot_id,
                tier,
                do_attack,
                do_structures,
                profile,
            });
        }

        schedule.sort_unstable_by_key(|s| s.bot_id);
        if std::env::var("SOW_AI_DEBUG").is_ok() {
            let probes: Vec<(u16, usize)> = self
                .state
                .players
                .iter()
                .map(|p| (p.id, p.border_tiles.count_ones()))
                .collect();
            let mut neutral_probe: Vec<(u16, bool)> = Vec::new();
            for p in &self.state.players {
                let mut has = false;
                for raw in p.border_tiles.ones() {
                    let bx = raw % self.state.map.width;
                    let by = raw / self.state.map.width;
                    self.state.map.for_each_neighbor(bx, by, |nx, ny| {
                        if self.state.map.owner_id(nx, ny) == 0
                            && self.state.map.terrain[self.state.map.ref_id(nx, ny)].is_land()
                        {
                            has = true;
                        }
                    });
                }
                neutral_probe.push((p.id, has));
            }
            eprintln!(
                "AICALL tick={} sched={} ids={:?} borders={:?} neutral={:?}",
                self.state.tick,
                schedule.len(),
                schedule.iter().map(|s| s.bot_id).collect::<Vec<_>>(),
                probes,
                neutral_probe
            );
        }
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

#[cfg(test)]
#[path = "bot_lab.rs"]
mod bot_lab;
