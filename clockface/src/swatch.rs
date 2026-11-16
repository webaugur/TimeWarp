//! Swatch Internet Time, the same definition as TimeWarp `iso.swatch_beats`.
//!
//! TimeWarp does not stream a live beat in the almanac JSON. The kiosk uses the
//! library's formula: beats after midnight BMT (UTC+1, no daylight saving), one
//! beat = 86.4 seconds, displayed as `@000`…`@999`. The integer is Python 3's
//! `round` (half to even), then `mod 1000`, matching `format_swatch`.

use chrono::{DateTime, TimeZone, Timelike, Utc};

/// Beats after midnight BMT, in `0.0 .. 1000.0`.
pub fn swatch_beats<Tz: TimeZone>(now: &DateTime<Tz>) -> f64 {
    let utc = now.with_timezone(&Utc);
    let secs = f64::from(utc.hour()) * 3600.0
        + f64::from(utc.minute()) * 60.0
        + f64::from(utc.second())
        + f64::from(utc.nanosecond()) / 1.0e9;
    let bmt = (secs + 3600.0) % 86_400.0;
    bmt / 86.4
}

/// `@000` … `@999`, TimeWarp's `format_swatch`.
pub fn format_swatch<Tz: TimeZone>(now: &DateTime<Tz>) -> String {
    let n = python_round_nonneg(swatch_beats(now)).rem_euclid(1000);
    format!("@{n:03}")
}

/// Python 3 `round` for a non-negative number: halves go to the even integer.
fn python_round_nonneg(x: f64) -> i64 {
    if !x.is_finite() || x <= 0.0 {
        return 0;
    }
    let floor = x.floor();
    let frac = x - floor;
    let n = floor as i64;
    if (frac - 0.5).abs() <= 1e-9 {
        if n % 2 == 0 {
            n
        } else {
            n + 1
        }
    } else if frac > 0.5 {
        n + 1
    } else {
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{TimeZone, Utc};

    fn utc(y: i32, m: u32, d: u32, h: u32, min: u32, s: u32, nano: u32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(y, m, d, h, min, s)
            .unwrap()
            .with_nanosecond(nano)
            .unwrap()
    }

    #[test]
    fn bmt_midnight_and_noon_match_timewarp() {
        // @000 is 00:00 BMT = 23:00 UTC. @500 is noon BMT = 11:00 UTC.
        let midnight = utc(2026, 7, 3, 23, 0, 0, 0);
        assert!((swatch_beats(&midnight) - 0.0).abs() < 1e-9);
        assert_eq!(format_swatch(&midnight), "@000");
        let noon = utc(2026, 7, 4, 11, 0, 0, 0);
        assert!((swatch_beats(&noon) - 500.0).abs() < 1e-6);
        assert_eq!(format_swatch(&noon), "@500");
    }

    #[test]
    fn published_clocks_use_the_same_beat() {
        // 13:00Z @583, 18:52Q (UTC−4) @994, 17:52R (UTC−5) @994.
        assert_eq!(format_swatch(&utc(2026, 7, 4, 13, 0, 0, 0)), "@583");
        assert_eq!(format_swatch(&utc(2026, 7, 4, 22, 52, 0, 0)), "@994");
    }

    #[test]
    fn half_a_millisecond_before_the_next_bmt_day_wraps() {
        // TimeWarp: 22:59:59.500000 UTC rounds to @000.
        let just_shy = utc(2026, 7, 3, 22, 59, 59, 500_000_000);
        assert_eq!(format_swatch(&just_shy), "@000");
    }

    #[test]
    fn a_minute_holds_more_than_one_beat() {
        let start = utc(2026, 7, 4, 11, 0, 0, 0);
        let still = utc(2026, 7, 4, 11, 0, 30, 0);
        let next = utc(2026, 7, 4, 11, 0, 50, 0);
        assert_eq!(format_swatch(&start), format_swatch(&still));
        assert_eq!(format_swatch(&start), "@500");
        assert_eq!(format_swatch(&next), "@501");
    }
}
