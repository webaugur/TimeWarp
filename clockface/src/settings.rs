use std::fs;
use std::path::{Path, PathBuf};

use egui::{Pos2, Vec2};
use serde::Deserialize;

use crate::events::CalendarEvent;

pub const DEFAULT_DIAMETER: f32 = 600.0;
pub const DEFAULT_REFRESH_MINUTES: u32 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Units {
    Imperial,
    Metric,
}

impl Default for Units {
    fn default() -> Self {
        Self::Imperial
    }
}

impl Units {
    pub fn parse(text: &str) -> Option<Self> {
        match text.trim().to_ascii_lowercase().as_str() {
            "imperial" | "f" | "fahrenheit" => Some(Self::Imperial),
            "metric" | "c" | "celsius" => Some(Self::Metric),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Place {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub units: Units,
    /// IANA zone from TimeWarp, when the city lookup returned one.
    pub tz: Option<String>,
}

impl Place {
    pub fn indianapolis() -> Self {
        Self {
            name: "Indianapolis".into(),
            latitude: 39.768623,
            longitude: -86.162643,
            units: Units::Imperial,
            tz: Some("America/Indiana/Indianapolis".into()),
        }
    }
}

impl Default for Place {
    fn default() -> Self {
        Self::indianapolis()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub diameter: f32,
    /// Set when a config file or `--diameter` chose the size. Otherwise the
    /// face is fitted to the window so the column stays on screen.
    pub diameter_explicit: bool,
    pub center_x: Option<f32>,
    pub center_y: Option<f32>,
    pub windowed: bool,
    pub place: Place,
    pub refresh_minutes: u32,
    /// `None` means the file omitted events. Holidays from TimeWarp still show,
    /// and the built-in examples are used only when that list cannot be read.
    pub events: Option<Vec<CalendarEvent>>,
    pub audio: crate::chime::AudioSettings,
    pub leds: crate::leds::LedSettings,
    /// Overrides the `timewarp` found on `PATH`.
    pub timewarp_bin: Option<String>,
    /// Hours ahead for the barometer forecast tick. The default is 3.
    pub barometer_hours: u32,
    /// Clockwise turn applied by the kms build. The desktop window ignores it.
    pub rotation: crate::display::DisplayRotation,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            diameter: DEFAULT_DIAMETER,
            diameter_explicit: false,
            center_x: None,
            center_y: None,
            windowed: false,
            place: Place::default(),
            refresh_minutes: DEFAULT_REFRESH_MINUTES,
            events: None,
            audio: crate::chime::AudioSettings::default(),
            leds: crate::leds::LedSettings::default(),
            timewarp_bin: None,
            barometer_hours: 3,
            rotation: crate::display::DisplayRotation::Deg0,
        }
    }
}

impl Settings {
    /// Center in pixels. A missing axis uses the viewport middle (X) or `diameter / 2` (Y),
    /// which puts the top of the face on the top of the window.
    pub fn center(&self, viewport: Vec2) -> Pos2 {
        Pos2::new(
            self.center_x.unwrap_or(viewport.x * 0.5),
            self.center_y.unwrap_or(self.diameter * 0.5),
        )
    }

    /// Face center and diameter for this viewport.
    ///
    /// With no explicit diameter, the face is the largest one whose column
    /// (face, mural windows, two frames, status bar) fits. `--diameter` and
    /// a config file keep the requested size.
    pub fn placement(&self, viewport: Vec2) -> (Pos2, f32) {
        let diameter = if self.diameter_explicit {
            self.diameter
        } else {
            crate::layout::fit_diameter(viewport)
        };
        let y = if let Some(y) = self.center_y {
            y
        } else if self.diameter_explicit {
            diameter * 0.5
        } else {
            crate::layout::TOP_MARGIN + diameter * 0.5
        };
        (
            Pos2::new(self.center_x.unwrap_or(viewport.x * 0.5), y),
            diameter,
        )
    }
}

/// Values taken from the command line. `Some` wins over the config file.
#[derive(Clone, Debug, Default)]
pub struct SettingsOverride {
    pub diameter: Option<f32>,
    pub center_x: Option<f32>,
    pub center_y: Option<f32>,
    pub windowed: Option<bool>,
    pub city: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub units: Option<Units>,
    pub audio_enabled: Option<bool>,
    pub rotation: Option<crate::display::DisplayRotation>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    #[serde(default)]
    diameter: Option<f32>,
    #[serde(default)]
    center_x: Option<f32>,
    #[serde(default)]
    center_y: Option<f32>,
    #[serde(default)]
    windowed: Option<bool>,
    #[serde(default)]
    place: Option<PlaceFile>,
    #[serde(default)]
    units: Option<String>,
    #[serde(default)]
    refresh_minutes: Option<u32>,
    /// Absent key stays `None` (holidays only, or the examples if TimeWarp is down).
    /// Present, even if empty, is kept and merged with the holiday list.
    #[serde(default)]
    events: Option<Vec<CalendarEvent>>,
    #[serde(default)]
    audio: Option<AudioFile>,
    #[serde(default)]
    leds: Option<LedsFile>,
    #[serde(default)]
    timewarp_bin: Option<String>,
    #[serde(default)]
    barometer_hours: Option<u32>,
    /// Clockwise kms rotation. Absent means leave the default (0°).
    #[serde(default)]
    display: Option<DisplayFile>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct DisplayFile {
    rotation: Option<i32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct LedsFile {
    backend: Option<String>,
    device: Option<String>,
    count: Option<u16>,
    brightness: Option<u8>,
    order: Option<String>,
    freq_hz: Option<u32>,
    symbol_bits: Option<u8>,
    serial_port: Option<String>,
    serial_baud: Option<u32>,
    honor_mute: Option<bool>,
    mute_path: Option<String>,
    debug: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields, default)]
struct AudioFile {
    enabled: Option<bool>,
    tick: Option<String>,
    mute_path: Option<String>,
    gpio_path: Option<String>,
    mute_chimes: Option<bool>,
    sounds_dir: Option<String>,
    hour: Option<String>,
    quarter: Option<String>,
    half: Option<String>,
    bell: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlaceFile {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    latitude: Option<f64>,
    #[serde(default)]
    longitude: Option<f64>,
    #[serde(default)]
    units: Option<String>,
}

pub fn load_settings(path: Option<&Path>, over: &SettingsOverride) -> Result<Settings, String> {
    let stored = crate::timewarp::read_stored_flags();
    load_settings_with(path, over, &stored)
}

pub fn load_settings_with(
    path: Option<&Path>,
    over: &SettingsOverride,
    stored: &crate::timewarp::StoredFlags,
) -> Result<Settings, String> {
    let mut settings = Settings::default();
    let mut file_layer = crate::timewarp::PlaceLayer::default();
    if let Some(path) = path {
        let text = fs::read_to_string(path)
            .map_err(|err| format!("could not read {}: {err}", path.display()))?;
        if !text.trim().is_empty() {
            let file: FileConfig = toml::from_str(&text)
                .map_err(|err| format!("could not parse {}: {err}", path.display()))?;
            file_layer = apply_file(&mut settings, file)?;
        }
    }
    let cli_layer = crate::timewarp::PlaceLayer {
        city: over.city.clone(),
        latitude: over.latitude,
        longitude: over.longitude,
        tz: None,
    };
    let (place, fault) = crate::timewarp::resolve_place(
        settings.place.clone(),
        stored,
        &file_layer,
        &cli_layer,
        settings.timewarp_bin.as_deref(),
    );
    settings.place.name = place.name;
    settings.place.latitude = place.latitude;
    settings.place.longitude = place.longitude;
    settings.place.tz = place.tz;
    apply_override(&mut settings, over)?;
    if let Some(fault) = fault {
        // The almanac fetch reports a missing binary. An unknown city is the
        // one case that has to be said here, because the coordinates never moved.
        if fault.starts_with("unknown city") {
            return Err(fault);
        }
    }
    if settings.diameter <= 0.0 {
        return Err("diameter must be greater than zero".into());
    }
    settings.audio.validate()?;
    settings.leds.validate()?;
    Ok(settings)
}

fn apply_file(
    settings: &mut Settings,
    file: FileConfig,
) -> Result<crate::timewarp::PlaceLayer, String> {
    if let Some(diameter) = file.diameter {
        settings.diameter = diameter;
        settings.diameter_explicit = true;
    }
    if file.center_x.is_some() {
        settings.center_x = file.center_x;
    }
    if file.center_y.is_some() {
        settings.center_y = file.center_y;
    }
    if let Some(windowed) = file.windowed {
        settings.windowed = windowed;
    }
    if let Some(minutes) = file.refresh_minutes {
        settings.refresh_minutes = minutes.max(1);
    }
    if let Some(units) = file.units {
        settings.place.units = Units::parse(&units)
            .ok_or_else(|| format!("units must be imperial or metric, not {units:?}"))?;
    }
    let mut layer = crate::timewarp::PlaceLayer::default();
    if let Some(place) = file.place {
        layer.city = place.name;
        layer.latitude = place.latitude;
        layer.longitude = place.longitude;
        if let Some(units) = place.units {
            settings.place.units = Units::parse(&units)
                .ok_or_else(|| format!("units must be imperial or metric, not {units:?}"))?;
        }
    }
    settings.events = file.events;
    settings.timewarp_bin = file.timewarp_bin;
    if let Some(hours) = file.barometer_hours {
        settings.barometer_hours = hours.clamp(1, 48);
    }
    if let Some(display) = file.display {
        if let Some(degrees) = display.rotation {
            settings.rotation =
                crate::display::DisplayRotation::from_degrees(degrees).ok_or_else(|| {
                    format!("display.rotation must be 0, 90, 180, or 270, not {degrees}")
                })?;
        }
    }
    if let Some(audio) = file.audio {
        apply_audio(&mut settings.audio, audio)?;
    }
    if let Some(leds) = file.leds {
        apply_leds(&mut settings.leds, leds)?;
    }
    Ok(layer)
}

fn apply_leds(leds: &mut crate::leds::LedSettings, file: LedsFile) -> Result<(), String> {
    use crate::leds::{Backend, Order};
    if let Some(backend) = file.backend {
        leds.backend = Backend::parse(&backend)
            .ok_or_else(|| format!("leds.backend must be none, spi, or serial, not {backend:?}"))?;
    }
    if let Some(device) = file.device {
        leds.device = PathBuf::from(device);
    }
    if let Some(count) = file.count {
        leds.count = count as usize;
    }
    if let Some(brightness) = file.brightness {
        leds.brightness = brightness;
    }
    if let Some(order) = file.order {
        leds.order = Order::parse(&order)
            .ok_or_else(|| format!("leds.order must be a channel permutation, not {order:?}"))?;
    }
    if let Some(freq) = file.freq_hz {
        leds.freq_hz = freq;
    }
    if let Some(bits) = file.symbol_bits {
        leds.symbol_bits = bits;
    }
    if let Some(port) = file.serial_port {
        leds.serial_port = PathBuf::from(port);
    }
    if let Some(baud) = file.serial_baud {
        leds.serial_baud = baud;
    }
    if let Some(honor) = file.honor_mute {
        leds.honor_mute = honor;
    }
    if let Some(path) = file.mute_path {
        leds.mute_path = PathBuf::from(path);
    }
    if let Some(debug) = file.debug {
        leds.debug = debug;
    }
    Ok(())
}

fn apply_audio(audio: &mut crate::chime::AudioSettings, file: AudioFile) -> Result<(), String> {
    if let Some(enabled) = file.enabled {
        audio.enabled = enabled;
    }
    if let Some(tick) = file.tick {
        audio.tick = tick;
    }
    if let Some(path) = file.mute_path {
        audio.mute_path = PathBuf::from(path);
    }
    if let Some(path) = file.gpio_path {
        let path = path.trim();
        audio.gpio_path = if path.is_empty() {
            None
        } else {
            Some(PathBuf::from(path))
        };
    }
    if let Some(mute_chimes) = file.mute_chimes {
        audio.mute_chimes = mute_chimes;
    }
    if let Some(dir) = file.sounds_dir {
        audio.sounds_dir = PathBuf::from(dir);
    }
    if let Some(hour) = file.hour {
        audio.hour = hour;
    }
    if let Some(quarter) = file.quarter {
        audio.quarter = quarter;
    }
    if let Some(half) = file.half {
        audio.half = half;
    }
    if let Some(bell) = file.bell {
        audio.bell = bell;
    }
    Ok(())
}

fn apply_override(settings: &mut Settings, over: &SettingsOverride) -> Result<(), String> {
    if let Some(diameter) = over.diameter {
        settings.diameter = diameter;
        settings.diameter_explicit = true;
    }
    if over.center_x.is_some() {
        settings.center_x = over.center_x;
    }
    if over.center_y.is_some() {
        settings.center_y = over.center_y;
    }
    if let Some(windowed) = over.windowed {
        settings.windowed = windowed;
    }
    if let Some(latitude) = over.latitude {
        settings.place.latitude = latitude;
    }
    if let Some(longitude) = over.longitude {
        settings.place.longitude = longitude;
    }
    if let Some(units) = over.units {
        settings.place.units = units;
    }
    if let Some(enabled) = over.audio_enabled {
        settings.audio.enabled = enabled;
    }
    if let Some(rotation) = over.rotation {
        settings.rotation = rotation;
    }
    Ok(())
}

/// `CLOCKFACE_ROTATION`. Empty or unset is `None`.
///
/// `EBOARD_ROTATION` is left for eboard, which applies it after `Options` and
/// so overrides the config file. `--rotation` and this variable are written
/// back into `EBOARD_ROTATION` before `eboard::run` so they stay in front.
pub fn rotation_from_env() -> Result<Option<crate::display::DisplayRotation>, String> {
    read_rotation_var("CLOCKFACE_ROTATION")
}

fn read_rotation_var(key: &str) -> Result<Option<crate::display::DisplayRotation>, String> {
    let Ok(raw) = std::env::var(key) else {
        return Ok(None);
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let degrees: i32 = raw
        .parse()
        .map_err(|_| format!("{key} must be 0, 90, 180, or 270, not {raw:?}"))?;
    crate::display::DisplayRotation::from_degrees(degrees)
        .map(Some)
        .ok_or_else(|| format!("{key} must be 0, 90, 180, or 270, not {raw:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EnvRestore {
        saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl EnvRestore {
        fn set(pairs: &[(&'static str, Option<&str>)]) -> Self {
            let saved = pairs
                .iter()
                .map(|(key, value)| {
                    let prev = std::env::var_os(key);
                    match value {
                        Some(value) => std::env::set_var(key, value),
                        None => std::env::remove_var(key),
                    }
                    (*key, prev)
                })
                .collect();
            Self { saved }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (key, prev) in self.saved.drain(..) {
                match prev {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    #[test]
    #[test]
    fn omitted_diameter_fits_a_1080p_frame_and_a_desktop_window() {
        let settings = Settings::default();
        let (center, diameter) = settings.placement(egui::vec2(1920.0, 1080.0));
        let column = crate::layout::preferred_column_height(diameter);
        assert!(
            diameter < 1080.0,
            "fitted {diameter} should be smaller than the frame"
        );
        assert!(column <= 1080.0 - crate::layout::TOP_MARGIN - crate::layout::BOTTOM_MARGIN + 0.5);
        assert!((center.y - (crate::layout::TOP_MARGIN + diameter * 0.5)).abs() < 0.5);
        let (_, windowed) = settings.placement(egui::vec2(1200.0, 800.0));
        assert!(windowed < diameter);
        assert!(
            crate::layout::preferred_column_height(windowed)
                <= 800.0 - crate::layout::TOP_MARGIN - crate::layout::BOTTOM_MARGIN + 0.5
        );
    }

    fn default_center_is_mid_width_and_radius_from_the_top() {
        let settings = Settings::default();
        let center = settings.center(egui::vec2(1920.0, 1080.0));
        assert_eq!(center, Pos2::new(960.0, 300.0));
    }

    #[test]
    fn flags_override_the_file() {
        let file = FileConfig {
            diameter: Some(400.0),
            center_x: Some(10.0),
            center_y: Some(20.0),
            windowed: Some(true),
            place: None,
            units: None,
            refresh_minutes: None,
            events: None,
            audio: None,
            leds: None,
            timewarp_bin: None,
            barometer_hours: None,
            display: None,
        };
        let mut settings = Settings::default();
        apply_file(&mut settings, file).unwrap();
        apply_override(
            &mut settings,
            &SettingsOverride {
                diameter: Some(800.0),
                center_x: Some(100.0),
                ..SettingsOverride::default()
            },
        )
        .unwrap();
        assert_eq!(settings.diameter, 800.0);
        assert!(settings.diameter_explicit);
        assert_eq!(settings.center_x, Some(100.0));
        assert_eq!(settings.center_y, Some(20.0));
        assert!(settings.windowed);
    }

    #[test]
    fn empty_file_is_the_default() {
        crate::timewarp::with_env_lock(|| {
            let dir = std::env::temp_dir().join(format!(
                "clockface-empty-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0)
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(dir.join("config")).unwrap();
            fs::create_dir_all(dir.join("cache")).unwrap();
            let restore = EnvRestore::set(&[
                ("HOME", Some(dir.to_str().unwrap())),
                (
                    "XDG_CONFIG_HOME",
                    Some(dir.join("config").to_str().unwrap()),
                ),
                ("XDG_CACHE_HOME", Some(dir.join("cache").to_str().unwrap())),
                ("TIMEWARP_CACHE", None),
                ("APPDATA", Some(dir.join("config").to_str().unwrap())),
            ]);
            let path = dir.join("empty.toml");
            fs::write(&path, "").unwrap();
            let settings = load_settings(Some(&path), &SettingsOverride::default()).unwrap();
            let expected = Settings::default();
            drop(restore);
            assert_eq!(settings, expected);
            let _ = fs::remove_dir_all(&dir);
        });
    }

    #[test]
    fn zero_diameter_is_rejected() {
        let err = load_settings(
            None,
            &SettingsOverride {
                diameter: Some(0.0),
                ..SettingsOverride::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("diameter"));
    }

    #[test]
    fn city_flag_sets_coordinates_and_lat_lon_win() {
        let settings = load_settings(
            None,
            &SettingsOverride {
                city: Some("london".into()),
                latitude: Some(40.0),
                ..SettingsOverride::default()
            },
        )
        .unwrap();
        assert_eq!(settings.place.name, "London");
        assert!((settings.place.latitude - 40.0).abs() < 1e-6);
        assert!((settings.place.longitude - -0.1278).abs() < 1e-6);
    }

    #[test]
    fn display_rotation_is_clockwise_and_overridable() {
        let file: FileConfig = toml::from_str("[display]\nrotation = 90\n").unwrap();
        let mut settings = Settings::default();
        apply_file(&mut settings, file).unwrap();
        assert_eq!(settings.rotation, crate::display::DisplayRotation::Deg90);
        apply_override(
            &mut settings,
            &SettingsOverride {
                rotation: Some(crate::display::DisplayRotation::Deg180),
                ..SettingsOverride::default()
            },
        )
        .unwrap();
        assert_eq!(settings.rotation, crate::display::DisplayRotation::Deg180);
    }

    #[test]
    fn display_rotation_rejects_other_angles_and_unknown_keys() {
        let file: FileConfig = toml::from_str("[display]\nrotation = 45\n").unwrap();
        let err = apply_file(&mut Settings::default(), file).unwrap_err();
        assert!(err.contains("0, 90, 180, or 270"), "{err}");
        let err = toml::from_str::<FileConfig>("[display]\nspin = 1\n").unwrap_err();
        assert!(err.to_string().contains("spin"), "{err}");
    }

    #[test]
    fn clockface_rotation_reads_the_env() {
        crate::timewarp::with_env_lock(|| {
            let restore = EnvRestore::set(&[("CLOCKFACE_ROTATION", Some("270"))]);
            let rotation = rotation_from_env().unwrap();
            drop(restore);
            assert_eq!(rotation, Some(crate::display::DisplayRotation::Deg270));
            let restore = EnvRestore::set(&[("CLOCKFACE_ROTATION", Some("15"))]);
            let err = rotation_from_env().unwrap_err();
            drop(restore);
            assert!(err.contains("0, 90, 180, or 270"), "{err}");
        });
    }
}
