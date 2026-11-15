//! Original drawing of the Roman-numeral face from the geometry spec.
//! Lengths marked as percents in the spec are fractions of the radius `r`
//! (`diameter / 2`), except the outline, which is a fraction of the diameter.

use chrono::{DateTime, Local, TimeZone, Timelike};
use egui::{Color32, FontFamily, Mesh, Painter, Pos2, Shape, Stroke, Vec2};

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
            numeral_color: Color32::BLACK,
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
    paint_brass(painter, center, r, 0.0);
    paint_markings_and_hands(painter, center, diameter, now, style, None);
}

/// Inner ring, ticks, numerals, and hands. The kiosk paints these after the aperture discs.
pub(crate) fn paint_markings_and_hands(
    painter: &Painter,
    center: Pos2,
    diameter: f32,
    now: DateTime<Local>,
    style: &RomanClockStyle,
    wind: Option<crate::weather::WindHand>,
) {
    let r = diameter * 0.5;
    paint_inner_ring(painter, center, r, &style);
    paint_ticks(painter, center, r, &style, now);
    paint_numerals(painter, center, r, &style);
    let (hour, minute, second) = hand_angles(now, &style);
    // Gold hands, then the wind hand, then the red second hand. The hub is
    // painted again last so it stays above all of them.
    paint_hand(painter, center, r, hour, &style.hour_hand, &style.shadow);
    paint_hand(
        painter,
        center,
        r,
        minute,
        &style.minute_hand,
        &style.shadow,
    );
    if let Some(wind) = wind {
        paint_wind_hand(painter, center, r, wind);
    }
    paint_hand(
        painter,
        center,
        r,
        second,
        &style.second_hand,
        &style.shadow,
    );
    paint_hub(painter, center, r, &style.hour_hand);
    paint_hub(painter, center, r, &style.minute_hand);
    paint_hub(painter, center, r, &style.second_hand);
}

pub(crate) fn direction(degrees_clockwise_from_12: f32) -> Vec2 {
    let rad = degrees_clockwise_from_12.to_radians();
    Vec2::new(rad.sin(), -rad.cos())
}

/// Dial ring. `inner` is a hole radius in pixels (0 for a solid face).
///
/// Cream at the center, amber at the rim, then a smooth darken to black over
/// the outer 7% of the radius. The mesh stays fine so the rim does not band.
pub(crate) fn paint_brass(painter: &Painter, center: Pos2, radius: f32, inner: f32) {
    let angular = 192u32;
    let radial = 48u32;
    let cols = radial + 1;
    let mut mesh = Mesh::default();
    for i in 0..=angular {
        let t = i as f32 / angular as f32 * std::f32::consts::TAU;
        let dir = Vec2::new(t.cos(), t.sin());
        for j in 0..=radial {
            let u = j as f32 / radial as f32;
            let dist = inner + (radius - inner) * u;
            let p = center + dir * dist;
            mesh.colored_vertex(p, dial_color(dist, radius));
        }
    }
    for i in 0..angular {
        for j in 0..radial {
            let a = i * cols + j;
            mesh.add_triangle(a, a + 1, a + cols);
            mesh.add_triangle(a + 1, a + cols + 1, a + cols);
        }
    }
    painter.add(Shape::mesh(mesh));
}

/// `#ffffb2` at the center, `#ffcc7f` at the dial edge, black in the outer 7%.
fn dial_color(dist: f32, radius: f32) -> Color32 {
    let t = if radius > 0.0 {
        (dist / radius).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let red = 255.0;
    let green = 255.0 + (204.0 - 255.0) * t;
    let blue = 178.0 + (127.0 - 178.0) * t;
    let fade = ((t - 0.93) / 0.07).clamp(0.0, 1.0);
    let fade = fade * fade * (3.0 - 2.0 * fade);
    let keep = 1.0 - fade;
    Color32::from_rgb(
        (red * keep).round() as u8,
        (green * keep).round() as u8,
        (blue * keep).round() as u8,
    )
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

fn paint_ticks(
    painter: &Painter,
    center: Pos2,
    r: f32,
    style: &RomanClockStyle,
    now: DateTime<Local>,
) {
    for step in 0..60 {
        let degrees = step as f32 * 6.0;
        // Every fifth mark, including the quarters, stays a gold pin.
        // Each mark between them is the period of its own place on this half-day.
        if step % 15 == 0 {
            paint_teardrop(painter, center, r, degrees, &style.quarter_tick, Some(1.06));
        } else if step % 5 == 0 {
            paint_teardrop(painter, center, r, degrees, &style.hour_tick, Some(1.0));
        } else {
            let mut minute = style.minute_tick;
            minute.color = crate::cycles::minute_tick_color_at(now, step);
            paint_teardrop(painter, center, r, degrees, &minute, None);
        }
    }
}

fn paint_teardrop(
    painter: &Painter,
    center: Pos2,
    r: f32,
    degrees: f32,
    tick: &TickStyle,
    metal: Option<f32>,
) {
    let dir = direction(degrees);
    let perp = Vec2::new(-dir.y, dir.x);
    let d = tick.distance * r;
    let width = tick.width * r;
    let radius = width * 0.5;
    let hub = center + dir * d;
    let left = hub + perp * radius;
    let right = hub - perp * radius;
    let tip = center + dir * (d - tick.inward * r);
    if let Some(gain) = metal {
        paint_gold_pin(painter, hub, tip, perp, radius + 1.4, gain, true);
        paint_gold_pin(painter, hub, tip, perp, radius, gain, false);
    } else {
        painter.circle_filled(hub, radius.max(0.0), tick.color);
        if tick.inward > 0.0 && width > 0.0 {
            painter.add(Shape::convex_polygon(
                vec![left, right, tip],
                tick.color,
                Stroke::NONE,
            ));
        }
    }
}

/// Raised gold pin: a tapered round body and a domed head.
///
/// The point sits at `tip` and the dome at `hub`. Length comes only from those
/// two points, so the window and the screenshot cannot grow a different spike.
fn paint_gold_pin(
    painter: &Painter,
    hub: Pos2,
    tip: Pos2,
    perp: Vec2,
    radius: f32,
    gain: f32,
    outline: bool,
) {
    let light_bias = perp.dot(crate::brass::light_vec2());
    let stations = 12u32;
    let across = 4u32;
    let cols = across + 1;
    let mut mesh = Mesh::default();
    for i in 0..=stations {
        let t = i as f32 / stations as f32;
        // t = 0 at the point, t = 1 at the head. The last fifth is the dome.
        let (along, half) = if t < 0.82 {
            let u = t / 0.82;
            (u, radius * u.powf(0.72))
        } else {
            let u = (t - 0.82) / 0.18;
            let dome = (1.0 - u * u).max(0.0).sqrt();
            (1.0, radius * dome.max(0.08))
        };
        let axis = tip + (hub - tip) * along.min(1.0);
        // The dome sits on the head, slightly outward of the hub along hub-tip.
        let axis = if t >= 0.82 {
            let u = (t - 0.82) / 0.18;
            hub + (hub - tip).normalized() * (radius * u * 0.85)
        } else {
            axis
        };
        for j in 0..=across {
            let cross = -1.0 + 2.0 * (j as f32 / across as f32);
            let p = axis + perp * (half * cross);
            let color = if outline {
                PIN_EDGE
            } else {
                crate::brass::shade_gold_pin(cross, along, light_bias, gain)
            };
            mesh.colored_vertex(p, color);
        }
    }
    for i in 0..stations {
        for j in 0..across {
            let a = i * cols + j;
            mesh.add_triangle(a, a + 1, a + cols);
            mesh.add_triangle(a + 1, a + cols + 1, a + cols);
        }
    }
    painter.add(Shape::mesh(mesh));
}

fn paint_numerals(painter: &Painter, center: Pos2, r: f32, _style: &RomanClockStyle) {
    crate::ornament::paint_numerals(painter, center, r);
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
    if hand.color == Color32::BLACK {
        if hand.length < 0.65 {
            crate::ornament::paint_hour_hand(painter, center, r, degrees);
        } else {
            crate::ornament::paint_minute_hand(painter, center, r, degrees);
        }
    } else {
        paint_shadow(painter, tail, tip, center, width, cap, blur, shadow.color);
        if width > 0.0 {
            painter.line_segment([tail, tip], Stroke::new(width, hand.color));
        }
    }
    paint_hub(painter, center, r, hand);
}

fn paint_hub(painter: &Painter, center: Pos2, r: f32, hand: &HandStyle) {
    let cap = hand.cap_radius * r;
    if cap <= 0.0 {
        return;
    }
    if hand.color == Color32::BLACK {
        painter.circle_filled(center, cap + 1.15, GOLD_EDGE);
        paint_gold_hub(painter, center, cap);
    } else {
        painter.circle_filled(center, cap, hand.color);
    }
}

const GOLD_EDGE: Color32 = Color32::from_rgb(0x3a, 0x24, 0x08);
/// Darker gold so the pins still read on the cream dial.
const PIN_EDGE: Color32 = Color32::from_rgb(0x5c, 0x3a, 0x0c);

fn paint_gold_hub(painter: &Painter, center: Pos2, radius: f32) {
    let light = crate::brass::light_vec2();
    let segs = 24u32;
    let mut mesh = Mesh::default();
    mesh.colored_vertex(center, crate::brass::shade_gold_pin(0.0, 0.9, 1.0, 1.0));
    for i in 0..=segs {
        let ang = i as f32 / segs as f32 * std::f32::consts::TAU;
        let n = Vec2::new(ang.cos(), ang.sin());
        let cross = n.dot(light);
        mesh.colored_vertex(
            center + n * radius,
            crate::brass::shade_gold_pin(cross, 0.45, 1.0, 1.0),
        );
    }
    for i in 0..segs {
        mesh.add_triangle(0, i + 1, i + 2);
    }
    painter.add(Shape::mesh(mesh));
}

const WIND_YELLOW: Color32 = Color32::from_rgb(0xff, 0xd2, 0x1f);
const WIND_EDGE: Color32 = Color32::from_rgb(0x1c, 0x14, 0x06);

/// Yellow staff pointing where the wind comes from, with station-model barbs.
fn paint_wind_hand(painter: &Painter, center: Pos2, r: f32, wind: crate::weather::WindHand) {
    let Some((pennants, full, half)) = crate::weather::wind_barbs(wind.knots) else {
        let hub = (0.028 * r).max(3.0);
        painter.circle_filled(center, hub + 1.1, WIND_EDGE);
        painter.circle_filled(center, hub, WIND_YELLOW);
        return;
    };
    let dir = direction(wind.from_degrees);
    let perp = Vec2::new(-dir.y, dir.x);
    let reach = 0.58 * r;
    let tip = center + dir * reach;
    // Just under the minute hand (0.01 of the radius). The barb strokes match.
    let width = 0.009 * r;
    let outline = width * 0.42;
    painter.line_segment([center, tip], Stroke::new(width + outline * 2.0, WIND_EDGE));
    painter.line_segment([center, tip], Stroke::new(width, WIND_YELLOW));

    let full_len = 0.11 * r;
    let step = 0.040 * r;
    let mut along = reach - 0.02 * r;
    let barb = |attach: Pos2, len: f32| {
        let lean = (-dir * 0.50 + perp * 0.866).normalized();
        attach + lean * len
    };
    for _ in 0..pennants {
        if along < 0.16 * r {
            break;
        }
        let outer = center + dir * along;
        let inner = center + dir * (along - 0.048 * r);
        let point = barb(outer, full_len * 0.92);
        painter.add(Shape::convex_polygon(
            vec![outer, inner, point],
            WIND_YELLOW,
            Stroke::new(outline.max(1.2), WIND_EDGE),
        ));
        along -= 0.062 * r;
    }
    for (count, len) in [(full, full_len), (u32::from(half), full_len * 0.5)] {
        for _ in 0..count {
            if along < 0.16 * r {
                break;
            }
            let attach = center + dir * along;
            let end = barb(attach, len);
            painter.line_segment([attach, end], Stroke::new(width + outline * 2.0, WIND_EDGE));
            painter.line_segment([attach, end], Stroke::new(width, WIND_YELLOW));
            along -= step;
        }
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
