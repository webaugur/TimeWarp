//! Curved thermometer and barometer in the lower corner wedges.
//!
//! Each gauge is a clinical glass thermometer bent along an arc concentric
//! with the dial, just outside the rim. The left wedge is temperature in °F.
//! The right wedge is mean-sea-level pressure in inches of mercury. A full
//! quarter-circle outside the dial does not fit: the square corner pinches
//! to nothing at 3, 6, and 9, so each arc is the span that stays inside the
//! wedge with the numerals still clear of the dial.

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

/// Scale plate, in face radii. Outside the cream dial (1.0) and inside the square.
/// The ends stay about 30° off the 3, 6, and 9 tangents, where the square pinches shut.
const PLATE_IN: f32 = 1.028;
const PLATE_OUT: f32 = 1.112;
const TUBE_IN: f32 = 1.076;
const TUBE_OUT: f32 = 1.106;
const FLUID_IN: f32 = 1.084;
const FLUID_OUT: f32 = 1.098;
const LABEL_R: f32 = 1.048;

/// Left wedge, bulb toward 6, values increasing toward 9.
const LEFT_START: f32 = 212.0;
const LEFT_END: f32 = 240.0;
/// Right wedge, mirrored: bulb toward 6, values increasing toward 3.
const RIGHT_START: f32 = 148.0;
const RIGHT_END: f32 = 120.0;

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
    paint_thermometer(painter, center, radius, now, weather);
    paint_barometer(painter, center, radius, now, weather, barometer_hours);
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
        unit: "°F",
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
    if let Some(value) = high {
        marker(
            &painter,
            center,
            radius,
            &spec,
            lo as f64,
            hi as f64,
            value as f64,
            RED,
        );
    }
    if let Some(value) = dew {
        marker(
            &painter,
            center,
            radius,
            &spec,
            lo as f64,
            hi as f64,
            value as f64,
            GREEN,
        );
    }
    if let Some(value) = feels {
        marker(
            &painter,
            center,
            radius,
            &spec,
            lo as f64,
            hi as f64,
            value as f64,
            YELLOW,
        );
    }
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
    if let Some(value) = peak {
        marker(&painter, center, radius, &spec, lo, hi, value, RED);
    }
    if let Some(value) = ahead {
        marker(&painter, center, radius, &spec, lo, hi, value, YELLOW);
    }
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
    let outline = 1.4 / radius;
    band(
        painter,
        center,
        radius,
        plate_start,
        plate_end,
        PLATE_IN - outline,
        PLATE_OUT + outline,
        &[Color32::BLACK, Color32::BLACK],
    );
    band(
        painter,
        center,
        radius,
        plate_start,
        plate_end,
        PLATE_IN,
        PLATE_OUT,
        &[Color32::WHITE, Color32::WHITE],
    );
    for value in tick_values(lo, hi, spec.minor) {
        let major = on_step(value, spec.major);
        let outer = if major {
            TUBE_IN - 0.002
        } else {
            TUBE_IN - 0.012
        };
        let width = if major { 1.25 } else { 0.8 };
        radial_line(
            painter,
            center,
            radius,
            angle_of(spec, lo, hi, value),
            LABEL_R + 0.006,
            outer,
            Stroke::new(width, Color32::BLACK),
        );
        if major {
            let label = format_tick(value, spec.decimals);
            let pos = polar(center, radius, angle_of(spec, lo, hi, value), LABEL_R);
            painter.text(
                pos,
                Align2::CENTER_CENTER,
                label,
                FontId::new(0.040 * radius, FontFamily::Proportional),
                Color32::BLACK,
            );
        }
    }
    let mid = (spec.start + spec.end) * 0.5;
    let unit_at = polar(center, radius, mid, PLATE_OUT + 0.030);
    let unit_font = FontId::new(0.030 * radius, FontFamily::Proportional);
    for (dx, dy) in [(-1.2, 0.0), (1.2, 0.0), (0.0, -1.2), (0.0, 1.2)] {
        painter.text(
            unit_at + Vec2::new(dx, dy),
            Align2::CENTER_CENTER,
            spec.unit,
            unit_font.clone(),
            Color32::WHITE,
        );
    }
    painter.text(
        unit_at,
        Align2::CENTER_CENTER,
        spec.unit,
        unit_font,
        Color32::from_rgb(0x1a, 0x1a, 0x1a),
    );

    let fluid_ang = angle_of(spec, lo, hi, fluid);
    band(
        painter,
        center,
        radius,
        plate_start,
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
        (TUBE_IN + TUBE_OUT) * 0.5,
        tube_px(radius),
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
    let sign = if spec.end >= spec.start { 1.0 } else { -1.0 };
    fluid_bulb(
        painter,
        center,
        radius,
        plate_start + sign * 1.5,
        (TUBE_IN + TUBE_OUT) * 0.5 - 0.008,
    );
}

/// A little plate past the first and last tick, so the end numerals and the bulb
/// are not sitting on the edge. Degrees, in the direction of increasing value.
fn extended(spec: &ArcSpec) -> (f32, f32) {
    let sign = if spec.end >= spec.start { 1.0 } else { -1.0 };
    (spec.start - sign * 3.5, spec.end + sign * 2.5)
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
) {
    let ang = angle_of(spec, lo, hi, value);
    radial_line(
        painter,
        center,
        radius,
        ang,
        PLATE_IN + 0.004,
        PLATE_OUT + 0.004,
        Stroke::new(3.2, EDGE),
    );
    radial_line(
        painter,
        center,
        radius,
        ang,
        PLATE_IN + 0.004,
        PLATE_OUT + 0.004,
        Stroke::new(1.8, color),
    );
    let tip = polar(center, radius, ang, PLATE_OUT + 0.010);
    painter.circle_filled(tip, 3.3, EDGE);
    painter.circle_filled(tip, 2.3, color);
}

/// Reservoir at the low end. Diameter is about 2.5 tube-widths, so the radius
/// is 1.25 tube-widths (`tube_px` is the tube's half-width).
fn fluid_bulb(painter: &Painter, center: Pos2, radius: f32, angle: f32, at: f32) {
    let origin = polar(center, radius, angle, at);
    let px = tube_px(radius) * 2.5;
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

fn wedge_rect(center: Pos2, radius: f32, left: bool) -> Rect {
    if left {
        Rect::from_min_max(
            Pos2::new(center.x - radius, center.y),
            Pos2::new(center.x, center.y + radius),
        )
    } else {
        Rect::from_min_max(
            Pos2::new(center.x, center.y),
            Pos2::new(center.x + radius, center.y + radius),
        )
    }
}

fn tube_px(radius: f32) -> f32 {
    (TUBE_OUT - TUBE_IN) * 0.5 * radius
}

fn fluid_px(radius: f32) -> f32 {
    (FLUID_OUT - FLUID_IN) * 0.5 * radius
}

fn fahrenheit(value: f32, units: Units) -> Option<f32> {
    if !value.is_finite() {
        return None;
    }
    Some(match units {
        Units::Imperial => value,
        Units::Metric => value * 9.0 / 5.0 + 32.0,
    })
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
