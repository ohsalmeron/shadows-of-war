struct TextGlobals {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
}

struct TextInstance {
    screen_pos: vec2<f32>,
    size: vec2<f32>,
    uv_rect: vec4<f32>,
    color: vec4<f32>,
    outline_color: vec4<f32>,
    face_dilate: f32,
    outline_thickness: f32,
    underlay_offset_y: f32,
    underlay_softness: f32,
    kind: f32,
}

var<uniform> globals: TextGlobals;
var font_atlas: texture_2d<f32>;
var font_sampler: sampler;
var emoji_atlas: texture_2d<f32>;
var emoji_sampler: sampler;
var avatar_atlas: texture_2d<f32>;
var avatar_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) outline_color: vec4<f32>,
    @location(3) face_dilate: f32,
    @location(4) outline_thickness: f32,
    @location(5) underlay_offset_y: f32,
    @location(6) underlay_softness: f32,
    @location(7) uv_rect: vec4<f32>,
    @location(8) kind: f32,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: TextInstance) -> VertexOutput {
    var out: VertexOutput;
    let corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0),
        vec2(1.0, 0.0),
        vec2(1.0, 1.0),
        vec2(0.0, 0.0),
        vec2(1.0, 1.0),
        vec2(0.0, 1.0),
    );
    let local = corners[vi];

    let screen = inst.screen_pos + local * inst.size;
    let ndc = vec2<f32>(
        screen.x / globals.screen_size.x * 2.0 - 1.0,
        1.0 - screen.y / globals.screen_size.y * 2.0,
    );

    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = mix(inst.uv_rect.xy, inst.uv_rect.zw, local);
    out.color = inst.color;
    out.outline_color = inst.outline_color;
    out.face_dilate = inst.face_dilate;
    out.outline_thickness = inst.outline_thickness;
    out.underlay_offset_y = inst.underlay_offset_y;
    out.underlay_softness = inst.underlay_softness;
    out.uv_rect = inst.uv_rect;
    out.kind = inst.kind;
    return out;
}

fn median(r: f32, g: f32, b: f32) -> f32 {
    return max(min(r, g), min(max(r, g), b));
}

fn sample_atlas(uv: vec2<f32>, uv_rect: vec4<f32>) -> vec3<f32> {
    let clamped_uv = clamp(uv, uv_rect.xy, uv_rect.zw);
    let sampled = textureSample(font_atlas, font_sampler, clamped_uv).rgb;

    let eps = 1e-5;
    let inside_x = step(uv_rect.x - eps, uv.x) * step(uv.x, uv_rect.z + eps);
    let inside_y = step(uv_rect.y - eps, uv.y) * step(uv.y, uv_rect.w + eps);

    return sampled * inside_x * inside_y;
}

fn sample_emoji(uv: vec2<f32>, lo: vec2<f32>, hi: vec2<f32>) -> vec4<f32> {
    let eps = 1e-5;
    let inside_x = step(lo.x - eps, uv.x) * step(uv.x, hi.x + eps);
    let inside_y = step(lo.y - eps, uv.y) * step(uv.y, hi.y + eps);
    return textureSample(emoji_atlas, emoji_sampler, clamp(uv, lo, hi)) * inside_x * inside_y;
}

// Procedural shapes (KIND_DISC / KIND_RING). For shapes uv_rect is [0,0,1,1], so `in.uv`
// is the local 0..1 quad coordinate. `outline_thickness` carries the ring width in pixels
// (converted to uv via fwidth). `color` is the fill/stroke color.
fn shade_shape(in: VertexOutput) -> vec4<f32> {
    let p = in.uv - vec2<f32>(0.5, 0.5);
    let dist = length(p);
    let aa = max(fwidth(dist), 1e-5);

    if (in.kind > 2.5) {
        // Ring: outer edge at uv radius 0.5, drawn inward by `outline_thickness` px.
        let tw = max(in.outline_thickness * fwidth(in.uv).x, aa);
        let outer = 1.0 - smoothstep(0.5 - aa, 0.5, dist);
        let inner = 1.0 - smoothstep(0.5 - tw - aa, 0.5 - tw, dist);
        let a = clamp(outer - inner, 0.0, 1.0) * in.color.a;
        return vec4<f32>(in.color.rgb, a);
    }

    // Disc: filled circle.
    let a = (1.0 - smoothstep(0.5 - aa, 0.5, dist)) * in.color.a;
    return vec4<f32>(in.color.rgb, a);
}

// Circle-clipped image sprite (KIND_SPRITE) sampled from the avatar atlas. `in.uv` is the
// atlas uv; deriving local 0..1 from uv_rect gives the disc mask. `color` tints the texels.
fn shade_sprite(in: VertexOutput) -> vec4<f32> {
    let span = max(in.uv_rect.zw - in.uv_rect.xy, vec2<f32>(1e-6));
    let local = (in.uv - in.uv_rect.xy) / span;
    let d = length(local - vec2<f32>(0.5, 0.5));
    let aa = max(fwidth(d), 1e-5);
    let mask = 1.0 - smoothstep(0.5 - aa, 0.5, d);
    let tex = textureSample(avatar_atlas, avatar_sampler, clamp(in.uv, in.uv_rect.xy, in.uv_rect.zw));
    return vec4<f32>(tex.rgb * in.color.rgb, tex.a * mask * in.color.a);
}

fn shade_text(in: VertexOutput) -> vec4<f32> {
    if (in.kind > 3.5) {
        return shade_sprite(in);
    }
    if (in.kind > 1.5) {
        return shade_shape(in);
    }
    if (in.kind > 0.5) {
        let lo = in.uv_rect.xy;
        let hi = in.uv_rect.zw;
        let span = hi - lo;

        let fw = fwidth(in.uv);
        var ow = vec2<f32>(0.0);
        if (in.outline_thickness > 0.0) {
            ow = max(fw * in.outline_thickness, span * 0.01);
        }
        var so = 0.0;
        if (in.underlay_offset_y > 0.0) {
            so = max(fw.y * in.underlay_offset_y, span.y * 0.01);
        }

        let center = sample_emoji(in.uv, lo, hi);

        var ring = sample_emoji(in.uv + vec2<f32>( ow.x, 0.0), lo, hi).a;
        ring = max(ring, sample_emoji(in.uv + vec2<f32>(-ow.x, 0.0), lo, hi).a);
        ring = max(ring, sample_emoji(in.uv + vec2<f32>(0.0,  ow.y), lo, hi).a);
        ring = max(ring, sample_emoji(in.uv + vec2<f32>(0.0, -ow.y), lo, hi).a);
        ring = max(ring, sample_emoji(in.uv + vec2<f32>( ow.x,  ow.y), lo, hi).a);
        ring = max(ring, sample_emoji(in.uv + vec2<f32>( ow.x, -ow.y), lo, hi).a);
        ring = max(ring, sample_emoji(in.uv + vec2<f32>(-ow.x,  ow.y), lo, hi).a);
        ring = max(ring, sample_emoji(in.uv + vec2<f32>(-ow.x, -ow.y), lo, hi).a);

        let shadow_a = sample_emoji(in.uv - vec2<f32>(0.0, so), lo, hi).a;

        let fill_a = center.a * in.color.a;
        let dark_a = max(ring, shadow_a) * in.outline_color.a;
        let out_a = max(fill_a, dark_a);
        if (out_a <= 0.001) {
            // Transparent (not `discard`): keeps this a plain helper so the
            // Naga GLSL/WebGL path stays happy; equivalent under alpha blending.
            return vec4<f32>(0.0);
        }
        let fill_ratio = fill_a / max(out_a, 0.0001);
        let rgb = mix(in.outline_color.rgb, center.rgb, fill_ratio);
        return vec4<f32>(rgb, out_a);
    }

    // MSDF text path
    let uv_dy = dpdy(in.uv);

    let unitRange = vec2<f32>(16.0, 16.0) / vec2<f32>(textureDimensions(font_atlas, 0));
    let screenTexSize = 1.0 / fwidth(in.uv);
    let screenPxRange = max(0.5 * dot(unitRange, screenTexSize), 1.0);

    let msd_c = sample_atlas(in.uv, in.uv_rect);
    let sd_c = median(msd_c.r, msd_c.g, msd_c.b);
    let centerDist = screenPxRange * (sd_c - 0.5) + in.face_dilate;

    let fillAlpha = clamp(centerDist + 0.5, 0.0, 1.0);
    let outlineAlpha = clamp(in.outline_thickness + centerDist + 0.5, 0.0, 1.0) * (1.0 - fillAlpha);

    let shadowUv = in.uv - uv_dy * in.underlay_offset_y;
    let msd_s = sample_atlas(shadowUv, in.uv_rect);
    let sd_s = median(msd_s.r, msd_s.g, msd_s.b);
    let shadowDist = screenPxRange * (sd_s - 0.5);
    let softness = max(in.underlay_softness * screenPxRange, 0.001);
    let shadowAlpha = clamp((shadowDist + 0.5) / (1.0 + softness), 0.0, 1.0);

    let fill_a = fillAlpha * in.color.a;
    let black_a = max(outlineAlpha, shadowAlpha) * in.outline_color.a;
    let final_a = max(fill_a, black_a);
    let fill_ratio = fill_a / max(final_a, 0.0001);
    let final_rgb = mix(in.outline_color.rgb, in.color.rgb, fill_ratio);

    return vec4<f32>(final_rgb, final_a);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return shade_text(in);
}

// WebGL canvas is a plain UNORM surface (no hardware linear->sRGB encode on
// write), so we encode here to match the sRGB swapchain used natively. Picked
// by surface format at pipeline creation — same contract as map.wgsl.
@fragment
fn fs_main_srgb(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = shade_text(in);
    return vec4<f32>(pow(c.rgb, vec3<f32>(1.0 / 2.2)), c.a);
}