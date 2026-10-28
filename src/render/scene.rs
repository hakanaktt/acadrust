//! Display list produced by [`compile`](super::compile).

use crate::types::{Handle, Vector2, Vector3};

use super::visual_style::ResolvedVisualStyle;

/// Where a primitive came from. The insert path is outermost first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityRef {
    pub handle: Handle,
    pub insert_path: Vec<Handle>,
}

impl EntityRef {
    /// Handle of the entity a user selects: the outermost insert when the
    /// primitive belongs to a block reference, otherwise the entity itself.
    pub fn selection_handle(&self) -> Handle {
        self.insert_path.first().copied().unwrap_or(self.handle)
    }

    /// True when `handle` selects this primitive, either directly or through
    /// an enclosing insert.
    pub fn selected_by(&self, handle: Handle) -> bool {
        handle == self.handle || handle == self.selection_handle()
    }
}

/// Whether this primitive is the entity's real picture or a stand-in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawPath {
    /// The arm that will remain for this entity kind.
    Dedicated,
    /// Extents stand-in. The kind stays unfinished until this disappears.
    Fallback,
    /// A ray or construction line rendered to a finite view-sized reach.
    ///
    /// The endpoints are intentionally excluded from scene extents because
    /// they are synthetic and must not dominate `View::zoom_extents`.
    Infinite,
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

/// A closed analytic surface that the GPU can evaluate per fragment.
///
/// The matching [`Fill`] still carries the CPU triangles so extents, picking,
/// clipping, and headless callers keep working. The renderer draws this
/// descriptor instead when it can, which keeps the silhouette and shading
/// independent of the compile-time chord tolerance and of zoom.
#[derive(Debug, Clone, PartialEq)]
pub enum CurvedSurface {
    /// A sphere centered at `center`.
    Sphere {
        center: Vector3,
        radius: f64,
        flip: bool,
    },
    /// A torus around `axis` with `east`/`north` spanning the ring plane.
    Torus {
        center: Vector3,
        axis: Vector3,
        east: Vector3,
        north: Vector3,
        major: f64,
        minor: f64,
        flip: bool,
    },
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
    pub lighting: super::visual_style::LightingModel,
    /// When set, the triangles are only the fallback mesh. The GPU evaluates
    /// this surface analytically, so zoomed silhouettes stay smooth.
    pub analytic: Option<CurvedSurface>,
}

/// Analytic piece drawn instead of the chord samples in [`Stroke::points`].
///
/// `points` stays populated so extents, linetype text, and existing callers
/// still see a polyline. The renderer paints `curves` when the list is not
/// empty, so the pixels follow the curve rather than those chords.
#[derive(Debug, Clone, PartialEq)]
pub enum Curve {
    /// Straight segment.
    Line { start: Vector3, end: Vector3 },
    /// `center + u cos t + v sin t` for `t` in `[start, start + sweep]`.
    /// A full turn uses a sweep of ±τ.
    Elliptic {
        center: Vector3,
        u: Vector3,
        v: Vector3,
        start: f64,
        sweep: f64,
    },
    /// Rational cubic Bézier. Weights of 1 are an ordinary polynomial cubic.
    Cubic {
        p0: Vector3,
        p1: Vector3,
        p2: Vector3,
        p3: Vector3,
        weights: [f64; 4],
    },
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
    /// Analytic geometry. Empty means `points` are straight segments.
    pub curves: Vec<Curve>,
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
    /// World-space unit vectors for the text's local X/Y axes. These preserve
    /// insert and viewport transforms, including non-uniform scale.
    pub axis_x: Vector3,
    pub axis_y: Vector3,
    pub oblique: f64,
    pub backward: bool,
    pub upside_down: bool,
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
            if stroke.path == DrawPath::Infinite {
                continue;
            }
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
            let width = glyph.height
                * glyph.width_factor.abs()
                * glyph.text.chars().count().max(1) as f64
                * 0.6;
            for point in [
                glyph.anchor + glyph.axis_x * width,
                glyph.anchor + glyph.axis_x * width + glyph.axis_y * glyph.height,
                glyph.anchor + glyph.axis_y * glyph.height,
            ] {
                include(&mut min, &mut max, point);
            }
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

    /// Entity nearest a viewport pixel.
    ///
    /// Strokes are measured by screen distance, filled primitives by whether
    /// the pixel lies inside them, and text by its run box. Ties go to the
    /// primitive closest to the camera, so an edge lying on a selected face
    /// wins over the face itself.
    pub fn pick_screen(
        &self,
        view: &super::View,
        viewport_px: (u32, u32),
        x: f64,
        y: f64,
        tolerance_px: f64,
    ) -> Option<&EntityRef> {
        let tolerance = tolerance_px.max(0.0);
        let width = viewport_px.0.max(1);
        let height = viewport_px.1.max(1);
        let mut best: Option<(&EntityRef, f64, f64)> = None;

        for stroke in &self.strokes {
            if stroke.curves.is_empty() {
                pick_segments(
                    &mut best,
                    &stroke.source,
                    stroke.points.windows(2).map(|pair| (pair[0], pair[1])),
                    view,
                    (width, height),
                    x,
                    y,
                    tolerance,
                );
            } else {
                for curve in &stroke.curves {
                    let samples = super::curves::sample_curve(curve, 32);
                    pick_segments(
                        &mut best,
                        &stroke.source,
                        samples.windows(2).map(|pair| (pair[0], pair[1])),
                        view,
                        (width, height),
                        x,
                        y,
                        tolerance,
                    );
                }
            }
        }

        for fill in &self.fills {
            for triangle in fill.triangles.chunks_exact(3) {
                let a = view.project(triangle[0], width, height);
                let b = view.project(triangle[1], width, height);
                let c = view.project(triangle[2], width, height);
                if point_in_triangle(x, y, &a, &b, &c) {
                    let depth = a.depth.max(b.depth).max(c.depth);
                    consider_pick(&mut best, &fill.source, 0.0, depth, tolerance);
                }
            }
        }

        for glyph in &self.glyphs {
            let characters = glyph.text.chars().count().max(1) as f64;
            let run_width = glyph.height * glyph.width_factor.abs() * characters * 0.6;
            let (start, end) = match glyph.horizontal {
                1 | 4 => (-run_width * 0.5, run_width * 0.5),
                2 => (-run_width, 0.0),
                _ => (0.0, run_width),
            };
            let baseline = glyph.anchor + glyph.axis_x * start;
            let corners = [
                baseline,
                baseline + glyph.axis_x * (end - start),
                baseline + glyph.axis_x * (end - start) + glyph.axis_y * glyph.height,
                baseline + glyph.axis_y * glyph.height,
            ]
            .map(|corner| view.project(corner, width, height));
            if point_in_quad(x, y, &corners) {
                let depth = corners
                    .iter()
                    .map(|corner| corner.depth)
                    .fold(f64::NEG_INFINITY, f64::max);
                consider_pick(&mut best, &glyph.source, 0.0, depth, tolerance);
            }
        }

        for picture in &self.pictures {
            for triangle in picture.triangles.chunks_exact(3) {
                let a = view.project(triangle[0], width, height);
                let b = view.project(triangle[1], width, height);
                let c = view.project(triangle[2], width, height);
                if point_in_triangle(x, y, &a, &b, &c) {
                    let depth = a.depth.max(b.depth).max(c.depth);
                    consider_pick(&mut best, &picture.source, 0.0, depth, tolerance);
                }
            }
        }

        best.map(|(source, _, _)| source)
    }

    /// World point nearest a stroke, within `tolerance`.
    pub fn pick(&self, point: Vector2, tolerance: f64) -> Option<&EntityRef> {
        let mut best: Option<(&EntityRef, f64)> = None;
        let limit = tolerance.max(0.0);
        for stroke in &self.strokes {
            if !stroke.curves.is_empty() {
                for curve in &stroke.curves {
                    let distance = super::curves::distance_xy(curve, point);
                    if distance <= limit && best.is_none_or(|(_, best_d)| distance < best_d) {
                        best = Some((&stroke.source, distance));
                    }
                }
                continue;
            }
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

/// Keep the closest primitive, breaking ties by depth toward the camera.
fn consider_pick<'a>(
    best: &mut Option<(&'a EntityRef, f64, f64)>,
    source: &'a EntityRef,
    distance: f64,
    depth: f64,
    tolerance: f64,
) {
    if distance > tolerance {
        return;
    }
    let better = match best {
        None => true,
        Some((_, best_distance, best_depth)) => {
            distance < *best_distance - 1e-6
                || ((distance - *best_distance).abs() <= 1e-6 && depth > *best_depth)
        }
    };
    if better {
        *best = Some((source, distance, depth));
    }
}

fn pick_segments<'a, I>(
    best: &mut Option<(&'a EntityRef, f64, f64)>,
    source: &'a EntityRef,
    segments: I,
    view: &super::View,
    viewport_px: (u32, u32),
    x: f64,
    y: f64,
    tolerance: f64,
) where
    I: Iterator<Item = (Vector3, Vector3)>,
{
    for (start, end) in segments {
        let start = view.project(start, viewport_px.0, viewport_px.1);
        let end = view.project(end, viewport_px.0, viewport_px.1);
        let (distance, t) = screen_segment_distance(x, y, &start, &end);
        let depth = start.depth + (end.depth - start.depth) * t;
        consider_pick(best, source, distance, depth, tolerance);
    }
}

/// Distance from a pixel to a projected segment, and the parameter of the
/// nearest point along it.
fn screen_segment_distance(
    x: f64,
    y: f64,
    start: &super::ScreenPoint,
    end: &super::ScreenPoint,
) -> (f64, f64) {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let len2 = dx * dx + dy * dy;
    let t = if len2 <= f64::EPSILON {
        0.0
    } else {
        (((x - start.x) * dx + (y - start.y) * dy) / len2).clamp(0.0, 1.0)
    };
    let px = start.x + dx * t - x;
    let py = start.y + dy * t - y;
    ((px * px + py * py).sqrt(), t)
}

fn point_in_triangle(
    x: f64,
    y: f64,
    a: &super::ScreenPoint,
    b: &super::ScreenPoint,
    c: &super::ScreenPoint,
) -> bool {
    let cross = |p: &super::ScreenPoint, q: &super::ScreenPoint| {
        (q.x - p.x) * (y - p.y) - (q.y - p.y) * (x - p.x)
    };
    let ab = cross(a, b);
    let bc = cross(b, c);
    let ca = cross(c, a);
    (ab >= -1e-6 && bc >= -1e-6 && ca >= -1e-6) || (ab <= 1e-6 && bc <= 1e-6 && ca <= 1e-6)
}

fn point_in_quad(x: f64, y: f64, corners: &[super::ScreenPoint; 4]) -> bool {
    point_in_triangle(x, y, &corners[0], &corners[1], &corners[2])
        || point_in_triangle(x, y, &corners[0], &corners[2], &corners[3])
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
