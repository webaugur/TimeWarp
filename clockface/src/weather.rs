//! Open-Meteo forecast, cached on disk. No API key.
//!
//! The live clock refreshes about every 20 minutes on a background thread (`curl`, so this
//! crate does not grow an HTTP stack). Offline, the last cache is shown and a failed fetch
//! is appended to the status line. A missing cache paints an "unavailable" scene rather than a blank
//! aperture. `--screenshot` never touches the network or the cache: it uses [`fixture_january`].

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Datelike;
use serde::{Deserialize, Serialize};

use crate::settings::{Place, Units};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DailyForecast {
    pub date: String,
    pub code: i32,
    pub high: f32,
    pub low: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    ClearDay,
    ClearNight,
    Cloud,
    Rain,
    Snow,
    Storm,
    Fog,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WeatherReport {
    pub fetched_unix: i64,
    pub latitude: f64,
    pub longitude: f64,
    pub units: Units,
    pub temperature: f32,
    pub code: Option<i32>,
    pub wind: Option<f32>,
    pub condition: String,
    pub daily: Vec<DailyForecast>,
    pub note: String,
    /// Open-Meteo cloud cover, 0–100. Missing on an old cache; [`Self::cloud_cover`] derives one.
    #[serde(default)]
    pub cloud_cover: Option<u8>,
    /// Meteorological direction the wind blows from, degrees clockwise from north.
    /// Missing on an old cache.
    #[serde(default)]
    pub wind_direction_10m: Option<f32>,
    /// Gust in the same unit as [`Self::wind`]. Missing on an old cache.
    #[serde(default)]
    pub wind_gusts_10m: Option<f32>,
    #[serde(skip)]
    pub night: bool,
}

impl WeatherReport {
    pub fn unavailable(place: &Place) -> Self {
        Self {
            fetched_unix: 0,
            latitude: place.latitude,
            longitude: place.longitude,
            units: place.units,
            temperature: f32::NAN,
            code: None,
            wind: None,
            condition: "unavailable".into(),
            daily: Vec::new(),
            note: "weather unavailable".into(),
            cloud_cover: None,
            wind_direction_10m: None,
            wind_gusts_10m: None,
            night: false,
        }
    }

    /// Percent of the sky covered. A missing reading is inferred from the weather code.
    pub fn cloud_cover(&self) -> u8 {
        if let Some(cover) = self.cloud_cover {
            return cover.min(100);
        }
        match self.code {
            Some(0) => 8,
            Some(1) => 28,
            Some(2) => 55,
            Some(3) => 100,
            Some(45 | 48) => 100,
            Some(51..=67 | 80..=82) => 90,
            Some(71..=77 | 85 | 86) => 92,
            Some(95..=99) => 100,
            Some(_) => 60,
            None => 40,
        }
    }

    pub fn scene(&self) -> Scene {
        match self.code {
            None => Scene::Unknown,
            Some(0) if self.night => Scene::ClearNight,
            Some(0) => Scene::ClearDay,
            Some(1..=3) => Scene::Cloud,
            Some(45 | 48) => Scene::Fog,
            Some(51..=67 | 80..=82) => Scene::Rain,
            Some(71..=77 | 85 | 86) => Scene::Snow,
            Some(95..=99) => Scene::Storm,
            Some(_) => Scene::Cloud,
        }
    }

    pub fn temperature_text(&self) -> String {
        if !self.temperature.is_finite() {
            return "—".into();
        }
        match self.units {
            Units::Imperial => format!("{:.0}°F", self.temperature),
            Units::Metric => format!("{:.0}°C", self.temperature),
        }
    }

    pub fn wind_text(&self) -> String {
        let Some(wind) = self.wind else {
            return String::new();
        };
        match self.units {
            Units::Imperial => format!("{wind:.0} mph"),
            Units::Metric => format!("{wind:.0} km/h"),
        }
    }

    pub fn is_fresh(&self, now_unix: i64, refresh_secs: i64) -> bool {
        self.fetched_unix > 0 && now_unix.saturating_sub(self.fetched_unix) < refresh_secs
    }
}

/// Stable January scene for `--screenshot` (light snow, Indianapolis, imperial).
pub fn fixture_january() -> WeatherReport {
    WeatherReport {
        fetched_unix: 1_768_478_917,
        latitude: 39.7684,
        longitude: -86.1581,
        units: Units::Imperial,
        temperature: 28.0,
        code: Some(71),
        // 25 kt from 300°, so the screenshot shows two full barbs and a half barb.
        wind: Some(25.0 / KT_PER_MPH),
        wind_direction_10m: Some(300.0),
        wind_gusts_10m: Some(34.0),
        condition: "light snow".into(),
        daily: vec![
            day("2026-01-15", 71, 31.0, 22.0),
            day("2026-01-16", 3, 34.0, 24.0),
            day("2026-01-17", 71, 29.0, 18.0),
            day("2026-01-18", 0, 36.0, 20.0),
            day("2026-01-19", 61, 40.0, 33.0),
        ],
        note: String::new(),
        cloud_cover: Some(92),
        night: false,
    }
}

fn day(date: &str, code: i32, high: f32, low: f32) -> DailyForecast {
    DailyForecast {
        date: date.into(),
        code,
        high,
        low,
    }
}

pub fn condition_name(code: i32) -> &'static str {
    match code {
        0 => "clear",
        1 => "mainly clear",
        2 => "partly cloudy",
        3 => "overcast",
        45 | 48 => "fog",
        51 | 53 | 55 => "drizzle",
        56 | 57 => "freezing drizzle",
        61 | 63 | 65 => "rain",
        66 | 67 => "freezing rain",
        71 => "light snow",
        73 | 75 => "snow",
        77 => "snow grains",
        80 | 81 | 82 => "showers",
        85 | 86 => "snow showers",
        95 => "thunderstorm",
        96 | 99 => "hailstorm",
        _ => "weather",
    }
}

/// Trailing forecast mark. Clear uses TimeWarp's sun (🌞). Cloud, rain, snow,
/// and storm use ☁ ☔ ❄ ⚡, parked after the words the same way TimeWarp parks a glyph.
pub fn condition_mark(code: i32) -> &'static str {
    match code {
        0 | 1 => "\u{1F31E}",
        2 => "\u{26C5}",
        3 | 45 | 48 => "\u{2601}",
        51..=67 | 80..=82 => "\u{2614}",
        71..=77 | 85 | 86 => "\u{2744}",
        95..=99 => "\u{26A1}",
        _ => "",
    }
}

pub fn weekday_short(date: &str) -> String {
    match chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d") {
        Ok(date) => {
            let name = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
            name[date.weekday().num_days_from_monday() as usize].into()
        }
        Err(_) => date.to_string(),
    }
}

pub fn cache_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_CACHE_HOME") {
        return Some(PathBuf::from(dir).join("clockface").join("weather.toml"));
    }
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".cache")
            .join("clockface")
            .join("weather.toml")
    })
}

pub fn load_cache(place: &Place) -> Option<WeatherReport> {
    let path = cache_path()?;
    let text = fs::read_to_string(path).ok()?;
    let report: WeatherReport = toml::from_str(&text).ok()?;
    if (report.latitude - place.latitude).abs() > 0.02
        || (report.longitude - place.longitude).abs() > 0.02
        || report.units != place.units
    {
        return None;
    }
    Some(report)
}

pub fn store_cache(report: &WeatherReport) {
    let Some(path) = cache_path() else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(text) = toml::to_string_pretty(report) {
        let _ = fs::write(path, text);
    }
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Blocking fetch. The app calls this from a worker thread.
pub fn fetch(place: &Place) -> Result<WeatherReport, String> {
    let unit_temp = match place.units {
        Units::Imperial => "fahrenheit",
        Units::Metric => "celsius",
    };
    let unit_wind = match place.units {
        Units::Imperial => "mph",
        Units::Metric => "kmh",
    };
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lon:.4}\
         &current=temperature_2m,weather_code,wind_speed_10m,wind_direction_10m,wind_gusts_10m,cloud_cover\
         &daily=weather_code,temperature_2m_max,temperature_2m_min\
         &temperature_unit={unit_temp}&wind_speed_unit={unit_wind}&timezone=auto&forecast_days=5",
        lat = place.latitude,
        lon = place.longitude,
    );
    let output = Command::new("curl")
        .args(["-fsS", "--max-time", "12", &url])
        .output()
        .map_err(|err| format!("curl: {err}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("weather fetch failed ({})", err.trim()));
    }
    let body = String::from_utf8(output.stdout).map_err(|err| err.to_string())?;
    parse_open_meteo(&body, place)
}

pub fn parse_open_meteo(body: &str, place: &Place) -> Result<WeatherReport, String> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|err| err.to_string())?;
    let current = &value["current"];
    let temperature = current["temperature_2m"]
        .as_f64()
        .ok_or("missing temperature")? as f32;
    let code = current["weather_code"]
        .as_i64()
        .ok_or("missing weather code")? as i32;
    let wind = current["wind_speed_10m"].as_f64().map(|v| v as f32);
    let wind_direction_10m = current["wind_direction_10m"].as_f64().map(|v| v as f32);
    let wind_gusts_10m = current["wind_gusts_10m"].as_f64().map(|v| v as f32);
    let times = value["daily"]["time"]
        .as_array()
        .ok_or("missing daily times")?;
    let codes = value["daily"]["weather_code"]
        .as_array()
        .ok_or("missing daily codes")?;
    let highs = value["daily"]["temperature_2m_max"]
        .as_array()
        .ok_or("missing highs")?;
    let lows = value["daily"]["temperature_2m_min"]
        .as_array()
        .ok_or("missing lows")?;
    let mut daily = Vec::new();
    for i in 0..times.len().min(5) {
        daily.push(DailyForecast {
            date: times[i].as_str().unwrap_or("").to_string(),
            code: codes.get(i).and_then(|v| v.as_i64()).unwrap_or(0) as i32,
            high: highs.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
            low: lows.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32,
        });
    }
    Ok(WeatherReport {
        fetched_unix: now_unix(),
        latitude: place.latitude,
        longitude: place.longitude,
        units: place.units,
        temperature,
        code: Some(code),
        wind,
        condition: condition_name(code).into(),
        daily,
        note: String::new(),
        cloud_cover: current["cloud_cover"]
            .as_f64()
            .map(|v| v.clamp(0.0, 100.0) as u8),
        wind_direction_10m,
        wind_gusts_10m,
        night: false,
    })
}

/// Statute miles per hour to knots, and kilometres per hour to knots.
const KT_PER_MPH: f32 = 0.868_976_24;
const KT_PER_KMH: f32 = 0.539_956_8;

pub fn speed_to_knots(speed: f32, units: Units) -> f32 {
    match units {
        Units::Imperial => speed * KT_PER_MPH,
        Units::Metric => speed * KT_PER_KMH,
    }
}

/// Where the wind comes from, and how hard, for the green hand.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindHand {
    pub from_degrees: f32,
    pub knots: f32,
}

impl WeatherReport {
    pub fn wind_hand(&self) -> Option<WindHand> {
        let from = self.wind_direction_10m?;
        let speed = self.wind?;
        if !from.is_finite() || !speed.is_finite() {
            return None;
        }
        Some(WindHand {
            from_degrees: from.rem_euclid(360.0),
            knots: speed_to_knots(speed, self.units),
        })
    }
}

/// Station-model marks for a speed already in knots.
///
/// `None` is calm (under 3 kt): no barbs. Otherwise pennants of 50, full barbs
/// of 10, and one half barb of 5, after rounding to the nearest 5 kt.
pub fn wind_barbs(knots: f32) -> Option<(u32, u32, bool)> {
    if !knots.is_finite() || knots < 3.0 {
        return None;
    }
    let rounded = ((knots / 5.0).round() as i32).max(1) * 5;
    let rounded = rounded as u32;
    let pennants = rounded / 50;
    let rest = rounded % 50;
    Some((pennants, rest / 10, rest % 10 != 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_is_a_snowy_day() {
        let weather = fixture_january();
        assert_eq!(weather.scene(), Scene::Snow);
        assert_eq!(weather.temperature_text(), "28°F");
        assert_eq!(weather.daily.len(), 5);
    }

    #[test]
    fn parses_a_small_open_meteo_payload() {
        let body = r#"{
            "current": {"temperature_2m": 72.4, "weather_code": 1, "wind_speed_10m": 6.2},
            "daily": {
                "time": ["2026-07-04"],
                "weather_code": [1],
                "temperature_2m_max": [88.0],
                "temperature_2m_min": [70.0]
            }
        }"#;
        let place = Place::indianapolis();
        let report = parse_open_meteo(body, &place).unwrap();
        assert_eq!(report.condition, "mainly clear");
        assert_eq!(report.cloud_cover(), 28);
        assert!((report.temperature - 72.4).abs() < 0.01);
        assert_eq!(report.daily[0].date, "2026-07-04");
        assert!(report.wind_direction_10m.is_none());
        assert!(report.wind_gusts_10m.is_none());
    }

    #[test]
    fn wind_direction_and_gusts_are_optional_and_round_to_barbs() {
        let body = r#"{
            "current": {
                "temperature_2m": 28.0,
                "weather_code": 71,
                "wind_speed_10m": 28.77,
                "wind_direction_10m": 300,
                "wind_gusts_10m": 34.0
            },
            "daily": {
                "time": ["2026-01-15"],
                "weather_code": [71],
                "temperature_2m_max": [31.0],
                "temperature_2m_min": [22.0]
            }
        }"#;
        let report = parse_open_meteo(body, &Place::indianapolis()).unwrap();
        assert_eq!(report.wind_direction_10m, Some(300.0));
        assert_eq!(report.wind_gusts_10m, Some(34.0));
        let hand = report.wind_hand().unwrap();
        assert!((hand.from_degrees - 300.0).abs() < 0.01);
        assert!((hand.knots - 25.0).abs() < 0.05);
        assert_eq!(wind_barbs(hand.knots), Some((0, 2, true)));
        assert_eq!(wind_barbs(2.9), None);
        assert_eq!(wind_barbs(50.0), Some((1, 0, false)));
        assert_eq!(wind_barbs(65.0), Some((1, 1, true)));
        let old = r#"
            fetched_unix = 1
            latitude = 39.7
            longitude = -86.1
            units = "imperial"
            temperature = 28.0
            code = 71
            wind = 9.0
            condition = "light snow"
            daily = []
            note = ""
        "#;
        let cached: WeatherReport = toml::from_str(old).unwrap();
        assert!(cached.wind_direction_10m.is_none());
        assert!(cached.wind_gusts_10m.is_none());
        assert!(cached.wind_hand().is_none());
    }

    #[test]
    fn forecast_marks_reuse_the_timewarp_sun_and_weather_glyphs() {
        assert_eq!(condition_mark(0), "\u{1F31E}");
        assert_eq!(condition_mark(61), "\u{2614}");
        assert_eq!(condition_mark(71), "\u{2744}");
        assert_eq!(condition_mark(95), "\u{26A1}");
    }
}
