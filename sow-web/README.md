# sow-web

Browser-facing assets for Shadows of War (HTML/JS only). Game logic is in the WASM client (`sow-client` / `sow-ui`).

| Subfolder | Product | Deploy target |
|-----------|---------|---------------|
| **`site/`** | Marketing — landing, privacy, terms | `shadowsofwar.io` (`cloud-site`) |
| **`shell/`** | WASM loader, `index.html.template`, portal SDK | `dist/play`, `dist/ptr`, `dist/crazygames`, `dist/poki` |

## Commands (copy-paste)

| Script | Writes | Notes |
|--------|--------|-------|
| `./scripts/local.sh` | `dist/play/` | WASM shell; `assets/static` symlink (local dev) |
| `./scripts/crazygames.sh` | `dist/crazygames/` | Always rebuilds; `.br` WASM/JS + SDK + `assets/static` symlink |
| `./scripts/crazygames.sh --sync-cdn` | same | Also sync streamed leaders to prod CDN first |
| `./scripts/poki.sh` | `dist/poki/` | Poki portal (full rebuild + CDN prereq) |
| `./scripts/cloud-game.sh` | `dist/play/` → VPS | Prod play host (always full deploy) |
| `./scripts/cloud.sh` | `dist/play/` + site → VPS | Full prod (incremental; `--force` to redeploy) |

## `dist/crazygames/` layout

- `index.html`, `sow_client_*.wasm.br`, `sow_client_*.js.br`, `sdk/`, favicons
- `assets/static` → symlink to repo `assets/static/` (not copied)

Streamed portraits and maps: client `SOW_ASSETS_URL` / `SOW_MAPS_URL` (prod CDN). No VPS sync on a normal `crazygames.sh` run.

**Upload:** all files inside `dist/crazygames/`. If the portal rejects symlinks:

```bash
rsync -aL dist/crazygames/ /tmp/cg-upload/
```

## Quick start

```bash
./scripts/crazygames.sh
./scripts/local.sh    # http://127.0.0.1:8080/
./scripts/cloud-game.sh
```

**Docs:** [CrazyGames SDK](https://docs.crazygames.com/sdk/)
