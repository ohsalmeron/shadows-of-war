# VFX: Zero-Cost Radial Effects

## How We Got Here

The map renderer uses a single fragment shader pass (`map.wgsl`) that already evaluates every visible pixel. Any math we add to that pass is "free" in the sense that:

- No extra draw calls
- No extra textures or buffers
- No extra GPU memory
- No CPU-side per-pixel work

The only cost is ALU cycles on pixels that pass the early-exit check. Pixels outside the radius skip entirely.

## The Core Technique

Pass an array of `vec4<f32>` slots as uniforms. Each slot encodes a positioned radial event:

```
slot.x = center_x (hex column, float)
slot.y = center_y (hex row, float)
slot.z = radius (world units)
slot.w = packed metadata (IDs, type flags)
```

In the shader, convert center to world coordinates, compute distance from current pixel, and derive a `threat` value (0..1) via `smoothstep`. Then apply any visual recipe gated by `threat > 0`.

```wgsl
let dist = distance(world_pos, front_world);
let threat = 1.0 - smoothstep(0.0, radius, dist);
if threat <= 0.0 { continue; } // Free skip for unaffected pixels
```

## Why It's Negative Cost

1. **No new pass.** The fragment shader already runs on every pixel. We're adding branched math to an existing loop.
2. **Early exit.** Empty slots (`radius <= 0`) and out-of-range pixels (`threat <= 0`) skip all work.
3. **Uniform data.** Slots live in the uniform buffer — no texture fetch, no buffer read. The GPU has this data in registers.
4. **Scales with events, not map size.** 0 active events = 0 extra ALU. 8 events = 8 distance checks, but only on pixels within radius.

## Packing Metadata Into `slot.w`

f32 can represent integers exactly up to 2^24 (16,777,216). We use this for packing multiple IDs:

```
slot.w = id_a * 1024.0 + id_b
```

Unpacking in WGSL:
```wgsl
let packed = u32(slot.w);
let id_a = packed / 1024u;
let id_b = packed % 1024u;
```

This works because our IDs are < 1024 (10 bits each), so `max_packed = 1023 * 1024 + 1023 = 1,048,575` — well under 2^24.

To add a type field: `type * 1048576 + id_a * 1024 + id_b` (type in bits 20+).

## Visual Recipes

A "recipe" is just the math applied when `threat > 0`. Examples from War Fog:

| Layer | What It Does | Cost |
|-------|-------------|------|
| Desaturation | `mix(color, grey(luminance), threat * 0.6)` | 1 dot + 1 mix |
| Smoke gradient | `mix(dark, bright, threat²)` | 1 mul + 1 mix |
| Ripple waves | `sin(dist * freq - time * speed)` | 1 sin + 1 mul |
| Corona ring | `smoothstep(width, 0, abs(dist - offset))` | 1 abs + 1 smoothstep |

Each layer is 1-3 ALU ops. The full War Fog stack is ~12 ops per affected pixel.

## Adding a New Effect

### Rust side (CPU)

1. In the render loop, fill a slot:
```rust
slots[i] = [center_x, center_y, radius, packed_ids];
```

2. That's it. No new structs, no new textures.

### WGSL side (GPU)

Add a branch inside the slot loop, gated by your type/condition:

```wgsl
if my_condition {
    // Your visual recipe here
    base_color = mix(base_color, effect_color, intensity);
}
```

## Current Slot Layout

```
event_slots: array<vec4<f32>, 8>
```

**Rust struct:** `MapGlobals.threat_slots: [[f32; 4]; 8]`

**Active effects:**

| Slot Use | Condition | Visual |
|----------|-----------|--------|
| War Fog | `target_id > 0` | Attacker-colored smoke, desaturation, ripples, corona |
| Frontier Glow | `target_id == 0` | Golden-green expansion aura, soft edge |

## Conquest Flash (Texture-Packed)

Separate from the slot system. Uses bits 16..23 of the R32Uint owner texture to store a per-tile flash byte (0..255). Decays by 4 per frame via sparse tracking on CPU. The shader reads it alongside owner_id:

```wgsl
let owner_packed = textureLoad(owner_texture, coords, 0).x;
let owner_id = owner_packed & 0xFFFFu;
let flash_val = f32((owner_packed >> 16u) & 0xFFu) / 255.0;
```

This is for per-tile effects (conquest shockwave). The radial slot system is for area effects.

## CPU canvas-Based Hybrid VFX (egui overlay)

While GPU fragment shaders handle massive per-pixel background effects (e.g. War Fog, territory glows), the CPU egui canvas layer (`painter`) is perfect for rendering localized, high-fidelity dynamic visual overlays like plasma laser beams, particle bursts, and volumetric mushroom clouds.

By combining the two layers, we achieve spectacular premium fidelity with **zero performance degradation**:
1. **GPU Shader**: Renders wide radial area distortions, fog-of-war coverage, and background flashes.
2. **CPU Canvas Overlay**: Stretches custom vector shapes, crackling lightning segment lines, and moving physics particle sprays on top.

---

## Custom Premium Visual Recipes

Below are the key visual algorithms implemented in our codebase to create "AAA-feel" premium visual feedback.

### 1. Volumetric Lobe Billowing (Fluffy Volumetric Clouds)
Instead of drawing a single flat circle for a cloud or explosion fireball, construct it by drawing a cluster of **overlapping circular lobes** offset in a staggered ring around the center and animated over time.

```rust
let num_lobes = 7;
for i in 0..num_lobes {
    // Stagger lobe angles symmetrically
    let angle = (i as f32 * (360.0 / num_lobes as f32) + elapsed * 60.0).to_radians();
    // Ease-out expansion outward from core
    let lobe_dist = cap_radius * 0.26 * (1.0 - (1.0 - p).powi(2));
    let lobe_center = cap_center + egui::vec2(angle.cos(), angle.sin()) * lobe_dist;
    let lobe_radius = cap_radius * (0.65 + (i % 3) as f32 * 0.08);

    // Layer 1: Outer dark billowing smoke
    painter.circle_filled(lobe_center, lobe_radius, smoke_color);
}
```
* **Visual Result**: High-density volumetric shape with natural irregular billowing curves that feel alive.
* **Tiers of Depth**: Draw three concentric layered passes (dark outer smoke, bright middle fireball, white-hot core) using this lobe cluster strategy.

### 2. High-Frequency Crackling Electrical Conduit (Lightning/Plasma Arcs)
To make laser/plasma beams feel high-energy and unstable, divide the linear segment into a series of steps and apply a dynamic perpendicular offset to each step using high-frequency sine/cosine wave overlays.

```rust
let steps = 8;
let dir = end_point - start_point;
let length = dir.length();
let perp = egui::vec2(-dir.y, dir.x) / length;
let mut prev_pt = start_point;

for step in 1..=steps {
    let t = step as f32 / steps as f32;
    let mut pt = start_point + dir * t;
    if step < steps {
        // Double-frequency sine offset crackle
        let offset_mag = (elapsed * 45.0 + step as f32 * 1.6).sin() * 5.0
            + (elapsed * 95.0 - step as f32 * 2.3).cos() * 2.5;
        pt += perp * offset_mag;
    }
    // Draw segmented laser line (Glowing outer stroke + thin white core)
    painter.line_segment([prev_pt, pt], egui::Stroke::new(6.0, glow_color));
    painter.line_segment([prev_pt, pt], egui::Stroke::new(1.2, egui::Color32::WHITE));
    prev_pt = pt;
}
```

### 3. Multi-Spiked Blinding Lens Flares
Give initial tactical detonations instant impact by drawing a blinding white flash overlaid with high-contrast screen-aligned horizontal, vertical, and diagonal needle-thin lens spikes.

```rust
let flare_len = max_radius * 5.0 * zoom_scaled * (1.0 - p / 0.15);
let flare_stroke = egui::Stroke::new(4.5 * (1.0 - p / 0.15), egui::Color32::WHITE);
// Horizontal needle
painter.line_segment([pos2(cx - flare_len, cy), pos2(cx + flare_len, cy)], flare_stroke);
// Diagonal needle
let diag = flare_len * 0.7;
painter.line_segment([pos2(cx - diag, cy - diag), pos2(cx + diag, cy + diag)], flare_stroke);
```

### 4. Parabolic Physics Spark/Ember Sprays
Create rich debris feedback by spraying glowing sparks from the impact site. Simulate gravity-driven parabolic arcs directly in coordinate space without maintaining a complex particle heap.

```rust
let num_sparks = 18;
for i in 0..num_sparks {
    let spark_angle = (i as f32 * (360.0 / num_sparks as f32)).to_radians();
    let speed = 35.0 + (i % 4) as f32 * 18.0;
    let t_sec = elapsed * 1.6;
    
    // Parabolic trajectory (x = horizontal drift, y = vertical speed + gravity pull)
    let spark_x = center.x + spark_angle.cos() * speed * t_sec * zoom_scaled;
    let spark_y = center.y + (spark_angle.sin() * speed * t_sec + 22.0 * t_sec * t_sec) * zoom_scaled;
    
    painter.circle_filled(pos2(spark_x, spark_y), 2.2, egui::Color32::WHITE);
    painter.circle_filled(pos2(spark_x, spark_y), 4.0, glow_color);
}
```

---

## Key Files

- [map.wgsl](file:///home/bizkit/Documents/GitHub/shadows-of-war/sow-render/src/shaders/map.wgsl) — all shader effects
- [map_renderer.rs](file:///home/bizkit/Documents/GitHub/shadows-of-war/sow-render/src/map_renderer.rs) — MapGlobals struct, R32Uint packing, flash decay
- [render/mod.rs](file:///home/bizkit/Documents/GitHub/shadows-of-war/sow-client/src/render/mod.rs) — slot filling from game state
- [layer4_5_effects.rs](file:///home/bizkit/Documents/GitHub/shadows-of-war/sow-client/src/render/world/layer4_5_effects.rs) — Mushroom clouds, lens flares, spark physics
- [layer3_buildings.rs](file:///home/bizkit/Documents/GitHub/shadows-of-war/sow-client/src/render/world/layer3_buildings.rs) — Bunker crackling plasma laser weapons

