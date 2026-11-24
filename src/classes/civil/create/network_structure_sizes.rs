//! Gravity structures of any catalog size.
//!
//! A structure size's part definition (`AECC_NETWORK_PART_DEF`) is its
//! family's definition with other field values: the writer starts from
//! the family's own size, sets the fields the caller chooses (from the
//! catalog's lists or ranges), recomputes the fields the catalog derives
//! from them, names the size and keys it by the key fields' values
//! (`1D412393942C400EAC7DCECB4D23AEE4SID60SRS60`). The application takes a
//! structure's extents from the blocks of its body, which the writer
//! scales from the family's.

use super::network::{body_refs, ensure_definition_with, place_with, write_structure_record, FamilyBody};
use super::network_families::STRUCTURES;
use super::support::invalid;
use crate::error::DxfError;
use crate::types::{Handle, Vector3};
use crate::CadDocument;

/// The values a size field of a structure family may take (inches).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum StructureSizeChoice {
    /// One of these.
    List(&'static [f64]),
    /// Any value from the first to the second.
    Range(f64, f64),
}

/// A structure family's size fields, the fields its key lists and how its
/// other fields and its size name follow from them (none when the catalog
/// rejects the size: a frame wider than the structure, a base taller than
/// the rim to sump height).
pub(super) struct StructureSizes {
    pub(super) family: &'static str,
    pub(super) key: &'static [&'static str],
    pub(super) choices: &'static [(&'static str, StructureSizeChoice)],
    pub(super) derive: fn(&mut Dims) -> Option<String>,
}

/// Field values of a size by code.
#[derive(Clone)]
pub(super) struct Dims(Vec<(String, f64)>);

impl Dims {
    fn g(&self, code: &str) -> f64 {
        self.0.iter().find(|(c, _)| c == code).map_or(0.0, |(_, v)| *v)
    }

    fn set(&mut self, code: &str, value: f64) {
        match self.0.iter_mut().find(|(c, _)| c == code) {
            Some(slot) => slot.1 = value,
            None => self.0.push((code.to_string(), value)),
        }
    }
}

fn bit(data: &[u8], p: usize) -> u8 {
    data[p / 8] >> (7 - p % 8) & 1
}

/// The raw double at bit `at`.
fn read_raw(data: &[u8], at: usize) -> f64 {
    let bytes: [u8; 8] = std::array::from_fn(|k| (0..8).fold(0u8, |b, i| b << 1 | bit(data, at + 8 * k + i)));
    f64::from_le_bytes(bytes)
}

/// The size fields of the structure family `family` (one of
/// [`super::network_part_families`] but the null structure) and the values
/// each takes, by code (`SID` inner diameter, `SRS` rim to sump height,
/// `WTh` wall thickness, ...; inches).
pub fn network_structure_size_fields(family: &str) -> Vec<(&'static str, StructureSizeChoice)> {
    STRUCTURE_SIZES.iter().find(|s| s.family == family).map(|s| s.choices.to_vec()).unwrap_or_default()
}

/// Adds a structure of the family `family` to `network` at `position`,
/// turned by `rotation` (radians) about Z, with the size fields `fields`
/// (code and value, see [`network_structure_size_fields`]; the fields not
/// given keep the family's own size). Its part definition:
///
/// ```text
/// the family's definition with the chosen values, the fields the catalog
/// derives from them (body and bounding sizes, offsets, clearances) and
/// the size name ("60 inch Cylindrical Structure")
/// key: family id, then the key fields' codes and values
/// body: the family's body scaled to the size's width, length and height
/// ```
///
/// The structure record is the family's (see
/// [`super::add_structure_of_family`]).
pub fn add_structure_of_size(doc: &mut CadDocument, network: Handle, family: &str, fields: &[(&str, f64)], position: Vector3, rotation: f64) -> Result<Handle, DxfError> {
    let sizes = STRUCTURE_SIZES.iter().find(|s| s.family == family).ok_or_else(|| invalid("no structure family of that name"))?;
    let part = STRUCTURES.iter().find(|f| f.name == family).ok_or_else(|| invalid("no structure family of that name"))?;
    part.record.ok_or_else(|| invalid("the family has no structure record"))?;
    if ![position.x, position.y, position.z, rotation].iter().all(|v| v.is_finite()) {
        return Err(invalid("a structure needs a finite position and rotation"));
    }
    for (code, value) in fields {
        let ok = match sizes.choices.iter().find(|(c, _)| c == code) {
            Some((_, StructureSizeChoice::List(values))) => values.iter().any(|v| (v - value).abs() < 1e-9),
            Some((_, StructureSizeChoice::Range(min, max))) => (*min..=*max).contains(value),
            None => return Err(invalid("not a size field of the family")),
        };
        if !ok {
            return Err(invalid("the value is not one of the family's"));
        }
    }

    // The family's own size, then the chosen and the derived values.
    let (bits, hex) = part.definition;
    let mut data: Vec<u8> = (0..hex.len()).step_by(2).map(|i| u8::from_str_radix(hex.get(i..i + 2).unwrap_or("0"), 16).unwrap_or(0)).collect();
    data.resize(bits.div_ceil(8) + 8, 0);
    let strings: Vec<String> = part.definition_strings.iter().map(|s| s.to_string()).collect();
    let slots = super::super::network::number_field_slots(&data, 0, bits, &strings);
    // A value is a bit double ending 8 bits before the next record: a raw
    // one (`00` and 64 bits) or the 2-bit form of 0 (`10`) or 1 (`01`).
    let slots: Vec<(String, usize, usize, f64)> = slots
        .into_iter()
        .map(|(code, _, _, at)| {
            let raw = read_raw(&data, at);
            if bit(&data, at - 2) | bit(&data, at - 1) == 0 && raw.is_finite() && (raw == 0.0 || (1e-9..1e12).contains(&raw.abs())) {
                (code, at - 2, 66, raw)
            } else {
                let short = (bit(&data, at + 62), bit(&data, at + 63));
                (code, at + 62, 2, if short == (0, 1) { 1.0 } else { 0.0 })
            }
        })
        .collect();
    let own = Dims(slots.iter().map(|(code, _, _, value)| (code.clone(), *value)).collect());
    let mut dims = own.clone();
    for (code, value) in fields {
        dims.set(code, *value);
    }
    let name = (sizes.derive)(&mut dims).ok_or_else(|| invalid("the catalog rejects the size"))?;
    if dims.0.iter().any(|(_, v)| !v.is_finite() || *v < 0.0) {
        return Err(invalid("the size gives a negative length"));
    }
    // Changed values are written raw.
    let mut out: Vec<u8> = Vec::new();
    let mut put = |b: u8, n: &mut usize| {
        if *n % 8 == 0 {
            out.push(0);
        }
        let last = out.len() - 1;
        out[last] |= b << (7 - *n % 8);
        *n += 1;
    };
    let (mut n, mut from) = (0usize, 0usize);
    for (code, at, len, value) in &slots {
        let new = dims.g(code);
        if new.to_bits() == value.to_bits() {
            continue;
        }
        for p in from..*at {
            put(bit(&data, p), &mut n);
        }
        put(0, &mut n);
        put(0, &mut n);
        for byte in new.to_le_bytes() {
            for i in 0..8 {
                put(byte >> (7 - i) & 1, &mut n);
            }
        }
        from = at + len;
    }
    for p in from..bits {
        put(bit(&data, p), &mut n);
    }
    let bits = n;
    let hex: String = out.iter().map(|b| format!("{b:02x}")).collect();

    let mut texts = strings;
    let i = texts.iter().position(|s| s == "Catalog_PartSizeName").ok_or_else(|| invalid("the family's definition lacks a size name"))?;
    texts[i + 2] = name;
    let number = |v: f64| if v.fract() == 0.0 { format!("{}", v as i64) } else { format!("{v}") };
    let key = format!("{}{}", part.family.replace('-', ""), sizes.key.iter().map(|c| format!("{c}{}", number(dims.g(c)))).collect::<String>());

    // The body, scaled along X by the width (or diameter), along Y by the
    // length (or diameter) and along Z by the height.
    // The application's bodies span the sizes in feet to five decimals.
    let scale = |a: &str, b: &str| {
        let pick = |d: &Dims| {
            let inches = if d.g(a) > 0.0 { d.g(a) } else { d.g(b) };
            (inches / 12.0 * 1e5).round() / 1e5
        };
        if pick(&own) > 0.0 { pick(&dims) / pick(&own) } else { 1.0 }
    };
    let (sx, sy, sz) = (scale("SBSW", "SBSD"), scale("SBSL", "SBSD"), scale("SBSH", "SBSH"));
    let body = match part.body {
        FamilyBody::Cylinder { radius, depth } => FamilyBody::Cylinder { radius: radius * sx, depth: depth * sz },
        FamilyBody::Box { min, max } => FamilyBody::Box { min: [min[0] * sx, min[1] * sy, min[2] * sz], max: [max[0] * sx, max[1] * sy, max[2] * sz] },
        FamilyBody::Sphere(r) => FamilyBody::Sphere(r),
        FamilyBody::None => FamilyBody::None,
    };
    let text_refs: Vec<&str> = texts.iter().map(String::as_str).collect();
    let definition = ensure_definition_with(doc, &key, (bits, &hex), &text_refs, |doc| body_refs(doc, &body))?;
    place_with(doc, network, part, true, Some(definition), |doc, common, refs, n| write_structure_record(doc, part, common, refs, n, position, rotation))
}

pub(super) const STRUCTURE_SIZES: &[StructureSizes] = &[
    StructureSizes {
        family: "CMP Rectangular End Section",
        key: &["SBSW", "SBSL", "SBSH"],
        choices: &[("SBSL", StructureSizeChoice::List(&[51.0, 60.0])), ("SBSW", StructureSizeChoice::List(&[152.0, 165.0])), ("SBSH", StructureSizeChoice::List(&[12.0, 15.0, 18.0, 21.0, 24.0, 30.0, 36.0, 42.0, 48.0, 54.0, 60.0, 66.0, 72.0, 78.0, 84.0]))],
        derive: |d| {
            d.set("BdyD1", d.g("SBSH"));
            d.set("BdyD2", d.g("SBSH"));
            d.set("BdyD3", d.g("SBSH")-2.0*d.g("LenA2"));
            d.set("LenA1", d.g("SBSH"));
            d.set("LenA3", d.g("SBSW"));
            d.set("SHBTh", d.g("SBSW"));
            d.set("SHBW", d.g("SBSL"));
            d.set("WPOf2", d.g("WPOf1"));
            d.set("WPOf3", d.g("SBSL"));
            Some(format!("{:.0} x {:.0} x {:.0} inch CMP Rectangular End Section", d.g("SBSL"), d.g("SBSW"), d.g("SBSH")))
        },
    },
    StructureSizes {
        family: "Concrete Rectangular Box Culvert Headwall",
        key: &["SHBTh", "SHBW", "SBSH", "LenA3", "LenA9", "LenA1", "WPOf2"],
        choices: &[("SHBTh", StructureSizeChoice::List(&[26.0, 28.0, 80.0, 40.0, 64.0])), ("SHBW", StructureSizeChoice::List(&[62.0, 66.0, 79.0, 93.0, 102.0, 154.0])), ("SBSH", StructureSizeChoice::List(&[35.0, 41.0, 56.0, 33.0, 39.0, 44.0, 50.0, 60.0])), ("LenA3", StructureSizeChoice::List(&[3.0, 6.0, 5.0, 4.0])), ("LenA9", StructureSizeChoice::List(&[20.0, 26.0, 20.0, 31.0, 28.0, 43.0, 15.0, 18.0])), ("LenA1", StructureSizeChoice::List(&[29.0, 31.0, 47.0, 70.0, 79.0])), ("WPOf2", StructureSizeChoice::List(&[3.0, 4.0, 5.0]))],
        derive: |d| {
            d.set("LenA2", d.g("SBSH"));
            d.set("LenA4", d.g("SHBW"));
            d.set("LenA5", d.g("SBSH"));
            d.set("LenA6", d.g("LenA3"));
            d.set("LenA7", d.g("LenA3"));
            d.set("LenA8", d.g("LenA3"));
            d.set("SBSL", d.g("SHBW"));
            d.set("SBSW", d.g("SHBTh"));
            d.set("WPOf1", 0.5*d.g("WPOf2"));
            d.set("WPOf3", d.g("SHBTh"));
            Some(format!("{:.0} x {:.0} x {:.0} inch Concrete Rectangular Box Culvert Headwall", d.g("SHBW"), d.g("SHBTh"), d.g("SBSH")))
        },
    },
    StructureSizes {
        family: "Concrete Flared End Section",
        key: &["SBSW", "SBSH", "SBSL"],
        choices: &[("SBSH", StructureSizeChoice::List(&[12.0, 15.0, 18.0, 21.0, 24.0, 30.0, 36.0, 42.0, 48.0, 54.0, 60.0, 72.0])), ("SBSW", StructureSizeChoice::List(&[67.0, 72.0])), ("SBSL", StructureSizeChoice::List(&[74.0, 98.0]))],
        derive: |d| {
            d.set("BdyD1", d.g("SBSH"));
            d.set("LenA1", d.g("SBSH"));
            d.set("LenA4", d.g("SBSW"));
            d.set("LenA6", d.g("SBSH"));
            d.set("SHBTh", d.g("SBSW"));
            d.set("SHBW", d.g("SBSL"));
            d.set("WPOf1", d.g("SBSL"));
            Some(format!("{:.0} inch Flared End Section  {:.0} x {:.0}", d.g("SBSH"), d.g("SBSW"), d.g("SBSL")))
        },
    },
    StructureSizes {
        family: "Concrete Rectangular Headwall Variable Height",
        key: &["SHBW", "SBSH"],
        choices: &[("SHBW", StructureSizeChoice::List(&[38.0, 44.0, 51.0, 56.0, 68.0, 79.0, 105.0])), ("SBSH", StructureSizeChoice::Range(12.0, 500.0))],
        derive: |d| {
            d.set("LenA1", d.g("SHBTh"));
            d.set("LenA2", d.g("SHBW"));
            d.set("LenB1", d.g("SBSH"));
            d.set("SBSL", d.g("SHBW"));
            d.set("SBSW", d.g("SHBTh"));
            Some(format!("{:.0} x {:.0} x {:.0} inch Concrete Rectangular Headwall VH", d.g("SHBW"), d.g("SHBTh"), d.g("SBSH")))
        },
    },
    StructureSizes {
        family: "Concrete Rectangular Headwall",
        key: &["SHBW", "SBSH"],
        choices: &[("SHBW", StructureSizeChoice::List(&[38.0, 44.0, 51.0, 56.0, 68.0, 79.0, 105.0])), ("SBSH", StructureSizeChoice::List(&[37.0, 44.0, 51.0, 57.0, 69.0, 80.0, 102.0]))],
        derive: |d| {
            d.set("LenA1", d.g("SHBTh"));
            d.set("LenA2", d.g("SHBW"));
            d.set("LenB1", d.g("SBSH"));
            d.set("SBSL", d.g("SHBW"));
            d.set("SBSW", d.g("SHBTh"));
            Some(format!("{:.0} x {:.0} x {:.0} inch Concrete Rectangular Headwall", d.g("SHBW"), d.g("SHBTh"), d.g("SBSH")))
        },
    },
    StructureSizes {
        family: "Concrete Rectangular Winged Headwall",
        key: &["SHBTh", "SHBW", "SBSH"],
        choices: &[("SHBTh", StructureSizeChoice::List(&[12.0])), ("SHBW", StructureSizeChoice::List(&[38.0, 54.0, 70.0])), ("SBSH", StructureSizeChoice::List(&[45.0, 55.0, 69.0, 77.0, 83.0]))],
        derive: |d| {
            d.set("LenA1", d.g("SHBTh"));
            d.set("LenA3", d.g("SHBTh"));
            d.set("LenA4", d.g("SHBTh"));
            d.set("LenA5", d.g("SHBW"));
            d.set("LenA7", d.g("LenA6"));
            d.set("LenA9", d.g("LenA8"));
            d.set("SBSL", d.g("SHBW"));
            d.set("SBSW", d.g("SHBTh"));
            d.set("WPOf3", d.g("SBSH")-d.g("WPOf4"));
            Some(format!("{:.0} x {:.0} x {:.0} inch Concrete Rectangular Winged Headwall", d.g("SHBW"), d.g("SHBTh"), d.g("SBSH")))
        },
    },
    StructureSizes {
        family: "Concentric Cylindrical Structure Rectangular Frame",
        key: &["SID", "SCH", "WTh", "FTh", "SFH", "SFL", "SFW", "SRS"],
        choices: &[("SID", StructureSizeChoice::List(&[48.0, 60.0, 72.0, 96.0])), ("SCH", StructureSizeChoice::List(&[24.0, 36.0, 48.0])), ("WTh", StructureSizeChoice::List(&[5.0, 6.0, 7.0, 8.0])), ("FTh", StructureSizeChoice::List(&[6.0, 7.0, 8.0, 10.0])), ("SFH", StructureSizeChoice::List(&[4.0, 6.0, 8.0])), ("SFL", StructureSizeChoice::List(&[18.0, 24.0, 30.0, 36.0, 48.0])), ("SFW", StructureSizeChoice::List(&[18.0, 24.0, 30.0, 36.0, 48.0])), ("SRS", StructureSizeChoice::Range(12.0, 6000.0))],
        derive: |d| {
            d.set("BdyD2", (d.g("SFL")*d.g("SFL")+d.g("SFW")*d.g("SFW")).sqrt()+2.0*d.g("WTh"));
            d.set("BdyD3", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("LenA1", d.g("SFL"));
            d.set("LenA2", d.g("SFW"));
            d.set("LenB2", d.g("SRS") + d.g("FTh") - (d.g("SCH") + d.g("SFH")));
            d.set("SBSD", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("SBSH", d.g("SRS") + d.g("FTh"));
            d.set("SVPC", d.g("SFH") + d.g("SCH") + d.g("SBPC"));
            d.set("WPOf1", d.g("SFH"));
            d.set("WPOf2", d.g("SFH") + d.g("SCH"));
            Some(format!("Concentric Structure {:.0} dia {:.0} x {:.0} frame {:.0} cone {:.0} wall {:.0} floor", d.g("SID"), d.g("SFL"), d.g("SFW"), d.g("SCH"), d.g("WTh"), d.g("FTh")))
        },
    },
    StructureSizes {
        family: "Concentric Cylindrical Structure",
        key: &["SID", "SCH", "WTh", "FTh", "SFD", "SFH", "SRS"],
        choices: &[("SID", StructureSizeChoice::List(&[48.0, 60.0, 72.0, 96.0])), ("SCH", StructureSizeChoice::List(&[24.0, 36.0, 48.0])), ("WTh", StructureSizeChoice::List(&[5.0, 6.0, 7.0, 8.0])), ("FTh", StructureSizeChoice::List(&[6.0, 7.0, 8.0, 10.0])), ("SFD", StructureSizeChoice::List(&[18.0, 24.0, 30.0, 36.0, 48.0])), ("SFH", StructureSizeChoice::List(&[4.0, 6.0, 8.0])), ("SRS", StructureSizeChoice::Range(12.0, 6000.0))],
        derive: |d| {
            d.set("BdyD1", d.g("SFD"));
            d.set("BdyD2", d.g("SFD") + (2.0 * d.g("WTh")));
            d.set("BdyD3", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("LenB1", d.g("SFH"));
            d.set("LenB2", d.g("SRS") + d.g("FTh") - (d.g("SCH") + d.g("SFH")));
            d.set("SBSD", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("SBSH", d.g("SRS") + d.g("FTh"));
            d.set("SVPC", d.g("SFH") + d.g("SCH") + d.g("SBPC"));
            d.set("WPOf1", d.g("SFH"));
            d.set("WPOf2", d.g("SFH") + d.g("SCH"));
            Some(format!("Concentric Structure {:.0} dia {:.0} frame {:.0} cone {:.0} wall {:.0} floor", d.g("SID"), d.g("SFD"), d.g("SCH"), d.g("WTh"), d.g("FTh")))
        },
    },
    StructureSizes {
        family: "Cylindrical Structure Slab Top Circular Frame",
        key: &["SID", "SFD", "SFH", "SSTh", "WTh", "FTh", "SRS"],
        choices: &[("SID", StructureSizeChoice::List(&[15.0, 18.0, 20.0, 24.0, 30.0, 36.0, 48.0, 54.0, 60.0, 72.0, 84.0, 96.0, 108.0, 120.0])), ("SFD", StructureSizeChoice::List(&[18.0, 24.0, 30.0, 36.0, 48.0])), ("SFH", StructureSizeChoice::List(&[4.0, 6.0, 8.0])), ("SSTh", StructureSizeChoice::List(&[4.0, 5.0, 6.0, 8.0, 10.0, 12.0])), ("WTh", StructureSizeChoice::List(&[3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0, 12.0])), ("FTh", StructureSizeChoice::List(&[4.0, 5.0, 6.0, 8.0, 10.0, 12.0])), ("SRS", StructureSizeChoice::Range(12.0, 6000.0))],
        derive: |d| {
            d.set("BdyD1", d.g("SFD"));
            d.set("BdyD2", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("LenB1", d.g("SFH"));
            d.set("LenB2", d.g("SRS") + d.g("FTh") - d.g("SFH"));
            d.set("SBSD", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("SBSH", d.g("SRS") + d.g("FTh"));
            d.set("SVPC", d.g("SFH") + d.g("SBPC"));
            d.set("WPOf1", d.g("SFH"));
            Some(format!("Slab Top Cylindrical Structure {:.0} dia {:.0} dia Frm {:.0} FrHt {:.0} Slab {:.0} Wall {:.0} Floor", d.g("SID"), d.g("SFD"), d.g("SFH"), d.g("SSTh"), d.g("WTh"), d.g("FTh")))
        },
    },
    StructureSizes {
        family: "Cylindrical Structure Slab Top Rectangular Frame",
        key: &["SID", "SFH", "SSTh", "WTh", "FTh", "SFL", "SFW", "SRS"],
        choices: &[("SID", StructureSizeChoice::List(&[15.0, 18.0, 20.0, 24.0, 30.0, 36.0, 48.0, 54.0, 60.0, 72.0, 84.0, 96.0, 108.0, 120.0])), ("SFH", StructureSizeChoice::List(&[4.0, 6.0, 8.0])), ("SSTh", StructureSizeChoice::List(&[4.0, 5.0, 6.0, 8.0, 10.0, 12.0])), ("WTh", StructureSizeChoice::List(&[3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0, 12.0])), ("FTh", StructureSizeChoice::List(&[4.0, 5.0, 6.0, 8.0, 10.0, 12.0])), ("SFL", StructureSizeChoice::List(&[10.0, 16.0, 22.0, 28.0, 40.0])), ("SFW", StructureSizeChoice::List(&[10.0, 16.0, 22.0, 28.0, 40.0])), ("SRS", StructureSizeChoice::Range(12.0, 6000.0))],
        derive: |d| {
            d.set("BdyD2", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("LenA1", d.g("SFL"));
            d.set("LenA2", d.g("SFW"));
            d.set("LenB2", d.g("SRS") + d.g("FTh") - d.g("SFH"));
            d.set("SBSD", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("SBSH", d.g("SRS") + d.g("FTh"));
            d.set("SVPC", d.g("SFH") + d.g("SBPC"));
            d.set("WPOf1", d.g("SFH"));
            Some(format!("Slab Top Rectagular Structure {:.0} dia {:.0} x {:.0} Frm {:.0} FrHt {:.0} Slab {:.0} Wall {:.0} Floor", d.g("SID"), d.g("SFL"), d.g("SFW"), d.g("SFH"), d.g("SSTh"), d.g("WTh"), d.g("FTh")))
        },
    },
    StructureSizes {
        family: "Eccentric Cylindrical Structure Rectangular Frame",
        key: &["SID", "SFL", "SFW", "WTh", "SCH", "FTh", "SFH", "SRS"],
        choices: &[("SID", StructureSizeChoice::List(&[48.0, 60.0, 72.0, 96.0])), ("SCH", StructureSizeChoice::List(&[24.0, 36.0, 48.0])), ("WTh", StructureSizeChoice::List(&[5.0, 6.0, 7.0, 8.0])), ("FTh", StructureSizeChoice::List(&[6.0, 7.0, 8.0, 10.0])), ("SFH", StructureSizeChoice::List(&[4.0, 6.0, 8.0])), ("SFL", StructureSizeChoice::List(&[18.0, 24.0, 30.0, 36.0, 48.0])), ("SFW", StructureSizeChoice::List(&[18.0, 24.0, 30.0, 36.0, 48.0])), ("SRS", StructureSizeChoice::Range(12.0, 6000.0))],
        derive: |d| {
            d.set("BdyD2", (d.g("SFL")*d.g("SFL")+d.g("SFW")*d.g("SFW")).sqrt() + (2.0 * d.g("WTh")));
            d.set("BdyD3", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("LenA1", (d.g("BdyD3") - d.g("BdyD2")) * 0.50);
            d.set("LenA2", d.g("SFL"));
            d.set("LenA3", d.g("SFW"));
            d.set("LenB2", d.g("SRS") + d.g("FTh") - (d.g("SCH") + d.g("SFH")));
            d.set("SBSD", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("SBSH", d.g("SRS") + d.g("FTh"));
            d.set("SVPC", d.g("SFH") + d.g("SCH") + d.g("SBPC"));
            d.set("WPOf1", d.g("SFH"));
            d.set("WPOf2", d.g("SFH") + d.g("SCH"));
            if !(d.g("SID") > d.g("BdyD2")) {
                return None;
            }
            Some(format!("Eccentric Structure {:.0} dia {:.0} x {:.0} frame {:.0} cone {:.0} wall {:.0} floor", d.g("SID"), d.g("SFL"), d.g("SFW"), d.g("SCH"), d.g("WTh"), d.g("FTh")))
        },
    },
    StructureSizes {
        family: "Eccentric Cylindrical Structure",
        key: &["SID", "SFD", "WTh", "SCH", "FTh", "SFH", "SRS"],
        choices: &[("SID", StructureSizeChoice::List(&[48.0, 60.0, 72.0, 96.0])), ("SCH", StructureSizeChoice::List(&[24.0, 36.0, 48.0])), ("WTh", StructureSizeChoice::List(&[5.0, 6.0, 7.0, 8.0])), ("FTh", StructureSizeChoice::List(&[6.0, 7.0, 8.0, 10.0])), ("SFD", StructureSizeChoice::List(&[18.0, 24.0, 30.0, 36.0, 48.0])), ("SFH", StructureSizeChoice::List(&[4.0, 6.0, 8.0])), ("SRS", StructureSizeChoice::Range(12.0, 6000.0))],
        derive: |d| {
            d.set("BdyD1", d.g("SFD"));
            d.set("BdyD2", d.g("SFD") + (2.0 * d.g("WTh")));
            d.set("BdyD3", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("LenA1", (d.g("BdyD3") - d.g("BdyD2")) * 0.50);
            d.set("LenB1", d.g("SFH"));
            d.set("LenB2", d.g("SRS") + d.g("FTh") - (d.g("SCH") + d.g("SFH")));
            d.set("SBSD", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("SBSH", d.g("SRS") + d.g("FTh"));
            d.set("SVPC", d.g("SFH") + d.g("SCH") + d.g("SBPC"));
            d.set("WPOf1", d.g("SFH"));
            d.set("WPOf2", d.g("SFH") + d.g("SCH"));
            if !(d.g("SID") > d.g("BdyD2")) {
                return None;
            }
            Some(format!("Eccentric Structure {:.0} dia {:.0} frame {:.0} cone {:.0} wall {:.0} floor", d.g("SID"), d.g("SFD"), d.g("SCH"), d.g("WTh"), d.g("FTh")))
        },
    },
    StructureSizes {
        family: "Rectangular Structure Slab Top Circular Frame",
        key: &["SIW", "SIL", "WTh", "FTh", "SFH", "SSTh", "SFD", "SRS"],
        choices: &[("SIW", StructureSizeChoice::List(&[15.0, 18.0, 24.0, 36.0, 48.0, 54.0, 60.0, 72.0, 84.0, 96.0, 108.0, 120.0, 144.0])), ("SIL", StructureSizeChoice::List(&[15.0, 18.0, 24.0, 36.0, 48.0, 54.0, 60.0, 72.0, 84.0, 96.0, 108.0, 120.0, 144.0])), ("WTh", StructureSizeChoice::List(&[3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0, 12.0])), ("FTh", StructureSizeChoice::List(&[4.0, 5.0, 6.0, 8.0, 10.0, 12.0])), ("SFH", StructureSizeChoice::List(&[4.0, 6.0, 8.0])), ("SSTh", StructureSizeChoice::List(&[2.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0])), ("SFD", StructureSizeChoice::List(&[24.0, 30.0, 36.0, 48.0])), ("SRS", StructureSizeChoice::Range(12.0, 9000.0))],
        derive: |d| {
            d.set("BdyD1", d.g("SFD"));
            d.set("LenA1", d.g("SIW") + (2.0 * d.g("WTh")));
            d.set("LenA2", d.g("SIL") + (2.0 * d.g("WTh")));
            d.set("LenB1", d.g("SRS") + d.g("FTh") - d.g("SFH"));
            d.set("LenB2", d.g("SFH"));
            d.set("SBSH", d.g("SRS") + d.g("FTh"));
            d.set("SBSL", d.g("SIL") + (2.0 * d.g("WTh")));
            d.set("SBSW", d.g("SIW") + (2.0 * d.g("WTh")));
            d.set("SVPC", d.g("SSTh") + d.g("SBPC"));
            d.set("WPOf1", d.g("SFH"));
            Some(format!("{:.0} x {:.0} Rect Structure {:.0} dia Frm {:.0} FmHt {:.0} Slab {:.0} Wall {:.0} Floor", d.g("SIL"), d.g("SIW"), d.g("SFD"), d.g("SFH"), d.g("SSTh"), d.g("WTh"), d.g("FTh")))
        },
    },
    StructureSizes {
        family: "Rectangular Structure Slab Top Rectangular Frame",
        key: &["SIW", "SIL", "WTh", "FTh", "SFH", "SSTh", "SFW", "SFL", "SRS"],
        choices: &[("SIW", StructureSizeChoice::List(&[15.0, 18.0, 24.0, 36.0, 48.0, 54.0, 60.0, 72.0, 84.0, 96.0, 108.0, 120.0, 144.0])), ("SIL", StructureSizeChoice::List(&[15.0, 18.0, 24.0, 36.0, 48.0, 54.0, 60.0, 72.0, 84.0, 96.0, 108.0, 120.0, 144.0])), ("WTh", StructureSizeChoice::List(&[3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 10.0, 12.0])), ("FTh", StructureSizeChoice::List(&[4.0, 5.0, 6.0, 8.0, 10.0, 12.0])), ("SFH", StructureSizeChoice::List(&[4.0, 6.0, 8.0])), ("SSTh", StructureSizeChoice::List(&[2.0, 4.0, 5.0, 6.0, 8.0, 10.0, 12.0])), ("SFW", StructureSizeChoice::List(&[12.0, 18.0, 24.0, 30.0, 36.0, 48.0])), ("SFL", StructureSizeChoice::List(&[12.0, 18.0, 24.0, 30.0, 36.0, 48.0])), ("SRS", StructureSizeChoice::Range(12.0, 6000.0))],
        derive: |d| {
            d.set("LenA1", d.g("SIW") + (2.0 * d.g("WTh")));
            d.set("LenA2", d.g("SIL") + (2.0 * d.g("WTh")));
            d.set("LenA3", d.g("SFW"));
            d.set("LenA4", d.g("SFL"));
            d.set("LenB1", d.g("SRS") + d.g("FTh") - d.g("SFH"));
            d.set("LenB2", d.g("SFH"));
            d.set("SBSH", d.g("SRS") + d.g("FTh"));
            d.set("SBSL", d.g("SIL") + (2.0 * d.g("WTh")));
            d.set("SBSW", d.g("SIW") + (2.0 * d.g("WTh")));
            d.set("SVPC", d.g("SSTh") + d.g("SBPC"));
            d.set("WPOf1", d.g("SFH"));
            Some(format!("{:.0} x {:.0} Rect Structure {:.0} x {:.0} Frm ", d.g("SIL"), d.g("SIW"), d.g("SFL"), d.g("SFW")))
        },
    },
    StructureSizes {
        family: "Eccentric Cylindrical 2-Tier Rectangular Frame",
        key: &["SID", "SRS", "SBH", "SCH", "SFL", "SFW", "WTh", "FTh", "SFH"],
        choices: &[("SID", StructureSizeChoice::List(&[72.0, 96.0, 120.0])), ("SCH", StructureSizeChoice::List(&[24.0, 36.0, 48.0])), ("WTh", StructureSizeChoice::List(&[6.0, 7.0, 8.0, 10.0])), ("FTh", StructureSizeChoice::List(&[7.0, 8.0, 10.0])), ("SFH", StructureSizeChoice::List(&[4.0, 6.0, 8.0])), ("SFL", StructureSizeChoice::List(&[24.0, 30.0, 36.0, 48.0])), ("SFW", StructureSizeChoice::List(&[24.0, 30.0, 36.0, 48.0])), ("SRS", StructureSizeChoice::Range(36.0, 9000.0)), ("SBH", StructureSizeChoice::Range(12.0, 9000.0))],
        derive: |d| {
            d.set("BdyD2", (d.g("SFL")*d.g("SFL")+d.g("SFW")*d.g("SFW")).sqrt()+2.0*d.g("WTh"));
            d.set("BdyD3", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("BdyD4", d.g("BdyD2"));
            d.set("LenA1", (d.g("SID") + (2.0 * d.g("WTh")) - d.g("BdyD2")) * 0.50);
            d.set("LenA2", d.g("SFL"));
            d.set("LenA3", d.g("SFW"));
            d.set("LenB2", d.g("SBH") + d.g("FTh"));
            d.set("SBSD", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("SBSH", d.g("SRS") + d.g("FTh"));
            d.set("SVPC", d.g("SBPC") + (d.g("SRS") - d.g("SBH")));
            d.set("WPOf1", d.g("SRS") - (d.g("SBH") + d.g("SCH")));
            d.set("WPOf2", d.g("SRS") - d.g("SBH"));
            d.set("WPOf3", d.g("SFH"));
            if !(d.g("SRS") > (d.g("SBH") + d.g("SCH"))) {
                return None;
            }
            if !(d.g("SID") > d.g("BdyD2")) {
                return None;
            }
            Some(format!("Eccentric Two Tier Structure {:.0} dia {:.0} x {:.0} frame {:.0} cone {:.0} wall {:.0} floor", d.g("SID"), d.g("SFL"), d.g("SFW"), d.g("SCH"), d.g("WTh"), d.g("FTh")))
        },
    },
    StructureSizes {
        family: "Eccentric Cylindrical 2-Tier Circular Frame",
        key: &["SID", "SRS", "SBH", "SCH", "WTh", "FTh", "SFD", "SFH"],
        choices: &[("SID", StructureSizeChoice::List(&[72.0, 96.0, 120.0])), ("SCH", StructureSizeChoice::List(&[24.0, 36.0, 48.0])), ("WTh", StructureSizeChoice::List(&[6.0, 7.0, 8.0, 10.0])), ("FTh", StructureSizeChoice::List(&[7.0, 8.0, 10.0])), ("SFD", StructureSizeChoice::List(&[24.0, 30.0, 36.0, 48.0])), ("SFH", StructureSizeChoice::List(&[4.0, 6.0, 8.0])), ("SRS", StructureSizeChoice::Range(36.0, 9000.0)), ("SBH", StructureSizeChoice::Range(12.0, 9000.0))],
        derive: |d| {
            d.set("BdyD1", d.g("SFD"));
            d.set("BdyD3", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("BdyD4", d.g("BdyD2"));
            d.set("LenA1", (d.g("SID") + (2.0 * d.g("WTh")) - d.g("BdyD2")) * 0.50);
            d.set("LenB1", d.g("SFH"));
            d.set("LenB2", d.g("SBH") + d.g("FTh"));
            d.set("SBSD", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("SBSH", d.g("SRS") + d.g("FTh"));
            d.set("SVPC", d.g("SBPC") + (d.g("SRS") - d.g("SBH")));
            d.set("WPOf1", d.g("SRS") - (d.g("SBH") + d.g("SCH")));
            d.set("WPOf2", d.g("SRS") - d.g("SBH"));
            d.set("WPOf3", d.g("SFH"));
            if !(d.g("SID") > d.g("BdyD2")) {
                return None;
            }
            if !(d.g("SRS") > (d.g("SBH") + d.g("SCH"))) {
                return None;
            }
            Some(format!("Eccentric Two Tier Structure {:.0} dia {:.0} frame {:.0} cone {:.0} wall {:.0} floor", d.g("SID"), d.g("SFD"), d.g("SCH"), d.g("WTh"), d.g("FTh")))
        },
    },
    StructureSizes {
        family: "Rectangular Structure 2-Tier Rectangular Frame",
        key: &["SIW", "SIL", "WTh", "FTh", "SFH", "SSTh", "SFL", "SFW", "SRS", "SBH"],
        choices: &[("SIW", StructureSizeChoice::List(&[48.0, 54.0, 60.0, 72.0, 84.0, 96.0, 108.0, 120.0, 144.0])), ("SIL", StructureSizeChoice::List(&[48.0, 54.0, 60.0, 72.0, 84.0, 96.0, 108.0, 120.0, 144.0])), ("WTh", StructureSizeChoice::List(&[6.0, 7.0, 8.0, 10.0, 12.0])), ("FTh", StructureSizeChoice::List(&[6.0, 7.0, 8.0, 10.0, 12.0])), ("SFH", StructureSizeChoice::List(&[4.0, 6.0, 8.0])), ("SSTh", StructureSizeChoice::List(&[4.0, 5.0, 6.0, 8.0, 10.0, 12.0])), ("SFL", StructureSizeChoice::List(&[18.0, 24.0, 30.0, 36.0, 48.0])), ("SFW", StructureSizeChoice::List(&[18.0, 24.0, 30.0, 36.0, 48.0])), ("SRS", StructureSizeChoice::Range(24.0, 9000.0)), ("SBH", StructureSizeChoice::Range(12.0, 9000.0))],
        derive: |d| {
            d.set("BdyD2", (d.g("SFL")*d.g("SFL")+d.g("SFW")*d.g("SFW")).sqrt()+2.0*d.g("WTh"));
            d.set("LenA1", d.g("SIW") + (2.0 * d.g("WTh")));
            d.set("LenA2", d.g("SIL") + (2.0 * d.g("WTh")));
            d.set("LenA3", d.g("SFL"));
            d.set("LenA4", d.g("SFW"));
            d.set("LenB1", d.g("SBH") + d.g("FTh"));
            d.set("LenB3", d.g("SRS") - d.g("SBH") - d.g("SFH"));
            d.set("SBSH", d.g("SRS") + d.g("FTh"));
            d.set("SBSL", d.g("SIL") + (2.0 * d.g("WTh")));
            d.set("SBSW", d.g("SIW") + (2.0 * d.g("WTh")));
            d.set("SVPC", d.g("SBPC") + (d.g("SRS") - d.g("SBH")));
            d.set("WPOf1", d.g("SRS") - d.g("SBH"));
            d.set("WPOf2", d.g("SFH"));
            Some(format!("{:.0} x {:.0} Rect Two Tier Structure {:.0} x {:.0} Frm {:.0} FmHt {:.0} Slab {:.0} Wall {:.0} Floor", d.g("SIL"), d.g("SIW"), d.g("SFL"), d.g("SFW"), d.g("SFH"), d.g("SSTh"), d.g("WTh"), d.g("FTh")))
        },
    },
    StructureSizes {
        family: "Rectangular Structure 2-Tier Circular Frame",
        key: &["SIW", "SIL", "WTh", "FTh", "SFH", "SSTh", "SFD", "SRS", "SBH"],
        choices: &[("SIW", StructureSizeChoice::List(&[48.0, 54.0, 60.0, 72.0, 84.0, 96.0, 108.0, 120.0, 144.0])), ("SIL", StructureSizeChoice::List(&[48.0, 54.0, 60.0, 72.0, 84.0, 96.0, 108.0, 120.0, 144.0])), ("WTh", StructureSizeChoice::List(&[6.0, 7.0, 8.0, 10.0, 12.0])), ("FTh", StructureSizeChoice::List(&[6.0, 7.0, 8.0, 10.0, 12.0])), ("SFH", StructureSizeChoice::List(&[4.0, 6.0, 8.0])), ("SSTh", StructureSizeChoice::List(&[4.0, 5.0, 6.0, 8.0, 10.0, 12.0])), ("SFD", StructureSizeChoice::List(&[18.0, 24.0, 30.0, 36.0, 48.0])), ("SRS", StructureSizeChoice::Range(24.0, 9000.0)), ("SBH", StructureSizeChoice::Range(12.0, 9000.0))],
        derive: |d| {
            d.set("BdyD1", d.g("SFD"));
            d.set("LenA1", d.g("SIW") + (2.0 * d.g("WTh")));
            d.set("LenA2", d.g("SIL") + (2.0 * d.g("WTh")));
            d.set("LenB1", d.g("SBH") + d.g("FTh"));
            d.set("LenB2", d.g("SFH"));
            d.set("LenB3", d.g("SRS") - d.g("SBH") - d.g("SFH"));
            d.set("SBSH", d.g("SRS") + d.g("FTh"));
            d.set("SBSL", d.g("SIL") + (2.0 * d.g("WTh")));
            d.set("SBSW", d.g("SIW") + (2.0 * d.g("WTh")));
            d.set("SVPC", d.g("SBPC") + (d.g("SRS") - d.g("SBH")));
            d.set("WPOf1", d.g("SRS") - d.g("SBH"));
            d.set("WPOf2", d.g("SFH"));
            Some(format!("{:.0} x {:.0} Rect Two Tier Structure {:.0} dia Frm {:.0} FmHt {:.0} Slab {:.0} Wall {:.0} Floor", d.g("SIL"), d.g("SIW"), d.g("SFD"), d.g("SFH"), d.g("SSTh"), d.g("WTh"), d.g("FTh")))
        },
    },
    StructureSizes {
        family: "Cylindrical Junction Structure NF",
        key: &["SID", "SRS"],
        choices: &[("SID", StructureSizeChoice::List(&[48.0, 60.0, 72.0, 96.0])), ("SRS", StructureSizeChoice::Range(12.0, 6000.0))],
        derive: |d| {
            d.set("BdyD1", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("SBSD", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("SBSH", d.g("SRS") + d.g("FTh"));
            d.set("LenB1", d.g("SBSH"));
            Some(format!("{:.0} inch Cylindrical Structure", d.g("SID")))
        },
    },
    StructureSizes {
        family: "Rectangular Junction Structure NF",
        key: &["SIW", "SIL", "SRS"],
        choices: &[("SIW", StructureSizeChoice::List(&[24.0, 36.0, 48.0, 54.0, 60.0, 72.0, 84.0, 96.0, 108.0, 120.0])), ("SIL", StructureSizeChoice::List(&[24.0, 36.0, 48.0, 54.0, 60.0, 72.0, 84.0, 96.0, 108.0, 120.0])), ("SRS", StructureSizeChoice::Range(12.0, 6000.0))],
        derive: |d| {
            d.set("LenA1", d.g("SIW") + (2.0 * d.g("WTh")));
            d.set("LenA2", d.g("SIL") + (2.0 * d.g("WTh")));
            d.set("LenB1", d.g("SRS") + d.g("FTh"));
            d.set("SBSH", d.g("SRS") + d.g("FTh"));
            d.set("SBSL", d.g("SIL") + (2.0 * d.g("WTh")));
            d.set("SBSW", d.g("SIW") + (2.0 * d.g("WTh")));
            Some(format!("{:.0} x {:.0} inch Rectangular Structure", d.g("SIW"), d.g("SIL")))
        },
    },
    StructureSizes {
        family: "Concentric Cylindrical Structure NF",
        key: &["SID", "SCH", "SRS"],
        choices: &[("SCH", StructureSizeChoice::List(&[24.0, 36.0, 48.0])), ("SID", StructureSizeChoice::List(&[48.0, 60.0, 72.0, 96.0])), ("SRS", StructureSizeChoice::Range(12.0, 6000.0))],
        derive: |d| {
            d.set("BdyD3", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("LenB2", d.g("SRS") + d.g("FTh") - d.g("SCH"));
            d.set("SBSD", d.g("SID") + (2.0 * d.g("WTh")));
            d.set("SBSH", d.g("SRS") + d.g("FTh"));
            d.set("SVPC", d.g("SCH") + d.g("SBPC"));
            d.set("WPOf2", d.g("SCH"));
            Some(format!("{:.0} inch dia {:.0} inch cone Concentric Structure", d.g("SID"), d.g("SCH")))
        },
    },
];
