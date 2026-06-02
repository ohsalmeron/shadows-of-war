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
| `sow-i18n` | Localized UI strings (`strings/{en,es}/`) |
| `sow-client` | Game executable (native + WASM) |
| `sow-map` | Map editor + generation |
| `sow-tools` | CLI: OSM bbox, heightmap import |
| `sow-web/site/` | Marketing site (static HTML: landing, privacy, terms) |
| `sow-web/shell/` | Game shell (WASM loader, index template, portal SDK) |
| `assets/` | All shipped art (maps, UI, icons, fonts) |
| `scripts/sow.sh` | Build, deploy, package |
| `deploy/` | nginx VPS configs, Android/iOS shells, release keystore (local) |
| `docs/leaders/` | Leader AI dossier (12 regions, chronological); see `docs/leaders/README.md` |

## Web hosts

| Host | Role |
|------|------|
| `shadowsofwar.io` | Static marketing site (links to play subdomain) |
| `play.shadowsofwar.io` | Production game shell (auto-load WASM) |
| `ptr.shadowsofwar.io` | Staging game shell |
| `shadowsofwar.io/assets`, `/maps`, `/ws` | Shared CDN for all shells |

Local dev: `./scripts/sow.sh site` (static `sow-web/site` at :8787) and `./scripts/sow.sh play` (fullscreen game shell at :8080 after `package`). The marketing site only links to the game; it does not embed WASM.

Deploy: `./scripts/sow.sh cloud` (full), `cloud-game` (WASM + play host + backend), or `cloud-site` (SSR landing only). First-time play host needs DNS `play` → VPS, then `cloud-game` runs certbot for `play.shadowsofwar.io` if the certificate is missing.

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
