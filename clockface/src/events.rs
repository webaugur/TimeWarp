//! Upcoming calendar lines for the left-hand box.
//!
//! TimeWarp holidays can replace this list later. An absent `[events]` key uses the built-in
//! examples so a fresh kiosk is not blank. An empty list is respected and the box says so.

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
}
