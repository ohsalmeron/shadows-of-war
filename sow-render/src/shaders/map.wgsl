struct Globals {
    camera_pos: vec2<f32>,
    zoom: f32,
    screen_size: vec2<f32>,
    map_size: vec2<f32>,
}

var<uniform> globals: Globals;
var territory_texture: texture_2d<u32>;
var territory_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    // Full-screen triangle
    let x = f32((in_vertex_index & 1u) << 2u);
    let y = f32((in_vertex_index & 2u) << 1u);
    out.clip_position = vec4<f32>(x - 1.0, 1.0 - y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5, y * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // Convert screen pixel to world-space tile coordinate using camera
    let screen_pixel = in.uv * globals.screen_size;
    let world_x = (screen_pixel.x - globals.camera_pos.x) / globals.zoom;
    let world_y = (screen_pixel.y - globals.camera_pos.y) / globals.zoom;

    // Out-of-bounds check — draw deep ocean
    if world_x < 0.0 || world_y < 0.0 || world_x >= globals.map_size.x || world_y >= globals.map_size.y {
        return vec4<f32>(0.02, 0.03, 0.06, 1.0);
    }

    let pixel_coords = vec2<i32>(i32(world_x), i32(world_y));
    let val = textureLoad(territory_texture, pixel_coords, 0).x;

    // Unpack: bits 0..15 = owner_id, bits 16..23 = terrain byte
    let owner_id = val & 0xFFFFu;
    let terrain_byte = (val >> 16u) & 0xFFu;
    let is_land = (terrain_byte & 0x80u) != 0u;

    // Player colors — matches sow-ui/src/main.ts
    if owner_id == 1u {
        // Human player — blue
        return vec4<f32>(0.133, 0.400, 1.0, 1.0); // #2266FF
    } else if owner_id == 100u {
        // Bot 1 — red
        return vec4<f32>(1.0, 0.267, 0.267, 1.0); // #FF4444
    } else if owner_id == 101u {
        // Bot 2 — green
        return vec4<f32>(0.267, 1.0, 0.267, 1.0); // #44FF44
    } else if owner_id == 102u {
        // Bot 3 — yellow
        return vec4<f32>(1.0, 1.0, 0.267, 1.0); // #FFFF44
    } else if owner_id == 103u {
        // Bot 4 — purple
        return vec4<f32>(0.667, 0.267, 0.667, 1.0); // #AA44AA
    } else if owner_id > 0u {
        // Other players — white
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }

    // Unowned terrain
    if is_land {
        return vec4<f32>(0.176, 0.298, 0.118, 1.0); // #2d4c1e dark green
    }

    // Water
    return vec4<f32>(0.118, 0.235, 0.353, 1.0); // #1e3c5a dark blue
}
