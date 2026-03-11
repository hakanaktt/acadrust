//! SVG export for [`CadDocument`].
//!
//! Converts a CAD drawing to a Scalable Vector Graphics (SVG) document,
//! supporting the most common entity types.
//!
//! # Coordinate system
//!
//! DXF uses a right-hand coordinate system with Y pointing up, while SVG uses
//! Y pointing down.  The writer automatically flips the Y-axis so that the
//! exported SVG appears correctly oriented.
//!
//! # Colour handling
//!
//! `ByLayer` colours are resolved against the document's layer table.  `ByBlock`
//! falls back to black.  AutoCAD Color Index values are converted using the
//! standard 256-colour ACI palette.  Because SVG files typically have a white
//! background, ACI colour 7 ("White") is remapped to black so that lines remain
//! visible.
//!
//! # Supported entities
//!
//! | DXF entity   | SVG element                                      |
//! |--------------|--------------------------------------------------|
//! | `LINE`       | `<line>`                                         |
//! | `CIRCLE`     | `<circle>`                                       |
//! | `ARC`        | `<path>` (SVG arc)                               |
//! | `ELLIPSE`    | `<ellipse>` (full) / `<path>` (partial)          |
//! | `LWPOLYLINE` | `<polyline>` / `<polygon>` / `<path>` (w/ bulge) |
//! | `POINT`      | small filled `<circle>`                          |
//! | `TEXT`       | `<text>`                                         |
//! | `MTEXT`      | `<text>` (formatting codes stripped)             |
//!
//! Entities on frozen or invisible layers, and entities whose `invisible` flag
//! is set, are omitted from the output.
//!
//! # Example
//!
//! ```rust,no_run
//! use acadrust::{CadDocument, EntityType};
//! use acadrust::entities::Line;
//! use acadrust::io::svg::SvgWriter;
//!
//! let mut doc = CadDocument::new();
//! doc.add_entity(EntityType::Line(
//!     Line::from_coords(0.0, 0.0, 0.0, 100.0, 50.0, 0.0),
//! ))
//! .unwrap();
//!
//! SvgWriter::new(&doc).write_to_file("output.svg").unwrap();
//! ```

use std::fmt::Write as FmtWrite;
use std::fs;
use std::io::Write as IoWrite;
use std::path::Path;

use crate::document::CadDocument;
use crate::entities::{EntityType, LwVertex};
use crate::error::Result;
use crate::types::{BoundingBox3D, Color, Vector3};

// ─── ACI colour palette ──────────────────────────────────────────────────────

/// Convert an AutoCAD Color Index (ACI) value to an sRGB triple.
///
/// Follows the standard AutoCAD 256-colour palette.
fn aci_to_rgb(index: u8) -> (u8, u8, u8) {
    match index {
        0 => (0, 0, 0),         // ByBlock – fall back to black
        1 => (255, 0, 0),       // Red
        2 => (255, 255, 0),     // Yellow
        3 => (0, 255, 0),       // Green
        4 => (0, 255, 255),     // Cyan
        5 => (0, 0, 255),       // Blue
        6 => (255, 0, 255),     // Magenta
        7 => (255, 255, 255),   // White (caller handles white-on-white)
        8 => (128, 128, 128),   // Dark Gray
        9 => (192, 192, 192),   // Light Gray
        10..=249 => {
            // 240 hue-based colours in 24 groups of 10.
            // Each group shares a hue (spaced 15° apart); positions 0-9 decrease
            // saturation and value.
            let group = (index - 10) / 10; // 0-23
            let pos = (index - 10) % 10;   // 0-9
            let hue = f64::from(group) * 15.0;
            let (sat, val): (f64, f64) = match pos {
                0 => (1.00, 1.00),
                1 => (0.33, 1.00),
                2 => (1.00, 0.74),
                3 => (0.33, 0.74),
                4 => (1.00, 0.50),
                5 => (0.33, 0.50),
                6 => (1.00, 0.30),
                7 => (0.33, 0.30),
                8 => (1.00, 0.15),
                9 => (0.33, 0.15),
                _ => unreachable!(),
            };
            hsv_to_rgb(hue, sat, val)
        }
        250 => (51, 51, 51),
        251 => (102, 102, 102),
        252 => (153, 153, 153),
        253 => (204, 204, 204),
        254 => (229, 229, 229),
        255 => (255, 255, 255),
    }
}

/// Convert HSV (hue 0–360°, saturation 0–1, value 0–1) to an sRGB triple.
fn hsv_to_rgb(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let h = h.rem_euclid(360.0);
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0).rem_euclid(2.0) - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    )
}

// ─── SvgWriter ───────────────────────────────────────────────────────────────

/// Exports a [`CadDocument`] to SVG format.
///
/// See the [module documentation](self) for the list of supported entities
/// and detailed usage notes.
pub struct SvgWriter<'a> {
    document: &'a CadDocument,
    /// Extra space around the drawing extent, as a fraction of the larger dimension.
    padding_factor: f64,
    /// Fixed stroke width in model units (`None` = auto: 0.5 % of larger dimension).
    stroke_width: Option<f64>,
    /// Fixed point radius in model units (`None` = auto: 0.4 % of larger dimension).
    point_radius: Option<f64>,
}

impl<'a> SvgWriter<'a> {
    /// Create a new `SvgWriter` for the given document.
    pub fn new(document: &'a CadDocument) -> Self {
        SvgWriter {
            document,
            padding_factor: 0.02,
            stroke_width: None,
            point_radius: None,
        }
    }

    /// Set the padding factor (fraction of the larger dimension, default `0.02` = 2 %).
    pub fn with_padding(mut self, factor: f64) -> Self {
        self.padding_factor = factor;
        self
    }

    /// Override the stroke width in model units.
    pub fn with_stroke_width(mut self, width: f64) -> Self {
        self.stroke_width = Some(width);
        self
    }

    /// Override the radius used for POINT entities in model units.
    pub fn with_point_radius(mut self, radius: f64) -> Self {
        self.point_radius = Some(radius);
        self
    }

    /// Write the SVG to a file at `path`.
    pub fn write_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let svg = self.write_to_string()?;
        fs::write(path, svg)?;
        Ok(())
    }

    /// Write the SVG to an arbitrary [`Write`](std::io::Write) sink.
    pub fn write_to_writer<W: IoWrite>(&self, writer: &mut W) -> Result<()> {
        let svg = self.write_to_string()?;
        writer.write_all(svg.as_bytes())?;
        Ok(())
    }

    /// Render the document to an SVG [`String`].
    pub fn write_to_string(&self) -> Result<String> {
        // ── bounding box ──────────────────────────────────────────────────────
        let bounds = self.compute_bounds();
        let (min_x, min_y, max_x, max_y) = match bounds {
            Some(b) => (b.min.x, b.min.y, b.max.x, b.max.y),
            None => (0.0, 0.0, 100.0, 100.0),
        };

        let w = (max_x - min_x).max(1e-6);
        let h = (max_y - min_y).max(1e-6);
        let larger = w.max(h);
        let pad = larger * self.padding_factor;

        let stroke_w = self.stroke_width.unwrap_or(larger * 0.005);
        let pt_r = self.point_radius.unwrap_or(larger * 0.004);

        // SVG viewBox: after flipping Y (svg_y = -dxf_y), the DXF extent
        // [min_y, max_y] maps to SVG extent [-max_y, -min_y].
        let vb_x = min_x - pad;
        let vb_y = -(max_y + pad);
        let vb_w = w + 2.0 * pad;
        let vb_h = h + 2.0 * pad;

        // ── start SVG ─────────────────────────────────────────────────────────
        let mut svg = String::with_capacity(64 * 1024);
        writeln!(svg, r#"<?xml version="1.0" encoding="UTF-8"?>"#).unwrap();
        writeln!(
            svg,
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{vb_x:.6} {vb_y:.6} {vb_w:.6} {vb_h:.6}">"#
        )
        .unwrap();

        // White background rectangle
        writeln!(
            svg,
            r#"  <rect x="{vb_x:.6}" y="{vb_y:.6}" width="{vb_w:.6}" height="{vb_h:.6}" fill="white"/>"#
        )
        .unwrap();

        // ── entities ──────────────────────────────────────────────────────────
        for entity in self.document.entities() {
            let common = entity.as_entity();
            if common.is_invisible() || self.is_layer_hidden(common.layer()) {
                continue;
            }

            let rgb = self.resolve_color(common.color(), common.layer());
            // Remap pure white to black so it stays visible on the white background.
            let rgb = if rgb == (255, 255, 255) { (0, 0, 0) } else { rgb };
            let stroke_css = format!("#{:02X}{:02X}{:02X}", rgb.0, rgb.1, rgb.2);

            match entity {
                EntityType::Line(e) => write_line(&mut svg, e, &stroke_css, stroke_w),
                EntityType::Circle(e) => write_circle(&mut svg, e, &stroke_css, stroke_w),
                EntityType::Arc(e) => write_arc(&mut svg, e, &stroke_css, stroke_w),
                EntityType::Ellipse(e) => write_ellipse(&mut svg, e, &stroke_css, stroke_w),
                EntityType::LwPolyline(e) => {
                    write_lwpolyline(&mut svg, e, &stroke_css, stroke_w)
                }
                EntityType::Point(e) => write_point(&mut svg, e, &stroke_css, pt_r),
                EntityType::Text(e) => write_text(&mut svg, e, &stroke_css),
                EntityType::MText(e) => write_mtext(&mut svg, e, &stroke_css),
                _ => {} // entity type not yet supported in SVG export
            }
        }

        writeln!(svg, "</svg>").unwrap();
        Ok(svg)
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    /// Compute the 3D bounding box of all visible entities.
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

    /// Returns `true` when the named layer is frozen or switched off.
    fn is_layer_hidden(&self, name: &str) -> bool {
        self.document
            .layers
            .get(name)
            .map(|l| l.flags.frozen || l.flags.off)
            .unwrap_or(false)
    }

    /// Resolve a [`Color`] to an sRGB triple, following the layer chain.
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

// ─── Y-axis helper ────────────────────────────────────────────────────────────

/// Flip the Y coordinate: DXF is Y-up; SVG is Y-down.
#[inline]
fn fy(y: f64) -> f64 {
    -y
}

// ─── per-entity SVG writers ──────────────────────────────────────────────────

fn write_line(svg: &mut String, e: &crate::entities::Line, stroke: &str, sw: f64) {
    writeln!(
        svg,
        r#"  <line x1="{:.6}" y1="{:.6}" x2="{:.6}" y2="{:.6}" stroke="{stroke}" stroke-width="{sw:.6}" fill="none"/>"#,
        e.start.x,
        fy(e.start.y),
        e.end.x,
        fy(e.end.y),
    )
    .unwrap();
}

fn write_circle(svg: &mut String, e: &crate::entities::Circle, stroke: &str, sw: f64) {
    writeln!(
        svg,
        r#"  <circle cx="{:.6}" cy="{:.6}" r="{:.6}" stroke="{stroke}" stroke-width="{sw:.6}" fill="none"/>"#,
        e.center.x,
        fy(e.center.y),
        e.radius,
    )
    .unwrap();
}

fn write_arc(svg: &mut String, e: &crate::entities::Arc, stroke: &str, sw: f64) {
    let sweep = e.sweep_angle();

    // Near-full circles become <circle> to avoid degenerate paths.
    if sweep >= 2.0 * std::f64::consts::PI - 1e-6 {
        write_circle(
            svg,
            &crate::entities::Circle {
                common: e.common.clone(),
                center: e.center,
                radius: e.radius,
                thickness: e.thickness,
                normal: e.normal,
            },
            stroke,
            sw,
        );
        return;
    }

    let start = e.start_point();
    let end = e.end_point();

    let large_arc = if sweep > std::f64::consts::PI { 1 } else { 0 };
    // DXF arcs are CCW (Y-up).  After flipping Y they become CW in SVG.
    // SVG sweep-flag = 1 means CW (positive-angle direction in Y-down coords).
    writeln!(
        svg,
        r#"  <path d="M {:.6} {:.6} A {:.6} {:.6} 0 {large_arc} 1 {:.6} {:.6}" stroke="{stroke}" stroke-width="{sw:.6}" fill="none"/>"#,
        start.x,
        fy(start.y),
        e.radius,
        e.radius,
        end.x,
        fy(end.y),
    )
    .unwrap();
}

fn write_ellipse(svg: &mut String, e: &crate::entities::Ellipse, stroke: &str, sw: f64) {
    let major_len = e.major_axis_length();
    let minor_len = e.minor_axis_length();
    // Rotation of the major axis in DXF degrees; negate for Y-flip.
    let rot_deg = -e.major_axis.y.atan2(e.major_axis.x).to_degrees();

    if e.is_full() {
        writeln!(
            svg,
            r#"  <ellipse cx="{:.6}" cy="{:.6}" rx="{:.6}" ry="{:.6}" transform="rotate({rot_deg:.6} {:.6} {:.6})" stroke="{stroke}" stroke-width="{sw:.6}" fill="none"/>"#,
            e.center.x,
            fy(e.center.y),
            major_len,
            minor_len,
            e.center.x,
            fy(e.center.y),
        )
        .unwrap();
    } else {
        // Partial ellipse: approximate with a polyline of 64 segments.
        let seg = 64_usize;
        let mut d = String::new();
        let sweep = {
            let s = e.end_parameter - e.start_parameter;
            if s <= 0.0 {
                s + 2.0 * std::f64::consts::TAU
            } else {
                s
            }
        };
        for i in 0..=seg {
            let t = e.start_parameter + sweep * (i as f64 / seg as f64);
            let pt = ellipse_point(e, t);
            if i == 0 {
                write!(d, "M {:.6} {:.6}", pt.x, fy(pt.y)).unwrap();
            } else {
                write!(d, " L {:.6} {:.6}", pt.x, fy(pt.y)).unwrap();
            }
        }
        writeln!(
            svg,
            r#"  <path d="{d}" stroke="{stroke}" stroke-width="{sw:.6}" fill="none"/>"#,
        )
        .unwrap();
    }
}

/// Compute a point on the ellipse at parameter `t`.
fn ellipse_point(e: &crate::entities::Ellipse, t: f64) -> Vector3 {
    let major = e.major_axis;
    // Perpendicular in the XY plane, scaled by the minor-to-major ratio.
    let perp_len = e.major_axis_length() * e.minor_axis_ratio;
    let minor = Vector3::new(-major.y, major.x, 0.0).normalize() * perp_len;
    Vector3::new(
        e.center.x + major.x * t.cos() + minor.x * t.sin(),
        e.center.y + major.y * t.cos() + minor.y * t.sin(),
        e.center.z,
    )
}

fn write_lwpolyline(
    svg: &mut String,
    e: &crate::entities::LwPolyline,
    stroke: &str,
    sw: f64,
) {
    let n = e.vertices.len();
    if n == 0 {
        return;
    }

    let has_bulge = e.vertices.iter().any(|v| v.bulge.abs() > 1e-9);

    if !has_bulge {
        // Simple polyline or closed polygon.
        let mut pts = String::new();
        for v in &e.vertices {
            write!(pts, "{:.6},{:.6} ", v.location.x, fy(v.location.y)).unwrap();
        }
        let tag = if e.is_closed { "polygon" } else { "polyline" };
        writeln!(
            svg,
            r#"  <{tag} points="{}" stroke="{stroke}" stroke-width="{sw:.6}" fill="none"/>"#,
            pts.trim_end(),
        )
        .unwrap();
    } else {
        // Build a path, converting bulge values to SVG arc segments.
        let mut d = String::new();
        write!(
            d,
            "M {:.6} {:.6}",
            e.vertices[0].location.x,
            fy(e.vertices[0].location.y)
        )
        .unwrap();

        let seg_count = if e.is_closed { n } else { n - 1 };
        for i in 0..seg_count {
            let from = &e.vertices[i];
            let to = &e.vertices[(i + 1) % n];
            if from.bulge.abs() < 1e-9 {
                write!(d, " L {:.6} {:.6}", to.location.x, fy(to.location.y)).unwrap();
            } else {
                d.push_str(&bulge_arc_segment(from, to));
            }
        }
        if e.is_closed {
            d.push('Z');
        }
        writeln!(
            svg,
            r#"  <path d="{d}" stroke="{stroke}" stroke-width="{sw:.6}" fill="none"/>"#,
        )
        .unwrap();
    }
}

/// Build a single SVG arc path segment from a bulge-encoded LwPolyline edge.
fn bulge_arc_segment(from: &LwVertex, to: &LwVertex) -> String {
    let (x1, y1) = (from.location.x, from.location.y);
    let (x2, y2) = (to.location.x, to.location.y);
    let bulge = from.bulge;

    // Chord length.
    let chord = ((x2 - x1).powi(2) + (y2 - y1).powi(2)).sqrt();
    // Central (included) angle: θ = 4 · atan(|bulge|).
    let half_theta = 2.0 * bulge.abs().atan(); // = θ/2
    let r = chord / (2.0 * half_theta.sin().max(1e-10));

    let large_arc = if bulge.abs() > 1.0 { 1 } else { 0 };
    // bulge > 0 → CCW in DXF (Y-up) → CW in SVG (Y-down) → sweep-flag = 1.
    // bulge < 0 → CW in DXF → CCW in SVG → sweep-flag = 0.
    let sweep = if bulge > 0.0 { 1 } else { 0 };

    format!(
        " A {r:.6} {r:.6} 0 {large_arc} {sweep} {:.6} {:.6}",
        x2,
        fy(y2),
    )
}

fn write_point(svg: &mut String, e: &crate::entities::Point, fill: &str, r: f64) {
    writeln!(
        svg,
        r#"  <circle cx="{:.6}" cy="{:.6}" r="{r:.6}" fill="{fill}"/>"#,
        e.location.x,
        fy(e.location.y),
    )
    .unwrap();
}

fn write_text(svg: &mut String, e: &crate::entities::Text, fill: &str) {
    let h = e.height.max(0.01);
    let x = e.insertion_point.x;
    let y = fy(e.insertion_point.y);
    // Negate the rotation because we flipped the Y-axis.
    let rot = -e.rotation.to_degrees();
    let content = escape_xml(&e.value);
    writeln!(
        svg,
        r#"  <text x="{x:.6}" y="{y:.6}" font-size="{h:.6}" fill="{fill}" transform="rotate({rot:.6} {x:.6} {y:.6})">{content}</text>"#,
    )
    .unwrap();
}

fn write_mtext(svg: &mut String, e: &crate::entities::MText, fill: &str) {
    let h = e.height.max(0.01);
    let x = e.insertion_point.x;
    let y = fy(e.insertion_point.y);
    let content = escape_xml(&strip_mtext_codes(&e.value));
    writeln!(
        svg,
        r#"  <text x="{x:.6}" y="{y:.6}" font-size="{h:.6}" fill="{fill}">{content}</text>"#,
    )
    .unwrap();
}

// ─── text helpers ─────────────────────────────────────────────────────────────

/// Escape special XML characters.
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Strip MTEXT inline formatting codes (e.g. `\P`, `\f{...}`, `{…}`).
fn strip_mtext_codes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                // Consume the format-code character that follows the backslash.
                chars.next();
            }
            '{' | '}' => {
                // Skip group delimiters used by MTEXT formatting.
            }
            _ => out.push(c),
        }
    }
    out
}

// ─── tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::CadDocument;
    use crate::entities::{Arc, Circle, EntityType, Line, LwPolyline, Point};
    use crate::types::Vector2;

    fn doc_with_line() -> CadDocument {
        let mut doc = CadDocument::new();
        doc.add_entity(EntityType::Line(Line::from_coords(
            0.0, 0.0, 0.0, 10.0, 5.0, 0.0,
        )))
        .unwrap();
        doc
    }

    #[test]
    fn test_svg_contains_svg_tag() {
        let doc = doc_with_line();
        let svg = SvgWriter::new(&doc).write_to_string().unwrap();
        assert!(svg.contains("<svg"), "SVG output must start with an <svg> element");
        assert!(svg.contains("</svg>"), "SVG output must end with </svg>");
    }

    #[test]
    fn test_svg_contains_line() {
        let doc = doc_with_line();
        let svg = SvgWriter::new(&doc).write_to_string().unwrap();
        assert!(svg.contains("<line"), "SVG must contain a <line> element");
    }

    #[test]
    fn test_svg_circle() {
        let mut doc = CadDocument::new();
        doc.add_entity(EntityType::Circle(Circle::from_coords(5.0, 5.0, 0.0, 3.0)))
            .unwrap();
        let svg = SvgWriter::new(&doc).write_to_string().unwrap();
        assert!(svg.contains("<circle"), "SVG must contain a <circle> element");
    }

    #[test]
    fn test_svg_arc() {
        let mut doc = CadDocument::new();
        doc.add_entity(EntityType::Arc(Arc::from_coords(
            0.0,
            0.0,
            0.0,
            5.0,
            0.0,
            std::f64::consts::PI / 2.0,
        )))
        .unwrap();
        let svg = SvgWriter::new(&doc).write_to_string().unwrap();
        assert!(
            svg.contains("<path") || svg.contains("<circle"),
            "SVG must contain a path or circle for ARC"
        );
    }

    #[test]
    fn test_svg_point() {
        let mut doc = CadDocument::new();
        doc.add_entity(EntityType::Point(Point::from_coords(1.0, 2.0, 0.0)))
            .unwrap();
        let svg = SvgWriter::new(&doc).write_to_string().unwrap();
        assert!(svg.contains("<circle"), "SVG must contain a <circle> for POINT");
    }

    #[test]
    fn test_svg_lwpolyline() {
        let mut doc = CadDocument::new();
        let mut poly = LwPolyline::new();
        poly.add_point(Vector2::new(0.0, 0.0));
        poly.add_point(Vector2::new(10.0, 0.0));
        poly.add_point(Vector2::new(10.0, 10.0));
        doc.add_entity(EntityType::LwPolyline(poly)).unwrap();
        let svg = SvgWriter::new(&doc).write_to_string().unwrap();
        assert!(
            svg.contains("<polyline") || svg.contains("<polygon"),
            "SVG must contain a polyline or polygon element"
        );
    }

    #[test]
    fn test_empty_document() {
        let doc = CadDocument::new();
        let svg = SvgWriter::new(&doc).write_to_string().unwrap();
        assert!(svg.contains("<svg"), "Empty document must still produce an SVG");
    }

    #[test]
    fn test_write_to_file() {
        let doc = doc_with_line();
        let path = std::env::temp_dir().join("acadrust_test_svg_output.svg");
        SvgWriter::new(&doc).write_to_file(&path).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        assert!(contents.contains("<line"), "Written SVG file must contain a <line>");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_aci_to_rgb_basic() {
        assert_eq!(aci_to_rgb(1), (255, 0, 0)); // Red
        assert_eq!(aci_to_rgb(3), (0, 255, 0)); // Green
        assert_eq!(aci_to_rgb(5), (0, 0, 255)); // Blue
    }

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("a&b<c>d\"e"), "a&amp;b&lt;c&gt;d&quot;e");
    }

    #[test]
    fn test_strip_mtext_codes() {
        assert_eq!(strip_mtext_codes(r"\P"), "");
        assert_eq!(strip_mtext_codes("{hello}"), "hello");
    }
}
