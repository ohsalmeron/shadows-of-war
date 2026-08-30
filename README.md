<p align="center">
  <img src="assets/shell/loader/sow-splash-desktop.webp" alt="Shadows of War — world map territory conquest" width="100%" />
</p>

<p align="center">
  <a href="https://shadowsofwar.io/play/"><img src="https://img.shields.io/badge/Play-shadowsofwar.io-ff6b2c?style=for-the-badge" alt="Play" /></a>
  <a href="https://shadowsofwar.io/how-to-play/"><img src="https://img.shields.io/badge/Docs-How_to_Play-0a0a0e?style=flat-square" alt="How to play" /></a>
  <a href="https://github.com/worldofunreal/shadows-of-war/blob/master/LICENSE"><img src="https://img.shields.io/badge/License-AGPL--3.0-blue?style=flat-square" alt="AGPL-3.0" /></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/Rust-1.85-orange?style=flat-square&logo=rust" alt="Rust 1.85" /></a>
  <a href="https://discord.gg/eauHRf7zP"><img src="https://img.shields.io/badge/Discord-Join-5865F2?style=flat-square&logo=discord" alt="Discord" /></a>
  <img src="https://img.shields.io/badge/Engine-deterministic_lockstep-0a0a0e?style=flat-square" alt="deterministic" />
</p>

<h1 align="center">Shadows of War — MMORTS Strategy Game</h1>

<p align="center">
  <strong>Match-based MMORTS about territory conquest on world maps.</strong><br/>
  Choose 1 of 12 legendary leaders, expand borders, build economy, forge alliances, betray rivals.<br/>
  <a href="https://shadowsofwar.io/play/"><strong>▶ Play at shadowsofwar.io</strong></a> · <a href="https://shadowsofwar.io/how-to-play/">How to Play</a> · <a href="docs/launch-graph.md">Launch Graph</a> · <a href="docs/launch-kit.md">Launch Kit</a>
</p>

> **A verifiable strategy game and systems project** — deterministic Rust engine shared across web, desktop and mobile; WebGPU `blade` rendering; DPDK/F-Stack kernel-bypass relay; procedural harmonic audio. One `sow-core`, three targets. Verified in production `2026-08-22` on IONOS `74.208.246.177` + Azure F-Stack relay `20.122.128.185` (`docs/relay-architecture.md`).

---

## Preview

| World map | Battle | Expansion | Leaders |
|---|---|---|---|
| ![World](assets/site/media/session-world.webp) | ![Battle](assets/site/media/session-battle.webp) | ![Expansion](assets/site/media/session-expansion.webp) | ![Leaders](assets/site/media/session-leader.webp) |

Gameplay capture: [`assets/site/media/shadows-of-war-gameplay.mp4`](assets/site/media/shadows-of-war-gameplay.mp4) · Trailer: [`assets/site/media/shadows-of-war-trailer.mp4`](assets/site/media/shadows-of-war-trailer.mp4) · More media in [`assets/site/media/`](assets/site/media/)

---

## Why state of the art

All claims are verifiable in this repo — no hype without evidence:

- **Deterministic lockstep `sow-core`** — strict integer math + `wyrand` RNG, compiles to `wasm32-unknown-unknown` and native. Zero-allocation paths where it matters. Same simulation drives WASM and desktop.
- **Kernel-bypass networking `fstack-bridge` + `sow-relay`** — F-Stack `FF_ZC_RECV` (FreeBSD 15, `52fa8f9ae666`) + `rustls` + `tokio-tungstenite`, 4 workers `sow-relay@0..3` via RSS (mgmt `8080..8083` HMAC, game `25592-26500` dynamic). Direct `wss://relay.shadowsofwar.io` — IONOS is not in the game packet path. See `docs/relay-architecture.md`.
- **WebGPU rendering** — `blade-graphics` WGSL + `egui` + `winit`. Thousands of tiles/units, shared code across web/native.
- **Procedural audio `sow-audio`** — harmonic synthesizer keyed by match seed + `rodio` spatialization, no shipped `.wav` bulk.
- **Reproducible pipeline `./sow p`** — 8 steps, immutable release `releases/<sha12>` + `release.json` content-addressed (`relay_sha256`, `relay_bin_sha256`, `fstack`, `ws_write_timeout_ms`), atomic symlink swap, health `systemctl + https://127.0.0.1:808x/healthz + HMAC /internal/metrics + sudo sha256sum`.

---

## Game Features

- **Massive Scale MMORTS:** Territory expansion, structures and large-scale battles on a global map.
- **Deep Diplomacy:** Alliances, trade and betrayal — with AI tiers (`docs/launch-graph.md`).
- **Civilizations & Identity:** 12 legendary leaders, Nations + Tribes, OSM-derived world maps.
- **Multiplayer & Skirmish:** Ranked matchmaking, private lobbies and offline vs bots (bot fill is internal, not all-human queue).
- **Cross-Platform:** Identical simulation in browser and native.

Shipping map: [launch graph](docs/launch-graph.md) · [launch kit](docs/launch-kit.md) · [how to play](https://shadowsofwar.io/how-to-play/)

---

## Technology Stack

Shadows of War is full-stack Rust, optimized for determinism and performance.

### Graphics & UI
*   **[blade-graphics](https://github.com/kvark/blade):** low-overhead WebGPU-like abstraction — thousands of tiles/units.
*   **[egui](https://github.com/emilk/egui):** immediate-mode UI overlay.
*   **[winit](https://github.com/rust-windowing/winit):** cross-platform windowing/input.

### Simulation & Networking
*   **Deterministic Engine (`sow-core`):** `wasm32-unknown-unknown`, integer math + custom RNG for lockstep.
*   **Relay (`sow-relay` + `fstack-bridge`):** DPDK/F-Stack userspace TCP, `rustls` TLS, WebSocket intent broadcast — no heavy server physics.
*   **Backend (`sow-database`):** `axum` + **Valkey/Redis** for profiles, matchmaking, leaderboards. Relay tickets required in prod (`SOW_RELAY_TICKETS_REQUIRED=1`).

### Procedural Audio (`sow-audio`)
Custom harmonic synthesizer — layered sine harmonics, key derived from match seed, constant-power panning + zoom attenuation via `rodio`.

---

## Repository Structure

| Crate / Path | Description |
|---|---|
| `sow-core` | Deterministic simulation brain. Zero platform dependencies. |
| `sow-data` | Static tables: tribe names, premium colors, leader metadata, emoji manifest. |
| `sow-net` | `bincode` wire protocol and message envelopes. |
| `fstack-bridge` | DPDK/F-Stack bridge (FFI + `FF_ZC_RECV` zero-copy). |
| `sow-relay` | F-Stack/DPDK WebSocket relay (4 workers). |
| `sow-server` | Lobbies, matchmaking orchestration, map playlists. |
| `sow-database` | Player data, profiles and API services. |
| `sow-client` | Thin native/WASM entry (`main`, cdylib). |
| `sow-render` | `blade` WGSL GPU pipeline. |
| `sow-ui` | Menus, HUD, `ClientApp`. |
| `sow-web/site` | Marketing site — landing, `/how-to-play/`, and legal pages. Marketing binaries live in `assets/site/media/`. |
| `sow-web/shell` | WASM game shell (`/play/`). |
| `sow-tools` | Map generation from OSM, asset packing, `check.sh`. |
| `sow-dist` | Build/deploy pipeline — the `./sow` CLI. |

---

## Developer Guide

The project uses `./sow` as single entrypoint:

```bash
./sow native        # desktop client (connects to prod by default)
./sow p             # full production deploy (WASM + FreeBSD + relay)
./sow p -v          # also bump public patch version
```

**Native:**
```bash
./sow native
# or
cargo run --release -p sow-client
```

<details>
<summary>Mobile (iOS & Android)</summary>

Generic `winit` + `blade` — standard toolchains:

**iOS:** macOS + Xcode + Apple Developer Account
```bash
rustup target add aarch64-apple-ios x86_64-apple-ios
# open sow-dist/deploy/ios/ wrapper in Xcode → Run
```

**Android:** Android Studio / NDK + `cargo-apk`
```bash
rustup target add aarch64-linux-android armv7-linux-androideabi
cargo apk run -p sow-client
```
</details>

**Production (`./sow p`):** builds WASM locally + FreeBSD binaries on builder + relay on Azure (`make -C lib FF_ZC_RECV=1` + `cargo build -p sow-relay`), assembles checksummed release, `remote_plan` diff vs `/srv/sow/current/COMPONENTS`, stages `~/.sow-deploy/release`, activates only changed services, verifies `systemctl is-active sow-relay@0..3` + `healthz` + `HMAC /internal/metrics`, retains 5 releases. See `docs/relay-architecture.md`.

Arch hosts need `binaryen` + `rust-wasm`.

**Guards:**
```bash
./sow-tools/check.sh   # cargo check + 600-line monolith guard
```

---

## Contributing & Security

- Contributing: [CONTRIBUTING.md](CONTRIBUTING.md)
- Security policy: [SECURITY.md](SECURITY.md) · Audit: [docs/security-audit.md](docs/security-audit.md)
- Relay ops: [docs/relay-architecture.md](docs/relay-architecture.md)

---

## License & Attribution

Licensed under [GNU Affero General Public License v3.0 or later (AGPL-3.0)](LICENSE).

Portions derive from [OpenFrontIO](https://github.com/openfrontio/OpenFrontIO) (© OpenFront Inc. and Contributors, AGPL-3.0-or-later). See [LICENSE](LICENSE), [COPYRIGHT](docs/legal/COPYRIGHT) and [NOTICE](docs/legal/NOTICE).
