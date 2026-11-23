//! Grading criteria (`AECC_GRADING_CRITERIA`): the target and the
//! projection a grading uses, kept as keys after a style-like head:
//!
//! ```text
//! BS 0x04BE, BL id, BL n, n x BL, BL m, m x BL, BL 0, B 1,
//! RC count, count x key, BL 32, ...
//! ```
//!
//! Grading entities (`AECC_GRADING`) carry no style, criteria, or site
//! assignment of their own: their class stream opens with a constant
//! schema head and their body holds references, not assignments — see
//! [`GradingEntity`].
//!
//! A key is the class key of [`super::alignment::read_key`] with two flag
//! bits: 0x00400000 locks the value, and with 0x40000000 the key carries no
//! value (a locked value the criteria does not override). A value the
//! criteria does not override has no key at all; the keys come in no fixed
//! order. The first id list holds the ids of the criteria derived from this
//! one. The references run: the two lists' entries, then one per key that
//! takes a reference (types 0x0F, 0x10, 0x12), then one more. Derived
//! criteria store the target and the projection keys with the lock bit set.
//!
//! Key ids (low 16 bits):
//!
//! ```text
//! 0x3001 target         0x3020 elevation projection  0x3060 interior corner overlap
//! 0x3002 elevation      0x3021 relative elevation p. 0x3090 search order
//! 0x3003 relative elev. 0x3022 distance projection   0x3100 / 0x3102 cut slope format / value
//! 0x3004 distance       0x3023 surface projection    0x3110 / 0x3112 fill slope format / value
//! 0x3140 projection relative elevation               0x3120 / 0x3122 slope format / value
//! 0x3150 projection distance                         0x101E / 0x101F created / modified (FILETIME)
//! 0x3008 target surface (a reference)
//! ```
//!
//! Grading groups (`AECC_GRADING_GROUP`) come in two forms; see
//! [`GradingGroupRecord`].

use super::alignment::{read_key, KeyValue};
use super::bits::Bits;

const LOCKED: u32 = 0x0040_0000;
const NO_VALUE: u32 = 0x4000_0000;

/// One criteria value: the value the criteria stores (`None` when it takes
/// the default) and whether it is locked.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CriteriaValue<T> {
    pub value: Option<T>,
    pub locked: bool,
}

/// What a grading projects to and how.
///
/// Enumerations as stored: target 0 surface, 1 elevation, 2 relative
/// elevation, 3 distance; slope format 0 slope, 1 grade; elevation and
/// surface projection 0 cut/fill slope, 1 cut slope, 2 fill slope; distance
/// projection 3 slope, 5 relative elevation; relative elevation projection
/// 3 slope, 6 distance; interior corner overlap 0 average slope, 1 hold
/// slope as minimum, 2 as maximum; search order 0 cut first, 1 fill first.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GradingCriteria {
    pub target: CriteriaValue<i32>,
    pub elevation: CriteriaValue<f64>,
    pub relative_elevation: CriteriaValue<f64>,
    pub distance: CriteriaValue<f64>,
    pub projection_distance: CriteriaValue<f64>,
    pub projection_relative_elevation: CriteriaValue<f64>,
    pub elevation_projection: CriteriaValue<i32>,
    pub relative_elevation_projection: CriteriaValue<i32>,
    pub distance_projection: CriteriaValue<i32>,
    pub surface_projection: CriteriaValue<i32>,
    pub slope: CriteriaValue<f64>,
    pub slope_format: CriteriaValue<i32>,
    pub cut_slope: CriteriaValue<f64>,
    pub cut_slope_format: CriteriaValue<i32>,
    pub fill_slope: CriteriaValue<f64>,
    pub fill_slope_format: CriteriaValue<i32>,
    pub interior_corner_overlap: CriteriaValue<i32>,
    pub search_order: CriteriaValue<i32>,
    /// FILETIME stamps.
    pub created: Option<u64>,
    pub modified: Option<u64>,
    /// Ids of the criteria derived from this one.
    pub derived: Vec<i32>,
    /// Handles of the criteria derived from this one.
    pub derived_criteria: Vec<u64>,
    /// Handle of the surface a criteria targeting a surface grades to.
    pub target_surface: Option<u64>,
}

// ponytail: keys 0x31x1 (a copy of each slope), 0x31x3, 0x3024, 0x3130,
// 0x1008 and 0x1010 are read past; no criteria property changed them.
pub(super) fn criteria(data: &[u8], start: usize, end: usize, handles: &[u64]) -> Option<GradingCriteria> {
    let mut bits = Bits::new(data, start, end);
    (bits.bit_short()? == 0x04BE).then_some(())?;
    bits.bit_long()?;
    let list = |bits: &mut Bits| {
        let n = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining() / 2)?;
        (0..n).map(|_| bits.bit_long()).collect::<Option<Vec<_>>>()
    };
    let derived = list(&mut bits)?;
    // References: the two lists' entries, then one per key taking one.
    let mut refs = derived.len() + list(&mut bits)?.len();
    (bits.bit_long()? == 0 && bits.bit()?).then_some(())?;
    let derived_criteria = handles.iter().take(derived.len()).copied().collect();
    let mut out = GradingCriteria { derived, derived_criteria, ..Default::default() };
    for _ in 0..bits.bits_value(8)? {
        let at = bits.pos;
        let key = bits.bit_long()? as u32;
        let value = if key & NO_VALUE != 0 {
            KeyValue::None
        } else {
            bits.pos = at;
            read_key(&mut bits)?.1
        };
        let locked = key & LOCKED != 0;
        let takes_reference = matches!((key >> 24) & 0x3F, 0x0F | 0x10 | 0x12);
        let f = CriteriaValue { value: if let KeyValue::Double(d) = value { Some(d) } else { None }, locked };
        let i = CriteriaValue { value: if let KeyValue::Long(l) = value { Some(l) } else { None }, locked };
        let stamp = if let KeyValue::Raw(r) = value { Some(r) } else { None };
        match key & 0xFFFF {
            0x3001 => out.target = i,
            0x3002 => out.elevation = f,
            0x3003 => out.relative_elevation = f,
            0x3004 => out.distance = f,
            0x3150 => out.projection_distance = f,
            0x3140 => out.projection_relative_elevation = f,
            0x3020 => out.elevation_projection = i,
            0x3021 => out.relative_elevation_projection = i,
            0x3022 => out.distance_projection = i,
            0x3023 => out.surface_projection = i,
            0x3122 => out.slope = f,
            0x3120 => out.slope_format = i,
            0x3102 => out.cut_slope = f,
            0x3100 => out.cut_slope_format = i,
            0x3112 => out.fill_slope = f,
            0x3110 => out.fill_slope_format = i,
            0x3060 => out.interior_corner_overlap = i,
            0x3090 => out.search_order = i,
            0x101E => out.created = stamp,
            0x101F => out.modified = stamp,
            0x3008 if takes_reference => out.target_surface = handles.get(refs).copied().filter(|h| *h != 0),
            _ => {}
        }
        refs += usize::from(takes_reference);
    }
    Some(out)
}

/// The references a grading entity (`AECC_GRADING`) record carries.
///
/// A grading's style and criteria assignment live with its group, not
/// here: Grading-3A.dwg holds the Ditch and the Cut Slope Grading styles
/// under byte-identical heads and references. `AECC_GRADING` also stores
/// no geometry — the record's graphic-image section draws the grading
/// solution.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GradingEntity {
    /// Handle of the `AECC_PARCEL` the grading grades from. In a drawing
    /// with two parcels each grading points at its own parcel; drawings
    /// with a single parcel point at it from every grading.
    pub parcel: u64,
    /// Handle of the `AECC_LABEL_COLLECTOR_STYLE` the record hard-points
    /// at — in the samples the label style of the parcels of the site
    /// the parcel belongs to (its name strings style `AeccDbFace`),
    /// constant per drawing.
    pub label_style: u64,
}

/// Reads a grading entity record: `data`/`start`/`end` is its class
/// stream, `handles` its class-body references in stream order.
///
/// The class stream opens with a constant schema head, identical in all
/// 26 observed records (15 Sample drawings and Grading-3A.dwg): the six
/// words below, the lot lines' schema word (see
/// [`super::parcel_segments::SCHEMA`]), and three more words. The two
/// references follow the common ones; everything after them (an ordinal
/// of the grading within its group and byte runs that match no oracle
/// quantity) is unmapped and read past.
pub(super) fn entity(data: &[u8], start: usize, end: usize, handles: &[u64]) -> Option<GradingEntity> {
    let mut bits = Bits::new(data, start, end);
    for word in [80u16, 256, 0xC76A, 80, 80, 0] {
        (bits.bit_short()? as u16 == word).then_some(())?;
    }
    (bits.bit_long()? as u32 == super::parcel_segments::SCHEMA).then_some(())?;
    for word in [80u16, 0x1540, 0xC1C0] {
        (bits.bit_short()? as u16 == word).then_some(())?;
    }
    let parcel = handles.get(1).copied().filter(|handle| *handle != 0)?;
    let label_style = handles.get(4).copied().filter(|handle| *handle != 0)?;
    Some(GradingEntity { parcel, label_style })
}

/// The two forms of a grading group record (`AECC_GRADING_GROUP`), after
/// the shared `BS 0x04BE` head and the head `BL` that tells the forms
/// apart. Both forms hold the body's references in the handle stream and
/// their text in the text stream, in stream order.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GradingGroupRecord {
    /// A site's root group: it lists the named groups created under the
    /// site (its default group, named `Grading Group` by the
    /// application though the record's own text holds no name). Its
    /// key in the application's tree is 259 in every observed drawing.
    Tree(GradingGroupTree),
    /// A named group of gradings.
    Group(GradingGroup),
}

/// A site's root group: the named grading groups under the site, in the
/// order of the application's tree.
///
/// ```text
/// BS 0x04BE, BL 259 (the key), BL n, n x BL (the members' name tags,
/// each 0x80000000 | tag), BL 0, ... (unmapped)
/// ```
///
/// The text stream holds the root's own (empty) name and then one name
/// per member, in the order of the tags. The handle stream holds one
/// member group per tag, then a null terminator. A site without named
/// groups stores the empty record.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GradingGroupTree {
    /// Names of the member groups, in tag order.
    pub names: Vec<String>,
    /// Handles of the member groups (`AECC_GRADING_GROUP` records of the
    /// [`Group`](GradingGroupRecord::Group) form), in tag order.
    pub members: Vec<u64>,
}

/// A named group of gradings.
///
/// ```text
/// BS 0x04BE, BL 0x80000000 | name tag, ... (unmapped settings the
/// application recomputes, in two layouts: a short one without and a
/// long one with two constant words 0x00202010, 0x00203e20)
/// ```
///
/// The record carries no style, criteria, or member data in the body:
/// what it maps is its name texts and the body's references, which run
/// the graded-from parcel, the member gradings, and then the group's
/// automatic surface and the drawing's volume base surface when the
/// group has them (a group without members stores no parcel and no
/// references at all). A grading's style is read through its group.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GradingGroup {
    /// The group's name-tag id: the low 31 bits of the head `BL`, the
    /// tag the root group's member list addresses the name by.
    pub name_tag: u32,
    /// Name of the group (`Building Pad`, `Ditch Drainage AB`, ...),
    /// the record's first text.
    pub name: String,
    /// The user that created the group, when the record carries it
    /// (the texts then run name, empty, creator).
    pub creator: Option<String>,
    /// Handle of the `AECC_PARCEL` the group grades from — the parcel
    /// its member gradings reference (the grading entity record's first
    /// hard pointer; the entity carries no group of its own), `None`
    /// for a group without members.
    pub parcel: Option<u64>,
    /// The references after the parcel: the member gradings, then the
    /// group's automatic surface and the volume base surface when
    /// present. The body stores no count separating the members from
    /// the surfaces, so the split is by class at the document level.
    pub references: Vec<u64>,
}

/// Reads a grading group record: `data`/`start`/`end` is its class
/// stream, `strings` its text stream, `handles` its class-body
/// references in stream order.
pub(super) fn group_record(data: &[u8], start: usize, end: usize, strings: &[String], handles: &[u64]) -> Option<GradingGroupRecord> {
    let mut bits = Bits::new(data, start, end);
    (bits.bit_short()? == 0x04BE).then_some(())?;
    let head = bits.bit_long()? as u32;
    match head {
        259 => tree(&mut bits, strings, handles).map(GradingGroupRecord::Tree),
        head if head & 0x8000_0000 != 0 => {
            group(head & 0x7FFF_FFFF, strings, handles).map(GradingGroupRecord::Group)
        }
        _ => None,
    }
}

fn tree(bits: &mut Bits, strings: &[String], handles: &[u64]) -> Option<GradingGroupTree> {
    let n = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining() / 2)?;
    for _ in 0..n {
        (bits.bit_long()? as u32 & 0x8000_0000 != 0).then_some(())?;
    }
    (bits.bit_long()? == 0).then_some(())?;
    (strings.first()?.is_empty()).then_some(())?;
    let names: Vec<String> = strings.iter().skip(1).take(n).cloned().collect();
    (names.len() == n).then_some(())?;
    (handles.last()? == &0).then_some(())?;
    let members = &handles[..handles.len() - 1];
    (members.len() == n && members.iter().all(|h| *h != 0)).then_some(())?;
    Some(GradingGroupTree { names, members: members.to_vec() })
}

fn group(name_tag: u32, strings: &[String], handles: &[u64]) -> Option<GradingGroup> {
    let name = strings.first()?.clone();
    (strings.get(1).map(String::is_empty).unwrap_or(true)).then_some(())?;
    let creator = (strings.len() >= 3).then(|| strings[2].clone());
    let parcel = handles.first().copied().filter(|h| *h != 0);
    let references = handles.iter().skip(1).copied().filter(|h| *h != 0).collect();
    Some(GradingGroup { name_tag, name, creator, parcel, references })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits_of(s: &str) -> Vec<u8> {
        let s: Vec<u8> = s.bytes().filter(|b| *b == b'0' || *b == b'1').collect();
        s.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | ((b - b'0') << (7 - i)))).collect()
    }

    /// The grading entity records' schema head as bits, MSB first: the
    /// six leading words, the lot lines' schema word, three more words.
    fn entity_head() -> String {
        let mut bits = String::new();
        let bs = |v: u16, bits: &mut String| match v {
            0 => bits.push_str("10"),
            256 => bits.push_str("11"),
            v if v < 256 => bits.push_str(&format!("01{v:08b}")),
            v => {
                bits.push_str("00");
                for b in v.to_le_bytes() {
                    bits.push_str(&format!("{b:08b}"));
                }
            }
        };
        for v in [80u16, 256, 0xC76A, 80, 80, 0] {
            bs(v, &mut bits);
        }
        bits.push_str("00");
        for b in super::super::parcel_segments::SCHEMA.to_le_bytes() {
            bits.push_str(&format!("{b:08b}"));
        }
        for v in [80u16, 0x1540, 0xC1C0] {
            bs(v, &mut bits);
        }
        bits
    }

    #[test]
    fn reads_values_locks_and_valueless_keys() {
        // BS 0x04BE, BL 0, no lists, BL 0, B 1, RC 3:
        // 0x02033004 BD 1.0 (distance), 0x0D433001 BL 3 (target, locked),
        // 0x4D433120 (slope format, locked, no value).
        let b = bits_of(
            "00 10111110 00000100 10 10 10 10 1 00000011
             00 00000100 00110000 00000011 00000010 01
             00 00000001 00110000 01000011 00001101 0100000011
             00 00100000 00110001 01000011 01001101 0000000",
        );
        let read = criteria(&b, 0, b.len() * 8, &[]).unwrap();
        assert_eq!(read.distance, CriteriaValue { value: Some(1.0), locked: false });
        assert_eq!(read.target, CriteriaValue { value: Some(3), locked: true });
        assert_eq!(read.slope_format, CriteriaValue { value: None, locked: true });
        assert_eq!(read.slope, CriteriaValue::default());
    }

    #[test]
    fn reads_a_grading_entitys_references() {
        // The head, then an unmapped tail; references as the handle
        // stream holds them: null, parcel, null, null, style, null.
        let mut bits = entity_head();
        bits.push_str("00000000"); // the unmapped body the samples carry
        let data: Vec<u8> = bits
            .as_bytes()
            .chunks(8)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap())
            .collect();
        let read = entity(&data, 0, data.len() * 8, &[0, 0xc68, 0, 0, 0x63d, 0]).unwrap();
        assert_eq!(read.parcel, 0xc68);
        assert_eq!(read.label_style, 0x63d);
    }

    #[test]
    fn another_schema_word_is_not_a_grading() {
        let mut bits = entity_head();
        // flip the schema word's first bit: 0x10025F04 becomes 0x10025F84
        assert_eq!(&bits[54..55], "0");
        bits.replace_range(54..55, "1");
        let data: Vec<u8> = bits
            .as_bytes()
            .chunks(8)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap())
            .collect();
        assert_eq!(entity(&data, 0, data.len() * 8, &[0, 0xc68, 0, 0, 0x63d]), None);
    }

    #[test]
    fn a_grading_without_its_references_decodes_to_none() {
        let bits = entity_head();
        let data: Vec<u8> = bits
            .as_bytes()
            .chunks(8)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap())
            .collect();
        assert_eq!(entity(&data, 0, data.len() * 8, &[]), None);
        assert_eq!(entity(&data, 0, data.len() * 8, &[0, 0, 0, 0, 0]), None);
    }

    /// The head `BL` as bits: code `00` and the value little-endian.
    fn bl(value: u32) -> String {
        let mut bits = String::from("00");
        for b in value.to_le_bytes() {
            bits.push_str(&format!("{b:08b}"));
        }
        bits
    }

    /// The short-form head `BL`: code `01` and the value as one byte.
    fn bl_byte(value: u8) -> String {
        format!("01{value:08b}")
    }

    fn group_bits(head: u32, rest: &str) -> Vec<u8> {
        bits_of(&format!("00 10111110 00000100 {} {rest}", bl(head)))
    }

    #[test]
    fn reads_a_sites_root_group() {
        // Grading-3A.dwg: the root group's two members.
        let b = group_bits(259, &format!("{} {} {} {} 0000000000", bl_byte(2), bl(0x80000151), bl(0x80000162), bl(0)));
        let strings = ["", "Ditch Drainage AB", "Ditch Drainage BC"].map(String::from);
        let read = group_record(&b, 0, b.len() * 8, &strings, &[0x13bf, 0x13c6, 0]).unwrap();
        let GradingGroupRecord::Tree(tree) = read else { panic!("not a tree") };
        assert_eq!(tree.names, ["Ditch Drainage AB", "Ditch Drainage BC"]);
        assert_eq!(tree.members, [0x13bf, 0x13c6]);
    }

    #[test]
    fn reads_an_empty_root_group() {
        // Grading-1.dwg: a site without named groups; no members, one
        // null terminator, one (empty) name.
        let b = group_bits(259, "10 10 0000000000");
        let read = group_record(&b, 0, b.len() * 8, &[String::from("")], &[0]).unwrap();
        let GradingGroupRecord::Tree(tree) = read else { panic!("not a tree") };
        assert_eq!(tree.names, Vec::<String>::new());
        assert_eq!(tree.members, Vec::<u64>::new());
    }

    #[test]
    fn reads_a_named_group_with_its_creator() {
        // Grading-4.dwg: Building Pad; parcel, one member grading, the
        // group's automatic surface, the volume base surface.
        let b = group_bits(0x8000015B, "10 10 10 10 00000000");
        let strings = ["Building Pad", "", "user"].map(String::from);
        let read = group_record(&b, 0, b.len() * 8, &strings, &[0xbb3, 0xc8e, 0x10c1, 0xbb7]).unwrap();
        let GradingGroupRecord::Group(group) = read else { panic!("not a group") };
        assert_eq!(group.name, "Building Pad");
        assert_eq!(group.name_tag, 0x15B);
        assert_eq!(group.creator.as_deref(), Some("user"));
        assert_eq!(group.parcel, Some(0xbb3));
        assert_eq!(group.references, [0xc8e, 0x10c1, 0xbb7]);
    }

    #[test]
    fn reads_a_named_group_without_its_creator() {
        // Grading-3A.dwg: the short layout's record stores no creator.
        let b = group_bits(0x80000151, "01 00100000 01 01100000 00000");
        let read = group_record(&b, 0, b.len() * 8, &["Ditch Drainage AB".to_string(), "".to_string()], &[0xc68, 0x13da, 0x13ea, 0x13c0, 0xbb7]).unwrap();
        let GradingGroupRecord::Group(group) = read else { panic!("not a group") };
        assert_eq!(group.name, "Ditch Drainage AB");
        assert_eq!(group.creator, None);
        assert_eq!(group.parcel, Some(0xc68));
        assert_eq!(group.references, [0x13da, 0x13ea, 0x13c0, 0xbb7]);
    }

    #[test]
    fn reads_a_group_without_members() {
        // Grading-6.dwg: Grading Group 1, created empty.
        let b = group_bits(0x80000491, "10 10 10 10 00000000");
        let read = group_record(&b, 0, b.len() * 8, &["Grading Group 1".to_string(), "".to_string(), "user".to_string()], &[0]).unwrap();
        let GradingGroupRecord::Group(group) = read else { panic!("not a group") };
        assert_eq!(group.name, "Grading Group 1");
        assert_eq!(group.creator.as_deref(), Some("user"));
        assert_eq!(group.parcel, None);
        assert_eq!(group.references, Vec::<u64>::new());
    }

    #[test]
    fn a_root_group_member_list_needs_its_terminator_and_names() {
        let strings = ["", "Ditch Drainage AB"].map(String::from);
        // no null terminator after the one member
        let b = group_bits(259, &format!("{} {} {} 0000000000", bl_byte(1), bl(0x80000151), bl(0)));
        assert_eq!(group_record(&b, 0, b.len() * 8, &strings, &[0x13bf]), None);
        // a member without a name text
        let b = group_bits(259, &format!("{} {} {} {} 0000000000", bl_byte(2), bl(0x80000151), bl(0x80000162), bl(0)));
        assert_eq!(group_record(&b, 0, b.len() * 8, &strings, &[0x13bf, 0x13c6, 0]), None);
        // a member name tag without the tag bit
        let b = group_bits(259, &format!("{} {} {} 0000000000", bl_byte(1), bl(0x151), bl(0)));
        assert_eq!(group_record(&b, 0, b.len() * 8, &strings, &[0x13bf, 0]), None);
    }

    #[test]
    fn another_head_is_not_a_grading_group() {
        let b = group_bits(0x123, "00000000");
        assert_eq!(group_record(&b, 0, b.len() * 8, &[String::from("")], &[0]), None);
    }
}
