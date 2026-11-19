//! Point groups that hold listed point numbers, and the query statements
//! the groups keep (the `_All Points` group is written by `points.rs`).

use super::super::bits::Bits;
use super::points::{support, Support};
use super::support::*;
use crate::classes::civil::CogoPoint;
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::{Handle, Vector3};
use crate::CadDocument;

/// Comparison codes of the query leaves.
pub(super) const EQUAL: u8 = 1;
pub(super) const GREATER: u8 = 3;
/// Id of the `_All Points` group in the point group node.
pub(super) const ALL_POINTS_ID: i32 = 33;
/// Attribute id of the point number in a query leaf.
const ATTRIBUTE_POINT_NUMBER: i32 = 19;
/// First text id given to a written group (its query takes the next one).
/// Kept above the ids the point writer gives descriptions.
const FIRST_GROUP_TAG: u32 = 0x8001_0001;
/// Bits after a group's extents: the same 164 bits in every group of WP01,
/// WP08, WP10 and WG01 (`_All Points` and six named groups; the group's
/// override settings, not mapped field by field).
const GROUP_TAIL: &str = "10100101010111101010101010100000000000000000000000000000000000000000000000000000000000000000101010000000000000000000000000000000000000000000000000000000000000000010";

/// One group of the point group node: its id, record and name.
#[derive(Clone)]
pub(super) struct Entry {
    pub id: i32,
    pub handle: Handle,
    pub name: String,
}

/// The query statement under the outer query: its tag, record and title.
#[derive(Clone)]
struct Query {
    tag: i32,
    handle: Handle,
    title: String,
}

/// Inserts `item` the way the application orders its hashed lists: by
/// bucket (key mod 8), a new key last in its bucket.
pub(super) fn bucket_insert<T>(v: &mut Vec<T>, item: T, key: impl Fn(&T) -> i32) {
    let bucket = |k: i32| k as u32 % 8;
    let b = bucket(key(&item));
    let at = v.iter().position(|e| bucket(key(e)) > b).unwrap_or(v.len());
    v.insert(at, item);
}

/// Group ids, records and names of the point group node.
pub(super) fn entries(doc: &CadDocument, node: Handle) -> Option<Vec<Entry>> {
    let o = object(doc, node)?;
    let (data, start, end) = fields_of(o)?;
    let mut b = Bits::new(data, start, end);
    (b.bit_long()? == KEY_POINT_GROUPS).then_some(())?;
    let n = usize::try_from(b.bit_long()?).ok()?;
    let ids = (0..n).map(|_| b.bit_long()).collect::<Option<Vec<_>>>()?;
    let d = o.decode();
    (d.handles.len() >= n).then_some(())?;
    let mut names = d.strings.into_iter().skip(1);
    let list = ids.into_iter().zip(d.handles).map(|(id, h)| Entry { id, handle: Handle::new(h), name: if id == ALL_POINTS_ID { String::new() } else { names.next().unwrap_or_default() } });
    Some(list.collect())
}

/// The point group node (Root key 32): group ids in stored order and the
/// drawing's default label and point styles. Its texts are one text the
/// samples leave empty, then the names of the groups other than
/// `_All Points` in stored order (`_All Points` has no stored name).
pub(super) fn write_group_node(doc: &mut CadDocument, handle: Handle, root: Handle, groups: &[Entry], label_style: u64, point_style: u64) -> Result<(), DxfError> {
    let mut refs: Vec<Ref> = groups.iter().map(|g| soft_owner(g.handle.value())).collect();
    refs.push(hard_pointer(label_style));
    refs.push(hard_pointer(point_style));
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
                w.write_bit_long(g.id);
            }
            w.write_bit_long(0);
            bits(w, "010");
        },
        Body { refs, strings: std::iter::once(String::new()).chain(groups.iter().filter(|g| g.id != ALL_POINTS_ID).map(|g| g.name.clone())).collect() },
    )
}

fn queries(doc: &CadDocument, outer: Handle) -> Option<Vec<Query>> {
    let o = object(doc, outer)?;
    let (data, start, end) = fields_of(o)?;
    let mut b = Bits::new(data, start, end);
    (b.bit_long()? == KEY_ALL_POINTS_QUERY).then_some(())?;
    let n = usize::try_from(b.bit_long()?).ok()?;
    let tags = (0..n).map(|_| b.bit_long()).collect::<Option<Vec<_>>>()?;
    let d = o.decode();
    (d.handles.len() >= n && d.strings.len() > n).then_some(())?;
    Some(tags.into_iter().zip(d.handles).zip(d.strings.into_iter().skip(1)).map(|((tag, h), title)| Query { tag, handle: Handle::new(h), title }).collect())
}

/// Outer query statement (Root key 75): the tags of the groups' query
/// statements, which it owns, in stored order.
fn write_outer_query(doc: &mut CadDocument, handle: Handle, root: Handle, list: &[Query]) -> Result<(), DxfError> {
    let mut strings = vec![String::new()];
    strings.extend(list.iter().map(|q| q.title.clone()));
    strings.push(String::new());
    put_object(
        doc,
        "AECC_QUERY_STATEMENT",
        handle,
        root,
        Vec::new(),
        |w| {
            w.write_bit_long(KEY_ALL_POINTS_QUERY);
            w.write_bit_long(list.len() as i32);
            for q in list {
                w.write_bit_long(q.tag);
            }
            bits(w, "100100010");
        },
        Body { refs: list.iter().map(|q| soft_owner(q.handle.value())).collect(), strings },
    )
}

/// Adds (or replaces) a query statement in the outer query, creating the
/// outer query when the drawing has none.
pub(super) fn set_query(doc: &mut CadDocument, title: &str, tag: i32, comparison: u8, values: &[u32]) -> Result<Handle, DxfError> {
    let root = ensure_civil(doc)?;
    let outer = match root_node(doc, KEY_ALL_POINTS_QUERY) {
        Some(h) => h,
        None => {
            let h = doc.allocate_handle();
            write_outer_query(doc, h, root, &[])?;
            set_root_entry(doc, KEY_ALL_POINTS_QUERY, h)?;
            h
        }
    };
    let mut list = queries(doc, outer).ok_or_else(|| invalid("outer query statement cannot be read"))?;
    let handle = match list.iter_mut().find(|q| q.tag == tag) {
        Some(q) => {
            q.title = title.into();
            q.handle
        }
        None => {
            let h = doc.allocate_handle();
            bucket_insert(&mut list, Query { tag, handle: h, title: title.into() }, |q| q.tag);
            h
        }
    };
    write_query(doc, handle, outer, tag, title, comparison, values)?;
    write_outer_query(doc, outer, root, &list)?;
    Ok(handle)
}

/// Query statement `(number <comparison> v1 OR number = v2 OR ...)`.
///
/// After the tag: `BL 0, BL 0`, five constant bits, `BL` count of
/// constants (byte form), eight zero bits; then one record per node in
/// pre-order: the parentheses, the OR nodes (left-deep), and one leaf per
/// value. Every record opens with `BL 80` and 15 zero bits; the
/// parentheses record carries the comparison when its child is a leaf,
/// a leaf carries the attribute id and the value. The node kinds are in
/// the texts. This reproduces the `_All Points` query and the number-list
/// queries of WP08 and WG01 bit for bit.
fn write_query(doc: &mut CadDocument, handle: Handle, outer: Handle, tag: i32, title: &str, comparison: u8, values: &[u32]) -> Result<(), DxfError> {
    let k = values.len();
    let head = |w: &mut DwgMergedWriter| {
        bl_byte(w, 80);
        bits(w, &"0".repeat(15));
    };
    let byte = |w: &mut DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    let mut strings = vec![title.to_string(), String::new()];
    strings.extend(values.iter().map(u32::to_string));
    strings.extend(["AeccQueryNodePARENS".into(), String::new()]);
    strings.extend((1..k).map(|_| "AeccQueryNodeOR".to_string()));
    let op = if comparison == GREATER { "AeccQueryNodeGT" } else { "AeccQueryNodeEQ" };
    for _ in values {
        strings.extend([op, "AeccQueryNodeATTRID", "", "", "AeccQueryNodeCONST", "", ""].map(String::from));
    }
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
            bits(w, "01000");
            bl_byte(w, k as u8);
            byte(w, 0);
            head(w);
            bits(w, if k == 1 { "111" } else { "110" });
            byte(w, if k == 1 { comparison } else { EQUAL });
            byte(w, 0);
            byte(w, 0);
            for j in 1..k {
                head(w);
                bits(w, if j < k - 1 { "100" } else { "101" });
                byte(w, 1);
                byte(w, 3);
                byte(w, 0);
            }
            for (i, v) in values.iter().enumerate() {
                head(w);
                w.write_bit(true);
                w.write_bit_long(ATTRIBUTE_POINT_NUMBER);
                w.write_bit_long(*v as i32);
                if i + 1 < k {
                    byte(w, 0);
                }
            }
        },
        Body { refs: Vec::new(), strings },
    )
}

/// The values of a number-list query (the texts after the title), `None`
/// for any other query.
fn listed_numbers(doc: &CadDocument, query: Handle) -> Option<Vec<u32>> {
    let s = object(doc, query)?.decode().strings;
    let k = s.iter().filter(|t| *t == "AeccQueryNodeCONST").count();
    let nodes = s.get(2 + k..)?;
    let known = ["AeccQueryNodePARENS", "AeccQueryNodeOR", "AeccQueryNodeEQ", "AeccQueryNodeATTRID", "AeccQueryNodeCONST", ""];
    (k > 0 && nodes.iter().all(|t| known.contains(&t.as_str()))).then_some(())?;
    s[2..2 + k].iter().map(|t| t.parse().ok()).collect()
}

/// A group record (`AECC_POINT_PG`).
pub(super) struct Group<'a> {
    pub id: i32,
    pub serial: i32,
    /// `None` for `_All Points`.
    pub name: Option<&'a str>,
    pub description: &'a str,
    pub numbers: Vec<u32>,
    /// Point extents; `None` writes the "not computed" pair `±1e20`
    /// `_All Points` always carries.
    pub extents: Option<(Vector3, Vector3)>,
    /// label style, point style, query, database, group node, settings node
    pub refs: [u64; 6],
}

/// `BL id, BL 0, BL 0` (no override lists or keys), `a` = 0 in its byte
/// form, 5 + 6 constant bits, `BL serial`, the numbers as `RL`, then two
/// constant bits, `BD 0`, the extents (min, max) and [`GROUP_TAIL`].
pub(super) fn write_group(doc: &mut CadDocument, handle: Handle, owner: Handle, g: &Group) -> Result<(), DxfError> {
    let (min, max) = g.extents.unwrap_or((Vector3::new(1e20, 1e20, 1e20), Vector3::new(-1e20, -1e20, -1e20)));
    let strings = match g.name {
        Some(name) => vec![name.to_string(), g.description.to_string(), "AeccImpPG".into()],
        None => vec![String::new(), "AeccImpAllPointsPG".into()],
    };
    let mut refs: Vec<Ref> = g.refs.iter().map(|h| hard_pointer(*h)).collect();
    refs.extend([hard_pointer(0), hard_pointer(0)]);
    put_object(
        doc,
        "AECC_POINT_PG",
        handle,
        owner,
        Vec::new(),
        |w| {
            w.write_bit_long(g.id);
            w.write_bit_long(0);
            w.write_bit_long(0);
            bl_byte(w, 0);
            bits(w, "00000100000");
            w.write_bit_long(g.serial);
            w.write_bit_long(g.numbers.len() as i32);
            for n in &g.numbers {
                w.write_raw_long(*n as i32);
            }
            bits(w, "00");
            w.write_bit_double(0.0);
            for v in [min.x, min.y, min.z, max.x, max.y, max.z] {
                w.write_bit_double(v);
            }
            bits(w, GROUP_TAIL);
        },
        Body { refs, strings },
    )
}

/// Members of a number list among `points` (sorted) and their extents.
fn members(points: &[CogoPoint], list: &[u32]) -> (Vec<u32>, Option<(Vector3, Vector3)>) {
    let mut numbers: Vec<u32> = points.iter().filter(|p| list.contains(&p.number)).map(|p| p.number).collect();
    numbers.sort_unstable();
    let mut extents: Option<(Vector3, Vector3)> = None;
    for p in points.iter().filter(|p| list.contains(&p.number)) {
        let q = p.position;
        extents = Some(match extents {
            None => (q, q),
            Some((a, b)) => (Vector3::new(a.x.min(q.x), a.y.min(q.y), a.z.min(q.z)), Vector3::new(b.x.max(q.x), b.y.max(q.y), b.z.max(q.z))),
        });
    }
    (numbers, extents)
}

/// True when a group record has no override lists or keys (the form this
/// crate writes), so rewriting it loses nothing.
fn plain(doc: &CadDocument, group: Handle) -> bool {
    let Some((data, start, end)) = object(doc, group).and_then(fields_of) else { return false };
    let mut b = Bits::new(data, start, end);
    (|| {
        b.bit_long()?;
        (b.bit_long()? == 0 && b.bit_long()? == 0).then_some(())?;
        (b.bits_value(3)? != 0b101).then_some(())
    })()
    .is_some()
}

/// Rewrites the members of every plain number-list group from `points`.
pub(super) fn refresh_groups(doc: &mut CadDocument, s: &Support, points: &[CogoPoint]) -> Result<(), DxfError> {
    for (i, e) in entries(doc, s.group_node).unwrap_or_default().into_iter().enumerate() {
        if e.handle == s.all_points || !plain(doc, e.handle) {
            continue;
        }
        let Some(d) = object(doc, e.handle).map(|o| o.decode()) else { continue };
        let Some(list) = d.handles.get(2).and_then(|q| listed_numbers(doc, Handle::new(*q))) else { continue };
        let (numbers, extents) = members(points, &list);
        let refs = [0, 1, 2, 3, 4, 5].map(|j| d.handles.get(j).copied().unwrap_or(0));
        let description = d.strings.get(1).cloned().unwrap_or_default();
        let g = Group { id: e.id, serial: 2 * i as i32, name: Some(&e.name), description: &description, numbers, extents, refs };
        write_group(doc, e.handle, s.group_node, &g)?;
    }
    Ok(())
}

/// Adds a point group holding the points whose numbers are in `numbers`
/// (its query lists them; points added later with one of these numbers
/// join it). The drawing's point support is found or written as for
/// [`super::add_cogo_point`].
pub fn add_point_group(doc: &mut CadDocument, name: &str, description: Option<&str>, numbers: &[u32]) -> Result<Handle, DxfError> {
    let s = support(doc)?;
    let list = entries(doc, s.group_node).ok_or_else(|| invalid("point group node cannot be read"))?;
    if name.is_empty() || list.iter().any(|e| e.name.eq_ignore_ascii_case(name)) {
        return Err(invalid("point group name is empty or already used"));
    }
    let used = root_node(doc, KEY_ALL_POINTS_QUERY).and_then(|o| queries(doc, o)).unwrap_or_default();
    let tags = list.iter().map(|e| e.id as u32).chain(used.iter().map(|q| q.tag as u32));
    let mut id = tags.filter(|t| *t & 0x8000_0000 != 0).max().map_or(FIRST_GROUP_TAG, |t| (t + 1).max(FIRST_GROUP_TAG));
    // A group whose id falls in bucket 0 is stored before `_All Points`,
    // which the application's audit reports as a damaged point group node
    // (R16 with 0x80000100 and 0x80010000); the application never gives a
    // group such an id in the samples, so skip it.
    if id % 8 == 0 {
        id += 1;
    }
    let id = id as i32;
    let handle = doc.allocate_handle();
    let mut list = list;
    bucket_insert(&mut list, Entry { id, handle, name: name.into() }, |e| e.id);
    let serial = 2 * list.iter().position(|e| e.handle == handle).unwrap_or(0) as i32;
    write_point_group(doc, &s, handle, id, serial, name, description.unwrap_or(""), numbers)?;
    let root = root(doc).ok_or_else(|| invalid("drawing has no root tree node"))?;
    write_group_node(doc, s.group_node, root, &list, s.defaults.label_style, s.defaults.point_style)?;
    Ok(handle)
}

/// Renames a point group written by [`add_point_group`] and gives it a new
/// number list.
pub fn set_point_group(doc: &mut CadDocument, group: Handle, name: &str, description: Option<&str>, numbers: &[u32]) -> Result<(), DxfError> {
    let s = support(doc)?;
    let mut list = entries(doc, s.group_node).ok_or_else(|| invalid("point group node cannot be read"))?;
    let at = list.iter().position(|e| e.handle == group).filter(|_| group != s.all_points).ok_or_else(|| invalid("handle is not a named point group"))?;
    if !plain(doc, group) || name.is_empty() || list.iter().any(|e| e.handle != group && e.name.eq_ignore_ascii_case(name)) {
        return Err(invalid("point group has overrides, or the name is empty or already used"));
    }
    list[at].name = name.into();
    let id = list[at].id;
    write_point_group(doc, &s, group, id, 2 * at as i32, name, description.unwrap_or(""), numbers)?;
    let root = root(doc).ok_or_else(|| invalid("drawing has no root tree node"))?;
    write_group_node(doc, s.group_node, root, &list, s.defaults.label_style, s.defaults.point_style)
}

#[allow(clippy::too_many_arguments)]
fn write_point_group(doc: &mut CadDocument, s: &Support, handle: Handle, id: i32, serial: i32, name: &str, description: &str, numbers: &[u32]) -> Result<(), DxfError> {
    let mut values: Vec<u32> = numbers.to_vec();
    values.dedup();
    if values.is_empty() || values.iter().any(|n| *n == 0 || *n > i32::MAX as u32) {
        return Err(invalid("point group needs point numbers"));
    }
    let query = set_query(doc, &format!("Point Group Query - {name}"), id.wrapping_add(1), EQUAL, &values)?;
    let (numbers, extents) = members(&super::points::cogo_points(doc), &values);
    let d = s.defaults;
    let refs = [d.label_style, d.point_style, query.value(), s.database.value(), s.group_node.value(), d.create_settings];
    write_group(doc, handle, s.group_node, &Group { id, serial, name: Some(name), description, numbers, extents, refs })
}
