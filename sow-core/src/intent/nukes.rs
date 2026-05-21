use crate::engine::SowEngine;
use crate::game::{BuildingKind, GamePhase, NukeKind, ProjectileKind};

/// Silo cooldown after launching (must match execution/nukes.rs).
const SILO_COOLDOWN_TICKS: u32 = 90;

impl SowEngine {
    pub fn apply_launch_nuke_intent(&mut self, player_id: u16, kind: NukeKind, target_tile: u32) {
        if self.state.phase != GamePhase::Playing {
            return;
        }
        let Some(player) = self.state.player(player_id) else {
            return;
        };
        if !player.alive {
            return;
        }

        let w = self.state.map.width;
        let area = w.saturating_mul(self.state.map.height);
        if target_tile >= area {
            return;
        }

        // Find the closest ready silo that's not on cooldown
        let tx = (target_tile % w) as i32;
        let ty = (target_tile / w) as i32;

        let best_silo = self.buildings.iter()
            .filter(|b| {
                b.kind == BuildingKind::MissileSilo
                    && b.owner_id == player_id
                    && !b.under_construction
                    && self.silo_cooldowns.get(&b.id).copied().unwrap_or(0) == 0
            })
            .min_by_key(|b| {
                let bx = (b.tile_idx % w) as i32;
                let by = (b.tile_idx / w) as i32;
                (bx - tx).abs() + (by - ty).abs()
            });

        let Some(silo) = best_silo else {
            return;
        };
        let silo_id = silo.id;
        let silo_tile = silo.tile_idx;

        // Check gold
        let prev_mirv = self.mirv_launches.get(&player_id).copied().unwrap_or(0);
        let cost = kind.gold_cost(prev_mirv);

        let Some(player_mut) = self.state.player_mut(player_id) else {
            return;
        };
        if player_mut.gold < cost {
            return;
        }
        player_mut.gold -= cost;

        // Set silo cooldown
        self.silo_cooldowns.insert(silo_id, SILO_COOLDOWN_TICKS);

        // Track MIRV launches
        if matches!(kind, NukeKind::MIRV) {
            *self.mirv_launches.entry(player_id).or_insert(0) += 1;
        }

        // Spawn projectile
        let id = self.state.next_projectile_id;
        self.state.next_projectile_id = self.state.next_projectile_id.wrapping_add(1).max(1);

        let sx = (silo_tile % w) as f32;
        let sy = (silo_tile / w) as f32;
        let dx = (target_tile % w) as f32;
        let dy = (target_tile / w) as f32;

        self.projectiles.push(crate::game::Projectile {
            id,
            kind: ProjectileKind::Nuke(kind),
            owner_id: player_id,
            src_x: sx,
            src_y: sy,
            dst_x: dx,
            dst_y: dy,
            progress: 0.0,
            speed: kind.speed(),
            active: true,
        });
    }
}
