//! A migration's time, `YYYYMMDDHHMMSS` in UTC, to and from seconds since the epoch.

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_seconds() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

/// Days since 1970-01-01 of a civil date (Howard Hinnant's `days_from_civil`).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

pub fn stamp(seconds: i64) -> String {
    let (days, rest) = (seconds.div_euclid(86400), seconds.rem_euclid(86400));
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}{m:02}{d:02}{:02}{:02}{:02}", rest / 3600, rest % 3600 / 60, rest % 60)
}

/// The seconds of a `YYYYMMDDHHMMSS` time.
pub fn seconds(stamp: &str) -> Option<i64> {
    if stamp.len() != 14 || !stamp.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n = |a: usize, b: usize| stamp[a..b].parse::<i64>().ok();
    Some(days_from_civil(n(0, 4)?, n(4, 6)?, n(6, 8)?) * 86400 + n(8, 10)? * 3600 + n(10, 12)? * 60 + n(12, 14)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        assert_eq!(stamp(0), "19700101000000");
        assert_eq!(seconds("20261030153000").map(stamp).as_deref(), Some("20261030153000"));
        assert_eq!(seconds("20240229235959").map(|s| stamp(s + 1)).as_deref(), Some("20240301000000"));
        assert_eq!(seconds("2026"), None);
    }
}
