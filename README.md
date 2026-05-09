# ⚔️ Shadows of War (SoW)

A high-performance, **deterministic lockstep** Real-Time Strategy (RTS) engine written in pure Rust. **Shadows of War** is engineered from the ground up to achieve 100% simulation parity across WebAssembly (WASM) browser clients and Native (Desktop) clients using a unified core architecture and a custom GPU rendering pipeline.

---

## 🏗️ Architecture

The project is structured as a multi-crate Rust workspace to strictly enforce separation of concerns between the deterministic simulation, networking, and platform-specific rendering:

### Core Engine
* **`sow-core`**: The heart of the engine. A `no_std`-compatible (conceptually), zero-allocation deterministic simulation loop. It handles the game state, lockstep execution, territory expansion, and bot AI mechanics. *Must remain 100% deterministic across all architectures (x86_64, ARM, WASM).*

### Rendering & Graphics
* **`sow-render`**: A custom, high-performance GPU pipeline built on top of [blade-graphics](https://github.com/kvark/blade). Uses shared memory upload buffers and custom WGSL shaders to pack and render the simulation state efficiently.
* **`sow-native`**: The desktop client (Linux/Windows). Uses `winit` for windowing and input handling, wiring `sow-core` state directly to the `sow-render` GPU pipeline.

### Web & UI
* **`sow-wasm`**: WebAssembly bindings using `wasm-bindgen`. Exposes the `sow-core` simulation to JavaScript/TypeScript.
* **`sow-ui`**: The web frontend client built with Vite and TypeScript. Renders the game state in the browser and provides the HTML/CSS user interface.

### Networking (WIP)
* **`sow-net`**: The multiplayer lockstep orchestration layer. Handles WebSocket connections, input buffering, and tick synchronization between the server and all connected clients.

---

## 🚀 Getting Started

### Prerequisites
* **Rust**: Latest stable toolchain (`rustup`).
* **Node.js / npm**: Required for building the web UI.
* **wasm-pack**: Required for building the WebAssembly targets (`cargo install wasm-pack`).
* **Vulkan / GPU Drivers**: Required for the native `blade-graphics` renderer.

### Launching the Cluster
The easiest way to run the project during development is to use the provided Python cluster script, which automatically builds the WASM package, installs NPM dependencies, compiles the native binaries, and launches both the Web client and the Native client simultaneously.

```bash
# From the repository root
./scripts/run_cluster.py
```

This script will:
1. Compile `sow-core` and `sow-wasm` into a WebAssembly package.
2. Compile `sow-render` and `sow-native` for your local OS.
3. Start the Vite dev server for `sow-ui` (usually at `http://localhost:5173`).
4. Launch the native desktop window.

### Manual Execution

**Run Native Client Only:**
```bash
cargo run -p sow-native
```

**Build WASM & Run Web Client:**
```bash
wasm-pack build sow-wasm --target web --out-dir ../sow-ui/pkg
cd sow-ui
npm install
npm run dev
```

---

## 🧠 Technical Highlights

### Deterministic Lockstep
To support fair and synchronous multiplayer without sending massive map state updates, `sow-core` uses a strict lockstep model. All clients start with the identical random seed and map configuration. Only player *inputs* (commands) are sent over the network. As long as the inputs are processed at the exact same "tick", the simulation remains perfectly synced across WASM and Native binaries.

### Custom GPU Pipeline (`blade-graphics`)
Instead of relying on heavy engines like Bevy or Unity, `Shadows of War` uses a lightweight, bare-metal-style rendering approach via `blade-graphics`. 
* The map state is efficiently bit-packed into `u32` arrays by the CPU (16 bits for Owner ID, 8 bits for Terrain).
* Uploaded to the GPU via Shared Memory buffers.
* Rendered using a custom `map.wgsl` shader that performs coordinate projection, bit-unpacking, and color mapping entirely on the GPU.
* The result is virtually zero CPU-overhead for rendering, leaving all cycles available for the deterministic lockstep simulation.

### Cross-Platform Parity
A primary goal of the project is ensuring the Native Client and the Web Client look, feel, and play identically. The WGSL shaders and TypeScript canvas renderer map to the exact same color palettes, coordinate systems, and input resolutions.

---

## 📝 Development Notes & Rules

* **Zero-Allocation Hot Path**: The `sow-core` simulation loop (`engine.tick()`) must avoid dynamic memory allocation (`Vec::push`, `Box::new`, etc.) to prevent GC stutters in WASM and ensure consistent frame times.
* **Float Determinism**: Avoid floating-point math in `sow-core` gameplay logic where possible to prevent architectural drift between WASM and x86_64 IEEE-754 implementations.
* **GPU Resource Lifecycle**: Native `blade-graphics` resources must be explicitly destroyed (`map_renderer.destroy(&render_ctx)`) during the `CloseRequested` window event to prevent Vulkan memory leaks.

---

## 📜 License
*TBD*
