//! Roman-numeral analog clock face for a Raspberry Pi kiosk.
//!
//! The painter is reusable. The binary in `main.rs` is the fullscreen shell.
//! Drawing follows the geometry spec in the crate README; it is not derived
//! from the timeanddate.com script the MagicMirror kiosk used to embed.

mod fonts;
mod raster;
mod roman;
mod settings;

pub use fonts::install_fonts;
pub use raster::{render_rgba, write_png};
pub use roman::{
    hand_angles, milliseconds_since_midnight, paint, paint_with, screenshot_time, HandStyle,
    RomanClockStyle, SecondBounce, ShadowStyle, TickStyle,
};
pub use settings::{load_settings, Settings, SettingsOverride};

/// Default face diameter, in pixels.
pub const DEFAULT_DIAMETER: f32 = 600.0;
