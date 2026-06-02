# sow-web

Browser-facing assets for Shadows of War (HTML/JS only). Game logic lives in `sow-client` / `sow-ui` (Rust).

| Subfolder | Product | Deploy target |
|-----------|---------|---------------|
| **`site/`** | Marketing website — landing, privacy, terms (static HTML) | `shadowsofwar.io` nginx `html/` |
| **`shell/`** | Game shell — WASM loader, `index.html.template`, SW, portal SDK | `dist/game-shell/` via `./scripts/sow.sh package` → play / ptr / CrazyGames / Poki |

**SEO** — Keyword spine: MMORTS, free-to-play, open-source, world maps, civilizations, alliances, expansion, economy. Edit `site/index.html` meta + hero; play shell mirrors meta in `shell/index.html.template` (run `package` after shell changes). Hidden `<h1>`/`<p>` in the shell is for portal upload scrapers (Poki, CrazyGames). `robots.txt` and `sitemap.xml` live in `site/`.

**Portals** — `./scripts/sow.sh package crazygames` injects CrazyGames SDK; `./scripts/sow.sh package poki` sets `SOW_PORTAL=poki` boot only (host provides PokiSDK).

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

Build the portal zip (injects CrazyGames SDK v3 + `SOW_PORTAL=crazygames` boot vars):

```bash
./scripts/sow.sh package crazygames
```

Output: `shadows-of-war-crazygames.zip` at repo root. Upload that archive to CrazyGames dev portal.

**Before submit — manual QA**

| Check | Pass criteria |
|-------|----------------|
| Boot | `SOW_portalLoadStart` → WASM loads → `load_stop` when main menu appears |
| SDK init | `SOW_initPortalSdk()` runs; no console errors from CrazyGames SDK |
| Gameplay ads | `gameplayStart` when entering a match; `gameplayStop` when returning to main menu |
| Service worker | Not registered on portal hosts (shell skips SW when `SOW_PORTAL` or crazygames hostname) |
| Solo | Single-player skirmish works offline |
| Multiplayer | Main menu connects to `wss://shadowsofwar.io/ws/`; can join a lobby |
| Fullscreen | Game playable fullscreen; Ctrl/Cmd+W does not close tab while fullscreen |
| Escape | Does not deselect HUD tools during play (use **Q** to clear building/nuke selection) |
| Tutorial | Overlay only after main-menu **TUTORIAL** button (never on lobby/multiplayer by default) |
| Listing copy | Hidden `<h1>`/`<p>` in `shell/index.html.template` matches solo + online MMORTS |
| Covers | Use `assets/ui/sow-splash-desktop.webp` / `sow-splash-mobile.webp` — genre-accurate MMORTS art |

**Official docs**

- [Quality guidelines](https://docs.crazygames.com/requirements/quality-guidelines/)
- [Requirements](https://docs.crazygames.com/requirements/)
- [SDK](https://docs.crazygames.com/sdk/)

**Extras on request:** Leader AI dossier path is noted in `assets/SOURCES.toml` (`ai_dossier`).
