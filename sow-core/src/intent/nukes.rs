use crate::engine::SowEngine;
use crate::game::{BuildingKind, GamePhase, NukeKind, ProjectileKind};

/// Silo cooldown after launching (must match execution/nukes.rs).
const SILO_COOLDOWN_TICKS: u32 = 90;

impl SowEngine {
    pub fn apply_launch_nuke_intent(&mut self, player_id: u16, _kind: NukeKind, target_tile: u32) {
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
                b.kind == BuildingKind::City
                    && b.owner_id == player_id
                    && !b.under_construction
                    && b.modules.arsenal >= 1
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
        let level = silo.modules.arsenal;

        // Check gold based on the level of this arsenal
        // Level 1 = 750k, Level 2 = 1.5M, Level 3 = 3.0M
        let cost = 750_000.0 * 2.0f64.powi(level as i32 - 1);

        let Some(player_mut) = self.state.player_mut(player_id) else {
            return;
        };
        if player_mut.gold < cost {
            return;
        }
        player_mut.gold -= cost;

        // Set silo cooldown
        self.silo_cooldowns.insert(silo_id, SILO_COOLDOWN_TICKS);

        // Spawn projectile
        let id = self.state.next_projectile_id;
        self.state.next_projectile_id = self.state.next_projectile_id.wrapping_add(1).max(1);

        let path = crate::pathfinding::bresenham_line(silo_tile, target_tile, w);

        self.projectiles.push(crate::game::Projectile {
            id,
            kind: ProjectileKind::Nuke { level },
            owner_id: player_id,
            src_tile: silo_tile,
            dst_tile: target_tile,
            path,
            path_cursor: 0,
            steps_per_tick: 1 + level,
            active: true,
        });
    }
}
