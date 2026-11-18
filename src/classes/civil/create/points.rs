//! COGO points, the point database and the `_All Points` group.

use super::support::*;
use crate::classes::civil::CogoPoint;
use crate::entities::{EntityCommon, EntityType, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::{Handle, Vector3};
use crate::CadDocument;

/// Id of the `_All Points` group in the point group node.
const ALL_POINTS_ID: i32 = 33;
/// Text id of the `_All Points` query (the application numbers texts
/// `0x80000000 | n` per session; a written drawing can number its own).
const QUERY_TAG: i32 = 0x8000_0001_u32 as i32;
/// First text id given to point descriptions.
const FIRST_DESCRIPTION_TAG: u32 = 0x8000_0002;

/// The bits after the point numbers of `_All Points`: the same 564 bits in
/// WP01, WP02, WP06, WP07, WP08 and WP10 (the group's override and query
/// settings, not mapped field by field).
const ALL_POINTS_TAIL: &str = "001000010000001000110010110101011110000001110110101111000101010100010000010000001000110010110101011110000001110110101111000101010100010000010000001000110010110101011110000001110110101111000101010100010000010000001000110010110101011110000001110110101111000101011100010000010000001000110010110101011110000001110110101111000101011100010000010000001000110010110101011110000001110110101111000101011100010010100101010111101010101010100000000000000000000000000000000000000000000000000000000000000000101010000000000000000000000000000000000000000000000000000000000000000010";
/// Outer `_All Points` query statement after `BL 75, BL 1, BL tag`
/// (the same in every sample).
const QUERY_OUTER_TAIL: &str = "100100010";
/// Inner query statement after `BL tag, BL 0, BL 0`: the query tree for
/// `PointNumber > 0`: 113 bits, the same in every sample.
const QUERY_INNER_TAIL: &str = "01000010000000100000000010101000000000000000000011100000011000000000000000001010100000000000000000001010001001110";
const QUERY_INNER_STRINGS: [&str; 12] = [
    "Point Group Query - _All Points", "", "0", "AeccQueryNodePARENS", "", "AeccQueryNodeGT", "AeccQueryNodeATTRID", "", "", "AeccQueryNodeCONST", "", "",
];

/// Points of the drawing, in database order (empty without a database).
pub fn cogo_points(doc: &CadDocument) -> Vec<CogoPoint> {
    root_node(doc, KEY_POINTS).and_then(|h| object(doc, h)).map(|o| o.cogo_points()).unwrap_or_default()
}

fn finite(p: Vector3) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.z.is_finite()
}

/// Handles of the point support objects, created on the first point.
struct Support {
    database: Handle,
    group_node: Handle,
    all_points: Handle,
    class_node: u64,
    defaults: Defaults,
}

/// The drawing's default point styles and the nodes a point group and the
/// database refer to.
#[derive(Clone, Copy)]
struct Defaults {
    point_style: u64,
    label_style: u64,
    /// Settings node of the create-points command (a point group's).
    create_settings: u64,
    /// Class node of COGO points (the database's last reference).
    class_node: u64,
}

/// Settings node named `name` (its first text).
fn settings_node(doc: &CadDocument, name: &str) -> Option<(u64, Vec<u64>)> {
    doc.objects.iter().find_map(|(h, o)| match o {
        crate::objects::ObjectType::Arch(a) if a.dxf_name == "AECC_SETTINGS_NODE" => {
            let d = a.decode();
            (d.strings.first().map(String::as_str) == Some(name)).then(|| (h.value(), d.handles))
        }
        _ => None,
    })
}

/// Finds the defaults in the drawing's settings tree: the points settings
/// node ("AeccPoints") ends with the default label style and point style,
/// the create-points settings node ("AeccCreatePoints") is what a group
/// refers to, and the class node of COGO points is the class node named
/// "AeccDbCogoPoint" (its first text; the parent class node lists every
/// class name). `None` when the drawing has no settings tree (see
/// docs/civil-write.md: the styles are not written by this crate yet).
fn defaults(doc: &CadDocument) -> Option<Defaults> {
    let (_, points) = settings_node(doc, "AeccPoints")?;
    let [.., label_style, point_style] = points[..] else { return None };
    let (create_settings, _) = settings_node(doc, "AeccCreatePoints")?;
    let class_node = doc.objects.iter().find_map(|(h, o)| match o {
        crate::objects::ObjectType::Arch(a) if a.dxf_name == "AECC_CLASS_NODE" && a.decode().strings.first().map(String::as_str) == Some("AeccDbCogoPoint") => Some(h.value()),
        _ => None,
    })?;
    (point_style != 0 && label_style != 0).then_some(Defaults { point_style, label_style, create_settings, class_node })
}

fn support(doc: &mut CadDocument) -> Result<Support, DxfError> {
    let defaults = defaults(doc).ok_or_else(|| invalid("drawing has no civil point styles"))?;
    ensure_civil(doc)?;
    if let (Some(database), Some(group_node)) = (root_node(doc, KEY_POINTS), root_node(doc, KEY_POINT_GROUPS)) {
        let groups = object(doc, group_node).map(|o| o.decode().handles).unwrap_or_default();
        let all_points = groups.into_iter().map(Handle::new).find(|h| object(doc, *h).is_some_and(|o| o.decode().strings.iter().any(|s| s == "AeccImpAllPointsPG")));
        let all_points = all_points.ok_or_else(|| invalid("point group node has no _All Points group"))?;
        let class_node = object(doc, database).and_then(|o| o.decode().handles.last().copied()).unwrap_or(0);
        return Ok(Support { database, group_node, all_points, class_node, defaults });
    }
    let root = ensure_civil(doc)?;
    let database = doc.allocate_handle();
    let group_node = doc.allocate_handle();
    let all_points = doc.allocate_handle();
    let outer = doc.allocate_handle();
    let inner = doc.allocate_handle();
    write_database(doc, database, root, &[], defaults.class_node)?;
    write_group_node(doc, group_node, root, &[(ALL_POINTS_ID, all_points)], defaults)?;
    write_outer_query(doc, outer, root, inner, QUERY_TAG)?;
    write_inner_query(doc, inner, outer, QUERY_TAG)?;
    let s = Support { database, group_node, all_points, class_node: defaults.class_node, defaults };
    write_all_points(doc, &s, inner, &[])?;
    set_root_entry(doc, KEY_POINTS, database)?;
    set_root_entry(doc, KEY_POINT_GROUPS, group_node)?;
    set_root_entry(doc, KEY_ALL_POINTS_QUERY, outer)?;
    Ok(s)
}

/// The point group node (Root key 32): group ids in draw order and the
/// drawing's default label and point styles.
fn write_group_node(doc: &mut CadDocument, handle: Handle, root: Handle, groups: &[(i32, Handle)], defaults: Defaults) -> Result<(), DxfError> {
    let mut refs: Vec<Ref> = groups.iter().map(|g| soft_owner(g.1.value())).collect();
    refs.push(hard_pointer(defaults.label_style));
    refs.push(hard_pointer(defaults.point_style));
    put_object(
        doc,
        "AECC_POINT_GROUP_NODE",
        handle,
        root,
        Vec::new(),
        |w| {
            w.write_bit_long(KEY_POINT_GROUPS);
            w.write_bit_long(groups.len() as i32);
            for g in groups {
                w.write_bit_long(g.0);
            }
            w.write_bit_long(0);
            bits(w, "010");
        },
        Body { refs, strings: vec![String::new()] },
    )
}

/// Outer `_All Points` query statement (Root key 75).
fn write_outer_query(doc: &mut CadDocument, handle: Handle, root: Handle, inner: Handle, tag: i32) -> Result<(), DxfError> {
    put_object(
        doc,
        "AECC_QUERY_STATEMENT",
        handle,
        root,
        Vec::new(),
        |w| {
            w.write_bit_long(KEY_ALL_POINTS_QUERY);
            w.write_bit_long(1);
            w.write_bit_long(tag);
            bits(w, QUERY_OUTER_TAIL);
        },
        Body { refs: vec![soft_owner(inner.value())], strings: vec![String::new(), "Point Group Query - _All Points".into(), String::new()] },
    )
}

/// Inner query statement: the query tree for `PointNumber > 0`.
fn write_inner_query(doc: &mut CadDocument, handle: Handle, outer: Handle, tag: i32) -> Result<(), DxfError> {
    put_object(
        doc,
        "AECC_QUERY_STATEMENT",
        handle,
        outer,
        Vec::new(),
        |w| {
            w.write_bit_long(tag);
            w.write_bit_long(0);
            w.write_bit_long(0);
            bits(w, QUERY_INNER_TAIL);
        },
        Body { refs: Vec::new(), strings: QUERY_INNER_STRINGS.iter().map(|s| s.to_string()).collect() },
    )
}

/// The `_All Points` group holding `numbers` (ascending); `query` is the
/// inner query statement.
fn write_all_points(doc: &mut CadDocument, s: &Support, query: Handle, numbers: &[u32]) -> Result<(), DxfError> {
    let mut numbers = numbers.to_vec();
    numbers.sort_unstable();
    // Styles and the settings node stay as the group has them.
    let old = object(doc, s.all_points).map(|o| o.decode().handles).unwrap_or_default();
    let keep = |i: usize, new: u64| old.get(i).copied().unwrap_or(new);
    let refs = [keep(0, 0), keep(1, s.defaults.point_style), query.value(), s.database.value(), s.group_node.value(), keep(5, s.defaults.create_settings), 0, 0];
    put_object(
        doc,
        "AECC_POINT_PG",
        s.all_points,
        s.group_node,
        Vec::new(),
        |w| {
            w.write_bit_long(ALL_POINTS_ID);
            w.write_bit_long(0);
            w.write_bit_long(0);
            // `a` = 0 is stored in its byte form; then 5 + 6 constant bits.
            bl_byte(w, 0);
            bits(w, "00000100000");
            w.write_bit_long(0); // serial
            w.write_bit_long(numbers.len() as i32);
            for n in &numbers {
                w.write_raw_long(*n as i32);
            }
            bits(w, ALL_POINTS_TAIL);
        },
        // label style, point style, query, database, group node, settings node, 0, 0
        Body {
            refs: refs.into_iter().map(hard_pointer).collect(),
            strings: vec![String::new(), "AeccImpAllPointsPG".into()],
        },
    )
}

fn write_database(doc: &mut CadDocument, handle: Handle, root: Handle, points: &[CogoPoint], class_node: u64) -> Result<(), DxfError> {
    let styled: Vec<&CogoPoint> = points.iter().filter(|p| p.style != 0).collect();
    let labelled: Vec<&CogoPoint> = points.iter().filter(|p| p.label_style != 0).collect();
    let described: Vec<&CogoPoint> = points.iter().filter(|p| p.description.is_some()).collect();
    let mut texts: Vec<&str> = Vec::new();
    let tags: Vec<i32> = described
        .iter()
        .map(|p| {
            let text = p.description.as_deref().unwrap_or("");
            let i = texts.iter().position(|t| *t == text).unwrap_or_else(|| {
                texts.push(text);
                texts.len() - 1
            });
            (FIRST_DESCRIPTION_TAG + i as u32) as i32
        })
        .collect();
    let mut refs: Vec<Ref> = styled.iter().map(|p| hard_pointer(p.style)).collect();
    refs.extend(labelled.iter().map(|p| hard_pointer(p.label_style)));
    refs.push(soft_pointer(class_node));
    let mut strings = vec![String::new()];
    strings.extend(described.iter().map(|p| p.description.clone().unwrap_or_default()));
    let list = |w: &mut DwgMergedWriter, v: &[&CogoPoint]| {
        w.write_bit_long(v.len() as i32);
        for p in v {
            w.write_bit_long(p.number as i32);
        }
    };
    put_object(
        doc,
        "AECC_POINT_NODE",
        handle,
        root,
        Vec::new(),
        |w| {
            w.write_bit_long(KEY_POINTS);
            w.write_bit_long(0);
            w.write_bit_long(0);
            bits(w, "010");
            w.write_bit_long(points.len() as i32);
            for p in points {
                w.write_bit_long(1);
                w.write_bit_double(p.position.x);
                w.write_bit_double(p.position.y);
                w.write_bit_double(p.position.z);
                w.write_bit_long(p.number as i32);
                w.write_bit_short(0x0420);
            }
            list(w, &styled);
            list(w, &labelled);
            for _ in 0..3 {
                w.write_bit_long(0);
            }
            w.write_bit_long(described.len() as i32);
            for (p, tag) in described.iter().zip(&tags) {
                w.write_bit_long(p.number as i32);
                w.write_bit_long(*tag);
            }
            for _ in 0..13 {
                w.write_bit_long(0);
            }
        },
        Body { refs, strings },
    )
}

/// Rewrites the database and `_All Points` from `points`.
fn store(doc: &mut CadDocument, s: &Support, points: &[CogoPoint]) -> Result<(), DxfError> {
    let root = root(doc).ok_or_else(|| invalid("drawing has no root tree node"))?;
    write_database(doc, s.database, root, points, s.class_node)?;
    let query = object(doc, s.all_points).and_then(|o| o.decode().handles.get(2).copied()).map(Handle::new).unwrap_or(Handle::NULL);
    let numbers: Vec<u32> = points.iter().map(|p| p.number).collect();
    write_all_points(doc, s, query, &numbers)
}

fn put_point(doc: &mut CadDocument, handle: Option<Handle>, number: u32, layer: Option<&str>) -> Result<Handle, DxfError> {
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let common = match handle.and_then(|h| doc.get_entity(h)) {
        Some(e) => e.common().clone(),
        None => EntityCommon {
            handle: doc.allocate_handle(),
            owner_handle: doc.header.model_space_block_handle,
            layer: layer.into(),
            layer_handle: Some(layer_handle),
            ..EntityCommon::default()
        },
    };
    put_entity(
        doc,
        "AECC_COGO_POINT",
        common,
        |w| {
            w.write_bit_long(number as i32);
            bits(w, "001010");
        },
        Body { refs: Vec::new(), strings: Vec::new() },
    )
}

/// Adds a COGO point. `number` `None` takes the next free number;
/// `style` `None` gives it the drawing's default point style. The drawing
/// must hold the civil settings and style tree (one the reference
/// application has opened); a new drawing returns an error.
pub fn add_cogo_point(
    doc: &mut CadDocument,
    number: Option<u32>,
    position: Vector3,
    description: Option<&str>,
    style: Option<Handle>,
    layer: Option<&str>,
) -> Result<Handle, DxfError> {
    if !finite(position) {
        return Err(invalid("point position must be finite"));
    }
    if layer.is_some_and(|l| doc.layers.get(l).is_none()) {
        return Err(invalid("layer does not exist"));
    }
    let mut points = cogo_points(doc);
    let number = number.unwrap_or_else(|| points.iter().map(|p| p.number).max().unwrap_or(0) + 1);
    if number == 0 || number > i32::MAX as u32 || points.iter().any(|p| p.number == number) {
        return Err(invalid("point number is zero or already used"));
    }
    let s = support(doc)?;
    points.push(CogoPoint {
        number,
        position,
        style: style.map_or(s.defaults.point_style, |h| h.value()),
        label_style: s.defaults.label_style,
        description: description.map(str::to_string),
    });
    store(doc, &s, &points)?;
    put_point(doc, None, number, layer)
}

/// Changes a point's number, position and description.
pub fn set_cogo_point(doc: &mut CadDocument, point: Handle, number: u32, position: Vector3, description: Option<&str>) -> Result<(), DxfError> {
    if !finite(position) || number == 0 || number > i32::MAX as u32 {
        return Err(invalid("point number or position is invalid"));
    }
    let old = point_number(doc, point)?;
    let mut points = cogo_points(doc);
    if number != old && points.iter().any(|p| p.number == number) {
        return Err(invalid("point number already used"));
    }
    let p = points.iter_mut().find(|p| p.number == old).ok_or_else(|| invalid("point is not in the database"))?;
    p.number = number;
    p.position = position;
    p.description = description.map(str::to_string);
    let s = support(doc)?;
    store(doc, &s, &points)?;
    put_point(doc, Some(point), number, None).map(|_| ())
}

/// Removes a point from the drawing and from the database.
pub fn remove_cogo_point(doc: &mut CadDocument, point: Handle) -> Result<(), DxfError> {
    let number = point_number(doc, point)?;
    let s = support(doc)?;
    let points: Vec<CogoPoint> = cogo_points(doc).into_iter().filter(|p| p.number != number).collect();
    store(doc, &s, &points)?;
    doc.remove_entity(point);
    Ok(())
}

fn point_number(doc: &CadDocument, point: Handle) -> Result<u32, DxfError> {
    match doc.get_entity(point) {
        Some(EntityType::Extended(x)) => match &x.data {
            ExtendedEntityData::Arch(d) => d.cogo_point_number(),
            _ => None,
        },
        _ => None,
    }
    .ok_or_else(|| invalid("handle is not a COGO point"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::DxfVersion;

    /// A drawing with stand-in settings and class nodes (not the reference
    /// application's records: only the texts and references the point
    /// writer looks up) and two stand-in style objects.
    fn drawing() -> CadDocument {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let root = ensure_civil(&mut doc).unwrap();
        let [point_style, label_style, points, create, class] = [(); 5].map(|_| doc.allocate_handle());
        let node = |name: &str, refs: Vec<u64>| Body { refs: refs.into_iter().map(hard_pointer).collect(), strings: vec![name.into()] };
        put_object(&mut doc, "AECC_SETTINGS_NODE", point_style, root, Vec::new(), |_| {}, node("style", vec![])).unwrap();
        put_object(&mut doc, "AECC_SETTINGS_NODE", label_style, root, Vec::new(), |_| {}, node("label", vec![])).unwrap();
        put_object(&mut doc, "AECC_SETTINGS_NODE", points, root, Vec::new(), |_| {}, node("AeccPoints", vec![label_style.value(), point_style.value()])).unwrap();
        put_object(&mut doc, "AECC_SETTINGS_NODE", create, root, Vec::new(), |_| {}, node("AeccCreatePoints", vec![])).unwrap();
        put_object(&mut doc, "AECC_CLASS_NODE", class, root, Vec::new(), |_| {}, node("AeccDbCogoPoint", vec![])).unwrap();
        doc
    }

    fn round_trip(doc: &CadDocument) -> CadDocument {
        let path = std::env::temp_dir().join(format!("civil-points-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        back
    }

    fn handle_of(doc: &CadDocument, number: u32) -> Handle {
        doc.entities()
            .find_map(|e| match e {
                EntityType::Extended(x) => match &x.data {
                    ExtendedEntityData::Arch(d) if d.cogo_point_number() == Some(number) => Some(x.common.handle),
                    _ => None,
                },
                _ => None,
            })
            .unwrap()
    }

    #[test]
    fn a_new_drawing_without_styles_is_refused_unchanged() {
        let mut doc = CadDocument::new();
        let before = doc.objects.len();
        assert!(add_cogo_point(&mut doc, None, Vector3::new(1.0, 2.0, 3.0), None, None, None).is_err());
        assert_eq!(doc.objects.len(), before);
    }

    #[test]
    fn points_round_trip() {
        let mut doc = drawing();
        add_cogo_point(&mut doc, Some(1), Vector3::new(100.0, 200.0, 10.0), Some("TEST"), None, None).unwrap();
        add_cogo_point(&mut doc, None, Vector3::new(150.0, 250.0, 12.0), None, None, None).unwrap();
        add_cogo_point(&mut doc, Some(5), Vector3::new(200.0, 210.0, 14.0), Some("TEST"), None, None).unwrap();
        assert!(add_cogo_point(&mut doc, Some(5), Vector3::ZERO, None, None, None).is_err());
        assert!(add_cogo_point(&mut doc, None, Vector3::new(f64::NAN, 0.0, 0.0), None, None, None).is_err());
        let back = round_trip(&doc);
        let points = cogo_points(&back);
        let summary: Vec<_> = points.iter().map(|p| (p.number, p.position, p.description.clone())).collect();
        assert_eq!(
            summary,
            vec![
                (1, Vector3::new(100.0, 200.0, 10.0), Some("TEST".into())),
                (2, Vector3::new(150.0, 250.0, 12.0), None),
                (5, Vector3::new(200.0, 210.0, 14.0), Some("TEST".into())),
            ]
        );
        assert!(points.iter().all(|p| p.style != 0 && p.label_style != 0));
        let group = object(&back, root_node(&back, KEY_POINT_GROUPS).unwrap()).unwrap().decode().handles[0];
        assert_eq!(object(&back, Handle::new(group)).unwrap().point_group_numbers(), vec![1, 2, 5]);

        let mut doc = back;
        let p2 = handle_of(&doc, 2);
        set_cogo_point(&mut doc, p2, 7, Vector3::new(1.0, 2.0, 3.0), Some("X")).unwrap();
        let p1 = handle_of(&doc, 1);
        remove_cogo_point(&mut doc, p1).unwrap();
        let back = round_trip(&doc);
        let summary: Vec<_> = cogo_points(&back).iter().map(|p| (p.number, p.position, p.description.clone())).collect();
        assert_eq!(summary, vec![(7, Vector3::new(1.0, 2.0, 3.0), Some("X".into())), (5, Vector3::new(200.0, 210.0, 14.0), Some("TEST".into()))]);
        assert_eq!(handle_of(&back, 7), p2);
        let group = object(&back, root_node(&back, KEY_POINT_GROUPS).unwrap()).unwrap().decode().handles[0];
        assert_eq!(object(&back, Handle::new(group)).unwrap().point_group_numbers(), vec![5, 7]);
    }
}
