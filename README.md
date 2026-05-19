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

## 📝 Development Notes & Rules

* **Zero-Allocation Hot Path**: The `sow-core` simulation loop (`engine.tick()`) must avoid dynamic memory allocation (`Vec::push`, `Box::new`, etc.) to prevent stutters and ensure consistent frame times.
* **GPU Resource Lifecycle**: Native `blade-graphics` resources must be explicitly destroyed (`map_renderer.destroy(&render_ctx)`) during phase transitions or `CloseRequested` window events. Ensure you call `context.wait_for(...)` on the inflight command buffer **before** triggering the destructors to avoid driver memory corruption.

---

## 📜 License
*TBD*
