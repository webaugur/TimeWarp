//! Local moon phase, season, and sunrise/sunset.
//!
//! Moon: synodic age since the new moon of 2000-01-06 18:14 UTC (Julian day 2451550.25972),
//! using a mean synodic month of 29.530588853 days. Phase 0 is new, 0.25 is first quarter
//! (northern hemisphere: right half lit), 0.5 is full, 0.75 is last quarter (left half lit).
//! This is a mean-phase clock, not a full lunar ephemeris, and it matches the northern view
//! the MagicMirror kiosk used for Indianapolis.
//!
//! Season wheel: day-of-year as a fraction of the tropical year, blended on a circle between
//! the solstices and equinoxes so the mural eases and does not jump. The named season in the
//! sun box uses those same boundaries.
//!
//! Sky wheel: fraction of the local civil day. In the visible wedge the painted time equals
//! "now": midnight is night, noon is day. See [`crate::kiosk`] for which screen angle that is.

use chrono::{DateTime, Datelike, Local, NaiveDate, Timelike};

const KNOWN_NEW_JD: f64 = 2_451_550.25972;
const SYNODIC_DAYS: f64 = 29.530588853;
const UNIX_EPOCH_JD: f64 = 2_440_587.5;

/// 0 = new, 0.5 = full.
pub fn moon_phase_unix(unix_secs: f64) -> f64 {
    let jd = unix_secs / 86_400.0 + UNIX_EPOCH_JD;
    let age = (jd - KNOWN_NEW_JD).rem_euclid(SYNODIC_DAYS);
    age / SYNODIC_DAYS
}

pub fn moon_phase(now: DateTime<Local>) -> f64 {
    let secs = now.timestamp() as f64 + now.timestamp_subsec_nanos() as f64 / 1e9;
    moon_phase_unix(secs)
}

/// Fraction of the local civil day, 0 at midnight and 0.5 at noon.
pub fn day_fraction(now: DateTime<Local>) -> f32 {
    let secs =
        now.time().num_seconds_from_midnight() as f64 + now.timestamp_subsec_nanos() as f64 / 1e9;
    (secs / 86_400.0) as f32
}

/// Fraction of the year, 0 at the start of 1 January (local).
pub fn year_fraction(now: DateTime<Local>) -> f32 {
    let date = now.date_naive();
    let ordinal = date.ordinal0() as f32;
    let length = if date.leap_year() { 366.0 } else { 365.0 };
    ordinal / length
}

/// Northern-hemisphere season name from solstice and equinox dates (approximate civil dates).
pub fn season_name(date: NaiveDate) -> &'static str {
    let key = date.month() * 100 + date.day();
    if key >= 1221 || key < 320 {
        "Winter"
    } else if key < 621 {
        "Spring"
    } else if key < 922 {
        "Summer"
    } else {
        "Autumn"
    }
}

#[derive(Clone, Copy, Debug)]
pub struct SunTimes {
    pub sunrise_unix: f64,
    pub sunset_unix: f64,
}

/// Sunrise and sunset as Unix seconds (UTC) for a local calendar date.
///
/// NOAA solar-calculator approximation: mean anomaly, equation of center, and the
/// hour angle at −0.833° (refraction plus the solar radius). `lon_deg` is east-positive.
/// Returns `None` inside the polar day or night.
pub fn sun_times(date: NaiveDate, lat_deg: f64, lon_deg: f64) -> Option<SunTimes> {
    let jd = julian_day(date.year(), date.month(), date.day());
    // NOAA solar calculator: `n` is the whole number of days since J2000, and
    // longitude is east-positive (so a west longitude is subtracted as a negative).
    let n = (jd - 2_451_545.0 + 0.0008).ceil();
    let j_star = n - lon_deg / 360.0;
    let m_deg = (357.5291 + 0.985_600_28 * j_star).rem_euclid(360.0);
    let m = m_deg.to_radians();
    let c = 1.9148 * m.sin() + 0.0200 * (2.0 * m).sin() + 0.0003 * (3.0 * m).sin();
    let lambda_deg = (m_deg + c + 180.0 + 102.9372).rem_euclid(360.0);
    let lambda = lambda_deg.to_radians();
    let j_transit = 2_451_545.0 + j_star + 0.0053 * m.sin() - 0.0069 * (2.0 * lambda).sin();
    let sin_delta = lambda.sin() * 23.4397_f64.to_radians().sin();
    let delta = sin_delta.asin();
    let lat = lat_deg.to_radians();
    let cos_h =
        ((-0.833_f64).to_radians().sin() - lat.sin() * delta.sin()) / (lat.cos() * delta.cos());
    if !(-1.0..=1.0).contains(&cos_h) {
        return None;
    }
    let h_deg = cos_h.acos().to_degrees();
    let rise_jd = j_transit - h_deg / 360.0;
    let set_jd = j_transit + h_deg / 360.0;
    Some(SunTimes {
        sunrise_unix: (rise_jd - UNIX_EPOCH_JD) * 86_400.0,
        sunset_unix: (set_jd - UNIX_EPOCH_JD) * 86_400.0,
    })
}

fn julian_day(year: i32, month: u32, day: u32) -> f64 {
    let mut y = year;
    let mut m = month as i32;
    if m <= 2 {
        y -= 1;
        m += 12;
    }
    let a = (y as f64 / 100.0).floor();
    let b = 2.0 - a + (a / 4.0).floor();
    (365.25 * (y as f64 + 4716.0)).floor() + (30.6001 * (m as f64 + 1.0)).floor() + day as f64 + b
        - 1524.5
}

pub fn format_local_hm(unix_secs: f64) -> String {
    let secs = unix_secs.floor() as i64;
    let nanos = ((unix_secs - secs as f64) * 1e9).clamp(0.0, 999_999_999.0) as u32;
    match chrono::DateTime::from_timestamp(secs, nanos) {
        Some(utc) => {
            let local = utc.with_timezone(&Local);
            format!("{:02}:{:02}", local.hour(), local.minute())
        }
        None => "--:--".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_new_moon_is_near_zero_and_full_is_near_half() {
        // 2000-01-06 18:14:00 UTC.
        let new_moon = 947_182_440.0;
        let phase = moon_phase_unix(new_moon);
        assert!(phase < 0.02 || phase > 0.98, "new moon phase {phase}");
        let full = new_moon + 14.765 * 86_400.0;
        let full_phase = moon_phase_unix(full);
        assert!(
            (full_phase - 0.5).abs() < 0.03,
            "full moon phase {full_phase}"
        );
    }

    #[test]
    fn january_is_winter_and_july_is_summer() {
        assert_eq!(
            season_name(NaiveDate::from_ymd_opt(2026, 1, 15).unwrap()),
            "Winter"
        );
        assert_eq!(
            season_name(NaiveDate::from_ymd_opt(2026, 7, 4).unwrap()),
            "Summer"
        );
    }

    #[test]
    fn indianapolis_january_day_is_shorter_than_night() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let times = sun_times(date, 39.7684, -86.1581).unwrap();
        let hours = (times.sunset_unix - times.sunrise_unix) / 3600.0;
        assert!(times.sunrise_unix < times.sunset_unix);
        assert!(hours > 8.5 && hours < 11.0, "day length {hours} h");
    }
}
