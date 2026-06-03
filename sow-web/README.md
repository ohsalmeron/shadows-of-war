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

## Legal copy

Privacy and Terms body text for the marketing site **and** in-game Settings modals lives in [`site/legal/*.en.toml`](site/legal/privacy.en.toml). Edit those TOML files first; keep [`site/privacy/index.html`](site/privacy/index.html) and [`site/terms/index.html`](site/terms/index.html) in sync manually (HTML files note the TOML source in a comment).

## `dist/crazygames/` layout

- `index.html` (loads `sow_client.js.br` / `sow_client_bg.wasm.br`)
- `assets/static/` — fonts, maps metadata, UI sources bundled for portal
- `sdk/`, favicons
- No `assets/cdn/` — CrazyGames loads art from prod CDN

## CrazyGames QA (layout & resize)

**Sizing:** WASM uses `window.innerWidth` / `innerHeight` (same as pre-embed builds). Drag-resize on play or CrazyGames should grow and shrink without letterboxing gaps; `SurfaceResized` drives the GPU surface.

**Native dev:** `cargo run --bin sow-client` loads boot splash/loader from `assets/static/ui/` or `assets/cdn/ui/` at runtime (those files may only exist under `cdn/` after CDN prep). Boot splash preloads the random main-menu leader portrait from CDN (not Caesar by default). Leader rail avatars load from `SOW_ASSETS_URL/cdn/avatars/` on wasm (not embedded in the binary).

**CDN art:** Leader portraits, boot UI webp, and avatars are under `/assets/cdn/` on shadowsofwar.io; portal zips do not ship `assets/cdn/`.

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
