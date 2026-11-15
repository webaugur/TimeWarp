//! Place, holidays, and the daily almanac from the `timewarp` CLI.
//!
//! Clockface reads TimeWarp's stored flags the same way `paths.py` does, then
//! asks the `timewarp` binary for the city, the holiday list, the seasons, and
//! the rise and set times. The built-in tables stay in place for when that
//! binary is missing or a command fails. Every command runs with a short
//! timeout, off the UI thread. One fault is reported on the status line.

use std::cell::Cell;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use chrono::{DateTime, Datelike, Local, NaiveDate, TimeZone};

use crate::events::{self, CalendarEvent};
use crate::settings::{Place, Settings};
use crate::sky::DaySky;

const TIMEOUT: Duration = Duration::from_secs(4);
const SYNODIC_DAYS: f64 = 29.530588853;
/// How long a failed fetch waits before it tries again. A good fetch lasts until midnight.
const RETRY_AFTER: Duration = Duration::from_secs(15 * 60);

static ENV_LOCK: Mutex<()> = Mutex::new(());
thread_local! {
    static ENV_HELD: Cell<bool> = const { Cell::new(false) };
}

/// Hold the env lock. Re-entrant on this thread so a test can set `HOME` and
/// then read the TimeWarp cache path.
fn enter_env() -> EnvGuard {
    if ENV_HELD.with(Cell::get) {
        return EnvGuard::Reentrant;
    }
    let guard = ENV_LOCK.lock().unwrap_or_else(|err| err.into_inner());
    ENV_HELD.with(|held| held.set(true));
    EnvGuard::Locked(guard)
}

enum EnvGuard {
    Locked(std::sync::MutexGuard<'static, ()>),
    Reentrant,
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        if matches!(self, EnvGuard::Locked(_)) {
            ENV_HELD.with(|held| held.set(false));
        }
    }
}

#[cfg(test)]
pub(crate) fn with_env_lock<T>(body: impl FnOnce() -> T) -> T {
    let _guard = enter_env();
    body()
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoredFlags {
    pub city: Option<String>,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub tz: Option<String>,
    pub country: Option<String>,
    pub region: Option<String>,
}

impl StoredFlags {
    pub fn place_layer(&self) -> PlaceLayer {
        PlaceLayer {
            city: self.city.clone(),
            latitude: self.lat,
            longitude: self.lon,
            tz: self.tz.clone(),
        }
    }

    pub fn has_place(&self) -> bool {
        self.city.is_some() || self.lat.is_some() || self.lon.is_some() || self.tz.is_some()
    }

    /// `--country`, or the stored `--holidays` country, or US federal.
    pub fn holiday_country(&self) -> String {
        self.country
            .clone()
            .filter(|text| !text.trim().is_empty())
            .unwrap_or_else(|| "US".into())
    }
}

#[derive(Clone, Debug, Default)]
pub struct PlaceLayer {
    pub city: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub tz: Option<String>,
}

impl PlaceLayer {
    pub fn is_empty(&self) -> bool {
        self.city.is_none()
            && self.latitude.is_none()
            && self.longitude.is_none()
            && self.tz.is_none()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedCity {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub tz: Option<String>,
}

enum Lookup {
    Found(ResolvedCity),
    /// The binary ran and does not know this name.
    Unknown,
    /// No binary, or it did not answer.
    Unavailable,
}

/// Apply place layers from lowest precedence to highest.
///
/// A city the lookup knows replaces the coordinates. A city only the built-in
/// table knows uses that table. Latitude and longitude on the same layer win
/// over the city's coordinates. An unknown name leaves the previous place and
/// is reported once.
fn fold_place(
    mut place: Place,
    layers: &[PlaceLayer],
    mut lookup: impl FnMut(&str) -> Lookup,
) -> (Place, Option<String>) {
    let mut fault = None;
    for layer in layers {
        if let Some(city) = layer.city.as_deref() {
            let name = city.trim();
            if !name.is_empty() {
                match lookup(name) {
                    Lookup::Found(found) => {
                        place.name = found.name;
                        place.latitude = found.lat;
                        place.longitude = found.lon;
                        if found.tz.is_some() {
                            place.tz = found.tz;
                        }
                    }
                    Lookup::Unknown => {
                        if let Some(builtin) = builtin_city(name) {
                            place.name = builtin.name;
                            place.latitude = builtin.lat;
                            place.longitude = builtin.lon;
                        } else if fault.is_none() {
                            fault = Some(format!("unknown city {name}"));
                        }
                    }
                    Lookup::Unavailable => {
                        if let Some(builtin) = builtin_city(name) {
                            place.name = builtin.name;
                            place.latitude = builtin.lat;
                            place.longitude = builtin.lon;
                        } else if fault.is_none() {
                            fault = Some("timewarp unavailable".into());
                        }
                    }
                }
            }
        }
        if let Some(latitude) = layer.latitude {
            place.latitude = latitude;
        }
        if let Some(longitude) = layer.longitude {
            place.longitude = longitude;
        }
        if layer.tz.is_some() {
            place.tz = layer.tz.clone();
        }
    }
    (place, fault)
}

#[derive(Clone, Debug)]
pub struct Almanac {
    pub civil_date: NaiveDate,
    pub events: Vec<CalendarEvent>,
    pub sky: DaySky,
    pub fault: String,
    pub from_timewarp: bool,
}

impl Almanac {
    pub fn builtin(now: DateTime<Local>, place: &Place, user: Option<&[CalendarEvent]>) -> Self {
        Self {
            civil_date: now.date_naive(),
            events: user
                .map(|rows| rows.to_vec())
                .unwrap_or_else(events::examples),
            sky: DaySky::builtin(now, place),
            fault: String::new(),
            from_timewarp: false,
        }
    }
}

/// In-memory almanac plus when the last attempt finished.
#[derive(Clone, Debug)]
pub struct AlmanacSlot {
    pub almanac: Almanac,
    pub attempted: Option<std::time::Instant>,
}

impl AlmanacSlot {
    pub fn fresh_for(&self, today: NaiveDate) -> bool {
        if self.almanac.civil_date != today {
            return false;
        }
        match self.attempted {
            None => false,
            Some(at) if !self.almanac.fault.is_empty() => at.elapsed() < RETRY_AFTER,
            Some(_) => true,
        }
    }
}

/// Blocking fetch. The kiosk calls this on a worker thread, and `--screenshot` calls it before it paints.
pub fn fetch_almanac(settings: &Settings, now: DateTime<Local>) -> Almanac {
    let stored = read_stored_flags();
    let mut almanac = Almanac::builtin(now, &settings.place, settings.events.as_deref());
    let Some(bin) = locate_bin(settings.timewarp_bin.as_deref()) else {
        almanac.fault = "timewarp unavailable".into();
        if let Some(cached) = read_disk(&settings.place, now.date_naive()) {
            return cached_with_fault(cached, "timewarp unavailable");
        }
        return almanac;
    };

    let mut fault: Option<String> = None;
    let mut got_holidays = false;
    let mut holiday_rows = Vec::new();
    let mut seasons = Vec::new();
    let date = now.format("%Y-%m-%d").to_string();
    let country = stored.holiday_country();
    let region = stored.region.clone();

    for year in [now.year(), now.year() + 1] {
        let mut args = vec![
            "holidays".into(),
            "--json".into(),
            "--no-color".into(),
            year.to_string(),
            "--country".into(),
            country.clone(),
        ];
        if let Some(region) = region.as_deref() {
            args.push("--region".into());
            args.push(region.to_owned());
        }
        match run(&bin, &args) {
            Ok(text) => match parse_holidays(&text) {
                Ok(rows) => {
                    got_holidays = true;
                    holiday_rows.extend(rows);
                }
                Err(err) => remember(&mut fault, &err),
            },
            Err(err) => remember(&mut fault, &err),
        }
        match run(
            &bin,
            &[
                "seasons".into(),
                "--json".into(),
                "--no-color".into(),
                year.to_string(),
            ],
        ) {
            Ok(text) => match parse_seasons(&text) {
                Ok(rows) => seasons.extend(rows),
                Err(err) => remember(&mut fault, &err),
            },
            Err(err) => remember(&mut fault, &err),
        }
    }

    let place_args = place_args(&settings.place);
    let mut sun_args = vec![
        "sun".into(),
        "--json".into(),
        "--no-color".into(),
        date.clone(),
    ];
    sun_args.extend(place_args.iter().cloned());
    match run(&bin, &sun_args) {
        Ok(text) => {
            if let Some((rise, set)) = parse_sun(&text) {
                almanac.sky.sunrise_unix = rise.or(almanac.sky.sunrise_unix);
                almanac.sky.sunset_unix = set.or(almanac.sky.sunset_unix);
                almanac.from_timewarp = true;
            }
        }
        Err(err) => remember(&mut fault, &err),
    }

    let mut moon_args = vec![
        "moon".into(),
        "--json".into(),
        "--no-color".into(),
        date.clone(),
    ];
    moon_args.extend(place_args.iter().cloned());
    match run(&bin, &moon_args) {
        Ok(text) => {
            if let Some(phase) = parse_moon_phase(&text) {
                almanac.sky.moon_phase = phase;
                almanac.from_timewarp = true;
            }
        }
        Err(err) => remember(&mut fault, &err),
    }

    let mut rise_args = vec![
        "rise".into(),
        "--json".into(),
        "--no-color".into(),
        "moon".into(),
        date,
    ];
    rise_args.extend(place_args);
    match run(&bin, &rise_args) {
        Ok(text) => {
            if let Some((rise, set)) = parse_rise(&text) {
                almanac.sky.moonrise_unix = rise.or(almanac.sky.moonrise_unix);
                almanac.sky.moonset_unix = set.or(almanac.sky.moonset_unix);
                almanac.from_timewarp = true;
            }
        }
        Err(err) => remember(&mut fault, &err),
    }

    if let Some(season) = season_label(now, &seasons) {
        almanac.sky.season = season;
        almanac.from_timewarp = true;
    }
    if got_holidays {
        almanac.from_timewarp = true;
        almanac.events = merge_events(&holiday_rows, &seasons, settings.events.as_deref());
    }
    if let Some(err) = fault {
        eprintln!("clockface: {err}");
        almanac.fault = status_fault(&err);
    }
    if almanac.from_timewarp {
        let _ = write_disk(&settings.place, &almanac);
    } else if let Some(cached) = read_disk(&settings.place, now.date_naive()) {
        return cached_with_fault(cached, &almanac.fault);
    }
    almanac
}

fn cached_with_fault(mut cached: Almanac, fault: &str) -> Almanac {
    if !fault.is_empty() {
        cached.fault = fault.to_owned();
    }
    cached
}

fn remember(slot: &mut Option<String>, err: &str) {
    if slot.is_none() {
        *slot = Some(err.to_owned());
    }
}

fn status_fault(err: &str) -> String {
    let lower = err.to_ascii_lowercase();
    if lower.contains("timed out") || lower.contains("unavailable") || lower.contains("not found") {
        "timewarp unavailable".into()
    } else {
        "timewarp failed".into()
    }
}

pub fn locate_bin(configured: Option<&str>) -> Option<PathBuf> {
    if let Some(configured) = configured {
        let path = PathBuf::from(configured);
        return path.is_file().then_some(path);
    }
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths).find_map(|dir| {
        let candidate = dir.join("timewarp");
        candidate.is_file().then_some(candidate)
    })
}

fn place_args(place: &Place) -> Vec<String> {
    let mut args = Vec::new();
    if !place.name.trim().is_empty() {
        args.push("--city".into());
        args.push(place.name.clone());
    }
    args.push("--lat".into());
    args.push(place.latitude.to_string());
    args.push("--lon".into());
    args.push(place.longitude.to_string());
    if let Some(tz) = &place.tz {
        args.push("--tz".into());
        args.push(tz.clone());
    }
    args
}

fn run(bin: &Path, args: &[String]) -> Result<String, String> {
    let child = Command::new(bin)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("timewarp unavailable ({err})"))?;
    let id = child.id();
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    match rx.recv_timeout(TIMEOUT) {
        Ok(Ok(output)) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
        }
        Ok(Ok(output)) => {
            let err = String::from_utf8_lossy(&output.stderr);
            let line = err
                .lines()
                .find(|line| !line.trim().is_empty())
                .unwrap_or("failed");
            Err(format!("timewarp: {line}"))
        }
        Ok(Err(err)) => Err(format!("timewarp unavailable ({err})")),
        Err(_) => {
            let _ = Command::new("kill").args(["-9", &id.to_string()]).status();
            Err("timewarp timed out".into())
        }
    }
}

pub fn parse_holidays(text: &str) -> Result<Vec<CalendarEvent>, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|err| format!("timewarp holidays: {err}"))?;
    let rows = value
        .get("holidays")
        .and_then(|rows| rows.as_array())
        .ok_or_else(|| "timewarp holidays: missing holidays".to_owned())?;
    let mut events = Vec::new();
    for row in rows {
        let Some(date) = row.get("date").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(name) = row.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        if events::parse_date(date).is_none() {
            continue;
        }
        events.push(CalendarEvent {
            date: date.to_owned(),
            title: name.to_owned(),
            time: "all day".into(),
        });
    }
    Ok(events)
}

pub fn parse_seasons(text: &str) -> Result<Vec<(String, f64)>, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|err| format!("timewarp seasons: {err}"))?;
    let rows = value
        .get("events")
        .and_then(|rows| rows.as_array())
        .ok_or_else(|| "timewarp seasons: missing events".to_owned())?;
    let mut points = Vec::new();
    for row in rows {
        let Some(name) = row.get("name").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(time) = row.get("time").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(unix) = parse_unix(time) else {
            continue;
        };
        points.push((name.to_owned(), unix));
    }
    if points.is_empty() {
        return Err("timewarp seasons: no events".into());
    }
    Ok(points)
}

pub fn parse_sun(text: &str) -> Option<(Option<f64>, Option<f64>)> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    Some((
        value.get("sunrise").and_then(json_unix),
        value.get("sunset").and_then(json_unix),
    ))
}

pub fn parse_moon_phase(text: &str) -> Option<f64> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let age = value.get("age_days")?.as_f64()?;
    Some((age / SYNODIC_DAYS).rem_euclid(1.0))
}

pub fn parse_rise(text: &str) -> Option<(Option<f64>, Option<f64>)> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    if let Some(events) = value.get("events").and_then(|v| v.as_array()) {
        let moon = events.iter().find(|row| {
            row.get("body")
                .and_then(|v| v.as_str())
                .map(|body| body.eq_ignore_ascii_case("moon"))
                .unwrap_or(true)
        });
        return moon.map(rise_pair);
    }
    Some(rise_pair(&value))
}

fn rise_pair(value: &serde_json::Value) -> (Option<f64>, Option<f64>) {
    (first_unix(value.get("rise")), first_unix(value.get("set")))
}

fn first_unix(value: Option<&serde_json::Value>) -> Option<f64> {
    let list = value?.as_array()?;
    list.iter().find_map(json_unix)
}

fn json_unix(value: &serde_json::Value) -> Option<f64> {
    value.as_str().and_then(parse_unix)
}

fn parse_unix(text: &str) -> Option<f64> {
    DateTime::parse_from_rfc3339(text)
        .ok()
        .map(|stamp| stamp.timestamp() as f64)
}

fn season_label(now: DateTime<Local>, points: &[(String, f64)]) -> Option<String> {
    let now_unix = now.timestamp() as f64;
    let mut best: Option<(f64, String)> = None;
    for (name, unix) in points {
        if *unix <= now_unix && best.as_ref().map(|(at, _)| *unix > *at).unwrap_or(true) {
            best = Some((*unix, season_word(name).to_owned()));
        }
    }
    best.map(|(_, name)| name).or_else(|| {
        points
            .iter()
            .find(|(name, _)| name.to_ascii_lowercase().contains("equinox"))
            .map(|_| "Winter".to_owned())
    })
}

fn season_word(name: &str) -> &'static str {
    let low = name.to_ascii_lowercase();
    if low.contains("march") {
        "Spring"
    } else if low.contains("june") {
        "Summer"
    } else if low.contains("september") {
        "Autumn"
    } else {
        "Winter"
    }
}

fn merge_events(
    holidays: &[CalendarEvent],
    seasons: &[(String, f64)],
    user: Option<&[CalendarEvent]>,
) -> Vec<CalendarEvent> {
    let mut rows = holidays.to_vec();
    for (name, unix) in seasons {
        let Some(date) = unix_to_local_date(*unix) else {
            continue;
        };
        rows.push(CalendarEvent {
            date: date.format("%Y-%m-%d").to_string(),
            title: name.clone(),
            time: "all day".into(),
        });
    }
    if let Some(user) = user {
        rows.extend(user.iter().cloned());
    }
    let mut seen = std::collections::HashSet::new();
    rows.retain(|row| {
        seen.insert((
            row.date.to_ascii_lowercase(),
            row.title.to_ascii_lowercase(),
        ))
    });
    rows
}

fn unix_to_local_date(unix: f64) -> Option<NaiveDate> {
    Local
        .timestamp_opt(unix.floor() as i64, 0)
        .single()
        .map(|stamp| stamp.date_naive())
}

pub fn read_stored_flags() -> StoredFlags {
    let _guard = enter_env();
    read_stored_flags_locked()
}

fn read_stored_flags_locked() -> StoredFlags {
    let path = config_cache_path();
    let Ok(text) = fs::read_to_string(&path) else {
        return StoredFlags::default();
    };
    parse_stored_flags(&text).unwrap_or_else(|err| {
        eprintln!("clockface: {}: {err}", path.display());
        StoredFlags::default()
    })
}

pub fn parse_stored_flags(text: &str) -> Result<StoredFlags, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|err| format!("stored flags: {err}"))?;
    let obj = value
        .as_object()
        .ok_or_else(|| "stored flags are not an object".to_owned())?;
    let country = json_string(obj.get("country")).or_else(|| json_string(obj.get("holidays")));
    Ok(StoredFlags {
        city: json_string(obj.get("city")),
        lat: json_number(obj.get("lat")),
        lon: json_number(obj.get("lon")),
        tz: json_string(obj.get("tz")),
        country,
        region: json_string(obj.get("region")),
    })
}

fn json_string(value: Option<&serde_json::Value>) -> Option<String> {
    let text = value?.as_str()?.trim();
    (!text.is_empty()).then(|| text.to_owned())
}

fn json_number(value: Option<&serde_json::Value>) -> Option<f64> {
    value?.as_f64()
}

/// `$TIMEWARP_CACHE`, else `$XDG_CONFIG_HOME/timewarp/cache.json`, else `~/.config/timewarp/cache.json`.
pub fn config_cache_path() -> PathBuf {
    if let Some(path) = env_path("TIMEWARP_CACHE") {
        return path;
    }
    user_config_dir().join("timewarp").join("cache.json")
}

fn user_config_dir() -> PathBuf {
    if let Some(path) = env_path("XDG_CONFIG_HOME") {
        return path;
    }
    if cfg!(windows) {
        if let Some(path) = env_path("APPDATA") {
            return path;
        }
        return home_dir().join("AppData").join("Roaming");
    }
    home_dir().join(".config")
}

fn user_cache_dir() -> PathBuf {
    if let Some(path) = env_path("XDG_CACHE_HOME") {
        return path;
    }
    if cfg!(windows) {
        if let Some(path) = env_path("LOCALAPPDATA") {
            return path;
        }
        return home_dir().join("AppData").join("Local");
    }
    home_dir().join(".cache")
}

fn clockface_cache_file() -> PathBuf {
    user_cache_dir()
        .join("timewarp")
        .join("clockface")
        .join("almanac.json")
}

fn env_path(key: &str) -> Option<PathBuf> {
    let value = std::env::var_os(key)?;
    if value.is_empty() {
        return None;
    }
    Some(PathBuf::from(value))
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn write_disk(place: &Place, almanac: &Almanac) -> Result<(), String> {
    let path = clockface_cache_file();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| err.to_string())?;
    }
    let body = serde_json::json!({
        "date": almanac.civil_date.format("%Y-%m-%d").to_string(),
        "place": place.name,
        "latitude": place.latitude,
        "longitude": place.longitude,
        "season": almanac.sky.season,
        "moon_phase": almanac.sky.moon_phase,
        "sunrise": almanac.sky.sunrise_unix,
        "sunset": almanac.sky.sunset_unix,
        "moonrise": almanac.sky.moonrise_unix,
        "moonset": almanac.sky.moonset_unix,
        "events": almanac.events,
    });
    fs::write(
        &path,
        serde_json::to_string_pretty(&body).unwrap_or_default(),
    )
    .map_err(|err| err.to_string())
}

fn read_disk(place: &Place, today: NaiveDate) -> Option<Almanac> {
    let text = fs::read_to_string(clockface_cache_file()).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let date = value.get("date")?.as_str()?;
    let civil = events::parse_date(date)?;
    if civil != today {
        return None;
    }
    let name = value.get("place").and_then(|v| v.as_str()).unwrap_or("");
    if !name.eq_ignore_ascii_case(&place.name) {
        return None;
    }
    let events = serde_json::from_value(value.get("events")?.clone()).unwrap_or_default();
    let sky = DaySky {
        sunrise_unix: value.get("sunrise").and_then(|v| v.as_f64()),
        sunset_unix: value.get("sunset").and_then(|v| v.as_f64()),
        season: value
            .get("season")
            .and_then(|v| v.as_str())
            .unwrap_or("Winter")
            .to_owned(),
        moon_phase: value
            .get("moon_phase")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0),
        moonrise_unix: value.get("moonrise").and_then(|v| v.as_f64()),
        moonset_unix: value.get("moonset").and_then(|v| v.as_f64()),
    };
    Some(Almanac {
        civil_date: civil,
        events,
        sky,
        fault: String::new(),
        from_timewarp: true,
    })
}

/// City lookup used while resolving settings. One `sun --json` call, then the built-in table.
fn lookup_city(configured_bin: Option<&str>, name: &str) -> Lookup {
    let Some(bin) = locate_bin(configured_bin) else {
        return Lookup::Unavailable;
    };
    match run(
        &bin,
        &[
            "sun".into(),
            "--json".into(),
            "--no-color".into(),
            "2026-01-15".into(),
            "--city".into(),
            name.to_owned(),
        ],
    ) {
        Ok(text) => parse_city(&text)
            .map(Lookup::Found)
            .unwrap_or(Lookup::Unknown),
        Err(err) => {
            let lower = err.to_ascii_lowercase();
            if lower.contains("unknown city") {
                Lookup::Unknown
            } else {
                Lookup::Unavailable
            }
        }
    }
}

fn parse_city(text: &str) -> Option<ResolvedCity> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    Some(ResolvedCity {
        name: value.get("place")?.as_str()?.to_owned(),
        lat: value.get("latitude")?.as_f64()?,
        lon: value.get("longitude")?.as_f64()?,
        tz: value.get("tz").and_then(|v| v.as_str()).map(str::to_owned),
    })
}

fn builtin_city(name: &str) -> Option<ResolvedCity> {
    let (label, lat, lon) = match name.trim().to_ascii_lowercase().as_str() {
        "indianapolis" | "indy" => ("Indianapolis", 39.7684, -86.1581),
        "chicago" => ("Chicago", 41.8781, -87.6298),
        "new york" | "nyc" | "newyork" => ("New York", 40.7128, -74.0060),
        "london" => ("London", 51.5074, -0.1278),
        "denver" => ("Denver", 39.7392, -104.9903),
        _ => return None,
    };
    Some(ResolvedCity {
        name: label.into(),
        lat,
        lon,
        tz: None,
    })
}

/// Resolve a place from the built-in default, then stored flags, the TOML, and the CLI.
pub fn resolve_place(
    base: Place,
    stored: &StoredFlags,
    file: &PlaceLayer,
    over: &PlaceLayer,
    configured_bin: Option<&str>,
) -> (Place, Option<String>) {
    let mut layers = Vec::new();
    if stored.has_place() {
        layers.push(stored.place_layer());
    }
    if !file.is_empty() {
        layers.push(file.clone());
    }
    if !over.is_empty() {
        layers.push(over.clone());
    }
    fold_place(base, &layers, |name| lookup_city(configured_bin, name))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOLIDAYS: &str = r#"{
      "country": "US",
      "holidays": [
        {"date": "2026-01-01", "name": "New Year's Day"},
        {"date": "2026-01-19", "name": "Martin Luther King Jr. Day"},
        {"date": "2026-02-16", "name": "Washington's Birthday"},
        {"date": "2026-05-25", "name": "Memorial Day"},
        {"date": "2026-06-19", "name": "Juneteenth National Independence Day"},
        {"date": "2026-07-03", "name": "Independence Day (observed)"},
        {"date": "2026-07-04", "name": "Independence Day"},
        {"date": "2026-09-07", "name": "Labor Day"},
        {"date": "2026-10-12", "name": "Columbus Day"},
        {"date": "2026-11-11", "name": "Veterans Day"},
        {"date": "2026-11-26", "name": "Thanksgiving Day"},
        {"date": "2026-12-25", "name": "Christmas Day"}
      ],
      "year": 2026
    }"#;

    const SEASONS: &str = r#"{
      "events": [
        {"name": "March equinox", "time": "2026-03-20T14:32:55+00:00"},
        {"name": "June solstice", "time": "2026-06-21T08:16:13+00:00"},
        {"name": "September equinox", "time": "2026-09-23T00:06:15+00:00"},
        {"name": "December solstice", "time": "2026-12-21T20:46:17+00:00"}
      ],
      "year": 2026
    }"#;

    #[test]
    fn holidays_and_seasons_parse_the_cli_shape() {
        let holidays = parse_holidays(HOLIDAYS).unwrap();
        assert_eq!(holidays[0].title, "New Year's Day");
        assert_eq!(holidays[1].date, "2026-01-19");
        assert_eq!(holidays[1].time, "all day");
        assert!(holidays
            .iter()
            .any(|row| row.title == "Washington's Birthday"));
        let seasons = parse_seasons(SEASONS).unwrap();
        assert_eq!(seasons[0].0, "March equinox");
        assert!(seasons[0].1 > 0.0);
        let january = Local
            .with_ymd_and_hms(2026, 1, 15, 10, 8, 37)
            .single()
            .unwrap();
        assert_eq!(season_label(january, &seasons).as_deref(), Some("Winter"));
        let july = Local
            .with_ymd_and_hms(2026, 7, 4, 12, 0, 0)
            .single()
            .unwrap();
        assert_eq!(season_label(july, &seasons).as_deref(), Some("Summer"));
        let merged = merge_events(&holidays, &seasons, None);
        let upcoming = events::upcoming(&merged, january.date_naive(), 4);
        assert_eq!(upcoming[0].title, "Martin Luther King Jr. Day");
        assert_eq!(upcoming[1].title, "Washington's Birthday");
        assert_eq!(upcoming[2].title, "March equinox");
        assert_eq!(
            events::event_mark(&upcoming[2].title, &upcoming[2].time),
            "\u{1F31E}"
        );
    }

    #[test]
    fn sun_moon_and_rise_parse_the_cli_shape() {
        let sun = r#"{"sunrise":"2026-01-15T08:04:14-05:00","sunset":"2026-01-15T17:44:28-05:00"}"#;
        let (rise, set) = parse_sun(sun).unwrap();
        assert!(rise.unwrap() < set.unwrap());
        let moon = r#"{"age_days":26.581,"phase":"Waning Crescent"}"#;
        let phase = parse_moon_phase(moon).unwrap();
        assert!(phase > 0.85 && phase < 0.95, "{phase}");
        let rise_json = r#"{"body":"moon","rise":["2026-01-15T05:53:47-05:00"],"set":["2026-01-15T14:40:16-05:00"]}"#;
        let (moonrise, moonset) = parse_rise(rise_json).unwrap();
        assert!(moonrise.unwrap() < moonset.unwrap());
    }

    #[test]
    fn cli_beats_toml_which_beats_stored_flags() {
        let stored = PlaceLayer {
            city: Some("Chicago".into()),
            ..PlaceLayer::default()
        };
        let file = PlaceLayer {
            city: Some("London".into()),
            longitude: Some(-1.0),
            ..PlaceLayer::default()
        };
        let over = PlaceLayer {
            latitude: Some(12.0),
            ..PlaceLayer::default()
        };
        let (place, fault) = fold_place(Place::indianapolis(), &[stored, file, over], |name| {
            let (lat, lon) = match name {
                "Chicago" => (41.8781, -87.6298),
                "London" => (51.5074, -0.1278),
                _ => return Lookup::Unknown,
            };
            Lookup::Found(ResolvedCity {
                name: name.into(),
                lat,
                lon,
                tz: Some("UTC".into()),
            })
        });
        assert!(fault.is_none());
        assert_eq!(place.name, "London");
        assert!((place.latitude - 12.0).abs() < 1e-9);
        assert!((place.longitude - -1.0).abs() < 1e-9);
        assert_eq!(place.tz.as_deref(), Some("UTC"));
    }

    #[test]
    fn stored_city_beats_the_built_in_default() {
        let (place, fault) = fold_place(
            Place::indianapolis(),
            &[PlaceLayer {
                city: Some("Denver".into()),
                ..PlaceLayer::default()
            }],
            |_| {
                Lookup::Found(ResolvedCity {
                    name: "Denver".into(),
                    lat: 39.7392,
                    lon: -104.9903,
                    tz: Some("America/Denver".into()),
                })
            },
        );
        assert!(fault.is_none());
        assert_eq!(place.name, "Denver");
        assert!((place.latitude - 39.7392).abs() < 1e-9);
    }

    #[test]
    fn a_missing_binary_keeps_a_known_city() {
        let (place, fault) = fold_place(
            Place::indianapolis(),
            &[PlaceLayer {
                city: Some("indy".into()),
                ..PlaceLayer::default()
            }],
            |_| Lookup::Unavailable,
        );
        assert!(fault.is_none(), "{fault:?}");
        assert_eq!(place.name, "Indianapolis");
    }

    #[test]
    fn stored_flags_read_city_country_and_region() {
        let flags = parse_stored_flags(
            r#"{"city":"Indianapolis","country":"US","region":"IN","holidays":"GB"}"#,
        )
        .unwrap();
        assert_eq!(flags.city.as_deref(), Some("Indianapolis"));
        assert_eq!(flags.holiday_country(), "US");
        assert_eq!(flags.region.as_deref(), Some("IN"));
        let from_holidays = parse_stored_flags(r#"{"holidays":"GB"}"#).unwrap();
        assert_eq!(from_holidays.holiday_country(), "GB");
    }

    #[test]
    fn config_path_follows_timewarp_cache_then_xdg() {
        with_env_lock(|| {
            let dir = std::env::temp_dir().join(format!(
                "clockface-tw-{}-{}",
                std::process::id(),
                "paths"
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(dir.join("timewarp")).unwrap();
            let cache = dir.join("cache.json");
            fs::write(&cache, "{}\n").unwrap();
            std::env::set_var("TIMEWARP_CACHE", &cache);
            assert_eq!(config_cache_path(), cache);
            std::env::remove_var("TIMEWARP_CACHE");
            std::env::set_var("XDG_CONFIG_HOME", &dir);
            assert_eq!(config_cache_path(), dir.join("timewarp").join("cache.json"));
            std::env::remove_var("XDG_CONFIG_HOME");
            let _ = fs::remove_dir_all(&dir);
        });
    }
}
