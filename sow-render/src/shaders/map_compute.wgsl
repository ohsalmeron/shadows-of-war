var<storage, read> raw_data: array<u32>;
var<storage, read_write> baked_data: array<u32>;

struct Globals {
    width: u32,
    height: u32,
}
var<uniform> globals: Globals;

fn check_neighbor(nx: i32, ny: i32, w: i32, h: i32, owner_id: u32, c_is_water: bool) -> bool {
    if nx >= 0 && ny >= 0 && nx < w && ny < h {
        let ni = u32(ny * w + nx);
        let n_val = raw_data[ni];
        let n_owner = n_val & 0x3FFu;
        let n_terrain = (n_val >> 16u) & 0xFFu;
        let n_is_water = (n_terrain & 0x80u) == 0u;
        if c_is_water {
            return !n_is_water;
        } else {
            return (owner_id != n_owner) || (owner_id == 0u && n_is_water);
        }
    }
    return !c_is_water;
}

@compute @workgroup_size(64, 1, 1)
fn cs_main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let index = global_id.x;
    let total = globals.width * globals.height;
    if index >= total {
        return;
    }
    
    let w = i32(globals.width);
    let h = i32(globals.height);
    let x = i32(index) % w;
    let y = i32(index) / w;

    let val = raw_data[index];
    let owner_id = val & 0x3FFu;
    let terrain_byte = (val >> 16u) & 0xFFu;
    let flash = (val >> 24u) & 0xFFu;

    var border_mask = 0u;
    let is_odd = (y % 2) != 0;
    let c_is_water = (terrain_byte & 0x80u) == 0u;
    
    if is_odd {
        if check_neighbor(x+1, y, w, h, owner_id, c_is_water) { border_mask |= 1u; }
        if check_neighbor(x-1, y, w, h, owner_id, c_is_water) { border_mask |= 2u; }
        if check_neighbor(x, y-1, w, h, owner_id, c_is_water) { border_mask |= 4u; }
        if check_neighbor(x+1, y-1, w, h, owner_id, c_is_water) { border_mask |= 8u; }
        if check_neighbor(x, y+1, w, h, owner_id, c_is_water) { border_mask |= 16u; }
        if check_neighbor(x+1, y+1, w, h, owner_id, c_is_water) { border_mask |= 32u; }
    } else {
        if check_neighbor(x+1, y, w, h, owner_id, c_is_water) { border_mask |= 1u; }
        if check_neighbor(x-1, y, w, h, owner_id, c_is_water) { border_mask |= 2u; }
        if check_neighbor(x-1, y-1, w, h, owner_id, c_is_water) { border_mask |= 4u; }
        if check_neighbor(x, y-1, w, h, owner_id, c_is_water) { border_mask |= 8u; }
        if check_neighbor(x-1, y+1, w, h, owner_id, c_is_water) { border_mask |= 16u; }
        if check_neighbor(x, y+1, w, h, owner_id, c_is_water) { border_mask |= 32u; }
    }

    baked_data[index] = owner_id | (border_mask << 10u) | (terrain_byte << 16u) | (flash << 24u);
}
