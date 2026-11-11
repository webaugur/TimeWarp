//! Original drawing of the Roman-numeral face from the geometry spec.
//! Lengths marked as percents in the spec are fractions of the radius `r`
//! (`diameter / 2`), except the outline, which is a fraction of the diameter.

use chrono::{DateTime, Local, TimeZone, Timelike};
use egui::{Color32, FontFamily, FontId, Mesh, Painter, Pos2, Shape, Stroke, Vec2};

/// Teardrop tick. Distances and `width` are fractions of the radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TickStyle {
    /// Distance from the clock center to the teardrop's round end.
    pub distance: f32,
    /// How much further inward the point sits, measured from the round end.
    pub inward: f32,
    /// Width of the round end. The circle radius is half of this.
    pub width: f32,
    pub color: Color32,
}

/// Straight hand. Lengths are fractions of the radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandStyle {
    pub color: Color32,
    /// Tip distance from the center, along the forward direction.
    pub length: f32,
    /// Tail distance behind the center.
    pub tail: f32,
    pub width: f32,
    /// Filled circle at the center.
    pub cap_radius: f32,
}

/// Damped overshoot on the second hand during the first part of each second.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SecondBounce {
    /// Bounce applies while `0 < milliseconds < window_ms`.
    pub window_ms: f64,
    /// Multiplies one tick (6°). The spec uses `0.55`.
    pub amplitude: f64,
    /// `sin(m / sin_divisor_ms)`, with the argument in radians.
    pub sin_divisor_ms: f64,
    /// `exp(m / exp_divisor_ms)`.
    pub exp_divisor_ms: f64,
}

/// Soft shadow under each hand. No offset; the blur is a fraction of `r`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShadowStyle {
    pub color: Color32,
    pub blur: f32,
}

/// Every tunable from the geometry spec.
#[derive(Clone, Debug, PartialEq)]
pub struct RomanClockStyle {
    pub face_center: Color32,
    pub face_edge: Color32,
    pub outline: Color32,
    /// Visible rim thickness as a fraction of the diameter (spec: `0.01`).
    pub outline_width_of_diameter: f32,
    pub inner_ring: Color32,
    /// Radius of the thin ring, fraction of `r`.
    pub inner_ring_radius: f32,
    /// Stroke width of the thin ring, fraction of `r`.
    pub inner_ring_width: f32,
    pub quarter_tick: TickStyle,
    pub hour_tick: TickStyle,
    pub minute_tick: TickStyle,
    pub numeral_color: Color32,
    /// Pixel height of XII / III / VI / IX, as a fraction of `r`.
    pub numeral_size: f32,
    /// Distance from the center to each numeral's anchor, fraction of `r`.
    pub numeral_radius: f32,
    pub numeral_font: FontFamily,
    pub hour_hand: HandStyle,
    pub minute_hand: HandStyle,
    pub second_hand: HandStyle,
    pub shadow: ShadowStyle,
    pub second_bounce: SecondBounce,
}

impl Default for RomanClockStyle {
    fn default() -> Self {
        Self {
            face_center: Color32::from_rgb(0xf7, 0xdb, 0xb7),
            face_edge: Color32::from_rgb(0xe5, 0xca, 0xa7),
            outline: Color32::from_rgb(0x7c, 0x4e, 0x1f),
            outline_width_of_diameter: 0.01,
            inner_ring: Color32::BLACK,
            inner_ring_radius: 0.68,
            inner_ring_width: 0.005,
            quarter_tick: TickStyle {
                distance: 0.70,
                inward: 0.20,
                width: 0.06,
                color: Color32::from_rgb(0x77, 0x6a, 0x1d),
            },
            hour_tick: TickStyle {
                distance: 0.72,
                inward: 0.20,
                width: 0.04,
                color: Color32::from_rgb(0x7c, 0x4e, 0x1f),
            },
            minute_tick: TickStyle {
                distance: 0.70,
                inward: 0.05,
                width: 0.01,
                color: Color32::from_rgb(0x77, 0x00, 0x00),
            },
            numeral_color: Color32::from_rgb(0x47, 0x30, 0x13),
            numeral_size: 0.24,
            numeral_radius: 0.84,
            numeral_font: FontFamily::Name("serif".into()),
            hour_hand: HandStyle {
                color: Color32::BLACK,
                length: 0.60,
                tail: 0.10,
                width: 0.03,
                cap_radius: 0.07,
            },
            minute_hand: HandStyle {
                color: Color32::BLACK,
                length: 0.70,
                tail: 0.20,
                width: 0.01,
                cap_radius: 0.05,
            },
            second_hand: HandStyle {
                color: Color32::from_rgb(0xff, 0x00, 0x00),
                length: 0.70,
                tail: 0.10,
                width: 0.01,
                cap_radius: 0.03,
            },
            shadow: ShadowStyle {
                color: Color32::from_rgba_unmultiplied(0, 0, 0, 153),
                blur: 0.01,
            },
            second_bounce: SecondBounce {
                window_ms: 250.0,
                amplitude: 0.55,
                sin_divisor_ms: 25.0,
                exp_divisor_ms: 100.0,
            },
        }
    }
}

/// Local civil milliseconds since midnight, including the fractional millisecond.
///
/// Uses the clock time in `DateTime<Local>` (zoneinfo, DST included).
/// A fixed offset such as EDT (−4h) is wrong for half the year.
pub fn milliseconds_since_midnight(now: DateTime<Local>) -> f64 {
    let secs = now.time().num_seconds_from_midnight() as f64;
    let sub_ms = now.timestamp_subsec_nanos() as f64 / 1_000_000.0;
    secs * 1000.0 + sub_ms
}

/// Hour, minute, and second angles in degrees, clockwise from 12 o'clock.
pub fn hand_angles(now: DateTime<Local>, style: &RomanClockStyle) -> (f32, f32, f32) {
    let ms = milliseconds_since_midnight(now);
    let hour = ((ms / 3_600_000.0) % 12.0) * 30.0;
    let minute = ((ms / 60_000.0) % 60.0) * 6.0;
    let sec_index = (ms / 1000.0).floor() % 60.0;
    let into_second = ms % 1000.0;
    let bounce = &style.second_bounce;
    let second = if into_second > 0.0 && into_second < bounce.window_ms {
        let amp = bounce.amplitude * 6.0 * (into_second / bounce.sin_divisor_ms).sin()
            / (into_second / bounce.exp_divisor_ms).exp();
        sec_index * 6.0 + amp
    } else {
        sec_index * 6.0
    };
    (hour as f32, minute as f32, second as f32)
}

/// 10:08:37.000 local, used by `--screenshot` so the face is reproducible.
pub fn screenshot_time() -> DateTime<Local> {
    match Local.with_ymd_and_hms(2026, 1, 15, 10, 8, 37) {
        chrono::LocalResult::Single(time) => time,
        chrono::LocalResult::Ambiguous(earliest, _) => earliest,
        chrono::LocalResult::None => {
            panic!("local civil time 2026-01-15 10:08:37 does not exist in this timezone")
        }
    }
}

/// Paint the face with [`RomanClockStyle::default`].
pub fn paint(painter: &Painter, center: Pos2, diameter: f32, now: DateTime<Local>) {
    paint_with(painter, center, diameter, now, &RomanClockStyle::default());
}

/// Paint the face. `center` is the middle of the dial; `diameter` is in pixels.
pub fn paint_with(
    painter: &Painter,
    center: Pos2,
    diameter: f32,
    now: DateTime<Local>,
    style: &RomanClockStyle,
) {
    if !diameter.is_finite() || diameter <= 0.0 || !center.x.is_finite() || !center.y.is_finite() {
        return;
    }
    let r = diameter * 0.5;
    paint_face(painter, center, r, style);
    paint_outline(painter, center, r, diameter, style);
    paint_inner_ring(painter, center, r, style);
    paint_ticks(painter, center, r, style);
    paint_numerals(painter, center, r, style);
    let (hour, minute, second) = hand_angles(now, style);
    // Hour, then minute, then second, so the second hand (and its cap) is on top.
    paint_hand(painter, center, r, hour, &style.hour_hand, &style.shadow);
    paint_hand(
        painter,
        center,
        r,
        minute,
        &style.minute_hand,
        &style.shadow,
    );
    paint_hand(
        painter,
        center,
        r,
        second,
        &style.second_hand,
        &style.shadow,
    );
}

pub(crate) fn direction(degrees_clockwise_from_12: f32) -> Vec2 {
    let rad = degrees_clockwise_from_12.to_radians();
    Vec2::new(rad.sin(), -rad.cos())
}

fn paint_face(painter: &Painter, center: Pos2, r: f32, style: &RomanClockStyle) {
    // Triangle fan. Edge vertices share one color, so the disk reads as a
    // radial blend from `face_center` to `face_edge`. The rim covers the
    // aliased circumference.
    const SEGMENTS: u32 = 128;
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, style.face_center);
    for i in 0..=SEGMENTS {
        let t = i as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
        let p = center + Vec2::new(t.cos(), t.sin()) * r;
        mesh.colored_vertex(p, style.face_edge);
    }
    for i in 0..SEGMENTS {
        mesh.add_triangle(0, 1 + i, 2 + i);
    }
    painter.add(Shape::mesh(mesh));
}

fn paint_outline(painter: &Painter, center: Pos2, r: f32, diameter: f32, style: &RomanClockStyle) {
    // The visible band is `outline_width_of_diameter` of the diameter, lying
    // entirely inside the face. A stroke centered on the circumference would
    // lose its outer half to the clip; insetting by half the width matches that.
    let width = style.outline_width_of_diameter * diameter;
    if width <= 0.0 {
        return;
    }
    let radius = r - width * 0.5;
    painter.circle_stroke(center, radius.max(0.0), Stroke::new(width, style.outline));
}

fn paint_inner_ring(painter: &Painter, center: Pos2, r: f32, style: &RomanClockStyle) {
    let width = style.inner_ring_width * r;
    if width <= 0.0 {
        return;
    }
    painter.circle_stroke(
        center,
        style.inner_ring_radius * r,
        Stroke::new(width, style.inner_ring),
    );
}

fn paint_ticks(painter: &Painter, center: Pos2, r: f32, style: &RomanClockStyle) {
    for step in 0..60 {
        let degrees = step as f32 * 6.0;
        let tick = if step % 15 == 0 {
            &style.quarter_tick
        } else if step % 5 == 0 {
            &style.hour_tick
        } else {
            &style.minute_tick
        };
        paint_teardrop(painter, center, r, degrees, tick);
    }
}

fn paint_teardrop(painter: &Painter, center: Pos2, r: f32, degrees: f32, tick: &TickStyle) {
    let dir = direction(degrees);
    let perp = Vec2::new(-dir.y, dir.x);
    let d = tick.distance * r;
    let width = tick.width * r;
    let radius = width * 0.5;
    let hub = center + dir * d;
    let left = hub + perp * radius;
    let right = hub - perp * radius;
    let tip = center + dir * (d - tick.inward * r);
    painter.circle_filled(hub, radius.max(0.0), tick.color);
    if tick.inward > 0.0 && width > 0.0 {
        painter.add(Shape::convex_polygon(
            vec![left, right, tip],
            tick.color,
            Stroke::NONE,
        ));
    }
}

fn paint_numerals(painter: &Painter, center: Pos2, r: f32, style: &RomanClockStyle) {
    const LABELS: [&str; 4] = ["XII", "III", "VI", "IX"];
    let px = style.numeral_size * r;
    if px <= 0.0 {
        return;
    }
    let font = FontId::new(px, style.numeral_font.clone());
    for (index, label) in LABELS.iter().enumerate() {
        let anchor = center + direction(index as f32 * 90.0) * (style.numeral_radius * r);
        painter.text(
            anchor,
            egui::Align2::CENTER_CENTER,
            *label,
            font.clone(),
            style.numeral_color,
        );
    }
}

fn paint_hand(
    painter: &Painter,
    center: Pos2,
    r: f32,
    degrees: f32,
    hand: &HandStyle,
    shadow: &ShadowStyle,
) {
    let dir = direction(degrees);
    let tip = center + dir * (hand.length * r);
    let tail = center - dir * (hand.tail * r);
    let width = hand.width * r;
    let cap = hand.cap_radius * r;
    let blur = shadow.blur * r;
    paint_shadow(painter, tail, tip, center, width, cap, blur, shadow.color);
    if width > 0.0 {
        painter.line_segment([tail, tip], Stroke::new(width, hand.color));
    }
    if cap > 0.0 {
        painter.circle_filled(center, cap, hand.color);
    }
}

/// A few wider translucent strokes stand in for a 1%-of-r gaussian blur.
/// Layer alphas are chosen so the stacked core is about `rgba(0,0,0,0.6)`.
fn paint_shadow(
    painter: &Painter,
    tail: Pos2,
    tip: Pos2,
    center: Pos2,
    width: f32,
    cap: f32,
    blur: f32,
    color: Color32,
) {
    // (extra stroke width, in multiples of `blur`, and straight alpha).
    // Composed opacity of the four alphas is ~0.596, i.e. the spec's 0.6.
    const LAYERS: [(f32, f32); 4] = [(2.0, 0.12), (1.2, 0.16), (0.55, 0.22), (0.18, 0.30)];
    let scale = (color.a() as f32 / 255.0) / 0.6;
    let [red, green, blue, _] = color.to_array();
    for (spread, alpha) in LAYERS {
        let a = ((alpha * scale) * 255.0).round().clamp(0.0, 255.0) as u8;
        if a == 0 {
            continue;
        }
        let layer = Color32::from_rgba_unmultiplied(red, green, blue, a);
        let extra = blur * spread;
        painter.line_segment([tail, tip], Stroke::new((width + extra).max(0.0), layer));
        painter.circle_filled(center, (cap + extra * 0.5).max(0.0), layer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn at_hms(h: u32, m: u32, s: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 1, 15, h, m, s)
            .single()
            .expect("civil time")
    }

    #[test]
    fn ten_oh_eight_thirty_seven_is_smooth_hour_minute_and_ticked_second() {
        let now = at_hms(10, 8, 37);
        let (hour, minute, second) = hand_angles(now, &RomanClockStyle::default());
        let ms = (10 * 3600 + 8 * 60 + 37) as f64 * 1000.0;
        let expect_hour = ((ms / 3_600_000.0) % 12.0) * 30.0;
        let expect_minute = ((ms / 60_000.0) % 60.0) * 6.0;
        assert!((f64::from(hour) - expect_hour).abs() < 1e-4);
        assert!((f64::from(minute) - expect_minute).abs() < 1e-4);
        assert!((f64::from(second) - 37.0 * 6.0).abs() < 1e-4);
        // 10:08 is just past the 10, not on a fixed UTC-4 conversion.
        assert!((f64::from(hour) - 304.308333).abs() < 1e-3);
        assert!((f64::from(minute) - 51.7).abs() < 1e-3);
    }

    #[test]
    fn second_hand_bounces_only_inside_the_window() {
        let base = at_hms(10, 8, 37);
        let still = hand_angles(base, &RomanClockStyle::default()).2;
        let early = hand_angles(
            base + chrono::Duration::milliseconds(100),
            &RomanClockStyle::default(),
        )
        .2;
        let done = hand_angles(
            base + chrono::Duration::milliseconds(250),
            &RomanClockStyle::default(),
        )
        .2;
        let bounce = 0.55 * 6.0 * (100.0_f64 / 25.0).sin() / (100.0_f64 / 100.0).exp();
        assert!((f64::from(still) - 222.0).abs() < 1e-4);
        assert!((f64::from(early) - (222.0 + bounce)).abs() < 1e-4);
        assert!((f64::from(done) - 222.0).abs() < 1e-4);
    }

    #[test]
    fn angles_follow_local_civil_time_not_utc() {
        let now = at_hms(10, 8, 37);
        let ms = milliseconds_since_midnight(now);
        let local_ms = now.naive_local().num_seconds_from_midnight() as f64 * 1000.0;
        assert!((ms - local_ms).abs() < 1.0);
        let offset = now.fixed_offset().offset().local_minus_utc();
        if offset % 86_400 != 0 {
            let utc_ms = now.naive_utc().num_seconds_from_midnight() as f64 * 1000.0;
            assert!(
                (ms - utc_ms).abs() > 1_000.0,
                "expected local midnight offset to move the hands"
            );
        }
    }
}
