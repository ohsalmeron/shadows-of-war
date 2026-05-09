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

---

## 🧠 Technical Highlights & Recent Design Choices

### Deterministic Lockstep (1000+ Bots)
To support massive scale RTS combat, `sow-core` uses a strict lockstep model. All clients and the server share the exact identical `GameConfig` (such as `bot_count = 1000`). Only player *inputs* are sent over the network. 
* **Design Choice**: The server was explicitly stripped of hardcoded constants (e.g., `BOT_COUNT = 4`) and forced to adopt the dynamically broadcasted config. A discrepancy of even 1 bot spawn alters the RNG state, causing catastrophic simulation drift on frame 1. The engine now effortlessly synchronizes 1000 active bots without a single dropped frame or desync.

### Premium 2D Visuals with "Toaster" Memory Footprint
Instead of relying on heavy 3D rendering or massive 4K textures, `Shadows of War` leverages extremely lightweight math:
* **Tiled Water Texture**: The water noise asset is a tiny `256x256` raw binary file (exactly 64 KB). The sampler uses `AddressMode::Repeat` to endlessly tile it across the infinite ocean. This keeps the VRAM usage near zero while retaining infinite map scalability.
* **4-Octave Noise Shader**: To replicate the gorgeous, rippling aesthetics of expensive 3D ray-traced water, `map.wgsl` samples the tiny 64 KB texture four times at varying speeds, scales, and opposing trajectories. By interpolating vibrant `pool_dark` and `pool_light` colors and applying sharp, non-linear specular highlights, we achieve a dynamic, premium "WebGPU-Water" appearance entirely in a 2D fragment shader.

### Non-Blocking Background Instantiation
* **Design Choice**: Generating pathfinding chunks, water geometry, and unrolling the map state is highly CPU intensive. Doing this on the main thread would freeze the application and panic the OS watchdog.
* **Solution**: Upon receiving the `ServerStartMessage`, `sow-client` spins up an OS-level `std::thread` to crunch the map generation. Meanwhile, the main loop drops into an asynchronous `Loading` phase, effortlessly rendering the 60 FPS `sow-ui` Loading Screen until the background thread pipes the constructed `GameState` over a crossbeam channel.

### Custom GPU Pipeline (`blade-graphics`)
* The map state is efficiently bit-packed into `u32` arrays by the CPU (16 bits for Owner ID, 8 bits for Terrain).
* Uploaded to the GPU via Shared Memory buffers perfectly synchronized with `wait_for` lifecycle barriers to prevent use-after-free `invalid size` driver panics.
* Rendered using a custom `map.wgsl` shader that performs coordinate projection, bit-unpacking, and color mapping entirely on the GPU.

---

## 📝 Development Notes & Rules

* **Zero-Allocation Hot Path**: The `sow-core` simulation loop (`engine.tick()`) must avoid dynamic memory allocation (`Vec::push`, `Box::new`, etc.) to prevent stutters and ensure consistent frame times.
* **GPU Resource Lifecycle**: Native `blade-graphics` resources must be explicitly destroyed (`map_renderer.destroy(&render_ctx)`) during phase transitions or `CloseRequested` window events. Ensure you call `context.wait_for(...)` on the inflight command buffer **before** triggering the destructors to avoid driver memory corruption.

---

## 📜 License
*TBD*
