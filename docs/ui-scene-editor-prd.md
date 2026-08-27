# PRD — UI Scene Editor (web authoring → codegen → 1:1 egui)

Status: **proposed / MVP scoping**
Owner: Omar
Last updated: 2026-06-30

---

## 0. The one-paragraph version

Authoring egui UI by hand is slow (recompile per pixel) and unpleasant for someone who
thinks in Unity's hierarchy/inspector/drag model. We build a **fast, pure-JavaScript web
editor** to lay out UI visually — no compile, instant feedback — that on **Export generates
Rust code** calling our *existing* theme components. You compile **only when you're done**, run
the real client, and see the **exact 1:1** result. The browser is the fast sketchpad; the
compiled client is the source of truth. No egui removal, no WASM-in-browser, no rewrite of the
UI crate.

---

## 1. Problem & goals

**Problem.** egui is immediate-mode imperative Rust. Nudging a label 2px, recoloring a panel,
or re-spacing a row all require editing Rust and recompiling. There is no visual canvas, no
hierarchy, no inspector. For UI-heavy iteration this is the bottleneck.

**Top requirement (non-negotiable).** *Fast JS prototyping; compile only when done; then see
1:1.* Anything that reintroduces a compile into the inner loop (e.g. embedded WASM viewport) is
out.

**Goals.**
- G1 — Lay out a screen visually in the browser with **zero compile** in the loop.
- G2 — Produce a result that is **exactly 1:1** with the real game when compiled.
- G3 — **No rewrite** of the existing UI crate or client; changes are additive and small.
- G4 — Reuse the **existing theme system** (palette, radius, frames, glow text) so authored UI
  inherits the real look automatically.

**Non-goals (MVP).**
- Not removing or replacing egui.
- Not making the browser preview itself pixel-perfect (it is an approximation for layout).
- Not retrofitting existing hand-written panels (leaderboard, HUD) into the editor.
- Not editing game logic/behavior — layout & static styling only.

---

## 2. Why codegen (the decision, recap)

Audited the stack:
- egui already renders 100% on GPU via our `blade-egui` fork; egui is the UI renderer, not a
  positioning layer.
- A panel's whole visual surface is a tiny param set: `Frame { fill, stroke, corner_radius,
  inner_margin, outer_margin, shadow }` ([frame.rs](../egui/crates/egui/src/containers/frame.rs)).
- But our actual look lives in a **theme layer** — `palette::*`, `radius::*`, `panel_frame()`,
  `leaderboard_panel_frame()`, `paint_premium_glow_text()`, `interact_card()`,
  `paint_count_badge()` ([panels.rs](../sow-ui/src/kit/theme/panels.rs),
  [palette](../sow-ui/src/kit/theme)) — much of it imperative, not declarative style.

Therefore: JS can be **1:1 only by emitting calls to those existing parameterized components**,
compiled by the real client. That is codegen. The alternative (a Rust interpreter reading data)
also works but adds an ongoing Rust interpreter to maintain and can only render what it's taught;
codegen leans on code we already have and is 1:1 by construction.

---

## 3. What you can do at the end of the MVP (capabilities)

After the MVP you can, **without writing Rust**:

1. **Open the editor in a browser** (`./sow ui` → serves the tool, like `./sow m` does for the
   campaign editor).
2. **Create a scene** (a named screen, e.g. `pause_menu`).
3. **Add components from a palette**: Panel, Text, Row, Column, Image (atlas sprite), Spacer.
4. **Place & size** them — drag in the canvas (absolute) or nest them in Row/Column (flow).
5. **Style via inspector**, mapped to your theme tokens:
   - Panel: fill (palette dropdown or hex), outline (stroke width + color), corner radius
     (XS/SM/MD/LG or custom), inner/outer margin, shadow (none/preset), or pick a prebuilt frame
     (`hud_panel_frame`, `leaderboard_panel_frame`, `panel_frame(kind)`).
   - Text: content, size, color (palette), and a "glow" toggle → `paint_premium_glow_text`.
6. **See an approximate live preview** in the browser (CSS/canvas) as you edit — instant.
7. **Reparent / reorder / rename / delete** nodes via a **hierarchy** tree.
8. **Save** → writes `assets/editor/ui/<scene>.json` (the source of truth) via `/__save`.
9. **Export → Compile → Run**: generates `sow-client/src/ui_scene/generated/<scene>.rs`, then
   `./sow` builds & launches the client in **scene-preview mode** showing the screen **1:1**.
10. **Round-trip**: reopen the scene later from its JSON and keep editing.

**Acceptance demo:** build a pause-menu mock (panel + title text + two labeled buttons) entirely
in the browser, hit Export+Run, and the compiled client shows it pixel-faithful to a hand-written
version — with no Rust typed by hand.

---

## 4. Feature list

### 4.1 MVP (must-have)
| # | Feature | Notes |
|---|---------|-------|
| F1 | Web editor shell | Pure JS/HTML, served by `./sow ui`. Mirrors `tools/campaign-editor/`. |
| F2 | Canvas viewport (approximate) | Renders nodes with CSS/canvas; for layout, not 1:1. |
| F3 | Hierarchy panel | Tree of nodes; select / reorder / reparent / rename / delete. |
| F4 | Inspector panel | Per-node fields driven by a component manifest. |
| F5 | Component palette | Panel, Text, Row, Column, Image, Spacer. |
| F6 | Absolute placement | Drag to (x,y) → `egui::Area::fixed_pos`. |
| F7 | Flow placement | Nesting in Row/Column → `ui.horizontal/vertical`. |
| F8 | Theme-token styling | Palette/radius/stroke pickers map to `theme::*`. |
| F9 | Scene JSON save/load | `assets/editor/ui/<scene>.json` via existing `/__save`. |
| F10 | Rust codegen | Emit `ui_scene/generated/<scene>.rs` calling theme + egui. |
| F11 | Scene-preview mode | Dev-gated client mode renders a chosen generated scene. |
| F12 | `./sow ui` command | Serve tool + save endpoint + "export & run". |
| F13 | Generated-scene compile test | A test ensures generated modules compile & a sample scene round-trips. |

### 4.2 vNext (deferred, model leaves room)
- Emoji/avatar nodes (route through `TextRenderer` GPU path).
- Custom components beyond the catalog (badges, cards, progress bars).
- Interaction/state (buttons that *do* something) — currently static layout only.
- Responsive rules (compact/portrait breakpoints already in theme).
- Two-way sync: edit generated `.rs` and re-import (hard; intentionally not supported).
- Animations / transitions.
- A tiny data-interpreter fallback for hot-reload-without-compile (optional alt mode).

---

## 5. Architecture & data flow

```
┌── BROWSER (fast, no compile) ─────────────────────────────┐
│  tools/ui-editor/  (pure JS/HTML)                          │
│   Hierarchy │ Canvas preview (approx) │ Inspector          │
│        └──────── edits ────────► scene model (in JS) ──────┤
│  Save  → POST /__save?file=ui/<scene>.json                 │
│  Export→ POST /__export?scene=<scene>  (gen .rs + build)   │
└───────────────────────────────────────────────────────────┘
            │ JSON (source of truth)        │ triggers codegen+build
            ▼                               ▼
   assets/editor/ui/<scene>.json        sow-client/src/ui_scene/generated/<scene>.rs
                                            │  (calls theme::* + egui, real code)
                                            ▼
                          REAL CLIENT (scene-preview mode) → 1:1 result
```

- **Source of truth = `assets/editor/ui/<scene>.json`.** The `.rs` is a *generated artifact*
  (regenerate, never hand-edit) — same discipline as `boudica.json` → no second hand-written
  source to drift ([boudica.rs](../sow-client/src/campaign/boudica.rs)).
- **The generator** is small and lives in `sow-dist` (Rust) or in the JS tool; recommended in
  `sow-dist` so it shares types and runs server-side on Export.

---

## 6. Component catalog (JS component → existing Rust)

The editor only exposes components that already exist as parameterized Rust, so output is 1:1:

| Editor component | Generated Rust |
|---|---|
| Panel (preset) | `theme::hud_panel_frame()` / `leaderboard_panel_frame()` / `panel_frame(kind, compact)` `.show(ui, ...)` |
| Panel (custom) | `egui::Frame::new().fill(..).stroke(Stroke::new(w, ..)).corner_radius(radius::md()).inner_margin(..).shadow(..)` |
| Text | `ui.label(RichText::new(s).size(sz).color(theme::palette::text_normal()))` |
| Text (glow) | `theme::paint_premium_glow_text(painter, pos, s, sz, shadow, color)` |
| Row / Column | `ui.horizontal(|ui| {..})` / `ui.vertical(|ui| {..})` |
| Image | atlas sprite via existing sprite path |
| Color value | `theme::palette::surface()` … `neon_cyan()`, `neon_gold()`, `field_bg()`, `danger()`, custom `Color32::from_rgba_unmultiplied(r,g,b,a)` |
| Radius | `theme::radius::{xs,sm,md,lg}()` or `CornerRadius::same(n)` |
| Stroke width | `theme::stroke::{HAIRLINE,EMPHASIS}` or literal |

**Single manifest** (`tools/ui-editor/manifest.json`) describes each component's editable fields
*and* its codegen template. Adding a component = manifest entry + (if new) one parameterized Rust
fn. The inspector and the generator both read this manifest → no divergence.

---

## 7. Codegen contract

Each scene generates one module:

```rust
// GENERATED from assets/editor/ui/pause_menu.json — do not edit by hand.
use sow_ui_kit::theme;
pub fn render(ctx: &egui::Context) {
    egui::Area::new("pause_menu.panel".into())
        .fixed_pos(egui::pos2(640.0, 360.0))
        .show(ctx, |ui| {
            theme::hud_panel_frame().show(ui, |ui| {
                ui.label(egui::RichText::new("Paused").size(28.0)
                    .color(theme::palette::text_normal()));
                // children…
            });
        });
}
```

- A generated `mod.rs` registers scenes: `pub fn render(name: &str, ctx: &egui::Context)`.
- The client's **scene-preview mode** calls `ui_scene::generated::render(active_scene, ctx)`.

---

## 8. Implementation plan (milestones)

**M0 — Contract & skeleton (no UI yet).**
- Define scene JSON schema + the manifest format.
- `sow-client/src/ui_scene/generated/mod.rs` registry + a hand-written `sample.rs` proving the
  call site renders in a dev-gated **scene-preview mode** (new `ClientPhase` branch or dev flag;
  dispatch beside [app.rs:97-123](../sow-ui/src/app.rs)).
- Compile test that `sample` renders.
- *Exit:* `./sow` can show a generated-style scene in preview mode.

**M1 — Codegen for one component (Panel + Text).**
- Generator (in `sow-dist`) reads `assets/editor/ui/<scene>.json` + manifest → emits `<scene>.rs`.
- `/__export` route in [serve.rs](../sow-dist/src/serve.rs): generate → `cargo run` (reuse the
  existing `/__launch` build path).
- *Exit:* a hand-authored JSON → Export → 1:1 panel+text in the client.

**M2 — JS editor shell (the fast loop).**
- Clone `tools/campaign-editor/` → `tools/ui-editor/`: canvas + hierarchy + inspector.
- Manifest-driven inspector; Panel + Text + Row/Column; absolute drag + flow nesting.
- Save → `/__save?file=ui/<scene>.json`.
- *Exit:* build a 3-element screen in the browser, Save, Export, Run → 1:1.

**M3 — Catalog widen + polish.**
- Image/Spacer; preset frames; glow text; palette/radius/stroke pickers; rename/reorder/delete.
- `./sow ui` command wraps serve + open.
- *Exit:* the §3 acceptance demo (pause-menu mock) passes.

---

## 9. File-level change map (additive, small)

**New:**
- `tools/ui-editor/` — the JS tool (index.html + logic.html), clone of campaign-editor.
- `sow-client/src/ui_scene/mod.rs` + `generated/{mod.rs,<scene>.rs}` — registry + generated code.
- `assets/editor/ui/<scene>.json` — scene sources.
- `tools/ui-editor/manifest.json` — component fields + codegen templates.

**Touched (minimal):**
- `sow-dist/src/serve.rs` — generalize `/__save` path to `ui/…`; add `/__export`.
- `sow-dist/src/main.rs` — add `ui` subcommand.
- One dispatch site in the client (`ClientPhase` / dev flag) to enter scene-preview mode.
- `sow-ui/src/kit/theme/*` — **only** to wrap any inline effect we want as a component into a
  parameterized `pub fn` (small, one-time per component; most already exist).

**Not touched:** existing panels, HUD, menus, sim, render hot loops. (Honors the surgical-scope
and tutorial-isolation discipline.)

---

## 10. The dev loop (how you actually use it)

1. `./sow ui` → browser opens the editor.
2. Drag/edit/style — **instant**, approximate preview, no compile.
3. **Save** often → `assets/editor/ui/<scene>.json`.
4. When happy → **Export & Run** → generates `.rs`, compiles, launches → **1:1 truth**.
5. Tweak more in browser; Export again only when you want another 1:1 check.

---

## 11. What-ifs / edge cases / risks & mitigations

| What-if | Answer / mitigation |
|---|---|
| **Browser preview ≠ compiled result** | Expected — preview is for layout, not fidelity. Keep CSS tokens aligned to palette/radius so drift is small; the compile is the truth. If a component can't be previewed faithfully (glow), show a labeled placeholder box at correct bounds. |
| **A look I want isn't a callable component** | Wrap it once as a parameterized `theme::` fn, add a manifest entry. After that JS emits it freely. This is the main recurring cost — by design. |
| **Someone hand-edits the generated `.rs`** | It's overwritten on next Export. Generated files carry a "do not edit" header and (optionally) live under a `generated/` dir gitignored or clearly marked. JSON is the only source of truth. |
| **Generated code doesn't compile** | The Export build surfaces the error; M0's compile test guards the registry. Generator emits only from the manifest's vetted templates, so malformed output is unlikely. |
| **egui is immediate-mode (no absolute coords natively)** | Absolute nodes use `egui::Area::fixed_pos`; flow nodes use `ui.horizontal/vertical`. The scene model tags each node absolute|flow. |
| **WASM/web build invariants** | Untouched — we don't add to the render/sim hot paths or change wasm flags; scene-preview is a dev-gated egui branch. (Respects the documented wasm landmines.) |
| **Scope creep into behavior/state** | MVP is static layout only. Buttons render but don't act. Interaction is vNext. |
| **Two editors of truth (campaign vs ui)** | Different domains, same pattern; share the `/__save` + serve infra, separate tool dirs. |
| **Coordinate space / DPI** | Author in logical points (egui's space); preview uses the same logical grid; the client renders at real DPI like any egui UI. |
| **Reading existing panels into the editor** | Not supported (they're imperative Rust). Editor authors *new* scenes; porting an old panel means rebuilding it in the tool. |

---

## 12. Verification / acceptance

- **Unit/build:** `cargo build -p sow-client`; generated-scene compile test (M0); JSON schema
  round-trip test (load→save→load is stable).
- **Loop test:** author `pause_menu.json` in the browser → Save → confirm file written → Export
  → client launches in scene-preview mode showing the screen.
- **1:1 test:** build the same screen by hand in egui and side-by-side compare with the generated
  one — must match (the demo in §3).
- **Isolation test:** scene-preview mode is unreachable in normal play (dev-gated), and no new
  branch lands in sim/income/render hot loops.

---

## 13. Open decisions (need a call before/at M0)

1. **Generator location:** `sow-dist` (Rust, shares types, runs on Export) — recommended — vs in
   the JS tool (no Rust roundtrip but duplicates templates). *Recommend Rust.*
2. **Preview-mode entry:** new `ClientPhase::SceneEditorPreview` vs a dev flag inside an existing
   phase. *Recommend a dev flag first (smaller), promote to a phase if it grows.*
3. **Generated dir under version control?** Commit (reviewable, ships in wasm) vs gitignore
   (treat as build output). *Recommend commit*, like `boudica.json`, so web/offline always has it.
4. **Layout default:** start nodes as absolute (Unity-feel) vs flow (egui-native). *Recommend
   absolute for the canvas feel; flow via explicit Row/Column.*
