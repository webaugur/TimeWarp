//! Upcoming calendar lines for the left-hand box.
//!
//! The live list is TimeWarp's holidays and seasons, with any `[events]` rows merged in.
//! These examples are only the fallback when `timewarp` cannot be read, so a fresh kiosk
//! is not blank. An empty `[events]` list adds no personal rows.

use chrono::{Datelike, NaiveDate};

#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
pub struct CalendarEvent {
    pub date: String,
    pub title: String,
    #[serde(default)]
    pub time: String,
}

pub fn examples() -> Vec<CalendarEvent> {
    vec![
        CalendarEvent {
            date: "2026-01-18".into(),
            title: "Family dinner".into(),
            time: "18:00".into(),
        },
        CalendarEvent {
            date: "2026-01-19".into(),
            title: "Martin Luther King Jr. Day".into(),
            time: "all day".into(),
        },
        CalendarEvent {
            date: "2026-11-14".into(),
            title: "Workshop open house".into(),
            time: "10:00".into(),
        },
        CalendarEvent {
            date: "2026-12-21".into(),
            title: "Winter solstice".into(),
            time: "all day".into(),
        },
        CalendarEvent {
            date: "2027-01-06".into(),
            title: "Epiphany".into(),
            time: "all day".into(),
        },
    ]
}

pub fn parse_date(text: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()
}

/// Events on or after `today`, soonest first. Undated rows are dropped.
pub fn upcoming(events: &[CalendarEvent], today: NaiveDate, limit: usize) -> Vec<CalendarEvent> {
    let mut rows: Vec<(NaiveDate, CalendarEvent)> = events
        .iter()
        .filter_map(|event| parse_date(&event.date).map(|date| (date, event.clone())))
        .filter(|(date, _)| *date >= today)
        .collect();
    rows.sort_by_key(|(date, _)| *date);
    rows.into_iter()
        .take(limit)
        .map(|(_, event)| event)
        .collect()
}

/// Trailing mark, using the same glyphs as `timewarp.ui.holiday_emoji` and `ICON`.
///
/// Solstices and equinoxes use the solar mark (🌞), the one TimeWarp puts on a
/// season heading. A named holiday uses that table. An all-day row with no closer
/// match uses the default holiday mark (🎉). A timed event uses the calendar mark (📅).
pub fn event_mark(title: &str, time: &str, date: &str) -> &'static str {
    let low = title.to_lowercase();
    if low.contains("solstice") || low.contains("equinox") {
        return "\u{1F31E}";
    }
    // Same table and order as timewarp.ui.holiday_emoji. Orthodox/Julian
    // Christmas on 6 January uses the eve mark, matching that function.
    let orthodox = low.contains("orthodox") || low.contains("julian");
    if date.ends_with("-01-06") && orthodox && low.contains("christmas") {
        return "\u{1F385}";
    }
    const KEYS: &[(&str, &str)] = &[
        ("orthodox christmas eve", "\u{1F385}"),
        ("julian christmas eve", "\u{1F385}"),
        ("christmas eve", "\u{1F385}"),
        ("orthodox christmas", "\u{1F384}"),
        ("julian christmas", "\u{1F384}"),
        ("christmas", "\u{1F384}"),
        ("new year", "\u{1F386}"),
        ("good friday", "\u{1F423}"),
        ("ascension", "\u{1F423}"),
        ("pentecost", "\u{1F423}"),
        ("whit", "\u{1F423}"),
        ("easter", "\u{1F423}"),
        ("thanksgiving", "\u{1F983}"),
        ("halloween", "\u{1F383}"),
        ("hanukkah", "\u{1F54E}"),
        ("chanukah", "\u{1F54E}"),
        ("diwali", "\u{1FA94}"),
        ("deepavali", "\u{1FA94}"),
        ("ramadan", "\u{1F319}"),
        ("eid", "\u{1F319}"),
        ("juneteenth", "\u{1F386}"),
        ("independence", "\u{1F386}"),
        ("bastille", "\u{1F386}"),
        ("canada day", "\u{1F386}"),
        ("national day", "\u{1F386}"),
        ("memorial", "\u{1F396}\u{FE0F}"),
        ("veterans", "\u{1F396}\u{FE0F}"),
        ("armistice", "\u{1F396}\u{FE0F}"),
        ("remembrance", "\u{1F396}\u{FE0F}"),
        ("anzac", "\u{1F396}\u{FE0F}"),
        ("may day", "\u{1F6E0}\u{FE0F}"),
        ("labour", "\u{1F6E0}\u{FE0F}"),
        ("labor", "\u{1F6E0}\u{FE0F}"),
    ];
    for (key, glyph) in KEYS {
        if low.contains(key) {
            return glyph;
        }
    }
    if time.eq_ignore_ascii_case("all day") {
        return "\u{1F389}";
    }
    "\u{1F4C5}"
}

pub fn short_when(event: &CalendarEvent) -> String {
    match parse_date(&event.date) {
        Some(date) => {
            let when = format!("{} {}", month_name(date.month()), date.day());
            if event.time.trim().is_empty() {
                when
            } else {
                format!("{when} · {}", event.time.trim())
            }
        }
        None => event.time.clone(),
    }
}

fn month_name(month: u32) -> &'static str {
    [
        "", "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ]
    .get(month as usize)
    .copied()
    .unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screenshot_day_and_november_both_have_upcoming_examples() {
        let events = examples();
        let january = NaiveDate::from_ymd_opt(2026, 1, 15).unwrap();
        let november = NaiveDate::from_ymd_opt(2026, 11, 11).unwrap();
        assert!(upcoming(&events, january, 4).len() >= 2);
        assert!(upcoming(&events, november, 4).len() >= 2);
        assert!(upcoming(&[], january, 4).is_empty());
    }

    #[test]
    fn marks_follow_the_timewarp_holiday_table() {
        assert_eq!(
            event_mark("Christmas Eve", "all day", "2026-12-24"),
            "\u{1F385}"
        );
        assert_eq!(
            event_mark("Christmas Day", "all day", "2026-12-25"),
            "\u{1F384}"
        );
        assert_eq!(
            event_mark("Orthodox Christmas", "all day", "2026-01-06"),
            "\u{1F385}"
        );
        assert_eq!(
            event_mark("Winter solstice", "all day", "2026-12-21"),
            "\u{1F31E}"
        );
        assert_eq!(
            event_mark("Thanksgiving Day", "all day", "2026-11-26"),
            "\u{1F983}"
        );
        assert_eq!(
            event_mark("Labor Day", "all day", "2026-09-07"),
            "\u{1F6E0}\u{FE0F}"
        );
        assert_eq!(
            event_mark("Martin Luther King Jr. Day", "all day", "2026-01-19"),
            "\u{1F389}"
        );
        assert_eq!(
            event_mark("Family dinner", "18:00", "2026-01-18"),
            "\u{1F4C5}"
        );
    }
}
