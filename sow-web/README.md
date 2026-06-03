# sow-web

Browser-facing assets for Shadows of War (HTML/JS only). Game logic is in the WASM client (`sow-client` / `sow-ui`).

| Subfolder | Product | Deploy target |
|-----------|---------|---------------|
| **`site/`** | Marketing — landing, privacy, terms | `shadowsofwar.io` (manual / separate) |
| **`shell/`** | WASM loader, `index.html.template`, portal SDK | `dist/play`, `dist/ptr`, `dist/crazygames` |

## Build & deploy (`sow-dist`)

All packaging goes through the Rust CLI. Output stays under `dist/`.

```bash
cargo run -p sow-dist -- cg
cargo run -p sow-dist -- cg -v            # portal release (increment .version)
cargo run -p sow-dist -- play
cargo run -p sow-dist -- ptr
```

| Command | Writes | Notes |
|---------|--------|-------|
| `cg` (`crazygames`) | `dist/crazygames/` | Portal `.br` shell + `assets/static/`; upload whole folder |
| `play` | `dist/play/` → VPS | Self-hosted shell + `assets/static/`; boot/leaders from prod CDN |
| `ptr` | `dist/ptr/` → VPS | Same as play on staging host |

**CDN:** `assets/cdn/` is synced only to `https://shadowsofwar.io/assets/cdn/` (parallel with WASM build). It is **not** copied into `dist/*`.

**Runtime:** Loader and WASM use `SOW_ASSETS_URL` → prod CDN for boot UI and leader portraits.

## `dist/crazygames/` layout

- `index.html` (loads `sow_client.js.br` / `sow_client_bg.wasm.br`)
- `assets/static/` — fonts, maps metadata, UI sources bundled for portal
- `sdk/`, favicons
- No `assets/cdn/` — CrazyGames loads art from prod CDN

## CrazyGames QA (layout & resize)

**Sizing:** On fullscreen shells (play, CrazyGames), WASM uses `window.innerWidth`/`innerHeight` when canvas width matches the viewport (avoids stale `clientHeight` after resize). When `#blade` is materially **narrower** than the viewport (site embed), WASM uses canvas `clientWidth`/`clientHeight`.

**Native dev:** `cargo run --bin sow-client` loads boot splash/loader from `assets/static/ui/` or `assets/cdn/ui/` at runtime (those files may only exist under `cdn/` after CDN prep). Boot splash preloads the random main-menu leader portrait from CDN (not Caesar by default).

**Layout:** UI compact mode uses width and height together: wide-short desktop player frames (e.g. ~900×520) use **desktop** menu layout; portrait phones (`width < 480`) stay compact.

| Check | Pass criteria |
|-------|----------------|
| Desktop QA layout | Main menu is horizontal (side panel), not a full-height vertical scroll stack |
| Resize | Drag the browser window on self-hosted `play` or CG QA; map and UI reflow smoothly |
| Portrait phone | Narrow width still uses compact stack + mobile leader art |

## Quick start

```bash
cargo run -p sow-dist -- crazygames
cargo run -p sow-dist -- play
```

**Docs:** [CrazyGames SDK](https://docs.crazygames.com/sdk/)
