use super::*;

/// Maximum Euclidean tile distance² to consider connecting two buildings.
const MAX_RAIL_DIST_SQ: f32 = 150.0 * 150.0;
/// Spatial grid cell size in tiles.
const CELL_SIZE: f32 = 150.0;

/// Baked L-shaped rail segment: A → corner → B (cardinal directions only).
struct RailSegment {
    ax: f32,
    ay: f32,
    cx: f32,
    cy: f32,
    bx: f32,
    by: f32,
    birth: f32,
    dead: bool,
    b1: u64,
    b2: u64,
}

struct TrackedBuilding {
    world_x: f32,
    world_y: f32,
    tile_x: f32,
    tile_y: f32,
}

/// Persistent railway state. Incremental: O(1) per building add/remove.
pub struct RailState {
    segments: Vec<RailSegment>,
    grid: std::collections::HashMap<(i32, i32), Vec<u64>>,
    known: std::collections::HashMap<u64, TrackedBuilding>,
    seg_indices: std::collections::HashMap<u64, Vec<usize>>,
    prev_hash: u64,
}

impl Default for RailState {
    fn default() -> Self {
        Self::new()
    }
}

impl RailState {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            grid: std::collections::HashMap::new(),
            known: std::collections::HashMap::new(),
            seg_indices: std::collections::HashMap::new(),
            prev_hash: 0,
        }
    }
}

fn is_eligible(b: &sow_core::protocol::BuildingSnapshot) -> bool {
    b.kind != sow_core::game::BuildingKind::Bunker && !b.under_construction
}

fn tile_to_world(tx: f32, ty: f32) -> (f32, f32) {
    (tx + 0.5, ty + 0.5)
}

fn grid_cell(tx: f32, ty: f32) -> (i32, i32) {
    ((tx / CELL_SIZE) as i32, (ty / CELL_SIZE) as i32)
}

/// Walk an axis-aligned line, return true if any tile is water.
fn walk_axis_water(
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    terrain: &[u8],
    map_w: u32,
    map_h: u32,
) -> bool {
    let mut x = x0;
    let mut y = y0;
    let sx = (x1 - x0).signum();
    let sy = (y1 - y0).signum();
    loop {
        if x >= 0 && y >= 0 && (x as u32) < map_w && (y as u32) < map_h {
            let idx = (y as u32 * map_w + x as u32) as usize;
            if idx < terrain.len() && (terrain[idx] & 0x80) == 0 {
                return true;
            }
        }
        if x == x1 && y == y1 {
            break;
        }
        x += sx;
        y += sy;
    }
    false
}

/// Check if an L-shaped path crosses water.
#[allow(clippy::too_many_arguments)]
fn l_crosses_water(
    ax: i32,
    ay: i32,
    bx: i32,
    by: i32,
    h_first: bool,
    terrain: &[u8],
    map_w: u32,
    map_h: u32,
) -> bool {
    let (mx, my) = if h_first { (bx, ay) } else { (ax, by) };
    walk_axis_water(ax, ay, mx, my, terrain, map_w, map_h)
        || walk_axis_water(mx, my, bx, by, terrain, map_w, map_h)
}

fn closest_point_on_segment(p: (f32, f32), s0: (f32, f32), s1: (f32, f32)) -> (f32, f32) {
    let (px, py) = p;
    let (x0, y0) = s0;
    let (x1, y1) = s1;
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-5 {
        return (x0, y0);
    }
    let t = (((px - x0) * dx + (py - y0) * dy) / len2).clamp(0.0, 1.0);
    (x0 + t * dx, y0 + t * dy)
}

#[allow(clippy::too_many_arguments)]
fn find_snap_point(
    state: &RailState,
    p_tx: f32,
    p_ty: f32,
    p_wx: f32,
    p_wy: f32,
    terrain: &[u8],
    map_w: u32,
    map_h: u32,
) -> Option<(f32, f32, bool, usize)> {
    let mut best: Option<(f32, f32, bool, usize, f32)> = None;

    for (seg_idx, seg) in state.segments.iter().enumerate() {
        if seg.dead {
            continue;
        }

        let q1 = closest_point_on_segment((p_wx, p_wy), (seg.ax, seg.ay), (seg.cx, seg.cy));
        let q2 = closest_point_on_segment((p_wx, p_wy), (seg.cx, seg.cy), (seg.bx, seg.by));

        for q in [q1, q2] {
            let q_ty = (q.1 - 0.5).round();
            let q_tx = (q.0 - 0.5).round();

            let dtx = p_tx - q_tx;
            let dty = p_ty - q_ty;
            let d2 = dtx * dtx + dty * dty;
            if d2 >= MAX_RAIL_DIST_SQ {
                continue;
            }

            let h_first = if !l_crosses_water(
                p_tx as i32,
                p_ty as i32,
                q_tx as i32,
                q_ty as i32,
                true,
                terrain,
                map_w,
                map_h,
            ) {
                Some(true)
            } else if !l_crosses_water(
                p_tx as i32,
                p_ty as i32,
                q_tx as i32,
                q_ty as i32,
                false,
                terrain,
                map_w,
                map_h,
            ) {
                Some(false)
            } else {
                None
            };

            if let Some(h_first_val) = h_first {
                if best.is_none_or(|(_, _, _, _, bd)| d2 < bd) {
                    best = Some((q.0, q.1, h_first_val, seg_idx, d2));
                }
            }
        }
    }

    best.map(|(qx, qy, h_first, seg_idx, _)| (qx, qy, h_first, seg_idx))
}

/// Create an L-shaped segment between two known buildings.
fn try_connect(
    state: &mut RailState,
    id_a: u64,
    id_b: u64,
    terrain: &[u8],
    map_w: u32,
    map_h: u32,
    now: f32,
) -> bool {
    let (a_tx, a_ty, a_wx, a_wy) = match state.known.get(&id_a) {
        Some(t) => (t.tile_x, t.tile_y, t.world_x, t.world_y),
        None => return false,
    };
    let (b_tx, b_ty, b_wx, b_wy) = match state.known.get(&id_b) {
        Some(t) => (t.tile_x, t.tile_y, t.world_x, t.world_y),
        None => return false,
    };

    let h_first = if !l_crosses_water(
        a_tx as i32,
        a_ty as i32,
        b_tx as i32,
        b_ty as i32,
        true,
        terrain,
        map_w,
        map_h,
    ) {
        true
    } else if !l_crosses_water(
        a_tx as i32,
        a_ty as i32,
        b_tx as i32,
        b_ty as i32,
        false,
        terrain,
        map_w,
        map_h,
    ) {
        false
    } else {
        return false;
    };

    // Corner in WORLD space — truly horizontal/vertical on screen
    let (cx, cy) = if h_first {
        (b_wx, a_wy) // horizontal to B's X, then vertical
    } else {
        (a_wx, b_wy) // vertical to B's Y, then horizontal
    };

    let seg_idx = state.segments.len();
    state.segments.push(RailSegment {
        ax: a_wx,
        ay: a_wy,
        cx,
        cy,
        bx: b_wx,
        by: b_wy,
        birth: now,
        dead: false,
        b1: id_a,
        b2: id_b,
    });
    state.seg_indices.entry(id_a).or_default().push(seg_idx);
    state.seg_indices.entry(id_b).or_default().push(seg_idx);
    true
}

/// Add a single building to the rail network.
/// Priority: 1) nearest building already on the network (join existing rail),
/// 2) nearest building, 3) pull lonely neighbors toward us.
fn add_building(
    state: &mut RailState,
    id: u64,
    tile_idx: u32,
    map_w: u32,
    map_h: u32,
    terrain: &[u8],
    now: f32,
) {
    let tx = (tile_idx % map_w) as f32;
    let ty = (tile_idx / map_w) as f32;
    let (wx, wy) = tile_to_world(tx, ty);
    let gc = grid_cell(tx, ty);

    state.grid.entry(gc).or_default().push(id);
    state.known.insert(
        id,
        TrackedBuilding {
            world_x: wx,
            world_y: wy,
            tile_x: tx,
            tile_y: ty,
        },
    );

    // Priority 1: Connect to nearest existing active railway segment (snapping)
    if let Some((qx, qy, h_first, parent_idx)) =
        find_snap_point(state, tx, ty, wx, wy, terrain, map_w, map_h)
    {
        let (cx, cy) = if h_first { (qx, wy) } else { (wx, qy) };
        let parent_b1 = state.segments[parent_idx].b1;
        let parent_b2 = state.segments[parent_idx].b2;

        let seg_idx = state.segments.len();
        state.segments.push(RailSegment {
            ax: wx,
            ay: wy,
            cx,
            cy,
            bx: qx,
            by: qy,
            birth: now,
            dead: false,
            b1: id,
            b2: parent_b1,
        });

        state.seg_indices.entry(id).or_default().push(seg_idx);
        state
            .seg_indices
            .entry(parent_b1)
            .or_default()
            .push(seg_idx);
        state
            .seg_indices
            .entry(parent_b2)
            .or_default()
            .push(seg_idx);
        return;
    }

    // Gather all nearby valid buildings within range
    let mut candidates = Vec::new();

    for dx in -1..=1 {
        for dy in -1..=1 {
            if let Some(cell) = state.grid.get(&(gc.0 + dx, gc.1 + dy)) {
                for &other_id in cell {
                    if other_id == id {
                        continue;
                    }
                    if let Some(other) = state.known.get(&other_id) {
                        let dtx = tx - other.tile_x;
                        let dty = ty - other.tile_y;
                        let d2 = dtx * dtx + dty * dty;
                        if d2 >= MAX_RAIL_DIST_SQ {
                            continue;
                        }

                        let has_path = !l_crosses_water(
                            tx as i32,
                            ty as i32,
                            other.tile_x as i32,
                            other.tile_y as i32,
                            true,
                            terrain,
                            map_w,
                            map_h,
                        ) || !l_crosses_water(
                            tx as i32,
                            ty as i32,
                            other.tile_x as i32,
                            other.tile_y as i32,
                            false,
                            terrain,
                            map_w,
                            map_h,
                        );

                        if has_path {
                            candidates.push((other_id, d2));
                        }
                    }
                }
            }
        }
    }

    // Sort by distance ascending
    candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    // Connect to up to two closest buildings to act as a bridge
    if let Some(&(first_id, _)) = candidates.first() {
        try_connect(state, id, first_id, terrain, map_w, map_h, now);
        if let Some(&(second_id, _)) = candidates.get(1) {
            try_connect(state, id, second_id, terrain, map_w, map_h, now);
        }
    }
}

fn remove_building(state: &mut RailState, id: u64) {
    if let Some(tracked) = state.known.remove(&id) {
        let gc = grid_cell(tracked.tile_x, tracked.tile_y);
        if let Some(cell) = state.grid.get_mut(&gc) {
            cell.retain(|&x| x != id);
            if cell.is_empty() {
                state.grid.remove(&gc);
            }
        }
    }
    if let Some(indices) = state.seg_indices.remove(&id) {
        for idx in indices {
            if idx < state.segments.len() {
                state.segments[idx].dead = true;
            }
        }
    }
}

fn hash_buildings(buildings: &[sow_core::protocol::BuildingSnapshot]) -> u64 {
    let mut h: u64 = buildings.len() as u64;
    for b in buildings {
        h = h
            .wrapping_mul(6364136223846793005)
            .wrapping_add(b.id ^ (b.under_construction as u64) << 32);
    }
    h
}

fn sync(
    state: &mut RailState,
    buildings: &[sow_core::protocol::BuildingSnapshot],
    terrain: &[u8],
    map_w: u32,
    map_h: u32,
    now: f32,
) {
    let mut current: std::collections::HashSet<u64> =
        std::collections::HashSet::with_capacity(buildings.len());
    for b in buildings {
        if is_eligible(b) {
            current.insert(b.id);
        }
    }

    let removed: Vec<u64> = state
        .known
        .keys()
        .filter(|id| !current.contains(id))
        .copied()
        .collect();
    for id in removed {
        remove_building(state, id);
    }

    for b in buildings {
        if is_eligible(b) && !state.known.contains_key(&b.id) {
            add_building(state, b.id, b.tile_idx, map_w, map_h, terrain, now);
        }
    }
}

pub(crate) fn render(
    ui: &mut crate::app::UiState,
    sim: &crate::app::SimState,
    input: &crate::app::InputState,
    time: &crate::app::TimeState,
    _gfx: &crate::app::GraphicsState,
    ctx: &RenderContext,
) {
    let zoom_scaled = ctx.zoom_scaled;
    if !crate::app::vfx_on(ctx.painter.ctx(), |f| f.railways) {
        return;
    }
    if zoom_scaled < super::BUILDINGS_HIDE_FLOOR {
        return;
    }

    let snap = match &sim.current_snapshot {
        Some(s) => s,
        None => return,
    };

    let h = hash_buildings(&snap.buildings);
    if h != ui.rail_state.prev_hash {
        ui.rail_state.prev_hash = h;
        let now = time.start_time.elapsed().as_secs_f32();
        sync(
            &mut ui.rail_state,
            &snap.buildings,
            ctx.terrain,
            sim.map_w,
            sim.map_h,
            now,
        );
    }

    if ui.rail_state.segments.is_empty() {
        return;
    }

    let sf = ctx.sf;
    let now = time.start_time.elapsed().as_secs_f32();

    let painter = ctx.painter.ctx().layer_painter(egui::LayerId::new(
        egui::Order::Background,
        egui::Id::new("world_railways"),
    ));

    let sw = input.screen_w / sf;
    let sh = input.screen_h / sf;

    let alpha_t = ((zoom_scaled - super::BUILDINGS_HIDE_FLOOR) / 0.4).clamp(0.0, 1.0);
    let base_alpha = (alpha_t * 100.0) as u8;
    let rail_color = egui::Color32::from_rgba_unmultiplied(90, 80, 70, base_alpha);
    let rail_stroke = egui::Stroke::new(2.5_f32, rail_color);

    for seg in &ui.rail_state.segments {
        if seg.dead {
            continue;
        }

        let s_ax = (input.camera_x + seg.ax * input.camera_zoom) / sf;
        let s_ay = (input.camera_y + seg.ay * input.camera_zoom) / sf;
        let s_cx = (input.camera_x + seg.cx * input.camera_zoom) / sf;
        let s_cy = (input.camera_y + seg.cy * input.camera_zoom) / sf;
        let s_bx = (input.camera_x + seg.bx * input.camera_zoom) / sf;
        let s_by = (input.camera_y + seg.by * input.camera_zoom) / sf;

        // Frustum cull
        let min_x = s_ax.min(s_cx).min(s_bx);
        let max_x = s_ax.max(s_cx).max(s_bx);
        let min_y = s_ay.min(s_cy).min(s_by);
        let max_y = s_ay.max(s_cy).max(s_by);
        if max_x < -5.0 || min_x > sw + 5.0 || max_y < -5.0 || min_y > sh + 5.0 {
            continue;
        }

        let age = now - seg.birth;
        let progress = (age / 0.6).clamp(0.0, 1.0);

        // Leg 1: A → C
        let t1 = (progress * 2.0).clamp(0.0, 1.0);
        let l1x = s_ax + (s_cx - s_ax) * t1;
        let l1y = s_ay + (s_cy - s_ay) * t1;
        painter.line_segment([egui::pos2(s_ax, s_ay), egui::pos2(l1x, l1y)], rail_stroke);

        // Leg 2: C → B
        if progress > 0.5 {
            let t2 = ((progress - 0.5) * 2.0).clamp(0.0, 1.0);
            let l2x = s_cx + (s_bx - s_cx) * t2;
            let l2y = s_cy + (s_by - s_cy) * t2;
            painter.line_segment([egui::pos2(s_cx, s_cy), egui::pos2(l2x, l2y)], rail_stroke);
        }
    }
}
