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
    outline_width_px: f32,
    glow_width_px: f32,
}

var<uniform> globals: TextGlobals;
var font_atlas: texture_2d<f32>;
var font_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) outline_color: vec4<f32>,
    @location(3) outline_width_px: f32,
    @location(4) glow_width_px: f32,
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
    out.outline_width_px = inst.outline_width_px;
    out.glow_width_px = inst.glow_width_px;
    return out;
}

fn median(r: f32, g: f32, b: f32) -> f32 {
    return max(min(r, g), min(max(r, g), b));
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv_dy = dpdy(in.uv);

    // 1. Center glyph sample (Fill)
    let msd_c = textureSample(font_atlas, font_sampler, in.uv).rgb;
    let sd_c = median(msd_c.r, msd_c.g, msd_c.b);
    
    let unitRange = vec2<f32>(16.0, 16.0) / vec2<f32>(textureDimensions(font_atlas, 0));
    let screenTexSize = 1.0 / fwidth(in.uv);
    let screenPxRange = max(0.5 * dot(unitRange, screenTexSize), 1.0);
    
    let thickness = 1.8; // Bold dilation in screen pixels
    let screenPxDist_c = screenPxRange * (sd_c - 0.5) + thickness;
    let fillAlpha = clamp(screenPxDist_c + 0.5, 0.0, 1.0);

    // 2. Downward offset glyph sample (Drop Shadow)
    let msd_s = textureSample(font_atlas, font_sampler, in.uv - uv_dy * in.outline_width_px).rgb;
    let sd_s = median(msd_s.r, msd_s.g, msd_s.b);
    let screenPxDist_s = screenPxRange * (sd_s - 0.5) + thickness;
    let shadowAlpha = clamp(screenPxDist_s + 0.5, 0.0, 1.0);

    // Composite shadow and fill
    let final_alpha = max(fillAlpha * in.color.a, shadowAlpha * in.outline_color.a);
    let final_color = mix(in.outline_color.rgb, in.color.rgb, fillAlpha);

    return vec4<f32>(final_color, final_alpha);
}
