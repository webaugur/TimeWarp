//! Year and civil-day wheels, clipped to the squared windows beside the dial.
//!
//! Each wheel is an SVG disc (`scenes/wheels/year.svg`, `scenes/wheels/day.svg`)
//! rasterized once and turned about the shaft. The dial covers the inside of the
//! disc. What remains in the window is the outer band, so the motifs in the SVG
//! sit between about 1.02 and 1.41 face radii. See the comment at the top of each SVG.

use std::sync::Mutex;

use egui::epaint::Vertex;
use egui::{Color32, Context, Id, Mesh, Painter, Pos2, Rect, TextureOptions};

use crate::layout::{SKY_WEDGE, YEAR_WEDGE};
use crate::roman::direction;

const YEAR_SVG: &str = include_str!("../scenes/wheels/year.svg");
const DAY_SVG: &str = include_str!("../scenes/wheels/day.svg");
const TEX_SIZE: u32 = 900;

pub fn paint(painter: &Painter, ctx: &Context, center: Pos2, radius: f32, year: f32, day: f32) {
    paint_wheel(
        painter,
        ctx,
        center,
        radius,
        YEAR_WEDGE,
        year,
        YEAR_SVG,
        "year-wheel",
    );
    paint_wheel(
        painter,
        ctx,
        center,
        radius,
        SKY_WEDGE,
        day,
        DAY_SVG,
        "day-wheel",
    );
}

fn paint_wheel(
    painter: &Painter,
    ctx: &Context,
    center: Pos2,
    radius: f32,
    wedge: (f32, f32),
    content: f32,
    svg: &str,
    name: &str,
) {
    let Some(rgba) = raster(svg) else {
        return;
    };
    let image =
        egui::ColorImage::from_rgba_premultiplied([TEX_SIZE as usize, TEX_SIZE as usize], &rgba);
    let texture = ctx.load_texture_cached(name, Id::unique(name), image, TextureOptions::LINEAR);
    let rect = window_rect(center, radius, wedge);
    let painter = painter.with_clip_rect(rect);
    let steps = 36u32;
    let mut mesh = Mesh::default();
    mesh.texture_id = texture.id();
    for iy in 0..=steps {
        for ix in 0..=steps {
            let p = Pos2::new(
                rect.left() + rect.width() * (ix as f32 / steps as f32),
                rect.top() + rect.height() * (iy as f32 / steps as f32),
            );
            let rel = (p - center) / radius.max(1.0);
            let dist = rel.length();
            let screen_deg = rel.x.atan2(-rel.y).to_degrees();
            let drawn = screen_deg + content * 360.0;
            let (u, v) = uv_at(drawn, dist);
            mesh.vertices.push(Vertex {
                pos: p,
                uv: Pos2::new(u, v),
                color: Color32::WHITE,
            });
        }
    }
    let stride = steps + 1;
    for iy in 0..steps {
        for ix in 0..steps {
            let a = iy * stride + ix;
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

/// Texture coordinate for a motif drawn at `drawn` degrees, `dist` face radii from the shaft.
/// The SVG maps face radius 1 to `1/√2` of the half-width, so the corner of the view is √2.
fn uv_at(drawn_deg: f32, dist: f32) -> (f32, f32) {
    let s = (dist / std::f32::consts::SQRT_2).clamp(0.0, 0.999);
    let dir = direction(drawn_deg);
    (0.5 + dir.x * s * 0.5, 0.5 + dir.y * s * 0.5)
}

fn window_rect(center: Pos2, radius: f32, wedge: (f32, f32)) -> Rect {
    let side = if wedge.0 >= 180.0 { -1.0 } else { 1.0 };
    let left = if side < 0.0 {
        center.x - radius
    } else {
        center.x
    };
    Rect::from_min_max(
        Pos2::new(left, center.y),
        Pos2::new(left + radius, center.y + radius),
    )
}

fn raster(svg: &str) -> Option<Vec<u8>> {
    static CACHE: Mutex<Vec<(u64, Vec<u8>)>> = Mutex::new(Vec::new());
    let hash = {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        svg.hash(&mut hasher);
        hasher.finish()
    };
    if let Ok(guard) = CACHE.lock() {
        if let Some((_, hit)) = guard.iter().find(|(key, _)| *key == hash) {
            return Some(hit.clone());
        }
    }
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(svg, &options).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(TEX_SIZE, TEX_SIZE)?;
    let size = tree.size();
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(
            TEX_SIZE as f32 / size.width(),
            TEX_SIZE as f32 / size.height(),
        ),
        &mut pixmap.as_mut(),
    );
    let rgba = pixmap.take();
    if let Ok(mut guard) = CACHE.lock() {
        guard.push((hash, rgba.clone()));
    }
    Some(rgba)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn january_window_center_lands_on_the_winter_band() {
        // 15 Jan, not a leap year: the window center should sample the winter sector (~239°).
        let year = 14.0 / 365.0;
        let (u, v) = uv_at(225.0 + year * 360.0, 1.22);
        assert!((0.10..0.45).contains(&u), "u {u}");
        assert!((0.65..0.95).contains(&v), "v {v}");
    }

    #[test]
    fn mid_morning_lands_on_the_day_sector() {
        let day = (10.0 * 3600.0 + 8.0 * 60.0 + 37.0) / 86_400.0;
        let (u, v) = uv_at(135.0 + day * 360.0, 1.18);
        // Angle ~287° is left of center, a bit above the 9 o'clock line.
        assert!(u < 0.42, "u {u}");
        assert!((0.35..0.62).contains(&v), "v {v}");
    }

    #[test]
    fn wheels_are_not_a_flat_field() {
        let year = raster(YEAR_SVG).unwrap();
        let day = raster(DAY_SVG).unwrap();
        let at = |buf: &[u8], deg: f32, r: f32| {
            let (u, v) = uv_at(deg, r);
            let x = (u * TEX_SIZE as f32) as usize;
            let y = (v * TEX_SIZE as f32) as usize;
            let i = (y * TEX_SIZE as usize + x) * 4;
            [buf[i], buf[i + 1], buf[i + 2]]
        };
        let sky = at(&year, 239.0, 1.10);
        let tree = at(&year, 228.0, 1.20);
        let dist = |a: [u8; 3], b: [u8; 3]| {
            (a[0] as i32 - b[0] as i32).abs()
                + (a[1] as i32 - b[1] as i32).abs()
                + (a[2] as i32 - b[2] as i32).abs()
        };
        assert!(
            dist(sky, tree) > 80,
            "winter sky {sky:?} and tree {tree:?} should differ"
        );
        let noon = at(&day, 287.0, 1.14);
        let ground = at(&day, 300.0, 1.34);
        assert!(
            dist(noon, ground) > 80,
            "day sun {noon:?} and ground {ground:?} should differ"
        );
        assert!(
            noon[0] > 180 && noon[1] > 160,
            "sun should be bright, {noon:?}"
        );
    }
}
