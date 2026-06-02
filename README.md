# Shadows of War

**[shadowsofwar.io](https://shadowsofwar.io)** — Free, open-source MMORTS: world maps, civilizations, alliances, expansion, and economy. Play on web or native desktop and mobile.

Rust workspace: shared game logic for web (WASM) and native clients.

**License:** [AGPL-3.0-or-later](LICENSE)

## Features

- **MMORTS gameplay:** Territory control, structures, diplomacy, and large-scale battles
- **Civilizations & leaders:** Nations, tribes, and leader identity on world maps
- **Alliances:** Coordinate with other players for defense and expansion
- **Multiplayer & single-player:** Online matches and offline play
- **Cross-platform:** Web, native desktop, iOS, Android
- **Map tools:** In-game editor, real-world regions, import pipeline

## Structure

| Crate / path | Role |
|--------------|------|
| `sow-core` | Deterministic simulation (WASM-safe) |
| `sow-net` | Wire protocol (`bincode`, message envelopes) |
| `sow-server` | Matchmaking / lobby server |
| `sow-relay` | Relay for games |
| `sow-render` | GPU map pipeline (`blade-graphics`, WGSL) |
| `sow-ui` | Menus and HUD (`egui`) |
| `sow-i18n` | Localized UI strings (`strings/{en,es}/`) |
| `sow-client` | Game executable (native + WASM) |
| `sow-map` | Map editor + generation |
| `sow-tools` | CLI: OSM bbox, heightmap import |
| `sow-web/site/` | Marketing site (static HTML: landing, privacy, terms) |
| `sow-web/shell/` | Game shell (WASM loader, index template, portal SDK) |
| `assets/` | All shipped art (maps, UI, icons, fonts) |
| `scripts/sow.sh` | Build, local dev, deploy (`local`, `crazygames`, `cloud`, …) |
| `deploy/` | nginx VPS configs, Android/iOS shells, release keystore (local) |
| `docs/leaders/` | Leader AI dossier (12 regions, chronological); see `docs/leaders/README.md` |

## Web hosts

| Host | Role |
|------|------|
| `shadowsofwar.io` | Static marketing site (links to play subdomain) |
| `play.shadowsofwar.io` | Production game shell (auto-load WASM) |
| `ptr.shadowsofwar.io` | Staging game shell |
| `shadowsofwar.io/assets`, `/maps`, `/ws` | Shared CDN for all shells |

## Commands (copy-paste)

Each web pipeline writes its **own** folder under `dist/`. WASM shells do not copy static files; **`assets/static`** is a **symlink** to repo [`assets/static/`](assets/static/) in `dist/crazygames/` (and `local` uses the same for dev). Streamed leaders/maps load via client CDN URLs.

| Command | Output | Purpose |
|---------|--------|---------|
| `./scripts/local.sh` | `dist/play/` + assets symlink | Browser game at http://127.0.0.1:8080 |
| `./scripts/native.sh` | *(no `dist/`)* | Rust server + 2 clients (fast logic debug) |
| `./scripts/crazygames.sh` | `dist/crazygames/` | Portal upload (always rebuilds; `--sync-cdn` refreshes prod leaders) |
| `./scripts/poki.sh` | `dist/poki/` | Poki portal upload folder |
| `./scripts/cloud-game.sh` | `dist/play/` → VPS | Production play.shadowsofwar.io |
| `./scripts/ptr.sh` | `dist/ptr/` → VPS | Staging ptr.shadowsofwar.io |
| `./scripts/cloud.sh` | `dist/play/` + marketing | Full prod (incremental; `--force` to redeploy) |
| `./scripts/sow.sh site` | *(none)* | Marketing pages at http://127.0.0.1:8787 |
| `./scripts/android.sh webview` | APK | Android WebView build |

Equivalent without wrappers: `./scripts/sow.sh local`, `crazygames`, `poki`, `cloud-game`, `ptr`, `cloud`, `native`, `site`.

**Try locally**

```bash
./scripts/local.sh
# open http://127.0.0.1:8080/
```

**Try CrazyGames build**

```bash
./scripts/crazygames.sh          # always rebuilds dist/crazygames/
./scripts/crazygames.sh --sync-cdn   # also push streamed leaders to prod CDN first
# upload everything inside dist/crazygames/ (assets/static is a symlink)
```

**Ship production play**

```bash
./scripts/cloud-game.sh
```

Details: [sow-web/README.md](sow-web/README.md).

## License

Shadows of War source is licensed under the [GNU Affero General Public License v3.0 or later](LICENSE).

Copyright holder and third-party notices: [docs/legal/COPYRIGHT](docs/legal/COPYRIGHT) and [docs/legal/NOTICE](docs/legal/NOTICE).

**Upstream:** Portions of this codebase derive from [OpenFrontIO](https://github.com/openfrontio/OpenFrontIO) (© OpenFront Inc. and Contributors, AGPL-3.0-or-later). See [LICENSE](LICENSE) for full terms.

### Attribution policy

| Surface | What to show |
|---------|----------------|
| In-game UI | `© Shadows of War`, AGPL line, **Based on OpenFront**, links to source and [NOTICE](docs/legal/NOTICE) (Credits modal + main menu) |
| Marketing site (`sow-web/site/`) | Brand footer + OpenFront + AGPL + source link — no personal name |
| Repo legal files (`docs/legal/`) | Full copyright holder name (required when conveying source) |

Do not put the copyright holder’s personal name on marketing copy, social bios, or landing hero text.

## Contributing

See [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md).
