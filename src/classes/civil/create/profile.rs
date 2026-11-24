//! Layout profiles of fixed tangents with free symmetric parabolas
//! (`AECC_PROFILE`), on an alignment this crate writes.

use super::alignment::{rewritable, rewrite, ProfileRef};
use super::support::*;
use crate::entities::{EntityCommon, EntityType, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::{Handle, Vector2};
use crate::CadDocument;

use super::tree::{ensure_profile_tree, KEY_PROFILE_STYLES};
/// Bits of a profile record after its identifier.
const TAIL: usize = 107;
/// The 107 bits after a profile's identifier, the same in every sample
/// (WR01 to WR09), ending with the version word: `PRE`, `BD 0`, `MID`,
/// four empty lists (`BL 0`), `END` (an offset profile fills them, see
/// [`OffsetTail`]).
const TAIL_BITS: &str = "10001111111100001111000000000000000010100001000000010100000001101010100000000110001010100001011111000000100";
// ponytail: the constant runs are not split into fields; name them once a
// profile varies them.
const PRE: &str = "10001111111100001111000000000000000010";
const MID: &str = "0001000000010100000001";
const END: &str = "0000000110001010100001011111000000100";
/// The text of a curb return profile's class key 0x0400FF80 (WC61).
const CURB_RETURN_CURVE_TYPE: &str = "{\r\n\t\"ProfileCurveType\": 0\r\n}";
/// A curb return profile's run in place of `END` (constant in WC60, WC61).
const CURB_RETURN_END: &str = "00000001101010000001001000000011010000001001000000101010100001011111000000100";

/// The helper records a profile owns, in the order of its references (and
/// of the class keys that refer to them).
const HELPERS: [&str; 7] = [
    "AECC_DB_FLOAT_TANGENT_GRADELEN",
    "AECC_DB_FIXED_PARABOLA_HLPT",
    "AECC_DB_FIXED_PARABOLA_HLPTPATSPEAK",
    "AECC_ENTITYCIRCLEINFOS",
    "AECC_ENTITYEXTENDPARAM",
    "AECC_DB_FIXED_ARC_HLPT",
    "AECC_NOTIFICATIONINFOS",
];

/// A class key of the profile record and what it holds.
enum Key {
    /// Type 4: a text (in the text stream).
    Text(&'static str),
    /// Type 0x12: the next helper record (a hard ownership).
    Helper,
    /// Type 3.
    Byte(u8),
    /// The geometry stamp (type 0x0B), written as 0.
    Stamp,
}

/// The profile record's class keys in the application's order (WR01).
const KEYS: [(u32, Key); 24] = [
    (0x0400_FE0B, Key::Text("A")),
    (0x1200_FF73, Key::Helper),
    (0x0400_FE08, Key::Text("HP")),
    (0x1200_FF70, Key::Helper),
    (0x0400_FE16, Key::Text("Overall LP")),
    (0x0400_FE05, Key::Text("EVC")),
    (0x0400_FE02, Key::Text("BVC")),
    (0x0300_FB05, Key::Byte(0)),
    (0x0400_FE0A, Key::Text("K")),
    (0x1200_FF72, Key::Helper),
    (0x0B00_1081, Key::Stamp),
    (0x0400_FE07, Key::Text("EVCE")),
    (0x0400_FE15, Key::Text("Overall HP")),
    (0x0400_FE04, Key::Text("BVCE")),
    (0x0400_FE01, Key::Text("BREAK")),
    (0x0300_FF00, Key::Byte(1)),
    (0x1200_FF58, Key::Helper),
    (0x1200_FF52, Key::Helper),
    (0x0400_FE09, Key::Text("LP")),
    (0x1200_FF71, Key::Helper),
    (0x0400_FE06, Key::Text("EVCS")),
    (0x0400_FE03, Key::Text("BVCS")),
    (0x0400_FE00, Key::Text("PVI")),
    (0x1200_FF57, Key::Helper),
];

/// The drawing's default profile style: the first style of the style
/// tree's profile style collection.
fn default_style(doc: &CadDocument) -> Option<u64> {
    let tree = read_node(doc, root_node(doc, 31)?)?;
    let head = tree.children.iter().find(|c| c.0 == KEY_PROFILE_STYLES)?.1;
    read_node(doc, Handle::new(head))?.children.first().map(|c| c.1).filter(|h| *h != 0)
}

/// The identifier a profile record stores after its entities, a
/// version-4 GUID (the alignment lists its profiles by it): before its
/// last 107 bits (`TAIL_BITS`) or a curb return profile's constant tail,
/// or, after a longer tail (an offset profile's), where the reader finds
/// it.
pub(super) fn identifier(doc: &CadDocument, profile: u64) -> Option<[u8; 16]> {
    let EntityType::Extended(x) = doc.get_entity(Handle::new(profile))? else { return None };
    let ExtendedEntityData::Arch(a) = &x.data else { return None };
    (a.dxf_name == "AECC_PROFILE").then_some(())?;
    let (data, start, end) = a.main_stream()?;
    let bit = |p: usize| data[p / 8] >> (7 - p % 8) & 1;
    let ends = |tail: &str| end >= start + tail.len() && tail.bytes().enumerate().all(|(i, c)| bit(end - tail.len() + i) == c - b'0');
    let curb_return = format!("{PRE}10{MID}10101010{CURB_RETURN_END}");
    let Some(tail) = [TAIL_BITS, curb_return.as_str()].into_iter().find(|t| ends(t)).map(str::len) else {
        return super::super::profile::after_entities(data, start, end).map(|(id, _)| id);
    };
    let at = end.checked_sub(tail + 128).filter(|at| *at >= start)?;
    let mut id = [0u8; 16];
    for (i, byte) in id.iter_mut().enumerate() {
        for k in 0..8 {
            let p = at + 8 * i + k;
            *byte = (*byte << 1) | (data[p / 8] >> (7 - p % 8) & 1);
        }
    }
    Some(id)
}

/// A new random version-4 GUID (in the byte order the records store:
/// the version in the high nibble of byte 7, the variant in byte 8). The
/// application stops while it opens a drawing whose profile identifiers
/// are not of this form.
pub(super) fn new_identifier(seed: u64) -> [u8; 16] {
    use std::hash::{BuildHasher, Hasher};
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let half = |salt: u64| {
        let mut h = std::collections::hash_map::RandomState::new().build_hasher();
        h.write_u128(now);
        h.write_u64(seed ^ salt);
        h.finish().to_le_bytes()
    };
    let mut id = [0u8; 16];
    id[..8].copy_from_slice(&half(1));
    id[8..].copy_from_slice(&half(2));
    id[7] = (id[7] & 0x0F) | 0x40;
    id[8] = (id[8] & 0x3F) | 0x80;
    id
}

/// The vertical curve at an inner PVI of a layout profile.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VerticalCurve {
    /// The tangents meet at the PVI.
    None,
    /// A free symmetric parabola of this length (crest where the grade
    /// falls).
    Parabola(f64),
    /// A free asymmetric parabola of these lengths before and after the
    /// PVI.
    Asymmetric { before: f64, after: f64 },
    /// A free circular curve of this radius.
    Circular(f64),
    /// A free symmetric parabola of this K value (length per percent of
    /// grade change), of this radius, or through this point (station,
    /// elevation).
    ParabolaK(f64),
    ParabolaRadius(f64),
    ParabolaThrough(Vector2),
    /// A free circular curve of this horizontal length, or through this
    /// point.
    CircularLength(f64),
    CircularThrough(Vector2),
}

impl VerticalCurve {
    fn is_some(self) -> bool {
        self != VerticalCurve::None
    }

    /// The stations it takes before and after its PVI `pvi` between the
    /// grades `g1` and `g2`, `None` when its values are not positive or no
    /// such curve passes its point.
    fn reach(self, pvi: Vector2, g1: f64, g2: f64) -> Option<(f64, f64)> {
        let positive = |v: f64| (v.is_finite() && v > 0.0).then_some(v);
        let parabola = |l: f64| positive(l).map(|l| (l / 2.0, l / 2.0));
        let circular = |r: f64| {
            let (a1, a2) = (g1.atan(), g2.atan());
            let t = positive(r)? * ((a2 - a1).abs() / 2.0).tan();
            Some((t * a1.cos(), t * a2.cos()))
        };
        // The unit grades' vertical parts: a circular curve's horizontal
        // length is its radius times their difference.
        let rise = |g: f64| g / (1.0 + g * g).sqrt();
        match self {
            VerticalCurve::None => Some((0.0, 0.0)),
            VerticalCurve::Parabola(l) => parabola(l),
            VerticalCurve::ParabolaK(k) => parabola(positive(k)? * 100.0 * (g2 - g1).abs()),
            VerticalCurve::ParabolaRadius(r) => parabola(positive(r)? * (g2 - g1).abs()),
            VerticalCurve::ParabolaThrough(p) => parabola(super::super::profile::parabola_length_through(pvi, g1, g2, p)?),
            VerticalCurve::Asymmetric { before, after } => Some((positive(before)?, positive(after)?)),
            VerticalCurve::Circular(r) => circular(r),
            VerticalCurve::CircularLength(l) => circular(positive(l)? / (rise(g1) - rise(g2)).abs()),
            VerticalCurve::CircularThrough(p) => circular(super::super::profile::circle_through(pvi, g1, g2, p)?),
        }
    }

    /// Whether the record names it a parabola (one more text each).
    fn parabola(self) -> bool {
        matches!(self, VerticalCurve::Parabola(_) | VerticalCurve::ParabolaK(_) | VerticalCurve::ParabolaRadius(_) | VerticalCurve::ParabolaThrough(_))
    }
}

/// One entity of the profile: a tangent from PVI to PVI, or a vertical
/// curve at an inner PVI.
enum Part {
    Tangent(Vector2, Vector2),
    Curve { curve: VerticalCurve, crest: bool },
    /// A fixed or floating parabola (an entity of its own).
    Fixed(ProfileEntity),
}

/// One entity of a profile laid out entity by entity (station, elevation
/// points; grades as ratios).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProfileEntity {
    /// A fixed tangent.
    Tangent { start: Vector2, end: Vector2 },
    /// A fixed symmetric parabola through three points (start, middle, end).
    ParabolaThreePoints { start: Vector2, middle: Vector2, end: Vector2 },
    /// A fixed circular curve through three points (start, one on it, end).
    CircularThreePoints { start: Vector2, on: Vector2, end: Vector2 },
    /// A fixed symmetric parabola between two points with its grade at the
    /// start (or at the end).
    ParabolaTwoPointsGrade { start: Vector2, end: Vector2, grade: f64, at_end: bool },
    /// A fixed symmetric parabola between two points with a K value.
    ParabolaTwoPointsK { start: Vector2, end: Vector2, k: f64, crest: bool },
    /// A fixed symmetric parabola between two points with a radius.
    ParabolaTwoPointsRadius { start: Vector2, end: Vector2, radius: f64, crest: bool },
    /// A floating symmetric parabola that leaves the entity before it and
    /// ends at a point, with a K value or its grade at the point.
    FloatingParabolaK { end: Vector2, k: f64, crest: bool },
    FloatingParabolaGrade { end: Vector2, grade: f64 },
    /// A floating tangent that leaves the parabola before it towards a
    /// point.
    FloatingTangent { through: Vector2 },
    /// A free tangent touching the parabolas before and after it.
    FreeTangent,
}

impl ProfileEntity {
    /// Its prefix and constraint codes.
    fn codes(self) -> (u32, i32) {
        match self {
            ProfileEntity::Tangent { .. } => (2, 0),
            ProfileEntity::ParabolaThreePoints { .. } => (6, 11),
            ProfileEntity::CircularThreePoints { .. } => (4, 11),
            ProfileEntity::ParabolaTwoPointsGrade { .. } => (6, 12),
            ProfileEntity::ParabolaTwoPointsK { .. } | ProfileEntity::ParabolaTwoPointsRadius { .. } => (6, 13),
            ProfileEntity::FloatingParabolaK { .. } => (6, 32),
            ProfileEntity::FloatingParabolaGrade { .. } => (6, 34),
            ProfileEntity::FloatingTangent { .. } => (2, 4),
            ProfileEntity::FreeTangent => (2, 9),
        }
    }

    fn valid(self) -> bool {
        let p = |v: Vector2| v.x.is_finite() && v.y.is_finite();
        let k = |v: f64| v.is_finite() && v > 0.0;
        match self {
            ProfileEntity::Tangent { start, end } => p(start) && p(end) && start.x < end.x,
            ProfileEntity::ParabolaThreePoints { start, middle, end } | ProfileEntity::CircularThreePoints { start, on: middle, end } => p(start) && p(middle) && p(end) && start.x < middle.x && middle.x < end.x,
            ProfileEntity::ParabolaTwoPointsGrade { start, end, grade, .. } => p(start) && p(end) && start.x < end.x && grade.is_finite(),
            ProfileEntity::ParabolaTwoPointsK { start, end, k: v, .. } | ProfileEntity::ParabolaTwoPointsRadius { start, end, radius: v, .. } => p(start) && p(end) && start.x < end.x && k(v),
            ProfileEntity::FloatingParabolaK { end, k: v, .. } => p(end) && k(v),
            ProfileEntity::FloatingParabolaGrade { end, grade } => p(end) && grade.is_finite(),
            ProfileEntity::FloatingTangent { through } => p(through),
            ProfileEntity::FreeTangent => true,
        }
    }

    /// A parabola (fixed or floating), which a floating or free tangent
    /// can touch.
    fn parabola(self) -> bool {
        self.codes().0 == 6
    }
}

/// The entities through `pvis`: the tangents (ids 1..n in station order)
/// then a curve per inner PVI that has one (ids after the tangents), as
/// `(id, previous, next, part)`, linked along the profile.
fn parts(pvis: &[Vector2], curves: &[VerticalCurve]) -> Vec<(i32, i32, i32, Part)> {
    let n = pvis.len() - 1;
    let mut next = n as i32;
    let ids: Vec<Option<i32>> = curves
        .iter()
        .map(|c| {
            c.is_some().then(|| {
                next += 1;
                next
            })
        })
        .collect();
    let mut out: Vec<(i32, i32, i32, Part)> = (0..n)
        .map(|i| {
            let id = i as i32 + 1;
            let previous = if i == 0 { 0 } else { ids[i - 1].unwrap_or(id - 1) };
            let following = if i + 1 == n { 0 } else { ids[i].unwrap_or(id + 1) };
            (id, previous, following, Part::Tangent(pvis[i], pvis[i + 1]))
        })
        .collect();
    let grade = |i: usize| (pvis[i + 1].y - pvis[i].y) / (pvis[i + 1].x - pvis[i].x);
    for (j, c) in curves.iter().enumerate() {
        if let Some(id) = ids[j] {
            out.push((id, j as i32 + 1, j as i32 + 2, Part::Curve { curve: *c, crest: grade(j + 1) < grade(j) }));
        }
    }
    out
}

/// The source of a profile sampled from a surface: the surface, the
/// offset from the alignment and the stations it samples between, and
/// whether they are the alignment's own (`whole`).
struct Sampled {
    surface: u64,
    offset: f64,
    range: (f64, f64),
    whole: bool,
}

/// What an offset profile's record holds besides its entities (see
/// [`add_offset_profile`]).
struct OffsetTail {
    slope: f64,
    offset: f64,
    parent_length: f64,
    /// For each source entity (by id) the entities copied from it.
    map: Vec<(i32, Vec<i32>)>,
    /// The parent's anchors at the source's start and end.
    anchors: [(f64, [u8; 16]); 2],
    /// Its companion and its source.
    feature: u64,
    source: u64,
}

/// The profile record.
///
/// ```text
/// BL 80, BL 0, BL 80, BL 80, BL 0, BL 0x10025F04, 6 bits 010101
/// RC 24 keys (KEYS), BL 0 (one-byte form)
/// 63 zero bits, BD 2, 9 bits 011011010
/// BL 0, BD 0, BD 0, BD 30, 5 x BS 0, 10 bits 0000001000
/// BL n, the tangents then the curves; each: 11 bits (2 tangent,
///   4 circular, 6 parabola, 8 asymmetric parabola), 23 bits
///   (0x08 << 13), BL constraint, BL id, BL previous id, BL next id, then
///   tangent (0):     2RD start, 2RD end (station, elevation), 4 zero bits
///   parabola (0):    BL 1, BD length, BL 0x1001 crest / 0x1000 sag, BL 0,
///                    3 bits 010, 3 x BD -1 (no minimum K)
///   circular (9):    BD radius
///   asymmetric (10): BD length before, BD length after the PVI
/// 2 bits 10, the identifier (a version-4 GUID, 16 bytes), TAIL_BITS
/// references: the seven helpers (hard ownership), soft 0, hard 0, hard 0,
///   soft 0, style (hard), alignment (hard), soft 0, hard 0, soft 0, soft 0
/// texts: "AeccImpVAlignment", "", "", the type-4 keys' texts, name, four
///   empty texts and one more per symmetric parabola
/// ```
///
/// Written with the PVIs and curves of WR01, WR02 and WR10 to WR13, the
/// records equal the samples' bit for bit apart from the geometry stamp
/// and the identifier.
///
/// An offset profile (OP01, OP02, OB01): `BL 0x00025F04`; three more keys
/// (`0x0100FF46` BL 0 after `0x0300FB05`, `0x0D00FF48` BL 4095 after
/// `0x0400FE09`, `0x0200FF40` BD slope last); 65 zero bits, `BD 2.25`, 7
/// bits `0111010` in place of the 63 bits, BD and 9 bits; `BL 0x102`,
/// `BD 0, BD 0, BD 30, BD 0, BD offset, 3 x BS 0` in place of `BL 0,
/// BD 0, BD 0, BD 30, 5 x BS 0`; the tail's lists filled (see
/// `profile::after_entities`); references 8 and 9 its companion, 15 the
/// source profile (soft).
#[allow(clippy::too_many_arguments)]
fn write_profile(doc: &mut CadDocument, common: EntityCommon, name: &str, parts: &[(i32, i32, i32, Part)], helpers: &[Handle; 7], style: u64, alignment: Handle, id: &[u8; 16]) -> Result<Handle, DxfError> {
    write_profile_of(doc, common, name, parts, helpers, style, alignment, id, None, None, (0.0, 0.0), None)
}

/// [`write_profile`]; with `sampled` a profile sampled from a surface (no
/// entities): after the 9 bits, `BL 7` (0x47 for stations other than the
/// alignment's), `BD start, BD end, BD 30, BD offset`, 12 bits
/// `101011000010`, then `2 bits 10`, a zero identifier and the tail; the
/// surface is the reference after the alignment (soft pointer; WS11 to
/// WS15). `extent` is the start and end station a static copy keeps in the
/// first two `BD` before the entities (0 otherwise; OS01, OS02).
#[allow(clippy::too_many_arguments)]
fn write_profile_of(doc: &mut CadDocument, common: EntityCommon, name: &str, parts: &[(i32, i32, i32, Part)], helpers: &[Handle; 7], style: u64, alignment: Handle, id: &[u8; 16], sampled: Option<&Sampled>, offset: Option<&OffsetTail>, extent: (f64, f64), curb_return: Option<[u64; 2]>) -> Result<Handle, DxfError> {
    let byte = |w: &mut DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    // A profile of an alignment on a site is a member of the site's
    // container: it names the container in place of a companion and
    // holds its edge id as an alignment does (WC64, WC65).
    let site = match super::site::site_container_of(doc, alignment) {
        Some(c) => Some((c, super::site::join_profile(doc, c, common.handle)?)),
        None => None,
    };
    let feature = site.map_or(offset.map_or(0, |o| o.feature), |s| s.0.value());
    let mut refs: Vec<Ref> = helpers.iter().map(|h| Ref(DwgReferenceType::HardOwnership, h.value())).collect();
    refs.extend([soft_pointer(0), hard_pointer(feature), hard_pointer(feature), soft_pointer(0), hard_pointer(style), hard_pointer(alignment.value()), soft_pointer(sampled.map_or(0, |s| s.surface)), hard_pointer(0), soft_pointer(offset.map_or(0, |o| o.source))]);
    // A curb return profile names the two offset profiles it joins and its
    // curb return (WC61); other profiles end with a soft 0.
    match curb_return {
        Some([incoming, outgoing]) => refs.extend([soft_pointer(incoming), soft_pointer(outgoing), soft_pointer(alignment.value())]),
        None => refs.push(soft_pointer(0)),
    }
    let mut strings = vec!["AeccImpVAlignment".to_string(), String::new(), String::new()];
    for (key, k) in &KEYS {
        if let Key::Text(t) = k {
            strings.push(t.to_string());
        }
        // A curb return profile's key 0x0400FF80 holds its curve type.
        if curb_return.is_some() && *key == 0x0400_FE0A {
            strings.push(CURB_RETURN_CURVE_TYPE.to_string());
        }
    }
    strings.push(name.to_string());
    let parabolas = parts.iter().filter(|p| matches!(p.3, Part::Curve { curve, .. } if curve.parabola()) || matches!(p.3, Part::Fixed(e) if e.codes().0 == 6)).count();
    strings.extend((0..4 + parabolas).map(|_| String::new()));
    put_entity(
        doc,
        "AECC_PROFILE",
        common,
        |w| {
            for v in [80, 0, 80, 80, 0, if offset.is_some() { 0x0002_5F04 } else { 0x1002_5F04 }] {
                w.write_bit_long(v);
            }
            bits(w, "010101");
            byte(w, KEYS.len() as u8 + if offset.is_some() { 3 } else { 0 } + u8::from(curb_return.is_some()));
            for (key, value) in &KEYS {
                w.write_bit_long(*key as i32);
                match value {
                    Key::Text(_) | Key::Helper => {}
                    Key::Byte(b) => byte(w, *b),
                    Key::Stamp => {
                        for v in [2, 0, 0] {
                            w.write_bit_long(v);
                        }
                    }
                }
                if curb_return.is_some() && *key == 0x0400_FE0A {
                    w.write_bit_long(0x0400_FF80);
                }
                if offset.is_some() {
                    match key {
                        0x0300_FB05 => {
                            w.write_bit_long(0x0100_FF46);
                            w.write_bit_long(0);
                        }
                        0x0400_FE09 => {
                            w.write_bit_long(0x0D00_FF48);
                            w.write_bit_long(0xFFF);
                        }
                        _ => {}
                    }
                }
            }
            if let Some(o) = offset {
                w.write_bit_long(0x0200_FF40);
                w.write_bit_double(o.slope);
            }
            bl_byte(w, 0);
            if let Some((_, edge)) = site {
                bits(w, &"0".repeat(115));
                bits(w, &format!("00000010{:08b}{:02b}11010", 0x40 | (edge >> 2), edge & 3));
            } else if offset.is_some() {
                bits(w, &"0".repeat(65));
                w.write_bit_double(2.25);
                bits(w, "0111010");
            } else {
                bits(w, &"0".repeat(63));
                w.write_bit_double(2.0);
                bits(w, "011011010");
            }
            if let Some(s) = sampled {
                w.write_bit_long(if s.whole { 7 } else { 0x47 });
                for v in [s.range.0, s.range.1, 30.0, s.offset] {
                    w.write_bit_double(v);
                }
                bits(w, "101011000010");
                bits(w, "10");
                w.write_bytes(&[0; 16]);
                bits(w, TAIL_BITS);
                return;
            }
            // `BD 0` and `BS 0` share their bits.
            w.write_bit_long(if offset.is_some() { 0x102 } else if curb_return.is_some() { 0x202 } else { 0 });
            for v in [extent.0, extent.1, 30.0, 0.0, offset.map_or(0.0, |o| o.offset)] {
                w.write_bit_double(v);
            }
            for _ in 0..3 {
                w.write_bit_short(0);
            }
            bits(w, "0000001000");
            w.write_bit_long(parts.len() as i32);
            for (id, previous, next, part) in parts {
                let (prefix, constraint) = match part {
                    Part::Tangent(..) => (2, 0),
                    Part::Curve { curve: VerticalCurve::Circular(_), .. } => (4, 9),
                    Part::Curve { curve: VerticalCurve::CircularLength(_), .. } => (4, 0),
                    Part::Curve { curve: VerticalCurve::CircularThrough(_), .. } => (4, 7),
                    Part::Curve { curve: VerticalCurve::Asymmetric { .. }, .. } => (8, 10),
                    Part::Curve { curve: VerticalCurve::ParabolaK(_), .. } => (6, 2),
                    Part::Curve { curve: VerticalCurve::ParabolaRadius(_), .. } => (6, 9),
                    Part::Curve { curve: VerticalCurve::ParabolaThrough(_), .. } => (6, 7),
                    Part::Curve { .. } => (6, 0),
                    Part::Fixed(e) => e.codes(),
                };
                bits(w, &format!("{prefix:011b}{:023b}", 0x08 << 13));
                for v in [constraint, *id, *previous, *next] {
                    w.write_bit_long(v);
                }
                match part {
                    Part::Tangent(a, b) | Part::Fixed(ProfileEntity::Tangent { start: a, end: b }) => {
                        for v in [a.x, a.y, b.x, b.y] {
                            w.write_raw_double(v);
                        }
                        bits(w, "0000");
                    }
                    // A floating tangent: its point, then 4 bits; a free
                    // tangent: the 4 bits only (PF02).
                    Part::Fixed(ProfileEntity::FloatingTangent { through }) => {
                        w.write_raw_double(through.x);
                        w.write_raw_double(through.y);
                        bits(w, "0000");
                    }
                    Part::Fixed(ProfileEntity::FreeTangent) => bits(w, "0000"),
                    // A circular curve: its three points only, like the
                    // other circular curves' values (OC01, OC02).
                    Part::Fixed(ProfileEntity::CircularThreePoints { start, on, end }) => {
                        for p in [start, on, end] {
                            w.write_raw_double(p.x);
                            w.write_raw_double(p.y);
                        }
                    }
                    Part::Fixed(e) => {
                        // BL 0, the points and values, then BL 0, 3 bits
                        // 010 and no minimum K (WE01 to WE08).
                        let point = |w: &mut DwgMergedWriter, p: Vector2| {
                            w.write_raw_double(p.x);
                            w.write_raw_double(p.y);
                        };
                        w.write_bit_long(0);
                        match *e {
                            ProfileEntity::ParabolaThreePoints { start, middle, end } => {
                                for p in [start, middle, end] {
                                    point(w, p);
                                }
                            }
                            ProfileEntity::ParabolaTwoPointsGrade { start, end, grade, at_end } => {
                                point(w, start);
                                point(w, end);
                                w.write_bit_long(i32::from(at_end));
                                w.write_bit_double(grade * 100.0);
                            }
                            // The value word: 0x2000 a K value, 0x2002 a radius.
                            ProfileEntity::ParabolaTwoPointsK { start, end, k: value, crest } | ProfileEntity::ParabolaTwoPointsRadius { start, end, radius: value, crest } => {
                                point(w, start);
                                point(w, end);
                                w.write_bit_long(if crest { 0x1001 } else { 0x1000 });
                                w.write_bit_long(if matches!(*e, ProfileEntity::ParabolaTwoPointsK { .. }) { 0x2000 } else { 0x2002 });
                                w.write_bit_double(value);
                            }
                            ProfileEntity::FloatingParabolaK { end, k, crest } => {
                                point(w, end);
                                w.write_bit_long(if crest { 0x1001 } else { 0x1000 });
                                w.write_bit_long(0x2000);
                                w.write_bit_double(k);
                            }
                            ProfileEntity::FloatingParabolaGrade { end, grade } => {
                                point(w, end);
                                w.write_bit_long(1);
                                w.write_bit_double(grade * 100.0);
                            }
                            ProfileEntity::Tangent { .. } | ProfileEntity::FloatingTangent { .. } | ProfileEntity::FreeTangent | ProfileEntity::CircularThreePoints { .. } => {}
                        }
                        w.write_bit_long(0);
                        bits(w, "010");
                        for _ in 0..3 {
                            w.write_bit_double(-1.0);
                        }
                    }
                    Part::Curve { curve, crest } => match *curve {
                        VerticalCurve::Parabola(_) | VerticalCurve::ParabolaK(_) | VerticalCurve::ParabolaRadius(_) | VerticalCurve::ParabolaThrough(_) => {
                            // A length opens with BL 1, a K value or radius
                            // with BL 0; a point has no crest flag.
                            match *curve {
                                VerticalCurve::Parabola(v) => {
                                    w.write_bit_long(1);
                                    w.write_bit_double(v);
                                }
                                VerticalCurve::ParabolaK(v) | VerticalCurve::ParabolaRadius(v) => {
                                    w.write_bit_long(0);
                                    w.write_bit_double(v);
                                }
                                VerticalCurve::ParabolaThrough(p) => {
                                    w.write_bit_long(0);
                                    w.write_raw_double(p.x);
                                    w.write_raw_double(p.y);
                                }
                                _ => {}
                            }
                            if !matches!(curve, VerticalCurve::ParabolaThrough(_)) {
                                w.write_bit_long(if *crest { 0x1001 } else { 0x1000 });
                            }
                            w.write_bit_long(0);
                            bits(w, "010");
                            for _ in 0..3 {
                                w.write_bit_double(-1.0);
                            }
                        }
                        VerticalCurve::Circular(v) | VerticalCurve::CircularLength(v) => w.write_bit_double(v),
                        VerticalCurve::CircularThrough(p) => {
                            w.write_raw_double(p.x);
                            w.write_raw_double(p.y);
                        }
                        VerticalCurve::Asymmetric { before, after } => {
                            w.write_bit_double(before);
                            w.write_bit_double(after);
                        }
                        VerticalCurve::None => {}
                    },
                }
            }
            // A curb return profile copies its three pieces (incoming
            // lane, joining tangent, outgoing lane), each as a list of one
            // unlinked tangent with ids 1, 2 and 1 (WC61).
            if curb_return.is_some() {
                for ((.., part), copy_id) in parts.iter().zip([1, 2, 1]) {
                    let Part::Tangent(a, b) = part else { continue };
                    w.write_bit_long(1);
                    bits(w, &format!("{:011b}{:023b}", 2, 0x08 << 13));
                    for v in [0, copy_id, 0, 0] {
                        w.write_bit_long(v);
                    }
                    for v in [a.x, a.y, b.x, b.y] {
                        w.write_raw_double(v);
                    }
                    bits(w, "0000");
                }
            }
            bits(w, "10");
            w.write_bytes(id);
            if curb_return.is_some() {
                bits(w, PRE);
                w.write_bit_double(0.0);
                bits(w, MID);
                for _ in 0..4 {
                    w.write_bit_long(0);
                }
                bits(w, CURB_RETURN_END);
            } else {
                write_tail(w, offset);
            }
        },
        Body { refs, strings },
    )
}

/// The profile's tail after its identifier (see [`OffsetTail`]): equal to
/// `TAIL_BITS` without an offset.
fn write_tail(w: &mut DwgMergedWriter, offset: Option<&OffsetTail>) {
    bits(w, PRE);
    w.write_bit_double(offset.map_or(0.0, |o| o.slope));
    bits(w, MID);
    let Some(o) = offset else {
        for _ in 0..4 {
            w.write_bit_long(0);
        }
        bits(w, END);
        return;
    };
    w.write_bit_long(1);
    for v in [o.slope, 0.0, o.parent_length, 0.0, 0.0] {
        w.write_bit_double(v);
    }
    for v in [2, 0, 0, 0, o.map.len() as i32] {
        w.write_bit_long(v);
    }
    for (source, ids) in &o.map {
        for v in [*source, -1, ids.len() as i32] {
            w.write_bit_long(v);
        }
        for id in ids {
            w.write_bit_long(*id);
        }
    }
    w.write_bit_long(2);
    for (station, id) in &o.anchors {
        w.write_bit_double(*station);
        w.write_bytes(id);
    }
    bits(w, END);
}

/// The helper records a profile owns: each the version word and `BL 0`
/// (the notification infos three more bits `010`).
fn write_helpers(doc: &mut CadDocument, profile: Handle) -> Result<[Handle; 7], DxfError> {
    let handles = [(); 7].map(|_| doc.allocate_handle());
    for (dxf, h) in HELPERS.iter().zip(handles) {
        put_object(
            doc,
            dxf,
            h,
            profile,
            Vec::new(),
            |w| {
                w.write_bit_long(0);
                if *dxf == "AECC_NOTIFICATIONINFOS" {
                    bits(w, "010");
                }
            },
            Body { refs: Vec::new(), strings: Vec::new() },
        )?;
    }
    Ok(handles)
}

/// Adds a layout profile to `alignment` through `pvis` (station,
/// elevation; stations increasing) on `layer` (layer "0" when `None`),
/// with a free symmetric parabola of length `curves[j]` at inner PVI
/// `j + 1` (0 for none). See [`add_profile_with_curves`].
pub fn add_profile(doc: &mut CadDocument, alignment: Handle, name: &str, pvis: &[Vector2], curves: &[f64], layer: Option<&str>) -> Result<Handle, DxfError> {
    let curves: Vec<VerticalCurve> = curves.iter().map(|c| if *c == 0.0 { VerticalCurve::None } else { VerticalCurve::Parabola(*c) }).collect();
    add_profile_with_curves(doc, alignment, name, pvis, &curves, layer)
}

/// Adds a layout profile to `alignment` through `pvis` (station,
/// elevation; stations increasing) on `layer` (layer "0" when `None`),
/// with the vertical curve `curves[j]` at inner PVI `j + 1`. The curves
/// must fit on the tangents. The alignment must be one this crate writes
/// (it is written again with the profile in its list); a drawing without
/// a profile style gets the default one.
pub fn add_profile_with_curves(doc: &mut CadDocument, alignment: Handle, name: &str, pvis: &[Vector2], curves: &[VerticalCurve], layer: Option<&str>) -> Result<Handle, DxfError> {
    if pvis.len() < 2 || pvis.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) || pvis.windows(2).any(|w| w[1].x <= w[0].x) {
        return Err(invalid("a profile needs two or more finite PVIs with increasing stations"));
    }
    if curves.len() + 2 != pvis.len() {
        return Err(invalid("a profile needs one curve (or none) per inner PVI"));
    }
    let grade = |i: usize| (pvis[i + 1].y - pvis[i].y) / (pvis[i + 1].x - pvis[i].x);
    let mut reach = Vec::with_capacity(curves.len());
    for (j, c) in curves.iter().enumerate() {
        let r = c.reach(pvis[j + 1], grade(j), grade(j + 1)).ok_or_else(|| invalid("a vertical curve needs positive values (or a point it can pass)"))?;
        if c.is_some() && grade(j) == grade(j + 1) {
            return Err(invalid("a curve's grades are equal"));
        }
        reach.push(r);
    }
    for j in 0..curves.len() {
        let before = if j == 0 { 0.0 } else { reach[j - 1].1 };
        let after = reach.get(j + 1).map_or(0.0, |r| r.0);
        if reach[j].0 + before > pvis[j + 1].x - pvis[j].x || reach[j].1 + after > pvis[j + 2].x - pvis[j + 1].x {
            return Err(invalid("a curve does not fit on its tangents"));
        }
    }
    if name.is_empty() {
        return Err(invalid("a profile needs a name"));
    }
    let (written, before) = rewritable(doc, alignment, |h| identifier(doc, h)).ok_or_else(|| invalid("the alignment is not one this crate writes"))?;
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    ensure_profile_tree(doc)?;
    let style = default_style(doc).ok_or_else(|| invalid("drawing has no profile styles"))?;
    let handle = doc.allocate_handle();
    let id = new_identifier(handle.value());
    let mut profiles: Vec<ProfileRef> = before.iter().map(|p| ProfileRef { handle: p.handle, id: p.id }).collect();
    profiles.push(ProfileRef { handle: handle.value(), id });
    rewrite(doc, alignment, written, &before, &profiles)?;
    let helpers = write_helpers(doc, handle)?;
    let common = EntityCommon {
        handle,
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    write_profile(doc, common, name, &parts(pvis, curves), &helpers, style, alignment, &id)
}

/// Adds a layout profile `name` to `alignment` made entity by entity
/// (`entities` in station order: fixed tangents, fixed parabolas by three
/// points, by two points and a grade or K, floating parabolas that leave
/// the entity before them, floating tangents that leave the parabola
/// before them towards a point, free tangents between two parabolas), on
/// `layer` (layer "0" when `None`). The ids
/// follow the order given and each entity is linked to its neighbours in
/// it. The alignment must be one this crate writes.
pub fn add_profile_entities(doc: &mut CadDocument, alignment: Handle, name: &str, entities: &[ProfileEntity], layer: Option<&str>) -> Result<Handle, DxfError> {
    if name.is_empty() || entities.is_empty() || entities.iter().any(|e| !e.valid()) {
        return Err(invalid("a profile needs a name and valid entities"));
    }
    if matches!(entities[0], ProfileEntity::FloatingParabolaK { .. } | ProfileEntity::FloatingParabolaGrade { .. }) {
        return Err(invalid("a floating parabola needs an entity before it"));
    }
    // A floating tangent leaves a parabola; a free tangent sits between two.
    for (i, e) in entities.iter().enumerate() {
        let parabola = |j: Option<usize>| j.and_then(|j| entities.get(j)).is_some_and(|e| e.parabola());
        let ok = match e {
            ProfileEntity::FloatingTangent { .. } => parabola(i.checked_sub(1)),
            ProfileEntity::FreeTangent => parabola(i.checked_sub(1)) && parabola(Some(i + 1)),
            _ => true,
        };
        if !ok {
            return Err(invalid("a floating tangent needs a parabola before it, a free tangent one on each side"));
        }
    }
    let n = entities.len() as i32;
    let parts: Vec<(i32, i32, i32, Part)> = entities.iter().enumerate().map(|(i, e)| {
        let id = i as i32 + 1;
        (id, id - 1, if id == n { 0 } else { id + 1 }, Part::Fixed(*e))
    }).collect();
    write_entity_profile(doc, alignment, name, &parts, layer, (0.0, 0.0))
}

/// Writes the entity profile `parts` on `alignment` (see
/// [`add_profile_entities`]), with `extent` for a static copy.
fn write_entity_profile(doc: &mut CadDocument, alignment: Handle, name: &str, parts: &[(i32, i32, i32, Part)], layer: Option<&str>, extent: (f64, f64)) -> Result<Handle, DxfError> {
    let (written, before) = rewritable(doc, alignment, |h| identifier(doc, h)).ok_or_else(|| invalid("the alignment is not one this crate writes"))?;
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    ensure_profile_tree(doc)?;
    let style = default_style(doc).ok_or_else(|| invalid("drawing has no profile styles"))?;
    let handle = doc.allocate_handle();
    let id = new_identifier(handle.value());
    let mut profiles: Vec<ProfileRef> = before.iter().map(|p| ProfileRef { handle: p.handle, id: p.id }).collect();
    profiles.push(ProfileRef { handle: handle.value(), id });
    rewrite(doc, alignment, written, &before, &profiles)?;
    let helpers = write_helpers(doc, handle)?;
    let common = EntityCommon {
        handle,
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    write_profile_of(doc, common, name, parts, &helpers, style, alignment, &id, None, None, extent, None)
}

/// Adds a static layout profile `name` copied from the offset profile
/// `source` (the application's static finished-ground copy, which it makes
/// only of derived profiles): on the source's alignment, with the source's
/// entities as they are stored and its start and end station kept with
/// them (OS01, OS02); on `layer` (layer "0" when `None`). The source's
/// alignment must be one this crate writes.
pub fn add_static_profile(doc: &mut CadDocument, source: Handle, name: &str, layer: Option<&str>) -> Result<Handle, DxfError> {
    if name.is_empty() {
        return Err(invalid("a profile needs a name"));
    }
    let src = match doc.get_entity(source) {
        Some(EntityType::Extended(x)) => match &x.data {
            ExtendedEntityData::Arch(a) if a.dxf_name == "AECC_PROFILE" => a,
            _ => return Err(invalid("source profile not found")),
        },
        _ => return Err(invalid("source profile not found")),
    };
    let alignment = src.typed_handles().get(12).map(|r| Handle::new(r.0)).ok_or_else(|| invalid("the source has no alignment"))?;
    let (data, start, end) = src.main_stream().ok_or_else(|| invalid("source profile cannot be read"))?;
    super::super::profile::offset_profile(data, start, end).ok_or_else(|| invalid("the source is not an offset profile"))?;
    let entities = super::super::profile::fixed_entities(data, start, end).ok_or_else(|| invalid("the source's entities are not fixed entities"))?;
    let points = super::super::profile::entity_points(data, start, end).filter(|p| !p.is_empty()).ok_or_else(|| invalid("source profile cannot be solved"))?;
    let extent = (points[0].2[0].x, points[points.len() - 1].2[2].x);
    let n = entities.len() as i32;
    let parts: Vec<(i32, i32, i32, Part)> = entities.iter().enumerate().map(|(i, e)| {
        let id = i as i32 + 1;
        (id, id - 1, if id == n { 0 } else { id + 1 }, Part::Fixed(*e))
    }).collect();
    write_entity_profile(doc, alignment, name, &parts, layer, extent)
}

/// Adds a profile `name` of `alignment` sampled from the TIN surface
/// `surface` at `offset` from the alignment (negative to the left), between
/// the stations `range` (the alignment's whole length when `None`), on
/// `layer` (layer "0" when `None`). The record holds no elevations: the
/// application samples the surface when it opens the drawing. The alignment
/// is written again with the profile in its list (no anchor).
pub fn add_surface_profile(doc: &mut CadDocument, alignment: Handle, surface: Handle, name: &str, offset: f64, range: Option<(f64, f64)>, layer: Option<&str>) -> Result<Handle, DxfError> {
    if name.is_empty() || !offset.is_finite() {
        return Err(invalid("a surface profile needs a name and a finite offset"));
    }
    if !matches!(doc.get_entity(surface), Some(EntityType::Extended(x)) if matches!(&x.data, ExtendedEntityData::Arch(a) if a.dxf_name == "AECC_TIN_SURFACE")) {
        return Err(invalid("surface not found"));
    }
    let (written, before) = rewritable(doc, alignment, |h| identifier(doc, h)).ok_or_else(|| invalid("the alignment is not one this crate writes"))?;
    let length = super::alignment::length_of(&written.elements).ok_or_else(|| invalid("the alignment cannot be solved"))?;
    let whole = range.is_none();
    let range = range.unwrap_or((0.0, length));
    if !(range.0.is_finite() && range.1.is_finite() && range.0 >= 0.0 && range.0 < range.1 && range.1 <= length + 1e-9 * length.max(1.0)) {
        return Err(invalid("the stations must run forward within the alignment"));
    }
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    ensure_profile_tree(doc)?;
    let style = default_style(doc).ok_or_else(|| invalid("drawing has no profile styles"))?;
    let handle = doc.allocate_handle();
    // The profile is listed without an anchor: its identifier is zero.
    let mut profiles: Vec<ProfileRef> = before.iter().map(|p| ProfileRef { handle: p.handle, id: p.id }).collect();
    profiles.push(ProfileRef { handle: handle.value(), id: [0; 16] });
    rewrite(doc, alignment, written, &before, &profiles)?;
    let helpers = write_helpers(doc, handle)?;
    let common = EntityCommon {
        handle,
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    let sampled = Sampled { surface: surface.value(), offset, range, whole };
    write_profile_of(doc, common, name, &[], &helpers, style, alignment, &[0; 16], Some(&sampled), None, (0.0, 0.0), None)
}

/// Anchor kinds of an offset profile's start and end on the parent (the
/// application wrote 221 and 9 for the start in two samples; it accepts
/// either).
const OFFSET_START_ANCHOR: i32 = 9;
const OFFSET_END_ANCHOR: i32 = 0;

/// The entities of an offset profile copied from `source` (link order: id,
/// kind, points at the start, middle station and end, curve), each cut
/// where `map` (parent stations to the offset alignment's, see
/// `offset_station_map`) changes and moved to the offset alignment's
/// stations, raised by `slope` times the offset there. A parent station
/// where a widening meets the nominal offset takes the nominal side (OW01:
/// a transition of length 0 follows no parent stations); a piece along a
/// transition is a parabola between its ends leaving with the grade of the
/// mapped curve there (OX01). A circular piece is the circle through its
/// three mapped points: exact where the offset is constant along a parent
/// line; along a parent arc the stations scale by `(R ± o) / R` and the
/// mapped curve is an ellipse the circle only passes through at those
/// points. Returns the entities in station order and, by source id,
/// the ids (1 up, in station order) made from each.
#[allow(clippy::type_complexity)]
fn offset_entities(source: &[(i32, crate::classes::civil::ProfilePiece, [Vector2; 3], super::super::profile::ProfileCurve)], map: &[super::alignment::StationRow], slope: f64) -> Option<(Vec<ProfileEntity>, Vec<(i32, Vec<i32>)>)> {
    use crate::classes::civil::ProfilePiece;
    let tol = 1e-9 * map.last()?.parent.1.abs().max(1.0);
    // The row at a parent station, the nominal side first.
    let row = |s: f64| {
        let inside = |m: &&super::alignment::StationRow| s >= m.parent.0 - tol && s <= m.parent.1 + tol;
        map.iter().filter(|m| m.nominal).find(inside).or_else(|| map.iter().find(inside))
    };
    // The offset alignment's station and the rise at a parent station.
    let at = |s: f64| -> Option<(f64, f64)> {
        let m = row(s)?;
        let t = (s - m.parent.0) / (m.parent.1 - m.parent.0);
        Some((m.own.0 + t * (m.own.1 - m.own.0), slope * (m.offset.0 + t * (m.offset.1 - m.offset.0)).abs()))
    };
    let mut entities = Vec::new();
    let mut ids: Vec<(i32, Vec<i32>)> = Vec::new();
    let mut unsourced = Vec::new();
    // The parent stations the offset alignment follows; a source piece is
    // cut to them (an intersection's offsets follow part of their road).
    let (lo, hi) = (map.iter().map(|m| m.parent.0).fold(f64::INFINITY, f64::min), map.iter().map(|m| m.parent.1).fold(f64::NEG_INFINITY, f64::max));
    for (id, piece, [a, _, b], curve) in source {
        let (a, b) = (Vector2::new(a.x.max(lo), curve.elevation_at(a.x.max(lo))), Vector2::new(b.x.min(hi), curve.elevation_at(b.x.min(hi))));
        if b.x - a.x <= tol {
            continue;
        }
        let z = |x: f64| curve.elevation_at(x);
        // Cut where the rows change and where an asymmetric parabola's two
        // parabolas meet.
        let mut cuts = vec![a.x];
        cuts.extend(map.iter().map(|m| m.parent.0).chain(curve.asymmetric_pvi()).filter(|s| *s > a.x + tol && *s < b.x - tol));
        cuts.sort_by(f64::total_cmp);
        cuts.push(b.x);
        let mut made = Vec::new();
        for w in cuts.windows(2) {
            let p = |x: f64| -> Option<Vector2> {
                let (own, rise) = at(x)?;
                Some(Vector2::new(own, z(x) + rise))
            };
            let m = row((w[0] + w[1]) / 2.0)?;
            let along_transition = m.transition;
            // The mapped curve's grade at the piece's start: the source's
            // grade plus the rise's, over the stations' scale.
            let grade = || {
                let span = m.parent.1 - m.parent.0;
                let sign = if m.offset.0 + m.offset.1 < 0.0 { -1.0 } else { 1.0 };
                let rise = slope * sign * (m.offset.1 - m.offset.0) / span;
                (curve.grade_at(w[0]) + rise) / ((m.own.1 - m.own.0) / span)
            };
            entities.push(match piece {
                _ if along_transition => ProfileEntity::ParabolaTwoPointsGrade { start: p(w[0])?, end: p(w[1])?, grade: grade(), at_end: false },
                ProfilePiece::Tangent => ProfileEntity::Tangent { start: p(w[0])?, end: p(w[1])? },
                // Circular curves stay circular, asymmetric parabolas become
                // parabolas through their points (OC01 to OC03).
                ProfilePiece::Circular => ProfileEntity::CircularThreePoints { start: p(w[0])?, on: p((w[0] + w[1]) / 2.0)?, end: p(w[1])? },
                _ => ProfileEntity::ParabolaThreePoints { start: p(w[0])?, middle: p((w[0] + w[1]) / 2.0)?, end: p(w[1])? },
            });
            // A tangent's piece along a transition is listed under id 0
            // (OX01).
            if along_transition && *piece == ProfilePiece::Tangent {
                unsourced.push(entities.len() as i32);
            } else {
                made.push(entities.len() as i32);
            }
        }
        ids.push((*id, made));
    }
    if !unsourced.is_empty() {
        unsourced.sort();
        ids.push((0, unsourced));
    }
    ids.sort_by_key(|e| e.0);
    Some((entities, ids))
}

/// Writes the curb return profile `name` at `handle` with the identifier
/// `id` on the curb return alignment `alignment` (written listing it):
/// fixed tangents through `pvis`
/// (station, elevation), joining the offset profiles `incoming` and
/// `outgoing` of the lanes it runs along. Its record is a layout
/// profile's with the class key 0x0400FF80 after 0x0400FE0A, `BL 0x202`
/// before the five `BD`, its own constant run in place of `END` and the
/// two offset profiles and the curb return as its last references (soft;
/// WC61).
#[allow(clippy::too_many_arguments)]
pub(super) fn write_curb_return_profile(doc: &mut CadDocument, handle: Handle, id: &[u8; 16], alignment: Handle, name: &str, pvis: &[Vector2], incoming: Handle, outgoing: Handle, style: Option<u64>, layer: Option<&str>) -> Result<Handle, DxfError> {
    if pvis.len() < 2 || pvis.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) || pvis.windows(2).any(|w| w[1].x <= w[0].x) {
        return Err(invalid("a curb return profile needs two or more finite PVIs with increasing stations"));
    }
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    ensure_profile_tree(doc)?;
    let style = match style { Some(s) => s, None => default_style(doc).ok_or_else(|| invalid("drawing has no profile styles"))? };
    let helpers = write_helpers(doc, handle)?;
    let common = EntityCommon { handle, owner_handle: doc.header.model_space_block_handle, layer: layer.into(), layer_handle: Some(layer_handle), ..EntityCommon::default() };
    let curves = vec![VerticalCurve::None; pvis.len() - 2];
    write_profile_of(doc, common, name, &parts(pvis, &curves), &helpers, style, alignment, id, None, None, (0.0, 0.0), Some([incoming.value(), outgoing.value()]))
}

/// Writes the layout profile `profile` again as fixed tangents through
/// `pvis` (station, elevation) with the vertical curve `curves[j]` at inner
/// PVI `j + 1`, keeping its name, layer, style, helper records and
/// identifier (the application's profile locking reshapes the other road's
/// profile this way). Two PVIs may share a station (a tangent of no
/// length: WC68) when neither has a curve. Only a layout profile (not an
/// offset, surface or curb return profile) is rewritten.
pub(super) fn set_profile_pvis(doc: &mut CadDocument, profile: Handle, pvis: &[Vector2], curves: &[VerticalCurve]) -> Result<(), DxfError> {
    if pvis.len() < 2 || curves.len() + 2 != pvis.len() || pvis.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) || pvis.windows(2).any(|w| w[1].x < w[0].x) || pvis[pvis.len() - 1].x <= pvis[0].x {
        return Err(invalid("a profile needs two or more finite PVIs with stations not decreasing, and one curve (or none) per inner PVI"));
    }
    let curved = |i: usize| i > 0 && i + 1 < pvis.len() && curves[i - 1].is_some();
    if (1..pvis.len()).any(|i| pvis[i].x == pvis[i - 1].x && (curved(i) || curved(i - 1))) {
        return Err(invalid("a curve's PVI shares its station with another PVI"));
    }
    let Some(EntityType::Extended(x)) = doc.get_entity(profile) else { return Err(invalid("profile not found")) };
    let ExtendedEntityData::Arch(a) = &x.data else { return Err(invalid("profile not found")) };
    if a.dxf_name != "AECC_PROFILE" || a.profile_offset().is_some() {
        return Err(invalid("only a layout profile is rewritten"));
    }
    let refs: Vec<u64> = a.typed_handles().iter().map(|r| r.0).collect();
    if refs.len() != 17 || refs[13] != 0 {
        return Err(invalid("only a layout profile is rewritten"));
    }
    let name = a.profile_name().unwrap_or_default();
    let common = x.common.clone();
    let helpers: [Handle; 7] = std::array::from_fn(|i| Handle::new(refs[i]));
    let id = identifier(doc, profile.value()).ok_or_else(|| invalid("the profile's identifier cannot be read"))?;
    write_profile(doc, common, &name, &parts(pvis, curves), &helpers, refs[11], Handle::new(refs[12]), &id)?;
    Ok(())
}

/// Adds an offset profile `name` on `offset_alignment` copied from
/// `source`, a profile of the offset alignment's parent, raised by `slope`
/// times the offset (the application's "by slope" offset profile), on
/// `layer` (layer "0" when `None`). Like the application it copies the
/// source's entities as fixed tangents, fixed parabolas by three points
/// (symmetric and asymmetric parabolas) and fixed circular curves by three
/// points, cut where the offset alignment's elements change, and ties it
/// to the source and the offset alignment by an action in the drawing's
/// associative network; the parent gets anchors at the source's start and
/// end.
///
/// The offset alignment must be one this crate writes (widenings with
/// linear transitions too), the source a profile within the stations the
/// offset alignment follows.
pub fn add_offset_profile(doc: &mut CadDocument, source: Handle, offset_alignment: Handle, name: &str, slope: f64, layer: Option<&str>) -> Result<Handle, DxfError> {
    add_offset_profile_styled(doc, source, offset_alignment, name, slope, None, layer)
}

/// [`add_offset_profile`] with `style` (else the drawing's default
/// profile style).
pub(super) fn add_offset_profile_styled(doc: &mut CadDocument, source: Handle, offset_alignment: Handle, name: &str, slope: f64, style: Option<u64>, layer: Option<&str>) -> Result<Handle, DxfError> {
    if name.is_empty() || !slope.is_finite() {
        return Err(invalid("an offset profile needs a name and a finite slope"));
    }
    let (parent, map) = super::alignment::offset_station_map(doc, offset_alignment).ok_or_else(|| invalid("the offset alignment's stations cannot be followed"))?;
    let arch = |h: Handle| match doc.get_entity(h) {
        Some(EntityType::Extended(x)) => match &x.data {
            ExtendedEntityData::Arch(a) => Some(a),
            _ => None,
        },
        _ => None,
    };
    let src = arch(source).filter(|a| a.dxf_name == "AECC_PROFILE").ok_or_else(|| invalid("source profile not found"))?;
    if src.typed_handles().get(12).map(|r| r.0) != Some(parent) {
        return Err(invalid("the source is not a profile of the offset alignment's parent"));
    }
    let (data, start, end) = src.main_stream().ok_or_else(|| invalid("source profile cannot be read"))?;
    let points = super::super::profile::entity_points(data, start, end).filter(|p| !p.is_empty()).ok_or_else(|| invalid("source profile cannot be solved"))?;
    let offset = arch(offset_alignment).and_then(|a| a.alignment_offset_parameters()).map(|o| o.offset).ok_or_else(|| invalid("offset alignment not found"))?;
    let parent_length = arch(Handle::new(parent)).and_then(|a| super::alignment::length_of(&a.alignment_elements()?)).ok_or_else(|| invalid("the parent cannot be solved"))?;
    // The parent anchors sit where the copied source starts and ends: on
    // an offset alignment over part of its parent, at that part's ends
    // (WC61).
    let (lo, hi) = (map.iter().map(|m| m.parent.0).fold(f64::INFINITY, f64::min), map.iter().map(|m| m.parent.1).fold(f64::NEG_INFINITY, f64::max));
    let (first, last) = (points[0].2[0].x.max(lo), points[points.len() - 1].2[2].x.min(hi));
    let (entities, ids) = offset_entities(&points, &map, slope).ok_or_else(|| invalid("the source must lie within the offset alignment's stations"))?;
    let (written, before) = rewritable(doc, offset_alignment, |h| identifier(doc, h)).ok_or_else(|| invalid("the offset alignment is not one this crate writes"))?;
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    ensure_profile_tree(doc)?;
    let style = match style { Some(s) => s, None => default_style(doc).ok_or_else(|| invalid("drawing has no profile styles"))? };
    let handle = doc.allocate_handle();
    let id = new_identifier(handle.value());
    let anchors = [(first, new_identifier(handle.value() ^ 1)), (last, new_identifier(handle.value() ^ 2))];
    super::alignment::add_anchors(doc, Handle::new(parent), OFFSET_START_ANCHOR, &[(anchors[0].1, anchors[0].0)])?;
    super::alignment::add_anchors(doc, Handle::new(parent), OFFSET_END_ANCHOR, &[(anchors[1].1, anchors[1].0)])?;
    let mut profiles: Vec<ProfileRef> = before.iter().map(|p| ProfileRef { handle: p.handle, id: p.id }).collect();
    profiles.push(ProfileRef { handle: handle.value(), id });
    rewrite(doc, offset_alignment, written, &before, &profiles)?;
    let helpers = write_helpers(doc, handle)?;
    // On a site the profile names the site's container instead.
    let feature = if super::site::site_container_of(doc, offset_alignment).is_some() { Handle::new(0) } else { super::alignment::add_profile_companion(doc, handle)? };
    let n = entities.len() as i32;
    let parts: Vec<(i32, i32, i32, Part)> = entities.iter().enumerate().map(|(i, e)| {
        let id = i as i32 + 1;
        (id, id - 1, if id == n { 0 } else { id + 1 }, Part::Fixed(*e))
    }).collect();
    let common = EntityCommon {
        handle,
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    let tail = OffsetTail { slope, offset, parent_length, map: ids, anchors, feature: feature.value(), source: source.value() };
    write_profile_of(doc, common, name, &parts, &helpers, style, offset_alignment, &id, None, Some(&tail), (0.0, 0.0), None)?;
    use super::assoc::{add_action, ActionBody, Dependency};
    add_action(
        doc,
        ActionBody { dxf: "AECC_ASSOC_OFFSET_VALIGNMENT_ACTION_BODY", refs: vec![handle], tail: "" },
        &[Dependency { object: source, writes: false, order: 0 }, Dependency { object: offset_alignment, writes: false, order: 0 }, Dependency { object: handle, writes: true, order: 0 }],
    )?;
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::DxfVersion;

    /// A drawing with stand-in nodes: a style tree listing one alignment
    /// style and one profile style, and the node of siteless alignments
    /// with its default feature style.
    fn drawing() -> CadDocument {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let root = ensure_civil(&mut doc).unwrap();
        let [tree, alignments, a_style, profiles, p_style, siteless, feature] = [(); 7].map(|_| doc.allocate_handle());
        let node = |doc: &mut CadDocument, handle: Handle, owner: Handle, dxf: &str, key: i32, children: &[(i32, Handle)]| {
            let refs = children.iter().map(|c| soft_owner(c.1.value())).collect();
            put_object(doc, dxf, handle, owner, Vec::new(), |w| {
                w.write_bit_long(key);
                w.write_bit_long(children.len() as i32);
                for c in children {
                    w.write_bit_long(c.0);
                }
                w.write_bit_long(0);
                bits(w, "010");
            }, Body { refs, strings: vec![String::new()] })
            .unwrap();
        };
        node(&mut doc, tree, root, "AECC_TREE_NODE", 31, &[(93, alignments), (KEY_PROFILE_STYLES, profiles)]);
        node(&mut doc, alignments, tree, "AECC_TREE_NODE", 93, &[(30, a_style)]);
        node(&mut doc, a_style, alignments, "AECC_TREE_NODE", 30, &[]);
        node(&mut doc, profiles, tree, "AECC_TREE_NODE", KEY_PROFILE_STYLES, &[(30, p_style)]);
        node(&mut doc, p_style, profiles, "AECC_TREE_NODE", 30, &[]);
        node(&mut doc, siteless, root, "AECC_TREE_NODE", 336, &[(30, feature)]);
        node(&mut doc, feature, siteless, "AECC_FEATURE_STYLE", 30, &[]);
        set_root_entry(&mut doc, 31, tree).unwrap();
        set_root_entry(&mut doc, 336, siteless).unwrap();
        doc
    }

    fn arch(doc: &CadDocument, h: Handle) -> crate::classes::arch::ArchEntityData {
        match doc.get_entity(h) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => a.clone(),
                _ => panic!(),
            },
            _ => panic!(),
        }
    }

    #[test]
    fn profiles_round_trip_and_join_their_alignment() {
        use crate::classes::civil::ProfilePiece;
        let mut doc = drawing();
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0)], None).unwrap();
        let v = [Vector2::new(0.0, 10.0), Vector2::new(50.0, 20.0), Vector2::new(100.0, 15.0)];
        assert!(add_profile(&mut doc, al, "X", &v, &[], None).is_err());
        assert!(add_profile(&mut doc, al, "X", &v, &[120.0], None).is_err());
        assert!(add_profile(&mut doc, al, "X", &[v[1], v[0]], &[], None).is_err());
        let p1 = add_profile(&mut doc, al, "P1", &v, &[0.0], None).unwrap();
        let p2 = add_profile(&mut doc, al, "P2", &v, &[20.0], None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-profile-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let kinds = |h: Handle| arch(&back, h).profile_pieces().iter().map(|p| p.0).collect::<Vec<_>>();
        assert_eq!(kinds(p1), vec![ProfilePiece::Tangent, ProfilePiece::Tangent]);
        assert_eq!(kinds(p2), vec![ProfilePiece::Tangent, ProfilePiece::Parabola, ProfilePiece::Tangent]);
        assert_eq!(arch(&back, p2).profile_name(), Some("P2".into()));
        let first = arch(&back, p2).profile_pieces()[0].1.last().copied().unwrap();
        assert!((first.x - 40.0).abs() < 1e-9 && (first.y - 18.0).abs() < 1e-9);
        // The alignment lists both profiles, each by its identifier.
        let a = arch(&back, al);
        assert_eq!(a.typed_handles()[6..].iter().map(|r| r.0).collect::<Vec<_>>(), vec![p1.value(), p2.value()]);
        assert_eq!(a.alignment_name(), Some("AL1".into()));
        let ids = [identifier(&back, p1.value()).unwrap(), identifier(&back, p2.value()).unwrap()];
        let (data, start, end) = a.main_stream().unwrap();
        let bits: Vec<u8> = (start..end).map(|i| data[i / 8] >> (7 - i % 8) & 1).collect();
        let find = |id: &[u8; 16]| {
            let want: Vec<u8> = id.iter().flat_map(|b| (0..8).rev().map(move |k| b >> k & 1)).collect();
            bits.windows(128).position(|w| w == want.as_slice())
        };
        // The anchors are stored in identifier order.
        assert_eq!(find(&ids[0]).unwrap() < find(&ids[1]).unwrap(), ids[0] < ids[1]);
        // A record that is not an alignment is refused.
        let mut doc = back;
        assert!(add_profile(&mut doc, p1, "P3", &v, &[0.0], None).is_err());
    }

    #[test]
    fn a_new_drawing_gets_its_profile_style() {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0)], None).unwrap();
        let p = add_profile(&mut doc, al, "P1", &[Vector2::new(0.0, 10.0), Vector2::new(100.0, 15.0)], &[], None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-profile-new-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let style = default_style(&back).unwrap();
        let st = object(&back, Handle::new(style)).unwrap();
        assert_eq!(st.dxf_name, "AECC_PROFILE_STYLE");
        // The style owns its extension (the reference of key 0x12010131).
        let extension = st.decode().handles[7];
        assert_eq!(object(&back, Handle::new(extension)).unwrap().dxf_name, "AECC_VALIGNMENT_STYLE_EXTENSION");
        assert_eq!(arch(&back, p).typed_handles()[11].0, style);
        assert_eq!(arch(&back, p).profile_name(), Some("P1".into()));
    }

    #[test]
    fn circular_and_asymmetric_curves_round_trip() {
        use crate::classes::civil::ProfilePiece;
        let mut doc = drawing();
        let v = |x: f64, y: f64| Vector2::new(x, y);
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(300.0, 0.0)], None).unwrap();
        let pvis = [v(0.0, 10.0), v(50.0, 20.0), v(100.0, 15.0), v(150.0, 25.0), v(250.0, 18.0)];
        let curves = [VerticalCurve::Circular(50.0), VerticalCurve::Asymmetric { before: 8.0, after: 12.0 }, VerticalCurve::Parabola(20.0)];
        assert!(add_profile_with_curves(&mut doc, al, "X", &pvis, &[VerticalCurve::Circular(2000.0), VerticalCurve::None, VerticalCurve::None], None).is_err());
        assert!(add_profile_with_curves(&mut doc, al, "X", &pvis, &[VerticalCurve::Asymmetric { before: 0.0, after: 1.0 }, VerticalCurve::None, VerticalCurve::None], None).is_err());
        let p = add_profile_with_curves(&mut doc, al, "P1", &pvis, &curves, None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-profile-curves-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let pieces = arch(&back, p).profile_pieces();
        let kinds: Vec<_> = pieces.iter().map(|p| p.0).collect();
        use ProfilePiece::*;
        assert_eq!(kinds, vec![Tangent, Circular, Tangent, AsymmetricParabola, Tangent, Parabola, Tangent]);
        // The circle of radius 50 leaves the first grade where the
        // reference application starts it (station 42.6636).
        assert!((pieces[0].1.last().unwrap().x - 42.663_583_344).abs() < 1e-6);
        assert!((pieces[3].1[0].x - 92.0).abs() < 1e-9 && (pieces[3].1.last().unwrap().x - 112.0).abs() < 1e-9);
    }

    #[test]
    fn curves_by_k_radius_length_and_point_read_back() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        let pvis = [v(0.0, 10.0), v(50.0, 20.0), v(100.0, 15.0)];
        // A point no parabola at the PVI passes is refused.
        assert!(add_profile_with_curves(&mut doc, al, "X", &pvis, &[VerticalCurve::ParabolaThrough(v(50.0, 30.0))], None).is_err());
        let curves = [
            (VerticalCurve::ParabolaK(0.5), (42.5, 57.5)),
            (VerticalCurve::ParabolaRadius(50.0), (42.5, 57.5)),
            (VerticalCurve::ParabolaThrough(v(50.0, 18.0)), (70.0 / 3.0, 230.0 / 3.0)),
            (VerticalCurve::CircularLength(15.0), (42.554880987, 57.554880987)),
            (VerticalCurve::CircularThrough(v(50.0, 18.0)), (23.673140622, 76.714991262)),
        ];
        let handles: Vec<Handle> = curves.iter().map(|(c, _)| add_profile_with_curves(&mut doc, al, "P", &pvis, &[*c], None).unwrap()).collect();
        let path = std::env::temp_dir().join(format!("civil-profile-curve-kinds-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        for (h, (_, (from, to))) in handles.iter().zip(&curves) {
            let pieces = match back.get_entity(*h) {
                Some(crate::entities::EntityType::Extended(x)) => match &x.data {
                    crate::entities::ExtendedEntityData::Arch(a) => a.profile_pieces(),
                    _ => panic!(),
                },
                _ => panic!(),
            };
            let curve = &pieces[1].1;
            assert!((curve[0].x - from).abs() < 1e-6 && (curve[curve.len() - 1].x - to).abs() < 1e-6);
        }
    }

    #[test]
    fn surface_profiles_name_their_surface() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        let pts = [(0.0, -10.0), (100.0, -10.0), (0.0, 10.0), (100.0, 10.0)].map(|(x, y)| crate::types::Vector3::new(x, y, 5.0));
        let s = crate::classes::civil::add_tin_surface(&mut doc, "S1", &pts, None).unwrap();
        assert!(add_surface_profile(&mut doc, al, al, "X", 0.0, None, None).is_err());
        assert!(add_surface_profile(&mut doc, al, s, "X", 0.0, Some((50.0, 150.0)), None).is_err());
        let eg = add_surface_profile(&mut doc, al, s, "EG", 5.0, Some((10.0, 90.0)), None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-surface-profile-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let arch = |h: Handle| match back.get_entity(h) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => a.clone(),
                _ => panic!(),
            },
            _ => panic!(),
        };
        let p = arch(eg);
        assert_eq!(p.profile_name().as_deref(), Some("EG"));
        let refs = p.typed_handles();
        assert_eq!((refs[12].0, refs[13].0), (al.value(), s.value()));
        // Listed by the alignment without an anchor.
        let a = arch(al);
        assert!(a.typed_handles().iter().any(|r| r.0 == eg.value()));
        assert!(a.alignment_anchors().unwrap().is_empty());
    }

    #[test]
    fn a_radius_parabola_reads_back_as_its_k() {
        // A radius of 500 is a K of 5 (grades in percent).
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        let k = add_profile_entities(&mut doc, al, "K", &[ProfileEntity::ParabolaTwoPointsK { start: v(0.0, 10.0), end: v(50.0, 11.0), k: 5.0, crest: true }], None).unwrap();
        let r = add_profile_entities(&mut doc, al, "R", &[ProfileEntity::ParabolaTwoPointsRadius { start: v(0.0, 10.0), end: v(50.0, 11.0), radius: 500.0, crest: true }], None).unwrap();
        let pieces = |h: Handle| match doc.get_entity(h) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => a.profile_pieces(),
                _ => panic!(),
            },
            _ => panic!(),
        };
        let (a, b) = (pieces(k), pieces(r));
        assert_eq!(a.len(), 1);
        assert!(a[0].1.iter().zip(&b[0].1).all(|(p, q)| p.distance(q) < 1e-9));
    }

    #[test]
    fn fixed_and_floating_parabolas_read_back() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        let t1 = ProfileEntity::Tangent { start: v(0.0, 10.0), end: v(40.0, 18.0) };
        assert!(add_profile_entities(&mut doc, al, "X", &[ProfileEntity::FloatingParabolaGrade { end: v(60.0, 19.0), grade: -0.1 }], None).is_err());
        let three = add_profile_entities(&mut doc, al, "P3", &[t1, ProfileEntity::ParabolaThreePoints { start: v(40.0, 18.0), middle: v(50.0, 19.5), end: v(60.0, 19.0) }, ProfileEntity::Tangent { start: v(60.0, 19.0), end: v(100.0, 15.0) }], None).unwrap();
        let floating = add_profile_entities(&mut doc, al, "FK", &[t1, ProfileEntity::FloatingParabolaK { end: v(60.0, 19.0), k: 0.5, crest: true }], None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-profile-entities-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let pieces = |h: Handle| match back.get_entity(h) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => a.profile_pieces(),
                _ => panic!(),
            },
            _ => panic!(),
        };
        let p = pieces(three);
        assert_eq!(p.len(), 3);
        assert!(p[1].1[0].distance(&v(40.0, 18.0)) < 1e-9 && p[2].1[0].distance(&v(60.0, 19.0)) < 1e-9);
        // The floating parabola leaves the tangent where its K brings it:
        // L = K * 100 * |g2 - g1| with the grade at its end.
        let p = pieces(floating);
        assert!((p[1].1[0].x - 42.679491924).abs() < 1e-6);
    }

    #[test]
    fn offset_profiles_copy_their_source_cut_at_the_offset_elements() {
        use crate::classes::civil::{add_alignment_with_curves, add_offset_alignment, VertexCurve};
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = add_alignment_with_curves(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0), v(200.0, 50.0)], &[VertexCurve::Arc(80.0)], None).unwrap();
        let fg = add_profile(&mut doc, al, "FG", &[v(0.0, 10.0), v(85.0, 15.0), v(200.0, 13.0)], &[30.0], None).unwrap();
        let off = add_offset_alignment(&mut doc, al, "OFF", -10.0, None, None).unwrap();
        let op = add_offset_profile(&mut doc, fg, off, "OP", 0.02, None).unwrap();
        let path = std::env::temp_dir().join(format!("acadrust_offset_profile_{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let EntityType::Extended(x) = back.get_entity(op).unwrap() else { panic!() };
        let ExtendedEntityData::Arch(a) = &x.data else { panic!() };
        // The parabola crossing the arc's start is cut in two (OB01).
        let o = a.profile_offset().unwrap();
        assert_eq!((o.source, o.slope, o.offset), (fg.value(), 0.02, -10.0));
        assert_eq!(o.entity_map, vec![(1, vec![1]), (2, vec![4, 5]), (3, vec![2, 3])]);
        let pvis = a.profile_pvis();
        assert!((pvis[0].point.y - 10.2).abs() < 1e-12);
        assert!((pvis.last().unwrap().point.x - 195.3635239099919).abs() < 1e-9);
        assert_eq!(x.common.reactors.len(), 1);
        assert!(identifier(&back, op.value()).is_some());
    }

    /// Circular curves and widenings (OX01): the circle is cut where the
    /// widening's regions change, a piece along a transition is a parabola
    /// by two points and a grade, a tangent's piece there is listed under
    /// id 0; a static copy keeps the entities.
    #[test]
    fn offset_profiles_over_widenings_and_their_static_copies() {
        use crate::classes::civil::{add_alignment_with_curves, add_offset_alignment_with_widenings, ProfilePiece, VertexCurve, Widening};
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = add_alignment_with_curves(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0), v(200.0, 50.0)], &[VertexCurve::Arc(80.0)], None).unwrap();
        let fgr = add_profile_with_curves(&mut doc, al, "FGR", &[v(0.0, 5.0), v(120.0, 9.0), v(200.0, 2.0)], &[VerticalCurve::Circular(800.0)], None).unwrap();
        let fga = add_profile(&mut doc, al, "FGA", &[v(0.0, 10.0), v(80.0, 16.0), v(200.0, 12.0)], &[20.0], None).unwrap();
        let off = add_offset_alignment_with_widenings(&mut doc, al, "OFFW", 10.0, None, &[Widening { start: 120.0, end: 160.0, offset: 15.0, entry: 8.0, exit: 6.0 }], None).unwrap();
        let circle = add_offset_profile(&mut doc, fgr, off, "OPC", 0.04, None).unwrap();
        let line = add_offset_profile(&mut doc, fga, off, "OPA", 0.04, None).unwrap();
        let copy = add_static_profile(&mut doc, circle, "SFC", None).unwrap();
        assert!(add_static_profile(&mut doc, fga, "X", None).is_err());
        let path = std::env::temp_dir().join(format!("acadrust_offset_widening_{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let pieces = |h| arch(&back, h).profile_pieces().iter().map(|p| p.0).collect::<Vec<_>>();
        use ProfilePiece::{Circular as C, Parabola as P, Tangent as T};
        assert_eq!(pieces(circle), [T, C, C, C, P, C, P, C, T]);
        assert_eq!(pieces(copy), pieces(circle));
        let o = arch(&back, line).profile_offset().unwrap();
        assert_eq!(o.entity_map[0], (0, vec![6, 8]));
        // The widened stretch is raised by the widened offset.
        let p = arch(&back, line).profile_pieces();
        let end = p[6].1[0];
        assert!((end.x - 134.07045722206465).abs() < 1e-9);
        assert!((end.y - (16.0 - 4.0 / 120.0 * 48.0 + 0.04 * 15.0)).abs() < 1e-9);
    }

    /// The offset profile's entities lie on the mapped curve: where the
    /// offset is constant along a parent line every point of a copied
    /// circle or asymmetric parabola is the source's raised by the rise, an
    /// asymmetric parabola is cut at its PVI, and a source tangent's piece
    /// along a transition is a straight line.
    #[test]
    fn offset_profile_entities_lie_on_the_mapped_curve() {
        use super::super::super::profile::entity_points;
        use crate::classes::civil::{add_alignment_with_curves, add_offset_alignment_with_widenings, VertexCurve, Widening};
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = add_alignment_with_curves(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0), v(200.0, 50.0)], &[VertexCurve::Arc(80.0)], None).unwrap();
        let fgc = add_profile_with_curves(&mut doc, al, "FGC", &[v(0.0, 5.0), v(40.0, 9.0), v(200.0, 2.0)], &[VerticalCurve::Circular(300.0)], None).unwrap();
        let fgs = add_profile_with_curves(&mut doc, al, "FGS", &[v(0.0, 5.0), v(50.0, 9.0), v(200.0, 2.0)], &[VerticalCurve::Asymmetric { before: 20.0, after: 25.0 }], None).unwrap();
        let fga = add_profile(&mut doc, al, "FGA", &[v(0.0, 10.0), v(200.0, 12.0)], &[], None).unwrap();
        let off = add_offset_alignment_with_widenings(&mut doc, al, "OFFW", 10.0, None, &[Widening { start: 130.0, end: 160.0, offset: 15.0, entry: 8.0, exit: 6.0 }], None).unwrap();
        let rise = 0.04 * 10.0;
        let written: Vec<Handle> = [fgc, fgs, fga].iter().map(|s| add_offset_profile(&mut doc, *s, off, "OP", 0.04, None).unwrap()).collect();
        let curves = |h: Handle| {
            let a = arch(&doc, h);
            let (data, start, end) = a.main_stream().unwrap();
            entity_points(data, start, end).unwrap()
        };
        // Before the parent's arc (81.1) the offset alignment's stations are
        // the parent's.
        for (source, h) in [(fgc, written[0]), (fgs, written[1])] {
            let src = curves(source);
            let z = |x: f64| src.iter().find(|e| e.2[0].x <= x && x <= e.2[2].x).unwrap().3.elevation_at(x);
            for e in curves(h).iter().filter(|e| e.2[2].x < 81.0) {
                for t in [0.25, 0.5, 0.75] {
                    let x = e.2[0].x + t * (e.2[2].x - e.2[0].x);
                    assert!((e.3.elevation_at(x) - (z(x) + rise)).abs() < 1e-9, "{x}");
                }
            }
        }
        assert!(curves(written[1]).iter().any(|e| (e.2[2].x - 50.0).abs() < 1e-9), "cut at the PVI");
        // The tangent's pieces along the transitions are straight.
        let pieces = curves(written[2]);
        assert_eq!(pieces.iter().filter(|e| e.1 == crate::classes::civil::ProfilePiece::Parabola).count(), 2);
        for e in pieces.iter().filter(|e| e.1 == crate::classes::civil::ProfilePiece::Parabola) {
            let [a, _, b] = e.2;
            let x = (a.x + b.x) / 2.0;
            assert!((e.3.elevation_at(x) - (a.y + b.y) / 2.0).abs() < 1e-9);
        }
    }

    #[test]
    fn floating_and_free_tangents_touch_their_parabolas() {
        use crate::classes::civil::ProfilePiece;
        let mut doc = drawing();
        let v = Vector2::new;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(200.0, 0.0)], None).unwrap();
        let p1 = ProfileEntity::ParabolaThreePoints { start: v(0.0, 10.0), middle: v(25.0, 12.0), end: v(50.0, 11.0) };
        let p2 = ProfileEntity::ParabolaThreePoints { start: v(120.0, 15.0), middle: v(150.0, 16.0), end: v(180.0, 15.0) };
        assert!(add_profile_entities(&mut doc, al, "X", &[ProfileEntity::Tangent { start: v(0.0, 10.0), end: v(50.0, 11.0) }, ProfileEntity::FloatingTangent { through: v(150.0, 14.0) }], None).is_err());
        assert!(add_profile_entities(&mut doc, al, "X", &[p1, ProfileEntity::FreeTangent], None).is_err());
        let floating = add_profile_entities(&mut doc, al, "P4", &[p1, ProfileEntity::FloatingTangent { through: v(150.0, 14.0) }], None).unwrap();
        let free = add_profile_entities(&mut doc, al, "P5", &[p1, ProfileEntity::FreeTangent, p2], None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-profile-tangents-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        // The tangents leave the parabolas where they touch them (the
        // application's stations: 25.836 and 22.102 to 134.741).
        let floating = arch(&back, floating).profile_pieces();
        assert_eq!(floating.iter().map(|p| p.0).collect::<Vec<_>>(), [ProfilePiece::Parabola, ProfilePiece::Tangent]);
        assert!((floating[1].1[0].x - 25.836129785405483).abs() < 1e-9);
        let free = arch(&back, free).profile_pieces();
        assert_eq!(free.iter().map(|p| p.0).collect::<Vec<_>>(), [ProfilePiece::Parabola, ProfilePiece::Tangent, ProfilePiece::Parabola]);
        let tangent = &free[1].1;
        assert!((tangent[0].x - 22.102416604041473).abs() < 1e-9 && (tangent[tangent.len() - 1].x - 134.74121986472957).abs() < 1e-9);
    }
}
