struct Globals {
    camera_pos: vec2<f32>,
    zoom: f32,
    time: f32,
    screen_size: vec2<f32>,
    map_size: vec2<f32>,
    visual_terrain_sharpness: f32,
    visual_interior_alpha: f32,
    visual_border_alpha: f32,
    visual_border_thickness: f32,
    effect_shockwave_intensity: f32,
    effect_border_breathe: f32,
    effect_energy_flow: f32,
    lod_2_zoom: f32,
    lod_3_zoom: f32,
    local_player_id: u32,
    padding1: u32,
    padding2: u32,
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

fn hex_to_world(cell_x: i32, cell_y: i32) -> vec2<f32> {
    let r = f32(cell_y);
    let q = f32(cell_x - (cell_y - (cell_y & 1)) / 2);
    let y = r * 0.86602540378;
    let x = q + y * 0.577350269;
    return vec2<f32>(x, y);
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
    
    let owner_id = val & 0x3FFu;
    let border_mask = (val >> 10u) & 0x3Fu;
    let terrain_byte = (val >> 16u) & 0xFFu;
    let is_land = (terrain_byte & 0x80u) != 0u;
    
    // Shockwave flash value [0.0 = none, 1.0 = just conquered]
    let flash_byte = (val >> 24u) & 0xFFu;
    let flash_val = f32(flash_byte) / 255.0;

    var base_color = vec4<f32>(0.0);

    let mag_center = f32(terrain_byte & 0x1Fu);

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
    } else {
        let t = globals.time;
        
        let DEEP_OCEAN_COLOR = vec3<f32>(0.01, 0.08, 0.23);
        let COASTAL_COLOR = vec3<f32>(0.1, 0.5, 0.6);
        let FOAM_COLOR = vec3<f32>(0.9, 0.95, 1.0);
        let SPECULAR_COLOR = vec3<f32>(1.0, 0.95, 0.8);
        
        var base_color = DEEP_OCEAN_COLOR;
        
        // Coastal Detection: if border_mask > 0, at least one neighbor is land
        if border_mask > 0u {
            base_color = COASTAL_COLOR;
        }

        // Texture-based organic waves using water.bin (256x256 wrapping noise)
        let uv = vec2<f32>(world_x, world_y) * 0.005;
        
        let uv1 = uv * 0.5 + vec2<f32>(t * 0.02, t * 0.01);
        let wave1 = textureSampleLevel(water_texture, water_sampler, uv1, 0.0).r;
        
        let uv2 = uv * 1.5 + vec2<f32>(-t * 0.03, t * 0.02);
        let wave2 = textureSampleLevel(water_texture, water_sampler, uv2, 0.0).r;
        
        let uv3 = uv * 4.0 + vec2<f32>(t * 0.05, -t * 0.04);
        let wave3 = textureSampleLevel(water_texture, water_sampler, uv3, 0.0).r;
        
        let combined_waves = (wave1 + wave2 * 0.5 + wave3 * 0.25) / 1.75;
        
        let WAVE_HIGHLIGHT_COLOR = vec3<f32>(0.1, 0.2, 0.7);
        var final_color = mix(base_color, WAVE_HIGHLIGHT_COLOR, combined_waves * 0.5);
        
        // If coastal, mix in foam based on noise
        if border_mask > 0u {
            let foam_mix = smoothstep(0.4, 0.8, wave2 + wave3 * 0.5);
            final_color = mix(final_color, FOAM_COLOR, foam_mix * 0.8);
        }

        // Specular glint (sun reflection)
        let glint = pow(combined_waves, 8.0);
        final_color += glint * SPECULAR_COLOR * 1.5;
        
        return vec4<f32>(final_color, 1.0);
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
            // Simple approximation of sin using triangle wave to save ALU
            let r = abs(fract(hue) * 2.0 - 1.0);
            let g = abs(fract(hue + 0.333) * 2.0 - 1.0);
            let b = abs(fract(hue + 0.666) * 2.0 - 1.0);
            player_color = vec4<f32>(r, g, b, 1.0);
        } else if is_nation {
            // Mid-tone stable colors for nations
            let id = f32(owner_id);
            let r = fract(id * 0.123);
            let g = fract(id * 0.456);
            let b = fract(id * 0.789);
            player_color = vec4<f32>(0.3 + r * 0.5, 0.3 + g * 0.5, 0.3 + b * 0.5, 1.0);
        } else {
            // Soft, washed-out pastels for tribes
            let id = f32(owner_id);
            let r = fract(id * 0.123);
            let g = fract(id * 0.456);
            let b = fract(id * 0.789);
            player_color = vec4<f32>(0.5 + r * 0.3, 0.5 + g * 0.3, 0.5 + b * 0.3, 1.0);
        }
        
        if is_human {
            // Holographic Animated Cyber-Stripes (Additive Blending)
            let stripe = (sin((world_x + world_y) * 0.15 - globals.time * 2.5) + 1.0) * 0.5;
            let stripe_fx = mix(1.0, (0.6 + 0.6 * stripe), globals.effect_energy_flow);
            let holo_color = player_color.rgb * stripe_fx;
            base_color = vec4<f32>(terrain_color.rgb + holo_color * globals.visual_interior_alpha * 0.7, 1.0);
        } else if is_nation {
            // Static Additive Glow
            base_color = vec4<f32>(terrain_color.rgb + player_color.rgb * globals.visual_interior_alpha * 0.4, 1.0);
        } else {
            // Flat mix for tribes (traditional skin)
            base_color = mix(terrain_color, player_color, globals.visual_interior_alpha * 0.6);
        }
        
        // Conquest Shockwave Flash on interior
        if flash_val > 0.0 && globals.effect_shockwave_intensity > 0.0 {
            let shockwave = flash_val * globals.effect_shockwave_intensity;
            let flash_color = mix(vec3<f32>(1.0, 1.0, 1.0), player_color.rgb, 1.0 - flash_val);
            base_color = vec4<f32>(mix(base_color.rgb, flash_color, shockwave * 0.8), 1.0);
        }
    } else {
        base_color = terrain_color;
    }

    // Border and Shoreline Logic
    // In OpenFront, a border is drawn if the owner changes OR if a land tile borders water
    var should_draw_border = false;
    
    if border_mask != 0u {
        let center = hex_to_world(cell_x, cell_y);
        let local_pos = vec2<f32>(world_x, world_y) - center;
        
        // Dynamic border thickness: Breathe + Shockwave
        var thickness = globals.visual_border_thickness;
        if globals.effect_border_breathe > 0.0 {
            let breathe = (sin(globals.time * 3.0 + f32(owner_id)) + 1.0) * 0.5; // 0 to 1
            thickness += breathe * 0.05 * globals.effect_border_breathe;
        }
        if flash_val > 0.0 && globals.effect_shockwave_intensity > 0.0 {
            thickness += flash_val * 0.2 * globals.effect_shockwave_intensity;
        }
        
        let border_threshold = 0.5 - thickness;

        let is_odd = (cell_y % 2) != 0;
        var dir_0: vec2<f32>; var dir_1: vec2<f32>; var dir_2: vec2<f32>; 
        var dir_3: vec2<f32>; var dir_4: vec2<f32>; var dir_5: vec2<f32>;
        
        if is_odd {
            dir_0 = hex_to_world(cell_x + 1, cell_y) - center;
            dir_1 = hex_to_world(cell_x - 1, cell_y) - center;
            dir_2 = hex_to_world(cell_x, cell_y - 1) - center;
            dir_3 = hex_to_world(cell_x + 1, cell_y - 1) - center;
            dir_4 = hex_to_world(cell_x, cell_y + 1) - center;
            dir_5 = hex_to_world(cell_x + 1, cell_y + 1) - center;
        } else {
            dir_0 = hex_to_world(cell_x + 1, cell_y) - center;
            dir_1 = hex_to_world(cell_x - 1, cell_y) - center;
            dir_2 = hex_to_world(cell_x - 1, cell_y - 1) - center;
            dir_3 = hex_to_world(cell_x, cell_y - 1) - center;
            dir_4 = hex_to_world(cell_x - 1, cell_y + 1) - center;
            dir_5 = hex_to_world(cell_x, cell_y + 1) - center;
        }

        if (border_mask & 1u) != 0u && dot(local_pos, dir_0) > border_threshold { should_draw_border = true; }
        if (border_mask & 2u) != 0u && dot(local_pos, dir_1) > border_threshold { should_draw_border = true; }
        if (border_mask & 4u) != 0u && dot(local_pos, dir_2) > border_threshold { should_draw_border = true; }
        if (border_mask & 8u) != 0u && dot(local_pos, dir_3) > border_threshold { should_draw_border = true; }
        if (border_mask & 16u) != 0u && dot(local_pos, dir_4) > border_threshold { should_draw_border = true; }
        if (border_mask & 32u) != 0u && dot(local_pos, dir_5) > border_threshold { should_draw_border = true; }
    }

    if should_draw_border {
        var border_color: vec4<f32>;
        if has_player_color {
            if owner_id == globals.local_player_id {
                // Pulsating bright highlight for MY borders
                let pulse = (sin(globals.time * 6.0) + 1.0) * 0.5;
                let highlight = mix(player_color.rgb, vec3<f32>(1.0, 1.0, 1.0), pulse * 0.7);
                border_color = vec4<f32>(highlight, 1.0);
                base_color = border_color;
            } else {
                // Standard contrast border logic
                let luminance = dot(player_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
                if luminance > 0.6 {
                    border_color = vec4<f32>(player_color.rgb * 0.3, 1.0);
                } else {
                    border_color = min(player_color * 1.5 + vec4<f32>(0.2, 0.2, 0.2, 0.0), vec4<f32>(1.0));
                }
                
                // Add energy flow for borders if enabled
                if globals.effect_energy_flow > 0.0 {
                    let flow = (sin((world_x - world_y) * 2.0 - globals.time * 8.0) + 1.0) * 0.5;
                    border_color = vec4<f32>(border_color.rgb + player_color.rgb * flow * 0.6 * globals.effect_energy_flow, 1.0);
                }
                
                // Add shockwave flash to border color
                if flash_val > 0.0 && globals.effect_shockwave_intensity > 0.0 {
                    let flash_color = mix(border_color.rgb, vec3<f32>(1.0, 1.0, 1.0), flash_val * globals.effect_shockwave_intensity);
                    border_color = vec4<f32>(flash_color, 1.0);
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
