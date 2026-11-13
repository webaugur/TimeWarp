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

/// First moonrise and moonset of a local civil day, as Unix seconds.
///
/// Same Schlyter lunar position and horizon scan TimeWarp uses (`ephem._moon_raw`,
/// `rise.events_for_day`): sample the local day, bisect each crossing of the
/// refracted upper limb. A day with no rise, or no set, leaves that side empty.
/// Open-Meteo does not publish moonrise or moonset.
pub fn moon_events(date: NaiveDate, lat_deg: f64, lon_deg: f64) -> (Option<f64>, Option<f64>) {
    let Some(start) = local_midnight(date) else {
        return (None, None);
    };
    let Some(next) = date.succ_opt().and_then(local_midnight) else {
        return (None, None);
    };
    let start_unix = start.timestamp() as f64;
    let end_unix = next.timestamp() as f64;
    if end_unix <= start_unix {
        return (None, None);
    }
    let step = 240.0;
    let mut t = start_unix;
    let mut prev = moon_above(t, lat_deg, lon_deg);
    let mut rise = None;
    let mut set = None;
    while t < end_unix && (rise.is_none() || set.is_none()) {
        let next_t = (t + step).min(end_unix);
        let sample = moon_above(next_t, lat_deg, lon_deg);
        if prev != 0.0 && sample != 0.0 && prev.signum() != sample.signum() {
            let cross = bisect_horizon(t, next_t, prev, lat_deg, lon_deg);
            if (start_unix..end_unix).contains(&cross) {
                if sample > prev && rise.is_none() {
                    rise = Some(cross);
                } else if sample < prev && set.is_none() {
                    set = Some(cross);
                }
            }
        }
        t = next_t;
        prev = sample;
    }
    (rise, set)
}

fn local_midnight(date: NaiveDate) -> Option<DateTime<Local>> {
    date.and_hms_opt(0, 0, 0)?
        .and_local_timezone(Local)
        .earliest()
}

/// Altitude of the Moon's upper limb above the refracted horizon, degrees.
fn moon_above(unix: f64, lat_deg: f64, lon_deg: f64) -> f64 {
    let d = unix / 86_400.0 + UNIX_EPOCH_JD - 2_451_543.5;
    let sun = schlyter_sun(d);
    let (lon, lat, r) = schlyter_moon(d, &sun);
    let (ra, dec) = ecliptic_to_equatorial(lon, lat, r, sun.ecl);
    let ut_hours = (unix.rem_euclid(86_400.0)) / 3600.0;
    let gmst = rev(sun.l + 180.0 + ut_hours * 15.0);
    let mut ha = rev(rev(gmst + lon_deg) - ra);
    if ha > 180.0 {
        ha -= 360.0;
    }
    let x = cosd(ha) * cosd(dec);
    let z = sind(dec);
    let zhor = x * cosd(lat_deg) + z * sind(lat_deg);
    let mut alt = zhor.clamp(-1.0, 1.0).asin().to_degrees();
    let parallax = (1.0 / r.max(1e-6)).clamp(-1.0, 1.0).asin().to_degrees();
    alt -= parallax * cosd(alt);
    let semidiameter = (1873.7 * 60.0 / 2.0) / 3600.0 / r;
    alt - (-0.583 - semidiameter)
}

fn bisect_horizon(t0: f64, t1: f64, a0: f64, lat: f64, lon: f64) -> f64 {
    let mut lo = t0;
    let mut hi = t1;
    let mut alo = a0;
    for _ in 0..40 {
        if hi - lo < 0.5 {
            break;
        }
        let mid = (lo + hi) * 0.5;
        let amid = moon_above(mid, lat, lon);
        if (alo > 0.0) == (amid > 0.0) {
            lo = mid;
            alo = amid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) * 0.5
}

struct SchlyterSun {
    m: f64,
    l: f64,
    ecl: f64,
}

fn schlyter_sun(d: f64) -> SchlyterSun {
    let w = 282.9404 + 4.70935e-5 * d;
    let m = rev(356.0470 + 0.985_600_258_5 * d);
    let ecl = 23.4393 - 3.563e-7 * d;
    SchlyterSun {
        m,
        l: rev(w + m),
        ecl,
    }
}

fn schlyter_moon(d: f64, sun: &SchlyterSun) -> (f64, f64, f64) {
    let n = rev(125.1228 - 0.052_953_808_3 * d);
    let i = 5.1454;
    let w = rev(318.0634 + 0.164_357_322_3 * d);
    let a = 60.2666;
    let e = 0.054900;
    let m = rev(115.3654 + 13.064_992_950_9 * d);
    let ecc = eccentric_anomaly(m, e);
    let (v, mut r) = true_anomaly(a, e, ecc);
    let (xh, yh, zh) = ecliptic_xyz(r, n, i, w, v);
    let (mut lon, mut lat, dist) = lon_lat_r(xh, yh, zh);
    r = dist;
    let lm = rev(n + w + m);
    let dm = rev(lm - sun.l);
    let f = rev(lm - n);
    lon += -1.274 * sind(m - 2.0 * dm) + 0.658 * sind(2.0 * dm)
        - 0.186 * sind(sun.m)
        - 0.059 * sind(2.0 * m - 2.0 * dm)
        - 0.057 * sind(m - 2.0 * dm + sun.m)
        + 0.053 * sind(m + 2.0 * dm)
        + 0.046 * sind(2.0 * dm - sun.m)
        + 0.041 * sind(m - sun.m)
        - 0.035 * sind(dm)
        - 0.031 * sind(m + sun.m)
        - 0.015 * sind(2.0 * f - 2.0 * dm)
        + 0.011 * sind(m - 4.0 * dm);
    lat += -0.173 * sind(f - 2.0 * dm)
        - 0.055 * sind(m - f - 2.0 * dm)
        - 0.046 * sind(m + f - 2.0 * dm)
        + 0.033 * sind(f + 2.0 * dm)
        + 0.017 * sind(2.0 * m + f);
    r += -0.58 * cosd(m - 2.0 * dm) - 0.46 * cosd(2.0 * dm);
    (rev(lon), lat, r)
}

fn eccentric_anomaly(m_deg: f64, e: f64) -> f64 {
    let m = rev(m_deg).to_radians();
    let mut ecc = m + e * m.sin() * (1.0 + e * m.cos());
    if e > 0.05 {
        for _ in 0..30 {
            let denom = 1.0 - e * ecc.cos();
            if denom.abs() < 1e-12 {
                break;
            }
            let next = ecc - (ecc - e * ecc.sin() - m) / denom;
            if (next - ecc).abs() < 1e-10 {
                ecc = next;
                break;
            }
            ecc = next;
        }
    }
    ecc.to_degrees()
}

fn true_anomaly(a: f64, e: f64, ecc_deg: f64) -> (f64, f64) {
    let ecc = ecc_deg.to_radians();
    let xv = a * (ecc.cos() - e);
    let yv = a * (1.0 - e * e).max(0.0).sqrt() * ecc.sin();
    (yv.atan2(xv).to_degrees(), xv.hypot(yv))
}

fn ecliptic_xyz(r: f64, n: f64, i: f64, w: f64, v: f64) -> (f64, f64, f64) {
    let vw = v + w;
    let xh = r * (cosd(n) * cosd(vw) - sind(n) * sind(vw) * cosd(i));
    let yh = r * (sind(n) * cosd(vw) + cosd(n) * sind(vw) * cosd(i));
    let zh = r * (sind(vw) * sind(i));
    (xh, yh, zh)
}

fn lon_lat_r(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let r = (x * x + y * y + z * z).sqrt();
    let lon = rev(y.atan2(x).to_degrees());
    let lat = z.atan2(x.hypot(y)).to_degrees();
    (lon, lat, r)
}

fn ecliptic_to_equatorial(lon: f64, lat: f64, r: f64, ecl: f64) -> (f64, f64) {
    let xg = r * cosd(lon) * cosd(lat);
    let yg = r * sind(lon) * cosd(lat);
    let zg = r * sind(lat);
    let ye = yg * cosd(ecl) - zg * sind(ecl);
    let ze = yg * sind(ecl) + zg * cosd(ecl);
    (
        rev(ye.atan2(xg).to_degrees()),
        ze.atan2(xg.hypot(ye)).to_degrees(),
    )
}

fn sind(deg: f64) -> f64 {
    deg.to_radians().sin()
}

fn cosd(deg: f64) -> f64 {
    deg.to_radians().cos()
}

fn rev(deg: f64) -> f64 {
    deg.rem_euclid(360.0)
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

    #[test]
    fn january_screenshot_day_has_a_moonrise_and_a_moonset() {
        let date = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let (rise, set) = moon_events(date, 39.7684, -86.1581);
        let rise = rise.expect("moonrise");
        let set = set.expect("moonset");
        assert!((rise - set).abs() > 3600.0);
        let clock = format_local_hm(rise);
        assert_eq!(clock.len(), 5, "{clock}");
        assert!(clock.as_bytes()[2] == b':');
    }
}
