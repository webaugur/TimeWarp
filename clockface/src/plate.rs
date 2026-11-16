//! Background plate and per-layer dirty tracking.
//!
//! The UI thread never rasterizes the kiosk. It keeps the last texture on
//! screen, draws the hands over it, and uploads a finished rectangle when the
//! worker has one. Layers that did not change stay as the patches from the
//! last time they were drawn. A minute and a Swatch beat a second apart are
//! one clock-row job, not two full plates.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use egui::{Context, Painter, Pos2, Rect, TextureHandle, TextureOptions};

use crate::kiosk::{self, KioskModel};
use crate::raster::SoftwareRaster;

/// How long the first picture waits so startup weather and the almanac share
/// one raster. The UI keeps drawing the hands on black during the wait.
pub(crate) const FIRST_WAIT_MS: u64 = 400;
/// How long a later burst waits. A minute and a beat about a second apart
/// leave as one job. The previous plate stays up the whole time.
pub(crate) const FOLLOW_WAIT_MS: u64 = 1200;

pub(crate) const LAYER_COUNT: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum Layer {
    Sky = 0,
    Caption = 1,
    Dial = 2,
    Ticks = 3,
    Numerals = 4,
    Gauges = 5,
    BoardLeft = 6,
    ClockRow = 7,
    BoardRight = 8,
    Status = 9,
}

pub(crate) const LAYERS: [Layer; LAYER_COUNT] = [
    Layer::Sky,
    Layer::Caption,
    Layer::Dial,
    Layer::Ticks,
    Layer::Numerals,
    Layer::Gauges,
    Layer::BoardLeft,
    Layer::ClockRow,
    Layer::BoardRight,
    Layer::Status,
];

const ALL_BITS: u16 = (1 << LAYER_COUNT) - 1;

impl Layer {
    pub(crate) fn index(self) -> usize {
        self as u8 as usize
    }

    pub(crate) const fn bit(self) -> u16 {
        1 << (self as u16)
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Layer::Sky => "sky",
            Layer::Caption => "caption",
            Layer::Dial => "dial",
            Layer::Ticks => "ticks",
            Layer::Numerals => "numerals",
            Layer::Gauges => "gauges",
            Layer::BoardLeft => "board-left",
            Layer::ClockRow => "clock",
            Layer::BoardRight => "board-right",
            Layer::Status => "status",
        }
    }
}

pub(crate) fn layer_list(bits: u16) -> String {
    let mut names = Vec::new();
    for layer in LAYERS {
        if bits & layer.bit() != 0 {
            names.push(layer.name());
        }
    }
    if names.is_empty() {
        "-".to_owned()
    } else {
        names.join(",")
    }
}

pub(crate) fn dirty_mask(prev: &[u64; LAYER_COUNT], next: &[u64; LAYER_COUNT]) -> u16 {
    let mut bits = 0u16;
    for layer in LAYERS {
        let i = layer.index();
        if prev[i] != next[i] {
            bits |= layer.bit();
        }
    }
    bits
}

/// Pixel rectangle, origin top-left, half-open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PxRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl PxRect {
    pub(crate) fn empty() -> Self {
        Self {
            x: 0,
            y: 0,
            w: 0,
            h: 0,
        }
    }

    pub(crate) fn is_empty(self) -> bool {
        self.w == 0 || self.h == 0
    }

    pub(crate) fn bottom(self) -> u32 {
        self.y.saturating_add(self.h)
    }

    pub(crate) fn right(self) -> u32 {
        self.x.saturating_add(self.w)
    }

    pub(crate) fn from_egui(rect: Rect, width: u32, height: u32) -> Self {
        if width == 0 || height == 0 || !rect.is_positive() {
            return Self::empty();
        }
        let x0 = rect.min.x.floor().max(0.0) as i64;
        let y0 = rect.min.y.floor().max(0.0) as i64;
        let x1 = rect.max.x.ceil().min(width as f32) as i64;
        let y1 = rect.max.y.ceil().min(height as f32) as i64;
        if x1 <= x0 || y1 <= y0 || x0 >= width as i64 || y0 >= height as i64 {
            return Self::empty();
        }
        let x0 = x0.max(0) as u32;
        let y0 = y0.max(0) as u32;
        let x1 = (x1 as u32).min(width);
        let y1 = (y1 as u32).min(height);
        Self {
            x: x0,
            y: y0,
            w: x1.saturating_sub(x0),
            h: y1.saturating_sub(y0),
        }
    }

    fn intersects(self, other: Self) -> bool {
        self.x < other.right()
            && other.x < self.right()
            && self.y < other.bottom()
            && other.y < self.bottom()
    }

    fn union(self, other: Self) -> Self {
        if self.is_empty() {
            return other;
        }
        if other.is_empty() {
            return self;
        }
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = self.right().max(other.right());
        let y1 = self.bottom().max(other.bottom());
        Self {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        }
    }
}

fn merge_rects(rects: Vec<PxRect>) -> Vec<PxRect> {
    let mut rects: Vec<PxRect> = rects.into_iter().filter(|rect| !rect.is_empty()).collect();
    let mut changed = true;
    while changed {
        changed = false;
        let mut i = 0;
        while i < rects.len() {
            let mut j = i + 1;
            while j < rects.len() {
                if rects[i].intersects(rects[j]) {
                    rects[i] = rects[i].union(rects[j]);
                    rects.swap_remove(j);
                    changed = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }
    rects
}

/// One layer's pixels, straight RGBA, row-major, cropped to its rectangle.
#[derive(Clone, Debug)]
pub(crate) struct Patch {
    pub layer: Layer,
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub rgba: Vec<u8>,
}

impl Patch {
    pub(crate) fn empty(layer: Layer) -> Self {
        Self {
            layer,
            x: 0,
            y: 0,
            w: 0,
            h: 0,
            rgba: Vec::new(),
        }
    }

    fn is_empty(&self) -> bool {
        self.w == 0 || self.h == 0 || self.rgba.is_empty()
    }

    fn rect(&self) -> PxRect {
        PxRect {
            x: self.x,
            y: self.y,
            w: self.w,
            h: self.h,
        }
    }
}

fn black_plate(width: u32, height: u32) -> Vec<u8> {
    let n = width as usize * height as usize * 4;
    let mut plate = vec![0u8; n];
    for pixel in plate.chunks_exact_mut(4) {
        pixel[3] = 255;
    }
    plate
}

fn copy_patch(plate: &mut [u8], width: u32, patch: &Patch) {
    if patch.is_empty() || width == 0 {
        return;
    }
    for row in 0..patch.h {
        let y = patch.y + row;
        let src = (row * patch.w * 4) as usize;
        let dst = ((y * width + patch.x) * 4) as usize;
        let n = (patch.w * 4) as usize;
        if src + n <= patch.rgba.len() && dst + n <= plate.len() {
            plate[dst..dst + n].copy_from_slice(&patch.rgba[src..src + n]);
        }
    }
}

/// Coalesce dirty bits so a burst becomes one dispatch.
#[derive(Clone, Debug)]
pub(crate) struct Coalesce {
    pending: u16,
    opened_ms: Option<u64>,
    have_plate: bool,
}

impl Coalesce {
    pub(crate) fn new() -> Self {
        Self {
            pending: 0,
            opened_ms: None,
            have_plate: false,
        }
    }

    /// Record `dirty` at `now_ms`. Returns the mask to send, or 0 when the
    /// burst is still open or a job is already running.
    pub(crate) fn observe(&mut self, dirty: u16, now_ms: u64, busy: bool) -> u16 {
        if dirty != 0 {
            if self.opened_ms.is_none() {
                self.opened_ms = Some(now_ms);
            }
            self.pending |= dirty;
        }
        if busy || self.pending == 0 {
            return 0;
        }
        let opened = self.opened_ms.unwrap_or(now_ms);
        let window = if self.have_plate {
            FOLLOW_WAIT_MS
        } else {
            FIRST_WAIT_MS
        };
        if now_ms.saturating_sub(opened) < window {
            return 0;
        }
        self.have_plate = true;
        let out = self.pending;
        self.pending = 0;
        self.opened_ms = None;
        out
    }

    fn pending(&self) -> bool {
        self.pending != 0
    }

    fn remaining_ms(&self, now_ms: u64) -> Option<u64> {
        let opened = self.opened_ms?;
        if self.pending == 0 {
            return None;
        }
        let window = if self.have_plate {
            FOLLOW_WAIT_MS
        } else {
            FIRST_WAIT_MS
        };
        Some(window.saturating_sub(now_ms.saturating_sub(opened)))
    }
}

struct Job {
    id: u64,
    layers: u16,
    width: u32,
    height: u32,
    center: Pos2,
    diameter: f32,
    model: KioskModel,
}

enum PlatePixels {
    /// A finished static plate. The UI uploads only the dirty rectangles.
    Full(Vec<u8>),
    /// Opaque rectangles. Copied over the plate the UI already has.
    Parts(Vec<Patch>),
}

struct Done {
    id: u64,
    width: u32,
    height: u32,
    layers: u16,
    raster_ms: f64,
    pixels: PlatePixels,
}

struct Hub {
    tx: Mutex<Sender<Job>>,
    rx: Mutex<Receiver<Done>>,
    held: Mutex<Vec<Done>>,
}

static HUB: OnceLock<Hub> = OnceLock::new();
static NEXT_JOB: AtomicU64 = AtomicU64::new(1);

fn hub() -> &'static Hub {
    HUB.get_or_init(|| {
        let (job_tx, job_rx) = mpsc::channel();
        let (done_tx, done_rx) = mpsc::channel();
        std::thread::Builder::new()
            .name("clockface-plate".into())
            .spawn(move || worker_loop(job_rx, done_tx))
            .expect("clockface plate thread");
        Hub {
            tx: Mutex::new(job_tx),
            rx: Mutex::new(done_rx),
            held: Mutex::new(Vec::new()),
        }
    })
}

fn worker_loop(rx: Receiver<Job>, tx: Sender<Done>) {
    let mut raster = SoftwareRaster::new();
    while let Ok(job) = rx.recv() {
        let started = Instant::now();
        let pixels = if clock_only(job.layers) {
            match raster.paint_clock_strip(
                job.width,
                job.height,
                job.center,
                job.diameter,
                &job.model,
            ) {
                Ok(patch) => PlatePixels::Parts(vec![patch]),
                Err(err) => {
                    eprintln!("clockface plate: {err}");
                    PlatePixels::Parts(Vec::new())
                }
            }
        } else if face_bits(job.layers) == 0 {
            match raster.paint_light(
                job.width,
                job.height,
                job.center,
                job.diameter,
                &job.model,
                job.layers,
            ) {
                Ok(patches) => PlatePixels::Parts(patches),
                Err(err) => {
                    eprintln!("clockface plate: {err}");
                    PlatePixels::Parts(Vec::new())
                }
            }
        } else {
            match raster.paint_static(job.width, job.height, job.center, job.diameter, &job.model) {
                Ok(rgba) => PlatePixels::Full(rgba),
                Err(err) => {
                    eprintln!("clockface plate: {err}");
                    PlatePixels::Full(Vec::new())
                }
            }
        };
        let raster_ms = started.elapsed().as_secs_f64() * 1000.0;
        if tx
            .send(Done {
                id: job.id,
                width: job.width,
                height: job.height,
                layers: job.layers,
                raster_ms,
                pixels,
            })
            .is_err()
        {
            break;
        }
    }
}

fn clock_only(bits: u16) -> bool {
    bits == Layer::ClockRow.bit()
}

/// Sky, caption, dial, ticks, numerals, gauges. These blend, so a change
/// repaints that stack. Boards and the status line do not.
fn face_bits(bits: u16) -> u16 {
    const FACE: u16 = Layer::Sky.bit()
        | Layer::Caption.bit()
        | Layer::Dial.bit()
        | Layer::Ticks.bit()
        | Layer::Numerals.bit()
        | Layer::Gauges.bit();
    bits & FACE
}

fn take_done(id: u64) -> Option<Done> {
    let hub = hub();
    let mut held = hub.held.lock().expect("plate results");
    if let Ok(rx) = hub.rx.lock() {
        while let Ok(done) = rx.try_recv() {
            held.push(done);
        }
    }
    held.retain(|done| done.id >= id);
    let index = held.iter().position(|done| done.id == id)?;
    Some(held.remove(index))
}

struct PlateEngine {
    created: Instant,
    coalesce: Coalesce,
    keys: Option<[u64; LAYER_COUNT]>,
    plate: Vec<u8>,
    width: u32,
    height: u32,
    center: Pos2,
    diameter: f32,
    texture: Option<TextureHandle>,
    busy: bool,
    inflight: u64,
    raster_ms: f64,
    upload_ms: f64,
    layers: String,
    uploaded: bool,
}

impl PlateEngine {
    fn new() -> Self {
        Self {
            created: Instant::now(),
            coalesce: Coalesce::new(),
            keys: None,
            plate: Vec::new(),
            width: 0,
            height: 0,
            center: Pos2::ZERO,
            diameter: 0.0,
            texture: None,
            busy: false,
            inflight: 0,
            raster_ms: 0.0,
            upload_ms: 0.0,
            layers: "-".to_owned(),
            uploaded: false,
        }
    }

    fn now_ms(&self) -> u64 {
        self.created.elapsed().as_millis() as u64
    }
}

#[derive(Clone)]
struct PlateSlot(Arc<Mutex<PlateEngine>>);

pub(crate) struct FrameNote {
    pub kind: &'static str,
    pub raster_ms: f64,
    pub upload_ms: f64,
    pub layers: String,
    pub wake: Option<Duration>,
}

impl PlateEngine {
    fn dispatch(&mut self, layers: u16, model: &KioskModel) {
        let id = NEXT_JOB.fetch_add(1, Ordering::Relaxed);
        let job = Job {
            id,
            layers,
            width: self.width,
            height: self.height,
            center: self.center,
            diameter: self.diameter,
            model: model.clone(),
        };
        match hub().tx.lock() {
            Ok(tx) => {
                if tx.send(job).is_ok() {
                    self.busy = true;
                    self.inflight = id;
                }
            }
            Err(_) => eprintln!("clockface plate: job channel closed"),
        }
    }
}

pub(crate) fn present_painter(
    painter: &Painter,
    width: u32,
    height: u32,
    center: Pos2,
    diameter: f32,
    model: &KioskModel,
) -> (Option<egui::TextureId>, FrameNote) {
    let ctx = painter.ctx();
    let id = egui::Id::unique("clockface-plate-engine");
    let slot = if let Some(slot) = ctx.data(|data| data.get_temp::<PlateSlot>(id)) {
        slot
    } else {
        let slot = PlateSlot(Arc::new(Mutex::new(PlateEngine::new())));
        ctx.data_mut(|data| data.insert_temp(id, slot.clone()));
        slot
    };
    let mut engine = slot.0.lock().expect("plate engine");
    engine.advance(ctx, width, height, center, diameter, model)
}

impl PlateEngine {
    fn advance(
        &mut self,
        ctx: &Context,
        width: u32,
        height: u32,
        center: Pos2,
        diameter: f32,
        model: &KioskModel,
    ) -> (Option<egui::TextureId>, FrameNote) {
        self.uploaded = false;
        self.raster_ms = 0.0;
        self.upload_ms = 0.0;
        self.layers = "-".to_owned();
        self.ensure_size(width, height, center, diameter);
        self.collect_ready(ctx);
        let next = kiosk::layer_keys(width, height, center, diameter, model);
        let dirty = match &self.keys {
            Some(prev) => dirty_mask(prev, &next),
            None => ALL_BITS,
        };
        self.keys = Some(next);
        let send = self.coalesce.observe(dirty, self.now_ms(), self.busy);
        if send != 0 {
            self.dispatch(send, model);
        }
        let wake = if self.busy {
            Some(Duration::from_millis(16))
        } else {
            self.coalesce
                .remaining_ms(self.now_ms())
                .map(|ms| Duration::from_millis(ms.max(1)))
        };
        let kind = if self.uploaded {
            "partial"
        } else if self.texture.is_some() {
            "cached"
        } else {
            "pending"
        };
        let note = FrameNote {
            kind,
            raster_ms: self.raster_ms,
            upload_ms: self.upload_ms,
            layers: if self.uploaded {
                self.layers.clone()
            } else {
                "-".to_owned()
            },
            wake,
        };
        let texture = self.texture.as_ref().map(|handle| handle.id());
        (texture, note)
    }

    fn ensure_size(&mut self, width: u32, height: u32, center: Pos2, diameter: f32) {
        if self.width == width
            && self.height == height
            && self.center == center
            && self.diameter.to_bits() == diameter.to_bits()
        {
            return;
        }
        self.width = width;
        self.height = height;
        self.center = center;
        self.diameter = diameter;
        self.plate = black_plate(width, height);
        self.texture = None;
        self.keys = None;
    }

    fn collect_ready(&mut self, ctx: &Context) {
        if !self.busy {
            return;
        }
        let Some(done) = take_done(self.inflight) else {
            return;
        };
        self.busy = false;
        if done.width != self.width || done.height != self.height {
            return;
        }
        self.raster_ms = done.raster_ms;
        let upload_started = Instant::now();
        let rects = match done.pixels {
            PlatePixels::Full(rgba) => {
                if rgba.len() != self.plate.len() {
                    return;
                }
                self.plate = rgba;
                merge_rects(layer_rects(
                    done.layers,
                    self.width,
                    self.height,
                    self.center,
                    self.diameter,
                ))
            }
            PlatePixels::Parts(patches) => {
                let mut rects = Vec::new();
                for patch in &patches {
                    copy_patch(&mut self.plate, self.width, patch);
                    rects.push(patch.rect());
                }
                if rects.is_empty() {
                    return;
                }
                merge_rects(rects)
            }
        };
        self.upload_rects(ctx, &rects);
        self.upload_ms = upload_started.elapsed().as_secs_f64() * 1000.0;
        self.layers = layer_list(done.layers);
        self.uploaded = true;
    }

    fn upload_rects(&mut self, ctx: &Context, rects: &[PxRect]) {
        if self.texture.is_none() {
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [self.width as usize, self.height as usize],
                &self.plate,
            );
            self.texture =
                Some(ctx.load_texture("clockface-plate", image, TextureOptions::NEAREST));
            return;
        }
        let Some(handle) = self.texture.as_mut() else {
            return;
        };
        for rect in rects {
            if rect.is_empty() {
                continue;
            }
            let image = crop_color(self.plate.as_slice(), self.width, *rect);
            handle.set_partial(
                [rect.x as usize, rect.y as usize],
                image,
                TextureOptions::NEAREST,
            );
        }
    }
}

fn layer_rects(bits: u16, width: u32, height: u32, center: Pos2, diameter: f32) -> Vec<PxRect> {
    let mut rects = Vec::new();
    for layer in LAYERS {
        if bits & layer.bit() != 0 {
            rects.push(kiosk::layer_px(layer, width, height, center, diameter));
        }
    }
    rects
}

fn crop_color(plate: &[u8], width: u32, rect: PxRect) -> egui::ColorImage {
    let mut bytes = Vec::with_capacity(rect.w as usize * rect.h as usize * 4);
    for y in rect.y..rect.bottom() {
        let start = ((y * width + rect.x) * 4) as usize;
        let end = start + rect.w as usize * 4;
        bytes.extend_from_slice(&plate[start..end]);
    }
    egui::ColorImage::from_rgba_unmultiplied([rect.w as usize, rect.h as usize], &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minute_and_a_beat_a_second_apart_are_one_job() {
        let mut burst = Coalesce::new();
        let clock = Layer::ClockRow.bit();
        // The first plate leaves immediately once its short wait ends.
        assert_eq!(burst.observe(ALL_BITS, 0, false), 0);
        assert_eq!(burst.observe(0, FIRST_WAIT_MS, false), ALL_BITS);
        // Minute at t, beat one second later: still closed at t+1000, one mask at t+1200.
        let t = 10_000;
        assert_eq!(burst.observe(clock, t, false), 0);
        assert_eq!(burst.observe(clock, t + 1000, false), 0);
        assert_eq!(burst.observe(0, t + FOLLOW_WAIT_MS - 1, false), 0);
        assert_eq!(burst.observe(0, t + FOLLOW_WAIT_MS, false), clock);
        assert_eq!(burst.observe(0, t + FOLLOW_WAIT_MS + 10, false), 0);
    }

    #[test]
    fn startup_changes_share_the_first_job_and_a_busy_worker_holds_the_next() {
        let mut burst = Coalesce::new();
        let sky = Layer::Sky.bit() | Layer::Caption.bit();
        let later = Layer::Gauges.bit() | Layer::BoardRight.bit();
        assert_eq!(burst.observe(sky, 0, false), 0);
        assert_eq!(burst.observe(later, 200, false), 0);
        assert_eq!(burst.observe(0, FIRST_WAIT_MS, false), sky | later);

        let clock = Layer::ClockRow.bit();
        assert_eq!(burst.observe(clock, 5_000, false), 0);
        assert_eq!(burst.observe(Layer::Sky.bit(), 5_500, true), 0);
        assert_eq!(
            burst.observe(0, 5_000 + FOLLOW_WAIT_MS, false),
            clock | Layer::Sky.bit()
        );
    }

    #[test]
    fn seconds_do_not_open_a_burst() {
        let mut burst = Coalesce::new();
        assert_eq!(burst.observe(0, 0, false), 0);
        assert_eq!(burst.observe(0, 5_000, false), 0);
        assert!(!burst.pending());
    }
}
