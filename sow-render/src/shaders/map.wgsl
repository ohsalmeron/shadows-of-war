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
    territory_opacity: f32,
    fallout_slots: array<vec4<f32>, 8>,
    nobuild_slots: array<vec4<f32>, 32>,
    sub_voxel_scale: f32,
    blend_mode: f32,
    _pad3: f32,
    _pad4: f32,
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
    return vec2<i32>(floor(world_pos));
}

fn hex_distance(a: vec2<i32>, b: vec2<i32>) -> i32 {
    let r1 = a.y;
    let q1 = a.x - (a.y - (a.y & 1)) / 2;
    let s1 = -q1 - r1;
    let r2 = b.y;
    let q2 = b.x - (b.y - (b.y & 1)) / 2;
    let s2 = -q2 - r2;
    return (abs(q1 - q2) + abs(r1 - r2) + abs(s1 - s2)) / 2;
}

fn hex_to_world(hex: vec2<i32>) -> vec2<f32> {
    return vec2<f32>(f32(hex.x) + 0.5, f32(hex.y) + 0.5);
}

fn get_hex_neighbor(hex: vec2<i32>, direction: i32) -> vec2<i32> {
    var offset = vec2<i32>(0, 0);
    if (direction == 0) {
        offset = vec2<i32>(1, 0); // East
    } else if (direction == 1) {
        offset = vec2<i32>(-1, 0); // West
    } else if (direction == 2) {
        offset = vec2<i32>(0, -1); // North
    } else if (direction == 3) {
        offset = vec2<i32>(0, 1); // South
    } else if (direction == 4) {
        offset = vec2<i32>(1, -1); // Northeast
    } else if (direction == 5) {
        offset = vec2<i32>(-1, -1); // Northwest
    } else if (direction == 6) {
        offset = vec2<i32>(1, 1); // Southeast
    } else if (direction == 7) {
        offset = vec2<i32>(-1, 1); // Southwest
    }
    return hex + offset;
}

fn owner_albedo(owner_id: u32) -> vec3<f32> {
    if owner_id < 256u {
        return player_colors.colors[owner_id].rgb;
    }
    return vec3<f32>(0.5, 0.5, 0.5); // Fallback if out of bounds
}

fn blend_channel_overlay(base: f32, blend: f32) -> f32 {
    if (base < 0.5) {
        return 2.0 * base * blend;
    } else {
        return 1.0 - 2.0 * (1.0 - base) * (1.0 - blend);
    }
}

fn blend_overlay(base: vec3<f32>, blend: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        blend_channel_overlay(base.r, blend.r),
        blend_channel_overlay(base.g, blend.g),
        blend_channel_overlay(base.b, blend.b)
    );
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let screen_pixel = in.uv * globals.screen_size;
    let world_x = (screen_pixel.x - globals.camera_pos.x) / globals.zoom;
    let world_y = (screen_pixel.y - globals.camera_pos.y) / globals.zoom;

    let scale = globals.sub_voxel_scale;
    var cell_hex = world_to_hex(vec2<f32>(world_x, world_y));
    var sub_dist_to_edge = 0.5;
    var sub_hex_center = vec2<f32>(world_x, world_y);

    if (scale > 1.05) {
        let scaled_pos = vec2<f32>(world_x, world_y) * scale;
        let sub_hex = world_to_hex(scaled_pos);
        sub_hex_center = hex_to_world(sub_hex) / scale;
        cell_hex = world_to_hex(sub_hex_center);
        
        let local_sub_pos = (vec2<f32>(world_x, world_y) - sub_hex_center) * scale;
        sub_dist_to_edge = 0.5 - max(abs(local_sub_pos.x), abs(local_sub_pos.y));
    }

    let cell_x = cell_hex.x;
    let cell_y = cell_hex.y;

    if cell_x < 0 || cell_y < 0 || cell_x >= i32(globals.map_size.x) || cell_y >= i32(globals.map_size.y) {
        return vec4<f32>(0.015, 0.015, 0.02, 1.0); // Sleek dark space/canvas backdrop
    }

    let hex_center = hex_to_world(cell_hex);
    let local_pos = vec2<f32>(world_x, world_y) - hex_center;
    let sub_hex_local_pos = sub_hex_center - hex_center;

    let pixel_coords = vec2<i32>(cell_x, cell_y);
    let terrain_rgba = textureLoad(terrain_texture, pixel_coords, 0);
    let terrain_byte = u32(terrain_rgba.x * 255.0 + 0.5);
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
        var mag_center = f32(terrain_byte & 0x1Fu);

        // Height interpolation towards nearest neighbor
        if (scale > 1.05) {
            let dist_to_parent = length(sub_hex_local_pos);
            if (dist_to_parent > 0.001) {
                let local_dir = sub_hex_local_pos / dist_to_parent;
                const neighbor_dirs = array<vec2<f32>, 8>(
                    vec2<f32>(1.0, 0.0),
                    vec2<f32>(-1.0, 0.0),
                    vec2<f32>(0.0, -1.0),
                    vec2<f32>(0.0, 1.0),
                    vec2<f32>(0.70710678, -0.70710678),
                    vec2<f32>(-0.70710678, -0.70710678),
                    vec2<f32>(0.70710678, 0.70710678),
                    vec2<f32>(-0.70710678, 0.70710678)
                );
                var nearest_neighbor_idx = 0;
                var max_dot = -999.0;
                for (var i = 0; i < 8; i = i + 1) {
                    let d = dot(local_dir, neighbor_dirs[i]);
                    if d > max_dot {
                        max_dot = d;
                        nearest_neighbor_idx = i;
                    }
                }
                
                let neighbor_hex = get_hex_neighbor(cell_hex, nearest_neighbor_idx);
                let neighbor_terrain = get_cell_terrain(neighbor_hex);
                let neighbor_is_land = (neighbor_terrain & 0x80u) != 0u;
                var neighbor_height = 0.0;
                if neighbor_is_land {
                    neighbor_height = f32(neighbor_terrain & 0x1Fu);
                }
                let t = clamp(dist_to_parent / 0.5, 0.0, 1.0);
                mag_center = mix(mag_center, neighbor_height, t * 0.5);
            }
        }
        
        let land_noise = terrain_rgba.w;
        let organic_wave = sin(world_x * 0.20) * cos(world_y * 0.20) * 0.04;
        let noise_offset = (land_noise - 0.5) * 0.03 + organic_wave; // Gentle organic landscape variation

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
        let elevation_shading = 0.82 + 0.28 * height_factor; // Brighten peaks, shadow valleys
        base_color = base_color * elevation_shading;
    }
    if owner_id > 0u {
        let albedo = owner_albedo(owner_id);
        
        var opacity = globals.territory_opacity;
        if (opacity <= 0.01) {
            opacity = 0.28;
        }

        // Territory overlay — supports different blend modes
        var blended_color = base_color;
        let mode = i32(round(globals.blend_mode));
        if (mode == 1) {
            // Multiply
            blended_color = base_color * albedo;
        } else if (mode == 2) {
            // Overlay
            blended_color = blend_overlay(base_color, albedo);
        } else if (mode == 3) {
            // All Albedo
            blended_color = albedo;
        } else {
            // Normal Mix
            blended_color = albedo;
        }

        if (mode == 3) {
            base_color = blended_color;
        } else {
            base_color = mix(base_color, blended_color, opacity);
        }

        // ── Territory Heartbeat Pulse (living empire breathe) ──
        let heartbeat = 0.97 + 0.03 * sin(globals.time * 1.8 + f32(owner_id) * 2.3);
        base_color = base_color * heartbeat;

        // Conquest shockwave flash on interior
        if flash_val > 0.0 && globals.effect_shockwave > 0.0 {
            let shockwave = flash_val * globals.effect_shockwave;
            let flash_color = mix(vec3<f32>(1.0, 1.0, 1.0), albedo, 1.0 - flash_val);
            base_color = mix(base_color, flash_color, shockwave * 0.8);
        }
    } else if is_land {
        // ── Wilderness Atmosphere Haze (unclaimed land fog) ──
        let haze_color = vec3<f32>(0.55, 0.58, 0.68);
        let haze_amount = 0.08 + 0.04 * sin(world_x * 0.05 + world_y * 0.07);
        base_color = mix(base_color, haze_color, haze_amount);
    }

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
    if globals.effect_breathe > 0.0 && owner_id > 0u {
        let breathe = (sin(globals.time * 3.0 + f32(owner_id)) + 1.0) * 0.5;
        thickness += breathe * 0.05 * globals.effect_breathe;
    }
    // Shockwave border explosion
    if flash_val > 0.0 && globals.effect_shockwave > 0.0 {
        thickness += flash_val * 0.2 * globals.effect_shockwave;
    }

    if is_land {
        let has_border = ((owner_packed >> 24u) & 1u) != 0u;
        let has_water_neighbor = ((owner_packed >> 25u) & 1u) != 0u;

        if has_border || has_water_neighbor {
            let is_tribe = owner_id >= 200u;
            const neighbor_dirs = array<vec2<f32>, 4>(
                vec2<f32>(1.0, 0.0),  // East
                vec2<f32>(-1.0, 0.0), // West
                vec2<f32>(0.0, -1.0), // North
                vec2<f32>(0.0, 1.0)   // South
            );

            for (var i = 0; i < 4; i = i + 1) {
                let neighbor_hex = get_hex_neighbor(cell_hex, i);
                let neighbor_terrain = get_cell_terrain(neighbor_hex);
                let neighbor_owner = get_cell_owner(neighbor_hex);
                let neighbor_is_land = (neighbor_terrain & 0x80u) != 0u;

                let dir = neighbor_dirs[i];
                var dist_to_edge = 0.5 - dot(local_pos, dir);
                if (scale > 1.05) {
                    dist_to_edge = 0.5 - dot(sub_hex_local_pos, dir);
                }

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
            var shore_t = 1.0 - smoothstep(shore_thickness - 0.04, shore_thickness, min_shore_dist);
            if (scale > 1.05) {
                shore_t = 1.0; // Sharp block-perfect shoreline
            }
            // Sandy coast tint — not the near-black political border line
            var shore_col = vec3<f32>(0.55, 0.50, 0.32) * shore_darkness;
            if (terrain_byte & 0x40u) != 0u {
                shore_col = vec3<f32>(0.72, 0.68, 0.42) * shore_darkness;
            }
            base_color = mix(base_color, shore_col, shore_t * 0.65);
        }

        if is_border {
            var border_t = 1.0 - smoothstep(thickness - 0.04, thickness, min_border_dist);
            if (scale > 1.05) {
                border_t = 1.0; // Sharp block-perfect political border
            }
            if is_green_border {
                let border_col = vec3<f32>(0.2, 0.8, 0.2) * border_darkness;
                base_color = mix(base_color, border_col, border_t);
            } else {
                var border_albedo = vec3<f32>(0.025, 0.020, 0.015) * border_darkness; // Dark neutral water border line
                if owner_id > 0u {
                    border_albedo = owner_albedo(owner_id) * border_darkness;

                    // ── Dynamic Conquest Border Shockwave Pulse ──
                    if flash_val > 0.0 && globals.effect_shockwave > 0.0 {
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
                    if contested_owner > 0u && contested_owner != owner_id {
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
    {
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
                slot.x + 0.5,
                slot.y + 0.5
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
                let atk_bright = atk_color * 1.4 + vec3<f32>(0.3);
                let atk_dark = atk_color * 0.15;

                // Desaturation — territory drains to grey
                let lum = dot(base_color, vec3<f32>(0.299, 0.587, 0.114));
                let desat = mix(base_color, vec3<f32>(lum), threat * 0.60);

                // Attacker smoke
                let smoke_color = mix(atk_dark, atk_bright, threat * threat);
                let smoke_blend = threat * 0.55;

                // Ripple waves
                let wave_phase = dist * 3.0 - globals.time * 4.0;
                let ripple = (sin(wave_phase) + 1.0) * 0.5;
                let ripple_intensity = ripple * threat * threat * 0.25;

                // Corona front
                let corona_dist = abs(dist - radius * 0.15);
                let corona = smoothstep(1.5, 0.0, corona_dist) * 0.7;
                let corona_color = min(atk_color * 2.0 + vec3<f32>(0.5), vec3<f32>(1.0));

                var war_color = mix(desat, smoke_color, smoke_blend);
                war_color += corona_color * corona;
                war_color += atk_bright * ripple_intensity;
                base_color = war_color;
            }
        }
    }

    // ── NUCLEAR FALLOUT CONTAMINATION ZONES ──
    {
        let cell_world = hex_to_world(cell_hex);
        for (var fi = 0; fi < 8; fi = fi + 1) {
            let slot = globals.fallout_slots[fi];
            let f_radius = slot.z;
            if f_radius <= 0.0 { continue; }

            let alpha_p = slot.w;
            let f_center = vec2<f32>(
                slot.x + 0.5,
                slot.y + 0.5
            );
            let dist = distance(cell_world, f_center);
            if dist > f_radius * 1.5 { continue; }

            let elapsed = (1.0 - alpha_p) * 7.0;

            // 1. SCORCHED EARTH (CRATER / BURN)
            let burn_radius = f_radius * 0.65;
            if dist <= burn_radius {
                let burn_factor = (1.0 - dist / burn_radius) * 0.85 * alpha_p;
                let charcoal = vec3<f32>(0.08, 0.08, 0.1);
                base_color = mix(base_color, charcoal, burn_factor);
            }

            // 2. ACTIVE EXPLOSION & STAGGERED SHOCKWAVE (First 3.0 seconds)
            if elapsed < 3.0 {
                let p_exp = elapsed / 3.0;
                let shockwave_r = p_exp * f_radius;

                let fire_noise = fract(sin(world_x * 12.3 + world_y * 17.7 - globals.time * 5.0) * 43758.5453);

                // --- A. Rising Mushroom Cloud & Stem ---
                let rise = p_exp * f_radius * 0.45;
                let cap_center = f_center - vec2<f32>(0.0, rise);
                let dist_cap = distance(cell_world, cap_center);
                let cap_r = f_radius * 0.42 * (0.35 + 0.65 * p_exp);

                let dist_stem = abs(cell_world.x - f_center.x);
                let stem_w = f_radius * 0.09 * (1.0 - p_exp * 0.4) * (1.0 + 0.15 * sin(cell_world.y * 1.5 - globals.time * 4.0));
                let is_in_stem = cell_world.y >= cap_center.y && cell_world.y <= f_center.y && dist_stem <= stem_w;
                let is_in_cap = dist_cap <= cap_r;

                if is_in_cap || is_in_stem {
                    let age_fade = 1.0 - p_exp;

                    var dist_factor = 0.0;
                    if is_in_cap {
                        dist_factor = 1.0 - dist_cap / cap_r;
                    } else {
                        dist_factor = 1.0 - dist_stem / stem_w;
                    }

                    let fire_white = vec3<f32>(2.5, 2.5, 2.5);
                    let fire_yellow = vec3<f32>(2.2, 1.8, 0.3);
                    let fire_red = vec3<f32>(1.8, 0.3, 0.05);
                    let smoke = vec3<f32>(0.05, 0.05, 0.05);

                    let heat = clamp(age_fade * 1.45 - (1.0 - dist_factor) * 0.45 + (fire_noise - 0.5) * 0.35, 0.0, 1.0);

                    var fire_rgb = vec3<f32>(0.0);
                    if heat > 0.8 {
                        fire_rgb = mix(fire_yellow, fire_white, (heat - 0.8) / 0.2);
                    } else if heat > 0.4 {
                        fire_rgb = mix(fire_red, fire_yellow, (heat - 0.4) / 0.4);
                    } else {
                        fire_rgb = mix(smoke, fire_red, heat / 0.4);
                    }

                    base_color = mix(base_color, fire_rgb, smoothstep(0.0, 0.2, heat) * age_fade);
                }

                // --- B. Blinding Ground-Zero Flash ---
                if elapsed < 0.35 {
                    let flash_fade = 1.0 - elapsed / 0.35;
                    let flash_r = f_radius * 1.1;
                    if dist <= flash_r {
                        let flash_intensity = (1.0 - dist / flash_r) * flash_fade * 0.85;
                        base_color += vec3<f32>(2.5, 2.4, 2.2) * flash_intensity;
                    }
                }

                // --- C. Expanding Shockwave Front Ring (Leading Edge) ---
                let wave_width = 2.0;
                let edge_dist = abs(dist - shockwave_r);
                if edge_dist < wave_width {
                    let wave_factor = 1.0 - edge_dist / wave_width;
                    let wave_pulse = wave_factor * wave_factor * (1.0 - p_exp);
                    base_color += vec3<f32>(2.5, 2.2, 1.8) * wave_pulse * 1.1;
                }
            }

            // 3. TOXIC RADIOACTIVE FALLOUT (Pulsing, boiling green sludge)
            if dist <= f_radius {
                let falloff = 1.0 - dist / f_radius;
                let pulse = sin(globals.time * 3.5) * 0.15 + 0.85;

                let n1 = fract(sin(world_x * 8.3 + world_y * 14.7 + globals.time * 0.9) * 43758.5453);
                let n2 = fract(sin(world_x * 21.1 - world_y * 12.3 + globals.time * 1.4) * 23421.631);
                let noise = (n1 + n2) * 0.5;

                let bubbles = sin(world_x * 2.0 + sin(globals.time * 2.0 + world_y) * 2.5) * cos(world_y * 2.0 + cos(globals.time * 2.0 - world_x) * 2.5);
                let bubble_glow = smoothstep(0.85, 1.0, bubbles) * pulse * 0.5 * alpha_p;

                let toxic_green = vec3<f32>(0.05, 0.90, 0.15);
                let toxic_bright = vec3<f32>(0.20, 1.00, 0.45);
                let glow_color = mix(toxic_green, toxic_bright, noise * 0.6) * 1.6;

                let intensity = falloff * falloff * alpha_p * pulse;
                let core_mix = smoothstep(0.0, 0.8, falloff * alpha_p);

                base_color = mix(base_color, glow_color, core_mix * 0.6);
                base_color += glow_color * intensity * 0.45;
                base_color += vec3<f32>(0.3, 1.0, 0.5) * bubble_glow;
            }
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

    // ── Golden Hour Sun Sweep (slow warm-cool color temperature cycle) ──
    let sun_phase = sin(globals.time * 0.08) * 0.5 + 0.5;
    let warm_tint = vec3<f32>(1.02 + 0.03 * sun_phase, 1.0 + 0.01 * sun_phase, 1.0 - 0.02 * sun_phase);
    base_color = base_color * warm_tint;

    // Hex SDF reused by land emboss + building-placement grid (computed once; emboss skipped on water)
    let min_dist_to_edge = 0.5 - max(abs(local_pos.x), abs(local_pos.y));

    // Embossed cell vignette — land only (on water it moirés into diagonal hatch when zoomed out)
    if is_land && globals.zoom >= 0.6 {
        var bevel_dist = min_dist_to_edge;
        if (scale > 1.05) {
            let zoom_lod = clamp((globals.zoom - 0.6) / 0.9, 0.0, 1.0);
            bevel_dist = mix(min_dist_to_edge, sub_dist_to_edge, zoom_lod);
        }
        let cell_bevel = smoothstep(0.0, 0.06, bevel_dist);
        base_color = base_color * (0.86 + 0.14 * cell_bevel);
    }

    // ── Screen Vignetting (Soft shading at screen edges) ──
    let d_center = length(in.uv - 0.5);
    let vignette = smoothstep(0.8, 0.45, d_center);
    base_color = base_color * (0.82 + 0.18 * vignette);

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