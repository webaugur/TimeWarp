//! Regions of the kiosk, all in units of the face radius and relative to the clock center.
//!
//! Angles are degrees clockwise from 12 o'clock. Screen +Y is down.

use egui::{Pos2, Rect};

use crate::roman::direction;

/// Outer radius of the three face apertures, just inside the black ring at 0.68 R.
pub const APERTURE_R: f32 = 0.64;
/// Year window: outside the dial, from 6 o'clock to 9, cut off by the left and bottom tangents.
pub const YEAR_WEDGE: (f32, f32) = (180.0, 270.0);
/// Sky window: the mirror, from 3 o'clock to 6, cut off by the right and bottom tangents.
pub const SKY_WEDGE: (f32, f32) = (90.0, 180.0);
/// Half-width of the three info frames, in face radii. 1.0 makes their outer edges
/// the same width as the face diameter (left tangent to right tangent).
const ROW_HALF: f32 = 1.0;
const GUTTER: f32 = 0.08;

#[derive(Clone, Copy, Debug)]
pub struct KioskLayout {
    pub center: Pos2,
    pub radius: f32,
    pub boxes: [Rect; 3],
    pub status: Rect,
}

impl KioskLayout {
    pub fn new(screen: Rect, center: Pos2, diameter: f32) -> Self {
        let radius = (diameter * 0.5).max(1.0);
        let gutter = GUTTER * radius;
        let col_left = center.x - ROW_HALF * radius;
        let col_right = center.x + ROW_HALF * radius;
        let col_width = col_right - col_left;
        let box_w = ((col_width - 2.0 * gutter) / 3.0).max(8.0);
        let status_h = (0.14 * radius).max(40.0);
        let status_top = screen.bottom() - status_h;
        let status = Rect::from_min_max(
            Pos2::new(col_left.max(screen.left()), status_top),
            Pos2::new(col_right.min(screen.right()), screen.bottom()),
        );
        // The squared windows end on the bottom tangent of the dial.
        let preferred_top = center.y + radius + gutter;
        let min_top = center.y + 1.05 * radius;
        let gap_above_bar = (0.05 * radius).max(10.0);
        let limit = status.top() - gap_above_bar;
        let mut top = preferred_top.max(screen.top());
        let mut height = (1.70 * box_w)
            .min(1.55 * radius)
            .min((limit - top).max(0.0));
        if height < 0.55 * box_w {
            top = (limit - 0.72 * box_w).max(min_top.min(limit - 8.0));
            height = (limit - top).max(8.0);
        }
        let boxes = [0, 1, 2].map(|i| {
            let x = col_left + i as f32 * (box_w + gutter);
            Rect::from_min_max(Pos2::new(x, top), Pos2::new(x + box_w, top + height))
        });
        Self {
            center,
            radius,
            boxes,
            status,
        }
    }

    pub fn point(&self, degrees: f32, radius_of_face: f32) -> Pos2 {
        self.center + direction(degrees) * (radius_of_face * self.radius)
    }
}

/// Where a ray from the shaft meets the straight outer edge of a mural window, in face radii.
///
/// The left window is the square corner outside the dial: the 9 o'clock tangent, the 6 o'clock
/// tangent, and the arc between them. The right window is the mirror. The ray length is 1 on
/// the tangency points and `sqrt(2)` at the outer corner, so the opening squares off instead of
/// bulging into a lobe.
pub fn window_outer(wedge: (f32, f32), degrees: f32) -> f32 {
    let dir = direction(degrees);
    let left = wedge.0 >= 180.0;
    let mut reach = f32::MAX;
    if left && dir.x < -1.0e-4 {
        reach = reach.min(-1.0 / dir.x);
    } else if !left && dir.x > 1.0e-4 {
        reach = reach.min(1.0 / dir.x);
    }
    if dir.y > 1.0e-4 {
        reach = reach.min(1.0 / dir.y);
    }
    if reach.is_finite() {
        reach.max(1.0)
    } else {
        1.0
    }
}

/// Fraction of the wheel shown at a screen angle.
///
/// `content` is the current year fraction or civil-day fraction. The center of the wedge
/// shows that value; other angles offset by their distance from the center, one full turn
/// of the wheel per year or per day.
pub fn wheel_sample(content: f32, degrees: f32, wedge: (f32, f32)) -> f32 {
    let center = wedge.0 + (wedge.1 - wedge.0) * 0.5;
    let delta = (degrees - center) / 360.0;
    (content + delta).rem_euclid(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::vec2;

    #[test]
    fn moving_the_face_moves_the_boxes_by_the_same_amount() {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1080.0));
        let a = KioskLayout::new(screen, Pos2::new(960.0, 300.0), 600.0);
        let b = KioskLayout::new(screen, Pos2::new(1100.0, 360.0), 600.0);
        let shift = b.boxes[0].min - a.boxes[0].min;
        assert!((shift - vec2(140.0, 60.0)).length() < 0.5, "{shift:?}");
        assert!((b.center.x - a.center.x - 140.0).abs() < 0.01);
    }

    #[test]
    fn boxes_stay_above_the_status_bar_and_clear_of_the_dial() {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1280.0, 720.0));
        let layout = KioskLayout::new(screen, Pos2::new(640.0, 280.0), 560.0);
        for panel in layout.boxes {
            assert!(panel.bottom() <= layout.status.top() + 0.5);
            assert!(
                panel.top() + 0.5 >= layout.center.y + 1.05 * layout.radius
                    || panel.height() < 40.0
            );
        }
        assert!(layout.status.bottom() <= screen.bottom() + 0.5);
    }

    #[test]
    fn three_frames_are_exactly_one_face_diameter_wide() {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1080.0));
        for (diameter, center) in [
            (600.0, Pos2::new(960.0, 300.0)),
            (420.0, Pos2::new(700.0, 210.0)),
            (880.0, Pos2::new(960.0, 440.0)),
        ] {
            let layout = KioskLayout::new(screen, center, diameter);
            let span = layout.boxes[2].right() - layout.boxes[0].left();
            assert!(
                (span - diameter).abs() < 0.5,
                "span {span} should equal diameter {diameter}"
            );
            assert!((layout.boxes[0].left() - (center.x - diameter * 0.5)).abs() < 0.5);
            assert!((layout.boxes[2].right() - (center.x + diameter * 0.5)).abs() < 0.5);
        }
    }

    #[test]
    fn window_corner_is_the_long_side_and_the_ends_meet_the_dial() {
        let mid = 225.0;
        let outer = window_outer(YEAR_WEDGE, mid);
        assert!(
            (outer - std::f32::consts::SQRT_2).abs() < 0.02,
            "corner reach {outer}"
        );
        assert!((window_outer(YEAR_WEDGE, 180.0) - 1.0).abs() < 0.03);
        assert!((window_outer(YEAR_WEDGE, 270.0) - 1.0).abs() < 0.03);
        let right = window_outer(SKY_WEDGE, 135.0);
        assert!((right - std::f32::consts::SQRT_2).abs() < 0.02, "{right}");
    }
}
