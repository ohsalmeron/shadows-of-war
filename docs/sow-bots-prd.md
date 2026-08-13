# SOW Bots — Product Requirements Document

Status: Draft  
Date: 2026-07-26  
Replaces: `sow-backfill` and the duplicate `sow-tools::bot-manager`

## 1. Product decision

`sow-backfill` will evolve incrementally into `sow-bots`: one FreeBSD-native
binary that can run on any authorized VPS and participate in a centrally
coordinated population of synthetic players.

The operator experience remains deliberately small:

```sh
./sow bots
```

That command reads one declarative `bots.ron`, validates it, builds and tests
on the local FreeBSD VM, deploys only changed artifacts, starts or preserves
the configured services, and prints an end-to-end health summary. Normal
operation must not require a sequence of manual build, copy, service, or
monitoring commands.

The coordinator owns identities, leases and durable history. Workers only
provide execution capacity. No bot and no worker owns an independent
database.

> **Document status: roadmap, not the live runtime contract.**
>
> This PRD preserves the 2026-07-26 bot product proposal. The running system
> has since changed: bot accounts are persistent `AccountKind::Bot` records,
> anonymous players use the canonical `POST /profile/anonymous` flow, and
> relay admission uses the short-lived `ReadyWithTicket` /
> `ReconnectWithTicket` messages. The `/session/crazygames` and PASETO session
> endpoints described later in this document are future design, not current
> endpoints. For the current contract use `docs/security-audit.md`, the
> protocol definitions, and the deployed pipeline configuration.

## 2. Historical baseline (2026-07-26)

The existing daemon already completes the essential anonymous gameplay path:

1. Connect to the public matchmaking WebSocket.
2. Observe matchmaking lobbies.
3. Pace synthetic joins.
4. Download and validate one map per lobby.
5. Ready all clients.
6. Follow the server handoff to a relay.
7. Apply turns through a shared `SowEngine`.
8. Send gameplay intents.
9. Leave the relay after a configured lifetime.

An observed production run on 2026-07-26 established the initial baseline:

- FreeBSD build and all three unit tests passed through `./sow b`.
- Ten of ten bots joined one Azure lobby.
- Ten of ten connected to relay port `25590`.
- All ten connections remained active for 120 seconds.
- All ten exited cleanly.
- The relay reached zero players, shut down and released its Valkey port.
- Azure remained approximately 99.9% idle with about 3 GiB free.
- ZFS, Valkey, Nginx, Cloudflare Tunnel, `sow-server` and `sow-database`
  remained healthy.

After that bounded characterization, the existing daemon was placed into a
continuous soak with:

```text
allow_empty_lobbies = true
min_fill = 25
max_fill = 45
max_bots_per_lobby = 10
max_match_seconds = 300
max_lobbies = unlimited
```

The soak is the reference behavior for phase-one parity. It must remain
possible to observe active relays, concurrent connections, resource growth,
errors and cleanup while later phases are developed.

### 2.1 Historical limitations (superseded where noted)

- This historical snapshot used process-local `ANON###` bot names. The current
  bot pool seeds persistent `AccountKind::Bot` accounts; this old naming
  behavior is not a live contract.
- `database_account_id` was `None` in this snapshot; this is no longer the
  current bot-account model.
- The current coordinator exists only inside one worker process.
- Multiple VPS instances can select the same lobby and exceed its target.
- The game server identifies synthetic clients as human players.
- A client could declare a `database_account_id` during `Join` in this
  snapshot. The field remains roster/progress metadata in the current wire
  protocol; it is not relay authentication.
- A relay reconnect used only `lobby_id + player_id` in this snapshot. Current
  production requires a relay reconnect ticket.
- Match statistics are declared by the client instead of being fully
  authoritative.
- The CrazyGames public key is fetched for every token verification.
- There is no lease proving that one identity belongs to one match.
- Load metrics do not yet include messages, bytes, intents or latency per bot.
- Operational settings are CLI flags instead of one versioned RON contract.
- The pipeline uploads the binary even when only configuration changed.
- A second bot implementation in `sow-tools` duplicates behavior.

## 3. Goals

### 3.1 Product goals

- Keep matchmaking populated before organic concurrency is sufficient.
- Make synthetic players behave like returning companions with stable
  identity, preferences and career history.
- Introduce new synthetic identities progressively instead of regenerating
  the entire population.
- Guarantee that one identity is never in more than one lobby or match.
- Run workers from local FreeBSD, Clouding, IONOS or future VPS hosts without
  giving those hosts direct database access.
- Reuse the same worker engine for controlled HTTP and WebSocket resistance
  scenarios against Shadows of War infrastructure.
- Preserve a single-command, single-configuration operational surface.
- Authenticate every game connection without adding database work to turns
  or relay messages.
- Keep human and synthetic analytics rigorously separable.

### 3.2 Performance goals

- No coordinator or database request per tick, intent or frame.
- One parsed map per active lobby, not per bot.
- Lease acquisition, renewal and result reporting operate in batches.
- Token verification occurs once when establishing a connection.
- Relay traffic after authentication remains the existing compact binary
  protocol.
- Adding a worker increases bot capacity without requiring another game
  server.
- The architecture must support a target of 5,000 concurrent synthetic
  clients across workers, subject to measured worker and origin capacity.
- Production population mode must apply backpressure instead of creating
  unbounded local tasks when a worker reaches its declared capacity.

## 4. Non-goals

- Bots will not impersonate real CrazyGames accounts, usernames or avatars.
- Synthetic activity will not be submitted to CrazyGames leaderboards or
  counted as organic DAU, retention or acquisition.
- Workers will not receive Valkey, REDB or server signing secrets.
- The first upgrade will not redesign bot strategy or the game AI.
- The first upgrade will not add a new observability platform.
- Fuzzing arbitrary third-party hosts is outside the product. Load targets
  must be explicitly allowlisted Shadows of War endpoints.

## 5. Operator experience

### 5.1 One command

The only normal reconciliation command is:

```sh
./sow bots
```

It performs, in order:

1. Parse and fully validate `bots.ron`.
2. Resolve the current game build version automatically.
3. Verify SSH access and FreeBSD architecture for every configured node.
4. Build and test once on the local FreeBSD build VM when source changed.
5. Generate node-specific runtime configuration.
6. Transfer only binary or configuration files whose hashes changed.
7. Preserve service PIDs when the effective node state is unchanged.
8. Restart only nodes affected by a binary or configuration change.
9. Verify coordinator authentication, worker registration and game reachability.
10. Print one aggregate status report and return non-zero on partial failure.

Setting `enabled: false` in `bots.ron` and running the same command performs a
controlled stop. No separate operator workflow is required.

### 5.2 Configuration

Secrets are never embedded in RON. Configuration refers only to protected
secret files generated or installed by the pipeline.

```ron
(
    schema: 1,
    enabled: true,

    build: (
        host: "freebsd",
        root: "/home/YOUR_USER/shadows-of-war",
    ),

    coordinator: (
        ssh_host: "YOUR_PRODUCTION_HOST",
        public_url: "wss://shadowsofwar.io/bots/ws/",
        service_user: "sowbots",
        signing_key: "/usr/local/etc/sow/bots-signing.key",
    ),

    workers: [
        (
            id: "YOUR_BACKFILL_HOST",
            ssh_host: "YOUR_BACKFILL_HOST",
            capacity: 500,
            identity_key: "/usr/local/etc/sow/bots-worker.key",
        ),
    ],

    game: (
        orchestrator: "wss://shadowsofwar.io/ws/",
        build_version: Auto,
    ),

    mode: Population((
        fill_percent: (min: 25, max: 45),
        max_bots_per_lobby: 10,
        max_match_seconds: 300,
        allow_empty_lobbies: true,
        returning_percent: 70,
        recent_percent: 20,
        new_percent: 10,
        new_identities_per_hour: 60,
        cooldown_seconds: (min: 15, max: 90),
    )),

    safety: (
        max_total_bots: 5000,
        max_error_percent: 10,
        worker_memory_percent: 85,
        origin_memory_percent: 85,
    ),

    telemetry: (
        interval_seconds: 1,
        jsonl: "/var/log/sow/bots.jsonl",
    ),
)
```

All percentages and capacities are validated before any remote mutation.
Population ratios must total 100. Load mode requires explicit duration,
target allowlist and resource ceilings.

## 6. Architecture

```text
                         Azure
                 +-------------------+
                 | sow-bots          |
                 | coordinator       |
                 +---------+---------+
                           |
                 leases / batches / results
                           |
        +------------------+------------------+
        |                  |                  |
  local worker      Clouding worker      IONOS worker
        |                  |                  |
        +--------- HTTP and WebSockets -------+
                           |
                 sow-server / relays

Coordinator -> bot namespace in Valkey and REDB
Workers     -> no direct database access
```

The same `sow-bots` artifact has two roles selected by its rendered runtime
configuration:

- `Coordinator`: authoritative allocation and persistence.
- `Worker`: executes leased players and load scenarios.

Player authentication remains in `sow-database`; it is not split into another
network service. Token encoding and verification live in a small shared
`sow-auth` library used by database, server, relay and coordinator.

### 6.1 Coordinator responsibilities

- Register and revoke workers.
- Maintain worker health and advertised capacity.
- Create, select and retire bot identities.
- Reserve a global number of bot slots for each lobby.
- Acquire, renew and recover exclusive identity leases.
- Issue short-lived bot sessions bound to valid leases.
- Receive idempotent result batches.
- Persist career summaries and synthetic match results.
- Expose aggregate status to the existing private admin surface.

### 6.2 Worker responsibilities

- Authenticate once to the coordinator.
- Request identities and leases in batches.
- Execute lobby and relay connections locally.
- Maintain per-session simulation state.
- Renew leases and publish health in batches.
- Apply local backpressure at declared capacity.
- Report compact results and telemetry.
- Cancel and clean up every supervised task during shutdown.

Workers must not ask the coordinator for decisions during a turn.

## 7. Identity, pool and leasing

### 7.1 Durable identity

```text
BotIdentity
  id
  display_name
  created_at
  last_seen_at
  generation
  status
  skill_rating
  behavior_profile
  preferred_leaders
  preferred_civilizations
  matches_played
  wins
  losses
  kills
  deaths
  assists
  xp
```

Bot names come from a dedicated namespace and are never copied from actual
CrazyGames users.

### 7.2 State machine

```text
Idle -> Reserved -> Joining -> Lobby -> Playing -> Reporting -> Cooldown -> Idle
                      |                                  |
                      +----------> Quarantine <----------+
```

Every transition is explicit and observable.

### 7.3 Exclusive lease

Valkey holds hot coordination:

```text
sow:bot:idle
sow:bot:<bot_id>
sow:bot:lease:<bot_id>
sow:lobby:<lobby_id>:reservations
sow:worker:<worker_id>
```

Lease acquisition is one atomic Valkey function and includes:

- Bot ID.
- Worker ID.
- Lobby and match assignment.
- Expiration.
- Monotonic fencing token.

Heartbeats renew leases in batches. If a worker disappears, leases expire and
the reconciler moves affected identities through cooldown before returning
them to the pool. Any report with an old fencing token is rejected.

The invariant is:

```text
active_leases(bot_id) <= 1
```

It must be enforced by storage, not merely by worker convention.

### 7.4 Durable storage

The existing databases remain shared and logically separated:

```text
REDB
  BOT_IDENTITIES
  BOT_CAREERS
  BOT_MATCH_RESULTS
  BOT_LOAD_RUNS

Valkey
  runtime:real:*
  runtime:bot:*
  sow:bot:lease:*
  sow:lobby:*:reservations
  sow:worker:*
```

Result writes are idempotent by `(match_id, bot_id)`.

## 8. Authentication

Every game connection receives a first-party SOW session. The expensive or
external credential is verified once when minting that session; lobbies and
relays then verify locally without querying REDB, Valkey or CrazyGames.

### 8.1 CrazyGames player

1. The client obtains the current CrazyGames JWT.
2. It sends that JWT once to `/session/crazygames`.
3. `sow-database` verifies RS256, expected game ID and expiration.
4. The CrazyGames public key is cached with a bounded TTL and refreshed after
   a key-related verification failure.
5. The database resolves or creates the SOW account.
6. It returns a short-lived SOW session.
7. `sow-server` derives the account ID from the session.

The server stops trusting a client-provided `database_account_id`.

CrazyGames guests must remain able to start as guests. They receive a
rate-limited first-party guest session without being represented as a
registered CrazyGames account.

### 8.2 Worker

Each worker owns an Ed25519 keypair generated locally with mode `0600`. The
coordinator stores only its public key.

At connection:

1. Coordinator supplies a nonce.
2. Worker signs nonce, timestamp and transcript hash.
3. Coordinator verifies the allowlisted public key and replay window.
4. Coordinator establishes the authenticated control connection.

No reusable database or coordinator master secret is copied to a worker.

### 8.3 SOW session

Use an audited PASETO v4.public implementation rather than a custom token
format. Required claims:

```text
subject
subject_kind = human | guest | bot
audience
session_id
issued_at
expires_at
bot_lease_id     optional
fencing_token    optional
```

Ed25519 verification occurs once during WebSocket establishment. Server and
relay need only the public verification key.

### 8.4 Relay ticket

After validating the SOW session, the orchestrator creates a random
single-match relay ticket. The client receives the ticket in `Start`; the
relay receives only the expected digest through its private launch
configuration.

Relay admission requires:

```text
lobby_id + player_id + relay_ticket
```

The ticket is bound to one match and player, compared in constant time,
excluded from URLs and logs, and destroyed with the relay. This keeps
reconnection inexpensive while preventing player-ID impersonation.

### 8.5 Authoritative results

The relay and game state determine result, kills, deaths and assists.
`SubmitStats` becomes a completion signal or is removed; it cannot overwrite
authoritative values.

## 9. Product modes

### 9.1 Population

Continuous default mode:

- Fill every eligible lobby to its global policy target.
- Prefer returning, recent and new identities by configured ratio.
- Respect identity cooldown and worker capacity.
- Reduce or stop synthetic joins as organic population grows.
- Continue operating across coordinator or worker restarts.

### 9.2 Load

The same workers can run bounded, reproducible resistance scenarios:

- Complete player journey.
- Session/profile endpoints.
- Matchmaking joins.
- Relay-only WebSockets.
- Reconnect churn.
- Sustained messages and intents.
- Slow clients and clean disconnects.
- Invalid, expired and replayed credentials.
- Bounded malformed protocol messages.

Every run has a global ID, fixed random seed, ramp, steady-state, ramp-down
and recovery window. It may target only configured SOW authorities.

Load mode never submits synthetic activity to CrazyGames analytics or
leaderboards.

## 10. Observability

Initial implementation reuses Valkey, the existing private admin endpoint and
rotated JSONL. A new metrics stack is not required for the MVP.

Required dimensions:

- Run ID.
- Worker.
- Bot identity.
- Lobby and match.
- Source mode: population or load.
- Actor kind: human, guest or bot.

Required measurements:

- Idle, reserved, joining, lobby, playing, cooldown and quarantined bots.
- Leases acquired, contended, expired and rejected by fencing.
- Active orchestrator and relay WebSockets.
- Messages, intents, turns and bytes per second.
- Join, lobby-start, relay and turn latency.
- Reconnects, timeouts and protocol failures.
- Bots and humans per lobby and match, separately.
- Worker and origin CPU, memory, sockets and network throughput.
- Valkey operations, memory, blocked clients, evictions and errors.
- REDB batch latency, logical size and physical growth.
- Relays spawned, cleaned and left orphaned.

`./sow bots` prints an aggregate post-reconciliation view. Detailed status
remains private.

## 11. Safety and failure behavior

- Invalid RON causes no remote changes.
- A partially reachable worker does not block healthy workers.
- A worker at capacity receives no new leases.
- Coordinator loss does not interrupt already authenticated active matches.
- Workers stop acquiring new work while the coordinator is unavailable.
- Expired leases cannot report results.
- A load scenario stops acquiring work when configured error or resource
  ceilings are crossed.
- Population mode remains active unless disabled in configuration.
- Abrupt worker termination must eventually leave no orphaned relay,
  reservation, port or permanent lease.

## 12. Delivery phases

Each phase is independently deployable and must preserve the continuous
population behavior established by the soak.

### Phase 0 — Observe the existing soak

- Keep the current daemon running continuously.
- Record concurrent matches, relays, connections and bots.
- Measure Clouding and Azure CPU, memory and network growth.
- Record join failures, turn gaps, relay exits and cleanup.
- Establish the practical ceiling of the existing implementation.

Exit criteria:

- At least one long-running observation covering repeated lobby cycles.
- Resource and error curves are captured.
- No unexplained orphan relay or port remains after bot expiry.

### Phase 1 — Rename, RON and one command

- Rename the product and binary to `sow-bots`.
- Replace operational flags with `bots.ron`.
- Make `./sow bots` the only reconciliation path.
- Preserve existing gameplay behavior exactly.
- Hash binary and configuration independently.
- Avoid builds, transfers and restarts when their inputs are unchanged.
- Remove `sow-tools::bot-manager` only after parity tests pass.

Exit criteria:

- The same settings produce the same pacing and bot count.
- An unchanged invocation preserves all service PIDs.
- A config-only change transfers no binary.
- Invalid config touches no machine.
- The operator uses only `./sow bots`.

### Phase 2 — Authenticate every connection

- Add shared `sow-auth`.
- Mint SOW sessions for CrazyGames players and guests.
- Cache the CrazyGames verification key safely.
- Authenticate workers with Ed25519.
- Bind bot sessions to leases.
- Require relay tickets.
- Derive account identity server-side.

Exit criteria:

- Forged, expired or wrong-audience sessions fail before lobby assignment.
- Invented account IDs cannot modify another profile.
- A relay ticket from another player or match fails.
- Normal token verification performs no database or external request.

### Phase 3 — Central coordinator and pool

- Deploy coordinator role on Azure.
- Add durable bot identities and atomic leases.
- Reserve global lobby capacity.
- Convert Clouding to the first worker.
- Add local and other VPS workers one at a time.

Exit criteria:

- Concurrent workers cannot lease the same bot.
- Concurrent workers cannot exceed a lobby's global target.
- Killing a worker recovers every lease after TTL.
- Restarting the coordinator neither duplicates nor loses identity state.
- Workers hold no direct database credential.

### Phase 4 — Career and behavior

- Persist career history.
- Add reproducible behavior profiles.
- Implement cooldown, rotation and progressive identity creation.
- Make match result writes idempotent.
- Compute results from authoritative state.

Exit criteria:

- A returning bot retains name, preferences and record.
- One completed match increments a career exactly once.
- Retried reports do not duplicate XP.
- Synthetic profiles remain absent from organic analytics and CrazyGames
  leaderboards.

### Phase 5 — Distributed resistance testing

- Add RON load scenarios.
- Coordinate multiple sources with one run ID.
- Capture endpoint and WebSocket performance.
- Add ramp, steady-state, recovery and abort thresholds.
- Produce a reproducible summary after each run.

Exit criteria:

- Local, Clouding and IONOS can contribute to one coordinated run.
- Reports include connections, packets/messages, bytes, latency percentiles
  and failures by source.
- A completed run cleans every session, lease, relay and port.
- Production returns to its measured baseline after load removal.

## 13. MVP acceptance

The first release called `sow-bots` is accepted when:

- `./sow bots` is the only command needed.
- A single `bots.ron` fully describes desired deployment and operation.
- One coordinator and at least two workers coordinate without overfill.
- Every human, guest and bot connection is authenticated.
- One bot identity cannot enter two matches.
- Returning bots preserve identity and career.
- Existing continuous population behavior remains stable.
- No database credential or signing key exists on a worker.
- Synthetic and organic statistics are separable at every storage and
  reporting boundary.
- A no-op reconciliation builds nothing, transfers nothing and restarts
  nothing.

## 14. External platform constraint

CrazyGames requires its user token to be sent to the backend and verified
there before using the contained user ID. Its documented token lifetime is
one hour, and guest play must remain available. SOW will follow that flow,
then issue its own short-lived session so CrazyGames verification is not on
the gameplay hot path.

Synthetic identities are SOW-owned and are never presented as actual
CrazyGames accounts.
