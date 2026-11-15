//! Ornate hour and minute hands, and the radial numeral ring.
//!
//! The drawings are CC0 vector clocks from Pixabay, normalized so the pivot
//! (or the dial center) is the origin and each hand points at 12. They are
//! rasterized with resvg and cached until the face is resized.

use std::sync::Mutex;

use egui::epaint::Vertex;
use egui::{Color32, Id, Mesh, Painter, Pos2, TextureOptions, Vec2};

const HOUR_SVG: &str = include_str!("../images/hand-hour.svg");
const MINUTE_SVG: &str = include_str!("../images/hand-minute.svg");
const NUMERAL_SVG: &str = include_str!("../images/numerals.svg");

/// Hour tip, as a fraction of the face radius. Matches the previous hour hand.
const HOUR_REACH: f32 = 0.60;
/// Minute tip on the minute track. Matches the previous minute hand.
const MINUTE_REACH: f32 = 0.70;
/// Outer edge of the numeral ink before the 20% shrink, on the dial rim.
const NUMERAL_OUTER: f32 = 1.0;
/// Midline of the numeral band, as a fraction of the outer ink radius.
const NUMERAL_BAND: f32 = 0.8655;
/// Glyphs are this fraction of their drawn size, about that same midline.
const NUMERAL_SHRINK: f32 = 0.80;

struct Placement {
    view: [f32; 4],
    tip: f32,
}

struct Cached {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

fn placement(svg: &str) -> Placement {
    let view_box = attr(svg, "viewBox");
    let mut nums = view_box
        .split_whitespace()
        .map(|s| s.parse::<f32>().unwrap());
    let view = [
        nums.next().unwrap(),
        nums.next().unwrap(),
        nums.next().unwrap(),
        nums.next().unwrap(),
    ];
    Placement {
        view,
        tip: attr(svg, "data-tip").parse().unwrap(),
    }
}

fn attr<'a>(svg: &'a str, key: &str) -> &'a str {
    let pat = format!("{key}=\"");
    let start = svg.find(&pat).unwrap_or_else(|| panic!("missing {key}")) + pat.len();
    let end = svg[start..].find('"').expect("attr");
    &svg[start..start + end]
}

pub fn paint_hour_hand(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    degrees: f32,
    shadow: &crate::roman::ShadowStyle,
) {
    paint_placed(
        painter,
        "clockface-hour-hand",
        HOUR_SVG,
        center,
        radius,
        degrees,
        HOUR_REACH,
        Some(shadow),
        false,
    );
}

pub fn paint_minute_hand(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    degrees: f32,
    shadow: &crate::roman::ShadowStyle,
) {
    paint_placed(
        painter,
        "clockface-minute-hand",
        MINUTE_SVG,
        center,
        radius,
        degrees,
        MINUTE_REACH,
        Some(shadow),
        false,
    );
}

pub fn paint_numerals(painter: &Painter, center: Pos2, radius: f32) {
    paint_placed(
        painter,
        "clockface-numerals",
        NUMERAL_SVG,
        center,
        radius,
        0.0,
        NUMERAL_OUTER,
        None,
        true,
    );
}

fn paint_placed(
    painter: &Painter,
    name: &str,
    svg: &str,
    center: Pos2,
    radius: f32,
    degrees: f32,
    reach: f32,
    shadow: Option<&crate::roman::ShadowStyle>,
    numeral_ring: bool,
) {
    if radius < 8.0 {
        return;
    }
    let place = placement(svg);
    if place.tip <= 0.0 {
        return;
    }
    let ctx = painter.ctx();
    let ppp = ctx.pixels_per_point().clamp(1.0, 3.0);
    let scale = reach * radius / place.tip;
    let width = (place.view[2] * scale * ppp * 1.7)
        .round()
        .clamp(32.0, 1600.0) as u32;
    let height = (place.view[3] * scale * ppp * 1.7)
        .round()
        .clamp(32.0, 1600.0) as u32;
    let Some(rgba) = cached_rgba(name, svg, width, height) else {
        return;
    };
    let image = egui::ColorImage::from_rgba_premultiplied([width as usize, height as usize], &rgba);
    let texture = ctx.load_texture_cached(
        name,
        Id::unique((name, width, height)),
        image,
        TextureOptions::LINEAR,
    );
    if let Some(shadow) = shadow {
        paint_hand_shadow(
            painter,
            texture.id(),
            center,
            degrees,
            scale,
            place.view,
            shadow.blur * radius,
            shadow.color,
        );
    }
    if numeral_ring {
        paint_numeral_ring(painter, texture.id(), center, scale, place.view, place.tip);
    } else {
        paint_quad(
            painter,
            texture.id(),
            center,
            degrees,
            scale,
            place.view,
            Vec2::ZERO,
            Color32::WHITE,
        );
    }
}

fn paint_hand_shadow(
    painter: &Painter,
    texture: egui::TextureId,
    center: Pos2,
    degrees: f32,
    scale: f32,
    view: [f32; 4],
    blur: f32,
    color: Color32,
) {
    let scale_a = (color.a() as f32 / 255.0) / 0.6;
    let [red, green, blue, _] = color.to_array();
    for (spread, alpha) in crate::roman::SHADOW_LAYERS {
        let a = ((alpha * scale_a) * 255.0).round().clamp(0.0, 255.0) as u8;
        if a == 0 {
            continue;
        }
        let tint = Color32::from_rgba_unmultiplied(red, green, blue, a);
        let radius = blur * spread * 0.5;
        for step in 0..8 {
            let ang = step as f32 / 8.0 * std::f32::consts::TAU;
            let offset = Vec2::new(ang.cos(), ang.sin()) * radius;
            paint_quad(painter, texture, center, degrees, scale, view, offset, tint);
        }
    }
}

/// Shrink each numeral about the band midline so the ring stays put.
fn paint_numeral_ring(
    painter: &Painter,
    texture: egui::TextureId,
    center: Pos2,
    scale: f32,
    view: [f32; 4],
    tip: f32,
) {
    let [minx, miny, vw, vh] = view;
    let band = NUMERAL_BAND * tip * scale;
    let sectors = 12u32;
    let ang_steps = 10u32;
    let rad_steps = 8u32;
    let r0 = tip * 0.62;
    let r1 = tip * 1.04;
    let mut mesh = Mesh::with_texture(texture);
    for sector in 0..sectors {
        let mid = sector as f32 * 30.0;
        let base = mesh.vertices.len() as u32;
        for i in 0..=ang_steps {
            let along = i as f32 / ang_steps as f32;
            let deg = mid - 15.0 + 30.0 * along;
            let (sin, cos) = deg.to_radians().sin_cos();
            for j in 0..=rad_steps {
                let dist = r0 + (r1 - r0) * (j as f32 / rad_steps as f32);
                let x = dist * sin;
                let y = -dist * cos;
                let d = dist * scale;
                let d2 = band + NUMERAL_SHRINK * (d - band);
                let deg2 = mid + NUMERAL_SHRINK * (deg - mid);
                let (s2, c2) = deg2.to_radians().sin_cos();
                mesh.vertices.push(Vertex {
                    pos: Pos2::new(center.x + d2 * s2, center.y - d2 * c2),
                    uv: Pos2::new((x - minx) / vw, (y - miny) / vh),
                    color: Color32::WHITE,
                });
            }
        }
        let cols = rad_steps + 1;
        for i in 0..ang_steps {
            for j in 0..rad_steps {
                let a = base + i * cols + j;
                mesh.add_triangle(a, a + 1, a + cols);
                mesh.add_triangle(a + 1, a + cols + 1, a + cols);
            }
        }
    }
    painter.add(egui::Shape::mesh(mesh));
}

fn paint_quad(
    painter: &Painter,
    texture: egui::TextureId,
    center: Pos2,
    degrees: f32,
    scale: f32,
    view: [f32; 4],
    offset: Vec2,
    tint: Color32,
) {
    let [minx, miny, w, h] = view;
    let corners = [
        (minx, miny, 0.0, 0.0),
        (minx + w, miny, 1.0, 0.0),
        (minx + w, miny + h, 1.0, 1.0),
        (minx, miny + h, 0.0, 1.0),
    ];
    let mut mesh = Mesh::with_texture(texture);
    for (x, y, u, v) in corners {
        let local = Vec2::new(x, y) * scale;
        let turned = rotate_clockwise(local, degrees);
        mesh.vertices.push(Vertex {
            pos: Pos2::new(center.x + turned.x, center.y + turned.y) + offset,
            uv: Pos2::new(u, v),
            color: tint,
        });
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    painter.add(egui::Shape::mesh(mesh));
}

fn rotate_clockwise(v: Vec2, degrees: f32) -> Vec2 {
    let (s, c) = degrees.to_radians().sin_cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

fn cached_rgba(name: &str, svg: &str, width: u32, height: u32) -> Option<Vec<u8>> {
    fn slot(name: &str) -> &'static Mutex<Option<Cached>> {
        match name {
            "clockface-hour-hand" => {
                static SLOT: Mutex<Option<Cached>> = Mutex::new(None);
                &SLOT
            }
            "clockface-minute-hand" => {
                static SLOT: Mutex<Option<Cached>> = Mutex::new(None);
                &SLOT
            }
            _ => {
                static SLOT: Mutex<Option<Cached>> = Mutex::new(None);
                &SLOT
            }
        }
    }
    let slot = slot(name);
    if let Ok(guard) = slot.lock() {
        if let Some(hit) = guard.as_ref() {
            if hit.width == width && hit.height == height {
                return Some(hit.rgba.clone());
            }
        }
    }
    let rgba = render_svg(svg, width, height)?;
    if let Ok(mut guard) = slot.lock() {
        *guard = Some(Cached {
            width,
            height,
            rgba: rgba.clone(),
        });
    }
    Some(rgba)
}

fn render_svg(svg: &str, width: u32, height: u32) -> Option<Vec<u8>> {
    let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).ok()?;
    let mut pixmap = tiny_skia::Pixmap::new(width, height)?;
    let size = tree.size();
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(
            width as f32 / size.width(),
            height as f32 / size.height(),
        ),
        &mut pixmap.as_mut(),
    );
    Some(pixmap.take())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(buf: &[u8], w: u32, u: f32, v: f32) -> [u8; 4] {
        let x = ((u * w as f32) as u32).min(w - 1);
        let h = (buf.len() / 4 / w as usize) as u32;
        let y = ((v * h as f32) as u32).min(h - 1);
        let i = ((y * w + x) * 4) as usize;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    }

    #[test]
    fn hour_hand_points_up_and_keeps_openwork() {
        let place = placement(HOUR_SVG);
        assert!(place.tip > 140.0 && place.tip < 170.0);
        let buf = render_svg(HOUR_SVG, 240, 720).unwrap();
        let tip = pixel(&buf, 240, 0.50, 0.04);
        assert!(tip[3] > 200, "tip should be the gold arrow, got {tip:?}");
        assert!(tip[0] > tip[2], "tip should be gold, got {tip:?}");
        // The fleur body is below the tip. A sample in that band should find
        // both gold and a see-through hole.
        let mut gold = 0;
        let mut clear = 0;
        for y in 180..420 {
            for x in 70..170 {
                let i = (y * 240 + x) * 4;
                if buf[i + 3] > 200 {
                    gold += 1;
                } else if buf[i + 3] < 20 {
                    clear += 1;
                }
            }
        }
        assert!(gold > 200, "fleur should be filled, gold {gold}");
        assert!(clear > 40, "openwork should stay clear, clear {clear}");
    }

    #[test]
    fn minute_hand_points_up() {
        let place = placement(MINUTE_SVG);
        assert!(place.tip > 170.0 && place.tip < 200.0);
        let buf = render_svg(MINUTE_SVG, 160, 640).unwrap();
        let tip = pixel(&buf, 160, 0.50, 0.04);
        assert!(tip[3] > 200 && tip[0] > tip[2], "minute tip {tip:?}");
        let corner = pixel(&buf, 160, 0.02, 0.02);
        assert!(
            corner[3] < 20,
            "the corner is not part of the hand, got {corner:?}"
        );
    }

    #[test]
    fn numerals_are_a_ring_of_near_black() {
        let place = placement(NUMERAL_SVG);
        assert!(place.tip > 260.0 && place.tip < 280.0);
        let buf = render_svg(NUMERAL_SVG, 400, 400).unwrap();
        let center = pixel(&buf, 400, 0.50, 0.50);
        assert!(
            center[3] < 10,
            "dial center should be empty, got {center:?}"
        );
        // The ink is a ring. Walk the 12 o'clock radius until a stroke.
        let mut xii = [0u8; 4];
        let mut found = false;
        for step in 0..80 {
            let v = step as f32 / 400.0;
            let px = pixel(&buf, 400, 0.50, v);
            if px[3] > 200 {
                xii = px;
                found = true;
                break;
            }
        }
        assert!(found, "XII should cross the 12 o'clock line");
        assert!(
            xii[0] < 40 && xii[1] < 40 && xii[2] < 40,
            "XII should be near-black, got {xii:?}"
        );
        let mut vi = [0u8; 4];
        found = false;
        for step in (320..400).rev() {
            let v = step as f32 / 400.0;
            let px = pixel(&buf, 400, 0.50, v);
            if px[3] > 200 {
                vi = px;
                found = true;
                break;
            }
        }
        assert!(found && vi[0] < 40, "VI should be near-black, got {vi:?}");
    }
}
