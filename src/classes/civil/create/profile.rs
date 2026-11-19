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
/// (WR01 to WR09), ending with the version word.
// ponytail: not split into fields; name them once a profile varies them.
const TAIL_BITS: &str = "10001111111100001111000000000000000010100001000000010100000001101010100000000110001010100001011111000000100";

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

/// The identifier a profile record stores before its last 107 bits, a
/// version-4 GUID (the alignment lists its profiles by it).
fn identifier(doc: &CadDocument, profile: u64) -> Option<[u8; 16]> {
    let EntityType::Extended(x) = doc.get_entity(Handle::new(profile))? else { return None };
    let ExtendedEntityData::Arch(a) = &x.data else { return None };
    (a.dxf_name == "AECC_PROFILE").then_some(())?;
    let (data, start, end) = a.main_stream()?;
    let at = end.checked_sub(TAIL + 128).filter(|at| *at >= start)?;
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
fn new_identifier(seed: u64) -> [u8; 16] {
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
}

impl VerticalCurve {
    fn is_some(self) -> bool {
        self != VerticalCurve::None
    }

    /// The stations it takes before and after its PVI between the grades
    /// `g1` and `g2`, `None` when its values are not positive.
    fn reach(self, g1: f64, g2: f64) -> Option<(f64, f64)> {
        let positive = |v: f64| (v.is_finite() && v > 0.0).then_some(v);
        match self {
            VerticalCurve::None => Some((0.0, 0.0)),
            VerticalCurve::Parabola(l) => positive(l).map(|l| (l / 2.0, l / 2.0)),
            VerticalCurve::Asymmetric { before, after } => Some((positive(before)?, positive(after)?)),
            VerticalCurve::Circular(r) => {
                let (a1, a2) = (g1.atan(), g2.atan());
                let t = positive(r)? * ((a2 - a1).abs() / 2.0).tan();
                Some((t * a1.cos(), t * a2.cos()))
            }
        }
    }
}

/// One entity of the profile: a tangent from PVI to PVI, or a vertical
/// curve at an inner PVI.
enum Part {
    Tangent(Vector2, Vector2),
    Curve { curve: VerticalCurve, crest: bool },
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
#[allow(clippy::too_many_arguments)]
fn write_profile(doc: &mut CadDocument, common: EntityCommon, name: &str, parts: &[(i32, i32, i32, Part)], helpers: &[Handle; 7], style: u64, alignment: Handle, id: &[u8; 16]) -> Result<Handle, DxfError> {
    let byte = |w: &mut DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    let mut refs: Vec<Ref> = helpers.iter().map(|h| Ref(DwgReferenceType::HardOwnership, h.value())).collect();
    refs.extend([soft_pointer(0), hard_pointer(0), hard_pointer(0), soft_pointer(0), hard_pointer(style), hard_pointer(alignment.value()), soft_pointer(0), hard_pointer(0), soft_pointer(0), soft_pointer(0)]);
    let mut strings = vec!["AeccImpVAlignment".to_string(), String::new(), String::new()];
    strings.extend(KEYS.iter().filter_map(|(_, k)| match k {
        Key::Text(t) => Some(t.to_string()),
        _ => None,
    }));
    strings.push(name.to_string());
    let parabolas = parts.iter().filter(|p| matches!(p.3, Part::Curve { curve: VerticalCurve::Parabola(_), .. })).count();
    strings.extend((0..4 + parabolas).map(|_| String::new()));
    put_entity(
        doc,
        "AECC_PROFILE",
        common,
        |w| {
            for v in [80, 0, 80, 80, 0, 0x1002_5F04] {
                w.write_bit_long(v);
            }
            bits(w, "010101");
            byte(w, KEYS.len() as u8);
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
            }
            bl_byte(w, 0);
            bits(w, &"0".repeat(63));
            w.write_bit_double(2.0);
            bits(w, "011011010");
            w.write_bit_long(0);
            for v in [0.0, 0.0, 30.0] {
                w.write_bit_double(v);
            }
            for _ in 0..5 {
                w.write_bit_short(0);
            }
            bits(w, "0000001000");
            w.write_bit_long(parts.len() as i32);
            for (id, previous, next, part) in parts {
                let (prefix, constraint) = match part {
                    Part::Tangent(..) => (2, 0),
                    Part::Curve { curve: VerticalCurve::Circular(_), .. } => (4, 9),
                    Part::Curve { curve: VerticalCurve::Asymmetric { .. }, .. } => (8, 10),
                    Part::Curve { .. } => (6, 0),
                };
                bits(w, &format!("{prefix:011b}{:023b}", 0x08 << 13));
                for v in [constraint, *id, *previous, *next] {
                    w.write_bit_long(v);
                }
                match part {
                    Part::Tangent(a, b) => {
                        for v in [a.x, a.y, b.x, b.y] {
                            w.write_raw_double(v);
                        }
                        bits(w, "0000");
                    }
                    Part::Curve { curve, crest } => match *curve {
                        VerticalCurve::Parabola(length) => {
                            w.write_bit_long(1);
                            w.write_bit_double(length);
                            w.write_bit_long(if *crest { 0x1001 } else { 0x1000 });
                            w.write_bit_long(0);
                            bits(w, "010");
                            for _ in 0..3 {
                                w.write_bit_double(-1.0);
                            }
                        }
                        VerticalCurve::Circular(radius) => w.write_bit_double(radius),
                        VerticalCurve::Asymmetric { before, after } => {
                            w.write_bit_double(before);
                            w.write_bit_double(after);
                        }
                        VerticalCurve::None => {}
                    },
                }
            }
            bits(w, "10");
            w.write_bytes(id);
            bits(w, TAIL_BITS);
        },
        Body { refs, strings },
    )
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
        let r = c.reach(grade(j), grade(j + 1)).ok_or_else(|| invalid("a vertical curve needs positive values"))?;
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
        // Newest first.
        assert!(find(&ids[1]).unwrap() < find(&ids[0]).unwrap());
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
}
