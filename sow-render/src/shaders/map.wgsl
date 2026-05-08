struct Globals {
    camera_pos: vec2<f32>,
    zoom: f32,
    screen_size: vec2<f32>,
}

var<uniform> globals: Globals;
var territory_texture: texture_2d<u32>;
var territory_sampler: sampler;

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_dim = textureDimensions(territory_texture);
    let pixel_coords = vec2<i32>(in.uv * vec2<f32>(tex_dim));
    
    let val = textureLoad(territory_texture, pixel_coords, 0).x;
    let owner_id = val & 0xFFFFu;
    
    if owner_id == 1u {
        return vec4<f32>(0.0, 1.0, 1.0, 1.0);
    } else if owner_id == 2u {
        return vec4<f32>(1.0, 0.0, 0.0, 1.0);
    } else if owner_id > 0u {
        return vec4<f32>(0.0, 1.0, 0.0, 1.0);
    }
    
    return vec4<f32>(0.05, 0.05, 0.05, 1.0);
}
