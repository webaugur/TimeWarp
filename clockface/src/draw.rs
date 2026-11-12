use egui::{Color32, Mesh, Painter, Pos2, Shape, Stroke, Vec2};

use crate::roman::direction;

pub fn vertex_color(mesh: &mut Mesh, pos: Pos2, color: Color32) -> u32 {
    let index = mesh.vertices.len() as u32;
    mesh.colored_vertex(pos, color);
    index
}

pub fn add_mesh(painter: &Painter, mesh: Mesh) {
    if mesh.indices.len() >= 3 {
        painter.add(Shape::mesh(mesh));
    }
}

/// Disc centered on `center`. `color_at` is sampled at each rim vertex and at the center.
pub fn paint_disc(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    segments: u32,
    mut color_at: impl FnMut(Pos2) -> Color32,
) {
    if radius <= 0.0 {
        return;
    }
    let mut mesh = Mesh::default();
    let c = vertex_color(&mut mesh, center, color_at(center));
    for i in 0..=segments {
        let t = i as f32 / segments as f32 * std::f32::consts::TAU;
        let p = center + Vec2::new(t.cos(), t.sin()) * radius;
        vertex_color(&mut mesh, p, color_at(p));
    }
    for i in 0..segments {
        mesh.add_triangle(c, 1 + i, 2 + i);
    }
    add_mesh(painter, mesh);
}

/// Annulus from `inner` to `outer` (pixels).
pub fn paint_annulus(
    painter: &Painter,
    center: Pos2,
    inner: f32,
    outer: f32,
    segments: u32,
    mut color_at: impl FnMut(Pos2) -> Color32,
) {
    if outer <= inner || outer <= 0.0 {
        return;
    }
    let mut mesh = Mesh::default();
    for i in 0..=segments {
        let t = i as f32 / segments as f32 * std::f32::consts::TAU;
        let dir = Vec2::new(t.cos(), t.sin());
        let a = center + dir * inner;
        let b = center + dir * outer;
        vertex_color(&mut mesh, a, color_at(a));
        vertex_color(&mut mesh, b, color_at(b));
    }
    for i in 0..segments {
        let base = i * 2;
        mesh.add_triangle(base, base + 1, base + 2);
        mesh.add_triangle(base + 1, base + 3, base + 2);
    }
    add_mesh(painter, mesh);
}

/// Pie slice, degrees clockwise from 12 o'clock. `end` may be less than `start` (wraps past 0).
pub fn paint_pie(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    start_deg: f32,
    end_deg: f32,
    segments: u32,
    mut color_at: impl FnMut(Pos2) -> Color32,
) {
    if radius <= 0.0 {
        return;
    }
    let mut span = (end_deg - start_deg).rem_euclid(360.0);
    if span < 0.5 {
        span = 360.0;
    }
    let mut mesh = Mesh::default();
    let c = vertex_color(&mut mesh, center, color_at(center));
    for i in 0..=segments {
        let deg = start_deg + span * (i as f32 / segments as f32);
        let p = center + direction(deg) * radius;
        vertex_color(&mut mesh, p, color_at(p));
    }
    for i in 0..segments {
        mesh.add_triangle(c, 1 + i, 2 + i);
    }
    add_mesh(painter, mesh);
}

pub fn stroke_arc(
    painter: &Painter,
    center: Pos2,
    radius: f32,
    start_deg: f32,
    end_deg: f32,
    width: f32,
    color: Color32,
) {
    let mut span = (end_deg - start_deg).rem_euclid(360.0);
    if span < 0.05 {
        span = 360.0;
    }
    let steps = (span / 4.0).ceil().max(8.0) as u32;
    let mut points = Vec::with_capacity(steps as usize + 1);
    for i in 0..=steps {
        let deg = start_deg + span * (i as f32 / steps as f32);
        points.push(center + direction(deg) * radius);
    }
    painter.add(Shape::line(points, Stroke::new(width, color)));
}
