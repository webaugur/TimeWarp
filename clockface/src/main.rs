//! Kiosk shell around the shared clock painter.
//!
//! The default build opens an eframe window (X11 or Wayland, glow). The `kms`
//! feature on Linux starts eboard instead, which talks to DRM/KMS or the
//! framebuffer and does not need a display server. `--screenshot` uses the
//! software rasterizer in either build.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::Local;
use clap::Parser;

use clockface::{
    example_events, fetch_almanac, fetch_weather, gather_status, install_fonts, load_cache,
    load_cached_sun, load_settings, load_sun, now_unix, paint_kiosk, render_rgba_with,
    repaint_delay, rotation_from_env, scanout_from_logical, screenshot_time, start_audio,
    store_cache, view_mounted_panel, write_png, Almanac, AlmanacSlot, DisplayRotation, KioskModel,
    Place, Settings, SettingsOverride, SoundFault, StatusLine, Units, WeatherReport,
    SUN_REFRESH_SECS,
};

#[derive(Parser, Debug)]
#[command(
    name = "clockface",
    version,
    about = "Roman-numeral grandfather clock for a Raspberry Pi kiosk"
)]
struct Cli {
    /// TOML file. Flags override it. See clockface.example.toml.
    #[arg(long)]
    config: Option<std::path::PathBuf>,

    /// Face diameter in pixels. Omit it and the face is sized so the whole column fits the window.
    #[arg(long)]
    diameter: Option<f32>,

    /// Clock center X in pixels from the left. Default: horizontal center of the window.
    #[arg(long)]
    center_x: Option<f32>,

    /// Clock center Y in pixels from the top. Default: diameter/2, so the face touches the top.
    #[arg(long)]
    center_y: Option<f32>,

    /// Open a decorated window instead of fullscreen.
    ///
    /// Ignored on the `kms` build: eboard is always fullscreen on the display.
    #[arg(long)]
    windowed: bool,

    /// Force fullscreen. Wins over a config file that sets `windowed = true`.
    #[arg(long)]
    fullscreen: bool,

    /// Write one PNG of the full kiosk at local time 10:08:37 and exit. No display is opened.
    #[arg(long, value_name = "FILE")]
    screenshot: Option<std::path::PathBuf>,

    /// Window or screenshot width in pixels. The kms build uses the display mode instead.
    ///
    /// A window defaults to 1200, which fits a 1080p desktop with a panel and a dock.
    /// `--screenshot` defaults to 1920 when this is omitted.
    #[arg(long)]
    width: Option<u32>,

    /// Window or screenshot height in pixels. The kms build uses the display mode instead.
    ///
    /// A window defaults to 800 so the title bar still fits under a desktop panel.
    /// `--screenshot` defaults to 1080 when this is omitted.
    #[arg(long)]
    height: Option<u32>,

    /// Latitude, decimal degrees. Overrides --city and the config file.
    #[arg(long)]
    lat: Option<f64>,

    /// Longitude, decimal degrees, east-positive. Overrides --city and the config file.
    #[arg(long)]
    lon: Option<f64>,

    /// Named place: indianapolis, chicago, new york, london, denver.
    #[arg(long)]
    city: Option<String>,

    /// imperial (°F, mph) or metric (°C, km/h). Default imperial.
    #[arg(long, value_parser = ["imperial", "metric"])]
    units: Option<String>,

    /// Do not play ticking or chimes.
    #[arg(long)]
    no_audio: bool,

    /// Clockwise kms rotation in degrees: 0, 90, 180, or 270.
    ///
    /// Overrides `CLOCKFACE_ROTATION`, `EBOARD_ROTATION`, and `[display] rotation`.
    /// The desktop window is not rotated. With `--screenshot`, width and height
    /// are the native panel and the PNG is the picture on the turned panel.
    #[arg(long, value_parser = parse_rotation_arg)]
    rotation: Option<i32>,
}

fn parse_rotation_arg(text: &str) -> Result<i32, String> {
    let degrees: i32 = text
        .parse()
        .map_err(|_| format!("rotation must be 0, 90, 180, or 270, not {text:?}"))?;
    if DisplayRotation::from_degrees(degrees).is_none() {
        return Err(format!("rotation must be 0, 90, 180, or 270, not {text:?}"));
    }
    Ok(degrees)
}

fn main() {
    if let Err(err) = run() {
        eprintln!("clockface: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse();
    let windowed = if cli.fullscreen {
        Some(false)
    } else if cli.windowed {
        Some(true)
    } else {
        None
    };
    let units = match cli.units.as_deref() {
        Some(text) => Some(Units::parse(text).ok_or_else(|| format!("unknown units {text:?}"))?),
        None => None,
    };
    let rotation = match cli.rotation {
        Some(degrees) => DisplayRotation::from_degrees(degrees),
        None => rotation_from_env()?,
    };
    #[cfg(all(feature = "kms", target_os = "linux"))]
    let pin_rotation = rotation.is_some();
    let settings = load_settings(
        cli.config.as_deref(),
        &SettingsOverride {
            diameter: cli.diameter,
            center_x: cli.center_x,
            center_y: cli.center_y,
            windowed,
            city: cli.city.clone(),
            latitude: cli.lat,
            longitude: cli.lon,
            units,
            audio_enabled: if cli.no_audio { Some(false) } else { None },
            rotation,
        },
    )?;

    if let Some(path) = cli.screenshot.as_deref() {
        let native_w = cli.width.unwrap_or(1920);
        let native_h = cli.height.unwrap_or(1080);
        let (width, height) = settings.rotation.logical_size(native_w, native_h);
        let viewport = egui::vec2(width as f32, height as f32);
        let (center, diameter) = settings.placement(viewport);
        let now = screenshot_time();
        let almanac = fetch_almanac(&settings, now);
        let sun = load_sun();
        let rgba = render_rgba_with(
            width,
            height,
            center,
            diameter,
            now,
            &settings.place,
            sun,
            Some(&almanac),
        )?;
        let (rgba, width, height) = if settings.rotation == DisplayRotation::Deg0 {
            (rgba, width, height)
        } else {
            let (scanout, panel_w, panel_h) =
                scanout_from_logical(&rgba, width, height, settings.rotation);
            view_mounted_panel(&scanout, panel_w, panel_h, settings.rotation)
        };
        write_png(path, width, height, &rgba)?;
        println!(
            "wrote {} ({}x{}, rotation {}, diameter {:.1}, center {:.1},{:.1}, local 10:08:37)",
            path.display(),
            width,
            height,
            settings.rotation.degrees(),
            diameter,
            center.x,
            center.y
        );
        return Ok(());
    }

    #[cfg(all(feature = "kms", target_os = "linux"))]
    {
        return run_kms(settings, pin_rotation);
    }
    #[cfg(all(feature = "desktop", not(all(feature = "kms", target_os = "linux"))))]
    {
        // 1200×800 fits a 1080p desktop under a panel, a title bar, and a dock.
        // A requested 1920×1080 client is taller than that work area, so the
        // frames and the status bar end up off the window.
        return run_desktop(
            settings,
            cli.width.unwrap_or(1200),
            cli.height.unwrap_or(800),
        );
    }
    #[cfg(not(any(feature = "desktop", all(feature = "kms", target_os = "linux"))))]
    {
        let _ = settings;
        Err("this build has no display backend; pass --screenshot or enable desktop or kms".into())
    }
}

struct ClockApp {
    settings: Settings,
    weather: Arc<Mutex<WeatherReport>>,
    sun: Arc<Mutex<Option<std::sync::Arc<clockface::SunImage>>>>,
    sun_at: Instant,
    almanac: Arc<Mutex<AlmanacSlot>>,
    status: StatusLine,
    status_at: Instant,
    fetching: Arc<AtomicBool>,
    fetching_sun: Arc<AtomicBool>,
    fetching_almanac: Arc<AtomicBool>,
    sound_fault: SoundFault,
}

impl ClockApp {
    fn new(settings: Settings) -> Self {
        let weather = load_cache(&settings.place)
            .unwrap_or_else(|| WeatherReport::unavailable(&settings.place));
        let sound_fault = start_audio(&settings.audio, &settings.leds);
        let initial = Almanac::builtin(Local::now(), &settings.place, settings.events.as_deref());
        Self {
            settings,
            sound_fault,
            weather: Arc::new(Mutex::new(weather)),
            sun: Arc::new(Mutex::new(load_cached_sun())),
            sun_at: Instant::now()
                .checked_sub(Duration::from_secs(SUN_REFRESH_SECS))
                .unwrap_or_else(Instant::now),
            almanac: Arc::new(Mutex::new(AlmanacSlot {
                almanac: initial,
                attempted: None,
            })),
            status: gather_status(),
            status_at: Instant::now(),
            fetching: Arc::new(AtomicBool::new(false)),
            fetching_sun: Arc::new(AtomicBool::new(false)),
            fetching_almanac: Arc::new(AtomicBool::new(false)),
        }
    }
}

fn show_clock(ui: &mut egui::Ui, app: &mut ClockApp) {
    let now = Local::now();
    ui.ctx().request_repaint_after(repaint_delay(now));
    if app.status_at.elapsed() >= Duration::from_secs(30) {
        app.status = gather_status();
        app.status_at = Instant::now();
    }
    refresh_weather_if_stale(app);
    refresh_sun_if_stale(app);
    refresh_almanac_if_stale(app);
    let weather = app.weather.lock().expect("weather lock").clone();
    let sun = app.sun.lock().expect("sun lock").clone();
    let held = app.almanac.lock().expect("almanac lock").almanac.clone();
    let mut status = app.status.clone();
    status.alert = join_faults(&weather.note, &held.fault);
    if let Some(fault) = clockface::gauge_fault(&weather) {
        status.alert = join_faults(&status.alert, fault);
    }
    if let Ok(sound) = app.sound_fault.lock() {
        let sound = sound.trim();
        if !sound.is_empty() {
            status.alert = join_faults(&status.alert, sound);
        }
    }
    let events = if held.from_timewarp {
        held.events.clone()
    } else {
        app.settings.events.clone().unwrap_or_else(example_events)
    };
    let sky = if held.from_timewarp {
        held.sky.clone()
    } else {
        clockface::DaySky::builtin(now, &app.settings.place)
    };
    let screen = ui.ctx().viewport_rect();
    let (center, diameter) = app.settings.placement(screen.size());
    let model = KioskModel {
        now,
        place: app.settings.place.clone(),
        weather,
        events,
        sky,
        status,
        sun,
        barometer_hours: app.settings.barometer_hours,
    };
    paint_kiosk(ui.painter(), screen, center, diameter, &model);
}

fn join_faults(weather: &str, timewarp: &str) -> String {
    match (weather.trim().is_empty(), timewarp.trim().is_empty()) {
        (true, true) => String::new(),
        (false, true) => weather.trim().to_owned(),
        (true, false) => timewarp.trim().to_owned(),
        (false, false) => format!("{}  {}", weather.trim(), timewarp.trim()),
    }
}

fn refresh_almanac_if_stale(app: &ClockApp) {
    let today = Local::now().date_naive();
    let due = {
        let slot = app.almanac.lock().expect("almanac lock");
        !slot.fresh_for(today)
    };
    if !due {
        return;
    }
    if app.fetching_almanac.swap(true, Ordering::SeqCst) {
        return;
    }
    let settings = app.settings.clone();
    let slot = Arc::clone(&app.almanac);
    let flag = Arc::clone(&app.fetching_almanac);
    std::thread::spawn(move || {
        let almanac = fetch_almanac(&settings, Local::now());
        let mut held = slot.lock().expect("almanac lock");
        held.almanac = almanac;
        held.attempted = Some(Instant::now());
        flag.store(false, Ordering::SeqCst);
    });
}

fn refresh_sun_if_stale(app: &mut ClockApp) {
    if app.sun_at.elapsed() < Duration::from_secs(SUN_REFRESH_SECS) {
        return;
    }
    if app.fetching_sun.swap(true, Ordering::SeqCst) {
        return;
    }
    app.sun_at = Instant::now();
    let slot = Arc::clone(&app.sun);
    let flag = Arc::clone(&app.fetching_sun);
    std::thread::spawn(move || {
        if let Some(image) = load_sun() {
            *slot.lock().expect("sun lock") = Some(image);
        }
        flag.store(false, Ordering::SeqCst);
    });
}

fn refresh_weather_if_stale(app: &ClockApp) {
    let weather = app.weather.lock().expect("weather lock").clone();
    let refresh = (app.settings.refresh_minutes as i64).saturating_mul(60);
    if weather.is_fresh(now_unix(), refresh) {
        return;
    }
    if app.fetching.swap(true, Ordering::SeqCst) {
        return;
    }
    let place: Place = app.settings.place.clone();
    let slot = Arc::clone(&app.weather);
    let flag = Arc::clone(&app.fetching);
    std::thread::spawn(move || {
        match fetch_weather(&place) {
            Ok(report) => {
                store_cache(&report);
                *slot.lock().expect("weather lock") = report;
            }
            Err(err) => {
                slot.lock().expect("weather lock").note = err;
            }
        }
        flag.store(false, Ordering::SeqCst);
    });
}

#[cfg(all(feature = "desktop", not(all(feature = "kms", target_os = "linux"))))]
fn run_desktop(settings: Settings, width: u32, height: u32) -> Result<(), String> {
    if settings.rotation != DisplayRotation::Deg0 {
        eprintln!(
            "clockface: display rotation {} applies to the kms build; this window stays unrotated",
            settings.rotation.degrees()
        );
    }
    let windowed = settings.windowed;
    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Clockface")
            .with_inner_size([width as f32, height as f32])
            .with_fullscreen(!windowed)
            .with_decorations(windowed)
            .with_resizable(true),
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };
    eframe::run_native(
        "clockface",
        native,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            Ok(Box::new(ClockApp::new(settings)))
        }),
    )
    .map_err(|err| err.to_string())
}

#[cfg(all(feature = "desktop", not(all(feature = "kms", target_os = "linux"))))]
impl eframe::App for ClockApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 1.0]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        show_clock(ui, self);
    }
}

#[cfg(all(feature = "kms", target_os = "linux"))]
fn run_kms(settings: Settings, pin_rotation: bool) -> Result<(), String> {
    if settings.windowed {
        eprintln!("clockface: --windowed is ignored; the kms build is always fullscreen");
    }
    // eboard::run reads EBOARD_ROTATION after Options. Pin an explicit
    // --rotation or CLOCKFACE_ROTATION so that variable cannot replace it.
    // A file-only value stays in Options and still yields to EBOARD_ROTATION.
    if pin_rotation {
        std::env::set_var("EBOARD_ROTATION", settings.rotation.degrees().to_string());
    }
    let mut options = eboard::Options {
        backend: eboard::Backend::Auto,
        vsync: true,
        ..eboard::Options::default()
    };
    options.output.rotation = match settings.rotation {
        DisplayRotation::Deg0 => eboard::Rotation::Deg0,
        DisplayRotation::Deg90 => eboard::Rotation::Deg90,
        DisplayRotation::Deg180 => eboard::Rotation::Deg180,
        DisplayRotation::Deg270 => eboard::Rotation::Deg270,
    };
    eboard::run(options, move |cc| {
        install_fonts(&cc.egui_ctx);
        Ok(Box::new(ClockApp::new(settings)) as Box<dyn eboard::App>)
    })
    .map_err(|err| err.to_string())
}

#[cfg(all(feature = "kms", target_os = "linux"))]
impl eboard::App for ClockApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 1.0]
    }

    fn ui(&mut self, ui: &mut egui::Ui, _board: &mut eboard::Board) {
        show_clock(ui, self);
    }
}
