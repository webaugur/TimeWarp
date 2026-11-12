//! Regions of the kiosk, all in units of the face radius and relative to the clock center.
//!
//! Angles are degrees clockwise from 12 o'clock. Screen +Y is down.

use egui::{Pos2, Rect};

use crate::roman::direction;

/// Outer radius of the three face apertures, just inside the black ring at 0.68 R.
pub const APERTURE_R: f32 = 0.64;
/// Mural wheels, hidden by the face except for the wedges.
pub const WHEEL_R: f32 = 1.55;
/// Left (year) window, 6:30 through 8 o'clock.
pub const YEAR_WEDGE: (f32, f32) = (195.0, 240.0);
/// Right (sky) window, the mirror of the year wedge across the 12–6 axis.
pub const SKY_WEDGE: (f32, f32) = (120.0, 165.0);
const ROW_HALF: f32 = 1.15;
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
        let status_h = (0.11 * radius).max(28.0);
        let status_top = screen.bottom() - status_h;
        let status = Rect::from_min_max(
            Pos2::new(col_left.max(screen.left()), status_top),
            Pos2::new(col_right.min(screen.right()), screen.bottom()),
        );
        let preferred_top = center.y + WHEEL_R * radius + gutter;
        let min_top = center.y + 1.05 * radius;
        let gap_above_bar = (0.05 * radius).max(10.0);
        let limit = status.top() - gap_above_bar;
        let mut top = preferred_top.max(screen.top());
        let mut height = (1.15 * box_w)
            .min(1.20 * radius)
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

/// Outer radius of a wedge, in face-radii. Meets the dial at the sides and reaches [`WHEEL_R`]
/// on the bisector, so the window reads as a rounded triangle rather than a hard slice.
pub fn wedge_outer(start_deg: f32, end_deg: f32, degrees: f32) -> f32 {
    let span = (end_deg - start_deg).rem_euclid(360.0).max(0.5);
    let along = ((degrees - start_deg).rem_euclid(360.0) / span).clamp(0.0, 1.0);
    // Peaks at 1 on the bisector and is 0 on both edges. `sin(π·along).powf` is NaN at the
    // endpoints because `sin(π)` is a tiny negative and a non-integer power of that is NaN.
    let flare = (4.0 * along * (1.0 - along)).powf(0.9);
    1.0 + (WHEEL_R - 1.0) * flare
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
    fn wedge_bisector_is_the_long_side() {
        let mid = 195.0 + (240.0 - 195.0) * 0.5;
        let outer = wedge_outer(YEAR_WEDGE.0, YEAR_WEDGE.1, mid);
        assert!((outer - WHEEL_R).abs() < 0.02, "{outer}");
        assert!(wedge_outer(YEAR_WEDGE.0, YEAR_WEDGE.1, YEAR_WEDGE.0) < 1.05);
    }
}
