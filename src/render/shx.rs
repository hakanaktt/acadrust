//! SHX and SHP shape fonts.
//!
//! These are the stroke fonts TEXT, SHAPE, and complex linetypes reference.
//! A missing file is reported by the caller. This module does not invent a
//! substitute alphabet.

use std::collections::HashMap;
use std::path::Path;

use crate::types::Vector2;

#[derive(Clone, Debug)]
pub(crate) struct ShapeFont {
    above: f64,
    shapes: HashMap<u16, ShapeDef>,
    names: HashMap<String, u16>,
}

#[derive(Clone, Debug)]
struct ShapeDef {
    codes: Vec<i16>,
}

#[derive(Clone, Copy)]
struct Pen {
    x: f64,
    y: f64,
    down: bool,
    scale: f64,
}

impl ShapeFont {
    pub(crate) fn load(path: &Path) -> Option<Self> {
        let bytes = std::fs::read(path).ok()?;
        if let Some(font) = parse_shx(&bytes) {
            return Some(font);
        }
        let text = String::from_utf8(bytes).ok()?;
        parse_shp(&text)
    }

    pub(crate) fn above(&self) -> f64 {
        if self.above > 0.0 {
            self.above
        } else {
            1.0
        }
    }

    /// Strokes in font units, and the pen advance.
    pub(crate) fn contains_number(&self, number: u16) -> bool {
        self.shapes.contains_key(&number)
    }

    pub(crate) fn glyph(&self, number: u16) -> (Vec<Vec<Vector2>>, f64) {
        let Some(shape) = self.shapes.get(&number) else {
            return (Vec::new(), self.above() * 0.6);
        };
        interpret(self, &shape.codes, 0)
    }

    pub(crate) fn glyph_for_char(&self, ch: char) -> (Vec<Vec<Vector2>>, f64) {
        self.glyph(ch as u16)
    }

    pub(crate) fn glyph_by_name(&self, name: &str) -> Option<(Vec<Vec<Vector2>>, f64)> {
        let number = self.names.get(&name.to_ascii_uppercase()).copied()?;
        Some(self.glyph(number))
    }
}

fn parse_shp(text: &str) -> Option<ShapeFont> {
    let mut shapes = HashMap::new();
    let mut names = HashMap::new();
    let mut number: Option<u16> = None;
    let mut name = String::new();
    let mut codes = Vec::new();
    let flush = |number: &mut Option<u16>,
                 name: &mut String,
                 codes: &mut Vec<i16>,
                 shapes: &mut HashMap<u16, ShapeDef>,
                 names: &mut HashMap<String, u16>| {
        let Some(number) = number.take() else {
            return;
        };
        if !name.is_empty() {
            names.insert(name.to_ascii_uppercase(), number);
        }
        name.clear();
        shapes.insert(
            number,
            ShapeDef {
                codes: std::mem::take(codes),
            },
        );
    };
    for line in text.lines() {
        let line = line.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix('*') {
            flush(&mut number, &mut name, &mut codes, &mut shapes, &mut names);
            let mut parts = rest.splitn(3, ',');
            number = parts.next().and_then(|part| part.trim().parse().ok());
            let _length = parts.next();
            name = parts.next().unwrap_or("").trim().to_string();
        } else if number.is_some() {
            codes.extend(shp_numbers(line));
        }
    }
    flush(&mut number, &mut name, &mut codes, &mut shapes, &mut names);
    if shapes.is_empty() {
        return None;
    }
    let above = shapes
        .get(&0)
        .and_then(|shape| shape.codes.first().copied())
        .unwrap_or(1)
        .max(1) as f64;
    Some(ShapeFont {
        above,
        shapes,
        names,
    })
}

fn shp_numbers(line: &str) -> Vec<i16> {
    line.replace(['(', ')'], ",")
        .split(',')
        .filter_map(|token| token.trim().parse::<i16>().ok())
        .collect()
}

fn parse_shx(data: &[u8]) -> Option<ShapeFont> {
    let end = data.iter().position(|byte| *byte == 0x1A)?;
    let header = std::str::from_utf8(&data[..end]).ok()?.to_ascii_lowercase();
    if !header.contains("shapes") && !header.contains("unifont") {
        return None;
    }
    let rest = &data[end + 1..];
    if rest.len() < 2 {
        return None;
    }
    let count = u16::from_le_bytes([rest[0], rest[1]]) as usize;
    if count == 0 || count > 100_000 {
        return None;
    }
    // AutoCAD's binary SHX format stores the shape directory as a sequence of
    // records. Each record starts with its shape number and byte length,
    // followed immediately by the record payload. The old implementation
    // first consumed all record headers as an index and then tried to read the
    // payloads, which works for neither the native Unicode SHX layout nor
    // `txt.shx` (the first payload is the font name and copyright string).
    let mut offset = 2;
    let mut shapes = HashMap::new();
    let mut names = HashMap::new();
    for _ in 0..count {
        if offset + 4 > rest.len() {
            return None;
        }
        let number = u16::from_le_bytes([rest[offset], rest[offset + 1]]);
        let length = u16::from_le_bytes([rest[offset + 2], rest[offset + 3]]) as usize;
        offset += 4;
        if offset + length > rest.len() {
            return None;
        }
        let record = &rest[offset..offset + length];
        offset += length;
        let name_end = record
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(record.len());
        let name = String::from_utf8_lossy(&record[..name_end]).into_owned();
        let codes = if name_end < record.len() {
            record[name_end + 1..]
                .iter()
                .map(|byte| *byte as i16)
                .collect()
        } else {
            Vec::new()
        };
        if !name.is_empty() {
            names.insert(name.to_ascii_uppercase(), number);
        }
        shapes.insert(number, ShapeDef { codes });
    }
    let above = shapes
        .get(&0)
        .and_then(|shape| shape.codes.first().copied())
        .unwrap_or(1)
        .max(1) as f64;
    Some(ShapeFont {
        above,
        shapes,
        names,
    })
}

fn interpret(font: &ShapeFont, codes: &[i16], depth: u32) -> (Vec<Vec<Vector2>>, f64) {
    if depth > 8 {
        return (Vec::new(), 0.0);
    }
    let mut pen = Pen {
        x: 0.0,
        y: 0.0,
        down: false,
        scale: 1.0,
    };
    let mut stack = Vec::new();
    let mut strokes = Vec::new();
    let mut current = Vec::new();
    let mut index = 0;
    while index < codes.len() {
        let code = codes[index];
        index += 1;
        match code {
            0 => break,
            1 => pen.down = true,
            2 => {
                pen.down = false;
                finish(&mut current, &mut strokes);
            }
            3 => {
                let divisor = next(codes, &mut index);
                if divisor != 0 {
                    pen.scale /= divisor as f64;
                }
            }
            4 => pen.scale *= next(codes, &mut index) as f64,
            5 => stack.push((pen.x, pen.y)),
            6 => {
                if let Some((x, y)) = stack.pop() {
                    finish(&mut current, &mut strokes);
                    pen.x = x;
                    pen.y = y;
                }
            }
            7 => {
                let number = next(codes, &mut index) as u16;
                if let Some(shape) = font.shapes.get(&number) {
                    let (nested, _) = interpret(font, &shape.codes, depth + 1);
                    for mut chain in nested {
                        for point in &mut chain {
                            *point = Vector2::new(
                                pen.x + point.x * pen.scale,
                                pen.y + point.y * pen.scale,
                            );
                        }
                        strokes.push(chain);
                    }
                }
            }
            8 => {
                let dx = disp(next(codes, &mut index)) * pen.scale;
                let dy = disp(next(codes, &mut index)) * pen.scale;
                move_pen(&mut pen, &mut current, dx, dy);
            }
            9 => loop {
                let dx = disp(next(codes, &mut index));
                let dy = disp(next(codes, &mut index));
                if dx == 0.0 && dy == 0.0 {
                    break;
                }
                let scale = pen.scale;
                move_pen(&mut pen, &mut current, dx * scale, dy * scale);
            },
            10 => {
                let radius = next(codes, &mut index) as f64 * pen.scale;
                let spec = next(codes, &mut index);
                draw_octant(&mut pen, &mut current, radius, spec);
            }
            14 => skip_command(codes, &mut index),
            code if code >= 16 => {
                let length = ((code as u16) >> 4) as f64 * pen.scale;
                let direction = (code & 0x0F) as f64;
                let angle = direction * std::f64::consts::PI / 8.0;
                move_pen(
                    &mut pen,
                    &mut current,
                    length * angle.cos(),
                    length * angle.sin(),
                );
            }
            _ => {}
        }
    }
    finish(&mut current, &mut strokes);
    (strokes, pen.x)
}

fn move_pen(pen: &mut Pen, current: &mut Vec<Vector2>, dx: f64, dy: f64) {
    let start = Vector2::new(pen.x, pen.y);
    pen.x += dx;
    pen.y += dy;
    if pen.down {
        if current.is_empty() {
            current.push(start);
        }
        current.push(Vector2::new(pen.x, pen.y));
    }
}

fn finish(current: &mut Vec<Vector2>, strokes: &mut Vec<Vec<Vector2>>) {
    if current.len() >= 2 {
        strokes.push(std::mem::take(current));
    } else {
        current.clear();
    }
}

fn next(codes: &[i16], index: &mut usize) -> i16 {
    let value = codes.get(*index).copied().unwrap_or(0);
    *index += 1;
    value
}

fn disp(value: i16) -> f64 {
    (value as u8 as i8) as f64
}

fn draw_octant(pen: &mut Pen, current: &mut Vec<Vector2>, radius: f64, spec: i16) {
    let clockwise = spec < 0;
    let spec = spec.unsigned_abs();
    let start = (spec & 0x0F) as f64 * std::f64::consts::FRAC_PI_4;
    let mut span = ((spec >> 4) & 0x0F) as f64;
    if span == 0.0 {
        span = 8.0;
    }
    let sweep = span * std::f64::consts::FRAC_PI_4 * if clockwise { -1.0 } else { 1.0 };
    let steps = span.max(1.0) as usize * 2;
    let origin_x = pen.x;
    let origin_y = pen.y;
    for step in 1..=steps {
        let angle = start + sweep * step as f64 / steps as f64;
        let x = origin_x + radius * (angle.cos() - start.cos());
        let y = origin_y + radius * (angle.sin() - start.sin());
        move_pen(pen, current, x - pen.x, y - pen.y);
    }
}

fn skip_command(codes: &[i16], index: &mut usize) {
    let Some(code) = codes.get(*index).copied() else {
        return;
    };
    *index += 1;
    let extra = match code {
        3 | 4 | 7 => 1,
        8 | 10 => 2,
        9 => {
            while *index + 1 < codes.len() {
                let dx = codes[*index];
                let dy = codes[*index + 1];
                *index += 2;
                if dx == 0 && dy == 0 {
                    break;
                }
            }
            0
        }
        _ => 0,
    };
    *index += extra;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shp_letter_advances_and_strokes() {
        let font =
            parse_shp("*0,4,TEST\n6,2,0,0\n*65,12,A\n1,8,(4,6),8,(4,-6),2,8,(2,0),0\n").unwrap();
        assert!((font.above() - 6.0).abs() < 1e-9);
        let (strokes, advance) = font.glyph_for_char('A');
        assert!((advance - 10.0).abs() < 1e-6);
        assert_eq!(strokes.len(), 1);
        assert!(strokes[0].iter().any(|point| (point.y - 6.0).abs() < 1e-6));
    }

    #[test]
    fn binary_shx_reads_sequential_shape_records() {
        let mut data = b"AutoCAD-86 unifont 1.0\r\n".to_vec();
        data.push(0x1a);
        data.extend_from_slice(&2_u16.to_le_bytes());
        // Shape 0 carries the font name in its payload.
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&4_u16.to_le_bytes());
        data.extend_from_slice(b"TXT\0");
        // Shape 65 (A): pen down, one unit to the right, end.
        data.extend_from_slice(&65_u16.to_le_bytes());
        data.extend_from_slice(&4_u16.to_le_bytes());
        data.extend_from_slice(&[0, 1, 0x10, 0]);

        let font = parse_shx(&data).expect("valid sequential SHX records");
        let (strokes, advance) = font.glyph_for_char('A');
        assert_eq!(strokes.len(), 1);
        assert!((advance - 1.0).abs() < 1e-6);
        assert_eq!(strokes[0][0], Vector2::new(0.0, 0.0));
        assert_eq!(strokes[0][1], Vector2::new(1.0, 0.0));
    }

}
