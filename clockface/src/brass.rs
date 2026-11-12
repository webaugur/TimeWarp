//! Polished-brass shading for the dial, bezel, hour markers, and movement.
//!
//! The metal is yellow gold, not a cream fill. Light comes from the upper left. A tight
//! specular sits on the annulus (the center of the dial is an aperture), and the lower right
//! falls off into antique brown.

use egui::{Color32, Pos2, Vec2};

const BRIGHT: [u8; 3] = [0xee, 0xc9, 0x6a];
const GOLD: [u8; 3] = [0xc4, 0x92, 0x38];
const DEEP: [u8; 3] = [0x7a, 0x4a, 0x18];
const SHADOW: [u8; 3] = [0x4a, 0x2c, 0x0e];
const GLINT: [u8; 3] = [0xff, 0xf6, 0xd2];

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
    let dist = rel.length();
    let facing = if dist > 0.04 {
        rel.normalized().dot(light())
    } else {
        0.55
    };
    // Upper-left is bright gold; lower-right is antique. Stay yellow, not peach.
    let lit = (0.55 + facing * 0.45).clamp(0.0, 1.0);
    let base = if lit > 0.5 {
        mix(rgb(GOLD), rgb(BRIGHT), (lit - 0.5) * 2.0)
    } else {
        mix(rgb(SHADOW), rgb(GOLD), lit * 2.0)
    };
    let base = mix(base, rgb(DEEP), (dist - 0.75).clamp(0.0, 0.35));
    // Tight glint on the upper-left of the ring, plus a softer sheen so the annulus is not flat.
    let glint_at = Vec2::new(-0.62, -0.52);
    let d2 = (rel - glint_at).length_sq();
    let spec = (-d2 * 22.0).exp();
    let sheen = (-d2 * 3.2).exp() * 0.28;
    let color = mix(base, rgb(GLINT), (spec * 0.82 + sheen).clamp(0.0, 0.88));
    scale(color, (0.78 + facing * 0.22) * gain)
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
    let lit = (0.5 + facing * 0.5).clamp(0.0, 1.0);
    let base = mix(rgb(SHADOW), rgb(BRIGHT), lit);
    // Rounded stock: a bright thread along the lit shoulder, dark on the far side.
    let tube = (1.0 - 5.0 * (across - 0.42).powi(2)).clamp(0.0, 1.0);
    let lip = if across < 0.1 || across > 0.9 {
        0.28
    } else {
        0.0
    };
    let spec = facing.max(0.0).powf(2.4);
    scale(
        mix(
            base,
            rgb(GLINT),
            (spec * 0.72 + lip + tube * 0.12).clamp(0.0, 0.92),
        ),
        0.82 + tube * 0.22,
    )
}
