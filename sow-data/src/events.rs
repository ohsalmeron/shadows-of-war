//! Product analytics event schema, closed taxonomy, and durable JSONL sink.
//!
//! Events arrive as a public batch POST, are validated against the closed
//! taxonomy, filtered for bot accounts by the caller, and appended to one
//! daily `events-YYYY-MM-DD.jsonl` file. Files older than [`RETENTION_DAYS`]
//! are pruned on day rotation. No external storage vendor is involved.

use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::PathBuf;

pub const SCHEMA_VERSION: u8 = 1;
pub const RETENTION_DAYS: u64 = 90;
pub const MAX_PROPS_BYTES: usize = 2048;
const MAX_STRING_FIELD: usize = 64;

/// Closed taxonomy. Unknown names are rejected at ingest — adding an event
/// means adding it here first.
pub const EVENT_NAMES: &[&str] = &[
    "landing_visit",
    "shell_loaded",
    "play_now_click",
    "boot_start",
    "boot_route_decision",
    "load_stage",
    "matchmaking_joined",
    "match_started_client",
    "match_started",
    "match_ended_client",
    "match_ended",
    "gameplay_start",
    "gameplay_stop",
    "tutorial_start",
    "tutorial_step",
    "tutorial_objective_complete",
    "tutorial_dialog_choice",
    "tutorial_exit_early",
];

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AnalyticsEvent {
    #[serde(default)]
    pub v: u8,
    pub name: String,
    pub ts_ms: u64,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub portal: Option<String>,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub build: Option<String>,
    #[serde(default)]
    pub locale: Option<String>,
    #[serde(default)]
    pub props: Option<serde_json::Value>,
}

impl AnalyticsEvent {
    /// Pure validation; `now_ms` injected so tests stay deterministic.
    pub fn validate(&self, now_ms: u64) -> Result<(), &'static str> {
        if self.v != SCHEMA_VERSION {
            return Err("unsupported schema version");
        }
        if !EVENT_NAMES.contains(&self.name.as_str()) {
            return Err("unknown event name");
        }
        let minute_ms: u64 = 60_000;
        if self.ts_ms < 1_600_000_000_000 || self.ts_ms > now_ms.saturating_add(5 * minute_ms) {
            return Err("timestamp out of range");
        }
        if self.session_id.len() > MAX_STRING_FIELD || !is_simple_string(&self.session_id) {
            return Err("invalid session_id");
        }
        if let Some(id) = &self.account_id
            && (id.is_empty() || id.len() > MAX_STRING_FIELD || !is_simple_string(id))
        {
            return Err("invalid account_id");
        }
        for field in [&self.portal, &self.platform, &self.build, &self.locale] {
            if let Some(value) = field
                && (value.is_empty() || value.len() > 32 || !is_simple_string(value))
            {
                return Err("invalid string field");
            }
        }
        if let Some(props) = &self.props {
            if !props.is_object() {
                return Err("props must be an object");
            }
            if serde_json::to_vec(props).map_or(true, |bytes| bytes.len() > MAX_PROPS_BYTES) {
                return Err("props too large");
            }
        }
        Ok(())
    }
}

fn is_simple_string(value: &str) -> bool {
    !value.chars().any(char::is_control)
}

pub fn utc_date_string() -> String {
    let dt = crate::time_util::now_utc();
    format!("{:04}-{:02}-{:02}", dt.year, dt.month, dt.day)
}

/// Shift an ISO UTC date by a number of calendar days.
pub fn shift_date(date: &str, offset_days: i64) -> Option<String> {
    let mut parts = date.split('-');
    let year = parts.next()?.parse::<i64>().ok()?;
    let month = parts.next()?.parse::<u32>().ok()?;
    let day = parts.next()?.parse::<u32>().ok()?;
    if parts.next().is_some() || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let days = days_from_civil(year, month, day).checked_add(offset_days)?;
    let (year, month, day) = civil_from_days(days);
    Some(format!("{year:04}-{month:02}-{day:02}"))
}

/// Appends validated event lines to one file per UTC day. Rotation happens on
/// write when the UTC date changes; pruning removes files past retention.
pub struct EventSink {
    dir: PathBuf,
    day: String,
    file: Option<std::fs::File>,
}

impl EventSink {
    pub fn new(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            dir,
            day: utc_date_string(),
            file: None,
        })
    }

    pub fn append_line(&mut self, line: &str) -> std::io::Result<()> {
        let today = utc_date_string();
        if today != self.day || self.file.is_none() {
            self.rotate(today)?;
        }
        self.file
            .as_mut()
            .expect("rotated file must be open")
            .write_all(line.as_bytes())
            .and_then(|_| self.file.as_mut().unwrap().write_all(b"\n"))
            .and_then(|_| self.file.as_mut().unwrap().flush())
            .and_then(|_| self.file.as_mut().unwrap().sync_data())
    }

    fn rotate(&mut self, today: String) -> std::io::Result<()> {
        self.file = None;
        self.day = today.clone();
        let path = self.dir.join(format!("events-{today}.jsonl"));
        self.file = Some(std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?);
        self.prune_old();
        Ok(())
    }

    fn prune_old(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let keep_from = Self::retention_floor_date();
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            let Some(date) = name
                .strip_prefix("events-")
                .and_then(|rest| rest.strip_suffix(".jsonl"))
            else {
                continue;
            };
            if date < keep_from.as_str() {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }

    fn retention_floor_date() -> String {
        // Walk back RETENTION_DAYS from the current UTC date using the same
        // pure calendar math as time_util (convert via day arithmetic).
        let dt = crate::time_util::now_utc();
        let days = days_from_civil(dt.year, dt.month, dt.day);
        let (year, month, day) = civil_from_days(days - RETENTION_DAYS as i64);
        format!("{year:04}-{month:02}-{day:02}")
    }
}

fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = year - if month <= 2 { 1 } else { 0 };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month as i64 + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let days = days + 719_468;
    let era = days.div_euclid(146_097);
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (y + if m <= 2 { 1 } else { 0 }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW_MS: u64 = 1_800_000_000_000;

    fn sample(name: &str) -> AnalyticsEvent {
        serde_json::from_value(serde_json::json!({
            "v": 1, "name": name, "ts_ms": NOW_MS,
            "session_id": "abc123", "portal": "crazygames"
        }))
        .unwrap()
    }

    #[test]
    fn accepts_known_event() {
        assert!(sample("boot_start").validate(NOW_MS).is_ok());
    }

    #[test]
    fn rejects_unknown_name_and_bad_version_and_bad_ts() {
        assert_eq!(sample("not_an_event").validate(NOW_MS), Err("unknown event name"));
        let mut bad = sample("boot_start");
        bad.v = 9;
        assert_eq!(bad.validate(NOW_MS), Err("unsupported schema version"));
        let mut old = sample("boot_start");
        old.ts_ms = 999_999_999_999;
        assert_eq!(old.validate(NOW_MS), Err("timestamp out of range"));
    }

    #[test]
    fn rejects_non_object_and_oversized_props() {
        let mut ev = sample("boot_start");
        ev.props = Some(serde_json::json!(["nope"]));
        assert_eq!(ev.validate(NOW_MS), Err("props must be an object"));
        let mut big = sample("boot_start");
        big.props = Some(serde_json::json!({"blob": "x".repeat(MAX_PROPS_BYTES + 1)}));
        assert_eq!(big.validate(NOW_MS), Err("props too large"));
    }

    #[test]
    fn sink_writes_lines_to_daily_file() {
        let dir = tempfile::tempdir().unwrap();
        let mut sink = EventSink::new(dir.path()).unwrap();
        sink.append_line("{\"a\":1}").unwrap();
        sink.append_line("{\"a\":2}").unwrap();
        let today = utc_date_string();
        let contents =
            std::fs::read_to_string(dir.path().join(format!("events-{today}.jsonl"))).unwrap();
        assert_eq!(contents.lines().count(), 2);
    }

    #[test]
    fn sink_prunes_files_past_retention() {
        let dir = tempfile::tempdir().unwrap();
        let floor = EventSink::retention_floor_date();
        let ancient = format!("events-2000-01-01.jsonl");
        std::fs::write(dir.path().join(&ancient), "{}\n").unwrap();
        std::fs::write(dir.path().join(format!("events-{floor}.jsonl")), "{}\n").unwrap();
        let mut sink = EventSink::new(dir.path()).unwrap();
        sink.append_line("{}").unwrap();
        assert!(!dir.path().join(&ancient).exists());
        assert!(dir.path().join(format!("events-{floor}.jsonl")).exists());
    }

    #[test]
    fn civil_roundtrip_matches_input() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(days_from_civil(2026, 8, 25)), (2026, 8, 25));
        assert_eq!(shift_date("2026-08-25", -7).as_deref(), Some("2026-08-18"));
        assert_eq!(shift_date("2026-01-01", -1).as_deref(), Some("2025-12-31"));
    }
}
