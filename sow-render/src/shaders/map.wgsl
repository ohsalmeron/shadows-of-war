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

    // Fetch full values for neighbors to extract both owner and terrain
    let val_up = textureLoad(territory_texture, pixel_coords + vec2<i32>(0, -1), 0).x;
    let val_down = textureLoad(territory_texture, pixel_coords + vec2<i32>(0, 1), 0).x;
    let val_left = textureLoad(territory_texture, pixel_coords + vec2<i32>(-1, 0), 0).x;
    let val_right = textureLoad(territory_texture, pixel_coords + vec2<i32>(1, 0), 0).x;

    let up = val_up & 0xFFFFu;
    let down = val_down & 0xFFFFu;
    let left = val_left & 0xFFFFu;
    let right = val_right & 0xFFFFu;
    
    let is_border = (owner_id != up || owner_id != down || owner_id != left || owner_id != right);

    var base_color = vec4<f32>(0.0);

    let mag_center = f32(terrain_byte & 0x1Fu);
    let mag_up = f32((val_up >> 16u) & 0x1Fu);
    let mag_left = f32((val_left >> 16u) & 0x1Fu);

    // Topographical shading (bump mapping approximation, sun from top-left)
    let dx = mag_center - mag_left;
    let dy = mag_center - mag_up;
    let shade = (dx + dy) * 0.15; // Shading intensity

    var terrain_color = vec4<f32>(0.0);
    
    if is_land {
        // Dynamic Biome Colors
        if mag_center < 10.0 {
            terrain_color = vec4<f32>(0.176, 0.298, 0.118, 1.0); // Lush Plains
        } else if mag_center < 20.0 {
            terrain_color = vec4<f32>(0.45, 0.38, 0.25, 1.0); // Earthy Highlands
        } else {
            let snow = clamp((mag_center - 20.0) / 11.0, 0.0, 1.0);
            terrain_color = mix(vec4<f32>(0.35, 0.35, 0.35, 1.0), vec4<f32>(0.9, 0.9, 0.95, 1.0), snow); // Snowy Mountains
        }
        // Apply topographical shading to land
        terrain_color = vec4<f32>(terrain_color.rgb + vec3<f32>(shade), 1.0);
    } else {
        // High-performance tiled water shader (webgpu-water style via 4-octaves)
        let t = globals.time;
        
        let uv0 = vec2<f32>(world_x, world_y) * 0.02 + vec2<f32>(0.015, 0.010) * t;
        let uv1 = vec2<f32>(world_x, world_y) * 0.04 + vec2<f32>(-0.020, 0.015) * t;
        let uv2 = vec2<f32>(world_x, world_y) * 0.08 + vec2<f32>(0.025, -0.010) * t;
        let uv3 = vec2<f32>(world_x, world_y) * 0.16 + vec2<f32>(-0.010, -0.025) * t;
        
        let n0 = textureSampleLevel(water_texture, water_sampler, uv0, 0.0).r;
        let n1 = textureSampleLevel(water_texture, water_sampler, uv1, 0.0).r;
        let n2 = textureSampleLevel(water_texture, water_sampler, uv2, 0.0).r;
        let n3 = textureSampleLevel(water_texture, water_sampler, uv3, 0.0).r;
        
        let wave = n0 * 0.5 + n1 * 0.25 + n2 * 0.125 + n3 * 0.0625;
        
        let pool_dark = vec3<f32>(0.01, 0.35, 0.55);
        let pool_light = vec3<f32>(0.15, 0.75, 0.85);
        let color = mix(pool_dark, pool_light, wave);
        let specular = pow(wave, 12.0) * 1.5;
        
        return vec4<f32>(color + vec3<f32>(specular), 1.0);
    }

    base_color = terrain_color;

    if owner_id > 0u {
        var player_color = vec4<f32>(1.0);
        if owner_id == 1u {
            player_color = vec4<f32>(0.133, 0.400, 1.0, 1.0); // #2266FF
        } else if owner_id == 100u {
            player_color = vec4<f32>(1.0, 0.267, 0.267, 1.0); // #FF4444
        } else if owner_id == 101u {
            player_color = vec4<f32>(0.267, 1.0, 0.267, 1.0); // #44FF44
        } else if owner_id == 102u {
            player_color = vec4<f32>(1.0, 1.0, 0.267, 1.0); // #FFFF44
        } else if owner_id == 103u {
            player_color = vec4<f32>(0.667, 0.267, 0.667, 1.0); // #AA44AA
        }

        // Mix terrain and player color to make terrain visible!
        base_color = mix(terrain_color, player_color, 0.55);

        if is_border {
            // Strong bright border for owned territory
            return min(base_color * 1.5 + vec4<f32>(0.2, 0.2, 0.2, 0.0), vec4<f32>(1.0));
        } else {
            return base_color * 0.85;
        }
    }

    return base_color;
}
