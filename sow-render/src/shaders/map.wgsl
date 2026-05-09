struct Globals {
    camera_pos: vec2<f32>,
    zoom: f32,
    time: f32,
    screen_size: vec2<f32>,
    map_size: vec2<f32>,
}

var<uniform> globals: Globals;
var territory_texture: texture_2d<u32>;
var territory_sampler: sampler;

var water_texture: texture_2d<f32>;
var water_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32((in_vertex_index & 1u) << 2u);
    let y = f32((in_vertex_index & 2u) << 1u);
    out.clip_position = vec4<f32>(x - 1.0, 1.0 - y, 0.0, 1.0);
    out.uv = vec2<f32>(x * 0.5, y * 0.5);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let screen_pixel = in.uv * globals.screen_size;
    let world_x = (screen_pixel.x - globals.camera_pos.x) / globals.zoom;
    let world_y = (screen_pixel.y - globals.camera_pos.y) / globals.zoom;

    if world_x < 0.0 || world_y < 0.0 || world_x >= globals.map_size.x || world_y >= globals.map_size.y {
        return vec4<f32>(0.02, 0.03, 0.06, 1.0);
    }

    let pixel_coords = vec2<i32>(i32(world_x), i32(world_y));
    let val = textureLoad(territory_texture, pixel_coords, 0).x;
    
    let owner_id = val & 0xFFFFu;
    let terrain_byte = (val >> 16u) & 0xFFu;
    let is_land = (terrain_byte & 0x80u) != 0u;

    // Check neighbors for borders (simple edge detection via sampling)
    let up = textureLoad(territory_texture, pixel_coords + vec2<i32>(0, -1), 0).x & 0xFFFFu;
    let down = textureLoad(territory_texture, pixel_coords + vec2<i32>(0, 1), 0).x & 0xFFFFu;
    let left = textureLoad(territory_texture, pixel_coords + vec2<i32>(-1, 0), 0).x & 0xFFFFu;
    let right = textureLoad(territory_texture, pixel_coords + vec2<i32>(1, 0), 0).x & 0xFFFFu;
    
    let is_border = (owner_id != up || owner_id != down || owner_id != left || owner_id != right);

    var base_color = vec4<f32>(0.0);

    // Player colors
    if owner_id == 1u {
        base_color = vec4<f32>(0.133, 0.400, 1.0, 1.0); // #2266FF
    } else if owner_id == 100u {
        base_color = vec4<f32>(1.0, 0.267, 0.267, 1.0); // #FF4444
    } else if owner_id == 101u {
        base_color = vec4<f32>(0.267, 1.0, 0.267, 1.0); // #44FF44
    } else if owner_id == 102u {
        base_color = vec4<f32>(1.0, 1.0, 0.267, 1.0); // #FFFF44
    } else if owner_id == 103u {
        base_color = vec4<f32>(0.667, 0.267, 0.667, 1.0); // #AA44AA
    } else if owner_id > 0u {
        base_color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    } else {
        if is_land {
            base_color = vec4<f32>(0.176, 0.298, 0.118, 1.0); // Unowned Land
        } else {
            // High-performance tiled water shader (webgpu-water style)
            let t = globals.time;
            
            // Fast scrolling UVs
            let uv_scale = 0.05;
            let uv1 = vec2<f32>(world_x, world_y) * uv_scale + vec2<f32>(t * 0.02, t * 0.015);
            let uv2 = vec2<f32>(world_x, world_y) * (uv_scale * 1.5) - vec2<f32>(t * 0.01, t * 0.025);
            
            // Sample seamless noise texture
            let n1 = textureSampleLevel(water_texture, water_sampler, uv1, 0.0).r;
            let n2 = textureSampleLevel(water_texture, water_sampler, uv2, 0.0).r;
            let wave = (n1 + n2) * 0.5;
            
            // Vibrant cyan/teal colors inspired by webgpu-water's ABOVEwaterColor
            // webgpu-water uses vec3(0.25, 1.0, 1.25) which we map to an SDR-friendly range
            let deep = vec4<f32>(0.15, 0.6, 0.75, 1.0);
            let shallow = vec4<f32>(0.25, 0.9, 1.0, 1.0);
            
            // Add a caustic-like highlight on the wave peaks
            let highlight = pow(wave, 3.0) * 0.5;
            
            return mix(deep, shallow, wave) + vec4<f32>(highlight, highlight, highlight, 0.0);
        }
    }

    if owner_id > 0u {
        if is_border {
            // Strong bright border for owned territory
            return min(base_color * 1.5 + vec4<f32>(0.2, 0.2, 0.2, 0.0), vec4<f32>(1.0));
        } else {
            return base_color * 0.85;
        }
    }

    return base_color;
}
