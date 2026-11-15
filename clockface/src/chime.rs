//! Minute-aligned ticking and Westminster-style chimes.
//!
//! This follows the old Pi clock: `ticktock.sh` under pm2, and cron launching
//! the hour, quarter, and half scripts. Playback is a background thread.
//! The UI never waits on it.
//!
//! The old quarter and half scripts both played `clock-hour`, not
//! `clock-quarter` or `clock-half`. That is still the default. The file for
//! each mark is configurable.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::{DateTime, Datelike, Local, Timelike};
use rodio::{Decoder, DeviceSinkBuilder, MixerDeviceSink, Player, Source};

/// Bells start this long after the top of the hour (four rainbow passes).
pub const BELL_START: Duration = Duration::from_millis(20_500);
/// One bell strike, matching the old theater-chase plus wipe.
pub const BELL_GAP: Duration = Duration::from_millis(1_700);
/// Pause after the last strike, or after the quarter melody, before the lights clear.
pub const CLEAR_PAUSE: Duration = Duration::from_secs(2);

/// What cron fired.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    Hour,
    Quarter,
    Half,
}

/// `:00` hour, `:15` and `:45` quarter, `:30` half. Anything else is silent.
pub fn mark_at(minute: u32) -> Option<Mark> {
    match minute {
        0 => Some(Mark::Hour),
        15 | 45 => Some(Mark::Quarter),
        30 => Some(Mark::Half),
        _ => None,
    }
}

/// Strikes for a 24-hour clock. Midnight and noon are 12. 13:00 is 1.
pub fn bell_count(hour24: u32) -> u32 {
    match hour24 % 12 {
        0 => 12,
        hour => hour,
    }
}

/// When each strike begins, measured from the top of the hour.
pub fn bell_offsets(count: u32) -> Vec<Duration> {
    (0..count).map(|i| BELL_START + BELL_GAP * i).collect()
}

/// Fallback length of `ticktock-*.flac` when the decoder does not report one.
pub const TICK_FALLBACK: Duration = Duration::from_millis(59_100);

/// Where to start the tick recording. `None` means the offset is past the
/// recording, so the caller waits for the next `:00`.
pub fn tick_seek(second: u32, nanosecond: u32, clip_len: Duration) -> Option<Duration> {
    let offset =
        Duration::from_secs(u64::from(second)) + Duration::from_nanos(u64::from(nanosecond));
    if offset >= clip_len {
        None
    } else {
        Some(offset)
    }
}

/// Top of the minute plays from the start. Later in the minute seeks.
pub fn tick_start_offset(second: u32, nanosecond: u32, clip_len: Duration) -> Option<Duration> {
    let offset = tick_seek(second, nanosecond, clip_len)?;
    if second < 2 {
        Some(Duration::ZERO)
    } else {
        Some(offset)
    }
}

/// Light cues that should have been issued by `elapsed` since the chime began.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LightPlan {
    pub rainbows: u32,
    /// Hour only: the strip is cleared as the bells begin, after the four rainbows.
    pub cleared_for_bells: bool,
    pub strikes: u32,
    pub finished: bool,
}

pub fn light_plan(mark: Mark, bells: u32, elapsed: Duration) -> LightPlan {
    let rainbows = ((elapsed.as_secs_f64() / 5.125).floor() as u32 + 1).min(4);
    let bells_started = elapsed >= BELL_START;
    let strikes = if bells_started && bells > 0 {
        let since = elapsed.saturating_sub(BELL_START);
        ((since.as_secs_f64() / 1.7).floor() as u32 + 1).min(bells)
    } else {
        0
    };
    let done_at = BELL_START + BELL_GAP * bells + CLEAR_PAUSE;
    LightPlan {
        rainbows,
        cleared_for_bells: mark == Mark::Hour && bells_started,
        strikes,
        finished: rainbows == 4 && strikes == bells && elapsed >= done_at,
    }
}

/// NeoPixel strip on the old clock: 20 pixels, SPI pin 10, 400 kHz, brightness
/// 192, GBR order. The real driver is not in this build. The no-op impl keeps
/// the audio timing, which does not wait on the lights themselves.
pub trait ChimeLights {
    fn rainbow(&mut self) {}
    fn flash(&mut self) {}
    fn clear(&mut self) {}
}

/// Lights that do nothing. The chime thread still waits out the old durations.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopLights;

impl ChimeLights for NoopLights {}

/// `1` in the mute file, or in the optional GPIO value, mutes ticking.
pub fn text_is_mute(text: &str) -> bool {
    text.trim() == "1"
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioSettings {
    pub enabled: bool,
    /// `10db`, `15db`, `20db`, or `25db`. Default `15db`.
    pub tick: String,
    pub mute_path: PathBuf,
    pub gpio_path: Option<PathBuf>,
    /// When set, the mute file also silences chimes. The old cron jobs did not.
    pub mute_chimes: bool,
    /// Directory checked before the sounds compiled into the binary.
    pub sounds_dir: PathBuf,
    pub hour: String,
    pub quarter: String,
    pub half: String,
    pub bell: String,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            tick: "15db".into(),
            mute_path: PathBuf::from("/tmp/mute"),
            gpio_path: None,
            mute_chimes: false,
            sounds_dir: default_sounds_dir(),
            hour: "clock-hour".into(),
            quarter: "clock-hour".into(),
            half: "clock-hour".into(),
            bell: "clock-bell".into(),
        }
    }
}

impl AudioSettings {
    pub fn tick_stem(&self) -> String {
        format!("ticktock-{}", self.tick)
    }

    pub fn melody(&self, mark: Mark) -> &str {
        match mark {
            Mark::Hour => &self.hour,
            Mark::Quarter => &self.quarter,
            Mark::Half => &self.half,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        match self.tick.as_str() {
            "10db" | "15db" | "20db" | "25db" => {}
            other => {
                return Err(format!(
                    "audio.tick must be 10db, 15db, 20db, or 25db, not {other:?}"
                ))
            }
        }
        for (name, value) in [
            ("hour", self.hour.as_str()),
            ("quarter", self.quarter.as_str()),
            ("half", self.half.as_str()),
            ("bell", self.bell.as_str()),
        ] {
            if value.trim().is_empty() || value.contains('/') || value.contains('\\') {
                return Err(format!("audio.{name} must be a file stem, not {value:?}"));
            }
        }
        Ok(())
    }
}

fn default_sounds_dir() -> PathBuf {
    match std::env::var_os("HOME") {
        Some(home) => PathBuf::from(home).join(".local/share/clockface/sounds"),
        None => PathBuf::from(".local/share/clockface/sounds"),
    }
}

pub fn is_muted(settings: &AudioSettings) -> bool {
    file_is_mute(&settings.mute_path)
        || settings
            .gpio_path
            .as_ref()
            .is_some_and(|path| file_is_mute(path))
}

fn file_is_mute(path: &Path) -> bool {
    std::fs::read_to_string(path)
        .map(|text| text_is_mute(&text))
        .unwrap_or(false)
}

enum Clip {
    Bundled(&'static [u8]),
    File(Vec<u8>),
}

fn bundled(stem: &str) -> Option<&'static [u8]> {
    Some(match stem {
        "clock-bell" => include_bytes!("../sounds/clock-bell.flac"),
        "clock-half" => include_bytes!("../sounds/clock-half.flac"),
        "clock-hour" => include_bytes!("../sounds/clock-hour.flac"),
        "clock-quarter" => include_bytes!("../sounds/clock-quarter.flac"),
        "ticktock-10db" => include_bytes!("../sounds/ticktock-10db.flac"),
        "ticktock-15db" => include_bytes!("../sounds/ticktock-15db.flac"),
        "ticktock-20db" => include_bytes!("../sounds/ticktock-20db.flac"),
        "ticktock-25db" => include_bytes!("../sounds/ticktock-25db.flac"),
        _ => return None,
    })
}

fn load_clip(settings: &AudioSettings, stem: &str) -> Result<Clip, String> {
    for ext in ["flac", "wav"] {
        let path = settings.sounds_dir.join(format!("{stem}.{ext}"));
        if path.is_file() {
            return std::fs::read(&path)
                .map(Clip::File)
                .map_err(|err| format!("could not read {}: {err}", path.display()));
        }
    }
    if let Some(bytes) = bundled(stem) {
        return Ok(Clip::Bundled(bytes));
    }
    Err(format!("sound missing: {stem}.wav"))
}

fn remember_fault(fault: &Mutex<String>, message: String) {
    if let Ok(mut slot) = fault.lock() {
        if slot.is_empty() {
            *slot = message;
        }
    }
}

fn play(mixer: &rodio::mixer::Mixer, clip: Clip, fault: &Mutex<String>, stem: &str) {
    let failed = match clip {
        Clip::Bundled(bytes) => start_player(mixer, Cursor::new(bytes)),
        Clip::File(bytes) => start_player(mixer, Cursor::new(bytes)),
    };
    if let Err(err) = failed {
        eprintln!("clockface: {stem}: {err}");
        remember_fault(fault, format!("sound missing: {stem}.wav"));
    }
}

fn start_player<T>(mixer: &rodio::mixer::Mixer, cursor: Cursor<T>) -> Result<(), String>
where
    T: AsRef<[u8]> + Send + Sync + 'static,
{
    let source = Decoder::try_from(cursor).map_err(|err| err.to_string())?;
    let player = Player::connect_new(mixer);
    player.append(source);
    player.detach();
    Ok(())
}

fn play_stem(
    settings: &AudioSettings,
    mixer: &rodio::mixer::Mixer,
    fault: &Mutex<String>,
    stem: &str,
) {
    match load_clip(settings, stem) {
        Ok(clip) => play(mixer, clip, fault, stem),
        Err(err) => {
            eprintln!("clockface: {err}");
            remember_fault(fault, err);
        }
    }
}

struct ChimeRun {
    mark: Mark,
    bells: u32,
    started: std::time::Instant,
    melody: bool,
    rainbows: u32,
    cleared_for_bells: bool,
    strikes: u32,
    finished: bool,
}

impl ChimeRun {
    fn begin(mark: Mark, hour24: u32) -> Self {
        Self {
            mark,
            bells: match mark {
                Mark::Hour => bell_count(hour24),
                Mark::Quarter | Mark::Half => 0,
            },
            started: std::time::Instant::now(),
            melody: false,
            rainbows: 0,
            cleared_for_bells: false,
            strikes: 0,
            finished: false,
        }
    }
}

fn advance_chime(
    run: &mut ChimeRun,
    settings: &AudioSettings,
    mixer: Option<&rodio::mixer::Mixer>,
    fault: &Mutex<String>,
    lights: &mut impl ChimeLights,
) {
    if run.finished {
        return;
    }
    let plan = light_plan(run.mark, run.bells, run.started.elapsed());
    if !run.melody {
        let stem = settings.melody(run.mark).to_string();
        if let Some(mixer) = mixer {
            play_stem(settings, mixer, fault, &stem);
        }
        run.melody = true;
    }
    while run.rainbows < plan.rainbows {
        lights.rainbow();
        run.rainbows += 1;
    }
    if plan.cleared_for_bells && !run.cleared_for_bells {
        lights.clear();
        run.cleared_for_bells = true;
    }
    while run.strikes < plan.strikes {
        if let Some(mixer) = mixer {
            play_stem(settings, mixer, fault, &settings.bell.clone());
        }
        lights.flash();
        run.strikes += 1;
    }
    if plan.finished {
        lights.clear();
        run.finished = true;
    }
}

fn play_stem_at(
    settings: &AudioSettings,
    mixer: &rodio::mixer::Mixer,
    fault: &Mutex<String>,
    stem: &str,
    offset: Duration,
) {
    match load_clip(settings, stem) {
        Ok(clip) => {
            if let Err(err) = play_offset(mixer, clip_bytes(clip), offset) {
                eprintln!("clockface: {stem}: {err}");
                remember_fault(fault, format!("sound missing: {stem}.wav"));
            }
        }
        Err(err) => {
            eprintln!("clockface: {err}");
            remember_fault(fault, err);
        }
    }
}

fn play_offset(
    mixer: &rodio::mixer::Mixer,
    bytes: Vec<u8>,
    offset: Duration,
) -> Result<(), String> {
    let mut decoder = Decoder::try_from(Cursor::new(bytes)).map_err(|err| err.to_string())?;
    if offset.is_zero() {
        let player = Player::connect_new(mixer);
        player.append(decoder);
        player.detach();
        return Ok(());
    }
    if decoder.try_seek(offset).is_ok() {
        let player = Player::connect_new(mixer);
        player.append(decoder);
        player.detach();
        return Ok(());
    }
    let player = Player::connect_new(mixer);
    player.append(decoder.skip_duration(offset));
    player.detach();
    Ok(())
}

/// Shared status-line text. Empty when every sound is present and the device opened.
pub type SoundFault = Arc<Mutex<String>>;

pub fn start(settings: &AudioSettings, leds: &crate::leds::LedSettings) -> SoundFault {
    let fault = Arc::new(Mutex::new(String::new()));
    if !settings.enabled {
        return fault;
    }
    let settings = settings.clone();
    let leds = leds.clone();
    let fault_thread = Arc::clone(&fault);
    if let Err(err) = thread::Builder::new()
        .name("clockface-audio".into())
        .spawn(move || audio_loop(settings, leds, fault_thread))
    {
        remember_fault(&fault, format!("audio thread: {err}"));
    }
    fault
}

struct Strip(Option<crate::leds::Handle>);

impl ChimeLights for Strip {
    fn rainbow(&mut self) {
        if let Some(lights) = &self.0 {
            lights.rainbow();
        }
    }

    fn flash(&mut self) {
        if let Some(lights) = &self.0 {
            lights.flash();
        }
    }

    fn clear(&mut self) {
        if let Some(lights) = &self.0 {
            lights.clear();
        }
    }
}

fn audio_loop(settings: AudioSettings, leds: crate::leds::LedSettings, fault: SoundFault) {
    let mut lights = Strip(crate::leds::open(&leds));
    let mut sink: Option<MixerDeviceSink> = None;
    let mut device_down = false;
    let mut tick_stamp: Option<String> = None;
    let mut tick_len: Option<Duration> = None;
    let mut was_muted = false;
    let mut chime_stamp: Option<String> = None;
    let mut chime: Option<ChimeRun> = None;
    loop {
        if sink.is_none() {
            match DeviceSinkBuilder::open_default_sink() {
                Ok(opened) => {
                    if device_down {
                        eprintln!("clockface: audio output is back");
                        tick_stamp = None;
                    }
                    device_down = false;
                    sink = Some(opened);
                }
                Err(err) => {
                    if !device_down {
                        eprintln!("clockface: audio output: {err}");
                        remember_fault(&fault, "audio output unavailable".into());
                        device_down = true;
                    }
                }
            }
        }
        let now = Local::now();
        let mixer = sink.as_ref().map(|opened| opened.mixer());
        if let Some(run) = chime.as_mut() {
            advance_chime(run, &settings, mixer, &fault, &mut lights);
            if run.finished {
                chime = None;
            }
        } else if let Some(mark) = mark_at(now.minute()) {
            if now.second() < 2 {
                let stamp_text = stamp(&now);
                if chime_stamp.as_deref() != Some(stamp_text.as_str()) {
                    chime_stamp = Some(stamp_text);
                    let muted = settings.mute_chimes && is_muted(&settings);
                    if !muted {
                        chime = Some(ChimeRun::begin(mark, now.hour()));
                        if let Some(run) = chime.as_mut() {
                            advance_chime(run, &settings, mixer, &fault, &mut lights);
                        }
                    }
                }
            }
        }
        let muted = is_muted(&settings);
        if muted {
            was_muted = true;
        } else if was_muted {
            was_muted = false;
            tick_stamp = None;
        }
        if !muted {
            if let Some(mixer) = mixer {
                let stamp_text = stamp(&now);
                if tick_stamp.as_deref() != Some(stamp_text.as_str()) {
                    let stem = settings.tick_stem();
                    match tick_start_offset(
                        now.second(),
                        now.nanosecond(),
                        tick_length(&settings, &stem, &mut tick_len),
                    ) {
                        Some(offset) => {
                            play_stem_at(&settings, mixer, &fault, &stem, offset);
                            tick_stamp = Some(stamp_text);
                        }
                        None => {}
                    }
                }
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn tick_length(settings: &AudioSettings, stem: &str, cached: &mut Option<Duration>) -> Duration {
    if let Some(len) = *cached {
        return len;
    }
    let len = match load_clip(settings, stem) {
        Ok(clip) => clip_duration(&clip_bytes(clip)),
        Err(_) => TICK_FALLBACK,
    };
    *cached = Some(len);
    len
}

fn clip_bytes(clip: Clip) -> Vec<u8> {
    match clip {
        Clip::Bundled(bytes) => bytes.to_vec(),
        Clip::File(bytes) => bytes,
    }
}

fn clip_duration(bytes: &[u8]) -> Duration {
    Decoder::try_from(Cursor::new(bytes.to_vec()))
        .ok()
        .and_then(|source| source.total_duration())
        .unwrap_or(TICK_FALLBACK)
}

fn stamp(now: &DateTime<Local>) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        now.year(),
        now.month(),
        now.day(),
        now.hour(),
        now.minute()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_follow_the_old_crontab() {
        assert_eq!(mark_at(0), Some(Mark::Hour));
        assert_eq!(mark_at(15), Some(Mark::Quarter));
        assert_eq!(mark_at(30), Some(Mark::Half));
        assert_eq!(mark_at(45), Some(Mark::Quarter));
        assert_eq!(mark_at(1), None);
        assert_eq!(mark_at(59), None);
    }

    #[test]
    fn bell_count_is_twelve_hour() {
        assert_eq!(bell_count(0), 12);
        assert_eq!(bell_count(12), 12);
        assert_eq!(bell_count(13), 1);
        assert_eq!(bell_count(10), 10);
        assert_eq!(bell_count(23), 11);
    }

    #[test]
    fn bell_offsets_start_at_twenty_point_five_seconds() {
        let offsets = bell_offsets(12);
        assert_eq!(offsets.len(), 12);
        assert_eq!(offsets[0], Duration::from_millis(20_500));
        assert_eq!(offsets[1] - offsets[0], Duration::from_millis(1_700));
        assert_eq!(offsets[11], Duration::from_millis(20_500 + 11 * 1_700));
        assert!(bell_offsets(0).is_empty());
        let one = bell_offsets(1);
        assert_eq!(one, vec![Duration::from_millis(20_500)]);
    }

    #[test]
    fn quarter_and_half_default_to_the_hour_melody() {
        let audio = AudioSettings::default();
        assert_eq!(audio.melody(Mark::Hour), "clock-hour");
        assert_eq!(audio.melody(Mark::Quarter), "clock-hour");
        assert_eq!(audio.melody(Mark::Half), "clock-hour");
        assert_eq!(audio.bell, "clock-bell");
        assert_eq!(audio.tick_stem(), "ticktock-15db");
        assert!(!audio.mute_chimes);
    }

    #[test]
    #[test]
    fn tick_seek_uses_the_wall_clock_offset_until_the_recording_ends() {
        let len = Duration::from_millis(59_100);
        assert_eq!(tick_seek(0, 0, len), Some(Duration::ZERO));
        assert_eq!(
            tick_seek(30, 500_000_000, len),
            Some(Duration::from_millis(30_500))
        );
        assert_eq!(tick_seek(59, 0, len), Some(Duration::from_secs(59)));
        assert_eq!(tick_seek(59, 100_000_000, len), None);
        assert_eq!(tick_seek(59, 200_000_000, len), None);
        assert_eq!(tick_start_offset(0, 400_000_000, len), Some(Duration::ZERO));
        assert_eq!(tick_start_offset(1, 0, len), Some(Duration::ZERO));
        assert_eq!(
            tick_start_offset(12, 250_000_000, len),
            Some(Duration::new(12, 250_000_000))
        );
        assert_eq!(tick_start_offset(59, 200_000_000, len), None);
    }

    #[test]
    fn hour_lights_rainbow_four_times_then_chase_each_bell() {
        let at = |ms| light_plan(Mark::Hour, 2, Duration::from_millis(ms));
        assert_eq!(at(0).rainbows, 1);
        assert!(!at(0).cleared_for_bells);
        assert_eq!(at(5_125).rainbows, 2);
        assert_eq!(at(15_375).rainbows, 4);
        assert!(!at(15_375).cleared_for_bells);
        let bells = at(20_500);
        assert!(bells.cleared_for_bells);
        assert_eq!(bells.strikes, 1);
        assert_eq!(at(22_200).strikes, 2);
        assert!(!at(22_200).finished);
        let done = 20_500 + 2 * 1_700 + 2_000;
        assert!(at(done).finished);
    }

    #[test]
    fn quarter_clears_two_seconds_after_the_rainbows_and_does_not_strike() {
        let during = light_plan(Mark::Quarter, 0, Duration::from_millis(20_500));
        assert_eq!(during.rainbows, 4);
        assert_eq!(during.strikes, 0);
        assert!(!during.finished);
        assert!(!during.cleared_for_bells);
        let done = light_plan(Mark::Quarter, 0, Duration::from_millis(22_500));
        assert!(done.finished);
        assert_eq!(
            light_plan(Mark::Half, 0, Duration::from_millis(10_250)).rainbows,
            3
        );
    }

    #[test]
    fn mute_file_is_the_digit_one() {
        assert!(text_is_mute("1"));
        assert!(text_is_mute("1\n"));
        assert!(!text_is_mute("0"));
        assert!(!text_is_mute(""));
        assert!(!text_is_mute("10"));
    }

    #[test]
    fn a_synthetic_tone_decodes() {
        let wav = sine_wav();
        let source = Decoder::try_from(Cursor::new(wav)).expect("wav");
        let frames = source.into_iter().take(200).count();
        assert!(frames > 50, "decoded {frames} frames");
    }

    fn sine_wav() -> Vec<u8> {
        let rate = 8_000u32;
        let samples = 160u32;
        let mut data = Vec::new();
        for i in 0..samples {
            let t = i as f32 / rate as f32;
            let sample = (t * 440.0 * std::f32::consts::TAU).sin();
            data.extend_from_slice(&((sample * 8000.0) as i16).to_le_bytes());
        }
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&rate.to_le_bytes());
        wav.extend_from_slice(&(rate * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend(data);
        wav
    }
}
