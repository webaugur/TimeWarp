//! CPU rasterizer for one egui frame.
//!
//! `--screenshot` and the pixel tests use this so the face can be checked
//! without a display. It follows the egui_glow 0.31 path: tessellated meshes
//! in points, vertex color multiplied by the texture in gamma space, then
//! premultiplied blending (`src + dst * (1 - src_a)`).

use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use chrono::{DateTime, Local};
use egui::epaint::{Color32, ImageData, ImageDelta, Primitive, Vertex};
use egui::{ClippedPrimitive, Context, Pos2, Rect, TextureId, Vec2};

use crate::fonts::install_fonts;
use crate::roman;

/// Render the clock on a black framebuffer. Row-major RGBA8, origin top-left.
pub fn render_rgba(
    width: u32,
    height: u32,
    center: Pos2,
    diameter: f32,
    now: DateTime<Local>,
) -> Result<Vec<u8>, String> {
    if width == 0 || height == 0 || width > 8192 || height > 8192 {
        return Err(format!("screenshot size {width}x{height} is out of range"));
    }
    let pixels = (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| "screenshot dimensions overflow".to_owned())?;

    let ctx = Context::default();
    install_fonts(&ctx);
    ctx.set_pixels_per_point(1.0);

    let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(width as f32, height as f32));
    let mut input = egui::RawInput::default();
    input.screen_rect = Some(screen);
    input.predicted_dt = 1.0 / 60.0;
    input.viewports.insert(
        egui::ViewportId::ROOT,
        egui::ViewportInfo {
            inner_rect: Some(screen),
            native_pixels_per_point: Some(1.0),
            ..Default::default()
        },
    );

    let output = ctx.run(input, |ctx| {
        let painter = ctx.layer_painter(egui::LayerId::background());
        painter.rect_filled(screen, 0.0, Color32::BLACK);
        roman::paint(&painter, center, diameter, now);
    });

    let mut textures = TextureSet::default();
    textures.apply(output.textures_delta);
    let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);

    let mut buf = vec![0u8; pixels];
    for pixel in buf.chunks_exact_mut(4) {
        pixel[2] = 0;
        pixel[3] = 255;
    }
    for clipped in primitives {
        paint_primitive(&mut buf, width, height, &clipped, &textures);
    }
    Ok(buf)
}

pub fn write_png(path: &Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    if rgba.len() != width as usize * height as usize * 4 {
        return Err("PNG buffer length does not match the image size".into());
    }
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|err| format!("creating {}: {err}", parent.display()))?;
        }
    }
    let file = File::create(path).map_err(|err| format!("creating {}: {err}", path.display()))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|err| format!("png header: {err}"))?;
    writer
        .write_image_data(rgba)
        .map_err(|err| format!("png body: {err}"))?;
    Ok(())
}

#[derive(Default)]
struct TextureSet {
    textures: HashMap<TextureId, Atlas>,
}

struct Atlas {
    width: usize,
    height: usize,
    /// Premultiplied sRGBA, row-major.
    pixels: Vec<[u8; 4]>,
}

impl TextureSet {
    fn apply(&mut self, delta: egui::TexturesDelta) {
        for (id, image) in delta.set {
            self.apply_one(id, image);
        }
        for id in delta.free {
            self.textures.remove(&id);
        }
    }

    fn apply_one(&mut self, id: TextureId, delta: ImageDelta) {
        let (width, height, pixels) = decode_image(&delta.image);
        match delta.pos {
            None => {
                self.textures.insert(
                    id,
                    Atlas {
                        width,
                        height,
                        pixels,
                    },
                );
            }
            Some([x, y]) => {
                let atlas = self.textures.entry(id).or_insert_with(|| Atlas {
                    width: x + width,
                    height: y + height,
                    pixels: vec![[0, 0, 0, 0]; (x + width) * (y + height)],
                });
                atlas.ensure_size(x + width, y + height);
                atlas.blit(x, y, width, height, &pixels);
            }
        }
    }

    fn sample(&self, id: TextureId, uv: [f32; 2]) -> [f32; 4] {
        let Some(atlas) = self.textures.get(&id) else {
            return [1.0, 1.0, 1.0, 1.0];
        };
        if atlas.width == 0 || atlas.height == 0 {
            return [1.0, 1.0, 1.0, 1.0];
        }
        let x = (uv[0] * atlas.width as f32 - 0.5).clamp(0.0, atlas.width as f32 - 1.0);
        let y = (uv[1] * atlas.height as f32 - 0.5).clamp(0.0, atlas.height as f32 - 1.0);
        let x0 = x.floor() as usize;
        let y0 = y.floor() as usize;
        let x1 = (x0 + 1).min(atlas.width - 1);
        let y1 = (y0 + 1).min(atlas.height - 1);
        let tx = x - x0 as f32;
        let ty = y - y0 as f32;
        let c00 = texel(atlas, x0, y0);
        let c10 = texel(atlas, x1, y0);
        let c01 = texel(atlas, x0, y1);
        let c11 = texel(atlas, x1, y1);
        let mut out = [0.0; 4];
        for i in 0..4 {
            let top = c00[i] + (c10[i] - c00[i]) * tx;
            let bot = c01[i] + (c11[i] - c01[i]) * tx;
            out[i] = top + (bot - top) * ty;
        }
        out
    }
}

fn texel(atlas: &Atlas, x: usize, y: usize) -> [f32; 4] {
    let p = atlas.pixels[y * atlas.width + x];
    [
        p[0] as f32 / 255.0,
        p[1] as f32 / 255.0,
        p[2] as f32 / 255.0,
        p[3] as f32 / 255.0,
    ]
}

impl Atlas {
    fn ensure_size(&mut self, width: usize, height: usize) {
        if width <= self.width && height <= self.height {
            return;
        }
        let new_w = width.max(self.width);
        let new_h = height.max(self.height);
        let mut next = vec![[0, 0, 0, 0]; new_w * new_h];
        for y in 0..self.height {
            let src = y * self.width;
            let dst = y * new_w;
            next[dst..dst + self.width].copy_from_slice(&self.pixels[src..src + self.width]);
        }
        self.width = new_w;
        self.height = new_h;
        self.pixels = next;
    }

    fn blit(&mut self, x: usize, y: usize, width: usize, height: usize, pixels: &[[u8; 4]]) {
        for row in 0..height {
            let dst = (y + row) * self.width + x;
            let src = row * width;
            self.pixels[dst..dst + width].copy_from_slice(&pixels[src..src + width]);
        }
    }
}

fn decode_image(image: &ImageData) -> (usize, usize, Vec<[u8; 4]>) {
    match image {
        ImageData::Color(image) => {
            let pixels = image
                .pixels
                .iter()
                .map(Color32::to_array)
                .collect::<Vec<_>>();
            (image.width(), image.height(), pixels)
        }
        ImageData::Font(image) => {
            let pixels = image
                .srgba_pixels(None)
                .map(|color| color.to_array())
                .collect();
            (image.width(), image.height(), pixels)
        }
    }
}

fn paint_primitive(
    buf: &mut [u8],
    width: u32,
    height: u32,
    clipped: &ClippedPrimitive,
    textures: &TextureSet,
) {
    let Primitive::Mesh(mesh) = &clipped.primitive else {
        return;
    };
    let clip = scissor(clipped.clip_rect, width, height);
    let indices = &mesh.indices;
    let vertices = &mesh.vertices;
    let mut i = 0;
    while i + 2 < indices.len() {
        let a = indices[i] as usize;
        let b = indices[i + 1] as usize;
        let c = indices[i + 2] as usize;
        i += 3;
        if a >= vertices.len() || b >= vertices.len() || c >= vertices.len() {
            continue;
        }
        fill_triangle(
            buf,
            width,
            height,
            clip,
            textures,
            mesh.texture_id,
            &vertices[a],
            &vertices[b],
            &vertices[c],
        );
    }
}

#[derive(Clone, Copy)]
struct Scissor {
    min_x: i32,
    min_y: i32,
    max_x: i32,
    max_y: i32,
}

fn scissor(clip: Rect, width: u32, height: u32) -> Scissor {
    // Match egui_glow: the clip rect is in points and the scissor is rounded pixels.
    let min_x = (clip.min.x).round() as i32;
    let min_y = (clip.min.y).round() as i32;
    let max_x = (clip.max.x).round() as i32;
    let max_y = (clip.max.y).round() as i32;
    Scissor {
        min_x: min_x.clamp(0, width as i32),
        min_y: min_y.clamp(0, height as i32),
        max_x: max_x.clamp(0, width as i32),
        max_y: max_y.clamp(0, height as i32),
    }
}

fn fill_triangle(
    buf: &mut [u8],
    width: u32,
    height: u32,
    clip: Scissor,
    textures: &TextureSet,
    texture: TextureId,
    v0: &Vertex,
    v1: &Vertex,
    v2: &Vertex,
) {
    let area = edge(v0.pos, v1.pos, v2.pos);
    if area.abs() < 1e-4 {
        return;
    }
    let min_x = v0.pos.x.min(v1.pos.x).min(v2.pos.x).floor() as i32;
    let max_x = v0.pos.x.max(v1.pos.x).max(v2.pos.x).ceil() as i32;
    let min_y = v0.pos.y.min(v1.pos.y).min(v2.pos.y).floor() as i32;
    let max_y = v0.pos.y.max(v1.pos.y).max(v2.pos.y).ceil() as i32;
    let min_x = min_x.max(clip.min_x).max(0);
    let min_y = min_y.max(clip.min_y).max(0);
    let max_x = max_x.min(clip.max_x).min(width as i32);
    let max_y = max_y.min(clip.max_y).min(height as i32);
    if min_x >= max_x || min_y >= max_y {
        return;
    }

    // Barycentric edge functions. d(edge(b, c, p))/dx = c.y - b.y, /dy = b.x - c.x.
    let e0_dx = v2.pos.y - v1.pos.y;
    let e1_dx = v0.pos.y - v2.pos.y;
    let e2_dx = v1.pos.y - v0.pos.y;
    let e0_dy = v1.pos.x - v2.pos.x;
    let e1_dy = v2.pos.x - v0.pos.x;
    let e2_dy = v0.pos.x - v1.pos.x;

    let origin = Pos2::new(min_x as f32 + 0.5, min_y as f32 + 0.5);
    let mut row0 = edge(v1.pos, v2.pos, origin);
    let mut row1 = edge(v2.pos, v0.pos, origin);
    let mut row2 = edge(v0.pos, v1.pos, origin);

    let c0 = color_of(v0);
    let c1 = color_of(v1);
    let c2 = color_of(v2);
    let uv0 = uv_of(v0);
    let uv1 = uv_of(v1);
    let uv2 = uv_of(v2);
    let inv_area = 1.0 / area;
    let stride = width as usize * 4;

    for y in min_y..max_y {
        let mut w0 = row0;
        let mut w1 = row1;
        let mut w2 = row2;
        let row = y as usize * stride;
        for x in min_x..max_x {
            let inside =
                (w0 >= 0.0 && w1 >= 0.0 && w2 >= 0.0) || (w0 <= 0.0 && w1 <= 0.0 && w2 <= 0.0);
            if inside {
                let b0 = w0 * inv_area;
                let b1 = w1 * inv_area;
                let b2 = w2 * inv_area;
                let mut color = [0.0; 4];
                let mut uv = [0.0; 2];
                for i in 0..4 {
                    color[i] = b0 * c0[i] + b1 * c1[i] + b2 * c2[i];
                }
                for i in 0..2 {
                    uv[i] = b0 * uv0[i] + b1 * uv1[i] + b2 * uv2[i];
                }
                let tex = textures.sample(texture, uv);
                let src = [
                    color[0] * tex[0],
                    color[1] * tex[1],
                    color[2] * tex[2],
                    color[3] * tex[3],
                ];
                blend(
                    &mut buf[row + x as usize * 4..row + x as usize * 4 + 4],
                    src,
                );
            }
            w0 += e0_dx;
            w1 += e1_dx;
            w2 += e2_dx;
        }
        row0 += e0_dy;
        row1 += e1_dy;
        row2 += e2_dy;
    }
}

fn edge(a: Pos2, b: Pos2, p: Pos2) -> f32 {
    (p.x - a.x) * (b.y - a.y) - (p.y - a.y) * (b.x - a.x)
}

fn color_of(v: &Vertex) -> [f32; 4] {
    let [r, g, b, a] = v.color.to_array();
    [
        r as f32 / 255.0,
        g as f32 / 255.0,
        b as f32 / 255.0,
        a as f32 / 255.0,
    ]
}

fn uv_of(v: &Vertex) -> [f32; 2] {
    [v.uv.x, v.uv.y]
}

fn blend(dst: &mut [u8], src: [f32; 4]) {
    let sa = src[3];
    if sa <= 0.0 {
        return;
    }
    if sa >= 0.999 {
        dst[0] = to_u8(src[0]);
        dst[1] = to_u8(src[1]);
        dst[2] = to_u8(src[2]);
        dst[3] = to_u8(sa);
        return;
    }
    let inv = 1.0 - sa;
    for i in 0..3 {
        let d = dst[i] as f32 / 255.0;
        dst[i] = to_u8(src[i] + d * inv);
    }
    let da = dst[3] as f32 / 255.0;
    dst[3] = to_u8(sa + da * inv);
}

fn to_u8(value: f32) -> u8 {
    value.clamp(0.0, 1.0).mul_add(255.0, 0.5) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::roman::{direction, screenshot_time, RomanClockStyle};
    use crate::Settings;

    #[test]
    fn screenshot_frame_matches_the_spec_colors() {
        let width = 960;
        let height = 700;
        let settings = Settings::default();
        let center = settings.center(Vec2::new(width as f32, height as f32));
        let diameter = settings.diameter;
        let rgba = render_rgba(width, height, center, diameter, screenshot_time()).unwrap();
        let r = diameter * 0.5;

        let corner = at(&rgba, width, 4, height as i32 - 4);
        assert_eq!(corner, [0, 0, 0, 255], "background should be black");

        // Clear of hands (10:08:37) and of the tick ring: 20° at 0.40 r.
        let face_at = center + direction(20.0) * (0.40 * r);
        let face = at_pos(&rgba, width, height, face_at);
        let t = 0.40;
        let expect = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as i32;
        let er = expect(0xf7, 0xe5);
        let eg = expect(0xdb, 0xca);
        let eb = expect(0xb7, 0xa7);
        assert!(
            (face[0] as i32 - er).abs() <= 3
                && (face[1] as i32 - eg).abs() <= 3
                && (face[2] as i32 - eb).abs() <= 3,
            "face gradient at {face_at:?} was {face:?}, expected ~[{er}, {eg}, {eb}]"
        );

        // The rim is a thin antialiased stroke; accept a pixel in the neighborhood.
        let rim_at = center + direction(20.0) * (0.99 * r);
        assert!(
            near_color(&rgba, width, height, rim_at, [0x7c, 0x4e, 0x1f], 18),
            "outline near {rim_at:?} was {}",
            format!("{:?}", at_pos(&rgba, width, height, rim_at))
        );

        let style = RomanClockStyle::default();
        let (hour, minute, second) = crate::hand_angles(screenshot_time(), &style);
        assert!((second - 222.0).abs() < 0.01);
        let _ = (hour, minute);
        assert!(
            near_color(
                &rgba,
                width,
                height,
                center + direction(second) * (0.45 * r),
                [255, 0, 0],
                40
            ),
            "second hand should be red"
        );
        assert!(
            near_color(
                &rgba,
                width,
                height,
                center + direction(minute) * (0.50 * r),
                [0, 0, 0],
                40
            ),
            "minute hand should be black"
        );

        let numeral = center + direction(0.0) * (style.numeral_radius * r);
        let brown = count_near(
            &rgba,
            width,
            height,
            numeral,
            0.16 * r,
            [0x47, 0x30, 0x13],
            48,
        );
        assert!(
            brown > 40,
            "XII should be painted in #473013, found {brown} pixels"
        );
    }

    fn at(buf: &[u8], width: u32, x: i32, y: i32) -> [u8; 4] {
        let i = (y as usize * width as usize + x as usize) * 4;
        [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
    }

    fn at_pos(buf: &[u8], width: u32, height: u32, p: Pos2) -> [u8; 4] {
        let x = (p.x.floor() as i32).clamp(0, width as i32 - 1);
        let y = (p.y.floor() as i32).clamp(0, height as i32 - 1);
        at(buf, width, x, y)
    }

    fn near_color(buf: &[u8], width: u32, height: u32, p: Pos2, expect: [u8; 3], tol: i32) -> bool {
        let x0 = p.x.floor() as i32;
        let y0 = p.y.floor() as i32;
        for dy in -2..=2 {
            for dx in -2..=2 {
                let x = x0 + dx;
                let y = y0 + dy;
                if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
                    continue;
                }
                let px = at(buf, width, x, y);
                if (px[0] as i32 - expect[0] as i32).abs() <= tol
                    && (px[1] as i32 - expect[1] as i32).abs() <= tol
                    && (px[2] as i32 - expect[2] as i32).abs() <= tol
                {
                    return true;
                }
            }
        }
        false
    }

    fn count_near(
        buf: &[u8],
        width: u32,
        height: u32,
        center: Pos2,
        radius: f32,
        expect: [u8; 3],
        tol: i32,
    ) -> usize {
        let x0 = (center.x - radius).floor().max(0.0) as i32;
        let y0 = (center.y - radius).floor().max(0.0) as i32;
        let x1 = (center.x + radius).ceil().min(width as f32 - 1.0) as i32;
        let y1 = (center.y + radius).ceil().min(height as f32 - 1.0) as i32;
        let mut n = 0;
        for y in y0..=y1 {
            for x in x0..=x1 {
                let px = at(buf, width, x, y);
                if (px[0] as i32 - expect[0] as i32).abs() <= tol
                    && (px[1] as i32 - expect[1] as i32).abs() <= tol
                    && (px[2] as i32 - expect[2] as i32).abs() <= tol
                {
                    n += 1;
                }
            }
        }
        n
    }
}
