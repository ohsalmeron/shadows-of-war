# sow-web

Browser-facing assets for Shadows of War (HTML/JS only). Game logic is in the WASM client (`sow-client` / `sow-ui`).

| Subfolder | Product | Deploy target |
|-----------|---------|---------------|
| **`site/`** | Marketing — landing, privacy, terms | `shadowsofwar.io` (via `sow-dist play`) |
| **`shell/`** | WASM loader, `index.html.template`, portal SDK | `dist/play`, `dist/ptr`, `dist/crazygames` |

## Build & deploy (`sow-dist`)

All packaging goes through the Rust CLI. Output stays under `dist/`.

```bash
cargo run -p sow-dist -- cg
cargo run -p sow-dist -- cg -v            # portal release (increment .version)
cargo run -p sow-dist -- play
cargo run -p sow-dist -- ptr
cargo run -p sow-dist -- localsite          # local marketing + iframe embed (alias: ls)
```

| Command | Writes | Notes |
|---------|--------|-------|
| `cg` (`crazygames`) | `dist/crazygames/` | Portal `.br` shell + `assets/static/`; upload whole folder |
| `play` | `dist/play/` → VPS | Fullscreen shell on `play.shadowsofwar.io`; marketing site on `shadowsofwar.io` |
| `ptr` | `dist/ptr/` → VPS | Same as play on staging host |
| `localsite` (`ls`) | `dist/site-dev/www/` | **Local only:** build + serve; iframe → `/game/`; prod wss/maps/CDN at runtime |

**CDN:** `assets/cdn/` is synced only to `https://shadowsofwar.io/assets/cdn/` (parallel with WASM build on `play` / `ptr` / `cg`). It is **not** copied into `dist/*`. **`localsite` skips CDN sync** so local rebuilds never touch the VPS.

## Dual entry: iframe embed + play subdomain

| URL | Experience |
|-----|------------|
| **`https://shadowsofwar.io/`** | Marketing HTML; **Play** embeds the game shell in-page (desktop) or fullscreen iframe (mobile ≤480px) |
| **`https://play.shadowsofwar.io/`** | Full game shell, auto-load WASM — share link / “Open full screen” |

Marketing [`site/index.html`](site/index.html) + [`site/game-embed.js`](site/game-embed.js) lazy-load an iframe to `play.shadowsofwar.io` (prod) or `/game/index.html` (localsite). No WASM on the marketing page itself.

```bash
cargo run -p sow-dist -- localsite   # or: ls — builds, then http://127.0.0.1:8787/
```

**Desktop:** Play shows a CrazyGames-style player frame above About; marketing stays visible. **Mobile (≤480px):** Play opens a fullscreen overlay; **Back to site** clears the iframe. Optional: `--build-only`, `--port 8787`.

Cloud `play` / `cg` / `ptr` use separate `dist/*` trees; `localsite` never overwrites them.

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

## Search Console (SEO)

After verifying the property in [Google Search Console](https://search.google.com/search-console):

1. **Sitemaps** → submit `sitemap.xml` (3 URLs: `/`, `/privacy`, `/terms`). Use **OPEN SITEMAP** in GSC to confirm raw XML loads in the browser.
2. If GSC shows **Sitemap could not be read** from an old submission: **Remove sitemap** → wait a minute → **Add a new sitemap** → `sitemap.xml` → wait 24–48h for re-read.
3. **URL Inspection** → `https://shadowsofwar.io/` → Test live URL → **Request indexing**.
4. Optional: [Rich Results Test](https://search.google.com/test/rich-results) and [PageSpeed Insights](https://pagespeed.web.dev/) on `/` (before clicking Play).
5. After deploy, re-request indexing if marketing HTML changed.

Live checks: `robots.txt` and `sitemap.xml` at site root (`Content-Type: application/xml`); home page has `VideoGame` JSON-LD, canonical, and no WASM until Play. `cargo run -p sow-dist -- play` runs sitemap verification after deploy.

## Quick start

```bash
cargo run -p sow-dist -- localsite   # local embed QA
cargo run -p sow-dist -- crazygames
cargo run -p sow-dist -- play
```

**Docs:** [CrazyGames SDK](https://docs.crazygames.com/sdk/)
