# Google Play Console — Shadows of War takeover

**Last audited:** September 6, 2026
**Package:** `com.shadowsofwar`
**Google Cloud project:** `worldofunreal`
**Service account:** `play-deploy@worldofunreal.iam.gserviceaccount.com`

This guide lets another agent continue Play Console and release work without repeating the audit.
Never store passwords, private keys, service-account JSON, or tokens here.

## Operating rules

- Use an API or CLI whenever Google supports the operation.
- Use the browser only when the public API does not expose the write operation or Google requires
  an owner confirmation.
- `./sow p` is the Web/backend/infra pipeline.
- `./sow a` is the Android/Play pipeline.
- Never replace either pipeline with manual `scp`, `rsync`, SSH activation, symlink swaps, or service
  restarts.

## Tester countries

The relevant Play country codes are:

| Country | Code |
|---|---|
| Indonesia | `ID` |
| Kenya | `KE` |
| Nigeria | `NG` |
| Philippines | `PH` |
| Vietnam | `VN` |

When a tester sees “This app isn’t available in your region”, check in this order:

1. The Alpha track targets the tester's Play country.
2. The exact Google account is in the Alpha tester list.
3. The tester opted in through the Alpha link.
4. The tester is not relying on Internal testing instead of Alpha.
5. The Google Play account country matches the intended market.
6. Only then wait for propagation.

The opt-in link is:

```text
https://play.google.com/apps/testing/com.shadowsofwar
```

Being in the email list is not the same as opting in. The tester must open the link with the listed
account and select **Join the test**. Later releases on the same track do not normally require a new
opt-in.

Country targeting for a closed track may require the Play Console UI; the public Publisher API is
primarily useful for reading release and availability state. Do not claim a country change succeeded
until Play Console or the API confirms it.

## Current Android release evidence

Re-read these values before every release; do not copy them from an old screenshot:

- Alpha: `0.1.2`, versionCode `38`.
- Internal: versionCode `3`.
- Beta, open, and production: no release verified in the last audit.
- Source and installed app target API 36.
- The old Alpha release still carries Play's edge-to-edge warning. A new AAB must be uploaded before
  Play can re-evaluate it.
- The next local Android pipeline run is expected to advance `.version` from `0.1.2` to `0.1.3` and
  choose the next unused Play versionCode after the device smoke test passes.

## Play product catalog

These one-time consumable products are active in Google Play and attached to the Android RevenueCat
offering:

| Product | ID | Price | Grant |
|---|---|---:|---:|
| Scout’s Cache | `sow_gems_500` | $1.99 | 500 gems |
| War Chest | `sow_gems_1200` | $4.99 | 1,200 gems |
| Kingdom Vault | `sow_gems_2600` | $9.99 | 2,600 gems |

The authoritative check is a successful Google Play Publisher API `GET` for each product under
`com.shadowsofwar`. RevenueCat cannot create a missing Play product.

## RevenueCat state

- Project: `projf6bc119d`.
- Play app: `appc659dd11dd`, package `com.shadowsofwar`.
- Apple app: `app44fef55941`, bundle `games.shadowsofwar.app`.
- Stripe Web app: `app5ab74d80c4`.
- Current offering: `default`, active.
- Android products: `sow_gems_500`, `sow_gems_1200`, `sow_gems_2600`.
- Production Web Purchase Link: `https://pay.rev.cat/dplwcwropdytvyzt/`.

RevenueCat package keys `$rc_monthly`, `$rc_annual`, and `$rc_lifetime` are legacy package names;
the products are consumable gem bundles, not subscriptions. Do not create a `pro` entitlement for
these bundles. The server grants gems from the purchase event and must remain idempotent.

The web link uses the existing Stripe-backed RevenueCat configuration. Do not create another Stripe
account, RevenueCat project, product set, or checkout flow.

## What can be automated

With the existing service account, an agent can read:

- tracks, releases, and version codes;
- country availability;
- product existence and state;
- AAB package name, version, and Play validation results;
- post-release health and public verification through the official pipeline.

An agent must not:

- print secrets or service-account contents;
- reuse a versionCode;
- upload a service-account JSON with an AAB;
- apply for Production while Play keeps the button disabled;
- claim a tester is opted in merely because the email is on a list;
- edit country targeting through an undocumented API path.

## Official API and read-only tooling

OAuth scope:

```text
https://www.googleapis.com/auth/androidpublisher
```

Local credential path (never copy it into the repository):

```text
/home/bizkit/.config/shadows-of-war/google-play-service-account.json
```

Fastlane path used by the repository:

```text
/home/bizkit/.local/share/gem/ruby/3.4.0/bin/fastlane
```

Read a track without deploying:

```sh
/home/bizkit/.local/share/gem/ruby/3.4.0/bin/fastlane \
  run google_play_track_version_codes \
  package_name:"com.shadowsofwar" \
  track:"alpha" \
  json_key:"/home/bizkit/.config/shadows-of-war/google-play-service-account.json"
```

Use the official pipelines for changes:

```sh
./sow p       # Web/backend/infra
./sow a       # Android/Google Play
./sow l       # local Web/WASM preview
./sow native  # native desktop client
```

## Opt-in procedure

For each final tester:

1. Add the exact Google account to Alpha testers.
2. Share the opt-in link.
3. The tester opens it with that same account.
4. The tester selects **Join the test**.
5. The tester installs Shadows of War from Google Play.
6. Confirm the account under Alpha → Testers or tester statistics.

Internal opt-in does not satisfy a closed-test requirement. Open testing is not a substitute for a
closed test when Play explicitly requires one.

## Safe takeover checklist

```text
[ ] Read this document completely.
[ ] Confirm repository, branch, and git status before touching files.
[ ] Read Alpha release, versionCode, countries, and tester state.
[ ] Separate “on the list” from “opted in”.
[ ] Verify ID, KE, NG, PH, and VN if a tester reports a region error.
[ ] Do not ask for RevenueCat login again while the authenticated profile is valid.
[ ] Use ./sow p for Web/backend/infra changes.
[ ] Use ./sow a for Android/Play changes.
[ ] Verify the live result and record the date, track, and versionCode.
```

## Evidence log

```text
Play Console app URL:
Account type:
Account creation date:
Current Alpha version/versionCode:
Current opted-in testers:
Continuous closed-test period complete:
Apply for production enabled:
Production access approved:
First Production release date/time:
Production versionCode:
United States available in Production:

RevenueCat project:
RevenueCat Android app/package:
Active offering:
Android product IDs:
Android purchase verified:
Server grant verified:
Restore verified:
Web checkout verified:

Public video URL:
Devpost URL:
Submission date/time:
```

## Sources

- [Google Play testing tracks](https://support.google.com/googleplay/android-developer/answer/9845334?hl=en)
- [Google Play country distribution](https://support.google.com/googleplay/android-developer/answer/7550024?hl=en)
- [Android Publisher API: country availability](https://developers.google.com/android-publisher/api-ref/rest/v3/edits.countryavailability/get)
- [Android Publisher API: tracks](https://developers.google.com/android-publisher/api-ref/rest/v3/edits.tracks)
- [RevenueCat Web Billing](https://www.revenuecat.com/docs/web/web-billing/overview)
- [RevenueCat Web Purchase Links](https://www.revenuecat.com/docs/web/web-billing/web-purchase-links)

## Handoff prompt

```text
Take over Google Play Console for Shadows of War.
Read docs/play-console-takeover.md first.
Package: com.shadowsofwar. Track: Closed testing / Alpha.
First report: release/versionCode, country targeting, tester-list count versus opted-in count,
and the exact remaining Production requirement.
Use the existing API/CLI authentication without printing secrets.
Use ./sow p for Web/backend changes and ./sow a for Android/Play changes.
Do not deploy manually or create duplicate products/projects.
```
