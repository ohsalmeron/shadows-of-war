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
| `sow-dist` | WASM `dist/` build + deploy; Fedora VPS templates in `sow-dist/deploy/`; Android/iOS shells in `sow-dist/deploy/` |
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

## `./sow`

Requires host **Rust**, **gcloud** (OS Login), and **rsync** on your PATH. `./sow` builds a cached release binary under `dist/.cargo-target/` (no Nix required for deploy).

### Setup

```bash
cp .env.example .env
# Edit .env: SOW_GCP_PROJECT, origins, certbot email (see .env.example)
```

Remote deploy (`./sow p`, `./sow ptr`, `./sow infra`) reads `.env` from the repo root (gitignored). Without it, `./sow` prints what to set.

```bash
./sow infra --confirm-destroy   # one-time: recreate Fedora VPS on GCP (nginx, TLS, valkey)
./sow ptr -v                    # staging: PTR shell + PTR server + cdn/
./sow prod -v                   # prod: play + marketing + prod server + cdn/
./sow p -v                      # same as prod
./sow cg                        # CrazyGames dist + cdn/
./sow local                     # local WASM QA → prod wss/CDN
./sow l                         # same as local
```

Native client (no local server): `cargo run -p sow-client` → production WebSocket endpoint.

| Env var | Purpose |
|---------|---------|
| `SOW_GCP_PROJECT` | GCP project ID (**required** for remote deploy) |
| `SOW_GCP_ZONE` | VM zone |
| `SOW_GCP_INSTANCE` | VM name |
| `SOW_SITE_ORIGIN` / `SOW_PLAY_ORIGIN` / `SOW_PTR_ORIGIN` | Public HTTPS origins |
| `SOW_CERTBOT_EMAIL` | TLS certificate contact |

Full list: [`.env.example`](.env.example).

## Commands

| Command | Output | VPS content | Server / infra |
|---------|--------|-------------|----------------|
| `infra --confirm-destroy` | — | — | Recreate Fedora VM on GCP; nginx, TLS, valkey, systemd (reproducible from `sow-dist/deploy/`) |
| `cg` / `crazygames` | `dist/crazygames/` | CDN rsync only | No |
| `p` / `prod` | `dist/play/` + marketing | play + shadowsofwar.io | Rsyncs server binaries + restarts `sow-server` when crates or version changed |
| `ptr` | `dist/ptr/` | ptr.shadowsofwar.io | Rsyncs server binaries + restarts `sow-server-ptr` only — never prod |
| `l` / `local` | `dist/site-dev/` | localhost only | No |

```bash
./sow cg -v
./sow prod -v
./sow ptr -v
./sow local --build-only
```

Each **prod** / **ptr** run ships WASM dist, rsyncs server binaries when crate inputs changed, pushes `.version` to the server working dir, restarts the orchestrator when needed, and verifies maps API + WebSocket. Use **`./sow infra --confirm-destroy`** only for a fresh VPS or changes under `sow-dist/deploy/` (nginx, TLS, valkey, systemd). That is the reproducible machine setup; routine releases use **`./sow p`** / **`./sow ptr`** only.

Optional Nix flake (`flake.nix`) is devShell-only — not used by `./sow` or VPS deploy.

Details: [sow-web/README.md](sow-web/README.md).

### Asset pipelines

| Pipeline | Source | Destination |
|----------|--------|-------------|
| CDN (parallel on cg/prod/ptr) | `assets/cdn/` only | `shadowsofwar.io/html/assets/cdn/` |
| WASM dist | `sow-web/shell` + compiled client | `dist/play`, `dist/ptr`, or `dist/crazygames` |
| Static in dist | `assets/static/` (fonts, icons — **not maps**) | `dist/crazygames/assets/static/` only |
| Maps (online) | `assets/maps/` | VPS maps dir → `/maps/` HTTP API (prod/ptr rsync) |
| Maps (offline) | `assets/static/maps/world/` only | Bundled inside client WASM |
| Server binaries | `cargo build --release` (`x86_64-unknown-linux-gnu`, glibc) | Rsync to `$HOME/shadowsofwar/` via gcloud; restart systemd unit |
| Marketing HTML | `sow-web/site/` | `shadowsofwar.io/html/` (via `sow prod`) |

Boot UI and leader portraits load from CDN at runtime for play/ptr shells (shell-only dist).

### What `sow prod` updates

| Artifact | Picked up? | Notes |
|----------|------------|-------|
| Game WASM shell | Yes → play.shadowsofwar.io | — |
| Marketing site | Yes → shadowsofwar.io | — |
| CDN (`assets/cdn/`) | Yes (parallel build) | — |
| Map files | Yes → prod maps dir | — |
| `sow-server` / `sow-relay` | Yes when server crates changed | rsync binaries + orchestrator restart; relays keep running (`KillMode=process`) |

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
