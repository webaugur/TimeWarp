//! Polished-brass shading shared by the dial, bezel, hour markers, aperture rims, and gears.
//!
//! The base metals are the old cream dial (`#f7dbb7` toward `#e5caa7`) pushed with a soft
//! upper-left specular highlight and a deeper antique falloff. It is a domed brass surface,
//! not a flat fill and not a chrome mirror.

use egui::{Color32, Pos2, Vec2};

const CENTER: [u8; 3] = [0xf7, 0xdb, 0xb7];
const MID: [u8; 3] = [0xe8, 0xc4, 0x8a];
const DEEP: [u8; 3] = [0x8a, 0x5a, 0x28];
const EDGE: [u8; 3] = [0x5c, 0x3a, 0x18];
const SPEC: [u8; 3] = [0xff, 0xf6, 0xdf];

/// Light comes from the upper left. Screen +Y is down, so the light vector points up-left.
fn light() -> Vec2 {
    Vec2::new(-0.62, -0.78).normalized()
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let a = a.to_array();
    let b = b.to_array();
    Color32::from_rgb(
        lerp_u8(a[0], b[0], t),
        lerp_u8(a[1], b[1], t),
        lerp_u8(a[2], b[2], t),
    )
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t)
        .round()
        .clamp(0.0, 255.0) as u8
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

fn scale(color: Color32, gain: f32) -> Color32 {
    let [r, g, b, _] = color.to_array();
    Color32::from_rgb(
        (r as f32 * gain).round().clamp(0.0, 255.0) as u8,
        (g as f32 * gain).round().clamp(0.0, 255.0) as u8,
        (b as f32 * gain).round().clamp(0.0, 255.0) as u8,
    )
}

/// Brass color at a point on a dial of the given radius.
///
/// `gain` brightens quarter markers (`~1.15`) relative to the other hour markers (`~0.92`).
pub fn shade_dial(center: Pos2, point: Pos2, radius: f32, gain: f32) -> Color32 {
    let rel = (point - center) / radius.max(1.0);
    let dist = rel.length().clamp(0.0, 1.35);
    let base = if dist < 0.55 {
        mix(rgb(CENTER), rgb(MID), dist / 0.55)
    } else {
        mix(rgb(MID), rgb(DEEP), ((dist - 0.55) / 0.5).clamp(0.0, 1.0))
    };
    // Wide specular blob centered up and to the left of the shaft, plus a gentle center sheen.
    let highlight = rel - Vec2::new(-0.26, -0.32);
    let spec = (-highlight.length() * 1.55).exp();
    let sheen = (1.0 - dist).clamp(0.0, 1.0).powf(1.5) * 0.18;
    let facing = if dist > 0.04 {
        rel.normalized().dot(light())
    } else {
        0.65
    };
    let lit = (0.78 + facing * 0.22 + spec * 0.48 + sheen) * gain;
    scale(mix(base, rgb(SPEC), (spec * 0.55).clamp(0.0, 0.65)), lit)
}

/// Rounded brass ring. `across` is 0 at the inner edge and 1 at the outer edge.
/// `normal` points out from the shaft in screen space.
pub fn shade_bezel(normal: Vec2, across: f32) -> Color32 {
    let n = if normal.length() > 0.001 {
        normal.normalized()
    } else {
        Vec2::Y
    };
    let facing = n.dot(light());
    // Tube: brighter along the middle of the band, with a thin lip on both edges.
    let tube = 1.0 - 4.0 * (across - 0.5).powi(2);
    let lip = if across < 0.14 || across > 0.86 {
        0.22
    } else {
        0.0
    };
    let base = mix(rgb(DEEP), rgb(MID), (0.45 + facing * 0.55).clamp(0.0, 1.0));
    let body = mix(base, rgb(SPEC), (tube * 0.28 + lip).clamp(0.0, 0.7));
    // Lower-right of the ring falls off toward antique brown.
    let falloff = (1.0 - facing) * 0.5;
    scale(mix(body, rgb(EDGE), falloff * 0.65), 0.92 + tube * 0.12)
}
