# sow-web

Browser-facing assets for Shadows of War (HTML/JS only). Game logic lives in `sow-client` / `sow-ui` (Rust).

| Subfolder | Product | Deploy target |
|-----------|---------|---------------|
| **`site/`** | Marketing website — landing, privacy, terms (static HTML) | `shadowsofwar.io` nginx `html/` |
| **`shell/`** | Game shell — WASM loader, `index.html.template`, SW, portal SDK | `dist/game-shell/` via `./scripts/sow.sh package` → play / ptr / CrazyGames / Poki |

**SEO** — Keyword spine: MMORTS, free-to-play, open-source, world maps, civilizations, alliances, expansion, economy. Edit `site/index.html` meta + hero; play shell mirrors meta in `shell/index.html.template` (run `package` after shell changes). Hidden `<h1>`/`<p>` in the shell is for portal upload scrapers (Poki, CrazyGames). `robots.txt` and `sitemap.xml` live in `site/`.

**Portals** — `./scripts/sow.sh package crazygames` injects CrazyGames SDK; `./scripts/sow.sh package poki` sets `SOW_PORTAL=poki` boot only (host provides PokiSDK).

**Assets layout** — `assets/static/` ships in `dist/game-shell` (fonts, maps, HUD, loaders on self-hosted play). `assets/streamed/` is CDN-only (leader portraits). Portal zips omit streamed art and local boot UI (CDN via `SOW_ASSETS_URL`). `./scripts/sow.sh package crazygames` (and `poki`) rsyncs streamed leaders to prod and verifies URLs before building the zip.

**Rules for agents**

- Never embed WASM or game boot code in `site/`.
- Never put privacy/terms or marketing copy in `shell/`.
- Do not conflate this folder with `sow-client` (the Rust game binary).

**Local dev**

```bash
./scripts/sow.sh site   # sow-web/site on :8787
./scripts/sow.sh play   # dist/game-shell/ on :8080 (after package)
```

## CrazyGames submission checklist

Build the CrazyGames shell (injects SDK v3 + `SOW_PORTAL=crazygames` boot vars):

```bash
./scripts/sow.sh package crazygames
```

**CrazyGames upload:** the dev portal does **not** accept `.zip` files. `./scripts/sow.sh package crazygames` writes only to `dist/game-shell/`. Select every file and folder inside that directory (Ctrl+A) and upload them so `index.html` is at the upload root—not the `game-shell` folder name alone.

**Before submit — manual QA**

| Check | Pass criteria |
|-------|----------------|
| Boot | `SOW_initPortalSdk()` → `SOW_portalLoadStart` → WASM loads → `load_stop` when main menu appears |
| SDK init | Console shows `CrazyGames SDK init OK (env=…)`; `crazygames-sdk-v3.js` in built `index.html` head (no `async`) |
| First gameplay start (QA auto) | In preview, start **solo skirmish** or **tutorial** once after main menu — `gameplayStart` only fires on match enter, not on menu |
| Loading start/stop (QA auto) | Console: SDK loading start/stop before main menu |
| Basic Launch ads | No `requestAd` until Full Launch (`SOW_ENABLE_PORTAL_ADS` stays off); `hasAdblock` still runs |
| Gameplay hooks | `gameplayStart` when entering a match; `gameplayStop` when returning to main menu |
| Service worker | Not registered on portal hosts (shell skips SW when `SOW_PORTAL` or crazygames hostname) |
| Solo | Single-player skirmish works offline |
| Multiplayer | Main menu connects to `wss://shadowsofwar.io/ws/`; can join a lobby |
| Fullscreen | Game playable fullscreen; Ctrl/Cmd+W does not close tab while fullscreen |
| Escape | Does not deselect HUD tools during play (use **Q** to clear building/nuke selection) |
| Tutorial | Overlay only after main-menu **TUTORIAL** button (never on lobby/multiplayer by default) |
| Listing copy | Hidden `<h1>`/`<p>` in `shell/index.html.template` matches solo + online MMORTS |
| Covers | Use `assets/static/ui/sow-splash-desktop.webp` / `sow-splash-mobile.webp` |
| CDN art | Leaders + portal boot UI stream from `shadowsofwar.io/assets/streamed/` and `/assets/static/ui/` — not in portal upload |

**Official docs**

- [Quality guidelines](https://docs.crazygames.com/requirements/quality-guidelines/)
- [Requirements](https://docs.crazygames.com/requirements/)
- [SDK](https://docs.crazygames.com/sdk/)

**Extras on request:** Leader AI dossier path is noted in `assets/SOURCES.toml` (`ai_dossier`).
