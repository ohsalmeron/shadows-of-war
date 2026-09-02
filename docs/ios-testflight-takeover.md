# iOS/TestFlight takeover record

Status: operational record, updated 2026-09-02.

This document records the boundary between the iOS workflow on macOS and the
Shadows of War production pipeline. It also records the evidence standard for
TestFlight claims so later agents do not confuse an upload with a testable build.

## Scope and pipeline boundary

- iOS-only work runs on macOS with Xcode through
  `scripts/ios-testflight.sh`.
- `./sow p` is the production pipeline for the web/backend/Android/FreeBSD
  release. It is not the iOS validation command and must not be invoked merely
  to validate an iOS-only change on a Mac.
- Do not create a FreeBSD VM, provision production keys, or configure Linux or
  production access as an implicit workaround for an iOS build.
- The iOS upload requires the Mac's Xcode signing identity/account or another
  explicit App Store Connect upload credential. The FreeBSD/Linux `sow p`
  credentials are not a substitute.

## Known Shadows of War upload

- Bundle ID: `games.shadowsofwar.app`
- App Store Connect app: Shadows of War
- Marketing version: `0.1.2`
- Build: `402`
- The archive/export path was repaired to register `Assets.xcassets` with
  `AppIcon`, include the required iPhone/iPad icon sizes, and declare
  `CFBundleIconName`.
- The observed Xcode result was `Upload succeeded` and App Store Connect
  created the `Build402` record under version `0.1.2`.

That result proves receipt/acceptance of the package only. It does not prove
that the build is ready for testing.

## App Store Connect status meanings

Use the status shown in App Store Connect as the source of truth:

| Status | Meaning for this project |
| --- | --- |
| `Missing Compliance` | Export-compliance information is missing; action is required and the build is not ready for testing. |
| `Waiting for Export Compliance Review` / `In Compliance Review` | Compliance information was submitted and Apple is reviewing it. |
| `Ready to Submit` | The build can be distributed to internal testers or sent to TestFlight review for external testing. |
| `Ready to Test` | The build can be tested by internal and external testers. |
| `Testing` | At least one tester/group is using the build. |

Never report “Apple is processing it”, “it is in TestFlight”, or “testers can
install it” based only on an Xcode upload log. Verify the current build status
in App Store Connect or through a working authenticated App Store Connect API
client.

Apple's documented flow for a build marked `Missing Compliance` is Apps → the
app → TestFlight → the platform → the build → `Manage` / `Provide Export
Compliance Information`. Apple also documents API resources to create and
assign app-encryption declarations, but API requests require JWT authorization
with an App Store Connect API key.

The questionnaire may next ask whether the app will be distributed in France.
That answer comes from the product's intended distribution countries, not from
the Rust or Xcode source. Stop and obtain the owner's decision rather than
guessing a country or making a legal/export declaration from code alone.

## Encryption evidence

The client uses standard TLS through Rustls for WebSocket connections:

- `Cargo.toml` enables `tokio-tungstenite` with the `rustls-tls-webpki-roots`
  feature.
- The client constructs `wss://` relay URLs in
  `sow-client/src/net/update/mod.rs`.

This is evidence for why Apple presents an export-compliance determination; it
is not a legal classification. Do not guess an exemption or documentation
answer on the user's behalf. If automation is desired, use an authenticated
App Store Connect API client with the required role and key, or complete the
Apple questionnaire in the account.

## Agent-Reach boundary

Agent-Reach was installed outside this repository for research. Its documented
purpose is internet access through web and upstream channels such as GitHub,
YouTube, RSS, Reddit, and X. It has no App Store Connect, TestFlight, Apple
Developer, or Xcode channel.

Its web backend can read public Apple documentation and verify claims about
Apple's workflow. It cannot authenticate to or prove the private state of this
team's App Store Connect account. Do not treat a public-web result, an
`agent-reach doctor` report, or a local Xcode log as proof of private ASC state.

## Evidence checklist before reporting completion

1. Confirm the archive/export completed and record the exact build number.
2. Confirm Xcode says the upload succeeded.
3. Open the build in App Store Connect and record its current status.
4. Complete or verify export compliance.
5. Confirm the status is `Ready to Test` or `Testing` and that a tester/group
   has access.

Only after step 5 may an agent say that this build is available in TestFlight.
