//! Daily period of H. Spencer Lewis, *Self-Mastery and Fate with the Cycles of Life*
//! (San Jose: AMORC, 1929), chapters 11–12 and Chart E (p. 105).
//!
//! The day is seven equal parts from midnight to midnight, about 3 h 25 min 43 s each.
//! Lewis's printed boundaries (00:00, 03:25, 06:51, 10:17, 13:42, 17:08, 20:34) are that
//! division rounded to the minute. This module uses the exact seventh so the last period
//! ends at the next midnight. He says to use local standard time and to ignore daylight
//! saving.
//!
//! Chart E is letters, not colors. The same clock hour is a different letter on a
//! different weekday. The letter is the color's key. The planets on those letters are the
//! ones Lewis's daily cycle is built on (Chaldean order, day ruler on the sunrise
//! period): A Sun, B Venus, C Mercury, D Moon, E Saturn, F Jupiter, G Mars. That
//! assignment is set out in Julie Nordhaus-Bike, "Mystical Astrology Behind
//! *Self-Mastery and Fate with the Cycles of Life*," *Rose+Croix Journal* 16 (2022).
//!
//! Lewis never prints a pigment for a letter. The pigment is the planet's color in
//! Max Heindel, *The Message of the Stars* (1918), the table on p. 64, as restated in
//! "Stars, Jewels, and Angels," *Rays from the Rose Cross*, July/August 1999:
//! Sun orange, Venus yellow, Mercury violet, Moon green, Saturn indigo, Jupiter blue,
//! Mars red.

use chrono::{DateTime, Datelike, Local, LocalResult, TimeZone, Timelike, Weekday};
use egui::Color32;

/// One seventh of a civil day, in seconds. Lewis's "3 hours, 25 minutes, and 43 seconds."
pub const PERIOD_SECS: f64 = 86_400.0 / 7.0;

const LETTERS: [char; 7] = ['A', 'B', 'C', 'D', 'E', 'F', 'G'];

/// Seconds to subtract from the civil clock so the result is local standard time.
///
/// Daylight saving, where this zone has it, is the larger of the January and July
/// offsets. The smaller of those two is the standard offset. Zones with no daylight
/// saving (this machine's UTC, and Indianapolis) contribute nothing.
pub fn dst_advance_seconds(now: DateTime<Local>) -> i32 {
    let year = now.year();
    let (Some(january), Some(july)) = (noon(year, 1, 15), noon(year, 7, 15)) else {
        return 0;
    };
    let standard = january
        .offset()
        .local_minus_utc()
        .min(july.offset().local_minus_utc());
    now.offset().local_minus_utc() - standard
}

/// Local standard seconds since midnight, in `0..86400`.
pub fn standard_seconds_from_midnight(now: DateTime<Local>) -> u32 {
    let civil = now.time().num_seconds_from_midnight() as i64;
    let standard = civil - i64::from(dst_advance_seconds(now));
    standard.rem_euclid(86_400) as u32
}

/// `0` is midnight–03:25, `2` is 06:51–10:17, `6` is 20:34–midnight.
pub fn period_index(standard_secs: u32) -> u8 {
    ((f64::from(standard_secs) / PERIOD_SECS).floor() as u8).min(6)
}

/// Chart E letter for this weekday and period. Sunday's first period is G, and each
/// later period steps one letter forward. Each later weekday starts three letters later,
/// which is why Monday 08:00 is E and Thursday 10:08 is G.
pub fn chart_e_letter(weekday: Weekday, period: u8) -> char {
    let sun0 = weekday.num_days_from_sunday();
    let start = (6 + 3 * sun0) % 7;
    LETTERS[((start + u32::from(period)) % 7) as usize]
}

pub fn letter_at(now: DateTime<Local>) -> char {
    let secs = standard_seconds_from_midnight(now);
    chart_e_letter(now.weekday(), period_index(secs))
}

/// Heindel's color for the Chart E letter in force at `now`.
pub fn minute_tick_color(now: DateTime<Local>) -> Color32 {
    color_for_letter(letter_at(now))
}

/// Letter for one 1-minute mark on the 12-hour dial.
///
/// The dial is the civil half the hour hand is in: midnight–noon before noon,
/// noon–midnight after. Mark `step` (0 at 12, clockwise) covers the 12 minutes
/// that begin `step * 12` minutes into that half. A slice that crosses a period
/// boundary takes the period of its midpoint, six minutes in. Daylight saving is
/// removed before the period is chosen, and a midpoint that falls on the previous
/// or next standard day uses that day's Chart E row.
pub fn tick_letter(now: DateTime<Local>, step: u32) -> char {
    let pm = now.hour() >= 12;
    tick_period_letter(now.weekday(), pm, step, dst_at_tick(now, pm, step))
}

/// Chart E letter for minute-mark `step` of one half-day.
///
/// `dst_advance_secs` is the civil clock's lead over local standard time
/// (0 in winter and in zones with no daylight saving, 3600 during summer time).
pub fn tick_period_letter(weekday: Weekday, pm: bool, step: u32, dst_advance_secs: i32) -> char {
    let half = if pm { 12 * 3600 } else { 0 };
    let midpoint = i64::from(half) + i64::from(step) * 12 * 60 + 6 * 60;
    let shifted = midpoint - i64::from(dst_advance_secs);
    let day_shift = if shifted < 0 {
        -1
    } else if shifted >= 86_400 {
        1
    } else {
        0
    };
    let secs = shifted.rem_euclid(86_400) as u32;
    chart_e_letter(shift_weekday(weekday, day_shift), period_index(secs))
}

/// Heindel color of minute-mark `step` for the half-day `now` is in.
pub fn minute_tick_color_at(now: DateTime<Local>, step: u32) -> Color32 {
    color_for_letter(tick_letter(now, step))
}

fn dst_at_tick(now: DateTime<Local>, pm: bool, step: u32) -> i32 {
    let half_h = if pm { 12 } else { 0 };
    let mins = step * 12 + 6;
    let hour = half_h + mins / 60;
    let minute = mins % 60;
    match Local.with_ymd_and_hms(now.year(), now.month(), now.day(), hour, minute, 0) {
        LocalResult::Single(time) => dst_advance_seconds(time),
        LocalResult::Ambiguous(earliest, _) => dst_advance_seconds(earliest),
        LocalResult::None => dst_advance_seconds(now),
    }
}

fn shift_weekday(mut weekday: Weekday, days: i32) -> Weekday {
    let mut n = days;
    while n > 0 {
        weekday = weekday.succ();
        n -= 1;
    }
    while n < 0 {
        weekday = weekday.pred();
        n += 1;
    }
    weekday
}

pub fn color_for_letter(letter: char) -> Color32 {
    match letter {
        'A' => Color32::from_rgb(0xFF, 0x8C, 0x00), // Sun, orange
        'B' => Color32::from_rgb(0xE6, 0xC2, 0x00), // Venus, yellow
        'C' => Color32::from_rgb(0x7B, 0x2D, 0xB0), // Mercury, violet
        'D' => Color32::from_rgb(0x2E, 0x8B, 0x57), // Moon, green
        'E' => Color32::from_rgb(0x32, 0x2A, 0x6E), // Saturn, indigo
        'F' => Color32::from_rgb(0x1E, 0x5A, 0xB0), // Jupiter, blue
        _ => Color32::from_rgb(0xD0, 0x22, 0x22),   // Mars, red (G, and any stray)
    }
}

fn noon(year: i32, month: u32, day: u32) -> Option<DateTime<Local>> {
    match Local.with_ymd_and_hms(year, month, day, 12, 0, 0) {
        LocalResult::Single(time) => Some(time),
        LocalResult::Ambiguous(earliest, _) => Some(earliest),
        LocalResult::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roman::screenshot_time;

    #[test]
    fn chart_e_matches_lewis_page_105() {
        // Monday 08:00 is the third period and the letter E. Monday noon is F.
        // Thursday's third period (the screenshot hour) is G.
        assert_eq!(chart_e_letter(Weekday::Mon, 2), 'E');
        assert_eq!(chart_e_letter(Weekday::Mon, 3), 'F');
        assert_eq!(chart_e_letter(Weekday::Thu, 2), 'G');
        assert_eq!(chart_e_letter(Weekday::Sun, 0), 'G');
        assert_eq!(chart_e_letter(Weekday::Sat, 2), 'F');
        assert_eq!(chart_e_letter(Weekday::Wed, 6), 'A');
    }

    #[test]
    fn equal_sevenths_match_lewis_three_hours_twenty_five_minutes() {
        // 86400/7 is 3 h 25 min 42.857 s. Lewis rounds that to 3 h 25 min 43 s
        // and prints the boundaries as 03:25, 06:51, 10:17, and so on.
        assert!((PERIOD_SECS - (3.0 * 3600.0 + 25.0 * 60.0 + 42.857)).abs() < 0.01);
        assert_eq!(period_index(0), 0);
        assert_eq!(period_index(PERIOD_SECS.ceil() as u32), 1);
        assert_eq!(period_index((2.0 * PERIOD_SECS).ceil() as u32), 2);
        assert_eq!(period_index(86_399), 6);
        // 10:08:37 is inside 06:51–10:17, the third period, on either rounding.
        let ten_oh_eight = 10 * 3600 + 8 * 60 + 37;
        assert_eq!(period_index(ten_oh_eight), 2);
    }

    #[test]
    fn screenshot_morning_is_thursday_period_g_mars_red() {
        let now = screenshot_time();
        assert_eq!(now.weekday(), Weekday::Thu);
        assert_eq!(dst_advance_seconds(now), 0, "January is standard time");
        assert_eq!(letter_at(now), 'G');
        let color = minute_tick_color(now);
        assert_eq!(color, Color32::from_rgb(0xD0, 0x22, 0x22));
    }

    #[test]
    fn morning_dial_uses_each_ticks_own_period() {
        // Thursday 10:08 is the morning half. Each mark is a 12-minute slice
        // from midnight, colored by the slice's midpoint. The 17th mark
        // straddles 06:51, and the midpoint is already in the next period.
        let thu = Weekday::Thu;
        assert_eq!(tick_period_letter(thu, false, 1, 0), 'E');
        assert_eq!(tick_period_letter(thu, false, 16, 0), 'E');
        assert_eq!(tick_period_letter(thu, false, 17, 0), 'F');
        assert_eq!(tick_period_letter(thu, false, 33, 0), 'F');
        assert_eq!(tick_period_letter(thu, false, 34, 0), 'G');
        assert_eq!(tick_period_letter(thu, false, 50, 0), 'G');
        assert_eq!(tick_period_letter(thu, false, 51, 0), 'A');
        assert_eq!(tick_period_letter(thu, false, 59, 0), 'A');

        let now = screenshot_time();
        assert!(now.hour() < 12);
        assert_eq!(tick_letter(now, 1), 'E');
        assert_eq!(minute_tick_color_at(now, 1), color_for_letter('E'));
        assert_eq!(minute_tick_color_at(now, 34), color_for_letter('G'));
        assert_eq!(minute_tick_color_at(now, 51), color_for_letter('A'));
        let letters: Vec<char> = (0..60).map(|step| tick_letter(now, step)).collect();
        let distinct = {
            let mut seen = letters.clone();
            seen.sort_unstable();
            seen.dedup();
            seen.len()
        };
        assert!(
            distinct >= 3,
            "morning ring should cross at least three periods, got {letters:?}"
        );
    }

    #[test]
    fn daylight_saving_shifts_the_tick_into_standard_time() {
        // Thursday 14:18 civil is period 4, letter B. An hour of daylight
        // saving makes that same midpoint 13:18 standard time, still period 3,
        // letter A. Just after civil midnight, the same hour belongs to
        // Wednesday evening.
        assert_eq!(tick_period_letter(Weekday::Thu, true, 11, 0), 'B');
        assert_eq!(tick_period_letter(Weekday::Thu, true, 11, 3600), 'A');
        // Civil 00:06 Thursday minus one hour of daylight saving is 23:06 Wednesday.
        assert_eq!(tick_period_letter(Weekday::Thu, false, 0, 3600), 'A');
        assert_eq!(tick_period_letter(Weekday::Wed, true, 59, 0), 'A');
    }
}
