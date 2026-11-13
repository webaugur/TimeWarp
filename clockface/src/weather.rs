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
            night: false,
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
        wind: Some(9.0),
        condition: "light snow".into(),
        daily: vec![
            day("2026-01-15", 71, 31.0, 22.0),
            day("2026-01-16", 3, 34.0, 24.0),
            day("2026-01-17", 71, 29.0, 18.0),
            day("2026-01-18", 0, 36.0, 20.0),
            day("2026-01-19", 61, 40.0, 33.0),
        ],
        note: String::new(),
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
         &current=temperature_2m,weather_code,wind_speed_10m\
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
        night: false,
    })
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
        assert!((report.temperature - 72.4).abs() < 0.01);
        assert_eq!(report.daily[0].date, "2026-07-04");
    }

    #[test]
    fn forecast_marks_reuse_the_timewarp_sun_and_weather_glyphs() {
        assert_eq!(condition_mark(0), "\u{1F31E}");
        assert_eq!(condition_mark(61), "\u{2614}");
        assert_eq!(condition_mark(71), "\u{2744}");
        assert_eq!(condition_mark(95), "\u{26A1}");
    }
}
