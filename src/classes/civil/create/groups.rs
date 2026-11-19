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

/// Id of the `_All Points` group in the point group node.
pub(super) const ALL_POINTS_ID: i32 = 33;
/// How a query compares a point's attribute with its constants.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Comparison {
    Equal,
    Greater,
    Less,
    GreaterOrEqual,
    LessOrEqual,
}

impl Comparison {
    const ALL: [Comparison; 5] = [Comparison::Equal, Comparison::Greater, Comparison::Less, Comparison::GreaterOrEqual, Comparison::LessOrEqual];

    /// Code in the query record and node name in its texts.
    fn code(self) -> (u8, &'static str) {
        match self {
            Comparison::Equal => (1, "AeccQueryNodeEQ"),
            Comparison::Greater => (3, "AeccQueryNodeGT"),
            Comparison::Less => (4, "AeccQueryNodeLT"),
            Comparison::GreaterOrEqual => (5, "AeccQueryNodeGTE"),
            Comparison::LessOrEqual => (6, "AeccQueryNodeLTE"),
        }
    }

    fn holds(self, value: f64, constant: f64) -> bool {
        match self {
            Comparison::Equal => value == constant,
            Comparison::Greater => value > constant,
            Comparison::Less => value < constant,
            Comparison::GreaterOrEqual => value >= constant,
            Comparison::LessOrEqual => value <= constant,
        }
    }
}

/// What a point group's query selects.
#[derive(Clone, Debug, PartialEq)]
pub enum PointQuery {
    /// Points with any of these numbers.
    Numbers(Vec<u32>),
    /// Points whose elevation compares so with the value.
    Elevation(Comparison, f64),
    /// Points whose raw description matches any of the patterns: `*` any
    /// run, `?` one character, case ignored, over the whole description.
    RawDescription(Vec<String>),
}

/// The point attribute a query compares, as its records store it: a node
/// record opens with the value type (3 bits), a flags byte, 12 zero bits
/// and the attribute class (3 bits); a leaf names the attribute id.
struct Attribute {
    value_type: u8,
    flags: u8,
    class: u8,
    /// Point numbers: the node records' first bit is set and a leaf holds
    /// its constant as a `BL`.
    number: bool,
    id: i32,
}

const POINT_NUMBER: Attribute = Attribute { value_type: 1, flags: 0x50, class: 0b000, number: true, id: 19 };
const ELEVATION: Attribute = Attribute { value_type: 2, flags: 0x50, class: 0b001, number: false, id: 5 };
const RAW_DESCRIPTION: Attribute = Attribute { value_type: 3, flags: 0x50, class: 0b101, number: false, id: 8 };

/// A query's constant.
enum Constant {
    Number(u32),
    Double(f64),
    Text(String),
}

impl Constant {
    fn text(&self) -> String {
        match self {
            Constant::Number(n) => n.to_string(),
            Constant::Double(v) => v.to_string(),
            Constant::Text(t) => t.clone(),
        }
    }
}

/// The records of a query: the attribute, the comparison every leaf
/// makes and the constants (one leaf each, joined by OR).
pub(super) struct Criteria {
    attribute: &'static Attribute,
    comparison: Comparison,
    constants: Vec<Constant>,
}

/// Characters other than `*` and `?` that a description pattern treats as
/// wildcards, and the separator of a pattern list.
const UNSUPPORTED_PATTERN: &[char] = &['#', '@', '.', '~', '[', ']', '`', ','];

impl PointQuery {
    fn criteria(&self) -> Result<Criteria, DxfError> {
        Ok(match self {
            PointQuery::Numbers(list) => {
                let mut values = list.clone();
                values.dedup();
                if values.is_empty() || values.iter().any(|n| *n == 0 || *n > i32::MAX as u32) {
                    return Err(invalid("point group needs point numbers"));
                }
                Criteria { attribute: &POINT_NUMBER, comparison: Comparison::Equal, constants: values.into_iter().map(Constant::Number).collect() }
            }
            PointQuery::Elevation(c, v) => {
                if !v.is_finite() {
                    return Err(invalid("elevation query needs a finite value"));
                }
                Criteria { attribute: &ELEVATION, comparison: *c, constants: vec![Constant::Double(*v)] }
            }
            PointQuery::RawDescription(patterns) => {
                if patterns.is_empty() || patterns.iter().any(|p| p.is_empty() || p.contains(UNSUPPORTED_PATTERN)) {
                    return Err(invalid("description query needs patterns of text, '*' and '?'"));
                }
                Criteria { attribute: &RAW_DESCRIPTION, comparison: Comparison::Equal, constants: patterns.iter().map(|p| Constant::Text(p.clone())).collect() }
            }
        })
    }

    /// True when the point belongs to the query's group.
    fn selects(&self, p: &CogoPoint) -> bool {
        match self {
            PointQuery::Numbers(list) => list.contains(&p.number),
            PointQuery::Elevation(c, v) => c.holds(p.position.z, *v),
            PointQuery::RawDescription(patterns) => {
                let d = p.description.as_deref().unwrap_or("");
                patterns.iter().any(|q| wildcard(q, d))
            }
        }
    }
}

/// `*` / `?` pattern match over the whole text, case ignored.
fn wildcard(pattern: &str, text: &str) -> bool {
    let t: Vec<char> = text.to_lowercase().chars().collect();
    // ok[j]: the pattern read so far matches t[..j]
    let mut ok = vec![false; t.len() + 1];
    ok[0] = true;
    for c in pattern.to_lowercase().chars() {
        let mut next = vec![false; t.len() + 1];
        for j in 0..=t.len() {
            next[j] = match c {
                '*' => ok[j] || (j > 0 && next[j - 1]),
                '?' => j > 0 && ok[j - 1],
                _ => j > 0 && ok[j - 1] && t[j - 1] == c,
            };
        }
        ok = next;
    }
    ok[t.len()]
}
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
pub(super) fn set_query(doc: &mut CadDocument, title: &str, tag: i32, criteria: Criteria) -> Result<Handle, DxfError> {
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
    write_query(doc, handle, outer, tag, title, &criteria)?;
    write_outer_query(doc, outer, root, &list)?;
    Ok(handle)
}

/// The query of `_All Points`, `(number > 0)`.
pub(super) fn all_points_query() -> Criteria {
    Criteria { attribute: &POINT_NUMBER, comparison: Comparison::Greater, constants: vec![Constant::Number(0)] }
}

/// Query statement `(attribute <comparison> c1 OR attribute <comparison>
/// c2 OR ...)`.
///
/// After the tag: `BL 0, BL 0`, five constant bits, `BL` count of
/// constants (byte form), seven zero bits; then one record per node in
/// pre-order: the parentheses, the OR nodes (left-deep), one leaf per
/// constant. Every record opens with the attribute's header (value type,
/// flags, 12 zero bits, class). The parentheses add three bits (number
/// attribute, 1, child is a leaf), the comparison of a leaf child (equal
/// under an OR) and 15 zero bits; an OR three bits (number attribute, 0,
/// last), bytes 1 and 3 and seven zero bits; a leaf the number bit, the
/// attribute id and, for numbers and elevations, the constant, then seven
/// zero bits unless it is the last. The node kinds and the constants are
/// in the texts; a description leaf repeats its pattern there. This
/// reproduces the `_All Points` query, the number-list queries of WP08 and
/// WG01 and the elevation and description queries of WQ03 bit for bit.
fn write_query(doc: &mut CadDocument, handle: Handle, outer: Handle, tag: i32, title: &str, c: &Criteria) -> Result<(), DxfError> {
    let a = c.attribute;
    let k = c.constants.len();
    let (code, op) = c.comparison.code();
    let byte = |w: &mut DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    let zeros = |w: &mut DwgMergedWriter, n: usize| bits(w, &"0".repeat(n));
    let head = |w: &mut DwgMergedWriter| {
        bits(w, &format!("{:03b}{:08b}{}{:03b}", a.value_type, a.flags, "0".repeat(12), a.class));
    };
    let mut strings = vec![title.to_string(), String::new()];
    strings.extend(c.constants.iter().map(Constant::text));
    strings.extend(["AeccQueryNodePARENS".into(), String::new()]);
    strings.extend((1..k).map(|_| "AeccQueryNodeOR".to_string()));
    for v in &c.constants {
        strings.extend([op, "AeccQueryNodeATTRID", "", "", "AeccQueryNodeCONST", "", ""].map(String::from));
        if let Constant::Text(t) = v {
            strings.push(t.clone());
        }
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
            zeros(w, 7);
            head(w);
            w.write_bit(a.number);
            w.write_bit(true);
            w.write_bit(k == 1);
            byte(w, if k == 1 { code } else { Comparison::Equal.code().0 });
            zeros(w, 15);
            for j in 1..k {
                head(w);
                w.write_bit(a.number);
                w.write_bit(false);
                w.write_bit(j == k - 1);
                byte(w, 1);
                byte(w, 3);
                zeros(w, 7);
            }
            for (i, v) in c.constants.iter().enumerate() {
                head(w);
                w.write_bit(a.number);
                w.write_bit_long(a.id);
                match v {
                    Constant::Number(n) => w.write_bit_long(*n as i32),
                    Constant::Double(d) => w.write_bit_double(*d),
                    Constant::Text(_) => {}
                }
                if i + 1 < k {
                    zeros(w, 7);
                }
            }
        },
        Body { refs: Vec::new(), strings },
    )
}

/// The query a group keeps, when it is one this crate writes (from the
/// record's value type and flags and its texts).
fn read_query(doc: &CadDocument, query: Handle) -> Option<PointQuery> {
    let o = object(doc, query)?;
    let (data, start, end) = fields_of(o)?;
    let mut b = Bits::new(data, start, end);
    b.bit_long()?;
    (b.bit_long()? == 0 && b.bit_long()? == 0).then_some(())?;
    b.skip(5)?;
    let k = usize::try_from(b.bit_long()?).ok().filter(|k| *k > 0)?;
    b.skip(7)?;
    let value_type = b.bits_value(3)?;
    let flags = b.bits_value(8)?;
    let s = o.decode().strings;
    let values = s.get(2..2 + k)?;
    let nodes: Vec<&String> = s.get(2 + k..)?.iter().filter(|t| t.starts_with("AeccQueryNode")).collect();
    let comparison = Comparison::ALL.into_iter().find(|c| nodes.iter().any(|t| *t == c.code().1))?;
    // PARENS, k - 1 ORs, then per leaf the comparison, ATTRID and CONST.
    let mut expected = vec!["AeccQueryNodePARENS"];
    expected.extend((1..k).map(|_| "AeccQueryNodeOR"));
    for _ in 0..k {
        expected.extend([comparison.code().1, "AeccQueryNodeATTRID", "AeccQueryNodeCONST"]);
    }
    (nodes.len() == expected.len() && nodes.iter().zip(&expected).all(|(a, b)| a == b)).then_some(())?;
    match (value_type, flags, comparison) {
        (1, 0x50, Comparison::Equal) => Some(PointQuery::Numbers(values.iter().map(|t| t.parse().ok()).collect::<Option<_>>()?)),
        (2, 0x50, c) if k == 1 => Some(PointQuery::Elevation(c, values[0].parse().ok()?)),
        (3, 0x50, Comparison::Equal) => Some(PointQuery::RawDescription(values.to_vec())),
        _ => None,
    }
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

/// Members of a query among `points` (sorted) and their extents.
fn members(points: &[CogoPoint], query: &PointQuery) -> (Vec<u32>, Option<(Vector3, Vector3)>) {
    let mut numbers: Vec<u32> = points.iter().filter(|p| query.selects(p)).map(|p| p.number).collect();
    numbers.sort_unstable();
    let mut extents: Option<(Vector3, Vector3)> = None;
    for p in points.iter().filter(|p| query.selects(p)) {
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

/// The serial of a plain group record (after `BL id, BL 0, BL 0`, the
/// one-byte `BL 0` and eleven constant bits).
pub(super) fn serial(doc: &CadDocument, group: Handle) -> Option<i32> {
    plain(doc, group).then_some(())?;
    let (data, start, end) = object(doc, group).and_then(fields_of)?;
    let mut b = Bits::new(data, start, end);
    for _ in 0..4 {
        b.bit_long()?;
    }
    b.skip(11)?;
    b.bit_long()
}

/// The serial the application gives a new group: one above every serial
/// of the group node (`_All Points` has 0, the first group 2, the next
/// ones count up in creation order: WQ03). Groups whose record is not
/// read count as having the serials below the number of groups.
fn next_serial(doc: &CadDocument, list: &[Entry]) -> i32 {
    let read = list.iter().filter_map(|e| serial(doc, e.handle)).max().unwrap_or(0);
    read.max(list.len() as i32) + 1
}

/// Rewrites the members of every plain group whose query this crate
/// writes from `points`.
pub(super) fn refresh_groups(doc: &mut CadDocument, s: &Support, points: &[CogoPoint]) -> Result<(), DxfError> {
    for e in entries(doc, s.group_node).unwrap_or_default() {
        if e.handle == s.all_points {
            continue;
        }
        let Some(serial) = serial(doc, e.handle) else { continue };
        let Some(d) = object(doc, e.handle).map(|o| o.decode()) else { continue };
        let Some(query) = d.handles.get(2).and_then(|q| read_query(doc, Handle::new(*q))) else { continue };
        let (numbers, extents) = members(points, &query);
        let refs = [0, 1, 2, 3, 4, 5].map(|j| d.handles.get(j).copied().unwrap_or(0));
        let description = d.strings.get(1).cloned().unwrap_or_default();
        let g = Group { id: e.id, serial, name: Some(&e.name), description: &description, numbers, extents, refs };
        write_group(doc, e.handle, s.group_node, &g)?;
    }
    Ok(())
}

/// Adds a point group holding the points whose numbers are in `numbers`
/// (its query lists them; points added later with one of these numbers
/// join it). The drawing's point support is found or written as for
/// [`super::add_cogo_point`].
pub fn add_point_group(doc: &mut CadDocument, name: &str, description: Option<&str>, numbers: &[u32]) -> Result<Handle, DxfError> {
    add_point_group_query(doc, name, description, &PointQuery::Numbers(numbers.to_vec()))
}

/// Adds a point group with the query `query`; its members are the points
/// the query selects now (points added or changed later join or leave it
/// as the query says).
pub fn add_point_group_query(doc: &mut CadDocument, name: &str, description: Option<&str>, query: &PointQuery) -> Result<Handle, DxfError> {
    let criteria = query.criteria()?;
    let s = support(doc)?;
    let list = entries(doc, s.group_node).ok_or_else(|| invalid("point group node cannot be read"))?;
    if name.is_empty() || list.iter().any(|e| e.name.eq_ignore_ascii_case(name)) {
        return Err(invalid("point group name is empty or already used"));
    }
    let used = root_node(doc, KEY_ALL_POINTS_QUERY).and_then(|o| queries(doc, o)).unwrap_or_default();
    let tags = list.iter().map(|e| e.id as u32).chain(used.iter().map(|q| q.tag as u32));
    let id = tags.filter(|t| *t & 0x8000_0000 != 0).max().map_or(FIRST_GROUP_TAG, |t| (t + 1).max(FIRST_GROUP_TAG)) as i32;
    let handle = doc.allocate_handle();
    let serial = next_serial(doc, &list);
    let mut list = list;
    bucket_insert(&mut list, Entry { id, handle, name: name.into() }, |e| e.id);
    write_point_group(doc, &s, handle, id, serial, name, description.unwrap_or(""), query, criteria)?;
    let root = root(doc).ok_or_else(|| invalid("drawing has no root tree node"))?;
    write_group_node(doc, s.group_node, root, &list, s.defaults.label_style, s.defaults.point_style)?;
    Ok(handle)
}

/// Renames a point group written by [`add_point_group`] and gives it a new
/// number list.
pub fn set_point_group(doc: &mut CadDocument, group: Handle, name: &str, description: Option<&str>, numbers: &[u32]) -> Result<(), DxfError> {
    set_point_group_query(doc, group, name, description, &PointQuery::Numbers(numbers.to_vec()))
}

/// Renames a point group written by [`add_point_group_query`] and gives it
/// a new query.
pub fn set_point_group_query(doc: &mut CadDocument, group: Handle, name: &str, description: Option<&str>, query: &PointQuery) -> Result<(), DxfError> {
    let criteria = query.criteria()?;
    let s = support(doc)?;
    let mut list = entries(doc, s.group_node).ok_or_else(|| invalid("point group node cannot be read"))?;
    let at = list.iter().position(|e| e.handle == group).filter(|_| group != s.all_points).ok_or_else(|| invalid("handle is not a named point group"))?;
    if !plain(doc, group) || name.is_empty() || list.iter().any(|e| e.handle != group && e.name.eq_ignore_ascii_case(name)) {
        return Err(invalid("point group has overrides, or the name is empty or already used"));
    }
    list[at].name = name.into();
    let id = list[at].id;
    let serial = serial(doc, group).ok_or_else(|| invalid("point group record cannot be read"))?;
    write_point_group(doc, &s, group, id, serial, name, description.unwrap_or(""), query, criteria)?;
    let root = root(doc).ok_or_else(|| invalid("drawing has no root tree node"))?;
    write_group_node(doc, s.group_node, root, &list, s.defaults.label_style, s.defaults.point_style)
}

#[allow(clippy::too_many_arguments)]
fn write_point_group(doc: &mut CadDocument, s: &Support, handle: Handle, id: i32, serial: i32, name: &str, description: &str, query: &PointQuery, criteria: Criteria) -> Result<(), DxfError> {
    let (numbers, extents) = members(&super::points::cogo_points(doc), query);
    let query = set_query(doc, &format!("Point Group Query - {name}"), id.wrapping_add(1), criteria)?;
    let d = s.defaults;
    let refs = [d.label_style, d.point_style, query.value(), s.database.value(), s.group_node.value(), d.create_settings];
    write_group(doc, handle, s.group_node, &Group { id, serial, name: Some(name), description, numbers, extents, refs })
}
