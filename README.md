# Shadows of War

**[shadowsofwar.io](https://shadowsofwar.io)** — Online MMORTS. Expand territory, lead civilizations, and compete with rival nations and tribes on world maps. Form alliances, choose leaders, and fight for control in the browser or on desktop.

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
| `sow-client` | Game executable (native + WASM) |
| `sow-map` | Map editor + generation |
| `sow-tools` | CLI: OSM bbox, heightmap import |
| `sow-site` | Leptos SSR (landing + legal pages) |
| `assets/` | All shipped art (maps, UI, icons, fonts) |
| `web/` | Browser shell and portal SDK hooks |
| `scripts/sow.sh` | Build, deploy, package |
| `deploy/nginx/` | VPS nginx site configs (synced by `sow.sh deploy`) |
| `legal/` | COPYRIGHT, NOTICE, NOTICE.deps |
| `docs/` | Contributor guide, VFX notes, leader reference |

## Web hosts

| Host | Role |
|------|------|
| `shadowsofwar.io` | Leptos landing + legal (links to play subdomain) |
| `play.shadowsofwar.io` | Production game shell (auto-load WASM) |
| `ptr.shadowsofwar.io` | Staging game shell |
| `shadowsofwar.io/assets`, `/maps`, `/ws` | Shared CDN for all shells |

Local dev: `./scripts/sow.sh site` (landing) and `./scripts/sow.sh play` (game shell on port 8080).

## License

Shadows of War source is licensed under the [GNU Affero General Public License v3.0 or later](LICENSE).

Copyright (c) 2024–2026 Omar Hernandez Salmeron. See [COPYRIGHT](legal/COPYRIGHT) and [NOTICE](legal/NOTICE).

**Upstream:** Portions of this codebase derive from [OpenFrontIO](https://github.com/openfrontio/OpenFrontIO) (© OpenFront LLC and Contributors, AGPL-3.0-or-later). Required notices appear in [NOTICE](legal/NOTICE) and in-game **Credits**—not in game marketing. See [LICENSE](LICENSE) for full terms.

## Contributing

See [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md).
