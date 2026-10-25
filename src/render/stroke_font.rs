//! Acadrust Stroke, an original single-stroke font for the renderer.
//!
//! Cap height is 72 font units. Descenders reach about 20 units below the
//! baseline, and rounds overshoot the cap and baseline by about 2. Digits
//! share one advance so columns of dimensions line up. The bytes are AutoCAD
//! shapes 1.0. The lettering is not derived from an Autodesk typeface.

use std::sync::OnceLock;

use super::shx::{parse_shx, ShapeFont};

const ABOVE: u8 = 72;
const BELOW: u8 = 20;

/// Compiled `acadrust.shx` bytes.
///
/// The renderer draws text with this font when a style's SHX or TrueType file
/// is not available. The same bytes can be written out for another program.
/// Glyph ids and `data-advance` widths are listed in the
/// [renderer advance table](crate::render#stroke-font-advances).
pub fn acadrust_shx() -> &'static [u8] {
    static BYTES: OnceLock<Vec<u8>> = OnceLock::new();
    BYTES.get_or_init(encode_shx)
}

/// Shape-source text for [`acadrust_shx`].
///
/// The [renderer advance table](crate::render#stroke-font-advances) matches
/// each character to its SHP name and `data-advance` width.
pub fn acadrust_shp() -> &'static str {
    static TEXT: OnceLock<String> = OnceLock::new();
    TEXT.get_or_init(encode_shp)
}

pub(crate) fn font() -> ShapeFont {
    static FONT: OnceLock<ShapeFont> = OnceLock::new();
    FONT.get_or_init(|| parse_shx(acadrust_shx()).expect("acadrust.shx"))
        .clone()
}

#[derive(Clone)]
struct Glyph {
    number: u16,
    name: String,
    advance: i32,
    strokes: Vec<Vec<(i32, i32)>>,
}

struct Pen {
    strokes: Vec<Vec<(i32, i32)>>,
    cur: Vec<(i32, i32)>,
}

impl Pen {
    fn new() -> Self {
        Self {
            strokes: Vec::new(),
            cur: Vec::new(),
        }
    }

    fn mv(&mut self, x: f64, y: f64) {
        self.break_stroke();
        self.cur.push(pt(x, y));
    }

    fn ln(&mut self, x: f64, y: f64) {
        let point = pt(x, y);
        if self.cur.last() != Some(&point) {
            self.cur.push(point);
        }
    }

    /// Elliptic arc. Angles are degrees, 0 at the right, counter-clockwise.
    /// `a1 < a0` sweeps clockwise.
    fn earc(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, a0: f64, a1: f64) {
        let sweep = a1 - a0;
        if sweep.abs() < 1e-6 || rx.abs() < 1e-6 || ry.abs() < 1e-6 {
            return;
        }
        let steps = (sweep.abs() / 7.0).ceil().max(2.0) as usize;
        for step in 0..=steps {
            let t = step as f64 / steps as f64;
            let angle = (a0 + sweep * t).to_radians();
            self.ln(cx + rx * angle.cos(), cy + ry * angle.sin());
        }
    }

    /// Smooth curve through `points`, continuing the current stroke when one exists.
    fn curve(&mut self, points: &[(f64, f64)]) {
        if points.is_empty() {
            return;
        }
        if self.cur.is_empty() {
            self.mv(points[0].0, points[0].1);
        } else {
            self.ln(points[0].0, points[0].1);
        }
        for index in 0..points.len().saturating_sub(1) {
            let p0 = points[index.saturating_sub(1)];
            let p1 = points[index];
            let p2 = points[index + 1];
            let p3 = points[(index + 2).min(points.len() - 1)];
            self.bez(
                p1.0 + (p2.0 - p0.0) / 6.0,
                p1.1 + (p2.1 - p0.1) / 6.0,
                p2.0 - (p3.0 - p1.0) / 6.0,
                p2.1 - (p3.1 - p1.1) / 6.0,
                p2.0,
                p2.1,
            );
        }
    }

    fn bez(&mut self, x1: f64, y1: f64, x2: f64, y2: f64, x3: f64, y3: f64) {
        let Some(&(x0, y0)) = self.cur.last() else {
            self.ln(x3, y3);
            return;
        };
        let (x0, y0) = (x0 as f64, y0 as f64);
        for step in 1..=14 {
            let t = step as f64 / 14.0;
            let u = 1.0 - t;
            let x = u * u * u * x0 + 3.0 * u * u * t * x1 + 3.0 * u * t * t * x2 + t * t * t * x3;
            let y = u * u * u * y0 + 3.0 * u * u * t * y1 + 3.0 * u * t * t * y2 + t * t * t * y3;
            self.ln(x, y);
        }
    }

    /// Tight spiral. At drawing size the lineweight closes it into a dot.
    fn blob(&mut self, cx: f64, cy: f64, r: f64) {
        self.break_stroke();
        self.cur.push(pt(cx, cy));
        for step in 1..=20 {
            let t = step as f64 / 20.0;
            let angle = t * std::f64::consts::TAU * 1.65;
            self.ln(cx + r * t * angle.cos(), cy + r * t * angle.sin());
        }
    }

    fn break_stroke(&mut self) {
        if self.cur.len() >= 2 {
            self.strokes.push(std::mem::take(&mut self.cur));
        } else {
            self.cur.clear();
        }
    }

    fn finish(mut self) -> Vec<Vec<(i32, i32)>> {
        self.break_stroke();
        self.strokes
    }
}

fn pt(x: f64, y: f64) -> (i32, i32) {
    (x.round() as i32, y.round() as i32)
}

/// Capital S. The upper bowl runs clockwise and the lower bowl counter-clockwise,
/// with a short curve between them so the spine does not kink.
fn draw_capital_s(pen: &mut Pen) {
    pen.earc(32.0, 54.0, 17.0, 20.0, 150.0, -18.0);
    let lower = 145.0_f64.to_radians();
    pen.bez(
        46.1,
        40.1,
        21.5,
        36.2,
        32.0 + 18.0 * lower.cos(),
        18.0 + 20.0 * lower.sin(),
    );
    // End on the lower-right terminal. A sweep past 360 climbs the right side
    // and closes the lower counter.
    pen.earc(32.0, 18.0, 18.0, 20.0, 145.0, 328.0);
}

fn glyph(ch: char, advance: i32, pen: Pen) -> Glyph {
    Glyph {
        number: ch as u16,
        name: shape_name(ch),
        advance,
        strokes: pen.finish(),
    }
}

fn shape_name(ch: char) -> String {
    if ch.is_ascii_uppercase() || ch.is_ascii_digit() {
        ch.to_string()
    } else {
        format!("U{:04X}", u32::from(ch))
    }
}

fn library() -> Vec<Glyph> {
    let mut glyphs = bases();
    glyphs.extend(accented(&glyphs));
    glyphs.extend(scaled_figures(&glyphs));
    glyphs
}

fn bases() -> Vec<Glyph> {
    let mut pen = Pen::new();
    let space = glyph(' ', 30, pen);

    pen = Pen::new();
    pen.mv(8.0, 0.0);
    pen.ln(32.0, 72.0);
    pen.mv(56.0, 0.0);
    pen.ln(32.0, 72.0);
    pen.mv(8.0 + 24.0 * 28.0 / 72.0, 28.0);
    pen.ln(56.0 - 24.0 * 28.0 / 72.0, 28.0);
    let upper_a = glyph('A', 64, pen);

    pen = Pen::new();
    pen.mv(10.0, 0.0);
    pen.ln(10.0, 72.0);
    pen.mv(10.0, 72.0);
    pen.ln(26.0, 72.0);
    pen.earc(26.0, 54.0, 16.0, 18.0, 90.0, -90.0);
    pen.ln(10.0, 36.0);
    pen.mv(10.0, 36.0);
    pen.ln(28.0, 36.0);
    pen.earc(28.0, 18.0, 18.0, 18.0, 90.0, -90.0);
    pen.ln(10.0, 0.0);
    let upper_b = glyph('B', 56, pen);

    pen = Pen::new();
    pen.earc(34.0, 36.0, 24.0, 38.0, 55.0, 305.0);
    let upper_c = glyph('C', 66, pen);

    pen = Pen::new();
    pen.mv(10.0, 0.0);
    pen.ln(10.0, 72.0);
    pen.mv(10.0, 72.0);
    pen.ln(30.0, 72.0);
    pen.earc(30.0, 36.0, 22.0, 36.0, 90.0, -90.0);
    pen.ln(10.0, 0.0);
    let upper_d = glyph('D', 62, pen);

    pen = Pen::new();
    pen.mv(10.0, 0.0);
    pen.ln(10.0, 72.0);
    pen.mv(10.0, 72.0);
    pen.ln(44.0, 72.0);
    pen.mv(10.0, 36.0);
    pen.ln(34.0, 36.0);
    pen.mv(10.0, 0.0);
    pen.ln(44.0, 0.0);
    let upper_e = glyph('E', 52, pen);

    pen = Pen::new();
    pen.mv(10.0, 0.0);
    pen.ln(10.0, 72.0);
    pen.mv(10.0, 72.0);
    pen.ln(44.0, 72.0);
    pen.mv(10.0, 36.0);
    pen.ln(34.0, 36.0);
    let upper_f = glyph('F', 50, pen);

    pen = Pen::new();
    pen.earc(34.0, 36.0, 24.0, 38.0, 50.0, 354.0);
    pen.ln(36.0, 32.0);
    let upper_g = glyph('G', 68, pen);

    pen = Pen::new();
    pen.mv(10.0, 0.0);
    pen.ln(10.0, 72.0);
    pen.mv(50.0, 0.0);
    pen.ln(50.0, 72.0);
    pen.mv(10.0, 34.0);
    pen.ln(50.0, 34.0);
    let upper_h = glyph('H', 60, pen);

    pen = Pen::new();
    pen.mv(8.0, 72.0);
    pen.ln(24.0, 72.0);
    pen.mv(16.0, 72.0);
    pen.ln(16.0, 0.0);
    pen.mv(8.0, 0.0);
    pen.ln(24.0, 0.0);
    let upper_i = glyph('I', 32, pen);

    pen = Pen::new();
    pen.mv(20.0, 72.0);
    pen.ln(42.0, 72.0);
    pen.mv(36.0, 72.0);
    pen.ln(36.0, 22.0);
    pen.earc(22.0, 22.0, 14.0, 24.0, 0.0, -180.0);
    let upper_j = glyph('J', 50, pen);

    pen = Pen::new();
    pen.mv(10.0, 0.0);
    pen.ln(10.0, 72.0);
    pen.mv(46.0, 72.0);
    pen.ln(10.0, 32.0);
    pen.ln(48.0, 0.0);
    let upper_k = glyph('K', 58, pen);

    pen = Pen::new();
    pen.mv(10.0, 72.0);
    pen.ln(10.0, 0.0);
    pen.ln(42.0, 0.0);
    let upper_l = glyph('L', 50, pen);

    pen = Pen::new();
    pen.mv(8.0, 0.0);
    pen.ln(8.0, 72.0);
    pen.ln(36.0, 0.0);
    pen.ln(64.0, 72.0);
    pen.ln(64.0, 0.0);
    let upper_m = glyph('M', 72, pen);

    pen = Pen::new();
    pen.mv(10.0, 0.0);
    pen.ln(10.0, 72.0);
    pen.ln(50.0, 0.0);
    pen.ln(50.0, 72.0);
    let upper_n = glyph('N', 60, pen);

    pen = Pen::new();
    pen.earc(34.0, 36.0, 24.0, 38.0, 0.0, 360.0);
    let upper_o = glyph('O', 68, pen);

    pen = Pen::new();
    pen.mv(10.0, 0.0);
    pen.ln(10.0, 72.0);
    pen.mv(10.0, 72.0);
    pen.ln(26.0, 72.0);
    pen.earc(26.0, 54.0, 16.0, 18.0, 90.0, -90.0);
    pen.ln(10.0, 36.0);
    let upper_p = glyph('P', 52, pen);

    pen = Pen::new();
    pen.earc(34.0, 36.0, 24.0, 38.0, 0.0, 360.0);
    pen.mv(42.0, 20.0);
    pen.ln(62.0, -4.0);
    let upper_q = glyph('Q', 70, pen);

    pen = Pen::new();
    pen.mv(10.0, 0.0);
    pen.ln(10.0, 72.0);
    pen.mv(10.0, 72.0);
    pen.ln(26.0, 72.0);
    pen.earc(26.0, 54.0, 16.0, 18.0, 90.0, -90.0);
    pen.ln(10.0, 36.0);
    pen.mv(24.0, 36.0);
    pen.ln(50.0, 0.0);
    let upper_r = glyph('R', 58, pen);

    pen = Pen::new();
    draw_capital_s(&mut pen);
    let upper_s = glyph('S', 60, pen);

    pen = Pen::new();
    pen.mv(6.0, 72.0);
    pen.ln(50.0, 72.0);
    pen.mv(28.0, 72.0);
    pen.ln(28.0, 0.0);
    let upper_t = glyph('T', 56, pen);

    pen = Pen::new();
    pen.mv(12.0, 72.0);
    pen.ln(12.0, 30.0);
    pen.earc(32.0, 30.0, 20.0, 32.0, 180.0, 360.0);
    pen.ln(52.0, 72.0);
    let upper_u = glyph('U', 64, pen);

    pen = Pen::new();
    pen.mv(8.0, 72.0);
    pen.ln(30.0, 0.0);
    pen.ln(52.0, 72.0);
    let upper_v = glyph('V', 60, pen);

    pen = Pen::new();
    pen.mv(4.0, 72.0);
    pen.ln(20.0, 0.0);
    pen.ln(36.0, 72.0);
    pen.ln(52.0, 0.0);
    pen.ln(68.0, 72.0);
    let upper_w = glyph('W', 76, pen);

    pen = Pen::new();
    pen.mv(8.0, 0.0);
    pen.ln(48.0, 72.0);
    pen.mv(8.0, 72.0);
    pen.ln(48.0, 0.0);
    let upper_x = glyph('X', 56, pen);

    pen = Pen::new();
    pen.mv(8.0, 72.0);
    pen.ln(28.0, 34.0);
    pen.ln(48.0, 72.0);
    pen.mv(28.0, 34.0);
    pen.ln(28.0, 0.0);
    let upper_y = glyph('Y', 56, pen);

    pen = Pen::new();
    pen.mv(8.0, 72.0);
    pen.ln(48.0, 72.0);
    pen.ln(8.0, 0.0);
    pen.ln(48.0, 0.0);
    let upper_z = glyph('Z', 56, pen);

    // Single-story geometric lowercase, the technical-lettering form.
    pen = Pen::new();
    pen.earc(24.0, 20.0, 16.0, 20.0, 0.0, 360.0);
    pen.mv(40.0, 0.0);
    pen.ln(40.0, 52.0);
    let lower_a = glyph('a', 50, pen);

    pen = Pen::new();
    pen.mv(10.0, 0.0);
    pen.ln(10.0, 72.0);
    pen.mv(10.0, 50.0);
    pen.ln(28.0, 50.0);
    pen.earc(28.0, 25.0, 18.0, 25.0, 90.0, -90.0);
    pen.ln(10.0, 0.0);
    let lower_b = glyph('b', 56, pen);

    pen = Pen::new();
    pen.earc(28.0, 26.0, 18.0, 27.0, 50.0, 310.0);
    let lower_c = glyph('c', 52, pen);

    pen = Pen::new();
    pen.mv(46.0, 0.0);
    pen.ln(46.0, 72.0);
    pen.mv(46.0, 50.0);
    pen.ln(28.0, 50.0);
    pen.earc(28.0, 25.0, 18.0, 25.0, 90.0, 270.0);
    pen.ln(46.0, 0.0);
    let lower_d = glyph('d', 56, pen);

    pen = Pen::new();
    // Open bowl in the same family as c. The bar crosses the counter and
    // stops in the right-hand opening.
    pen.earc(28.0, 26.0, 18.0, 27.0, 48.0, 312.0);
    pen.mv(10.2, 30.0);
    pen.ln(40.0, 30.0);
    let lower_e = glyph('e', 54, pen);

    pen = Pen::new();
    pen.mv(20.0, 0.0);
    pen.ln(20.0, 56.0);
    pen.bez(20.0, 76.0, 42.0, 76.0, 40.0, 56.0);
    pen.mv(8.0, 38.0);
    pen.ln(34.0, 38.0);
    let lower_f = glyph('f', 46, pen);

    pen = Pen::new();
    pen.earc(24.0, 22.0, 16.0, 20.0, 0.0, 360.0);
    pen.mv(40.0, 52.0);
    pen.ln(40.0, -2.0);
    pen.earc(28.0, -2.0, 12.0, 16.0, 0.0, -180.0);
    let lower_g = glyph('g', 50, pen);

    pen = Pen::new();
    pen.mv(10.0, 0.0);
    pen.ln(10.0, 72.0);
    pen.mv(10.0, 52.0);
    pen.bez(34.0, 52.0, 46.0, 34.0, 46.0, 0.0);
    let lower_h = glyph('h', 56, pen);

    pen = Pen::new();
    pen.mv(14.0, 0.0);
    pen.ln(14.0, 50.0);
    pen.blob(14.0, 66.0, 4.2);
    let lower_i = glyph('i', 28, pen);

    pen = Pen::new();
    pen.blob(18.0, 66.0, 4.2);
    pen.mv(18.0, 50.0);
    pen.ln(18.0, -2.0);
    pen.earc(10.0, -2.0, 8.0, 14.0, 0.0, -180.0);
    let lower_j = glyph('j', 28, pen);

    pen = Pen::new();
    pen.mv(10.0, 0.0);
    pen.ln(10.0, 72.0);
    pen.mv(40.0, 50.0);
    pen.ln(10.0, 24.0);
    pen.ln(42.0, 0.0);
    let lower_k = glyph('k', 52, pen);

    pen = Pen::new();
    pen.mv(14.0, 0.0);
    pen.ln(14.0, 72.0);
    let lower_l = glyph('l', 28, pen);

    pen = Pen::new();
    pen.mv(8.0, 0.0);
    pen.ln(8.0, 52.0);
    pen.bez(24.0, 52.0, 32.0, 32.0, 32.0, 0.0);
    pen.mv(32.0, 0.0);
    pen.ln(32.0, 52.0);
    pen.bez(48.0, 52.0, 58.0, 32.0, 58.0, 0.0);
    let lower_m = glyph('m', 68, pen);

    pen = Pen::new();
    pen.mv(10.0, 0.0);
    pen.ln(10.0, 52.0);
    pen.bez(34.0, 52.0, 46.0, 34.0, 46.0, 0.0);
    let lower_n = glyph('n', 56, pen);

    pen = Pen::new();
    pen.earc(28.0, 26.0, 18.0, 27.0, 0.0, 360.0);
    let lower_o = glyph('o', 56, pen);

    pen = Pen::new();
    pen.mv(10.0, -18.0);
    pen.ln(10.0, 50.0);
    pen.ln(28.0, 50.0);
    pen.earc(28.0, 25.0, 18.0, 25.0, 90.0, -90.0);
    pen.ln(10.0, 0.0);
    let lower_p = glyph('p', 56, pen);

    pen = Pen::new();
    pen.mv(46.0, -18.0);
    pen.ln(46.0, 50.0);
    pen.ln(28.0, 50.0);
    pen.earc(28.0, 25.0, 18.0, 25.0, 90.0, 270.0);
    pen.ln(46.0, 0.0);
    let lower_q = glyph('q', 56, pen);

    pen = Pen::new();
    pen.mv(10.0, 0.0);
    pen.ln(10.0, 50.0);
    pen.mv(10.0, 30.0);
    pen.bez(10.0, 52.0, 36.0, 54.0, 36.0, 40.0);
    let lower_r = glyph('r', 42, pen);

    pen = Pen::new();
    pen.curve(&[
        (16.0, 36.0),
        (10.0, 44.0),
        (14.0, 52.0),
        (28.0, 56.0),
        (40.0, 50.0),
        (42.0, 40.0),
        (34.0, 32.0),
        (20.0, 26.0),
        (10.0, 18.0),
        (8.0, 8.0),
        (16.0, 0.0),
        (30.0, -2.0),
        (42.0, 6.0),
        (40.0, 16.0),
    ]);
    let lower_s = glyph('s', 50, pen);

    pen = Pen::new();
    pen.mv(18.0, 64.0);
    pen.ln(18.0, 14.0);
    pen.bez(18.0, 0.0, 38.0, 0.0, 36.0, 12.0);
    pen.mv(8.0, 46.0);
    pen.ln(32.0, 46.0);
    let lower_t = glyph('t', 42, pen);

    pen = Pen::new();
    pen.mv(10.0, 52.0);
    pen.ln(10.0, 22.0);
    pen.bez(10.0, -6.0, 46.0, -6.0, 46.0, 22.0);
    pen.ln(46.0, 52.0);
    let lower_u = glyph('u', 56, pen);

    pen = Pen::new();
    pen.mv(8.0, 50.0);
    pen.ln(26.0, 0.0);
    pen.ln(44.0, 50.0);
    let lower_v = glyph('v', 52, pen);

    pen = Pen::new();
    pen.mv(4.0, 50.0);
    pen.ln(16.0, 0.0);
    pen.ln(30.0, 50.0);
    pen.ln(44.0, 0.0);
    pen.ln(56.0, 50.0);
    let lower_w = glyph('w', 64, pen);

    pen = Pen::new();
    pen.mv(8.0, 0.0);
    pen.ln(40.0, 50.0);
    pen.mv(8.0, 50.0);
    pen.ln(40.0, 0.0);
    let lower_x = glyph('x', 48, pen);

    pen = Pen::new();
    pen.mv(8.0, 50.0);
    pen.ln(26.0, 8.0);
    pen.mv(44.0, 50.0);
    pen.ln(26.0, 8.0);
    pen.ln(12.0, -18.0);
    let lower_y = glyph('y', 52, pen);

    pen = Pen::new();
    pen.mv(8.0, 50.0);
    pen.ln(40.0, 50.0);
    pen.ln(8.0, 0.0);
    pen.ln(40.0, 0.0);
    let lower_z = glyph('z', 48, pen);

    let digits = digits();
    let punct = punctuation();
    let symbols = symbols();

    let mut glyphs = vec![
        space, upper_a, upper_b, upper_c, upper_d, upper_e, upper_f, upper_g, upper_h, upper_i,
        upper_j, upper_k, upper_l, upper_m, upper_n, upper_o, upper_p, upper_q, upper_r, upper_s,
        upper_t, upper_u, upper_v, upper_w, upper_x, upper_y, upper_z, lower_a, lower_b, lower_c,
        lower_d, lower_e, lower_f, lower_g, lower_h, lower_i, lower_j, lower_k, lower_l, lower_m,
        lower_n, lower_o, lower_p, lower_q, lower_r, lower_s, lower_t, lower_u, lower_v, lower_w,
        lower_x, lower_y, lower_z,
    ];
    glyphs.extend(digits);
    glyphs.extend(punct);
    glyphs.extend(symbols);
    glyphs
}

fn digits() -> Vec<Glyph> {
    // Tabular figures, advance 52, body centered in the cell.
    let mut pen = Pen::new();
    pen.mv(12.0, 18.0);
    pen.ln(12.0, 54.0);
    pen.earc(26.0, 54.0, 14.0, 18.0, 180.0, 0.0);
    pen.ln(40.0, 18.0);
    pen.earc(26.0, 18.0, 14.0, 20.0, 0.0, -180.0);
    let zero = glyph('0', 52, pen);

    pen = Pen::new();
    pen.mv(16.0, 54.0);
    pen.ln(28.0, 72.0);
    pen.ln(28.0, 0.0);
    pen.mv(16.0, 0.0);
    pen.ln(40.0, 0.0);
    let one = glyph('1', 52, pen);

    pen = Pen::new();
    pen.curve(&[
        (14.0, 48.0),
        (12.0, 62.0),
        (22.0, 74.0),
        (36.0, 74.0),
        (46.0, 64.0),
        (42.0, 50.0),
        (24.0, 22.0),
        (12.0, 2.0),
    ]);
    pen.ln(44.0, 0.0);
    let two = glyph('2', 52, pen);

    pen = Pen::new();
    pen.curve(&[
        (12.0, 60.0),
        (18.0, 72.0),
        (34.0, 74.0),
        (46.0, 64.0),
        (44.0, 50.0),
        (32.0, 40.0),
        (22.0, 36.0),
        (34.0, 32.0),
        (46.0, 22.0),
        (46.0, 10.0),
        (34.0, 0.0),
        (18.0, 0.0),
        (12.0, 12.0),
    ]);
    let three = glyph('3', 52, pen);

    pen = Pen::new();
    pen.mv(34.0, 0.0);
    pen.ln(34.0, 72.0);
    pen.mv(34.0, 72.0);
    pen.ln(10.0, 26.0);
    pen.ln(44.0, 26.0);
    let four = glyph('4', 52, pen);

    pen = Pen::new();
    pen.mv(42.0, 72.0);
    pen.ln(14.0, 72.0);
    pen.ln(14.0, 38.0);
    pen.curve(&[
        (14.0, 38.0),
        (30.0, 46.0),
        (46.0, 36.0),
        (46.0, 18.0),
        (34.0, 4.0),
        (18.0, 4.0),
        (12.0, 14.0),
    ]);
    let five = glyph('5', 52, pen);

    pen = Pen::new();
    pen.curve(&[
        (40.0, 64.0),
        (28.0, 76.0),
        (14.0, 66.0),
        (10.0, 48.0),
        (12.0, 28.0),
        (20.0, 10.0),
        (34.0, 4.0),
        (44.0, 16.0),
        (42.0, 30.0),
        (30.0, 34.0),
        (18.0, 26.0),
    ]);
    let six = glyph('6', 52, pen);

    pen = Pen::new();
    pen.mv(12.0, 72.0);
    pen.ln(42.0, 72.0);
    pen.ln(18.0, 0.0);
    pen.mv(18.0, 40.0);
    pen.ln(34.0, 40.0);
    let seven = glyph('7', 52, pen);

    pen = Pen::new();
    pen.earc(26.0, 54.0, 14.0, 18.0, 0.0, 360.0);
    pen.earc(26.0, 20.0, 16.0, 20.0, 0.0, 360.0);
    let eight = glyph('8', 52, pen);

    pen = Pen::new();
    pen.curve(&[
        (12.0, 8.0),
        (24.0, -4.0),
        (38.0, 6.0),
        (42.0, 24.0),
        (40.0, 44.0),
        (32.0, 62.0),
        (18.0, 68.0),
        (8.0, 56.0),
        (10.0, 42.0),
        (22.0, 38.0),
        (34.0, 46.0),
    ]);
    let nine = glyph('9', 52, pen);

    vec![zero, one, two, three, four, five, six, seven, eight, nine]
}

fn punctuation() -> Vec<Glyph> {
    let mut out = Vec::new();
    let mut pen = Pen::new();
    pen.blob(8.0, 6.0, 4.0);
    out.push(glyph('.', 20, pen));

    pen = Pen::new();
    pen.blob(10.0, 8.0, 4.0);
    pen.mv(10.0, 6.0);
    pen.bez(10.0, -2.0, 2.0, -14.0, 2.0, -16.0);
    out.push(glyph(',', 20, pen));

    pen = Pen::new();
    pen.blob(8.0, 14.0, 3.6);
    pen.blob(8.0, 36.0, 3.6);
    out.push(glyph(':', 20, pen));

    pen = Pen::new();
    pen.blob(10.0, 36.0, 3.6);
    pen.blob(10.0, 10.0, 4.0);
    pen.mv(10.0, 8.0);
    pen.bez(10.0, 0.0, 2.0, -12.0, 2.0, -14.0);
    out.push(glyph(';', 20, pen));

    pen = Pen::new();
    pen.mv(12.0, 24.0);
    pen.ln(12.0, 72.0);
    pen.blob(12.0, 8.0, 4.0);
    out.push(glyph('!', 24, pen));

    pen = Pen::new();
    pen.earc(18.0, 54.0, 12.0, 16.0, 180.0, -90.0);
    pen.ln(18.0, 24.0);
    pen.blob(18.0, 8.0, 4.0);
    out.push(glyph('?', 36, pen));

    pen = Pen::new();
    pen.blob(12.0, 64.0, 4.0);
    pen.mv(12.0, 0.0);
    pen.ln(12.0, 48.0);
    out.push(glyph('¡', 24, pen));

    pen = Pen::new();
    pen.blob(18.0, 64.0, 4.0);
    pen.mv(18.0, 48.0);
    pen.ln(18.0, 28.0);
    pen.earc(18.0, 18.0, 12.0, 16.0, 90.0, 360.0);
    out.push(glyph('¿', 36, pen));

    pen = Pen::new();
    pen.mv(4.0, 34.0);
    pen.ln(28.0, 34.0);
    out.push(glyph('-', 32, pen));

    pen = Pen::new();
    pen.mv(2.0, -8.0);
    pen.ln(46.0, -8.0);
    out.push(glyph('_', 48, pen));

    pen = Pen::new();
    pen.mv(6.0, 46.0);
    pen.ln(6.0, 72.0);
    pen.mv(16.0, 46.0);
    pen.ln(16.0, 72.0);
    out.push(glyph('"', 24, pen));

    pen = Pen::new();
    pen.mv(8.0, 46.0);
    pen.ln(8.0, 72.0);
    out.push(glyph('\'', 16, pen));

    pen = Pen::new();
    pen.mv(14.0, 52.0);
    pen.ln(6.0, 72.0);
    out.push(glyph('`', 20, pen));

    pen = Pen::new();
    pen.mv(8.0, 36.0);
    pen.ln(22.0, 56.0);
    pen.ln(36.0, 36.0);
    out.push(glyph('^', 44, pen));

    pen = Pen::new();
    pen.mv(6.0, 40.0);
    pen.bez(14.0, 52.0, 24.0, 28.0, 36.0, 40.0);
    out.push(glyph('~', 42, pen));

    pen = Pen::new();
    pen.mv(14.0, 48.0);
    pen.ln(6.0, 72.0);
    pen.mv(26.0, 48.0);
    pen.ln(18.0, 72.0);
    out.push(glyph('“', 32, pen));

    pen = Pen::new();
    pen.mv(8.0, 48.0);
    pen.ln(16.0, 72.0);
    pen.mv(20.0, 48.0);
    pen.ln(28.0, 72.0);
    out.push(glyph('”', 34, pen));

    pen = Pen::new();
    pen.mv(10.0, 48.0);
    pen.ln(4.0, 72.0);
    out.push(glyph('‘', 16, pen));

    pen = Pen::new();
    pen.mv(4.0, 48.0);
    pen.ln(12.0, 72.0);
    out.push(glyph('’', 16, pen));

    pen = Pen::new();
    pen.earc(22.0, 36.0, 16.0, 40.0, 115.0, 245.0);
    out.push(glyph('(', 28, pen));

    pen = Pen::new();
    pen.earc(6.0, 36.0, 16.0, 40.0, -65.0, 65.0);
    out.push(glyph(')', 28, pen));

    pen = Pen::new();
    pen.mv(18.0, -2.0);
    pen.ln(8.0, -2.0);
    pen.ln(8.0, 74.0);
    pen.ln(18.0, 74.0);
    out.push(glyph('[', 26, pen));

    pen = Pen::new();
    pen.mv(8.0, -2.0);
    pen.ln(18.0, -2.0);
    pen.ln(18.0, 74.0);
    pen.ln(8.0, 74.0);
    out.push(glyph(']', 26, pen));

    pen = Pen::new();
    pen.mv(20.0, 74.0);
    pen.bez(8.0, 74.0, 10.0, 52.0, 16.0, 42.0);
    pen.ln(8.0, 36.0);
    pen.ln(16.0, 30.0);
    pen.bez(10.0, 20.0, 8.0, -2.0, 20.0, -2.0);
    out.push(glyph('{', 30, pen));

    pen = Pen::new();
    pen.mv(8.0, 74.0);
    pen.bez(20.0, 74.0, 18.0, 52.0, 12.0, 42.0);
    pen.ln(20.0, 36.0);
    pen.ln(12.0, 30.0);
    pen.bez(18.0, 20.0, 20.0, -2.0, 8.0, -2.0);
    out.push(glyph('}', 30, pen));

    pen = Pen::new();
    pen.mv(8.0, 0.0);
    pen.ln(28.0, 72.0);
    out.push(glyph('/', 36, pen));

    pen = Pen::new();
    pen.mv(28.0, 0.0);
    pen.ln(8.0, 72.0);
    out.push(glyph('\\', 36, pen));

    pen = Pen::new();
    pen.mv(8.0, -2.0);
    pen.ln(8.0, 74.0);
    out.push(glyph('|', 16, pen));

    pen = Pen::new();
    pen.mv(8.0, 18.0);
    pen.ln(36.0, 18.0);
    pen.mv(8.0, 34.0);
    pen.ln(36.0, 34.0);
    out.push(glyph('=', 44, pen));

    pen = Pen::new();
    pen.mv(8.0, 36.0);
    pen.ln(36.0, 36.0);
    pen.mv(22.0, 22.0);
    pen.ln(22.0, 50.0);
    out.push(glyph('+', 44, pen));

    pen = Pen::new();
    pen.mv(14.0, 22.0);
    pen.ln(30.0, 50.0);
    out.push(glyph('<', 44, pen));

    pen = Pen::new();
    pen.mv(14.0, 50.0);
    pen.ln(30.0, 22.0);
    out.push(glyph('>', 44, pen));

    pen = Pen::new();
    pen.mv(6.0, 62.0);
    pen.ln(22.0, 46.0);
    pen.ln(6.0, 30.0);
    pen.mv(22.0, 46.0);
    pen.ln(38.0, 62.0);
    pen.ln(22.0, 46.0);
    pen.ln(38.0, 30.0);
    out.push(glyph('*', 44, pen));

    pen = Pen::new();
    pen.earc(16.0, 54.0, 10.0, 12.0, 0.0, 360.0);
    pen.earc(32.0, 16.0, 10.0, 12.0, 0.0, 360.0);
    pen.mv(40.0, 68.0);
    pen.ln(10.0, 4.0);
    out.push(glyph('%', 52, pen));

    pen = Pen::new();
    pen.earc(28.0, 36.0, 22.0, 26.0, 30.0, 330.0);
    pen.earc(26.0, 34.0, 8.0, 10.0, 300.0, 620.0);
    pen.mv(34.0, 26.0);
    pen.ln(34.0, 46.0);
    out.push(glyph('@', 60, pen));

    pen = Pen::new();
    pen.curve(&[
        (40.0, 62.0),
        (34.0, 74.0),
        (18.0, 72.0),
        (12.0, 58.0),
        (18.0, 46.0),
        (32.0, 40.0),
        (20.0, 30.0),
        (12.0, 18.0),
        (16.0, 6.0),
        (30.0, 2.0),
        (42.0, 12.0),
        (36.0, 24.0),
    ]);
    pen.mv(24.0, 36.0);
    pen.ln(50.0, 0.0);
    out.push(glyph('&', 56, pen));

    pen = Pen::new();
    pen.mv(16.0, 0.0);
    pen.ln(22.0, 72.0);
    pen.mv(30.0, 0.0);
    pen.ln(36.0, 72.0);
    pen.mv(10.0, 26.0);
    pen.ln(42.0, 26.0);
    pen.mv(12.0, 48.0);
    pen.ln(44.0, 48.0);
    out.push(glyph('#', 52, pen));

    pen = Pen::new();
    draw_capital_s(&mut pen);
    pen.mv(32.0, -6.0);
    pen.ln(32.0, 82.0);
    out.push(glyph('$', 60, pen));

    out
}

fn symbols() -> Vec<Glyph> {
    let mut out = Vec::new();
    let mut pen = Pen::new();

    pen.earc(12.0, 60.0, 8.0, 8.0, 0.0, 360.0);
    out.push(glyph('°', 24, pen));

    pen = Pen::new();
    pen.mv(8.0, 52.0);
    pen.ln(40.0, 52.0);
    pen.mv(24.0, 38.0);
    pen.ln(24.0, 66.0);
    pen.mv(8.0, 18.0);
    pen.ln(40.0, 18.0);
    out.push(glyph('±', 48, pen));

    pen = Pen::new();
    pen.earc(26.0, 36.0, 18.0, 18.0, 0.0, 360.0);
    pen.mv(10.0, 12.0);
    pen.ln(42.0, 60.0);
    let diameter = glyph('⌀', 52, pen);
    out.push(diameter.clone());
    out.push(Glyph {
        number: '∅' as u16,
        name: shape_name('∅'),
        ..diameter
    });

    pen = Pen::new();
    pen.mv(10.0, 14.0);
    pen.ln(38.0, 58.0);
    pen.mv(10.0, 58.0);
    pen.ln(38.0, 14.0);
    out.push(glyph('×', 48, pen));

    pen = Pen::new();
    pen.mv(8.0, 36.0);
    pen.ln(40.0, 36.0);
    pen.blob(24.0, 54.0, 3.4);
    pen.blob(24.0, 18.0, 3.4);
    out.push(glyph('÷', 48, pen));

    pen = Pen::new();
    pen.mv(10.0, 50.0);
    pen.ln(10.0, 24.0);
    pen.earc(28.0, 24.0, 18.0, 26.0, 180.0, 360.0);
    pen.ln(46.0, 50.0);
    pen.mv(10.0, 24.0);
    pen.ln(10.0, -18.0);
    out.push(glyph('µ', 56, pen));
    out.push(Glyph {
        number: 'μ' as u16,
        name: shape_name('μ'),
        advance: 56,
        strokes: out.last().unwrap().strokes.clone(),
    });

    pen = Pen::new();
    pen.mv(8.0, 40.0);
    pen.ln(16.0, 74.0);
    out.push(glyph('′', 22, pen));

    pen = Pen::new();
    pen.mv(6.0, 40.0);
    pen.ln(14.0, 74.0);
    pen.mv(16.0, 40.0);
    pen.ln(24.0, 74.0);
    out.push(glyph('″', 32, pen));

    pen = Pen::new();
    pen.mv(4.0, 34.0);
    pen.ln(28.0, 34.0);
    out.push(glyph('–', 32, pen));

    pen = Pen::new();
    pen.mv(4.0, 34.0);
    pen.ln(48.0, 34.0);
    out.push(glyph('—', 52, pen));

    pen = Pen::new();
    pen.blob(8.0, 8.0, 3.6);
    pen.blob(24.0, 8.0, 3.6);
    pen.blob(40.0, 8.0, 3.6);
    out.push(glyph('…', 52, pen));

    pen = Pen::new();
    pen.blob(12.0, 20.0, 5.0);
    out.push(glyph('•', 28, pen));

    pen = Pen::new();
    pen.blob(8.0, 18.0, 3.2);
    out.push(glyph('·', 18, pen));

    pen = Pen::new();
    pen.mv(30.0, 54.0);
    pen.ln(12.0, 36.0);
    pen.ln(30.0, 18.0);
    pen.mv(46.0, 54.0);
    pen.ln(28.0, 36.0);
    pen.ln(46.0, 18.0);
    out.push(glyph('«', 52, pen));

    pen = Pen::new();
    pen.mv(8.0, 54.0);
    pen.ln(26.0, 36.0);
    pen.ln(8.0, 18.0);
    pen.mv(24.0, 54.0);
    pen.ln(42.0, 36.0);
    pen.ln(24.0, 18.0);
    out.push(glyph('»', 52, pen));

    pen = Pen::new();
    pen.mv(40.0, 64.0);
    pen.bez(40.0, 80.0, 10.0, 78.0, 16.0, 52.0);
    pen.ln(16.0, 10.0);
    pen.bez(16.0, -4.0, 42.0, -6.0, 40.0, 12.0);
    pen.mv(8.0, 40.0);
    pen.ln(36.0, 40.0);
    pen.mv(6.0, 22.0);
    pen.ln(30.0, 22.0);
    out.push(glyph('£', 50, pen));

    pen = Pen::new();
    pen.mv(8.0, 72.0);
    pen.ln(28.0, 36.0);
    pen.ln(48.0, 72.0);
    pen.mv(28.0, 36.0);
    pen.ln(28.0, 0.0);
    pen.mv(14.0, 28.0);
    pen.ln(42.0, 28.0);
    pen.mv(12.0, 16.0);
    pen.ln(44.0, 16.0);
    out.push(glyph('¥', 56, pen));

    pen = Pen::new();
    pen.earc(34.0, 36.0, 22.0, 30.0, 55.0, 305.0);
    pen.mv(6.0, 46.0);
    pen.ln(36.0, 46.0);
    pen.mv(8.0, 26.0);
    pen.ln(32.0, 26.0);
    out.push(glyph('€', 58, pen));

    pen = Pen::new();
    pen.earc(28.0, 36.0, 16.0, 22.0, 55.0, 305.0);
    pen.mv(28.0, 8.0);
    pen.ln(28.0, 64.0);
    out.push(glyph('¢', 48, pen));

    pen = Pen::new();
    pen.earc(26.0, 54.0, 12.0, 14.0, 210.0, 500.0);
    pen.earc(26.0, 22.0, 12.0, 14.0, 30.0, 320.0);
    out.push(glyph('§', 48, pen));

    pen = Pen::new();
    pen.earc(22.0, 48.0, 14.0, 16.0, 90.0, 450.0);
    pen.mv(36.0, 0.0);
    pen.ln(36.0, 64.0);
    out.push(glyph('¶', 48, pen));

    pen = Pen::new();
    pen.mv(10.0, 8.0);
    pen.ln(18.0, 8.0);
    pen.ln(18.0, 40.0);
    pen.earc(36.0, 40.0, 18.0, 32.0, 180.0, 0.0);
    pen.ln(54.0, 8.0);
    pen.ln(62.0, 8.0);
    out.push(glyph('Ω', 72, pen));

    pen = Pen::new();
    pen.mv(6.0, 0.0);
    pen.ln(32.0, 72.0);
    pen.ln(58.0, 0.0);
    pen.ln(6.0, 0.0);
    out.push(glyph('Δ', 64, pen));

    pen = Pen::new();
    pen.mv(6.0, 48.0);
    pen.ln(46.0, 48.0);
    pen.mv(16.0, 48.0);
    pen.ln(14.0, 0.0);
    pen.mv(36.0, 48.0);
    pen.ln(40.0, 0.0);
    out.push(glyph('π', 52, pen));

    pen = Pen::new();
    pen.earc(24.0, 22.0, 14.0, 16.0, 30.0, 330.0);
    pen.mv(34.0, 14.0);
    pen.ln(44.0, 0.0);
    out.push(glyph('α', 50, pen));

    pen = Pen::new();
    pen.mv(14.0, -16.0);
    pen.ln(14.0, 52.0);
    pen.mv(14.0, 52.0);
    pen.ln(24.0, 52.0);
    pen.earc(24.0, 40.0, 12.0, 12.0, 90.0, -90.0);
    pen.ln(14.0, 28.0);
    pen.mv(14.0, 28.0);
    pen.ln(26.0, 28.0);
    pen.earc(26.0, 14.0, 14.0, 14.0, 90.0, -90.0);
    pen.ln(14.0, 0.0);
    out.push(glyph('β', 46, pen));

    pen = Pen::new();
    pen.mv(8.0, 50.0);
    pen.ln(24.0, 16.0);
    pen.mv(42.0, 50.0);
    pen.ln(24.0, 16.0);
    pen.ln(10.0, -18.0);
    out.push(glyph('γ', 50, pen));

    pen = Pen::new();
    pen.earc(26.0, 26.0, 18.0, 26.0, 0.0, 360.0);
    pen.mv(10.0, 26.0);
    pen.ln(42.0, 26.0);
    out.push(glyph('θ', 54, pen));

    pen = Pen::new();
    pen.earc(26.0, 28.0, 16.0, 16.0, 0.0, 360.0);
    pen.mv(26.0, -16.0);
    pen.ln(26.0, 70.0);
    out.push(glyph('φ', 52, pen));

    // Ligatures and letters that are not a base plus a mark.
    pen = Pen::new();
    pen.mv(6.0, 0.0);
    pen.ln(20.0, 72.0);
    pen.ln(34.0, 0.0);
    pen.mv(12.0, 28.0);
    pen.ln(28.0, 28.0);
    pen.mv(34.0, 0.0);
    pen.ln(34.0, 72.0);
    pen.mv(34.0, 72.0);
    pen.ln(60.0, 72.0);
    pen.mv(34.0, 36.0);
    pen.ln(54.0, 36.0);
    pen.mv(34.0, 0.0);
    pen.ln(60.0, 0.0);
    out.push(glyph('Æ', 68, pen));

    pen = Pen::new();
    pen.earc(16.0, 16.0, 12.0, 14.0, 30.0, 330.0);
    pen.mv(28.0, 0.0);
    pen.ln(28.0, 26.0);
    pen.ln(46.0, 26.0);
    pen.mv(28.0, 26.0);
    pen.curve(&[
        (28.0, 26.0),
        (24.0, 40.0),
        (36.0, 50.0),
        (48.0, 40.0),
        (48.0, 28.0),
    ]);
    out.push(glyph('æ', 56, pen));

    pen = Pen::new();
    pen.mv(10.0, 0.0);
    pen.ln(10.0, 72.0);
    pen.mv(10.0, 72.0);
    pen.ln(28.0, 72.0);
    pen.earc(28.0, 36.0, 20.0, 36.0, 90.0, -90.0);
    pen.ln(10.0, 0.0);
    pen.mv(4.0, 36.0);
    pen.ln(24.0, 36.0);
    out.push(glyph('Ð', 58, pen));

    pen = Pen::new();
    pen.earc(28.0, 24.0, 16.0, 22.0, 0.0, 360.0);
    pen.mv(44.0, 0.0);
    pen.ln(44.0, 70.0);
    pen.mv(36.0, 36.0);
    pen.ln(52.0, 48.0);
    out.push(glyph('ð', 58, pen));

    pen = Pen::new();
    pen.mv(10.0, 0.0);
    pen.ln(10.0, 72.0);
    pen.mv(10.0, 72.0);
    pen.ln(26.0, 72.0);
    pen.earc(26.0, 54.0, 16.0, 18.0, 90.0, -90.0);
    pen.ln(10.0, 36.0);
    out.push(glyph('Þ', 52, pen));

    pen = Pen::new();
    pen.mv(12.0, -18.0);
    pen.ln(12.0, 50.0);
    pen.mv(12.0, 50.0);
    pen.ln(26.0, 50.0);
    pen.earc(26.0, 34.0, 14.0, 16.0, 90.0, -90.0);
    pen.ln(12.0, 18.0);
    out.push(glyph('þ', 50, pen));

    pen = Pen::new();
    pen.mv(16.0, 0.0);
    pen.ln(16.0, 40.0);
    pen.curve(&[
        (16.0, 40.0),
        (16.0, 58.0),
        (28.0, 66.0),
        (42.0, 58.0),
        (40.0, 44.0),
        (24.0, 38.0),
        (18.0, 32.0),
        (28.0, 26.0),
        (42.0, 18.0),
        (40.0, 6.0),
        (26.0, 2.0),
        (16.0, 10.0),
    ]);
    out.push(glyph('ß', 52, pen));

    pen = Pen::new();
    // Non-breaking space matches the word space.
    out.push(glyph('\u{00A0}', 30, pen));

    pen = Pen::new();
    pen.mv(4.0, 34.0);
    pen.ln(28.0, 34.0);
    out.push(glyph('\u{00AD}', 32, pen));

    out
}

#[derive(Clone, Copy)]
enum Mark {
    Acute,
    Grave,
    Circumflex,
    Tilde,
    Diaeresis,
    Ring,
    Cedilla,
    Stroke,
}

fn accented(bases: &[Glyph]) -> Vec<Glyph> {
    const MAP: &[(char, char, Mark, bool)] = &[
        ('A', 'À', Mark::Grave, false),
        ('A', 'Á', Mark::Acute, false),
        ('A', 'Â', Mark::Circumflex, false),
        ('A', 'Ã', Mark::Tilde, false),
        ('A', 'Ä', Mark::Diaeresis, false),
        ('A', 'Å', Mark::Ring, false),
        ('E', 'È', Mark::Grave, false),
        ('E', 'É', Mark::Acute, false),
        ('E', 'Ê', Mark::Circumflex, false),
        ('E', 'Ë', Mark::Diaeresis, false),
        ('I', 'Ì', Mark::Grave, false),
        ('I', 'Í', Mark::Acute, false),
        ('I', 'Î', Mark::Circumflex, false),
        ('I', 'Ï', Mark::Diaeresis, false),
        ('N', 'Ñ', Mark::Tilde, false),
        ('O', 'Ò', Mark::Grave, false),
        ('O', 'Ó', Mark::Acute, false),
        ('O', 'Ô', Mark::Circumflex, false),
        ('O', 'Õ', Mark::Tilde, false),
        ('O', 'Ö', Mark::Diaeresis, false),
        ('O', 'Ø', Mark::Stroke, false),
        ('U', 'Ù', Mark::Grave, false),
        ('U', 'Ú', Mark::Acute, false),
        ('U', 'Û', Mark::Circumflex, false),
        ('U', 'Ü', Mark::Diaeresis, false),
        ('Y', 'Ý', Mark::Acute, false),
        ('C', 'Ç', Mark::Cedilla, false),
        ('a', 'à', Mark::Grave, false),
        ('a', 'á', Mark::Acute, false),
        ('a', 'â', Mark::Circumflex, false),
        ('a', 'ã', Mark::Tilde, false),
        ('a', 'ä', Mark::Diaeresis, false),
        ('a', 'å', Mark::Ring, false),
        ('e', 'è', Mark::Grave, false),
        ('e', 'é', Mark::Acute, false),
        ('e', 'ê', Mark::Circumflex, false),
        ('e', 'ë', Mark::Diaeresis, false),
        ('i', 'ì', Mark::Grave, true),
        ('i', 'í', Mark::Acute, true),
        ('i', 'î', Mark::Circumflex, true),
        ('i', 'ï', Mark::Diaeresis, true),
        ('n', 'ñ', Mark::Tilde, false),
        ('o', 'ò', Mark::Grave, false),
        ('o', 'ó', Mark::Acute, false),
        ('o', 'ô', Mark::Circumflex, false),
        ('o', 'õ', Mark::Tilde, false),
        ('o', 'ö', Mark::Diaeresis, false),
        ('o', 'ø', Mark::Stroke, false),
        ('u', 'ù', Mark::Grave, false),
        ('u', 'ú', Mark::Acute, false),
        ('u', 'û', Mark::Circumflex, false),
        ('u', 'ü', Mark::Diaeresis, false),
        ('y', 'ý', Mark::Acute, false),
        ('y', 'ÿ', Mark::Diaeresis, false),
        ('c', 'ç', Mark::Cedilla, false),
    ];
    MAP.iter()
        .filter_map(|(base, ch, mark, drop_dot)| {
            let src = bases.iter().find(|glyph| glyph.number == *base as u16)?;
            let mut strokes = src.strokes.clone();
            if *drop_dot {
                strokes.retain(|stroke| !is_tittle(stroke));
            }
            // Ø from the mark would duplicate the hand-built diameter. The
            // mark still builds the Latin letter; the earlier symbols entry
            // is the engineering slashed circle and is overwritten on encode
            // only if the number collides. Keep one by skipping the duplicate
            // number here when a base glyph already claimed it.
            if bases.iter().any(|glyph| glyph.number == *ch as u16) {
                return None;
            }
            add_mark(&mut strokes, *mark);
            Some(Glyph {
                number: *ch as u16,
                name: shape_name(*ch),
                advance: src.advance,
                strokes,
            })
        })
        .collect()
}

fn is_tittle(stroke: &[(i32, i32)]) -> bool {
    let min_y = stroke.iter().map(|point| point.1).min().unwrap_or(0);
    let max_y = stroke.iter().map(|point| point.1).max().unwrap_or(0);
    let min_x = stroke.iter().map(|point| point.0).min().unwrap_or(0);
    let max_x = stroke.iter().map(|point| point.0).max().unwrap_or(0);
    min_y >= 52 && max_y - min_y <= 22 && max_x - min_x <= 22
}

fn add_mark(strokes: &mut Vec<Vec<(i32, i32)>>, mark: Mark) {
    let (min_x, max_x, min_y, max_y) = ink_bounds(strokes);
    let cx = (min_x + max_x) as f64 * 0.5;
    let high = max_y >= 64;
    let mut pen = Pen::new();
    match mark {
        Mark::Acute => {
            let y = if high { 78.0 } else { 58.0 };
            pen.mv(cx - 5.0, y);
            pen.ln(cx + 8.0, y + 14.0);
        }
        Mark::Grave => {
            let y = if high { 92.0 } else { 72.0 };
            pen.mv(cx + 5.0, y - 14.0);
            pen.ln(cx - 8.0, y);
        }
        Mark::Circumflex => {
            let y = if high { 80.0 } else { 60.0 };
            pen.mv(cx - 9.0, y);
            pen.ln(cx, y + 12.0);
            pen.ln(cx + 9.0, y);
        }
        Mark::Tilde => {
            let y = if high { 84.0 } else { 64.0 };
            pen.mv(cx - 11.0, y);
            pen.bez(cx - 4.0, y + 8.0, cx + 4.0, y - 8.0, cx + 11.0, y);
        }
        Mark::Diaeresis => {
            let y = if high { 86.0 } else { 66.0 };
            pen.blob(cx - 7.0, y, 3.3);
            pen.blob(cx + 7.0, y, 3.3);
        }
        Mark::Ring => {
            let y = if high { 88.0 } else { 70.0 };
            pen.earc(cx, y, 6.5, 6.5, 0.0, 360.0);
        }
        Mark::Cedilla => {
            let y = min_y as f64;
            pen.mv(cx, y + 2.0);
            pen.bez(cx, y - 16.0, cx - 14.0, y - 18.0, cx - 8.0, y - 8.0);
        }
        Mark::Stroke => {
            pen.mv(min_x as f64 - 1.0, min_y as f64 + 8.0);
            pen.ln(max_x as f64 + 1.0, max_y as f64 - 6.0);
        }
    }
    strokes.extend(pen.finish());
}

fn ink_bounds(strokes: &[Vec<(i32, i32)>]) -> (i32, i32, i32, i32) {
    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    for stroke in strokes {
        for (x, y) in stroke {
            min_x = min_x.min(*x);
            max_x = max_x.max(*x);
            min_y = min_y.min(*y);
            max_y = max_y.max(*y);
        }
    }
    if min_x == i32::MAX {
        (0, 0, 0, 0)
    } else {
        (min_x, max_x, min_y, max_y)
    }
}

fn scaled_figures(bases: &[Glyph]) -> Vec<Glyph> {
    let mut out = Vec::new();
    let Some(one) = bases.iter().find(|glyph| glyph.number == '1' as u16) else {
        return out;
    };
    let Some(two) = bases.iter().find(|glyph| glyph.number == '2' as u16) else {
        return out;
    };
    let Some(three) = bases.iter().find(|glyph| glyph.number == '3' as u16) else {
        return out;
    };
    let Some(four) = bases.iter().find(|glyph| glyph.number == '4' as u16) else {
        return out;
    };
    out.push(scale_glyph(one, '¹', 0.48, 0.48, 2.0, 38.0, 30));
    out.push(scale_glyph(two, '²', 0.48, 0.48, 2.0, 38.0, 30));
    out.push(scale_glyph(three, '³', 0.48, 0.48, 2.0, 38.0, 30));
    out.push(fraction(one, four, '¼'));
    out.push(fraction(one, two, '½'));
    out.push(fraction(three, four, '¾'));
    out
}

fn scale_glyph(src: &Glyph, ch: char, sx: f64, sy: f64, ox: f64, oy: f64, advance: i32) -> Glyph {
    let strokes = src
        .strokes
        .iter()
        .map(|stroke| {
            stroke
                .iter()
                .map(|(x, y)| pt(ox + *x as f64 * sx, oy + *y as f64 * sy))
                .fold(Vec::new(), |mut acc, point| {
                    if acc.last() != Some(&point) {
                        acc.push(point);
                    }
                    acc
                })
        })
        .filter(|stroke| stroke.len() >= 2)
        .collect();
    Glyph {
        number: ch as u16,
        name: shape_name(ch),
        advance,
        strokes,
    }
}

fn fraction(top: &Glyph, bottom: &Glyph, ch: char) -> Glyph {
    let mut upper = scale_glyph(top, ch, 0.40, 0.40, 28.0, 40.0, 52);
    let lower = scale_glyph(bottom, ch, 0.40, 0.40, 0.0, 0.0, 52);
    upper.strokes.extend(lower.strokes);
    let mut pen = Pen::new();
    pen.mv(16.0, 4.0);
    pen.ln(30.0, 68.0);
    upper.strokes.extend(pen.finish());
    upper.name = shape_name(ch);
    upper.number = ch as u16;
    upper.advance = 52;
    upper
}

fn encode_shx() -> Vec<u8> {
    let mut glyphs = library();
    glyphs.sort_by_key(|glyph| glyph.number);
    glyphs.dedup_by_key(|glyph| glyph.number);
    let mut out = b"AutoCAD-86 shapes 1.0\r\n".to_vec();
    out.push(0x1A);
    out.extend_from_slice(&((glyphs.len() + 1) as u16).to_le_bytes());
    push_record(&mut out, 0, "ACADRUST", &[ABOVE, BELOW, 0, 0]);
    for glyph in &glyphs {
        let codes = encode_codes(glyph);
        push_record(&mut out, glyph.number, &glyph.name, &codes);
    }
    out
}

fn push_record(out: &mut Vec<u8>, number: u16, name: &str, codes: &[u8]) {
    let mut payload = name.as_bytes().to_vec();
    payload.push(0);
    payload.extend_from_slice(codes);
    out.extend_from_slice(&number.to_le_bytes());
    out.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    out.extend_from_slice(&payload);
}

fn encode_codes(glyph: &Glyph) -> Vec<u8> {
    let mut codes = Vec::new();
    let mut x = 0i32;
    let mut y = 0i32;
    for stroke in &glyph.strokes {
        if stroke.len() < 2 {
            continue;
        }
        codes.push(2);
        displace(&mut codes, &mut x, &mut y, stroke[0].0, stroke[0].1);
        codes.push(1);
        for point in &stroke[1..] {
            displace(&mut codes, &mut x, &mut y, point.0, point.1);
        }
    }
    codes.push(2);
    displace(&mut codes, &mut x, &mut y, glyph.advance, 0);
    codes.push(0);
    codes
}

fn displace(codes: &mut Vec<u8>, x: &mut i32, y: &mut i32, to_x: i32, to_y: i32) {
    let mut dx = to_x - *x;
    let mut dy = to_y - *y;
    while dx != 0 || dy != 0 {
        let step_x = dx.clamp(-127, 127);
        let step_y = dy.clamp(-127, 127);
        codes.push(8);
        codes.push(step_x as i8 as u8);
        codes.push(step_y as i8 as u8);
        dx -= step_x;
        dy -= step_y;
    }
    *x = to_x;
    *y = to_y;
}

fn encode_shp() -> String {
    let bytes = acadrust_shx();
    let end = bytes.iter().position(|byte| *byte == 0x1A).unwrap();
    let rest = &bytes[end + 1..];
    let count = u16::from_le_bytes([rest[0], rest[1]]) as usize;
    let mut text = String::from(
        "; Acadrust Stroke. Original single-stroke font, shapes 1.0.\n; Cap height 72, descender 20.\n",
    );
    let mut offset = 2;
    for _ in 0..count {
        let number = u16::from_le_bytes([rest[offset], rest[offset + 1]]);
        let length = u16::from_le_bytes([rest[offset + 2], rest[offset + 3]]) as usize;
        offset += 4;
        let record = &rest[offset..offset + length];
        offset += length;
        let name_end = record.iter().position(|byte| *byte == 0).unwrap();
        let name = std::str::from_utf8(&record[..name_end]).unwrap();
        let codes = &record[name_end + 1..];
        text.push_str(&format!("*{number},{},{name}\n", codes.len()));
        text.push_str(&format_codes(codes));
        text.push('\n');
    }
    text
}

fn format_codes(codes: &[u8]) -> String {
    let mut parts = Vec::new();
    let mut index = 0;
    while index < codes.len() {
        let code = codes[index];
        index += 1;
        parts.push(code.to_string());
        let signed = match code {
            8 => 2,
            9 => {
                let start = parts.len();
                while index + 1 < codes.len() {
                    let dx = codes[index] as i8;
                    let dy = codes[index + 1] as i8;
                    index += 2;
                    parts.push(dx.to_string());
                    parts.push(dy.to_string());
                    if dx == 0 && dy == 0 {
                        break;
                    }
                }
                let _ = start;
                0
            }
            _ => 0,
        };
        for _ in 0..signed {
            if index >= codes.len() {
                break;
            }
            parts.push((codes[index] as i8).to_string());
            index += 1;
        }
    }
    let mut lines = String::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            if index % 12 == 0 {
                lines.push('\n');
            } else {
                lines.push(',');
            }
        }
        lines.push_str(part);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_font_covers_text_and_keeps_its_box() {
        let font = font();
        assert!((font.above() - 72.0).abs() < 1e-9);
        let sample = "\
            ABCDEFGHIJKLMNOPQRSTUVWXYZ\
            abcdefghijklmnopqrstuvwxyz\
            0123456789\
            .,:;!?\"'`()[]{}-+*/\\|=<>%#@&$_ \
            °±Øøµ×÷–—•…«»£¥€¢′″¹²³½¼¾\
            ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖÙÚÛÜÝÞß\
            àáâãäåæçèéêëìíîïðñòóôõöùúûüýþÿ\
            ΩΔπαβγθφ";
        for ch in sample.chars() {
            let (strokes, advance) = font.glyph_for_char(ch);
            assert!(advance > 8.0, "{ch:?} advance {advance}");
            if ch != ' ' && ch != '\u{00A0}' {
                assert!(!strokes.is_empty(), "{ch:?} drew nothing");
            }
            for stroke in &strokes {
                assert!(stroke.len() >= 2, "{ch:?}");
                for point in stroke {
                    assert!(
                        (-40.0..advance + 40.0).contains(&point.x),
                        "{ch:?} x {}",
                        point.x
                    );
                    assert!((-40.0..110.0).contains(&point.y), "{ch:?} y {}", point.y);
                }
            }
        }
        let (_, advance_a) = font.glyph_for_char('A');
        assert!((advance_a - 64.0).abs() < 1e-6);
        let (strokes, _) = font.glyph_for_char('H');
        let max_y = strokes
            .iter()
            .flat_map(|stroke| stroke.iter())
            .map(|point| point.y)
            .fold(0.0, f64::max);
        assert!((max_y - 72.0).abs() < 1e-6);
        let (strokes, _) = font.glyph_for_char('O');
        let max_y = strokes
            .iter()
            .flat_map(|stroke| stroke.iter())
            .map(|point| point.y)
            .fold(f64::MIN, f64::max);
        let min_y = strokes
            .iter()
            .flat_map(|stroke| stroke.iter())
            .map(|point| point.y)
            .fold(f64::MAX, f64::min);
        assert!(max_y > 72.0, "O should overshoot the cap, y={max_y}");
        assert!(min_y < 0.0, "O should overshoot the baseline, y={min_y}");
        let digit_advance = font.glyph_for_char('0').1;
        for digit in '0'..='9' {
            assert!((font.glyph_for_char(digit).1 - digit_advance).abs() < 1e-6);
        }
        let (dotted, _) = font.glyph_for_char('i');
        let (acute, _) = font.glyph_for_char('í');
        let top = |strokes: &[Vec<crate::types::Vector2>]| {
            strokes
                .iter()
                .flat_map(|stroke| stroke.iter())
                .map(|point| point.y)
                .fold(0.0, f64::max)
        };
        assert!(
            top(&acute) > top(&dotted) + 1.0,
            "accented i should rise above the tittle"
        );
    }

    #[test]
    fn e_and_s_keep_their_openings() {
        let font = font();
        let (strokes, _) = font.glyph_for_char('e');
        let bar = strokes.iter().any(|stroke| {
            let flat = stroke.windows(2).any(|pair| {
                (pair[1].y - pair[0].y).abs() < 1.0 && (pair[1].x - pair[0].x).abs() > 20.0
            });
            flat
        });
        assert!(bar, "e needs a crossbar");
        let closed_on_the_right = strokes
            .iter()
            .flatten()
            .any(|point| point.x > 42.0 && (18.0..40.0).contains(&point.y));
        assert!(!closed_on_the_right, "e bowl should stay open on the right");

        let (strokes, _) = font.glyph_for_char('S');
        let lower_terminal = strokes
            .iter()
            .flatten()
            .filter(|point| point.x > 40.0 && point.y < 36.0)
            .map(|point| point.y)
            .fold(0.0_f64, f64::max);
        assert!(
            lower_terminal < 16.0,
            "S lower terminal climbed to {lower_terminal}"
        );
    }

    #[test]
    fn shp_text_parses_back_to_the_same_advances() {
        let from_shx = font();
        let from_shp = parse_shp_for_test(acadrust_shp());
        for ch in "Acadrust 10.5°".chars() {
            let (_, shx) = from_shx.glyph_for_char(ch);
            let (_, shp) = from_shp.glyph_for_char(ch);
            assert!((shx - shp).abs() < 1e-6, "{ch:?} shx {shx} shp {shp}");
        }
    }

    fn parse_shp_for_test(text: &str) -> ShapeFont {
        super::super::shx::ShapeFont::load_bytes(text.as_bytes()).expect("shp")
    }

    #[test]
    fn checked_in_font_matches_the_encoder() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/fonts");
        let shx = root.join("acadrust.shx");
        let shp = root.join("acadrust.shp");
        let shx_bytes = acadrust_shx();
        let shp_text = acadrust_shp();
        if std::env::var_os("ACADRUST_WRITE_FONT").is_some() {
            std::fs::create_dir_all(&root).unwrap();
            std::fs::write(&shx, shx_bytes).unwrap();
            std::fs::write(&shp, shp_text).unwrap();
        }
        assert_eq!(
            std::fs::read(&shx).expect("assets/fonts/acadrust.shx"),
            shx_bytes
        );
        assert_eq!(
            std::fs::read_to_string(&shp).expect("assets/fonts/acadrust.shp"),
            shp_text
        );
    }

    #[test]
    fn write_specimen_when_requested() {
        if std::env::var_os("ACADRUST_FONT_SPECIMEN").is_none() {
            return;
        }
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
        write_specimen(&root.join("acadrust-font-specimen-4.png"));
        write_contact(&root.join("acadrust-font-contact-4.png"));
    }

    fn write_specimen(path: &std::path::Path) {
        let font = font();
        let scale = 1.05_f64;
        let lines = [
            "ACADRUST STROKE",
            "The quick brown fox jumps over the lazy dog.",
            "THE QUICK BROWN FOX JUMPS OVER THE LAZY DOG.",
            "Hamburgefonstiv 0123456789 10.25° ±0.05 Ø12",
            "1′-6″  m²  ½ ¼ ¾  £ ¥ €  «quoted»",
            "ABCDEFGHIJKLMNOPQRSTUVWXYZ",
            "abcdefghijklmnopqrstuvwxyz",
            "ÀÁÂÃÄÅÆÇÈÉÊËÌÍÎÏÐÑÒÓÔÕÖØÙÚÛÜÝÞß",
            "àáâãäåæçèéêëìíîïðñòóôõöøùúûüýþÿ",
            ".,:;!?\"'`()[]{}<>/\\|+-=*_#$%@&",
            "°±×÷–—•…µμΩΔπαβγθφ⌀",
        ];
        let line_h = 120.0 * scale;
        let width = 2480u32;
        let height = (lines.len() as f64 * line_h + 40.0) as u32;
        let mut image =
            image::RgbaImage::from_pixel(width, height, image::Rgba([247, 244, 236, 255]));
        let ink = image::Rgba([22, 24, 28, 255]);
        for (index, line) in lines.iter().enumerate() {
            let baseline = 36.0 + index as f64 * line_h + 78.0 * scale;
            draw_line(&mut image, &font, line, 24.0, baseline, scale, ink);
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        image.save(path).unwrap();
    }

    fn write_contact(path: &std::path::Path) {
        let font = font();
        let sample = "AaBbCcDdEeFfGgHhIiJjKkLlMmNnOoPpQqRrSsTtUuVvWwXxYyZz0123456789&@#$%";
        let scale = 2.4_f64;
        let cell_w = 150.0 * scale;
        let cell_h = 90.0 * scale;
        let cols = 6u32;
        let rows = sample.chars().count().div_ceil(cols as usize) as u32;
        let mut image = image::RgbaImage::from_pixel(
            (cols as f64 * cell_w) as u32 + 20,
            (rows as f64 * cell_h) as u32 + 20,
            image::Rgba([247, 244, 236, 255]),
        );
        let ink = image::Rgba([22, 24, 28, 255]);
        for (index, ch) in sample.chars().enumerate() {
            let col = index as u32 % cols;
            let row = index as u32 / cols;
            let x = 16.0 + col as f64 * cell_w;
            let baseline = 24.0 + row as f64 * cell_h + 78.0 * scale;
            draw_line(&mut image, &font, &ch.to_string(), x, baseline, scale, ink);
        }
        image.save(path).unwrap();
    }

    fn draw_line(
        image: &mut image::RgbaImage,
        font: &ShapeFont,
        text: &str,
        mut x: f64,
        baseline: f64,
        scale: f64,
        ink: image::Rgba<u8>,
    ) {
        for ch in text.chars() {
            let (strokes, advance) = font.glyph_for_char(ch);
            for stroke in strokes {
                let mut previous: Option<(f64, f64)> = None;
                for point in stroke {
                    let px = x + point.x * scale;
                    let py = baseline - point.y * scale;
                    if let Some((x0, y0)) = previous {
                        paint_segment(image, x0, y0, px, py, 1.35, ink);
                    }
                    previous = Some((px, py));
                }
            }
            x += advance * scale;
        }
    }

    fn paint_segment(
        image: &mut image::RgbaImage,
        x0: f64,
        y0: f64,
        x1: f64,
        y1: f64,
        radius: f64,
        ink: image::Rgba<u8>,
    ) {
        let length = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt();
        let steps = (length / 0.8).ceil().max(1.0) as usize;
        for step in 0..=steps {
            let t = step as f64 / steps as f64;
            paint_disk(image, x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, radius, ink);
        }
    }

    fn paint_disk(image: &mut image::RgbaImage, x: f64, y: f64, radius: f64, ink: image::Rgba<u8>) {
        let reach = radius.ceil() as i32;
        let cx = x.round() as i32;
        let cy = y.round() as i32;
        let limit = radius * radius;
        for dy in -reach..=reach {
            for dx in -reach..=reach {
                if (dx * dx + dy * dy) as f64 > limit {
                    continue;
                }
                let px = cx + dx;
                let py = cy + dy;
                if px >= 0 && py >= 0 && (px as u32) < image.width() && (py as u32) < image.height()
                {
                    image.put_pixel(px as u32, py as u32, ink);
                }
            }
        }
    }
}
