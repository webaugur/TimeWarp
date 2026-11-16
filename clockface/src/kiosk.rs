//! Full kiosk: mural wedges, aperture discs, brass dial, info boxes, status bar.
//!
//! Paint order is back to front. Hands, numerals, ticks, and the brass ring are drawn after
//! the discs they overlap. Every region is placed from the clock center and the face radius,
//! so moving or resizing the face moves the complications with it.
//!
//! The corners outside the chrome ring stay the black screen. The ring itself is
//! a full circle and is not clipped to the dial. The sun and moon quarters above
//! the 9–3 line are a separate layer and still fill the upper half.
//!
//! On the kiosk loop the plate is a texture. A background thread rasterizes the
//! layers that changed (dial, numerals, ticks, landscape, caption, gauges, each
//! board, the clock row, status). The UI thread uploads that rectangle and
//! draws the hands every frame, so a raster never stops the second hand.
//! `CLOCKFACE_FRAME_LOG=1` prints the UI time, the layers, the worker raster,
//! and the upload.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant};

use chrono::{DateTime, Datelike, Local, Timelike};

use egui::{Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Vec2};

use crate::events::{self, CalendarEvent};
use crate::layout::{KioskLayout, APERTURE_R};
use crate::matrix;
use crate::plate::{Layer, PxRect};
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
    /// Chart E rows from `timewarp cycle`. None uses the built-in Lewis table.
    pub cycles: Option<crate::cycles::CycleTable>,
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
            cycles: None,
        }
    }
}

pub fn paint(painter: &Painter, screen: Rect, center: Pos2, diameter: f32, model: &KioskModel) {
    paint_inner(painter, screen, center, diameter, model, false);
}

/// Static plate: everything except the hands. The boards keep the clock line,
/// which no longer includes seconds.
pub(crate) fn paint_static(
    painter: &Painter,
    screen: Rect,
    center: Pos2,
    diameter: f32,
    model: &KioskModel,
) {
    paint_masked(painter, screen, center, diameter, model, true, None);
}

/// Boards and the status bar, not the face.
///
/// Both boards and the status bar are painted, even when the caller will keep
/// only one rectangle. A later board's shadow falls on the earlier panel, and
/// leaving that shadow out makes the edge a few shades too light.
pub(crate) fn paint_light(
    painter: &Painter,
    screen: Rect,
    center: Pos2,
    diameter: f32,
    model: &KioskModel,
    _layers: u16,
) {
    let layout = KioskLayout::new(screen, center, diameter);
    paint_boxes(painter, &layout, model);
    paint_status(painter, &layout, model);
}

/// The clock heading and the board behind it, plus the later shadows that
/// cross that row on the full plate.
pub(crate) fn paint_clock_strip(
    painter: &Painter,
    screen: Rect,
    center: Pos2,
    diameter: f32,
    model: &KioskModel,
) {
    paint_light(painter, screen, center, diameter, model, 0);
}

fn paint_inner(
    painter: &Painter,
    screen: Rect,
    center: Pos2,
    diameter: f32,
    model: &KioskModel,
    omit_hands: bool,
) {
    paint_masked(painter, screen, center, diameter, model, omit_hands, None);
}

fn paint_masked(
    painter: &Painter,
    screen: Rect,
    center: Pos2,
    diameter: f32,
    model: &KioskModel,
    omit_hands: bool,
    only: Option<Layer>,
) {
    let show = |layer: Layer| only.map(|only| only == layer).unwrap_or(true);
    if only.is_none() {
        painter.rect_filled(screen, 0.0, Color32::BLACK);
    }
    let layout = KioskLayout::new(screen, center, diameter);
    let radius = layout.radius;
    let aperture = APERTURE_R * radius;

    if show(Layer::Sky) {
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
    }
    if show(Layer::Caption) {
        paint_weather_caption(painter, center, radius, &model.weather);
    }
    if show(Layer::Dial) {
        // Under the dial, so the chrome meets the cream and the boards cover the rest.
        crate::gauges::paint_trough(painter, center, radius, &model.weather);
        roman::paint_brass(painter, center, radius, aperture);
        roman::paint_aperture_shadow(painter, center, aperture);
        roman::paint_inner_ring(painter, center, radius, &roman::RomanClockStyle::default());
    }
    let style = roman::RomanClockStyle::default();
    if show(Layer::Ticks) {
        roman::paint_ticks(
            painter,
            center,
            radius,
            &style,
            model.now,
            model.cycles.as_ref(),
        );
    }
    if show(Layer::Numerals) {
        roman::paint_numerals(painter, center, radius, &style);
    }
    if only.is_none() && !omit_hands {
        roman::paint_hands(
            painter,
            center,
            diameter,
            model.now,
            &style,
            model.weather.wind_hand(),
        );
    }
    if show(Layer::Gauges) {
        crate::gauges::paint(
            painter,
            center,
            radius,
            model.now,
            &model.weather,
            model.barometer_hours,
        );
    }
    if only.is_none() {
        paint_boxes(painter, &layout, model);
    } else {
        paint_board_layer(painter, &layout, model, only.unwrap());
    }
    if show(Layer::Status) {
        paint_status(painter, &layout, model);
    }
    if only.is_none() {
        crate::leds::paint_debug(painter, screen);
    }
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
    // condition line is 42% of that. The pair sits high enough that the longest
    // condition phrase clears the inward tips of the 5-minute ticks near VI.
    let style = roman::RomanClockStyle::default();
    let tick_tip = style.hour_tick.distance - style.hour_tick.inward;
    let mut temp_size = style.numeral_size * radius;
    let (temp_at, cond_at, cond_size) = loop {
        let cond_size = temp_size * 0.42;
        let (_, th) = caption_extent(&painter, &temp, temp_size);
        let (cw, ch) = caption_extent(&painter, condition, cond_size);
        let (tw, _) = caption_extent(&painter, &temp, temp_size);
        let wide = widest_condition(&painter, cond_size).max(cw);
        let gap = 0.018 * radius;
        let sep = th * 0.5 + gap + ch * 0.5;
        let halo = 0.014 * radius;
        let hub = 0.12 * radius;
        let natural_temp_y = center.y + hub + th * 0.5 + halo;
        let natural_cond_y = natural_temp_y + sep;
        let half_w = wide * 0.5 + halo;
        let tick_r = (tick_tip - 0.018) * radius;
        let max_down = if half_w < tick_r {
            (tick_r * tick_r - half_w * half_w).sqrt()
        } else {
            0.0
        };
        let natural_bottom = natural_cond_y + ch * 0.5 + halo;
        let lift = (natural_bottom - (center.y + max_down)).max(0.0);
        let temp_y = natural_temp_y - lift;
        let cond_y = natural_cond_y - lift;
        let top = temp_y - th * 0.5 - halo;
        let bottom = cond_y + ch * 0.5 + halo;
        let inside = top >= center.y + 0.045 * radius
            && bottom <= center.y + aperture - 0.012 * radius
            && caption_in_circle(center, aperture, temp_y, tw, th, halo)
            && caption_in_circle(center, aperture, cond_y, wide, ch, halo);
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

fn widest_condition(painter: &Painter, size: f32) -> f32 {
    // Every phrase `condition_name` can return. The layout clears the longest.
    const CODES: &[i32] = &[0, 1, 2, 3, 45, 51, 56, 61, 66, 71, 73, 77, 80, 85, 95, 96];
    CODES
        .iter()
        .map(|code| caption_extent(painter, crate::weather::condition_name(*code), size).0)
        .fold(0.0_f32, f32::max)
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
    // One galley, stamped at each halo offset. `painter.text` would lay the
    // same line out again for every stamp.
    let galley = painter.layout_no_wrap(text.to_owned(), font, Color32::PLACEHOLDER);
    let blur = 0.012 * radius;
    let drop = Vec2::new(0.15 * blur, 0.55 * blur);
    let shadow = Color32::from_rgba_unmultiplied(0, 0, 0, 153);
    let scale = (shadow.a() as f32 / 255.0) / 0.6;
    let [red, green, blue, _] = shadow.to_array();
    let place = |at: Pos2| Align2::CENTER_CENTER.anchor_size(at, galley.size()).min;
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
            painter.galley_with_override_text_color(
                place(pos + drop + offset),
                galley.clone(),
                layer,
            );
        }
    }
    painter.galley_with_override_text_color(place(pos), galley, color);
}

fn paint_boxes(painter: &Painter, layout: &KioskLayout, model: &KioskModel) {
    let pitch = matrix::frame_pitch(layout.radius);
    for (index, panel) in layout.boxes.iter().enumerate() {
        matrix::paint_board(painter, *panel, pitch);
        let origin = Pos2::new(panel.left() + pitch, panel.top() + pitch);
        match index {
            0 => paint_calendar(painter, *panel, origin, pitch, model, true),
            _ => paint_forecast(painter, *panel, origin, pitch, model),
        }
    }
}

/// One board layer. The clock heading is its own layer so a minute or a beat
/// does not redraw the events under it.
fn paint_board_layer(painter: &Painter, layout: &KioskLayout, model: &KioskModel, layer: Layer) {
    let pitch = matrix::frame_pitch(layout.radius);
    match layer {
        Layer::BoardLeft => {
            let panel = layout.boxes[0];
            matrix::paint_board(painter, panel, pitch);
            let origin = Pos2::new(panel.left() + pitch, panel.top() + pitch);
            paint_calendar(painter, panel, origin, pitch, model, false);
        }
        Layer::ClockRow => {
            let panel = layout.boxes[0];
            let origin = Pos2::new(panel.left() + pitch, panel.top() + pitch);
            if line_fits(origin.y, pitch, panel) {
                let clock = board_clock_line(&model.now, model.cycles.as_ref());
                paint_heading(painter, panel, origin, pitch, &clock);
            }
        }
        Layer::BoardRight => {
            let panel = layout.boxes[1];
            matrix::paint_board(painter, panel, pitch);
            let origin = Pos2::new(panel.left() + pitch, panel.top() + pitch);
            paint_forecast(painter, panel, origin, pitch, model);
        }
        _ => {}
    }
}

fn paint_calendar(
    painter: &Painter,
    panel: Rect,
    origin: Pos2,
    pitch: f32,
    model: &KioskModel,
    include_clock: bool,
) {
    let mut y = origin.y;
    if line_fits(y, pitch, panel) {
        if include_clock {
            let clock = board_clock_line(&model.now, model.cycles.as_ref());
            y = paint_heading(painter, panel, origin, pitch, &clock);
        } else {
            // The heading layer owns this strip. Events start where it ends.
            y = origin.y + pitch * 15.0;
        }
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
            events::event_mark(&event.title, &event.time, &event.date),
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
    sky_lines.push(format!("\u{1F319} {}", model.sky.moon_name));
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
fn board_hm(now: &DateTime<Local>) -> String {
    format!("{:02}:{:02}", now.hour(), now.minute())
}

/// Left-board clock: `HH:MM @BBB COLOR`, for example `10:08 @673 RED`.
///
/// No seconds. The beat is TimeWarp's Swatch Internet Time (BMT, 86.4 s), so
/// the line changes when the minute changes or when the beat changes, not on
/// every tick. The second hand is the only per-second motion. The color name
/// comes from TimeWarp's Chart E rows when they are loaded, else the built-in
/// Lewis table.
fn board_clock_line(now: &DateTime<Local>, cycles: Option<&crate::cycles::CycleTable>) -> String {
    let name = cycles
        .and_then(|table| table.color_name_at(*now))
        .unwrap_or_else(|| crate::cycles::period_color_name(*now).to_owned());
    format!(
        "\u{1F55B} {} {} {}",
        board_hm(now),
        crate::swatch::format_swatch(now),
        name.to_ascii_uppercase()
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

/// Pixel bounds of one layer in a framebuffer whose origin is the top-left.
pub(crate) fn layer_px(
    layer: Layer,
    width: u32,
    height: u32,
    center: Pos2,
    diameter: f32,
) -> PxRect {
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(width as f32, height as f32));
    let layout = KioskLayout::new(screen, center, diameter);
    let radius = layout.radius;
    let outer = radius * (1.0 + crate::gauges::RING_OVERHANG) + 4.0;
    let aperture = APERTURE_R * radius + 4.0;
    let rect = match layer {
        Layer::Sky => Rect::from_center_size(center, Vec2::splat(aperture * 2.0)),
        Layer::Caption => Rect::from_min_max(
            Pos2::new(center.x - aperture, center.y - 0.06 * radius),
            Pos2::new(center.x + aperture, center.y + aperture),
        ),
        Layer::Dial | Layer::Ticks | Layer::Numerals | Layer::Gauges => {
            Rect::from_center_size(center, Vec2::splat(outer * 2.0))
        }
        Layer::BoardLeft => layout.boxes[0].expand(16.0),
        Layer::ClockRow => {
            let pitch = matrix::frame_pitch(radius);
            let panel = layout.boxes[0];
            Rect::from_min_max(
                Pos2::new(panel.left(), panel.top()),
                Pos2::new(panel.right(), panel.top() + pitch * 16.0 + 2.0),
            )
        }
        Layer::BoardRight => layout.boxes[1].expand(16.0),
        Layer::Status => layout.status.expand(16.0),
    };
    PxRect::from_egui(rect, width, height)
}

/// The panel itself, without the shadow outside it.
pub(crate) fn panel_px(
    layer: Layer,
    width: u32,
    height: u32,
    center: Pos2,
    diameter: f32,
) -> PxRect {
    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(width as f32, height as f32));
    let layout = KioskLayout::new(screen, center, diameter);
    let rect = match layer {
        Layer::BoardLeft => layout.boxes[0],
        Layer::BoardRight => layout.boxes[1],
        Layer::Status => layout.status,
        _ => return layer_px(layer, width, height, center, diameter),
    };
    PxRect::from_egui(rect, width, height)
}

/// One hash per layer. Seconds are not part of any of them.
pub(crate) fn layer_keys(
    width: u32,
    height: u32,
    center: Pos2,
    diameter: f32,
    model: &KioskModel,
) -> [u64; crate::plate::LAYER_COUNT] {
    let mut keys = [0u64; crate::plate::LAYER_COUNT];
    let geom = hash_with(|hasher| {
        width.hash(hasher);
        height.hash(hasher);
        center.x.to_bits().hash(hasher);
        center.y.to_bits().hash(hasher);
        diameter.to_bits().hash(hasher);
    });
    let trough = model.weather.temperature.is_finite()
        || model.weather.pressure_hpa.is_some_and(|p| p.is_finite());
    keys[Layer::Dial.index()] = hash_with(|hasher| {
        geom.hash(hasher);
        trough.hash(hasher);
    });
    keys[Layer::Numerals.index()] = geom;
    keys[Layer::Ticks.index()] = hash_with(|hasher| {
        geom.hash(hasher);
        for step in 0..60 {
            let color = crate::cycles::minute_tick_color_at(model.now, step, model.cycles.as_ref());
            color.to_array().hash(hasher);
        }
    });
    keys[Layer::Sky.index()] = hash_with(|hasher| {
        geom.hash(hasher);
        scene_id(model.weather.scene()).hash(hasher);
        model.weather.cloud_cover().hash(hasher);
        model.weather.night.hash(hasher);
        model.sky.season.hash(hasher);
        sky_band_id(model.now).hash(hasher);
        ((model.sky.moon_phase * 1000.0).round() as i32).hash(hasher);
        model.sky.sunrise_unix.map(|v| v.to_bits()).hash(hasher);
        model.sky.sunset_unix.map(|v| v.to_bits()).hash(hasher);
        model
            .sun
            .as_ref()
            .map(|image| std::sync::Arc::as_ptr(image) as usize)
            .hash(hasher);
    });
    keys[Layer::Caption.index()] = hash_with(|hasher| {
        geom.hash(hasher);
        model.weather.temperature_text().hash(hasher);
        model.weather.condition.hash(hasher);
    });
    keys[Layer::Gauges.index()] = hash_with(|hasher| {
        geom.hash(hasher);
        model.weather.temperature.to_bits().hash(hasher);
        model
            .weather
            .apparent_temperature
            .map(f32::to_bits)
            .hash(hasher);
        model.weather.dew_point.map(f32::to_bits).hash(hasher);
        model.weather.pressure_hpa.map(f32::to_bits).hash(hasher);
        match model.weather.units {
            crate::settings::Units::Imperial => 0u8,
            crate::settings::Units::Metric => 1u8,
        }
        .hash(hasher);
        model.barometer_hours.hash(hasher);
        model.now.timestamp().div_euclid(1800).hash(hasher);
        model.now.date_naive().hash(hasher);
        for day in &model.weather.daily {
            day.date.hash(hasher);
            day.high.to_bits().hash(hasher);
            day.low.to_bits().hash(hasher);
        }
        for pressure in &model.weather.hourly_pressure_hpa {
            pressure.to_bits().hash(hasher);
        }
    });
    keys[Layer::BoardLeft.index()] = hash_with(|hasher| {
        geom.hash(hasher);
        board_date(model.now).hash(hasher);
        for event in &model.events {
            event.date.hash(hasher);
            event.title.hash(hasher);
            event.time.hash(hasher);
        }
    });
    keys[Layer::ClockRow.index()] = hash_with(|hasher| {
        geom.hash(hasher);
        board_clock_line(&model.now, model.cycles.as_ref()).hash(hasher);
    });
    keys[Layer::BoardRight.index()] = hash_with(|hasher| {
        geom.hash(hasher);
        board_date(model.now).hash(hasher);
        model.sky.season.hash(hasher);
        model.sky.moon_name.hash(hasher);
        model.sky.sunrise_unix.map(|v| v.to_bits()).hash(hasher);
        model.sky.sunset_unix.map(|v| v.to_bits()).hash(hasher);
        model.sky.moonrise_unix.map(|v| v.to_bits()).hash(hasher);
        model.sky.moonset_unix.map(|v| v.to_bits()).hash(hasher);
        for day in &model.weather.daily {
            day.date.hash(hasher);
            day.code.hash(hasher);
            day.high.to_bits().hash(hasher);
            day.low.to_bits().hash(hasher);
        }
    });
    keys[Layer::Status.index()] = hash_with(|hasher| {
        geom.hash(hasher);
        model.status.hostname.hash(hasher);
        model.status.address.hash(hasher);
        model.status.warning.hash(hasher);
        model.status.alert.hash(hasher);
    });
    keys
}

fn hash_with(fill: impl FnOnce(&mut DefaultHasher)) -> u64 {
    let mut hasher = DefaultHasher::new();
    fill(&mut hasher);
    hasher.finish()
}

fn scene_id(scene: crate::weather::Scene) -> u8 {
    use crate::weather::Scene::*;
    match scene {
        ClearDay => 0,
        ClearNight => 1,
        Cloud => 2,
        Rain => 3,
        Snow => 4,
        Storm => 5,
        Fog => 6,
        Unknown => 7,
    }
}

fn sky_band_id(now: DateTime<Local>) -> u8 {
    match scene::sky_band(now) {
        scene::SkyBand::Night => 0,
        scene::SkyBand::Dawn => 1,
        scene::SkyBand::Day => 2,
        scene::SkyBand::Dusk => 3,
    }
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
    let next = matrix::paint_text_nowrap(painter, panel, origin, pitch, text);
    let rule_y = next - pitch;
    matrix::paint_rule(painter, panel, rule_y, pitch);
    rule_y + pitch * 7.0
}

/// Draw the plate, then the hands.
///
/// The plate is whatever the worker has finished. This pass never rasterizes
/// it: a minute change, a beat, or a weather arrival only records dirty
/// layers. eboard clears the offscreen target and presents after this pass
/// returns, so the hands are in the same pass as the plate.
pub fn paint_live(
    painter: &Painter,
    screen: Rect,
    center: Pos2,
    diameter: f32,
    model: &KioskModel,
) {
    let started = Instant::now();
    let pixels_w = screen.width().round().clamp(1.0, 4096.0) as u32;
    let pixels_h = screen.height().round().clamp(1.0, 4096.0) as u32;
    let local_center = Pos2::new(center.x - screen.min.x, center.y - screen.min.y);
    let (texture, note) =
        crate::plate::present_painter(painter, pixels_w, pixels_h, local_center, diameter, model);
    if let Some(texture) = texture {
        painter.image(
            texture,
            screen,
            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }
    // The strip is a few rects and changes with the chime thread. It stays
    // out of the plate so a chase is not frozen until the next raster.
    crate::leds::paint_debug(painter, screen);
    roman::paint_hands(
        painter,
        center,
        diameter,
        model.now,
        &roman::RomanClockStyle::default(),
        model.weather.wind_hand(),
    );
    if let Some(after) = note.wake {
        painter.ctx().request_repaint_after(after);
    }
    log_frame(started.elapsed(), &note, pixels_w, pixels_h);
}

fn log_frame(elapsed: Duration, note: &crate::plate::FrameNote, width: u32, height: u32) {
    let enabled = std::env::var("CLOCKFACE_FRAME_LOG")
        .map(|value| matches!(value.as_str(), "1" | "true" | "TRUE" | "yes" | "YES"))
        .unwrap_or(false);
    if !enabled {
        return;
    }
    let cpu_ms = elapsed.as_secs_f64() * 1000.0;
    // 60 Hz page flip is 16.7 ms. The spring asks for a frame every 16 ms
    // during the 250 ms after each tick. `raster` is the worker; `upload` is
    // this thread's sub-texture copy.
    eprintln!(
        "clockface frame {cpu_ms:.1}ms plate={} raster={:.1}ms upload={:.1}ms layers={} {width}x{height} budget=16.7ms",
        note.kind, note.raster_ms, note.upload_ms, note.layers
    );
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
        assert_eq!(board_hm(&now), "10:08");
        assert_eq!(board_date(now), "THU JAN 15 2026");
        assert!(
            !board_clock_line(&now, None).contains(":37"),
            "seconds stay off the board"
        );
        assert_eq!(
            board_clock_line(&now, None),
            format!("\u{1F55B} 10:08 {} RED", crate::swatch::format_swatch(&now))
        );
        let sunday = match Local.with_ymd_and_hms(2026, 11, 15, 9, 5, 0) {
            chrono::LocalResult::Single(time) => time,
            chrono::LocalResult::Ambiguous(earliest, _) => earliest,
            chrono::LocalResult::None => panic!("2026-11-15 09:05 is not a local time"),
        };
        assert_eq!(board_hm(&sunday), "09:05");
        assert_eq!(board_date(sunday), "SUN NOV 15 2026");
    }

    #[test]
    fn the_board_line_ignores_seconds_and_follows_the_beat() {
        use chrono::{TimeZone, Utc};
        let local = |h, m, s| {
            Utc.with_ymd_and_hms(2026, 7, 4, h, m, s)
                .unwrap()
                .with_timezone(&Local)
        };
        // 11:00:00 UTC and 11:00:30 UTC are both @500. 11:00:50 UTC is @501.
        let start = local(11, 0, 0);
        let still = local(11, 0, 30);
        let next = local(11, 0, 50);
        assert_eq!(
            board_clock_line(&start, None),
            board_clock_line(&still, None)
        );
        assert_ne!(
            board_clock_line(&start, None),
            board_clock_line(&next, None)
        );
        assert!(board_clock_line(&next, None).contains("@501"));
        let model = |now| KioskModel::for_screenshot(now, &crate::settings::Place::default());
        let keys = |now| layer_keys(640, 800, Pos2::new(320.0, 400.0), 400.0, &model(now));
        let start_keys = keys(start);
        let still_keys = keys(still);
        let next_keys = keys(next);
        assert_eq!(start_keys, still_keys, "seconds do not dirty a layer");
        assert_ne!(
            start_keys[Layer::ClockRow.index()],
            next_keys[Layer::ClockRow.index()],
            "a new beat dirties the clock row"
        );
        assert_eq!(
            start_keys[Layer::BoardLeft.index()],
            next_keys[Layer::BoardLeft.index()]
        );
        assert_eq!(
            start_keys[Layer::Dial.index()],
            next_keys[Layer::Dial.index()]
        );
        assert_eq!(
            start_keys[Layer::Ticks.index()],
            next_keys[Layer::Ticks.index()]
        );
        let minute = local(11, 1, 0);
        let minute_keys = keys(minute);
        assert_ne!(
            start_keys[Layer::ClockRow.index()],
            minute_keys[Layer::ClockRow.index()]
        );
        assert_eq!(
            start_keys[Layer::Sky.index()],
            minute_keys[Layer::Sky.index()]
        );
        assert_eq!(
            start_keys[Layer::Gauges.index()],
            minute_keys[Layer::Gauges.index()]
        );
        let moved = layer_keys(640, 800, Pos2::new(300.0, 400.0), 400.0, &model(start));
        assert_eq!(
            crate::plate::dirty_mask(&start_keys, &moved),
            (1u16 << crate::plate::LAYER_COUNT) - 1,
            "a move dirties every layer"
        );
    }

    #[test]
    fn a_cached_frame_redraws_the_hands_and_not_the_boards() {
        use std::time::Instant;
        let ctx = egui::Context::default();
        crate::fonts::install_fonts(&ctx);
        let width = 480.0;
        let height = 640.0;
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(width, height));
        let (center, diameter) = Settings::default().placement(screen.size());
        let now = screenshot_time();
        let model = KioskModel::for_screenshot(now, &crate::settings::Place::default());
        let mut input = egui::RawInput::default();
        input.screen_rect = Some(screen);
        input.predicted_dt = 1.0 / 60.0;
        let pass = |ctx: &egui::Context, input: egui::RawInput| {
            let started = Instant::now();
            let mut output = ctx.run_pass(input, |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::background());
                paint_live(&painter, screen, center, diameter, &model);
            });
            let cpu = started.elapsed();
            let primitives = ctx.tessellate(output.shapes.clone(), 1.0);
            let verts: usize = primitives
                .iter()
                .map(|clipped| match &clipped.primitive {
                    egui::epaint::Primitive::Mesh(mesh) => mesh.vertices.len(),
                    _ => 0,
                })
                .sum();
            let red = primitives.iter().any(|clipped| {
                let egui::epaint::Primitive::Mesh(mesh) = &clipped.primitive else {
                    return false;
                };
                mesh.vertices.iter().any(|vertex| {
                    let [r, g, b, _] = vertex.color.to_array();
                    r > 250 && g < 8 && b < 8
                })
            });
            output.textures_delta.clear();
            (cpu, verts, red)
        };
        let (first, first_verts, first_red) = pass(&ctx, input.clone());
        let (second, second_verts, second_red) = pass(&ctx, input);
        eprintln!(
            "live frame rebuild {:.1} ms ({} verts), cached {:.1} ms ({} verts)",
            first.as_secs_f64() * 1000.0,
            first_verts,
            second.as_secs_f64() * 1000.0,
            second_verts
        );
        assert!(
            first_red && second_red,
            "the second hand is in the presented frame"
        );
        assert!(
            first_verts < 20_000 && second_verts < 20_000,
            "a live frame tessellated the plate ({first_verts}, {second_verts})"
        );
        assert!(
            first.as_secs_f64() * 1000.0 < 400.0,
            "the UI thread rasterized the plate ({:.1} ms)",
            first.as_secs_f64() * 1000.0
        );
        assert!(
            second.as_secs_f64() * 1000.0 < 50.0,
            "cached frame took {:.1} ms",
            second.as_secs_f64() * 1000.0
        );
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
    fn a_clock_row_and_a_board_match_the_full_plate() {
        let width = 320u32;
        let height = 400u32;
        let settings = Settings::default();
        let viewport = Vec2::new(width as f32, height as f32);
        let (center, diameter) = settings.placement(viewport);
        let model = KioskModel::for_screenshot(screenshot_time(), &settings.place);
        let full = crate::raster::render_static(width, height, center, diameter, &model).unwrap();
        let mut session = crate::raster::SoftwareRaster::new();
        let strip = session
            .paint_clock_strip(width, height, center, diameter, &model)
            .unwrap();
        assert_row(&full, width, &strip, "clock row");
        let boards = session
            .paint_light(
                width,
                height,
                center,
                diameter,
                &model,
                Layer::BoardLeft.bit() | Layer::BoardRight.bit() | Layer::Status.bit(),
            )
            .unwrap();
        assert!(!boards.is_empty());
        for patch in &boards {
            assert_row(&full, width, patch, patch.layer.name());
        }
    }

    /// One level of difference is a coverage sample where a lamp edge meets a
    /// pixel the full plate had already painted. It is not a different lamp.
    fn assert_row(full: &[u8], width: u32, patch: &crate::plate::Patch, label: &str) {
        let mut mismatch = 0usize;
        let mut first = None;
        for row in 0..patch.h {
            let y = patch.y + row;
            for col in 0..patch.w {
                let x = patch.x + col;
                let fi = ((y * width + x) * 4) as usize;
                let pi = ((row * patch.w + col) * 4) as usize;
                let a = &full[fi..fi + 4];
                let b = &patch.rgba[pi..pi + 4];
                let off = a.iter().zip(b).any(|(p, q)| p.abs_diff(*q) > 1);
                if off {
                    mismatch += 1;
                    if first.is_none() {
                        first = Some((x, y, [a[0], a[1], a[2], a[3]], [b[0], b[1], b[2], b[3]]));
                    }
                }
            }
        }
        assert_eq!(
            mismatch, 0,
            "{label} differs at {first:?} ({mismatch} pixels)"
        );
    }

    #[test]
    fn the_clock_line_fits_on_one_row_of_the_board() {
        // Longest Heindel name plus `@BBB`. No seconds, so this is the wide line.
        for viewport in [
            Vec2::new(1920.0, 1080.0),
            Vec2::new(1200.0, 800.0),
            Vec2::new(1024.0, 1280.0),
        ] {
            assert!(
                clock_line_fits("\u{1F55B} 22:23 @473 ORANGE", viewport),
                "clock line wraps on {viewport:?}"
            );
        }
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
