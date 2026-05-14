struct Globals {
    camera_pos: vec2<f32>,
    zoom: f32,
    time: f32,
    screen_size: vec2<f32>,
    map_size: vec2<f32>,
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
    if owner_id <= 16u {
        let hue = f32(owner_id) * 0.618033988749895;
        let r = abs(fract(hue) * 2.0 - 1.0);
        let g = abs(fract(hue + 0.333) * 2.0 - 1.0);
        let b = abs(fract(hue + 0.666) * 2.0 - 1.0);
        return clamp(vec3<f32>(r, g, b) * 0.52 + vec3<f32>(0.32, 0.32, 0.32), vec3<f32>(0.42), vec3<f32>(1.0));
    } else if owner_id <= 116u {
        let id = f32(owner_id);
        let r = fract(id * 0.123);
        let g = fract(id * 0.456);
        let b = fract(id * 0.789);
        return clamp(
            vec3<f32>(0.22 + r * 0.58, 0.2 + g * 0.55, 0.22 + b * 0.58),
            vec3<f32>(0.4),
            vec3<f32>(1.0)
        );
    } else {
        let id = f32(owner_id);
        let r = fract(id * 0.123);
        let g = fract(id * 0.456);
        let b = fract(id * 0.789);
        return clamp(
            vec3<f32>(0.24 + r * 0.56, 0.2 + g * 0.56, 0.22 + b * 0.54),
            vec3<f32>(0.4),
            vec3<f32>(1.0)
        );
    }
}



@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let screen_pixel = in.uv * globals.screen_size;
    let world_x = (screen_pixel.x - globals.camera_pos.x) / globals.zoom;
    let world_y = (screen_pixel.y - globals.camera_pos.y) / globals.zoom;

    let cell_x = i32(floor(world_x));
    let cell_y = i32(floor(world_y));

    if cell_x < 0 || cell_y < 0 || cell_x >= i32(globals.map_size.x) || cell_y >= i32(globals.map_size.y) {
        return vec4<f32>(0.02, 0.03, 0.06, 1.0);
    }

    let pixel_coords = vec2<i32>(cell_x, cell_y);
    let val = textureLoad(territory_texture, pixel_coords, 0).x;
    
    let owner_id = val & 0xFFFFu;
    let terrain_byte = (val >> 16u) & 0xFFu;
    let is_land = (terrain_byte & 0x80u) != 0u;

    var terrain_color = vec4<f32>(0.0);
    
    if is_land {
        let mag_center = f32(terrain_byte & 0x1Fu);
        if mag_center < 10.0 {
            terrain_color = vec4<f32>(0.12, 0.2, 0.1, 1.0); // Lush Plains
        } else if mag_center < 20.0 {
            terrain_color = vec4<f32>(0.28, 0.22, 0.14, 1.0); // Earthy Highlands
        } else {
            let snow = clamp((mag_center - 20.0) / 11.0, 0.0, 1.0);
            terrain_color = mix(vec4<f32>(0.28, 0.28, 0.3, 1.0), vec4<f32>(0.58, 0.6, 0.62, 1.0), snow); // Snowy Mountains
        }
    } else {
        let is_ocean_water = (terrain_byte & 0x20u) != 0u;
        if !is_ocean_water {
            terrain_color = vec4<f32>(0.12, 0.38, 0.58, 1.0); // River/Lake
        } else {
            terrain_color = vec4<f32>(0.05, 0.45, 0.65, 1.0); // Plain Ocean
        }
    }

    var base_color = terrain_color.rgb;
    if owner_id > 0u {
        let albedo = owner_albedo(owner_id);
        base_color = mix(terrain_color.rgb, albedo, 0.95);
    }

    // Check the 31st bit (highest bit of the 32-bit uint) for the CPU-computed border flag
    let is_border = (val & 0x80000000u) != 0u;

    if owner_id > 0u {
        if is_border {
            base_color = base_color * 0.3; // Darken for outline
        }
    }

    return vec4<f32>(base_color, 1.0);
}
