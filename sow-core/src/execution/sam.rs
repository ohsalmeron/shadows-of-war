use crate::engine::SowEngine;
use crate::game::{BuildingKind, GamePhase, ProjectileKind, NukeKind};

/// SAM missile speed (tiles per tick).
const SAM_MISSILE_SPEED: f32 = 12.0;

/// SAM cooldown in ticks.
const SAM_COOLDOWN_TICKS: u32 = 90;

/// Base SAM range constant.
const SAM_MAX_RANGE: f32 = 150.0;

/// SAM effective range: 150 - 480/(level+5)
fn sam_range(level: u8) -> f32 {
    SAM_MAX_RANGE - 480.0 / (level as f32 + 5.0)
}

impl SowEngine {
    pub fn execute_sam(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }

        // Collect SAM launchers that are ready (not under construction)
        let sams: Vec<(u64, u16, u32, u32, f32)> = self.buildings.iter()
            .filter(|b| b.kind == BuildingKind::SamLauncher && !b.under_construction)
            .map(|b| {
                let x = b.tile_idx % self.state.map.width;
                let y = b.tile_idx / self.state.map.width;
                (b.id, b.owner_id, x, y, sam_range(b.level))
            })
            .collect();

        let mut nukes_to_delete = Vec::new();
        let mut sam_missiles = Vec::new();

        for (sam_id, sam_owner, sx, sy, range) in &sams {
            // Check cooldown
            if self.silo_cooldowns.get(sam_id).copied().unwrap_or(0) > 0 {
                continue;
            }

            let range_sq = range * range;

            // Find closest incoming nuke (not owned by us) within range
            let mut best_target: Option<(u64, f32)> = None;
            for proj in &self.projectiles {
                if !proj.active {
                    continue;
                }
                // Only intercept nukes and MIRV warheads, not SAM missiles or shells
                let is_nuke = matches!(proj.kind, ProjectileKind::Nuke(_) | ProjectileKind::MIRVWarhead);
                if !is_nuke || proj.owner_id == *sam_owner {
                    continue;
                }

                // Check if destination is within range of SAM
                let dx = proj.dst_x - *sx as f32;
                let dy = proj.dst_y - *sy as f32;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq > range_sq {
                    continue;
                }

                // MIRV warheads: instant delete within 50 radius (matching OpenFront)
                if matches!(proj.kind, ProjectileKind::MIRVWarhead) {
                    if dist_sq < 50.0 * 50.0 {
                        nukes_to_delete.push(proj.id);
                        continue;
                    }
                }

                // Prioritize H-bombs over atom bombs
                let priority = match proj.kind {
                    ProjectileKind::Nuke(NukeKind::HydrogenBomb) => 0.0,
                    ProjectileKind::Nuke(NukeKind::MIRV) => 1.0,
                    ProjectileKind::Nuke(NukeKind::AtomBomb) => 2.0,
                    _ => 3.0,
                };

                let score = priority * 10000.0 + dist_sq;
                if best_target.map_or(true, |(_, s)| score < s) {
                    best_target = Some((proj.id, score));
                }
            }

            if let Some((target_id, _)) = best_target {
                // Find the target projectile
                if let Some(target) = self.projectiles.iter().find(|p| p.id == target_id) {
                    let id = self.state.next_projectile_id;
                    self.state.next_projectile_id = self.state.next_projectile_id.wrapping_add(1).max(1);

                    sam_missiles.push((id, *sam_owner, *sx as f32, *sy as f32, target.dst_x, target.dst_y));
                    self.silo_cooldowns.insert(*sam_id, SAM_COOLDOWN_TICKS);
                }
            }
        }

        // Delete MIRV warheads that were instantly intercepted
        for nuke_id in &nukes_to_delete {
            if let Some(p) = self.projectiles.iter_mut().find(|p| p.id == *nuke_id) {
                p.active = false;
            }
        }

        // Spawn SAM missiles
        for (id, owner, sx, sy, dst_x, dst_y) in sam_missiles {
            self.projectiles.push(crate::game::Projectile {
                id,
                kind: ProjectileKind::SAMMissile,
                owner_id: owner,
                src_x: sx,
                src_y: sy,
                dst_x,
                dst_y,
                progress: 0.0,
                speed: SAM_MISSILE_SPEED,
                active: true,
            });
        }

        // Check SAM missile proximity to their targets → intercept
        let mut intercepted_nukes = Vec::new();
        let mut intercepted_sams = Vec::new();
        for sam in &self.projectiles {
            if !sam.active || !matches!(sam.kind, ProjectileKind::SAMMissile) {
                continue;
            }
            let sam_x = sam.src_x + (sam.dst_x - sam.src_x) * sam.progress;
            let sam_y = sam.src_y + (sam.dst_y - sam.src_y) * sam.progress;

            for nuke in &self.projectiles {
                if !nuke.active || nuke.id == sam.id {
                    continue;
                }
                let is_nuke = matches!(nuke.kind, ProjectileKind::Nuke(_) | ProjectileKind::MIRVWarhead);
                if !is_nuke {
                    continue;
                }
                let nuke_x = nuke.src_x + (nuke.dst_x - nuke.src_x) * nuke.progress;
                let nuke_y = nuke.src_y + (nuke.dst_y - nuke.src_y) * nuke.progress;

                let dx = sam_x - nuke_x;
                let dy = sam_y - nuke_y;
                if dx * dx + dy * dy < 4.0 {
                    intercepted_nukes.push(nuke.id);
                    intercepted_sams.push(sam.id);
                    break;
                }
            }
        }

        for id in intercepted_nukes.iter().chain(intercepted_sams.iter()) {
            if let Some(p) = self.projectiles.iter_mut().find(|p| p.id == *id) {
                p.active = false;
            }
        }

        // Cleanup
        self.projectiles.retain(|p| p.active);
    }
}
