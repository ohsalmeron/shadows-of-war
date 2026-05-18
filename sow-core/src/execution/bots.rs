use crate::engine::SowEngine;
use crate::game::GamePhase;
use crate::protocol::{AttackIntent, StampedIntent};
use crate::rng::NextIntExt;

impl SowEngine {
    /// Deterministic bot AI execution.
    pub fn tick_bots(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }

        let _tick_now = self.state.tick;
        let mut intents_to_apply = Vec::new();

        // 1. Evaluate intents
        for i in 0..self.state.players.len() {
            let map_width = self.state.map.width;

            let (bx, by, my_team, my_id) = {
                let player = &mut self.state.players[i];

                if player.is_human() || !player.alive {
                    continue;
                }

                let border_count = player.border_tiles.count_ones();
                if border_count == 0 {
                    continue;
                }

                if _tick_now % self.state.config.bot_attack_interval_ticks
                    != (player.id as u64 % self.state.config.bot_attack_interval_ticks)
                {
                    continue;
                }

                let r_idx = player.bot_rng.next_int(0, border_count as i32) as usize;
                let chosen_border_idx = player.border_tiles.ones().nth(r_idx).unwrap();

                (
                    chosen_border_idx % map_width,
                    chosen_border_idx / map_width,
                    player.team,
                    player.id,
                )
            };

            let neighbors = self.state.map.neighbors(bx, by);
            let mut targets = Vec::new();
            for (nx, ny) in neighbors {
                let owner = self.state.map.owner_id(nx, ny);
                if owner != my_id {
                    let is_land = self.state.map.terrain[self.state.map.ref_id(nx, ny)].is_land();
                    if !is_land {
                        continue;
                    }

                    if owner != 0 {
                        if let Some(target_player) =
                            self.state.players.iter().find(|p| p.id == owner)
                        {
                            if my_team.is_some() && my_team == target_player.team {
                                continue;
                            }
                        }
                    }

                    targets.push(owner);
                    if owner == 0 {
                        targets.push(0);
                        targets.push(0);
                    }
                }
            }

            if targets.is_empty() {
                continue;
            }

            let target_owner = {
                let player = &mut self.state.players[i];
                targets[player.bot_rng.next_int(0, targets.len() as i32) as usize]
            };

            let required_cost = if target_owner == 0 {
                self.state.config.attack_cost_neutral
            } else {
                self.state.config.attack_cost_enemy
            };

            if self.state.players[i].troops < required_cost {
                continue;
            }

            intents_to_apply.push(StampedIntent {
                player_id: my_id,
                intent: crate::protocol::GameplayIntent::Attack(AttackIntent {
                    target_owner,
                    troops: None,
                }),
            });
        }

        // 2. Apply intents immediately
        self.apply_intents(&intents_to_apply);
    }
}
