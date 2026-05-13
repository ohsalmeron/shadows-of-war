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
    uniform_reserved: f32,
    // Keeps host `MapGlobals` size aligned with uniform struct size (multiple of 8); do not drop.
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

/// Neighbor texel offset for hex bit index (must match `map_compute.wgsl` / `GameMap::for_each_neighbor`).
fn water_tex_neighbor_delta(bit: u32, odd_row: bool) -> vec2<i32> {
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

/// Bits toward hex neighbors that are **land** (texture probe; fixes rivers/lakes where bake omits coast bits).
fn land_neighbor_mask_from_tex(px: vec2<i32>, odd_row: bool) -> u32 {
    var m = 0u;
    let mw = i32(globals.map_size.x);
    let mh = i32(globals.map_size.y);
    for (var b = 0u; b < 6u; b++) {
        let d = water_tex_neighbor_delta(b, odd_row);
        let nx = px.x + d.x;
        let ny = px.y + d.y;
        if nx < 0 || ny < 0 || nx >= mw || ny >= mh {
            continue;
        }
        let nv = textureLoad(territory_texture, vec2<i32>(nx, ny), 0).x;
        let ntb = (nv >> 16u) & 0xFFu;
        if (ntb & 0x80u) != 0u {
            m |= 1u << b;
        }
    }
    return m;
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

/// Dark, saturated faction ink — no lift toward white / grey fog.
fn grade_paper_rgb(rgb: vec3<f32>, saturation: f32) -> vec3<f32> {
    let y = dot(rgb, vec3<f32>(0.299, 0.587, 0.114));
    let grey = vec3<f32>(y);
    var out = mix(grey, rgb, saturation);
    out = out * (0.34 + 0.38 * (1.0 - y));
    return clamp(out, vec3<f32>(0.02), vec3<f32>(0.48));
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
        // Dynamic Biome Colors (darker bases — was reading mint-washed on screen).
        if mag_center < 10.0 {
            terrain_color = vec4<f32>(0.095, 0.165, 0.072, 1.0); // Lush Plains
        } else if mag_center < 20.0 {
            terrain_color = vec4<f32>(0.22, 0.175, 0.11, 1.0); // Earthy Highlands
        } else {
            let snow = clamp((mag_center - 20.0) / 11.0, 0.0, 1.0);
            terrain_color = mix(vec4<f32>(0.22, 0.22, 0.24, 1.0), vec4<f32>(0.52, 0.54, 0.56, 1.0), snow); // Snowy Mountains
        }
    } else {
        let t = globals.time;
        // Bit 5 `is_ocean` on `MapTile`: ocean vs inland lake/river (see sow-core `map.rs`).
        let is_ocean_water = (terrain_byte & 0x20u) != 0u;
        let odd_w = (cell_y % 2) != 0;
        let land_tex_mask = land_neighbor_mask_from_tex(pixel_coords, odd_w);
        let land_edge_mask = border_mask | land_tex_mask;

        let center_w = hex_to_world(cell_x, cell_y);
        let local_w = vec2<f32>(world_x, world_y) - center_w;
        let dirs_w = hex_neighbor_dirs(cell_x, cell_y);

        let DEEP_OCEAN_COLOR = vec3<f32>(0.01, 0.08, 0.23);
        let COASTAL_COLOR = vec3<f32>(0.1, 0.5, 0.6);
        let FOAM_COLOR = vec3<f32>(0.9, 0.95, 1.0);
        let SPECULAR_COLOR = vec3<f32>(1.0, 0.95, 0.8);
        let WAVE_HIGHLIGHT_COLOR = vec3<f32>(0.1, 0.2, 0.7);

        let uv = vec2<f32>(world_x, world_y) * 0.005;
        let uv1 = uv * 0.5 + vec2<f32>(t * 0.02, t * 0.01);
        let wave1 = textureSampleLevel(water_texture, water_sampler, uv1, 0.0).r;
        let uv2 = uv * 1.5 + vec2<f32>(-t * 0.03, t * 0.02);
        let wave2 = textureSampleLevel(water_texture, water_sampler, uv2, 0.0).r;
        let uv3 = uv * 4.0 + vec2<f32>(t * 0.05, -t * 0.04);
        let wave3 = textureSampleLevel(water_texture, water_sampler, uv3, 0.0).r;
        let combined_waves = (wave1 + wave2 * 0.5 + wave3 * 0.25) / 1.75;

        var edge_w = 0.0;
        if land_edge_mask > 0u {
            edge_w = water_coastal_edge_weight(local_w, land_edge_mask, dirs_w);
        }

        // Rivers / lakes: plain readable blue + black inner stroke (bake often skips coast bits for non-ocean).
        if !is_ocean_water {
            let RIVER = vec3<f32>(0.075, 0.26, 0.42);
            var fc = RIVER;
            fc = mix(fc, WAVE_HIGHLIGHT_COLOR, combined_waves * 0.055);
            if land_edge_mask > 0u {
                let inner = smoothstep(0.12, 0.30, edge_w) * (1.0 - smoothstep(0.42, 0.78, edge_w));
                fc = mix(fc, vec3<f32>(0.0, 0.0, 0.02), inner * 0.94);
                let bank = smoothstep(0.52, 0.74, edge_w);
                fc = mix(fc, vec3<f32>(0.02, 0.03, 0.06), bank * 0.62);
            }
            return vec4<f32>(fc, 1.0);
        }

        // Ocean: flatter coast than before; land adjacency from texture + bake.
        var base_color = DEEP_OCEAN_COLOR;
        var foam_coast_scale = 0.0;
        if land_edge_mask > 0u {
            let n = (land_edge_mask & 1u) + ((land_edge_mask >> 1u) & 1u) + ((land_edge_mask >> 2u) & 1u) + ((land_edge_mask >> 3u) & 1u) + ((land_edge_mask >> 4u) & 1u) + ((land_edge_mask >> 5u) & 1u);
            let k_enclosure = clamp(0.22 + 0.72 / f32(n * n), 0.08, 0.88);
            let coast_mix = clamp(k_enclosure * edge_w * 0.82, 0.0, 1.0);
            base_color = mix(DEEP_OCEAN_COLOR, COASTAL_COLOR, coast_mix);
            foam_coast_scale = 0.18 * clamp(1.0 - 0.14 * f32(n - 1u), 0.12, 1.0);
        }

        var final_color = mix(base_color, WAVE_HIGHLIGHT_COLOR, combined_waves * 0.14);
        if land_edge_mask > 0u {
            let foam_mix = smoothstep(0.4, 0.8, wave2 + wave3 * 0.5);
            final_color = mix(final_color, FOAM_COLOR, foam_mix * foam_coast_scale);
            let inner_o = smoothstep(0.20, 0.34, edge_w) * (1.0 - smoothstep(0.50, 0.75, edge_w));
            final_color = mix(final_color, vec3<f32>(0.01, 0.015, 0.03), inner_o * 0.38);
        }
        let glint = pow(combined_waves, 10.0);
        final_color += glint * SPECULAR_COLOR * 0.14;
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
            paper_sat = 0.78;
        } else if is_nation {
            let id = f32(owner_id);
            let r = fract(id * 0.123);
            let g = fract(id * 0.456);
            let b = fract(id * 0.789);
            raw_rgb = vec3<f32>(0.03 + r * 0.36, 0.025 + g * 0.34, 0.03 + b * 0.38);
            paper_sat = 0.84;
        } else {
            let id = f32(owner_id);
            let r = fract(id * 0.123);
            let g = fract(id * 0.456);
            let b = fract(id * 0.789);
            raw_rgb = vec3<f32>(0.04 + r * 0.34, 0.03 + g * 0.36, 0.035 + b * 0.34);
            paper_sat = 0.86;
        }
        player_color = vec4<f32>(grade_paper_rgb(raw_rgb, paper_sat), 1.0);

        // Transparency = let terrain dominate; tint is a dark veil (not chalky full replacement).
        let interior_mix = min(0.52, globals.visual_interior_alpha * 0.72);
        base_color = mix(terrain_color, player_color, interior_mix);
        
        // Conquest flash: nudge tint, do not blow out toward white.
        if flash_val > 0.0 && globals.effect_shockwave_intensity > 0.0 {
            let shockwave = flash_val * globals.effect_shockwave_intensity;
            let flash_color = mix(player_color.rgb * 0.92, player_color.rgb * 1.04, 1.0 - flash_val);
            base_color = vec4<f32>(mix(base_color.rgb, flash_color, shockwave * 0.12), 1.0);
        }
    } else {
        base_color = terrain_color;
    }

    // `border_mask` is political-only on claimed tiles; neutral keeps ocean coast bits (see map_compute).
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
                // Thin edge read — do not replace the whole tile (that reads as “inner border paint”).
                let pulse = (sin(globals.time * 6.0) + 1.0) * 0.5;
                let highlight = mix(player_color.rgb, player_color.rgb * 1.08, pulse * 0.08);
                border_color = vec4<f32>(highlight, 1.0);
                base_color = vec4<f32>(mix(base_color.rgb, border_color.rgb, 0.28), base_color.a);
            } else {
                // Dark outline: avoid light grey/halation on the fill.
                let luminance = dot(player_color.rgb, vec3<f32>(0.299, 0.587, 0.114));
                if luminance > 0.45 {
                    border_color = vec4<f32>(player_color.rgb * 0.22, 1.0);
                } else {
                    border_color = vec4<f32>(clamp(player_color.rgb * 0.62 + vec3<f32>(0.02, 0.02, 0.03), vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
                }
                
                if globals.effect_energy_flow > 0.0 {
                    let flow = (sin((world_x - world_y) * 2.0 - globals.time * 8.0) + 1.0) * 0.5;
                    border_color = vec4<f32>(border_color.rgb + player_color.rgb * flow * 0.18 * globals.effect_energy_flow, 1.0);
                }
                
                if flash_val > 0.0 && globals.effect_shockwave_intensity > 0.0 {
                    let flash_color = mix(border_color.rgb, player_color.rgb * 1.05, flash_val * globals.effect_shockwave_intensity * 0.25);
                    border_color = vec4<f32>(flash_color, 1.0);
                }
                
                base_color = mix(base_color, border_color, globals.visual_border_alpha);
            }
        } else {
            // Wilderness border (shoreline or adjacent to player)
            border_color = vec4<f32>(terrain_color.rgb * 0.32, 1.0);
            base_color = mix(base_color, border_color, globals.visual_border_alpha * 0.55);
        }
    }

    // Opaque land: “transparency” is rgb mix with terrain above, not alpha×black clear.
    return vec4<f32>(base_color.rgb, 1.0);
}
