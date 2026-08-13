# Agent Instructions

These instructions describe the current Shadows of War deployment. Historical
hostnames, paths and compatibility behavior are not production instructions.

## Core rules

- Execute exactly the requested scope; do not invent adjacent work.
- Verify before making claims. Distinguish current runtime facts from historical
  evidence and roadmap design.
- Infrastructure decisions, firewall/PF/NSG changes and new resources require
  explicit user approval.
- The deploy pipeline is the only deployment interface. Never activate a
  release, copy artifacts, restart services, or edit production configuration
  manually.
- If `./sow p` fails, fix the pipeline and rerun it. Do not bypass it.
- Do not commit or push unless explicitly requested.

## Current production topology

### IONOS game/orchestrator host

- SSH alias: `ionos`; public IP: `74.208.246.177`.
- FreeBSD 15.1-STABLE, 4 vCPU, approximately 8 GiB RAM.
- Release layout: `/srv/sow/current` → `releases/<sha>`; web assets at
  `/srv/sow/web`; state at `/var/db/sow/`; logs at `/var/log/sow/`.
- Loopback-only services: `sow-server` WebSocket `25564`, HTTP/admin/maps
  `25566`, `sow-database` `25585`, and Valkey `6379`.
- nginx terminates public HTTP/TLS on `:80`/`:443` and proxies the game
  orchestrator WebSocket to `127.0.0.1:25564`.
- `/admin/api/status` is localhost-only; `/health` is not a server route.

### Azure F-Stack relay host

- SSH alias: `relay`; VM: `sow-dev-2nic`; Linux with F-Stack/DPDK.
- Management IP: `20.230.49.9`; data/public IP: `20.122.128.185`.
- Four `sow-relay@0..3` workers, one per DPDK queue. Management HTTPS is on
  `8080..8083`; game listeners use dynamically allocated ports registered by
  `sow-server`.
- Clients receive `relay.shadowsofwar.io` and a dynamic game port, then connect
  directly with `wss://`; IONOS is not in the game-packet data path.
- `SOW_RELAY_WORKERS` is the authoritative catalog. Production requires relay
  management TLS and relay tickets (`SOW_RELAY_TICKETS_REQUIRED=1`).
- The old Azure FreeBSD host `sow`/`20.7.77.78` and aliases `azure` and
  `sow-prod` are stale and must not be used.

## Deploy pipeline (`./sow p`)

1. Preflight release directory, sudo and checksums on the target hosts.
2. Build WASM locally and FreeBSD binaries on the `freebsd` build VM.
3. Assemble a content-addressed release.
4. Upload and activate through the pipeline activator.
5. Restart only the services affected by the release, wait for database
   readiness before starting `sow-server`, and verify health/public reachability.

`./sow b` is an optional backfill/bot test workflow; it is not a production
deployment path. Backfill hosts are test capacity and are not part of the live
IONOS + Azure relay topology unless explicitly enabled.

## Read-only debugging

The following commands are diagnostics only; any mutation belongs in `./sow p`.

```sh
ssh ionos 'sudo service sow_server status'
ssh ionos 'sudo service sow_database status'
ssh ionos 'sudo tail -50 /var/log/sow/server.log'
ssh ionos 'sudo tail -50 /var/log/sow/database.log'
ssh ionos 'sudo sockstat -4l | grep -E "sow_|relay"'
ssh ionos 'ps aux | grep -E "sow_server|sow_database|relay"'
ssh ionos 'readlink /srv/sow/current'
ssh ionos 'curl -s http://127.0.0.1:25566/admin/api/status'
ssh relay 'systemctl is-active sow-relay@0 sow-relay@1 sow-relay@2 sow-relay@3'
ssh relay 'systemctl show sow-relay@0 sow-relay@1 sow-relay@2 sow-relay@3 -p ActiveState -p Result -p ExecMainStatus'
```

For relay health use the authenticated HTTPS management endpoints; do not use
an unauthenticated HTTP `/internal/lobbies` request as a health probe.

## Identity and player-flow contract

- Anonymous players have one canonical account ID issued by
  `POST /profile/anonymous`, stored client-side as `sow_account_id`.
- A browser refresh or cache deletion may create a new anonymous account; this
  is intentional. CrazyGames verified identities and persistent bot accounts
  use `LinkedIdentity` records and are separate provider cases.
- `Join.database_account_id` is client-declared roster/progress metadata. The
  server may use it to correlate a lobby reconnect or ban, but it is not proof
  of identity or relay authentication. The relay authenticates the direct game
  connection with a short-lived match ticket
  (`ReadyWithTicket`/`ReconnectWithTicket`).
- Anonymous `account_id` is a bearer-like progress lookup key, not a secret or
  platform credential; do not use it as an authorization decision.
- Unticketed relay frames remain only as wire-compatibility decoding; production
  refuses them when `SOW_RELAY_TICKETS_REQUIRED=1`.

## Audit guidance

- Label historical reports and PRDs as historical/roadmap when their baseline
  differs from the running system.
- Do not remove active bot accounts, CrazyGames `LinkedIdentity`, canonical
  anonymous identity, or protocol compatibility solely because the names look
  similar; verify call sites first.
- Search for stale hosts, old relay routing, guest-ID migrations, `/profile/link`,
  and unauthenticated relay assumptions before changing code.

## Safety lessons

- A hardcoded PF IP previously caused total SSH loss. Never edit PF/NSG without
  explicit approval; validate syntax before loading rules and preserve SSH.
- On a new VM, verify `sudo id`/root access before any other operation.
- Every infrastructure mutation must be reproducible from this repository and
  the pipeline.
