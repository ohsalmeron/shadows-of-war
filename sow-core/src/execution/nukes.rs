use crate::engine::SowEngine;
use crate::game::{GamePhase, NukeKind, ProjectileKind};

/// Silo cooldown in ticks (90 ticks ≈ 9 seconds at 100ms/tick).
pub const SILO_COOLDOWN_TICKS: u32 = 90;

/// MIRV warhead count (matching OpenFront).
const MIRV_WARHEAD_COUNT: u32 = 350;

/// Minimum manhattan spread between MIRV warheads.
const MIRV_MIN_SPREAD: i32 = 55;

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
        self.silo_cooldowns.retain(|_, cd| *cd > 0);

        // Advance all active projectiles
        let mut detonations = Vec::new();
        let mut mirv_separations = Vec::new();

        for proj in &mut self.projectiles {
            if !proj.active {
                continue;
            }

            let dx = proj.dst_x - proj.src_x;
            let dy = proj.dst_y - proj.src_y;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let step = proj.speed / dist;
            proj.progress += step;

            if proj.progress >= 1.0 {
                proj.active = false;
                match proj.kind {
                    ProjectileKind::Nuke(NukeKind::MIRV) => {
                        mirv_separations.push((
                            proj.owner_id,
                            proj.dst_x,
                            proj.dst_y,
                        ));
                    }
                    ProjectileKind::Nuke(nk) => {
                        detonations.push((
                            proj.dst_x as u32,
                            proj.dst_y as u32,
                            nk.inner_radius(),
                            nk.outer_radius(),
                            proj.owner_id,
                        ));
                    }
                    ProjectileKind::MIRVWarhead => {
                        detonations.push((
                            proj.dst_x as u32,
                            proj.dst_y as u32,
                            NukeKind::MIRV.inner_radius(),
                            NukeKind::MIRV.outer_radius(),
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

        // Process MIRV separations → spawn warheads
        for (owner_id, cx, cy) in mirv_separations {
            self.spawn_mirv_warheads(owner_id, cx, cy);
        }

        // Process detonations
        for (dx, dy, inner, outer, owner_id) in detonations {
            self.detonate_nuke(dx, dy, inner, outer, owner_id);
        }
    }

    fn spawn_mirv_warheads(&mut self, owner_id: u16, cx: f32, cy: f32) {
        let w = self.state.map.width as i32;
        let h = self.state.map.height as i32;

        // Simple spread pattern: concentric rings around the target
        let mut placed = Vec::with_capacity(MIRV_WARHEAD_COUNT as usize);
        let mut rng_seed = self.state.tick.wrapping_mul(owner_id as u64);

        for _ in 0..MIRV_WARHEAD_COUNT {
            // Pseudo-random offset
            rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let angle = (rng_seed & 0xFFFF) as f32 / 65536.0 * std::f32::consts::TAU;
            rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let radius = ((rng_seed & 0xFFFF) as f32 / 65536.0) * 80.0;

            let tx = (cx + angle.cos() * radius).clamp(0.0, (w - 1) as f32);
            let ty = (cy + angle.sin() * radius).clamp(0.0, (h - 1) as f32);

            // Check minimum spread
            let too_close = placed.iter().any(|&(px, py): &(f32, f32)| {
                let md = (tx - px).abs() + (ty - py).abs();
                md < MIRV_MIN_SPREAD as f32
            });
            if too_close {
                continue;
            }
            placed.push((tx, ty));

            let id = self.state.next_projectile_id;
            self.state.next_projectile_id = self.state.next_projectile_id.wrapping_add(1).max(1);

            // Random delay via slower speed
            rng_seed = rng_seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let speed = 15.0 + (rng_seed & 0xF) as f32 * 0.33;

            self.projectiles.push(crate::game::Projectile {
                id,
                kind: ProjectileKind::MIRVWarhead,
                owner_id,
                src_x: cx,
                src_y: cy,
                dst_x: tx,
                dst_y: ty,
                progress: 0.0,
                speed,
                active: true,
            });
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
