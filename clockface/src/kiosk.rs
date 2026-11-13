//! Full kiosk: mural wedges, aperture discs, brass dial, info boxes, status bar.
//!
//! Paint order is back to front. Hands, numerals, ticks, and the brass ring are drawn after
//! the discs they overlap. Every region is placed from the clock center and the face radius,
//! so moving or resizing the face moves the complications with it.
//!
//! The year wheel (lower left) turns once per tropical year. The sky wheel (lower right)
//! turns once per local civil day. Each window is the square corner outside the dial: the
//! arc, the side tangent, and the bottom tangent. Sampling is
//! `shown = content + (angle - wedge_center) / 360`, so the middle of each window shows the
//! current season or time of day and the mural eases instead of snapping.

use chrono::{DateTime, Datelike, Local};

use egui::{Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Stroke, Vec2};

use crate::brass::{mix, shade_bezel};
use crate::draw::{paint_pie, stroke_arc};
use crate::events::{self, CalendarEvent};
use crate::layout::{KioskLayout, APERTURE_R, SKY_WEDGE, YEAR_WEDGE};
use crate::matrix;
use crate::mural;
use crate::roman::{self, direction};
use crate::scene;
use crate::settings::Place;
use crate::sky::{self, season_name};
use crate::status::StatusLine;
use crate::weather::{self, WeatherReport};

#[derive(Clone, Debug)]
pub struct KioskModel {
    pub now: DateTime<Local>,
    pub place: Place,
    pub weather: WeatherReport,
    pub events: Vec<CalendarEvent>,
    pub status: StatusLine,
}

impl KioskModel {
    /// Deterministic weather and the built-in calendar. Status is the real machine.
    pub fn for_screenshot(now: DateTime<Local>, place: &Place) -> Self {
        let mut status = crate::status::gather();
        status.alert.clear();
        Self {
            now,
            place: place.clone(),
            weather: weather::fixture_january(),
            events: events::examples(),
            status,
        }
    }
}

pub fn paint(painter: &Painter, screen: Rect, center: Pos2, diameter: f32, model: &KioskModel) {
    painter.rect_filled(screen, 0.0, Color32::BLACK);
    let layout = KioskLayout::new(screen, center, diameter);
    let radius = layout.radius;

    mural::paint(
        painter,
        painter.ctx(),
        center,
        radius,
        sky::year_fraction(model.now),
        sky::day_fraction(model.now),
    );
    paint_wedge_rim(painter, center, radius, YEAR_WEDGE);
    paint_wedge_rim(painter, center, radius, SKY_WEDGE);

    let aperture = APERTURE_R * radius;
    // Upper half, horizon on the 9–3 line. The SVG theme fills that semicircle.
    scene::paint_horizon(
        painter,
        painter.ctx(),
        center,
        aperture,
        model.now,
        &model.weather,
    );
    paint_aperture(painter, center, aperture, 180.0, 270.0, |p| {
        date_disc_color(center, aperture, p)
    });
    paint_aperture(painter, center, aperture, 90.0, 180.0, |p| {
        date_disc_color(center, aperture, p)
    });

    paint_date_words(painter, center, radius, model.now);
    paint_condition_words(painter, center, radius, &model.weather, model.now);

    // Same brass mesh the window and the screenshot both fill.
    roman::paint_brass(painter, center, radius, aperture);
    paint_aperture_rims(painter, center, radius);
    roman::paint_markings_and_hands(
        painter,
        center,
        diameter,
        model.now,
        &roman::RomanClockStyle::default(),
    );

    paint_boxes(painter, &layout, model);
    paint_status(painter, &layout, model);
}

fn paint_wedge_rim(painter: &Painter, center: Pos2, radius: f32, wedge: (f32, f32)) {
    // Straight outer edges: the side tangent and the bottom tangent. The dial is the inner edge.
    let side = if wedge.0 >= 180.0 { -1.0 } else { 1.0 };
    let nine_or_three = center + Vec2::new(side * radius, 0.0);
    let corner = center + Vec2::new(side * radius, radius);
    let six = center + Vec2::new(0.0, radius);
    let brass = shade_bezel(Vec2::new(side, 0.2), 0.45);
    let width = (radius * 0.02).max(1.5);
    painter.line_segment([nine_or_three, corner], Stroke::new(width, brass));
    painter.line_segment([corner, six], Stroke::new(width, brass));
}

fn paint_aperture(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    start: f32,
    end: f32,
    color_at: impl FnMut(Pos2) -> Color32,
) {
    paint_pie(painter, center, radius, start, end, 36, color_at);
}

fn date_disc_color(center: Pos2, radius: f32, point: Pos2) -> Color32 {
    let rel = (point - center) / radius.max(1.0);
    let dist = rel.length();
    mix(
        Color32::from_rgb(0xf4, 0xe6, 0xcc),
        Color32::from_rgb(0xd9, 0xc2, 0x98),
        (dist * 0.75).clamp(0.0, 1.0),
    )
}

fn quarter_clip(center: Pos2, radius: f32, left: bool) -> Rect {
    let aperture = APERTURE_R * radius;
    let (x0, x1) = if left {
        (center.x - aperture, center.x)
    } else {
        (center.x, center.x + aperture)
    };
    Rect::from_min_max(Pos2::new(x0, center.y), Pos2::new(x1, center.y + aperture))
}

fn paint_date_words(painter: &Painter, center: Pos2, radius: f32, now: DateTime<Local>) {
    let painter = painter.with_clip_rect(quarter_clip(center, radius, true));
    let origin = center + direction(225.0) * (0.36 * radius);
    let color = Color32::from_rgb(0x3a, 0x28, 0x16);
    let weekday = weekday_name(now.weekday().num_days_from_monday());
    let month = month_name(now.month());
    painter.text(
        origin + Vec2::new(0.0, -0.055 * radius),
        Align2::CENTER_CENTER,
        weekday,
        FontId::new(0.042 * radius, serif()),
        color,
    );
    painter.text(
        origin,
        Align2::CENTER_CENTER,
        month,
        FontId::new(0.04 * radius, serif()),
        color,
    );
    painter.text(
        origin + Vec2::new(0.0, 0.06 * radius),
        Align2::CENTER_CENTER,
        format!("\u{1F4C5} {}", now.day()),
        FontId::new(0.07 * radius, serif()),
        color,
    );
}

fn paint_condition_words(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    weather: &WeatherReport,
    _now: DateTime<Local>,
) {
    let painter = painter.with_clip_rect(quarter_clip(center, radius, false));
    let origin = center + direction(135.0) * (0.36 * radius);
    let color = Color32::from_rgb(0x3a, 0x28, 0x16);
    painter.text(
        origin + Vec2::new(0.0, -0.02 * radius),
        Align2::CENTER_CENTER,
        weather.temperature_text(),
        FontId::new(0.055 * radius, serif()),
        color,
    );
    painter.text(
        origin + Vec2::new(0.0, 0.045 * radius),
        Align2::CENTER_CENTER,
        {
            let mark = weather::condition_mark(weather.code.unwrap_or(-1));
            if mark.is_empty() {
                weather.condition.clone()
            } else {
                format!("{mark} {}", weather.condition)
            }
        },
        FontId::new(0.034 * radius, serif()),
        color,
    );
}

fn paint_aperture_rims(painter: &Painter, center: Pos2, radius: f32) {
    let brass = shade_bezel(Vec2::new(-0.4, -0.6), 0.35);
    let width = (0.012 * radius).max(1.5);
    stroke_arc(
        painter,
        center,
        APERTURE_R * radius,
        0.0,
        360.0,
        width,
        brass,
    );
    let aperture = APERTURE_R * radius;
    // Horizon across the 9–3 line, and the divider between the two lower quarters.
    painter.line_segment(
        [
            Pos2::new(center.x - aperture, center.y),
            Pos2::new(center.x + aperture, center.y),
        ],
        Stroke::new(width, brass),
    );
    painter.line_segment(
        [center, center + direction(180.0) * aperture],
        Stroke::new(width, brass),
    );
}

fn paint_boxes(painter: &Painter, layout: &KioskLayout, model: &KioskModel) {
    let pitch = matrix::frame_pitch(layout.radius);
    for (index, panel) in layout.boxes.iter().enumerate() {
        matrix::paint_board(painter, *panel, pitch);
        let origin = Pos2::new(panel.left() + pitch, panel.top() + pitch);
        match index {
            0 => paint_calendar(painter, *panel, origin, pitch, model),
            _ => paint_forecast(painter, *panel, origin, pitch, model),
        }
    }
}

fn paint_calendar(painter: &Painter, panel: Rect, origin: Pos2, pitch: f32, model: &KioskModel) {
    let today = model.now.date_naive();
    let rows = events::upcoming(&model.events, today, 4);
    let mut y = origin.y;
    if rows.is_empty() {
        matrix::paint_text(
            painter,
            panel,
            Pos2::new(origin.x, y),
            pitch,
            "\u{1F4C5} Nothing on the calendar",
            2,
        );
        return;
    }
    for event in rows.iter().take(4) {
        let title = format!(
            "{} {}",
            events::event_mark(&event.title, &event.time),
            event.title
        );
        y = matrix::paint_text(painter, panel, Pos2::new(origin.x, y), pitch, &title, 2);
        let when = if event.time.is_empty() || event.time.eq_ignore_ascii_case("all day") {
            events::short_when(event)
        } else {
            format!("\u{23F3} {}", events::short_when(event))
        };
        // The date sits under the title, inset like a wrapped continuation.
        y = matrix::paint_text(
            painter,
            panel,
            Pos2::new(origin.x + matrix::hang(pitch), y),
            pitch,
            &when,
            0,
        );
        y += pitch;
    }
}

fn paint_forecast(painter: &Painter, panel: Rect, origin: Pos2, pitch: f32, model: &KioskModel) {
    let mut y = origin.y;
    let date = model.now.date_naive();
    let mut sky_lines = Vec::new();
    if let Some(times) = sky::sun_times(date, model.place.latitude, model.place.longitude) {
        sky_lines.push(format!(
            "\u{2B06} Sunrise  {}",
            sky::format_local_hm(times.sunrise_unix)
        ));
        sky_lines.push(format!(
            "\u{2B07} Sunset   {}",
            sky::format_local_hm(times.sunset_unix)
        ));
    }
    sky_lines.push(format!("\u{1F31E} {}", season_name(date)));
    sky_lines.push(format!(
        "\u{1F319} {}",
        moon_name(sky::moon_phase(model.now))
    ));
    let (moonrise, moonset) = sky::moon_events(date, model.place.latitude, model.place.longitude);
    if let Some(when) = moonrise {
        sky_lines.push(format!("\u{2B06} Moonrise  {}", sky::format_local_hm(when)));
    }
    if let Some(when) = moonset {
        sky_lines.push(format!("\u{2B07} Moonset   {}", sky::format_local_hm(when)));
    }
    for line in &sky_lines {
        if y + pitch * 7.0 > panel.bottom() {
            break;
        }
        y = matrix::paint_text(painter, panel, Pos2::new(origin.x, y), pitch, line, 2);
    }
    y += pitch;
    if model.weather.daily.is_empty() {
        matrix::paint_text(
            painter,
            panel,
            Pos2::new(origin.x, y),
            pitch,
            "Forecast unavailable",
            2,
        );
        return;
    }
    for day in model.weather.daily.iter().take(5) {
        if y + pitch * 7.0 > panel.bottom() {
            break;
        }
        let mark = weather::condition_mark(day.code);
        let line = if mark.is_empty() {
            format!(
                "{} {} {:.0}°/{:.0}°",
                weather::weekday_short(&day.date),
                brief_condition(day.code),
                day.high,
                day.low,
            )
        } else {
            format!(
                "{mark} {} {} {:.0}°/{:.0}°",
                weather::weekday_short(&day.date),
                brief_condition(day.code),
                day.high,
                day.low,
            )
        };
        y = matrix::paint_text(painter, panel, Pos2::new(origin.x, y), pitch, &line, 2);
    }
}

fn paint_status(painter: &Painter, layout: &KioskLayout, model: &KioskModel) {
    let bar = layout.status;
    let text = model.status.text();
    let cap = ((bar.height() - 4.0) / 8.0).clamp(1.4, 2.4);
    let pitch = matrix::pitch_to_fit(&text, bar.width() - 8.0, cap);
    matrix::paint_board(painter, bar, pitch);
    let y = bar.center().y - pitch * 3.5;
    matrix::paint_text(
        painter,
        bar,
        Pos2::new(bar.left() + pitch, y),
        pitch,
        &text,
        0,
    );
}

fn brief_condition(code: i32) -> &'static str {
    match code {
        0 => "clear",
        1..=3 => "cloud",
        45 | 48 => "fog",
        51..=67 | 80..=82 => "rain",
        71..=77 | 85 | 86 => "snow",
        95..=99 => "storm",
        _ => "—",
    }
}

fn moon_name(phase: f64) -> &'static str {
    let p = phase.rem_euclid(1.0);
    if p < 0.03 || p > 0.97 {
        "new"
    } else if p < 0.22 {
        "waxing crescent"
    } else if p < 0.28 {
        "first quarter"
    } else if p < 0.47 {
        "waxing gibbous"
    } else if p < 0.53 {
        "full"
    } else if p < 0.72 {
        "waning gibbous"
    } else if p < 0.78 {
        "last quarter"
    } else {
        "waning crescent"
    }
}

fn serif() -> FontFamily {
    FontFamily::Name("serif".into())
}

fn weekday_name(index: u32) -> &'static str {
    [
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
        "Sunday",
    ][index as usize]
}

fn month_name(month: u32) -> &'static str {
    [
        "",
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ][month as usize]
}
