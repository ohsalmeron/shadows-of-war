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
//!   - every entry is an INDEPENDENT random event with WIDE variance
//!   - bursts of 2-3 ghosts back-to-back are normal
//!   - silences of 30-60+ seconds are normal
//!   - a lobby filling in ~15s is normal; a lobby half-full for minutes
//!     is normal; a lobby stuck at 70% forever is normal
//!   - the fill percentage is random per lobby (min/max bounds only)
//!   - the drip mean is random per lobby (wide bounds only)
//!   - the inter-arrival time follows an exponential distribution: short
//!     gaps are common, long gaps happen, a "beat" NEVER exists
//!
//! REGRESSION WARNINGS (for future agents):
//!   - adding a fixed interval, constant pace, linear ramp, deterministic
//!     schedule, or "smoother" distribution = REGRESSION, not a fix
//!   - "fixing" randomness that looks random = REGRESSION
//!   - "helping" by making entry consistent = REGRESSION
//!   - replacing randomness with anything an observer could predict =
//!     REGRESSION. The legacy daemon this replaces was pure chaos:
//!     random fill %, random entry times, never the same profile twice.
//!     That chaos is the spec.
//!
//! Identities come exclusively from the persistent bot-account pool seeded
//! at boot (`BotPool`) — real `database_account_id`s, stats accumulation,
//! display names reused from `names::BOT_NAMES`. If the pool is
//! unavailable, nothing is injected (no anonymous fallback names).
//! ══════════════════════════════════════════════════════════════════════

pub mod names;

use crate::lobby::{LobbyPhase, PlayerConnection, ServerLobby, TICK_SECS};
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

// ── Organic staged drip ───────────────────────────────────────────────────

/// Per-tick staged injection of fictional humans. Called from
/// `promote_countdown` every tick (10 Hz).
///
/// MENTAL MODEL (organic — read the module header before changing this):
///  1. First sighting of a lobby: roll a random fill percentage
///     (`SOW_BOT_FILL_MIN`..`SOW_BOT_FILL_MAX` of max_players) AND a random
///     drip mean for THIS lobby (1–8s). Every lobby gets its own numbers —
///     no two lobbies ever fill alike.
///  2. Identities are reserved up-front ONLY so display names stay unique
///     within the lobby. This says nothing about timing.
///  3. Every drip is an independent event: draw the next cooldown from an
///     EXPONENTIAL distribution around the lobby's mean. Exponential means
///     bursts (short gaps are the most likely outcome) and occasional long
///     silences (the tail) — the exact profile nobody can predict. With real
///     humans present the mean shrinks (activity attracts activity) but the
///     chaos stays: still random per event.
///  4. When the reserved batch runs out, the lobby stops filling. A lobby
///     can legitimately sit half-full forever. That is correct.
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

        // ── First sighting: this lobby gets its OWN random profile. ────────
        // Random fill % (TOTAL target) AND a random drip mean (1–8s).
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
            log::info!(
                "[BOT_FILL] Lobby {}: target {} of {} ({:.0}%), {} identities reserved",
                g.id, target, max, pct * 100.0, identities.len()
            );
            g.bot_fill_target = Some(target);
            g.bot_fill_mean = rng.gen_range(1.0..8.0);
            g.bot_fill_cooldown = drip_cooldown(g.bot_fill_mean, &mut rng);
            g.pending_bots = identities
                .into_iter()
                .map(|e| (e.account_id, e.display_name))
                .collect();
        }

        // ── Leave room for humans: bots yield to real players. ────────────
        // Target is a TOTAL (humans + bots). Humans present → fewer bots
        // needed. Trim the pending queue so a human always has room to join.
        let target = g.bot_fill_target.unwrap_or(0);
        let bots_desired = target.saturating_sub(humans);
        let bots_needed_now = bots_desired.saturating_sub(bots);
        if g.pending_bots.len() > bots_needed_now {
            g.pending_bots.truncate(bots_needed_now);
        }

        // ── Drip clock: one independent event at a time, always random. ───
        if g.bot_fill_cooldown > 0.0 {
            g.bot_fill_cooldown -= TICK_SECS;
            continue;
        }

        let Some((account_id, display_name)) = g.pending_bots.pop() else {
            continue;
        };
        // Re-draw the NEXT cooldown from the exponential every single event.
        // Two lobbies never drip alike; one lobby never drips in a pattern.
        let mut rng = rand::thread_rng();
        g.bot_fill_cooldown = drip_cooldown(g.bot_fill_mean, &mut rng);

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
        // mirroring join_player.
        let team = if g.game_mode == "Teams" {
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
            is_internal_bot: true,
        });
        g.ready_players.insert(player_id);
        crate::lobby::sync_host_lobby_to_members(g);
        log::debug!(
            "[BOT_FILL] Lobby {}: ghost {player_id} ({}) joined ({}/{} target)",
            g.id,
            g.players.last().map(|p| p.name.as_str()).unwrap_or("?"),
            g.players.len(),
            g.bot_fill_target.unwrap_or(0)
        );
    }
}

/// Organic inter-arrival time between ghost drips.
///
/// Exponential sample around the lobby's random mean (`bot_fill_mean`, rolled
/// per lobby at 1–8s). Always random, never conditioned on anything. Clamped
/// wide (0.15–45s) so a single draw can never stall a lobby forever nor empty
/// the batch in one tick.
///
/// MENTAL MODEL: the exponential is the POINT. Short gaps are common (bursts),
/// long gaps happen (silences), and no observer can predict the next one. A
/// uniform range (like the old 0.1–1.5s) is a metronome in disguise — do NOT
/// "simplify" back to it.
fn drip_cooldown(mean: f32, rng: &mut impl Rng) -> f32 {
    // Inverse-CDF of Exp(mean): -ln(1 - u) * mean, u ~ U(0,1).
    let u = rng.gen_range(f32::EPSILON..1.0);
    (-(1.0 - u).ln() * mean.max(0.1)).clamp(0.15, 45.0)
}
