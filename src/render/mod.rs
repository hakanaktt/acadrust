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

mod compile;
mod curves;
mod dash;
mod gpu;
mod scene;
mod style;
mod visual_style;

pub use compile::FALLBACK_KINDS;
pub use gpu::{render_rgba, RenderError, Renderer, RendererConfig, RgbaImage};
pub use scene::{DrawPath, EntityRef, FallbackEntry, Fill, Scene, SkippedEntity, Stroke, Style};
pub use visual_style::{
    EdgeMode, FaceColorMode, FaceShading, LightingModel, ResolvedVisualStyle, StandardVisualStyle,
    VisualStyleRequest,
};

use crate::document::CadDocument;
use crate::types::{Handle, Vector3};

/// Provisional model-space lineweight scale, in pixels per millimetre.
///
/// AutoCAD's Options slider is not stored in the file. This stands in until
/// a lineweight chart captured from AutoCAD replaces it.
pub const DEFAULT_MODEL_LINEWEIGHT_PX_PER_MM: f64 = 2.0;

/// What to compile, independent of the camera.
#[derive(Debug, Clone)]
pub struct CompileOptions {
    pub font_dirs: Vec<std::path::PathBuf>,
    pub resource_dirs: Vec<std::path::PathBuf>,
    pub model_lineweight_px_per_mm: f64,
    pub visual_style: VisualStyleRequest,
}

impl Default for CompileOptions {
    fn default() -> Self {
        Self {
            font_dirs: Vec::new(),
            resource_dirs: Vec::new(),
            model_lineweight_px_per_mm: DEFAULT_MODEL_LINEWEIGHT_PX_PER_MM,
            visual_style: VisualStyleRequest::ActiveViewport,
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

/// Resolve `doc` into a display list for model space.
pub fn compile(doc: &CadDocument, options: &CompileOptions) -> Scene {
    compile::compile(doc, options)
}

#[cfg(test)]
mod tests;
