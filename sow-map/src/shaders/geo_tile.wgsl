struct GeoGlobals {
    camera_pos: vec2<f32>,
    zoom: f32,
    _pad0: f32,
    screen_size: vec2<f32>,
}

struct GeoTileInstance {
    world_pos: vec2<f32>,
    size: f32,
    _pad1: f32,
}

var<uniform> globals: GeoGlobals;
var tile_tex: texture_2d<f32>;
var tile_sampler: sampler;

fn world_to_clip(world: vec2<f32>) -> vec4<f32> {
    let screen = world * globals.zoom + globals.camera_pos;
    let ndc = vec2(
        screen.x / globals.screen_size.x * 2.0 - 1.0,
        1.0 - screen.y / globals.screen_size.y * 2.0,
    );
    return vec4(ndc, 0.0, 1.0);
}

struct GeoVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: GeoTileInstance) -> GeoVertexOutput {
    var out: GeoVertexOutput;
    let corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0),
        vec2(1.0, 0.0),
        vec2(1.0, 1.0),
        vec2(0.0, 0.0),
        vec2(1.0, 1.0),
        vec2(0.0, 1.0),
    );
    let uv = corners[vi];
    let world = inst.world_pos + uv * inst.size;
    out.clip_position = world_to_clip(world);
    out.uv = uv;
    return out;
}

@fragment
fn fs_main(in: GeoVertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tile_tex, tile_sampler, in.uv);
}
