# Security Audit Checkpoint

**Date:** 2026-08-09
**Scope:** IONOS + Azure relay deployment and the code at the current
`feat/dpdk-relay` HEAD.  The baseline observations below were captured before
the controlled data reset; the cleanup checkpoint records the resulting zero
state.

## Confirmed protections

- IONOS has separate FreeBSD jails for `sow-server`, `sow-database`, Valkey,
  and mail.
- PF is enabled with a default inbound block rule.
- IONOS Valkey is loopback-bound and has protected mode enabled.
- IONOS `sow-server` and its map listener are loopback-bound.
- Azure relay management ports `8080..8083` are NSG-allowlisted to the IONOS
  public IP.
- Relay workers use the installed `relay.shadowsofwar.io` certificate and the
  relay-to-database path is HTTPS.
- SSH password and keyboard-interactive authentication are disabled.

These controls reduce exposure; they do not replace application authentication
or least-privilege service isolation.

## Critical-risk backlog

1. Relay management HTTP has no application authentication; it relies only on
   the Azure NSG source allowlist.
2. The relay handoff has no signed, expiring player capability; `Ready` is
   authorized by `lobby_id` + `player_id` alone.
3. Public orchestrator and relay connections have no verified per-IP admission
   or handshake rate limit.
4. Relay workers run as root and relay SSH is reachable from any source allowed
   by the NSG.
5. Public profile routes can
   create arbitrary-provider accounts without platform authentication.
6. Game ports are public and the Azure VNet has no attached Azure DDoS
   Protection plan.

## Risk 1 — internal database bearer and transport

### What the secret is

`SOW_DB_SECRET` is a shared bearer credential used for server-to-database and
relay-to-database calls.  The database accepts a request when its
`Authorization` header is exactly `Bearer <SOW_DB_SECRET>`.

Evidence:

- Validation: [sow-data/src/main.rs](/home/bizkit/Github/shadows-of-war/sow-data/src/main.rs:233)
- Server use: [sow-server/src/main.rs](/home/bizkit/Github/shadows-of-war/sow-server/src/main.rs:384)
- Relay use: [sow-relay/src/main.rs](/home/bizkit/Github/shadows-of-war/sow-relay/src/main.rs:503)

### Current verified state

- A new random 64-character secret is installed in IONOS and all four Azure
  relay workers.  The two installed values were compared by hash and matched.
- IONOS secret configuration is mode 600; the relay systemd drop-in contains
  the same value.  The value is not stored in the repository or printed by the
  pipeline.
- The source fallback was removed.  `sow-data`, `sow-server`, `sow-relay`, and
  the F-Stack example now fail closed when `SOW_DB_SECRET` is absent or empty.
- The pipeline stages the secret through a temporary mode-600 file and removes
  it after activation.  It also restarts the database and server through
  `./sow p`.
- The old credential returned HTTP 401 at `/internal/save`; the new credential
  reached authorization and returned HTTP 404 for a deliberately nonexistent
  account.  This verifies rejection and acceptance without creating data.
- A stale root-owned `/root/shadowsofwar/sow-database` daemon was found
  listening on `*:25585`.  The pipeline was corrected to stop its daemon and
  child before restart.  The current listener is only
  `127.0.0.1:25585` and runs `/srv/sow/current/bin/sow-database` as `sowdb`.
- Relay-to-IONOS database traffic now uses
  `https://shadowsofwar.io`; a post-deploy authenticated probe reached the
  endpoint over HTTPS and returned the expected account-not-found response.
- nginx still publicly proxies `/internal/`
  [shadowsofwar.io.conf](/home/bizkit/Github/shadows-of-war/sow-dist/deploy/freebsd/conf.d/shadowsofwar.io.conf:35).
- Exact public `GET /internal/stats` is now denied with HTTP 404 over both HTTP
  and HTTPS; sow-server continues to query the loopback DB endpoint directly.
- Relay TLS is enabled from the existing certificate for
  `relay.shadowsofwar.io`; all four workers passed the certificate/file check.

### Impact

Before this checkpoint, an attacker who read the open-source repository could
obtain the development fallback and forge trusted server-to-database requests.
That retired value remains visible in old Git history and must never be reused;
the current tree and active services reject it.  The old HTTP transport
permitted interception between Azure and IONOS; the active relay path now uses
HTTPS.

### Completed in this checkpoint

1. Generated and installed a unique secret outside the repository.
2. Removed insecure source fallbacks and made all affected processes fail
   closed when the variable is missing.
3. Updated IONOS, relay, and the deploy pipeline through `./sow p`.
4. Verified old-credential rejection, new-credential acceptance, listener
   binding, service state, and four relay workers with zero restarts.

### Still outstanding

1. Remove public access to the remaining `/internal/*` mutation routes; either
   keep them behind a private channel or require application authentication.
2. Authenticate and restrict any
   operational endpoint that must remain reachable.
3. Inspect logs for prior use of the retired credential.

## Runtime data observations — pre-cleanup baseline

- IONOS Valkey currently contains 78 account records: 77 with provider
  `local` (anonymous guest identities such as `guest_<hex>`) and one `test`
  record created by the audit probe.  The code does not persist whether a
  `local` identity came from a human or a backfill.
- 33 of those accounts currently have nonzero `matches_played`; the 33
  accounts sum to 98 recorded matches (45 accounts have zero; the rest have
  values from 1 through 28).  `local` means the game supplied an anonymous
  guest identity; it does not identify a human versus a backfill.
- The retained database log shows 32 match registrations and 16 finalizations,
  while replay-write lines include duplicates.  It also contains 56
  `Match not registered` finalize errors.  Match IDs near 99,000 are IDs
  processed, not evidence of 99,000 simultaneous sockets.
- Relay journals contain thousands of successful finalization messages, so the
  evidence does not support “all relays died.”  It does show an ordering or
  registration gap for some matches: replay writing can succeed before stats
  finalization, and bot-only matches intentionally skip account-stat updates.
- Replay snapshots and the relay's dead-letter queue were present at the
  baseline and are covered by the reset below.  New code stores only a pointer
  in that queue and removes relay replay/metadata files after a successful DB
  ACK.
- Replay append queues are bounded and apply backpressure instead of silently
  dropping turns.  Both relay and DB enforce a 16 MiB replay ceiling; DB writes
  use temporary files plus fsync and atomic rename before statistics finalize.
- sow-server now awaits successful `/match/start` registration before
  registering the relay and broadcasting `Start`, eliminating the observed
  registration/finalization race for new matches.

## Cleanup checkpoint — 2026-08-09T08:30Z

The application was quiesced first: sow-server and sow-database were stopped
on IONOS, and all four Azure relay workers were stopped.  The following exact
application-state namespaces were then removed:

- IONOS Valkey: 79 player accounts, 79 identity mappings, 6 analytics keys,
  and 1 match key.  Static `sow:leader:*` and `sow:geo:*` catalog keys were
  intentionally retained.
- IONOS Redb: the player-bearing database was replaced with a freshly
  initialized file; the old file was deleted only after the new service seeded
  successfully with static metadata and zero player records.
- IONOS replay spool: 31,376 `.replay` files and 31,376 `.json` sidecars
  removed.
- Azure Redis: the `sow:match_history:dead_letter`, `sow:ports`, and relay
  registration keys were removed; queue length is now zero and memory fell to
  1.39 MiB.
- Azure replay spool: 31,543 `.replay` files and 167 `.journal` files removed.
- Application logs were reset at the same checkpoint: the 7.9 GiB IONOS
  `server.log` and 8.6 MiB `database.log` were truncated; system journal
  history was left intact and should be queried from the checkpoint timestamp.

Post-reset verification: IONOS has zero account, identity, analytics, and
match keys; zero replay files; and static catalog counts of 1,062 geo plus 12
leader records.  Azure has zero replay/journal files, zero Redis keys, empty
dead-letter queue, four active workers with `NRestarts=0`, and empty lobby
rosters on ports 8080–8083.

The reset was deployed and restarted through `./sow p` as release
`0.1.2-6884b4ebcafc`.  To roll back code, use `git revert <this-commit>` and
run `./sow p`; do not restore the retired database secret or the deleted
runtime data.
