// Bakes packed territory texels: owner (0..9), border_mask (10..15), terrain byte (16..23), flash (24..31).
// Neighbor order for border_mask bits 0..5 matches `GameMap::for_each_neighbor` in sow-core/src/map.rs
// and edge direction tests in map.wgsl (odd vs even row).
struct MapComputeGlobals {
    width: u32,
    height: u32,
}

@group(0) @binding(0)
var<storage, read> raw_data: array<u32>;

@group(0) @binding(1)
var<storage, read_write> baked_data: array<u32>;

@group(0) @binding(2)
var<uniform> globals: MapComputeGlobals;

fn neighbor_delta(bit: u32, odd_row: bool) -> vec2<i32> {
    if odd_row {
        switch bit {
            case 0u: { return vec2<i32>(1, 0); }
            case 1u: { return vec2<i32>(-1, 0); }
            case 2u: { return vec2<i32>(0, -1); }
            case 3u: { return vec2<i32>(1, -1); }
            case 4u: { return vec2<i32>(0, 1); }
            case 5u: { return vec2<i32>(1, 1); }
            default: { return vec2<i32>(0, 0); }
        }
    } else {
        switch bit {
            case 0u: { return vec2<i32>(1, 0); }
            case 1u: { return vec2<i32>(-1, 0); }
            case 2u: { return vec2<i32>(-1, -1); }
            case 3u: { return vec2<i32>(0, -1); }
            case 4u: { return vec2<i32>(-1, 1); }
            case 5u: { return vec2<i32>(0, 1); }
            default: { return vec2<i32>(0, 0); }
        }
    }
}

@compute @workgroup_size(64, 1, 1)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let w = globals.width;
    let h = globals.height;
    let total = w * h;
    let i = gid.x;
    if i >= total {
        return;
    }

    let x = i32(i % w);
    let y = i32(i / w);
    let raw = raw_data[i];
    let owner = raw & 0x3FFu;
    let terrain_byte = (raw >> 16u) & 0xFFu;
    let flash = (raw >> 24u) & 0xFFu;
    let c_land = (terrain_byte & 0x80u) != 0u;

    let odd_row = (y % 2) != 0;
    var mask = 0u;
    for (var b = 0u; b < 6u; b++) {
        let d = neighbor_delta(b, odd_row);
        let nx = x + d.x;
        let ny = y + d.y;
        if nx < 0 || ny < 0 || nx >= i32(w) || ny >= i32(h) {
            mask |= 1u << b;
            continue;
        }
        let ni = u32(ny) * w + u32(nx);
        let nraw = raw_data[ni];
        let n_owner = nraw & 0x3FFu;
        let n_terrain = (nraw >> 16u) & 0xFFu;
        let n_land = (n_terrain & 0x80u) != 0u;
        if n_owner != owner {
            mask |= 1u << b;
            continue;
        }
        if c_land != n_land {
            mask |= 1u << b;
        }
    }

    let out = owner | (mask << 10u) | (terrain_byte << 16u) | (flash << 24u);
    baked_data[i] = out;
}
