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
use crate::types::{Handle, Vector3};

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
            resource_dirs: Vec::new(),
            model_lineweight_px_per_mm: DEFAULT_MODEL_LINEWEIGHT_PX_PER_MM,
            visual_style: VisualStyleRequest::ActiveViewport,
            space: DisplaySpace::Model,
            solid_style: SolidStyle::ShadedWithEdges,
        }
    }
}

/// Orthographic plan camera. Pan and zoom change this and do not recompile.
#[derive(Debug, Clone, PartialEq)]
pub struct View {
    pub target: Vector3,
    pub height: f64,
    pub twist: f64,
    /// `None` keeps the style `compile` resolved from the drawing.
    pub visual_style: Option<Handle>,
}

impl Default for View {
    fn default() -> Self {
        Self {
            target: Vector3::ZERO,
            height: 1.0,
            twist: 0.0,
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
            visual_style: None,
        }
    }
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
