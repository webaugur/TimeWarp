//! Roman-numeral analog clock face for a Raspberry Pi kiosk.
//!
//! The painter is reusable. The binary in `main.rs` is the fullscreen shell.
//! Drawing follows the geometry spec in the crate README; it is not derived
//! from the timeanddate.com script the MagicMirror kiosk used to embed.

mod brass;
mod chime;
mod cycles;
mod display;
mod draw;
mod events;
mod fonts;
mod gauges;
mod kiosk;
mod layout;
mod leds;
mod matrix;
mod mural;
mod ornament;
mod plate;
mod quarters;
mod raster;
mod roman;
mod scene;
mod settings;
mod sky;
mod status;
mod sun_image;
mod swatch;
mod timewarp;
mod weather;

pub use chime::{start as start_audio, SoundFault};
pub use display::{scanout_from_logical, view_mounted_panel, DisplayRotation};
pub use events::{examples as example_events, CalendarEvent};
pub use fonts::install_fonts;
pub use gauges::gauge_fault;
pub use kiosk::{paint as paint_kiosk, paint_live, repaint_delay, KioskModel};
pub use raster::{render_rgba, render_rgba_with, write_png};
pub use roman::{
    hand_angles, milliseconds_since_midnight, paint, paint_with, screenshot_time, HandStyle,
    RomanClockStyle, SecondBounce, ShadowStyle, TickStyle,
};
pub use settings::{load_settings, rotation_from_env, Place, Settings, SettingsOverride, Units};
pub use sky::DaySky;
pub use status::{gather as gather_status, StatusLine};
pub use sun_image::{
    load_cached as load_cached_sun, load_or_fetch as load_sun, SunImage,
    REFRESH_SECS as SUN_REFRESH_SECS,
};
pub use timewarp::{cached_almanac, fetch_almanac, Almanac, AlmanacSlot};
pub use weather::{fetch as fetch_weather, load_cache, now_unix, store_cache, WeatherReport};

/// Default face diameter, in pixels.
pub const DEFAULT_DIAMETER: f32 = 600.0;
