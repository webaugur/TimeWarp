//! Layered SVG horizon for the lower half of the dial.
//!
//! The shipped theme is `scenes/default/`. `CLOCKFACE_SCENE` may point at another
//! folder with the same `scene.svg` and `theme.toml`. Layers are groups in the SVG.
//! This module turns them on or off by rewriting `display` on those groups, then
//! rasterizes with resvg. The moon is not drawn here; it has its own quarter.
//! See `scenes/CREATING_A_THEME.md`.

use std::collections::BTreeSet;
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::Mutex;

use chrono::{DateTime, Local};
use egui::epaint::Vertex;
use egui::{Color32, Context, Id, Mesh, Painter, Pos2, TextureOptions};

use crate::sky::{self, season_name};
use crate::weather::{Scene, WeatherReport};

const DEFAULT_SVG: &str = include_str!("../scenes/default/scene.svg");
const DEFAULT_TOML: &str = include_str!("../scenes/default/theme.toml");

#[derive(Clone, Debug)]
pub struct Theme {
    pub name: String,
    svg: String,
    manifest: Manifest,
    /// Directory of an external theme, so relative `<image>` hrefs resolve.
    resources_dir: Option<std::path::PathBuf>,
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
    /// Photographic skies. When set, these replace the flat `sky` gradients.
    #[serde(default)]
    photos: Option<PhotoIds>,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct PhotoIds {
    clear: String,
    partly: String,
    overcast: String,
    night: String,
    #[serde(default)]
    dawn: Option<String>,
    #[serde(default)]
    dusk: Option<String>,
}

#[derive(Clone, Debug, serde::Deserialize)]
struct SkyIds {
    day: String,
    twilight: String,
    night: String,
    #[serde(default)]
    overcast: Option<String>,
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
    few: Vec<String>,
    #[serde(default)]
    partly: Vec<String>,
    #[serde(default)]
    thick: Vec<String>,
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
    Night,
    Dawn,
    Day,
    Dusk,
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
            resources_dir: None,
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
    let mut theme = Theme::from_parts(&svg, &toml_text)?;
    theme.resources_dir = Some(dir.to_path_buf());
    Ok(theme)
}

pub fn sky_band(now: DateTime<Local>) -> SkyBand {
    let day = sky::day_fraction(now);
    let alt = ((day - 0.25) * std::f32::consts::TAU).sin();
    let dayness = alt.clamp(0.0, 1.0);
    let twilight = (1.0 - alt.abs() * 2.4).clamp(0.0, 1.0);
    if dayness < 0.08 {
        SkyBand::Night
    } else if twilight > 0.55 && dayness < 0.35 {
        if day < 0.5 {
            SkyBand::Dawn
        } else {
            SkyBand::Dusk
        }
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

    let cover = weather.cloud_cover();
    let scene = weather.scene();
    let band = sky_band(now);
    if let Some(photos) = &manifest.photos {
        on.insert(photo_layer(photos, band, scene, cover).to_owned());
        let tint = match band {
            SkyBand::Dawn => photos.dawn.as_ref(),
            SkyBand::Dusk => photos.dusk.as_ref(),
            _ => None,
        };
        if let Some(id) = tint {
            on.insert(id.clone());
        }
    } else {
        let overcast =
            cover >= 75 || matches!(scene, Scene::Rain | Scene::Snow | Scene::Storm | Scene::Fog);
        let sky = match band {
            SkyBand::Night => &manifest.sky.night,
            SkyBand::Dawn | SkyBand::Dusk => &manifest.sky.twilight,
            SkyBand::Day if overcast => manifest.sky.overcast.as_ref().unwrap_or(&manifest.sky.day),
            SkyBand::Day => &manifest.sky.day,
        };
        on.insert(sky.clone());
    }

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

    let weather_layers: &[String] = match scene {
        Scene::Rain => &manifest.weather.rain,
        Scene::Snow => &manifest.weather.snow,
        Scene::Storm => &manifest.weather.storm,
        Scene::Fog => &manifest.weather.fog,
        Scene::Cloud | Scene::Unknown | Scene::ClearDay | Scene::ClearNight => {
            cloud_band(&manifest.weather, cover)
        }
    };
    on.extend(weather_layers.iter().cloned());

    let time_layers: &[String] = match band {
        SkyBand::Day => &manifest.time.day,
        SkyBand::Dawn | SkyBand::Dusk => &manifest.time.twilight,
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

fn photo_layer(photos: &PhotoIds, band: SkyBand, scene: Scene, cover: u8) -> &str {
    if band == SkyBand::Night {
        return &photos.night;
    }
    let heavy =
        cover >= 75 || matches!(scene, Scene::Rain | Scene::Snow | Scene::Storm | Scene::Fog);
    if heavy {
        &photos.overcast
    } else if cover >= 20 {
        &photos.partly
    } else {
        &photos.clear
    }
}

/// Cloud groups for a cover percentage. Themes that only list `clouds` use that
/// one group whenever the sky is not clear.
fn cloud_band(weather: &WeatherIds, cover: u8) -> &[String] {
    let banded = !weather.few.is_empty() || !weather.partly.is_empty() || !weather.thick.is_empty();
    if !banded {
        return if cover < 15 { &[] } else { &weather.clouds };
    }
    match cover {
        0..=15 => &[],
        16..=40 => &weather.few,
        41..=74 => &weather.partly,
        _ => &weather.thick,
    }
}

fn catalog(manifest: &Manifest) -> BTreeSet<String> {
    let mut ids = BTreeSet::new();
    ids.extend(manifest.always.iter().cloned());
    ids.insert(manifest.sky.day.clone());
    ids.insert(manifest.sky.twilight.clone());
    ids.insert(manifest.sky.night.clone());
    if let Some(id) = &manifest.sky.overcast {
        ids.insert(id.clone());
    }
    if let Some(photos) = &manifest.photos {
        for id in [
            &photos.clear,
            &photos.partly,
            &photos.overcast,
            &photos.night,
        ] {
            ids.insert(id.clone());
        }
        if let Some(id) = &photos.dawn {
            ids.insert(id.clone());
        }
        if let Some(id) = &photos.dusk {
            ids.insert(id.clone());
        }
    }
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
        &manifest.weather.few,
        &manifest.weather.partly,
        &manifest.weather.thick,
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

fn render_svg(
    svg: &str,
    width: u32,
    height: u32,
    resources_dir: Option<&Path>,
) -> Result<Vec<u8>, String> {
    let mut options = usvg::Options::default();
    options.resources_dir = resources_dir.map(|dir| dir.to_path_buf());
    // The shipped photographs are compiled in. An external theme can still
    // point `<image>` at a file next to its own scene.svg.
    options.image_href_resolver.resolve_string = Box::new(|href, opts| {
        if let Some(dir) = opts.resources_dir.as_ref() {
            let path = dir.join(href);
            if let Ok(data) = std::fs::read(&path) {
                return image_kind(href, data);
            }
        }
        let bundled: Option<&[u8]> = match href {
            "skies/clear.jpg" => Some(include_bytes!("../scenes/default/skies/clear.jpg")),
            "skies/partly.jpg" => Some(include_bytes!("../scenes/default/skies/partly.jpg")),
            "skies/over.jpg" => Some(include_bytes!("../scenes/default/skies/over.jpg")),
            "skies/night.jpg" => Some(include_bytes!("../scenes/default/skies/night.jpg")),
            _ => None,
        };
        bundled.and_then(|bytes| image_kind(href, bytes.to_vec()))
    });
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

fn image_kind(href: &str, data: Vec<u8>) -> Option<usvg::ImageKind> {
    let jpeg = href.ends_with(".jpg") || href.ends_with(".jpeg") || data.starts_with(&[0xff, 0xd8]);
    let png = href.ends_with(".png") || data.starts_with(b"\x89PNG");
    if jpeg {
        Some(usvg::ImageKind::JPEG(std::sync::Arc::new(data)))
    } else if png {
        Some(usvg::ImageKind::PNG(std::sync::Arc::new(data)))
    } else {
        None
    }
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
    let mut rgba = render_svg(&svg, width, height, theme.resources_dir.as_deref())?;
    // A few levels of stable noise. A long vertical gradient otherwise posts
    // as horizontal bands once it is quantized to 8 bits.
    dither(&mut rgba);
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

fn dither(rgba: &mut [u8]) {
    for (i, px) in rgba.chunks_exact_mut(4).enumerate() {
        let h = i.wrapping_mul(1664525).wrapping_add(1013904223);
        for (k, channel) in px[..3].iter_mut().enumerate() {
            let n = ((h >> (12 + k * 5)) & 15) as i32 - 7;
            *channel = (*channel as i32 + n).clamp(0, 255) as u8;
        }
    }
}

/// Rasterize the theme into the lower-half aperture.
///
/// The bitmap's top (sky) sits on the 9–3 line and the ground runs toward 6,
/// so the landscape stays upright. The mesh is the semicircle, not the square.
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
    // Twice the aperture's physical pixels. The mesh then scales down, so the
    // sky and clouds are not a small bitmap stretched to the dial.
    let ppp = ctx.pixels_per_point().clamp(1.0, 3.0);
    let width = (aperture * 2.0 * ppp * 2.0).round().clamp(128.0, 2048.0) as u32;
    let height = (width / 2).max(64);
    match cached_rgba(&theme, &visible, width, height) {
        Ok(rgba) => {
            let image =
                egui::ColorImage::from_rgba_premultiplied([width as usize, height as usize], &rgba);
            let texture = ctx.load_texture_cached(
                "clockface-horizon",
                Id::unique(("clockface-horizon", width, height)),
                image,
                TextureOptions::LINEAR,
            );
            paint_lower_half(painter, center, aperture, texture.id());
        }
        Err(err) => {
            eprintln!("clockface: scenery render: {err}");
        }
    }
}

fn paint_lower_half(painter: &Painter, center: Pos2, aperture: f32, texture: egui::TextureId) {
    // The top of the landscape, along the 9–3 line, fades to black over about
    // 4% of the face radius so the sky meets the black quarters.
    let band = (aperture / crate::layout::APERTURE_R * 0.04).max(1.0);
    let segs = 72u32;
    let mut mesh = Mesh::with_texture(texture);
    for i in 0..=segs {
        let t = i as f32 / segs as f32;
        let x = -aperture + aperture * 2.0 * t;
        let y_arc = (aperture * aperture - x * x).max(0.0).sqrt();
        let mut push = |y: f32| {
            let fade = (y / band).clamp(0.0, 1.0);
            let c = (fade * 255.0).round() as u8;
            mesh.vertices.push(Vertex {
                pos: Pos2::new(center.x + x, center.y + y),
                uv: Pos2::new(t, (y / aperture).clamp(0.0, 1.0)),
                color: Color32::from_rgb(c, c, c),
            });
        };
        push(0.0);
        push(y_arc.min(band));
        push(y_arc);
    }
    for i in 0..segs {
        let a = i * 3;
        let b = a + 3;
        mesh.add_triangle(a, b, b + 1);
        mesh.add_triangle(a, b + 1, a + 1);
        mesh.add_triangle(a + 1, b + 1, b + 2);
        mesh.add_triangle(a + 1, b + 2, a + 2);
    }
    if mesh.indices.len() >= 3 {
        painter.add(egui::Shape::mesh(mesh));
    }
}

/// How much of a surface point faces the Sun. `nx` is right, `ny` is down.
/// Northern hemisphere: waxing lights the right side, waning the left.
pub fn moon_illumination(nx: f32, ny: f32, phase: f64) -> f32 {
    let s = (1.0 - ny * ny).max(0.0).sqrt();
    let edge = (phase * std::f64::consts::TAU).cos() as f32 * s;
    let waxing = (phase * std::f64::consts::TAU).sin() >= 0.0;
    let signed = if waxing { nx - edge } else { -nx - edge };
    smoothstep(-0.04, 0.06, signed)
}

#[cfg(test)]
fn moon_color(nx: f32, ny: f32, radius: f32, phase: f64) -> Color32 {
    if radius > 1.0 {
        return Color32::TRANSPARENT;
    }
    let alb = moon_albedo(nx, ny);
    let mu = (1.0 - radius * radius).max(0.0).sqrt();
    let limb = 0.38 + 0.62 * mu.powf(0.5);
    let lit = moon_illumination(nx, ny, phase);
    let shade = alb * limb;
    let lit_r = 236.0 * shade;
    let lit_g = 226.0 * shade;
    let lit_b = 204.0 * shade;
    // Darkened surface on the unlit side, day and night. Opaque, so scenery
    // does not show through the disc.
    let earth = 0.48;
    let r = lit_r * lit + 78.0 * shade * earth * (1.0 - lit);
    let g = lit_g * lit + 74.0 * shade * earth * (1.0 - lit);
    let b = lit_b * lit + 68.0 * shade * earth * (1.0 - lit);
    Color32::from_rgb(
        r.clamp(0.0, 255.0) as u8,
        g.clamp(0.0, 255.0) as u8,
        b.clamp(0.0, 255.0) as u8,
    )
}

#[cfg(test)]
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

#[cfg(test)]
fn mare(nx: f32, ny: f32, cx: f32, cy: f32, rx: f32, ry: f32, dark: f32) -> f32 {
    let e = ((nx - cx) / rx).powi(2) + ((ny - cy) / ry).powi(2);
    if e < 1.0 {
        let t = (1.0 - e).powf(0.75);
        1.0 - t * (1.0 - dark)
    } else {
        1.0
    }
}

#[cfg(test)]
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
            "leaves-winter",
            "ground-winter",
            "snow",
            "snow-cover",
            "sky-overcast",
        ] {
            assert!(on.contains(id), "missing {id} in {on:?}");
        }
        for id in [
            "sky-night",
            "sky-day",
            "sky-twilight",
            "sun",
            "stars",
            "clouds-few",
            "clouds-partly",
            "clouds-thick",
            "sky-clear",
            "sky-partly",
            "sky-night",
            "tint-dawn",
            "tint-dusk",
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
        let lit = moon_color(-0.95, 0.0, 0.95, phase);
        let dark = moon_color(0.4, 0.0, 0.4, phase);
        assert!(
            lit.a() == 255 && lit.r() > dark.r() + 40,
            "western limb should be lit, got {lit:?} against {dark:?}"
        );
        assert_eq!(dark.a(), 255, "unlit side must be opaque, got {dark:?}");
        assert!(
            dark.r() + 50 < lit.r() && dark.r() > 12,
            "unlit side should be a darkened surface, got {dark:?} against {lit:?}"
        );
    }

    #[test]
    fn default_scene_raster_is_a_blue_day_sky() {
        let theme = Theme::from_parts(DEFAULT_SVG, DEFAULT_TOML).unwrap();
        let on = visible_layers(&theme, screenshot_time(), &fixture_january());
        let rgba = cached_rgba(&theme, &on, 200, 100).unwrap();
        // Mid-sky of the January fixture, which is an overcast snow day.
        let x = 100usize;
        let y = 34usize;
        let i = (y * 200 + x) * 4;
        let px = &rgba[i..i + 4];
        let spread = (px[0] as i32 - px[2] as i32).abs();
        assert!(
            px[0] > 40 && spread < 90,
            "mid sky should be the overcast photograph, got {px:?}"
        );
    }
}
