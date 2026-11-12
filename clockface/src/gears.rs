//! Brass gears in the black field around the dial. They sit behind the face, the wedges,
//! the boxes, and the status bar, so the clock stays readable.

use egui::{Color32, Mesh, Painter, Pos2, Vec2};

use crate::brass::{mix, shade_dial};
use crate::draw::{add_mesh, vertex_color};

struct Gear {
    /// Offset from the clock center, in face radii.
    at: Vec2,
    radius: f32,
    teeth: u32,
    /// Turns per second, signed.
    speed: f32,
    phase: f32,
}

pub fn paint(painter: &Painter, center: Pos2, radius: f32, seconds: f64) {
    let gears = [
        Gear {
            at: Vec2::new(-1.55, -0.15),
            radius: 0.42,
            teeth: 16,
            speed: 0.012,
            phase: 0.2,
        },
        Gear {
            at: Vec2::new(1.48, -0.35),
            radius: 0.36,
            teeth: 14,
            speed: -0.015,
            phase: 1.1,
        },
        Gear {
            at: Vec2::new(-1.72, 0.85),
            radius: 0.28,
            teeth: 12,
            speed: 0.02,
            phase: 0.4,
        },
        Gear {
            at: Vec2::new(1.70, 0.95),
            radius: 0.30,
            teeth: 12,
            speed: -0.018,
            phase: 2.0,
        },
        Gear {
            at: Vec2::new(0.15, 1.85),
            radius: 0.34,
            teeth: 18,
            speed: 0.008,
            phase: 0.7,
        },
        Gear {
            at: Vec2::new(-0.95, 1.55),
            radius: 0.22,
            teeth: 10,
            speed: -0.03,
            phase: 1.6,
        },
        Gear {
            at: Vec2::new(1.05, 1.48),
            radius: 0.20,
            teeth: 10,
            speed: 0.028,
            phase: 0.9,
        },
        Gear {
            at: Vec2::new(-1.15, -1.05),
            radius: 0.24,
            teeth: 11,
            speed: -0.01,
            phase: 2.4,
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
    // Reduce in f64. `cos` of a raw Unix timestamp does not return a usable angle.
    let turns = (gear.phase as f64 + seconds * gear.speed as f64).rem_euclid(1.0);
    let spin = (turns as f32) * std::f32::consts::TAU;
    let teeth = gear.teeth.max(6);
    let mut mesh = Mesh::default();
    let hub = vertex_color(&mut mesh, origin, shade_dial(origin, origin, radius, 1.05));
    let steps = teeth * 2;
    for i in 0..=steps {
        let tooth = i % 2 == 0;
        let r = if tooth { radius } else { radius * 0.78 };
        let t = spin + i as f32 / steps as f32 * std::f32::consts::TAU;
        let p = origin + Vec2::new(t.cos(), t.sin()) * r;
        let color = shade_dial(origin, p, radius, if tooth { 1.02 } else { 0.86 });
        vertex_color(&mut mesh, p, color);
    }
    for i in 0..steps {
        mesh.add_triangle(hub, 1 + i, 2 + i);
    }
    add_mesh(painter, mesh);

    let mut arbor = Mesh::default();
    let dark = mix(
        Color32::from_rgb(0x3a, 0x24, 0x12),
        Color32::from_rgb(0x1a, 0x10, 0x08),
        0.45,
    );
    let c = vertex_color(&mut arbor, origin, dark);
    let hole = radius * 0.18;
    for i in 0..=12 {
        let t = i as f32 / 12.0 * std::f32::consts::TAU;
        let p = origin + Vec2::new(t.cos(), t.sin()) * hole;
        vertex_color(&mut arbor, p, Color32::from_rgb(0x14, 0x0c, 0x06));
    }
    for i in 0..12 {
        arbor.add_triangle(c, 1 + i, 2 + i);
    }
    add_mesh(painter, arbor);
}
