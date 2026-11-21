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
/// Gap between the two boards, in face radii. The same gap sits under the dial.
const GUTTER: f32 = 0.03;
/// Inset so the column is not flush with a window border or a panel.
pub const TOP_MARGIN: f32 = 16.0;
pub const BOTTOM_MARGIN: f32 = 12.0;
pub const SIDE_MARGIN: f32 = 28.0;

#[derive(Clone, Copy, Debug)]
pub struct KioskLayout {
    pub center: Pos2,
    pub radius: f32,
    pub boxes: [Rect; 2],
    pub status: Rect,
}

impl KioskLayout {
    pub fn new(screen: Rect, center: Pos2, diameter: f32) -> Self {
        let radius = (diameter * 0.5).max(1.0);
        let gutter = GUTTER * radius;
        // Outer edges of the chrome trough, clamped to the side margin.
        // Wider than the dial. `fit_diameter` still budgets the dial width,
        // so this does not shrink the face.
        let (col_left, col_right) = column_edges(screen, center.x, radius);
        let col_width = (col_right - col_left).max(16.0);
        let box_w = ((col_width - gutter) / 2.0).max(8.0);
        let status_h = status_height(radius);
        let status_bottom = screen.bottom() - BOTTOM_MARGIN;
        let status_top = status_bottom - status_h;
        let status = Rect::from_min_max(
            Pos2::new(col_left, status_top),
            Pos2::new(col_right, status_bottom),
        );
        // The squared windows end on the bottom tangent of the dial.
        let preferred_top = center.y + radius + gutter;
        let min_top = center.y + 1.05 * radius;
        let gap_above_bar = (0.05 * radius).max(10.0);
        let limit = status.top() - gap_above_bar;
        let mut top = preferred_top.max(screen.top());
        // The boards fill the space between the dial and the status bar. The face is
        // sized first (fit_diameter_padded still budgets the old board height), so a
        // taller board never shrinks the dial.
        let mut height = (limit - top).max(0.0);
        let narrow = ((col_width - 2.0 * gutter) / 3.0).max(8.0);
        if height < 0.55 * narrow {
            top = (limit - 0.72 * narrow).max(min_top.min(limit - 8.0));
            height = (limit - top).max(8.0);
        }
        let boxes = [
            Rect::from_min_max(
                Pos2::new(col_left, top),
                Pos2::new(col_left + box_w, top + height),
            ),
            Rect::from_min_max(
                Pos2::new(col_right - box_w, top),
                Pos2::new(col_right, top + height),
            ),
        ];
        Self {
            center,
            radius,
            boxes,
            status,
        }
    }

    #[allow(dead_code)]
    pub fn point(&self, degrees: f32, radius_of_face: f32) -> Pos2 {
        self.center + direction(degrees) * (radius_of_face * self.radius)
    }
}

/// Left and right edges of the board column, in pixels.
///
/// Unclamped, each edge is `TROUGH_OUT` radii from the clock center, which is
/// the outer width of the chrome ring. A column that would pass the side
/// margin is pulled in to that margin.
fn column_edges(screen: Rect, center_x: f32, radius: f32) -> (f32, f32) {
    let half = crate::gauges::TROUGH_OUT * radius;
    let left_limit = screen.left() + SIDE_MARGIN;
    let right_limit = screen.right() - SIDE_MARGIN;
    if right_limit <= left_limit + 8.0 {
        return (screen.left(), screen.right().max(screen.left() + 8.0));
    }
    let left = (center_x - half).max(left_limit);
    let right = (center_x + half).min(right_limit).max(left + 8.0);
    (left, right)
}

/// Width of the board halo, in pixels. About 5 px on the Pi portrait face
/// (radius near 338 at dial_scale 0.95). It stays inside half the gutter and
/// half the gap above the status bar, so a neighbor does not cover it.
pub fn board_shadow_px(radius: f32) -> f32 {
    let wanted = radius * (5.0 / 338.0);
    let gutter = GUTTER * radius;
    let gap = (0.05 * radius).max(10.0);
    // Half the gap, minus a pixel, so the two halos meet without stacking.
    let room = (gutter.min(gap) * 0.5 - 1.0).max(0.0);
    wanted.min(room).clamp(0.0, 8.0)
}

/// Point size of the status line for a face of this radius.
pub fn status_font_size(radius: f32) -> f32 {
    (0.06 * radius).clamp(18.0, 24.0)
}

/// Room for one status line: host, address, version, and a warning when there is one.
pub fn status_height(radius: f32) -> f32 {
    let font = status_font_size(radius);
    font + 10.0
}

/// Preferred height of one info board. This is the old three-across size, so
/// two wider boards do not ask the auto-fit for a smaller face.
fn info_frame_height(radius: f32) -> f32 {
    let gutter = GUTTER * radius;
    let narrow = ((radius * 2.0 - 2.0 * gutter) / 3.0).max(8.0);
    (1.70 * narrow).min(1.55 * radius)
}

/// Height from the top of the face through the status bar, when the info frames
/// are at their preferred size. This is what [`fit_diameter`] reserves.
pub fn preferred_column_height(diameter: f32) -> f32 {
    let radius = (diameter * 0.5).max(1.0);
    let gutter = GUTTER * radius;
    let gap = (0.05 * radius).max(10.0);
    diameter + gutter + info_frame_height(radius) + gap + status_height(radius)
}

/// Largest face diameter whose column fits in `viewport`.
///
/// The column is the face, the squared windows under it, the two frames, and a
/// single-line status bar. The width budget is the dial (plus `side_pad`), not
/// the wider trough, so the boards can stick out to the chrome without
/// shrinking the face. Height and that dial width both constrain it.
pub fn fit_diameter(viewport: egui::Vec2) -> f32 {
    fit_diameter_padded(viewport, 0.0, 0.0)
}

/// Like [`fit_diameter`], with extra room above the dial and on each side.
///
/// `top_pad` and `side_pad` are fractions of the face radius. The top pad sits
/// above the dial (the chrome ring may occupy it and hide behind the case).
/// The side pad is added on each side, so the width budget is
/// `diameter * (1 + side_pad)`.
pub fn fit_diameter_padded(viewport: egui::Vec2, top_pad: f32, side_pad: f32) -> f32 {
    let top_pad = top_pad.max(0.0);
    let side_pad = side_pad.max(0.0);
    let available_w = (viewport.x - 2.0 * SIDE_MARGIN).max(120.0);
    let available_h = (viewport.y - TOP_MARGIN - BOTTOM_MARGIN).max(120.0);
    let height = |diameter: f32| preferred_column_height(diameter) + top_pad * diameter * 0.5;
    let width = |diameter: f32| diameter * (1.0 + side_pad);
    let mut lo = 80.0_f32;
    let mut hi = available_w.min(available_h);
    if height(lo) > available_h || width(lo) > available_w {
        return lo.min(available_w / (1.0 + side_pad));
    }
    for _ in 0..28 {
        let mid = (lo + hi) * 0.5;
        if height(mid) <= available_h && width(mid) <= available_w {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Where a ray from the shaft meets the straight outer edge of a mural window, in face radii.
///
/// The left window is the square corner outside the dial: the 9 o'clock tangent, the 6 o'clock
/// tangent, and the arc between them. The right window is the mirror. The ray length is 1 on
/// the tangency points and `sqrt(2)` at the outer corner, so the opening squares off instead of
/// bulging into a lobe.
#[allow(dead_code)]
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
#[allow(dead_code)]
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
    fn two_frames_match_the_trough_and_the_status_bar() {
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(1920.0, 1080.0));
        for (diameter, center) in [
            (600.0, Pos2::new(960.0, 300.0)),
            (420.0, Pos2::new(700.0, 210.0)),
            (880.0, Pos2::new(960.0, 440.0)),
        ] {
            let layout = KioskLayout::new(screen, center, diameter);
            let span = layout.boxes[1].right() - layout.boxes[0].left();
            let trough = diameter * crate::gauges::TROUGH_OUT;
            assert!(
                (span - trough).abs() < 0.5,
                "span {span} should equal trough width {trough}"
            );
            assert!((layout.boxes[0].left() - (center.x - trough * 0.5)).abs() < 0.5);
            assert!((layout.boxes[1].right() - (center.x + trough * 0.5)).abs() < 0.5);
            let gutter = 0.03 * diameter * 0.5;
            let expected = (trough - gutter) / 2.0;
            assert!(
                (layout.boxes[0].width() - expected).abs() < 1.0,
                "each board should be half the trough minus the gap"
            );
            assert!(layout.boxes[1].left() > layout.boxes[0].right() - 0.5);
            assert!((layout.status.left() - layout.boxes[0].left()).abs() < 0.5);
            assert!((layout.status.right() - layout.boxes[1].right()).abs() < 0.5);
            let shadow = board_shadow_px(diameter * 0.5);
            assert!(
                layout.boxes[1].left() - layout.boxes[0].right() + 0.05 >= shadow * 2.0,
                "shadow {shadow} is clipped by the gutter"
            );
        }
    }

    #[test]
    #[test]
    fn fitted_diameter_keeps_the_column_on_screen() {
        for (w, h) in [(1920.0, 1080.0), (1200.0, 800.0), (1280.0, 720.0)] {
            let viewport = vec2(w, h);
            let diameter = fit_diameter(viewport);
            let center = Pos2::new(w * 0.5, TOP_MARGIN + diameter * 0.5);
            let screen = Rect::from_min_size(Pos2::ZERO, viewport);
            let layout = KioskLayout::new(screen, center, diameter);
            let span = layout.boxes[1].right() - layout.boxes[0].left();
            let trough = diameter * crate::gauges::TROUGH_OUT;
            let limit = w - 2.0 * SIDE_MARGIN;
            let expected = trough.min(limit);
            assert!(
                (span - expected).abs() < 1.5,
                "{w}x{h} span {span} trough {trough} limit {limit}"
            );
            assert!(layout.boxes[0].left() + 0.5 >= SIDE_MARGIN);
            assert!(layout.boxes[1].right() <= w - SIDE_MARGIN + 0.5);
            assert!(layout.boxes[0].bottom() <= layout.status.top() + 0.5);
            assert!(layout.status.bottom() <= h - BOTTOM_MARGIN + 0.5);
            assert!(
                layout.boxes[0].height() > 0.45 * layout.radius,
                "{w}x{h} frame height {}",
                layout.boxes[0].height()
            );
            assert!(center.y - diameter * 0.5 >= TOP_MARGIN - 0.5);
        }
    }

    #[test]
    fn a_wider_column_does_not_shrink_the_fitted_dial() {
        // Tall and narrow enough that the dial width binds, not the height.
        // Budgeting the trough here would stop the face at available / TROUGH_OUT.
        let viewport = vec2(800.0, 2000.0);
        let fitted = fit_diameter(viewport);
        let available_w = 800.0 - 2.0 * SIDE_MARGIN;
        assert!(
            (fitted - available_w).abs() < 1.0,
            "fitted {fitted}, dial budget {available_w}"
        );
        assert!(
            fitted > available_w / crate::gauges::TROUGH_OUT + 40.0,
            "fitted {fitted} shrank to the trough"
        );
        let screen = Rect::from_min_size(Pos2::ZERO, viewport);
        let center = Pos2::new(viewport.x * 0.5, TOP_MARGIN + fitted * 0.5);
        let layout = KioskLayout::new(screen, center, fitted);
        let span = layout.boxes[1].right() - layout.boxes[0].left();
        assert!((span - available_w).abs() < 1.5, "clamped span {span}");
    }

    #[test]
    fn pi_portrait_boards_are_wider_and_the_halo_clears_its_neighbors() {
        let (before_chars, after_chars, text_rows, shadow, gap) = pi_board_capacity();
        assert!(
            after_chars > before_chars,
            "chars {before_chars} -> {after_chars}"
        );
        assert!(text_rows > 4, "rows {text_rows}");
        assert!((4.0..5.5).contains(&shadow), "shadow {shadow}");
        assert!(
            gap + 0.05 >= shadow * 2.0,
            "status gap {gap} clips shadow {shadow}"
        );
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
        let screen = Rect::from_min_size(Pos2::ZERO, vec2(200.0, 200.0));
        let layout = KioskLayout::new(screen, Pos2::new(100.0, 80.0), 60.0);
        let corner = layout.point(225.0, 1.2);
        assert!(corner.x < layout.center.x && corner.y > layout.center.y);
        let shown = wheel_sample(0.04, 225.0, YEAR_WEDGE);
        assert!((shown - 0.04).abs() < 0.001, "{shown}");
    }
}

#[cfg(test)]
/// Pi portrait (1024×1280, dial_scale 0.95, top_pad 0.248, center_y 422).
///
/// Returns glyph columns before the widen, after, text rows, halo width, and
/// the gap between a board and the status bar.
fn pi_board_capacity() -> (u32, u32, u32, f32, f32) {
    use crate::matrix::{self, cell};
    use crate::Settings;

    let viewport = egui::vec2(1024.0, 1280.0);
    let mut settings = Settings::default();
    settings.top_pad = Some(0.248);
    settings.dial_scale = 0.95;
    settings.center_y = Some(422.0);
    let (center, diameter) = settings.placement(viewport);
    let screen = Rect::from_min_size(Pos2::ZERO, viewport);
    let layout = KioskLayout::new(screen, center, diameter);
    let pitch = matrix::frame_pitch(layout.radius);
    let panel = layout.boxes[0];
    let before_width = {
        let gutter = 0.03 * layout.radius;
        (diameter - gutter) * 0.5
    };
    let before_chars = glyph_columns(before_width, pitch);
    let after_chars = glyph_columns(panel.width(), pitch);
    let rows = text_rows(panel.height(), pitch);
    let shadow = board_shadow_px(layout.radius);
    let gap = layout.status.top() - panel.bottom();
    eprintln!(
        "pi diameter {diameter:.2} radius {:.2} center ({:.1},{:.1}) pitch {pitch:.4} cell {:.3} board {:.1}x{:.1} (was {before_width:.1} wide) chars {before_chars}->{after_chars} text_rows {rows} lamp_cols {}->{} lamp_rows {} shadow {shadow:.2} status_gap {gap:.2}",
        layout.radius,
        center.x,
        center.y,
        cell(pitch),
        panel.width(),
        panel.height(),
        lamp_steps(before_width, pitch),
        lamp_steps(panel.width(), pitch),
        lamp_steps(panel.height(), pitch),
    );
    (before_chars, after_chars, rows, shadow, gap)
}

/// Glyphs that fit on one line. The first lamp is one pitch in, the wrap
/// limit is `0.4 * pitch` inside the right edge, and each cell is `6 * pitch`.
#[cfg(test)]
fn glyph_columns(width: f32, pitch: f32) -> u32 {
    let room = width - pitch - pitch * 0.4;
    if room <= 0.0 {
        return 0;
    }
    (room / (pitch * 6.0)).floor() as u32
}

/// Text lines, using the same step as `paint_text` (`9 * pitch`) and the same
/// fit test as `line_fits` (`y + 7 * pitch` inside the panel).
#[cfg(test)]
fn text_rows(height: f32, pitch: f32) -> u32 {
    let mut rows = 0u32;
    let mut y = pitch;
    while y + pitch * 7.0 <= height + 0.01 {
        rows += 1;
        y += pitch * 9.0;
    }
    rows
}

#[cfg(test)]
fn lamp_steps(span: f32, pitch: f32) -> u32 {
    let mut n = 0u32;
    let mut at = pitch;
    while at < span - pitch * 0.3 {
        n += 1;
        at += pitch;
    }
    n
}
