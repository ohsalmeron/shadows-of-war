use sow_core::game::{ProjectileKind, UnitType};
use sow_core::protocol::{FleetSnapshot, PlayerSnapshot, ProjectileSnapshot, SimSnapshot};
use sow_render::{MoverInstanceGpu, MoverSpriteId, TrailSegmentGpu};
use std::collections::{HashMap, HashSet};
use web_time::Instant;

const TRAIL_CAP: usize = 32;
const HEX_Y_SCALE: f32 = 0.8660254;
const NUKE_ARC_PEAK: f32 = 4.0;
const NUKE_ARC_LIFT: f32 = 20.0;

#[inline]
fn flight_progress(path_len: usize, path_index: f32) -> f32 {
    if path_len <= 1 {
        1.0
    } else {
        (path_index / (path_len - 1) as f32).clamp(0.0, 1.0)
    }
}

#[inline]
fn nuke_arc_height(progress: f32) -> f32 {
    NUKE_ARC_PEAK * progress * (1.0 - progress)
}

#[inline]
fn lift_world_for_arc(wx: f32, wy: f32, progress: f32) -> [f32; 2] {
    [wx, wy - nuke_arc_height(progress) * NUKE_ARC_LIFT]
}

#[inline]
pub fn tile_to_world(tile: u32, map_w: u32) -> (f32, f32) {
    let tx = (tile % map_w) as f32;
    let ty = (tile / map_w) as f32;
    let wx = tx + 0.5 + (ty as i32 % 2) as f32 * 0.5;
    let wy = (ty + 0.5) * HEX_Y_SCALE;
    (wx, wy)
}

#[derive(Clone, Copy)]
struct MoverSlot {
    prev_x: f32,
    prev_y: f32,
    curr_x: f32,
    curr_y: f32,
    path_progress_prev: f32,
    path_progress_curr: f32,
    size: f32,
    color: [f32; 4],
    trail_color: [f32; 4],
    sprite: MoverSpriteId,
    trail_start: u32,
    trail_len: u32,
    is_fleet: bool,
    arc_trail: bool,
}

pub struct MoverScene {
    id_to_idx: HashMap<u64, u32>,
    slots: Vec<MoverSlot>,
    trail_points: Vec<[f32; 2]>,
    last_snap_tick: u64,
}

pub struct MoverPackParams<'a> {
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    pub screen_w: f32,
    pub screen_h: f32,
    pub alpha: f32,
    pub selected_warships: &'a [u64],
}

impl MoverScene {
    pub fn new() -> Self {
        Self {
            id_to_idx: HashMap::new(),
            slots: Vec::new(),
            trail_points: Vec::new(),
            last_snap_tick: u64::MAX,
        }
    }

    pub fn on_snapshot(&mut self, snap: &SimSnapshot, map_w: u32) {
        if snap.tick == self.last_snap_tick {
            return;
        }
        self.last_snap_tick = snap.tick;
        self.trail_points.clear();

        let mut alive: HashSet<u64> = HashSet::new();

        for fleet in &snap.fleets {
            alive.insert(fleet.id);
            self.ingest_fleet(fleet, map_w, &snap.players);
        }
        for proj in &snap.projectiles {
            if proj.path.is_empty() || matches!(proj.kind, ProjectileKind::Shell) {
                continue;
            }
            let key = proj.id | (1u64 << 63);
            alive.insert(key);
            self.ingest_projectile(proj, map_w);
        }

        let dead: Vec<u64> = self
            .id_to_idx
            .keys()
            .copied()
            .filter(|id| !alive.contains(id))
            .collect();
        for id in dead {
            if let Some(idx) = self.id_to_idx.remove(&id) {
                let rem = idx as usize;
                if rem < self.slots.len() {
                    let last = self.slots.len() - 1;
                    if rem != last {
                        self.slots.swap(rem, last);
                        if let Some(moved_id) = self
                            .id_to_idx
                            .iter()
                            .find(|(_, i)| **i as usize == last)
                            .map(|(k, _)| *k)
                        {
                            self.id_to_idx.insert(moved_id, rem as u32);
                        }
                    }
                    self.slots.pop();
                }
            }
        }
    }

    fn ingest_fleet(&mut self, fleet: &FleetSnapshot, map_w: u32, players: &[PlayerSnapshot]) {
        let (curr_x, curr_y) = tile_to_world(fleet.current_tile, map_w);
        let (prev_x, prev_y) = if fleet.path_cursor > 1 && !fleet.path.is_empty() {
            let prev_idx = fleet
                .path_cursor
                .saturating_sub(2)
                .min(fleet.path.len().saturating_sub(1));
            tile_to_world(fleet.path[prev_idx], map_w)
        } else {
            (curr_x, curr_y)
        };

        let sprite = match fleet.unit_type {
            UnitType::TransportShip => MoverSpriteId::TransportShip,
            UnitType::TradeShip => MoverSpriteId::TradeShip,
            UnitType::Warship => MoverSpriteId::Warship,
        };

        let rgb = players
            .iter()
            .find(|p| p.id == fleet.owner_id)
            .map(|p| p.color)
            .unwrap_or([0.5, 0.5, 0.5]);
        let color = [rgb[0], rgb[1], rgb[2], 1.0];
        let trail_color = [rgb[0], rgb[1], rgb[2], 0.59];

        let trail_start = self.trail_points.len() as u32;
        let traveled = fleet.path_cursor.saturating_sub(1);
        if traveled > 0 {
            let start = traveled.saturating_sub(TRAIL_CAP);
            for &tile in &fleet.path[start..traveled] {
                let (wx, wy) = tile_to_world(tile, map_w);
                self.trail_points.push([wx, wy]);
            }
        }
        let trail_len = self.trail_points.len() as u32 - trail_start;

        let entry = MoverSlot {
            prev_x,
            prev_y,
            curr_x,
            curr_y,
            path_progress_prev: 0.0,
            path_progress_curr: 0.0,
            size: 0.7,
            color,
            trail_color,
            sprite,
            trail_start,
            trail_len,
            is_fleet: true,
            arc_trail: false,
        };
        self.upsert_slot(fleet.id, entry);
    }

    fn ingest_projectile(&mut self, proj: &ProjectileSnapshot, map_w: u32) {
        let cursor = proj.path_cursor.min(proj.path.len().saturating_sub(1));
        let prev_idx = cursor.saturating_sub(1);
        let (curr_x, curr_y) = tile_to_world(proj.path[cursor], map_w);
        let (prev_x, prev_y) = tile_to_world(proj.path[prev_idx], map_w);

        let path_len = proj.path.len();
        let progress_curr = flight_progress(path_len, cursor as f32);
        let progress_prev = flight_progress(path_len, prev_idx as f32);

        let (sprite, size, trail_color) = match proj.kind {
            ProjectileKind::Nuke { level } => {
                let sprite = MoverSpriteId::AtomBomb;
                let tc = if level >= 3 {
                    [1.0, 0.667, 0.0, 0.63]
                } else if level == 2 {
                    [1.0, 0.196, 0.0, 0.59]
                } else {
                    [1.0, 0.353, 0.0, 0.55]
                };
                (sprite, 0.55 + level as f32 * 0.15, tc)
            }
            ProjectileKind::SAMMissile => (
                MoverSpriteId::SamMissile,
                0.55,
                [0.39, 0.78, 1.0, 0.39],
            ),
            ProjectileKind::Shell => return,
        };

        let is_nuke = matches!(proj.kind, ProjectileKind::Nuke { .. });

        let trail_start = self.trail_points.len() as u32;
        let traveled = cursor;
        if is_nuke && traveled > 0 {
            let trail_step = (traveled / 15).max(1);
            for i in (0..=traveled).step_by(trail_step) {
                let p = flight_progress(path_len, i as f32);
                let (wx, wy) = tile_to_world(proj.path[i], map_w);
                self.trail_points
                    .push(lift_world_for_arc(wx, wy, p));
            }
        } else if traveled > 0 {
            let start = traveled.saturating_sub(TRAIL_CAP);
            let stride = ((traveled - start) / 12).max(1);
            for i in (start..=traveled).step_by(stride) {
                let (wx, wy) = tile_to_world(proj.path[i], map_w);
                self.trail_points.push([wx, wy]);
            }
        }
        let trail_len = self.trail_points.len() as u32 - trail_start;

        let entry = MoverSlot {
            prev_x,
            prev_y,
            curr_x,
            curr_y,
            path_progress_prev: progress_prev,
            path_progress_curr: progress_curr,
            size,
            color: [1.0, 1.0, 1.0, 1.0],
            trail_color,
            sprite,
            trail_start,
            trail_len,
            is_fleet: false,
            arc_trail: is_nuke,
        };
        self.upsert_slot(proj.id | (1u64 << 63), entry);
    }

    fn upsert_slot(&mut self, id: u64, mut entry: MoverSlot) {
        if let Some(&idx) = self.id_to_idx.get(&id) {
            let old = &self.slots[idx as usize];
            entry.prev_x = old.curr_x;
            entry.prev_y = old.curr_y;
            entry.path_progress_prev = old.path_progress_curr;
            self.slots[idx as usize] = entry;
        } else {
            let idx = self.slots.len() as u32;
            self.id_to_idx.insert(id, idx);
            self.slots.push(entry);
        }
    }

    pub fn pack_gpu(&self, params: &MoverPackParams<'_>, renderer: &mut sow_render::MoverRenderer) {
        renderer.begin_frame();
        let alpha = params.alpha;
        let margin = 64.0;
        let min_sx = -margin;
        let min_sy = -margin;
        let max_sx = params.screen_w + margin;
        let max_sy = params.screen_h + margin;

        for (id, &idx) in &self.id_to_idx {
            let slot = &self.slots[idx as usize];
            let wx = slot.prev_x + (slot.curr_x - slot.prev_x) * alpha;
            let wy = slot.prev_y + (slot.curr_y - slot.prev_y) * alpha;
            let progress = slot.path_progress_prev
                + (slot.path_progress_curr - slot.path_progress_prev) * alpha;
            let height = if slot.is_fleet {
                0.0
            } else {
                nuke_arc_height(progress)
            };
            let world_pos = if slot.arc_trail {
                lift_world_for_arc(wx, wy, progress)
            } else if slot.is_fleet {
                [wx, wy]
            } else {
                [wx, wy - height * NUKE_ARC_LIFT]
            };
            let world_y_lifted = world_pos[1];

            let sx = params.camera_x + wx * params.camera_zoom;
            let sy = params.camera_y + world_y_lifted * params.camera_zoom;
            if sx < min_sx || sx > max_sx || sy < min_sy || sy > max_sy {
                continue;
            }

            let dx = slot.curr_x - slot.prev_x;
            let dy = slot.curr_y - slot.prev_y;
            let rotation = if slot.arc_trail && slot.trail_len > 0 {
                let last = self.trail_points[(slot.trail_start + slot.trail_len - 1) as usize];
                let dir_x = world_pos[0] - last[0];
                let dir_y = world_pos[1] - last[1];
                if dir_x * dir_x + dir_y * dir_y > 1e-8 {
                    dir_y.atan2(dir_x) + std::f32::consts::FRAC_PI_2
                } else {
                    0.0
                }
            } else if dx * dx + dy * dy > 1e-8 {
                dy.atan2(dx) + std::f32::consts::FRAC_PI_2
            } else {
                0.0
            };

            let scale = if slot.is_fleet {
                1.0
            } else {
                (1.0 + height * 0.5).min(2.0)
            };

            renderer.push_sprite(MoverInstanceGpu {
                world_pos: [wx, wy],
                size: slot.size * scale,
                rotation,
                color: slot.color,
                uv_rect: slot.sprite.uv_rect(),
                height: height * NUKE_ARC_LIFT,
            });

            let _ = params.selected_warships.contains(id);

            if slot.trail_len > 0 {
                let start = slot.trail_start as usize;
                let end = start + slot.trail_len as usize;
                let width = (params.camera_zoom * 0.4).clamp(1.0, 6.0);
                let color = slot.trail_color;
                let mut prev = self.trail_points[start];
                for pt in &self.trail_points[start + 1..end] {
                    renderer.push_trail_segment(TrailSegmentGpu {
                        p0: prev,
                        p1: *pt,
                        width,
                        color,
                    });
                    prev = *pt;
                }
                renderer.push_trail_segment(TrailSegmentGpu {
                    p0: prev,
                    p1: world_pos,
                    width,
                    color,
                });
            }
        }
    }
}

impl Default for MoverScene {
    fn default() -> Self {
        Self::new()
    }
}

pub fn update_and_pack(
    scene: &mut MoverScene,
    snap: &SimSnapshot,
    map_w: u32,
    renderer: &mut sow_render::MoverRenderer,
    params: MoverPackParams<'_>,
) {
    scene.on_snapshot(snap, map_w);
    scene.pack_gpu(&params, renderer);
}

pub fn interp_alpha(time: &crate::app::TimeState, now: Instant) -> f32 {
    time.interp.alpha(now)
}
