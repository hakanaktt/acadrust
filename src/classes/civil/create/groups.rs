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
    /// Points whose full description matches any of the patterns (as for
    /// raw descriptions). The writer adds no description keys, so a point's
    /// full description is its raw description.
    FullDescription(Vec<String>),
    /// Points whose name matches any of the patterns (as for raw
    /// descriptions); the query keeps the patterns in lower case.
    Names(Vec<String>),
    /// Points whose elevation lies in `[low, high]`.
    ElevationRange(f64, f64),
    /// Points the first query selects (every point when `None`) that the
    /// second does not; neither may be an exclusion itself.
    Excluding(Option<Box<PointQuery>>, Box<PointQuery>),
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
const FULL_DESCRIPTION: Attribute = Attribute { value_type: 4, flags: 0x50, class: 0b010, number: false, id: 8 };
const NAME: Attribute = Attribute { value_type: 3, flags: 0x10, class: 0b101, number: false, id: 8 };

/// A query's constant.
enum Constant {
    Number(u32),
    Double(f64),
    Text(String),
    /// `low-high`, both ends included.
    Range(f64, f64),
}

impl Constant {
    fn text(&self) -> String {
        match self {
            Constant::Number(n) => n.to_string(),
            Constant::Double(v) => v.to_string(),
            Constant::Text(t) => t.clone(),
            Constant::Range(a, b) => format!("{a}-{b}"),
        }
    }
}

/// A query statement: what it includes (every point when `None`) and what
/// it excludes.
pub(super) struct Statement {
    include: Option<Criteria>,
    exclude: Option<Criteria>,
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
    fn statement(&self) -> Result<Statement, DxfError> {
        Ok(match self {
            PointQuery::Excluding(include, exclude) => {
                let plain = |q: &PointQuery| if matches!(q, PointQuery::Excluding(..)) { Err(invalid("an exclusion cannot hold another")) } else { q.criteria() };
                Statement { include: include.as_deref().map(plain).transpose()?, exclude: Some(plain(exclude)?) }
            }
            q => Statement { include: Some(q.criteria()?), exclude: None },
        })
    }

    fn criteria(&self) -> Result<Criteria, DxfError> {
        Ok(match self {
            PointQuery::Excluding(..) => return Err(invalid("an exclusion is a statement")),
            PointQuery::ElevationRange(a, b) => {
                if !(a.is_finite() && b.is_finite() && a <= b) {
                    return Err(invalid("elevation range needs finite ends, low first"));
                }
                Criteria { attribute: &ELEVATION, comparison: Comparison::Equal, constants: vec![Constant::Range(*a, *b)] }
            }
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
            PointQuery::RawDescription(patterns) | PointQuery::FullDescription(patterns) | PointQuery::Names(patterns) => {
                if patterns.is_empty() || patterns.iter().any(|p| p.is_empty() || p.contains(UNSUPPORTED_PATTERN)) {
                    return Err(invalid("description and name queries need patterns of text, '*' and '?'"));
                }
                let (attribute, lower) = match self {
                    PointQuery::RawDescription(_) => (&RAW_DESCRIPTION, false),
                    PointQuery::FullDescription(_) => (&FULL_DESCRIPTION, false),
                    _ => (&NAME, true),
                };
                let text = |p: &String| Constant::Text(if lower { p.to_lowercase() } else { p.clone() });
                Criteria { attribute, comparison: Comparison::Equal, constants: patterns.iter().map(text).collect() }
            }
        })
    }

    /// True when the point belongs to the query's group.
    fn selects(&self, p: &CogoPoint) -> bool {
        match self {
            PointQuery::Numbers(list) => list.contains(&p.number),
            PointQuery::Elevation(c, v) => c.holds(p.position.z, *v),
            PointQuery::RawDescription(patterns) | PointQuery::FullDescription(patterns) => {
                let d = p.description.as_deref().unwrap_or("");
                patterns.iter().any(|q| wildcard(q, d))
            }
            PointQuery::Names(patterns) => p.name.as_deref().is_some_and(|n| patterns.iter().any(|q| wildcard(q, n))),
            PointQuery::ElevationRange(a, b) => (*a..=*b).contains(&p.position.z),
            PointQuery::Excluding(include, exclude) => include.as_ref().map_or(true, |q| q.selects(p)) && !exclude.selects(p),
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
pub(super) fn set_query(doc: &mut CadDocument, title: &str, tag: i32, statement: Statement) -> Result<Handle, DxfError> {
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
    write_query(doc, handle, outer, tag, title, &statement)?;
    write_outer_query(doc, outer, root, &list)?;
    Ok(handle)
}

/// The query of `_All Points`, `(number > 0)`.
pub(super) fn all_points_query() -> Statement {
    Statement { include: Some(Criteria { attribute: &POINT_NUMBER, comparison: Comparison::Greater, constants: vec![Constant::Number(0)] }), exclude: None }
}

/// The bits that close the last node record of the included part when an
/// excluded part follows (constant in every sample: WQ01, WQ02, WQ04).
const AND_MARK: &str = "0011111111111111111111111100000000000000000000000001";

/// Attribute id of a range's equal leaf, whose constant is the range text.
const RANGE_TEXT_ID: i32 = 8;

/// Query statement `(include) AND NOT (exclude)`, either part optional,
/// each part `(attribute <comparison> c1 OR attribute <comparison> c2 OR
/// ...)`.
///
/// After the tag: `BL 0, BL 0`, five constant bits, `BL` count of
/// constant texts (byte form: the included part's, an empty text when
/// both parts are present, the excluded part's), seven zero bits; then
/// the node records: the parentheses and OR nodes (left-deep) of the
/// included part, those of the excluded part, the leaves of the included
/// part, those of the excluded part. AND, NOT and the generated
/// parentheses of a range have no record. Every record opens with the
/// attribute's header (value type, flags, 12 zero bits, class). The
/// parentheses add three bits (number attribute, 1, the part has one
/// constant), the comparison of a single constant (equal for a range or
/// under an OR) and a byte (0, or 2 under a NOT); an OR three bits
/// (number attribute, 0, last), bytes 1 and 3. The last node record of
/// the included part ends with `AND_MARK` when an excluded part follows.
/// Node records end with seven zero bits. A leaf is the number bit, the
/// attribute id and, for numbers and elevations, the constant, then seven
/// zero bits unless it is the last record; a range is three leaves: low
/// (greater or equal), high (less or equal) and an equal leaf of id 8
/// whose constant is the range text. The node kinds and the constants are
/// in the texts; a description leaf and a range repeat their text there.
/// This reproduces the `_All Points` query, the number-list queries of
/// WP08 and WG01, the elevation, description, name and full description
/// queries of WQ03 and the exclusion and range queries of WQ04 bit for bit.
fn write_query(doc: &mut CadDocument, handle: Handle, outer: Handle, tag: i32, title: &str, st: &Statement) -> Result<(), DxfError> {
    let parts: Vec<(&Criteria, bool)> = st.include.iter().map(|c| (c, false)).chain(st.exclude.iter().map(|c| (c, true))).collect();
    let both = parts.len() == 2;
    let byte = |w: &mut DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    let zeros = |w: &mut DwgMergedWriter, n: usize| bits(w, &"0".repeat(n));
    let head = |w: &mut DwgMergedWriter, a: &Attribute| bits(w, &format!("{:03b}{:08b}{}{:03b}", a.value_type, a.flags, "0".repeat(12), a.class));
    let mut texts: Vec<String> = Vec::new();
    for (c, excluded) in &parts {
        if *excluded && both {
            texts.push(String::new());
        }
        texts.extend(c.constants.iter().map(Constant::text));
    }
    let mut strings = vec![title.to_string(), String::new()];
    strings.extend(texts.iter().cloned());
    if both {
        strings.push("AeccQueryNodeAND".into());
    }
    let leaf_texts = |strings: &mut Vec<String>, op: &str| strings.extend([op, "AeccQueryNodeATTRID", "", "", "AeccQueryNodeCONST", "", ""].map(String::from));
    for (c, excluded) in &parts {
        if *excluded {
            strings.extend(["AeccQueryNodeNOT".into(), String::new()]);
        }
        let op = c.comparison.code().1;
        strings.extend(["AeccQueryNodePARENS".into(), String::new()]);
        strings.extend((1..c.constants.len()).map(|_| "AeccQueryNodeOR".to_string()));
        for v in &c.constants {
            match v {
                Constant::Range(..) => {
                    strings.extend(["AeccQueryNodeGENERATED_PARENS".into(), String::new(), "AeccQueryNodeAND".into()]);
                    leaf_texts(&mut strings, "AeccQueryNodeGTE");
                    leaf_texts(&mut strings, "AeccQueryNodeLTE");
                    leaf_texts(&mut strings, "AeccQueryNodeEQ");
                    strings.push(v.text());
                }
                Constant::Text(t) => {
                    leaf_texts(&mut strings, op);
                    strings.push(t.clone());
                }
                _ => leaf_texts(&mut strings, op),
            }
        }
    }
    let leaves: usize = parts.iter().flat_map(|(c, _)| c.constants.iter()).map(|v| if matches!(v, Constant::Range(..)) { 3 } else { 1 }).sum();
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
            bl_byte(w, texts.len() as u8);
            zeros(w, 7);
            for (c, excluded) in &parts {
                let a = c.attribute;
                let k = c.constants.len();
                let close = |w: &mut DwgMergedWriter, last: bool| {
                    if last && both && !excluded {
                        bits(w, AND_MARK);
                    }
                    zeros(w, 7);
                };
                head(w, a);
                w.write_bit(a.number);
                w.write_bit(true);
                w.write_bit(k == 1);
                let single = match c.constants.first() {
                    Some(Constant::Range(..)) | None => Comparison::Equal,
                    _ => c.comparison,
                };
                byte(w, if k == 1 { single.code().0 } else { Comparison::Equal.code().0 });
                byte(w, if *excluded { 2 } else { 0 });
                close(w, k == 1);
                for j in 1..k {
                    head(w, a);
                    w.write_bit(a.number);
                    w.write_bit(false);
                    w.write_bit(j == k - 1);
                    byte(w, 1);
                    byte(w, 3);
                    close(w, j == k - 1);
                }
            }
            let mut left = leaves;
            for (c, _) in &parts {
                let a = c.attribute;
                let mut leaf = |w: &mut DwgMergedWriter, id: i32, value: Option<&Constant>| {
                    head(w, a);
                    w.write_bit(a.number);
                    w.write_bit_long(id);
                    match value {
                        Some(Constant::Number(n)) => w.write_bit_long(*n as i32),
                        Some(Constant::Double(d)) => w.write_bit_double(*d),
                        _ => {}
                    }
                    left -= 1;
                    if left > 0 {
                        zeros(w, 7);
                    }
                };
                for v in &c.constants {
                    match v {
                        Constant::Range(lo, hi) => {
                            leaf(w, a.id, Some(&Constant::Double(*lo)));
                            leaf(w, a.id, Some(&Constant::Double(*hi)));
                            leaf(w, RANGE_TEXT_ID, None);
                        }
                        v => leaf(w, a.id, Some(v)),
                    }
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
    let s = o.decode().strings;
    let texts = s.get(2..2 + k)?;
    let nodes: Vec<&str> = s.get(2 + k..)?.iter().map(String::as_str).filter(|t| t.starts_with("AeccQueryNode")).collect();
    let (both, mut rest) = match nodes.first()? {
        &"AeccQueryNodeAND" => (true, &nodes[1..]),
        _ => (false, &nodes[..]),
    };
    // A part: PARENS, its ORs, then per constant the comparison, ATTRID and
    // CONST, or for a range GENERATED_PARENS, AND and three such leaves.
    // Returns the comparison, which constants are ranges and the nodes left.
    let part = |nodes: &[&str]| -> Option<(Comparison, Vec<bool>, usize)> {
        (nodes.first()? == &"AeccQueryNodePARENS").then_some(())?;
        let ors = nodes[1..].iter().take_while(|t| **t == "AeccQueryNodeOR").count();
        let mut at = 1 + ors;
        let mut comparison = None;
        let mut ranges = Vec::new();
        for _ in 0..=ors {
            if nodes.get(at) == Some(&"AeccQueryNodeGENERATED_PARENS") {
                let want = ["AeccQueryNodeAND", "AeccQueryNodeGTE", "AeccQueryNodeATTRID", "AeccQueryNodeCONST", "AeccQueryNodeLTE", "AeccQueryNodeATTRID", "AeccQueryNodeCONST", "AeccQueryNodeEQ", "AeccQueryNodeATTRID", "AeccQueryNodeCONST"];
                (nodes.get(at + 1..at + 11)? == want).then_some(())?;
                at += 11;
                ranges.push(true);
            } else {
                let c = Comparison::ALL.into_iter().find(|c| nodes.get(at) == Some(&c.code().1))?;
                (comparison.map_or(true, |p| p == c) && nodes.get(at + 1..at + 3)? == ["AeccQueryNodeATTRID", "AeccQueryNodeCONST"]).then_some(())?;
                comparison = Some(c);
                at += 3;
                ranges.push(false);
            }
        }
        Some((comparison.unwrap_or(Comparison::Equal), ranges, at))
    };
    let mut parts = Vec::new();
    if rest.first() != Some(&"AeccQueryNodeNOT") {
        let (c, r, used) = part(rest)?;
        parts.push((false, c, r));
        rest = &rest[used..];
    }
    if rest.first() == Some(&"AeccQueryNodeNOT") {
        let (c, r, used) = part(&rest[1..])?;
        parts.push((true, c, r));
        rest = &rest[1 + used..];
    }
    (rest.is_empty() && both == (parts.len() == 2)).then_some(())?;
    // The attribute of each part, from the header of its parentheses
    // record (the included part's node records come first).
    let mut texts = texts.iter();
    let mut built = Vec::new();
    for (i, (excluded, comparison, ranges)) in parts.iter().enumerate() {
        let head = b.pos;
        let value_type = b.bits_value(3)?;
        let flags = b.bits_value(8)?;
        if i == 0 && parts.len() == 2 {
            // Skip the included part's node records to the next part's.
            b.pos = head;
            for j in 0..ranges.len() {
                b.skip(26 + 3 + 16)?;
                if j + 1 == ranges.len() {
                    b.skip(AND_MARK.len())?;
                }
                b.skip(7)?;
            }
        }
        if *excluded && parts.len() == 2 {
            (texts.next()?.is_empty()).then_some(())?;
        }
        let values: Vec<String> = texts.by_ref().take(ranges.len()).cloned().collect();
        let q = match (value_type, flags, comparison, ranges.as_slice()) {
            (2, 0x50, _, [true]) => {
                let t = &values[0];
                let cut = t.char_indices().skip(1).find(|(_, c)| *c == '-')?.0;
                PointQuery::ElevationRange(t[..cut].parse().ok()?, t[cut + 1..].parse().ok()?)
            }
            _ if ranges.iter().any(|r| *r) => return None,
            (1, 0x50, Comparison::Equal, _) => PointQuery::Numbers(values.iter().map(|t| t.parse().ok()).collect::<Option<_>>()?),
            (2, 0x50, c, [false]) => PointQuery::Elevation(*c, values[0].parse().ok()?),
            (3, 0x50, Comparison::Equal, _) => PointQuery::RawDescription(values),
            (3, 0x10, Comparison::Equal, _) => PointQuery::Names(values),
            (4, 0x50, Comparison::Equal, _) => PointQuery::FullDescription(values),
            _ => return None,
        };
        built.push((*excluded, q));
    }
    match built.as_slice() {
        [(false, q)] => Some(q.clone()),
        [(true, q)] => Some(PointQuery::Excluding(None, Box::new(q.clone()))),
        [(false, a), (true, x)] => Some(PointQuery::Excluding(Some(Box::new(a.clone())), Box::new(x.clone()))),
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
    let criteria = query.statement()?;
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
    let criteria = query.statement()?;
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
fn write_point_group(doc: &mut CadDocument, s: &Support, handle: Handle, id: i32, serial: i32, name: &str, description: &str, query: &PointQuery, criteria: Statement) -> Result<(), DxfError> {
    let (numbers, extents) = members(&super::points::cogo_points(doc), query);
    let query = set_query(doc, &format!("Point Group Query - {name}"), id.wrapping_add(1), criteria)?;
    let d = s.defaults;
    let refs = [d.label_style, d.point_style, query.value(), s.database.value(), s.group_node.value(), d.create_settings];
    write_group(doc, handle, s.group_node, &Group { id, serial, name: Some(name), description, numbers, extents, refs })
}
