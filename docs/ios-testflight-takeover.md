# iOS/TestFlight takeover record

Status: operational record, updated 2026-09-04.

This document records the boundary between the iOS workflow on macOS and the
Shadows of War production pipeline. It also records the evidence standard for
TestFlight claims so later agents do not confuse an upload with a testable build.

## Scope and pipeline boundary

- iOS-only work runs on macOS with Xcode through
  `scripts/ios-testflight.sh`.
- `./sow p` is the production pipeline for the web/backend/FreeBSD/Azure
  release. It is not the iOS validation command and must not be invoked merely
  to validate an iOS-only change on a Mac.
- Do not create a FreeBSD VM, provision production keys, or configure Linux or
  production access as an implicit workaround for an iOS build.
- The iOS upload requires the Mac's Xcode signing identity/account or another
  explicit App Store Connect upload credential. The FreeBSD/Linux `sow p`
  credentials are not a substitute.

## Current Shadows of War state

- Bundle ID: `games.shadowsofwar.app`
- App Store Connect app: Shadows of War
- Marketing version: `0.1.2`
- The last previously uploaded build was `0.1.2 (403)`; build `402` remains
  `Missing Compliance`. Neither status is evidence that the newly fixed binary
  is in TestFlight.
- The reconciled source completed a signed local archive/export as
  `0.1.2 (409)` on 2026-09-04. Xcode reported `ARCHIVE SUCCEEDED` and
  `EXPORT SUCCEEDED`; nothing was uploaded.
- A previous experiment built a standalone
  `@rpath/libSOWRevenueCatBridge.dylib`; that design is retired. The current
  design links `SOWStoreBridge.swift` and RevenueCat directly in the Xcode
  target, with no manual dylib or IPA rewriting.
- Physical-device launch, product loading, sandbox purchase, and post-webhook
  gem refresh still require an attached iPhone or iPad.
- Three App Store Connect consumables exist as drafts:
  `sow_gems_500` (`6808291603`), `sow_gems_1200` (`6808291825`), and
  `sow_gems_2600` (`6808294808`).
- All three consumables now have verified availability in 173 of 175 Apple
  territories: all selectable territories except Russia and Yemen. Iran, North
  Korea, and Somalia are not selectable in this App Store Connect list.
- The iOS version page now contains one real gameplay screenshot for iPhone
  6.5-inch and one for iPad 13-inch. Both survived a page reload.
- The native iOS RevenueCat bridge and temporary in-game store entry are
  present in source. The public `appl_...` SDK key is supplied through the
  signed build configuration; it is not hardcoded in source.

There is currently no evidence that build `407` has reached Apple, has been
processed, or is available to testers. The only valid evidence for that claim
will be the build status in App Store Connect after a real upload.

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

The client uses platform-specific standard TLS for WebSocket connections:

- iOS uses `tokio-tungstenite` with `native-tls`; the archived IPA links
  `/System/Library/Frameworks/Security.framework/Security`.
- Other native targets retain `rustls-tls-webpki-roots`.
- The client constructs `wss://` relay URLs in
  `sow-client/src/net/update/mod.rs`.
- The final IPA contains `ITSAppUsesNonExemptEncryption = false`.

The local exported IPA was inspected after this implementation and plist
declaration were verified. It has not been uploaded. This is technical
evidence for the non-exempt-encryption declaration, not a substitute for legal
advice.

## Store metadata and screenshots

- The iOS draft has a description, keywords, support URL, marketing URL, and
  copyright saved from the repository's existing launch/site copy.
- The local screenshot artifacts are
  `dist/ios/screenshots/ShadowsOfWar-iPhone-6.5.png` and
  `dist/ios/screenshots/ShadowsOfWar-iPad-13.png`. They were generated from
  real gameplay frames, validated at Apple's accepted dimensions, uploaded
  through the authenticated App Store Connect browser, and rechecked after a
  reload.
- Store screenshots are separate from TestFlight build processing: their
  presence does not make an unuploaded build testable.

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
