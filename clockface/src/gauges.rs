//! Curved thermometer and barometer in one chrome trough under the dial.
//!
//! The trough is a half circle from 9 o'clock through 6 to 3, concentric with
//! the dial. Its inner lip tucks under the cream and its outer lip is a
//! constant radius, so the dial and the boards clip it. The floor glows like
//! a lamp behind frosted glass. Each vial floats in that channel: temperature
//! in °F on the left, sea-level pressure in inches of mercury on the right.
//! Numerals and tick icons stay inside the rim, on the outer side of the tube.

use chrono::{DateTime, Duration, Local, NaiveDateTime};
use egui::epaint::{Vertex, WHITE_UV};
use egui::{Align2, Color32, FontFamily, FontId, Mesh, Painter, Pos2, Rect, Shape, Stroke, Vec2};

use crate::roman::direction;
use crate::settings::Units;
use crate::weather::WeatherReport;

/// Standard atmosphere: 1013.25 hPa = 29.921255 inHg.
pub const HPA_TO_INHG: f64 = 0.029_529_983_071_445;

const RED: Color32 = Color32::from_rgb(0xe0, 0x00, 0x00);
const GREEN: Color32 = Color32::from_rgb(0x18, 0xc0, 0x00);
const YELLOW: Color32 = Color32::from_rgb(0xff, 0xd0, 0x00);
const EDGE: Color32 = Color32::from_rgb(0x22, 0x22, 0x22);

/// Glass highlight, inner edge to outer edge.
const GLASS: &[Color32] = &[
    Color32::from_rgb(0x8c, 0x8c, 0x8c),
    Color32::from_rgb(0xcc, 0xcc, 0xcc),
    Color32::from_rgb(0x4c, 0x4c, 0x4c),
    Color32::from_rgb(0xe8, 0xe8, 0xe8),
    Color32::from_rgb(0x7f, 0x7f, 0x7f),
];
/// Alcohol-thermometer red, dark at the glass edges and a narrow shine down the bore.
const FLUID: &[Color32] = &[
    Color32::from_rgb(0x8a, 0x06, 0x06),
    Color32::from_rgb(0xc4, 0x10, 0x10),
    Color32::from_rgb(0xff, 0x86, 0x86),
    Color32::from_rgb(0xd0, 0x14, 0x14),
    Color32::from_rgb(0x74, 0x05, 0x05),
];

/// Trough, in face radii. The inner lip starts under the dial (the disc ends at 1.0)
/// so the chrome meets the cream with no sky between them. The outer lip is a
/// constant radius: a half circle from 9 o'clock through 6 to 3, clipped by the
/// dial and the boards rather than shrunk to the wedges.
const TROUGH_IN: f32 = 0.990;
const TROUGH_OUT: f32 = 1.248;
const INNER_LIP: f32 = 0.024;
const OUTER_LIP: f32 = 0.012;
const TUBE_IN: f32 = 1.042;
const TUBE_OUT: f32 = 1.074;
const FLUID_IN: f32 = 1.050;
const FLUID_OUT: f32 = 1.066;
/// Numerals sit just outside the tube, still inside the outer chrome.
const LABEL_R: f32 = 1.150;
/// Glyphs must stay inside the dial's left and right edges, and above the boards.
const DIAL_LIMIT: f32 = 0.988;
const BOARD_LIMIT: f32 = 1.020;
/// Glass past the low tick, before the bulb, and empty glass past the high tick.
/// Degrees. Long enough to use the dial-width clip and the space above the boards.
const TUBE_BEFORE_BULB: f32 = 12.5;
const TUBE_PAST_CAP: f32 = 12.5;
/// Outer glass radius of the bulb, in tube half-widths. 1.95 is 78% of the
/// previous 2.5. The red reservoir sits inside that glass.
const BULB_RADIUS: f32 = 1.95;
/// Glass wall around the red, in the same half-width units.
const BULB_WALL: f32 = 0.50;
/// 9 o'clock, sweeping through 6 to 3 o'clock.
const TROUGH_A0: f32 = 270.0;
const TROUGH_A1: f32 = 90.0;

/// Left wedge, bulb toward 6, values increasing toward 9.
/// The span stops short of 9 and of the boards so the end numerals, the ticks,
/// and the bulb stay inside the dial-width clip.
const LEFT_START: f32 = 216.0;
const LEFT_END: f32 = 234.0;
/// Right wedge, mirrored: bulb toward 6, values increasing toward 3.
const RIGHT_START: f32 = 144.0;
const RIGHT_END: f32 = 126.0;

struct ArcSpec {
    start: f32,
    end: f32,
    unit: &'static str,
    /// Minor tick step, major tick step.
    minor: f64,
    major: f64,
    decimals: usize,
}

pub fn hpa_to_inhg(hpa: f64) -> f64 {
    hpa * HPA_TO_INHG
}

/// Map `value` along the scale onto the arc. `start_deg` is the low end.
pub fn value_to_angle(value: f64, lo: f64, hi: f64, start_deg: f64, end_deg: f64) -> f64 {
    let span = hi - lo;
    let t = if span.abs() < 1e-9 {
        0.0
    } else {
        ((value - lo) / span).clamp(0.0, 1.0)
    };
    start_deg + t * (end_deg - start_deg)
}

/// 40°F window snapped outward to 10°F, widened when the readings need it.
/// Every finite reading falls inside the result.
pub fn thermometer_scale(values: &[f32]) -> Option<(f32, f32)> {
    let (lo, hi) = padded_span(finite(values), 40.0, 10.0)?;
    Some((lo as f32, hi as f32))
}

/// About 1.0–1.5 inHg, snapped to 0.1, kept near 28.5–31.0.
/// A wider cluster is expanded so every reading stays on the scale.
pub fn barometer_scale(values: &[f64]) -> Option<(f64, f64)> {
    let vals = finite64(values);
    if vals.is_empty() {
        return None;
    }
    let min_v = vals.iter().copied().fold(f64::INFINITY, f64::min);
    let max_v = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let data = (max_v - min_v).max(0.0);
    let span = if data <= 1.5 { data.max(1.0) } else { data };
    let mid = (min_v + max_v) / 2.0;
    let pad = 0.05;
    let mut lo = snap_floor(mid - span / 2.0 - pad, 0.1);
    let mut hi = snap_ceil(mid + span / 2.0 + pad, 0.1);
    if hi - lo < 1.0 {
        lo = snap_floor(mid - 0.5, 0.1);
        hi = snap_ceil(mid + 0.5, 0.1);
    }
    // Prefer the usual sea-level window when the readings already sit in it.
    if min_v >= 28.5 && max_v <= 31.0 {
        if lo < 28.5 {
            let shift = 28.5 - lo;
            lo = 28.5;
            hi += shift;
        }
        if hi > 31.0 {
            let shift = hi - 31.0;
            hi = 31.0;
            lo -= shift;
        }
        lo = snap_floor(lo.max(28.5), 0.1).max(28.5);
        hi = snap_ceil(hi.min(31.0), 0.1).min(31.0);
        if hi - lo < 1.0 {
            hi = (lo + 1.0).min(31.0);
            lo = (hi - 1.0).max(28.5);
            lo = snap_floor(lo, 0.1).max(28.5);
            hi = snap_ceil(hi, 0.1).min(31.0);
        }
    }
    if min_v < lo {
        lo = snap_floor(min_v - 0.05, 0.1);
    }
    if max_v > hi {
        hi = snap_ceil(max_v + 0.05, 0.1);
    }
    Some((round_tenth(lo), round_tenth(hi)))
}

/// Highest `pressure_msl` whose timestamp falls on `today` (`YYYY-MM-DD`).
pub fn today_max_pressure(times: &[String], values: &[f32], today: &str) -> Option<f32> {
    let mut max: Option<f32> = None;
    for (time, value) in times.iter().zip(values.iter()) {
        if !on_date(time, today) || !value.is_finite() {
            continue;
        }
        max = Some(max.map_or(*value, |best| best.max(*value)));
    }
    max
}

/// Hourly pressure nearest to `now + hours`. Nothing is returned if the
/// nearest slot is more than 90 minutes away.
pub fn pressure_hours_ahead(
    times: &[String],
    values: &[f32],
    now: NaiveDateTime,
    hours: i64,
) -> Option<f32> {
    let target = now + Duration::hours(hours);
    let mut best: Option<(i64, f32)> = None;
    for (time, value) in times.iter().zip(values.iter()) {
        if !value.is_finite() {
            continue;
        }
        let Some(stamp) = parse_stamp(time) else {
            continue;
        };
        let delta = (stamp - target).num_seconds().abs();
        match best {
            Some((so_far, _)) if delta >= so_far => {}
            _ => best = Some((delta, *value)),
        }
    }
    best.filter(|(delta, _)| *delta <= 90 * 60)
        .map(|(_, value)| value)
}

/// One status fault when a usable report is missing a gauge field.
pub fn gauge_fault(weather: &WeatherReport) -> Option<&'static str> {
    if !weather.note.is_empty() || !weather.temperature.is_finite() {
        return None;
    }
    let high_missing = weather
        .daily
        .first()
        .map(|day| !day.high.is_finite())
        .unwrap_or(true);
    if weather.apparent_temperature.is_none()
        || weather.dew_point.is_none()
        || weather.pressure_hpa.is_none()
        || weather.hourly_time.is_empty()
        || weather.hourly_pressure_hpa.is_empty()
        || high_missing
    {
        Some("weather gauge incomplete")
    } else {
        None
    }
}

pub fn paint(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    now: DateTime<Local>,
    weather: &WeatherReport,
    barometer_hours: u32,
) {
    if radius < 8.0 {
        return;
    }
    let show_thermo = weather.temperature.is_finite();
    let show_baro = weather.pressure_hpa.is_some_and(|p| p.is_finite());
    if !show_thermo && !show_baro {
        return;
    }
    if show_thermo {
        paint_thermometer(painter, center, radius, now, weather);
    }
    if show_baro {
        paint_barometer(painter, center, radius, now, weather, barometer_hours);
    }
}

fn paint_thermometer(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    now: DateTime<Local>,
    weather: &WeatherReport,
) {
    let Some(current) = fahrenheit(weather.temperature, weather.units) else {
        return;
    };
    let high = today_high_f(weather, now);
    let dew = weather
        .dew_point
        .and_then(|value| fahrenheit(value, weather.units));
    let feels = weather
        .apparent_temperature
        .and_then(|value| fahrenheit(value, weather.units));
    let mut samples = vec![current];
    samples.extend(high);
    samples.extend(dew);
    samples.extend(feels);
    let Some((lo, hi)) = thermometer_scale(&samples) else {
        return;
    };
    let spec = ArcSpec {
        start: LEFT_START,
        end: LEFT_END,
        unit: match weather.units {
            crate::settings::Units::Imperial => "°F",
            crate::settings::Units::Metric => "°C",
        },
        minor: 5.0,
        major: 10.0,
        decimals: 0,
    };
    let clip = wedge_rect(center, radius, true);
    let painter = painter.with_clip_rect(clip);
    paint_arc(
        &painter,
        center,
        radius,
        &spec,
        lo as f64,
        hi as f64,
        current as f64,
    );
    let mut icons = Vec::new();
    if let Some(value) = high {
        icons.push(marker(
            &painter,
            center,
            radius,
            &spec,
            lo as f64,
            hi as f64,
            value as f64,
            RED,
            Mark::High,
        ));
    }
    if let Some(value) = dew {
        icons.push(marker(
            &painter,
            center,
            radius,
            &spec,
            lo as f64,
            hi as f64,
            value as f64,
            GREEN,
            Mark::Dew,
        ));
    }
    if let Some(value) = feels {
        icons.push(marker(
            &painter,
            center,
            radius,
            &spec,
            lo as f64,
            hi as f64,
            value as f64,
            YELLOW,
            Mark::Feels,
        ));
    }
    paint_marker_icons(&painter, center, radius, &icons);
}

fn paint_barometer(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    now: DateTime<Local>,
    weather: &WeatherReport,
    hours: u32,
) {
    let Some(current_hpa) = weather.pressure_hpa else {
        return;
    };
    if !current_hpa.is_finite() {
        return;
    }
    let today = now.format("%Y-%m-%d").to_string();
    let current = hpa_to_inhg(current_hpa as f64);
    let peak = today_max_pressure(&weather.hourly_time, &weather.hourly_pressure_hpa, &today)
        .map(|hpa| hpa_to_inhg(hpa as f64));
    let ahead = pressure_hours_ahead(
        &weather.hourly_time,
        &weather.hourly_pressure_hpa,
        now.naive_local(),
        hours.max(1) as i64,
    )
    .map(|hpa| hpa_to_inhg(hpa as f64));
    let mut samples = vec![current];
    samples.extend(peak);
    samples.extend(ahead);
    let Some((lo, hi)) = barometer_scale(&samples) else {
        return;
    };
    let spec = ArcSpec {
        start: RIGHT_START,
        end: RIGHT_END,
        unit: "inHg",
        minor: 0.1,
        major: 0.5,
        decimals: 1,
    };
    let clip = wedge_rect(center, radius, false);
    let painter = painter.with_clip_rect(clip);
    paint_arc(&painter, center, radius, &spec, lo, hi, current);
    let mut icons = Vec::new();
    if let Some(value) = peak {
        icons.push(marker(
            &painter,
            center,
            radius,
            &spec,
            lo,
            hi,
            value,
            RED,
            Mark::High,
        ));
    }
    if let Some(value) = ahead {
        icons.push(marker(
            &painter,
            center,
            radius,
            &spec,
            lo,
            hi,
            value,
            YELLOW,
            Mark::Forecast,
        ));
    }
    paint_marker_icons(&painter, center, radius, &icons);
}

fn paint_arc(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    spec: &ArcSpec,
    lo: f64,
    hi: f64,
    fluid: f64,
) {
    let (plate_start, plate_end) = extended(spec);
    for value in tick_values(lo, hi, spec.minor) {
        let major = on_step(value, spec.major);
        let outer = if major {
            TUBE_OUT + 0.020
        } else {
            TUBE_OUT + 0.012
        };
        let width = if major { 1.25 } else { 0.8 };
        radial_line(
            painter,
            center,
            radius,
            angle_of(spec, lo, hi, value),
            TUBE_OUT + 0.004,
            outer,
            Stroke::new(width, Color32::from_rgb(0x2a, 0x18, 0x08)),
        );
        if major {
            let label = format_tick(value, spec.decimals);
            let angle = angle_of(spec, lo, hi, value);
            let size = 0.040 * radius;
            let (half_x, half_y) = text_half(painter, &label, size);
            let pos = polar(
                center,
                radius,
                angle,
                label_face_r(angle, half_x / radius, half_y / radius),
            );
            painter.text(
                pos,
                Align2::CENTER_CENTER,
                label,
                FontId::new(size, FontFamily::Proportional),
                Color32::from_rgb(0x2a, 0x18, 0x08),
            );
        }
    }
    let (unit_at, unit_size) = unit_label_pos(painter, center, radius, spec);
    painter.text(
        unit_at,
        Align2::CENTER_CENTER,
        spec.unit,
        FontId::new(unit_size, FontFamily::Proportional),
        Color32::from_rgb(0x3a, 0x22, 0x08),
    );

    let fluid_ang = angle_of(spec, lo, hi, fluid);
    let sign = if spec.end >= spec.start { 1.0 } else { -1.0 };
    let centerline = (TUBE_IN + TUBE_OUT) * 0.5;
    // Carry the glass a little past the bulb center so the radial cut sits
    // inside the bulb instead of showing as a step.
    let tuck = sign * 1.6;
    band(
        painter,
        center,
        radius,
        plate_start - tuck,
        plate_end,
        TUBE_IN,
        TUBE_OUT,
        GLASS,
    );
    cap(
        painter,
        center,
        radius,
        plate_end,
        centerline,
        tube_px(radius),
        GLASS,
    );
    cap(
        painter,
        center,
        radius,
        plate_start,
        centerline,
        tube_px(radius) * BULB_RADIUS,
        GLASS,
    );
    band(
        painter,
        center,
        radius,
        plate_start,
        fluid_ang,
        FLUID_IN,
        FLUID_OUT,
        FLUID,
    );
    cap(
        painter,
        center,
        radius,
        fluid_ang,
        (FLUID_IN + FLUID_OUT) * 0.5,
        fluid_px(radius),
        FLUID,
    );
    fluid_bulb(painter, center, radius, plate_start, centerline);
}

/// Glass beyond the scale: a run of tube before the bulb, and empty glass
/// past the high tick up to the cap. The scale angles themselves stay put.
fn extended(spec: &ArcSpec) -> (f32, f32) {
    let sign = if spec.end >= spec.start { 1.0 } else { -1.0 };
    (
        spec.start - sign * TUBE_BEFORE_BULB,
        spec.end + sign * TUBE_PAST_CAP,
    )
}

/// What a colored tick is marking. These gauges have no minimum tick; a later
/// one would use [`triangle`] with the point down.
enum Mark {
    High,
    Dew,
    Feels,
    Forecast,
}

struct IconMark {
    angle: f32,
    color: Color32,
    kind: Mark,
    /// True when the scale runs clockwise, so the forecast arrow points that way.
    increasing_clockwise: bool,
}

fn marker(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    spec: &ArcSpec,
    lo: f64,
    hi: f64,
    value: f64,
    color: Color32,
    kind: Mark,
) -> IconMark {
    let ang = angle_of(spec, lo, hi, value);
    radial_line(
        painter,
        center,
        radius,
        ang,
        TUBE_IN - 0.006,
        TUBE_OUT + 0.008,
        Stroke::new(3.2, EDGE),
    );
    radial_line(
        painter,
        center,
        radius,
        ang,
        TUBE_IN - 0.006,
        TUBE_OUT + 0.008,
        Stroke::new(1.8, color),
    );
    let tip = polar(center, radius, ang, TUBE_OUT + 0.016);
    painter.circle_filled(tip, 3.3, EDGE);
    painter.circle_filled(tip, 2.3, color);
    IconMark {
        angle: ang,
        color,
        kind,
        increasing_clockwise: spec.end >= spec.start,
    }
}

fn icon_px(radius: f32) -> f32 {
    (0.028 * 1.4 * radius).clamp(9.5, 16.0)
}

/// Just inside the trough's outer lip, and inside the dial edge and the boards.
fn icon_face_r(angle: f32, half: f32) -> f32 {
    let mut face_r = TROUGH_OUT - OUTER_LIP - half - 0.008;
    let rad = angle.to_radians();
    let outward = rad.sin().abs();
    let down = (-rad.cos()).max(0.0);
    if outward > 1e-3 {
        face_r = face_r.min((DIAL_LIMIT - half) / outward);
    }
    if down > 1e-3 {
        face_r = face_r.min((BOARD_LIMIT - half) / down);
    }
    face_r.max(LABEL_R + half + 0.012)
}

/// Push icons apart along the arc. Each stays as close to its marker angle as
/// the gap allows; a uniform shift keeps the group from walking off one end.
fn spread_icon_angles(homes: &[f32], min_gap: f32) -> Vec<f32> {
    let mut order: Vec<usize> = (0..homes.len()).collect();
    order.sort_by(|&a, &b| homes[a].total_cmp(&homes[b]));
    let mut placed = homes.to_vec();
    if order.len() < 2 {
        return placed;
    }
    let first = order[0];
    placed[first] = homes[first];
    for pair in order.windows(2) {
        let prev = pair[0];
        let cur = pair[1];
        placed[cur] = homes[cur].max(placed[prev] + min_gap);
    }
    let last = *order.last().unwrap();
    let excess = placed[last] - homes[last];
    if excess > 0.0 {
        for slot in &mut placed {
            *slot -= excess;
        }
    }
    placed
}

fn paint_marker_icons(painter: &Painter, center: Pos2, radius: f32, icons: &[IconMark]) {
    if icons.is_empty() {
        return;
    }
    let s = icon_px(radius);
    let half = s * 0.55 / radius;
    let sample = icon_face_r(icons[0].angle, half).max(1.05);
    let gap = (s * 1.35 / (sample * radius)).to_degrees();
    let homes: Vec<f32> = icons.iter().map(|icon| icon.angle).collect();
    let placed = spread_icon_angles(&homes, gap);
    let leader = Color32::from_rgba_unmultiplied(0x2a, 0x18, 0x08, 78);
    for (icon, angle) in icons.iter().zip(placed) {
        let face_r = icon_face_r(angle, half);
        let from = polar(center, radius, icon.angle, TUBE_OUT + 0.024);
        let to = polar(center, radius, angle, face_r - half - 0.006);
        painter.line(vec![from, to], Stroke::new(0.75, leader));
        let origin = polar(center, radius, angle, face_r);
        match icon.kind {
            Mark::High => triangle(painter, origin, s, icon.color, true),
            Mark::Dew => droplet(painter, origin, s, icon.color),
            Mark::Feels => figure(painter, origin, s, icon.color),
            Mark::Forecast => ahead_arrow(
                painter,
                origin,
                s,
                icon.color,
                angle,
                icon.increasing_clockwise,
            ),
        }
    }
}

fn triangle(painter: &Painter, origin: Pos2, s: f32, color: Color32, up: bool) {
    let h = s * 0.92;
    let w = s * 0.84;
    let (y_tip, y_base) = if up {
        (-h * 0.5, h * 0.5)
    } else {
        (h * 0.5, -h * 0.5)
    };
    let pts = [
        origin + Vec2::new(0.0, y_tip),
        origin + Vec2::new(-w * 0.5, y_base),
        origin + Vec2::new(w * 0.5, y_base),
    ];
    fill_poly(painter, &pts, color);
}

fn droplet(painter: &Painter, origin: Pos2, s: f32, color: Color32) {
    let bowl = origin + Vec2::new(0.0, s * 0.08);
    let rad = s * 0.32;
    painter.circle_filled(bowl, rad + 0.9, EDGE);
    painter.circle_filled(bowl, rad, color);
    let tip = origin + Vec2::new(0.0, -s * 0.50);
    let left = bowl + Vec2::new(-rad * 0.62, -rad * 0.45);
    let right = bowl + Vec2::new(rad * 0.62, -rad * 0.45);
    fill_poly(painter, &[tip, left, right], color);
}

fn figure(painter: &Painter, origin: Pos2, s: f32, color: Color32) {
    let head = origin + Vec2::new(0.0, -s * 0.24);
    let hr = s * 0.18;
    painter.circle_stroke(head, hr, Stroke::new(2.1, EDGE));
    painter.circle_stroke(head, hr, Stroke::new(1.25, color));
    let neck = head + Vec2::new(0.0, hr + 0.4);
    let hip = origin + Vec2::new(0.0, s * 0.46);
    let arm = neck.y + s * 0.08;
    stroke(painter, neck, hip, 2.4, EDGE);
    stroke(painter, neck, hip, 1.35, color);
    stroke(
        painter,
        Pos2::new(origin.x - s * 0.36, arm),
        Pos2::new(origin.x + s * 0.36, arm),
        2.4,
        EDGE,
    );
    stroke(
        painter,
        Pos2::new(origin.x - s * 0.36, arm),
        Pos2::new(origin.x + s * 0.36, arm),
        1.35,
        color,
    );
}

fn ahead_arrow(
    painter: &Painter,
    origin: Pos2,
    s: f32,
    color: Color32,
    angle: f32,
    increasing_clockwise: bool,
) {
    let dir = ahead_tangent(angle, increasing_clockwise);
    let tail = origin - dir * (s * 0.42);
    let head = origin + dir * (s * 0.46);
    stroke(painter, tail, head, 2.5, EDGE);
    stroke(painter, tail, head, 1.45, color);
    let side = Vec2::new(-dir.y, dir.x) * (s * 0.30);
    let back = head - dir * (s * 0.40);
    fill_poly(painter, &[head, back + side, back - side], color);
}

fn ahead_tangent(angle: f32, increasing_clockwise: bool) -> Vec2 {
    let rad = angle.to_radians();
    let clockwise = Vec2::new(rad.cos(), rad.sin());
    if increasing_clockwise {
        clockwise
    } else {
        -clockwise
    }
}

fn fill_poly(painter: &Painter, pts: &[Pos2], color: Color32) {
    if pts.len() < 3 {
        return;
    }
    let n = pts.len() as f32;
    let sum = pts.iter().fold(Vec2::ZERO, |acc, p| acc + p.to_vec2());
    let center = Pos2::new(sum.x / n, sum.y / n);
    let mut mesh = Mesh::default();
    mesh.vertices.push(Vertex {
        pos: center,
        uv: WHITE_UV,
        color,
    });
    for p in pts {
        mesh.vertices.push(Vertex {
            pos: *p,
            uv: WHITE_UV,
            color,
        });
    }
    for i in 0..pts.len() {
        let next = if i + 1 == pts.len() {
            1
        } else {
            (i as u32) + 2
        };
        mesh.add_triangle(0, (i as u32) + 1, next);
    }
    painter.add(Shape::mesh(mesh));
    let mut edge = pts.to_vec();
    edge.push(pts[0]);
    painter.line(edge, Stroke::new(1.35, EDGE));
}

fn stroke(painter: &Painter, a: Pos2, b: Pos2, width: f32, color: Color32) {
    painter.line(vec![a, b], Stroke::new(width, color));
}

/// Reservoir at the tube's low end. The center is the argument: the caller
/// places it on the tube centerline at `plate_start`, so the stem meets the
/// bulb with no kink. Diameter is about 78% of the previous 2.5 tube-widths.
fn fluid_bulb(painter: &Painter, center: Pos2, radius: f32, angle: f32, at: f32) {
    let origin = polar(center, radius, angle, at);
    let px = tube_px(radius) * (BULB_RADIUS - BULB_WALL);
    let segs = 28u32;
    let mut mesh = Mesh::default();
    mesh.vertices.push(Vertex {
        pos: origin,
        uv: WHITE_UV,
        color: Color32::from_rgb(0xd8, 0x18, 0x18),
    });
    for i in 0..=segs {
        let t = i as f32 / segs as f32;
        let ang = t * std::f32::consts::TAU;
        let offset = Vec2::new(ang.cos(), ang.sin()) * px;
        let nx = offset.x / px;
        let ny = offset.y / px;
        mesh.vertices.push(Vertex {
            pos: origin + offset,
            uv: WHITE_UV,
            color: bulb_color(nx, ny),
        });
    }
    for i in 0..segs {
        mesh.add_triangle(0, i + 1, i + 2);
    }
    painter.add(Shape::mesh(mesh));
}

fn bulb_color(nx: f32, ny: f32) -> Color32 {
    let light = ((-nx - ny) * 0.5 + 0.55).clamp(0.25, 1.0);
    let body = Color32::from_rgb(
        (0xb4 as f32 * light) as u8,
        (0x0c as f32 * light) as u8,
        (0x0c as f32 * light) as u8,
    );
    let dx = nx + 0.36;
    let dy = ny + 0.42;
    let spec = (1.0 - (dx * dx + dy * dy).sqrt() / 0.42).clamp(0.0, 1.0);
    let spec = spec * spec;
    lerp_color(body, Color32::from_rgb(0xff, 0xe4, 0xe4), spec * 0.9)
}

fn band(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    a0: f32,
    a1: f32,
    r0: f32,
    r1: f32,
    stops: &[Color32],
) {
    if (a1 - a0).abs() < 0.05 || r1 <= r0 {
        return;
    }
    let steps = 40u32;
    let bands = (stops.len().saturating_sub(1).max(1)) as u32;
    let cols = bands + 1;
    let mut mesh = Mesh::default();
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let ang = a0 + (a1 - a0) * t;
        for b in 0..=bands {
            let u = b as f32 / bands as f32;
            let r = r0 + (r1 - r0) * u;
            mesh.vertices.push(Vertex {
                pos: polar(center, radius, ang, r),
                uv: WHITE_UV,
                color: sample_stops(stops, u),
            });
        }
    }
    for i in 0..steps {
        for b in 0..bands {
            let i0 = i * cols + b;
            let i1 = i0 + 1;
            let i2 = i0 + cols;
            let i3 = i2 + 1;
            mesh.add_triangle(i0, i2, i1);
            mesh.add_triangle(i1, i2, i3);
        }
    }
    painter.add(Shape::mesh(mesh));
}

fn cap(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    angle: f32,
    at: f32,
    px: f32,
    stops: &[Color32],
) {
    let origin = polar(center, radius, angle, at);
    let segs = 18u32;
    let mut mesh = Mesh::default();
    mesh.vertices.push(Vertex {
        pos: origin,
        uv: WHITE_UV,
        color: sample_stops(stops, 0.5),
    });
    for i in 0..=segs {
        let t = i as f32 / segs as f32;
        let ang = t * std::f32::consts::TAU;
        let pos = origin + Vec2::new(ang.cos(), ang.sin()) * px;
        let face = (pos - center).length() / radius.max(1.0);
        let u = ((face - TUBE_IN) / (TUBE_OUT - TUBE_IN)).clamp(0.0, 1.0);
        mesh.vertices.push(Vertex {
            pos,
            uv: WHITE_UV,
            color: sample_stops(stops, u),
        });
    }
    for i in 0..segs {
        mesh.add_triangle(0, i + 1, i + 2);
    }
    painter.add(Shape::mesh(mesh));
}

fn radial_line(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    angle: f32,
    r0: f32,
    r1: f32,
    stroke: Stroke,
) {
    painter.line(
        vec![
            polar(center, radius, angle, r0),
            polar(center, radius, angle, r1),
        ],
        stroke,
    );
}

fn polar(center: Pos2, radius: f32, degrees: f32, face_r: f32) -> Pos2 {
    center + direction(degrees) * (face_r * radius)
}

fn angle_of(spec: &ArcSpec, lo: f64, hi: f64, value: f64) -> f32 {
    value_to_angle(value, lo, hi, spec.start as f64, spec.end as f64) as f32
}

/// Unit label just past the red bulb, in the trough, clear of the scale.
///
/// The font is about 80% of the free radial gap between the tube and the
/// nearer of the trough lip, the dial edge, and the boards.
fn unit_label_pos(painter: &Painter, center: Pos2, radius: f32, spec: &ArcSpec) -> (Pos2, f32) {
    let (hx, hy) = text_half(painter, spec.unit, 100.0);
    let (angle, face_r, em) = unit_label_geometry(spec.start, spec.end, hx / 100.0, hy / 100.0);
    (polar(center, radius, angle, face_r), em * radius)
}

/// `(angle, face_r, em)` with lengths in face radii. `w_em` and `h_em` are the
/// text's half-width and half-height divided by the font size.
fn unit_label_geometry(start: f32, end: f32, w_em: f32, h_em: f32) -> (f32, f32, f32) {
    let sign = if end >= start { 1.0 } else { -1.0 };
    let centerline = (TUBE_IN + TUBE_OUT) * 0.5;
    let bulb_half = (TUBE_OUT - TUBE_IN) * 0.5 * BULB_RADIUS;
    let bulb = start - sign * TUBE_BEFORE_BULB;
    // Just up the tube from the bulb, still short of the first numeral.
    let bulb_ang = (bulb_half / centerline).atan().to_degrees();
    let mut angle = bulb + sign * (bulb_ang + 4.0);
    let before_scale = start - sign * 2.2;
    angle = if sign > 0.0 {
        angle.min(before_scale)
    } else {
        angle.max(before_scale)
    };
    let inner = TUBE_OUT + 0.014;
    let limit = radial_limit(angle);
    let gap = (limit - inner).max(0.02);
    // The opening beside the bulb is taller on the screen than the radial gap,
    // because the board edge is horizontal. Size the cap to about 80% of that
    // vertical room, and still stay inside the radial gap.
    let down_for_size = (-angle.to_radians().cos()).max(0.35);
    let vertical = ((BOARD_LIMIT - 0.016) - down_for_size * inner).max(gap);
    let cap_over_em = (2.0 * h_em).max(0.9);
    let em = (0.80 * vertical.max(gap) / cap_over_em).clamp(0.055, 0.10);
    let down = (-angle.to_radians().cos()).max(0.2);
    let bulb_down = (-bulb.to_radians().cos()) * centerline;
    // Keep the word level with the bulb. A larger radius at this low angle
    // drops the label under the glass and into the boards.
    let mut face_r = inner + gap * 0.50;
    let level = bulb_down + bulb_half * 0.25;
    if face_r * down > level {
        face_r = level / down;
    }
    face_r = face_r.clamp(inner + 0.01, (limit - 0.012).max(inner + 0.01));
    let _ = (w_em, h_em);
    (angle, face_r, em)
}

fn radial_limit(angle: f32) -> f32 {
    let rad = angle.to_radians();
    let outward = rad.sin().abs();
    let down = (-rad.cos()).max(0.0);
    let mut limit = TROUGH_OUT - OUTER_LIP - 0.008;
    if outward > 1e-3 {
        limit = limit.min((DIAL_LIMIT - 0.006) / outward);
    }
    if down > 1e-3 {
        limit = limit.min((BOARD_LIMIT - 0.006) / down);
    }
    limit
}

fn text_half(painter: &Painter, text: &str, size: f32) -> (f32, f32) {
    let galley = painter.layout(
        text.to_owned(),
        FontId::new(size, FontFamily::Proportional),
        Color32::PLACEHOLDER,
        f32::INFINITY,
    );
    (galley.size().x * 0.5 + 1.5, galley.size().y * 0.5 + 1.0)
}

/// Preferred numeral radius, pulled in when the glyph would cross the dial edge
/// or the top of the info boards.
fn label_face_r(angle: f32, half_x: f32, half_y: f32) -> f32 {
    let rad = angle.to_radians();
    let outward = rad.sin().abs();
    let down = (-rad.cos()).max(0.0);
    let mut face_r = LABEL_R;
    if outward > 1e-3 {
        face_r = face_r.min((DIAL_LIMIT - half_x) / outward);
    }
    if down > 1e-3 {
        face_r = face_r.min((BOARD_LIMIT - half_y) / down);
    }
    face_r.max(TUBE_OUT + 0.028)
}

fn glyph_inside(angle: f32, face_r: f32, half_x: f32, half_y: f32) -> bool {
    let rad = angle.to_radians();
    let x = rad.sin().abs() * face_r + half_x;
    let y = (-rad.cos()).max(0.0) * face_r + half_y;
    x <= DIAL_LIMIT + 1e-4 && y <= BOARD_LIMIT + 1e-4
}

fn wedge_rect(center: Pos2, radius: f32, left: bool) -> Rect {
    // Stop at the board line so a numeral cannot draw underneath the lamps.
    let bottom = center.y + radius * BOARD_LIMIT;
    if left {
        Rect::from_min_max(
            Pos2::new(center.x - radius, center.y),
            Pos2::new(center.x, bottom),
        )
    } else {
        Rect::from_min_max(
            Pos2::new(center.x, center.y),
            Pos2::new(center.x + radius, bottom),
        )
    }
}

/// The glowing half-circle, drawn before the dial so the cream covers the overlap.
pub fn paint_trough(painter: &Painter, center: Pos2, radius: f32, weather: &WeatherReport) {
    if radius < 8.0 {
        return;
    }
    let show =
        weather.temperature.is_finite() || weather.pressure_hpa.is_some_and(|p| p.is_finite());
    if !show {
        return;
    }
    // The half circle stays inside the dial's left and right edges. The boards
    // cover whatever continues below the face.
    let clip = Rect::from_min_max(
        Pos2::new(center.x - radius, center.y - 3.0),
        Pos2::new(center.x + radius, center.y + radius * 1.6),
    );
    let painter = painter.with_clip_rect(clip);
    let steps = 96u32;
    let bands = 10u32;
    let cols = bands + 1;
    let mut mesh = Mesh::default();
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        let deg = TROUGH_A0 + (TROUGH_A1 - TROUGH_A0) * t;
        for b in 0..=bands {
            let u = b as f32 / bands as f32;
            let r = TROUGH_IN + (TROUGH_OUT - TROUGH_IN) * u;
            mesh.vertices.push(Vertex {
                pos: polar(center, radius, deg, r),
                uv: WHITE_UV,
                color: trough_color(deg, r),
            });
        }
    }
    for i in 0..steps {
        for b in 0..bands {
            let i0 = i * cols + b;
            let i1 = i0 + 1;
            let i2 = i0 + cols;
            let i3 = i2 + 1;
            mesh.add_triangle(i0, i2, i1);
            mesh.add_triangle(i1, i2, i3);
        }
    }
    painter.add(Shape::mesh(mesh));
}

fn trough_color(deg: f32, r: f32) -> Color32 {
    if r <= TROUGH_IN + INNER_LIP {
        let u = ((r - TROUGH_IN) / INNER_LIP).clamp(0.0, 1.0);
        return chrome(u);
    }
    if r >= TROUGH_OUT - OUTER_LIP {
        let u = ((TROUGH_OUT - r) / OUTER_LIP).clamp(0.0, 1.0);
        return chrome(u);
    }
    let floor_in = TROUGH_IN + INNER_LIP;
    let floor_out = TROUGH_OUT - OUTER_LIP;
    let u = ((r - floor_in) / (floor_out - floor_in)).clamp(0.0, 1.0);
    let edge = u.min(1.0 - u);
    let shade = ((0.07 - edge) / 0.07).clamp(0.0, 1.0);
    let glow = lamp_color(deg, u);
    lerp_color(glow, Color32::from_rgb(0xa0, 0x62, 0x18), shade * 0.45)
}

fn chrome(u: f32) -> Color32 {
    const STOPS: &[Color32] = &[
        Color32::from_rgb(0x4a, 0x4a, 0x4e),
        Color32::from_rgb(0xf4, 0xf4, 0xf6),
        Color32::from_rgb(0x8c, 0x8c, 0x90),
        Color32::from_rgb(0xdc, 0xdc, 0xe0),
        Color32::from_rgb(0x32, 0x32, 0x36),
    ];
    sample_stops(STOPS, u)
}

fn lamp_color(deg: f32, u: f32) -> Color32 {
    let hot = Color32::from_rgb(0xff, 0xf3, 0xc4);
    let edge = Color32::from_rgb(0xff, 0xb3, 0x47);
    let bell = (1.0 - (u * 2.0 - 1.0).abs()).clamp(0.0, 1.0);
    let glow = lerp_color(edge, hot, bell.powf(0.5));
    let ang = ((deg - 180.0).abs() / 70.0).clamp(0.0, 1.0);
    let fall = 1.0 - ang * 0.08;
    Color32::from_rgb(
        (glow.r() as f32 * fall).round() as u8,
        (glow.g() as f32 * fall).round() as u8,
        (glow.b() as f32 * fall).round() as u8,
    )
}

fn tube_px(radius: f32) -> f32 {
    (TUBE_OUT - TUBE_IN) * 0.5 * radius
}

fn fluid_px(radius: f32) -> f32 {
    (FLUID_OUT - FLUID_IN) * 0.5 * radius
}

fn fahrenheit(value: f32, _units: Units) -> Option<f32> {
    // The glass shows the number the report already uses, °F or °C.
    value.is_finite().then_some(value)
}

fn today_high_f(weather: &WeatherReport, now: DateTime<Local>) -> Option<f32> {
    let today = now.format("%Y-%m-%d").to_string();
    let high = weather
        .daily
        .iter()
        .find(|day| day.date == today)
        .map(|day| day.high)?;
    fahrenheit(high, weather.units)
}

fn sample_stops(stops: &[Color32], t: f32) -> Color32 {
    if stops.is_empty() {
        return Color32::WHITE;
    }
    if stops.len() == 1 {
        return stops[0];
    }
    let t = t.clamp(0.0, 1.0);
    let scaled = t * (stops.len() - 1) as f32;
    let i = (scaled.floor() as usize).min(stops.len() - 2);
    let u = scaled - i as f32;
    lerp_color(stops[i], stops[i + 1], u)
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    Color32::from_rgba_unmultiplied(
        lerp_u8(a.r(), b.r(), t),
        lerp_u8(a.g(), b.g(), t),
        lerp_u8(a.b(), b.b(), t),
        lerp_u8(a.a(), b.a(), t),
    )
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

fn padded_span(vals: Vec<f64>, min_span: f64, step: f64) -> Option<(f64, f64)> {
    if vals.is_empty() {
        return None;
    }
    let min_v = vals.iter().copied().fold(f64::INFINITY, f64::min);
    let max_v = vals.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = (max_v - min_v).max(min_span);
    let mid = (min_v + max_v) / 2.0;
    let pad = step * 0.5;
    let mut lo = snap_floor(mid - span / 2.0 - pad, step);
    let mut hi = snap_ceil(mid + span / 2.0 + pad, step);
    if min_v < lo {
        lo = snap_floor(min_v - pad, step);
    }
    if max_v > hi {
        hi = snap_ceil(max_v + pad, step);
    }
    while hi - lo < min_span - 1e-6 {
        lo -= step;
        hi += step;
    }
    Some((lo, hi))
}

fn tick_values(lo: f64, hi: f64, step: f64) -> Vec<f64> {
    let mut values = Vec::new();
    let mut value = snap_ceil(lo - step * 1e-6, step);
    if value < lo - 1e-6 {
        value += step;
    }
    while value <= hi + 1e-6 {
        values.push(round_to(value, step));
        value += step;
    }
    values
}

fn on_step(value: f64, step: f64) -> bool {
    let n = value / step;
    (n - n.round()).abs() < 1e-4
}

fn format_tick(value: f64, decimals: usize) -> String {
    if decimals == 0 {
        format!("{:.0}", value)
    } else {
        format!("{value:.1}")
    }
}

fn finite(values: &[f32]) -> Vec<f64> {
    values
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .map(|v| v as f64)
        .collect()
}

fn finite64(values: &[f64]) -> Vec<f64> {
    values.iter().copied().filter(|v| v.is_finite()).collect()
}

fn snap_floor(value: f64, step: f64) -> f64 {
    (value / step + 1e-9).floor() * step
}

fn snap_ceil(value: f64, step: f64) -> f64 {
    (value / step - 1e-9).ceil() * step
}

fn round_tenth(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn round_to(value: f64, step: f64) -> f64 {
    (value / step).round() * step
}

fn on_date(stamp: &str, today: &str) -> bool {
    stamp.len() >= today.len()
        && stamp.as_bytes().get(..today.len()) == Some(today.as_bytes())
        && stamp
            .as_bytes()
            .get(today.len())
            .map(|ch| *ch == b'T' || *ch == b' ')
            .unwrap_or(false)
}

fn parse_stamp(stamp: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(stamp, "%Y-%m-%dT%H:%M")
        .or_else(|_| NaiveDateTime::parse_from_str(stamp, "%Y-%m-%dT%H:%M:%S"))
        .ok()
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    #[test]
    fn thermometer_span_is_forty_degrees_snapped_and_contains_the_marks() {
        let (lo, hi) = thermometer_scale(&[28.0, 31.0, 19.0, 22.0]).unwrap();
        assert!(hi - lo >= 40.0);
        assert_eq!(lo % 10.0, 0.0);
        assert_eq!(hi % 10.0, 0.0);
        for value in [19.0, 22.0, 28.0, 31.0] {
            assert!(lo <= value && value <= hi, "{value} outside {lo}..={hi}");
        }
        let wide = thermometer_scale(&[0.0, 55.0]).unwrap();
        assert!(wide.1 - wide.0 >= 55.0);
        assert!(wide.0 <= 0.0 && 55.0 <= wide.1);
    }

    #[test]
    fn barometer_span_snaps_to_tenths_inside_the_usual_window() {
        let values = [29.94, 30.18, 30.07];
        let (lo, hi) = barometer_scale(&values).unwrap();
        assert!(hi - lo >= 1.0 - 1e-6);
        assert!(hi - lo <= 1.5 + 1e-6);
        assert!(lo >= 28.5 - 1e-6 && hi <= 31.0 + 1e-6);
        assert!((lo * 10.0).round() / 10.0 - lo < 1e-6);
        for value in values {
            assert!(lo <= value && value <= hi, "{value} outside {lo}..={hi}");
        }
        let wide = barometer_scale(&[29.0, 30.8]).unwrap();
        assert!(wide.0 <= 29.0 && 30.8 <= wide.1);
    }

    #[test]
    fn icons_sit_by_the_trough_edge_and_separate_when_close() {
        let half = 0.024;
        for angle in [216.0, 222.0, 227.0, 234.0, 144.0, 134.0, 126.0] {
            let face_r = icon_face_r(angle, half);
            assert!(
                face_r >= LABEL_R + half,
                "icon at {angle} still sits on the numerals ({face_r})"
            );
            assert!(
                glyph_inside(angle, face_r, half, half),
                "icon at {angle} r={face_r} leaves the dial or the boards"
            );
        }
        let homes = [222.8, 223.9, 227.2];
        let placed = spread_icon_angles(&homes, 3.0);
        let mut ordered = placed.clone();
        ordered.sort_by(|a, b| a.total_cmp(b));
        for pair in ordered.windows(2) {
            assert!(pair[1] - pair[0] >= 2.99, "{ordered:?}");
        }
        assert!(
            (placed[2] - homes[2]).abs() < 0.01,
            "the free icon should stay put"
        );
        assert!((placed[0] - homes[0]).abs() < 4.0);
    }

    #[test]
    fn scale_ends_stay_inside_the_dial_and_above_the_boards() {
        // "30.5" is the wide end numeral. Half-extents are in face radii.
        let wide = (0.062, 0.026);
        let narrow = (0.030, 0.026);
        for (angle, half) in [
            (LEFT_END, wide),
            (RIGHT_END, wide),
            (LEFT_START, narrow),
            (RIGHT_START, wide),
        ] {
            let face_r = label_face_r(angle, half.0, half.1);
            assert!(
                face_r >= TUBE_OUT + 0.028,
                "label at {angle} collapsed onto the tube ({face_r})"
            );
            assert!(
                glyph_inside(angle, face_r, half.0, half.1),
                "label at {angle} r={face_r} leaves the dial or the boards"
            );
        }
        let centerline = (TUBE_IN + TUBE_OUT) * 0.5;
        let bulb_half = (TUBE_OUT - TUBE_IN) * 0.5 * BULB_RADIUS;
        let cap_half = (TUBE_OUT - TUBE_IN) * 0.5;
        for (start, end) in [(LEFT_START, LEFT_END), (RIGHT_START, RIGHT_END)] {
            let spec = ArcSpec {
                start,
                end,
                unit: "",
                minor: 1.0,
                major: 1.0,
                decimals: 0,
            };
            let (bulb, cap) = extended(&spec);
            let sign = if end >= start { 1.0 } else { -1.0 };
            assert!(
                (start - bulb) * sign > 8.0,
                "tube should run well past the low tick toward the bulb"
            );
            assert!(
                (cap - end) * sign > 8.0,
                "empty glass should run well past the high tick"
            );
            assert!(
                glyph_inside(bulb, centerline, bulb_half, bulb_half),
                "bulb at {bulb} is clipped"
            );
            assert!(
                glyph_inside(cap, centerline, cap_half, cap_half),
                "tube cap at {cap} is clipped"
            );
        }
    }

    #[test]
    fn unit_labels_sit_beside_the_bulb_in_the_free_gap() {
        let centerline = (TUBE_IN + TUBE_OUT) * 0.5;
        let bulb_half = (TUBE_OUT - TUBE_IN) * 0.5 * BULB_RADIUS;
        // Half-extents over em, roughly "°F" and the wider "inHg".
        for (start, end, w_em) in [(LEFT_START, LEFT_END, 0.70), (RIGHT_START, RIGHT_END, 1.15)] {
            let (angle, face_r, em) = unit_label_geometry(start, end, w_em, 0.55);
            let sign = if end >= start { 1.0 } else { -1.0 };
            let bulb = start - sign * TUBE_BEFORE_BULB;
            let mid = (start + end) * 0.5;
            assert!(
                (angle - bulb).abs() < (angle - mid).abs(),
                "unit at {angle} is closer to mid-scale {mid} than to the bulb {bulb}"
            );
            assert!(
                (angle - bulb) * sign > 0.0,
                "unit should sit on the scale side of the bulb, not past it into the boards"
            );
            assert!(
                face_r > TUBE_OUT + 0.01,
                "unit overlaps the tube ({face_r})"
            );
            assert!(
                (0.05..0.11).contains(&em),
                "font {em} should fill most of the pocket beside the bulb"
            );
            let bulb_down = (-bulb.to_radians().cos()) * centerline;
            let label_down = (-angle.to_radians().cos()) * face_r;
            assert!(
                label_down <= bulb_down + bulb_half + 0.02,
                "unit sits under the bulb ({label_down} > {bulb_down})"
            );
            let half_x = w_em * em;
            let half_y = 0.55 * em;
            assert!(
                glyph_inside(angle, face_r, half_x, half_y),
                "unit at {angle} r={face_r} leaves the dial or the boards"
            );
            let from_bulb = direction(bulb) * centerline - direction(angle) * face_r;
            assert!(
                from_bulb.length() > bulb_half + half_y,
                "unit collides with the bulb ({})",
                from_bulb.length()
            );
        }
    }

    #[test]
    fn value_maps_linearly_along_the_arc_including_the_mirror() {
        assert!((value_to_angle(0.0, 0.0, 50.0, 212.0, 240.0) - 212.0).abs() < 1e-9);
        assert!((value_to_angle(50.0, 0.0, 50.0, 212.0, 240.0) - 240.0).abs() < 1e-9);
        assert!((value_to_angle(28.0, 0.0, 50.0, 212.0, 240.0) - 227.68).abs() < 1e-6);
        assert!((value_to_angle(29.5, 29.5, 30.7, 148.0, 120.0) - 148.0).abs() < 1e-9);
        assert!((value_to_angle(30.7, 29.5, 30.7, 148.0, 120.0) - 120.0).abs() < 1e-9);
        let mid = value_to_angle(30.1, 29.5, 30.7, 148.0, 120.0);
        assert!((mid - 134.0).abs() < 1e-6);
    }

    #[test]
    fn hpa_converts_with_the_standard_atmosphere() {
        let inhg = hpa_to_inhg(1013.25);
        assert!((inhg - 29.921255).abs() < 1e-4);
        assert!((hpa_to_inhg(1018.5) - 30.075).abs() < 0.01);
    }

    #[test]
    fn today_max_ignores_other_dates_and_the_offset_picks_the_hour() {
        let times = vec![
            "2026-01-14T22:00".into(),
            "2026-01-15T06:00".into(),
            "2026-01-15T10:00".into(),
            "2026-01-15T12:00".into(),
            "2026-01-15T13:00".into(),
            "2026-01-15T15:00".into(),
            "2026-01-16T00:00".into(),
        ];
        let values = vec![1030.0, 1022.0, 1019.0, 1016.0, 1014.0, 1012.0, 1004.0];
        assert_eq!(
            today_max_pressure(&times, &values, "2026-01-15"),
            Some(1022.0)
        );
        let now = NaiveDate::from_ymd_opt(2026, 1, 15)
            .unwrap()
            .and_hms_opt(10, 8, 37)
            .unwrap();
        assert_eq!(pressure_hours_ahead(&times, &values, now, 3), Some(1014.0));
        assert_eq!(pressure_hours_ahead(&times, &values, now, 1), Some(1016.0));
        assert_eq!(pressure_hours_ahead(&times, &values, now, 5), Some(1012.0));
        assert!(pressure_hours_ahead(&times, &values, now, 48).is_none());
    }
}
