# Shadows of War

**[shadowsofwar.io](https://shadowsofwar.io)** — A free, open-source MMORTS featuring world maps, civilizations, alliances, expansion, and economy. 

*Shadows of War* is built entirely in **Rust** from the ground up. It shares a single deterministic game engine across Web (WASM), native Desktop (Windows/macOS/Linux), and Mobile (iOS/Android) clients, providing a seamless and highly scalable multiplayer experience.

---

## 🚀 Game Features & Mechanics

- **Massive Scale MMORTS:** Expand your territory, construct structures, and engage in large-scale strategic battles on a global map.
- **Deep Diplomacy:** Forge alliances to conquer neighbors, negotiate trade, or commit betrayal when the time is right.
- **Civilizations & Identity:** Choose from unique Nations and Tribes. Spawn on world maps derived from real-world OpenStreetMap (OSM) data.
- **Multiplayer & Skirmish:** Play in Ranked Online Matchmaking, Host Private Lobbies with friends, or play offline against AI bots.
- **Cross-Platform Play:** The game runs identically in the browser and natively.

---

## 🛠 Technology Stack

Shadows of War is a "full-stack" Rust video game, designed for absolute performance, determinism, and zero-allocation pipelines where possible.

### Graphics & UI
*   **[blade-graphics](https://github.com/kvark/blade):** The rendering pipeline is built on `blade`, a highly optimized, low-overhead WebGPU-like abstraction. It allows us to render thousands of map tiles and units blazingly fast.
*   **[egui](https://github.com/emilk/egui):** Used for a responsive, immediate-mode interface overlay.
*   **[winit](https://github.com/rust-windowing/winit):** Handles robust, cross-platform windowing and input events.

### Simulation & Networking
*   **Deterministic Engine (`sow-core`):** The game logic compiles to `wasm32-unknown-unknown` and uses strict integer math and custom RNG to guarantee lockstep synchronization across all clients.
*   **Relay Server (`sow-relay`):** A lightweight `tokio-tungstenite` WebSocket relay that broadcasts player intents without having to run heavy server-side physics.
*   **Backend (`sow-database`):** An `axum` REST API backed by **Valkey/Redis** for player profiles, matchmaking, and leaderboards.

### Procedural Audio (`sow-audio`)
Instead of shipping massive `.wav` files, the game features a custom **harmonic procedural synthesizer**.
*   Warm mobile-RTS sound effects are generated mathematically on the fly using layered sine harmonics.
*   **Harmonic System:** Every note played harmonizes perfectly. The musical key is derived from the match seed, ensuring combat and UI sounds blend into a cohesive soundscape.
*   **Spatialization:** Constant-power stereo panning and zoom-based attenuation handled via a background `rodio` worker.

---

## 🏗 Repository Structure

| Crate / Path | Description |
|---|---|
| `sow-core` | The deterministic simulation brain. Zero platform dependencies. |
| `sow-client` | The game executable. Glues the engine, renderer, and netcode together. |
| `sow-render` | The `blade` WGSL GPU rendering pipeline. |
| `sow-ui` | The `egui` menus, HUD, and overlays. |
| `sow-net` | The `bincode` serialized wire protocol and message envelopes. |
| `sow-relay` | The WebSocket intent broadcaster. |
| `sow-server` | Lobbies and matchmaking orchestration. |
| `sow-database` | Player data, profiles, and API microservices. |
| `sow-tools` | Developer CLI for map generation from OSM bounding boxes and asset packing. |
| `sow-dist` | Deployment scripts, Docker/Debian VPS templates, and the `./sow` CLI tool. |

---

## 🎮 Developer Guide & Building

The project uses a custom deployment script `./sow` to handle building, packing WASM, and deploying to Google Cloud.

### Playing / Testing Locally

**1. Run the Web (WASM) version locally:**
This will build the WASM payload, serve it locally, and automatically connect to the public production WebSockets (no `.env` required).
```bash
./sow local
# or its alias:
./sow l
```

**2. Run the Native Desktop client:**
The native client directly connects to the production endpoints by default.
```bash
cargo run --release -p sow-client
```

### Running on Mobile (iOS & Android)

Because the game logic and renderer are built on generic, standard Rust abstractions (`winit` + `blade`), the mobile deployment process relies on standard native toolchains.

**iOS Requirements & Setup:**
*   **Requirements:** A macOS machine, Xcode installed, and a valid Apple Developer Account (to sign the application).
*   **Building:** 
    1. Add the iOS targets via rustup: `rustup target add aarch64-apple-ios x86_64-apple-ios`
    2. Open the provided Xcode wrapper project located in `sow-dist/deploy/ios/`. 
    3. In Xcode, configure your Team/Developer License for signing, and hit **Run** to deploy the native app directly to your device or simulator.

**Android Requirements & Setup:**
*   **Requirements:** Android Studio (or the standalone Android SDK/NDK) and the `cargo-apk` tool (`cargo install cargo-apk`).
*   **Building:**
    1. Add the Android targets via rustup: `rustup target add aarch64-linux-android armv7-linux-androideabi`
    2. Because `winit` handles the `android-native-activity` lifecycle natively, you can build and launch the game directly via `cargo-apk`:
    ```bash
    cargo apk run -p sow-client
    ```

### Advanced Deployments (`./sow`)
The `./sow` script coordinates building the WASM bundles, compiling the GNU binaries for the backend, and shipping them over GCP IAP. *(Requires `gcloud auth login` and `tar`)*

*   `./sow ptr -v` : Deploys to the staging (PTR) environment.
*   `./sow prod -v` : Deploys to production (`shadowsofwar.io`).
*   `./sow cg` : Compiles and synchronizes the CrazyGames distribution.
*   `./sow infra --confirm-destroy` : Completely reprovisions a fresh Debian 13 VPS with Nginx, TLS, and Valkey.

### Source file size guard

Run the workspace compile check and the 600-line monolith guard (allowlisted debt shrinks as splits land):

```bash
./sow-tools/check.sh
```

---

## 📜 License & Attribution

Shadows of War is licensed under the [GNU Affero General Public License v3.0 or later (AGPL-3.0)](LICENSE). 

Portions of this codebase derive from [OpenFrontIO](https://github.com/openfrontio/OpenFrontIO) (© OpenFront Inc. and Contributors, AGPL-3.0-or-later). 
Please see the [LICENSE](LICENSE), [COPYRIGHT](docs/legal/COPYRIGHT), and [NOTICE](docs/legal/NOTICE) files for full terms and third-party notices.
