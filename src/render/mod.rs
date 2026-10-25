//! Display compiler for an AutoCAD-style view of a [`CadDocument`](crate::CadDocument).
//!
//! `compile` resolves visual style, layer color, linetype, and block inserts
//! into a camera-independent [`Scene`]. [`Renderer`] draws that scene with wgpu.
//!
//! Every built-in AutoCAD visual style has a recipe in [`StandardVisualStyle`].
//! A viewport render mode selects one of the seven classic modes. A named
//! [`VisualStyle`](crate::objects::VisualStyle) object selects the rest,
//! including Realistic, Conceptual, Shades of Gray, Sketchy, and X-Ray.
//! A custom style uses the object's face and edge fields.

mod bitmap;
mod clip;
mod compile;
mod curves;
mod dash;
mod external;
mod gpu;
mod hatch_fill;
mod metafile;
mod pdf_page;
mod scene;
mod sheet;
mod shx;
mod style;
mod tess;
mod text_draw;
mod ttf_atlas;
mod visual_style;

pub use compile::FALLBACK_KINDS;
pub use gpu::{render_rgba, render_rgba_with, RenderError, Renderer, RendererConfig, RgbaImage};
pub use scene::{
    DrawPath, EntityRef, FallbackEntry, Fill, Picture, Scene, SkippedEntity, Stroke, Style,
};
pub use visual_style::{
    EdgeMode, FaceColorMode, FaceShading, LightingModel, ResolvedVisualStyle, StandardVisualStyle,
    VisualStyleRequest,
};

use crate::document::CadDocument;
use crate::types::{Handle, Matrix3, Vector3};

/// Model-space lineweight scale, in pixels per millimetre.
///
/// AutoCAD stores `LWDISPLAY` in the file and keeps the Options "Adjust Display
/// Scale" slider on the machine. At that slider's default position a 0.25 mm
/// stroke is 2 pixels wide, which is 8 pixels per millimetre. The width does
/// not change when the view zooms. Callers who moved the slider pass a
/// different [`CompileOptions::model_lineweight_px_per_mm`].
pub const DEFAULT_MODEL_LINEWEIGHT_PX_PER_MM: f64 = 8.0;

/// Device pixels for a model-space lineweight. The result is at least 1.
pub fn model_lineweight_pixels(millimetres: f64, px_per_mm: f64) -> u32 {
    let pixels = millimetres.max(0.0) * px_per_mm.max(0.0);
    pixels.round().max(1.0) as u32
}

/// How ACIS solids, regions, bodies, and surfaces are drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SolidStyle {
    /// Faces in the entity color, with the face boundary on top.
    #[default]
    ShadedWithEdges,
    /// Boundary edges and parameter isolines. No faces.
    Wireframe,
}

/// Model space, or one layout tab.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplaySpace {
    /// `*Model_Space`. Lineweights are pixels.
    Model,
    /// A layout tab. `"Model"` is model space. Any other name draws that sheet:
    /// paper geometry, then each viewport's model picture.
    Layout(String),
}

/// What to compile, independent of the camera.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    pub font_dirs: Vec<std::path::PathBuf>,
    /// Font files supplied directly by the caller. The name is matched
    /// against a text style's font filename, which lets browser callers use
    /// uploaded SHX/SHP data without a filesystem.
    pub font_files: Vec<(String, Vec<u8>)>,
    pub resource_dirs: Vec<std::path::PathBuf>,
    pub model_lineweight_px_per_mm: f64,
    pub visual_style: VisualStyleRequest,
    /// Model space, or the named layout tab.
    pub space: DisplaySpace,
    /// Solids are shaded with their edges unless this asks for isolines.
    pub solid_style: SolidStyle,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            font_dirs: Vec::new(),
            font_files: Vec::new(),
            resource_dirs: Vec::new(),
            model_lineweight_px_per_mm: DEFAULT_MODEL_LINEWEIGHT_PX_PER_MM,
            visual_style: VisualStyleRequest::ActiveViewport,
            space: DisplaySpace::Model,
            solid_style: SolidStyle::ShadedWithEdges,
        }
    }
}

/// Orthographic camera. Pan, zoom, and orbit change this and do not recompile.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub target: Vector3,
    /// Vertical size of the view, in drawing units.
    pub height: f64,
    /// Roll about the view direction. Positive roll turns the model clockwise,
    /// matching a paper viewport's twist.
    pub twist: f64,
    /// From the target toward the camera. `(0, 0, 1)` is the plan view.
    /// Paper viewports use this same axis and the same twist sense.
    pub direction: Vector3,
    /// `None` keeps the style `compile` resolved from the drawing.
    pub visual_style: Option<Handle>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            target: Vector3::ZERO,
            height: 1.0,
            twist: 0.0,
            direction: Vector3::UNIT_Z,
            visual_style: None,
        }
    }
}

impl View {
    /// Frame `scene` with a small margin. `width_px` and `height_px` preserve
    /// the viewport aspect.
    pub fn zoom_extents(scene: &Scene, width_px: u32, height_px: u32) -> Self {
        let Some((min, max)) = scene.bounds_min_max() else {
            return Self::default();
        };
        let width = (max.x - min.x).abs();
        let depth = (max.y - min.y).abs();
        let aspect = if height_px == 0 {
            1.0
        } else {
            width_px.max(1) as f64 / height_px as f64
        };
        let height = (depth.max(width / aspect) * 1.05).max(1e-6);
        Self {
            target: Vector3::new(
                (min.x + max.x) * 0.5,
                (min.y + max.y) * 0.5,
                (min.z + max.z) * 0.5,
            ),
            height,
            twist: 0.0,
            direction: Vector3::UNIT_Z,
            visual_style: None,
        }
    }

    /// Unit vector from the target toward the camera. A zero direction is the
    /// plan view.
    pub fn toward_camera(&self) -> Vector3 {
        if self.direction.length_squared() > 1e-18 {
            self.direction.normalize()
        } else {
            Vector3::UNIT_Z
        }
    }

    /// Screen axes. `right` is +X on screen, `up` is +Y on screen, and
    /// `toward_camera` points from the target to the camera.
    pub fn basis(&self) -> (Vector3, Vector3, Vector3) {
        let normal = self.toward_camera();
        let (x_dir, y_dir) = view_reference_axes(normal);
        let (sin, cos) = self.twist.sin_cos();
        let right = x_dir * cos + y_dir * sin;
        let up = x_dir * (-sin) + y_dir * cos;
        (right, up, normal)
    }

    /// Frame `scene` on the current direction and twist.
    pub fn frame(&mut self, scene: &Scene, width_px: u32, height_px: u32) {
        let Some((min, max)) = scene.bounds_min_max() else {
            return;
        };
        let center = (min + max) * 0.5;
        let (right, up, _) = self.basis();
        let mut min_u = f64::INFINITY;
        let mut max_u = f64::NEG_INFINITY;
        let mut min_v = f64::INFINITY;
        let mut max_v = f64::NEG_INFINITY;
        for x in [min.x, max.x] {
            for y in [min.y, max.y] {
                for z in [min.z, max.z] {
                    let delta = Vector3::new(x, y, z) - center;
                    let u = delta.dot(&right);
                    let v = delta.dot(&up);
                    min_u = min_u.min(u);
                    max_u = max_u.max(u);
                    min_v = min_v.min(v);
                    max_v = max_v.max(v);
                }
            }
        }
        let aspect = if height_px == 0 {
            1.0
        } else {
            width_px.max(1) as f64 / height_px as f64
        };
        let span_u = max_u - min_u;
        let span_v = max_v - min_v;
        self.target = center;
        self.height = (span_v.max(span_u / aspect) * 1.05).max(1e-6);
    }

    /// Turntable orbit. Positive `yaw` moves the camera from +X toward +Y.
    /// Positive `pitch` rotates about the camera's right axis, lowering a
    /// front camera toward the bottom view.
    pub fn orbit(&mut self, yaw: f64, pitch: f64) {
        if yaw == 0.0 && pitch == 0.0 {
            return;
        }
        let (mut right, _, mut direction) = self.basis();
        if yaw != 0.0 {
            direction = rotate_around(direction, Vector3::UNIT_Z, yaw);
            right = rotate_around(right, Vector3::UNIT_Z, yaw);
        }
        if pitch != 0.0 {
            direction = rotate_around(direction, right, pitch);
        }
        self.set_frame(direction, right);
    }

    /// Snap to a view direction and clear the roll, as a ViewCube face does.
    pub fn look_along(&mut self, direction: Vector3) {
        let direction = direction.normalize();
        if direction.length_squared() == 0.0 {
            return;
        }
        self.direction = direction;
        self.twist = 0.0;
    }

    fn set_frame(&mut self, direction: Vector3, right: Vector3) {
        let direction = direction.normalize();
        if direction.length_squared() == 0.0 {
            return;
        }
        let mut right = right - direction * direction.dot(&right);
        if right.length_squared() < 1e-16 {
            right = direction.cross(&Vector3::UNIT_Z);
            if right.length_squared() < 1e-16 {
                right = direction.cross(&Vector3::UNIT_Y);
            }
        }
        let right = right.normalize();
        self.direction = direction;
        self.twist = twist_for(direction, right);
    }
}

fn view_reference_axes(normal: Vector3) -> (Vector3, Vector3) {
    let axes = Matrix3::arbitrary_axis(normal);
    let x_dir = Vector3::new(axes.m[0][0], axes.m[1][0], axes.m[2][0]);
    let y_dir = Vector3::new(axes.m[0][1], axes.m[1][1], axes.m[2][1]);
    (x_dir, y_dir)
}

fn twist_for(direction: Vector3, right: Vector3) -> f64 {
    let (x_dir, y_dir) = view_reference_axes(direction);
    right.dot(&y_dir).atan2(right.dot(&x_dir))
}

fn rotate_around(vector: Vector3, axis: Vector3, angle: f64) -> Vector3 {
    let axis = axis.normalize();
    if axis.length_squared() == 0.0 || angle == 0.0 {
        return vector;
    }
    let (sin, cos) = angle.sin_cos();
    vector * cos + axis.cross(&vector) * sin + axis * (axis.dot(&vector) * (1.0 - cos))
}

/// Resolve `doc` into a display list.
///
/// [`CompileOptions::space`] selects model space or a named layout. A layout
/// is compiled in paper coordinates. Paper entities are drawn where they sit.
/// Each viewport window receives model space, scaled, twisted, and clipped.
pub fn compile(doc: &CadDocument, options: &CompileOptions) -> Scene {
    compile::compile(doc, options)
}

#[cfg(test)]
mod tests;
