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

/// Unit directions from hex center toward each neighbor (odd/even row layout matches border tests).
fn hex_neighbor_dirs(cell_x: i32, cell_y: i32) -> array<vec2<f32>, 6> {
    var d: array<vec2<f32>, 6>;
    let center = hex_to_world(cell_x, cell_y);
    let is_odd = (cell_y % 2) != 0;
    if is_odd {
        d[0] = hex_to_world(cell_x + 1, cell_y) - center;
        d[1] = hex_to_world(cell_x - 1, cell_y) - center;
        d[2] = hex_to_world(cell_x, cell_y - 1) - center;
        d[3] = hex_to_world(cell_x + 1, cell_y - 1) - center;
        d[4] = hex_to_world(cell_x, cell_y + 1) - center;
        d[5] = hex_to_world(cell_x + 1, cell_y + 1) - center;
    } else {
        d[0] = hex_to_world(cell_x + 1, cell_y) - center;
        d[1] = hex_to_world(cell_x - 1, cell_y) - center;
        d[2] = hex_to_world(cell_x - 1, cell_y - 1) - center;
        d[3] = hex_to_world(cell_x, cell_y - 1) - center;
        d[4] = hex_to_world(cell_x - 1, cell_y + 1) - center;
        d[5] = hex_to_world(cell_x, cell_y + 1) - center;
    }
    return d;
}

/// How close this fragment is to any land-facing edge (0 = hex interior, 1 = at rim).
fn water_coastal_edge_weight(local_pos: vec2<f32>, border_mask: u32, dirs: array<vec2<f32>, 6>) -> f32 {
    var w = 0.0;
    if (border_mask & 1u) != 0u {
        w = max(w, smoothstep(0.05, 0.42, dot(local_pos, normalize(dirs[0]))));
    }
    if (border_mask & 2u) != 0u {
        w = max(w, smoothstep(0.05, 0.42, dot(local_pos, normalize(dirs[1]))));
    }
    if (border_mask & 4u) != 0u {
        w = max(w, smoothstep(0.05, 0.42, dot(local_pos, normalize(dirs[2]))));
    }
    if (border_mask & 8u) != 0u {
        w = max(w, smoothstep(0.05, 0.42, dot(local_pos, normalize(dirs[3]))));
    }
    if (border_mask & 16u) != 0u {
        w = max(w, smoothstep(0.05, 0.42, dot(local_pos, normalize(dirs[4]))));
    }
    if (border_mask & 32u) != 0u {
        w = max(w, smoothstep(0.05, 0.42, dot(local_pos, normalize(dirs[5]))));
    }
    return clamp(w, 0.0, 1.0);
}

/// Rich map-grade: keep chroma, deepen lows, avoid pastel mid-grey wash.
fn grade_paper_rgb(rgb: vec3<f32>, saturation: f32) -> vec3<f32> {
    let y = dot(rgb, vec3<f32>(0.299, 0.587, 0.114));
    let grey = vec3<f32>(y);
    var out = mix(grey, rgb, saturation);
    // Richer chroma: keep deep blacks in lows, avoid pastel mid-grey wash on highs.
    let lift = 0.58 + 0.52 * y;
    out = out * lift;
    return clamp(out, vec3<f32>(0.04), vec3<f32>(0.98));
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

        // Thinner visual shores: blend deep/coast by how enclosed the water is (land neighbor count),
        // and only emphasize cyan/foam near land-facing hex edges (not the whole water tile).
        var foam_coast_scale = 0.0;
        if border_mask > 0u {
            let n = countOneBits(border_mask);
            // Open water (few land neighbors) stays vivid; narrow channels / holes pull toward deep blue.
            let k_enclosure = clamp(0.24 + 0.76 / f32(n * n), 0.1, 1.0);
            let center_w = hex_to_world(cell_x, cell_y);
            let local_w = vec2<f32>(world_x, world_y) - center_w;
            let dirs_w = hex_neighbor_dirs(cell_x, cell_y);
            let edge_w = water_coastal_edge_weight(local_w, border_mask, dirs_w);
            let coast_mix = clamp(k_enclosure * edge_w, 0.0, 1.0);
            base_color = mix(DEEP_OCEAN_COLOR, COASTAL_COLOR, coast_mix);
            foam_coast_scale = 0.32 * clamp(1.0 - 0.13 * f32(n - 1u), 0.18, 1.0);
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
        var final_color = mix(base_color, WAVE_HIGHLIGHT_COLOR, combined_waves * 0.18);
        
        // If coastal, mix in foam based on noise (weaker in enclosed water).
        if border_mask > 0u {
            let foam_mix = smoothstep(0.4, 0.8, wave2 + wave3 * 0.5);
            final_color = mix(final_color, FOAM_COLOR, foam_mix * foam_coast_scale);
        }

        // Specular glint — keep very subtle (matte paper map; avoid shiny ocean).
        let glint = pow(combined_waves, 10.0);
        final_color += glint * SPECULAR_COLOR * 0.22;
        
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

        var raw_rgb = vec3<f32>(0.0);
        var paper_sat = 0.68;

        if is_human {
            // Hue from id (unchanged identity); saturation applied after via grade_paper_rgb.
            let hue = f32(owner_id) * 0.618033988749895;
            let r = abs(fract(hue) * 2.0 - 1.0);
            let g = abs(fract(hue + 0.333) * 2.0 - 1.0);
            let b = abs(fract(hue + 0.666) * 2.0 - 1.0);
            raw_rgb = vec3<f32>(r, g, b);
            paper_sat = 0.82;
        } else if is_nation {
            let id = f32(owner_id);
            let r = fract(id * 0.123);
            let g = fract(id * 0.456);
            let b = fract(id * 0.789);
            raw_rgb = vec3<f32>(0.1 + r * 0.78, 0.08 + g * 0.76, 0.12 + b * 0.8);
            paper_sat = 0.86;
        } else {
            let id = f32(owner_id);
            let r = fract(id * 0.123);
            let g = fract(id * 0.456);
            let b = fract(id * 0.789);
            raw_rgb = vec3<f32>(0.18 + r * 0.72, 0.12 + g * 0.76, 0.14 + b * 0.74);
            paper_sat = 0.9;
        }
        player_color = vec4<f32>(grade_paper_rgb(raw_rgb, paper_sat), 1.0);
        
        if is_human {
            // Stripe amplitude scales with effect_energy_flow (0 = calm matte interior).
            let stripe = (sin((world_x + world_y) * 0.15 - globals.time * 2.5) + 1.0) * 0.5;
            let stripe_fx = mix(1.0, (0.6 + 0.6 * stripe), globals.effect_energy_flow);
            let holo_color = player_color.rgb * stripe_fx;
            // ~0.9 effective tint at default `visual_interior_alpha` (see client_config `shader_interior_alpha`).
            let interior_mix = min(1.0, globals.visual_interior_alpha * 1.023);
            base_color = mix(terrain_color, vec4<f32>(holo_color, 1.0), interior_mix);
        } else if is_nation {
            let interior_mix = min(1.0, globals.visual_interior_alpha * 1.023);
            base_color = mix(terrain_color, player_color, interior_mix);
        } else {
            let interior_mix = min(1.0, globals.visual_interior_alpha * 1.023);
            base_color = mix(terrain_color, player_color, interior_mix);
        }
        
        // Conquest Shockwave Flash on interior (softer toward white when intensity low)
        if flash_val > 0.0 && globals.effect_shockwave_intensity > 0.0 {
            let shockwave = flash_val * globals.effect_shockwave_intensity;
            let flash_color = mix(vec3<f32>(1.0, 1.0, 1.0), player_color.rgb, 1.0 - flash_val);
            base_color = vec4<f32>(mix(base_color.rgb, flash_color, shockwave * 0.32), 1.0);
        }
    } else {
        base_color = terrain_color;
    }

    // Owned tiles: rim shadow + slow micro-gradient (depth without washing toward white).
    if has_player_color {
        let hc = hex_to_world(cell_x, cell_y);
        let lp = vec2<f32>(world_x, world_y) - hc;
        let rim = clamp(length(lp) * 0.36, 0.0, 1.0);
        let depth = mix(1.0, 0.64, rim * rim);
        let grain = sin(dot(lp, vec2<f32>(11.0, 7.3)) + f32(cell_x * 17 + cell_y * 3) + globals.time * 0.07) * 0.042;
        base_color = vec4<f32>(clamp(base_color.rgb * (depth + grain), vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
    }

    // Border: political owner change, or land vs open ocean (not inland lake — see map_compute.wgsl).
    var should_draw_border = false;
    
    if border_mask != 0u {
        let center = hex_to_world(cell_x, cell_y);
        let local_pos = vec2<f32>(world_x, world_y) - center;
        
        // Dynamic border thickness: Breathe + Shockwave
        var thickness = globals.visual_border_thickness;
        if globals.effect_border_breathe > 0.0 {
            let breathe = (sin(globals.time * 3.0 + f32(owner_id)) + 1.0) * 0.5; // 0 to 1
            thickness += breathe * 0.022 * globals.effect_border_breathe;
        }
        if flash_val > 0.0 && globals.effect_shockwave_intensity > 0.0 {
            thickness += flash_val * 0.065 * globals.effect_shockwave_intensity;
        }
        
        let border_threshold = 0.5 - thickness;

        let dirs_b = hex_neighbor_dirs(cell_x, cell_y);
        if (border_mask & 1u) != 0u && dot(local_pos, dirs_b[0]) > border_threshold { should_draw_border = true; }
        if (border_mask & 2u) != 0u && dot(local_pos, dirs_b[1]) > border_threshold { should_draw_border = true; }
        if (border_mask & 4u) != 0u && dot(local_pos, dirs_b[2]) > border_threshold { should_draw_border = true; }
        if (border_mask & 8u) != 0u && dot(local_pos, dirs_b[3]) > border_threshold { should_draw_border = true; }
        if (border_mask & 16u) != 0u && dot(local_pos, dirs_b[4]) > border_threshold { should_draw_border = true; }
        if (border_mask & 32u) != 0u && dot(local_pos, dirs_b[5]) > border_threshold { should_draw_border = true; }
    }

    if should_draw_border {
        var border_color: vec4<f32>;
        if has_player_color {
            if owner_id == globals.local_player_id {
                // Subtle edge emphasis (avoid bright pulsing "neon" border).
                let pulse = (sin(globals.time * 6.0) + 1.0) * 0.5;
                let highlight = mix(player_color.rgb, vec3<f32>(1.0, 1.0, 1.0), pulse * 0.12);
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
                    border_color = vec4<f32>(border_color.rgb + player_color.rgb * flow * 0.35 * globals.effect_energy_flow, 1.0);
                }
                
                // Add shockwave flash to border color
                if flash_val > 0.0 && globals.effect_shockwave_intensity > 0.0 {
                    let flash_color = mix(border_color.rgb, vec3<f32>(1.0, 1.0, 1.0), flash_val * globals.effect_shockwave_intensity * 0.38);
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
