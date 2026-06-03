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
| `sow-dist` | WASM `dist/` build + deploy; `sow-dist/deploy/` (nginx, Android/iOS shells, keystores) |
| `sow-web/site/` | Marketing site (static HTML: landing, privacy, terms) |
| `sow-web/shell/` | Game shell (WASM loader, index template, portal SDK) |
| `assets/` | Art sources (`static/`, published `cdn/`) |
| `docs/leaders/` | Leader AI dossier (12 regions, chronological); see `docs/leaders/README.md` |

## Web hosts

| Host | Role |
|------|------|
| `shadowsofwar.io` | Marketing + shared CDN (`/assets/cdn/`, fonts, maps API) |
| `play.shadowsofwar.io` | Production game shell |
| `ptr.shadowsofwar.io` | Staging game shell |

## Commands (`sow-dist`)

Output under `dist/`. **CDN** (`assets/cdn/`) syncs to `shadowsofwar.io` in parallel with each build; **dist folders** get `assets/static/` only (no `assets/cdn/`).

```bash
cargo run -p sow-dist -- cg              # crazygames package (alias)
cargo run -p sow-dist -- cg -v           # portal release (+ .version)
cargo run -p sow-dist -- play            # deploy (keeps current .version)
cargo run -p sow-dist -- play -v         # increment .version for this deploy
cargo run -p sow-dist -- ptr
cargo run -p sow-dist -- ptr -v
```

Release binary: `cargo build -p sow-dist --release` → `./target/release/sow-dist play`

| Command | Output | Purpose |
|---------|--------|---------|
| `crazygames` / `cg` | `dist/crazygames/` | Portal zip (WASM `.br` + `assets/static/`) |
| `play` | `dist/play/` → VPS | Production play.shadowsofwar.io |
| `ptr` | `dist/ptr/` → VPS | Staging ptr.shadowsofwar.io |

Details: [sow-web/README.md](sow-web/README.md).

### Asset pipelines

| Pipeline | Source | Destination |
|----------|--------|-------------|
| CDN (every deploy, parallel) | `assets/cdn/` | `shadowsofwar.io/html/assets/cdn/` (rsync) |
| WASM dist | `sow-web/shell` + compiled client | `dist/play`, `dist/ptr`, or `dist/crazygames` |
| Static in dist | `assets/static/` | `dist/*/assets/static/` |
| Maps on server | `assets/static/maps/` | VPS sow-server maps dir (play/ptr deploy) |

Boot UI and leader portraits load from the CDN URL at runtime, not from files inside `dist/`.

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
