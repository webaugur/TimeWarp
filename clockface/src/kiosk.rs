//! Full kiosk: mural wedges, aperture discs, brass dial, info boxes, status bar.
//!
//! Paint order is back to front. Hands, numerals, ticks, and the brass ring are drawn after
//! the discs they overlap. Every region is placed from the clock center and the face radius,
//! so moving or resizing the face moves the complications with it.
//!
//! The two lower corners, outside the dial and below the 9–3 line, stay the black
//! screen. The trough and the gauges are the only paint there. The sun and moon
//! quarters above the 9–3 line are a separate layer and still fill the upper half.

use std::time::Duration;

use chrono::{DateTime, Datelike, Local, Timelike};

use egui::{Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Vec2};

use crate::events::{self, CalendarEvent};
use crate::layout::{KioskLayout, APERTURE_R};
use crate::matrix;
use crate::quarters;
use crate::roman;
use crate::scene;
use crate::settings::Place;
use crate::sky::{self, DaySky};
use crate::status::StatusLine;
use crate::sun_image::SunImage;
use crate::weather::{self, WeatherReport};

#[derive(Clone, Debug)]
pub struct KioskModel {
    pub now: DateTime<Local>,
    pub place: Place,
    pub weather: WeatherReport,
    pub events: Vec<CalendarEvent>,
    pub sky: DaySky,
    pub status: StatusLine,
    /// Latest GOES SUVI frame, when one has been fetched. None draws a plain sun.
    pub sun: Option<std::sync::Arc<SunImage>>,
    /// Hours ahead for the barometer's forecast tick. Three is the tendency window.
    pub barometer_hours: u32,
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
            sky: DaySky::builtin(now, place),
            status,
            sun: None,
            barometer_hours: 3,
        }
    }
}

pub fn paint(painter: &Painter, screen: Rect, center: Pos2, diameter: f32, model: &KioskModel) {
    painter.rect_filled(screen, 0.0, Color32::BLACK);
    let layout = KioskLayout::new(screen, center, diameter);
    let radius = layout.radius;

    let aperture = APERTURE_R * radius;
    // Lower half: the landscape, sky on the 9–3 line and ground toward 6.
    scene::paint_horizon(
        painter,
        painter.ctx(),
        center,
        aperture,
        model.now,
        &model.weather,
        Some(model.sky.season.as_str()),
    );
    // Upper quarters: sun on the left (9 to 12), moon on the right (12 to 3).
    quarters::paint(
        painter,
        painter.ctx(),
        center,
        aperture,
        model.sky.moon_phase,
        model.sun.as_deref(),
    );
    paint_weather_caption(painter, center, radius, &model.weather);

    // Under the dial, so the chrome meets the cream and the boards cover the rest.
    crate::gauges::paint_trough(painter, center, radius, &model.weather);
    // Same dial mesh the window and the screenshot both fill.
    roman::paint_brass(painter, center, radius, aperture);
    roman::paint_markings_and_hands(
        painter,
        center,
        diameter,
        model.now,
        &roman::RomanClockStyle::default(),
        model.weather.wind_hand(),
    );
    crate::gauges::paint(
        painter,
        center,
        radius,
        model.now,
        &model.weather,
        model.barometer_hours,
    );

    paint_boxes(painter, &layout, model);
    paint_status(painter, &layout, model);
    crate::leds::paint_debug(painter, screen);
}

fn paint_weather_caption(painter: &Painter, center: Pos2, radius: f32, weather: &WeatherReport) {
    let aperture = APERTURE_R * radius;
    let clip = Rect::from_min_max(
        Pos2::new(center.x - aperture, center.y),
        Pos2::new(center.x + aperture, center.y + aperture),
    );
    let painter = painter.with_clip_rect(clip);
    let temp = weather.temperature_text();
    let condition = weather.condition.as_str();
    let color = Color32::from_rgb(0xff, 0xf6, 0xe8);
    // Roman numeral cap height is `numeral_size` (0.24 of the radius). The
    // condition line is 42% of that. Shrink only if the pair would leave the
    // landscape circle or sit on the hand hub.
    let mut temp_size = roman::RomanClockStyle::default().numeral_size * radius;
    let (temp_at, cond_at, cond_size) = loop {
        let cond_size = temp_size * 0.42;
        let (_, th) = caption_extent(&painter, &temp, temp_size);
        let (cw, ch) = caption_extent(&painter, condition, cond_size);
        let (tw, _) = caption_extent(&painter, &temp, temp_size);
        let gap = 0.018 * radius;
        let sep = th * 0.5 + gap + ch * 0.5;
        let halo = 0.014 * radius;
        let hub = 0.12 * radius;
        let temp_y = center.y + hub + th * 0.5 + halo;
        let cond_y = temp_y + sep;
        let bottom = cond_y + ch * 0.5 + halo;
        let inside = bottom <= center.y + aperture - 0.012 * radius
            && caption_in_circle(center, aperture, temp_y, tw, th, halo)
            && caption_in_circle(center, aperture, cond_y, cw, ch, halo);
        if inside || temp_size < 0.08 * radius {
            break (
                Pos2::new(center.x, temp_y),
                Pos2::new(center.x, cond_y),
                cond_size,
            );
        }
        temp_size *= 0.94;
    };
    paint_shadowed(&painter, temp_at, &temp, temp_size, color, radius);
    paint_shadowed(&painter, cond_at, condition, cond_size, color, radius);
}

fn caption_extent(painter: &Painter, text: &str, size: f32) -> (f32, f32) {
    let galley = painter.layout(
        text.to_owned(),
        FontId::new(size, serif()),
        Color32::PLACEHOLDER,
        f32::INFINITY,
    );
    (galley.size().x, galley.size().y)
}

fn caption_in_circle(
    center: Pos2,
    aperture: f32,
    y: f32,
    width: f32,
    height: f32,
    halo: f32,
) -> bool {
    let half_w = width * 0.5 + halo;
    let half_h = height * 0.5 + halo;
    for dy in [-half_h, half_h] {
        let down = (y + dy) - center.y;
        if down < 0.0 || down >= aperture {
            return false;
        }
        let chord = (aperture * aperture - down * down).sqrt();
        if half_w > chord - 1.0 {
            return false;
        }
    }
    true
}

/// Soft black halo, the same stacked blur the hands use, plus a slight drop.
fn paint_shadowed(
    painter: &Painter,
    pos: Pos2,
    text: &str,
    size: f32,
    color: Color32,
    radius: f32,
) {
    let font = FontId::new(size, serif());
    let blur = 0.012 * radius;
    let drop = Vec2::new(0.15 * blur, 0.55 * blur);
    let shadow = Color32::from_rgba_unmultiplied(0, 0, 0, 153);
    let scale = (shadow.a() as f32 / 255.0) / 0.6;
    let [red, green, blue, _] = shadow.to_array();
    for (spread, alpha) in roman::SHADOW_LAYERS {
        let a = ((alpha * scale) * 255.0).round().clamp(0.0, 255.0) as u8;
        if a == 0 {
            continue;
        }
        let layer = Color32::from_rgba_unmultiplied(red, green, blue, a);
        let reach = blur * spread * 0.5;
        for step in 0..12 {
            let ang = step as f32 / 12.0 * std::f32::consts::TAU;
            let offset = Vec2::new(ang.cos(), ang.sin()) * reach;
            painter.text(
                pos + drop + offset,
                Align2::CENTER_CENTER,
                text,
                font.clone(),
                layer,
            );
        }
    }
    painter.text(pos, Align2::CENTER_CENTER, text, font, color);
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
    let mut y = origin.y;
    if line_fits(y, pitch, panel) {
        let clock = board_clock_line(&model.now);
        y = paint_heading(painter, panel, origin, pitch, &clock);
    }
    let today = model.now.date_naive();
    let rows = events::upcoming(&model.events, today, 4);
    if rows.is_empty() {
        if line_fits(y, pitch, panel) {
            matrix::paint_text(
                painter,
                panel,
                Pos2::new(origin.x, y),
                pitch,
                "\u{1F4C5} Nothing on the calendar",
                2,
            );
        }
        return;
    }
    for event in rows.iter().take(4) {
        if !line_fits(y, pitch, panel) {
            break;
        }
        let title = format!(
            "{} {}",
            events::event_mark(&event.title, &event.time),
            event.title
        );
        y = matrix::paint_text(painter, panel, Pos2::new(origin.x, y), pitch, &title, 2);
        if !line_fits(y, pitch, panel) {
            break;
        }
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
    if line_fits(y, pitch, panel) {
        let today = format!("\u{1F4C5} {}", board_date(model.now));
        y = paint_heading(painter, panel, origin, pitch, &today);
    }
    let mut sky_lines = Vec::new();
    if let Some(when) = model.sky.sunrise_unix {
        sky_lines.push(format!("\u{2B06} Sunrise  {}", sky::format_local_hm(when)));
    }
    if let Some(when) = model.sky.sunset_unix {
        sky_lines.push(format!("\u{2B07} Sunset   {}", sky::format_local_hm(when)));
    }
    sky_lines.push(format!("\u{1F31E} {}", model.sky.season));
    sky_lines.push(format!("\u{1F319} {}", moon_name(model.sky.moon_phase)));
    if let Some(when) = model.sky.moonrise_unix {
        sky_lines.push(format!("\u{2B06} Moonrise  {}", sky::format_local_hm(when)));
    }
    if let Some(when) = model.sky.moonset_unix {
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
    // The old ceiling was ((height - 4) / 8), at most 2.4. About 12% smaller,
    // still in the same range as the board lamps.
    let previous = ((bar.height() - 4.0) / 8.0).clamp(1.4, 2.4);
    let pitch = (previous * 0.88).clamp(1.3, 2.2);
    matrix::paint_board(painter, bar, pitch);
    let y = bar.center().y - pitch * 3.5;
    let version = StatusLine::version_text();
    let mark = |ch: char| {
        let font = FontId::new(pitch * 7.0, serif());
        painter
            .layout_no_wrap(ch.to_string(), font, Color32::WHITE)
            .size()
            .x
            + pitch * 0.5
    };
    let version_w = matrix::text_width(&version, pitch, &mark);
    // Last lamp of the version sits one pitch in from the right edge.
    let version_x = bar.right() - version_w + pitch;
    let left_max = (version_x - matrix::cell(pitch) * 2.0) - (bar.left() + pitch);
    let left = matrix::ellipsize(&model.status.left_text(), pitch, left_max.max(0.0), mark);
    matrix::paint_text(
        painter,
        bar,
        Pos2::new(bar.left() + pitch, y),
        pitch,
        &left,
        0,
    );
    matrix::paint_text_nowrap(painter, bar, Pos2::new(version_x, y), pitch, &version);
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

/// How long to wait before painting again.
///
/// The seconds digit and the hands change on the whole second.
/// During the second hand's bounce the face still paints often enough to show it.
/// After that it waits for the next second.
pub fn repaint_delay(now: DateTime<Local>) -> Duration {
    let into_ms = f64::from(now.timestamp_subsec_millis());
    let window = crate::roman::RomanClockStyle::default()
        .second_bounce
        .window_ms;
    if into_ms < window {
        Duration::from_millis(16)
    } else {
        let remain = (1000.0 - into_ms).ceil() as u64;
        Duration::from_millis(remain.max(1))
    }
}

/// Local civil time as HH:MM:SS, the same 24-hour form as sunrise and sunset.
fn board_hms(now: &DateTime<Local>) -> String {
    format!("{:02}:{:02}:{:02}", now.hour(), now.minute(), now.second())
}

/// Left-board clock: local `HH:MM:SS` and the Cycles color, for example `10:08:37 RED`.
fn board_clock_line(now: &DateTime<Local>) -> String {
    format!(
        "\u{1F55B} {} {}",
        board_hms(now),
        crate::cycles::period_color_name(*now)
    )
}

/// Local civil date in the board's short style, for example `THU JAN 15 2026`.
fn board_date(now: DateTime<Local>) -> String {
    const DAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];
    const MONTHS: [&str; 13] = [
        "", "JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC",
    ];
    format!(
        "{} {} {} {}",
        DAYS[now.weekday().num_days_from_sunday() as usize],
        MONTHS[now.month() as usize],
        now.day(),
        now.year()
    )
}

fn line_fits(y: f32, pitch: f32, panel: Rect) -> bool {
    y + pitch * 7.0 <= panel.bottom()
}

/// A heading, then one lit row, then a blank character cell before the next line.
///
/// The glyph occupies seven rows and the line advances nine, so one grid row
/// stays dark above the rule. The next line starts seven pitches below the rule,
/// which is the height of one character cell of unlit lamps.
fn paint_heading(painter: &Painter, panel: Rect, origin: Pos2, pitch: f32, text: &str) -> f32 {
    let next = matrix::paint_text(painter, panel, origin, pitch, text, 2);
    let rule_y = next - pitch;
    matrix::paint_rule(painter, panel, rule_y, pitch);
    rule_y + pitch * 7.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roman::screenshot_time;
    use crate::settings::Settings;
    use chrono::TimeZone;
    use egui::Vec2;

    #[test]
    fn the_board_clock_is_24_hour_and_the_date_is_the_short_form() {
        let now = screenshot_time();
        assert_eq!(board_hms(&now), "10:08:37");
        assert_eq!(board_date(now), "THU JAN 15 2026");
        assert_eq!(board_clock_line(&now), "\u{1F55B} 10:08:37 RED");
        let sunday = match Local.with_ymd_and_hms(2026, 11, 15, 9, 5, 0) {
            chrono::LocalResult::Single(time) => time,
            chrono::LocalResult::Ambiguous(earliest, _) => earliest,
            chrono::LocalResult::None => panic!("2026-11-15 09:05 is not a local time"),
        };
        assert_eq!(board_hms(&sunday), "09:05:00");
        assert_eq!(board_date(sunday), "SUN NOV 15 2026");
    }

    #[test]
    fn repaint_waits_out_the_rest_of_the_second() {
        let at_tick = screenshot_time();
        assert_eq!(repaint_delay(at_tick), Duration::from_millis(16));
        let after_bounce = at_tick + chrono::Duration::milliseconds(400);
        let wait = repaint_delay(after_bounce);
        assert_eq!(wait, Duration::from_millis(600));
    }

    #[test]
    fn the_clock_line_fits_on_one_row_of_the_board() {
        // Longest Heindel name. The 643-pixel face holds it on one line.
        assert!(clock_line_fits(
            "\u{1F55B} 10:08:37 ORANGE",
            Vec2::new(1920.0, 1080.0)
        ));
        assert!(clock_line_fits(
            "\u{1F55B} 10:08:37 RED",
            Vec2::new(1200.0, 800.0)
        ));
    }

    fn clock_line_fits(line: &str, viewport: Vec2) -> bool {
        let (center, diameter) = Settings::default().placement(viewport);
        let screen = Rect::from_min_size(Pos2::ZERO, viewport);
        let layout = KioskLayout::new(screen, center, diameter);
        let pitch = matrix::frame_pitch(layout.radius);
        let panel = layout.boxes[0];
        let room = panel.width() - pitch - pitch * 0.4;
        let width = matrix::text_width(line, pitch, |_| pitch * 8.0);
        width <= room
    }

    #[test]
    fn time_and_date_leave_room_for_the_rest_of_each_board() {
        for viewport in [Vec2::new(1920.0, 1080.0), Vec2::new(1200.0, 800.0)] {
            let (center, diameter) = Settings::default().placement(viewport);
            let screen = Rect::from_min_size(Pos2::ZERO, viewport);
            let layout = KioskLayout::new(screen, center, diameter);
            let pitch = matrix::frame_pitch(layout.radius);
            let line = pitch * 9.0;
            let left = layout.boxes[0];
            // Heading ends one character cell below the rule. Then four events, one wrap.
            let heading = line + pitch * 6.0;
            let left_used = heading + 4.0 * (2.0 * line + pitch) + line;
            let rule = line - pitch;
            assert!((heading - (rule + pitch * 7.0)).abs() < 0.01);
            assert!(
                left.top() + pitch + left_used <= left.bottom() + 0.5,
                "left board overflows on {viewport:?}"
            );
            let right = layout.boxes[1];
            // Date and its rule, six sky lines, a gap, and five forecast days.
            let right_used = heading + 6.0 * line + pitch + 5.0 * line;
            assert!(
                right.top() + pitch + right_used <= right.bottom() + 0.5,
                "right board overflows on {viewport:?}"
            );
        }
    }
}
