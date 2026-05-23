use crate::engine::SowEngine;
use crate::game::{BuildingKind, GamePhase, NukeKind, ProjectileKind};

/// SAM steps per tick (fast interceptor).
const SAM_STEPS_PER_TICK: u8 = 4;

/// SAM cooldown in ticks.
const SAM_COOLDOWN_TICKS: u32 = 90;

/// Base SAM range constant.
const SAM_MAX_RANGE: f32 = 150.0;

/// SAM effective range: 150 - 480/(level+5)
pub fn sam_range(level: u8) -> f32 {
    SAM_MAX_RANGE - 480.0 / (level as f32 + 5.0)
}

/// Hex manhattan distance between two tile indices on an offset hex grid.
fn hex_distance(a: u32, b: u32, width: u32) -> u32 {
    let ax = (a % width) as i32;
    let ay = (a / width) as i32;
    let bx = (b % width) as i32;
    let by = (b / width) as i32;

    // Convert offset to axial (cube) coordinates
    let q1 = ax - (ay - (ay & 1)) / 2;
    let r1 = ay;
    let q2 = bx - (by - (by & 1)) / 2;
    let r2 = by;

    let dq = q1 - q2;
    let dr = r1 - r2;
    ((dq.abs() + (dq + dr).abs() + dr.abs()) / 2) as u32
}

impl SowEngine {
    pub fn execute_sam(&mut self) {
        if self.state.phase != GamePhase::Playing {
            return;
        }

        let width = self.state.map.width;

        // Collect SAM launchers that are ready (not under construction)
        let sams: Vec<(u64, u16, u32, f32)> = self.buildings.iter()
            .filter(|b| b.kind == BuildingKind::SamLauncher && !b.under_construction)
            .map(|b| (b.id, b.owner_id, b.tile_idx, sam_range(b.level)))
            .collect();

        let mut nukes_to_delete = Vec::new();
        let mut sam_missiles = Vec::new();

        for (sam_id, sam_owner, sam_tile, range) in &sams {
            // Check cooldown
            if self.silo_cooldowns.get(sam_id).copied().unwrap_or(0) > 0 {
                continue;
            }

            let range_u32 = *range as u32;

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
                let dist = hex_distance(*sam_tile, proj.dst_tile, width);
                if dist > range_u32 {
                    continue;
                }

                // MIRV warheads: instant delete within 50 hex tiles
                if matches!(proj.kind, ProjectileKind::MIRVWarhead) {
                    if dist < 50 {
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

                let score = priority * 10000.0 + dist as f32;
                if best_target.map_or(true, |(_, s)| score < s) {
                    best_target = Some((proj.id, score));
                }
            }

            if let Some((target_id, _)) = best_target {
                // Find the target projectile's destination
                if let Some(target) = self.projectiles.iter().find(|p| p.id == target_id) {
                    let id = self.state.next_projectile_id;
                    self.state.next_projectile_id = self.state.next_projectile_id.wrapping_add(1).max(1);

                    sam_missiles.push((id, *sam_owner, *sam_tile, target.dst_tile));
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
        for (id, owner, src_tile, dst_tile) in sam_missiles {
            let path = crate::pathfinding::bresenham_line(src_tile, dst_tile, width);
            self.projectiles.push(crate::game::Projectile {
                id,
                kind: ProjectileKind::SAMMissile,
                owner_id: owner,
                src_tile,
                dst_tile,
                path,
                path_cursor: 0,
                steps_per_tick: SAM_STEPS_PER_TICK,
                active: true,
            });
        }

        // Check SAM missile proximity to their targets → intercept (same tile = hit)
        let mut intercepted_nukes = Vec::new();
        let mut intercepted_sams = Vec::new();
        for sam in &self.projectiles {
            if !sam.active || !matches!(sam.kind, ProjectileKind::SAMMissile) {
                continue;
            }
            let sam_tile = sam.path.get(sam.path_cursor).copied()
                .unwrap_or(sam.dst_tile);

            for nuke in &self.projectiles {
                if !nuke.active || nuke.id == sam.id {
                    continue;
                }
                let is_nuke = matches!(nuke.kind, ProjectileKind::Nuke(_) | ProjectileKind::MIRVWarhead);
                if !is_nuke {
                    continue;
                }
                let nuke_tile = nuke.path.get(nuke.path_cursor).copied()
                    .unwrap_or(nuke.dst_tile);

                // Intercept if on the same tile or within 1 hex neighbor
                if hex_distance(sam_tile, nuke_tile, width) <= 1 {
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
