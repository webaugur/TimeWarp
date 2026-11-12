//! Full kiosk: gears, mural wedges, aperture discs, brass dial, info boxes, status bar.
//!
//! Paint order is back to front. Hands, numerals, ticks, and the brass ring are drawn after
//! the discs they overlap. Every region is placed from the clock center and the face radius,
//! so moving or resizing the face moves the complications with it.
//!
//! The year wheel (lower left) turns once per tropical year. The sky wheel (lower right)
//! turns once per local civil day. Sampling is
//! `shown = content + (angle - wedge_center) / 360`, so the middle of each wedge shows the
//! current season or time of day and the mural eases instead of snapping.

use chrono::{DateTime, Datelike, Local, Timelike};

use egui::{Align2, Color32, FontFamily, FontId, Painter, Pos2, Rect, Stroke, Vec2};

use crate::brass::{mix, shade_bezel, shade_dial};
use crate::draw::{paint_annulus, paint_disc, paint_pie, stroke_arc};
use crate::events::{self, CalendarEvent};
use crate::gears;
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
    let seconds = model.now.timestamp() as f64 + model.now.timestamp_subsec_nanos() as f64 / 1e9;

    gears::paint(painter, center, radius, seconds);
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
    paint_aperture(painter, center, aperture, 270.0, 360.0, |p| {
        weather_color(&model.weather, model.now, center, aperture, p)
    });
    paint_aperture(painter, center, aperture, 0.0, 90.0, |p| {
        moon_color(center, aperture, p, sky::moon_phase(model.now))
    });
    paint_aperture(painter, center, aperture, 90.0, 270.0, |p| {
        date_disc_color(center, aperture, p)
    });

    paint_weather_glyphs(painter, center, radius, &model.weather, model.now);
    paint_moon_disc(painter, center, radius, sky::moon_phase(model.now));
    paint_stars(painter, center, radius);
    paint_date_words(painter, center, radius, model.now);

    // Brass annulus over the discs, then the bezel, then the markings and hands.
    paint_annulus(painter, center, aperture, radius, 96, |p| {
        shade_dial(center, p, radius, 1.0)
    });
    roman::paint_bezel(painter, center, radius);
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
        let outer = layout::wedge_outer(wedge.0, wedge.1, deg);
        let inner_p = layout.point(deg, 1.0);
        let outer_p = layout.point(deg, outer);
        crate::draw::vertex_color(&mut mesh, inner_p, color_at(deg, 0.0));
        crate::draw::vertex_color(&mut mesh, outer_p, color_at(deg, (outer - 1.0) / 0.55));
    }
    for i in 0..steps {
        let base = i * 2;
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base + 1, base + 3, base + 2);
    }
    crate::draw::add_mesh(painter, mesh);
}

fn paint_wedge_rim(painter: &Painter, center: Pos2, radius: f32, wedge: (f32, f32)) {
    let steps = 28u32;
    let span = (wedge.1 - wedge.0).rem_euclid(360.0);
    let mut points = Vec::with_capacity(steps as usize + 1);
    for i in 0..=steps {
        let deg = wedge.0 + span * (i as f32 / steps as f32);
        let outer = layout::wedge_outer(wedge.0, wedge.1, deg);
        points.push(center + direction(deg) * (outer * radius));
    }
    let brass = shade_bezel(direction((wedge.0 + wedge.1) * 0.5), 0.4);
    painter.add(egui::Shape::line(
        points,
        Stroke::new(radius * 0.018, brass),
    ));
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

fn weather_color(
    weather: &WeatherReport,
    now: DateTime<Local>,
    center: Pos2,
    radius: f32,
    point: Pos2,
) -> Color32 {
    let rel = (point - center) / radius.max(1.0);
    let sky = match scene_now(weather, now) {
        Scene::ClearDay => mix(
            Color32::from_rgb(0x7e, 0xc8, 0xef),
            Color32::from_rgb(0xc8, 0xe6, 0xf8),
            rel.y.abs(),
        ),
        Scene::ClearNight => Color32::from_rgb(0x0c, 0x14, 0x2c),
        Scene::Cloud | Scene::Unknown => mix(
            Color32::from_rgb(0x8a, 0x96, 0xa4),
            Color32::from_rgb(0xc5, 0xcc, 0xd4),
            0.4,
        ),
        Scene::Rain => Color32::from_rgb(0x4e, 0x62, 0x74),
        Scene::Snow => mix(
            Color32::from_rgb(0xb7, 0xc6, 0xd4),
            Color32::from_rgb(0xe7, 0xee, 0xf2),
            0.5,
        ),
        Scene::Storm => Color32::from_rgb(0x2c, 0x34, 0x44),
        Scene::Fog => Color32::from_rgb(0xc8, 0xcc, 0xce),
    };
    sky
}

fn moon_color(_center: Pos2, _radius: f32, _point: Pos2, _phase: f64) -> Color32 {
    // Flat night. Stars are separate discs: a cream vertex on this fan would
    // be interpolated into a ray across the whole quarter.
    Color32::from_rgb(0x0b, 0x12, 0x22)
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

fn paint_weather_glyphs(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    weather: &WeatherReport,
    now: DateTime<Local>,
) {
    let origin = center + direction(315.0) * (0.34 * radius);
    let clip = Rect::from_center_size(origin, Vec2::splat(0.42 * radius));
    let painter = painter.with_clip_rect(clip);
    let r = radius;
    let ink = match scene_now(weather, now) {
        Scene::ClearNight | Scene::Rain | Scene::Storm => Color32::from_rgb(0xf4, 0xec, 0xd8),
        _ => Color32::from_rgb(0x2a, 0x1c, 0x10),
    };
    match scene_now(weather, now) {
        Scene::ClearDay => {
            paint_disc(
                &painter,
                origin + Vec2::new(-0.02 * r, -0.02 * r),
                0.055 * r,
                16,
                |_| Color32::from_rgb(0xff, 0xc8, 0x3a),
            );
        }
        Scene::ClearNight => {
            paint_disc(&painter, origin, 0.05 * r, 16, |_| {
                Color32::from_rgb(0xf0, 0xea, 0xc8)
            });
        }
        Scene::Cloud | Scene::Unknown | Scene::Fog => {
            cloud(&painter, origin, r, Color32::from_rgb(0xf4, 0xf6, 0xf8));
        }
        Scene::Rain => {
            cloud(
                &painter,
                origin + Vec2::new(0.0, -0.02 * r),
                r,
                Color32::from_rgb(0xd5, 0xdc, 0xe2),
            );
            for i in 0..4 {
                let x = (i as f32 - 1.5) * 0.03 * r;
                painter.line_segment(
                    [
                        origin + Vec2::new(x, 0.04 * r),
                        origin + Vec2::new(x - 0.01 * r, 0.09 * r),
                    ],
                    Stroke::new(1.4, Color32::from_rgb(0x7e, 0xb6, 0xd8)),
                );
            }
        }
        Scene::Snow => {
            cloud(
                &painter,
                origin + Vec2::new(0.0, -0.03 * r),
                r,
                Color32::from_rgb(0xee, 0xf2, 0xf4),
            );
            for (i, (dx, dy)) in [(0.0, 0.05), (-0.04, 0.08), (0.045, 0.075), (0.01, 0.1)]
                .iter()
                .enumerate()
            {
                paint_disc(
                    &painter,
                    origin + Vec2::new(*dx * r, *dy * r),
                    0.012 * r + (i as f32) * 0.001 * r,
                    8,
                    |_| Color32::WHITE,
                );
            }
        }
        Scene::Storm => {
            cloud(&painter, origin, r, Color32::from_rgb(0x6a, 0x74, 0x82));
            painter.line_segment(
                [
                    origin + Vec2::new(0.01 * r, 0.02 * r),
                    origin + Vec2::new(-0.02 * r, 0.09 * r),
                ],
                Stroke::new(2.0, Color32::from_rgb(0xff, 0xe0, 0x6a)),
            );
        }
    }
    let title = weather.temperature_text();
    painter.text(
        origin + Vec2::new(0.0, 0.13 * r),
        Align2::CENTER_TOP,
        title,
        FontId::new(0.07 * r, serif()),
        ink,
    );
    painter.text(
        origin + Vec2::new(0.0, -0.14 * r),
        Align2::CENTER_BOTTOM,
        weather.condition.clone(),
        FontId::new(0.038 * r, serif()),
        ink,
    );
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
        if !angle_in(deg, YEAR_WEDGE) {
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
    if angle_in(sun, SKY_WEDGE) {
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
        if angle_in(deg, SKY_WEDGE) {
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

fn paint_date_words(painter: &Painter, center: Pos2, radius: f32, now: DateTime<Local>) {
    let origin = center + direction(180.0) * (0.38 * radius);
    let color = Color32::from_rgb(0x3a, 0x28, 0x16);
    let weekday = weekday_name(now.weekday().num_days_from_monday());
    let month = month_name(now.month());
    painter.text(
        origin + Vec2::new(0.0, -0.07 * radius),
        Align2::CENTER_CENTER,
        weekday,
        FontId::new(0.055 * radius, serif()),
        color,
    );
    painter.text(
        origin,
        Align2::CENTER_CENTER,
        month,
        FontId::new(0.05 * radius, serif()),
        color,
    );
    painter.text(
        origin + Vec2::new(0.0, 0.08 * radius),
        Align2::CENTER_CENTER,
        now.day().to_string(),
        FontId::new(0.09 * radius, serif()),
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
    for deg in [0.0, 90.0, 270.0] {
        let inner = center + direction(deg) * (0.12 * radius);
        let outer = center + direction(deg) * (APERTURE_R * radius);
        painter.line_segment([inner, outer], Stroke::new(width, brass));
    }
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
            FontId::new((0.055 * layout.radius).clamp(13.0, 22.0), serif()),
            Color32::from_rgb(0xe6, 0xc2, 0x7a),
        );
        let body_top = panel.top() + (0.10 * layout.radius).clamp(26.0, 36.0);
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
    let size = (0.042 * radius).clamp(12.0, 18.0);
    if rows.is_empty() {
        painter.text(
            Pos2::new(panel.left() + 10.0, top),
            Align2::LEFT_TOP,
            "Nothing on the calendar",
            FontId::new(size, serif()),
            Color32::from_rgb(0xf3, 0xe6, 0xcf),
        );
        return;
    }
    for (i, event) in rows.iter().enumerate() {
        let y = top + i as f32 * (size * 2.15);
        painter.text(
            Pos2::new(panel.left() + 10.0, y),
            Align2::LEFT_TOP,
            event.title.clone(),
            FontId::new(size, serif()),
            Color32::from_rgb(0xf3, 0xe6, 0xcf),
        );
        painter.text(
            Pos2::new(panel.left() + 10.0, y + size * 0.85),
            Align2::LEFT_TOP,
            events::short_when(event),
            FontId::new(size * 0.78, serif()),
            Color32::from_rgb(0xc8, 0xb4, 0x8e),
        );
    }
}

fn paint_forecast(painter: &Painter, panel: Rect, top: f32, radius: f32, model: &KioskModel) {
    let size = (0.04 * radius).clamp(12.0, 17.0);
    if model.weather.daily.is_empty() {
        painter.text(
            Pos2::new(panel.left() + 10.0, top),
            Align2::LEFT_TOP,
            "Forecast unavailable",
            FontId::new(size, serif()),
            Color32::from_rgb(0xf3, 0xe6, 0xcf),
        );
        return;
    }
    let unit = match model.weather.units {
        crate::settings::Units::Imperial => "°",
        crate::settings::Units::Metric => "°",
    };
    for (i, day) in model.weather.daily.iter().take(5).enumerate() {
        let y = top + i as f32 * (size + 4.0);
        let line = format!(
            "{}   {}   {:.0}{unit} / {:.0}{unit}",
            weather::weekday_short(&day.date),
            weather::condition_name(day.code),
            day.high,
            day.low,
        );
        painter.text(
            Pos2::new(panel.left() + 10.0, y),
            Align2::LEFT_TOP,
            line,
            FontId::new(size, serif()),
            Color32::from_rgb(0xf3, 0xe6, 0xcf),
        );
    }
}

fn paint_sun(painter: &Painter, panel: Rect, top: f32, radius: f32, model: &KioskModel) {
    let size = (0.042 * radius).clamp(12.0, 18.0);
    let color = Color32::from_rgb(0xf3, 0xe6, 0xcf);
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
    for (i, line) in lines.iter().enumerate() {
        painter.text(
            Pos2::new(panel.left() + 10.0, top + i as f32 * (size + 6.0)),
            Align2::LEFT_TOP,
            line,
            FontId::new(size, serif()),
            color,
        );
    }
    let _ = panel;
}

fn paint_status(painter: &Painter, layout: &KioskLayout, model: &KioskModel) {
    let bar = layout.status;
    painter.rect_filled(bar, 0.0, Color32::from_rgb(0x14, 0x0e, 0x0a));
    painter.rect_stroke(
        bar,
        0.0,
        Stroke::new(1.0, shade_bezel(Vec2::new(0.0, -1.0), 0.5)),
        egui::StrokeKind::Inside,
    );
    painter.text(
        bar.left_center() + Vec2::new(12.0, 0.0),
        Align2::LEFT_CENTER,
        model.status.text(),
        FontId::new((0.04 * layout.radius).clamp(12.0, 16.0), serif()),
        Color32::from_rgb(0xf3, 0xe6, 0xcf),
    );
}

fn paint_panel(painter: &Painter, rect: Rect) {
    painter.rect_filled(rect, 0.0, Color32::from_rgb(0x1c, 0x14, 0x0e));
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.6, shade_bezel(Vec2::new(-0.2, -0.8), 0.55)),
        egui::StrokeKind::Inside,
    );
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
