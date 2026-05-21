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

    var terrain_color = vec4<f32>(0.0);
    
    if is_land {
        let is_shoreline = (terrain_byte & 0x40u) != 0u;
        let mag_center = f32(terrain_byte & 0x1Fu);
        
        let px = floor(world_x * 8.0);
        let py = floor(world_y * 8.0);
        let land_noise = fract(sin(px * 12.9898 + py * 78.233) * 43758.5453);
        let noise_offset = (land_noise - 0.5) * 0.05; // Gentle ±2.5% color variation

        if is_shoreline {
            let base = vec3<f32>(204.0 / 255.0, 203.0 / 255.0, 158.0 / 255.0);
            terrain_color = vec4<f32>(base + noise_offset * 0.5, 1.0); // OpenFront Shore
        } else if mag_center < 10.0 {
            let r = 190.0 / 255.0;
            let g = (220.0 - 2.0 * mag_center) / 255.0;
            let b = 138.0 / 255.0;
            terrain_color = vec4<f32>(vec3<f32>(r, g, b) + noise_offset, 1.0); // OpenFront Plains
        } else if mag_center < 20.0 {
            let r = (200.0 + 2.0 * mag_center) / 255.0;
            let g = (183.0 + 2.0 * mag_center) / 255.0;
            let b = (138.0 + 2.0 * mag_center) / 255.0;
            terrain_color = vec4<f32>(vec3<f32>(r, g, b) + noise_offset * 1.2, 1.0); // OpenFront Highlands
        } else {
            // Smooth blend/fusion from high Highland color to snowy white peak
            let highland_base = vec3<f32>(240.0 / 255.0, 223.0 / 255.0, 178.0 / 255.0);
            let snowy_peak = vec3<f32>(245.0 / 255.0, 245.0 / 255.0, 245.0 / 255.0);
            let blend = clamp((mag_center - 20.0) / 11.0, 0.0, 1.0);
            let peak_color = mix(highland_base, snowy_peak, blend);
            terrain_color = vec4<f32>(peak_color + noise_offset * 0.8, 1.0); // OpenFront Mountains
        }
    } else {
        let is_ocean_water = (terrain_byte & 0x20u) != 0u;
        
        let px = floor(world_x * 8.0);
        let py = floor(world_y * 8.0);
        
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

        var color_deep = vec3<f32>(70.0 / 255.0, 132.0 / 255.0, 180.0 / 255.0); // OpenFront base blue
        var color_mid  = vec3<f32>(85.0 / 255.0, 143.0 / 255.0, 215.0 / 255.0);
        var color_foam = vec3<f32>(100.0 / 255.0, 143.0 / 255.0, 255.0 / 255.0); // OpenFront Shoreline water
        
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
    }

    // Convert sRGB palette input to linear space so final pow(base_color, 1.0/2.2) renders the exact intended colors
    terrain_color = vec4<f32>(pow(terrain_color.rgb, vec3<f32>(2.2)), terrain_color.a);

    var base_color = terrain_color.rgb;
    if owner_id > 0u {
        let albedo = owner_albedo(owner_id);
        base_color = mix(terrain_color.rgb, albedo, 0.75);
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