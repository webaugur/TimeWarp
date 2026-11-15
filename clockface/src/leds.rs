//! Chime lights for the 20-pixel WS281x strip on the old Pi clock.
//!
//! Effects run on their own thread. The chime scheduler only sends a cue, so a
//! rainbow or a chase never blocks the clock or the audio. `none` is the
//! default and does not open a device. `spi` writes a WS281x bitstream on
//! Linux when the `leds-spi` feature is on (the kms/Pi build). `serial` writes
//! a small framed payload to a tty for a future Arduino. A device that cannot
//! be opened is logged, and the strip stays dark.

use std::io::Write;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use egui::{Color32, Painter, Pos2, Rect};

/// One logical color, before channel order and brightness.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };

    fn scale(self, brightness: u8) -> Self {
        Self {
            r: scale_channel(self.r, brightness),
            g: scale_channel(self.g, brightness),
            b: scale_channel(self.b, brightness),
        }
    }
}

/// Adafruit `wheel`. `pos` is 0..255.
pub fn wheel(pos: u8) -> Rgb {
    let pos = u16::from(pos);
    if pos < 85 {
        Rgb {
            r: (pos * 3) as u8,
            g: (255 - pos * 3) as u8,
            b: 0,
        }
    } else if pos < 170 {
        let p = pos - 85;
        Rgb {
            r: (255 - p * 3) as u8,
            g: 0,
            b: (p * 3) as u8,
        }
    } else {
        let p = pos - 170;
        Rgb {
            r: 0,
            g: (p * 3) as u8,
            b: (255 - p * 3) as u8,
        }
    }
}

pub fn rainbow_frame(count: usize, j: u8) -> Vec<Rgb> {
    (0..count)
        .map(|i| wheel(((i as u16 + u16::from(j)) & 255) as u8))
        .collect()
}

/// Every third pixel, starting at `q` (0, 1, or 2). The rest are off.
pub fn theater_frame(count: usize, q: usize, color: Rgb) -> Vec<Rgb> {
    let mut pixels = vec![Rgb::BLACK; count];
    let mut i = q;
    while i < count {
        pixels[i] = color;
        i += 3;
    }
    pixels
}

/// Old unused experiment: the whole strip at `Color(200, 160, 80)`.
pub fn sky_frame(count: usize) -> Vec<Rgb> {
    vec![
        Rgb {
            r: 200,
            g: 160,
            b: 80
        };
        count
    ]
}

/// Sky, with sunshine `Color(255, 100, 255)` at `pos` and at the mirrored pixel.
pub fn sun_frame(count: usize, pos: usize) -> Vec<Rgb> {
    let mut pixels = sky_frame(count);
    if count == 0 {
        return pixels;
    }
    let pos = pos % count;
    let sun = Rgb {
        r: 255,
        g: 100,
        b: 255,
    };
    pixels[pos] = sun;
    pixels[count - 1 - pos] = sun;
    pixels
}

/// How long one theater chase plus the black wipe takes, for `count` pixels.
pub fn flash_duration(count: usize) -> Duration {
    const CHASE: Duration = Duration::from_millis(50 * 3 * 10);
    const WIPE: Duration = Duration::from_millis(10);
    CHASE + WIPE * count as u32
}

/// Byte order on the wire. `gbr` is the old strip: a logical `(r, g, b)` is
/// sent as green, then blue, then red.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    Rgb,
    Grb,
    Gbr,
    Bgr,
    Brg,
    Rbg,
}

impl Order {
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text.trim().to_ascii_lowercase().as_str() {
            "rgb" => Self::Rgb,
            "grb" => Self::Grb,
            "gbr" => Self::Gbr,
            "bgr" => Self::Bgr,
            "brg" => Self::Brg,
            "rbg" => Self::Rbg,
            _ => return None,
        })
    }

    fn index(self) -> u8 {
        match self {
            Self::Rgb => 0,
            Self::Grb => 1,
            Self::Gbr => 2,
            Self::Bgr => 3,
            Self::Brg => 4,
            Self::Rbg => 5,
        }
    }

    fn apply(self, color: Rgb) -> [u8; 3] {
        let (r, g, b) = (color.r, color.g, color.b);
        match self {
            Self::Rgb => [r, g, b],
            Self::Grb => [g, r, b],
            Self::Gbr => [g, b, r],
            Self::Bgr => [b, g, r],
            Self::Brg => [b, r, g],
            Self::Rbg => [r, b, g],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    None,
    Spi,
    Serial,
}

impl Backend {
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text.trim().to_ascii_lowercase().as_str() {
            "none" => Self::None,
            "spi" => Self::Spi,
            "serial" => Self::Serial,
            _ => return None,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LedSettings {
    pub backend: Backend,
    pub device: PathBuf,
    pub count: usize,
    pub brightness: u8,
    pub order: Order,
    pub freq_hz: u32,
    /// SPI symbols per WS281x bit. 3 at `3 * freq_hz`, or 8 at `8 * freq_hz`.
    pub symbol_bits: u8,
    pub serial_port: PathBuf,
    pub serial_baud: u32,
    pub honor_mute: bool,
    pub mute_path: PathBuf,
    /// Draw the latest frame along the bottom of the window.
    pub debug: bool,
}

impl Default for LedSettings {
    fn default() -> Self {
        Self {
            backend: Backend::None,
            device: PathBuf::from("/dev/spidev0.0"),
            count: 20,
            brightness: 192,
            order: Order::Gbr,
            freq_hz: 400_000,
            symbol_bits: 3,
            serial_port: PathBuf::from("/dev/ttyACM0"),
            serial_baud: 115_200,
            honor_mute: true,
            mute_path: PathBuf::from("/tmp/mute"),
            debug: false,
        }
    }
}

impl LedSettings {
    pub fn validate(&self) -> Result<(), String> {
        if self.count == 0 || self.count > 1024 {
            return Err(format!("leds.count must be 1..1024, not {}", self.count));
        }
        if self.freq_hz < 100_000 || self.freq_hz > 1_600_000 {
            return Err(format!(
                "leds.freq_hz must be 100000..1600000, not {}",
                self.freq_hz
            ));
        }
        if self.symbol_bits != 3 && self.symbol_bits != 8 {
            return Err(format!(
                "leds.symbol_bits must be 3 or 8, not {}",
                self.symbol_bits
            ));
        }
        if self.serial_baud == 0 {
            return Err("leds.serial_baud must be greater than zero".into());
        }
        Ok(())
    }
}

pub fn scale_channel(value: u8, brightness: u8) -> u8 {
    ((u16::from(value) * u16::from(brightness)) / 255) as u8
}

pub fn wire_bytes(pixels: &[Rgb], order: Order, brightness: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(pixels.len() * 3);
    for pixel in pixels {
        out.extend(order.apply(pixel.scale(brightness)));
    }
    out
}

/// SPI clock for the chosen symbol width. 800 kHz is 2.4 MHz at 3 bits, or 6.4 MHz at 8.
pub fn spi_clock_hz(freq_hz: u32, symbol_bits: u8) -> u32 {
    freq_hz.saturating_mul(u32::from(symbol_bits))
}

/// Pack WS281x bits into SPI symbols. A 0 is `100` or `0xC0`. A 1 is `110` or `0xFC`.
pub fn encode_symbols(wire: &[u8], symbol_bits: u8) -> Vec<u8> {
    if symbol_bits == 8 {
        let mut out = Vec::with_capacity(wire.len() * 8);
        for byte in wire {
            for bit in (0..8).rev() {
                let on = byte & (1 << bit) != 0;
                out.push(if on { 0xfc } else { 0xc0 });
            }
        }
        return out;
    }
    let mut out = Vec::with_capacity(wire.len() * 3);
    let mut acc: u32 = 0;
    let mut nbits: u32 = 0;
    for byte in wire {
        for bit in (0..8).rev() {
            let symbol: u32 = if byte & (1 << bit) != 0 { 0b110 } else { 0b100 };
            acc = (acc << 3) | symbol;
            nbits += 3;
            if nbits >= 8 {
                nbits -= 8;
                out.push((acc >> nbits) as u8);
                acc &= (1u32 << nbits).wrapping_sub(1);
            }
        }
    }
    if nbits > 0 {
        out.push((acc << (8 - nbits)) as u8);
    }
    out
}

/// Low time after the bitstream so the strip latches. 300 µs is enough for WS2811 and WS2812.
pub fn reset_latch(spi_hz: u32) -> usize {
    let bits = u64::from(spi_hz) * 300 / 1_000_000;
    (bits / 8) as usize + 1
}

/// `LED1` frame: magic, version, count, order, brightness-applied RGB in `order`, xor.
pub fn serial_frame(pixels: &[Rgb], order: Order, brightness: u8) -> Vec<u8> {
    let body = wire_bytes(pixels, order, brightness);
    let mut frame = Vec::with_capacity(8 + body.len());
    frame.extend_from_slice(b"LED1");
    frame.push(1);
    let count = pixels.len() as u16;
    frame.extend_from_slice(&count.to_le_bytes());
    frame.push(order.index());
    let xor = body.iter().fold(0u8, |acc, byte| acc ^ byte);
    frame.extend(body);
    frame.push(xor);
    frame
}

#[derive(Clone, Copy, Debug)]
pub enum Cmd {
    Rainbow,
    Flash,
    Clear,
    Sky,
    Sun(usize),
}

#[derive(Clone, Debug)]
pub struct Handle {
    tx: Sender<Cmd>,
}

impl Handle {
    pub fn rainbow(&self) {
        let _ = self.tx.send(Cmd::Rainbow);
    }

    pub fn flash(&self) {
        let _ = self.tx.send(Cmd::Flash);
    }

    pub fn clear(&self) {
        let _ = self.tx.send(Cmd::Clear);
    }
}

/// Open the configured backend. Failure logs and returns `None` (lights stay off).
pub fn open(settings: &LedSettings) -> Option<Handle> {
    if settings.backend == Backend::None && !settings.debug {
        return None;
    }
    let output: Box<dyn LedOutput> = match settings.backend {
        Backend::None => Box::new(NullOutput),
        Backend::Spi => match open_spi(settings) {
            Ok(device) => device,
            Err(err) => {
                eprintln!("clockface: leds spi: {err}; continuing with lights off");
                Box::new(NullOutput)
            }
        },
        Backend::Serial => match open_serial(settings) {
            Ok(port) => port,
            Err(err) => {
                eprintln!("clockface: leds serial: {err}; continuing with lights off");
                Box::new(NullOutput)
            }
        },
    };
    let (tx, rx) = mpsc::channel();
    let settings = settings.clone();
    if thread::Builder::new()
        .name("clockface-leds".into())
        .spawn(move || led_loop(settings, rx, output))
        .is_err()
    {
        eprintln!("clockface: leds thread did not start");
        return None;
    }
    Some(Handle { tx })
}

fn led_loop(settings: LedSettings, rx: Receiver<Cmd>, mut output: Box<dyn LedOutput>) {
    let mut queue = std::collections::VecDeque::new();
    let mut job: Option<Job> = None;
    let mut showed_mute = false;
    loop {
        if job.is_none() && queue.is_empty() {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(cmd) => queue.push_back(cmd),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        while let Ok(cmd) = rx.try_recv() {
            queue.push_back(cmd);
        }
        if settings.honor_mute && file_is_mute(&settings.mute_path) {
            if !showed_mute {
                show(&mut *output, &settings, &vec![Rgb::BLACK; settings.count]);
                showed_mute = true;
            }
            queue.clear();
            job = None;
            thread::sleep(Duration::from_millis(50));
            continue;
        }
        showed_mute = false;
        if job.is_none() {
            let Some(cmd) = queue.pop_front() else {
                continue;
            };
            job = Some(Job::start(cmd, settings.count));
        }
        let Some(current) = job.as_mut() else {
            continue;
        };
        match current.step() {
            Step::Frame(pixels, wait) => {
                show(&mut *output, &settings, &pixels);
                thread::sleep(wait);
            }
            Step::Done => job = None,
        }
    }
}

fn show(output: &mut dyn LedOutput, settings: &LedSettings, pixels: &[Rgb]) {
    if settings.debug {
        if let Ok(mut slot) = debug_frames().lock() {
            *slot = Some(pixels.to_vec());
        }
    }
    output.show(pixels, settings);
}

fn file_is_mute(path: &std::path::Path) -> bool {
    std::fs::read_to_string(path)
        .map(|text| text.trim() == "1")
        .unwrap_or(false)
}

fn debug_frames() -> &'static Mutex<Option<Vec<Rgb>>> {
    static SLOT: OnceLock<Mutex<Option<Vec<Rgb>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(None))
}

/// A row of squares along the bottom edge when `[leds] debug` is set.
pub fn paint_debug(painter: &Painter, screen: Rect) {
    let Ok(slot) = debug_frames().lock() else {
        return;
    };
    let Some(pixels) = slot.as_ref() else {
        return;
    };
    if pixels.is_empty() {
        return;
    }
    let height = 12.0_f32.min(screen.height() * 0.02).max(4.0);
    let width = screen.width() / pixels.len() as f32;
    let top = screen.bottom() - height;
    for (i, pixel) in pixels.iter().enumerate() {
        let rect = Rect::from_min_max(
            Pos2::new(screen.left() + width * i as f32, top),
            Pos2::new(screen.left() + width * (i as f32 + 1.0), screen.bottom()),
        );
        painter.rect_filled(rect, 0.0, Color32::from_rgb(pixel.r, pixel.g, pixel.b));
    }
}

enum Step {
    Frame(Vec<Rgb>, Duration),
    Done,
}

struct Job {
    kind: JobKind,
    count: usize,
    index: u32,
}

enum JobKind {
    Rainbow,
    /// `phase` counts chase steps, then the black wipe.
    Flash {
        phase: u32,
    },
    Solid(Rgb),
    Sky,
    Sun(usize),
}

impl Job {
    fn start(cmd: Cmd, count: usize) -> Self {
        let kind = match cmd {
            Cmd::Rainbow => JobKind::Rainbow,
            Cmd::Flash => JobKind::Flash { phase: 0 },
            Cmd::Clear => JobKind::Solid(Rgb::BLACK),
            Cmd::Sky => JobKind::Sky,
            Cmd::Sun(pos) => JobKind::Sun(pos),
        };
        Self {
            kind,
            count,
            index: 0,
        }
    }

    fn step(&mut self) -> Step {
        match &mut self.kind {
            JobKind::Rainbow => {
                if self.index >= 256 {
                    return Step::Done;
                }
                let frame = rainbow_frame(self.count, self.index as u8);
                self.index += 1;
                Step::Frame(frame, Duration::from_millis(20))
            }
            JobKind::Flash { phase } => {
                let chase_steps = 10 * 3;
                let white = Rgb {
                    r: 127,
                    g: 127,
                    b: 127,
                };
                if *phase < chase_steps {
                    let q = (*phase % 3) as usize;
                    let frame = theater_frame(self.count, q, white);
                    *phase += 1;
                    Step::Frame(frame, Duration::from_millis(50))
                } else {
                    let wipe_at = *phase - chase_steps;
                    if wipe_at >= self.count as u32 {
                        return Step::Done;
                    }
                    *phase += 1;
                    let mut frame = theater_frame(self.count, 2, white);
                    for pixel in frame.iter_mut().take(wipe_at as usize + 1) {
                        *pixel = Rgb::BLACK;
                    }
                    Step::Frame(frame, Duration::from_millis(10))
                }
            }
            JobKind::Solid(color) => {
                if self.index > 0 {
                    return Step::Done;
                }
                self.index = 1;
                Step::Frame(vec![*color; self.count], Duration::from_millis(0))
            }
            JobKind::Sky => {
                if self.index > 0 {
                    return Step::Done;
                }
                self.index = 1;
                Step::Frame(sky_frame(self.count), Duration::from_millis(0))
            }
            JobKind::Sun(pos) => {
                if self.index > 0 {
                    return Step::Done;
                }
                let pos = *pos;
                self.index = 1;
                Step::Frame(sun_frame(self.count, pos), Duration::from_millis(0))
            }
        }
    }
}

trait LedOutput: Send {
    fn show(&mut self, pixels: &[Rgb], settings: &LedSettings);
}

struct NullOutput;

impl LedOutput for NullOutput {
    fn show(&mut self, _pixels: &[Rgb], _settings: &LedSettings) {}
}

fn open_spi(settings: &LedSettings) -> Result<Box<dyn LedOutput>, String> {
    #[cfg(all(target_os = "linux", feature = "leds-spi"))]
    {
        SpiPort::open(settings).map(|port| Box::new(port) as Box<dyn LedOutput>)
    }
    #[cfg(not(all(target_os = "linux", feature = "leds-spi")))]
    {
        let _ = settings;
        Err("the spi backend needs a Linux build with the leds-spi feature".into())
    }
}

fn open_serial(settings: &LedSettings) -> Result<Box<dyn LedOutput>, String> {
    #[cfg(target_os = "linux")]
    {
        SerialPort::open(settings).map(|port| Box::new(port) as Box<dyn LedOutput>)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = settings;
        Err("the serial backend is implemented for Linux".into())
    }
}

#[cfg(all(target_os = "linux", feature = "leds-spi"))]
struct SpiPort {
    file: std::fs::File,
}

#[cfg(all(target_os = "linux", feature = "leds-spi"))]
impl SpiPort {
    fn open(settings: &LedSettings) -> Result<Self, String> {
        use std::os::unix::io::AsRawFd;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&settings.device)
            .map_err(|err| format!("{}: {err}", settings.device.display()))?;
        let fd = file.as_raw_fd();
        let mode: u8 = 0;
        ioctl_write(fd, SPI_IOC_WR_MODE, &mode).map_err(|err| format!("spi mode: {err}"))?;
        let bits: u8 = 8;
        ioctl_write(fd, SPI_IOC_WR_BITS, &bits).map_err(|err| format!("spi bits: {err}"))?;
        let speed = spi_clock_hz(settings.freq_hz, settings.symbol_bits);
        ioctl_write(fd, SPI_IOC_WR_SPEED, &speed).map_err(|err| format!("spi speed: {err}"))?;
        Ok(Self { file })
    }
}

#[cfg(all(target_os = "linux", feature = "leds-spi"))]
impl LedOutput for SpiPort {
    fn show(&mut self, pixels: &[Rgb], settings: &LedSettings) {
        use std::os::unix::io::AsRawFd;
        let wire = wire_bytes(pixels, settings.order, settings.brightness);
        let mut bytes = encode_symbols(&wire, settings.symbol_bits);
        let clock = spi_clock_hz(settings.freq_hz, settings.symbol_bits);
        bytes.extend(std::iter::repeat(0u8).take(reset_latch(clock)));
        let transfer = SpiTransfer {
            tx_buf: bytes.as_ptr() as u64,
            rx_buf: 0,
            len: bytes.len() as u32,
            speed_hz: clock,
            delay_usecs: 0,
            bits_per_word: 8,
            cs_change: 0,
            tx_nbits: 0,
            rx_nbits: 0,
            word_delay_usecs: 0,
            pad: 0,
        };
        if let Err(err) = ioctl_write(self.file.as_raw_fd(), SPI_IOC_MESSAGE_1, &transfer) {
            eprintln!("clockface: leds spi write: {err}");
        }
    }
}

#[cfg(target_os = "linux")]
struct SerialPort {
    file: std::fs::File,
}

#[cfg(target_os = "linux")]
impl SerialPort {
    fn open(settings: &LedSettings) -> Result<Self, String> {
        use std::os::unix::io::AsRawFd;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&settings.serial_port)
            .map_err(|err| format!("{}: {err}", settings.serial_port.display()))?;
        set_baud(file.as_raw_fd(), settings.serial_baud)?;
        Ok(Self { file })
    }
}

#[cfg(target_os = "linux")]
impl LedOutput for SerialPort {
    fn show(&mut self, pixels: &[Rgb], settings: &LedSettings) {
        let frame = serial_frame(pixels, settings.order, settings.brightness);
        if let Err(err) = self.file.write_all(&frame) {
            eprintln!("clockface: leds serial write: {err}");
        }
    }
}

#[cfg(target_os = "linux")]
fn ioctl_write<T>(fd: i32, request: u64, value: &T) -> Result<(), String> {
    extern "C" {
        fn ioctl(fd: i32, request: u64, value: *const std::ffi::c_void) -> i32;
    }
    let rc = unsafe { ioctl(fd, request, value as *const T as *const std::ffi::c_void) };
    if rc < 0 {
        Err(std::io::Error::last_os_error().to_string())
    } else {
        Ok(())
    }
}

#[cfg(all(target_os = "linux", feature = "leds-spi"))]
const SPI_IOC_WR_MODE: u64 = 0x4001_6b01;
#[cfg(all(target_os = "linux", feature = "leds-spi"))]
const SPI_IOC_WR_BITS: u64 = 0x4001_6b03;
#[cfg(all(target_os = "linux", feature = "leds-spi"))]
const SPI_IOC_WR_SPEED: u64 = 0x4004_6b04;
#[cfg(all(target_os = "linux", feature = "leds-spi"))]
const SPI_IOC_MESSAGE_1: u64 = 0x4020_6b00;

#[cfg(all(target_os = "linux", feature = "leds-spi"))]
#[repr(C)]
struct SpiTransfer {
    tx_buf: u64,
    rx_buf: u64,
    len: u32,
    speed_hz: u32,
    delay_usecs: u16,
    bits_per_word: u8,
    cs_change: u8,
    tx_nbits: u8,
    rx_nbits: u8,
    word_delay_usecs: u8,
    pad: u8,
}

#[cfg(target_os = "linux")]
fn set_baud(fd: i32, baud: u32) -> Result<(), String> {
    const TCGETS: u64 = 0x5401;
    const TCSETS: u64 = 0x5402;
    let speed = match baud {
        9_600 => 13u32,
        19_200 => 14,
        38_400 => 15,
        57_600 => 4097,
        115_200 => 4098,
        230_400 => 4099,
        other => {
            return Err(format!(
                "unsupported baud {other}; use 9600, 19200, 38400, 57600, 115200, or 230400"
            ))
        }
    };
    let mut term = Termios::zero();
    ioctl_read(fd, TCGETS, &mut term)?;
    const CBAUD: u32 = 0o10017;
    const CS8: u32 = 0o60;
    const CLOCAL: u32 = 0o4000;
    const CREAD: u32 = 0o200;
    term.c_cflag &= !CBAUD;
    term.c_cflag |= speed | CS8 | CLOCAL | CREAD;
    term.c_iflag = 0;
    term.c_oflag = 0;
    term.c_lflag = 0;
    ioctl_write(fd, TCSETS, &term)
}

#[cfg(target_os = "linux")]
fn ioctl_read<T>(fd: i32, request: u64, value: &mut T) -> Result<(), String> {
    ioctl_write(fd, request, value)
}

#[cfg(target_os = "linux")]
#[repr(C)]
struct Termios {
    c_iflag: u32,
    c_oflag: u32,
    c_cflag: u32,
    c_lflag: u32,
    c_line: u8,
    c_cc: [u8; 19],
}

#[cfg(target_os = "linux")]
impl Termios {
    fn zero() -> Self {
        Self {
            c_iflag: 0,
            c_oflag: 0,
            c_cflag: 0,
            c_lflag: 0,
            c_line: 0,
            c_cc: [0; 19],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wheel_matches_the_adafruit_splits() {
        assert_eq!(wheel(0), Rgb { r: 0, g: 255, b: 0 });
        assert_eq!(wheel(84), Rgb { r: 252, g: 3, b: 0 });
        assert_eq!(wheel(85), Rgb { r: 255, g: 0, b: 0 });
        assert_eq!(wheel(169), Rgb { r: 3, g: 0, b: 252 });
        assert_eq!(wheel(170), Rgb { r: 0, g: 0, b: 255 });
        assert_eq!(wheel(255), Rgb { r: 0, g: 255, b: 0 });
    }

    #[test]
    fn rainbow_pixel_zero_tracks_the_wheel() {
        let frame = rainbow_frame(20, 10);
        assert_eq!(frame[0], wheel(10));
        assert_eq!(frame[3], wheel(13));
        assert_eq!(frame.len(), 20);
    }

    #[test]
    fn gbr_order_and_three_bit_symbols_pack_a_red_pixel() {
        let logical = Rgb { r: 255, g: 0, b: 0 };
        let wire = wire_bytes(&[logical], Order::Gbr, 255);
        assert_eq!(wire, vec![0x00, 0x00, 0xff]);
        let packed = encode_symbols(&wire, 3);
        assert_eq!(
            packed,
            vec![0x92, 0x49, 0x24, 0x92, 0x49, 0x24, 0xdb, 0x6d, 0xb6]
        );
        let wide = encode_symbols(&[0xff], 8);
        assert_eq!(wide, vec![0xfc; 8]);
        assert_eq!(encode_symbols(&[0x00], 8), vec![0xc0; 8]);
    }

    #[test]
    fn brightness_scales_before_the_wire_bytes() {
        let wire = wire_bytes(&[Rgb { r: 255, g: 0, b: 0 }], Order::Rgb, 192);
        assert_eq!(wire, vec![192, 0, 0]);
    }

    #[test]
    fn spi_clock_follows_the_symbol_width() {
        assert_eq!(spi_clock_hz(400_000, 3), 1_200_000);
        assert_eq!(spi_clock_hz(800_000, 3), 2_400_000);
        assert_eq!(spi_clock_hz(800_000, 8), 6_400_000);
        assert!(reset_latch(1_200_000) >= 45);
    }

    #[test]
    fn theater_lights_every_third_pixel_and_the_flash_is_one_point_seven_seconds() {
        let frame = theater_frame(
            20,
            1,
            Rgb {
                r: 127,
                g: 127,
                b: 127,
            },
        );
        assert_eq!(frame[0], Rgb::BLACK);
        assert_eq!(frame[1].r, 127);
        assert_eq!(frame[2], Rgb::BLACK);
        assert_eq!(frame[4].r, 127);
        assert_eq!(flash_duration(20), Duration::from_millis(1_700));
    }

    #[test]
    fn serial_frame_carries_the_ordered_pixels_and_a_checksum() {
        let frame = serial_frame(&[Rgb { r: 1, g: 2, b: 3 }], Order::Gbr, 255);
        assert_eq!(&frame[..4], b"LED1");
        assert_eq!(frame[4], 1);
        assert_eq!(u16::from_le_bytes([frame[5], frame[6]]), 1);
        assert_eq!(frame[7], Order::Gbr.index());
        assert_eq!(&frame[8..11], &[2, 3, 1]);
        assert_eq!(frame[11], 2 ^ 3 ^ 1);
    }

    #[test]
    fn sun_marks_a_pixel_and_its_mirror_on_the_sky() {
        let frame = sun_frame(20, 3);
        assert_eq!(
            frame[0],
            Rgb {
                r: 200,
                g: 160,
                b: 80
            }
        );
        assert_eq!(
            frame[3],
            Rgb {
                r: 255,
                g: 100,
                b: 255
            }
        );
        assert_eq!(frame[16], frame[3]);
    }
}
