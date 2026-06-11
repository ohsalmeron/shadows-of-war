# sow-web

Browser-facing assets for Shadows of War (HTML/JS only). Game logic is in the WASM client (`sow-client` / `sow-ui`).

| Subfolder | Product | Deploy target |
|-----------|---------|---------------|
| **`site/`** | Marketing — landing, privacy, terms | `shadowsofwar.io` (via `./sow prod`) |
| **`shell/`** | WASM loader, `index.html.template`, portal SDK | `dist/play`, `dist/ptr`, `dist/crazygames` |

## Build & deploy (`./sow`)

All packaging goes through **`./sow`**. Output stays under `dist/`.

```bash
./sow cg
./sow prod -v
./sow ptr -v
./sow local
```

| Command | Writes | Notes |
|---------|--------|-------|
| `cg` / `crazygames` | `dist/crazygames/` | Portal `.br` shell + embedded `assets/static/`; upload whole folder |
| `prod` / `p` | `dist/play/` → VPS | play.shadowsofwar.io + marketing + **prod server** |
| `ptr` | `dist/ptr/` → VPS | PTR shell + **PTR server only** |
| `local` / `l` | `dist/site-dev/www/` | Local iframe QA; prod wss/CDN at runtime |

**CDN:** only `assets/cdn/` rsyncs to `https://shadowsofwar.io/assets/cdn/` (parallel with WASM on `prod` / `ptr` / `cg`). Not copied into play/ptr dist shells. **`local` skips CDN sync.**

## Dual entry: iframe embed + play subdomain

| URL | Experience |
|-----|------------|
| **`https://shadowsofwar.io/`** | Marketing HTML; **Play** embeds the game shell in-page (desktop) or fullscreen iframe (mobile ≤480px) |
| **`https://play.shadowsofwar.io/`** | Full game shell, auto-load WASM — share link / “Open full screen” |

Marketing [`site/index.html`](site/index.html) + [`site/game-embed.js`](site/game-embed.js) lazy-load an iframe to `play.shadowsofwar.io` (prod) or `/game/index.html` (`local`). No WASM on the marketing page itself.

```bash
./sow local   # http://127.0.0.1:8787/ — optional --build-only, --port
```

**Desktop:** Play shows a CrazyGames-style player frame above About; marketing stays visible. **Mobile (≤480px):** Play opens a fullscreen overlay; **Back to site** clears the iframe.

`prod` / `cg` / `ptr` use separate `dist/*` trees; `local` never overwrites them.

**Runtime:** Loader and WASM use `SOW_ASSETS_URL` → prod CDN for boot UI and leader portraits.

## Legal copy

Privacy and Terms body text for the marketing site **and** in-game Settings modals lives in [`site/legal/*.en.toml`](site/legal/privacy.en.toml). Edit those TOML files first; keep [`site/privacy/index.html`](site/privacy/index.html) and [`site/terms/index.html`](site/terms/index.html) in sync manually (HTML files note the TOML source in a comment).

## `dist/crazygames/` layout

- `index.html` (loads `sow_client.js.br` / `sow_client_bg.wasm.br`)
- `assets/static/` — fonts, icons, HUD sources, **one offline map** (`maps/world/`)
- `sdk/`, favicons
- No `assets/cdn/` — CrazyGames loads art from prod CDN

## CrazyGames QA (layout & resize)

**Sizing:** WASM uses `window.innerWidth` / `innerHeight`. Drag-resize on play or CrazyGames should grow and shrink without letterboxing gaps.

**Native dev:** `cargo run -p sow-client` → prod VPS wss. Boot art from CDN at runtime.

**CDN art:** Leader portraits, boot UI webp, and avatars under `/assets/cdn/` on shadowsofwar.io.

## CrazyGames Basic Launch checklist

1. `./sow cg` → upload entire `dist/crazygames/` folder in the Developer Portal.
2. Run the portal **QA tool** (SDK init, loading, gameplayStart, user, updateRoom).
3. Submit lobby sizes (FFA / Teams max players) in upload metadata.
4. Test with two logged-in CG accounts: instant multiplayer host, friend join via CG UI, full match, **PLAY AGAIN** in a private room.
5. Protocol/server changes ship with `./sow p -v` (rebuilds server when `sow-server` / `sow-relay` / `sow-database` crates changed).
6. Ads stay off for Basic Launch (`SOW_ENABLE_PORTAL_ADS` is never set in the build).

## CrazyGames Developer Portal form

| Field | Select |
|-------|--------|
| Game engine | HTML5 |
| Save progress | Yes, using the **Data Module** from the CrazyGames SDK |
| Mobile | Yes — orientation **LANDSCAPE** (or BOTH) |
| Online multiplayer | Yes — lobby min **1**, max **8** |
| Invite link & button | Yes |
| IsInstantMultiplayer | Yes |
| SDK mute audio | Yes |

**CSP / frame-ancestors:** not required for HTML5 uploads (CrazyGames hosts the package). Only needed for iframe games loaded from your own domain.

**Sitelock:** enforced in [`shell/sdk/store_portals.js`](shell/sdk/store_portals.js) for `SOW_PORTAL=crazygames` builds (regional `crazygames.*` domains + `game-files.crazygames.com`).

**Cloud profile `/api/profile` 400 in QA preview:** harmless until `./sow p -v` deploys the current `sow-database` (guest/local saves use SDK Data module until the player signs in on CrazyGames).

## Search Console (SEO)

After verifying the property in [Google Search Console](https://search.google.com/search-console):

1. **Sitemaps** → submit `sitemap.xml` (3 URLs: `/`, `/privacy`, `/terms`).
2. **URL Inspection** → `https://shadowsofwar.io/` → Test live URL → **Request indexing**.

Live checks: `robots.txt` and `sitemap.xml` at site root. `./sow prod` runs sitemap verification after deploy.

## Quick start

```bash
./sow local
./sow cg
./sow prod -v
```

**Docs:** [CrazyGames SDK](https://docs.crazygames.com/sdk/)
