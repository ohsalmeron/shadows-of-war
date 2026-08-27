struct MoverGlobals {
    camera_pos: vec2<f32>,
    zoom: f32,
    sprite_count: u32,
    screen_size: vec2<f32>,
    trail_count: u32,
    _pad: f32,
}

// Per-instance attributes (instanced vertex buffer, divisor = 1).
// Field names MUST match `MoverInstanceGpu` in mover_renderer.rs.
struct MoverInstance {
    world_pos: vec2<f32>,
    size: f32,
    rotation: f32,
    color: vec4<f32>,
    uv_rect: vec4<f32>,
    height: f32,
}

var<uniform> globals: MoverGlobals;
var sprite_atlas: texture_2d<f32>;
var sprite_sampler: sampler;

fn world_to_clip(world: vec2<f32>) -> vec4<f32> {
    let screen = world * globals.zoom + globals.camera_pos;
    let ndc = vec2(
        screen.x / globals.screen_size.x * 2.0 - 1.0,
        1.0 - screen.y / globals.screen_size.y * 2.0,
    );
    return vec4(ndc, 0.0, 1.0);
}

struct SpriteVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: MoverInstance) -> SpriteVertexOutput {
    var out: SpriteVertexOutput;
    let corners = array<vec2<f32>, 6>(
        vec2(-0.5, -0.5),
        vec2(0.5, -0.5),
        vec2(0.5, 0.5),
        vec2(-0.5, -0.5),
        vec2(0.5, 0.5),
        vec2(-0.5, 0.5),
    );
    let local = corners[vi];
    let c = cos(inst.rotation);
    let s = sin(inst.rotation);
    let rotated = vec2(
        local.x * c - local.y * s,
        local.x * s + local.y * c,
    );
    let size_screen = inst.size * globals.zoom;
    let offset = rotated * size_screen;
    let world = inst.world_pos
        + vec2(offset.x / globals.zoom, (offset.y - inst.height * globals.zoom) / globals.zoom);
    out.clip_position = world_to_clip(world);
    let uv_local = local + vec2(0.5, 0.5);
    out.uv = mix(inst.uv_rect.xy, inst.uv_rect.zw, uv_local);
    out.color = inst.color;
    return out;
}

@fragment
fn fs_main(in: SpriteVertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(sprite_atlas, sprite_sampler, in.uv);
    return tex * in.color;
}

// WebGL canvas is a plain UNORM surface (no hardware linear->sRGB encode), so
// encode here to match the native sRGB swapchain. Selected by surface format at
// pipeline creation — same contract as map.wgsl. Alpha stays linear.
@fragment
fn fs_main_srgb(in: SpriteVertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(sprite_atlas, sprite_sampler, in.uv);
    let c = tex * in.color;
    return vec4<f32>(pow(c.rgb, vec3<f32>(1.0 / 2.2)), c.a);
}

