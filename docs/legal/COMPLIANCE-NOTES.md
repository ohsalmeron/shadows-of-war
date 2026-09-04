# Compliance notes (operator decisions, no budget spent)

## EU representative (GDPR Art. 27) — deferred, documented

We operate from outside the EU/UK with users in the EU/UK and have not
appointed an EU/UK representative: there is currently no budget for a
representation service. This is a known, accepted residual risk, mitigated by:

- First-party-only analytics, no advertising tech, no data brokers, no sale
  of personal data anywhere — there is minimal third-party exposure to defend.
- Published rights channel (`hello@shadowsofwar.io`) that handles GDPR
  requests directly, with the same SLA as EU requests.
- 90-day analytical retention enforced in code, erasure engine + runbook in
  `docs/legal/DATA-DELETION.md`.
- The Privacy Policy states the absence of a representative plainly instead
  of hiding it.

Revisit when EU revenue can fund it or when counsel advises otherwise. Do not
"fix" this by geo-blocking paying users without an explicit decision.

## Sub-processor DPAs — operator paperwork

Our published policy names IONOS, Microsoft Azure, Cloudflare, Google Play,
Apple, Stripe, and RevenueCat. The corresponding Data Processing Addenda are
signed in those vendors' consoles/dashboards, not in this repo. Nothing to
code; keep the vendor list in the Privacy Policy identical to reality.

## Store console tasks (manual, owner-only)

- Google Play → Data safety: declare collected data per the Privacy Policy;
  set the data-deletion path to `https://shadowsofwar.io/privacy/`.
- Google Play → Content rating / target age stays 13+ (matches the in-game
  age gate and the Terms).
- Apple (when iOS ships): same two entries in App Store Connect.

## What changed in-repo for compliance (no new vendors, no new cost)

- Fonts self-hosted (`sow-web/site/fonts/`): pages make zero cross-origin
  requests — removes the ePrivacy consent question for fonts entirely.
- In-game 13+ gate (`sow_age_ok`): the Terms' eligibility clause is now
  asked, not just written.
- Store fine print: immediate-delivery consent + all-sales-final notice at
  the point of purchase (EU digital-content withdrawal exception).
- Refund/void/chargeback webhooks revoke gems (floored at zero, deduplicated)
  so the no-refund economy is enforced, not just declared.
