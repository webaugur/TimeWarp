//! Fullscreen kiosk shell around [`clockface::paint`].

use std::path::PathBuf;
use std::time::Duration;

use chrono::Local;
use clap::Parser;
use eframe::egui::{self, Color32, Vec2};
use eframe::Renderer;

use clockface::{
    install_fonts, load_settings, paint, render_rgba, write_png, Settings, SettingsOverride,
};

#[derive(Parser, Debug)]
#[command(
    name = "clockface",
    version,
    about = "Roman-numeral grandfather clock for a Raspberry Pi kiosk"
)]
struct Cli {
    /// TOML file with diameter, center_x, center_y, and windowed. Flags override it.
    #[arg(long)]
    config: Option<PathBuf>,

    /// Face diameter in pixels (default 600).
    #[arg(long)]
    diameter: Option<f32>,

    /// Clock center X in pixels from the left. Default: horizontal center of the window.
    #[arg(long)]
    center_x: Option<f32>,

    /// Clock center Y in pixels from the top. Default: diameter/2, so the face touches the top.
    #[arg(long)]
    center_y: Option<f32>,

    /// Open a decorated window instead of fullscreen.
    #[arg(long)]
    windowed: bool,

    /// Force fullscreen. Wins over a config file that sets `windowed = true`.
    #[arg(long)]
    fullscreen: bool,

    /// Write one PNG at local time 10:08:37 and exit. No window is opened.
    #[arg(long, value_name = "FILE")]
    screenshot: Option<PathBuf>,

    /// Window or screenshot width in pixels.
    #[arg(long, default_value_t = 1920)]
    width: u32,

    /// Window or screenshot height in pixels.
    #[arg(long, default_value_t = 1080)]
    height: u32,
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
    let settings = load_settings(
        cli.config.as_deref(),
        &SettingsOverride {
            diameter: cli.diameter,
            center_x: cli.center_x,
            center_y: cli.center_y,
            windowed,
        },
    )?;

    if let Some(path) = cli.screenshot.as_deref() {
        let center = settings.center(Vec2::new(cli.width as f32, cli.height as f32));
        let rgba = render_rgba(
            cli.width,
            cli.height,
            center,
            settings.diameter,
            clockface::screenshot_time(),
        )?;
        write_png(path, cli.width, cli.height, &rgba)?;
        println!(
            "wrote {} ({}x{}, diameter {}, center {:.1},{:.1}, local 10:08:37)",
            path.display(),
            cli.width,
            cli.height,
            settings.diameter,
            center.x,
            center.y
        );
        return Ok(());
    }

    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Clockface")
            .with_inner_size([cli.width as f32, cli.height as f32])
            .with_fullscreen(!settings.windowed)
            .with_decorations(settings.windowed)
            .with_resizable(true),
        renderer: Renderer::Glow,
        vsync: true,
        ..Default::default()
    };

    eframe::run_native(
        "clockface",
        native,
        Box::new(move |cc| {
            install_fonts(&cc.egui_ctx);
            Ok(Box::new(ClockApp { settings }))
        }),
    )
    .map_err(|err| err.to_string())
}

struct ClockApp {
    settings: Settings,
}

impl eframe::App for ClockApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 1.0]
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // ~60 Hz. Vsync caps this at the display refresh; the timer keeps the
        // hour and minute hands moving when the window would otherwise idle,
        // and it feeds the second-hand bounce in the first 250 ms of each second.
        ctx.request_repaint_after(Duration::from_nanos(16_666_667));

        let settings = self.settings.clone();
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::BLACK))
            .show(ctx, |ui| {
                let screen = ctx.screen_rect();
                let center = settings.center(screen.size());
                paint(ui.painter(), center, settings.diameter, Local::now());
            });
    }
}
