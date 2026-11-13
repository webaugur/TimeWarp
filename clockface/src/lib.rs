//! Roman-numeral analog clock face for a Raspberry Pi kiosk.
//!
//! The painter is reusable. The binary in `main.rs` is the fullscreen shell.
//! Drawing follows the geometry spec in the crate README; it is not derived
//! from the timeanddate.com script the MagicMirror kiosk used to embed.

mod brass;
mod cycles;
mod draw;
mod events;
mod fonts;
mod kiosk;
mod layout;
mod mural;
mod raster;
mod roman;
mod scene;
mod settings;
mod sky;
mod status;
mod weather;

pub use events::{examples as example_events, CalendarEvent};
pub use fonts::install_fonts;
pub use kiosk::{paint as paint_kiosk, KioskModel};
pub use raster::{render_rgba, write_png};
pub use roman::{
    hand_angles, milliseconds_since_midnight, paint, paint_with, screenshot_time, HandStyle,
    RomanClockStyle, SecondBounce, ShadowStyle, TickStyle,
};
pub use settings::{load_settings, Place, Settings, SettingsOverride, Units};
pub use status::{gather as gather_status, StatusLine};
pub use weather::{fetch as fetch_weather, load_cache, now_unix, store_cache, WeatherReport};

/// Default face diameter, in pixels.
pub const DEFAULT_DIAMETER: f32 = 600.0;
