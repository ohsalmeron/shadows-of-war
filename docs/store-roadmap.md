# Shadows of War — store decision log

## Decided for the first Android slice

- Main menu entry: `Store`.
- Eight leaders are free in a deterministic weekly rotation.
- A new player receives one random leader from that rotation.
- Leaders outside the rotation are locked until the player unlocks them with 500 laurels or 1,500 gems.
- Leader unlocks offer either currency; the server owns both balances and validates the selected spend.
- The server resolves the selected leader before a match, so a client cannot use a locked leader by editing local state.
- The purchase surface is universal: in-game store → platform checkout/RevenueCat → server grant. It is not tied to CrazyGames or Poki.
- Android gem bundles use Google Play Billing through the native RevenueCat bridge. Product IDs are `sow_gems_500`, `sow_gems_1200`, and `sow_gems_2600`; RevenueCat receives the public profile ID, never the private account ID.
- RevenueCat purchase events are granted by the server and deduplicated by event ID.

## Implemented in the first Android slice

- Three original SOW skins are catalogued, previewed, unlocked with gems, and equipped through authenticated server endpoints.
- Skin prices are 500 / 1,000 / 1,500 gems for the launch catalog; the 2,600-gem Kingdom Vault cannot buy all three skins.
- The end-of-match screen shows the next unowned skin as a featured store offer and can open the store after returning to the menu.

## Current release state

- Android gem bundles are registered in Google Play and attached to the same
  RevenueCat offering as the three Stripe web products. The production AAB
  currently contains the native RevenueCat purchase bridge.
- Web uses the production RevenueCat Purchase Link. Each gem bundle opens its
  matching package checkout and passes the public player ID; a real payment
  has not yet been performed in this audit.
- The Apple App Store app exists in the same RevenueCat project with bundle ID
  `games.shadowsofwar.app`, but App Store Connect credentials, Apple products,
  and the native iOS RevenueCat SDK are not configured yet. TestFlight upload
  alone does not configure purchases.

## Remaining release work

- Validate one Google Play purchase on-device and one Stripe purchase end to
  end, including the idempotent server grant.
- Configure Apple App Store products and App Store Connect credentials in the
  existing RevenueCat app, then add and validate the native iOS purchase UI.
- Balance leader prices after real retention and economy data exists.

The current leaders have gameplay perks. Selling access to them is intentional under the rotation model, but future hackathon/store copy must not claim that every purchase is purely cosmetic.

OpenFrontIO is a behavior reference only. Do not copy its proprietary/premium assets or code; use original Shadows of War assets and the small contract above.
