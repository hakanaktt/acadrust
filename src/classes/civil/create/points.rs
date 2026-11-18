//! COGO points, the point database and the `_All Points` group.

use super::groups::{refresh_groups, ALL_POINTS_ID, set_query, write_group, write_group_node, Entry, Group, GREATER};
use super::support::*;
use crate::classes::civil::CogoPoint;
use crate::entities::{EntityCommon, EntityType, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::{Handle, Vector3};
use crate::CadDocument;

/// Text id of the `_All Points` query (the application numbers texts
/// `0x80000000 | n` per session; a written drawing can number its own).
const QUERY_TAG: i32 = 0x8000_0001_u32 as i32;
/// First text id given to point descriptions.
const FIRST_DESCRIPTION_TAG: u32 = 0x8000_0002;

/// Points of the drawing, in database order (empty without a database).
pub fn cogo_points(doc: &CadDocument) -> Vec<CogoPoint> {
    root_node(doc, KEY_POINTS).and_then(|h| object(doc, h)).map(|o| o.cogo_points()).unwrap_or_default()
}

fn finite(p: Vector3) -> bool {
    p.x.is_finite() && p.y.is_finite() && p.z.is_finite()
}

/// Handles of the point support objects, created on the first point.
pub(super) struct Support {
    pub database: Handle,
    pub group_node: Handle,
    pub all_points: Handle,
    pub class_node: u64,
    pub defaults: Defaults,
}

/// The drawing's default point styles and the nodes a point group and the
/// database refer to.
#[derive(Clone, Copy)]
pub(super) struct Defaults {
    pub point_style: u64,
    pub label_style: u64,
    /// Settings node of the create-points command (a point group's).
    pub create_settings: u64,
    /// Class node of COGO points (the database's last reference).
    pub class_node: u64,
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

pub(super) fn support(doc: &mut CadDocument) -> Result<Support, DxfError> {
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
    write_database(doc, database, root, &[], defaults.class_node)?;
    let all = Entry { id: ALL_POINTS_ID, handle: all_points, name: String::new() };
    write_group_node(doc, group_node, root, &[all], defaults.label_style, defaults.point_style)?;
    let inner = set_query(doc, "Point Group Query - _All Points", QUERY_TAG, GREATER, &[0])?;
    let s = Support { database, group_node, all_points, class_node: defaults.class_node, defaults };
    write_all_points(doc, &s, inner, &[])?;
    set_root_entry(doc, KEY_POINTS, database)?;
    set_root_entry(doc, KEY_POINT_GROUPS, group_node)?;
    Ok(s)
}

/// The `_All Points` group holding `numbers` (ascending); `query` is the
/// inner query statement.
fn write_all_points(doc: &mut CadDocument, s: &Support, query: Handle, numbers: &[u32]) -> Result<(), DxfError> {
    let mut numbers = numbers.to_vec();
    numbers.sort_unstable();
    // Styles and the settings node stay as the group has them.
    let old = object(doc, s.all_points).map(|o| o.decode().handles).unwrap_or_default();
    let keep = |i: usize, new: u64| old.get(i).copied().unwrap_or(new);
    let refs = [keep(0, 0), keep(1, s.defaults.point_style), query.value(), s.database.value(), s.group_node.value(), keep(5, s.defaults.create_settings)];
    let g = Group { id: ALL_POINTS_ID, serial: 0, name: None, description: "", numbers, extents: None, refs };
    write_group(doc, s.all_points, s.group_node, &g)
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
    write_all_points(doc, s, query, &numbers)?;
    refresh_groups(doc, s, points)
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
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("civil-points-{}-{n}.dwg", std::process::id()));
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

    #[test]
    fn number_list_groups_follow_their_points() {
        let mut doc = drawing();
        for n in 1..=6 {
            add_cogo_point(&mut doc, Some(n), Vector3::new(10.0 * n as f64, 5.0, n as f64), None, None, None).unwrap();
        }
        let g = crate::classes::civil::add_point_group(&mut doc, "G", Some("even"), &[2, 4, 6, 8]).unwrap();
        assert!(crate::classes::civil::add_point_group(&mut doc, "g", None, &[1]).is_err());
        let back = round_trip(&doc);
        assert_eq!(object(&back, g).unwrap().point_group_numbers(), vec![2, 4, 6]);
        let group_node = root_node(&back, KEY_POINT_GROUPS).unwrap();
        let names: Vec<_> = super::super::groups::entries(&back, group_node).unwrap().into_iter().map(|e| e.name).collect();
        assert_eq!(names, vec!["".to_string(), "G".into()]);

        // A new point with a listed number joins; a removed one leaves.
        let mut doc = back;
        add_cogo_point(&mut doc, Some(8), Vector3::new(1.0, 1.0, 1.0), None, None, None).unwrap();
        let p4 = handle_of(&doc, 4);
        remove_cogo_point(&mut doc, p4).unwrap();
        crate::classes::civil::set_point_group(&mut doc, g, "H", None, &[1, 8]).unwrap();
        let back = round_trip(&doc);
        assert_eq!(object(&back, g).unwrap().point_group_numbers(), vec![1, 8]);
        let all = object(&back, group_node).unwrap().decode().handles[0];
        assert_eq!(object(&back, Handle::new(all)).unwrap().point_group_numbers(), vec![1, 2, 3, 5, 6, 8]);
    }
}
