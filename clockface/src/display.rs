//! Clockwise display rotation for the kms build.
//!
//! The Pi's vc4 planes cannot rotate 90° or 270° in hardware, and the kernel
//! `video=…,rotate=` token only turns fbcon and Plymouth. eboard lays egui out
//! at the swapped logical size, paints it into an offscreen texture, and blits
//! one textured quad onto the native scanout with a GLES 2 (`#version 100`)
//! shader. This module is the same pixel map, so the headless screenshot can
//! show what that blit produces. Touch and mouse coordinates are remapped
//! inside eboard before egui sees them.

/// Clockwise turn of the egui frame onto the native panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DisplayRotation {
    #[default]
    Deg0,
    Deg90,
    Deg180,
    Deg270,
}

impl DisplayRotation {
    /// `0`, `90`, `180`, `270`, and the same angles wrapped by full turns.
    pub fn from_degrees(degrees: i32) -> Option<Self> {
        match degrees.rem_euclid(360) {
            0 => Some(Self::Deg0),
            90 => Some(Self::Deg90),
            180 => Some(Self::Deg180),
            270 => Some(Self::Deg270),
            _ => None,
        }
    }

    pub fn degrees(self) -> i32 {
        match self {
            Self::Deg0 => 0,
            Self::Deg90 => 90,
            Self::Deg180 => 180,
            Self::Deg270 => 270,
        }
    }

    pub fn swaps_axes(self) -> bool {
        matches!(self, Self::Deg90 | Self::Deg270)
    }

    /// Size egui should lay out at, given the native panel mode.
    pub fn logical_size(self, panel_w: u32, panel_h: u32) -> (u32, u32) {
        if self.swaps_axes() {
            (panel_h, panel_w)
        } else {
            (panel_w, panel_h)
        }
    }

    /// Where logical pixel `(x, y)` lands on the native panel.
    ///
    /// This is `eboard::Rotation::logical_to_panel_px` (top-left origin, +y down).
    pub fn logical_to_panel(
        self,
        x: usize,
        y: usize,
        panel_w: usize,
        panel_h: usize,
    ) -> (usize, usize) {
        match self {
            Self::Deg0 => (x, y),
            Self::Deg90 => (panel_w.saturating_sub(1 + y), x),
            Self::Deg180 => (panel_w.saturating_sub(1 + x), panel_h.saturating_sub(1 + y)),
            Self::Deg270 => (y, panel_h.saturating_sub(1 + x)),
        }
    }
}

/// Paint `logical` (row-major RGBA) onto a native scanout buffer.
///
/// `logical_w` × `logical_h` must be [`DisplayRotation::logical_size`] of the
/// panel this rotation implies. The returned buffer is the panel size.
pub fn scanout_from_logical(
    logical: &[u8],
    logical_w: u32,
    logical_h: u32,
    rotation: DisplayRotation,
) -> (Vec<u8>, u32, u32) {
    let (panel_w, panel_h) = if rotation.swaps_axes() {
        (logical_h, logical_w)
    } else {
        (logical_w, logical_h)
    };
    let pixels = panel_w as usize * panel_h as usize;
    let mut out = vec![0u8; pixels * 4];
    blit(
        logical, logical_w, logical_h, rotation, panel_w, panel_h, &mut out,
    );
    (out, panel_w, panel_h)
}

/// Read a native scanout back as the picture on a panel that was turned
/// clockwise by `rotation`. For the Dell (90°) this is the upright portrait.
pub fn view_mounted_panel(
    scanout: &[u8],
    panel_w: u32,
    panel_h: u32,
    rotation: DisplayRotation,
) -> (Vec<u8>, u32, u32) {
    let (logical_w, logical_h) = rotation.logical_size(panel_w, panel_h);
    let mut out = vec![0u8; logical_w as usize * logical_h as usize * 4];
    let pw = panel_w as usize;
    for y in 0..logical_h as usize {
        for x in 0..logical_w as usize {
            let (px, py) = rotation.logical_to_panel(x, y, pw, panel_h as usize);
            let src = (py * pw + px) * 4;
            let dst = (y * logical_w as usize + x) * 4;
            out[dst..dst + 4].copy_from_slice(&scanout[src..src + 4]);
        }
    }
    (out, logical_w, logical_h)
}

fn blit(
    logical: &[u8],
    logical_w: u32,
    logical_h: u32,
    rotation: DisplayRotation,
    panel_w: u32,
    panel_h: u32,
    out: &mut [u8],
) {
    let lw = logical_w as usize;
    let pw = panel_w as usize;
    let ph = panel_h as usize;
    for y in 0..logical_h as usize {
        for x in 0..lw {
            let src = (y * lw + x) * 4;
            let (px, py) = rotation.logical_to_panel(x, y, pw, ph);
            let dst = (py * pw + px) * 4;
            out[dst..dst + 4].copy_from_slice(&logical[src..src + 4]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degrees_are_only_the_four_right_angles() {
        assert_eq!(
            DisplayRotation::from_degrees(0),
            Some(DisplayRotation::Deg0)
        );
        assert_eq!(
            DisplayRotation::from_degrees(90),
            Some(DisplayRotation::Deg90)
        );
        assert_eq!(
            DisplayRotation::from_degrees(-90),
            Some(DisplayRotation::Deg270)
        );
        assert_eq!(
            DisplayRotation::from_degrees(450),
            Some(DisplayRotation::Deg90)
        );
        assert_eq!(DisplayRotation::from_degrees(45), None);
    }

    #[test]
    fn portrait_layout_is_the_swapped_panel() {
        let rotation = DisplayRotation::Deg90;
        assert_eq!(rotation.logical_size(1280, 1024), (1024, 1280));
        assert_eq!(DisplayRotation::Deg0.logical_size(1280, 1024), (1280, 1024));
    }

    #[test]
    fn clockwise_90_puts_the_logical_top_on_the_native_right() {
        // eboard Deg90: panel = (panel_w - 1 - y, x). The Dell is turned so
        // that native right is the physical top.
        let rotation = DisplayRotation::Deg90;
        assert_eq!(rotation.logical_to_panel(0, 0, 1280, 1024), (1279, 0));
        assert_eq!(rotation.logical_to_panel(0, 1279, 1280, 1024), (0, 0));
        assert_eq!(rotation.logical_to_panel(1023, 0, 1280, 1024), (1279, 1023));
    }

    #[test]
    fn viewing_the_scanout_recovers_the_logical_frame() {
        let logical_w = 4u32;
        let logical_h = 6u32;
        let rotation = DisplayRotation::Deg90;
        let mut logical = vec![0u8; (logical_w * logical_h) as usize * 4];
        logical[0..4].copy_from_slice(&[255, 0, 0, 255]);
        let (scanout, panel_w, panel_h) =
            scanout_from_logical(&logical, logical_w, logical_h, rotation);
        assert_eq!((panel_w, panel_h), (6, 4));
        let top_right = ((0 * panel_w + (panel_w - 1)) * 4) as usize;
        assert_eq!(&scanout[top_right..top_right + 4], &[255, 0, 0, 255]);
        let (viewed, vw, vh) = view_mounted_panel(&scanout, panel_w, panel_h, rotation);
        assert_eq!((vw, vh), (logical_w, logical_h));
        assert_eq!(viewed, logical);
    }
}
