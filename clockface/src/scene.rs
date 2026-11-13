//! Layered SVG horizon for the upper half of the dial.
//!
//! The shipped theme is `scenes/default/`. `CLOCKFACE_SCENE` may point at another
//! folder with the same `scene.svg` and `theme.toml`. Layers are groups in the SVG.
//! This module turns them on or off by rewriting `display` on those groups, then
//! rasterizes with resvg. The moon is not a layer: it is shaded here so the phase
//! stays correct. See `scenes/CREATING_A_THEME.md`.

use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Mutex;

use chrono::{DateTime, Local};
use egui::{Color32, Context, Id, Mesh, Painter, Pos2, Rect, TextureOptions, Vec2};

use crate::roman::direction;
use crate::sky::{self, season_name};
use crate::weather::{Scene, WeatherReport};

const DEFAULT_SVG: &str = include_str!("../scenes/default/scene.svg");
const DEFAULT_TOML: &str = include_str!("../scenes/default/theme.toml");

#[derive(Clone, Debug)]
pub struct Theme {
    pub name: String,
    svg: String,
    manifest: Manifest,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct Manifest {
    #[serde(default)]
    always: Vec<String>,
    sky: SkyIds,
    season: SeasonIds,
    weather: WeatherIds,
    time: TimeIds,
    #[serde(default)]
    hide_sun: Vec<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct SkyIds {
    day: String,
    twilight: String,
    night: String,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct SeasonIds {
    leaves: SeasonFour,
    ground: SeasonFour,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct SeasonFour {
    spring: String,
    summer: String,
    autumn: String,
    winter: String,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
struct WeatherIds {
    #[serde(default)]
    clouds: Vec<String>,
    #[serde(default)]
    rain: Vec<String>,
    #[serde(default)]
    snow: Vec<String>,
    #[serde(default)]
    storm: Vec<String>,
    #[serde(default)]
    fog: Vec<String>,
    #[serde(default)]
    unknown: Vec<String>,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
struct TimeIds {
    #[serde(default)]
    day: Vec<String>,
    #[serde(default)]
    twilight: Vec<String>,
    night: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SkyBand {
    Day,
    Twilight,
    Night,
}

impl Theme {
    fn from_parts(svg: &str, toml_text: &str) -> Result<Self, String> {
        let file: Manifest = toml::from_str(toml_text).map_err(|err| err.to_string())?;
        let name = toml::from_str::<toml::Value>(toml_text)
            .ok()
            .and_then(|v| v.get("name").and_then(|n| n.as_str()).map(|s| s.to_owned()))
            .unwrap_or_else(|| "theme".into());
        Ok(Self {
            name,
            svg: svg.to_owned(),
            manifest: file,
        })
    }
}

/// Embedded default, or the folder in `CLOCKFACE_SCENE` when that folder loads.
pub fn theme() -> Theme {
    if let Ok(dir) = std::env::var("CLOCKFACE_SCENE") {
        let path = Path::new(&dir);
        match load_dir(path) {
            Ok(theme) => return theme,
            Err(err) => eprintln!("clockface: scenery theme {}: {err}", path.display()),
        }
    }
    Theme::from_parts(DEFAULT_SVG, DEFAULT_TOML).expect("default scenery theme")
}

fn load_dir(dir: &Path) -> Result<Theme, String> {
    let svg = std::fs::read_to_string(dir.join("scene.svg"))
        .map_err(|err| format!("scene.svg: {err}"))?;
    let toml_text = std::fs::read_to_string(dir.join("theme.toml"))
        .map_err(|err| format!("theme.toml: {err}"))?;
    Theme::from_parts(&svg, &toml_text)
}

pub fn sky_band(now: DateTime<Local>) -> SkyBand {
    let day = sky::day_fraction(now);
    let alt = ((day - 0.25) * std::f32::consts::TAU).sin();
    let dayness = alt.clamp(0.0, 1.0);
    let twilight = (1.0 - alt.abs() * 2.4).clamp(0.0, 1.0);
    if dayness < 0.08 {
        SkyBand::Night
    } else if twilight > 0.55 && dayness < 0.35 {
        SkyBand::Twilight
    } else {
        SkyBand::Day
    }
}

/// Group ids that should be visible for this instant.
pub fn visible_layers(
    theme: &Theme,
    now: DateTime<Local>,
    weather: &WeatherReport,
) -> BTreeSet<String> {
    let manifest = &theme.manifest;
    let mut on = BTreeSet::new();
    on.extend(manifest.always.iter().cloned());

    let sky = match sky_band(now) {
        SkyBand::Day => &manifest.sky.day,
        SkyBand::Twilight => &manifest.sky.twilight,
        SkyBand::Night => &manifest.sky.night,
    };
    on.insert(sky.clone());

    let (leaves, ground) = match season_name(now.date_naive()) {
        "Spring" => (
            &manifest.season.leaves.spring,
            &manifest.season.ground.spring,
        ),
        "Summer" => (
            &manifest.season.leaves.summer,
            &manifest.season.ground.summer,
        ),
        "Autumn" => (
            &manifest.season.leaves.autumn,
            &manifest.season.ground.autumn,
        ),
        _ => (
            &manifest.season.leaves.winter,
            &manifest.season.ground.winter,
        ),
    };
    on.insert(leaves.clone());
    on.insert(ground.clone());

    let scene = weather.scene();
    let weather_layers: &[String] = match scene {
        Scene::Cloud => &manifest.weather.clouds,
        Scene::Rain => &manifest.weather.rain,
        Scene::Snow => &manifest.weather.snow,
        Scene::Storm => &manifest.weather.storm,
        Scene::Fog => &manifest.weather.fog,
        Scene::Unknown => &manifest.weather.unknown,
        Scene::ClearDay | Scene::ClearNight => &[],
    };
    on.extend(weather_layers.iter().cloned());

    let time_layers: &[String] = match sky_band(now) {
        SkyBand::Day => &manifest.time.day,
        SkyBand::Twilight => &manifest.time.twilight,
        SkyBand::Night => &manifest.time.night,
    };
    let scene_name = match scene {
        Scene::Storm => "storm",
        Scene::Fog => "fog",
        Scene::Rain => "rain",
        Scene::Snow => "snow",
        Scene::Cloud => "clouds",
        Scene::Unknown => "unknown",
        Scene::ClearDay | Scene::ClearNight => "clear",
    };
    if !manifest.hide_sun.iter().any(|name| name == scene_name) {
        on.extend(time_layers.iter().cloned());
    } else if sky_band(now) == SkyBand::Night {
        on.extend(manifest.time.night.iter().cloned());
    }
    on
}

fn catalog(manifest: &Manifest) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    ids.extend(manifest.always.iter().cloned());
    ids.insert(manifest.sky.day.clone());
    ids.insert(manifest.sky.twilight.clone());
    ids.insert(manifest.sky.night.clone());
    for id in [
        &manifest.season.leaves.spring,
        &manifest.season.leaves.summer,
        &manifest.season.leaves.autumn,
        &manifest.season.leaves.winter,
        &manifest.season.ground.spring,
        &manifest.season.ground.summer,
        &manifest.season.ground.autumn,
        &manifest.season.ground.winter,
    ] {
        ids.insert(id.clone());
    }
    for list in [
        &manifest.weather.clouds,
        &manifest.weather.rain,
        &manifest.weather.snow,
        &manifest.weather.storm,
        &manifest.weather.fog,
        &manifest.weather.unknown,
        &manifest.time.day,
        &manifest.time.twilight,
        &manifest.time.night,
    ] {
        ids.extend(list.iter().cloned());
    }
    ids
}

/// Set `display` on catalogued groups. `moon` is always hidden; the app draws the phase.
pub fn apply_visibility(
    svg: &str,
    catalog: &BTreeSet<String>,
    visible: &BTreeSet<String>,
) -> String {
    let mut out = String::with_capacity(svg.len() + 128);
    let mut rest = svg;
    while let Some(start) = rest.find("<g") {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        let Some(end) = after.find('>') else {
            out.push_str(after);
            return out;
        };
        let tag = &after[..=end];
        if let Some(id) = group_id(tag) {
            if id == "moon" || catalog.contains(id) {
                let show = id != "moon" && visible.contains(id);
                out.push_str(&with_display(tag, show));
            } else {
                out.push_str(tag);
            }
        } else {
            out.push_str(tag);
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

fn group_id(tag: &str) -> Option<&str> {
    let key = "id=\"";
    let start = tag.find(key)? + key.len();
    let end = tag[start..].find('"')? + start;
    Some(&tag[start..end])
}

fn with_display(tag: &str, show: bool) -> String {
    let value = if show { "inline" } else { "none" };
    let body = tag.trim_end_matches('>').trim_end();
    let body = match body.find("display=\"") {
        Some(at) => {
            let after = at + "display=\"".len();
            let end = body[after..]
                .find('"')
                .map(|i| after + i)
                .unwrap_or(body.len());
            format!("{}{value}{}", &body[..after], &body[end..])
        }
        None => format!("{body} display=\"{value}\""),
    };
    format!("{body}>")
}

struct CachedPixels {
    hash: u64,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

fn render_svg(svg: &str, width: u32, height: u32) -> Result<Vec<u8>, String> {
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg, &options).map_err(|err| err.to_string())?;
    let mut pixmap = tiny_skia::Pixmap::new(width, height).ok_or("pixmap")?;
    let size = tree.size();
    let sx = width as f32 / size.width();
    let sy = height as f32 / size.height();
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(sx, sy),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap.take())
}

fn cached_rgba(
    theme: &Theme,
    visible: &BTreeSet<String>,
    width: u32,
    height: u32,
) -> Result<Vec<u8>, String> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    theme.name.hash(&mut hasher);
    theme.svg.hash(&mut hasher);
    visible.hash(&mut hasher);
    width.hash(&mut hasher);
    height.hash(&mut hasher);
    let hash = hasher.finish();

    static CACHE: Mutex<Option<CachedPixels>> = Mutex::new(None);
    if let Ok(guard) = CACHE.lock() {
        if let Some(hit) = guard.as_ref() {
            if hit.hash == hash && hit.width == width && hit.height == height {
                return Ok(hit.rgba.clone());
            }
        }
    }

    let svg = apply_visibility(&theme.svg, &catalog(&theme.manifest), visible);
    let rgba = render_svg(&svg, width, height)?;
    if let Ok(mut guard) = CACHE.lock() {
        *guard = Some(CachedPixels {
            hash,
            width,
            height,
            rgba: rgba.clone(),
        });
    }
    Ok(rgba)
}

/// Rasterize the theme into the upper-half aperture. The brass ring covers the
/// square corners outside the circle.
pub fn paint_horizon(
    painter: &Painter,
    ctx: &Context,
    center: Pos2,
    aperture: f32,
    now: DateTime<Local>,
    weather: &WeatherReport,
) {
    let theme = theme();
    let visible = visible_layers(&theme, now, weather);
    let width = (aperture * 2.0).round().clamp(64.0, 1200.0) as u32;
    let height = (width / 2).max(32);
    let rect = Rect::from_min_size(
        Pos2::new(center.x - aperture, center.y - aperture),
        Vec2::new(aperture * 2.0, aperture),
    );
    let clip = Rect::from_min_max(
        Pos2::new(center.x - aperture, center.y - aperture),
        Pos2::new(center.x + aperture, center.y + 1.0),
    );
    let painter = painter.with_clip_rect(clip);
    match cached_rgba(&theme, &visible, width, height) {
        Ok(rgba) => {
            let image =
                egui::ColorImage::from_rgba_premultiplied([width as usize, height as usize], &rgba);
            let texture = ctx.load_texture_cached(
                "clockface-horizon",
                Id::unique("clockface-horizon"),
                image,
                TextureOptions::LINEAR,
            );
            painter.image(
                texture.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        Err(err) => {
            eprintln!("clockface: scenery render: {err}");
        }
    }
    let night = sky_band(now) == SkyBand::Night;
    paint_moon(
        &painter,
        center,
        aperture / crate::layout::APERTURE_R,
        sky::moon_phase(now),
        night,
    );
}

/// Shaded moon at the historical spot: 45° from 12, 0.36 of the face radius out.
///
/// By day the unlit side is clear, the way a crescent sits in a blue sky. At night
/// that side stays up as earthshine so the disc is still there.
pub fn paint_moon(painter: &Painter, center: Pos2, face_radius: f32, phase: f64, night: bool) {
    let origin = center + direction(45.0) * (0.36 * face_radius);
    let moon_r = 0.11 * face_radius;
    if moon_r < 1.0 {
        return;
    }
    let rings = 28u32;
    let segs = 56u32;
    let mut mesh = Mesh::default();
    for ring in 0..=rings {
        let t = ring as f32 / rings as f32;
        for seg in 0..=segs {
            let ang = seg as f32 / segs as f32 * std::f32::consts::TAU;
            let nx = ang.cos() * t;
            let ny = ang.sin() * t;
            let p = origin + Vec2::new(nx, ny) * moon_r;
            let color = moon_color(nx, ny, t, phase, night);
            mesh.colored_vertex(p, color);
        }
    }
    let stride = segs + 1;
    for ring in 0..rings {
        for seg in 0..segs {
            let a = ring * stride + seg;
            let b = a + 1;
            let c = a + stride;
            let d = c + 1;
            mesh.add_triangle(a, b, c);
            mesh.add_triangle(b, d, c);
        }
    }
    if mesh.indices.len() >= 3 {
        painter.add(egui::Shape::mesh(mesh));
    }
}

fn moon_color(nx: f32, ny: f32, radius: f32, phase: f64, night: bool) -> Color32 {
    if radius > 1.0 {
        return Color32::TRANSPARENT;
    }
    let alb = moon_albedo(nx, ny);
    let mu = (1.0 - radius * radius).max(0.0).sqrt();
    let limb = 0.38 + 0.62 * mu.powf(0.5);
    let s = (1.0 - ny * ny).max(0.0).sqrt();
    let edge = (phase * std::f64::consts::TAU).cos() as f32 * s;
    let waxing = (phase * std::f64::consts::TAU).sin() >= 0.0;
    let signed = if waxing { nx - edge } else { -nx - edge };
    let lit = smoothstep(-0.04, 0.06, signed);
    let shade = alb * limb;
    let lit_r = 236.0 * shade;
    let lit_g = 226.0 * shade;
    let lit_b = 204.0 * shade;
    // Earthshine is bright enough to show maria at night, and absent by day.
    let earth = if night { 0.42 } else { 0.0 };
    let r = lit_r * lit + 90.0 * shade * earth * (1.0 - lit);
    let g = lit_g * lit + 98.0 * shade * earth * (1.0 - lit);
    let b = lit_b * lit + 112.0 * shade * earth * (1.0 - lit);
    let alpha = if night { 1.0 } else { lit };
    Color32::from_rgba_premultiplied(
        (r * alpha).clamp(0.0, 255.0) as u8,
        (g * alpha).clamp(0.0, 255.0) as u8,
        (b * alpha).clamp(0.0, 255.0) as u8,
        (alpha * 255.0).clamp(0.0, 255.0) as u8,
    )
}

fn moon_albedo(nx: f32, ny: f32) -> f32 {
    let mut a = 0.86;
    a *= mare(nx, ny, -0.18, -0.22, 0.42, 0.30, 0.52);
    a *= mare(nx, ny, 0.20, 0.02, 0.24, 0.16, 0.60);
    a *= mare(nx, ny, -0.02, 0.30, 0.34, 0.14, 0.66);
    a *= mare(nx, ny, 0.08, -0.38, 0.18, 0.12, 0.58);
    a = crater(a, nx, ny, 0.38, -0.12, 0.09);
    a = crater(a, nx, ny, -0.40, 0.18, 0.07);
    a = crater(a, nx, ny, 0.02, -0.48, 0.055);
    a = crater(a, nx, ny, -0.22, -0.08, 0.045);
    a = crater(a, nx, ny, 0.18, 0.32, 0.05);
    a
}

fn mare(nx: f32, ny: f32, cx: f32, cy: f32, rx: f32, ry: f32, dark: f32) -> f32 {
    let e = ((nx - cx) / rx).powi(2) + ((ny - cy) / ry).powi(2);
    if e < 1.0 {
        let t = (1.0 - e).powf(0.75);
        1.0 - t * (1.0 - dark)
    } else {
        1.0
    }
}

fn crater(albedo: f32, nx: f32, ny: f32, cx: f32, cy: f32, radius: f32) -> f32 {
    let d = ((nx - cx).powi(2) + (ny - cy).powi(2)).sqrt();
    if (d - radius).abs() < radius * 0.18 {
        (albedo + 0.16).min(1.0)
    } else if d < radius * 0.72 {
        albedo * 0.78
    } else {
        albedo
    }
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roman::screenshot_time;
    use crate::weather::fixture_january;

    #[test]
    fn january_screenshot_turns_on_winter_snow_and_day() {
        let theme = Theme::from_parts(DEFAULT_SVG, DEFAULT_TOML).unwrap();
        let on = visible_layers(&theme, screenshot_time(), &fixture_january());
        for id in [
            "land",
            "tree-trunk",
            "sky-day",
            "leaves-winter",
            "ground-winter",
            "clouds",
            "snow",
            "snow-cover",
            "sun",
        ] {
            assert!(on.contains(id), "missing {id} in {on:?}");
        }
        for id in [
            "sky-night",
            "sky-twilight",
            "stars",
            "rain",
            "storm",
            "fog",
            "leaves-spring",
            "leaves-summer",
            "leaves-autumn",
            "ground-spring",
            "ground-summer",
            "ground-autumn",
        ] {
            assert!(!on.contains(id), "{id} should be off, got {on:?}");
        }
    }

    #[test]
    fn hidden_groups_get_display_none() {
        let svg = r#"<svg><g id="sky-day"><rect/></g><g id="rain"><rect/></g></svg>"#;
        let catalog = BTreeSet::from(["sky-day".into(), "rain".into()]);
        let visible = BTreeSet::from(["sky-day".into()]);
        let out = apply_visibility(svg, &catalog, &visible);
        assert!(
            out.contains("id=\"sky-day\" display=\"inline\"") || out.contains("display=\"inline\"")
        );
        assert!(out.contains("id=\"rain\" display=\"none\""));
    }

    #[test]
    fn screenshot_moon_is_a_waning_crescent() {
        let phase = sky::moon_phase(screenshot_time());
        assert!(
            (0.78..0.97).contains(&phase),
            "expected a waning crescent, phase {phase}"
        );
        let lit = moon_color(-0.95, 0.0, 0.95, phase, false);
        let dark = moon_color(0.4, 0.0, 0.4, phase, false);
        assert!(lit.a() > 200, "western limb should be lit, got {lit:?}");
        assert_eq!(
            dark.a(),
            0,
            "daytime unlit side should be clear, got {dark:?}"
        );
        let earth = moon_color(0.2, 0.1, 0.22, phase, true);
        assert!(
            earth.a() == 255 && earth.r() > 20,
            "night earthshine, got {earth:?}"
        );
    }

    #[test]
    fn default_scene_raster_is_a_blue_day_sky() {
        let theme = Theme::from_parts(DEFAULT_SVG, DEFAULT_TOML).unwrap();
        let on = visible_layers(&theme, screenshot_time(), &fixture_january());
        let rgba = cached_rgba(&theme, &on, 200, 100).unwrap();
        // Zenith sample used by the kiosk test, scaled into this pixmap.
        let x = 100usize;
        let y = 34usize;
        let i = (y * 200 + x) * 4;
        let px = &rgba[i..i + 4];
        assert!(
            px[2] > 100 && px[2] as i32 + 10 > px[0] as i32,
            "zenith should be blue sky, got {px:?}"
        );
    }
}
