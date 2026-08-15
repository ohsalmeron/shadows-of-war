# Relay Architecture: F-Stack as the TLS endpoint

> Status: live. Verified 2026-08-08 with an end-to-end TLS + WebSocket test.

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
   (DNS gray-cloud → 20.122.128.185, no CF)  ───►      │  queue 2 → worker 2 (mgmt :8082)     │
                                                       │  queue 3 → worker 3 (mgmt :8083)     │
                                                       └──────────────────────────────────────┘
```

- IONOS = orchestrator (matchmaking, web, API, lobby creation). OUT of the
  game data path.
- Azure = relay only. Each worker is a full endpoint: DPDK RX → F-Stack
  userspace TCP (FreeBSD stack) → rustls TLS → WebSocket → game loop.

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
  `relay.shadowsofwar.io`. DNS record is gray-cloud (proxied=false) so
  browsers connect directly to Azure, no Cloudflare in the path.
- Env: `SOW_RELAY_TLS_CERT` / `SOW_RELAY_TLS_KEY` on the relay VM
  (systemd drop-in `/etc/systemd/system/sow-relay@.service.d/override.conf`).
- If the cert is missing the relay logs `[BOOT] TLS disabled` and serves plain
  ws:// — clients expecting wss:// will fail. This is the first thing to check
  in an outage.

## Pipeline (./sow p)

`./sow p` now owns the control-host release lifecycle only: build, package,
hash, stage, atomically activate, restart only affected jail services, and
verify. Relay workers are deliberately not restarted by this path. A relay
deployment requires a real drain/ownership protocol first; killing a worker
with active games is not an acceptable fallback.

The worker catalog (`SOW_RELAY_HOST`/`SOW_RELAY_WORKERS`) is runtime
configuration for `sow-server` and is synchronized only when a server-side
runtime component changes. The advertised `host` remains the TLS hostname,
not a raw IP.

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

Cert expires 90 days after issuance (see `notAfter`). Renewal:

Renewal and relay-worker restart are a separate operational lifecycle. Do not
use `./sow p` as a relay restart workaround; the production pipeline will not
destroy active games to install a certificate.

## Disaster recovery (from a dead laptop)

SPOFs: Cloudflare API token (zone shadowsofwar.io, DNS:Edit), SSH keys
(~/.ssh/id_rsa — ionos, relay, freebsd). Everything else is in this repo or
regenerable.

```
1. new machine:  git clone https://github.com/worldofunreal/shadows-of-war
2. cp sow-dist/.env.example sow-dist/.env     # fill hosts (documented inline)
3. restore Cloudflare API token from vault → ~/.cloudflared/cert.pem.bak format:
   {"zoneID":"1e4d2979bf3209a3d03a3248a116da3c","accountID":"...",
    "apiToken":"cfut_..."}                    # or create a fresh token in CF UI
4. restore or provision the relay VM using the infrastructure provider's
   separate, reviewed provisioning process
5. run `./sow p` for the control-host release only
```
