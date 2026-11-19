//! Section views of sample lines.

use super::profile_view::{point, text};
use super::sample_lines::{arch, read_line, write_line};
use super::site::default_style_of;
use super::support::*;
use crate::entities::EntityCommon;
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::{Handle, Vector2};
use crate::CadDocument;

/// Style tree key of the section view style collection.
const KEY_SECTION_VIEW_STYLES: i32 = 0x33;
/// The class words of a section view record.
const GRAPH_WORDS: [i32; 2] = [357_877_593, 142_348_576];
/// The graph keys and their values.
const GRAPH_KEYS: [(u32, i32); 2] = [(0x0301_D029, 1), (0x0D01_D024, 0)];
/// The text id the view's range is kept under.
const VIEW_TEXT_ID: i32 = 0x8000_0440_u32 as i32;
/// The view title (a field code the application expands).
const TITLE: &str = "<[33673217(2097|2474|2387|2433|2593|2609|2657|2674|2689|2706|2721|2707|2803|2817)]>";
/// The range the default style gives a view: the sample line's offsets
/// and the section's elevations padded by 20 and rounded out to 10. A
/// section the application has not sampled has no elevations, so the
/// view spans elevations -20 to 20.
const MARGIN: f64 = 20.0;
const STEP: f64 = 10.0;
const ELEVATION_MIN: f64 = -20.0;
const ELEVATION_MAX: f64 = 20.0;

/// An offset value of the view's graph record.
#[derive(Clone, Copy)]
enum Value {
    OffsetMin,
    OffsetMax,
    Width,
    Center,
}

/// A piece of the view's graph record after the vertical scale.
enum Piece {
    /// A constant run (the same in every sample).
    Bits(&'static str),
    /// A `BD` from the view's offset range.
    Value(Value),
    /// The view title (`BL` byte count, UTF-16 with its terminator).
    Title,
}

/// The graph record after the vertical scale: the title again, then the
/// graph's frame and axes. Only the offset range's limits, width and
/// center depend on the view; the elevation values (-20 to 20) and the
/// default style's settings are part of the constant runs, which are the
/// same in every sample (views at two locations, offset ranges -30 to 30,
/// -40 to 50 and -30 to 60, lines with and without a section).
// ponytail: the runs are not split into fields; name them once a view of
// a sampled section (other elevations) or another style is written.
const GRAPH: &[Piece] = &[
    Piece::Bits("10100000010000000000100000000000000000"),
    Piece::Title,
    Piece::Bits("101000000000000000000000000000000000000000000000000000010001000100000010100001010101010101010101010101010101010101010101010110010101001111111000000100010001000100010001000100010001000100010001100000010011111110100100001000"),
    Piece::Value(Value::OffsetMin),
    Piece::Bits("000000000000000000000000000000000000000000000000000011010011000000101000000000000000000000000000000000000000000000000000001101001100000010"),
    Piece::Value(Value::OffsetMax),
    Piece::Bits("0000000000000000000000000000000000000000000000000000110100010000001001000001100100000001"),
    Piece::Value(Value::OffsetMin),
    Piece::Bits("10101000000000000000000000000000000000000000000000000000010001000100000010100000000000000000000000000000000000000000000000000001000100010000001010101001000001000100010100010001010000000001101100000000000110010100000000011101100000000001100001000000000111010000000000011010010000000001101111000000000110111000000000000000000000000010"),
    Piece::Value(Value::OffsetMin),
    Piece::Bits("0000000000000000000000000000000000000000000000000000110100010000001000000100010001000100010001000100010001000100010001100000011011111110100000010001000100010001000100010001000100010001000110000001001111111000110111100001111010000100100000000111110011011001000100100100000001000010000100000010101010100000000000000000000000000000000000000000000000000001000100010000001010000000000000000000000000000000000000000000000000000100010001000000101010100100000100010001010001000101000000000110110000000000011001010000000001110110000000000110000100000000011101000000000001101001000000000110111100000000011011100000000000000000000000001010000000000000000000000000000000000000000000000000000011010001000000100000010001000100010001000100010001000100010001000110000001101111111010000001000100010001000100010001000100010001000100011000000100111111101001000010000100000100"),
    Piece::Value(Value::OffsetMax),
    Piece::Bits("10101000000000000000000000000000000000000000000000000000010001000100000010100000000000000000000000000000000000000000000000000001000100010000001010101001000001000100010100010001010000000001101100000000000110010100000000011101100000000001100001000000000111010000000000011010010000000001101111000000000110111000000000000000000000000010"),
    Piece::Value(Value::OffsetMax),
    Piece::Bits("00000000000000000000000000000000000000000000000000001101000100000010000001000100010001000100010001000100010001000100011000000100111111101000000100010001000100010001000100010001000100010001100000010011111110000100101011011000000100100100110111111011001000011111100100111111010000100001000010001000000000000000000000000000000000000000000000000000010001000100000010"),
    Piece::Value(Value::Width),
    Piece::Bits("1010"),
    Piece::Value(Value::OffsetMax),
    Piece::Bits("1010"),
    Piece::Value(Value::OffsetMin),
    Piece::Bits("101001000001000100001110010011110000000001100110000000000110011000000000011100110000000001100101000000000111010000000000000000000000000010"),
    Piece::Value(Value::Center),
    Piece::Bits("00000000000000000000000000000000000000000000000000010001000100000010100000010001000100010001000100010001000100010001000110000001001111111000000100010001000100010001000100010001000100010001100000010011111110100100001000010001000010101010101010101010101010010000001000000000000000001010101010101010101001000001010100100000101010"),
    Piece::Value(Value::Width),
    Piece::Bits("1010"),
    Piece::Value(Value::OffsetMax),
    Piece::Bits("1010"),
    Piece::Value(Value::OffsetMin),
    Piece::Bits("101001000001000100001110010011110000000001100110000000000110011000000000011100110000000001100101000000000111010000000000000000000000000010"),
    Piece::Value(Value::Center),
    Piece::Bits("1010100000010001000100010001000100010001000100010001000110000001101111111000000100010001000100010001000100010001000100010001100000010011111110100100000010"),
    Piece::Value(Value::OffsetMin),
    Piece::Bits("000000000000000000000000000000000000000000000000000011010011000000101000000000000000000000000000000000000000000000000000001101001100000010"),
    Piece::Value(Value::OffsetMax),
    Piece::Bits("000000000000000000000000000000000000000000000000000011010001000000100101010100000101010101010101010101010101010101010101010101011010010100111111000101010101010101010101010101010101010101010101011010010100111111000101010101010101010101010101010101010101010101011010010100111111000101010101010101010101010101010101010101010101011010010100111111101010101010100100000001100100000010100100000011100100000100100001011111000000100"),
];

/// The range of a view of a line reaching `left` and `right`.
fn range(left: f64, right: f64) -> (f64, f64) {
    (((-left - MARGIN) / STEP).floor() * STEP, ((right + MARGIN) / STEP).ceil() * STEP)
}

/// The section view record (`AECC_GRAPH_SECTION_VIEW`, an entity in
/// model space):
///
/// ```text
/// BL 80, 2 bits 11, the class words, BL 5, BL 2, the graph keys,
/// BL 0 (byte form), 121 zero bits
/// 3BD origin, 9 BD (identity), BL n sections, n x 5 bits 10000, BL 0,
/// B 1, 3BD origin, BL 4, BL text id
/// 3BD (offset min, elevation min, 0), (0, elevation min, 0), (offset
///   max, elevation max, 0), twice
/// BL 528, the title (BL byte count, UTF-16), BL 0
/// 7 BD (0, height, 0, 0, 1/48, 0, 1/120)
/// BL 0, 0, 8, 1, 4, text id
/// 3BD origin, BD 0, 0, 0, 1, BD 1 (vertical scale), then [`GRAPH`]
/// references: soft 0, hard owner 0, style (hard), sample line (hard),
///   per section {section (soft), hard 0, hard 0}, hard 0 x 4, soft 0,
///   hard 0
/// texts: "", "", name, "", name, ""
/// ```
///
/// The origin is the point of offset 0 at the view's lowest elevation.
/// Written with the samples' origins, lines and sections the record
/// equals theirs bit for bit.
#[allow(clippy::too_many_arguments)]
fn write_view(doc: &mut CadDocument, common: EntityCommon, name: &str, origin: Vector2, (lo, hi): (f64, f64), style: u64, line: u64, sections: &[u64]) -> Result<Handle, DxfError> {
    let mut refs = vec![soft_pointer(0), Ref(DwgReferenceType::HardOwnership, 0), hard_pointer(style), hard_pointer(line)];
    for s in sections {
        refs.extend([soft_pointer(*s), hard_pointer(0), hard_pointer(0)]);
    }
    refs.extend([hard_pointer(0), hard_pointer(0), hard_pointer(0), hard_pointer(0), soft_pointer(0), hard_pointer(0)]);
    let strings = ["", "", name, "", name, ""].iter().map(|s| s.to_string()).collect();
    let corners = [Vector2::new(lo, ELEVATION_MIN), Vector2::new(0.0, ELEVATION_MIN), Vector2::new(hi, ELEVATION_MAX)];
    let value = |v: Value| match v {
        Value::OffsetMin => lo,
        Value::OffsetMax => hi,
        Value::Width => hi - lo,
        Value::Center => (lo + hi) / 2.0,
    };
    put_entity(
        doc,
        "AECC_GRAPH_SECTION_VIEW",
        common,
        |w: &mut DwgMergedWriter| {
            w.write_bit_long(80);
            bits(w, "11");
            for v in GRAPH_WORDS {
                w.write_bit_long(v);
            }
            w.write_bit_long(5);
            w.write_bit_long(GRAPH_KEYS.len() as i32);
            for (k, v) in GRAPH_KEYS {
                w.write_bit_long(k as i32);
                if k >> 24 == 3 {
                    w.write_byte(v as u8);
                } else {
                    w.write_bit_long(v);
                }
            }
            bl_byte(w, 0);
            bits(w, &"0".repeat(121));
            point(w, origin);
            for v in [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0] {
                w.write_bit_double(v);
            }
            w.write_bit_long(sections.len() as i32);
            for _ in sections {
                bits(w, "10000");
            }
            w.write_bit_long(0);
            w.write_bit(true);
            point(w, origin);
            w.write_bit_long(4);
            w.write_bit_long(VIEW_TEXT_ID);
            for p in corners.iter().chain(&corners) {
                point(w, *p);
            }
            w.write_bit_long(528);
            text(w, TITLE);
            w.write_bit_long(0);
            for v in [0.0, ELEVATION_MAX - ELEVATION_MIN, 0.0, 0.0, 1.0 / 48.0, 0.0, 1.0 / 120.0] {
                w.write_bit_double(v);
            }
            for v in [0, 0, 8, 1, 4, VIEW_TEXT_ID] {
                w.write_bit_long(v);
            }
            point(w, origin);
            for v in [0.0, 0.0, 0.0, 1.0, 1.0] {
                w.write_bit_double(v);
            }
            for piece in GRAPH {
                match piece {
                    Piece::Bits(b) => bits(w, b),
                    Piece::Value(v) => w.write_bit_double(value(*v)),
                    Piece::Title => text(w, TITLE),
                }
            }
        },
        Body { refs, strings },
    )
}

/// Adds a section view `name` of the sample line `line`, its point of
/// offset 0 at the lowest elevation at `origin`, on `layer` (layer "0"
/// when `None`), with the drawing's default section view style and the
/// default range: the line's offsets and the section's elevations padded
/// by 20 and rounded out to 10. The line must be one this crate writes
/// (it is written again with the view in its list) and have at most one
/// section, not yet sampled (as [`super::add_sections`] writes it): the
/// view spans elevations -20 to 20.
pub fn add_section_view(doc: &mut CadDocument, line: Handle, name: &str, origin: Vector2, layer: Option<&str>) -> Result<Handle, DxfError> {
    if name.is_empty() {
        return Err(invalid("a section view needs a name"));
    }
    if !origin.x.is_finite() || !origin.y.is_finite() {
        return Err(invalid("a section view needs a finite origin"));
    }
    let mut l = read_line(doc, line).ok_or_else(|| invalid("the sample line is not one this crate writes"))?;
    if l.sections.len() > 1 {
        return Err(invalid("views of a line with several sections are not written"));
    }
    if l.sections.iter().any(|s| arch(doc, Handle::new(*s)).and_then(|a| a.section_geometry()).is_some_and(|g| !g.points.is_empty())) {
        return Err(invalid("views of a sampled section are not written"));
    }
    let style = default_style_of(doc, KEY_SECTION_VIEW_STYLES).ok_or_else(|| invalid("drawing has no section view styles"))?;
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let handle = doc.allocate_handle();
    let common = EntityCommon { handle, owner_handle: doc.header.model_space_block_handle, layer: layer.into(), layer_handle: Some(layer_handle), ..EntityCommon::default() };
    let sections = l.sections.clone();
    write_view(doc, common, name, origin, range(-l.offsets[0], l.offsets[1]), style, line.value(), &sections)?;
    l.views.push(handle.value());
    write_line(doc, &l)?;
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classes::civil::create::sample_lines::KEY_SECTION_STYLES;
    use crate::classes::civil::create::tree::{DEFAULT_STYLE_ID, KEY_STYLES};
    use crate::classes::civil::SectionViewFrame;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::DxfVersion;

    /// A stand-in style collection: a tree node `key` under `owner` with
    /// the given children.
    fn node(doc: &mut CadDocument, handle: Handle, owner: Handle, key: i32, children: &[(i32, Handle)]) {
        put_object(doc, "AECC_TREE_NODE", handle, owner, Vec::new(), |w| {
            w.write_bit_long(key);
            w.write_bit_long(children.len() as i32);
            for c in children {
                w.write_bit_long(c.0);
            }
            w.write_bit_long(0);
            bits(w, "010");
        }, Body { refs: children.iter().map(|c| soft_owner(c.1.value())).collect(), strings: vec![String::new()] })
        .unwrap();
    }

    /// A collection `key` of the style tree holding one default style.
    fn collection(doc: &mut CadDocument, path: &[i32]) -> Handle {
        let tree = root_node(doc, KEY_STYLES).unwrap();
        let nodes: Vec<Handle> = path.iter().map(|_| doc.allocate_handle()).collect();
        for (i, key) in path.iter().enumerate() {
            let owner = if i == 0 { tree } else { nodes[i - 1] };
            let children: Vec<(i32, Handle)> = nodes.get(i + 1).map(|n| vec![(path[i + 1], *n)]).unwrap_or_default();
            node(doc, nodes[i], owner, *key, &children);
        }
        add_node_child(doc, tree, path[0], nodes[0], "").unwrap();
        *nodes.last().unwrap()
    }

    #[test]
    fn views_take_the_line_offsets_and_join_the_line() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        collection(&mut doc, &[KEY_SECTION_STYLES, DEFAULT_STYLE_ID]);
        let g = crate::classes::civil::add_sample_line_group(&mut doc, al, "SLG1", None).unwrap();
        let a = crate::classes::civil::add_sample_line(&mut doc, g, "SL1", 50.0, 3.0, 33.0, None).unwrap();
        let b = crate::classes::civil::add_sample_line(&mut doc, g, "SL2", 80.0, 10.0, 10.0, None).unwrap();
        let p = |x: f64, y: f64| crate::types::Vector3::new(x, y, 10.0);
        let surface = crate::classes::civil::add_tin_surface(&mut doc, "S1", &[p(-10.0, -40.0), p(110.0, -40.0), p(110.0, 40.0)], None).unwrap();
        crate::classes::civil::add_sections(&mut doc, g, surface).unwrap();
        // No section view style yet.
        assert!(add_section_view(&mut doc, a, "SV1", v(200.0, 0.0), None).is_err());
        let style = collection(&mut doc, &[KEY_SECTION_VIEW_STYLES, DEFAULT_STYLE_ID]);
        assert!(add_section_view(&mut doc, a, "", v(200.0, 0.0), None).is_err());
        assert!(add_section_view(&mut doc, g, "SV1", v(200.0, 0.0), None).is_err());
        let first = add_section_view(&mut doc, a, "SV1", v(200.0, 0.0), None).unwrap();
        let second = add_section_view(&mut doc, a, "SV1b", v(200.0, 100.0), None).unwrap();
        let third = add_section_view(&mut doc, b, "SV2", v(300.0, 0.0), None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-section-views-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let frame = |h: Handle| arch(&back, h).unwrap().section_view_frame().unwrap();
        assert_eq!(frame(first), SectionViewFrame { origin: v(200.0, 0.0), offset_min: -30.0, offset_max: 60.0, elevation_min: -20.0, elevation_max: 20.0, vertical_scale: 1.0 });
        assert_eq!(frame(second).origin, v(200.0, 100.0));
        assert_eq!((frame(third).offset_min, frame(third).offset_max), (-30.0, 30.0));
        let line = read_line(&back, a).unwrap();
        assert_eq!(line.views, vec![first.value(), second.value()]);
        let view = arch(&back, first).unwrap();
        let refs: Vec<u64> = view.typed_handles().iter().map(|r| r.0).collect();
        assert_eq!(&refs[2..5], &[style.value(), a.value(), line.sections[0]]);
        assert_eq!(view.decode().strings[2], "SV1");
    }
}
