//! PDF export for [`CadDocument`].
//!
//! Converts a CAD drawing to a PDF (Portable Document Format) file using
//! only the Rust standard library — no external dependencies are required.
//!
//! The output is a valid PDF 1.4 document containing a single page with
//! all visible entities rendered as vector graphics.
//!
//! # Coordinate system
//!
//! PDF and DXF share the same coordinate convention (Y-axis points up,
//! origin at the bottom-left).  The writer automatically scales and
//! translates the drawing to fit a configurable page size with padding.
//!
//! # Page size
//!
//! The default page size is **A4** (595 × 842 pt).  Use
//! [`PdfWriter::with_page_size`] to specify a custom size in points,
//! or [`PdfPageSize`] for the built-in presets.
//!
//! # Colour handling
//!
//! Colour resolution follows the same rules as [`SvgWriter`](super::svg::SvgWriter):
//! `ByLayer` colours are looked up in the layer table; `ByBlock` falls back
//! to black; ACI indices are converted using the standard 256-colour palette.
//!
//! # Supported entities
//!
//! | DXF entity   | PDF output                                       |
//! |--------------|--------------------------------------------------|
//! | `LINE`       | stroked path segment                             |
//! | `CIRCLE`     | stroked circular path (4 Bézier curves)          |
//! | `ARC`        | stroked arc path                                 |
//! | `ELLIPSE`    | stroked ellipse / partial arc path               |
//! | `LWPOLYLINE` | stroked polyline / path (w/ bulge arcs)          |
//! | `POINT`      | small filled circle                              |
//! | `TEXT`       | text with Helvetica (Type1, always embedded)     |
//! | `MTEXT`      | text (formatting codes stripped)                 |
//!
//! # Example
//!
//! ```rust,no_run
//! use acadrust::{CadDocument, EntityType};
//! use acadrust::entities::Line;
//! use acadrust::io::pdf::PdfWriter;
//!
//! let mut doc = CadDocument::new();
//! doc.add_entity(EntityType::Line(
//!     Line::from_coords(0.0, 0.0, 0.0, 100.0, 50.0, 0.0),
//! ))
//! .unwrap();
//!
//! PdfWriter::new(&doc).write_to_file("output.pdf").unwrap();
//! ```

use std::fmt::Write as FmtWrite;
use std::io::Write as IoWrite;
use std::path::Path;

use crate::document::CadDocument;
use crate::entities::{EntityType, LwVertex};
use crate::error::Result;
use crate::types::{BoundingBox3D, Color, Vector3};
use crate::io::svg::aci_to_rgb; // reuse the same ACI palette

// ─── Page size presets ───────────────────────────────────────────────────────

/// Common PDF page sizes (width × height in points, 1 pt = 1/72 inch).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PdfPageSize {
    /// Page width in PDF points
    pub width: f64,
    /// Page height in PDF points
    pub height: f64,
}

impl PdfPageSize {
    /// A4 (210 × 297 mm)
    pub const A4: PdfPageSize = PdfPageSize { width: 595.276, height: 841.890 };
    /// A3 (297 × 420 mm)
    pub const A3: PdfPageSize = PdfPageSize { width: 841.890, height: 1190.551 };
    /// Letter (8.5 × 11 in)
    pub const LETTER: PdfPageSize = PdfPageSize { width: 612.0, height: 792.0 };
    /// Legal (8.5 × 14 in)
    pub const LEGAL: PdfPageSize = PdfPageSize { width: 612.0, height: 1008.0 };

    /// Create a custom page size in points.
    pub fn custom(width: f64, height: f64) -> Self {
        PdfPageSize { width, height }
    }

    /// Create a page size from millimetres.
    pub fn from_mm(width_mm: f64, height_mm: f64) -> Self {
        PdfPageSize {
            width: width_mm * 2.8346457,
            height: height_mm * 2.8346457,
        }
    }
}

impl Default for PdfPageSize {
    fn default() -> Self {
        PdfPageSize::A4
    }
}

// ─── PdfWriter ───────────────────────────────────────────────────────────────

/// Exports a [`CadDocument`] to a PDF file.
///
/// See the [module documentation](self) for supported entities and details.
pub struct PdfWriter<'a> {
    document: &'a CadDocument,
    page: PdfPageSize,
    margin: f64,
    stroke_width: Option<f64>,
    point_radius: Option<f64>,
}

impl<'a> PdfWriter<'a> {
    /// Create a new `PdfWriter` for the given document.
    pub fn new(document: &'a CadDocument) -> Self {
        PdfWriter {
            document,
            page: PdfPageSize::A4,
            margin: 20.0,
            stroke_width: None,
            point_radius: None,
        }
    }

    /// Set the output page size (default: A4).
    pub fn with_page_size(mut self, size: PdfPageSize) -> Self {
        self.page = size;
        self
    }

    /// Set the page margin in points (default: 20 pt).
    pub fn with_margin(mut self, margin: f64) -> Self {
        self.margin = margin;
        self
    }

    /// Override the stroke width in PDF points.
    pub fn with_stroke_width(mut self, width: f64) -> Self {
        self.stroke_width = Some(width);
        self
    }

    /// Override the radius used for POINT entities in PDF points.
    pub fn with_point_radius(mut self, radius: f64) -> Self {
        self.point_radius = Some(radius);
        self
    }

    /// Write the PDF to a file at `path`.
    pub fn write_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let bytes = self.write_to_vec()?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Write the PDF to an arbitrary [`Write`](std::io::Write) sink.
    pub fn write_to_writer<W: IoWrite>(&self, writer: &mut W) -> Result<()> {
        let bytes = self.write_to_vec()?;
        writer.write_all(&bytes)?;
        Ok(())
    }

    /// Render the document to a `Vec<u8>` containing the PDF bytes.
    pub fn write_to_vec(&self) -> Result<Vec<u8>> {
        // ── drawing bounds ────────────────────────────────────────────────────
        let bounds = self.compute_bounds();
        let (draw_x0, draw_y0, draw_x1, draw_y1) = match bounds {
            Some(b) => (b.min.x, b.min.y, b.max.x, b.max.y),
            None => (0.0, 0.0, 100.0, 100.0),
        };
        let draw_w = (draw_x1 - draw_x0).max(1e-6);
        let draw_h = (draw_y1 - draw_y0).max(1e-6);

        let usable_w = self.page.width - 2.0 * self.margin;
        let usable_h = self.page.height - 2.0 * self.margin;
        let scale = (usable_w / draw_w).min(usable_h / draw_h);

        let larger_dim = draw_w.max(draw_h) * scale;
        let default_sw = larger_dim * 0.005;
        let sw = self.stroke_width.unwrap_or(default_sw).max(0.1);
        let pt_r = self.point_radius.unwrap_or(default_sw * 0.8).max(0.1);

        // Transform: pdf_coord = (dxf_coord - draw_min) * scale + margin
        let tx = |x: f64| (x - draw_x0) * scale + self.margin;
        let ty = |y: f64| (y - draw_y0) * scale + self.margin;

        // ── content stream ────────────────────────────────────────────────────
        let mut cs = String::with_capacity(64 * 1024);

        for entity in self.document.entities() {
            let common = entity.as_entity();
            if common.is_invisible() || self.is_layer_hidden(common.layer()) {
                continue;
            }

            let rgb = self.resolve_color(common.color(), common.layer());

            match entity {
                EntityType::Line(e) => {
                    cs_line(&mut cs, tx(e.start.x), ty(e.start.y),
                            tx(e.end.x), ty(e.end.y), rgb, sw);
                }
                EntityType::Circle(e) => {
                    cs_circle(&mut cs, tx(e.center.x), ty(e.center.y),
                              e.radius * scale, rgb, sw);
                }
                EntityType::Arc(e) => {
                    cs_arc(&mut cs, tx(e.center.x), ty(e.center.y),
                           e.radius * scale, e.start_angle, e.end_angle,
                           e.sweep_angle(), rgb, sw);
                }
                EntityType::Ellipse(e) => {
                    cs_ellipse(&mut cs, e, scale, &tx, &ty, rgb, sw);
                }
                EntityType::LwPolyline(e) => {
                    cs_lwpolyline(&mut cs, e, scale, &tx, &ty, rgb, sw);
                }
                EntityType::Point(e) => {
                    cs_point(&mut cs, tx(e.location.x), ty(e.location.y),
                             pt_r, rgb);
                }
                EntityType::Text(e) => {
                    let h = (e.height * scale).max(1.0);
                    cs_text(&mut cs, tx(e.insertion_point.x),
                            ty(e.insertion_point.y),
                            &e.value, h, e.rotation, rgb);
                }
                EntityType::MText(e) => {
                    let h = (e.height * scale).max(1.0);
                    let plain = strip_mtext_codes(&e.value);
                    cs_text(&mut cs, tx(e.insertion_point.x),
                            ty(e.insertion_point.y),
                            &plain, h, 0.0, rgb);
                }
                _ => {}
            }
        }

        // ── assemble PDF objects ──────────────────────────────────────────────
        build_pdf(
            &cs,
            self.page.width,
            self.page.height,
        )
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    fn compute_bounds(&self) -> Option<BoundingBox3D> {
        let mut bounds: Option<BoundingBox3D> = None;
        for entity in self.document.entities() {
            let common = entity.as_entity();
            if common.is_invisible() || self.is_layer_hidden(common.layer()) {
                continue;
            }
            let bb = common.bounding_box();
            bounds = Some(match bounds {
                None => bb,
                Some(b) => b.merge(&bb),
            });
        }
        bounds
    }

    fn is_layer_hidden(&self, name: &str) -> bool {
        self.document
            .layers
            .get(name)
            .map(|l| l.flags.frozen || l.flags.off)
            .unwrap_or(false)
    }

    fn resolve_color(&self, color: Color, layer_name: &str) -> (u8, u8, u8) {
        match color {
            Color::Rgb { r, g, b } => (r, g, b),
            Color::Index(i) => aci_to_rgb(i),
            Color::ByBlock => (0, 0, 0),
            Color::ByLayer => self
                .document
                .layers
                .get(layer_name)
                .map(|l| self.resolve_color(l.color, layer_name))
                .unwrap_or((0, 0, 0)),
        }
    }
}

// ─── Content stream helpers ───────────────────────────────────────────────────

/// Kappa constant for circular Bézier approximation.
const KAPPA: f64 = 0.5522847498;

/// Write a stroke-color command (PDF `RG`).
fn cs_set_stroke(cs: &mut String, rgb: (u8, u8, u8)) {
    let r = rgb.0 as f64 / 255.0;
    let g = rgb.1 as f64 / 255.0;
    let b = rgb.2 as f64 / 255.0;
    writeln!(cs, "{:.4} {:.4} {:.4} RG", r, g, b).unwrap();
}

/// Write a fill-color command (PDF `rg`).
fn cs_set_fill(cs: &mut String, rgb: (u8, u8, u8)) {
    let r = rgb.0 as f64 / 255.0;
    let g = rgb.1 as f64 / 255.0;
    let b = rgb.2 as f64 / 255.0;
    writeln!(cs, "{:.4} {:.4} {:.4} rg", r, g, b).unwrap();
}

/// Set line width.
fn cs_set_lw(cs: &mut String, w: f64) {
    writeln!(cs, "{:.4} w", w).unwrap();
}

/// Draw a line segment.
fn cs_line(cs: &mut String, x1: f64, y1: f64, x2: f64, y2: f64,
           rgb: (u8, u8, u8), sw: f64) {
    cs_set_stroke(cs, rgb);
    cs_set_lw(cs, sw);
    writeln!(cs, "{:.4} {:.4} m {:.4} {:.4} l S", x1, y1, x2, y2).unwrap();
}

/// Draw a filled circle using 4 Bézier arcs.
fn cs_circle_path(cs: &mut String, cx: f64, cy: f64, r: f64) {
    let k = r * KAPPA;
    writeln!(cs,
        "{:.4} {:.4} m \
{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} c \
{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} c \
{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} c \
{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} c h",
        cx + r, cy,                        // start
        cx + r, cy + k, cx + k, cy + r, cx, cy + r,   // Q1
        cx - k, cy + r, cx - r, cy + k, cx - r, cy,   // Q2
        cx - r, cy - k, cx - k, cy - r, cx, cy - r,   // Q3
        cx + k, cy - r, cx + r, cy - k, cx + r, cy,   // Q4
    ).unwrap();
}

/// Draw a stroked circle.
fn cs_circle(cs: &mut String, cx: f64, cy: f64, r: f64,
             rgb: (u8, u8, u8), sw: f64) {
    cs_set_stroke(cs, rgb);
    cs_set_lw(cs, sw);
    cs_circle_path(cs, cx, cy, r);
    writeln!(cs, "S").unwrap();
}

/// Draw a stroked arc.
fn cs_arc(cs: &mut String, cx: f64, cy: f64, r: f64,
          start_angle: f64, _end_angle: f64, sweep: f64,
          rgb: (u8, u8, u8), sw: f64) {
    if sweep >= 2.0 * std::f64::consts::PI - 1e-6 {
        cs_circle(cs, cx, cy, r, rgb, sw);
        return;
    }

    cs_set_stroke(cs, rgb);
    cs_set_lw(cs, sw);

    // Approximate arc with Bézier segments (each ≤ 90°).
    let segments = arc_bezier_segments(cx, cy, r, start_angle, sweep);
    let mut first = true;
    for seg in segments {
        if first {
            writeln!(cs, "{:.4} {:.4} m", seg[0], seg[1]).unwrap();
            first = false;
        }
        writeln!(cs,
            "{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} c",
            seg[2], seg[3], seg[4], seg[5], seg[6], seg[7],
        ).unwrap();
    }
    writeln!(cs, "S").unwrap();
}

/// Approximate a circular arc with cubic Bézier segments (≤ 90° each).
///
/// Returns a list of `[x0,y0, x1,y1, x2,y2, x3,y3]` for each segment,
/// where (x0,y0) is the start (repeated from previous segment's end).
fn arc_bezier_segments(cx: f64, cy: f64, r: f64, start: f64, sweep: f64)
    -> Vec<[f64; 8]>
{
    let max_seg = std::f64::consts::PI / 2.0; // 90°
    let n = (sweep / max_seg).ceil() as usize;
    let n = n.max(1);
    let step = sweep / n as f64;

    let mut segs = Vec::with_capacity(n);
    for i in 0..n {
        let a0 = start + step * i as f64;
        let a1 = a0 + step;
        // α = 4/3 * tan(θ/4) for the Bézier control point distance
        let alpha = (4.0 / 3.0) * (step / 4.0).tan();
        let (s0, c0) = a0.sin_cos();
        let (s1, c1) = a1.sin_cos();
        let x0 = cx + r * c0;
        let y0 = cy + r * s0;
        let x1 = x0 + r * alpha * (-s0);
        let y1 = y0 + r * alpha * c0;
        let x3 = cx + r * c1;
        let y3 = cy + r * s1;
        let x2 = x3 - r * alpha * (-s1);
        let y2 = y3 - r * alpha * c1;
        segs.push([x0, y0, x1, y1, x2, y2, x3, y3]);
    }
    segs
}

/// Draw a full or partial ellipse.
fn cs_ellipse(cs: &mut String, e: &crate::entities::Ellipse,
              scale: f64,
              tx: &impl Fn(f64) -> f64,
              ty: &impl Fn(f64) -> f64,
              rgb: (u8, u8, u8), sw: f64) {
    let major_len = e.major_axis_length() * scale;
    let minor_len = e.minor_axis_length() * scale;

    cs_set_stroke(cs, rgb);
    cs_set_lw(cs, sw);

    if e.is_full() {
        // Rotation of the major axis
        let rot = e.major_axis.y.atan2(e.major_axis.x);
        let (sin_r, cos_r) = rot.sin_cos();
        let cx = tx(e.center.x);
        let cy = ty(e.center.y);

        // Use a transformation matrix to draw the ellipse.
        // Save state, apply scale/rotate, draw unit circle, restore.
        writeln!(cs, "q").unwrap();
        writeln!(cs,
            "{:.6} {:.6} {:.6} {:.6} {:.4} {:.4} cm",
            major_len * cos_r, major_len * sin_r,
            -minor_len * sin_r, minor_len * cos_r,
            cx, cy,
        ).unwrap();
        cs_circle_path(cs, 0.0, 0.0, 1.0);
        writeln!(cs, "S Q").unwrap();
    } else {
        // Approximate with 64 line segments.
        let sweep = {
            let s = e.end_parameter - e.start_parameter;
            if s <= 0.0 { s + std::f64::consts::TAU } else { s }
        };
        let n = 64_usize;
        let mut first = true;
        for i in 0..=n {
            let t = e.start_parameter + sweep * (i as f64 / n as f64);
            let pt = ellipse_point(e, t);
            let px = tx(pt.x);
            let py = ty(pt.y);
            if first {
                writeln!(cs, "{:.4} {:.4} m", px, py).unwrap();
                first = false;
            } else {
                writeln!(cs, "{:.4} {:.4} l", px, py).unwrap();
            }
        }
        writeln!(cs, "S").unwrap();
    }
}

/// Draw a POINT entity as a small filled circle.
fn cs_point(cs: &mut String, cx: f64, cy: f64, r: f64, rgb: (u8, u8, u8)) {
    cs_set_fill(cs, rgb);
    cs_circle_path(cs, cx, cy, r);
    writeln!(cs, "f").unwrap();
}

/// Draw a LWPOLYLINE (with bulge support).
fn cs_lwpolyline(cs: &mut String, e: &crate::entities::LwPolyline,
                 scale: f64,
                 tx: &impl Fn(f64) -> f64,
                 ty: &impl Fn(f64) -> f64,
                 rgb: (u8, u8, u8), sw: f64) {
    let n = e.vertices.len();
    if n == 0 {
        return;
    }

    cs_set_stroke(cs, rgb);
    cs_set_lw(cs, sw);

    let seg_count = if e.is_closed { n } else { n - 1 };

    let v0 = &e.vertices[0];
    writeln!(cs, "{:.4} {:.4} m", tx(v0.location.x), ty(v0.location.y)).unwrap();

    for i in 0..seg_count {
        let from = &e.vertices[i];
        let to = &e.vertices[(i + 1) % n];

        if from.bulge.abs() < 1e-9 {
            writeln!(cs, "{:.4} {:.4} l", tx(to.location.x), ty(to.location.y)).unwrap();
        } else {
            // Convert bulge to arc Bézier segments.
            let (segs, is_first_from_start) = bulge_bezier(from, to, scale, tx, ty);
            for (idx, seg) in segs.iter().enumerate() {
                if idx == 0 && !is_first_from_start {
                    writeln!(cs, "{:.4} {:.4} m", seg[0], seg[1]).unwrap();
                }
                writeln!(cs,
                    "{:.4} {:.4} {:.4} {:.4} {:.4} {:.4} c",
                    seg[2], seg[3], seg[4], seg[5], seg[6], seg[7],
                ).unwrap();
            }
        }
    }

    if e.is_closed {
        writeln!(cs, "h").unwrap();
    }
    writeln!(cs, "S").unwrap();
}

/// Convert a bulge-encoded polyline segment to cubic Bézier segments in PDF space.
/// Returns (segments, first_point_is_from_start).
fn bulge_bezier(
    from: &LwVertex,
    to: &LwVertex,
    scale: f64,
    tx: &impl Fn(f64) -> f64,
    ty: &impl Fn(f64) -> f64,
) -> (Vec<[f64; 8]>, bool) {
    let (x1, y1) = (from.location.x, from.location.y);
    let (x2, y2) = (to.location.x, to.location.y);
    let bulge = from.bulge;

    let chord = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
    let half_theta = 2.0 * bulge.abs().atan(); // half of included angle

    if half_theta.sin().abs() < 1e-10 || chord < 1e-10 {
        let segs = vec![[tx(x1), ty(y1), tx(x1), ty(y1), tx(x2), ty(y2), tx(x2), ty(y2)]];
        return (segs, true);
    }

    let r = chord / (2.0 * half_theta.sin());

    // Midpoint of chord
    let mx = (x1 + x2) / 2.0;
    let my = (y1 + y2) / 2.0;

    // Direction perpendicular to chord
    let dx = x2 - x1;
    let dy = y2 - y1;
    let len = (dx * dx + dy * dy).sqrt();
    let perp_x = -dy / len;
    let perp_y = dx / len;

    // Distance from midpoint to center
    // bulge > 0 → CCW → center is to the left of the chord direction
    let d = r * half_theta.cos();
    let sign = if bulge > 0.0 { -1.0 } else { 1.0 };
    let cx = mx + sign * perp_x * d;
    let cy = my + sign * perp_y * d;

    let start_angle = (y1 - cy).atan2(x1 - cx);
    let sweep = 4.0 * bulge.atan(); // signed sweep (negative for CW)

    // Convert center/radius/sweep to PDF Bézier in transformed space
    let cx_pdf = tx(cx);
    let cy_pdf = ty(cy);
    let r_pdf = r * scale;

    // For CW arcs (bulge < 0), adjust the start angle so the arc is drawn
    // in the correct direction; for CCW arcs (bulge > 0), start normally.
    let segs = if bulge < 0.0 {
        arc_bezier_segments(cx_pdf, cy_pdf, r_pdf, start_angle + sweep, (-sweep).abs())
    } else {
        arc_bezier_segments(cx_pdf, cy_pdf, r_pdf, start_angle, sweep.abs())
    };

    (segs, true)
}

/// Compute a point on an ellipse at parameter `t`.
fn ellipse_point(e: &crate::entities::Ellipse, t: f64) -> Vector3 {
    let major = e.major_axis;
    let perp_len = e.major_axis_length() * e.minor_axis_ratio;
    let minor = Vector3::new(-major.y, major.x, 0.0).normalize() * perp_len;
    Vector3::new(
        e.center.x + major.x * t.cos() + minor.x * t.sin(),
        e.center.y + major.y * t.cos() + minor.y * t.sin(),
        e.center.z,
    )
}

/// Draw a text string using the built-in Helvetica font.
fn cs_text(cs: &mut String, x: f64, y: f64, text: &str, size: f64,
           rotation_rad: f64, rgb: (u8, u8, u8)) {
    if text.is_empty() {
        return;
    }
    let escaped = pdf_escape_string(text);
    cs_set_fill(cs, rgb);
    let (sin_r, cos_r) = rotation_rad.sin_cos();
    writeln!(cs, "BT").unwrap();
    writeln!(cs, "/F1 {:.4} Tf", size).unwrap();
    writeln!(cs,
        "{:.6} {:.6} {:.6} {:.6} {:.4} {:.4} Tm",
        cos_r, sin_r, -sin_r, cos_r, x, y,
    ).unwrap();
    writeln!(cs, "({escaped}) Tj").unwrap();
    writeln!(cs, "ET").unwrap();
}

// ─── PDF structure builder ────────────────────────────────────────────────────

/// Build a complete PDF 1.4 document from a content stream string.
fn build_pdf(content: &str, page_w: f64, page_h: f64) -> Result<Vec<u8>> {
    let mut buf: Vec<u8> = Vec::with_capacity(64 * 1024);
    let mut offsets: Vec<usize> = Vec::new(); // byte offset for each object (1-based)

    // PDF binary header (contains 4 bytes > 127 to signal binary content)
    buf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

    // ── Object 1: Catalog ─────────────────────────────────────────────────────
    offsets.push(buf.len());
    buf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    // ── Object 2: Pages ───────────────────────────────────────────────────────
    offsets.push(buf.len());
    buf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    // ── Object 3: Page ────────────────────────────────────────────────────────
    offsets.push(buf.len());
    {
        let page_obj = format!(
            "3 0 obj\n<< /Type /Page /Parent 2 0 R \
/MediaBox [0 0 {:.3} {:.3}] \
/Contents 5 0 R \
/Resources << /Font << /F1 4 0 R >> >> \
>>\nendobj\n",
            page_w, page_h,
        );
        buf.extend_from_slice(page_obj.as_bytes());
    }

    // ── Object 4: Font (Helvetica) ────────────────────────────────────────────
    offsets.push(buf.len());
    buf.extend_from_slice(
        b"4 0 obj\n\
<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica \
/Encoding /WinAnsiEncoding >>\nendobj\n",
    );

    // ── Object 5: Content stream ──────────────────────────────────────────────
    offsets.push(buf.len());
    {
        let cs_bytes = content.as_bytes();
        let header = format!(
            "5 0 obj\n<< /Length {} >>\nstream\n",
            cs_bytes.len()
        );
        buf.extend_from_slice(header.as_bytes());
        buf.extend_from_slice(cs_bytes);
        buf.extend_from_slice(b"\nendstream\nendobj\n");
    }

    // ── Cross-reference table ─────────────────────────────────────────────────
    let xref_offset = buf.len();
    let obj_count = offsets.len() + 1; // +1 for the free entry at index 0

    buf.extend_from_slice(format!("xref\n0 {obj_count}\n").as_bytes());
    // Free entry
    buf.extend_from_slice(b"0000000000 65535 f \n");
    for &off in &offsets {
        buf.extend_from_slice(format!("{:010} 00000 n \n", off).as_bytes());
    }

    // ── Trailer ───────────────────────────────────────────────────────────────
    buf.extend_from_slice(
        format!(
            "trailer\n<< /Size {obj_count} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n"
        )
        .as_bytes(),
    );

    Ok(buf)
}

// ─── String helpers ───────────────────────────────────────────────────────────

/// Escape a string for use in a PDF literal string `(…)`.
fn pdf_escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '(' => out.push_str(r"\("),
            ')' => out.push_str(r"\)"),
            '\\' => out.push_str(r"\\"),
            c if c.is_ascii() => out.push(c),
            // Non-ASCII: use octal escapes for printable Latin-1 range.
            c if (c as u32) < 256 => {
                write!(out, "\\{:03o}", c as u32).unwrap();
            }
            _ => {} // skip characters outside Latin-1
        }
    }
    out
}

/// Strip MTEXT formatting codes (same logic as `SvgWriter`).
fn strip_mtext_codes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => { chars.next(); }
            '{' | '}' => {}
            _ => out.push(c),
        }
    }
    out
}

// ─── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::CadDocument;
    use crate::entities::{Arc, Circle, EntityType, Line, LwPolyline, Point, Text};
    use crate::types::{Vector2, Vector3};

    fn doc_with_line() -> CadDocument {
        let mut doc = CadDocument::new();
        doc.add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 10.0, 5.0, 0.0,
        )))
        .unwrap();
        doc
    }

    #[test]
    fn test_pdf_header() {
        let doc = doc_with_line();
        let bytes = PdfWriter::new(&doc).write_to_vec().unwrap();
        assert!(bytes.starts_with(b"%PDF-1.4"), "Must begin with %PDF-1.4");
    }

    #[test]
    fn test_pdf_eof() {
        let doc = doc_with_line();
        let bytes = PdfWriter::new(&doc).write_to_vec().unwrap();
        let tail = std::str::from_utf8(&bytes[bytes.len().saturating_sub(10)..]).unwrap_or("");
        assert!(tail.contains("%%EOF"), "Must end with %%EOF");
    }

    #[test]
    fn test_pdf_xref() {
        let doc = doc_with_line();
        let bytes = PdfWriter::new(&doc).write_to_vec().unwrap();
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains("xref"), "Must contain xref table");
        assert!(s.contains("startxref"), "Must contain startxref");
    }

    #[test]
    fn test_pdf_circle() {
        let mut doc = CadDocument::new();
        doc.add_entity(EntityType::Circle(Circle::from_coords(5.0, 5.0, 0.0, 3.0)))
            .unwrap();
        let bytes = PdfWriter::new(&doc).write_to_vec().unwrap();
        // The Bézier circle path uses the curveto operator 'c'
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains(" c "), "Circle must use Bézier curves");
    }

    #[test]
    fn test_pdf_arc() {
        let mut doc = CadDocument::new();
        doc.add_entity(EntityType::Arc(Arc::from_coords(
            0.0, 0.0, 0.0, 5.0,
            0.0, std::f64::consts::PI / 2.0,
        )))
        .unwrap();
        let bytes = PdfWriter::new(&doc).write_to_vec().unwrap();
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains(" c\n") || s.contains(" S\n"),
                "Arc must produce stroked path");
    }

    #[test]
    fn test_pdf_point() {
        let mut doc = CadDocument::new();
        doc.add_entity(EntityType::Point(Point::from_coords(1.0, 2.0, 0.0)))
            .unwrap();
        let bytes = PdfWriter::new(&doc).write_to_vec().unwrap();
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains("\nf\n"), "Point must use fill operator");
    }

    #[test]
    fn test_pdf_lwpolyline() {
        let mut doc = CadDocument::new();
        let mut poly = LwPolyline::new();
        poly.add_point(Vector2::new(0.0, 0.0));
        poly.add_point(Vector2::new(10.0, 0.0));
        poly.add_point(Vector2::new(10.0, 10.0));
        doc.add_entity(EntityType::LwPolyline(poly)).unwrap();
        let bytes = PdfWriter::new(&doc).write_to_vec().unwrap();
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains(" m\n") && s.contains(" l\n"),
                "LwPolyline must use moveto/lineto operators");
    }

    #[test]
    fn test_pdf_text() {
        let mut doc = CadDocument::new();
        let text = Text::with_value("Hello PDF", Vector3::new(0.0, 0.0, 0.0));
        doc.add_entity(EntityType::Text(text)).unwrap();
        let bytes = PdfWriter::new(&doc).write_to_vec().unwrap();
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains("BT"), "Text must use BT operator");
        assert!(s.contains("Hello PDF"), "Text content must appear in PDF");
    }

    #[test]
    fn test_pdf_empty_document() {
        let doc = CadDocument::new();
        let bytes = PdfWriter::new(&doc).write_to_vec().unwrap();
        assert!(bytes.starts_with(b"%PDF-1.4"), "Empty doc must still produce PDF");
    }

    #[test]
    fn test_pdf_write_to_file() {
        let doc = doc_with_line();
        let path = std::env::temp_dir().join("acadrust_test_pdf_output.pdf");
        PdfWriter::new(&doc).write_to_file(&path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"%PDF-1.4"));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_pdf_page_sizes() {
        let doc = doc_with_line();
        let bytes = PdfWriter::new(&doc)
            .with_page_size(PdfPageSize::LETTER)
            .write_to_vec()
            .unwrap();
        let s = String::from_utf8_lossy(&bytes);
        assert!(s.contains("612"), "Letter page must reference 612 pt width");
    }

    #[test]
    fn test_pdf_escape_string() {
        assert_eq!(pdf_escape_string("hello (world)"), r"hello \(world\)");
        assert_eq!(pdf_escape_string(r"back\slash"), r"back\\slash");
    }

    #[test]
    fn test_strip_mtext_codes() {
        assert_eq!(strip_mtext_codes(r"\P"), "");
        assert_eq!(strip_mtext_codes("{hello}"), "hello");
        assert_eq!(strip_mtext_codes(r"hello\"), "hello");
    }
}
