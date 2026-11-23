//! Point groups that hold listed point numbers, and the query statements
//! the groups keep (the `_All Points` group is written by `points.rs`).

use super::super::bits::Bits;
use super::points::{support, Support};
use super::support::*;
use crate::classes::civil::cogo::{CLASSIFICATION_ALL, CLASSIFICATION_NONE};
use crate::classes::civil::{CogoPoint, PointGroupClassification, PointGroupOverrides};
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
    /// A custom query: comparisons joined by AND and OR, grouped by
    /// parentheses, the first one or group possibly negated.
    Custom(QueryExpression),
    /// The query with its description patterns matched case and all (the
    /// query's case-sensitive match setting; PC01). Not over name patterns
    /// nor inside or around an exclusion.
    CaseSensitive(Box<PointQuery>),
}

/// A point attribute a custom query compares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointAttribute {
    Number,
    Elevation,
    RawDescription,
    FullDescription,
    Name,
}

/// What a custom query compares an attribute with: a point number, an
/// elevation, or a description or name pattern (`*` any run, `?` one
/// character, case ignored; equal comparisons only).
#[derive(Clone, Debug, PartialEq)]
pub enum QueryConstant {
    Number(u32),
    Elevation(f64),
    Pattern(String),
}

/// A custom point group query. AND binds tighter than OR and NOT tighter
/// than both, as the application reads them; a NOT may only open the
/// query (the application refuses `AND NOT` and `OR NOT`), and no
/// comparison may open or close two parentheses.
#[derive(Clone, Debug, PartialEq)]
pub enum QueryExpression {
    Compare(PointAttribute, Comparison, QueryConstant),
    And(Box<QueryExpression>, Box<QueryExpression>),
    Or(Box<QueryExpression>, Box<QueryExpression>),
    Not(Box<QueryExpression>),
    Parens(Box<QueryExpression>),
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
    custom: Option<QueryExpression>,
    /// Patterns are matched case and all (the last bit of the statement's
    /// five-bit head).
    case_sensitive: bool,
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
                Statement { include: include.as_deref().map(plain).transpose()?, exclude: Some(plain(exclude)?), custom: None, case_sensitive: false }
            }
            PointQuery::Custom(e) => {
                custom_terms(e)?;
                Statement { include: None, exclude: None, custom: Some(e.clone()), case_sensitive: false }
            }
            PointQuery::CaseSensitive(q) => {
                if matches!(**q, PointQuery::CaseSensitive(_) | PointQuery::Excluding(..)) || q.has_names() {
                    return Err(invalid("a case-sensitive query holds no name patterns, exclusion or another case-sensitive query"));
                }
                Statement { case_sensitive: true, ..q.statement()? }
            }
            q => Statement { include: Some(q.criteria()?), exclude: None, custom: None, case_sensitive: false },
        })
    }

    fn criteria(&self) -> Result<Criteria, DxfError> {
        Ok(match self {
            PointQuery::Excluding(..) | PointQuery::Custom(_) | PointQuery::CaseSensitive(_) => return Err(invalid("an exclusion, custom or case-sensitive query is a statement")),
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

    /// It matches names somewhere, through exclusions and custom
    /// expressions too.
    fn has_names(&self) -> bool {
        match self {
            PointQuery::Names(_) => true,
            PointQuery::Excluding(include, exclude) => include.as_deref().is_some_and(PointQuery::has_names) || exclude.has_names(),
            PointQuery::Custom(e) => e.compares_names(),
            PointQuery::CaseSensitive(q) => q.has_names(),
            _ => false,
        }
    }

    /// True when the point belongs to the query's group (patterns matched
    /// case and all when `case`).
    fn selects(&self, p: &CogoPoint, case: bool) -> bool {
        match self {
            PointQuery::Numbers(list) => list.contains(&p.number),
            PointQuery::Elevation(c, v) => c.holds(p.position.z, *v),
            PointQuery::RawDescription(patterns) | PointQuery::FullDescription(patterns) => {
                let d = p.description.as_deref().unwrap_or("");
                patterns.iter().any(|q| wildcard(q, d, case))
            }
            PointQuery::Names(patterns) => p.name.as_deref().is_some_and(|n| patterns.iter().any(|q| wildcard(q, n, case))),
            PointQuery::ElevationRange(a, b) => (*a..=*b).contains(&p.position.z),
            PointQuery::Excluding(include, exclude) => include.as_ref().map_or(true, |q| q.selects(p, case)) && !exclude.selects(p, case),
            PointQuery::Custom(e) => e.selects(p, case),
            PointQuery::CaseSensitive(q) => q.selects(p, true),
        }
    }
}

/// `*` / `?` pattern match over the whole text, case ignored unless
/// `case`.
fn wildcard(pattern: &str, text: &str, case: bool) -> bool {
    let fold = |s: &str| if case { s.to_string() } else { s.to_lowercase() };
    let t: Vec<char> = fold(text).chars().collect();
    // ok[j]: the pattern read so far matches t[..j]
    let mut ok = vec![false; t.len() + 1];
    ok[0] = true;
    for c in fold(pattern).chars() {
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
/// First text id given to a group's override texts.
const FIRST_OVERRIDE_TAG: u32 = 0x8004_0001;

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
    Statement { include: Some(Criteria { attribute: &POINT_NUMBER, comparison: Comparison::Greater, constants: vec![Constant::Number(0)] }), exclude: None, custom: None, case_sensitive: false }
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
    if let Some(e) = &st.custom {
        return write_custom_query(doc, handle, outer, tag, title, e, st.case_sensitive);
    }
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
            // The last bit: patterns matched case and all (PC01).
            bits(w, if st.case_sensitive { "01001" } else { "01000" });
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
    let custom = (0..5).map(|_| b.bit()).collect::<Option<Vec<_>>>()? == [false, true, false, true, false];
    let k = usize::try_from(b.bit_long()?).ok().filter(|k| *k > 0)?;
    let seven = b.bits_value(7)?;
    let s = o.decode().strings;
    if custom {
        (seven == 0).then_some(())?;
        return read_custom_query(&mut b, s.get(2..2 + k)?, k).map(PointQuery::Custom);
    }
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
    pub settings: GroupSettings,
}

/// A point group's overrides as its record stores them (see
/// `cogo::group_record`).
#[derive(Clone)]
pub(super) struct GroupSettings {
    pub flags: u8,
    pub run: u8,
    pub elevation: f64,
    /// Raw description text id and text.
    pub raw: Option<(i32, String)>,
    /// Classification word and, for a named one, its name.
    pub classification: (i32, Option<String>),
    /// The bits after the classification word.
    pub rest: String,
}

impl Default for GroupSettings {
    fn default() -> Self {
        GroupSettings { flags: 0, run: 0, elevation: 0.0, raw: None, classification: (CLASSIFICATION_NONE, None), rest: GROUP_TAIL[14..].to_string() }
    }
}

/// The settings of a group record of the recent form.
pub(super) fn group_settings(doc: &CadDocument, group: Handle) -> Option<GroupSettings> {
    let o = object(doc, group)?;
    let (data, start, end) = o.main_stream()?;
    let g = crate::classes::civil::cogo::group_record(data, start, end)?;
    let t = g.tail?;
    let strings = o.decode().strings;
    let own = strings.iter().position(|s| s == "AeccImpPG" || s == "AeccImpAllPointsPG")? + 1;
    let mut texts = strings.into_iter().skip(own);
    let raw = if t.raw_tag != 0 { Some((t.raw_tag, texts.next()?)) } else { None };
    let named = t.classification as u32 & 0x8000_0000 != 0;
    let classification = (t.classification, if named { Some(texts.next()?) } else { None });
    let rest = (t.rest..end).map(|i| if data[i / 8] >> (7 - i % 8) & 1 == 1 { '1' } else { '0' }).collect();
    Some(GroupSettings { flags: g.flags, run: g.run, elevation: t.elevation, raw, classification, rest })
}

/// A text id for an override text of a group: an id another group uses
/// for the same text, else one above every override id of the groups.
fn override_tag(doc: &CadDocument, text: &str) -> i32 {
    let mut top = FIRST_OVERRIDE_TAG - 1;
    for h in doc.objects.iter().filter(|(_, o)| matches!(o, crate::objects::ObjectType::Arch(a) if a.dxf_name == "AECC_POINT_PG")).map(|(h, _)| *h) {
        let Some(s) = group_settings(doc, h) else { continue };
        for (tag, t) in s.raw.iter().map(|r| (r.0, Some(&r.1))).chain([(s.classification.0, s.classification.1.as_ref())]) {
            if tag as u32 & 0x8000_0000 == 0 {
                continue;
            }
            if t.is_some_and(|t| t == text) {
                return tag;
            }
            top = top.max(tag as u32);
        }
    }
    (top.max(FIRST_OVERRIDE_TAG - 1) + 1) as i32
}

/// Sets what point group `group` overrides in its points: styles,
/// elevation, raw description, lock and classification. The group keeps
/// its points, query and serial; it must be of the form recent releases
/// write. A custom classification is named, not checked.
pub fn set_point_group_overrides(doc: &mut CadDocument, group: Handle, o: &PointGroupOverrides) -> Result<(), DxfError> {
    let bad = || invalid("point group record cannot be rewritten");
    if !plain(doc, group) {
        return Err(bad());
    }
    let obj = object(doc, group).ok_or_else(bad)?;
    let (owner, d) = (obj.owner, obj.decode());
    let (data, start, end) = obj.main_stream().ok_or_else(bad)?;
    let rec = crate::classes::civil::cogo::group_record(data, start, end).ok_or_else(bad)?;
    let tail = rec.tail.ok_or_else(bad)?;
    let mut settings = group_settings(doc, group).ok_or_else(bad)?;
    let id = fields_of(obj).and_then(|(x, s0, e0)| Bits::new(x, s0, e0).bit_long()).ok_or_else(bad)?;
    let all = d.strings.iter().any(|s| s == "AeccImpAllPointsPG");
    let (name, description) = if all { (None, String::new()) } else { (d.strings.first().cloned(), d.strings.get(1).cloned().unwrap_or_default()) };
    let mut refs = [0, 1, 2, 3, 4, 5].map(|j| d.handles.get(j).copied().unwrap_or(0));
    if let Some(h) = o.label_style {
        refs[0] = h;
    }
    if let Some(h) = o.point_style {
        refs[1] = h;
    }
    if o.elevation.is_some_and(|e| !e.is_finite()) {
        return Err(invalid("override elevation must be finite"));
    }
    settings.run = (u8::from(o.label_style.is_some()) << 4) | (u8::from(o.point_style.is_some()) << 3) | (settings.run & 0b110) | u8::from(o.locked);
    settings.flags = (settings.flags & !3) | (u8::from(o.elevation.is_some()) << 1) | u8::from(o.raw_description.is_some());
    if let Some(e) = o.elevation {
        settings.elevation = e;
    }
    if let Some(text) = &o.raw_description {
        let tag = match &settings.raw {
            Some((tag, t)) if t == text => *tag,
            _ => override_tag(doc, text),
        };
        settings.raw = Some((tag, text.clone()));
    }
    settings.classification = match &o.classification {
        PointGroupClassification::None => (CLASSIFICATION_NONE, None),
        PointGroupClassification::All => (CLASSIFICATION_ALL, None),
        PointGroupClassification::Custom(n) if n.trim().is_empty() => return Err(invalid("a classification needs a name")),
        PointGroupClassification::Custom(n) => (override_tag(doc, n), Some(n.clone())),
    };
    let g = Group { id, serial: rec.serial.ok_or_else(bad)?, name: name.as_deref(), description: &description, numbers: rec.numbers, extents: Some(tail.extents), refs, settings };
    write_group(doc, group, owner, &g)
}

/// `BL id, BL 0, BL 0` (no override lists or keys), `a` = 0 in its byte
/// form, 5 + 6 constant bits, `BL serial`, the numbers as `RL`, then two
/// constant bits, `BD 0`, the extents (min, max) and [`GROUP_TAIL`].
pub(super) fn write_group(doc: &mut CadDocument, handle: Handle, owner: Handle, g: &Group) -> Result<(), DxfError> {
    let (min, max) = g.extents.unwrap_or((Vector3::new(1e20, 1e20, 1e20), Vector3::new(-1e20, -1e20, -1e20)));
    let mut strings = match g.name {
        Some(name) => vec![name.to_string(), g.description.to_string(), "AeccImpPG".into()],
        None => vec![String::new(), "AeccImpAllPointsPG".into()],
    };
    let st = &g.settings;
    strings.extend(st.raw.iter().map(|r| r.1.clone()));
    strings.extend(st.classification.1.iter().cloned());
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
            bl_byte(w, st.flags);
            for i in (0..5).rev() {
                w.write_bit(st.run >> i & 1 == 1);
            }
            bits(w, "100000");
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
            w.write_bit_double(st.elevation);
            w.write_bit_long(st.raw.as_ref().map_or(0, |r| r.0));
            w.write_bit_long(st.classification.0);
            bits(w, &st.rest);
        },
        Body { refs, strings },
    )
}

/// Members of a query among `points` (sorted) and their extents.
fn members(points: &[CogoPoint], query: &PointQuery) -> (Vec<u32>, Option<(Vector3, Vector3)>) {
    let mut numbers: Vec<u32> = points.iter().filter(|p| query.selects(p, false)).map(|p| p.number).collect();
    numbers.sort_unstable();
    let mut extents: Option<(Vector3, Vector3)> = None;
    for p in points.iter().filter(|p| query.selects(p, false)) {
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
        let settings = group_settings(doc, e.handle).unwrap_or_default();
        let g = Group { id: e.id, serial, name: Some(&e.name), description: &description, numbers, extents, refs, settings };
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
    write_group(doc, handle, s.group_node, &Group { id, serial, name: Some(name), description, numbers, extents, refs, settings: GroupSettings::default() })
}

/// The attribute's record header, and the attribute a header names.
fn attribute(a: PointAttribute) -> &'static Attribute {
    match a {
        PointAttribute::Number => &POINT_NUMBER,
        PointAttribute::Elevation => &ELEVATION,
        PointAttribute::RawDescription => &RAW_DESCRIPTION,
        PointAttribute::FullDescription => &FULL_DESCRIPTION,
        PointAttribute::Name => &NAME,
    }
}

const ATTRIBUTES: [PointAttribute; 5] = [PointAttribute::Number, PointAttribute::Elevation, PointAttribute::RawDescription, PointAttribute::FullDescription, PointAttribute::Name];

impl QueryExpression {
    /// It compares a name somewhere.
    fn compares_names(&self) -> bool {
        match self {
            QueryExpression::Compare(a, ..) => *a == PointAttribute::Name,
            QueryExpression::And(a, b) | QueryExpression::Or(a, b) => a.compares_names() || b.compares_names(),
            QueryExpression::Not(e) | QueryExpression::Parens(e) => e.compares_names(),
        }
    }

    fn selects(&self, p: &CogoPoint, case: bool) -> bool {
        match self {
            QueryExpression::Compare(a, c, v) => match (a, v) {
                (PointAttribute::Number, QueryConstant::Number(n)) => c.holds(f64::from(p.number), f64::from(*n)),
                (PointAttribute::Elevation, QueryConstant::Elevation(z)) => c.holds(p.position.z, *z),
                (PointAttribute::Name, QueryConstant::Pattern(t)) => p.name.as_deref().is_some_and(|n| wildcard(t, n, case)),
                (_, QueryConstant::Pattern(t)) => wildcard(t, p.description.as_deref().unwrap_or(""), case),
                _ => false,
            },
            QueryExpression::And(a, b) => a.selects(p, case) && b.selects(p, case),
            QueryExpression::Or(a, b) => a.selects(p, case) || b.selects(p, case),
            QueryExpression::Not(e) => !e.selects(p, case),
            QueryExpression::Parens(e) => e.selects(p, case),
        }
    }
}

/// One comparison of a custom query as its records store it, in the
/// order of the query text: what joins it to the one before (0 none, 1
/// AND, 3 OR; 2 a NOT opening the query) and whether it opens or closes
/// parentheses.
#[derive(Clone, Debug, PartialEq)]
struct Term {
    attribute: PointAttribute,
    comparison: Comparison,
    constant: QueryConstant,
    joint: u8,
    open: bool,
    close: bool,
}

/// The comparisons of a custom query (an OR under an AND or NOT and an
/// AND under a NOT are put in parentheses).
fn custom_terms(e: &QueryExpression) -> Result<Vec<Term>, DxfError> {
    fn walk(e: &QueryExpression, joint: &mut Option<u8>, open: &mut bool, out: &mut Vec<Term>) -> Result<(), DxfError> {
        let parens = |e: &QueryExpression| QueryExpression::Parens(Box::new(e.clone()));
        match e {
            QueryExpression::Compare(a, c, v) => {
                let ok = match (a, v) {
                    (PointAttribute::Number, QueryConstant::Number(n)) => *n <= i32::MAX as u32,
                    (PointAttribute::Elevation, QueryConstant::Elevation(z)) => z.is_finite(),
                    (PointAttribute::RawDescription | PointAttribute::FullDescription | PointAttribute::Name, QueryConstant::Pattern(t)) => *c == Comparison::Equal && !t.is_empty() && !t.contains(UNSUPPORTED_PATTERN) && !t.contains('\''),
                    _ => false,
                };
                if !ok {
                    return Err(invalid("a custom query compares numbers and elevations with numbers, descriptions and names (equal) with patterns"));
                }
                let constant = match (a, v) {
                    (PointAttribute::Name, QueryConstant::Pattern(t)) => QueryConstant::Pattern(t.to_lowercase()),
                    _ => v.clone(),
                };
                out.push(Term { attribute: *a, comparison: *c, constant, joint: joint.take().unwrap_or(0), open: std::mem::take(open), close: false });
            }
            QueryExpression::And(a, b) | QueryExpression::Or(a, b) => {
                let and = matches!(e, QueryExpression::And(..));
                for (i, x) in [a, b].into_iter().enumerate() {
                    if i == 1 {
                        *joint = Some(if and { 1 } else { 3 });
                    }
                    match &**x {
                        QueryExpression::Or(..) if and => walk(&parens(x), joint, open, out)?,
                        QueryExpression::Not(_) if i == 1 => return Err(invalid("a NOT may only open a custom query")),
                        _ => walk(x, joint, open, out)?,
                    }
                }
            }
            QueryExpression::Not(x) => {
                if !out.is_empty() || joint.is_some() {
                    return Err(invalid("a NOT may only open a custom query"));
                }
                *joint = Some(2);
                match &**x {
                    QueryExpression::And(..) | QueryExpression::Or(..) => walk(&parens(x), joint, open, out)?,
                    _ => walk(x, joint, open, out)?,
                }
            }
            QueryExpression::Parens(x) => {
                if *open {
                    return Err(invalid("a custom query comparison opens two parentheses"));
                }
                *open = true;
                let first = out.len();
                walk(x, joint, open, out)?;
                let grew = out.len() > first;
                let last = out.last_mut().filter(|_| grew).ok_or_else(|| invalid("empty parentheses"))?;
                if last.close {
                    return Err(invalid("a custom query comparison closes two parentheses"));
                }
                last.close = true;
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(e, &mut None, &mut false, &mut out)?;
    if out.iter().filter(|t| t.open).count() != out.iter().filter(|t| t.close).count() {
        return Err(invalid("custom query parentheses do not pair"));
    }
    Ok(out)
}

/// The expression the comparisons read as: NOT before AND before OR,
/// left to right (the tree whose nodes the query's texts list).
fn parse_terms(terms: &[Term]) -> Option<QueryExpression> {
    fn or(t: &[Term], at: &mut usize) -> Option<QueryExpression> {
        let mut e = and(t, at)?;
        while t.get(*at).is_some_and(|x| x.joint == 3) {
            e = QueryExpression::Or(Box::new(e), Box::new(and(t, at)?));
        }
        Some(e)
    }
    fn and(t: &[Term], at: &mut usize) -> Option<QueryExpression> {
        let mut e = unary(t, at)?;
        while t.get(*at).is_some_and(|x| x.joint == 1) {
            e = QueryExpression::And(Box::new(e), Box::new(unary(t, at)?));
        }
        Some(e)
    }
    fn unary(t: &[Term], at: &mut usize) -> Option<QueryExpression> {
        let x = t.get(*at)?;
        let e = if x.open {
            // Parse the group as if it opened the query, then close it.
            let mut inner: Vec<Term> = t.to_vec();
            inner[*at].open = false;
            inner[*at].joint = 0;
            let mut depth = 0;
            let depth_end = (*at..t.len()).find(|j| {
                depth += i32::from(t[*j].open) - i32::from(t[*j].close);
                depth == 0
            })?;
            inner[depth_end].close = false;
            let mut i = *at;
            let e = or(&inner[..=depth_end], &mut i)?;
            (i == depth_end + 1).then_some(())?;
            *at = i;
            QueryExpression::Parens(Box::new(e))
        } else {
            *at += 1;
            QueryExpression::Compare(x.attribute, x.comparison, x.constant.clone())
        };
        Some(if x.joint == 2 { QueryExpression::Not(Box::new(e)) } else { e })
    }
    let mut at = 0;
    let e = or(terms, &mut at)?;
    (at == terms.len()).then_some(e)
}

fn constant_text(c: &QueryConstant) -> String {
    match c {
        QueryConstant::Number(n) => n.to_string(),
        QueryConstant::Elevation(z) => z.to_string(),
        QueryConstant::Pattern(t) => t.clone(),
    }
}

/// Query statement of a custom query (PQ01 to PQ03): after the tag `BL 0,
/// BL 0`, the bits 01010, `BL` the number of comparisons (byte form) and
/// seven zero bits; then a record per comparison in the order of the
/// query text: the attribute's header, its number bit, opens and closes
/// parentheses, the comparison code byte, the joint byte (0 first, 1 AND,
/// 3 OR, 2 a NOT opening the query) and seven zero bits; then the leaves
/// in the same order: header, number bit, attribute id, the constant
/// (`BL` number, `BD` elevation, none for a pattern) and seven zero bits
/// but after the last. The texts list the constants, then the expression's
/// nodes in pre-order (AND, OR; NOT and PARENS each with an empty text;
/// a comparison as in the standard queries, a pattern after its leaf).
fn write_custom_query(doc: &mut CadDocument, handle: Handle, outer: Handle, tag: i32, title: &str, e: &QueryExpression, case_sensitive: bool) -> Result<(), DxfError> {
    let terms = custom_terms(e)?;
    let tree = parse_terms(&terms).ok_or_else(|| invalid("custom query cannot be written"))?;
    let mut strings = vec![title.to_string(), String::new()];
    strings.extend(terms.iter().map(|t| constant_text(&t.constant)));
    fn nodes(e: &QueryExpression, s: &mut Vec<String>) {
        match e {
            QueryExpression::Compare(_, c, v) => {
                s.extend([c.code().1, "AeccQueryNodeATTRID", "", "", "AeccQueryNodeCONST", "", ""].map(String::from));
                if let QueryConstant::Pattern(t) = v {
                    s.push(t.clone());
                }
            }
            QueryExpression::And(a, b) | QueryExpression::Or(a, b) => {
                s.push(if matches!(e, QueryExpression::And(..)) { "AeccQueryNodeAND" } else { "AeccQueryNodeOR" }.into());
                nodes(a, s);
                nodes(b, s);
            }
            QueryExpression::Not(x) | QueryExpression::Parens(x) => {
                s.extend([if matches!(e, QueryExpression::Not(_)) { "AeccQueryNodeNOT" } else { "AeccQueryNodePARENS" }.to_string(), String::new()]);
                nodes(x, s);
            }
        }
    }
    nodes(&tree, &mut strings);
    let head = |w: &mut DwgMergedWriter, a: &Attribute| bits(w, &format!("{:03b}{:08b}{}{:03b}", a.value_type, a.flags, "0".repeat(12), a.class));
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
            bits(w, if case_sensitive { "01011" } else { "01010" });
            bl_byte(w, terms.len() as u8);
            bits(w, "0000000");
            for t in &terms {
                let a = attribute(t.attribute);
                head(w, a);
                w.write_bit(a.number);
                w.write_bit(t.open);
                w.write_bit(t.close);
                bits(w, &format!("{:08b}{:08b}0000000", t.comparison.code().0, t.joint));
            }
            for (i, t) in terms.iter().enumerate() {
                let a = attribute(t.attribute);
                head(w, a);
                w.write_bit(a.number);
                w.write_bit_long(a.id);
                match &t.constant {
                    QueryConstant::Number(n) => w.write_bit_long(*n as i32),
                    QueryConstant::Elevation(z) => w.write_bit_double(*z),
                    QueryConstant::Pattern(_) => {}
                }
                if i + 1 < terms.len() {
                    bits(w, "0000000");
                }
            }
        },
        Body { refs: Vec::new(), strings },
    )
}

/// A custom query's expression from its records (after the head) and its
/// constant texts.
fn read_custom_query(b: &mut Bits, texts: &[String], k: usize) -> Option<QueryExpression> {
    let header = |b: &mut Bits| -> Option<PointAttribute> {
        let (value_type, flags) = (b.bits_value(3)?, b.bits_value(8)?);
        (b.bits_value(8)? == 0 && b.bits_value(4)? == 0).then_some(())?;
        let class = b.bits_value(3)?;
        ATTRIBUTES.into_iter().find(|a| {
            let x = attribute(*a);
            (x.value_type, x.flags, x.class) == (value_type, flags, class)
        })
    };
    let mut terms = Vec::with_capacity(k);
    for _ in 0..k {
        let a = header(b)?;
        (b.bit()? == attribute(a).number).then_some(())?;
        let (open, close) = (b.bit()?, b.bit()?);
        let code = b.bits_value(8)?;
        let joint = b.bits_value(8)?;
        (b.bits_value(7)? == 0 && [0, 1, 2, 3].contains(&joint)).then_some(())?;
        let comparison = Comparison::ALL.into_iter().find(|c| c.code().0 == code)?;
        terms.push(Term { attribute: a, comparison, constant: QueryConstant::Number(0), joint, open, close });
    }
    for (i, t) in terms.iter_mut().enumerate() {
        (header(b)? == t.attribute && b.bit()? == attribute(t.attribute).number && b.bit_long()? == attribute(t.attribute).id).then_some(())?;
        t.constant = match t.attribute {
            PointAttribute::Number => QueryConstant::Number(u32::try_from(b.bit_long()?).ok()?),
            PointAttribute::Elevation => QueryConstant::Elevation(b.bit_double()?),
            _ => QueryConstant::Pattern(texts.get(i)?.clone()),
        };
        if i + 1 < k {
            (b.bits_value(7)? == 0).then_some(())?;
        }
    }
    parse_terms(&terms)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A custom query's comparisons in text order with their joints and
    /// parentheses, and the tree its texts list (PQ01 C8, PQ02 D2).
    #[test]
    fn custom_queries_read_as_the_application_parses_them() {
        let cmp = |a, c, v| QueryExpression::Compare(a, c, v);
        let n = |c, v| cmp(PointAttribute::Number, c, QueryConstant::Number(v));
        let z = |c, v| cmp(PointAttribute::Elevation, c, QueryConstant::Elevation(v));
        // n <= 3 OR (n >= 8 AND z < 100): the AND is put in parentheses.
        let e = QueryExpression::Or(Box::new(n(Comparison::LessOrEqual, 3)), Box::new(QueryExpression::Parens(Box::new(QueryExpression::And(Box::new(n(Comparison::GreaterOrEqual, 8)), Box::new(z(Comparison::Less, 100.0)))))));
        let t = custom_terms(&e).unwrap();
        assert_eq!(t.iter().map(|t| (t.joint, t.open, t.close)).collect::<Vec<_>>(), vec![(0, false, false), (3, true, false), (1, false, true)]);
        assert_eq!(parse_terms(&t).unwrap(), e);
        // NOT n = 3 AND z > 23 reads (NOT n = 3) AND z > 23.
        let e = QueryExpression::And(Box::new(QueryExpression::Not(Box::new(n(Comparison::Equal, 3)))), Box::new(z(Comparison::Greater, 23.0)));
        let t = custom_terms(&e).unwrap();
        assert_eq!(t.iter().map(|t| t.joint).collect::<Vec<_>>(), vec![2, 1]);
        assert_eq!(parse_terms(&t).unwrap(), e);
        // An OR under an AND gets parentheses; AND NOT is refused.
        let e = QueryExpression::And(Box::new(n(Comparison::Equal, 1)), Box::new(QueryExpression::Or(Box::new(n(Comparison::Equal, 2)), Box::new(n(Comparison::Equal, 3)))));
        assert_eq!(custom_terms(&e).unwrap().iter().map(|t| (t.open, t.close)).collect::<Vec<_>>(), vec![(false, false), (true, false), (false, true)]);
        assert!(custom_terms(&QueryExpression::And(Box::new(n(Comparison::Equal, 1)), Box::new(QueryExpression::Not(Box::new(n(Comparison::Equal, 2)))))).is_err());
    }

    /// A case-sensitive query sets the statement's flag and matches case
    /// and all (PC01); name patterns are refused.
    #[test]
    fn case_sensitive_queries_match_case_and_all() {
        assert!(wildcard("test*", "TEST", false) && !wildcard("test*", "TEST", true) && wildcard("TE?T", "TEST", true));
        let raw = |p: &str| PointQuery::RawDescription(vec![p.to_string()]);
        assert!(PointQuery::CaseSensitive(Box::new(raw("test*"))).statement().unwrap().case_sensitive);
        assert!(!raw("test*").statement().unwrap().case_sensitive);
        assert!(PointQuery::CaseSensitive(Box::new(PointQuery::Names(vec!["a".into()]))).statement().is_err());
        assert!(PointQuery::Excluding(None, Box::new(PointQuery::CaseSensitive(Box::new(raw("a"))))).statement().is_err());
        let names = || Box::new(PointQuery::Names(vec!["a".into()]));
        let excluding = PointQuery::Excluding(Some(names()), Box::new(raw("a")));
        assert!(excluding.has_names());
        assert!(PointQuery::CaseSensitive(Box::new(excluding)).statement().is_err());
        assert!(PointQuery::CaseSensitive(Box::new(PointQuery::Excluding(None, Box::new(raw("a"))))).statement().is_err());
    }
}
