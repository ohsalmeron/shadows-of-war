use crate::game::GamePhase;
use crate::rng::NextIntExt;
use crate::protocol::{AttackIntent, StampedIntent};
use crate::engine::SowEngine;

impl SowEngine {
    /// Deterministic bot AI execution.
    pub fn tick_bots(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }

        let _tick_now = self.state.tick;
        let map_width = self.state.map.width;
        let mut intents_to_apply = Vec::new();

        // 1. Evaluate intents
        for i in 0..self.state.players.len() {
            let player = &mut self.state.players[i];
            
            if player.is_human() || !player.alive {
                continue;
            }

            // 5% chance per tick to act
            if player.bot_rng.next_int(0, 100) >= 5 {
                continue;
            }

            // Bots shouldn't spend if they don't have base expansion cost
            if player.troops < self.state.config.attack_cost_neutral {
                continue;
            }

            let border_count = player.border_tiles.count_ones();
            if border_count == 0 {
                continue;
            }

            // Pick a random border tile
            let r_idx = player.bot_rng.next_int(0, border_count as i32) as usize;
            let chosen_border_idx = player.border_tiles.ones().nth(r_idx).unwrap();
            
            let bx = chosen_border_idx % map_width;
            let by = chosen_border_idx / map_width;

            // Find neighbors not owned by the bot
            let neighbors = self.state.map.neighbors(bx, by);
            let mut targets = Vec::new();
            for (nx, ny) in neighbors {
                let owner = self.state.map.owner_id(nx, ny);
                if owner != player.id {
                    let is_land = self.state.map.terrain[self.state.map.ref_id(nx as u32, ny as u32)].is_land();
                    if !is_land { continue; }
                    targets.push(owner);
                    if owner == 0 {
                        // Double priority for expanding into neutral territory
                        targets.push(0);
                        targets.push(0);
                    }
                }
            }

            if targets.is_empty() {
                continue;
            }

            // Pick random target
            let target_owner = targets[player.bot_rng.next_int(0, targets.len() as i32) as usize];
            let required_cost = if target_owner == 0 {
                self.state.config.attack_cost_neutral
            } else {
                self.state.config.attack_cost_enemy
            };

            if player.troops < required_cost {
                continue;
            }

            intents_to_apply.push(StampedIntent {
                player_id: player.id,
                intent: crate::protocol::GameplayIntent::Attack(AttackIntent { target_owner, troops: None }),
            });
        }

        // 2. Apply intents immediately
        self.apply_intents(&intents_to_apply);
    }
}
