# Shadows of War: Backend Architecture & Decoupling Plan

This document outlines the current state of the `sow-server` and `sow-relay` pipeline, the infrastructure configuration, and a concrete plan to safely decouple game sessions from the master orchestrator.

## 1. Infrastructure Validation
Based on an audit of the current environment, network restrictions are **not** the cause of relay connectivity issues.

*   **GCloud Firewall:** The rule `allow-sow-relay` is correctly configured to accept TCP traffic on ports `25570-25600`. The rule `allow-sow-web` is also active for standard web ports (80, 443, 3000, 8080).
*   **VPS OS Firewall:** The server uses `iptables` which is currently completely open (`ACCEPT` policy across all chains). No traffic is being dropped locally.

## 2. Server-Relay Pipeline Breakdown
The backend utilizes a master-worker architecture where `sow-server` acts as the matchmaking lobby orchestrator and spawns `sow-relay` binaries for active games. 

### Current Lifecycle:
1. **Lobby Management (`sow-server/src/lobby.rs`)**
   * The server runs a `master_tick` loop that ensures at least one `Waiting` lobby exists.
   * When players join and the countdown finishes, the lobby promotes to `Loading`.
2. **Player Verification**
   * **Are relays spawned with zero players? No.** The code explicitly checks `!players.is_empty()` during the `CountingDown` and `Loading` phases. If a lobby times out while loading and no human players successfully reported `Ready`, the lobby is silently destroyed. Relays only spawn for validated, ready humans.
3. **Relay Orchestration (`sow-server/src/main.rs`)**
   * Once clients send their `Ready` map progress, the lobby enters `ReadyForRelay`.
   * The main Tokio select loop pulls these lobbies out of the queue.
   * It allocates a port via an in-memory `AtomicU16` (starting at `25570`).
   * It spawns the `./sow-relay` binary via `tokio::process::Command::new()`, passing the port and config via CLI arguments.
   * It waits `500ms` for the relay to bind, then broadcasts a `ServerStartMessage` directing clients to reconnect to the newly spawned port.

### The Problem: Why Relays Die
When the `sow-server` service is restarted, the relays are killed due to three primary lifecycle coupling issues:

1. **Systemd CGroup Execution (Critical)**: `sow-server.service` uses the default systemd `KillMode=control-group`. This means when you restart the service, systemd forcibly sends a `SIGTERM` to the orchestrator *and all of its child relay processes*.
2. **Port State Amnesia**: The `NEXT_RELAY_PORT` is stored in RAM. If relays *did* survive a restart, the new server process would reset its counter back to `25570`. The next spawned game would try to use port 25570, hit an `EADDRINUSE` (Address already in use) collision, and crash.
3. **I/O Coupling**: The `tokio::process::Command` inherits `stdin` from the parent process by default. It's best practice to set this to `Stdio::null()` to prevent child processes from hanging when the parent pipe closes.

## 3. Action Plan for Decoupling
To achieve true decoupling (allowing `sow-server` restarts without dropping active games), the following steps must be implemented:

### Step 1: Escape the Systemd Control Group
Update `/etc/systemd/system/sow-server.service` to include:
```ini
[Service]
KillMode=process
```
*Why: This instructs systemd to only terminate the main `sow-server` PID when restarting, leaving the child `sow-relay` processes untouched.*

### Step 2: Implement Stateful Port Allocation
Remove the in-memory `AtomicU16` for ports. Instead, dynamically find an open port between `25570` and `25600` before spawning a relay.
*Why: A restarted server won't accidentally try to allocate a port that an orphaned, surviving relay is currently using.*

### Step 3: Decouple Process I/O
Modify the `Command::new("./sow-relay")` configuration in `main.rs` to explicitly discard standard input:
```rust
cmd.stdin(std::process::Stdio::null());
```

### Step 4: Graceful Orphan Reaping (Optional but Recommended)
Use `CommandExt::process_group(0)` to put relays in their own process group. This makes them true OS daemons that don't rely on the `sow-server` Tokio runtime to reap them when they exit.
