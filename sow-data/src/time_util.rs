use std::time::{SystemTime, UNIX_EPOCH};

pub struct UtcDateTime {
    pub year: i64,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub millisecond: u32,
}

/// Computes the current UTC date and time from SystemTime.
/// Uses a pure-math Gregorian calendar algorithm with no external dependencies.
pub fn now_utc() -> UtcDateTime {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs() as i64;
    let ms = duration.subsec_millis();

    let days = secs.div_euclid(86400);
    let rem_secs = secs.rem_euclid(86400);
    let hour = (rem_secs / 3600) as u32;
    let minute = ((rem_secs % 3600) / 60) as u32;
    let second = (rem_secs % 60) as u32;

    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = if mp < 10 { y } else { y + 1 };

    UtcDateTime {
        year: y,
        month: m,
        day: d,
        hour,
        minute,
        second,
        millisecond: ms,
    }
}
