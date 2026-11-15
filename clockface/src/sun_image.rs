//! GOES SUVI disc from NOAA SWPC, cached under `~/.cache/clockface`.
//!
//! The image is a US government work. SWPC asks users not to hammer the server;
//! this fetches at most every 15 minutes. The file is not stored in the repo.
//! `--screenshot` may use a fresh cache or one fetch. Tests pass no image and
//! paint a drawn sun instead.

use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use std::process::Command;

/// GOES SUVI primary composite, 304 Å. `latest.png` is the frame SWPC is serving now.
pub const SUVI_URL: &str =
    "https://services.swpc.noaa.gov/images/animations/suvi/primary/304/latest.png";

pub const REFRESH_SECS: u64 = 15 * 60;

/// Fraction of the frame, from the center, that is the solar disc.
/// The caption along the bottom edge sits outside this circle.
pub const DISC_RADIUS: f32 = 0.42;

#[derive(Clone, Debug)]
pub struct SunImage {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
    pub stamp: u64,
}

pub fn cache_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_CACHE_HOME") {
        return Some(PathBuf::from(dir).join("clockface").join("suvi-304.png"));
    }
    std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join(".cache")
            .join("clockface")
            .join("suvi-304.png")
    })
}

pub fn load_cached() -> Option<Arc<SunImage>> {
    let path = cache_path()?;
    let bytes = std::fs::read(&path).ok()?;
    let mut image = decode_png(&bytes).ok()?;
    image.stamp = file_stamp(&path);
    Some(Arc::new(image))
}

pub fn cache_age_secs() -> Option<u64> {
    let path = cache_path()?;
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let age = SystemTime::now().duration_since(modified).ok()?;
    Some(age.as_secs())
}

/// Cache when it is younger than [`REFRESH_SECS`], otherwise one fetch, otherwise
/// a stale cache. `None` when nothing is on disk and the network did not answer.
pub fn load_or_fetch() -> Option<Arc<SunImage>> {
    if cache_age_secs().is_some_and(|age| age < REFRESH_SECS) {
        if let Some(image) = load_cached() {
            return Some(image);
        }
    }
    match fetch() {
        Ok(image) => Some(Arc::new(image)),
        Err(err) => {
            eprintln!("clockface: SUVI image: {err}");
            load_cached()
        }
    }
}

pub fn fetch() -> Result<SunImage, String> {
    let output = Command::new("curl")
        .args([
            "-fsS",
            "--max-time",
            "20",
            "-A",
            "clockface/0.1 (SUVI display; 15min)",
            SUVI_URL,
        ])
        .output()
        .map_err(|err| format!("curl: {err}"))?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!("SUVI fetch failed ({})", err.trim()));
    }
    let mut image = decode_png(&output.stdout)?;
    image.stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(1);
    if let Some(path) = cache_path() {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, &output.stdout);
    }
    Ok(image)
}

pub fn decode_png(bytes: &[u8]) -> Result<SunImage, String> {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().map_err(|err| format!("png: {err}"))?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|err| format!("png frame: {err}"))?;
    let data = &buf[..info.buffer_size()];
    let width = info.width as usize;
    let height = info.height as usize;
    let rgba = match info.color_type {
        png::ColorType::Rgba => data.to_vec(),
        png::ColorType::Rgb => expand(data, 3, width, height),
        png::ColorType::Grayscale => expand(data, 1, width, height),
        png::ColorType::GrayscaleAlpha => expand(data, 2, width, height),
        png::ColorType::Indexed => {
            return Err("indexed PNG is not expanded".into());
        }
    };
    if rgba.len() != width * height * 4 {
        return Err("png size does not match the frame".into());
    }
    Ok(SunImage {
        width,
        height,
        rgba,
        stamp: 0,
    })
}

fn expand(data: &[u8], samples: usize, width: usize, height: usize) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(width * height * 4);
    for px in data.chunks_exact(samples) {
        match samples {
            1 => rgba.extend_from_slice(&[px[0], px[0], px[0], 255]),
            2 => rgba.extend_from_slice(&[px[0], px[0], px[0], px[1]]),
            _ => rgba.extend_from_slice(&[px[0], px[1], px[2], 255]),
        }
    }
    let _ = height;
    rgba
}

fn file_stamp(path: &std::path::Path) -> u64 {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_small_rgba_png_decodes() {
        let mut pixels = vec![0u8; 2 * 2 * 4];
        pixels[0] = 255;
        pixels[3] = 255;
        let mut encoded = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut encoded, 2, 2);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder.write_header().unwrap();
            writer.write_image_data(&pixels).unwrap();
        }
        let image = decode_png(&encoded).unwrap();
        assert_eq!(image.width, 2);
        assert_eq!(image.rgba[0], 255);
        assert!((DISC_RADIUS - 0.42).abs() < f32::EPSILON);
    }
}
