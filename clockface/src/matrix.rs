//! Amber dot-matrix for the info frames and the status line.
//!
//! Original 5×7 lamp glyphs. This is not a copy of any departure-board illustration.
//! Letters, digits, and punctuation are lamps. A symbol the matrix does not include
//! (the TimeWarp mark at the start of a line, or the network mark) is drawn from the
//! outline font and tinted the same amber as a lit lamp.

use egui::{Color32, FontId, Mesh, Painter, Pos2, Rect, Shape, Vec2};

const AMBER: Color32 = Color32::from_rgb(0xff, 0xb0, 0x28);
const BOARD: Color32 = Color32::from_rgb(0x07, 0x09, 0x10);
const UNLIT: Color32 = Color32::from_rgb(0x2a, 0x31, 0x3c);

/// Lamp pitch for the info frames. A glyph is 5×7 lamps, so the cap height is `7 * pitch`.
///
/// About 18% under the previous pitch, so a line holds more letters and the
/// forecast is less likely to wrap off the bottom of the board.
pub fn frame_pitch(radius: f32) -> f32 {
    (0.049 * radius / 7.0).clamp(1.8, 2.7)
}

pub fn paint_board(painter: &Painter, rect: Rect, pitch: f32) {
    paint_board_shadow(painter, rect);
    painter.rect_filled(rect, 0.0, BOARD);
    paint_unlit_field(painter, rect, pitch);
}

/// Soft square shadow on all four sides. The board itself stays square.
fn paint_board_shadow(painter: &Painter, rect: Rect) {
    for (grow, alpha) in [(14.0, 22u8), (8.0, 36), (3.0, 54)] {
        painter.rect_filled(
            rect.expand(grow),
            0.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, alpha),
        );
    }
}

/// Unlit lamps as one texture, not a dot mesh.
///
/// A portrait board is on the order of 10^4 lamps. Each lamp was a 9-vertex
/// fan, so one board was past 65,535 vertices. GLES 2 draws indexed meshes
/// with 16-bit indices unless `GL_OES_element_index_uint` is present and
/// honored; the overflow (or the cost of uploading it every frame) is the
/// scrambled blue band on the Pi. One quad stays under that limit, and
/// `egui_glow` already sets `GL_UNPACK_ALIGNMENT` to 1 before the upload.
fn paint_unlit_field(painter: &Painter, rect: Rect, pitch: f32) {
    let Some(texture) = unlit_texture(painter.ctx(), rect.size(), pitch) else {
        return;
    };
    painter.image(
        texture,
        rect,
        Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
        Color32::WHITE,
    );
}

#[derive(Clone)]
struct UnlitTex {
    w: u32,
    h: u32,
    pitch_bits: u32,
    handle: egui::TextureHandle,
}

fn unlit_texture(ctx: &egui::Context, size: Vec2, pitch: f32) -> Option<egui::TextureId> {
    let w = size.x.round().clamp(1.0, 2048.0) as u32;
    let h = size.y.round().clamp(1.0, 2048.0) as u32;
    let pitch_bits = (pitch.max(0.5) * 100.0).round() as u32;
    let id = egui::Id::unique("clockface-unlit-field");
    if let Some(hit) = ctx.data(|data| {
        data.get_temp::<Vec<UnlitTex>>(id).and_then(|entries| {
            entries
                .iter()
                .find(|entry| entry.w == w && entry.h == h && entry.pitch_bits == pitch_bits)
                .map(|entry| entry.handle.id())
        })
    }) {
        return Some(hit);
    }
    let image = unlit_image(w, h, pitch);
    let handle = ctx.load_texture("clockface-unlit", image, egui::TextureOptions::NEAREST);
    let texture = handle.id();
    ctx.data_mut(|data| {
        let entries = data.get_temp_mut_or_default::<Vec<UnlitTex>>(id);
        entries.retain(|entry| !(entry.w == w && entry.h == h && entry.pitch_bits == pitch_bits));
        if entries.len() >= 4 {
            entries.remove(0);
        }
        entries.push(UnlitTex {
            w,
            h,
            pitch_bits,
            handle,
        });
    });
    Some(texture)
}

fn unlit_image(w: u32, h: u32, pitch: f32) -> egui::ColorImage {
    let mut pixels = vec![BOARD; (w as usize) * (h as usize)];
    let radius = (pitch * 0.22).max(0.55);
    let mut y = pitch;
    let limit_y = h as f32 - pitch * 0.3;
    let limit_x = w as f32 - pitch * 0.3;
    while y < limit_y {
        let mut x = pitch;
        while x < limit_x {
            stamp_dot(&mut pixels, w, h, x, y, radius, UNLIT);
            x += pitch;
        }
        y += pitch;
    }
    egui::ColorImage::new([w as usize, h as usize], pixels)
}

fn stamp_dot(
    pixels: &mut [Color32],
    w: u32,
    h: u32,
    cx: f32,
    cy: f32,
    radius: f32,
    color: Color32,
) {
    let min_x = (cx - radius).floor().max(0.0) as i32;
    let max_x = (cx + radius).ceil().min(w as f32 - 1.0) as i32;
    let min_y = (cy - radius).floor().max(0.0) as i32;
    let max_y = (cy + radius).ceil().min(h as f32 - 1.0) as i32;
    let r2 = radius * radius;
    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            if dx * dx + dy * dy <= r2 {
                pixels[(y as u32 * w + x as u32) as usize] = color;
            }
        }
    }
}

/// How many unlit lamps the old per-frame mesh would have emitted.
pub fn unlit_lamp_count(rect: Rect, pitch: f32) -> usize {
    let mut count = 0usize;
    let mut y = rect.top() + pitch;
    while y < rect.bottom() - pitch * 0.3 {
        let mut x = rect.left() + pitch;
        while x < rect.right() - pitch * 0.3 {
            count += 1;
            x += pitch;
        }
        y += pitch;
    }
    count
}

/// Build the old fan mesh, so a test can time the cost the Pi was paying.
pub fn legacy_unlit_mesh(rect: Rect, pitch: f32) -> Mesh {
    let mut dots = Mesh::default();
    let r = pitch * 0.22;
    let mut y = rect.top() + pitch;
    while y < rect.bottom() - pitch * 0.3 {
        let mut x = rect.left() + pitch;
        while x < rect.right() - pitch * 0.3 {
            push_dot(&mut dots, Pos2::new(x, y), r, UNLIT);
            x += pitch;
        }
        y += pitch;
    }
    dots
}

/// One row of lit lamps across the text column. `y` is a grid row, the same pitch as the glyphs.
pub fn paint_rule(painter: &Painter, bounds: Rect, y: f32, pitch: f32) {
    let painter = painter.with_clip_rect(bounds);
    let mut glow = Mesh::default();
    let mut lit = Mesh::default();
    let glow_color = Color32::from_rgba_premultiplied(0x55, 0x32, 0x08, 0x66);
    let mut x = bounds.left() + pitch;
    let right = bounds.right() - pitch * 0.3;
    while x < right {
        let at = Pos2::new(x, y);
        push_dot(&mut glow, at, pitch * 0.72, glow_color);
        push_dot(&mut lit, at, pitch * 0.36, AMBER);
        x += pitch;
    }
    add_mesh(&painter, glow);
    add_mesh(&painter, lit);
}

/// Width of one 5×7 character cell, including the gap before the next glyph.
pub fn cell(pitch: f32) -> f32 {
    pitch * 6.0
}

/// Two character cells. Wrapped lines, and a calendar date under its title, start here.
pub fn hang(pitch: f32) -> f32 {
    cell(pitch) * 2.0
}

/// Largest pitch at which `text` fits on one row inside `width`.
pub fn pitch_to_fit(text: &str, width: f32, max_pitch: f32) -> f32 {
    let mut pitch = max_pitch.max(1.2);
    while pitch > 1.2 && line_width(text, pitch) > width {
        pitch -= 0.05;
    }
    pitch
}

/// Draw `text` on the lamp grid. ASCII letters and numbers are lamps. Any other
/// glyph is an amber exception, the same hue as a lit lamp.
///
/// `hang_cells` is how far a wrapped continuation moves in from `origin`. The
/// first line stays at `origin`. A calendar date passes an origin that is already
/// indented and `hang_cells` of 0, so a wrap of that date stays under the date.
/// Returns the y of the next row.
pub fn paint_text(
    painter: &Painter,
    bounds: Rect,
    origin: Pos2,
    pitch: f32,
    text: &str,
    hang_cells: u32,
) -> f32 {
    paint_text_limited(
        painter,
        bounds,
        origin,
        pitch,
        text,
        hang_cells,
        bounds.right() - pitch * 0.4,
    )
}

/// One row, clipped to `bounds`, with no wrap. Used for the status version.
pub fn paint_text_nowrap(
    painter: &Painter,
    bounds: Rect,
    origin: Pos2,
    pitch: f32,
    text: &str,
) -> f32 {
    paint_text_limited(painter, bounds, origin, pitch, text, 0, f32::MAX)
}

fn paint_text_limited(
    painter: &Painter,
    bounds: Rect,
    origin: Pos2,
    pitch: f32,
    text: &str,
    hang_cells: u32,
    max_x: f32,
) -> f32 {
    let painter = painter.with_clip_rect(bounds);
    let line = pitch * 9.0;
    let hang_x = (origin.x + hang_cells as f32 * cell(pitch)).min(max_x - cell(pitch));
    let mut x = origin.x;
    let mut y = origin.y;
    let mut line_left = origin.x;
    let mut glow = Mesh::default();
    let mut lit = Mesh::default();
    let serif = egui::FontFamily::Name("serif".into());
    let mark_font = FontId::new(pitch * 7.0, serif);

    for word in words(text) {
        let width: f32 = word.chars().map(|ch| char_width(&painter, ch, pitch)).sum();
        if x > line_left + 0.5 && x + width > max_x {
            if word == " " {
                y += line;
                line_left = hang_x;
                x = hang_x;
                continue;
            }
            y += line;
            line_left = hang_x;
            x = hang_x;
        }
        if y > bounds.bottom() {
            break;
        }
        // A word longer than the remaining row still breaks between lamps.
        for ch in word.chars() {
            let width = char_width(&painter, ch, pitch);
            if x > line_left + 0.5 && x + width > max_x {
                y += line;
                line_left = hang_x;
                x = hang_x;
            }
            if let Some(rows) = glyph(ch) {
                stamp(&mut glow, &mut lit, Pos2::new(x, y), pitch, rows);
            } else {
                let galley = painter.layout_no_wrap(ch.to_string(), mark_font.clone(), AMBER);
                painter.galley(Pos2::new(x, y - pitch * 0.15), galley, AMBER);
            }
            x += width;
        }
    }
    add_mesh(&painter, glow);
    add_mesh(&painter, lit);
    y + line
}

/// GLES 2 indexes a mesh with 16-bit elements unless the uint extension is
/// reliable. Keep every draw under that, on a boundary between lamp fans
/// (9 vertices each) so a triangle is never split.
fn add_mesh(painter: &Painter, mesh: Mesh) {
    const LIMIT: usize = 16380;
    if mesh.vertices.len() <= LIMIT {
        if mesh.indices.len() >= 3 {
            painter.add(Shape::mesh(mesh));
        }
        return;
    }
    let mut start = 0;
    while start < mesh.vertices.len() {
        let end = (start + LIMIT).min(mesh.vertices.len());
        let mut part = Mesh::default();
        part.vertices.extend_from_slice(&mesh.vertices[start..end]);
        for tri in mesh.indices.chunks_exact(3) {
            let a = tri[0] as usize;
            let b = tri[1] as usize;
            let c = tri[2] as usize;
            if a >= start && b >= start && c >= start && a < end && b < end && c < end {
                part.add_triangle((a - start) as u32, (b - start) as u32, (c - start) as u32);
            }
        }
        if part.indices.len() >= 3 {
            painter.add(Shape::mesh(part));
        }
        start = end;
    }
}

fn words(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut word = String::new();
    for ch in text.chars() {
        if ch == ' ' {
            if !word.is_empty() {
                out.push(word);
                word = String::new();
            }
            out.push(" ".into());
        } else {
            word.push(ch);
        }
    }
    if !word.is_empty() {
        out.push(word);
    }
    out
}

/// Width of `text` in lamp cells. A character the matrix does not include uses `mark`.
pub fn text_width(text: &str, pitch: f32, mark: impl Fn(char) -> f32) -> f32 {
    text.chars()
        .map(|ch| {
            if glyph(ch).is_some() {
                cell(pitch)
            } else {
                mark(ch)
            }
        })
        .sum()
}

/// Shorten `text` with `...` so it fits in `max_width`. A string that already fits is unchanged.
pub fn ellipsize(text: &str, pitch: f32, max_width: f32, mark: impl Fn(char) -> f32) -> String {
    if text_width(text, pitch, &mark) <= max_width {
        return text.to_string();
    }
    let dots = "...";
    let dots_w = text_width(dots, pitch, &mark);
    if dots_w > max_width {
        return String::new();
    }
    let mut out = String::new();
    let mut used = 0.0;
    for ch in text.chars() {
        let width = if glyph(ch).is_some() {
            cell(pitch)
        } else {
            mark(ch)
        };
        if used + width + dots_w > max_width {
            break;
        }
        out.push(ch);
        used += width;
    }
    while out.ends_with(' ') {
        out.pop();
    }
    out.push_str(dots);
    out
}

fn char_width(painter: &Painter, ch: char, pitch: f32) -> f32 {
    if glyph(ch).is_some() {
        pitch * 6.0
    } else {
        let font = FontId::new(pitch * 7.0, egui::FontFamily::Name("serif".into()));
        painter.layout_no_wrap(ch.to_string(), font, AMBER).size().x + pitch * 0.5
    }
}

fn line_width(text: &str, pitch: f32) -> f32 {
    text.chars()
        .map(|ch| {
            if glyph(ch).is_some() {
                pitch * 6.0
            } else {
                pitch * 7.5
            }
        })
        .sum()
}

fn stamp(glow: &mut Mesh, lit: &mut Mesh, origin: Pos2, pitch: f32, rows: [u8; 7]) {
    let glow_color = Color32::from_rgba_premultiplied(0x55, 0x32, 0x08, 0x66);
    for (row, bits) in rows.iter().enumerate() {
        for col in 0..5 {
            if bits & (1 << (4 - col)) == 0 {
                continue;
            }
            let at = origin + Vec2::new(col as f32 * pitch, row as f32 * pitch);
            push_dot(glow, at, pitch * 0.72, glow_color);
            push_dot(lit, at, pitch * 0.36, AMBER);
        }
    }
}

fn push_dot(mesh: &mut Mesh, center: Pos2, radius: f32, color: Color32) {
    let n = 7u32;
    let base = mesh.vertices.len() as u32;
    mesh.colored_vertex(center, color);
    for i in 0..=n {
        let a = i as f32 / n as f32 * std::f32::consts::TAU;
        mesh.colored_vertex(center + Vec2::new(a.cos(), a.sin()) * radius, color);
    }
    for i in 0..n {
        mesh.add_triangle(base, base + 1 + i, base + 2 + i);
    }
}

fn glyph(ch: char) -> Option<[u8; 7]> {
    let ch = match ch {
        'a'..='z' => ch.to_ascii_uppercase(),
        ' ' => return Some([0; 7]),
        other => other,
    };
    Some(match ch {
        'A' => rows([
            ".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
        ]),
        'B' => rows([
            "####.", "#...#", "#...#", "####.", "#...#", "#...#", "####.",
        ]),
        'C' => rows([
            ".###.", "#...#", "#....", "#....", "#....", "#...#", ".###.",
        ]),
        'D' => rows([
            "####.", "#...#", "#...#", "#...#", "#...#", "#...#", "####.",
        ]),
        'E' => rows([
            "#####", "#....", "#....", "####.", "#....", "#....", "#####",
        ]),
        'F' => rows([
            "#####", "#....", "#....", "####.", "#....", "#....", "#....",
        ]),
        'G' => rows([
            ".###.", "#...#", "#....", "#.###", "#...#", "#...#", ".###.",
        ]),
        'H' => rows([
            "#...#", "#...#", "#...#", "#####", "#...#", "#...#", "#...#",
        ]),
        'I' => rows([
            ".###.", "..#..", "..#..", "..#..", "..#..", "..#..", ".###.",
        ]),
        'J' => rows([
            "..###", "...#.", "...#.", "...#.", "#..#.", "#..#.", ".##..",
        ]),
        'K' => rows([
            "#...#", "#..#.", "#.#..", "##...", "#.#..", "#..#.", "#...#",
        ]),
        'L' => rows([
            "#....", "#....", "#....", "#....", "#....", "#....", "#####",
        ]),
        'M' => rows([
            "#...#", "##.##", "#.#.#", "#...#", "#...#", "#...#", "#...#",
        ]),
        'N' => rows([
            "#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#",
        ]),
        'O' => rows([
            ".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
        ]),
        'P' => rows([
            "####.", "#...#", "#...#", "####.", "#....", "#....", "#....",
        ]),
        'Q' => rows([
            ".###.", "#...#", "#...#", "#...#", "#.#.#", "#..#.", ".##.#",
        ]),
        'R' => rows([
            "####.", "#...#", "#...#", "####.", "#.#..", "#..#.", "#...#",
        ]),
        'S' => rows([
            ".####", "#....", "#....", ".###.", "....#", "....#", "####.",
        ]),
        'T' => rows([
            "#####", "..#..", "..#..", "..#..", "..#..", "..#..", "..#..",
        ]),
        'U' => rows([
            "#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###.",
        ]),
        'V' => rows([
            "#...#", "#...#", "#...#", "#...#", "#...#", ".#.#.", "..#..",
        ]),
        'W' => rows([
            "#...#", "#...#", "#...#", "#.#.#", "#.#.#", "##.##", "#...#",
        ]),
        'X' => rows([
            "#...#", "#...#", ".#.#.", "..#..", ".#.#.", "#...#", "#...#",
        ]),
        'Y' => rows([
            "#...#", "#...#", ".#.#.", "..#..", "..#..", "..#..", "..#..",
        ]),
        'Z' => rows([
            "#####", "....#", "...#.", "..#..", ".#...", "#....", "#####",
        ]),
        '0' => rows([
            ".###.", "#...#", "#..##", "#.#.#", "##..#", "#...#", ".###.",
        ]),
        '1' => rows([
            "..#..", ".##..", "..#..", "..#..", "..#..", "..#..", ".###.",
        ]),
        '2' => rows([
            ".###.", "#...#", "....#", "...#.", "..#..", ".#...", "#####",
        ]),
        '3' => rows([
            ".###.", "#...#", "....#", "..##.", "....#", "#...#", ".###.",
        ]),
        '4' => rows([
            "...#.", "..##.", ".#.#.", "#..#.", "#####", "...#.", "...#.",
        ]),
        '5' => rows([
            "#####", "#....", "####.", "....#", "....#", "#...#", ".###.",
        ]),
        '6' => rows([
            ".###.", "#....", "#....", "####.", "#...#", "#...#", ".###.",
        ]),
        '7' => rows([
            "#####", "....#", "...#.", "..#..", ".#...", ".#...", ".#...",
        ]),
        '8' => rows([
            ".###.", "#...#", "#...#", ".###.", "#...#", "#...#", ".###.",
        ]),
        '9' => rows([
            ".###.", "#...#", "#...#", ".####", "....#", "....#", ".###.",
        ]),
        '.' => rows([
            ".....", ".....", ".....", ".....", ".....", "..#..", "..#..",
        ]),
        ',' => rows([
            ".....", ".....", ".....", ".....", "..#..", "..#..", ".#...",
        ]),
        ':' => rows([
            ".....", "..#..", "..#..", ".....", "..#..", "..#..", ".....",
        ]),
        ';' => rows([
            ".....", "..#..", "..#..", ".....", "..#..", "..#..", ".#...",
        ]),
        '\'' => rows([
            ".#...", ".#...", ".....", ".....", ".....", ".....", ".....",
        ]),
        '-' => rows([
            ".....", ".....", ".....", "#####", ".....", ".....", ".....",
        ]),
        '_' => rows([
            ".....", ".....", ".....", ".....", ".....", ".....", "#####",
        ]),
        '/' => rows([
            "....#", "...#.", "...#.", "..#..", ".#...", ".#...", "#....",
        ]),
        '+' => rows([
            "..#..", "..#..", "#####", "..#..", "#####", "..#..", "..#..",
        ]),
        '(' => rows([
            "..#..", ".#...", ".#...", ".#...", ".#...", ".#...", "..#..",
        ]),
        ')' => rows([
            "..#..", "...#.", "...#.", "...#.", "...#.", "...#.", "..#..",
        ]),
        '?' => rows([
            ".###.", "#...#", "....#", "..#..", "..#..", ".....", "..#..",
        ]),
        '!' => rows([
            "..#..", "..#..", "..#..", "..#..", "..#..", ".....", "..#..",
        ]),
        '°' => rows([
            ".##..", "#..#.", ".##..", ".....", ".....", ".....", ".....",
        ]),
        '@' => rows([
            ".###.", "#...#", "#.##.", "#.#.#", "#.##.", "#....", ".###.",
        ]),
        '·' | '•' => rows([
            ".....", ".....", "..#..", ".....", "..#..", ".....", ".....",
        ]),
        '—' | '–' => rows([
            ".....", ".....", ".....", "#####", ".....", ".....", ".....",
        ]),
        _ => return None,
    })
}

fn rows(lines: [&str; 7]) -> [u8; 7] {
    let mut out = [0u8; 7];
    for (i, line) in lines.iter().enumerate() {
        let mut bits = 0u8;
        for (col, ch) in line.chars().take(5).enumerate() {
            if ch == '#' {
                bits |= 1 << (4 - col);
            }
        }
        out[i] = bits;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn letters_are_five_by_seven_lamps() {
        let a = glyph('A').unwrap();
        assert_eq!(a.len(), 7);
        assert!(a.iter().any(|row| *row != 0));
        assert_eq!(glyph('a'), glyph('A'));
        assert_eq!(glyph(' '), Some([0; 7]));
        assert!(glyph('°').unwrap().iter().any(|row| *row != 0));
        assert!(glyph('@').unwrap().iter().any(|row| *row != 0));
        assert!(glyph('❄').is_none());
    }

    #[test]
    fn hanging_indent_is_two_character_cells() {
        let pitch = frame_pitch(321.6);
        assert!((cell(pitch) - pitch * 6.0).abs() < 1e-4);
        assert!(
            (hang(pitch) - cell(pitch) * 2.0).abs() < 1e-4,
            "hang {} should be two cells",
            hang(pitch)
        );
    }

    #[test]
    fn a_long_status_fault_is_cut_before_it_passes_the_budget() {
        let pitch = 2.0;
        let mark = |ch: char| {
            let _ = ch;
            pitch * 7.5
        };
        let text = "cursor \u{1F5A7} 172.30.0.2  time sync unknown and a very long weather note";
        let fitted = ellipsize(text, pitch, 80.0, mark);
        assert!(fitted.ends_with("..."), "{fitted}");
        assert!(text_width(&fitted, pitch, mark) <= 80.0, "{fitted}");
        let short = ellipsize("cursor", pitch, 80.0, mark);
        assert_eq!(short, "cursor");
    }

    #[test]
    fn frame_pitch_is_about_a_sixteen_pixel_cap_on_the_screenshot_face() {
        let pitch = frame_pitch(321.6);
        let cap = pitch * 7.0;
        assert!((15.0..17.0).contains(&cap), "cap height {cap}");
    }

    #[test]
    fn a_portrait_board_grid_used_to_exceed_the_gles2_index_limit() {
        // Pi 3, logical 1024×1280, one info board about half the dial wide.
        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::new(383.0, 250.0));
        let pitch = 2.7;
        let lamps = unlit_lamp_count(rect, pitch);
        let started = std::time::Instant::now();
        let mesh = legacy_unlit_mesh(rect, pitch);
        let built = started.elapsed();
        eprintln!(
            "legacy unlit board: {lamps} lamps, {} vertices, built in {:.1} ms",
            mesh.vertices.len(),
            built.as_secs_f64() * 1000.0
        );
        assert!(
            mesh.vertices.len() > 65_535,
            "the old grid has to be the GLES overflow, got {}",
            mesh.vertices.len()
        );
        assert_eq!(mesh.vertices.len(), lamps * 9);

        let ctx = egui::Context::default();
        let mut input = egui::RawInput::default();
        let screen = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0));
        input.screen_rect = Some(screen);
        let mut output = ctx.run_pass(input, |ctx| {
            let painter = ctx.layer_painter(egui::LayerId::background());
            paint_board(&painter, rect, pitch);
        });
        let primitives = ctx.tessellate(output.shapes, 1.0);
        let verts: usize = primitives
            .iter()
            .map(|clipped| match &clipped.primitive {
                egui::epaint::Primitive::Mesh(mesh) => mesh.vertices.len(),
                _ => 0,
            })
            .sum();
        assert!(
            verts < 1_000,
            "the texture board should be a handful of quads, got {verts}"
        );
        eprintln!("texture board: {verts} vertices");
        output.textures_delta.clear();
    }
}
