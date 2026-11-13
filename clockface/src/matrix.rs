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
pub fn frame_pitch(radius: f32) -> f32 {
    (0.060 * radius / 7.0).clamp(2.2, 3.3)
}

pub fn paint_board(painter: &Painter, rect: Rect, pitch: f32) {
    painter.rect_filled(rect, 0.0, BOARD);
    let painter = painter.with_clip_rect(rect);
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
    if dots.indices.len() >= 3 {
        painter.add(Shape::mesh(dots));
    }
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
    let painter = painter.with_clip_rect(bounds);
    let line = pitch * 9.0;
    let max_x = bounds.right() - pitch * 0.4;
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
    if glow.indices.len() >= 3 {
        painter.add(Shape::mesh(glow));
    }
    if lit.indices.len() >= 3 {
        painter.add(Shape::mesh(lit));
    }
    y + line
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
    fn frame_pitch_is_about_a_nineteen_pixel_cap_on_the_screenshot_face() {
        let pitch = frame_pitch(321.6);
        let cap = pitch * 7.0;
        assert!((17.0..23.0).contains(&cap), "cap height {cap}");
    }
}
