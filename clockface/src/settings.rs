use std::fs;
use std::path::Path;

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
}

impl Place {
    pub fn indianapolis() -> Self {
        Self {
            name: "Indianapolis".into(),
            latitude: 39.7684,
            longitude: -86.1581,
            units: Units::Imperial,
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
    /// `None` means the file omitted events, so the built-in examples are used.
    pub events: Option<Vec<CalendarEvent>>,
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
    /// Absent key stays `None` (use examples). Present, even if empty, is kept.
    #[serde(default)]
    events: Option<Vec<CalendarEvent>>,
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
    let mut settings = Settings::default();
    if let Some(path) = path {
        let text = fs::read_to_string(path)
            .map_err(|err| format!("could not read {}: {err}", path.display()))?;
        if !text.trim().is_empty() {
            let file: FileConfig = toml::from_str(&text)
                .map_err(|err| format!("could not parse {}: {err}", path.display()))?;
            apply_file(&mut settings, file)?;
        }
    }
    apply_override(&mut settings, over)?;
    if settings.diameter <= 0.0 {
        return Err("diameter must be greater than zero".into());
    }
    Ok(settings)
}

fn apply_file(settings: &mut Settings, file: FileConfig) -> Result<(), String> {
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
    if let Some(place) = file.place {
        if let Some(name) = place.name {
            apply_city(&mut settings.place, &name)?;
        }
        if let Some(latitude) = place.latitude {
            settings.place.latitude = latitude;
        }
        if let Some(longitude) = place.longitude {
            settings.place.longitude = longitude;
        }
        if let Some(units) = place.units {
            settings.place.units = Units::parse(&units)
                .ok_or_else(|| format!("units must be imperial or metric, not {units:?}"))?;
        }
    }
    settings.events = file.events;
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
    if let Some(city) = &over.city {
        apply_city(&mut settings.place, city)?;
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
    Ok(())
}

fn apply_city(place: &mut Place, name: &str) -> Result<(), String> {
    let key = name.trim().to_ascii_lowercase();
    let (label, lat, lon) = match key.as_str() {
        "indianapolis" | "indy" => ("Indianapolis", 39.7684, -86.1581),
        "chicago" => ("Chicago", 41.8781, -87.6298),
        "new york" | "nyc" | "newyork" => ("New York", 40.7128, -74.0060),
        "london" => ("London", 51.5074, -0.1278),
        "denver" => ("Denver", 39.7392, -104.9903),
        _ => {
            return Err(format!(
                "unknown city {name:?} (indianapolis, chicago, new york, london, denver), or pass --lat and --lon"
            ));
        }
    };
    place.name = label.into();
    place.latitude = lat;
    place.longitude = lon;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let dir = std::env::temp_dir();
        let path = dir.join("clockface-empty-test.toml");
        fs::write(&path, "").unwrap();
        let settings = load_settings(Some(&path), &SettingsOverride::default()).unwrap();
        assert_eq!(settings, Settings::default());
        let _ = fs::remove_file(path);
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
}
