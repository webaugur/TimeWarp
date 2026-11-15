//! Polished brass and gold.
//!
//! Colors are computed in linear light and encoded to sRGB. The dial is a shallow
//! dished mirror: a tight specular, a broader reflection of the room, and faint
//! turned rings. The bezel is a round section. Hour markers are gold pins with a
//! round cross-section, not a flat fill.

use egui::{Color32, Pos2, Vec2};

/// Light comes from the upper left and toward the viewer. Screen +Y is down.
fn light_dir() -> [f32; 3] {
    norm([-0.42, -0.52, 0.74])
}

fn view_dir() -> [f32; 3] {
    [0.0, 0.0, 1.0]
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

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn norm(v: [f32; 3]) -> [f32; 3] {
    let len = dot(v, v).sqrt().max(1.0e-6);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn encode(linear: [f32; 3]) -> Color32 {
    Color32::from_rgb(to_srgb(linear[0]), to_srgb(linear[1]), to_srgb(linear[2]))
}

fn to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.0031308 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0).round().clamp(0.0, 255.0) as u8
}

/// Brass at a point on a dial of the given radius, including the outer bezel.
///
/// `gain` is kept for callers that used to brighten markers. The dial uses `1`.
pub fn shade_dial(center: Pos2, point: Pos2, radius: f32, gain: f32) -> Color32 {
    let rel = (point - center) / radius.max(1.0);
    let dist = rel.length();
    let radial = if dist > 1.0e-4 {
        [rel.x / dist, rel.y / dist]
    } else {
        [1.0, 0.0]
    };
    let normal = if dist > 0.948 {
        bezel_normal(radial, dist)
    } else {
        // Curvature chosen so the mirror peak lands on the ring, not in the aperture.
        norm([rel.x * 0.48, rel.y * 0.48, 1.0])
    };
    let light = light_dir();
    let view = view_dir();
    let half = norm(add(light, view));
    let ndotl = dot(normal, light).max(0.0);
    let ndoth = dot(normal, half).max(0.0);
    let ndotv = dot(normal, view).max(0.0);
    let tight = ndoth.powf(64.0);
    let broad = ndoth.powf(6.0);
    let fresnel = (1.0 - ndotv).powf(2.5);
    // Polished streak along the chapter ring, upper left, where a window reflects.
    let ang = rel.y.atan2(rel.x);
    let aim = (-0.42_f32).atan2(-0.62);
    let mut dang = ang - aim;
    if dang > std::f32::consts::PI {
        dang -= std::f32::consts::TAU;
    } else if dang < -std::f32::consts::PI {
        dang += std::f32::consts::TAU;
    }
    let radial_off = dist - 0.80;
    let streak = (-(dang * dang) * 22.0 - radial_off * radial_off * 90.0).exp();
    let rings = (dist * 36.0 * std::f32::consts::TAU).sin() * 0.035;
    let albedo = [0.74, 0.48, 0.12];
    let spec_tint = [1.0, 0.95, 0.78];
    let mut color = [0.0; 3];
    for i in 0..3 {
        let diffuse = albedo[i] * (0.08 + 0.5 * ndotl);
        let spec = spec_tint[i] * (tight * 0.45 + broad * 0.14 + streak * 0.38);
        let edge = albedo[i] * fresnel * 0.2;
        let raw = (diffuse + spec + edge) * (1.0 + rings) * gain;
        // Shoulder so the reflection stays a hot brass glint, not a flat white disc.
        color[i] = raw / (1.0 + raw * 0.45);
    }
    encode(color)
}

fn bezel_normal(radial: [f32; 2], dist: f32) -> [f32; 3] {
    // Round wire. The crown leans outward so the upper-left of the rim
    // faces the light and the lower-right falls into shade.
    let across = ((dist - 0.948) / 0.052).clamp(0.0, 1.0);
    let ang = (across - 0.42) * std::f32::consts::PI;
    let nr = 0.65 + ang.sin() * 0.55;
    let nz = ang.cos().abs().max(0.2);
    norm([radial[0] * nr, radial[1] * nr, nz])
}

/// Rounded brass for thin strokes (window rims, panel edges).
///
/// `normal` points outward in screen space. `across` is 0 at one lip and 1 at the other.
pub fn shade_bezel(normal: Vec2, across: f32) -> Color32 {
    let n2 = if normal.length() > 0.001 {
        normal.normalized()
    } else {
        Vec2::new(0.0, -1.0)
    };
    let ang = (across - 0.5) * std::f32::consts::PI;
    let n = norm([n2.x * ang.sin(), n2.y * ang.sin(), ang.cos().max(0.05)]);
    let light = light_dir();
    let half = norm(add(light, view_dir()));
    let ndotl = dot(n, light).max(0.0);
    let tight = dot(n, half).max(0.0).powf(48.0);
    let albedo = [0.52, 0.36, 0.11];
    let mut color = [0.0; 3];
    for i in 0..3 {
        color[i] = albedo[i] * (0.06 + 0.4 * ndotl) + tight * 0.85;
    }
    encode(color)
}

/// Gold pin. `cross` is -1 on one edge, 0 on the crown, +1 on the other edge.
/// `along` is 0 at the point and 1 at the round head. `light_bias` is how far
/// the pin's positive-cross side faces the upper-left light (-1 to 1).
pub fn shade_gold_pin(cross: f32, along: f32, light_bias: f32, gain: f32) -> Color32 {
    let edge = cross.abs().clamp(0.0, 1.0);
    let crown = (1.0 - edge * edge).max(0.0).sqrt();
    // The lit flank is the one facing the upper-left light. The crown is the bright ridge.
    let flank = (0.55 + 0.45 * cross * light_bias).clamp(0.15, 1.0);
    let glint = crown.powf(6.0) * (0.35 + 0.65 * along);
    let albedo = [0.92, 0.58, 0.07];
    let spec = [1.0, 0.97, 0.84];
    let mut color = [0.0; 3];
    for i in 0..3 {
        color[i] = (albedo[i] * (0.08 + 0.85 * crown) * flank + spec[i] * glint) * gain;
    }
    encode(color)
}

/// Screen-space direction of the upper-left light, for placing a pin's lit flank.
pub fn light_vec2() -> Vec2 {
    Vec2::new(-0.42, -0.52).normalized()
}
