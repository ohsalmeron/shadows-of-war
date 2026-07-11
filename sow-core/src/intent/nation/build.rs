use crate::building::{structure_build_cost_gold, structure_kind_enabled};
use crate::engine::SowEngine;
use crate::game::BuildingKind;
use crate::protocol::GameplayIntent;
use crate::rng::NextIntExt;

use super::profile::{AiSlot, BotDecision, BotDecisionKind};
use super::structures::{
    bot_structure_target_count, cheapest_gold_cost, resolve_structure_from_candidates,
    stack_build_decision, StructureCandidates, PLACEMENT_ATTEMPTS,
};

impl SowEngine {
    pub(super) fn nation_run_structure_build_for_slot(
        &mut self,
        slot: &AiSlot,
        bot_id: u16,
        bot_iq: u32,
        build_cost: f64,
        decisions: &mut Vec<BotDecision>,
    ) {
        // ── Structure building (IQ-scaled, all bots) ─────────────────
        if slot.do_structures {
            let current_points = self.state.player(bot_id).unwrap().iq_points;
            if current_points >= build_cost {
                let (player_gold, player_tile_count) = {
                    if let Some(player) = self.state.player(bot_id) {
                        (player.gold, player.tile_count)
                    } else {
                        return;
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
                    let mut build_order = [
                        BuildingKind::Bunker,
                        BuildingKind::City,
                        BuildingKind::Factory,
                        BuildingKind::Port,
                    ];
                    build_order.sort_by(|&a, &b| {
                        let levels_a = agg.levels_of_kind(a);
                        let levels_b = agg.levels_of_kind(b);
                        let cost_a = structure_build_cost_gold(a, levels_a, &self.state.config);
                        let cost_b = structure_build_cost_gold(b, levels_b, &self.state.config);
                        cost_a
                            .partial_cmp(&cost_b)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    });
                    for kind in build_order {
                        if !structure_kind_enabled(kind) {
                            continue;
                        }
                        let owned = agg.total_structures_of_kind(kind);
                        let level_count = agg.levels_of_kind(kind);
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
                        let total_owned =
                            agg.count_city + agg.count_bunker + agg.count_factory + agg.count_port;
                        let density = total_owned as f32 / player_tile_count.max(1) as f32;
                        let is_density_high = bot_iq >= 110 && density > 1.0 / 600.0;
                        let structure_floor = player_tile_count / 800;
                        let under_structure_floor = total_owned < structure_floor;
                        let wants_new = owned < target_count || under_structure_floor;
                        let cost = structure_build_cost_gold(kind, level_count, &self.state.config);

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
                            let border_tiles_vec: Vec<u32> = p.border_tiles.ones().collect();
                            let border_len = border_tiles_vec.len();
                            for _ in 0..PLACEMENT_ATTEMPTS {
                                if border_len > 0 {
                                    let pick = p.bot_rng.next_int(0, border_len as i32) as usize;
                                    border.push(border_tiles_vec[pick]);
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
                        if let Some(d) =
                            stack_build_decision(&self.buildings, bot_id, kind, player_gold, cost)
                        {
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
    }
}
