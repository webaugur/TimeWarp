//! A longcase movement in the black field beside the dial.
//!
//! Each wheel is a disc with trapezoidal teeth around the rim, a hub, and an arbor. They sit
//! behind the face, the mural windows, the boxes, and the status bar. Centers are outside the
//! dial and outside the squared windows, in the open field to the left and right.

use egui::{Color32, Mesh, Painter, Pos2, Vec2};

use crate::brass::shade_dial;
use crate::draw::{add_mesh, paint_disc, vertex_color};

struct Gear {
    /// Offset from the clock center, in face radii.
    at: Vec2,
    /// Tip radius, in face radii.
    radius: f32,
    teeth: u32,
    /// Turns per second, signed. Neighboring wheels turn opposite ways.
    speed: f32,
    phase: f32,
}

pub fn paint(painter: &Painter, center: Pos2, radius: f32, seconds: f64) {
    // Larger wheels first so a pinion reads as lying in front where they mesh.
    let gears = [
        // Left train, largest first. Centers stay outside the dial and the squared windows.
        Gear {
            at: Vec2::new(-2.20, -0.15),
            radius: 0.74,
            teeth: 26,
            speed: 0.008,
            phase: 0.12,
        },
        Gear {
            at: Vec2::new(-2.90, -0.78),
            radius: 0.40,
            teeth: 14,
            speed: -0.012,
            phase: 0.22,
        },
        Gear {
            at: Vec2::new(-1.95, 1.48),
            radius: 0.62,
            teeth: 22,
            speed: 0.010,
            phase: 1.05,
        },
        Gear {
            at: Vec2::new(-2.95, 0.55),
            radius: 0.40,
            teeth: 15,
            speed: -0.014,
            phase: 0.8,
        },
        Gear {
            at: Vec2::new(-1.66, 0.66),
            radius: 0.32,
            teeth: 12,
            speed: -0.020,
            phase: 0.04,
        },
        Gear {
            at: Vec2::new(-2.70, 2.02),
            radius: 0.36,
            teeth: 13,
            speed: -0.015,
            phase: 1.4,
        },
        // Right train, not a mirror copy.
        Gear {
            at: Vec2::new(2.20, -0.02),
            radius: 0.70,
            teeth: 24,
            speed: -0.009,
            phase: 0.35,
        },
        Gear {
            at: Vec2::new(2.88, -0.68),
            radius: 0.40,
            teeth: 14,
            speed: 0.013,
            phase: 1.15,
        },
        Gear {
            at: Vec2::new(1.98, 1.52),
            radius: 0.58,
            teeth: 20,
            speed: 0.011,
            phase: 0.65,
        },
        Gear {
            at: Vec2::new(2.95, 0.48),
            radius: 0.38,
            teeth: 14,
            speed: -0.016,
            phase: 1.6,
        },
        Gear {
            at: Vec2::new(1.69, 0.74),
            radius: 0.30,
            teeth: 11,
            speed: 0.022,
            phase: 0.5,
        },
        Gear {
            at: Vec2::new(2.72, 2.05),
            radius: 0.34,
            teeth: 12,
            speed: -0.017,
            phase: 0.9,
        },
    ];
    for gear in gears {
        paint_gear(
            painter,
            center + gear.at * radius,
            gear.radius * radius,
            &gear,
            seconds,
        );
    }
}

fn paint_gear(painter: &Painter, origin: Pos2, radius: f32, gear: &Gear, seconds: f64) {
    let turns = (gear.phase as f64 + seconds * gear.speed as f64).rem_euclid(1.0);
    let spin = (turns as f32) * std::f32::consts::TAU;
    let root = radius * 0.84;
    paint_disc(painter, origin, root, 36, |p| {
        shade_dial(origin, p, radius, 1.0)
    });

    let teeth = gear.teeth.max(8);
    let pitch = std::f32::consts::TAU / teeth as f32;
    let root_half = pitch * 0.34;
    let tip_half = pitch * 0.18;
    let mut mesh = Mesh::default();
    for i in 0..teeth {
        let mid = spin + i as f32 * pitch;
        let corners = [
            polar(origin, root, mid - root_half),
            polar(origin, root, mid + root_half),
            polar(origin, radius, mid + tip_half),
            polar(origin, radius, mid - tip_half),
        ];
        let base = mesh.vertices.len() as u32;
        for p in corners {
            vertex_color(&mut mesh, p, shade_dial(origin, p, radius, 1.08));
        }
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base, base + 2, base + 3);
    }
    add_mesh(painter, mesh);

    // Hub and arbor, so the wheel reads as a turning part and not a blank disc.
    paint_disc(painter, origin, radius * 0.28, 18, |p| {
        shade_dial(origin, p, radius, 0.72)
    });
    paint_disc(painter, origin, radius * 0.1, 12, |_| {
        Color32::from_rgb(0x14, 0x0c, 0x06)
    });
}

fn polar(origin: Pos2, radius: f32, angle: f32) -> Pos2 {
    origin + Vec2::new(angle.cos(), angle.sin()) * radius
}
