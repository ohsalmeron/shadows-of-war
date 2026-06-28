struct MoverGlobals {
    camera_pos: vec2<f32>,
    zoom: f32,
    sprite_count: u32,
    screen_size: vec2<f32>,
    trail_count: u32,
    _pad: f32,
}

// Per-instance attributes (instanced vertex buffer, divisor = 1).
// Field names MUST match `TrailSegmentGpu` in mover_renderer.rs.
struct TrailSegment {
    p0: vec2<f32>,
    p1: vec2<f32>,
    width: f32,
    color: vec4<f32>,
}

var<uniform> globals: MoverGlobals;

fn world_to_clip(world: vec2<f32>) -> vec4<f32> {
    let screen = world * globals.zoom + globals.camera_pos;
    let ndc = vec2(
        screen.x / globals.screen_size.x * 2.0 - 1.0,
        1.0 - screen.y / globals.screen_size.y * 2.0,
    );
    return vec4(ndc, 0.0, 1.0);
}

struct TrailVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, seg: TrailSegment) -> TrailVertexOutput {
    var out: TrailVertexOutput;
    let dir = seg.p1 - seg.p0;
    let len = length(dir);
    let tangent = select(vec2(1.0, 0.0), dir / len, len > 0.0001);
    let normal = vec2(-tangent.y, tangent.x);
    let half_w = seg.width * 0.5;

    // (corner across width, position along segment): two triangles.
    let corners = array<vec2<f32>, 6>(
        vec2(-1.0, 0.0),
        vec2(1.0, 0.0),
        vec2(1.0, 1.0),
        vec2(-1.0, 0.0),
        vec2(1.0, 1.0),
        vec2(-1.0, 1.0),
    );
    let corner = corners[vi];
    let world = mix(seg.p0, seg.p1, corner.y) + normal * half_w * corner.x / globals.zoom;
    out.clip_position = world_to_clip(world);
    out.color = seg.color;
    return out;
}

@fragment
fn fs_main(in: TrailVertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}

// WebGL canvas is a plain UNORM surface (no hardware linear->sRGB encode), so
// encode here to match the native sRGB swapchain. Selected by surface format at
// pipeline creation — same contract as map.wgsl. Alpha stays linear.
@fragment
fn fs_main_srgb(in: TrailVertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(pow(in.color.rgb, vec3<f32>(1.0 / 2.2)), in.color.a);
}
