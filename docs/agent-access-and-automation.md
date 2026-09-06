# Agent Access and External Automation

Last verified: 2026-09-06

## Operating rule

Before asking the owner to click, copy a key, or fill a form, the agent must
inspect the connected MCP servers, installed plugins, available CLIs, and
browser capabilities. Use an authenticated API or CLI path when one exists.
Opening a browser page is not the same as controlling the page. If browser
control is unavailable, ask only for the missing authorization or bootstrap
action and state the exact boundary.

The owner authorizes external actions. The agent performs the resulting work
through the API, CLI, or MCP whenever that path is available.

The owner must never be asked to run a command that the agent can run with an
already authenticated credential. This includes enabling Google APIs,
checking service access, and running the official project pipelines. Ask only
for an owner-only authorization or Console bootstrap when no API, CLI, MCP, or
controllable browser path exists. A physically locked Android device is the
only current local-test boundary: the agent may wake it, but cannot unlock it
without the owner's device credential.

## Google Play and Play Games capability matrix

| Area | Official interface | Automation available here | Boundary |
|---|---|---|---|
| Android releases, tracks, listings, testers, IAP | Google Play Android Publisher API v3 | `./sow a` and `fastlane`; `gpc` CLI can also operate the API | Android uploads stay in `./sow a`; `./sow p` never uploads Android |
| Play Games configuration | Play Games Services Publishing API v1configuration | `gpc games` can manage achievement and leaderboard configurations | The API does not cover the complete Play Console bootstrap; event setup remains a Console operation |
| Play Games runtime data | Play Games Services API v1 | `gpc games runtime` is read-only and requires the `games` OAuth scope | It is not the same API as configuration/publishing |
| Play Games management | Play Games Services Management API v1management | Direct REST/client-library access is possible when the correct user OAuth scope exists | It is not exposed by an installed MCP in this session |
| Play Games Android authentication | Play Games Services v2 SDK | Implemented in the Android launcher and server handoff | Requires the Play Console game configuration and server OAuth web client |

## Verified local tools

- `gcloud` is installed and authenticated for the `worldofunreal` project.
- A Play deployment service-account credential exists locally with mode `0600`
  and can reach the Android Publisher API.
- `gpc` was verified through `npx @gpc-cli/cli@0.9.96`. It is MIT-licensed,
  open source, and exposes `gpc games achievements` and
  `gpc games leaderboards`. It is a CLI, not an MCP server.
- `gamesconfiguration.googleapis.com` and `games.googleapis.com` are enabled
  in the `worldofunreal` Cloud project. The Cloud project number is not the
  Play Games `applicationId`; the latter must come from the Play Games
  configuration.
- `gpc games runtime` needs a user OAuth token with the `games` scope. The
  existing Play deployment service account is valid for Play Publisher access,
  but its current token does not provide that runtime scope.
- The active MCP inventory contains RevenueCat. No Google Play or Play Games
  MCP is connected in this Codex session.
- The agent can use the existing controlled browser session for the small Play
  Games Console bootstrap surface that is not exposed by the APIs/CLI. Do not
  delegate those clicks to the owner when the agent has that control.

## Current Play Games state

The Play Console route for Shadows of War is:

`https://play.google.com/console/u/0/developers/8869397679256420631/app/4974190480489596523/games/configuration`

The game is linked to the existing Google Cloud project `worldofunreal`
(project number `285304985776`). No duplicate Cloud project was created.

Verified configuration:

- Play Games application ID: `285304985776`.
- Web/server OAuth client is configured; its secret is stored only in ignored
  local/production environment files.
- Android OAuth credentials exist for the debug SHA-1 and the Play-signed
  release SHA-1 for package `com.shadowsofwar`.
- OAuth consent is published; `ohsalmeron@gmail.com` is a test user.
- Match event ID: `CgkIsLGC7KYIEIAQw`.
- First Victory achievement ID: `CgkIsLGC7KYIEAIQBA` (draft).
- Battle Hardened achievement ID: `CgkIsLGC7KYIEAIQBg` (draft, 10 matches).
- Victory March achievement ID: `CgkIsLGC7KYIEAIQBw` (draft, 5 victories).
- Laurel Hoard achievement ID: `CgkIsLGC7KYIEAIQCA` (draft, 500 laurels).
- First Command achievement ID: `CgkIsLGC7KYIEAIQCQ` (draft).
- Commander Victorious achievement ID: `CgkIsLGC7KYIEAIQCg` (draft).
- Veteran Commander achievement ID: `CgkIsLGC7KYIEAIQCw` (draft, 10 wins with one leader).
- Banner Collector achievement ID: `CgkIsLGC7KYIEAIQDA` (draft, 5 distinct leaders).
- Leader Path achievement ID: `CgkIsLGC7KYIEAIQDQ` (draft, 10 leader-backed matches).
- Victories leaderboard ID: `CgkIsLGC7KYIEAIQBQ` (draft).
- `./sow p` propagates the event, achievement, and leaderboard IDs to all
  production service environments and verifies configuration drift.

The Play Games v2 Android launcher authenticates before starting the TWA,
exchanges a one-use server auth code with the backend, and does not provide a
guest fallback. The authoritative server submits match events, achievement
increments/unlocks for matches, victories, laurels, and leader milestones, plus
cumulative victory scores, only for verified Play Games sessions. No parallel
achievement counters are stored; all thresholds are derived from the existing
authoritative profile and finalized match reward.

## Tool selection

Use the smallest verified path:

1. `gpc` or a direct official Google API for read-only/configuration work.
2. `./sow a` for Android build, device validation, and Play upload.
3. `./sow p` only for Web/backend/infra production deployment.
4. Agent-controlled browser interaction for the small Play Games Console
   bootstrap surface that is not exposed by the APIs/CLI. Do not delegate
   those clicks to the owner when the agent has a controllable session.

Do not install duplicate Play Console MCPs just because they have overlapping
release/listing tools. A general Play Console MCP does not automatically
provide Play Games configuration or Play Games v2 authentication setup.

## Google documentation

- [Play Games Services Publishing API overview](https://developer.android.com/games/pgs/publishing/publishing)
- [Publishing API setup](https://developer.android.com/games/pgs/publishing/publishing-start)
- [Play Games Services Publishing API reference](https://developer.android.com/games/services/publishing/api)
- [Play Games Services Management API](https://developers.google.com/games/services/management/api)
- [Google Play Developer API](https://developers.google.com/android-publisher)
- [Server-side access to Play Games Services](https://developer.android.com/games/pgs/android/server-access)
- [Open-source `gpc` CLI](https://github.com/yasserstudio/gpc)
