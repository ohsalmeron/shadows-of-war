use crate::engine::SowEngine;
use crate::game::{GamePhase, ProjectileKind};

/// Silo cooldown in ticks (90 ticks ≈ 9 seconds at 100ms/tick).
pub const SILO_COOLDOWN_TICKS: u32 = 90;

impl SowEngine {
    pub fn execute_projectiles(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }

        // Tick down silo cooldowns
        self.silo_cooldowns.values_mut().for_each(|cd| {
            if *cd > 0 {
                *cd -= 1;
            }
        });
        self.silo_cooldowns.retain(|_, cd| *cd > 0);        // Advance all active projectiles
        let mut detonations = Vec::new();

        for proj in &mut self.projectiles {
            if !proj.active {
                continue;
            }

            proj.path_cursor += proj.steps_per_tick as usize;

            if proj.path_cursor >= proj.path.len() {
                proj.path_cursor = proj.path.len().saturating_sub(1);
                proj.active = false;
                let dst_x = proj.dst_tile % self.state.map.width;
                let dst_y = proj.dst_tile / self.state.map.width;
                match proj.kind {
                    ProjectileKind::Nuke { level } => {
                        let inner = 12 + (level.saturating_sub(1) as u32) * 10;
                        let outer = 30 + (level.saturating_sub(1) as u32) * 25;
                        detonations.push((
                            dst_x,
                            dst_y,
                            inner,
                            outer,
                            proj.owner_id,
                        ));
                    }
                    ProjectileKind::SAMMissile => {
                        // SAM missiles delete their target on contact — handled in execute_sam
                    }
                    ProjectileKind::Shell => {
                        // Shell damage handled separately
                    }
                }
            }
        }

        // Cleanup inactive projectiles
        self.projectiles.retain(|p| p.active);

        // Process detonations
        for (dx, dy, inner, outer, owner_id) in detonations {
            self.detonate_nuke(dx, dy, inner, outer, owner_id);
        }
    }

    fn detonate_nuke(&mut self, cx: u32, cy: u32, inner: u32, outer: u32, owner_id: u16) {
        let w = self.state.map.width;
        let h = self.state.map.height;
        let outer_sq = (outer * outer) as i64;
        let inner_sq = (inner * inner) as i64;
        let cx_i = cx as i32;
        let cy_i = cy as i32;

        let x_min = (cx_i - outer as i32).max(0) as u32;
        let x_max = (cx_i + outer as i32).min(w as i32 - 1) as u32;
        let y_min = (cy_i - outer as i32).max(0) as u32;
        let y_max = (cy_i + outer as i32).min(h as i32 - 1) as u32;

        let mut troops_damage: Vec<(u16, f64)> = Vec::new();

        for y in y_min..=y_max {
            for x in x_min..=x_max {
                let dx = x as i64 - cx as i64;
                let dy = y as i64 - cy as i64;
                let dist_sq = dx * dx + dy * dy;

                if dist_sq > outer_sq {
                    continue;
                }

                let tile_owner = self.state.map.owner_id(x, y);

                if dist_sq <= inner_sq {
                    // Inner radius: wipe ownership
                    if tile_owner != 0 {
                        self.state.set_tile_owner(x, y, 0);
                    }
                } else {
                    // Outer radius: troop damage proportional to proximity
                    if tile_owner != 0 && tile_owner != owner_id {
                        let ratio = 1.0 - (dist_sq as f64 / outer_sq as f64);
                        let damage = ratio * 5.0;
                        let entry = troops_damage.iter_mut().find(|(pid, _)| *pid == tile_owner);
                        if let Some(e) = entry {
                            e.1 += damage;
                        } else {
                            troops_damage.push((tile_owner, damage));
                        }
                    }
                }
            }
        }

        // Apply troop damage
        for (pid, dmg) in &troops_damage {
            if let Some(p) = self.state.player_mut(*pid) {
                p.troops = (p.troops - dmg).max(0.0);
            }
        }

        // Destroy buildings in inner radius
        self.buildings.retain(|b| {
            let bx = b.tile_idx % w;
            let by = b.tile_idx / w;
            let dx = bx as i64 - cx as i64;
            let dy = by as i64 - cy as i64;
            dx * dx + dy * dy > inner_sq
        });
        self.building_grid.dirty = true;
        self.building_aggregates_dirty = true;
        self.defense_grid_dirty = true;
        self.railroads_dirty = true;
        self.sea_lanes_dirty = true;

        self.state.events.push(crate::game::GameEvent::NukeDetonated {
            tile_x: cx,
            tile_y: cy,
            inner_radius: inner,
            outer_radius: outer,
            owner_id,
        });
    }
}
