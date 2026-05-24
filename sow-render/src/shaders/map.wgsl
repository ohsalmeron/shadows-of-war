struct Globals {
    camera_pos: vec2<f32>,
    zoom: f32,
    time: f32,
    screen_size: vec2<f32>,
    map_size: vec2<f32>,
    border_thickness: f32,
    border_darkness: f32,
    shore_thickness: f32,
    shore_darkness: f32,
    threat_slots: array<vec4<f32>, 4>,
    effect_shockwave: f32,
    effect_breathe: f32,
    effect_energy_flow: f32,
    _pad0: f32,
}

struct PlayerColors {
    colors: array<vec4<f32>, 256>,
}

var<uniform> globals: Globals;
var<uniform> player_colors: PlayerColors;
var terrain_texture: texture_2d<u32>;
var owner_texture: texture_2d<u32>;

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

fn get_cell_owner(hex: vec2<i32>) -> u32 {
    if (hex.x < 0 || hex.y < 0 || hex.x >= i32(globals.map_size.x) || hex.y >= i32(globals.map_size.y)) {
        return 0u;
    }
    return textureLoad(owner_texture, vec2<i32>(hex.x, hex.y), 0).x & 0xFFFFu;
}

fn get_cell_terrain(hex: vec2<i32>) -> u32 {
    if (hex.x < 0 || hex.y < 0 || hex.x >= i32(globals.map_size.x) || hex.y >= i32(globals.map_size.y)) {
        return 0u;
    }
    return textureLoad(terrain_texture, vec2<i32>(hex.x, hex.y), 0).x;
}

fn world_to_hex(world_pos: vec2<f32>) -> vec2<i32> {
    let q_f = world_pos.x - world_pos.y * 0.577350269;
    let r_f = world_pos.y * 1.154700538;
    let s_f = -q_f - r_f;

    var rq = round(q_f);
    var rr = round(r_f);
    let rs = round(s_f);

    let q_diff = abs(rq - q_f);
    let r_diff = abs(rr - r_f);
    let s_diff = abs(rs - s_f);

    if q_diff > r_diff && q_diff > s_diff {
        rq = -rr - rs;
    } else if r_diff > s_diff {
        rr = -rq - rs;
    }

    let col = i32(rq) + (i32(rr) - (i32(rr) & 1)) / 2;
    let row = i32(rr);
    return vec2<i32>(col, row);
}

fn hex_to_world(hex: vec2<i32>) -> vec2<f32> {
    let col = f32(hex.x);
    let row = f32(hex.y);
    let bx = col + 0.5 + f32(hex.y & 1) * 0.5;
    let by = (row + 0.5) * 0.8660254;
    return vec2<f32>(bx, by);
}

fn get_hex_neighbor(hex: vec2<i32>, direction: i32) -> vec2<i32> {
    let is_odd = (hex.y % 2) != 0;
    var offset = vec2<i32>(0, 0);
    if (direction == 0) {
        offset = vec2<i32>(1, 0); // East
    } else if (direction == 1) {
        offset = vec2<i32>(-1, 0); // West
    } else if (direction == 2) {
        if (is_odd) { offset = vec2<i32>(0, -1); } else { offset = vec2<i32>(-1, -1); } // Northwest
    } else if (direction == 3) {
        if (is_odd) { offset = vec2<i32>(1, -1); } else { offset = vec2<i32>(0, -1); } // Northeast
    } else if (direction == 4) {
        if (is_odd) { offset = vec2<i32>(0, 1); } else { offset = vec2<i32>(-1, 1); } // Southwest
    } else if (direction == 5) {
        if (is_odd) { offset = vec2<i32>(1, 1); } else { offset = vec2<i32>(0, 1); } // Southeast
    }
    return hex + offset;
}

fn owner_albedo(owner_id: u32) -> vec3<f32> {
    if owner_id < 256u {
        return player_colors.colors[owner_id].rgb;
    }
    return vec3<f32>(0.5, 0.5, 0.5); // Fallback if out of bounds
}

fn get_elevation(cx: i32, cy: i32) -> f32 {
    if (cx < 0 || cy < 0 || cx >= i32(globals.map_size.x) || cy >= i32(globals.map_size.y)) {
        return 0.0;
    }
    let terrain_byte = textureLoad(terrain_texture, vec2<i32>(cx, cy), 0).x;
    let is_land = (terrain_byte & 0x80u) != 0u;
    if (is_land) {
        return f32(terrain_byte & 0x1Fu);
    }
    return 0.0;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let screen_pixel = in.uv * globals.screen_size;
    let world_x = (screen_pixel.x - globals.camera_pos.x) / globals.zoom;
    let world_y = (screen_pixel.y - globals.camera_pos.y) / globals.zoom;

    let cell_hex = world_to_hex(vec2<f32>(world_x, world_y));
    let cell_x = cell_hex.x;
    let cell_y = cell_hex.y;

    if cell_x < 0 || cell_y < 0 || cell_x >= i32(globals.map_size.x) || cell_y >= i32(globals.map_size.y) {
        return vec4<f32>(0.015, 0.015, 0.02, 1.0); // Sleek dark space/canvas backdrop
    }

    let pixel_coords = vec2<i32>(cell_x, cell_y);
    let terrain_byte = textureLoad(terrain_texture, pixel_coords, 0).x;
    let owner_packed = textureLoad(owner_texture, pixel_coords, 0).x;
    let owner_id = owner_packed & 0xFFFFu;
    let flash_byte = (owner_packed >> 16u) & 0xFFu;
    let flash_val = f32(flash_byte) / 255.0;
    let is_land = (terrain_byte & 0x80u) != 0u;

    var terrain_color = vec4<f32>(0.0);
    var normal = vec3<f32>(0.0, 0.0, 1.0);
    var is_specular = false;
    
    if is_land {
        let is_shoreline = (terrain_byte & 0x40u) != 0u;
        let mag_center = f32(terrain_byte & 0x1Fu);
        
        let px = floor(world_x * 8.0);
        let py = floor(world_y * 8.0);
        let land_noise = fract(sin(px * 12.9898 + py * 78.233) * 43758.5453);
        let noise_offset = (land_noise - 0.5) * 0.03; // Gentle ±1.5% color variation

        if is_shoreline {
            let base = vec3<f32>(204.0 / 255.0, 203.0 / 255.0, 158.0 / 255.0);
            terrain_color = vec4<f32>(base + noise_offset * 0.5, 1.0); // Shore
        } else if mag_center < 10.0 {
            let r = 190.0 / 255.0;
            let g = (220.0 - 2.0 * mag_center) / 255.0;
            let b = 138.0 / 255.0;
            terrain_color = vec4<f32>(vec3<f32>(r, g, b) + noise_offset, 1.0); // Plains
        } else if mag_center < 20.0 {
            let r = (200.0 + 2.0 * mag_center) / 255.0;
            let g = (183.0 + 2.0 * mag_center) / 255.0;
            let b = (138.0 + 2.0 * mag_center) / 255.0;
            terrain_color = vec4<f32>(vec3<f32>(r, g, b) + noise_offset * 1.2, 1.0); // Highlands
        } else {
            // Smooth blend/fusion from high Highland color to snowy white peak
            let highland_base = vec3<f32>(240.0 / 255.0, 223.0 / 255.0, 178.0 / 255.0);
            let snowy_peak = vec3<f32>(248.0 / 255.0, 248.0 / 255.0, 248.0 / 255.0);
            let blend = clamp((mag_center - 20.0) / 11.0, 0.0, 1.0);
            let peak_color = mix(highland_base, snowy_peak, blend);
            terrain_color = vec4<f32>(peak_color + noise_offset * 0.8, 1.0); // Mountains
        }

        // Procedural 6-Directional Normal Mapping from Adjacent elevation gradient
        let is_odd = (cell_y % 2) != 0;
        let h_right = get_elevation(cell_x + 1, cell_y);
        let h_left  = get_elevation(cell_x - 1, cell_y);
        var h_up_l = 0.0;
        var h_up_r = 0.0;
        var h_dn_l = 0.0;
        var h_dn_r = 0.0;
        if is_odd {
            h_up_l = get_elevation(cell_x, cell_y - 1);
            h_up_r = get_elevation(cell_x + 1, cell_y - 1);
            h_dn_l = get_elevation(cell_x, cell_y + 1);
            h_dn_r = get_elevation(cell_x + 1, cell_y + 1);
        } else {
            h_up_l = get_elevation(cell_x - 1, cell_y - 1);
            h_up_r = get_elevation(cell_x, cell_y - 1);
            h_dn_l = get_elevation(cell_x - 1, cell_y + 1);
            h_dn_r = get_elevation(cell_x, cell_y + 1);
        }
        
        let dx = ((h_right + 0.5 * h_up_r + 0.5 * h_dn_r) - (h_left + 0.5 * h_up_l + 0.5 * h_dn_l)) * 0.10;
        let dy = ((0.866 * h_dn_l + 0.866 * h_dn_r) - (0.866 * h_up_l + 0.866 * h_up_r)) * 0.10;
        normal = normalize(vec3<f32>(-dx, -dy, 1.0));
    } else {
        let is_ocean_water = (terrain_byte & 0x20u) != 0u;
        
        let px = world_x * 8.0;
        let py = world_y * 8.0;
        
        // Procedural stable seed per tile for unique regional wave properties
        let tile_seed = fract(sin(f32(cell_x) * 12.9898 + f32(cell_y) * 78.233) * 43758.5453);
        let wave_speed = 0.8 + tile_seed * 1.4;
        let wave_phase = tile_seed * 6.28318;
        let freq_x = 0.12 + tile_seed * 0.06;
        let freq_y = 0.06 + (1.0 - tile_seed) * 0.06;

        let t = globals.time * wave_speed + wave_phase;
        let wave = sin(px * freq_x + py * freq_y + t) + cos(py * freq_x - px * freq_y + t * 0.7);

        // Animated sparkling/glittering sparkles (1.2% chance per pixel, changes 4 times/sec)
        let sparkle_t = floor(globals.time * 4.0);
        let sparkle_hash = fract(sin(px * 12.9898 + py * 78.233 + sparkle_t) * 43758.5453);
        let has_sparkle = sparkle_hash > 0.988;

        var color_deep = vec3<f32>(70.0 / 255.0, 132.0 / 255.0, 180.0 / 255.0); // Ocean
        var color_mid  = vec3<f32>(85.0 / 255.0, 143.0 / 255.0, 215.0 / 255.0);
        var color_foam = vec3<f32>(100.0 / 255.0, 143.0 / 255.0, 255.0 / 255.0); // Shoreline water
        
        if !is_ocean_water {
            // River/Lake uses a fresh, teal-tinted pastel blue
            color_deep = vec3<f32>(60.0 / 255.0, 140.0 / 255.0, 175.0 / 255.0);
            color_mid  = vec3<f32>(75.0 / 255.0, 155.0 / 255.0, 195.0 / 255.0);
            color_foam = vec3<f32>(95.0 / 255.0, 175.0 / 255.0, 220.0 / 255.0);
        }

        var final_water_color = color_deep;
        if has_sparkle {
            final_water_color = color_foam;
        } else if wave > 1.2 {
            final_water_color = color_foam;
        } else if wave > 0.4 {
            final_water_color = color_mid;
        }

        terrain_color = vec4<f32>(final_water_color, 1.0);

        // Water Wave Normal Shading
        let wave_dx = cos(px * freq_x + py * freq_y + t) * freq_x - sin(py * freq_x - px * freq_y + t * 0.7) * freq_y;
        let wave_dy = cos(px * freq_x + py * freq_y + t) * freq_y + sin(py * freq_x - px * freq_y + t * 0.7) * freq_x;
        normal = normalize(vec3<f32>(-wave_dx * 0.8, -wave_dy * 0.8, 1.0));
        is_specular = true;
    }

    // Convert sRGB palette input to linear space
    terrain_color = vec4<f32>(pow(terrain_color.rgb, vec3<f32>(2.2)), terrain_color.a);

    var base_color = terrain_color.rgb;
    if owner_id > 0u {
        let albedo = owner_albedo(owner_id);

        // Energy flow: animated diagonal stripes on interior
        var interior_mod = 1.0;
        if globals.effect_energy_flow > 0.0 {
            let stripe = (sin((world_x + world_y) * 0.15 - globals.time * 2.5) + 1.0) * 0.5;
            interior_mod = mix(1.0, 0.6 + 0.6 * stripe, globals.effect_energy_flow);
        }

        base_color = mix(terrain_color.rgb, albedo * interior_mod, 0.75);

        // Conquest shockwave flash on interior
        if flash_val > 0.0 && globals.effect_shockwave > 0.0 {
            let shockwave = flash_val * globals.effect_shockwave;
            let flash_color = mix(vec3<f32>(1.0, 1.0, 1.0), albedo, 1.0 - flash_val);
            base_color = mix(base_color, flash_color, shockwave * 0.8);
        }
    }
    let hex_center = hex_to_world(cell_hex);
    let local_pos = vec2<f32>(world_x, world_y) - hex_center;

    var is_shore = false;
    var is_border = false;
    var is_green_border = false;

    var thickness = globals.border_thickness;
    let border_darkness = globals.border_darkness;
    let s_thickness = globals.shore_thickness;
    let s_darkness = globals.shore_darkness;

    // Border breathe: subtle thickness pulse per owner
    if globals.effect_breathe > 0.0 && owner_id > 0u {
        let breathe = (sin(globals.time * 3.0 + f32(owner_id)) + 1.0) * 0.5;
        thickness += breathe * 0.05 * globals.effect_breathe;
    }
    // Shockwave border explosion
    if flash_val > 0.0 && globals.effect_shockwave > 0.0 {
        thickness += flash_val * 0.2 * globals.effect_shockwave;
    }

    if is_land {
        let is_tribe = owner_id >= 200u;

        for (var i = 0; i < 6; i = i + 1) {
            let neighbor_hex = get_hex_neighbor(cell_hex, i);
            let neighbor_terrain = get_cell_terrain(neighbor_hex);
            let neighbor_owner = get_cell_owner(neighbor_hex);
            let neighbor_is_land = (neighbor_terrain & 0x80u) != 0u;

            var dir = vec2<f32>(0.0, 0.0);
            if (i == 0) { dir = vec2<f32>(1.0, 0.0); }
            else if (i == 1) { dir = vec2<f32>(-1.0, 0.0); }
            else if (i == 2) { dir = vec2<f32>(-0.5, -0.8660254); }
            else if (i == 3) { dir = vec2<f32>(0.5, -0.8660254); }
            else if (i == 4) { dir = vec2<f32>(-0.5, 0.8660254); }
            else if (i == 5) { dir = vec2<f32>(0.5, 0.8660254); }

            let dist_to_edge = 0.5 - dot(local_pos, dir);

            if !neighbor_is_land {
                if dist_to_edge < s_thickness {
                    is_shore = true;
                }
                if owner_id > 0u {
                    if dist_to_edge < thickness {
                        is_border = true;
                    }
                }
            } else {
                if owner_id > 0u && neighbor_owner != owner_id {
                    if dist_to_edge < thickness {
                        is_border = true;
                        let green_exists = is_tribe && (neighbor_owner >= 200u);
                        if green_exists {
                            is_green_border = true;
                        }
                    }
                }
            }
        }

        if is_shore {
            if owner_id > 0u {
                let border_albedo = owner_albedo(owner_id) * border_darkness;
                base_color = mix(border_albedo, vec3<f32>(0.015, 0.012, 0.010), 0.50);
            } else {
                base_color = vec3<f32>(0.025, 0.020, 0.015);
            }
        } else if is_border {
            if is_green_border {
                base_color = vec3<f32>(0.2, 0.8, 0.2) * border_darkness;
            } else {
                var border_albedo = owner_albedo(owner_id) * border_darkness;

                // Energy flow on borders: faster, tighter wave
                if globals.effect_energy_flow > 0.0 {
                    let flow = (sin((world_x - world_y) * 2.0 - globals.time * 8.0) + 1.0) * 0.5;
                    border_albedo += owner_albedo(owner_id) * flow * 0.6 * globals.effect_energy_flow;
                }

                // Shockwave flash on border
                if flash_val > 0.0 && globals.effect_shockwave > 0.0 {
                    border_albedo = mix(border_albedo, vec3<f32>(1.0, 1.0, 1.0), flash_val * globals.effect_shockwave);
                }

                base_color = border_albedo;
            }
        }
    }

    // ── WAR FOG: Attack Threat Visualization ──
    // Multi-layered: desaturation → smoke gradient → ripple waves → corona front
    if owner_id > 0u {
        let world_pos_hex = hex_to_world(cell_hex);
        for (var ti = 0; ti < 4; ti = ti + 1) {
            let slot = globals.threat_slots[ti];
            let radius = slot.z;
            if radius <= 0.0 { continue; }
            if u32(slot.w) != owner_id { continue; }
            let front_world = vec2<f32>(
                slot.x + 0.5 + f32(i32(slot.y) & 1) * 0.5,
                (slot.y + 0.5) * 0.8660254
            );
            let dist = distance(world_pos_hex, front_world);
            let threat = 1.0 - smoothstep(0.0, radius, dist);
            if threat <= 0.0 { continue; }

            // Layer 1: Desaturation — threatened territory looks drained/dying
            let lum = dot(base_color, vec3<f32>(0.299, 0.587, 0.114));
            let desat = mix(base_color, vec3<f32>(lum), threat * 0.6);

            // Layer 2: Smoke gradient — hot ember core fading to dark ash at edge
            let ember = vec3<f32>(0.95, 0.15, 0.05);   // Bright red-orange core
            let ash   = vec3<f32>(0.12, 0.04, 0.02);    // Dark smoke edge
            let smoke_color = mix(ash, ember, threat * threat); // Quadratic falloff = sharp core
            let smoke_blend = threat * 0.55;

            // Layer 3: Ripple waves — directional pulses radiating from attack front
            let wave_dir = normalize(world_pos_hex - front_world + vec2<f32>(0.001));
            let wave_phase = dist * 3.0 - globals.time * 4.0;
            let ripple = (sin(wave_phase) + 1.0) * 0.5;
            let ripple_intensity = ripple * threat * threat * 0.25;

            // Layer 4: Corona — bright hot line at the attack front edge
            let corona_dist = abs(dist - radius * 0.15); // Ring near the front
            let corona = smoothstep(1.5, 0.0, corona_dist) * 0.7;
            let corona_color = vec3<f32>(1.0, 0.4, 0.1); // Hot orange-white

            // Composite all layers
            var war_color = mix(desat, smoke_color, smoke_blend);
            war_color += corona_color * corona;
            war_color += ember * ripple_intensity;

            base_color = war_color;
        }
    }

    // ── Upgraded Lighting (Sun directional diffuse + Specular highlights) ──
    let light_dir = normalize(vec3<f32>(-1.0, -1.0, 1.6)); // Directional sun light from top-left
    let diffuse = max(0.68, dot(normal, light_dir));
    base_color = base_color * (diffuse * 1.12);

    if is_specular {
        let view_dir = vec3<f32>(0.0, 0.0, 1.0);
        let half_dir = normalize(light_dir + view_dir);
        let spec = pow(max(0.0, dot(normal, half_dir)), 96.0);
        base_color = base_color + vec3<f32>(0.15 * spec);
    }

    // ── Embossed Cell Vignette (Real board game physical tile borders) ──
    var min_dist_to_edge = 0.5;
    for (var i = 0; i < 6; i = i + 1) {
        var dir = vec2<f32>(0.0, 0.0);
        if (i == 0) { dir = vec2<f32>(1.0, 0.0); }
        else if (i == 1) { dir = vec2<f32>(-1.0, 0.0); }
        else if (i == 2) { dir = vec2<f32>(-0.5, -0.8660254); }
        else if (i == 3) { dir = vec2<f32>(0.5, -0.8660254); }
        else if (i == 4) { dir = vec2<f32>(-0.5, 0.8660254); }
        else if (i == 5) { dir = vec2<f32>(0.5, 0.8660254); }

        let dist_to_edge = 0.5 - dot(local_pos, dir);
        min_dist_to_edge = min(min_dist_to_edge, dist_to_edge);
    }
    let cell_bevel = smoothstep(0.0, 0.06, min_dist_to_edge);
    base_color = base_color * (0.86 + 0.14 * cell_bevel); // Emphasizes 3D depth of individual tiles

    // ── Tactile Canvas/Matte Paper Texture Overlay ──
    let px_screen = in.uv.x * 2400.0;
    let py_screen = in.uv.y * 2400.0;
    let paper_noise = fract(sin(px_screen * 12.9898 + py_screen * 78.233) * 43758.5453);
    let paper_grain = 0.95 + 0.05 * paper_noise;
    base_color = base_color * paper_grain;

    // ── Screen Vignetting (Soft shading at screen edges) ──
    let d_center = length(in.uv - 0.5);
    let vignette = smoothstep(0.8, 0.45, d_center);
    base_color = base_color * (0.82 + 0.18 * vignette);

    // Convert from linear to sRGB for final output
    let final_color = pow(base_color, vec3<f32>(1.0 / 2.2));
    return vec4<f32>(final_color, 1.0);
}