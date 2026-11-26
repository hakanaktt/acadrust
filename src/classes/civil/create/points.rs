//! COGO points, the point database and the `_All Points` group.

use super::groups::{all_points_query, refresh_groups, set_query, write_group, write_group_node, Entry, Group, ALL_POINTS_ID};
use super::support::*;
use crate::classes::civil::cogo::key_code;
use crate::classes::civil::{CogoPoint, PropertyValue};
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
/// class name). `None` when the drawing has no settings tree; a label
/// style of 0 (none) is accepted.
fn defaults(doc: &CadDocument) -> Option<Defaults> {
    let (_, points) = settings_node(doc, "AeccPoints")?;
    let [.., label_style, point_style] = points[..] else { return None };
    let (create_settings, _) = settings_node(doc, "AeccCreatePoints")?;
    let class_node = doc.objects.iter().find_map(|(h, o)| match o {
        crate::objects::ObjectType::Arch(a) if a.dxf_name == "AECC_CLASS_NODE" && a.decode().strings.first().map(String::as_str) == Some("AeccDbCogoPoint") => Some(h.value()),
        _ => None,
    })?;
    (point_style != 0).then_some(Defaults { point_style, label_style, create_settings, class_node })
}

pub(super) fn support(doc: &mut CadDocument) -> Result<Support, DxfError> {
    let defaults = match defaults(doc) {
        Some(d) => d,
        None if super::tree::ensure_point_tree(doc)? => defaults(doc).ok_or_else(|| invalid("written point tree cannot be read"))?,
        None => return Err(invalid("drawing has a civil tree without point styles")),
    };
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
    write_database(doc, database, root, &[], defaults.class_node, &[])?;
    let all = Entry { id: ALL_POINTS_ID, handle: all_points, name: String::new() };
    write_group_node(doc, group_node, root, &[all], defaults.label_style, defaults.point_style)?;
    let inner = set_query(doc, "Point Group Query - _All Points", QUERY_TAG, all_points_query())?;
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
    let settings = super::groups::group_settings(doc, s.all_points).unwrap_or_default();
    let g = Group { id: ALL_POINTS_ID, serial: 0, name: None, description: "", numbers, extents: None, refs, settings };
    write_group(doc, s.all_points, s.group_node, &g)
}

fn write_database<'a>(doc: &mut CadDocument, handle: Handle, root: Handle, points: &'a [CogoPoint], class_node: u64, keys: &[(String, i32)]) -> Result<(), DxfError> {
    let styled: Vec<&CogoPoint> = points.iter().filter(|p| p.style != 0).collect();
    let labelled: Vec<&CogoPoint> = points.iter().filter(|p| p.label_style != 0).collect();
    let named: Vec<&CogoPoint> = points.iter().filter(|p| p.name.is_some()).collect();
    let described: Vec<&CogoPoint> = points.iter().filter(|p| p.description.is_some()).collect();
    // Names and descriptions share the text ids; equal texts share one.
    let mut texts: Vec<&str> = Vec::new();
    let mut tag = |text: &'a str| {
        let i = texts.iter().position(|t| *t == text).unwrap_or_else(|| {
            texts.push(text);
            texts.len() - 1
        });
        (FIRST_DESCRIPTION_TAG + i as u32) as i32
    };
    let name_tags: Vec<i32> = named.iter().map(|p| tag(p.name.as_deref().unwrap_or(""))).collect();
    let tags: Vec<i32> = described.iter().map(|p| tag(p.description.as_deref().unwrap_or(""))).collect();
    let mut refs: Vec<Ref> = styled.iter().map(|p| hard_pointer(p.style)).collect();
    refs.extend(labelled.iter().map(|p| hard_pointer(p.label_style)));
    refs.push(soft_pointer(class_node));
    let mut strings = vec![String::new()];
    for _ in 0..2 {
        strings.extend(named.iter().map(|p| p.name.clone().unwrap_or_default()));
    }
    strings.extend(described.iter().map(|p| p.description.clone().unwrap_or_default()));
    // Property values by slot, one entry per key in key order, points in
    // database order; each entry's key text, then its text values.
    let mut slots: Vec<Vec<(i32, Vec<(u32, &PropertyValue)>)>> = vec![Vec::new(); 13];
    for (text, key) in keys {
        let values: Vec<(u32, &PropertyValue)> = points.iter().filter_map(|p| p.properties.iter().find(|(k, _)| k == text).map(|(_, v)| (p.number, v))).collect();
        if let (false, Some(slot)) = (values.is_empty(), PropertyValue::slot(key_code(*key))) {
            slots[slot].push((*key, values));
        }
    }
    for (key, values) in slots.iter().flatten() {
        strings.extend(keys.iter().find(|k| k.1 == *key).map(|k| k.0.clone()));
        strings.extend(values.iter().filter_map(|(_, v)| match v {
            PropertyValue::Text(t) => Some(t.clone()),
            _ => None,
        }));
    }
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
            w.write_bit_long(0);
            // Names by number, then the same keyed by name.
            for by_name in [false, true] {
                w.write_bit_long(named.len() as i32);
                for (p, tag) in named.iter().zip(&name_tags) {
                    let pair = [p.number as i32, *tag];
                    w.write_bit_long(pair[by_name as usize]);
                    w.write_bit_long(pair[!by_name as usize]);
                }
            }
            w.write_bit_long(described.len() as i32);
            for (p, tag) in described.iter().zip(&tags) {
                w.write_bit_long(p.number as i32);
                w.write_bit_long(*tag);
            }
            for slot in &slots {
                w.write_bit_long(slot.len() as i32);
                for (key, values) in slot {
                    w.write_bit_long(key | 0x4000_0000);
                    w.write_bit_long(values.len() as i32);
                    for (number, value) in values {
                        w.write_bit_long(*number as i32);
                        match value {
                            PropertyValue::Integer(v) => w.write_bit_long(*v),
                            PropertyValue::Double(v) => w.write_bit_double(*v),
                            PropertyValue::Boolean(v) => w.write_bit(*v),
                            PropertyValue::Choice(v) => w.write_bit_long(*v as i32),
                            PropertyValue::Text(_) => {}
                        }
                    }
                }
            }
        },
        Body { refs, strings },
    )
}

/// Rewrites the database and `_All Points` from `points`.
pub(super) fn store_points(doc: &mut CadDocument, points: &[CogoPoint]) -> Result<(), DxfError> {
    let s = support(doc)?;
    store(doc, &s, points)
}

/// Rewrites the database and `_All Points` from `points`.
fn store(doc: &mut CadDocument, s: &Support, points: &[CogoPoint]) -> Result<(), DxfError> {
    let root = root(doc).ok_or_else(|| invalid("drawing has no root tree node"))?;
    let keys = super::user_properties::property_keys(doc, s.class_node);
    write_database(doc, s.database, root, points, s.class_node, &keys)?;
    let held: Vec<i32> = keys.iter().filter(|(t, _)| points.iter().any(|p| p.properties.iter().any(|(k, _)| k == t))).map(|k| k.1).collect();
    super::user_properties::sync_users(doc, s.class_node, s.database.value(), &held)?;
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
            w.write_bit_short(VERSION);
            w.write_bit_long(number as i32);
            bits(w, "001010");
        },
        Body { refs: Vec::new(), strings: Vec::new() },
    )
}

/// Adds a COGO point. `number` `None` takes the next free number;
/// `style` `None` gives it the drawing's default point style. A drawing
/// without the civil settings and style tree gets the point style, class
/// and settings nodes a point needs (`tree.rs`); a drawing with a tree
/// that has no point styles returns an error.
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
        name: None,
        properties: Vec::new(),
    });
    store(doc, &s, &points)?;
    put_point(doc, None, number, layer)
}

/// The stored elevation of a point made without one (`.` at the
/// elevation prompt).
pub const NO_ELEVATION: f64 = -1.0e20;

/// The next point number setting of the create-points settings node, a
/// long key of the node's settings list (`BL 0, B 1, BL count, count x
/// key` after its children).
const KEY_NEXT_POINT_NUMBER: u32 = 0x0100_5408;

/// The layer setting of the create-points settings node (a soft pointer
/// key: the record is the node's reference in the order of its reference
/// keys).
const KEY_POINT_LAYER: u32 = 0x0F00_5416;

/// One key of a settings list: its key, value, the bit span of the value
/// and, for a key that takes a reference, its index among those.
struct SettingKey {
    key: u32,
    value: crate::classes::civil::alignment::KeyValue,
    span: std::ops::Range<usize>,
    reference: Option<usize>,
}

/// The create-points settings node, its record and its settings list.
fn create_settings(doc: &CadDocument) -> Option<(Handle, NodeTail, Vec<SettingKey>)> {
    use crate::classes::civil::alignment::{read_key, KeyValue};
    use crate::classes::civil::bits::Bits;
    let node = Handle::new(settings_node(doc, "AeccCreatePoints")?.0);
    let tail = read_node(doc, node)?;
    let bytes = bit_bytes(&tail.bits);
    let end = tail.bits.len();
    let mut b = Bits::new(&bytes, 0, end);
    (b.bit_long()? == 0 && b.bit()?).then_some(())?;
    let mut keys = Vec::new();
    let mut references = 0;
    for _ in 0..b.bit_long()? {
        let at = b.pos;
        let (key, value) = read_key(&mut b)?;
        let mut k = Bits::new(&bytes, at, end);
        k.bit_long()?;
        let reference = KeyValue::has_reference(key).then(|| {
            references += 1;
            references - 1
        });
        keys.push(SettingKey { key, value, span: k.pos..b.pos, reference });
    }
    Some((node, tail, keys))
}

/// The create-points settings node, its record and the bit span of the
/// next point number's value in the record's tail, with the value.
fn next_number_setting(doc: &CadDocument) -> Option<(Handle, NodeTail, std::ops::Range<usize>, u32)> {
    let (node, tail, keys) = create_settings(doc)?;
    let k = keys.into_iter().find(|k| k.key == KEY_NEXT_POINT_NUMBER)?;
    let crate::classes::civil::alignment::KeyValue::Long(v) = k.value else { return None };
    Some((node, tail, k.span, u32::try_from(v).ok()?))
}

/// The layer the drawing's point creation settings put new points on;
/// `None` when they name none (the current layer then).
pub fn point_creation_layer(doc: &CadDocument) -> Option<String> {
    let (_, tail, keys) = create_settings(doc)?;
    let index = keys.iter().find(|k| k.key == KEY_POINT_LAYER)?.reference?;
    let handle = tail.refs.get(index)?.1;
    doc.layers.iter().find(|l| l.handle.value() == handle).map(|l| l.name.clone())
}

/// `bits` ("0"/"1" text, first bit first) packed into bytes.
fn bit_bytes(bits: &str) -> Vec<u8> {
    bits.as_bytes().chunks(8).map(|c| c.iter().enumerate().fold(0u8, |b, (i, d)| b | (u8::from(*d == b'1') << (7 - i)))).collect()
}

/// A bit long as "0"/"1" text.
fn bit_long_text(v: i32) -> String {
    match v {
        0 => "10".to_string(),
        1..=255 => format!("01{v:08b}"),
        _ => std::iter::once("00".to_string()).chain(v.to_le_bytes().iter().map(|b| format!("{b:08b}"))).collect(),
    }
}

/// Sets the drawing's next point number when its create-points settings
/// keep one (a drawing without the setting leaves it to the application).
fn set_next_number(doc: &mut CadDocument, next: u32) -> Result<(), DxfError> {
    let Some((node, n, span, _)) = next_number_setting(doc) else { return Ok(()) };
    let bits_text = format!("{}{}{}", &n.bits[..span.start], bit_long_text(next as i32), &n.bits[span.end..]);
    let o = object(doc, node).ok_or_else(|| invalid("settings node cannot be read"))?;
    let (dxf, owner, reactors) = (o.dxf_name.clone(), o.owner, o.reactors.clone());
    let mut refs: Vec<Ref> = n.children.iter().map(|c| soft_owner(c.1)).collect();
    refs.extend(n.refs);
    let mut strings = n.own;
    strings.extend(n.names);
    strings.extend(n.texts);
    put_object(
        doc,
        &dxf,
        node,
        owner,
        reactors,
        |w| {
            w.write_bit_long(n.key);
            w.write_bit_long(n.children.len() as i32);
            for c in &n.children {
                w.write_bit_long(c.0);
            }
            bits(w, &bits_text);
        },
        Body { refs, strings },
    )
}

/// The number a new point takes: the drawing's next point number setting
/// (the first free number from it), else one above the highest.
fn next_number(doc: &CadDocument, points: &[CogoPoint]) -> u32 {
    let used = |n: u32| points.iter().any(|p| p.number == n);
    let mut n = next_number_setting(doc).map(|s| s.3).filter(|n| *n > 0).unwrap_or_else(|| points.iter().map(|p| p.number).max().unwrap_or(0) + 1);
    while used(n) {
        n += 1;
    }
    n
}

/// Adds a COGO point as the point creation commands do: it takes the
/// drawing's next point number and advances it, and has no point or label
/// style of its own (its point group's draw it). `position.z` is
/// [`NO_ELEVATION`] for a point without an elevation. Returns the point and
/// its number.
pub fn create_cogo_point(doc: &mut CadDocument, position: Vector3, description: Option<&str>, layer: Option<&str>) -> Result<(Handle, u32), DxfError> {
    if !finite(position) {
        return Err(invalid("point position must be finite"));
    }
    if layer.is_some_and(|l| doc.layers.get(l).is_none()) {
        return Err(invalid("layer does not exist"));
    }
    let mut points = cogo_points(doc);
    let number = next_number(doc, &points);
    let s = support(doc)?;
    points.push(CogoPoint { number, position, style: 0, label_style: 0, description: description.map(str::to_string), name: None, properties: Vec::new() });
    store(doc, &s, &points)?;
    let point = put_point(doc, None, number, layer)?;
    set_next_number(doc, number + 1)?;
    Ok((point, number))
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

/// Names a point (`None` removes its name). Names are unique in the
/// drawing, case ignored.
pub fn set_cogo_point_name(doc: &mut CadDocument, point: Handle, name: Option<&str>) -> Result<(), DxfError> {
    if name.is_some_and(|n| n.trim().is_empty()) {
        return Err(invalid("a point name needs text"));
    }
    let number = point_number(doc, point)?;
    let mut points = cogo_points(doc);
    if let Some(n) = name {
        if points.iter().any(|p| p.number != number && p.name.as_deref().is_some_and(|o| o.eq_ignore_ascii_case(n))) {
            return Err(invalid("point name already used"));
        }
    }
    let p = points.iter_mut().find(|p| p.number == number).ok_or_else(|| invalid("point is not in the database"))?;
    p.name = name.map(str::to_string);
    let s = support(doc)?;
    store(doc, &s, &points)
}

/// Gives a point the label style `style` (an `AECC_LABEL_COLLECTOR_STYLE`
/// of the drawing's point label styles).
pub fn set_cogo_point_label_style(doc: &mut CadDocument, point: Handle, style: Handle) -> Result<(), DxfError> {
    if object(doc, style).is_none_or(|o| o.dxf_name != "AECC_LABEL_COLLECTOR_STYLE") {
        return Err(invalid("not a label style"));
    }
    let number = point_number(doc, point)?;
    let mut points = cogo_points(doc);
    let p = points.iter_mut().find(|p| p.number == number).ok_or_else(|| invalid("point is not in the database"))?;
    p.label_style = style.value();
    let s = support(doc)?;
    store(doc, &s, &points)
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

pub(super) fn point_number(doc: &CadDocument, point: Handle) -> Result<u32, DxfError> {
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
    fn a_new_drawing_gets_its_point_style_and_settings() {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        add_cogo_point(&mut doc, Some(1), Vector3::new(1.0, 2.0, 3.0), None, None, None).unwrap();
        let back = round_trip(&doc);
        let style = cogo_points(&back)[0].style;
        assert_ne!(style, 0);
        // The default point label style of the label styles a new drawing
        // gets.
        assert_eq!(cogo_points(&back)[0].label_style, super::super::labels::default_label_style(&back, super::super::labels::POINT_LABELS).unwrap());
        for key in [31, 5, 45] {
            assert!(root_node(&back, key).is_some(), "root key {key}");
        }
        // The written style reads back with its six components and marker.
        let o = object(&back, Handle::new(style)).unwrap();
        let (data, start, end) = o.main_stream().unwrap();
        let handles = o.decode().handles;
        let components = crate::classes::civil::style::components(data, start, end, &handles);
        assert_eq!(components.len(), 6);
        assert!(components[0].visible && !components[2].visible);
        assert_eq!(crate::classes::civil::style::marker(data, start, end).map(|m| (m.kind, m.absolute)), Some((3, false)));
    }

    #[test]
    fn created_points_take_the_next_number_and_their_groups_styles() {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        add_cogo_point(&mut doc, Some(4), Vector3::new(1.0, 2.0, 3.0), None, None, None).unwrap();
        let (_, n) = create_cogo_point(&mut doc, Vector3::new(5.0, 6.0, NO_ELEVATION), Some("D 3"), None).unwrap();
        assert_eq!(n, 5);
        let back = round_trip(&doc);
        let p = cogo_points(&back).into_iter().find(|p| p.number == 5).unwrap();
        assert_eq!((p.style, p.label_style, p.position.z, p.description.as_deref()), (0, 0, NO_ELEVATION, Some("D 3")));
        // A written tree keeps no settings of its own: no layer is named.
        assert_eq!(point_creation_layer(&back), None);
    }

    #[test]
    fn bit_long_text_reads_back() {
        for v in [0, 1, 255, 256, 1234, 777_777, -5] {
            let t = bit_long_text(v);
            let bytes = bit_bytes(&t);
            assert_eq!(crate::classes::civil::bits::Bits::new(&bytes, 0, t.len()).bit_long(), Some(v));
        }
    }

    #[test]
    fn a_tree_without_point_styles_is_refused_unchanged() {
        let mut doc = CadDocument::new();
        let root = ensure_civil(&mut doc).unwrap();
        let styles = doc.allocate_handle();
        put_object(&mut doc, "AECC_TREE_NODE", styles, root, Vec::new(), |_| {}, Body { refs: Vec::new(), strings: Vec::new() }).unwrap();
        set_root_entry(&mut doc, 31, styles).unwrap();
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

    #[test]
    fn elevation_and_description_groups_select_their_points() {
        use crate::classes::civil::{add_point_group_query, set_point_group_query, Comparison, PointQuery};
        let mut doc = drawing();
        for (n, (z, d)) in [(10.0, "TEST"), (11.0, "test"), (20.0, "TEXT"), (26.0, "ABC"), (30.0, "TE"), (12.0, "TESTS")].iter().enumerate() {
            add_cogo_point(&mut doc, Some(n as u32 + 1), Vector3::new(10.0 * n as f64, 0.0, *z), Some(d), None, None).unwrap();
        }
        let text = |v: &[&str]| PointQuery::RawDescription(v.iter().map(|t| t.to_string()).collect());
        let cases = [
            ("GA", text(&["TEST"]), vec![1, 2]),
            ("GB", text(&["te*"]), vec![1, 2, 3, 5, 6]),
            ("GJ", text(&["T?ST"]), vec![1, 2]),
            ("GK", text(&["TEST", "ABC"]), vec![1, 2, 4]),
            ("GE", PointQuery::Elevation(Comparison::Greater, 25.0), vec![4, 5]),
            ("GF", PointQuery::Elevation(Comparison::Less, 12.0), vec![1, 2]),
            ("GG", PointQuery::Elevation(Comparison::GreaterOrEqual, 26.0), vec![4, 5]),
            ("GH", PointQuery::Elevation(Comparison::LessOrEqual, 11.0), vec![1, 2]),
            ("GI", PointQuery::Elevation(Comparison::Equal, 20.0), vec![3]),
        ];
        let groups: Vec<_> = cases.iter().map(|(n, q, _)| add_point_group_query(&mut doc, n, None, q).unwrap()).collect();
        assert!(add_point_group_query(&mut doc, "X", None, &text(&["A,B"])).is_err());
        assert!(add_point_group_query(&mut doc, "X", None, &text(&["A#"])).is_err());
        let back = round_trip(&doc);
        for (g, (_, _, want)) in groups.iter().zip(&cases) {
            assert_eq!(&object(&back, *g).unwrap().point_group_numbers(), want);
        }
        // Serials count up in creation order from 2 (`_All Points` has 0).
        let serials: Vec<i32> = groups.iter().map(|g| super::super::groups::serial(&back, *g).unwrap()).collect();
        assert_eq!(serials, (2..2 + groups.len() as i32).collect::<Vec<_>>());

        // Members follow edits: point 4 drops to 5 and leaves GE, point 7
        // with "abc" joins GK; GA becomes an elevation group.
        let mut doc = back;
        let p4 = handle_of(&doc, 4);
        set_cogo_point(&mut doc, p4, 4, Vector3::new(30.0, 0.0, 5.0), Some("ABC")).unwrap();
        add_cogo_point(&mut doc, Some(7), Vector3::new(70.0, 0.0, 40.0), Some("abc"), None, None).unwrap();
        set_point_group_query(&mut doc, groups[0], "GA", None, &PointQuery::Elevation(Comparison::Less, 10.5)).unwrap();
        let back = round_trip(&doc);
        assert_eq!(object(&back, groups[4]).unwrap().point_group_numbers(), vec![5, 7]);
        assert_eq!(object(&back, groups[3]).unwrap().point_group_numbers(), vec![1, 2, 4, 7]);
        assert_eq!(object(&back, groups[0]).unwrap().point_group_numbers(), vec![1, 4]);
    }

    #[test]
    fn names_ranges_and_exclusions_select_their_points() {
        use crate::classes::civil::{add_point_group_query, set_cogo_point_name, Comparison, PointQuery};
        let mut doc = drawing();
        let mut points = Vec::new();
        for (n, (z, d)) in [(10.0, "TEST"), (11.0, "test"), (20.0, "TEXT"), (26.0, "ABC"), (30.0, "TE"), (12.0, "TESTS")].iter().enumerate() {
            points.push(add_cogo_point(&mut doc, Some(n as u32 + 1), Vector3::new(10.0 * n as f64, 0.0, *z), Some(d), None, None).unwrap());
        }
        set_cogo_point_name(&mut doc, points[4], Some("N1")).unwrap();
        set_cogo_point_name(&mut doc, points[5], Some("n2")).unwrap();
        assert!(set_cogo_point_name(&mut doc, points[0], Some("n1")).is_err(), "names are unique, case ignored");
        let b = |q: PointQuery| Box::new(q);
        let text = |v: &[&str]| v.iter().map(|t| t.to_string()).collect::<Vec<_>>();
        let cases = [
            ("GC", PointQuery::FullDescription(text(&["TEST"])), vec![1, 2]),
            ("GD", PointQuery::Names(text(&["N1"])), vec![5]),
            ("XA", PointQuery::Excluding(Some(b(PointQuery::Numbers(vec![3]))), b(PointQuery::Numbers(vec![5]))), vec![3]),
            ("XB", PointQuery::Excluding(Some(b(PointQuery::Elevation(Comparison::Greater, 20.0))), b(PointQuery::Elevation(Comparison::Less, 27.0))), vec![5]),
            ("XC", PointQuery::Excluding(Some(b(PointQuery::RawDescription(text(&["TEST", "ABC"])))), b(PointQuery::Numbers(vec![1]))), vec![2, 4]),
            ("XE", PointQuery::ElevationRange(10.5, 20.25), vec![2, 3, 6]),
            ("XF", PointQuery::Excluding(Some(b(PointQuery::Names(text(&["n*"])))), b(PointQuery::FullDescription(text(&["TE"])))), vec![6]),
            ("XG", PointQuery::Excluding(None, b(PointQuery::Numbers(vec![2]))), vec![1, 3, 4, 5, 6]),
            ("XJ", PointQuery::Excluding(Some(b(PointQuery::Numbers(vec![1]))), b(PointQuery::ElevationRange(10.0, 11.0))), vec![]),
        ];
        let groups: Vec<_> = cases.iter().map(|(n, q, _)| add_point_group_query(&mut doc, n, None, q).unwrap()).collect();
        let nested = PointQuery::Excluding(None, b(PointQuery::Excluding(None, b(PointQuery::Numbers(vec![1])))));
        assert!(add_point_group_query(&mut doc, "X", None, &nested).is_err());
        assert!(add_point_group_query(&mut doc, "X", None, &PointQuery::ElevationRange(5.0, 1.0)).is_err());
        let back = round_trip(&doc);
        let names: Vec<_> = cogo_points(&back).iter().map(|p| p.name.clone()).collect();
        assert_eq!(names, [None, None, None, None, Some("N1".into()), Some("n2".into())]);
        for (g, (_, _, want)) in groups.iter().zip(&cases) {
            assert_eq!(&object(&back, *g).unwrap().point_group_numbers(), want);
        }
        // The queries read back: a point added later joins the groups that
        // select it.
        let mut doc = back;
        add_cogo_point(&mut doc, Some(7), Vector3::new(70.0, 0.0, 15.0), Some("TEST"), None, None).unwrap();
        let back = round_trip(&doc);
        assert_eq!(object(&back, groups[5]).unwrap().point_group_numbers(), vec![2, 3, 6, 7]);
        assert_eq!(object(&back, groups[4]).unwrap().point_group_numbers(), vec![2, 4, 7]);
        assert_eq!(object(&back, groups[0]).unwrap().point_group_numbers(), vec![1, 2, 7]);
    }

}
