# Shadows of War — Shipaton 2026

Operational checklist for eligibility, Google Play, RevenueCat, monetization, and submission.

**Last reviewed:** September 6, 2026
**Deadline:** September 30, 2026 at 11:45 p.m. PDT
**Category:** Best Game Award
**Android package:** `com.shadowsofwar`

This is an evidence log, not a place for passwords, service-account JSON, API secrets, or tokens.
Update status from Play Console, RevenueCat, and Devpost; do not preserve assumptions as facts.

## Current state

| Area | Verified state | Next proof required |
|---|---|---|
| Android target | `compileSdk 36`, `targetSdk 36` | Upload a new AAB so Play re-evaluates the old edge-to-edge warning |
| Play Alpha | `0.1.2`, versionCode `38` | Local smoke test, then `./sow a` should publish `0.1.3` / code `39` |
| Play products | Three one-time products active | Test a purchase through the Android app |
| RevenueCat | Android app connected; default offering has the three Play products | Verify purchase, grant, and restore end to end |
| Web payments | Stripe-backed RevenueCat purchase link exists | Verify checkout and server grant with a real authorized test |
| Identity | WOU-ID is the web/cross-platform identity service; SOW owns game progress | Decide Android Play Games integration separately |
| Production access | Controlled by the current Play Console testing requirement | Read the live closed-testing state before applying |

## Release pipelines

- `./sow p` deploys Web, backend, database, and relay. It never uploads Android.
- `./sow a` builds the signed Android AAB, runs the USB smoke test, validates it with Play, and
  uploads the configured Play track.
- `./sow l` is local Web/WASM preview only.
- `./sow` or `./sow native` launches the native desktop client.

Android versioning is independent from Web deployment. The Android release path must advance both
the semantic `versionName` and the Play `versionCode`; never reuse an uploaded AAB.

## Android catalog

Google Play and RevenueCat are separate states. The current Play products are active:

| Product | Product ID | Price | Type |
|---|---|---:|---|
| Scout’s Cache | `sow_gems_500` | $1.99 | Consumable, 500 gems |
| War Chest | `sow_gems_1200` | $4.99 | Consumable, 1,200 gems |
| Kingdom Vault | `sow_gems_2600` | $9.99 | Consumable, 2,600 gems |

Authoritative Play verification is a successful `GET` for each product under
`com.shadowsofwar`. A product being `active` in RevenueCat alone is not enough.

## RevenueCat

RevenueCat's active default offering contains the three Android Play consumables and their web
counterparts. The Android public SDK key may be shipped in the app; private API keys and service
account files must stay outside the repository.

Required end-to-end behavior:

1. The player opens Store.
2. Android uses Google Play Billing through RevenueCat.
3. RevenueCat reports the transaction to the server webhook.
4. The server grants gems idempotently to the correct game account.
5. Reopening the game does not duplicate the grant.
6. Restore/reconciliation recovers a valid purchase.

Consumable gem bundles are not entitlements. The server owns the balance and validates every grant.
Leaders use the rotation/laurel system; original SOW skins use gems.

The web checkout is a separate Stripe-backed RevenueCat flow. Do not create a second Stripe account,
second RevenueCat project, duplicate products, or a second checkout architecture.

## Identity architecture

- **WOU-ID:** canonical identity for the web, Hyper, and future cross-platform account linking.
- **SOW database:** game-specific profile, progress, and economy keyed by the canonical WOU account ID.
- **Android Play Games Services:** a platform layer for the Android Play Games profile, achievements,
  and leaderboards. It is not automatically the game's primary account.

These are intentionally separate systems. The SOW server validates WOU-ID tokens through the WOU-ID
API; it does not share JWT signing secrets or duplicate the identity database.

The Android Play Games decision is documented separately from the current WOU-ID flow. Do not bind a
Play Games Player ID to a SOW account until the account-linking rules are explicitly chosen.

## Closed testing and Production

Use the live Play Console state, not an old screenshot, for these values:

- Is the closed test published?
- How many testers are currently opted in?
- Have the required testers remained opted in for the required period?
- Is **Apply for production** enabled?
- Is United States targeted in Production?

When Play enables the Production application:

1. Submit the closed-test feedback and production-readiness answers.
2. Create the first Production release with the next verified `versionCode`.
3. Confirm countries, listing, privacy policy, Data safety, content rating, app access, and ads
   declarations.
4. Roll out to Production and verify **Published/Available**, not merely **In review**.

Do not infer the tester count or completion date from a prior release. Record the exact values shown
by Play Console.

## Shipaton submission

The submission must be in English and must show the product working on a real device.

Core message:

> **Every match becomes a war story.**

Show this loop:

1. Choose a leader.
2. Claim territory.
3. Manage resources.
4. Expand and fight.
5. Form an alliance or betray it.
6. Win, lose, or recover from a memorable war.

Video requirements:

- Public YouTube or Vimeo link.
- Less than two minutes.
- Real gameplay on a real device.
- English subtitles or narration.
- No unlicensed music or copyrighted material.
- Show the store and purchase/restore flow without exposing secrets.

Suggested 120-second structure:

| Time | Content |
|---|---|
| 0–8 s | Invasion, betrayal, or comeback; “Every match becomes a war story.” |
| 8–25 s | Leader selection and world map |
| 25–65 s | Economy, expansion, combat, and defense |
| 65–90 s | Alliance, betrayal, and outcome |
| 90–108 s | Universal store: leaders, gems, and original skins |
| 108–120 s | RevenueCat purchase/restore and “Play your first war.” |

Do not invent metrics. If a metric is not measured, write **not yet measured** and state how it will
be instrumented.

## Evidence log

```text
Play Console app URL:
Play account type:
Play account creation date:
Highest uploaded versionCode:
Current track:
Closed test published:
Current opted-in testers:
Continuous tester period complete:
Apply for production enabled:
Production access approved:
First Production release date/time:
Production versionCode:
United States available in Production:

RevenueCat project:
RevenueCat Android app/package:
Google Play connection:
Active offering:
Android product IDs:
Public Android SDK key stored in development:
Android purchase verified:
Server grant verified:
Restore verified:
Web checkout verified:

Public video URL:
Devpost draft URL:
Devpost submission URL:
Submission date/time:
```

## Final checklist

### Google Play

- [ ] Dashboard has no blocking errors.
- [x] Package is `com.shadowsofwar`.
- [x] Android target is API 36 in source.
- [ ] New Android release is uploaded and verified on-device.
- [ ] Store listing, privacy policy, Data safety, content rating, app access, and ads declaration are complete.
- [ ] Required closed-testing period is complete, if Play requires it.
- [ ] Production access is approved, if Play requires it.
- [ ] United States is available in Production.
- [ ] Release is Published/Available.

### RevenueCat

- [x] Google Play products exist and are active.
- [x] Android products are attached to the active offering.
- [ ] Android purchase works in a permitted test environment.
- [ ] Web checkout works through the existing Stripe-backed link.
- [ ] Server grant is idempotent and tied to the correct account.
- [ ] Restore/reconciliation works.
- [ ] No private key is in source, the AAB, video, or Devpost.

### Submission

- [ ] Description is in English.
- [ ] Gameplay and art direction are explained.
- [ ] Monetization is explained without making the game pay-to-play.
- [ ] Public video is under two minutes.
- [ ] Public Google Play URL is available.
- [ ] Judge access or a safe premium demo is ready.
- [ ] Submission is sent before the deadline.

## Official sources

- [Shipaton rules](https://revenuecat-shipaton-2026.devpost.com/rules)
- [Shipaton FAQ](https://shipaton.com/faq)
- [Google Play testing requirements](https://support.google.com/googleplay/android-developer/answer/14151465?hl=en)
- [Google Play testing tracks](https://support.google.com/googleplay/android-developer/answer/9845334?hl=en)
- [Google Play release preparation](https://support.google.com/googleplay/android-developer/answer/9859348?hl=en)
- [Google Play payments policy](https://support.google.com/googleplay/android-developer/answer/10281818?hl=en)
- [RevenueCat Android SDK](https://www.revenuecat.com/docs/getting-started/installation/android)
- [RevenueCat Google Play products](https://www.revenuecat.com/docs/getting-started/entitlements/android-products)
- [RevenueCat products, entitlements, and offerings](https://www.revenuecat.com/docs/projects/configuring-products)
