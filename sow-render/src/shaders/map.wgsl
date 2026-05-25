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
    threat_slots: array<vec4<f32>, 8>,
    effect_shockwave: f32,
    effect_breathe: f32,
    effect_energy_flow: f32,
    my_player_id: f32,
    hover_hex: vec2<f32>,
    hover_building_kind: f32,
    _pad1: f32,
    nobuild_slots: array<vec4<f32>, 32>,
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

fn hex_distance(a: vec2<i32>, b: vec2<i32>) -> i32 {
    let q1 = a.x - (a.y - (a.y & 1)) / 2;
    let r1 = a.y;
    let q2 = b.x - (b.y - (b.y & 1)) / 2;
    let r2 = b.y;

    let dq = q2 - q1;
    let dr = r2 - r1;
    return (abs(dq) + abs(dr) + abs(dq + dr)) / 2;
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
    return vec3<f32>(0.5, 0.5, 0.5);
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

// ── FBM Noise (4-octave layered noise for organic terrain variation) ──
fn hash2d(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn value_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f); // smoothstep interpolation
    let a = hash2d(i);
    let b = hash2d(i + vec2<f32>(1.0, 0.0));
    let c = hash2d(i + vec2<f32>(0.0, 1.0));
    let d = hash2d(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var val = 0.0;
    var amp = 0.5;
    var pos = p;
    for (var i = 0; i < 4; i = i + 1) {
        val += amp * value_noise(pos);
        pos = pos * 2.17 + vec2<f32>(1.7, 4.6);
        amp *= 0.5;
    }
    return val;
}

// ── Hex direction helper ──
fn hex_dir(i: i32) -> vec2<f32> {
    if (i == 0) { return vec2<f32>(1.0, 0.0); }
    if (i == 1) { return vec2<f32>(-1.0, 0.0); }
    if (i == 2) { return vec2<f32>(-0.5, -0.8660254); }
    if (i == 3) { return vec2<f32>(0.5, -0.8660254); }
    if (i == 4) { return vec2<f32>(-0.5, 0.8660254); }
    return vec2<f32>(0.5, 0.8660254);
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
        return vec4<f32>(0.015, 0.015, 0.02, 1.0);
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

    // FBM noise sampled at world coords for organic sub-tile variation
    let world_p = vec2<f32>(world_x, world_y);
    let n1 = fbm(world_p * 6.0);
    let n2 = fbm(world_p * 12.0 + vec2<f32>(50.0, 80.0));
    let noise_detail = (n1 - 0.5) * 0.08 + (n2 - 0.5) * 0.03;

    if is_land {
        let is_shoreline = (terrain_byte & 0x40u) != 0u;
        let mag_center = f32(terrain_byte & 0x1Fu);

        if is_shoreline {
            // Warm sandy shore with slight color variation
            let base_sand = vec3<f32>(0.82, 0.78, 0.60);
            let wet_sand = vec3<f32>(0.72, 0.68, 0.50);
            let shore_blend = fbm(world_p * 10.0 + vec2<f32>(30.0, 70.0));
            let shore_col = mix(base_sand, wet_sand, shore_blend);
            terrain_color = vec4<f32>(shore_col + noise_detail * 0.6, 1.0);
        } else if mag_center < 10.0 {
            // Plains — lush greens with subtle biome tinting
            let grass_a = vec3<f32>(0.52, 0.72, 0.38); // Fresh green
            let grass_b = vec3<f32>(0.60, 0.76, 0.42); // Lighter meadow
            let grass_c = vec3<f32>(0.45, 0.62, 0.32); // Deeper forest-edge green
            let blend_t = fbm(world_p * 4.0 + vec2<f32>(100.0, 200.0));
            var grass = mix(grass_a, grass_b, blend_t);
            grass = mix(grass, grass_c, smoothstep(0.55, 0.75, n1));
            // Darken slightly with elevation within plains range
            grass = grass * (1.0 - mag_center * 0.012);
            terrain_color = vec4<f32>(grass + noise_detail, 1.0);
        } else if mag_center < 20.0 {
            // Highlands — earthy clay/ochre tones blending from green
            let highland_green = vec3<f32>(0.50, 0.58, 0.35);
            let highland_clay  = vec3<f32>(0.62, 0.52, 0.38);
            let highland_rock  = vec3<f32>(0.55, 0.48, 0.40);
            let elev_t = (mag_center - 10.0) / 10.0;
            var highland = mix(highland_green, highland_clay, elev_t);
            highland = mix(highland, highland_rock, smoothstep(0.4, 0.8, n1));
            terrain_color = vec4<f32>(highland + noise_detail * 1.2, 1.0);
        } else {
            // Mountains — rocky grey to snowy peaks
            let rock_base = vec3<f32>(0.52, 0.50, 0.48);
            let rock_light = vec3<f32>(0.62, 0.60, 0.58);
            let snowy_peak = vec3<f32>(0.92, 0.93, 0.95);
            let elev_t = clamp((mag_center - 20.0) / 11.0, 0.0, 1.0);
            var mountain = mix(rock_base, rock_light, n1);
            mountain = mix(mountain, snowy_peak, elev_t * elev_t);
            // Snow dusting — higher elevations get more white patches
            let snow_chance = fbm(world_p * 8.0 + vec2<f32>(200.0, 300.0));
            if snow_chance > (1.0 - elev_t * 0.6) {
                mountain = mix(mountain, snowy_peak, 0.7);
            }
            terrain_color = vec4<f32>(mountain + noise_detail * 0.6, 1.0);
        }

        // Procedural 6-Directional Normal Mapping from adjacent elevation gradient
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

        let tile_seed = fract(sin(f32(cell_x) * 12.9898 + f32(cell_y) * 78.233) * 43758.5453);
        let wave_speed = 0.8 + tile_seed * 1.4;
        let wave_phase = tile_seed * 6.28318;
        let freq_x = 0.12 + tile_seed * 0.06;
        let freq_y = 0.06 + (1.0 - tile_seed) * 0.06;

        let t = globals.time * wave_speed + wave_phase;
        let wave = sin(px * freq_x + py * freq_y + t) + cos(py * freq_x - px * freq_y + t * 0.7);

        let sparkle_t = floor(globals.time * 4.0);
        let sparkle_hash = fract(sin(px * 12.9898 + py * 78.233 + sparkle_t) * 43758.5453);
        let has_sparkle = sparkle_hash > 0.988;

        var color_deep = vec3<f32>(0.16, 0.30, 0.52);
        var color_mid  = vec3<f32>(0.22, 0.40, 0.62);
        var color_foam = vec3<f32>(0.32, 0.50, 0.75);

        if !is_ocean_water {
            color_deep = vec3<f32>(0.18, 0.42, 0.52);
            color_mid  = vec3<f32>(0.24, 0.50, 0.60);
            color_foam = vec3<f32>(0.32, 0.58, 0.68);
        }

        // FBM-driven depth variation for water
        let water_fbm = fbm(world_p * 3.0 + vec2<f32>(400.0, 500.0));
        color_deep = mix(color_deep, color_deep * 0.85, water_fbm);

        var final_water_color = color_deep;
        if has_sparkle {
            final_water_color = color_foam;
        } else if wave > 1.2 {
            final_water_color = color_foam;
        } else if wave > 0.4 {
            final_water_color = color_mid;
        }

        terrain_color = vec4<f32>(final_water_color, 1.0);

        let wave_dx = cos(px * freq_x + py * freq_y + t) * freq_x - sin(py * freq_x - px * freq_y + t * 0.7) * freq_y;
        let wave_dy = cos(px * freq_x + py * freq_y + t) * freq_y + sin(py * freq_x - px * freq_y + t * 0.7) * freq_x;
        normal = normalize(vec3<f32>(-wave_dx * 0.8, -wave_dy * 0.8, 1.0));
        is_specular = true;
    }

    // Convert sRGB palette input to linear space
    terrain_color = vec4<f32>(pow(terrain_color.rgb, vec3<f32>(2.2)), terrain_color.a);

    // ── Valley Ambient Occlusion ──
    // Low-elevation land tiles surrounded by higher terrain get darkened
    var ao_factor = 1.0;
    if is_land {
        let mag_center = f32(terrain_byte & 0x1Fu);
        var avg_neighbor_elev = 0.0;
        var neighbor_count = 0.0;
        for (var i = 0; i < 6; i = i + 1) {
            let nh = get_hex_neighbor(cell_hex, i);
            let ne = get_elevation(nh.x, nh.y);
            avg_neighbor_elev += ne;
            neighbor_count += 1.0;
        }
        avg_neighbor_elev = avg_neighbor_elev / neighbor_count;
        let height_diff = avg_neighbor_elev - mag_center;
        // Only darken when neighbors are higher (valley)
        ao_factor = 1.0 - clamp(height_diff * 0.025, 0.0, 0.15);
    }

    var base_color = terrain_color.rgb * ao_factor;

    // ── Territory overlay — reduced blend lets terrain detail show through ──
    if owner_id > 0u {
        let albedo = owner_albedo(owner_id);
        base_color = mix(terrain_color.rgb * ao_factor, albedo, 0.50);

        if flash_val > 0.0 && globals.effect_shockwave > 0.0 {
            let shockwave = flash_val * globals.effect_shockwave;
            let flash_color = mix(vec3<f32>(1.0, 1.0, 1.0), albedo, 1.0 - flash_val);
            base_color = mix(base_color, flash_color, shockwave * 0.8);
        }
    }

    let hex_center = hex_to_world(cell_hex);
    let local_pos = vec2<f32>(world_x, world_y) - hex_center;

    // ── Compute min distance to hex edge (used for borders, bevel, grid) ──
    var min_dist_to_edge = 0.5;
    for (var i = 0; i < 6; i = i + 1) {
        let dist_to_edge = 0.5 - dot(local_pos, hex_dir(i));
        min_dist_to_edge = min(min_dist_to_edge, dist_to_edge);
    }

    // ── Anti-aliased borders with outer glow ──
    var thickness = globals.border_thickness;
    let border_darkness = globals.border_darkness;
    let s_thickness = globals.shore_thickness;
    let s_darkness = globals.shore_darkness;

    if globals.effect_breathe > 0.0 && owner_id > 0u {
        let breathe = (sin(globals.time * 3.0 + f32(owner_id)) + 1.0) * 0.5;
        thickness += breathe * 0.05 * globals.effect_breathe;
    }
    if flash_val > 0.0 && globals.effect_shockwave > 0.0 {
        thickness += flash_val * 0.2 * globals.effect_shockwave;
    }

    // Glow extend zone beyond the hard border
    let glow_extent = thickness * 1.8;

    if is_land {
        let is_tribe = owner_id >= 200u;

        for (var i = 0; i < 6; i = i + 1) {
            let neighbor_hex = get_hex_neighbor(cell_hex, i);
            let neighbor_terrain = get_cell_terrain(neighbor_hex);
            let neighbor_owner = get_cell_owner(neighbor_hex);
            let neighbor_is_land = (neighbor_terrain & 0x80u) != 0u;

            let dir = hex_dir(i);
            let dist_to_edge = 0.5 - dot(local_pos, dir);

            if !neighbor_is_land {
                // Shore border — dark coastline edge
                if dist_to_edge < s_thickness {
                    let shore_t = 1.0 - smoothstep(0.0, s_thickness, dist_to_edge);
                    if owner_id > 0u {
                        let border_albedo = owner_albedo(owner_id) * border_darkness;
                        let shore_col = mix(border_albedo, vec3<f32>(0.015, 0.012, 0.010), 0.50);
                        base_color = mix(base_color, shore_col, shore_t);
                    } else {
                        base_color = mix(base_color, vec3<f32>(0.025, 0.020, 0.015), shore_t);
                    }
                }
                // Political border on coast
                if owner_id > 0u && dist_to_edge < thickness {
                    let border_t = 1.0 - smoothstep(0.0, thickness, dist_to_edge);
                    var border_albedo = owner_albedo(owner_id) * border_darkness;
                    if flash_val > 0.0 && globals.effect_shockwave > 0.0 {
                        border_albedo = mix(border_albedo, vec3<f32>(1.0), flash_val * globals.effect_shockwave);
                    }
                    base_color = mix(base_color, border_albedo, border_t);
                }
            } else if owner_id > 0u && neighbor_owner != owner_id {
                // Political border — anti-aliased with outer glow
                if dist_to_edge < glow_extent {
                    var border_col = owner_albedo(owner_id) * border_darkness;
                    let green_exists = is_tribe && (neighbor_owner >= 200u);
                    if green_exists {
                        border_col = vec3<f32>(0.2, 0.8, 0.2) * border_darkness;
                    }

                    if flash_val > 0.0 && globals.effect_shockwave > 0.0 {
                        border_col = mix(border_col, vec3<f32>(1.0), flash_val * globals.effect_shockwave);
                    }

                    if dist_to_edge < thickness {
                        // Hard border core — smooth anti-aliased falloff
                        let core_t = 1.0 - smoothstep(0.0, thickness * 0.3, dist_to_edge);
                        base_color = mix(base_color, border_col, 0.6 + 0.4 * core_t);
                    } else {
                        // Outer glow — soft feathered light beyond the hard line
                        let glow_t = 1.0 - smoothstep(thickness, glow_extent, dist_to_edge);
                        let glow_col = mix(border_col, base_color, 0.5);
                        base_color = mix(base_color, glow_col, glow_t * 0.35);
                    }
                }
            }
        }
    }

    // ── WAR FOG + FRONTIER GLOW ──
    {
        let world_pos_hex = hex_to_world(cell_hex);
        for (var ti = 0; ti < 8; ti = ti + 1) {
            let slot = globals.threat_slots[ti];
            let radius = slot.z;
            if radius <= 0.0 { continue; }

            let packed = u32(slot.w);
            let target_id = packed / 1024u;
            let attacker_id = packed % 1024u;
            if target_id != owner_id { continue; }

            let front_world = vec2<f32>(
                slot.x + 0.5 + f32(i32(slot.y) & 1) * 0.5,
                (slot.y + 0.5) * 0.8660254
            );
            let dist = distance(world_pos_hex, front_world);
            let threat = 1.0 - smoothstep(0.0, radius, dist);
            if threat <= 0.0 { continue; }

            if target_id == 0u {
                let atk_color = owner_albedo(attacker_id);
                let gold = vec3<f32>(0.95, 0.85, 0.3);
                let frontier_bright = mix(atk_color, gold, 0.5) * 1.2 + vec3<f32>(0.15);
                let frontier_dark = mix(atk_color, gold, 0.3) * 0.2;

                let glow_color = mix(frontier_dark, frontier_bright, threat);
                let glow_blend = threat * 0.35;

                let wave_phase = dist * 2.0 - globals.time * 2.0;
                let ripple = (sin(wave_phase) + 1.0) * 0.5;
                let ripple_glow = ripple * threat * 0.15;

                let edge_dist = abs(dist - radius * 0.85);
                let edge = smoothstep(2.5, 0.0, edge_dist) * 0.3;
                let edge_color = min(frontier_bright, vec3<f32>(1.0));

                base_color = mix(base_color, glow_color, glow_blend);
                base_color += edge_color * edge;
                base_color += frontier_bright * ripple_glow;
            } else {
                let atk_color = owner_albedo(attacker_id);
                let atk_bright = atk_color * 1.1 + vec3<f32>(0.15);
                let atk_dark = atk_color * 0.15;

                let lum = dot(base_color, vec3<f32>(0.299, 0.587, 0.114));
                let desat = mix(base_color, vec3<f32>(lum), threat * 0.45);

                let smoke_color = mix(atk_dark, atk_bright, threat * threat);
                let smoke_blend = threat * 0.35;

                let wave_phase = dist * 3.0 - globals.time * 4.0;
                let ripple = (sin(wave_phase) + 1.0) * 0.5;
                let ripple_intensity = ripple * threat * threat * 0.12;

                let corona_dist = abs(dist - radius * 0.15);
                let corona = smoothstep(1.5, 0.0, corona_dist) * 0.4;
                let corona_color = min(atk_color * 1.6 + vec3<f32>(0.3), vec3<f32>(1.0));

                var war_color = mix(desat, smoke_color, smoke_blend);
                war_color += corona_color * corona;
                war_color += atk_bright * ripple_intensity;
                base_color = war_color;
            }
        }
    }

    // ── Directional Lighting (diffuse + specular) ──
    let light_dir = normalize(vec3<f32>(-1.0, -1.0, 1.6));
    let diffuse = max(0.68, dot(normal, light_dir));
    base_color = base_color * (diffuse * 1.12);

    if is_specular {
        let view_dir = vec3<f32>(0.0, 0.0, 1.0);
        let half_dir = normalize(light_dir + view_dir);
        let spec = pow(max(0.0, dot(normal, half_dir)), 96.0);
        base_color = base_color + vec3<f32>(0.15 * spec);
    }

    // ── Embossed Cell Bevel ──
    let cell_bevel = smoothstep(0.0, 0.06, min_dist_to_edge);
    base_color = base_color * (0.86 + 0.14 * cell_bevel);

    // ── Tactile Canvas Paper Overlay ──
    let px_screen = in.uv.x * 2400.0;
    let py_screen = in.uv.y * 2400.0;
    let paper_noise = fract(sin(px_screen * 12.9898 + py_screen * 78.233) * 43758.5453);
    let paper_grain = 0.95 + 0.05 * paper_noise;
    base_color = base_color * paper_grain;

    // ── Screen Vignette ──
    let d_center = length(in.uv - 0.5);
    let vignette = smoothstep(0.8, 0.45, d_center);
    base_color = base_color * (0.82 + 0.18 * vignette);

    // ── Building Placement Holographic Grid ──
    if (globals.hover_building_kind > 0.0) {
        // 1. Check if this cell is inside any active no-build exclusion zones (scanning all 32 slots)
        var cell_in_nobuild_zone = false;
        for (var i = 0; i < 32; i = i + 1) {
            let slot = globals.nobuild_slots[i];
            let active_flag = slot.w;
            if (active_flag > 0.0) {
                let b_hex = vec2<i32>(i32(slot.x), i32(slot.y));
                let b_dist = hex_distance(cell_hex, b_hex);
                let block_radius = i32(slot.z);
                if (b_dist < block_radius) {
                    cell_in_nobuild_zone = true;
                }
            }
        }

        let hover_center = vec2<i32>(i32(globals.hover_hex.x), i32(globals.hover_hex.y));
        let dist = hex_distance(cell_hex, hover_center);

        if (dist <= 12) {
            // --- CURSOR ACTIVE SCAN FIELD ---
            let is_mine = owner_id == u32(globals.my_player_id);
            if (is_land && is_mine) {
                var overlay_color = vec3<f32>(0.0, 0.85, 1.0); // Neon Cyan
                var fill_intensity = 0.28;
                if (cell_in_nobuild_zone) {
                    overlay_color = vec3<f32>(1.0, 0.15, 0.15); // Vibrant Red
                    fill_intensity = 0.38;
                }

                // Holographic scan pulse ripples outward from the hover center
                let scan_fade = 1.0 - smoothstep(8.0, 12.0, f32(dist));

                // Slow, elegant radial waves rippling outward
                let wave = sin(globals.time * 1.5 - f32(dist) * 0.35) * 0.5 + 0.5;
                let border_pulse = 0.75 + 0.25 * wave;
                let fill_pulse = 0.85 + 0.15 * wave;

                // Mix self-illuminated fill
                let fill_alpha = fill_intensity * scan_fade * fill_pulse;
                base_color = mix(base_color, overlay_color, fill_alpha);

                // Emissive neon border line
                let line_intensity = smoothstep(0.045, 0.005, min_dist_to_edge);
                let border_alpha = line_intensity * border_pulse * scan_fade * 0.85;
                base_color = mix(base_color, overlay_color * 1.5, border_alpha);
            } else {
                // Highly visible red overlay for invalid water/enemy tiles
                let overlay_color = vec3<f32>(1.0, 0.12, 0.12);
                let scan_fade = 1.0 - smoothstep(8.0, 12.0, f32(dist));

                // Mix subtle unbuildable border lines to show hex grid shapes in water/enemy land
                let line_intensity = smoothstep(0.035, 0.0, min_dist_to_edge);
                let line_alpha = line_intensity * scan_fade * 0.25;
                base_color = mix(base_color, overlay_color, line_alpha);

                let fill_alpha = 0.10 * scan_fade;
                base_color = mix(base_color, overlay_color, fill_alpha);
            }
        } else if (cell_in_nobuild_zone) {
            // --- CONSTANT BACKGROUND EXCLUSION ZONE INDICATORS ---
            let overlay_color = vec3<f32>(1.0, 0.12, 0.12); // Soft holographic red

            // Faint breathing fill to keep it alive but unobtrusive
            let fill_pulse = 0.88 + 0.12 * sin(globals.time * 1.5);
            let fill_alpha = 0.15 * fill_pulse;
            base_color = mix(base_color, overlay_color, fill_alpha);

            // Subtle background grid wireframe
            let line_intensity = smoothstep(0.035, 0.0, min_dist_to_edge);
            let line_alpha = line_intensity * 0.35;
            base_color = mix(base_color, overlay_color * 1.2, line_alpha);
        }
    }

    // Convert from linear to sRGB
    let final_color = pow(base_color, vec3<f32>(1.0 / 2.2));
    return vec4<f32>(final_color, 1.0);
}