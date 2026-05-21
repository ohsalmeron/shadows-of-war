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
    border_roundness: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
    player_colors: array<vec4<f32>, 256>,
}

var<uniform> globals: Globals;
var territory_texture: texture_2d<u32>;

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

fn owner_albedo(owner_id: u32) -> vec3<f32> {
    if owner_id < 256u {
        return globals.player_colors[owner_id].rgb;
    }
    return vec3<f32>(0.5, 0.5, 0.5); // Fallback if out of bounds
}



@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let screen_pixel = in.uv * globals.screen_size;
    let world_x = (screen_pixel.x - globals.camera_pos.x) / globals.zoom;
    let world_y = (screen_pixel.y - globals.camera_pos.y) / globals.zoom;

    let cell_x = i32(floor(world_x));
    let cell_y = i32(floor(world_y));

    if cell_x < 0 || cell_y < 0 || cell_x >= i32(globals.map_size.x) || cell_y >= i32(globals.map_size.y) {
        return vec4<f32>(0.0, 0.0, 0.0, 1.0); // Matte black background
    }

    let pixel_coords = vec2<i32>(cell_x, cell_y);
    let val = textureLoad(territory_texture, pixel_coords, 0).x;
    
    let owner_id = val & 0x7FFFu;
    let is_green_border = (val & 0x00008000u) != 0u;
    let terrain_byte = (val >> 16u) & 0xFFu;
    let is_land = (terrain_byte & 0x80u) != 0u;

    // 1. Static high-fidelity paper parchment grain anchor
    let px = floor(world_x * 8.0);
    let py = floor(world_y * 8.0);
    let paper_grain = fract(sin(px * 12.9898 + py * 78.233) * 43758.5453);
    let land_texture = 0.94 + paper_grain * 0.06; // Fine organic paper tooth

    var terrain_color = vec4<f32>(0.0);
    
    if is_land {
        let is_shoreline = (terrain_byte & 0x40u) != 0u;
        let mag_center = f32(terrain_byte & 0x1Fu);

        // A. Smooth Bilinear Lambertian hillshading (shaded mountain relief)
        // Fetch 4 surrounding elevations to calculate smooth slopes crossing tile boundaries
        let val_10 = textureLoad(territory_texture, pixel_coords + vec2<i32>(1, 0), 0).x;
        let is_land_10 = ((val_10 >> 16u) & 0x80u) != 0u;
        let elev_10 = select(0.0, f32((val_10 >> 16u) & 0x1Fu), is_land_10);

        let val_01 = textureLoad(territory_texture, pixel_coords + vec2<i32>(0, 1), 0).x;
        let is_land_01 = ((val_01 >> 16u) & 0x80u) != 0u;
        let elev_01 = select(0.0, f32((val_01 >> 16u) & 0x1Fu), is_land_01);

        let val_11 = textureLoad(territory_texture, pixel_coords + vec2<i32>(1, 1), 0).x;
        let is_land_11 = ((val_11 >> 16u) & 0x80u) != 0u;
        let elev_11 = select(0.0, f32((val_11 >> 16u) & 0x1Fu), is_land_11);

        let tx = fract(world_x);
        let ty = fract(world_y);

        // Continuous partial derivatives of bilinear interpolation
        let slope_x = mix(elev_10 - mag_center, elev_11 - elev_01, ty);
        let slope_y = mix(elev_01 - mag_center, elev_11 - elev_10, tx);
        
        let light_dir = normalize(vec2<f32>(-1.0, -1.0)); // Top-Left virtual sun
        let slope = vec2<f32>(slope_x, slope_y);
        let hillshade = dot(slope, light_dir) * 0.035; // Drastically softened for watercolor/pencil feel

        // Gentle noise integration for hand-sketched parchment look
        let shaded_relief = hillshade * (1.0 + paper_grain * 0.2);

        // B. Steppe Parchment desaturated paper base palette (darker & richer)
        let shore_base = vec3<f32>(0.65, 0.58, 0.44);      // Rich Warm Sand Parchment
        let plains_base = vec3<f32>(0.35, 0.45, 0.28);     // Rich Deep Mossy Green
        let highland_base = vec3<f32>(0.52, 0.42, 0.30);   // Rich Cardboard Kraft Paper Brown
        let mountain_base = vec3<f32>(0.32, 0.30, 0.28);   // Dark Slate Gray Paper
        let snowy_peak = vec3<f32>(0.72, 0.72, 0.75);      // Soft Muted Snowy Peak

        var base_land_color = plains_base;

        if is_shoreline {
            base_land_color = shore_base;
        } else if mag_center < 10.0 {
            let factor = mag_center / 10.0;
            base_land_color = mix(shore_base, plains_base, factor);
        } else if mag_center < 20.0 {
            let factor = (mag_center - 10.0) / 10.0;
            base_land_color = mix(plains_base, highland_base, factor);
        } else {
            // Mountain Elevation Blending with Pencil Capping
            if mag_center < 24.0 {
                let blend = (mag_center - 20.0) / 4.0;
                base_land_color = mix(highland_base, mountain_base, blend);
            } else if mag_center < 27.0 {
                let blend = (mag_center - 24.0) / 3.0;
                base_land_color = mix(mountain_base, mountain_base * 0.7, blend); // Deep dark stone ridges
            } else {
                let blend = clamp((mag_center - 27.0) / 3.0, 0.0, 1.0);
                base_land_color = mix(mountain_base * 0.7, snowy_peak, blend);
            }
        }

        // Apply paper texture + hillshade relief
        let final_land = (base_land_color * land_texture) + vec3<f32>(shaded_relief);
        terrain_color = vec4<f32>(final_land, 1.0);

    } else {
        // C. Coastal ribbon glows fading into Prussian Navy
        // Check neighbors to identify water tiles adjacent to shorelines
        let neighbor_u = textureLoad(territory_texture, pixel_coords + vec2<i32>(0, -1), 0).x;
        let neighbor_d = textureLoad(territory_texture, pixel_coords + vec2<i32>(0, 1), 0).x;
        let neighbor_l = textureLoad(territory_texture, pixel_coords + vec2<i32>(-1, 0), 0).x;
        let neighbor_r = textureLoad(territory_texture, pixel_coords + vec2<i32>(1, 0), 0).x;

        let is_land_u = ((neighbor_u >> 16u) & 0x80u) != 0u;
        let is_land_d = ((neighbor_d >> 16u) & 0x80u) != 0u;
        let is_land_l = ((neighbor_l >> 16u) & 0x80u) != 0u;
        let is_land_r = ((neighbor_r >> 16u) & 0x80u) != 0u;

        let fx = fract(world_x);
        let fy = fract(world_y);

        var dist_to_land = 1.0;
        if is_land_u { dist_to_land = min(dist_to_land, fy); }
        if is_land_d { dist_to_land = min(dist_to_land, 1.0 - fy); }
        if is_land_l { dist_to_land = min(dist_to_land, fx); }
        if is_land_r { dist_to_land = min(dist_to_land, 1.0 - fx); }

        let is_near_shore = is_land_u || is_land_d || is_land_l || is_land_r;
        let glow_factor = select(0.0, clamp(1.0 - dist_to_land, 0.0, 1.0), is_near_shore);

        let ocean_base = vec3<f32>(0.16, 0.28, 0.44); // Deep Navy Prussian Blue
        let coast_glow = vec3<f32>(0.38, 0.56, 0.78); // Glowing light cyan ribbon

        // Blend smooth coastal glow into ocean
        var water_color = mix(ocean_base, coast_glow, pow(glow_factor, 1.8) * 0.7);

        // Procedural stable seed for wave animations
        let tile_seed = fract(sin(f32(cell_x) * 12.9898 + f32(cell_y) * 78.233) * 43758.5453);
        let wave_speed = 0.8 + tile_seed * 1.4;
        let wave_phase = tile_seed * 6.28318;
        let freq_x = 0.12 + tile_seed * 0.06;
        let freq_y = 0.06 + (1.0 - tile_seed) * 0.06;

        let t = globals.time * wave_speed + wave_phase;
        let wave = sin(px * freq_x + py * freq_y + t) + cos(py * freq_x - px * freq_y + t * 0.7);

        // Subtle retro water foam sparkle
        if wave > 1.3 {
            water_color = mix(water_color, coast_glow, 0.25);
        }

        terrain_color = vec4<f32>(water_color * (0.98 + paper_grain * 0.02), 1.0);
    }

    // Convert sRGB palette input to linear space so final pow(base_color, 1.0/2.2) renders the exact intended colors
    terrain_color = vec4<f32>(pow(terrain_color.rgb, vec3<f32>(2.2)), terrain_color.a);

    var base_color = terrain_color.rgb;
    if owner_id > 0u {
        let albedo = owner_albedo(owner_id);
        // Rich 50% opacity alpha blend for player territories
        base_color = mix(terrain_color.rgb, albedo, 0.50);
    }

    if owner_id > 0u {
        let border_up = (val & 0x80000000u) != 0u;
        let border_down = (val & 0x40000000u) != 0u;
        let border_left = (val & 0x20000000u) != 0u;
        let border_right = (val & 0x10000000u) != 0u;

        let shore_up = (val & 0x08000000u) != 0u;
        let shore_down = (val & 0x04000000u) != 0u;
        let shore_left = (val & 0x02000000u) != 0u;
        let shore_right = (val & 0x01000000u) != 0u;

        let fx = fract(world_x);
        let fy = fract(world_y);
        
        let thickness = globals.border_thickness;
        let border_darkness = globals.border_darkness;
        let s_thickness = globals.shore_thickness;
        let s_darkness = globals.shore_darkness;

        let roundness = globals.border_roundness;
        let border_r = thickness * roundness;
        
        let core_min_x = select(0.0, thickness + border_r, border_left);
        let core_max_x = select(1.0, 1.0 - thickness - border_r, border_right);
        let core_min_y = select(0.0, thickness + border_r, border_up);
        let core_max_y = select(1.0, 1.0 - thickness - border_r, border_down);

        let dx = max(core_min_x - fx, max(0.0, fx - core_max_x));
        let dy = max(core_min_y - fy, max(0.0, fy - core_max_y));
        let is_border = sqrt(dx*dx + dy*dy) > border_r;

        let shore_r = s_thickness * roundness;
        
        let s_core_min_x = select(0.0, s_thickness + shore_r, shore_left);
        let s_core_max_x = select(1.0, 1.0 - s_thickness - shore_r, shore_right);
        let s_core_min_y = select(0.0, s_thickness + shore_r, shore_up);
        let s_core_max_y = select(1.0, 1.0 - s_thickness - shore_r, shore_down);

        let s_dx = max(s_core_min_x - fx, max(0.0, fx - s_core_max_x));
        let s_dy = max(s_core_min_y - fy, max(0.0, fy - s_core_max_y));
        let is_shore = sqrt(s_dx*s_dx + s_dy*s_dy) > shore_r;

        let is_defended = (terrain_byte & 0x40u) != 0u;
        let is_even_tile = (u32(world_x) + u32(world_y)) % 2u == 0u;
        let draw_line = !is_defended || is_even_tile;

        if is_shore && draw_line {
            base_color = base_color * s_darkness;
        } else if is_border && draw_line {
            if is_green_border {
                base_color = vec3<f32>(0.2, 0.8, 0.2) * border_darkness;
            } else {
                let border_albedo = owner_albedo(owner_id) * border_darkness;
                base_color = border_albedo;
            }
        }
    }

    // Convert from linear to sRGB for final output to the Unorm surface
    let final_color = pow(base_color, vec3<f32>(1.0 / 2.2));
    return vec4<f32>(final_color, 1.0);
}
