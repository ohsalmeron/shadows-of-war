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
| `sow-dist` | WASM `dist/` build + deploy; `nix/nixos/vps/` (NixOS VPS); Android/iOS shells in `sow-dist/deploy/` |
| `sow-web/site/` | Marketing site (static HTML: landing, privacy, terms) |
| `sow-web/shell/` | Game shell (WASM loader, index template, portal SDK) |
| `assets/` | Art sources (`static/`, online `maps/`, published `cdn/`) |
| `docs/leaders/` | Leader AI dossier (12 regions, chronological); see `docs/leaders/README.md` |

## Web hosts

| Host | Role |
|------|------|
| `shadowsofwar.io` | Marketing site + shared CDN (`/assets/cdn/`). **Play in browser:** [shadowsofwar.io](https://shadowsofwar.io/) |
| `play.shadowsofwar.io` | Full-screen game shell (share link) |
| `ptr.shadowsofwar.io` | Staging game shell |

## Nix + `./sow`

Install [Nix](https://github.com/DeterminateSystems/nix-installer) (or `direnv allow` — `.envrc` uses `use flake`). Then run **`./sow`** — it enters the Nix shell automatically and uses a cached release binary (no `cargo run` every time).

```bash
./sow infra         # apply NixOS VPS config (nginx, systemd, server binaries)
./sow ptr -v        # staging: PTR shell + cdn/
./sow prod -v       # prod: play + marketing + cdn/
./sow p -v          # same as prod
./sow cg            # CrazyGames dist + cdn/
./sow local         # local WASM QA → prod wss/CDN
./sow l             # same as local
```

Native client (no local server): `cargo run -p sow-client` → production WebSocket endpoint.

| Env var | Purpose |
|---------|---------|
| `SOW_IN_NIX_SHELL=1` | Set inside Nix shell (auto via `./sow`) |
| `SOW_NO_NIX=1` | Skip auto `nix develop` (host tools on PATH) |

Server packages (`nix build .#packages.x86_64-linux.sow-server`) use a trimmed workspace (no `blade/` required). WASM/client builds still use the full repo via `./sow` devShell.

## Commands

| Command | Output | VPS content | NixOS infra |
|---------|--------|-------------|-------------|
| `infra` | — | — | nixos-anywhere (first install) or `nixos-rebuild switch` |
| `cg` / `crazygames` | `dist/crazygames/` | CDN rsync only | No |
| `p` / `prod` | `dist/play/` + marketing | play + shadowsofwar.io | No |
| `ptr` | `dist/ptr/` | ptr.shadowsofwar.io | No |
| `l` / `local` | `dist/site-dev/` | localhost only | No |

```bash
./sow cg -v
./sow prod -v
./sow ptr -v
./sow local --build-only
```

Each **prod** / **ptr** run ships WASM dist and verifies maps API, WebSocket proxy, and systemd. Server binaries and nginx/systemd changes go through `./sow infra` only.

Details: [sow-web/README.md](sow-web/README.md).

### Asset pipelines

| Pipeline | Source | Destination |
|----------|--------|-------------|
| CDN (parallel on cg/prod/ptr) | `assets/cdn/` only | `shadowsofwar.io/html/assets/cdn/` |
| WASM dist | `sow-web/shell` + compiled client | `dist/play`, `dist/ptr`, or `dist/crazygames` |
| Static in dist | `assets/static/` (fonts, icons — **not maps**) | `dist/crazygames/assets/static/` only |
| Maps (online) | `assets/maps/` | VPS maps dir → `/maps/` HTTP API (prod/ptr rsync) |
| Maps (offline) | `assets/static/maps/world/` only | Bundled inside client WASM |
| Server binaries | flake `packages.sow-server` / `sow-relay` | NixOS systemd store paths (via `./sow infra`) |
| Marketing HTML | `sow-web/site/` | `shadowsofwar.io/html/` (via `sow prod`) |

Boot UI and leader portraits load from CDN at runtime for play/ptr shells (shell-only dist).

### What `sow prod` updates

| Artifact | Picked up? | Infra reload? |
|----------|------------|---------------|
| Game WASM shell | Yes → play.shadowsofwar.io | No |
| Marketing site | Yes → shadowsofwar.io | No |
| CDN (`assets/cdn/`) | Yes (parallel build) | No |
| Map files | Yes → prod maps dir | No |
| `sow-server` / `sow-relay` | No (use `./sow infra`) | Yes (`./sow infra`) |

`sow ptr` updates PTR shell + PTR server only — never restarts prod.

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
