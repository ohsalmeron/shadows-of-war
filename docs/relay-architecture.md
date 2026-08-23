# Relay Architecture: F-Stack as the TLS endpoint

> Status: live. Verified 2026-08-22 with `./sow p` release `0.1.2-3b977aa9f91c` (relay `3ff15be`, bin `002edc46`, `ws_write_timeout_ms 15000`) and real game traffic `319 [DIAG RELAY TX]` on worker 1.

## Why this exists

The game relay runs on a DPDK F-Stack VM in Azure because it needs kernel-bypass
hardware (SR-IOV VF, hugepages). Browsers only speak `wss://`. For the relay to
BE the endpoint, it must terminate TLS itself — no nginx, no HAProxy, no IONOS
proxy in the data path. This document explains the topology, how connections
are distributed, and how to operate/recover it.

## Topology

```
                    IONOS (FreeBSD VPS)              Azure (Ubuntu, DPDK VM)
                    ──────────────────              ────────────────────────
 Browser ──https──► │ nginx (web, TLS)   │
        ──wss─────► │ sow-server :25564  │  Start{relay_host, relay_port}
                    │ sow-database      │───────POST /internal/lobby/register──► mgmt :8080-8083 (kernel)
                    │ valkey            │                                        │
                    │                   │              ┌─────────────────────────┴──────────┐
                    │                   │              │  NIC DPDK ConnectX-5 VF 100Gbps      │
                    └───────────────────┘              │  RSS hash → queue                     │
                                                       │  queue 0 → worker 0 (mgmt :8080)     │
   Browser ──wss://relay.shadowsofwar.io:25592──►       │  queue 1 → worker 1 (mgmt :8081)     │
    (DNS relay.shadowsofwar.io → 20.122.128.185 via Cloudflare) ───► │  queue 2 → worker 2 (mgmt :8082)     │
                                                       │  queue 3 → worker 3 (mgmt :8083)     │
                                                       └──────────────────────────────────────┘
```

- IONOS (`74.208.246.177`, `ionos`) = orchestrator (matchmaking, web, API, lobby creation). OUT of the game data path. Release at `/srv/sow/current`.
- Azure (`sow-dev-2nic`, `relay`) = relay only. Two PIPs: mgmt `20.230.49.9` (eth0, SSH/control, NSG 8080-8083 only from IONOS) and data `20.122.128.185` (eth1, game, NSG 25590-26500 open). Each worker is a full endpoint: DPDK RX → F-Stack userspace TCP (FreeBSD 15, `52fa8f9ae666`) → rustls TLS → WebSocket → game loop. Workers `sow-relay@0..3`, mgmt HTTPS `8080-8083` (HMAC), game ports `25592-26500` dynamic. State at `/var/lib/sow-relay/manifest.json`.

## How connections are distributed (worker-per-queue)

1. The NIC RSS hashes each incoming packet (src/dst IP + port) into one of 4
   queues. Each queue is owned by exactly one worker process (`proc_id ==
   queue_id`, verified at boot; enforced by `ff_rss_self_queue_info`).
2. `relay_packet_dispatcher` (fstack-bridge/src/packet.rs) parses every packet
   (eth → IPv4 → TCP dst port) and computes `dst_port % nb_queues`. This is
   F-Stack's cross-queue dispatch: a packet that lands on the wrong RSS queue
   is forwarded to the owning worker's queue via the shared dispatch ring.
3. When a lobby starts, sow-server picks a dynamic port (1024..65535) and
   registers it with the worker where `port % 4 == worker_id`:
   `POST /internal/lobby/register {lobby_id, relay_port, ...}`.
4. That worker binds the port as an F-Stack listener (userspace, invisible to
   the kernel) and replies to sow-server, which broadcasts
   `Start{relay_host, relay_port}` to the players.
5. Players connect directly: `wss://{relay_host}:{relay_port}/ws/`.

## The client handoff

`sow-client/src/net/update/mod.rs` — on receiving Start:

```rust
if let Some(host) = relay_host {
    self.net.ws_url = format!("wss://{}:{}/ws/", host, relay_port);
}
```

There is deliberately NO branch for "secure page" anymore. The relay terminates
TLS on every game port, so WASM and native clients use the same URL shape.
Historical retired path: commit 25c1af1 introduced an `is_secure` branch that routed browsers
through `wss://shadowsofwar.io/relay/{port}/ws/` → nginx on IONOS → Azure,
turning IONOS into a middleman for every game packet (~300ms vs ~67ms). The
fix (this doc's era) removed the branch and gave the relay its own TLS.

## TLS on the relay

- rustls (tokio-rustls) wraps the bridge `Conn` before the WebSocket upgrade
  in `sow-relay/src/main.rs` (`MaybeTlsConn` unifies plain/TLS streams).
- Cert: Let's Encrypt via DNS-01 (`certbot-dns-cloudflare`) for
  `relay.shadowsofwar.io` (SAN verified). Files at `/usr/local/etc/sow/relay.crt` / `relay.key` (checked by pipeline: `test -s` + `openssl x509 -checkend 86400`). DNS `relay.shadowsofwar.io` → Cloudflare → data PIP.
- Secrets: systemd drop-in `/etc/systemd/system/sow-relay@.service.d/override.conf` (`0600`) carries `SOW_RELAY_CONTROL_SECRET` / `SOW_DB_SECRET` (injected via `sed` on the host) plus `SOW_WS_WRITE_TIMEOUT_MS=15000` (`[BOOT] ws_write_timeout_ms` verified). TLS cert is not an env var.
- If the cert is missing/expired the relay logs `[BOOT] TLS disabled` and serves plain ws:// — clients expecting wss:// will fail. This is the first thing to check in an outage.

## Pipeline (./sow p — `sow-dist/src/prod.rs`)

`./sow p` owns the full lifecycle (8 steps). Control last verified `0.1.2-3b977aa9f91c`:
1. Preflight (cargo/curl/rsync/ssh/wasm-opt + hosts). 2. Builds parallel: WASM local, FreeBSD on builder (`freebsd`), relay on Azure (`rsync` + `make -C lib FF_ZC_RECV=1` + `cargo build -p sow-relay`, `FSTACK_LIB_DIR=f-stack-src/lib`, `cargo:rerun-if-changed` on `libfstack.a`). 3. Assemble immutable release (`COMPONENTS` web/maps/server/database/ops/relay + `release.json` with `relay.bin_sha256`, `fstack`, `ws_write_timeout_ms`). 4. `remote_plan` diffs `/srv/sow/current/COMPONENTS` vs local and `relay_sha256` in `/var/lib/sow-relay/manifest.json`; if `!plan.any()` → `no production component changed; no restart performed`. 5. Stage (`~/.sow-deploy/release` on IONOS, `~/.sow-deploy/relay` on Azure if `plan.relay`). 6. Activate granular: symlink swap + only affected `sow_server`/`sow_database`/`nginx`; relay **last** via `activate_relay_host` — per-worker `stop/start sow-relay@N` with `healthz` poll, backups `.bak_$ts`, perms `750 root:sowrelay`, `drain=force-kill (user-authorized 2026-08-21; non-destructive drain pending)` in manifest. 7. Healthcheck (`verify_relay_runtime` systemctl + `https://127.0.0.1:808x/healthz`, HMAC `GET /internal/metrics` queue_id/count, `verify_relay_identity` manifest + `[BOOT] git=` + `sudo sha256sum`). 8. Public verification.

The worker catalog (`SOW_RELAY_WORKERS`, `SOW_RELAY_MGMT_RESOLVE_IP=20.230.49.9`, `SOW_WS_WRITE_TIMEOUT_MS=15000`) is release content — it hashes into the relay component, so changes redeploy via the plan instead of ghosting. Advertised `host` remains `relay.shadowsofwar.io`.

## Operations

- Relay health: `curl -kfsS https://127.0.0.1:8080/healthz` on the VM (repeat
  for :8081..:8083). The `/internal/lobbies` roster is an authenticated
  management endpoint, not an unauthenticated health probe; production
  management access is HTTPS and restricted by the NSG to IONOS.
- Live TLS check:
  `echo | openssl s_client -connect 20.122.128.185:<dynamic-port> -servername relay.shadowsofwar.io`
  (25592 is only an example allocation, not a fixed game port.)
- Workers: `systemctl status 'sow-relay@0..3'`; expect exactly 4 active units.
  Do not treat historical `NRestarts` values or old journal entries as current
  health. Verify `ActiveState=active`, `Result=success`, `ExecMainStatus=0`,
  and the four HTTPS health checks on ports 8080–8083.
- CPU ~100% per worker is normal (DPDK busy-poll).

## Certificate renewal

Cert expires 90 days after issuance (see `notAfter`). Pipeline checks `openssl x509 -checkend 86400` during activate — expired cert fails deploy.

Renewal still requires a worker restart; that restart now goes through `./sow p` (per-worker `stop/start` with the `force-kill` drain registered in the manifest). Non-destructive drain is pending — current mode is user-authorized force-kill.

## Disaster recovery (from a dead laptop)

SPOFs: Cloudflare API token (zone shadowsofwar.io, DNS:Edit), SSH keys
(~/.ssh/id_rsa — ionos, relay, freebsd). Everything else is in this repo or
regenerable.

```
1. new machine:  git clone https://github.com/worldofunreal/shadows-of-war
2. cp sow-dist/.env.example sow-dist/.env     # fill SOW_DB_SECRET, SOW_RELAY_CONTROL_SECRET, hosts (inline)
3. restore Cloudflare API token from vault → ~/.cloudflared/cert.pem.bak format:
   {"zoneID":"1e4d2979bf3209a3d03a3248a116da3c","accountID":"...",
    "apiToken":"cfut_..."}                    # or create a fresh token in CF UI
4. restore or provision the relay VM using the infrastructure provider's
    separate, reviewed provisioning process
5. run `./sow p` (full: control + relay if plan.relay; early exits with no restart if unchanged)
```
