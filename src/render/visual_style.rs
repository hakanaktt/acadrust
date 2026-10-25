//! Every AutoCAD visual style the drawing can name, plus a custom style
//! read from a [`VisualStyle`](crate::objects::VisualStyle) property bag.

use crate::document::CadDocument;
use crate::entities::ViewportRenderMode;
use crate::objects::{ObjectType, VisualStyle, VisualStylePropertyValue};
use crate::types::Handle;

/// Built-in AutoCAD visual styles. `Custom` is anything else, resolved from
/// the style object's face and edge fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StandardVisualStyle {
    Wireframe2D,
    Wireframe3D,
    Hidden,
    FlatShaded,
    GouraudShaded,
    FlatShadedWithEdges,
    GouraudShadedWithEdges,
    Realistic,
    Conceptual,
    Shaded,
    ShadedWithEdges,
    ShadesOfGray,
    Sketchy,
    XRay,
    Custom,
}

impl StandardVisualStyle {
    pub const ALL: [StandardVisualStyle; 14] = [
        Self::Wireframe2D,
        Self::Wireframe3D,
        Self::Hidden,
        Self::FlatShaded,
        Self::GouraudShaded,
        Self::FlatShadedWithEdges,
        Self::GouraudShadedWithEdges,
        Self::Realistic,
        Self::Conceptual,
        Self::Shaded,
        Self::ShadedWithEdges,
        Self::ShadesOfGray,
        Self::Sketchy,
        Self::XRay,
    ];

    pub fn from_name(name: &str) -> Option<Self> {
        let key: String = name
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .flat_map(|ch| ch.to_lowercase())
            .collect();
        Some(match key.as_str() {
            "2dwireframe" => Self::Wireframe2D,
            "wireframe" | "3dwireframe" => Self::Wireframe3D,
            "hidden" | "hiddenline" => Self::Hidden,
            "flat" | "flatshaded" => Self::FlatShaded,
            "gouraud" | "gouraudshaded" => Self::GouraudShaded,
            "flatwithedges" | "flatshadedwithedges" => Self::FlatShadedWithEdges,
            "gouraudwithedges" | "gouraudshadedwithedges" => Self::GouraudShadedWithEdges,
            "realistic" => Self::Realistic,
            "conceptual" => Self::Conceptual,
            "shaded" => Self::Shaded,
            "shadedwithedges" => Self::ShadedWithEdges,
            "shadesofgray" | "shadesofgrey" => Self::ShadesOfGray,
            "sketchy" => Self::Sketchy,
            "xray" => Self::XRay,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wireframe2D => "2dWireframe",
            Self::Wireframe3D => "Wireframe",
            Self::Hidden => "Hidden",
            Self::FlatShaded => "FlatShaded",
            Self::GouraudShaded => "GouraudShaded",
            Self::FlatShadedWithEdges => "FlatShadedWithEdges",
            Self::GouraudShadedWithEdges => "GouraudShadedWithEdges",
            Self::Realistic => "Realistic",
            Self::Conceptual => "Conceptual",
            Self::Shaded => "Shaded",
            Self::ShadedWithEdges => "ShadedWithEdges",
            Self::ShadesOfGray => "Shades of Gray",
            Self::Sketchy => "Sketchy",
            Self::XRay => "X-Ray",
            Self::Custom => "Custom",
        }
    }

    fn from_render_mode(mode: ViewportRenderMode) -> Self {
        match mode {
            ViewportRenderMode::Wireframe2D => Self::Wireframe2D,
            ViewportRenderMode::Wireframe3D => Self::Wireframe3D,
            ViewportRenderMode::HiddenLine => Self::Hidden,
            ViewportRenderMode::FlatShaded => Self::FlatShaded,
            ViewportRenderMode::GouraudShaded => Self::GouraudShaded,
            ViewportRenderMode::FlatShadedWithEdges => Self::FlatShadedWithEdges,
            ViewportRenderMode::GouraudShadedWithEdges => Self::GouraudShadedWithEdges,
        }
    }
}

/// How faces of solids, meshes, and regions are lit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceShading {
    /// No faces. Wireframe and hidden styles.
    None,
    /// One color per face. Flat and flat-with-edges.
    Flat,
    /// Smoothed vertex normals. Gouraud, shaded, realistic, conceptual.
    Gouraud,
}

/// Which edges a style draws on top of faces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeMode {
    None,
    /// Parameter lines, used by both wireframe styles.
    Isolines,
    /// The face boundary. Hidden, shaded-with-edges, sketchy, x-ray.
    Facet,
}

/// What replaces the entity color on faces. The discriminator follows the
/// DXF group 73 values so a style object maps directly: 0 no color, 1 object,
/// 2 background, 3 custom, 4 monochrome, 5 tinted, 6 desaturated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaceColorMode {
    /// Entity or layer color.
    Object,
    /// One gray. Shades of Gray.
    Mono,
    /// Warm or cool shift.
    Tint,
    /// Entity color pulled toward gray.
    Desaturate,
    /// The view background. Hidden and Sketchy erase faces into it.
    Background,
    /// A style-supplied single color.
    Custom,
}

/// Lighting equation. Drawing lights are not used. This is the style's
/// display model only.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LightingModel {
    None,
    Constant,
    Phong,
    /// Gooch warm-to-cool shading. Conceptual.
    Gooch,
}

/// The display recipe a viewport or a named visual style resolves to.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedVisualStyle {
    pub kind: StandardVisualStyle,
    pub name: String,
    pub faces: FaceShading,
    pub edges: EdgeMode,
    pub lighting: LightingModel,
    pub face_color: FaceColorMode,
    /// Faces and edges drop surfaces that face away or sit behind a closer face.
    pub hidden_removal: bool,
    /// Obscured edges are drawn dashed instead of omitted.
    pub show_obscured: bool,
    pub silhouette: bool,
    /// Jittered overhanging edges. Sketchy.
    pub sketchy: bool,
    /// 2D linetype patterns are honored. Off once edges are facet edges of a shade style.
    pub linetypes: bool,
    /// Entity lineweight follows `LWDISPLAY`. Wireframe styles.
    pub lineweights: bool,
    /// Face opacity, 0 to 1. X-Ray is below 1. Other styles stay at 1
    /// unless the style object's opacity modifier says otherwise.
    pub face_opacity: f64,
}

impl ResolvedVisualStyle {
    pub fn from_standard(kind: StandardVisualStyle) -> Self {
        let mut style = Self::wireframe_2d();
        style.kind = kind;
        style.name = kind.as_str().to_string();
        match kind {
            StandardVisualStyle::Wireframe2D => {
                style.show_obscured = true;
            }
            StandardVisualStyle::Wireframe3D => {
                style.show_obscured = true;
                style.linetypes = true;
                style.lineweights = false;
            }
            // Hidden uses opaque background faces as a depth mask, so the
            // facet edges of nearer faces hide the edges behind them.
            StandardVisualStyle::Hidden => {
                shade(
                    &mut style,
                    FaceShading::Flat,
                    EdgeMode::Facet,
                    LightingModel::Constant,
                );
                style.face_color = FaceColorMode::Background;
                style.silhouette = true;
            }
            StandardVisualStyle::FlatShaded => shade(
                &mut style,
                FaceShading::Flat,
                EdgeMode::None,
                LightingModel::Constant,
            ),
            StandardVisualStyle::GouraudShaded | StandardVisualStyle::Shaded => {
                shade(
                    &mut style,
                    FaceShading::Gouraud,
                    EdgeMode::None,
                    LightingModel::Phong,
                );
                style.show_obscured = true;
            }
            // The with-edges recipes use the isoline edge model, the same as
            // AutoCAD's FlatWithEdges/GouraudWithEdges/Shaded with edges.
            StandardVisualStyle::FlatShadedWithEdges => {
                shade(
                    &mut style,
                    FaceShading::Flat,
                    EdgeMode::Isolines,
                    LightingModel::Constant,
                );
            }
            StandardVisualStyle::GouraudShadedWithEdges | StandardVisualStyle::ShadedWithEdges => {
                shade(
                    &mut style,
                    FaceShading::Gouraud,
                    EdgeMode::Isolines,
                    LightingModel::Phong,
                );
                style.silhouette = true;
            }
            StandardVisualStyle::Realistic => {
                shade(
                    &mut style,
                    FaceShading::Gouraud,
                    EdgeMode::None,
                    LightingModel::Phong,
                );
            }
            StandardVisualStyle::Conceptual => {
                shade(
                    &mut style,
                    FaceShading::Gouraud,
                    EdgeMode::Facet,
                    LightingModel::Gooch,
                );
                style.silhouette = true;
            }
            StandardVisualStyle::ShadesOfGray => {
                shade(
                    &mut style,
                    FaceShading::Gouraud,
                    EdgeMode::Facet,
                    LightingModel::Phong,
                );
                style.face_color = FaceColorMode::Mono;
                style.silhouette = true;
            }
            StandardVisualStyle::Sketchy => {
                shade(
                    &mut style,
                    FaceShading::Flat,
                    EdgeMode::Facet,
                    LightingModel::Constant,
                );
                style.face_color = FaceColorMode::Background;
                style.sketchy = true;
                style.silhouette = true;
            }
            StandardVisualStyle::XRay => {
                shade(
                    &mut style,
                    FaceShading::Gouraud,
                    EdgeMode::Isolines,
                    LightingModel::Phong,
                );
                style.face_opacity = 0.5;
            }
            StandardVisualStyle::Custom => {}
        }
        style
    }

    pub fn from_render_mode(mode: ViewportRenderMode) -> Self {
        Self::from_standard(StandardVisualStyle::from_render_mode(mode))
    }

    /// Named style when `description` matches a built-in. Otherwise the
    /// face and edge fields, so a custom style still has a recipe.
    pub fn from_object(style: &VisualStyle) -> Self {
        let named = StandardVisualStyle::from_name(&style.description);
        let mut resolved = if named.is_some() && style.properties.is_empty() {
            Self::from_standard(named.unwrap())
        } else {
            from_fields(style)
        };
        if let Some(kind) = named {
            resolved.kind = kind;
            resolved.name = kind.as_str().to_string();
            if kind == StandardVisualStyle::Sketchy {
                resolved.sketchy = true;
            }
        } else {
            resolved.name = if style.description.is_empty() {
                "Custom".to_string()
            } else {
                style.description.clone()
            };
        }
        apply_opacity(style, &mut resolved);
        resolved
    }

    fn wireframe_2d() -> Self {
        Self {
            kind: StandardVisualStyle::Wireframe2D,
            name: StandardVisualStyle::Wireframe2D.as_str().to_string(),
            faces: FaceShading::None,
            edges: EdgeMode::Isolines,
            lighting: LightingModel::None,
            face_color: FaceColorMode::Object,
            hidden_removal: false,
            show_obscured: false,
            silhouette: false,
            sketchy: false,
            linetypes: true,
            lineweights: true,
            face_opacity: 1.0,
        }
    }
}

/// Which style `compile` should start from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisualStyleRequest {
    /// The `*Active` model-space viewport. A visual-style handle on it wins
    /// over its render-mode code.
    ActiveViewport,
    /// A built-in AutoCAD visual style, independent of the active viewport.
    Standard(StandardVisualStyle),
    RenderMode(ViewportRenderMode),
    Object(Handle),
}

impl Default for VisualStyleRequest {
    fn default() -> Self {
        Self::ActiveViewport
    }
}

pub(crate) fn resolve(doc: &CadDocument, request: &VisualStyleRequest) -> ResolvedVisualStyle {
    match request {
        VisualStyleRequest::RenderMode(mode) => ResolvedVisualStyle::from_render_mode(*mode),
        VisualStyleRequest::Standard(kind) => ResolvedVisualStyle::from_standard(*kind),
        VisualStyleRequest::Object(handle) => {
            object_style(doc, *handle).unwrap_or_else(ResolvedVisualStyle::wireframe_2d)
        }
        VisualStyleRequest::ActiveViewport => {
            let vport = doc.vports.get("*Active");
            if let Some(vport) = vport {
                if !vport.visual_style_handle.is_null() {
                    if let Some(style) = object_style(doc, vport.visual_style_handle) {
                        return style;
                    }
                }
                return ResolvedVisualStyle::from_render_mode(vport.render_mode);
            }
            ResolvedVisualStyle::wireframe_2d()
        }
    }
}

fn object_style(doc: &CadDocument, handle: Handle) -> Option<ResolvedVisualStyle> {
    match doc.objects.get(&handle)? {
        ObjectType::VisualStyle(style) => Some(ResolvedVisualStyle::from_object(style)),
        _ => None,
    }
}

fn shade(
    style: &mut ResolvedVisualStyle,
    faces: FaceShading,
    edges: EdgeMode,
    lighting: LightingModel,
) {
    style.faces = faces;
    style.edges = edges;
    style.lighting = lighting;
    style.linetypes = false;
    style.lineweights = false;
    style.hidden_removal = true;
}

fn from_fields(style: &VisualStyle) -> ResolvedVisualStyle {
    // DXF group 70. 4 is the 2D wireframe type and ignores the face model.
    if style.style_type == 4 {
        return ResolvedVisualStyle::wireframe_2d();
    }
    let mut resolved = ResolvedVisualStyle::wireframe_2d();
    resolved.kind = StandardVisualStyle::Custom;
    // Group 71: 0 invisible, 1 constant, 2 phong, 3 gooch.
    let (faces, lighting) = match style.face_lighting_model {
        1 => (FaceShading::Flat, LightingModel::Constant),
        2 => (FaceShading::Gouraud, LightingModel::Phong),
        3 => (FaceShading::Gouraud, LightingModel::Gooch),
        _ => (FaceShading::None, LightingModel::None),
    };
    // Group 74: 0 none, 1 isolines, 2 facet edges.
    let edges = match style.edge_model {
        1 => EdgeMode::Isolines,
        2 => EdgeMode::Facet,
        _ => EdgeMode::None,
    };
    resolved.faces = faces;
    resolved.lighting = lighting;
    resolved.edges = edges;
    resolved.hidden_removal = faces != FaceShading::None;
    resolved.show_obscured = style.edge_style & 4 != 0;
    resolved.silhouette = style.edge_style & 2 != 0;
    resolved.sketchy = style.style_type == 24;
    resolved.linetypes = faces == FaceShading::None && edges == EdgeMode::Isolines;
    resolved.lineweights = resolved.linetypes && style.style_type == 4;
    // Group 73: 0 none, 1 object, 2 background, 3 custom, 4 mono, 5 tint,
    // 6 desaturate.
    resolved.face_color = match style.face_color_mode {
        2 => FaceColorMode::Background,
        3 => FaceColorMode::Custom,
        4 => FaceColorMode::Mono,
        5 => FaceColorMode::Tint,
        6 => FaceColorMode::Desaturate,
        _ => FaceColorMode::Object,
    };
    resolved
}

fn apply_opacity(style: &VisualStyle, resolved: &mut ResolvedVisualStyle) {
    // Face modifier bit 1 enables opacity. Core property 4 is that amount.
    if style.face_modifier & 1 == 0 {
        return;
    }
    let Some(property) = style.properties.get(4) else {
        return;
    };
    let amount = match property.value {
        VisualStylePropertyValue::Double(value) => value,
        VisualStylePropertyValue::Long(value) => value as f64,
        VisualStylePropertyValue::Short(value) => value as f64,
        _ => return,
    };
    if (0.0..=1.0).contains(&amount) {
        resolved.face_opacity = amount;
    }
}
