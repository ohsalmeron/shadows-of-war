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
    flat_map_mode: f32,
    fallout_slots: array<vec4<f32>, 8>,
    nobuild_slots: array<vec4<f32>, 32>,
}

struct PlayerColors {
    colors: array<vec4<f32>, 256>,
}

var<uniform> globals: Globals;
var<uniform> player_colors: PlayerColors;
var terrain_texture: texture_2d<f32>;
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
    return u32(textureLoad(terrain_texture, vec2<i32>(hex.x, hex.y), 0).x * 255.0 + 0.5);
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
    return vec3<f32>(0.5, 0.5, 0.5); // Fallback if out of bounds
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
    let terrain_rgba = textureLoad(terrain_texture, pixel_coords, 0);
    let terrain_byte = u32(terrain_rgba.x * 255.0 + 0.5);
    let owner_packed = textureLoad(owner_texture, pixel_coords, 0).x;
    let owner_id = owner_packed & 0xFFFFu;
    let flash_byte = (owner_packed >> 16u) & 0xFFu;
    let flash_val = f32(flash_byte) / 255.0;
    let is_land = (terrain_byte & 0x80u) != 0u;
    let flat_map = globals.flat_map_mode > 0.5;

    var terrain_color = vec4<f32>(0.0);
    var normal = vec3<f32>(0.0, 0.0, 1.0);
    var is_specular = false;
    
    if is_land {
        let is_shoreline = (terrain_byte & 0x40u) != 0u;
        let mag_center = f32(terrain_byte & 0x1Fu);
        
        let land_noise = terrain_rgba.w;
        let organic_wave = sin(world_x * 0.20) * cos(world_y * 0.20) * 0.04;
        var noise_offset = (land_noise - 0.5) * 0.03 + organic_wave; // Gentle organic landscape variation
        if flat_map {
            noise_offset = 0.0;
        }

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

        // Unpack procedural 6-directional elevation normals directly from G/B channels in O(1) time
        let dx = (terrain_rgba.y * 16.0) - 8.0;
        let dy = (terrain_rgba.z * 16.0) - 8.0;
        normal = normalize(vec3<f32>(-dx, -dy, 1.0));
    } else {
        let is_ocean_water = (terrain_byte & 0x20u) != 0u;

        // Flat water — no procedural waves (they moiré into diagonal scanlines when zoomed out)
        var color_flat = vec3<f32>(65.0 / 255.0, 128.0 / 255.0, 175.0 / 255.0);
        if !is_ocean_water {
            color_flat = vec3<f32>(55.0 / 255.0, 135.0 / 255.0, 168.0 / 255.0);
        }

        terrain_color = vec4<f32>(color_flat, 1.0);
    }

    // Convert sRGB palette input to linear space
    terrain_color = vec4<f32>(pow(terrain_color.rgb, vec3<f32>(2.2)), terrain_color.a);

    var base_color = terrain_color.rgb;

    // ── Peak/Valley Elevation Shading (Premium 3D Relief) ──
    if is_land {
        let mag_center = f32(terrain_byte & 0x1Fu);
        let height_factor = mag_center / 32.0;
        var elevation_shading = 0.82 + 0.28 * height_factor; // Brighten peaks, shadow valleys
        if flat_map {
            elevation_shading = 1.0;
        }
        base_color = base_color * elevation_shading;
    }
    if owner_id > 0u {
        let albedo = owner_albedo(owner_id);
        let lum = dot(base_color, vec3<f32>(0.299, 0.587, 0.114));
        base_color = albedo * (0.3 + 1.0 * lum);

        // ── Territory Heartbeat Pulse (living empire breathe) ──
        if !flat_map {
            let heartbeat = 0.97 + 0.03 * sin(globals.time * 1.8 + f32(owner_id) * 2.3);
            base_color = base_color * heartbeat;
        }

        // Conquest shockwave flash on interior
        if !flat_map && flash_val > 0.0 && globals.effect_shockwave > 0.0 {
            let shockwave = flash_val * globals.effect_shockwave;
            let flash_color = mix(vec3<f32>(1.0, 1.0, 1.0), albedo, 1.0 - flash_val);
            base_color = mix(base_color, flash_color, shockwave * 0.8);
        }
    } else if is_land && !flat_map {
        // ── Wilderness Atmosphere Haze (unclaimed land fog) ──
        let haze_color = vec3<f32>(0.55, 0.58, 0.68);
        let haze_amount = 0.08 + 0.04 * sin(world_x * 0.05 + world_y * 0.07);
        base_color = mix(base_color, haze_color, haze_amount);
    }
    let hex_center = hex_to_world(cell_hex);
    let local_pos = vec2<f32>(world_x, world_y) - hex_center;

    var is_border = false;
    var is_green_border = false;
    var min_border_dist = 99.0;
    var is_shore_edge = false;
    var min_shore_dist = 99.0;

    var thickness = globals.border_thickness;
    let border_darkness = globals.border_darkness;
    let shore_thickness = globals.shore_thickness;
    let shore_darkness = globals.shore_darkness;

    // Border breathe: subtle thickness pulse per owner
    if !flat_map && globals.effect_breathe > 0.0 && owner_id > 0u {
        let breathe = (sin(globals.time * 3.0 + f32(owner_id)) + 1.0) * 0.5;
        thickness += breathe * 0.05 * globals.effect_breathe;
    }
    // Shockwave border explosion
    if !flat_map && flash_val > 0.0 && globals.effect_shockwave > 0.0 {
        thickness += flash_val * 0.2 * globals.effect_shockwave;
    }

    if is_land {
        let has_border = ((owner_packed >> 24u) & 1u) != 0u;
        let has_water_neighbor = ((owner_packed >> 25u) & 1u) != 0u;

        if has_border || has_water_neighbor {
            let is_tribe = owner_id >= 200u;
            const neighbor_dirs = array<vec2<f32>, 6>(
                vec2<f32>(1.0, 0.0),
                vec2<f32>(-1.0, 0.0),
                vec2<f32>(-0.5, -0.86602540378),
                vec2<f32>(0.5, -0.86602540378),
                vec2<f32>(-0.5, 0.86602540378),
                vec2<f32>(0.5, 0.86602540378)
            );

            for (var i = 0; i < 6; i = i + 1) {
                let neighbor_hex = get_hex_neighbor(cell_hex, i);
                let neighbor_terrain = get_cell_terrain(neighbor_hex);
                let neighbor_owner = get_cell_owner(neighbor_hex);
                let neighbor_is_land = (neighbor_terrain & 0x80u) != 0u;

                let dir = neighbor_dirs[i];
                let dist_to_edge = 0.5 - dot(local_pos, dir);

                if !neighbor_is_land {
                    if dist_to_edge < shore_thickness {
                        is_shore_edge = true;
                        min_shore_dist = min(min_shore_dist, dist_to_edge);
                    }
                } else {
                    if owner_id > 0u && neighbor_owner != owner_id {
                        let green_exists = is_tribe && (neighbor_owner >= 200u);
                        // LOD 3 Optimization: Skip drawing borders between minor tribes to reduce macro noise
                        if globals.zoom < 0.6 && green_exists {
                            continue;
                        }
                        
                        if dist_to_edge < thickness {
                            is_border = true;
                            min_border_dist = min(min_border_dist, dist_to_edge);
                            if green_exists {
                                is_green_border = true;
                            }
                        }
                    }
                }
            }
        }

        if is_shore_edge {
            let shore_t = 1.0 - smoothstep(shore_thickness - 0.04, shore_thickness, min_shore_dist);
            // Sandy coast tint — not the near-black political border line
            var shore_col = vec3<f32>(0.55, 0.50, 0.32) * shore_darkness;
            if (terrain_byte & 0x40u) != 0u {
                shore_col = vec3<f32>(0.72, 0.68, 0.42) * shore_darkness;
            }
            base_color = mix(base_color, shore_col, shore_t * 0.65);
        }

        if is_border {
            let border_t = 1.0 - smoothstep(thickness - 0.04, thickness, min_border_dist);
            if is_green_border {
                let border_col = vec3<f32>(0.2, 0.8, 0.2) * border_darkness;
                base_color = mix(base_color, border_col, border_t);
            } else {
                var border_albedo = vec3<f32>(0.025, 0.020, 0.015) * border_darkness; // Dark neutral water border line
                if owner_id > 0u {
                    border_albedo = owner_albedo(owner_id) * border_darkness;

                    // ── Dynamic Conquest Border Shockwave Pulse ──
                    if !flat_map && flash_val > 0.0 && globals.effect_shockwave > 0.0 {
                        let pulse = 1.0 - min_border_dist / globals.border_thickness;
                        // Keep conquest pulse in the conquering player's hue instead of whitening it.
                        let conquer_glow = min(owner_albedo(owner_id) * 1.35, vec3<f32>(1.0, 1.0, 1.0));
                        border_albedo = mix(
                            border_albedo,
                            conquer_glow,
                            flash_val * globals.effect_shockwave * pulse * 0.5
                        );
                    }

                    // ── Contested Border Energy Crackling (PvP shimmer) ──
                    let neighbor_hex_0 = get_hex_neighbor(cell_hex, 0);
                    let contested_owner = get_cell_owner(neighbor_hex_0);
                    if !flat_map && contested_owner > 0u && contested_owner != owner_id {
                        let enemy_albedo = owner_albedo(contested_owner);
                        let energy_t = sin(globals.time * 2.5) * 0.5 + 0.5;
                        border_albedo = mix(border_albedo, enemy_albedo * border_darkness, energy_t * 0.22);
                    }
                }

                base_color = mix(base_color, border_albedo, border_t);
            }
        }
    }

    // ── WAR FOG + FRONTIER GLOW ──
    if !flat_map {
        let world_pos_hex = hex_to_world(cell_hex);
        for (var ti = 0; ti < 8; ti = ti + 1) {
            let slot = globals.threat_slots[ti];
            let radius = slot.z;
            if radius <= 0.0 { continue; }

            let packed = u32(slot.w + 0.5);
            let target_id = packed / 1024u;
            let attacker_id = packed % 1024u;
            if target_id != owner_id { continue; }

            let front_world = vec2<f32>(
                slot.x + 0.5 + f32(i32(slot.y) % 2) * 0.5,
                (slot.y + 0.5) * 0.8660254
            );
            let dist = distance(world_pos_hex, front_world);
            let threat = 1.0 - smoothstep(0.0, radius, dist);
            if threat <= 0.0 { continue; }

            if target_id == 0u {
                // ── FRONTIER GLOW: Wilderness expansion ──
                // Golden-green aura — your civilization spreading into the wild
                let atk_color = owner_albedo(attacker_id);
                let gold = vec3<f32>(0.95, 0.85, 0.3);
                let frontier_bright = mix(atk_color, gold, 0.5) * 1.2 + vec3<f32>(0.15);
                let frontier_dark = mix(atk_color, gold, 0.3) * 0.2;

                // Gentle radial glow
                let glow_color = mix(frontier_dark, frontier_bright, threat);
                let glow_blend = threat * 0.35;

                // Slower, wider expansion ripples
                let wave_phase = dist * 2.0 - globals.time * 2.0;
                let ripple = (sin(wave_phase) + 1.0) * 0.5;
                let ripple_glow = ripple * threat * 0.15;

                // Soft leading edge (no aggressive corona)
                let edge_dist = abs(dist - radius * 0.85);
                let edge = smoothstep(2.5, 0.0, edge_dist) * 0.3;
                let edge_color = min(frontier_bright, vec3<f32>(1.0));

                base_color = mix(base_color, glow_color, glow_blend);
                base_color += edge_color * edge;
                base_color += frontier_bright * ripple_glow;
            } else {
                // ── WAR FOG: PvP attack ──
                let atk_color = owner_albedo(attacker_id);

                // Clean, elegant, and highly performant threat glow (no high-frequency noise)
                let slow_breathe = 0.92 + 0.08 * sin(globals.time * 2.0);
                let intensity = threat * slow_breathe;

                // Softly tint base color with attacker color
                base_color = mix(base_color, atk_color * 0.22, intensity * 0.45);

                // A subtle highlight towards the center of attack
                base_color += atk_color * (intensity * intensity * 0.12);
            }
        }
    }

    // ── NUCLEAR FALLOUT CONTAMINATION ZONES ──
    if !flat_map {
        let cell_world = hex_to_world(cell_hex);
        for (var fi = 0; fi < 8; fi = fi + 1) {
            let slot = globals.fallout_slots[fi];
            let f_radius = slot.z;
            if f_radius <= 0.0 { continue; }

            let alpha_p = slot.w;
            let f_center = vec2<f32>(
                slot.x + 0.5 + f32(i32(slot.y) % 2) * 0.5,
                (slot.y + 0.5) * 0.8660254
            );
            let dist = distance(cell_world, f_center);
            if dist > f_radius { continue; }

            let falloff = 1.0 - dist / f_radius;
            let pulse = sin(globals.time * 3.0) * 0.15 + 0.85;

            // Cheap procedural toxic noise — two octaves of fract-sin hash
            let n1 = fract(sin(world_x * 7.3 + world_y * 13.7 + globals.time * 0.8) * 43758.5453);
            let n2 = fract(sin(world_x * 19.1 - world_y * 11.3 + globals.time * 1.3) * 23421.631);
            let noise = (n1 + n2) * 0.5;

            // Toxic green glow with noise variation
            let toxic_green = vec3<f32>(0.15, 0.85, 0.25);
            let toxic_bright = vec3<f32>(0.3, 1.0, 0.45);
            let glow_color = mix(toxic_green, toxic_bright, noise * 0.6);

            let intensity = falloff * falloff * alpha_p * pulse;

            // Additive blend — makes terrain glow without washing it out
            base_color = base_color + glow_color * intensity * 0.35;

            // Inner core is brighter
            let core = smoothstep(0.7, 1.0, falloff);
            base_color = base_color + toxic_bright * core * alpha_p * 0.15 * pulse;
        }
    }

    // ── Upgraded Lighting (Sun directional diffuse + Specular highlights) ──
    if !flat_map {
        let light_dir = normalize(vec3<f32>(-1.0, -1.0, 1.6)); // Directional sun light from top-left
        let diffuse = max(0.68, dot(normal, light_dir));
        base_color = base_color * (diffuse * 1.12);

        if is_specular {
            let view_dir = vec3<f32>(0.0, 0.0, 1.0);
            let half_dir = normalize(light_dir + view_dir);
            let spec = pow(max(0.0, dot(normal, half_dir)), 96.0);
            base_color = base_color + vec3<f32>(0.15 * spec);
        }
    }

    // ── Golden Hour Sun Sweep (slow warm-cool color temperature cycle) ──
    if !flat_map {
        let sun_phase = sin(globals.time * 0.08) * 0.5 + 0.5;
        let warm_tint = vec3<f32>(1.02 + 0.03 * sun_phase, 1.0 + 0.01 * sun_phase, 1.0 - 0.02 * sun_phase);
        base_color = base_color * warm_tint;
    }

    // Hex SDF reused by land emboss + building-placement grid (computed once; emboss skipped on water)
    let min_dist_to_edge = 0.5 - max(abs(local_pos.x), 0.5 * abs(local_pos.x) + 0.86602540378 * abs(local_pos.y));

    // Embossed cell vignette — land only (on water it moirés into diagonal hatch when zoomed out)
    if !flat_map && is_land && globals.zoom >= 0.6 {
        let cell_bevel = smoothstep(0.0, 0.06, min_dist_to_edge);
        base_color = base_color * (0.86 + 0.14 * cell_bevel);
    }

    // ── Tactile Canvas/Matte Paper Texture Overlay (Zoom-dependent LOD) ──
    if !flat_map && globals.zoom >= 2.0 {
        let px_screen = in.uv.x * 2400.0;
        let py_screen = in.uv.y * 2400.0;
        let paper_noise = fract(sin(px_screen * 12.9898 + py_screen * 78.233) * 43758.5453);
        let grain_scale = clamp((globals.zoom - 2.0) / 3.0, 0.0, 1.0);
        let paper_grain = 1.0 + (paper_noise - 0.5) * 0.08 * grain_scale;
        base_color = base_color * paper_grain;
    }

    // ── Screen Vignetting (Soft shading at screen edges) ──
    if !flat_map {
        let d_center = length(in.uv - 0.5);
        let vignette = smoothstep(0.8, 0.45, d_center);
        base_color = base_color * (0.82 + 0.18 * vignette);
    }

    // ── Building Placement Holographic Grid ──
    if (globals.hover_building_kind > 0.0) {
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
        let hover_w = hex_to_world(hover_center);
        let cell_w = hex_to_world(cell_hex);
        let dist_w = distance(hover_w, cell_w);

        if (dist_w <= 6.0) {
            let is_mine = owner_id == u32(globals.my_player_id);
            if (is_land && is_mine) {
                var overlay_color = vec3<f32>(0.0, 0.85, 1.0);
                var fill_intensity = 0.28;
                if (cell_in_nobuild_zone) {
                    overlay_color = vec3<f32>(1.0, 0.15, 0.15);
                    fill_intensity = 0.38;
                }

                // ── Cybernetic Placement Scan Lines ──
                let scanline = sin(screen_pixel.y * 0.9 + globals.time * 15.0) * 0.22 + 0.78;
                overlay_color = overlay_color * scanline;

                let scan_fade = 1.0 - smoothstep(4.5, 6.0, dist_w);
                let wave = sin(globals.time * 1.5 - dist_w * 0.35) * 0.5 + 0.5;
                let border_pulse = 0.75 + 0.25 * wave;
                let fill_pulse = 0.85 + 0.15 * wave;

                let fill_alpha = fill_intensity * scan_fade * fill_pulse;
                base_color = mix(base_color, overlay_color, fill_alpha);

                let line_intensity = smoothstep(0.090, 0.005, min_dist_to_edge);
                let border_alpha = line_intensity * border_pulse * scan_fade * 0.85;
                base_color = mix(base_color, overlay_color * 1.5, border_alpha);
            } else {
                var overlay_color = vec3<f32>(1.0, 0.12, 0.12);
                let scan_fade = 1.0 - smoothstep(4.5, 6.0, dist_w);

                // ── Cybernetic Placement Scan Lines ──
                let scanline = sin(screen_pixel.y * 0.9 + globals.time * 15.0) * 0.22 + 0.78;
                overlay_color = overlay_color * scanline;

                let line_intensity = smoothstep(0.080, 0.0, min_dist_to_edge);
                let line_alpha = line_intensity * scan_fade * 0.25;
                base_color = mix(base_color, overlay_color, line_alpha);

                let fill_alpha = 0.10 * scan_fade;
                base_color = mix(base_color, overlay_color, fill_alpha);
            }
        } else if (cell_in_nobuild_zone) {
            var overlay_color = vec3<f32>(1.0, 0.12, 0.12);

            // ── Cybernetic Placement Scan Lines ──
            let scanline = sin(screen_pixel.y * 0.9 + globals.time * 15.0) * 0.22 + 0.78;
            overlay_color = overlay_color * scanline;

            let fill_pulse = 0.88 + 0.12 * sin(globals.time * 1.5);
            let fill_alpha = 0.15 * fill_pulse;
            base_color = mix(base_color, overlay_color, fill_alpha);

            let line_intensity = smoothstep(0.035, 0.0, min_dist_to_edge);
            let line_alpha = line_intensity * 0.35;
            base_color = mix(base_color, overlay_color * 1.2, line_alpha);
        }
    }

    // Convert from linear to sRGB
    let final_color = pow(base_color, vec3<f32>(1.0 / 2.2));
    return vec4<f32>(final_color, 1.0);
}