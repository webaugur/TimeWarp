//! Upper quarters: a NOAA sun on the left, a photographed moon on the right.
//!
//! Each quarter is solid black, the space behind the discs.
//! The moon photograph is NASA SVS 5001 (LRO), shaded with the current phase.
//! The unlit side stays a dim photograph (earthshine), never transparent.

use std::sync::OnceLock;

use egui::epaint::Vertex;
use egui::{Color32, Context, Id, Mesh, Painter, Pos2, TextureId, TextureOptions, Vec2};

use crate::draw::paint_pie;
use crate::roman::direction;
use crate::scene::moon_illumination;
use crate::sun_image::{self, SunImage};

const MOON_PNG: &[u8] = include_bytes!("../images/lro-full-moon.png");

/// How far the disc center sits along the quarter's bisector, in aperture radii.
const DISC_DISTANCE: f32 = 0.52;
/// Disc radius, in aperture radii. Stays inside the arc and off the diameters.
const DISC_RADIUS: f32 = 0.34;
/// Unlit moon, as a fraction of the photograph. Faint, still a surface.
const EARTHSHINE: f32 = 0.28;

pub fn paint(
    painter: &Painter,
    ctx: &Context,
    center: Pos2,
    aperture: f32,
    phase: f64,
    sun: Option<&SunImage>,
) {
    paint_quarter(painter, center, aperture, 270.0, 360.0);
    paint_quarter(painter, center, aperture, 0.0, 90.0);
    let (sun_at, radius) = disc_place(center, aperture, 315.0);
    let (moon_at, _) = disc_place(center, aperture, 45.0);
    if let Some(image) = sun {
        paint_sun_photo(painter, ctx, sun_at, radius, image);
    } else {
        paint_drawn_sun(painter, sun_at, radius);
    }
    paint_moon_photo(painter, ctx, moon_at, radius, phase);
}

fn disc_place(center: Pos2, aperture: f32, degrees: f32) -> (Pos2, f32) {
    (
        center + direction(degrees) * (aperture * DISC_DISTANCE),
        aperture * DISC_RADIUS,
    )
}

fn paint_quarter(painter: &Painter, center: Pos2, aperture: f32, start: f32, end: f32) {
    paint_pie(painter, center, aperture, start, end, 24, |_| {
        Color32::BLACK
    });
}

fn paint_sun_photo(painter: &Painter, ctx: &Context, origin: Pos2, radius: f32, image: &SunImage) {
    let color = egui::ColorImage::from_rgba_unmultiplied([image.width, image.height], &image.rgba);
    let texture = ctx.load_texture_cached(
        "clockface-suvi",
        Id::unique(("clockface-suvi", image.stamp)),
        color,
        TextureOptions::LINEAR,
    );
    paint_disc(
        painter,
        origin,
        radius,
        texture.id(),
        |nx, ny| {
            let _ = (nx, ny);
            Color32::WHITE
        },
        |nx, ny| {
            Pos2::new(
                0.5 + nx * sun_image::DISC_RADIUS,
                0.5 + ny * sun_image::DISC_RADIUS,
            )
        },
    );
}

fn paint_drawn_sun(painter: &Painter, origin: Pos2, radius: f32) {
    paint_disc(
        painter,
        origin,
        radius,
        TextureId::default(),
        |nx, ny| {
            let t = (nx * nx + ny * ny).sqrt();
            let spot = if (nx + 0.22).powi(2) + (ny - 0.08).powi(2) < 0.018 {
                0.72
            } else {
                1.0
            };
            let limb = 1.0 - 0.38 * t * t;
            let g = limb * spot;
            Color32::from_rgb((255.0 * g) as u8, (148.0 * g) as u8, (32.0 * limb) as u8)
        },
        |_, _| Pos2::ZERO,
    );
}

fn paint_moon_photo(painter: &Painter, ctx: &Context, origin: Pos2, radius: f32, phase: f64) {
    let image = moon_image();
    let color = egui::ColorImage::from_rgba_unmultiplied([image.width, image.height], &image.rgba);
    let texture = ctx.load_texture_cached(
        "clockface-moon",
        Id::unique("clockface-lro-moon"),
        color,
        TextureOptions::LINEAR,
    );
    paint_disc(
        painter,
        origin,
        radius,
        texture.id(),
        |nx, ny| {
            let lit = moon_illumination(nx, ny, phase);
            let gain = EARTHSHINE + (1.0 - EARTHSHINE) * lit;
            let g = (gain * 255.0).clamp(0.0, 255.0) as u8;
            Color32::from_rgb(g, g, g)
        },
        |nx, ny| Pos2::new(0.5 + nx * 0.47, 0.5 + ny * 0.47),
    );
}

fn paint_disc(
    painter: &Painter,
    origin: Pos2,
    radius: f32,
    texture: TextureId,
    tint: impl Fn(f32, f32) -> Color32,
    uv_at: impl Fn(f32, f32) -> Pos2,
) {
    if radius < 1.0 {
        return;
    }
    let rings = 28u32;
    let segs = 56u32;
    let mut mesh = if texture == TextureId::default() {
        Mesh::default()
    } else {
        Mesh::with_texture(texture)
    };
    let push = |mesh: &mut Mesh, nx: f32, ny: f32| {
        mesh.vertices.push(Vertex {
            pos: origin + Vec2::new(nx, ny) * radius,
            uv: uv_at(nx, ny),
            color: tint(nx, ny),
        });
    };
    for ring in 0..=rings {
        let t = ring as f32 / rings as f32;
        for seg in 0..=segs {
            let ang = seg as f32 / segs as f32 * std::f32::consts::TAU;
            push(&mut mesh, ang.cos() * t, ang.sin() * t);
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

fn moon_image() -> &'static SunImage {
    static IMAGE: OnceLock<SunImage> = OnceLock::new();
    IMAGE.get_or_init(|| {
        sun_image::decode_png(MOON_PNG)
            .unwrap_or_else(|err| panic!("bundled LRO moon photograph failed to decode: {err}"))
    })
}
