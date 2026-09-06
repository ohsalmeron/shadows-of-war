//! Client diagnostics switch + sim-tick cost recorder.
//!
//! Verbose `[DIAG NET]`/`[DIAG SIM TICK]` logging is OFF by default in
//! production; enable with `?diag=1` in the URL (WASM) or `SOW_DIAG=1`
//! (native). Tick durations are always measured (two `Instant::now()` calls
//! per tick) but only reported on anomaly — a slow frame is the actual
//! freeze signal, so it logs even without diag enabled.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Ring buffer of the last N sim-tick durations in microseconds.
const CAP: usize = 128;
/// Anomaly threshold: one lockstep tick should cost far below its 100 ms slot.
const TICK_WARN_US: u64 = 80_000;
/// Seconds between perf summaries while anomalies keep occurring.
const SUMMARY_SECS: u64 = 30;

static RING: [AtomicUsize; CAP] = [const { AtomicUsize::new(0) }; CAP];
static IDX: AtomicUsize = AtomicUsize::new(0);
static ANOMALIES: AtomicUsize = AtomicUsize::new(0);
static LAST_SUMMARY_MS: AtomicUsize = AtomicUsize::new(0);

pub fn init_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

#[cfg(target_arch = "wasm32")]
pub fn init_from_url() {
    let on = web_sys::window()
        .and_then(|w| w.location().search().ok())
        .map(|s| s.contains("diag=1"))
        .unwrap_or(false);
    init_enabled(on);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn init_from_url() {
    init_enabled(std::env::var("SOW_DIAG").is_ok_and(|v| v == "1"));
}

pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Record one sim-tick duration. Returns `Some(dt_us)` when the tick was an
/// anomaly (slow enough to threaten the frame budget).
pub fn record_tick(duration_us: u64) -> Option<u64> {
    let i = IDX.fetch_add(1, Ordering::Relaxed) % CAP;
    RING[i].store(duration_us as usize, Ordering::Relaxed);
    if duration_us > TICK_WARN_US {
        ANOMALIES.fetch_add(1, Ordering::Relaxed);
        Some(duration_us)
    } else {
        None
    }
}

fn snapshot_sorted() -> Vec<u64> {
    let mut v: Vec<u64> = RING
        .iter()
        .map(|a| a.load(Ordering::Relaxed) as u64)
        .filter(|&us| us > 0)
        .collect();
    v.sort_unstable();
    v
}

/// `(min_us, p95_us, max_us)` over the recent window.
pub fn percentiles_recent() -> (u64, u64, u64) {
    let v = snapshot_sorted();
    if v.is_empty() {
        return (0, 0, 0);
    }
    let p = |q: f64| v[((v.len() as f64 * q).ceil() as usize).saturating_sub(1)];
    (*v.first().unwrap_or(&0), p(0.95), *v.last().unwrap_or(&0))
}

/// Emits the periodic summary line when anomalies occurred and `SUMMARY_SECS`
/// elapsed since the last one. `now_ms`: monotonic wall clock in milliseconds.
pub fn maybe_summary(now_ms: u64) {
    if ANOMALIES.load(Ordering::Relaxed) == 0 {
        return;
    }
    let last = LAST_SUMMARY_MS.load(Ordering::Relaxed) as u64;
    if last != 0 && now_ms.saturating_sub(last) < SUMMARY_SECS * 1000 {
        return;
    }
    if LAST_SUMMARY_MS
        .compare_exchange(
            last as usize,
            now_ms as usize,
            Ordering::Relaxed,
            Ordering::Relaxed,
        )
        .is_err()
    {
        return;
    }
    let (p50, p95, max) = percentiles_recent();
    log::warn!(
        "[SIM PERF] last {}s: anomalies={} tick_ms p50={} p95={} max={} (budget 100ms)",
        SUMMARY_SECS,
        ANOMALIES.load(Ordering::Relaxed),
        p50 / 1000,
        p95 / 1000,
        max / 1000,
    );
}
