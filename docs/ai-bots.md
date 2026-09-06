# AI Bots — design, parameters, and the 24-hour saga (August 2026)

Reference for future bot work. Operational lessons first:

> **Three expensive lessons — each cost a failed deployment:**
> 1. Copying OpenFront rules without its ecosystem produces the opposite result. OpenFront's
>    2× bar works because its bots stay small; Shadows of War tribes grow without a cap.
> 2. Any troop-ratio gate structurally favors tribes: tribes stay near `max_troops` because they
>    rarely spend, while nations sit at 30–60% after sweeps drain troops and conquests raise the cap.
> 3. Run the Lab before deployment and verify the live code path. World maps use
>    `random_spawn=false`, so spawning runs through the Spawning phase in `tick.rs`, not `spawn_human`.

## Causal chain (August 26–28, 2026)

Each fix exposed the next failure:

1. **`iq_points` bankruptcy** caused the original “never advance” behavior. War cost 5–10 points
   per action while a ghost earned about 1.7 points per second; bankruptcy froze every action,
   including growth. Neutral expansion is now free, war deductions clamp at `.max(0.0)`, and the
   bankruptcy lock is gone (`combat.rs`).
2. **Impossible attack trigger.** `troops ≥ max_troops × trigger_ratio` became unreachable as
   territory increased the cap faster than troops. A committed odds decision now triggers the attack
   path (`odds_committed || trigger gate`).
3. **Player odds discipline.** In FFA, do not start below 20% of the target's troops or attack a
   stronger player. If blocked with a neutral border, expand; if enclosed, bank. Defense,
   retaliation, and team games remain exempt; OpenFront's FFA-only rules must not leak into other modes.
4. **Attrition versus tribes.** OpenFront parity at 2× was mathematically unsatisfiable; 1× still
   stalled the Lab. The final rule has no floor against tribes: attack the frontier tribe with
   `min(4× tribe_troops, affordable)`.
5. **Anti-stall swing.** If the weakest target is blocked by odds and a tribe borders the bot,
   attack the tribe instead of banking. Lab W1 caught the zero-attack failure.
6. **Mobility.** D1: if no neutral tile borders the bot, launch a free boat to a random neutral shore
   (`try_expansion_boat`, all tiers). D2: an enclosed non-tribe can launch a fleet.
7. **Cascade.** Vanilla tribes cannot capture player tiles during enclosure cascades
   (`set_tile_owner`, guarded by `capturer_is_passive_tribe`).
8. **Zone spawning.** `Team::Red` uses the left half of the map and `Team::Blue` the right half.
   All team members, including the first player and late human joins, spawn inside their team zone.
   The cross-team home-distance floor is 14 tiles. Fallback is ring 12..36, then random.
   World maps take this path through the Spawning phase in `tick.rs`; `spawn_human` only records
   the player and returns when `random_spawn=false`.

## Current parameters

Source of truth is code. Update this table when behavior changes.

| Parameter | Ghost | Nation | Tribe (Vanilla) |
|---|---:|---:|---:|
| IQ band | 160–181 | 130–160 | 50–86 |
| Base cadence (ticks) | 5 | 30 | 100 |
| `trigger_ratio` | 0.05 | 0.45 | ignored (`attacks_players=false`) |
| `reserve_ratio` | 0.02 | 0.20 | 0.50 |
| `expand_ratio` | 0.02 | 0.15 | 0.10 |
| IQ costs (war/build/alliance/send) | 5/5/5/5 | 5/5/5/999 | 10/10/10/999 |
| Real counts | ~83–118 | 128 | 420 |

- Neutral expansion is free; war costs `attack_cost`, clamped at zero.
- `max_troops = 10 + tiles^0.625 × 350 + 5000×city_levels`; tribes divide it by 1.5.
- Troop income is `250 + 25×cities + tiles/16` per second; tribes receive 0.75× that value.
- Ghost fill is 65–92% of `max_players` (`SOW_BOT_FILL_MIN/MAX`).
- Faces: `spawn_ai(nation_count=128, bot_count=420)` plus scripted map spawns.
- Tribes never initiate against non-tribes; tribe-versus-tribe combat is allowed at `troops/4`.

## Behavior regression Lab

Location: `sow-core/src/intent/nation/bot_lab.rs`, scenarios S1–S18 under `#[cfg(test)]`.

S1 ghost expands · S2 ghost pressures · S3 passive-growing Vanilla tribe · S4 nation defends ·
S5 water harness (ignored) · S7 someone wins · S9 cluster · S10 real-world midgame ·
S11 partitioned world still fights · S12 island boat · S13 team zones · S14 decisive tribe hunt ·
S15 FFA no-suicide · S16 long ecosystem freeze signature · S17 Spawning-phase zones ·
S18 real-world geography plus zones.

**Rule:** Do not deploy an AI change without a green Lab. The Lab uses `WORLD_MAP_BYTES`, so it
tests real geography rather than flat boxes.

## Process lessons

- A fix without a commit can disappear during a sibling refactor.
- Verify the path that runs in production before shipping: grep `random_spawn` and every caller of
  shared helpers.
- A green Lab that disagrees with production means the simulation conditions are wrong, or the
  wrong entity tier was created. `build_lab` once mislabeled nations as Bot-type; `ai_tier` resolves
  from `(player_type, is_ai_controlled)`.
