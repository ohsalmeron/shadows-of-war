use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct MapGlobals {
    pub camera_pos: [f32; 2],
    pub zoom: f32,
    pub time: f32,
    pub screen_size: [f32; 2],
    pub map_size: [f32; 2],
    pub border_thickness: f32,
    pub border_darkness: f32,
    pub shore_thickness: f32,
    pub shore_darkness: f32,
    /// Up to 8 attack threat slots: [front_x, front_y, radius, packed_ids].
    pub threat_slots: [[f32; 4]; 8],
    pub effect_shockwave: f32,
    pub effect_breathe: f32,
    pub effect_energy_flow: f32,
    pub my_player_id: f32,
    pub hover_hex: [f32; 2],
    pub hover_building_kind: f32,
    pub territory_opacity: f32,
    /// Up to 8 fallout zones: [center_col, center_row, radius, alpha_progress].
    pub fallout_slots: [[f32; 4]; 8],
    /// Up to 32 nobuild exclusion zones: [center_col, center_row, radius, active].
    pub nobuild_slots: [[f32; 4]; 32],
    pub blend_mode: f32,
    pub effect_heartbeat: f32,
    pub effect_war_fog: f32,
    pub effect_fallout: f32,
    pub effect_golden_hour: f32,
    pub effect_holo_grid: f32,
    /// Player ID whose borders should flash red (0 = none).
    pub attack_flash_target: f32,
    /// Attack border flash progress: 1.0 → 0.0 ease-out.
    pub attack_flash_t: f32,
    /// Viewport alert vignette intensity: 0.0 → 1.0.
    pub alert_intensity: f32,
    pub fog_of_war: f32,
    pub _pad1: f32,
    pub _pad2: f32,
    /// Viewport alert vignette color: [r, g, b, a].
    pub alert_color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct PlayerColors {
    pub colors: [[f32; 4]; 256],
}

pub(crate) fn get_neighbors(idx: u32, width: u32, height: u32) -> [Option<u32>; 4] {
    let x = idx % width;
    let y = idx / width;
    let deltas = [
        (1, 0),  // East
        (-1, 0), // West
        (0, -1), // North
        (0, 1),  // South
    ];
    let mut neighbors = [None; 4];
    for (i, &(dx, dy)) in deltas.iter().enumerate() {
        let nx = x as i32 + dx;
        let ny = y as i32 + dy;
        if nx >= 0 && nx < width as i32 && ny >= 0 && ny < height as i32 {
            neighbors[i] = Some((ny as u32) * width + (nx as u32));
        }
    }
    neighbors
}

pub(crate) fn compute_has_border(idx: u32, owners: &[u16], width: u32, height: u32) -> bool {
    let owner = owners[idx as usize];
    if owner == 0 {
        return false;
    }
    let neighbors = get_neighbors(idx, width, height);
    for &n_idx in &neighbors {
        if let Some(n) = n_idx {
            if owners[n as usize] != owner {
                return true;
            }
        } else {
            // Map edge has out-of-bounds neighbor (treated as owner 0)
            if owner != 0 {
                return true;
            }
        }
    }
    false
}

pub(crate) fn get_elevation_cpu(x: i32, y: i32, width: u32, height: u32, terrain: &[u8]) -> f32 {
    if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
        return 0.0;
    }
    let terrain_byte = terrain[(y as u32 * width + x as u32) as usize];
    let is_land = (terrain_byte & 0x80) != 0;
    if is_land {
        (terrain_byte & 0x1F) as f32
    } else {
        0.0
    }
}

pub(crate) fn compute_terrain_gradient(
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    terrain: &[u8],
) -> (f32, f32) {
    let cell_x = x as i32;
    let cell_y = y as i32;

    let h_right = get_elevation_cpu(cell_x + 1, cell_y, width, height, terrain);
    let h_left = get_elevation_cpu(cell_x - 1, cell_y, width, height, terrain);
    let h_up = get_elevation_cpu(cell_x, cell_y - 1, width, height, terrain);
    let h_down = get_elevation_cpu(cell_x, cell_y + 1, width, height, terrain);

    let dx = (h_right - h_left) * 0.10;
    let dy = (h_down - h_up) * 0.10;
    (dx, dy)
}

pub(crate) fn fill_terrain_buffer(
    terrain: &[u8],
    width: u32,
    height: u32,
    terrain_bytes_per_row: u32,
    terrain_slice: &mut [u8],
) {
    for y in 0..height {
        for x in 0..width {
            let src = (y * width + x) as usize;
            let dst = (y * terrain_bytes_per_row + x * 4) as usize;

            let terrain_byte = terrain[src];
            let (dx, dy) = compute_terrain_gradient(x, y, width, height, terrain);

            let packed_dx = (((dx + 8.0) / 16.0) * 255.0).round().clamp(0.0, 255.0) as u8;
            let packed_dy = (((dy + 8.0) / 16.0) * 255.0).round().clamp(0.0, 255.0) as u8;

            let seed = (x as u32)
                .wrapping_mul(374761393)
                .wrapping_add((y as u32).wrapping_mul(668265263));
            let mut hash = seed;
            hash ^= hash >> 16;
            hash = hash.wrapping_mul(0x85ebca6b);
            hash ^= hash >> 13;
            hash = hash.wrapping_mul(0xc2b2ae35);
            hash ^= hash >> 16;
            let noise_byte = (hash & 0xFF) as u8;

            terrain_slice[dst] = terrain_byte;
            terrain_slice[dst + 1] = packed_dx;
            terrain_slice[dst + 2] = packed_dy;
            terrain_slice[dst + 3] = noise_byte;
        }
    }
}
