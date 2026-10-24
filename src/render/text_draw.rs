//! Place SHX strokes for a single-line label.

use crate::types::{Matrix3, Vector2, Vector3};

use super::shx::ShapeFont;

pub(crate) struct GlyphLabel<'a> {
    pub text: &'a str,
    pub shape_number: Option<u16>,
    pub insertion: Vector3,
    pub alignment: Vector3,
    pub use_alignment: bool,
    pub height: f64,
    pub rotation: f64,
    pub width_factor: f64,
    pub oblique: f64,
    pub horizontal: u8,
    pub vertical: u8,
    pub normal: Vector3,
    pub backward: bool,
    pub upside_down: bool,
}

pub(crate) fn place_label(font: &ShapeFont, label: &GlyphLabel<'_>) -> Vec<Vec<Vector3>> {
    let width_factor = if label.width_factor == 0.0 {
        1.0
    } else {
        label.width_factor
    };
    let above = font.above();
    let unit = if label.height > 0.0 {
        label.height / above
    } else {
        1.0
    };
    let mut cursor = 0.0;
    let mut local = Vec::new();
    if let Some(number) = label.shape_number {
        let (strokes, advance) = font
            .glyph_by_name(label.text)
            .filter(|_| !font.contains_number(number) && !label.text.is_empty())
            .unwrap_or_else(|| font.glyph(number));
        push_strokes(&mut local, &strokes, cursor, width_factor);
        cursor += advance * width_factor;
    } else {
        for ch in label.text.chars() {
            if ch == '\r' {
                continue;
            }
            let (strokes, advance) = font.glyph_for_char(ch);
            push_strokes(&mut local, &strokes, cursor, width_factor);
            cursor += advance * width_factor;
        }
    }
    let mut shift_x = match label.horizontal {
        1 | 4 => -cursor * 0.5,
        2 => -cursor,
        _ => 0.0,
    };
    let mut shift_y = match label.vertical {
        2 => -above * 0.5,
        3 => -above,
        _ => 0.0,
    };
    if label.horizontal == 4 {
        shift_x = -cursor * 0.5;
        shift_y = -above * 0.5;
    }
    if label.backward {
        shift_x = -shift_x;
    }
    let anchor = if label.use_alignment {
        label.alignment
    } else {
        label.insertion
    };
    let basis = Matrix3::arbitrary_axis(label.normal);
    let rotation = Matrix3::rotation_z(label.rotation);
    let oblique = label.oblique.tan();
    local
        .into_iter()
        .map(|stroke| {
            stroke
                .into_iter()
                .map(|point| {
                    let mut x = point.x + shift_x;
                    let mut y = point.y + shift_y;
                    if label.backward {
                        x = -x;
                    }
                    if label.upside_down {
                        y = -y;
                    }
                    x += y * oblique;
                    anchor + basis * (rotation * Vector3::new(x * unit, y * unit, 0.0))
                })
                .collect()
        })
        .collect()
}

fn push_strokes(
    out: &mut Vec<Vec<Vector2>>,
    strokes: &[Vec<Vector2>],
    cursor: f64,
    width_factor: f64,
) {
    for stroke in strokes {
        out.push(
            stroke
                .iter()
                .map(|point| Vector2::new(point.x * width_factor + cursor, point.y))
                .collect(),
        );
    }
}

pub(crate) fn quick_box(label: &GlyphLabel<'_>) -> Vec<Vector3> {
    let count = label.text.chars().count().max(1) as f64;
    let width = label.height * 0.6 * count * label.width_factor.abs().max(1.0);
    let height = label.height;
    let local = [
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(width, 0.0, 0.0),
        Vector3::new(width, height, 0.0),
        Vector3::new(0.0, height, 0.0),
        Vector3::new(0.0, 0.0, 0.0),
    ];
    let basis = Matrix3::arbitrary_axis(label.normal);
    let rotation = Matrix3::rotation_z(label.rotation);
    let anchor = if label.use_alignment {
        label.alignment
    } else {
        label.insertion
    };
    local
        .into_iter()
        .map(|point| anchor + basis * (rotation * point))
        .collect()
}
