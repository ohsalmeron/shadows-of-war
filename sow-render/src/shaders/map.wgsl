struct Globals {
    camera_pos: vec2<f32>,
    zoom: f32,
    time: f32,
    screen_size: vec2<f32>,
    map_size: vec2<f32>,
    visual_terrain_sharpness: f32,
    visual_interior_alpha: f32,
    visual_border_alpha: f32,
    lod_2_zoom: f32,
    lod_3_zoom: f32,
    local_player_id: u32,
    padding1: f32,
    padding2: f32,
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

fn world_to_hex(world: vec2<f32>) -> vec2<i32> {
    let q_f = world.x - world.y * 0.577350269;
    let r_f = world.y * 1.154700538;
    let s_f = -q_f - r_f;

    var rq = round(q_f);
    var rr = round(r_f);
    var rs = round(s_f);

    let q_diff = abs(rq - q_f);
    let r_diff = abs(rr - r_f);
    let s_diff = abs(rs - s_f);

    if (q_diff > r_diff && q_diff > s_diff) {
        rq = -rr - rs;
    } else if (r_diff > s_diff) {
        rr = -rq - rs;
    }
    
    let q = i32(rq);
    let r = i32(rr);
    let col = q + (r - (r & 1i)) / 2i;
    let row = r;
    return vec2<i32>(col, row);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let screen_pixel = in.uv * globals.screen_size;
    let world_x = (screen_pixel.x - globals.camera_pos.x) / globals.zoom;
    let world_y = (screen_pixel.y - globals.camera_pos.y) / globals.zoom;

    let hex_coord = world_to_hex(vec2<f32>(world_x, world_y));
    let cell_x = hex_coord.x;
    let cell_y = hex_coord.y;

    if cell_x < 0 || cell_y < 0 || cell_x >= i32(globals.map_size.x) || cell_y >= i32(globals.map_size.y) {
        return vec4<f32>(0.02, 0.03, 0.06, 1.0);
    }

    let pixel_coords = vec2<i32>(cell_x, cell_y);
    let val = textureLoad(territory_texture, pixel_coords, 0).x;
    
    let owner_id = val & 0xFFFFu;
    let terrain_byte = (val >> 16u) & 0xFFu;
    let is_land = (terrain_byte & 0x80u) != 0u;

    let is_odd = (cell_y % 2) != 0;
    
    var offsets: array<vec2<i32>, 6>;
    if is_odd {
        offsets = array<vec2<i32>, 6>(
            vec2<i32>(1, 0), vec2<i32>(-1, 0),
            vec2<i32>(0, -1), vec2<i32>(1, -1),
            vec2<i32>(0, 1), vec2<i32>(1, 1)
        );
    } else {
        offsets = array<vec2<i32>, 6>(
            vec2<i32>(1, 0), vec2<i32>(-1, 0),
            vec2<i32>(-1, -1), vec2<i32>(0, -1),
            vec2<i32>(-1, 1), vec2<i32>(0, 1)
        );
    }

    let val_0 = textureLoad(territory_texture, pixel_coords + offsets[0], 0).x;
    let val_1 = textureLoad(territory_texture, pixel_coords + offsets[1], 0).x;
    let val_2 = textureLoad(territory_texture, pixel_coords + offsets[2], 0).x;
    let val_3 = textureLoad(territory_texture, pixel_coords + offsets[3], 0).x;
    let val_4 = textureLoad(territory_texture, pixel_coords + offsets[4], 0).x;
    let val_5 = textureLoad(territory_texture, pixel_coords + offsets[5], 0).x;

    let own_0 = val_0 & 0xFFFFu;
    let own_1 = val_1 & 0xFFFFu;
    let own_2 = val_2 & 0xFFFFu;
    let own_3 = val_3 & 0xFFFFu;
    let own_4 = val_4 & 0xFFFFu;
    let own_5 = val_5 & 0xFFFFu;

    let is_border = (owner_id != own_0 || owner_id != own_1 || owner_id != own_2 || 
                     owner_id != own_3 || owner_id != own_4 || owner_id != own_5);

    var base_color = vec4<f32>(0.0);

    let mag_center = f32(terrain_byte & 0x1Fu);
    let mag_left = f32((val_1 >> 16u) & 0x1Fu);
    let mag_up = f32((val_2 >> 16u) & 0x1Fu);

    // Topographical shading (bump mapping approximation, sun from top-left)
    let dx = mag_center - mag_left;
    let dy = mag_center - mag_up;
    let shade = (dx + dy) * globals.visual_terrain_sharpness; // Configurable shading intensity

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
        // High-performance tiled water shader with LODs based on zoom
        let t = globals.time;
        var wave = 0.0;
        
        // Always calculate the base large waves
        let uv0 = vec2<f32>(world_x, world_y) * 0.02 + vec2<f32>(0.015, 0.010) * t;
        let n0 = textureSampleLevel(water_texture, water_sampler, uv0, 0.0).r;
        wave += n0 * 0.5;

        // Add medium waves only if we are somewhat zoomed in
        let min_lod = min(globals.lod_2_zoom, globals.lod_3_zoom);
        let max_lod = max(globals.lod_2_zoom, globals.lod_3_zoom);
        
        if globals.zoom >= min_lod * 0.5 {
            let uv1 = vec2<f32>(world_x, world_y) * 0.04 + vec2<f32>(-0.020, 0.015) * t;
            let n1 = textureSampleLevel(water_texture, water_sampler, uv1, 0.0).r;
            wave += n1 * 0.25;
            
            // Add fine detail waves only when closely zoomed in
            if globals.zoom >= max_lod {
                let uv2 = vec2<f32>(world_x, world_y) * 0.08 + vec2<f32>(0.025, -0.010) * t;
                let uv3 = vec2<f32>(world_x, world_y) * 0.16 + vec2<f32>(-0.010, -0.025) * t;
                let n2 = textureSampleLevel(water_texture, water_sampler, uv2, 0.0).r;
                let n3 = textureSampleLevel(water_texture, water_sampler, uv3, 0.0).r;
                wave += n2 * 0.125 + n3 * 0.0625;
            }
        }
        
        let pool_dark = vec3<f32>(0.01, 0.35, 0.55);
        let pool_light = vec3<f32>(0.15, 0.75, 0.85);
        let color = mix(pool_dark, pool_light, wave);
        let specular = pow(wave, 12.0) * 1.5;
        
        return vec4<f32>(color + vec3<f32>(specular), 1.0);
    }

    var player_color = vec4<f32>(1.0);
    var has_player_color = false;
    
    var is_human = false;
    var is_nation = false;
    var is_tribe = false;
    
    if owner_id > 0u {
        has_player_color = true;
        
        is_human = owner_id <= 16u;
        is_nation = owner_id > 16u && owner_id <= 116u;
        is_tribe = owner_id > 116u;

        if is_human {
            // Highly saturated colors for humans based on golden ratio hue
            let hue = f32(owner_id) * 0.618033988749895;
            let r = 0.5 + 0.5 * sin(hue * 6.28318 + 0.0);
            let g = 0.5 + 0.5 * sin(hue * 6.28318 + 2.09439);
            let b = 0.5 + 0.5 * sin(hue * 6.28318 + 4.18879);
            player_color = vec4<f32>(r, g, b, 1.0);
        } else if is_nation {
            // Mid-tone stable colors for nations
            let id = f32(owner_id);
            let r = 0.3 + 0.5 * fract(sin(id * 12.9898) * 43758.5453);
            let g = 0.3 + 0.5 * fract(sin(id * 78.233) * 43758.5453);
            let b = 0.3 + 0.5 * fract(sin(id * 39.346) * 43758.5453);
            player_color = vec4<f32>(r, g, b, 1.0);
        } else {
            // Soft, washed-out pastels for tribes
            let id = f32(owner_id);
            let r = 0.5 + 0.3 * fract(sin(id * 12.9898) * 43758.5453);
            let g = 0.5 + 0.3 * fract(sin(id * 78.233) * 43758.5453);
            let b = 0.5 + 0.3 * fract(sin(id * 39.346) * 43758.5453);
            player_color = vec4<f32>(r, g, b, 1.0);
        }
        
        if is_human {
            // Holographic Animated Cyber-Stripes (Additive Blending)
            let stripe = (sin((world_x + world_y) * 0.15 - globals.time * 2.5) + 1.0) * 0.5;
            let holo_color = player_color.rgb * (0.6 + 0.6 * stripe);
            base_color = vec4<f32>(terrain_color.rgb + holo_color * globals.visual_interior_alpha * 0.7, 1.0);
        } else if is_nation {
            // Static Additive Glow
            base_color = vec4<f32>(terrain_color.rgb + player_color.rgb * globals.visual_interior_alpha * 0.4, 1.0);
        } else {
            // Flat mix for tribes (traditional skin)
            base_color = mix(terrain_color, player_color, globals.visual_interior_alpha * 0.6);
        }
    } else {
        base_color = terrain_color;
    }

    // Border and Shoreline Logic
    // In OpenFront, a border is drawn if the owner changes OR if a land tile borders water
    let is_0_water = ((val_0 >> 16u) & 0x80u) == 0u;
    let is_1_water = ((val_1 >> 16u) & 0x80u) == 0u;
    let is_2_water = ((val_2 >> 16u) & 0x80u) == 0u;
    let is_3_water = ((val_3 >> 16u) & 0x80u) == 0u;
    let is_4_water = ((val_4 >> 16u) & 0x80u) == 0u;
    let is_5_water = ((val_5 >> 16u) & 0x80u) == 0u;
    
    let has_water_neighbor = is_0_water || is_1_water || is_2_water || is_3_water || is_4_water || is_5_water;
    
    var should_draw_border = false;
    if owner_id == 0u {
        // Wilderness draws borders against owned tiles AND shorelines
        should_draw_border = is_border || has_water_neighbor;
    } else {
        // Owned territory draws borders where ownership changes
        should_draw_border = is_border;
    }

    if should_draw_border {
        var border_color: vec4<f32>;
        if has_player_color {
            if owner_id == globals.local_player_id {
                // Pulsating bright highlight for MY borders
                let pulse = (sin(globals.time * 6.0) + 1.0) * 0.5;
                let highlight = mix(player_color.rgb, vec3<f32>(1.0, 1.0, 1.0), pulse * 0.7);
                border_color = vec4<f32>(highlight, 1.0);
                // Force maximum opacity for local player borders
                base_color = border_color;
            } else {
                // Standard contrast border logic
                let luminance = dot(player_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
                if luminance > 0.6 {
                    border_color = vec4<f32>(player_color.rgb * 0.3, 1.0);
                } else {
                    border_color = min(player_color * 1.5 + vec4<f32>(0.2, 0.2, 0.2, 0.0), vec4<f32>(1.0));
                }
                base_color = mix(base_color, border_color, globals.visual_border_alpha);
            }
        } else {
            // Wilderness border (shoreline or adjacent to player)
            border_color = vec4<f32>(terrain_color.rgb * 0.4, 1.0);
            base_color = mix(base_color, border_color, globals.visual_border_alpha * 0.8);
        }
    }

    return base_color;
}
