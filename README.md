# Shadows of War

**[shadowsofwar.io](https://shadowsofwar.io)** — Online MMORTS. Expand territory, lead civilizations, and compete with rival nations and tribes on world maps. Form alliances, choose leaders, and fight for control in the browser or on desktop.

Rust workspace: shared game logic for web (WASM) and native clients.

**License:** [AGPL-3.0-or-later](LICENSE) · **Play:** [shadowsofwar.io/play/](https://shadowsofwar.io/play/) · **Source:** [github.com/ohsalmeron/shadows-of-war](https://github.com/ohsalmeron/shadows-of-war)

## License

Shadows of War source is licensed under the [GNU Affero General Public License v3.0 or later](LICENSE).

Copyright (c) 2024–2026 Omar Hernandez Salmeron. See [COPYRIGHT](COPYRIGHT) and [NOTICE](NOTICE).

**Upstream:** Portions of this codebase derive from [OpenFrontIO](https://github.com/openfrontio/OpenFrontIO) (© OpenFront LLC and Contributors, AGPL-3.0-or-later). Required notices appear in [NOTICE](NOTICE) and in-game **Credits**—not in game marketing. See [LICENSE](LICENSE) for full terms.

## Features

- **MMORTS gameplay:** Territory control, structures, diplomacy, and large-scale battles
- **Civilizations & leaders:** Nations, tribes, and leader identity on world maps
- **Alliances:** Coordinate with other players for defense and expansion
- **Multiplayer & single-player:** Online matches and offline play
- **Cross-platform:** Web, native desktop, iOS, Android
- **Map tools:** In-game editor, real-world regions, import pipeline

## Prerequisites

- **Rust** (latest stable via `rustup`)
- **Python 3** (local cluster script)
- **Vulkan / GPU drivers** (native client)
- **Vendored forks** for a from-scratch clone: `egui/`, `winit/`, `blade/` at commits in [NOTICE](NOTICE)

Web/deploy builds also need on `PATH`: `cargo`, `wasm-bindgen`, `brotli`, `cwebp` (optional: `wasm-opt` from binaryen).

## Installation

```bash
git clone https://github.com/ohsalmeron/shadows-of-war.git
cd shadows-of-war
```

Install Rust targets as needed (`rustup target add wasm32-unknown-unknown` for web builds).

## Running the game

### Local development (fastest)

Native server + two clients (debug):

```bash
./scripts/sow.sh l
# or
./scripts/run_cluster.py
```

### Web / production builds

All build and deploy flows go through **`scripts/sow.sh`**:

| Command | Alias | Use |
|---------|-------|-----|
| `local` | `l` | Debug native server + clients |
| `ptr` | `p` | Staging → ptr.shadowsofwar.io |
| `cloud` | `c` | Production site + `/play/` WASM |
| `package` | `pkg` | Portal zip (CrazyGames) |
| `site` | — | Landing/legal SSR only (`sow-site`) |
| `android` | `a` | APK (`n` native / `w` webview) |

Examples:

```bash
./scripts/sow.sh l          # local multiplayer smoke test
./scripts/sow.sh site       # SSR dev (/, /privacy, /terms)
./scripts/sow.sh pkg        # dist/play + portal zip
./scripts/sow.sh c          # full production deploy
```

After `pkg`, smoke-test the web shell: `cd dist/play && python -m http.server 8080`.

### iOS

Open [`ios/sow_ios.xcodeproj`](ios/sow_ios.xcodeproj), set signing Team, run on device. One-time: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios`.

## Development tools

```bash
cargo build -p sow-client          # native client
cargo test -p sow-core             # simulation tests (core changes must be tested)
cargo run -p sow-tools -- --help   # map CLI
```

**Rules for contributors**

- `sow-core` hot path (`engine.tick()`): no heap allocation in the tick loop
- `sow-core` changes: add or update tests
- GPU resources: destroy via `map_renderer.destroy` after `context.wait_for` on inflight work

See [CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md).

## Project structure

| Crate / path | Role |
|--------------|------|
| `sow-core` | Deterministic simulation (WASM-safe) |
| `sow-net` | Wire protocol (`bincode`, message envelopes) |
| `sow-server` | Matchmaking / lobby server |
| `sow-relay` | Relay for production |
| `sow-render` | GPU map pipeline (`blade-graphics`, WGSL) |
| `sow-ui` | Menus and HUD (`egui`) |
| `sow-client` | Game executable (native + WASM) |
| `sow-map` | Map editor + generation |
| `sow-tools` | CLI: OSM bbox, heightmap import |
| `sow-site` | Leptos SSR (marketing, legal) |
| `web/` | Browser shell and portal SDK hooks |
| `assets/maps/` | Shipped maps (`northamerica` in-repo) |
| `scripts/sow.sh` | Build, deploy, package |

## Map authoring

Runtime maps are **`map.bin`** (+ `map.bin.br`) per region. Three ways to author:

| Method | How |
|--------|-----|
| **Paint** | In-game Map Editor → Compile & Export |
| **Heightmap** | `cargo run -p sow-tools -- import-openfront --input <folder> --name <name>` |
| **OSM bbox** | `cargo run -p sow-tools -- --bbox "min_lon,min_lat,max_lon,max_lat" --name <name> --scale 1000` |

OSM-derived maps must credit [© OpenStreetMap contributors](https://www.openstreetmap.org/copyright) (ODbL). Do not bulk-cache `tile.openstreetmap.org`. Record bbox in `assets/maps/SOURCES.toml`.

Hand-painted / heightmap-only maps have no OSM obligation unless they include OSM geometry.

## Store & deploy notes

- **CrazyGames:** `./scripts/sow.sh package` → `shadows-of-war-crazygames.zip`; privacy URL https://shadowsofwar.io/privacy
- **AGPL source:** production runs `sow-server` + `sow-relay`; matching source at git tag in `SOW_BUILD_VERSION` / Credits link
- **AI art:** splash, avatars, leader portraits (Gemini, Meta AI, Midjourney)—see `assets/SOURCES.toml` and `leaders.md` for store disclosure
- **Launch:** push repo, tag version (`git push origin v$(cat .version)`), deploy with `./scripts/sow.sh cloud`

```bash
git clone https://github.com/ohsalmeron/shadows-of-war.git
cd shadows-of-war
cargo build --release -p sow-server -p sow-relay
```

## Contributing

Contributions welcome. Open an issue for large changes. Keep PRs focused; test `sow-core` when touching simulation.

Fork → branch → PR. Maintainer review required before merge.
