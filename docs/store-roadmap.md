# Shadows of War — store decision log

## Decided for the first Android slice

- Main menu entry: `Store`.
- Eight leaders are free in a deterministic weekly rotation.
- A new player receives one random leader from that rotation.
- Leaders outside the rotation are locked until the player unlocks them with laurels.
- Leader unlocks cost laurels; the server owns the balance and validates the spend.
- Future leader releases may be gems-only during a timed launch window, then move to laurels; this is intentionally not part of the first slice.
- The server resolves the selected leader before a match, so a client cannot use a locked leader by editing local state.
- The purchase surface is universal: in-game store → platform checkout/RevenueCat → server grant. It is not tied to CrazyGames or Poki.
- Android gem bundles use Google Play Billing through the native RevenueCat bridge. Product IDs are `sow_gems_500`, `sow_gems_1200`, and `sow_gems_2600`; RevenueCat receives the public profile ID, never the private account ID.
- RevenueCat purchase events are granted by the server and deduplicated by event ID.

## Implemented in the first Android slice

- Three original SOW skins are catalogued, previewed, unlocked with gems, and equipped through authenticated server endpoints.
- The end-of-match screen shows the next unowned skin as a featured store offer and can open the store after returning to the menu.

## Remaining release work

- Connect the gem bundles to the configured Google Play/RevenueCat products.
- Supply the RevenueCat public Android key and webhook secret, then validate a sandbox purchase on-device.
- Balance leader prices after real retention and economy data exists.

The current leaders have gameplay perks. Selling access to them is intentional under the rotation model, but future hackathon/store copy must not claim that every purchase is purely cosmetic.

OpenFrontIO is a behavior reference only. Do not copy its proprietary/premium assets or code; use original Shadows of War assets and the small contract above.
