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
| `assets/` | Art sources (`static/`, published `cdn/`) |
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
./sow ptr -v        # staging: PTR shell + cdn/ (+ nixos-rebuild if server changed)
./sow prod -v       # prod: play + marketing + cdn/ (+ nixos-rebuild if server changed)
./sow p -v          # same as prod
./sow cg            # CrazyGames dist + cdn/
./sow local         # local WASM QA → prod wss/CDN
./sow l             # same as local
```

Native client (no local server): `cargo run -p sow-client` → VPS `wss://shadowsofwar.io/ws/`.

| Env var | Purpose |
|---------|---------|
| `SOW_IN_NIX_SHELL=1` | Set inside Nix shell (auto via `./sow`) |
| `SOW_NO_NIX=1` | Skip auto `nix develop` (host tools on PATH) |

**VPS:** **NixOS** — fully declarative in `flake.nix` → `nixosConfigurations.vps` (`nix/nixos/vps/`). nginx, valkey, prod + PTR systemd, and server binaries (flake packages) are applied with `./sow infra` or automatically when server crates change during `./sow prod` / `./sow ptr`. **Prod** and **PTR** are separate stacks (ports 25565/25566 vs 25575/25576). Shared Valkey on 6379.

**Replace Debian on existing GCE VM** (`35.239.160.167`):

1. Add your SSH public key to [`nix/nixos/vps/authorized_keys`](nix/nixos/vps/authorized_keys)
2. Optional: back up maps under `/home/bizkit/shadowsofwar*/assets/maps` and static web under `/var/www/` (content is re-pushed by `./sow` anyway)
3. From repo root: `./sow infra` — on Debian runs **nixos-anywhere** with `.#vps-install` (repartitions disk, ~5–15 min downtime); on NixOS runs `nixos-rebuild`
4. After reboot: `./sow ptr -v` then `./sow prod -v` — seed WASM, marketing, CDN, maps

**New VPS from scratch:** same flake; use `./nix/nixos/gce-image.sh` or any NixOS install, then `./sow infra` → `./sow ptr -v` → `./sow prod -v`.

Server packages (`nix build .#packages.x86_64-linux.sow-server`) use a trimmed workspace (no `blade/` required). WASM/client builds still use the full repo via `./sow` devShell.

## Commands

| Command | Output | VPS content | NixOS infra |
|---------|--------|-------------|-------------|
| `infra` | — | — | nixos-anywhere (first install) or `nixos-rebuild switch` |
| `cg` / `crazygames` | `dist/crazygames/` | CDN rsync only | No |
| `p` / `prod` | `dist/play/` + marketing | play + shadowsofwar.io | If server crates changed |
| `ptr` | `dist/ptr/` | ptr.shadowsofwar.io | If server crates changed |
| `l` / `local` | `dist/site-dev/` | localhost only | No |

```bash
./sow cg -v
./sow prod -v
./sow ptr -v
./sow local --build-only
```

Each **prod** / **ptr** run ships WASM dist and triggers `nixos-rebuild` when `sow-server` / `sow-relay` / `sow-core` / `sow-net` changed (same `.version` bump). Deploy verifies maps API, WebSocket proxy, and systemd — not just static HTML.

Details: [sow-web/README.md](sow-web/README.md).

### Asset pipelines

| Pipeline | Source | Destination |
|----------|--------|-------------|
| CDN (parallel on cg/prod/ptr) | `assets/cdn/` only | `shadowsofwar.io/html/assets/cdn/` |
| WASM dist | `sow-web/shell` + compiled client | `dist/play`, `dist/ptr`, or `dist/crazygames` |
| Static in dist | `assets/static/` embedded | `dist/crazygames/assets/static/` only |
| Maps on server | `assets/static/maps/` | VPS maps dir (prod/ptr content rsync) |
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
| `sow-server` / `sow-relay` | Yes → NixOS store paths | Yes (`nixos-rebuild` when crates changed) |

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
