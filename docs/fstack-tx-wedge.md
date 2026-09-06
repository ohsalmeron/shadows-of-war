# F-Stack TX wedge — the silent bug (August 2026)

Root cause and instrumentation for the relay TX wedge. Reference for future work in
`fstack-bridge/` and the relay.

## The bug in one sentence

In f-stack, the user-space TCP stack over DPDK, `callout_when` — the retransmission-timer
function — was a no-op stub in the deployed tree. TCP expects lost packets to be retransmitted;
without the timer, one lost packet froze the connection forever (`so_snd` full, writes stuck in
`EAGAIN`) with no log error.

## Timeline (supervisor local time, UTC-6)

| Date | Event |
|---|---|
| ~August 1 | Azure built and deployed `libfstack.a` containing the port's stub. |
| August 18 | First supervisor freeze report; incorrectly attributed to an old bundle. |
| August 20 | Freezes persisted while relay logs stayed healthy; the WS layer had no counters. |
| August 21 | Host evidence confirmed the root cause: `nm` showed `callout_when` as a stub in the deployed archive; disassembly confirmed it. The supervisor send-timer knob changed freeze duration, not existence. |
| August 21–22 | Implemented `callout_when` in the fork (`52fa8f9ae`), rebuilt `libfstack.a`, and rebuilt the relay. Live proof: `[BOOT] fstack=52fa8f9ae666`. |
| August 22–23 | Set the write timeout to 15000 ms, enabled TCP_NODELAY, and added bridge instrumentation (`e3fa033`). |
| August 26 | Verified TCP_NODELAY live (`ed/txd 1:1`). |

## Bridge instrumentation

A silent wedge is invisible without counters. `fstack-bridge/src/bridge.rs` provides:

- `Cmd::Send { fd, generation, buf, tx_pending }`: shared pending-byte counter readable by WS.
- `PendingSend`: unsent bytes, first stall time, and the counter update for the upper layer.
- `TX_STALL_TIMEOUT_SECS = 10`: a stall beyond this is a wedge candidate.
- `MAX_CONN_PENDING_BYTES = 128 * 1024`: per-connection pending-byte cap.
- Production knob: `SOW_WS_WRITE_TIMEOUT_MS=15000`, recorded in the relay manifest.

This converts “the game freezes and logs show nothing” into measurable pending bytes and stall
duration.

## Lessons

1. A stub must panic or fail loudly instead of returning zero. The silent `callout_when` stub lasted
   for months.
2. Add counters before an incident; after an incident there is nothing to inspect.
3. `strings(1)` on release binaries is not source forensics here: `log` literals do not survive,
   as verified against the control binary. Use `BUILD_EPOCH`, `[SERVER-BOOT]`, and release hashes.
4. A knob that changes symptom duration, not existence, helps locate the fault. Its sensitivity
   pointed to the TCP drain path before binary evidence was available.

## Fork status

- `callout_when` fix: `52fa8f9ae` in our fork, not upstream; the fork was about 439 commits ahead of
  `origin/master` at the time, most of which had since reached F-Stack `dev`.
- Azure runs our fork; `[BOOT] fstack=52fa8f9ae…` is the runtime proof.
- Remaining port stubs in `ff_stub_14_extra.c` should fail loudly; follow-up remains open.
