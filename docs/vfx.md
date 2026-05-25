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

## Key Files

- [map.wgsl](file:///home/bizkit/Documents/GitHub/shadows-of-war/sow-render/src/shaders/map.wgsl) — all shader effects
- [map_renderer.rs](file:///home/bizkit/Documents/GitHub/shadows-of-war/sow-render/src/map_renderer.rs) — MapGlobals struct, R32Uint packing, flash decay
- [render/mod.rs](file:///home/bizkit/Documents/GitHub/shadows-of-war/sow-client/src/render/mod.rs) — slot filling from game state
