use egui::{pos2, Pos2, Vec2};
use serde::Deserialize;

use crate::DEFAULT_DIAMETER;

/// Where the face sits in the window. Pixel coordinates, origin top-left.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub diameter: f32,
    /// `None` centers the face horizontally in the viewport.
    pub center_x: Option<f32>,
    /// `None` places the center at `diameter / 2`, so the face touches the top edge.
    pub center_y: Option<f32>,
    pub windowed: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            diameter: DEFAULT_DIAMETER,
            center_x: None,
            center_y: None,
            windowed: false,
        }
    }
}

impl Settings {
    pub fn center(&self, viewport: Vec2) -> Pos2 {
        pos2(
            self.center_x.unwrap_or(viewport.x * 0.5),
            self.center_y.unwrap_or(self.diameter * 0.5),
        )
    }
}

/// Values supplied on the command line. `None` means "not passed".
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsOverride {
    pub diameter: Option<f32>,
    pub center_x: Option<f32>,
    pub center_y: Option<f32>,
    pub windowed: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    diameter: Option<f32>,
    center_x: Option<f32>,
    center_y: Option<f32>,
    windowed: Option<bool>,
}

/// Load an optional TOML document, then apply CLI overrides.
///
/// `toml_text` is `None` when the user did not pass `--config`.
pub fn merge_settings(
    toml_text: Option<&str>,
    flags: &SettingsOverride,
) -> Result<Settings, String> {
    let mut settings = Settings::default();
    if let Some(text) = toml_text {
        let file: FileConfig =
            toml::from_str(text).map_err(|err| format!("parsing config: {err}"))?;
        if let Some(diameter) = file.diameter {
            settings.diameter = diameter;
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
    }
    if let Some(diameter) = flags.diameter {
        settings.diameter = diameter;
    }
    if flags.center_x.is_some() {
        settings.center_x = flags.center_x;
    }
    if flags.center_y.is_some() {
        settings.center_y = flags.center_y;
    }
    if let Some(windowed) = flags.windowed {
        settings.windowed = windowed;
    }
    validate(&settings)?;
    Ok(settings)
}

pub fn load_settings(
    path: Option<&std::path::Path>,
    flags: &SettingsOverride,
) -> Result<Settings, String> {
    let text = match path {
        Some(path) => Some(
            std::fs::read_to_string(path)
                .map_err(|err| format!("reading {}: {err}", path.display()))?,
        ),
        None => None,
    };
    merge_settings(text.as_deref(), flags).map_err(|err| match path {
        Some(path) => format!("{}: {err}", path.display()),
        None => err,
    })
}

fn validate(settings: &Settings) -> Result<(), String> {
    if !settings.diameter.is_finite() || settings.diameter <= 0.0 {
        return Err("diameter must be a finite number greater than 0".into());
    }
    if let Some(x) = settings.center_x {
        if !x.is_finite() {
            return Err("center_x must be finite".into());
        }
    }
    if let Some(y) = settings.center_y {
        if !y.is_finite() {
            return Err("center_y must be finite".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_sit_against_the_top_and_centered() {
        let settings = Settings::default();
        let center = settings.center(egui::vec2(1920.0, 1080.0));
        assert_eq!(center, pos2(960.0, 300.0));
        assert!(!settings.windowed);
    }

    #[test]
    fn flags_override_toml() {
        let text = r#"
            diameter = 400
            center_x = 10
            center_y = 20
            windowed = true
        "#;
        let flags = SettingsOverride {
            diameter: Some(800.0),
            center_x: Some(50.0),
            center_y: None,
            windowed: Some(false),
        };
        let settings = merge_settings(Some(text), &flags).unwrap();
        assert_eq!(settings.diameter, 800.0);
        assert_eq!(settings.center_x, Some(50.0));
        assert_eq!(settings.center_y, Some(20.0));
        assert!(!settings.windowed);
    }

    #[test]
    fn missing_file_fields_keep_defaults() {
        let settings = merge_settings(Some(""), &SettingsOverride::default()).unwrap();
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn rejects_bad_diameter() {
        let flags = SettingsOverride {
            diameter: Some(0.0),
            ..SettingsOverride::default()
        };
        assert!(merge_settings(None, &flags).is_err());
    }
}
