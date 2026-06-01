# ⚔️ Shadows of War (SoW)

A high-performance, **deterministic lockstep** Real-Time Strategy (RTS) engine written in pure Rust. **Shadows of War** is engineered from the ground up to achieve 100% simulation parity across WebAssembly (WASM) browser clients and Native (Desktop) clients using a unified core architecture and a custom GPU rendering pipeline.

---

## 🏗️ Architecture

The project is structured as a multi-crate Rust workspace to strictly enforce separation of concerns between the deterministic simulation, networking, and platform-specific rendering:

### Core Simulation
* **`sow-core`**: The heart of the engine. A deterministic, zero-allocation simulation loop. It handles the game state, lockstep execution, territory expansion, and bot AI mechanics. *Must remain 100% deterministic.*

### Rendering & Graphics
* **`sow-render`**: A custom, high-performance GPU pipeline built on top of [blade-graphics](https://github.com/kvark/blade). Uses shared memory upload buffers and custom WGSL shaders to pack and render the simulation state efficiently.
* **`sow-ui`**: The immediate mode GUI built using `egui` and `blade-egui`. Handles lobbies, menus, and in-game HUDs seamlessly over the native rendering context.
* **`sow-client`**: The primary game executable. Wires `winit` for windowing/input, runs the background loading threads, and binds `sow-core` state directly to the `sow-render` pipeline and `sow-ui`.

### Networking
* **`sow-server`**: An authoritative matchmaking and lobby server. Handles WebSocket connections, orchestrates the `ClientReadyMessage` handshake, and broadcasts uniform game configurations and lockstep turn data to clients.
* **`sow-net`**: The shared networking protocol defining the serialized JSON messages used for the lockstep synchronization.

---

## 🚀 Getting Started

### Prerequisites
* **Rust**: Latest stable toolchain (`rustup`).
* **Python 3**: Required for the cluster script.
* **Vulkan / GPU Drivers**: Required for the native `blade-graphics` renderer.
* **Vendored forks** (required for a from-scratch clone): clone `egui/`, `winit/`, and `blade/` next to this repo at the commits pinned in [NOTICE](NOTICE).

### Deploy (`scripts/sow.sh`)

One entrypoint for build, deploy, and portal packaging. Legacy names (`cloud.sh`, `ptr.sh`, etc.) are thin wrappers that call the same script.

#### When to use which

| Command | Short | What it builds | When to use it |
|---------|-------|----------------|----------------|
| **local** | `l` | **Debug** native `sow-server`, `sow-relay`, 2× `sow-client` (no WASM) | Rapid native prototyping — faster compile times than release or web builds |
| **ptr** | `p` | `wasm-release` client + same `dist/` layout as production (no CrazyGames SDK) | Staging on ptr.shadowsofwar.io — edge cases, multiplayer debugging against prod-like assets |
| **cloud** | `c` | Same web bundle as ptr + deploy to shadowsofwar.io | Stable public web when you intend to ship |
| **package** | `pkg` | Same WASM pipeline as cloud, plus CrazyGames HTML slots + zip | Portal upload only (e.g. CrazyGames) — no VPS deploy |
| **android** | `a` | APK (`native`/`n` or `webview`/`w`) | Mobile builds |

Examples: `./scripts/sow.sh l` · `./scripts/sow.sh p` · `./scripts/sow.sh c` · `./scripts/sow.sh a n` · `./scripts/sow.sh a w`

#### Interruption and failures

All commands use `set -e` so a failed step stops the script instead of continuing blindly.

- **local (`l`)**: `trap` on EXIT/INT/TERM kills server, clients, and clears Redis port keys — safe to Ctrl+C or close the terminal mid-run.
- **package (`pkg`)**: Abort leaves a partial `dist/`; delete `dist/` and re-run.
- **ptr / cloud**: Abort stops rsync/ssh; the VPS may be mid-sync — re-run the same command to converge.
- **android**: Gradle/cargo errors exit immediately; fix the error and re-run.

#### CrazyGames vs cloud (same pipeline, different HTML)

`cloud` and `package` share one WASM build path (`wasm-release`, `wasm-bindgen`, loader assets, brotli). They are **not** separate asset pipelines.

- **cloud / ptr** ship `index.html` **without** the CrazyGames SDK CDN (website/PTR detect host at runtime).
- **package** runs the same `dist/` assembly, then fills `PORTAL_SDK_SLOT` / `PORTAL_BOOT_SLOT` in the template and zips `dist/`. Zipping `dist/` after a normal `cloud` deploy would upload the **wrong** HTML for CrazyGames.
- **package** is **lighter** than cloud: no `sow-server`/`sow-relay` build and no `rsync`.

See [Partner platforms](#partner-platforms-crazygames--poki) for the upload checklist.

#### Rust (`sow-tools`)

Deploy and packaging stay in bash for now. [`sow-tools`](sow-tools/) handles maps and data import. A future phase may add `cargo run -p sow-tools -- package` for cross-platform parity; that is not required today.

**Requires on PATH:** `cargo`, `wasm-bindgen`, `brotli`, `cwebp`, `terser` or `npx` (optional: `wasm-opt` / binaryen for smaller WASM).

### Launching the Cluster
The easiest way to run the project during development is to use the provided Python cluster script, which automatically builds the server and spawns multiple native clients.

```bash
# From the repository root
./scripts/run_cluster.py
```

This script will:
1. Compile the workspace in debug mode.
2. Launch the `sow-server` matchmaking daemon.
3. Launch 2 instances of `sow-client` connected to the local server.

### iPhone / iPad (Xcode)

Open [`ios/sow_ios.xcodeproj`](ios/sow_ios.xcodeproj), set your **Team** on the **ShadowsOfWar** target (Signing & Capabilities), plug in the device, choose it as the run destination, then Run. The **Rust** build phase compiles `sow-client` and installs it as the app executable (same idea as the Bevy mobile template). One-time: `rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios`.

---

## 🧠 Technical Highlights & Recent Design Choices

### Manifest-Driven Spawning & Historical AI Names
* **The Problem**: In previous engine iterations, when maps defined a specific set of historically accurate nations in `manifest.json` (e.g., 52 nations for Europe), any bots spawned beyond this count (e.g. up to 120 Nations and 650 Tribes) were assigned generic procedural names like `Nation 73` and `Tribe 17`. Furthermore, a legacy engine optimization in `build_snapshot` explicitly stripped string names for bots to reduce serialization payloads, forcing the UI to permanently fall back to these generic names.
* **The Fix**: 
    1. Removed the aggressive string-stripping in `build_snapshot` as modern hardware handles 700+ short string clones instantaneously.
    2. Implemented a deterministic `FALLBACK_TRIBES` pool inside `sow-core/src/tribes.rs` containing over 670 real-world historical tribes and nations (e.g., Picts, Xhosa, Comanches, Purépecha). 
    3. Fixed bot ID assignment overlapping by calculating the starting index of Tribes dynamically from the `nation_count` rather than a hardcoded `200`.
    4. The `spawn_ai` orchestrator now pulls uniquely from a randomly shuffled, deterministic array of these fallback names once the `manifest.json` coordinate-locked names are exhausted. This completely eliminates generic procedural bot names from the game and maintains flawless lockstep consistency across all multiplayer clients.

### Binary Synchronization (Bincode vs JSON)
To support massive scale RTS combat, `sow-core` uses a strict lockstep model. All clients and the server share the exact identical `GameConfig` (such as `bot_count = 1000`). Only player *inputs* are sent over the network.
* **Design Choice**: The network layer was migrated from JSON to strict binary serialization using `bincode` over persistent WebSockets.
* **Reasoning**: An RTS engine transmits dense numeric arrays (unit coordinates, spawn intents, ticks) up to 60 times a second. Binary packing drastically reduces bandwidth footprint and CPU parsing overhead compared to JSON, crucial for hitting 60 FPS on the single-threaded WASM client.
* **The "Envelope" Paradigm**: Binary lacks JSON's human-readable metadata, making it prone to silent deserialization failures. To prevent catastrophic packet misrouting, we wrapped all network traffic in strict `ServerMessage` and `ClientMessage` tagged enums. This enforces schema discipline at the Rust type-system level, using the enum discriminant to safely route raw byte payloads.

### Premium 2D Visuals with "Toaster" Memory Footprint
Instead of relying on heavy 3D rendering or massive 4K textures, `Shadows of War` leverages extremely lightweight math:
* **Tiled Water Texture**: The water noise asset is a tiny `256x256` raw binary file (exactly 64 KB). The sampler uses `AddressMode::Repeat` to endlessly tile it across the infinite ocean. This keeps the VRAM usage near zero while retaining infinite map scalability.
* **4-Octave Noise Shader**: To replicate the gorgeous, rippling aesthetics of expensive 3D ray-traced water, `map.wgsl` samples the tiny 64 KB texture four times at varying speeds, scales, and opposing trajectories. By interpolating vibrant `pool_dark` and `pool_light` colors and applying sharp, non-linear specular highlights, we achieve a dynamic, premium "WebGPU-Water" appearance entirely in a 2D fragment shader.

### Non-Blocking Background Instantiation
* **Design Choice**: Generating pathfinding chunks, water geometry, and unrolling the map state is highly CPU intensive. Doing this on the main thread would freeze the application and panic the OS watchdog.
* **Solution**: Upon receiving the `ServerStartMessage`, `sow-client` spins up an OS-level `std::thread` to crunch the map generation. Meanwhile, the main loop drops into an asynchronous `Loading` phase, effortlessly rendering the 60 FPS `sow-ui` Loading Screen until the background thread pipes the constructed `GameState` over a crossbeam channel.

### State Machine & Phase Transitions (Avoiding Race Conditions)
The engine strictly separates the UI state (`ClientPhase`) from the deterministic simulation state (`GamePhase`) to avoid race conditions. 
* **`ClientPhase`** (UI Layer): `Splash` -> `MainMenu` -> `Playing`
* **`GamePhase`** (Simulation Layer): `Lobby` -> `Spawning` (Deployment) -> `Playing` -> `GameOver`

**The Lifecycle Cue Flow:**
1. **Lobby & Loading**: The user connects and waits. `ClientPhase::MainMenu`.
2. **Init / Map Download**: The map is downloaded, engine spins up in the background. `ClientPhase::Splash` (loading screen). The map is instantiated.
3. **Deployment Phase (`GamePhase::Spawning`)**: The loading screen finishes (`gpu_load_step == 4`). The client swaps to `ClientPhase::Playing` so the HUD and map are visible, **BUT** the engine is still in `GamePhase::Spawning`. In this phase, the simulation ticks process only placement logic, not combat/movement. The user clicks to deploy.
4. **Game Starts (`GamePhase::Playing`)**: Once the spawn timer ends, the engine swaps to `GamePhase::Playing`. 
* **Important Rule**: **Never tie player-specific logic (like camera snapping to their base)** to the end of the Loading Screen. It must execute only after the player has actually deployed (detected by checking their `tile_count > 0` and observing the transition into `GamePhase::Playing`), ensuring it works flawlessly for both multiplayer and single-player.

### Offline Network Integrity (Single-Player Mode)
* **Design Choice**: In Single-Player mode, the local deterministic simulation is run directly on the client, completely bypassing the network.
* **Input Routing Trap**: During the Map Loading `Splash` phase, background reconnection routines can accidentally re-establish a WebSocket connection to the multiplayer orchestrator. If this occurs, local `GameplayIntent`s (such as Spawning) are inadvertently routed to the remote server instead of the offline engine, causing severe visual and mechanical desyncs.
* **Solution**: The engine strictly enforces a `!is_offline` constraint on the core reconnection loop. This guarantees that `self.net.client` remains completely `None` during offline matches, ensuring all user clicks are immediately captured by `offline_intents` and processed instantly by the local simulation tick.

### Custom GPU Pipeline (`blade-graphics`)
* The map state is efficiently bit-packed into `u32` arrays by the CPU (16 bits for Owner ID, 8 bits for Terrain).
* Uploaded to the GPU via Shared Memory buffers perfectly synchronized with `wait_for` lifecycle barriers to prevent use-after-free `invalid size` driver panics.
* Rendered using a custom `map.wgsl` shader that performs coordinate projection, bit-unpacking, and color mapping entirely on the GPU.

### WebAssembly (WASM) & WebGL Stability
Achieving true 1:1 cross-platform stability on WebAssembly required extensive patches to the underlying `blade-graphics` GLES abstraction:
* **Split Buffer Architecture:** WebGL strictly prohibits binding the same `WebGLBuffer` as both an `ARRAY_BUFFER` and `ELEMENT_ARRAY_BUFFER`. To prevent immediate `INVALID_OPERATION` crashes on WASM, `blade-graphics` now maintains a dual-buffer system, dynamically copying and binding index data to a dedicated `raw_index` target under the hood.
* **Canvas Cropping & DPI scaling:** Winit 0.29 on WASM properly scales the inline CSS, but natively fails to update the actual DOM `<canvas width="...">` properties during resize events. This caused WebGL to map high-res 2000x1600 framebuffers into small 800x600 canvases, severely cropping the output. We resolved this by bypassing `winit` and forcibly syncing the canvas DOM attributes with the physical surface extent inside `reconfigure_surface`. We also reverted `sow-client` to utilize `LogicalSize`, ensuring Winit accurately multiplies CSS logic by the browser's `devicePixelRatio` without manual, erroneous `scale_factor` overrides.
* **Texture Unit Collisions:** WebGL lacks explicit `layout(binding=X)`. Consequently, the graphics API was assigning all uniform samplers to texture unit `0`, causing `egui` (FLOAT textures) and `sow-render` (UINT textures) to overwrite one another. We implemented a dynamic `next_texture_slot` assignment in the pipeline creation, guaranteeing deterministic, sequential texture unit mapping.
* **Non-Blocking Synchronisation:** WebGL2 sets `MAX_CLIENT_WAIT_TIMEOUT_WEBGL` to `0`, strictly forbidding blocking the main browser thread. `blade` originally attempted to block for 1 second, crashing the context. The engine now conditionally passes a `0` timeout exclusively on `wasm32`, forcing a clean, non-blocking polling architecture.
* **Event Loop Starvation (`ControlFlow`):** In `winit`, using `ControlFlow::Poll` forces the event loop to spin continuously without yielding. On native platforms this needlessly burns CPU, but on WebGL (Firefox/Chrome) it is catastrophic. It completely starves the browser's `requestAnimationFrame` compositor, reducing the game to a buggy ~15 FPS. The engine must explicitly use `ControlFlow::Wait` and trigger `window.request_redraw()` to properly sync with the browser's vsync refresh rate.
* **SVG vs PNG Rendering Overhead:** While `egui_extras` supports native SVG loading via `resvg`, it relies on heavy CPU vector math to rasterize the SVG into a flat texture buffer before uploading it to the WebGL context. If an SVG (e.g., UI icons floating above players) scales continuously with the camera zoom, `resvg` is forced to re-rasterize and re-upload a new texture every single frame, crippling WebGL performance. Using pre-rasterized PNGs completely eliminates this CPU math, allowing the GPU to effortlessly stretch the static texture using hardware interpolation at zero cost.

### Legacy Android & GLES Compatibility (Mali-T720)
Achieving seamless performance on "shitty" low-end hardware (like older Androids with Mali-T720 GPUs) required deep modifications to the GLES backend of the engine:
* **Manual Buffer Synchronization**: Modern Androids (e.g., Galaxy S9+) support `GL_EXT_buffer_storage` allowing zero-copy "coherent" memory mapping between CPU and GPU. Older OpenGL ES 3.1 hardware lacks this, meaning we had to surgically instrument `BufferBelt` allocations and `MapRenderer` uploads to explicitly flush and execute `sync_buffer()` (`gl.buffer_sub_data`) **before** `context.submit()` to prevent transparent/corrupted frames.
* **Uniform Block Binding Hacks**: Legacy GLSL compilers often strip variable names or rename struct blocks. We modified the `blade-graphics` shader reflection pipeline to include a robust fallback chain (checking by struct type name, indexing, and parameter sizes) so older Android drivers can correctly map and bind our UI uniform buffers.
* **Scissor Coordinates**: OpenGL operates with a bottom-left origin, while `egui` and `wgpu` rely on top-left. We baked coordinate inversion logic directly into `set_scissor_rect` at the driver level to render the native UI perfectly across all mobile devices.
* **Fragment Shader FP16 Precision Trap**: Android GLES hardware (specifically Adreno/Mali) aggressively downgrades fragment shader integer math (`i32`) to `mediump` (16-bit Float hardware) to save battery. This caused our map coordinate bounds math (e.g., `pixel_x + 1`) to hit the 11-bit mantissa ceiling and silently truncate on large maps (e.g., `2500 + 1 = 2500`), totally breaking neighbor-checking for territory borders.
* **CPU-Side Directional Bit-Packing for Thin Borders**: To permanently bypass Android GPU optimizer bugs while maintaining the ability to draw razor-thin territory outlines, we use a hybrid directional bit-packing approach. The CPU evaluates the 4 adjacent neighbors during the lockstep `dirty_tiles` loop in `O(1)` time, and explicitly packs the 4 directional borders into the top 4 unused bits of the 32-bit texture payload (Bit 31: Up, 30: Down, 29: Left, 28: Right). The fragment shader simply reads these 4 bits and applies precise mathematical clipping (`fract`) to draw the borders exactly on the outer edge, allowing customizable thickness without any heavy GPU-side neighbor checking. This ensures 100% compatibility across legacy Android devices with perfect performance.

### Egui Font Fallback Crashes
* **The Trap**: When overriding `egui`'s `FontDefinitions` to register custom `.ttf` weights (e.g., `Bold` or `Thin`), manually defining the fallback lists (like `vec!["Bold", "Default"]`) implicitly drops `egui`'s internal emoji fonts. The moment the UI attempts to render an emoji (like ⚔ or ★), `epaint` immediately panics with `No font data found for "emoji"` because the fallback chain was severed.
* **The Fix**: Instead of hardcoding the fallback lists and trying to guess `egui`'s internal font keys, we dynamically clone the fallback vector directly from `egui::FontFamily::Proportional` (which is guaranteed to contain the correctly configured emoji fallbacks) and securely prepend our custom font names to it (`bold_family.insert(0, "Bold".to_owned())`).

---

## Map authoring (paint, OpenFront import, OSM)

Shadows of War uses a single **SOWM** artifact per map (`map.bin` / `map.bin.br` with embedded spawns). Three creation paths mirror the OpenFront community workflow:

| Layer | Tool | Output |
|-------|------|--------|
| **Paint** | In-game Map Editor (`sow-client` main menu) | `assets/maps/<name>/` via Compile & Export |
| **OpenFront import** | `sow-tools import-openfront` | Same SOWM layout from `image.png` + `info.json` or legacy `map.bin` + `manifest.json` |
| **OSM region** | `sow-tools` bbox CLI | Overpass fetch → rasterizer → spawns from `place=*` nodes |

### Map pipeline (how data flows)

The runtime game reads **`map.bin`**: one byte per map cell (`MapTile` in `sow-core`). That format is **not** a slippy-map PNG. Preview tiles and shipped terrain are separate layers.

```
  [Optional] OSM Standard preview tiles    [Build] Game terrain
  256×256 PNG, lon/lat for selection    →   map.bin MapTile grid
         │                                  land / ocean / shore / magnitude
         │ (does NOT feed generate)              ↑
         └────────────────────────────────────────┘
                    Overpass vector OSM
                    OR OpenFront image.png
```

**What a `MapTile` byte means** (see `sow-core/src/map.rs`):

| Bits | Meaning |
|------|---------|
| Land (bit 7) | 1 = land, 0 = water |
| Shoreline | coast transition |
| Ocean | exterior sea (vs inland lake) |
| Magnitude 0–31 | On land: plains (0–9), highlands (10–19), mountains (20+). On water: distance-to-shore for rendering |

**Path A — OpenFront / MapGenerator (canonical for elevation)**

Used by OpenFrontIO’s Go MapGenerator and ported to `sow-map/src/image_pipeline.rs` and `sow-map/src/generator.rs`.

1. Author paints **`image.png`** where pixel **blue channel encodes terrain** (not real-world DEM):
   - Water: `blue == 106` (or very low alpha)
   - Land: `blue` 140–200 → elevation → magnitude after pipeline
2. **`generate_from_rgba`** (or `generate_map`): classify pixels → remove tiny islands/lakes → downscale to mobile budget (~1M cells) → mark ocean + shoreline + water depth → **pack** into `MapTile` bytes.
3. **`sow-tools import-openfront`** reads `image.png` + `info.json` (nation spawns) → writes `map.bin`, `map.bin.br`, `thumbnail.webp`.

No coordinates API: the PNG **is** the source of truth. OSM is only involved if a human (or tool) drew coastlines into that PNG.

**Path B — OSM bbox via `sow-tools` / map editor Generate (coordinates → geometry)**

Input: bounding box `min_lon,min_lat,max_lon,max_lat` and **scale** (pixels per degree).

1. **Dimensions**: `map_dims_for_bbox` → width×height (aligned to 4, capped by `MAX_MAP_PIXELS`).
2. **Overpass** (`sow-map/src/osm_overpass.rs`): tiled HTTP queries to public Overpass servers.
   - Coastlines: OSM ways tagged `natural=coastline` with geometry.
   - Inland water (optional): `natural=water`, `bay`, `landuse=water`, etc.
   - Places: `place=city|town|…` nodes for spawn candidates.
3. **Rasterize** (`sow-map/src/osm_coast.rs`):
   - Project lon/lat → map pixel polylines.
   - Draw coast barriers → flood ocean from map edges → flood-fill land from bbox center → tag ocean + shoreline.
   - Stamp lake polygons as pure water.
   - **All generated land starts as flat plains** (magnitude 0); no hills from OSM or from preview tiles.
4. **Spawns**: map `place=*` nodes to grid cells, or fallback scatter on land.
5. **Export** to `assets/maps/<name>/` (`sow-tools/src/exporter.rs`).

This path uses **OpenStreetMap vector data**, not rendered map tiles. It does **not** use OpenFront blue-channel rules unless you later paint over the result in the editor.

**Path C — Map editor only**

- **Brush**: edit `MapTile` bytes directly on the grid (same format as shipped maps).
- **OSM picker preview**: fetches **raster tiles** for display while you drag a selection square; **Generate** runs Path B on the selection bbox (Overpass), not on tile pixels.

**Pre-rendered raster tiles (preview only)**

The map editor fetches [OSM Standard](https://tile.openstreetmap.org/) PNG tiles (`256×256` per zoom/x/y) so you can see geography while dragging a selection. **Generate** still uses Path B (Overpass coastlines), not tile pixels.

- Preview tiles are **not** the OpenFront `image.png` encoding; do not feed them into `generate_from_rgba` without a custom classifier.
- **Weight**: one HTTP request per visible tile; editor caches tiles. Overpass generate is separate (geometry JSON).
- Editor generate rejects selections that span more than **4** Overpass grid cells (~15°×10° each); zoom in and select smaller.

| Goal | Use |
|------|-----|
| Pick a region on a familiar map | OSM Standard preview + Path B generate |
| Correct **coast** without maritime line artifacts | Path B Overpass, not raster tile copy |
| **Mountains / highlands** on the shipped map | Path A PNG, or brush after Path B |

### OpenFront mapper workflow (Layer 1b)

1. Author in OpenFront style: paint `image.png` (blue channel = water), place nations in `info.json` (`name`, `flag`, `coordinates`).
2. From the repo root, import into SOW:

```bash
cargo run -p sow-tools -- import-openfront \
  --input path/to/OpenFrontIO/map-generator/assets/maps/europe \
  --name europe
```

Or re-pack an existing shipped folder that already has `map.bin`:

```bash
cargo run -p sow-tools -- import-openfront --input assets/maps/europe
```

This writes `map.bin`, `map.bin.br`, `thumbnail.webp`, and refreshes `assets/maps/catalog.bin`.

### OSM bbox generation (Layer 2)

```bash
cargo run -p sow-tools -- \
  --bbox "-103.4,20.6,-103.3,20.7" \
  --name guadalajara \
  --scale 1000
```

Set `SOW_MAPS_ROOT` to override the output directory (default: `assets/maps`). Requires network access to the public Overpass API.

### Map editor OSM picker (preview tiles)

Preview uses `tile.openstreetmap.org` (OSM Standard). **Generate from Selection** runs Path B (Overpass) on the selection bbox only — not preview pixels. Side panel shows lon/lat bounds and estimated Overpass tile count (max 4). Attribution: © OpenStreetMap contributors.

**OSM attribution:** Maps built from Overpass must credit [© OpenStreetMap contributors](https://www.openstreetmap.org/copyright) (ODbL). Hand-painted and OpenFront PNG imports have no OSM obligation unless they incorporate OSM-derived geometry.

---

## 📝 Development Notes & Rules

* **Zero-Allocation Hot Path**: The `sow-core` simulation loop (`engine.tick()`) must avoid dynamic memory allocation (`Vec::push`, `Box::new`, etc.) to prevent stutters and ensure consistent frame times.
* **GPU Resource Lifecycle**: Native `blade-graphics` resources must be explicitly destroyed (`map_renderer.destroy(&render_ctx)`) during phase transitions or `CloseRequested` window events. Ensure you call `context.wait_for(...)` on the inflight command buffer **before** triggering the destructors to avoid driver memory corruption.

---

## License

Shadows of War is free software licensed under the [GNU Affero General Public License v3.0 or later](LICENSE) (AGPL-3.0-or-later).

Copyright (c) 2024–2026 Omar Hernandez Salmeron. See [COPYRIGHT](COPYRIGHT).

This project is a derivative work based on [OpenFront](https://openfront.io) (© OpenFront LLC and Contributors, AGPL-3.0). Owned art (avatars, splash, select leader portraits, and core building icons) is restored; remaining sprites use themed procedural placeholders. Only the `northamerica` map ships in-repo. Third-party notices: [NOTICE](NOTICE).

## AI-generated art (store submission)

Splash screens, avatars, and leader portraits were created with Gemini, Meta AI, and Midjourney. Prompt history and iteration notes live in [`leaders.md`](leaders.md) (internal roster; share with Poki/CrazyGames on request). Verify each tool’s Terms of Service allows commercial redistribution in compiled game binaries before store submission. See [`assets/SOURCES.toml`](assets/SOURCES.toml).

## Partner platforms (CrazyGames / Poki)

### Build portal zip (CrazyGames)

```bash
./scripts/sow.sh package              # → shadows-of-war-crazygames.zip + dist/
cd dist && python -m http.server 8080   # smoke-test locally
```

The `package` subcommand reuses the production WASM build, copies `web/sdk/` and `web/privacy.html`, sets portal WS to `wss://shadowsofwar.io/ws/` for multiplayer, loads the CrazyGames SDK v3 CDN, and skips service workers on CrazyGames hosts. Production deploy: `./scripts/sow.sh cloud`.

Web builds load [`web/sdk/store_portals.js`](web/sdk/store_portals.js) for `gameplayStart` / `gameplayStop` and loading hooks. Privacy policy: [web/privacy.html](web/privacy.html).

### CrazyGames Developer Portal checklist

1. Create account at [developer.crazygames.com](https://developer.crazygames.com) → **Submit game** → upload `shadows-of-war-crazygames.zip`.
2. Metadata: title *Shadows of War*, genre strategy/io, **English**, 800×800 cover + short gameplay video.
3. QA notes for reviewers:
   - Derivative of OpenFront (AGPL); "Based on OpenFront" on main menu; full notices in Credits.
   - Character art and splash screens are AI-generated by the developer (Gemini / Meta / Midjourney); prompts documented in `leaders.md`.
   - **Single Player** works offline; **Multiplayer** connects to `wss://shadowsofwar.io/ws/`.
   - How to play: Main Menu → Single Player, or connect and join ranked.
4. Test **Basic Launch** preview: confirm SDK loading events, single-player boot, and multiplayer join to your server.
5. After metrics look good, request **Full Launch** (ads) if desired.

WASM bundle size is printed at the end of `./scripts/sow.sh package` (CrazyGames Basic Launch limit ~50 MB).

### WASM size (browser / portal builds)

Deploy scripts compile the client with the dedicated **`wasm-release`** profile (`opt-level = "z"`, LTO, strip) instead of plain `--release`:

```bash
RUSTFLAGS="-C target-feature=-bulk-memory" cargo build --profile wasm-release -p sow-client --target wasm32-unknown-unknown
```

After `wasm-bindgen`, an optional **`wasm-opt -Oz`** pass (install **binaryen**) shrinks the bundle further before brotli. `./scripts/sow.sh ptr` and `./scripts/sow.sh cloud` run this automatically when `wasm-opt` is on `PATH`.

Recent slimming (Phase 2): removed Android-only deps from the client crate, dropped `resvg`/`usvg`/`tiny-skia` from mover sprite atlas (pre-baked PNGs), unified workspace dependency pins. Ship mover icons: `transport_ship.png`, `trade_ship.png`, `battleship.png` in `sow-client/assets/`.

**Measured** (post-bindgen, pre-brotli, `wasm-release` profile): ~11.2 MB client WASM; ~10.0 MB after `wasm-opt -Oz` (requires `binaryen`). Brotli (`-Z`) compresses further for CDN delivery.

## Public launch checklist (AGPL)

Before first **public** release or store monetization:

1. Commit and push your branch.
2. **Make the GitHub repo public** — [github.com/ohsalmeron/shadows-of-war](https://github.com/ohsalmeron/shadows-of-war) → Settings → Change repository visibility.
3. Deploy or package (`./scripts/sow.sh cloud` or `./scripts/sow.sh package`). The script prints the `git tag` command; push the tag so Credits links work (`git push origin v$(cat .version)`).
4. Upload `shadows-of-war-crazygames.zip` to the CrazyGames Developer Portal (see checklist below).

Credits in-game link to `https://github.com/ohsalmeron/shadows-of-war/tree/v<version>` matching `.version`.

## Corresponding Source (AGPL §13)

The production server at https://shadowsofwar.io runs `sow-server` and `sow-relay` from this repository. Corresponding source for any deployed version is available at the matching git tag or commit shown in the client build (`SOW_BUILD_VERSION`).

```bash
git clone https://github.com/ohsalmeron/shadows-of-war.git
cd shadows-of-war
cargo build --release -p sow-server -p sow-relay
```

## OpenStreetMap Compliance

Maps generated via `sow-tools` from OpenStreetMap data must credit [© OpenStreetMap contributors](https://www.openstreetmap.org/copyright) (ODbL). Requirements:

- Do **not** use `tile.openstreetmap.org` as your own CDN or tile cache.
- Overpass API: use an identifiable User-Agent (`ShadowsOfWar-MapEditor/1.0` in the map editor, same string for `sow-tools`), respect rate limits, and avoid hammering public instances.
- Map editor preview tiles: `tile.openstreetmap.org` with User-Agent `ShadowsOfWar-MapEditor/1.0`; credit © OpenStreetMap contributors in the editor UI. Do not bulk-cache or redistribute tiles.
- Store the Overpass query bbox in `assets/maps/SOURCES.toml` for each OSM-derived map.
- Mark OSM maps with `source = "osm"` in SOURCES.toml.

Hand-painted maps and OpenFront-format imports have no OSM obligation unless they incorporate OSM data.

See also [CONTRIBUTING.md](CONTRIBUTING.md) and [SECURITY.md](SECURITY.md).

## Map Attribution

Only the `northamerica` map is included in `assets/maps/`. See `assets/maps/SOURCES.toml`. OpenFront-derived maps were removed from the repository; CC-BY-SA map history is documented in NOTICE and README.

