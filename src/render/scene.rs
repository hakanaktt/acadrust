//! Display list produced by [`compile`](super::compile).

use crate::types::{Handle, Vector2, Vector3};

use super::visual_style::ResolvedVisualStyle;

/// Where a primitive came from. The insert path is outermost first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityRef {
    pub handle: Handle,
    pub insert_path: Vec<Handle>,
}

/// Whether this primitive is the entity's real picture or a stand-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawPath {
    /// The arm that will remain for this entity kind.
    Dedicated,
    /// Extents stand-in. The kind stays unfinished until this disappears.
    Fallback,
}

/// Resolved stroke appearance. Dash lengths are already multiplied by
/// `LTSCALE` and the entity linetype scale, before alignment fitting.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub layer: String,
    /// `true` when the resolved ACI is 7. The pixel is the contrast color
    /// against the view background. `rgb` is the dark-background value.
    pub aci7: bool,
    pub rgb: [u8; 3],
    /// 0 is fully transparent, 1 is opaque.
    pub opacity: f32,
    /// Millimetres. Zero when the stroke is a one-pixel cosmetic hairline.
    pub lineweight_mm: f64,
    pub cosmetic: bool,
    /// Paper-space width. The millimetres become drawing units on the sheet
    /// and grow with zoom. Model space leaves this false and uses pixels.
    pub sheet: bool,
}

/// A filled triangle list. `triangles.len()` is a multiple of three.
#[derive(Debug, Clone, PartialEq)]
pub struct Fill {
    pub triangles: Vec<Vector3>,
    pub style: Style,
    pub source: EntityRef,
    pub path: DrawPath,
    /// Paint the view background. Wipeouts use this so they cover earlier fills.
    pub background: bool,
    /// One RGB triple per triangle vertex. Empty means `style` colors the fill.
    pub vertex_rgb: Vec<[u8; 3]>,
    /// One world-space normal per triangle vertex. Empty means the fill is
    /// unlit, which is how hatches and wipeouts stay flat.
    pub normals: Vec<Vector3>,
}

/// One solid run of a stroked curve, after linetype fitting.
#[derive(Debug, Clone, PartialEq)]
pub struct Stroke {
    pub points: Vec<Vector3>,
    pub closed: bool,
    /// A zero-length mark from a linetype dot.
    pub dot: bool,
    pub style: Style,
    pub source: EntityRef,
    pub path: DrawPath,
}

/// A graphical entity that does not yet have its dedicated arm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackEntry {
    pub kind: String,
    pub source: EntityRef,
}

/// Something AutoCAD would also omit, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkippedEntity {
    pub kind: String,
    pub reason: String,
    pub source: EntityRef,
}

/// A TrueType run. The atlas is built when the scene is prepared for a view.
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphRun {
    pub text: String,
    pub anchor: Vector3,
    pub rotation: f64,
    pub height: f64,
    pub width_factor: f64,
    pub horizontal: u8,
    pub vertical: u8,
    pub normal: Vector3,
    pub font_path: std::path::PathBuf,
    pub rgb: [u8; 3],
    pub aci7: bool,
    pub opacity: f32,
    pub source: EntityRef,
}

/// A decoded raster placed on a quad, or on the clip polygon's triangles.
/// `uvs` matches `triangles` one to one. Texture v of 0 is the top row.
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub triangles: Vec<Vector3>,
    pub uvs: Vec<[f32; 2]>,
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub opacity: f32,
    pub source: EntityRef,
}

/// Camera-independent display list.
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub strokes: Vec<Stroke>,
    pub fills: Vec<Fill>,
    pub glyphs: Vec<GlyphRun>,
    pub pictures: Vec<Picture>,
    pub fallbacks: Vec<FallbackEntry>,
    pub skipped: Vec<SkippedEntity>,
    pub visual_style: ResolvedVisualStyle,
    /// Provisional model-space pixels per millimetre of lineweight.
    /// Replaced when the AutoCAD lineweight chart is calibrated.
    pub model_lineweight_px_per_mm: f64,
    /// Drawing units per millimetre of lineweight. Zero in model space.
    pub sheet_units_per_mm: f64,
}

impl Scene {
    pub fn bounds_min_max(&self) -> Option<(Vector3, Vector3)> {
        let mut min = Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut max = Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        let mut any = false;
        for stroke in &self.strokes {
            for point in &stroke.points {
                any = true;
                min.x = min.x.min(point.x);
                min.y = min.y.min(point.y);
                min.z = min.z.min(point.z);
                max.x = max.x.max(point.x);
                max.y = max.y.max(point.y);
                max.z = max.z.max(point.z);
            }
        }
        for fill in &self.fills {
            for point in &fill.triangles {
                any = true;
                include(&mut min, &mut max, *point);
            }
        }
        for picture in &self.pictures {
            for point in &picture.triangles {
                any = true;
                include(&mut min, &mut max, *point);
            }
        }
        for glyph in &self.glyphs {
            any = true;
            include(&mut min, &mut max, glyph.anchor);
            include(
                &mut min,
                &mut max,
                Vector3::new(
                    glyph.anchor.x + glyph.height,
                    glyph.anchor.y + glyph.height,
                    glyph.anchor.z,
                ),
            );
        }
        any.then_some((min, max))
    }

    /// Kinds that contributed at least one fallback primitive.
    pub fn fallback_kinds(&self) -> Vec<&str> {
        let mut kinds: Vec<&str> = self
            .fallbacks
            .iter()
            .map(|entry| entry.kind.as_str())
            .collect();
        kinds.sort_unstable();
        kinds.dedup();
        kinds
    }

    /// World point nearest a stroke, within `tolerance`.
    pub fn pick(&self, point: Vector2, tolerance: f64) -> Option<&EntityRef> {
        let mut best: Option<(&EntityRef, f64)> = None;
        let limit = tolerance.max(0.0);
        for stroke in &self.strokes {
            if stroke.points.len() < 2 {
                continue;
            }
            for pair in stroke.points.windows(2) {
                let distance = segment_distance(point, pair[0], pair[1]);
                if distance <= limit && best.is_none_or(|(_, best_d)| distance < best_d) {
                    best = Some((&stroke.source, distance));
                }
            }
        }
        best.map(|(source, _)| source)
    }
}

fn include(min: &mut Vector3, max: &mut Vector3, point: Vector3) {
    min.x = min.x.min(point.x);
    min.y = min.y.min(point.y);
    min.z = min.z.min(point.z);
    max.x = max.x.max(point.x);
    max.y = max.y.max(point.y);
    max.z = max.z.max(point.z);
}

fn segment_distance(point: Vector2, start: Vector3, end: Vector3) -> f64 {
    let ax = start.x;
    let ay = start.y;
    let bx = end.x;
    let by = end.y;
    let dx = bx - ax;
    let dy = by - ay;
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= f64::EPSILON {
        0.0
    } else {
        (((point.x - ax) * dx + (point.y - ay) * dy) / len2).clamp(0.0, 1.0)
    };
    let x = ax + dx * t - point.x;
    let y = ay + dy * t - point.y;
    (x * x + y * y).sqrt()
}
