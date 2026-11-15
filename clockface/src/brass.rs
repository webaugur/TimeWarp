//! Gold pins.
//!
//! Colors are computed in linear light and encoded to sRGB. Hour markers are
//! gold pins with a round cross-section, not a flat fill.

use egui::{Color32, Vec2};

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
