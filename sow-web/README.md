# sow-web

Browser-facing assets for Shadows of War (HTML/JS only). Game logic lives in `sow-client` / `sow-ui` (Rust).

| Subfolder | Product | Deploy target |
|-----------|---------|---------------|
| **`site/`** | Marketing website — landing, privacy, terms (static HTML) | `shadowsofwar.io` nginx `html/` |
| **`shell/`** | Game shell — WASM loader, `index.html.template`, SW, portal SDK | `dist/game-shell/` via `./scripts/sow.sh package` → play / ptr / CrazyGames |

**Rules for agents**

- Never embed WASM or game boot code in `site/`.
- Never put privacy/terms or marketing copy in `shell/`.
- Do not conflate this folder with `sow-client` (the Rust game binary).

**Local dev**

```bash
./scripts/sow.sh site   # sow-web/site on :8787
./scripts/sow.sh play   # dist/game-shell/ on :8080 (after package)
```
