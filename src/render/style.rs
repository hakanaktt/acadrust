//! ByLayer, ByBlock, and layer-0 resolution.

use crate::document::CadDocument;
use crate::entities::EntityCommon;
use crate::tables::LineType;
use crate::types::{Color, LineWeight, Transparency};

use super::scene::Style;
use super::visual_style::ResolvedVisualStyle;

const DEFAULT_LINEWEIGHT_MM: f64 = 0.25;

#[derive(Clone)]
pub(crate) struct Frame {
    /// `true` while walking the contents of an insert.
    pub in_insert: bool,
    pub layer: String,
    pub color: Color,
    pub lineweight: LineWeight,
    pub linetype: String,
    pub transparency: Transparency,
}

impl Frame {
    pub(crate) fn model() -> Self {
        Self {
            in_insert: false,
            layer: "0".to_string(),
            color: Color::ByLayer,
            lineweight: LineWeight::ByLayer,
            linetype: "ByLayer".to_string(),
            transparency: Transparency::ByLayer,
        }
    }

    /// The paint a nested ByBlock or layer-0 entity inherits from this insert.
    ///
    /// `ByLayer` on the insert stays `ByLayer`. The child resolves it against
    /// the insert's effective layer, which is this frame's `layer`.
    pub(crate) fn from_insert(parent: &Self, common: &EntityCommon) -> Self {
        Self {
            in_insert: true,
            layer: effective_layer(&common.layer, parent),
            color: inherit_color(common.color, parent),
            lineweight: inherit_weight(common.line_weight, parent),
            linetype: inherit_linetype(&common.linetype, parent),
            transparency: inherit_transparency(common.transparency, parent),
        }
    }
}

pub(crate) fn resolve_style(
    doc: &CadDocument,
    common: &EntityCommon,
    frame: &Frame,
    visual: &ResolvedVisualStyle,
) -> Style {
    let layer_name = effective_layer(&common.layer, frame);
    let layer = doc.layers.get(&layer_name);
    let color = inherit_color(common.color, frame);
    let color = if matches!(color, Color::ByLayer) {
        layer.map(|layer| layer.color).unwrap_or(Color::WHITE)
    } else {
        color
    };
    let (aci7, rgb) = match color {
        Color::Index(7) => (true, [255, 255, 255]),
        Color::Rgb { r, g, b } => (false, [r, g, b]),
        Color::Index(index) => {
            let (r, g, b) = crate::types::aci_table::aci_to_rgb(index).unwrap_or((255, 255, 255));
            (false, [r, g, b])
        }
        Color::None => (false, [0, 0, 0]),
        Color::ByBlock | Color::ByLayer => (true, [255, 255, 255]),
    };
    let transparency = inherit_transparency(common.transparency, frame);
    let transparency = if transparency.is_by_layer() {
        layer
            .map(|layer| layer.transparency)
            .unwrap_or(Transparency::OPAQUE)
    } else {
        transparency
    };
    let opacity = 1.0 - transparency.alpha() as f32 / 255.0;

    let weight = inherit_weight(common.line_weight, frame);
    let weight = if matches!(weight, LineWeight::ByLayer) {
        layer
            .map(|layer| layer.line_weight)
            .unwrap_or(LineWeight::Default)
    } else {
        weight
    };
    let cosmetic = !visual.lineweights || !doc.header.lineweight_display;
    let lineweight_mm = if cosmetic {
        0.0
    } else {
        match weight {
            LineWeight::Value(value) => value as f64 / 100.0,
            LineWeight::Default | LineWeight::ByLayer | LineWeight::ByBlock => {
                DEFAULT_LINEWEIGHT_MM
            }
        }
    };

    Style {
        layer: layer_name,
        aci7,
        rgb,
        opacity,
        lineweight_mm,
        cosmetic,
    }
}

/// One fitted linetype element. Length is already multiplied by `LTSCALE`
/// and the entity scale. Positive is a dash, negative a gap, zero a dot.
#[derive(Debug, Clone)]
pub(crate) struct PatternMark {
    pub length: f64,
    pub text: Option<String>,
    pub shape_number: Option<u16>,
    pub style_name: String,
    pub scale: f64,
    pub rotation: f64,
    pub absolute: bool,
    pub upright: bool,
    pub offset_along: f64,
    pub offset_perp: f64,
}

pub(crate) fn pattern_elements(
    doc: &CadDocument,
    common: &EntityCommon,
    frame: &Frame,
    visual: &ResolvedVisualStyle,
) -> Vec<PatternMark> {
    if !visual.linetypes {
        return Vec::new();
    }
    let name = inherit_linetype(&common.linetype, frame);
    let linetype = if name.eq_ignore_ascii_case("bylayer") {
        let layer_name = effective_layer(&common.layer, frame);
        doc.layers
            .get(&layer_name)
            .map(|layer| layer.line_type.as_str())
            .unwrap_or("Continuous")
    } else {
        name.as_str()
    };
    let Some(linetype) = lookup_linetype(doc, linetype) else {
        return Vec::new();
    };
    if linetype.is_continuous() || is_inherited_name(&linetype.name) {
        return Vec::new();
    }
    let scale = doc.header.linetype_scale * common.linetype_scale;
    linetype
        .elements
        .iter()
        .map(|element| {
            let mut mark = PatternMark {
                length: element.length * scale,
                text: None,
                shape_number: None,
                style_name: "Standard".to_string(),
                scale: scale,
                rotation: 0.0,
                absolute: false,
                upright: false,
                offset_along: 0.0,
                offset_perp: 0.0,
            };
            if let Some(complex) = &element.complex {
                mark.style_name = doc
                    .text_styles
                    .iter()
                    .find(|style| style.handle == complex.style_handle)
                    .map(|style| style.name.clone())
                    .unwrap_or_else(|| "Standard".to_string());
                mark.scale = complex.scale * scale;
                mark.rotation = complex.rotation;
                mark.absolute = complex.absolute_rotation;
                mark.upright = complex.upright;
                mark.offset_along = complex.offset[0] * scale;
                mark.offset_perp = complex.offset[1] * scale;
                match &complex.content {
                    crate::tables::LineTypeComplexContent::Text { text } => {
                        mark.text = Some(text.clone());
                    }
                    crate::tables::LineTypeComplexContent::Shape { shape_number } => {
                        mark.shape_number = Some(*shape_number as u16);
                    }
                }
            }
            mark
        })
        .collect()
}

pub(crate) fn layer_is_hidden(doc: &CadDocument, layer_name: &str) -> bool {
    doc.layers
        .get(layer_name)
        .is_some_and(|layer| layer.flags.off || layer.flags.frozen)
}

fn effective_layer(layer: &str, frame: &Frame) -> String {
    if frame.in_insert && layer.eq_ignore_ascii_case("0") {
        frame.layer.clone()
    } else if layer.is_empty() {
        "0".to_string()
    } else {
        layer.to_string()
    }
}

fn inherit_color(color: Color, frame: &Frame) -> Color {
    if color == Color::ByBlock {
        if frame.in_insert {
            frame.color
        } else {
            Color::ByLayer
        }
    } else {
        color
    }
}

fn inherit_weight(weight: LineWeight, frame: &Frame) -> LineWeight {
    if weight == LineWeight::ByBlock {
        if frame.in_insert {
            frame.lineweight
        } else {
            LineWeight::ByLayer
        }
    } else {
        weight
    }
}

fn inherit_transparency(transparency: Transparency, frame: &Frame) -> Transparency {
    if transparency.is_by_block() {
        if frame.in_insert {
            frame.transparency
        } else {
            Transparency::ByLayer
        }
    } else {
        transparency
    }
}

fn inherit_linetype(name: &str, frame: &Frame) -> String {
    if name.is_empty() || name.eq_ignore_ascii_case("bylayer") {
        "ByLayer".to_string()
    } else if name.eq_ignore_ascii_case("byblock") {
        if frame.in_insert {
            frame.linetype.clone()
        } else {
            "ByLayer".to_string()
        }
    } else {
        name.to_string()
    }
}

fn is_inherited_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("bylayer") || name.eq_ignore_ascii_case("byblock")
}

fn lookup_linetype<'a>(doc: &'a CadDocument, name: &str) -> Option<&'a LineType> {
    doc.line_types.get(name)
}
