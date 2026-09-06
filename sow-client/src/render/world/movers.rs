use crate::render::gpu::{MoverInstanceGpu, MoverSpriteId, TrailSegmentGpu};
use sow_core::game::{ProjectileKind, UnitType};
use sow_core::protocol::{FleetSnapshot, PlayerSnapshot, ProjectileSnapshot, SimSnapshot};
use std::collections::{HashMap, HashSet};
use web_time::Instant;

const TRAIL_CAP: usize = 32;
const NUKE_ARC_PEAK: f32 = 4.0;
const NUKE_ARC_LIFT: f32 = 20.0;
const NUKE_ARC_SAMPLES: usize = 40;
const MIN_PROJECTILE_SCREEN_PX: f32 = 11.0;
const PROJECTILE_TRAIL_WIDTH_MIN: f32 = 3.0;
const PROJECTILE_TRAIL_WIDTH_MAX: f32 = 10.0;

#[inline]
pub fn world_to_tile(wx: f32, wy: f32) -> (i32, i32) {
    (wx.floor() as i32, wy.floor() as i32)
}

#[inline]
pub fn tile_to_world(tile: u32, map_w: u32) -> (f32, f32) {
    let tx = (tile % map_w) as f32;
    let ty = (tile / map_w) as f32;
    let wx = tx + 0.5;
    let wy = ty + 0.5;
    (wx, wy)
}

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
fn path_world_at(path: &[u32], map_w: u32, index_f: f32) -> (f32, f32) {
    if path.is_empty() {
        return (0.0, 0.0);
    }
    let max_idx = path.len() - 1;
    let idx = (index_f.floor() as usize).min(max_idx);
    let next = (idx + 1).min(max_idx);
    let frac = (index_f - idx as f32).clamp(0.0, 1.0);
    let (x0, y0) = tile_to_world(path[idx], map_w);
    if idx == next || frac <= 0.0 {
        (x0, y0)
    } else {
        let (x1, y1) = tile_to_world(path[next], map_w);
        (x0 + (x1 - x0) * frac, y0 + (y1 - y0) * frac)
    }
}

fn sample_nuke_arc(path: &[u32], map_w: u32, progress: f32, out: &mut Vec<[f32; 2]>) {
    out.clear();
    let path_len = path.len();
    if path_len <= 1 || progress <= 0.0 {
        return;
    }
    for s in 0..=NUKE_ARC_SAMPLES {
        let p = progress * (s as f32 / NUKE_ARC_SAMPLES as f32);
        let idx_f = p * (path_len - 1) as f32;
        let (wx, wy) = path_world_at(path, map_w, idx_f);
        out.push(lift_world_for_arc(wx, wy, p));
    }
}

#[inline]
fn screen_margin(zoom: f32) -> f32 {
    64.0 + NUKE_ARC_PEAK * NUKE_ARC_LIFT * zoom
}

#[inline]
fn in_viewport(sx: f32, sy: f32, min_sx: f32, min_sy: f32, max_sx: f32, max_sy: f32) -> bool {
    sx >= min_sx && sx <= max_sx && sy >= min_sy && sy <= max_sy
}

#[inline]
fn projectile_trail_width(zoom: f32) -> f32 {
    (zoom * 0.5 + 2.0).clamp(PROJECTILE_TRAIL_WIDTH_MIN, PROJECTILE_TRAIL_WIDTH_MAX)
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
    arc_paths: HashMap<u64, Vec<u32>>,
    last_snap_tick: u64,
    map_w: u32,
}

pub struct MoverPackParams {
    pub camera_x: f32,
    pub camera_y: f32,
    pub camera_zoom: f32,
    pub screen_w: f32,
    pub screen_h: f32,
    pub alpha: f32,
    pub linear_alpha: f32,
}

impl MoverScene {
    pub fn new() -> Self {
        Self {
            id_to_idx: HashMap::new(),
            slots: Vec::new(),
            trail_points: Vec::new(),
            arc_paths: HashMap::new(),
            last_snap_tick: u64::MAX,
            map_w: 1,
        }
    }

    pub fn on_snapshot(
        &mut self,
        snap: &SimSnapshot,
        map_w: u32,
        fog_of_war_enabled: bool,
        my_id: u16,
        fog_visible: &sow_core::bitset::DenseBitSet,
    ) {
        if snap.tick == self.last_snap_tick {
            return;
        }
        self.last_snap_tick = snap.tick;
        self.map_w = map_w.max(1);
        self.trail_points.clear();

        let mut alive: HashSet<u64> = HashSet::new();

        for fleet in &snap.fleets {
            let is_visible = !fog_of_war_enabled
                || fleet.owner_id == my_id
                || fog_visible.contains(fleet.current_tile);
            if is_visible {
                alive.insert(fleet.id);
                self.ingest_fleet(fleet, map_w, &snap.players);
            }
        }
        for proj in &snap.projectiles {
            if proj.path.is_empty() || matches!(proj.kind, ProjectileKind::Shell) {
                continue;
            }
            let is_visible = !fog_of_war_enabled
                || fog_visible.contains(proj.src_tile)
                || fog_visible.contains(proj.dst_tile);
            if is_visible {
                let key = proj.id | (1u64 << 63);
                alive.insert(key);
                self.ingest_projectile(proj, map_w);
            }
        }

        let dead: Vec<u64> = self
            .id_to_idx
            .keys()
            .copied()
            .filter(|id| !alive.contains(id))
            .collect();
        for id in dead {
            self.arc_paths.remove(&id);
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
            .map(|p| p.team.map_or(p.color, sow_core::player::team_territory_rgb))
            .unwrap_or([0.5, 0.5, 0.5]);
        let color = [rgb[0], rgb[1], rgb[2], 1.0];
        let trail_color = [
            rgb[0] * 0.7 + 0.3,
            rgb[1] * 0.7 + 0.3,
            rgb[2] * 0.7 + 0.3,
            0.75,
        ];

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
        let prev_idx = cursor.saturating_sub(proj.steps_per_tick as usize);
        let (curr_x, curr_y) = tile_to_world(proj.path[cursor], map_w);
        let (prev_x, prev_y) = tile_to_world(proj.path[prev_idx], map_w);

        let path_len = proj.path.len();
        let progress_curr = flight_progress(path_len, cursor as f32);
        let progress_prev = flight_progress(path_len, prev_idx as f32);

        let (sprite, size, trail_color) = match proj.kind {
            ProjectileKind::Nuke { level } => {
                let sprite = MoverSpriteId::AtomBomb;
                let tc = if level >= 3 {
                    [1.0, 0.667, 0.0, 0.95]
                } else if level == 2 {
                    [1.0, 0.196, 0.0, 0.92]
                } else {
                    [1.0, 0.353, 0.0, 0.88]
                };
                (sprite, 0.65 + level as f32 * 0.12, tc)
            }
            ProjectileKind::SAMMissile => (MoverSpriteId::SamMissile, 0.6, [0.39, 0.78, 1.0, 0.85]),
            ProjectileKind::Shell => return,
        };

        let is_nuke = matches!(proj.kind, ProjectileKind::Nuke { .. });
        let key = proj.id | (1u64 << 63);
        self.arc_paths.insert(key, proj.path.clone());

        let (trail_start, trail_len) = if is_nuke {
            (0, 0)
        } else {
            let trail_start = self.trail_points.len() as u32;
            let traveled = cursor;
            if traveled > 0 {
                let start = traveled.saturating_sub(TRAIL_CAP);
                let stride = ((traveled - start) / 12).max(1);
                for i in (start..=traveled).step_by(stride) {
                    let (wx, wy) = tile_to_world(proj.path[i], map_w);
                    self.trail_points.push([wx, wy]);
                }
            }
            let trail_len = self.trail_points.len() as u32 - trail_start;
            (trail_start, trail_len)
        };

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
        self.upsert_slot(key, entry);
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

    fn push_trail_segments(
        &self,
        renderer: &mut crate::render::gpu::MoverRenderer,
        points: &[[f32; 2]],
        head: [f32; 2],
        width: f32,
        color: [f32; 4],
    ) {
        if points.is_empty() {
            return;
        }
        let mut prev = points[0];
        for pt in &points[1..] {
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
            p1: head,
            width,
            color,
        });
    }

    fn arc_visible(
        &self,
        points: &[[f32; 2]],
        head: [f32; 2],
        params: &MoverPackParams,
        bounds: (f32, f32, f32, f32),
    ) -> bool {
        let (min_sx, min_sy, max_sx, max_sy) = bounds;
        for pt in points {
            let sx = params.camera_x + pt[0] * params.camera_zoom;
            let sy = params.camera_y + pt[1] * params.camera_zoom;
            if in_viewport(sx, sy, min_sx, min_sy, max_sx, max_sy) {
                return true;
            }
        }
        let sx = params.camera_x + head[0] * params.camera_zoom;
        let sy = params.camera_y + head[1] * params.camera_zoom;
        in_viewport(sx, sy, min_sx, min_sy, max_sx, max_sy)
    }

    pub fn pack_gpu(
        &self,
        params: &MoverPackParams,
        renderer: &mut crate::render::gpu::MoverRenderer,
    ) {
        renderer.begin_frame();
        let alpha = params.alpha;
        let margin = screen_margin(params.camera_zoom);
        let min_sx = -margin;
        let min_sy = -margin;
        let max_sx = params.screen_w + margin;
        let max_sy = params.screen_h + margin;
        let mut arc_scratch = Vec::with_capacity(NUKE_ARC_SAMPLES + 1);

        for (id, &idx) in &self.id_to_idx {
            let slot = &self.slots[idx as usize];

            let (wx, wy, progress) = if slot.is_fleet {
                let wx = slot.prev_x + (slot.curr_x - slot.prev_x) * alpha;
                let wy = slot.prev_y + (slot.curr_y - slot.prev_y) * alpha;
                let progress = slot.path_progress_prev
                    + (slot.path_progress_curr - slot.path_progress_prev) * alpha;
                (wx, wy, progress)
            } else {
                let progress = slot.path_progress_prev
                    + (slot.path_progress_curr - slot.path_progress_prev) * params.linear_alpha;
                if let Some(path) = self.arc_paths.get(id) {
                    let path_len = path.len();
                    if path_len > 0 {
                        let idx_f = progress * (path_len - 1) as f32;
                        let pos = path_world_at(path, self.map_w, idx_f);
                        (pos.0, pos.1, progress)
                    } else {
                        let wx = slot.prev_x + (slot.curr_x - slot.prev_x) * params.linear_alpha;
                        let wy = slot.prev_y + (slot.curr_y - slot.prev_y) * params.linear_alpha;
                        (wx, wy, progress)
                    }
                } else {
                    let wx = slot.prev_x + (slot.curr_x - slot.prev_x) * params.linear_alpha;
                    let wy = slot.prev_y + (slot.curr_y - slot.prev_y) * params.linear_alpha;
                    (wx, wy, progress)
                }
            };

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

            let sx = params.camera_x + wx * params.camera_zoom;
            let sy = params.camera_y + world_pos[1] * params.camera_zoom;
            let sprite_visible = in_viewport(sx, sy, min_sx, min_sy, max_sx, max_sy);

            let trail_width = if slot.is_fleet {
                (params.camera_zoom * 0.4).clamp(1.0, 6.0)
            } else {
                projectile_trail_width(params.camera_zoom)
            };

            if slot.arc_trail {
                // Fast O(1) nuke arc bounding box frustum culling
                let min_x = slot.prev_x.min(slot.curr_x);
                let max_x = slot.prev_x.max(slot.curr_x);
                let min_y = slot.prev_y.min(slot.curr_y) - NUKE_ARC_PEAK * NUKE_ARC_LIFT;
                let max_y = slot.prev_y.max(slot.curr_y);

                let min_sx = params.camera_x + min_x * params.camera_zoom;
                let max_sx = params.camera_x + max_x * params.camera_zoom;
                let min_sy = params.camera_y + min_y * params.camera_zoom;
                let max_sy = params.camera_y + max_y * params.camera_zoom;

                let arc_visible = max_sx >= -margin
                    && min_sx <= params.screen_w + margin
                    && max_sy >= -margin
                    && min_sy <= params.screen_h + margin;

                if arc_visible && let Some(path) = self.arc_paths.get(id) {
                    sample_nuke_arc(path, self.map_w, progress, &mut arc_scratch);
                    self.push_trail_segments(
                        renderer,
                        &arc_scratch,
                        world_pos,
                        trail_width,
                        slot.trail_color,
                    );
                }
            } else if slot.trail_len > 0 {
                let start = slot.trail_start as usize;
                let end = start + slot.trail_len as usize;
                let trail_points = &self.trail_points[start..end];
                if self.arc_visible(
                    trail_points,
                    world_pos,
                    params,
                    (min_sx, min_sy, max_sx, max_sy),
                ) {
                    self.push_trail_segments(
                        renderer,
                        trail_points,
                        world_pos,
                        trail_width,
                        slot.trail_color,
                    );
                }
            }

            if !sprite_visible {
                continue;
            }

            let dx = slot.curr_x - slot.prev_x;
            let dy = slot.curr_y - slot.prev_y;
            let rotation = if slot.arc_trail && arc_scratch.len() >= 2 {
                let last = arc_scratch[arc_scratch.len() - 2];
                let dir_x = world_pos[0] - last[0];
                let dir_y = world_pos[1] - last[1];
                if dir_x * dir_x + dir_y * dir_y > 1e-8 {
                    dir_y.atan2(dir_x) + std::f32::consts::FRAC_PI_2
                } else {
                    0.0
                }
            } else if slot.trail_len > 0 {
                let last = self.trail_points[(slot.trail_start + slot.trail_len - 1) as usize];
                let dir_x = world_pos[0] - last[0];
                let dir_y = world_pos[1] - last[1];
                if dir_x * dir_x + dir_y * dir_y > 1e-8 {
                    dir_y.atan2(dir_x) + std::f32::consts::FRAC_PI_2
                } else if dx * dx + dy * dy > 1e-8 {
                    dy.atan2(dx) + std::f32::consts::FRAC_PI_2
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

            let mut sprite_size = slot.size * scale;
            if !slot.is_fleet {
                let screen_size = sprite_size * params.camera_zoom;
                if screen_size < MIN_PROJECTILE_SCREEN_PX {
                    sprite_size = MIN_PROJECTILE_SCREEN_PX / params.camera_zoom;
                }
            }

            renderer.push_sprite(MoverInstanceGpu {
                world_pos: [wx, wy],
                size: sprite_size,
                rotation,
                color: slot.color,
                uv_rect: slot.sprite.uv_rect(),
                height: height * NUKE_ARC_LIFT,
            });
        }
    }
}

impl Default for MoverScene {
    fn default() -> Self {
        Self::new()
    }
}

pub fn interp_alpha(time: &crate::app::TimeState, now: Instant) -> f32 {
    time.interp.alpha(now)
}
