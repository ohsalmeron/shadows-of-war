struct StructureGlobals {
    camera_pos: vec2<f32>,
    zoom: f32,
    _pad1: f32,
    screen_size: vec2<f32>,
    /// Outline thickness in physical pixels (from dev-tools calibrated value).
    outline_px: f32,
    /// Drop-shadow offset in physical pixels (from dev-tools calibrated value).
    shadow_px: f32,
}

struct StructureInstance {
    world_pos: vec2<f32>,
    size: f32,
    shape_type: f32, // 0=circle, 1=octagon, 2=square, 3=pentagon
    color: vec4<f32>,
    outline_color: vec4<f32>,
    icon_uv: vec4<f32>,
    opacity: f32,
}

var<uniform> globals: StructureGlobals;
var icon_atlas: texture_2d<f32>;
var icon_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) shape_type: f32,
    @location(2) color: vec4<f32>,
    @location(3) outline_color: vec4<f32>,
    @location(4) icon_uv: vec4<f32>,
    @location(5) opacity: f32,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, inst: StructureInstance) -> VertexOutput {
    var out: VertexOutput;
    let corners = array<vec2<f32>, 6>(
        vec2(-1.0, -1.0),
        vec2(1.0, -1.0),
        vec2(1.0, 1.0),
        vec2(-1.0, -1.0),
        vec2(1.0, 1.0),
        vec2(-1.0, 1.0),
    );
    let local = corners[vi];

    // Bare-emoji quads (shape_type >= 4): expand 25% so the outline taps have room.
    // The FS insets the emoji UV by the same 1.25 factor.
    var size = select(inst.size, inst.size * 1.25, inst.shape_type >= 3.5);

    var screen: vec2<f32>;
    if (inst.shape_type > 3.5 && inst.shape_type < 4.5) {
        // Screen-space bare emoji (HUD/nameplate): already in physical pixels.
        screen = inst.world_pos + local * size;
    } else {
        // World-space (shapes 0-3, world emoji 5): camera transform.
        let world = inst.world_pos + local * size;
        screen = world * globals.zoom + globals.camera_pos;
    }
    let ndc = vec2<f32>(
        screen.x / globals.screen_size.x * 2.0 - 1.0,
        1.0 - screen.y / globals.screen_size.y * 2.0,
    );

    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.local_pos = local;
    out.shape_type = inst.shape_type;
    out.color = inst.color;
    out.outline_color = inst.outline_color;
    out.icon_uv = inst.icon_uv;
    out.opacity = inst.opacity;
    return out;
}

fn sdPolygon(p: vec2<f32>, r: f32, n: f32, rot: f32) -> f32 {
    let an = 3.1415926535 / n;
    let c = cos(rot);
    let s = sin(rot);
    let pr = vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
    let a = atan2(pr.y, pr.x) + an;
    // Handle positive modulo
    let rem = a % (2.0 * an);
    let a_folded = select(rem, rem + 2.0 * an, rem < 0.0) - an;
    return length(pr) * cos(a_folded) - r * cos(an);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let p = in.local_pos;

    // Bare emoji with alpha-dilated outline. The quad is 1.25× the sprite size;
    // inset the UV by the same factor so the emoji maps to the inner 80% and the
    // outer ring is breathing room for the outline taps.
    if (in.shape_type >= 3.5) {
        let uv_local = (p * 1.25) * 0.5 + 0.5;
        let uv = mix(in.icon_uv.xy, in.icon_uv.zw, uv_local);
        let span = in.icon_uv.zw - in.icon_uv.xy;
        let lo = in.icon_uv.xy;
        let hi = in.icon_uv.zw;

        // Outline width: derive from the calibrated outline_px converted to UV space
        // via fwidth so it tracks zoom correctly. Fallback: 7% of the cell span.
        let fw = fwidth(uv);
        let ow_screen = globals.outline_px;
        // Convert screen-px outline to UV delta: fw is UV-per-pixel.
        let ow = max(fw * ow_screen, span * 0.04);
        // Shadow offset: shadow_px downward in screen space → upward UV offset.
        let so = max(fw.y * globals.shadow_px, span.y * 0.06);

        let center = textureSample(icon_atlas, icon_sampler, clamp(uv, lo, hi));

        // Outline ring: max alpha over 8 offset taps (alpha dilation).
        var ring = textureSample(icon_atlas, icon_sampler, clamp(uv + vec2<f32>( ow.x, 0.0), lo, hi)).a;
        ring = max(ring, textureSample(icon_atlas, icon_sampler, clamp(uv + vec2<f32>(-ow.x, 0.0), lo, hi)).a);
        ring = max(ring, textureSample(icon_atlas, icon_sampler, clamp(uv + vec2<f32>(0.0,  ow.y), lo, hi)).a);
        ring = max(ring, textureSample(icon_atlas, icon_sampler, clamp(uv + vec2<f32>(0.0, -ow.y), lo, hi)).a);
        ring = max(ring, textureSample(icon_atlas, icon_sampler, clamp(uv + vec2<f32>( ow.x,  ow.y), lo, hi)).a);
        ring = max(ring, textureSample(icon_atlas, icon_sampler, clamp(uv + vec2<f32>( ow.x, -ow.y), lo, hi)).a);
        ring = max(ring, textureSample(icon_atlas, icon_sampler, clamp(uv + vec2<f32>(-ow.x,  ow.y), lo, hi)).a);
        ring = max(ring, textureSample(icon_atlas, icon_sampler, clamp(uv + vec2<f32>(-ow.x, -ow.y), lo, hi)).a);

        // Drop shadow: sample upward in the atlas (reads below the emoji on screen).
        let shadow_a = textureSample(icon_atlas, icon_sampler, clamp(uv - vec2<f32>(0.0, so), lo, hi)).a;

        let fill_a = center.a * in.color.a;
        let dark_a = max(ring, shadow_a) * in.outline_color.a;
        let out_a = max(fill_a, dark_a) * in.opacity;
        if (out_a <= 0.001) {
            discard;
        }
        let fill_ratio = fill_a / max(out_a, 0.0001);
        let rgb = mix(in.outline_color.rgb, center.rgb, fill_ratio);
        return vec4<f32>(rgb, out_a);
    }

    // Evaluate procedural SDF shape
    var d = 0.0;
    let r = 0.85; // base radius inside the unit quad [-1, 1]
    
    if (in.shape_type < 0.5) {
        // Circle (City)
        d = length(p) - r;
    } else if (in.shape_type < 1.5) {
        // Octagon (Bunker)
        d = sdPolygon(p, r, 8.0, 0.0);
    } else if (in.shape_type < 2.5) {
        // Square (Factory)
        d = max(abs(p.x), abs(p.y)) - r;
    } else {
        // Pentagon (Port)
        d = sdPolygon(p, r, 5.0, 3.1415926535 / 2.0); // vertex up
    }
    
    // Evaluate anti-aliased edge + border
    let fw = fwidth(d);
    let fill_mask = 1.0 - smoothstep(-fw, fw, d);
    
    let border_width = 0.08;
    let border_mask = 1.0 - smoothstep(-fw, fw, d + border_width);
    
    if (fill_mask <= 0.0) {
        discard;
    }
    
    // Composite shape fill over outline border
    var color = mix(in.outline_color.rgb, in.color.rgb, border_mask);
    
    // Draw icon emoji inside
    // Map local_pos [-1, 1] to UV rect [icon_uv.xy, icon_uv.zw]
    let uv_local = p * 0.5 + 0.5; // map to [0, 1]
    
    // Scale icon padding to nest cleanly inside the shape
    let icon_padding = 0.20;
    let uv_scaled = (uv_local - 0.5) * (1.0 + icon_padding) + 0.5;
    
    if (uv_scaled.x >= 0.0 && uv_scaled.x <= 1.0 && uv_scaled.y >= 0.0 && uv_scaled.y <= 1.0) {
        let uv = mix(in.icon_uv.xy, in.icon_uv.zw, uv_scaled);
        let icon_texel = textureSample(icon_atlas, icon_sampler, uv);
        
        // Render icon clipped cleanly inside border
        let icon_alpha = icon_texel.a * border_mask;
        color = mix(color, icon_texel.rgb, icon_alpha);
    }
    
    return vec4<f32>(color, fill_mask * in.opacity);
}
