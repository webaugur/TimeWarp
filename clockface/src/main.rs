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
    example_events, fetch_weather, gather_status, install_fonts, load_cache, load_settings,
    now_unix, paint_kiosk, render_rgba, store_cache, write_png, KioskModel, Place, Settings,
    SettingsOverride, StatusLine, Units, WeatherReport,
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
        },
    )?;

    if let Some(path) = cli.screenshot.as_deref() {
        let width = cli.width.unwrap_or(1920);
        let height = cli.height.unwrap_or(1080);
        let viewport = egui::vec2(width as f32, height as f32);
        let (center, diameter) = settings.placement(viewport);
        let rgba = render_rgba(
            width,
            height,
            center,
            diameter,
            clockface::screenshot_time(),
            &settings.place,
        )?;
        write_png(path, width, height, &rgba)?;
        println!(
            "wrote {} ({}x{}, diameter {:.1}, center {:.1},{:.1}, local 10:08:37)",
            path.display(),
            width,
            height,
            diameter,
            center.x,
            center.y
        );
        return Ok(());
    }

    #[cfg(all(feature = "kms", target_os = "linux"))]
    {
        return run_kms(settings);
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
    status: StatusLine,
    status_at: Instant,
    fetching: Arc<AtomicBool>,
}

impl ClockApp {
    fn new(settings: Settings) -> Self {
        let weather = load_cache(&settings.place)
            .unwrap_or_else(|| WeatherReport::unavailable(&settings.place));
        Self {
            settings,
            weather: Arc::new(Mutex::new(weather)),
            status: gather_status(),
            status_at: Instant::now(),
            fetching: Arc::new(AtomicBool::new(false)),
        }
    }
}

fn show_clock(ui: &mut egui::Ui, app: &mut ClockApp) {
    ui.ctx()
        .request_repaint_after(Duration::from_nanos(16_666_667));
    if app.status_at.elapsed() >= Duration::from_secs(30) {
        app.status = gather_status();
        app.status_at = Instant::now();
    }
    refresh_weather_if_stale(app);
    let weather = app.weather.lock().expect("weather lock").clone();
    let mut status = app.status.clone();
    if !weather.note.is_empty() {
        status.alert = weather.note.clone();
    }
    let events = app.settings.events.clone().unwrap_or_else(example_events);
    let screen = ui.ctx().viewport_rect();
    let (center, diameter) = app.settings.placement(screen.size());
    let model = KioskModel {
        now: Local::now(),
        place: app.settings.place.clone(),
        weather,
        events,
        status,
    };
    paint_kiosk(ui.painter(), screen, center, diameter, &model);
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
fn run_kms(settings: Settings) -> Result<(), String> {
    if settings.windowed {
        eprintln!("clockface: --windowed is ignored; the kms build is always fullscreen");
    }
    let options = eboard::Options {
        backend: eboard::Backend::Auto,
        vsync: true,
        ..eboard::Options::default()
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
