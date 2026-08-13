//! Fictional humans — internal bot fill for Matchmaking lobbies.
//!
//! ══════════════════════════════════════════════════════════════════════
//! MENTAL MODEL — read this before touching ANYTHING in this module.
//! ══════════════════════════════════════════════════════════════════════
//! These are FICTIONAL HUMANS. Not PvE bots, not a daemon, not "the
//! backfill". They are human players that don't exist, and they must look
//! like it.
//!
//! THE ONE RULE: organic. Nothing here may ever be predictable, regular,
//! or metronomic. A lobby must never fill at a fixed rate. Two lobbies
//! must never fill alike. Nobody — player or agent — may ever be able to
//! guess how many ghosts there are, or when the next one joins.
//!
//! What "organic" looks like in practice:
//!   - each ghost INDEPENDENTLY picks a random moment somewhere across the
//!     whole countdown window (second 0 to the last second). No mean, no
//!     drip, no "next after this one"
//!   - bursts (several ghosts landing on the same moment) are normal —
//!     that's what independent placement produces
//!   - silences (nobody for several seconds) are normal — same reason
//!   - a lobby can fill in the first 2s, in the last 2s, spread out, or
//!     never get past half. Nobody can guess the rhythm because there IS none
//!   - the fill percentage is random per lobby (min/max bounds only)
//!   - there is NO perceptible rate. If a human watching can tap a tempo,
//!     it's broken
//!
//! REGRESSION WARNINGS (for future agents):
//!   - reintroducing a per-lobby mean, a drip cooldown, an inter-arrival
//!     distribution with a central tendency, or ANY sequential "next entry
//!     depends on the previous" structure = REGRESSION. That recreates the
//!     perceptible heartbeat this module exists to kill
//!   - adding a fixed interval, constant pace, linear ramp, deterministic
//!     schedule, or "smoother" distribution = REGRESSION, not a fix
//!   - "fixing" randomness that looks random = REGRESSION
//!   - "helping" by making entry consistent = REGRESSION
//!   - replacing randomness with anything an observer could predict =
//!     REGRESSION.
//!
//! Identities come exclusively from the persistent bot-account pool seeded
//! at boot (`BotPool`) — real `database_account_id`s, stats accumulation,
//! display names reused from `names::BOT_NAMES`. If the pool is
//! unavailable, nothing is injected (no anonymous fallback names).
//! ══════════════════════════════════════════════════════════════════════

pub mod names;

use crate::lobby::{LobbyPhase, LOBBY_COUNTDOWN_SECS, PlayerConnection, ServerLobby};
use rand::Rng;
use std::env;
use std::sync::OnceLock;
use sow_core::player::Leader;
use sow_core::protocol::{LobbyKind, Team};
use tokio::sync::mpsc;

// ── Persistent bot-account pool ───────────────────────────────────────────

/// One persistent bot identity: a stable account id (resolves to a
/// `PlayerAccount` with `kind = Bot` in sow-data) paired with the display
/// name shown in lobbies and in-game.
///
/// MENTAL MODEL: the pool is just identities. It says NOTHING about when
/// or how many of them join — entry timing lives in the drip below and is
/// deliberately chaotic.
#[derive(Clone, Debug)]
pub struct BotPoolEntry {
    pub account_id: String,
    pub display_name: String,
}

/// The resolved bot pool. Installed once at boot via `BotPool::install`; read
/// from every subsequent `inject_internal_bots` call through the `OnceLock`.
pub struct BotPool {
    entries: Vec<BotPoolEntry>,
}

static BOT_POOL: OnceLock<BotPool> = OnceLock::new();

impl BotPool {
    pub fn new(entries: Vec<BotPoolEntry>) -> Self {
        Self { entries }
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Publish this pool as the process-wide bot identity source. Called once
    /// at boot from `main`; subsequent calls are silently ignored (the first
    /// one wins), which keeps boot idempotent if init is retried.
    pub fn install(self) {
        let n = self.entries.len();
        if BOT_POOL.set(self).is_err() {
            log::warn!("[BOT_POOL] install ignored — pool already installed");
        } else {
            log::info!("[BOT_POOL] installed with {} identities", n);
        }
    }

    /// Process-wide accessor. Returns `None` until `install` has run.
    pub fn get() -> Option<&'static BotPool> {
        BOT_POOL.get()
    }

    /// Pull up to `count` identities with **unique display names** within the
    /// returned batch (so a single lobby never shows two bots with the same
    /// name). Uses a partial Fisher–Yates shuffle over index space — O(count)
    /// swaps, no full sort.
    ///
    /// NOTE: this shuffle is only about NAME UNIQUENESS inside one lobby. It
    /// is NOT an entry order — the drip below consumes the batch in whatever
    /// order it lands, and the TIMING is what the observer sees. Do not
    /// "sort" or "pace" this batch. (MENTAL MODEL: organic.)
    pub fn take(&self, count: usize, rng: &mut impl Rng) -> Vec<BotPoolEntry> {
        if self.entries.is_empty() || count == 0 {
            return Vec::new();
        }
        let mut idx: Vec<usize> = (0..self.entries.len()).collect();
        let swaps = count.min(idx.len());
        for i in 0..swaps {
            let j = rng.gen_range(i..idx.len());
            idx.swap(i, j);
        }
        let mut seen_names: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut out: Vec<BotPoolEntry> = Vec::with_capacity(count);
        for &i in &idx {
            let e = &self.entries[i];
            // Dedup by display name within this draw. Different account_ids
            // may share a display name (the legacy pool cycles 1000 names
            // across 10k accounts); within one lobby that would be visually
            // ambiguous, so we skip duplicates.
            if !seen_names.insert(e.display_name.as_str()) {
                continue;
            }
            out.push(e.clone());
            if out.len() >= count {
                break;
            }
        }
        out
    }
}

// ── Independent-moment placement ──────────────────────────────────────────

/// Per-tick injection of fictional humans. Called from `promote_countdown`
/// every tick (10 Hz).
///
/// MENTAL MODEL (organic — read the module header before changing this):
///  1. First sighting of a lobby: roll a random fill percentage
///     (`SOW_BOT_FILL_MIN`..`SOW_BOT_FILL_MAX` of max_players). Every lobby
///     gets its own count — no two lobbies ever fill alike.
///  2. Each reserved ghost INDEPENDENTLY picks a random moment
///     (`join_at_elapsed`, uniform over the whole countdown window) to show
///     up. There is NO mean, NO drip clock, NO "next entry depends on the
///     previous one". Timing is a set of independent points on a line, not a
///     sequence. That is what kills any perceptible rate.
///  3. Leave room for humans: target is a TOTAL (humans + bots). Humans
///     present → fewer bots needed → trim the pending queue. Already-met
///     ghosts stay; the lobby keeps the ghosts that were going to show up
///     soonest.
///  4. Each tick: every pending ghost whose `join_at_elapsed` has been
///     reached enters NOW. Several can land on the same tick (a burst); none
///     can land for many ticks (a silence). Both are correct and intended —
///     that is literally independent placement.
///  5. When the reserved batch runs out, the lobby stops filling. A lobby can
///     legitimately sit half-full forever. That is correct.
pub fn inject_internal_bots(games: &mut [ServerLobby]) {
    let min_pct = env::var("SOW_BOT_FILL_MIN")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .map(|v| v.clamp(0.0, 1.0))
        .unwrap_or(0.65);
    let max_pct = env::var("SOW_BOT_FILL_MAX")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .map(|v| v.clamp(min_pct, 1.0))
        .unwrap_or(0.92);

    for g in games.iter_mut() {
        if g.kind != LobbyKind::Matchmaking {
            continue;
        }
        if g.phase != LobbyPhase::CountingDown {
            continue;
        }

        let max = g.config.max_players as usize;
        let humans = g.players.iter().filter(|p| !p.is_internal_bot).count();
        let bots = g.players.iter().filter(|p| p.is_internal_bot).count();

        // ── First sighting: this lobby gets its OWN count, and every ghost
        //    independently picks when (across the whole countdown) it shows. ─
        if g.bot_fill_target.is_none() {
            let mut rng = rand::thread_rng();
            let pct = rng.gen_range(min_pct..=max_pct);
            let target = ((max as f32) * pct).round() as usize;
            let identities: Vec<BotPoolEntry> = match BotPool::get() {
                Some(pool) if !pool.is_empty() => pool.take(target, &mut rng),
                _ => {
                    log::warn!(
                        "[BOT_FILL] Lobby {}: bot pool unavailable — skipping fill",
                        g.id
                    );
                    Vec::new()
                }
            };
            // Each ghost picks its own moment, uniform over [0, countdown].
            // Independent draws → clusters and silences emerge naturally; no
            // mean exists for a watcher to tap a tempo to.
            let mut pending: Vec<(String, String, f32)> = identities
                .into_iter()
                .map(|e| {
                    let t = rng.gen_range(0.0..=LOBBY_COUNTDOWN_SECS);
                    (e.account_id, e.display_name, t)
                })
                .collect();
            pending.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));
            log::info!(
                "[BOT_FILL] Lobby {}: target {} of {} ({:.0}%), {} ghosts each on their own clock",
                g.id, target, max, pct * 100.0, pending.len()
            );
            g.bot_fill_target = Some(target);
            g.pending_bots = pending;
        }

        // ── Leave room for humans: bots yield to real players. ────────────
        // Target is a TOTAL (humans + bots). Humans present → fewer bots
        // needed. Trim the tail of the pending queue (latest-arriving ghosts
        // first) so a human always has room to join.
        let target = g.bot_fill_target.unwrap_or(0);
        let bots_desired = target.saturating_sub(humans);
        let bots_needed_now = bots_desired.saturating_sub(bots);
        if g.pending_bots.len() > bots_needed_now {
            g.pending_bots.truncate(bots_needed_now);
        }

        // ── Who's due? Every ghost whose independent moment has arrived. ──
        // Sorted ascending by join_at_elapsed, so partition_point + drain
        // pops exactly the due ones in O(log n) + O(due). Nothing is drawn
        // here — the moments were all decided at lobby birth.
        let elapsed = (LOBBY_COUNTDOWN_SECS - g.countdown_secs).max(0.0);
        let split = g
            .pending_bots
            .partition_point(|(_, _, t)| *t <= elapsed);
        if split == 0 {
            continue;
        }
        let due: Vec<(String, String, f32)> = g.pending_bots.drain(..split).collect();
        let _ = bots; // already counted above; silence unused-after-move

        let mut rng = rand::thread_rng();
        let mut pushed = false;
        for (account_id, display_name, _t) in due {
            let leader = Leader::ALL[rng.gen_range(0..Leader::ALL.len())];
            let civilization = leader.civilization();
            let player_id = g
                .players
                .iter()
                .map(|p| p.player_id)
                .max()
                .unwrap_or(0)
                .saturating_add(1);

            let (dummy_tx, _dummy_rx) = mpsc::channel::<Vec<u8>>(8);

            // Teams: drop the ghost into whichever team is smaller (Red on a tie),
            // mirroring join_player. HumansVsNations: ghosts are human-side → Red.
            let team = if g.game_mode == "HumansVsNations" {
                Some(Team::Red)
            } else if g.game_mode == "Teams" {
                let reds = g
                    .players
                    .iter()
                    .filter(|p| p.team == Some(Team::Red))
                    .count();
                let blues = g
                    .players
                    .iter()
                    .filter(|p| p.team == Some(Team::Blue))
                    .count();
                Some(if blues < reds { Team::Blue } else { Team::Red })
            } else {
                None
            };

            let database_account_id = if account_id.is_empty() {
                None
            } else {
                Some(account_id)
            };

            g.players.push(PlayerConnection {
                name: display_name,
                clan_tag: String::new(),
                player_id,
                tx: dummy_tx,
                download_progress: 100,
                civilization,
                leader,
                database_account_id,
                team,
                ip: "127.0.0.1".to_string(),
                session_id: None,
                is_internal_bot: true,
            });
            g.ready_players.insert(player_id);
            log::debug!(
                "[BOT_FILL] Lobby {}: ghost {player_id} ({}) joined at {:.1}s ({}/{} target)",
                g.id,
                g.players.last().map(|p| p.name.as_str()).unwrap_or("?"),
                elapsed,
                g.players.len(),
                g.bot_fill_target.unwrap_or(0)
            );
            pushed = true;
        }

        // One sync per tick, not per ghost: a burst (several due on the same
        // tick) lands as a single roster update so the UI shows them arriving
        // together — exactly the "varios de golpe" that is correct here.
        if pushed {
            crate::lobby::sync_host_lobby_to_members(g);
        }
    }
}
