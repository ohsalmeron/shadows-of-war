# Data deletion runbook (operator-only)

Player-facing promise: `sow-web/site/privacy/` ("Your rights and deletion").
This is the internal procedure that fulfills it. Never expose the endpoint
below publicly — it is bearer-gated and loopback-bound by design.

## 1. Receive and verify

1. Request arrives at `hello@shadowsofwar.io` with an `account_id`
   (32 hex chars, found in-game) or a display name + approximate creation date.
2. Look up the account before touching anything:
   ```sh
   ssh ionos 'valkey-cli -h 127.0.0.1 GET "sow:player:account:<account_id>"'
   ```
   Save the `public_id` from the JSON — you need it for step 3 verification.
   If the requester gave only a display name, resolve it via
   `GET /profiles/search` on the database host first and confirm ownership
   (creation date, linked platform, recent matches) before proceeding.

## 2. Erase

```sh
ssh ionos 'curl -s -X POST http://127.0.0.1:25585/internal/profile/delete \
  -H "Authorization: Bearer $SOW_DB_SECRET" \
  -H "Content-Type: application/json" \
  -d "{\"account_id\": \"<account_id>\"}"'
```

Expected response:

```json
{"account_id":"…","found":true,"keys_removed":2,"redb_rows_removed":2,"analytics_sets_scrubbed":14}
```

What it does (`PlayerDb::delete_account` in `sow-data/src/db.rs`):

- `DEL sow:player:account:{id}` plus every
  `sow:player:identity:{provider}:{external_id}` mapping from the account's
  linked identities.
- Removes the `PLAYERS_TABLE` row and the `PUBLIC_PROFILES_TABLE` index row
  from the redb mirror.
- `SREM`s the id from every dated analytics set
  (`sow:analytics:event_users:*`, `activated`, `cohort`, `active`, `sow:active:*`).

Deliberately retained (and disclosed in the Privacy Policy):

- Aggregate match-history rows (competitive record, no longer linkable).
- JSONL event lines under `/var/db/sow/analytics/events-*.jsonl` — pseudonymous
  `session_id` + `account_id` pairs that age out via the automatic 90-day file
  rotation. Scope them with `grep -l "<account_id>" events-*.jsonl`, but do not
  hand-edit files; rotation deletes them within 90 days of the request.
- The `sow:analytics:unique_users` HyperLogLog (probabilistic, no per-member
  removal; bounded by its own 90-day key TTL).

## 3. Verify and reply

```sh
ssh ionos 'valkey-cli -h 127.0.0.1 GET "sow:player:account:<account_id>"'
# expect: (nil)
ssh ionos 'curl -s -o /dev/null -w "%{http_code}\n" http://127.0.0.1:25585/profiles/<public_id>'
# expect: 404
```

Reply to the requester with what was erased and the JSONL 90-day note.
Respond within 45 days of the request at most; denials must state the reason
and offer appeal by reply.

## 4. Children under 13

Same procedure, expedited: suspend first (Terms violation — underage account),
erase second, reply to the parent/guardian. Do not ask the child for new
identity documents; verify via the parent's email context instead.
