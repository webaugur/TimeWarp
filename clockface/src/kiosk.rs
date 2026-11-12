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

use chrono::{DateTime, Datelike, Local, Timelike};

use egui::{Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Stroke, Vec2};

use crate::brass::{mix, shade_bezel};
use crate::draw::{paint_disc, paint_pie, stroke_arc};
use crate::events::{self, CalendarEvent};
use crate::layout::{self, KioskLayout, APERTURE_R, SKY_WEDGE, YEAR_WEDGE};
use crate::roman::{self, direction};
use crate::settings::Place;
use crate::sky::{self, season_name};
use crate::status::StatusLine;
use crate::weather::{self, Scene, WeatherReport};

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

    paint_wedge(painter, &layout, YEAR_WEDGE, |deg, frac| {
        season_color(
            layout::wheel_sample(sky::year_fraction(model.now), deg, YEAR_WEDGE),
            frac,
        )
    });
    paint_wedge(painter, &layout, SKY_WEDGE, |deg, frac| {
        sky_color(
            layout::wheel_sample(sky::day_fraction(model.now), deg, SKY_WEDGE),
            frac,
        )
    });
    paint_wedge_rim(painter, center, radius, YEAR_WEDGE);
    paint_wedge_rim(painter, center, radius, SKY_WEDGE);
    paint_year_marks(painter, center, radius, sky::year_fraction(model.now));
    paint_sky_marks(painter, center, radius, sky::day_fraction(model.now));

    let aperture = APERTURE_R * radius;
    // Upper half, horizon on the 9–3 line. One sky, not two quarters.
    paint_sector(painter, center, aperture, 270.0, 180.0, |p| {
        horizon_color(&model.weather, model.now, center, aperture, p)
    });
    paint_aperture(painter, center, aperture, 180.0, 270.0, |p| {
        date_disc_color(center, aperture, p)
    });
    paint_aperture(painter, center, aperture, 90.0, 180.0, |p| {
        date_disc_color(center, aperture, p)
    });

    paint_horizon_scene(painter, center, radius, aperture, &model.weather, model.now);
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

fn paint_wedge(
    painter: &Painter,
    layout: &KioskLayout,
    wedge: (f32, f32),
    mut color_at: impl FnMut(f32, f32) -> Color32,
) {
    let steps = 28u32;
    let span = (wedge.1 - wedge.0).rem_euclid(360.0);
    let mut mesh = egui::Mesh::default();
    for i in 0..=steps {
        let deg = wedge.0 + span * (i as f32 / steps as f32);
        let outer = layout::window_outer(wedge, deg);
        let inner_p = layout.point(deg, 1.0);
        let outer_p = layout.point(deg, outer);
        let outward = ((outer - 1.0) / (std::f32::consts::SQRT_2 - 1.0)).clamp(0.0, 1.0);
        crate::draw::vertex_color(&mut mesh, inner_p, color_at(deg, 0.0));
        crate::draw::vertex_color(&mut mesh, outer_p, color_at(deg, outward));
    }
    for i in 0..steps {
        let base = i * 2;
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base + 1, base + 3, base + 2);
    }
    crate::draw::add_mesh(painter, mesh);
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

/// Sector of a disc, sampled on a grid so a horizon gradient survives the fill.
/// `start` is degrees clockwise from 12. `sweep` is clockwise and positive.
fn paint_sector(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    start: f32,
    sweep: f32,
    mut color_at: impl FnMut(Pos2) -> Color32,
) {
    let angular = 48u32;
    let radial = 14u32;
    let cols = radial + 1;
    let mut mesh = egui::Mesh::default();
    for i in 0..=angular {
        let deg = start + sweep * (i as f32 / angular as f32);
        let dir = direction(deg);
        for j in 0..=radial {
            let p = center + dir * (radius * (j as f32 / radial as f32));
            crate::draw::vertex_color(&mut mesh, p, color_at(p));
        }
    }
    for i in 0..angular {
        for j in 0..radial {
            let a = i * cols + j;
            mesh.add_triangle(a, a + 1, a + cols);
            mesh.add_triangle(a + 1, a + cols + 1, a + cols);
        }
    }
    crate::draw::add_mesh(painter, mesh);
}

fn season_color(shown: f32, outward: f32) -> Color32 {
    // Solstices and equinoxes as fractions of a non-leap year, blended on a circle.
    let keys = [
        (
            0.0,
            Color32::from_rgb(0x1c, 0x2a, 0x38),
            Color32::from_rgb(0xe8, 0xf2, 0xf6),
            Color32::from_rgb(0x9a, 0xb8, 0xc4),
        ),
        (
            0.216,
            Color32::from_rgb(0x6a, 0x9a, 0x4a),
            Color32::from_rgb(0xc5, 0xe4, 0xf2),
            Color32::from_rgb(0xf2, 0x8a, 0xb0),
        ),
        (
            0.471,
            Color32::from_rgb(0x1f, 0x6b, 0x32),
            Color32::from_rgb(0x7e, 0xc8, 0xef),
            Color32::from_rgb(0xe6, 0xc4, 0x4a),
        ),
        (
            0.726,
            Color32::from_rgb(0x8a, 0x3e, 0x18),
            Color32::from_rgb(0xf0, 0xb0, 0x6a),
            Color32::from_rgb(0xd4, 0x52, 0x2a),
        ),
        (
            1.0,
            Color32::from_rgb(0x1c, 0x2a, 0x38),
            Color32::from_rgb(0xe8, 0xf2, 0xf6),
            Color32::from_rgb(0x9a, 0xb8, 0xc4),
        ),
    ];
    let (land, sky, accent) = blend_season(shown.rem_euclid(1.0), &keys);
    let _ = accent;
    mix(land, sky, (outward * 0.85).clamp(0.0, 1.0))
}

fn blend_season(t: f32, keys: &[(f32, Color32, Color32, Color32)]) -> (Color32, Color32, Color32) {
    let mut lo = keys[0];
    let mut hi = keys[keys.len() - 1];
    for window in keys.windows(2) {
        if t >= window[0].0 && t <= window[1].0 {
            lo = window[0];
            hi = window[1];
            break;
        }
    }
    let span = (hi.0 - lo.0).max(0.001);
    let u = ((t - lo.0) / span).clamp(0.0, 1.0);
    (mix(lo.1, hi.1, u), mix(lo.2, hi.2, u), mix(lo.3, hi.3, u))
}

fn sky_color(shown: f32, _outward: f32) -> Color32 {
    let t = shown.rem_euclid(1.0);
    // midnight, dawn, morning, noon, afternoon, dusk, night
    let keys = [
        (0.00, Color32::from_rgb(0x07, 0x0b, 0x1c)),
        (0.22, Color32::from_rgb(0x1a, 0x24, 0x48)),
        (0.30, Color32::from_rgb(0xe0, 0x86, 0x4a)),
        (0.42, Color32::from_rgb(0x4a, 0x90, 0xc8)),
        (0.50, Color32::from_rgb(0x3d, 0x8e, 0xd0)),
        (0.70, Color32::from_rgb(0x3a, 0x78, 0xb0)),
        (0.80, Color32::from_rgb(0xc4, 0x58, 0x32)),
        (0.90, Color32::from_rgb(0x14, 0x18, 0x36)),
        (1.00, Color32::from_rgb(0x07, 0x0b, 0x1c)),
    ];
    let mut color = keys[0].1;
    for window in keys.windows(2) {
        if t >= window[0].0 && t <= window[1].0 {
            let u = (t - window[0].0) / (window[1].0 - window[0].0).max(0.001);
            color = mix(window[0].1, window[1].1, u);
            break;
        }
    }
    color
}

/// Sky above the 9–3 horizon, land along that line. `radius` is the aperture radius.
fn horizon_color(
    weather: &WeatherReport,
    now: DateTime<Local>,
    center: Pos2,
    radius: f32,
    point: Pos2,
) -> Color32 {
    let rel = (point - center) / radius.max(1.0);
    let up = (-rel.y).clamp(0.0, 1.0);
    let day = sky::day_fraction(now);
    // Positive while the sun is up. 6:00 is the horizon, noon is overhead.
    let alt = ((day - 0.25) * std::f32::consts::TAU).sin();
    let dayness = alt.clamp(0.0, 1.0);
    let twilight = (1.0 - alt.abs() * 2.4).clamp(0.0, 1.0);
    let zenith = mix(
        Color32::from_rgb(0x07, 0x0c, 0x1e),
        Color32::from_rgb(0x2c, 0x6e, 0xb8),
        dayness,
    );
    let mut horizon = mix(
        Color32::from_rgb(0x16, 0x24, 0x44),
        Color32::from_rgb(0xc5, 0xdf, 0xf4),
        dayness,
    );
    horizon = mix(horizon, Color32::from_rgb(0xe8, 0x86, 0x42), twilight * 0.8);
    let mut color = mix(horizon, zenith, up.powf(0.55));
    let scene = scene_now(weather, now);
    let weather_tint = match scene {
        Scene::Snow => (Color32::from_rgb(0xd8, 0xe4, 0xee), 0.5),
        Scene::Rain => (Color32::from_rgb(0x5c, 0x6c, 0x7c), 0.45),
        Scene::Cloud | Scene::Unknown => (Color32::from_rgb(0x9a, 0xa6, 0xb2), 0.4),
        Scene::Storm => (Color32::from_rgb(0x2a, 0x32, 0x42), 0.6),
        Scene::Fog => (Color32::from_rgb(0xcc, 0xd0, 0xd2), 0.65),
        Scene::ClearDay | Scene::ClearNight => (color, 0.0),
    };
    color = mix(color, weather_tint.0, weather_tint.1);
    if up < 0.16 {
        let ground = match scene {
            Scene::Snow => Color32::from_rgb(0xe7, 0xee, 0xf2),
            Scene::ClearNight | Scene::Storm => Color32::from_rgb(0x12, 0x16, 0x20),
            Scene::Rain => Color32::from_rgb(0x3e, 0x4a, 0x3a),
            _ if dayness < 0.25 => Color32::from_rgb(0x14, 0x18, 0x22),
            _ => Color32::from_rgb(0x6d, 0x84, 0x58),
        };
        color = mix(ground, color, up / 0.16);
    }
    color
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

fn paint_horizon_scene(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    aperture: f32,
    weather: &WeatherReport,
    now: DateTime<Local>,
) {
    // The rectangle covers the upper half. The brass ring, painted later, covers
    // anything that falls outside the circular aperture.
    let clip = Rect::from_min_max(
        Pos2::new(center.x - aperture, center.y - aperture),
        Pos2::new(center.x + aperture, center.y + 1.0),
    );
    let painter = painter.with_clip_rect(clip);
    let day = sky::day_fraction(now);
    let alt = ((day - 0.25) * std::f32::consts::TAU).sin();
    let dayness = alt.clamp(0.0, 1.0);
    let scene = scene_now(weather, now);
    let night = dayness < 0.08;

    if night {
        paint_stars(&painter, center, radius);
    }
    if dayness > 0.05 && !matches!(scene, Scene::Storm) {
        let t = ((day - 0.25) / 0.5).clamp(0.0, 1.0);
        let x = aperture * (0.55 - 1.1 * t);
        let y = -aperture * (0.12 + 0.62 * (t * std::f32::consts::PI).sin());
        let sun = center + Vec2::new(x, y);
        let dim = matches!(
            scene,
            Scene::Cloud | Scene::Rain | Scene::Snow | Scene::Fog | Scene::Unknown
        );
        paint_disc(
            &painter,
            sun,
            if dim { 0.045 * radius } else { 0.07 * radius },
            16,
            |_| {
                if dim {
                    Color32::from_rgb(0xf0, 0xe2, 0xb0)
                } else {
                    Color32::from_rgb(0xff, 0xc8, 0x3a)
                }
            },
        );
    }
    if night || dayness < 0.45 {
        paint_moon_disc(&painter, center, radius, sky::moon_phase(now));
    }

    let cloud_color = match scene {
        Scene::Storm => Color32::from_rgb(0x5c, 0x66, 0x74),
        Scene::Rain => Color32::from_rgb(0xd0, 0xd8, 0xe0),
        _ => Color32::from_rgb(0xf4, 0xf7, 0xfa),
    };
    match scene {
        Scene::Cloud | Scene::Unknown | Scene::Fog | Scene::Rain | Scene::Snow | Scene::Storm => {
            for (dx, dy, scale) in [
                (-0.22, -0.28, 1.15),
                (0.16, -0.42, 0.9),
                (0.34, -0.22, 0.75),
                (-0.02, -0.16, 1.0),
            ] {
                cloud(
                    &painter,
                    center + Vec2::new(dx * aperture, dy * aperture),
                    radius * scale,
                    cloud_color,
                );
            }
        }
        _ => {}
    }
    match scene {
        Scene::Rain => {
            for (dx, dy) in [
                (-0.3, -0.2),
                (-0.1, -0.35),
                (0.12, -0.18),
                (0.28, -0.32),
                (0.02, -0.48),
                (-0.22, -0.5),
            ] {
                let at = center + Vec2::new(dx * aperture, dy * aperture);
                painter.line_segment(
                    [at, at + Vec2::new(-0.012 * radius, 0.06 * radius)],
                    Stroke::new(1.3, Color32::from_rgb(0x9e, 0xc4, 0xdc)),
                );
            }
        }
        Scene::Snow => {
            for (i, (dx, dy)) in [
                (-0.32, -0.22),
                (-0.14, -0.4),
                (0.02, -0.18),
                (0.2, -0.34),
                (0.36, -0.16),
                (-0.24, -0.52),
                (0.1, -0.5),
                (-0.04, -0.3),
            ]
            .iter()
            .enumerate()
            {
                paint_disc(
                    &painter,
                    center + Vec2::new(dx * aperture, dy * aperture),
                    0.01 * radius + (i as f32 % 3.0) * 0.003 * radius,
                    6,
                    |_| Color32::WHITE,
                );
            }
        }
        Scene::Storm => {
            let at = center + Vec2::new(0.05 * aperture, -0.25 * aperture);
            painter.line_segment(
                [at, at + Vec2::new(-0.04 * radius, 0.1 * radius)],
                Stroke::new(2.0, Color32::from_rgb(0xff, 0xe0, 0x6a)),
            );
        }
        _ => {}
    }
}

fn scene_now(weather: &WeatherReport, now: DateTime<Local>) -> Scene {
    let night = now.hour() < 6 || now.hour() >= 20;
    match weather.code {
        Some(0) if night => Scene::ClearNight,
        Some(0) => Scene::ClearDay,
        _ => weather.scene(),
    }
}

fn cloud(painter: &Painter, origin: Pos2, radius: f32, color: Color32) {
    for (dx, dy, s) in [
        (0.0, 0.0, 0.045),
        (-0.04, 0.01, 0.032),
        (0.042, 0.012, 0.03),
        (0.0, 0.02, 0.038),
    ] {
        paint_disc(
            painter,
            origin + Vec2::new(dx * radius, dy * radius),
            s * radius,
            10,
            |_| color,
        );
    }
}

fn paint_stars(painter: &Painter, center: Pos2, radius: f32) {
    let stars = [
        (12.0, 0.22, 0.008),
        (28.0, 0.50, 0.006),
        (70.0, 0.48, 0.009),
        (82.0, 0.24, 0.005),
        (55.0, 0.18, 0.004),
    ];
    for (deg, dist, size) in stars {
        let at = center + direction(deg) * (dist * radius);
        paint_disc(painter, at, size * radius, 6, |_| {
            Color32::from_rgb(0xf4, 0xf0, 0xd8)
        });
    }
}

fn paint_year_marks(painter: &Painter, center: Pos2, radius: f32, content: f32) {
    // A few blossoms, leaves, and snow dots that travel with the year wheel.
    let marks = [
        (0.00, 1.18, Color32::from_rgb(0xf4, 0xf7, 0xfa)),
        (0.04, 1.38, Color32::from_rgb(0xd5, 0xe4, 0xea)),
        (0.22, 1.22, Color32::from_rgb(0xf2, 0x8a, 0xb0)),
        (0.28, 1.40, Color32::from_rgb(0xc5, 0xe8, 0x9a)),
        (0.48, 1.25, Color32::from_rgb(0xe6, 0xc4, 0x4a)),
        (0.55, 1.42, Color32::from_rgb(0x7d, 0xc8, 0x6a)),
        (0.74, 1.20, Color32::from_rgb(0xd4, 0x52, 0x2a)),
        (0.80, 1.40, Color32::from_rgb(0xc4, 0x78, 0x28)),
    ];
    for (shown, dist, color) in marks {
        let deg = content_angle(content, shown, YEAR_WEDGE);
        if !in_opening(deg, dist, YEAR_WEDGE) {
            continue;
        }
        paint_disc(
            painter,
            center + direction(deg) * (dist * radius),
            0.035 * radius,
            8,
            |_| color,
        );
    }
}

fn paint_sky_marks(painter: &Painter, center: Pos2, radius: f32, content: f32) {
    let sun = content_angle(content, 0.5, SKY_WEDGE);
    if in_opening(sun, 1.32, SKY_WEDGE) {
        paint_disc(
            painter,
            center + direction(sun) * (1.32 * radius),
            0.06 * radius,
            10,
            |_| Color32::from_rgb(0xff, 0xf2, 0xb0),
        );
    }
    for shown in [0.02, 0.08, 0.14, 0.88, 0.93, 0.97] {
        let deg = content_angle(content, shown, SKY_WEDGE);
        if in_opening(deg, 1.28, SKY_WEDGE) {
            paint_disc(
                painter,
                center + direction(deg) * (1.28 * radius),
                0.012 * radius,
                5,
                |_| Color32::from_rgb(0xf4, 0xf0, 0xd8),
            );
        }
    }
}

fn content_angle(content: f32, shown: f32, wedge: (f32, f32)) -> f32 {
    let middle = wedge.0 + (wedge.1 - wedge.0) * 0.5;
    middle + (shown - content) * 360.0
}

fn angle_in(deg: f32, wedge: (f32, f32)) -> bool {
    let span = (wedge.1 - wedge.0).rem_euclid(360.0);
    (deg - wedge.0).rem_euclid(360.0) <= span
}

fn in_opening(deg: f32, dist: f32, wedge: (f32, f32)) -> bool {
    angle_in(deg, wedge) && dist > 1.03 && dist < layout::window_outer(wedge, deg) - 0.03
}

fn paint_moon_disc(painter: &Painter, center: Pos2, radius: f32, phase: f64) {
    let origin = center + direction(45.0) * (0.36 * radius);
    let moon_r = 0.11 * radius;
    // Northern hemisphere: waxing lights the right side (positive screen X is east on the disc).
    let lit = Color32::from_rgb(0xf3, 0xec, 0xd4);
    let dark = Color32::from_rgb(0x2a, 0x32, 0x40);
    paint_disc(painter, origin, moon_r, 28, |p| {
        let n = (p - origin) / moon_r;
        let nx = n.x.clamp(-1.0, 1.0);
        let ny = n.y.clamp(-1.0, 1.0);
        let s = (1.0 - ny * ny).max(0.0).sqrt();
        let edge = (phase * std::f64::consts::TAU).cos() as f32 * s;
        let waxing = (phase * std::f64::consts::TAU).sin() >= 0.0;
        let illuminated = if waxing { nx >= edge } else { nx <= -edge };
        if illuminated {
            lit
        } else {
            dark
        }
    });
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
        now.day().to_string(),
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
        weather.condition.clone(),
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
    let titles = ["Calendar", "Forecast", "Sun & moon"];
    for (panel, title) in layout.boxes.iter().zip(titles) {
        paint_panel(painter, *panel);
        let inner = painter.with_clip_rect(inset(*panel, 5.0));
        inner.text(
            panel.left_top() + Vec2::new(10.0, 8.0),
            Align2::LEFT_TOP,
            title,
            FontId::new((0.08 * layout.radius).clamp(22.0, 32.0), serif()),
            TITLE,
        );
        let body_top = panel.top() + (0.12 * layout.radius).clamp(36.0, 48.0);
        match title {
            "Calendar" => paint_calendar(&inner, *panel, body_top, layout.radius, model),
            "Forecast" => paint_forecast(&inner, *panel, body_top, layout.radius, model),
            _ => paint_sun(&inner, *panel, body_top, layout.radius, model),
        }
    }
}

fn paint_calendar(painter: &Painter, panel: Rect, top: f32, radius: f32, model: &KioskModel) {
    let today = model.now.date_naive();
    let rows = events::upcoming(&model.events, today, 4);
    let size = (0.07 * radius).clamp(20.0, 28.0);
    let width = (panel.width() - 24.0).max(40.0);
    if rows.is_empty() {
        paint_wrapped(
            painter,
            Pos2::new(panel.left() + 12.0, top),
            width,
            "Nothing on the calendar",
            size,
            INK,
        );
        return;
    }
    let mut y = top;
    for event in rows.iter().take(4) {
        let title = paint_wrapped(
            painter,
            Pos2::new(panel.left() + 12.0, y),
            width,
            &event.title,
            size,
            INK,
        );
        y += title.height() + 2.0;
        let when = paint_wrapped(
            painter,
            Pos2::new(panel.left() + 12.0, y),
            width,
            &events::short_when(event),
            size * 0.82,
            INK_DIM,
        );
        y += when.height() + size * 0.45;
    }
}

fn paint_forecast(painter: &Painter, panel: Rect, top: f32, radius: f32, model: &KioskModel) {
    let size = (0.064 * radius).clamp(20.0, 26.0);
    if model.weather.daily.is_empty() {
        painter.text(
            Pos2::new(panel.left() + 12.0, top),
            Align2::LEFT_TOP,
            "Forecast unavailable",
            FontId::new(size, serif()),
            INK,
        );
        return;
    }
    let unit = match model.weather.units {
        crate::settings::Units::Imperial => "°",
        crate::settings::Units::Metric => "°",
    };
    for (i, day) in model.weather.daily.iter().take(5).enumerate() {
        let y = top + i as f32 * (size + 8.0);
        let line = format!(
            "{}   {}   {:.0}{unit}/{:.0}{unit}",
            weather::weekday_short(&day.date),
            brief_condition(day.code),
            day.high,
            day.low,
        );
        painter.text(
            Pos2::new(panel.left() + 12.0, y),
            Align2::LEFT_TOP,
            line,
            FontId::new(size, serif()),
            INK,
        );
    }
}

fn paint_sun(painter: &Painter, panel: Rect, top: f32, radius: f32, model: &KioskModel) {
    let size = (0.07 * radius).clamp(20.0, 28.0);
    let color = INK;
    let date = model.now.date_naive();
    let sun = sky::sun_times(date, model.place.latitude, model.place.longitude);
    let (rise, set) = match sun {
        Some(times) => (
            sky::format_local_hm(times.sunrise_unix),
            sky::format_local_hm(times.sunset_unix),
        ),
        None => ("—".into(), "—".into()),
    };
    let lines = [
        format!("Sunrise  {rise}"),
        format!("Sunset   {set}"),
        format!("{}", season_name(date)),
        format!("Moon  {}", moon_name(sky::moon_phase(model.now))),
    ];
    let width = (panel.width() - 24.0).max(8.0);
    let mut y = top;
    for line in &lines {
        let drawn = paint_wrapped(
            painter,
            Pos2::new(panel.left() + 12.0, y),
            width,
            line,
            size,
            color,
        );
        y = drawn.bottom() + 8.0;
    }
}

fn paint_status(painter: &Painter, layout: &KioskLayout, model: &KioskModel) {
    let bar = layout.status;
    painter.rect_filled(bar, 0.0, Color32::from_rgb(0x2a, 0x1c, 0x12));
    painter.rect_stroke(
        bar,
        0.0,
        Stroke::new(1.0, shade_bezel(Vec2::new(0.0, -1.0), 0.5)),
        egui::StrokeKind::Inside,
    );
    let size = layout::status_font_size(layout.radius);
    let width = (bar.width() - 24.0).max(8.0);
    let galley = painter.layout(model.status.text(), FontId::new(size, serif()), INK, width);
    let y = bar.top() + ((bar.height() - galley.size().y) * 0.5).max(4.0);
    painter.galley(Pos2::new(bar.left() + 12.0, y), galley, INK);
}

fn paint_panel(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, 0.0, Color32::from_rgb(0x1a, 0x12, 0x0c));
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(2.0, shade_bezel(Vec2::new(-0.2, -0.8), 0.35)),
        egui::StrokeKind::Inside,
    );
}

const INK: Color32 = Color32::from_rgb(0xff, 0xf8, 0xea);
const INK_DIM: Color32 = Color32::from_rgb(0xf3, 0xdc, 0xa8);
const TITLE: Color32 = Color32::from_rgb(0xff, 0xe0, 0x7a);

fn paint_wrapped(
    painter: &Painter,
    pos: Pos2,
    width: f32,
    text: &str,
    size: f32,
    color: Color32,
) -> Rect {
    let galley = painter.layout(text.to_owned(), FontId::new(size, serif()), color, width);
    let rect = Rect::from_min_size(pos, galley.size());
    painter.galley(pos, galley, color);
    rect
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

fn inset(rect: Rect, margin: f32) -> Rect {
    Rect::from_min_max(
        rect.min + Vec2::splat(margin),
        rect.max - Vec2::splat(margin),
    )
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
