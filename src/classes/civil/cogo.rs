//! COGO points.
//!
//! A COGO point entity (`AECC_COGO_POINT`) is not an AEC entity: after the
//! common entity data its body is only
//!
//! ```text
//! BS 0x04BE, BL point number, 6 bits (001010)
//! ```
//!
//! with no references of its own; what it draws is its proxy graphics. The
//! drawing's point database (`AECC_POINT_NODE`) holds every point:
//!
//! ```text
//! BS 0x04BE, BL 34 (its key in the root tree node), BL 0, BL 0, 3 bits (010)
//! BL count, count × { BL 1, 3BD position, BL number, BS 0x0420 }
//! BL n, n × BL number           points with their own point style
//! BL m, m × BL number           points with their own label style
//! BL 0
//! BL j, j × { BL number, BL name tag }          named points
//! BL j, j × { BL name tag, BL number }          the same, keyed by name
//! BL k, k × { BL number, BL description tag }
//! 13 × slot: BL e, e × { BL key | 0x40000000, BL c, c × { BL number, value } }
//! handles: n × point style, m × label style, the AeccDbCogoPoint class node
//! strings: "", the names of the j points, the same names again, the raw
//!          description of each of the k points in order, then for each
//!          slot entry its property's key text and its text values
//! ```
//!
//! The slots hold the values of user-defined properties by kind: slot 4
//! doubles (BD), 5 booleans (B; RC in the definitions), 6 texts (none inline: each text follows
//! the key text), 7 integers (BL); the others are empty in every sample.
//! `key` is the property's (see [`UserProperty`]).
//!
//! Records are in the database's own order, not by number. A point without
//! its own style draws with its point group's; the description tag
//! (`0x80000000 | n`) is the application's id for the description text and
//! equal texts share it. The values given as constants are the same in
//! every sample.
//!
//! A point group (`AECC_POINT_PG`) lists the numbers of the points it holds:
//!
//! ```text
//! BS 0x04BE, BL id, BL n, n × BL, BL m, m × BL,
//! [BL 0 (as 2 bits), B 1, RC count, count × class key]   settings
//! BL flags, 5 bits, [flags < 4: 6 bits, BL serial], BL count, count × RL number
//! [flags < 4: 2 bits, BD, 3BD extents min, 3BD extents max,
//!   BD fixed elevation, BL raw description text id (0: none),
//!   BL classification (87 none, 0xEA7 all, a text id: the one named), the rest]
//! ```
//!
//! Groups made or edited by recent releases store flags below 4 (1 the
//! raw description overridden, 2 the elevation) and a serial that counts
//! the groups made; the 5 bits are the label style overridden, the point
//! style overridden, two unset bits and locked. Older releases' groups
//! store other flags (0x48, 0x4A) and nothing before the count. The
//! override texts follow the group's own (name, description,
//! "AeccImpPG"): the raw description, then the classification name; the
//! overriding styles are the group's first two references. `id` is 33 for `_All Points` and the group name's tag
//! for the others; the point group node (`AECC_POINT_GROUP_NODE`) lists them
//! in draw order: `BS 0x04BE, BL 32 (its key in the root tree node), BL n,
//! n × BL id, BL 0, 3 bits (010)`,
//! handles `n × group, label style, point style`.

use super::alignment::key;
use super::bits::Bits;
use crate::types::Vector3;

/// One point of the drawing's point database.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CogoPoint {
    pub number: u32,
    /// Easting, northing and elevation.
    pub position: Vector3,
    /// The point's own point style (`AECC_POINT_STYLE`), 0 when it draws
    /// with its point group's.
    pub style: u64,
    /// The point's own label style (`AECC_LABEL_COLLECTOR_STYLE`), 0 when
    /// it uses its point group's.
    pub label_style: u64,
    /// Raw description, `None` when the point has none.
    pub description: Option<String>,
    /// Point name, `None` when the point has none.
    pub name: Option<String>,
    /// Values of user-defined properties the point holds, by the
    /// property's key text ([`UserProperty::key`]).
    pub properties: Vec<(String, PropertyValue)>,
}

/// Value of a user-defined property.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PropertyValue {
    Integer(i32),
    Double(f64),
    Boolean(bool),
    Text(String),
    /// The index of one of an enumeration property's choices.
    Choice(u32),
}

impl PropertyValue {
    /// Kind code of the property keys (`0x80000000 | code << 24 | serial`).
    pub fn code(&self) -> u32 {
        match self {
            Self::Integer(_) => 1,
            Self::Double(_) => 2,
            Self::Boolean(_) => 3,
            Self::Text(_) => 4,
            Self::Choice(_) => 13,
        }
    }

    /// Slot of the point database holding values of kind `code`.
    pub(super) fn slot(code: u32) -> Option<usize> {
        match code {
            1 | 13 => Some(7),
            2 => Some(4),
            3 => Some(5),
            4 => Some(6),
            _ => None,
        }
    }

    /// Reads a value of kind `code`; a text value takes the next text.
    pub(super) fn read(bits: &mut Bits, code: u32, texts: &mut dyn Iterator<Item = String>) -> Option<Self> {
        Some(match code {
            1 => Self::Integer(bits.bit_long()?),
            2 => Self::Double(bits.bit_double()?),
            3 => Self::Boolean(bits.raw_short_byte()? != 0),
            4 => Self::Text(texts.next()?),
            13 => Self::Choice(u32::try_from(bits.bit_long()?).ok()?),
            _ => return None,
        })
    }
}

/// A user-defined property of COGO points (`AECC_USER_DEFINED_ATTRIBUTE`):
///
/// ```text
/// BS 0x04BE, BL key, BL 0, BL 0, 3 bits 010      (a tree node without children)
/// BL key, default (integer BL, double BD, boolean RC, text: none,
///   choice: BL index), BL key
/// B use default, B bounded,
/// [bounded: BL code << 24 | 0x400000 if inclusive, lower,
///           BL code << 24 | 0x400000 if inclusive, upper]
/// [enumeration: BL n choices]
/// BL 0x4000806B, 3 bits 010
/// handles: hard pointer to itself
/// strings: key, "", key, [text default], key, the type-info class name,
///          [the n choices], name, description, classification name
/// ```
///
/// `key` is `0x80000000 | code << 24 | serial` with the code of
/// [`PropertyValue::code`] and a serial counted per kind over the drawing;
/// its text is 32 hexadecimal digits of an identifier and the name. Whole
/// numbers, doubles and booleans are bounded (by default by their full
/// range, ±1e24 for doubles), texts are not.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UserProperty {
    /// Key text: identifier digits and name.
    pub key: String,
    pub name: String,
    pub description: String,
    /// Name of the classification holding the property.
    pub classification: String,
    pub default: PropertyValue,
    pub use_default: bool,
    /// Lower and upper bound, each with whether it is inclusive.
    pub bounds: Option<((PropertyValue, bool), (PropertyValue, bool))>,
    /// An enumeration's choices, in order (empty for the other kinds).
    pub choices: Vec<String>,
}

/// Inclusive flag of a bound word.
pub(super) const INCLUSIVE: u32 = 0x0040_0000;

/// Kind code of a property key.
pub(super) fn key_code(key: i32) -> u32 {
    (key as u32 >> 24) & 0x3F
}

pub(super) fn user_property(data: &[u8], start: usize, end: usize, strings: &[String]) -> Option<UserProperty> {
    let mut bits = Bits::new(data, start, end);
    (bits.bit_short()? == 0x04BE).then_some(())?;
    let key = bits.bit_long()?;
    (bits.bit_long()? == 0 && bits.bit_long()? == 0 && bits.bits_value(3)? == 0b010).then_some(())?;
    let code = key_code(key);
    (bits.bit_long()? == key).then_some(())?;
    // Texts: key, "", key, [text default], key, class name, name, description, classification.
    let mut texts = strings.iter().skip(3).cloned();
    let default = PropertyValue::read(&mut bits, code, &mut texts)?;
    (bits.bit_long()? == key).then_some(())?;
    let use_default = bits.bit()?;
    let bounds = if bits.bit()? {
        let mut bound = || -> Option<(PropertyValue, bool)> {
            let word = bits.bit_long()? as u32;
            (word & !INCLUSIVE == code << 24).then_some(())?;
            Some((PropertyValue::read(&mut bits, code, &mut std::iter::empty())?, word & INCLUSIVE != 0))
        };
        let lower = bound()?;
        Some((lower, bound()?))
    } else {
        None
    };
    let count = if code == 13 { usize::try_from(bits.bit_long()?).ok()? } else { 0 };
    bits.bit_long()?;
    (bits.bits_value(3)? == 0b010).then_some(())?;
    let rest: Vec<String> = texts.collect();
    let (choices, rest) = (rest.get(2..2 + count)?.to_vec(), [&rest[..2], &rest[2 + count..]].concat());
    let [_, _class, name, description, classification] = rest.as_slice() else {
        return None;
    };
    Some(UserProperty {
        key: strings.first()?.clone(),
        name: name.clone(),
        description: description.clone(),
        classification: classification.clone(),
        default,
        use_default,
        bounds,
        choices,
    })
}

/// Point number a COGO point entity refers to.
pub(super) fn entity_number(data: &[u8], start: usize, end: usize) -> Option<u32> {
    let mut bits = Bits::new(data, start, end);
    (bits.bit_short()? == 0x04BE).then_some(())?;
    u32::try_from(bits.bit_long()?).ok().filter(|n| *n > 0)
}

fn record(bits: &mut Bits) -> Option<(u32, Vector3)> {
    (bits.bit_long()? == 1).then_some(())?;
    let position = Vector3::new(bits.bit_double()?, bits.bit_double()?, bits.bit_double()?);
    let number = u32::try_from(bits.bit_long()?).ok().filter(|n| *n > 0)?;
    (bits.bit_short()? == 0x0420).then_some(())?;
    Some((number, position))
}

fn numbers(bits: &mut Bits) -> Option<Vec<u32>> {
    let n = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining() / 2)?;
    (0..n).map(|_| u32::try_from(bits.bit_long()?).ok()).collect()
}

/// The fields of a point database (see the module docs).
struct PointDatabase {
    records: Vec<(u32, Vector3)>,
    styled: Vec<u32>,
    labelled: Vec<u32>,
    named: Vec<u32>,
    /// Length of the list after the names (the same names keyed by text).
    again: usize,
    described: Vec<u32>,
    /// Property values: (key, values by number; text values `None`).
    values: Vec<(i32, Vec<(u32, Option<PropertyValue>)>)>,
}

/// `BL k, k × { BL number, BL text tag }`: the numbers.
fn tagged(bits: &mut Bits) -> Option<Vec<u32>> {
    let k = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining() / 4)?;
    (0..k)
        .map(|_| {
            let number = u32::try_from(bits.bit_long()?).ok()?;
            bits.bit_long()?;
            Some(number)
        })
        .collect()
}

fn database(bits: &mut Bits) -> Option<PointDatabase> {
    (bits.bit_short()? == 0x04BE).then_some(())?;
    (bits.bit_long()? == 34 && bits.bit_long()? == 0 && bits.bit_long()? == 0).then_some(())?;
    bits.skip(3)?;
    let count = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 44 <= bits.remaining())?;
    let records = (0..count).map(|_| record(bits)).collect::<Option<Vec<_>>>()?;
    let styled = numbers(bits)?;
    let labelled = numbers(bits)?;
    (bits.bit_long()? == 0).then_some(())?;
    let named = tagged(bits)?;
    // The same names keyed by text: { BL tag, BL number }.
    let again = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining() / 4)?;
    for _ in 0..2 * again {
        bits.bit_long()?;
    }
    let described = tagged(bits)?;
    let mut values = Vec::new();
    for slot in 0..13 {
        for _ in 0..bits.bit_long()? {
            let key = bits.bit_long()? & !0x4000_0000;
            let code = key_code(key);
            (PropertyValue::slot(code) == Some(slot)).then_some(())?;
            let count = bits.bit_long()?;
            let list = (0..count)
                .map(|_| {
                    let number = u32::try_from(bits.bit_long()?).ok()?;
                    let value = match code {
                        3 => Some(PropertyValue::Boolean(bits.bit()?)),
                        4 => None,
                        _ => Some(PropertyValue::read(bits, code, &mut std::iter::empty())?),
                    };
                    Some((number, value))
                })
                .collect::<Option<Vec<_>>>()?;
            values.push((key, list));
        }
    }
    Some(PointDatabase { records, styled, labelled, named, again, described, values })
}

/// Every point of a point database, in record order. `handles` and
/// `strings` are the record's references and texts in stream order.
pub(super) fn points(data: &[u8], start: usize, end: usize, handles: &[u64], strings: &[String]) -> Vec<CogoPoint> {
    let Some(db) = database(&mut Bits::new(data, start, end)) else {
        return Vec::new();
    };
    // Each value entry: its key text, then its text values.
    let mut texts = strings.iter().skip(1 + db.named.len() + db.again + db.described.len()).cloned();
    let mut values: Vec<(String, Vec<(u32, PropertyValue)>)> = Vec::new();
    for (_, list) in &db.values {
        let Some(key) = texts.next() else { return Vec::new() };
        let mut entry = Vec::new();
        for (number, value) in list {
            match value.clone().or_else(|| texts.next().map(PropertyValue::Text)) {
                Some(v) => entry.push((*number, v)),
                None => return Vec::new(),
            }
        }
        values.push((key, entry));
    }
    let handle = |list: &[u32], offset: usize, number: u32| {
        list.iter().position(|n| *n == number).and_then(|i| handles.get(offset + i)).copied().unwrap_or(0)
    };
    db.records
        .iter()
        .map(|&(number, position)| CogoPoint {
            number,
            position,
            style: handle(&db.styled, 0, number),
            label_style: handle(&db.labelled, db.styled.len(), number),
            description: db
                .described
                .iter()
                .position(|n| *n == number)
                .and_then(|i| strings.get(1 + db.named.len() + db.again + i))
                .cloned(),
            name: db.named.iter().position(|n| *n == number).and_then(|i| strings.get(1 + i)).cloned(),
            properties: values
                .iter()
                .filter_map(|(key, list)| list.iter().find(|(n, _)| *n == number).map(|(_, v)| (key.clone(), v.clone())))
                .collect(),
        })
        .collect()
}

/// Point numbers a point group holds, in record order.
pub(super) fn group_numbers(data: &[u8], start: usize, end: usize) -> Option<Vec<u32>> {
    group_record(data, start, end).map(|g| g.numbers)
}

/// How a point group applies the point property classifications.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum PointGroupClassification {
    None,
    All,
    /// The classification of this name.
    Custom(String),
}

/// What a point group overrides in the points it holds.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PointGroupOverrides {
    /// The overriding point style (`AECC_POINT_STYLE`) when overridden.
    pub point_style: Option<u64>,
    /// The overriding point label style when overridden.
    pub label_style: Option<u64>,
    /// The fixed elevation when the elevation is overridden.
    pub elevation: Option<f64>,
    /// The raw description when it is overridden.
    pub raw_description: Option<String>,
    /// Whether the group's points are locked.
    pub locked: bool,
    pub classification: PointGroupClassification,
}

/// Classification word of a point group that applies none / all.
pub(super) const CLASSIFICATION_NONE: i32 = 87;
pub(super) const CLASSIFICATION_ALL: i32 = 0xEA7;

/// The fields of a point group record (see the module docs); the tail
/// fields only for the recent form.
pub(super) struct GroupRecord {
    pub flags: u8,
    pub run: u8,
    pub serial: Option<i32>,
    pub numbers: Vec<u32>,
    pub tail: Option<GroupTail>,
}

pub(super) struct GroupTail {
    pub extents: (Vector3, Vector3),
    pub elevation: f64,
    pub raw_tag: i32,
    pub classification: i32,
    /// Position of the bits after the classification word.
    pub rest: usize,
}

pub(super) fn group_record(data: &[u8], start: usize, end: usize) -> Option<GroupRecord> {
    let mut bits = Bits::new(data, start, end);
    (bits.bit_short()? == 0x04BE).then_some(())?;
    bits.bit_long()?;
    for _ in 0..2 {
        let n = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining() / 2)?;
        for _ in 0..n {
            bits.bit_long()?;
        }
    }
    // The settings' BL 0 is the 2-bit form; `a` is written with its byte.
    let mut probe = Bits::new(data, bits.pos, end);
    if probe.bits_value(3)? == 0b101 {
        for _ in 0..probe.bits_value(8)? {
            key(&mut probe)?;
        }
        bits = probe;
    }
    let flags = u8::try_from(bits.bit_long()?).ok()?;
    let run = bits.bits_value(5)?;
    let recent = flags & !3 == 0;
    let serial = if recent {
        bits.skip(6)?;
        Some(bits.bit_long()?)
    } else {
        None
    };
    let n = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 32 <= bits.remaining())?;
    let numbers = (0..n).map(|_| u32::try_from(bits.raw_long()?).ok()).collect::<Option<Vec<_>>>()?;
    let tail = recent
        .then(|| -> Option<GroupTail> {
            bits.skip(2)?;
            bits.bit_double()?;
            let mut v = || Some(Vector3::new(bits.bit_double()?, bits.bit_double()?, bits.bit_double()?));
            let extents = (v()?, v()?);
            Some(GroupTail { extents, elevation: bits.bit_double()?, raw_tag: bits.bit_long()?, classification: bits.bit_long()?, rest: bits.pos })
        })
        .flatten();
    Some(GroupRecord { flags, run, serial, numbers, tail })
}

/// The overrides of a recent point group record; `handles` and `strings`
/// are the record's.
pub(super) fn group_overrides(data: &[u8], start: usize, end: usize, handles: &[u64], strings: &[String]) -> Option<PointGroupOverrides> {
    let g = group_record(data, start, end)?;
    let t = g.tail?;
    // name, description, "AeccImpPG" ("", "AeccImpAllPointsPG" for _All Points)
    let own = strings.iter().position(|s| s == "AeccImpPG" || s == "AeccImpAllPointsPG")? + 1;
    let mut texts = strings.iter().skip(own).cloned();
    let raw_text = if t.raw_tag != 0 { Some(texts.next()?) } else { None };
    let classification = match t.classification {
        CLASSIFICATION_NONE => PointGroupClassification::None,
        CLASSIFICATION_ALL => PointGroupClassification::All,
        k if k as u32 & 0x8000_0000 != 0 => PointGroupClassification::Custom(texts.next()?),
        _ => return None,
    };
    Some(PointGroupOverrides {
        point_style: (g.run & 0b01000 != 0).then(|| handles.get(1).copied()).flatten(),
        label_style: (g.run & 0b10000 != 0).then(|| handles.first().copied()).flatten(),
        elevation: (g.flags & 2 != 0).then_some(t.elevation),
        raw_description: raw_text.filter(|_| g.flags & 1 != 0),
        locked: g.run & 1 != 0,
        classification,
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    fn bits_of(s: &str) -> Vec<u8> {
        let s: Vec<u8> = s.bytes().filter(|b| *b == b'0' || *b == b'1').collect();
        s.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | ((b - b'0') << (7 - i)))).collect()
    }

    fn bl(v: u32) -> String {
        match v {
            0 => "10".into(),
            1..=255 => format!("01{v:08b}"),
            v => format!("00{}", v.to_le_bytes().iter().map(|b| format!("{b:08b}")).collect::<String>()),
        }
    }

    fn bd(v: f64) -> String {
        format!("00{}", v.to_le_bytes().iter().map(|b| format!("{b:08b}")).collect::<String>())
    }

    #[test]
    fn point_database_maps_styles_and_descriptions_by_number() {
        // Two points; only point 7 has its own styles and a description.
        let bs_04be = "00 10111110 00000100";
        let bs_0420 = "00 00100000 00000100";
        let record = |p: [f64; 3], n: u32| format!("{} {}{}{} {} {bs_0420}", bl(1), bd(p[0]), bd(p[1]), bd(p[2]), bl(n));
        let s = [
            format!("{bs_04be} {}{}{} 010", bl(34), bl(0), bl(0)),
            format!("{} {} {}", bl(2), record([1.0, 2.0, 3.0], 9), record([100.0, 200.0, 10.0], 7)),
            format!("{}{} {}{}", bl(1), bl(7), bl(1), bl(7)),
            format!("{}{}{}", bl(0), bl(0), bl(0)),
            format!("{}{}{}", bl(1), bl(7), bl(0x8000_0001)),
            bl(0).repeat(13),
        ]
        .concat();
        let b = bits_of(&s);
        let len = s.bytes().filter(|c| *c == b'0' || *c == b'1').count();
        let points = points(&b, 0, len, &[0x68D, 0x690, 0x185], &["".into(), "TEST".into()]);
        assert_eq!(points.len(), 2);
        assert_eq!((points[0].number, points[0].style, points[0].label_style, points[0].description.as_deref()), (9, 0, 0, None));
        assert_eq!(points[1].position, Vector3::new(100.0, 200.0, 10.0));
        assert_eq!((points[1].style, points[1].label_style, points[1].description.as_deref()), (0x68D, 0x690, Some("TEST")));
    }

    #[test]
    fn group_overrides_read_flags_and_tail() {
        // id 0x21, no lists; flags 3 (raw description and elevation), run
        // label + locked, 6 bits, serial 2, no numbers; 2 bits, BD 0,
        // extents, elevation 5, raw description tag, classification all.
        let s = [
            format!("00 10111110 00000100 {} {}{} 01 00000011 10001 100000 {} {}", bl(0x21), bl(0), bl(0), bl(2), bl(0)),
            format!("00 {} {}{}{}{}{}{}", bd(0.0), bd(1.0), bd(1.0), bd(1.0), bd(2.0), bd(2.0), bd(2.0)),
            format!("{} {} {}", bd(5.0), bl(0x8004_0001), bl(0xEA7)),
        ]
        .concat();
        let b = bits_of(&s);
        let len = s.bytes().filter(|c| *c == b'0' || *c == b'1').count();
        let strings: Vec<String> = ["G", "", "AeccImpPG", "XY"].map(String::from).to_vec();
        let o = group_overrides(&b, 0, len, &[0x690, 0x68D], &strings).unwrap();
        assert_eq!((o.label_style, o.point_style, o.elevation, o.raw_description.as_deref(), o.locked), (Some(0x690), None, Some(5.0), Some("XY"), true));
        assert_eq!(o.classification, PointGroupClassification::All);
        assert_eq!(group_record(&b, 0, len).unwrap().serial, Some(2));
    }

    #[test]
    fn user_property_and_point_values_read_by_kind() {
        let bs_04be = "00 10111110 00000100";
        let key = 0x8200_0001;
        let s = [
            format!("{bs_04be} {}{}{} 010 {}{}{}", bl(key), bl(0), bl(0), bl(key), bd(1.5), bl(key)),
            format!("1 1 {}{} {}{}", bl(0x0200_0000), bd(0.0), bl(0x0240_0000), bd(10.0)),
            format!("{} 010", bl(0x4000_806B)),
        ]
        .concat();
        let b = bits_of(&s);
        let len = s.bytes().filter(|c| *c == b'0' || *c == b'1').count();
        let t: Vec<String> = ["kP1", "", "kP1", "kP1", "AeccAttributeTypeInfoDouble", "P1", "d", "C1"].map(String::from).to_vec();
        let p = user_property(&b, 0, len, &t).unwrap();
        assert_eq!((p.key.as_str(), p.name.as_str(), p.description.as_str(), p.classification.as_str()), ("kP1", "P1", "d", "C1"));
        assert_eq!(p.default, PropertyValue::Double(1.5));
        assert_eq!(p.bounds, Some(((PropertyValue::Double(0.0), false), (PropertyValue::Double(10.0), true))));
        // One point (5) holding a boolean (slot 5, one bit) and a text (slot 6).
        let record = format!("{} {} {}{}{}{} 00 00100000 00000100", bl(1), bl(1), bd(1.0), bd(2.0), bd(3.0), bl(5));
        let s = [
            format!("{bs_04be} {}{}{} 010 {record}", bl(34), bl(0), bl(0)),
            bl(0).repeat(6),
            bl(0).repeat(5),
            format!("{} {} {} {} 1", bl(1), bl(0xC300_0001), bl(1), bl(5)),
            format!("{} {} {} {}", bl(1), bl(0xC400_0001), bl(1), bl(5)),
            bl(0).repeat(6),
        ]
        .concat();
        let b = bits_of(&s);
        let len = s.bytes().filter(|c| *c == b'0' || *c == b'1').count();
        let points = points(&b, 0, len, &[0x185], &["".into(), "kB1".into(), "kS1".into(), "xy".into()]);
        assert_eq!(points[0].properties, vec![("kB1".into(), PropertyValue::Boolean(true)), ("kS1".into(), PropertyValue::Text("xy".into()))]);
    }

    #[test]
    fn choice_properties_read_their_choices() {
        let key = 0x8D00_0001;
        let s = format!("00 10111110 00000100 {}{}{} 010 {}{}{} 1 0 {} {} 010", bl(key), bl(0), bl(0), bl(key), bl(1), bl(key), bl(3), bl(0x4000_806B));
        let b = bits_of(&s);
        let len = s.bytes().filter(|c| *c == b'0' || *c == b'1').count();
        let t: Vec<String> = ["kE1", "", "kE1", "kE1", "AeccAttributeTypeInfoUserDefinedEnum", "A", "B", "C", "E1", "", "C1"].map(String::from).to_vec();
        let p = user_property(&b, 0, len, &t).unwrap();
        assert_eq!((p.default, p.choices, p.name.as_str(), p.bounds), (PropertyValue::Choice(1), vec!["A".to_string(), "B".into(), "C".into()], "E1", None));
        // Choices share the whole numbers' slot.
        assert_eq!(PropertyValue::slot(13), PropertyValue::slot(1));
    }

    #[test]
    fn group_numbers_follow_the_serial() {
        // Head without lists or settings, a = 0 (byte form), 5 + 6 bits,
        // serial 1, two numbers: 5 and 7.
        let s = "00 10111110 00000100 10 10 10 01 00000000 00000 100000 0100000001 0100000010
                 00000101 00000000 00000000 00000000 00000111 00000000 00000000 00000000";
        let b = bits_of(s);
        let len = s.bytes().filter(|c| *c == b'0' || *c == b'1').count();
        assert_eq!(group_numbers(&b, 0, len), Some(vec![5, 7]));
    }
}
