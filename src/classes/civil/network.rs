//! Pipe network parts: pipe ends, structure positions and the sizes their
//! part definitions carry.
//!
//! A part definition (`AECC_NETWORK_PART_DEF`, class version 80) holds its
//! data fields as a part record, in the order of the
//! `AeccNetworkPartDataField` groups of its text stream:
//!
//! ```text
//! strings: "AeccNetworkPartDataField", "", code, description, context, unit[, text value]
//! record:  BL 80, BL 0, BL 80, BL 80, 9 bits (0), BS 0x04BE, BL 80, BL 80,
//!          BL n, n x { BL 0, field }
//! field:   BL 80, BS 0x04BE, BL index, BS kind, 2 bits, BL 0, RC, RC type,
//!          B, value, BL 1 (2 in pressure part definitions)
//! value:   type 0x80 (number): B set, BD
//!          type 0x81 (flag or integer): B set, B; B clear, BL
//!          type 0x82 (text, kept in the text stream): B clear, nothing
//! ```
//!
//! The index counts the fields that share a context (`BdyD1`, `BdyD2`).
//!
//! A pipe (`AECC_PIPE`) and a structure (`AECC_STRUCTURE`) start alike:
//!
//! ```text
//! class header and keys (super::alignment), 3 bits (010), GUID
//! structure, pressure fitting, appurtenance: 3BD insertion point,
//!            3 x 3BD axes (X, Y, Z; the rotation is the X axis's angle)
//! pipe:      26 bits, BL 80, BS 0x04BE, BL 80, BL 80, BL n,
//!            n x { BL 0, field }, BL connectors,
//!            3 x BL, GUID, 1 bit, 8 x BD, values, 5 x BD, 1 bit,
//!            3BD start, 3BD end
//! values:    B, BL k, k x 40 bits, BL, BL, BL m,
//!            m x { 2 bits, BL key (type 0x02), BD, 4 bits }
//! ```
//!
//! A pipe the application wrote before it cached its derived values (slope,
//! lengths, read back as the BDs of the values) has `k = m = 0`.
//!
//! A pressure pipe (`AECC_PRESSUREPIPE`) has the same head, then:
//!
//! ```text
//! 36 bits, 2 x port, 18 bits, 3BD start, 3BD end, BD bulge
//! port: 3BD position, 3BD direction, BD engagement length,
//!       BL port index of the part it connects to (-1 when open),
//!       BD outside diameter, BD nominal diameter, BD wall thickness,
//!       BD deflection, BD, 8 bits
//! references: ..., start's part, end's part, one more
//! ```
//!
//! The ports are the pipe's ends (the start's direction points back along
//! the pipe); the bulge is 0 for a straight pipe.
//!
//! The pipe's fields are the part data the pipe keeps itself, in the order
//! of its text stream's `AeccNetworkPartDataField` groups. A field's kind is
//! 101 for a unitless number, 104 for a length and 301 for a text.

use super::alignment::{class_header, key, KEYED};
use super::bits::Bits;
use crate::types::Vector3;

/// One number field of a pipe network part definition.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PartField {
    /// Short code (`PID`, `WTh`, `SBSD`, ...).
    pub code: String,
    /// Context name (`PipeInnerDiameter`, `WallThickness`, ...).
    pub context: String,
    /// Unit the value is in (`mm`, `inch`, ...); empty for unitless fields.
    pub unit: String,
    pub value: f64,
}

/// One data field of a part record (see the module docs), read after the
/// list's `BL 0`: its number (`None` for a flag, integer or text field)
/// and, for a number, the bit range of its `BD`.
fn data_field(bits: &mut Bits) -> Option<(Option<f64>, (usize, usize))> {
    (bits.bit_long()? == 80 && bits.bit_short()? == 0x04BE).then_some(())?;
    bits.bit_long()?;
    bits.bit_short()?;
    bits.skip(2)?;
    bits.bit_long()?;
    bits.bits_value(8)?;
    let kind = bits.bits_value(8)?;
    let set = bits.bit()?;
    let at = bits.pos;
    let number = match (kind, set) {
        (0x80, true) => Some(bits.bit_double()?),
        (0x81, true) => {
            bits.bit()?;
            None
        }
        (0x81, false) => {
            bits.bit_long()?;
            None
        }
        (0x82, false) => None,
        _ => return None,
    };
    let span = (at, bits.pos);
    matches!(bits.bit_long()?, 1 | 2).then_some((number, span))
}

/// `BL n, n x { BL 0, field }`: each field's number and `BD` span.
fn data_fields_of(bits: &mut Bits) -> Option<Vec<(Option<f64>, (usize, usize))>> {
    let n = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 60 <= bits.remaining())?;
    (0..n)
        .map(|_| {
            (bits.bit_long()? == 0).then_some(())?;
            data_field(bits)
        })
        .collect()
}

/// The fields of a part definition record (see the module docs).
fn definition_fields(data: &[u8], start: usize, end: usize) -> Option<Vec<(Option<f64>, (usize, usize))>> {
    let mut bits = Bits::new(data, start, end);
    for want in [80, 0, 80, 80] {
        (bits.bit_long()? == want).then_some(())?;
    }
    (bits.bits_value(8)? == 0 && !bits.bit()?).then_some(())?;
    (bits.bit_short()? == 0x04BE && bits.bit_long()? == 80 && bits.bit_long()? == 80).then_some(())?;
    data_fields_of(&mut bits)
}

pub(super) fn part_fields(data: &[u8], start: usize, end: usize, strings: &[String]) -> Vec<PartField> {
    number_field_slots(data, start, end, strings)
        .into_iter()
        .map(|(code, context, unit, _, value)| PartField { code, context, unit, value })
        .collect()
}

/// The number fields of a part definition, with the bit range of each
/// value's `BD`: (code, context, unit, range, value). Empty when the record
/// is not laid out as the module docs describe or its fields do not match
/// the text stream's groups.
pub(super) fn number_field_slots(data: &[u8], start: usize, end: usize, strings: &[String]) -> Vec<(String, String, String, (usize, usize), f64)> {
    let Some(fields) = definition_fields(data, start, end) else {
        return Vec::new();
    };
    let groups = data_fields(strings);
    if groups.len() != fields.len() {
        return Vec::new();
    }
    groups
        .into_iter()
        .zip(fields)
        .filter_map(|((code, context, unit, _), (number, span))| Some((code, context, unit, span, number?)))
        .collect()
}

/// The `AeccNetworkPartDataField` groups of a text stream: (code, context,
/// unit, whether a text value follows the unit).
fn data_fields(strings: &[String]) -> Vec<(String, String, String, bool)> {
    (0..strings.len().saturating_sub(5))
        .filter(|&i| strings[i] == "AeccNetworkPartDataField")
        .map(|i| {
            let text_value = strings.get(i + 6).is_some_and(|s| s != "AeccNetworkPartDataField");
            (strings[i + 2].clone(), strings[i + 4].clone(), strings[i + 5].clone(), text_value)
        })
        .collect()
}

/// Class header, keys, 3 bits and the GUID a pipe and a structure start
/// with.
fn part_head(bits: &mut Bits) -> Option<()> {
    (class_header(bits)? == KEYED).then_some(())?;
    for _ in 0..bits.bits_value(8)? {
        key(bits)?;
    }
    (bits.bits_value(3)? == 0b010).then_some(())?;
    bits.skip(128)
}

fn point(bits: &mut Bits) -> Option<Vector3> {
    let p = Vector3::new(bits.bit_double()?, bits.bit_double()?, bits.bit_double()?);
    [p.x, p.y, p.z].into_iter().all(f64::is_finite).then_some(p)
}

/// A structure's (or pressure fitting's or appurtenance's) rotation about
/// its Z axis: the angle of the X axis stored after the insertion point.
pub(super) fn part_rotation(data: &[u8], start: usize, end: usize) -> Option<f64> {
    let mut bits = Bits::new(data, start, end);
    part_head(&mut bits)?;
    point(&mut bits)?;
    let x = point(&mut bits)?;
    Some(x.y.atan2(x.x))
}

/// A structure's insertion point, read by the record's fields.
pub(super) fn structure_at(data: &[u8], start: usize, end: usize) -> Option<Vector3> {
    let mut bits = Bits::new(data, start, end);
    part_head(&mut bits)?;
    point(&mut bits)
}

/// A pressure pipe: its ends, bulge and the joint sizes of its start.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PressurePipe {
    pub start: Vector3,
    pub end: Vector3,
    /// 0 for a straight pipe.
    pub bulge: f64,
    /// Joint sizes, in the units the record's text stream names (inch and
    /// degrees in the samples).
    pub outside_diameter: f64,
    pub nominal_diameter: f64,
    pub wall_thickness: f64,
    pub deflection: f64,
    /// Part the start and the end connect to: (handle, that part's port
    /// index); `None` for an open end.
    pub start_connection: Option<(u64, i32)>,
    pub end_connection: Option<(u64, i32)>,
}

/// A port of a pressure pipe, fitting or appurtenance: where it is, the
/// direction it faces (away from the part) and the port of the part it
/// connects to (`-1` for an open port).
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct PressurePort {
    pub position: Vector3,
    pub direction: Vector3,
    pub connected_port: i32,
    /// Joint sizes, in the units the record's text stream names (inch and
    /// degrees in the samples).
    pub outside_diameter: f64,
    pub nominal_diameter: f64,
    pub wall_thickness: f64,
    pub deflection: f64,
    /// Bit range of `connected_port` in the record.
    pub(crate) connected_at: (usize, usize),
}

/// The ports of a pressure part, with the bits left after the last.
///
/// ```text
/// pipe:   head, 36 bits, 2 ports (each followed by 8 bits)
/// others: head, 3BD insertion point, 3 x 3BD axes, BL ports, 2 bits,
///         ports (each followed by 8 bits, the last by 2)
/// port:   3BD position, 3BD direction, BD engagement length,
///         BL connected port, 4 x BD sizes, BD
/// ```
fn ports_of<'a>(data: &'a [u8], start: usize, end: usize, pipe: bool) -> Option<(Vec<PressurePort>, Bits<'a>)> {
    let mut bits = Bits::new(data, start, end);
    part_head(&mut bits)?;
    let n = if pipe {
        bits.skip(36)?;
        2
    } else {
        for _ in 0..4 {
            point(&mut bits)?;
        }
        let n = usize::try_from(bits.bit_long()?).ok().filter(|n| (1..=16).contains(n))?;
        bits.skip(2)?;
        n
    };
    let mut ports = Vec::with_capacity(n);
    for k in 0..n {
        let (position, direction) = (point(&mut bits)?, point(&mut bits)?);
        bits.bit_double()?;
        let at = bits.pos;
        let connected_port = bits.bit_long()?;
        let connected_at = (at, bits.pos);
        let [outside_diameter, nominal_diameter, wall_thickness, deflection] = [(); 4].map(|_| bits.bit_double());
        bits.bit_double()?;
        bits.skip(if pipe || k + 1 < n { 8 } else { 2 })?;
        ports.push(PressurePort {
            position,
            direction,
            connected_port,
            outside_diameter: outside_diameter?,
            nominal_diameter: nominal_diameter?,
            wall_thickness: wall_thickness?,
            deflection: deflection?,
            connected_at,
        });
    }
    Some((ports, bits))
}

pub(crate) fn pressure_ports(data: &[u8], start: usize, end: usize, pipe: bool) -> Option<Vec<PressurePort>> {
    ports_of(data, start, end, pipe).map(|(p, _)| p)
}

// ponytail: the 36, 18 and 8-bit runs and the ports' fifth number (1 in
// every sample) are read past; both ports carry the same sizes.
pub(super) fn pressure_pipe(data: &[u8], start: usize, end: usize, handles: &[u64]) -> Option<PressurePipe> {
    let (ports, mut bits) = ports_of(data, start, end, true)?;
    bits.skip(18)?;
    let (a, b) = (point(&mut bits)?, point(&mut bits)?);
    let bulge = bits.bit_double().filter(|v| v.is_finite())?;
    let p = ports[0];
    // The connected parts are the third and second last references.
    let connected = |port: i32, from_end: usize| {
        let handle = *handles.len().checked_sub(from_end).and_then(|i| handles.get(i))?;
        (port >= 0 && handle != 0).then_some((handle, port))
    };
    Some(PressurePipe {
        start: a,
        end: b,
        bulge,
        outside_diameter: p.outside_diameter,
        nominal_diameter: p.nominal_diameter,
        wall_thickness: p.wall_thickness,
        deflection: p.deflection,
        start_connection: connected(ports[0].connected_port, 3),
        end_connection: connected(ports[1].connected_port, 2),
    })
}

/// The part record a pipe or structure keeps: its fields' numbers in record
/// order (`None` for other fields) and its connector count, with `bits`
/// left after the count. `count_at` receives the count's bit range.
///
/// ```text
/// BL 80, BS 0x04BE, BL 80, BL 80, BL n, n x { BL 0, field },
/// BL connectors (a pipe has none or two, one per end; a structure one per
/// connected pipe)
/// ```
fn part_record(bits: &mut Bits, count_at: &mut (usize, usize)) -> Option<(Vec<Option<f64>>, i32)> {
    (bits.bit_long()? == 80 && bits.bit_short()? == 0x04BE).then_some(())?;
    (bits.bit_long()? == 80 && bits.bit_long()? == 80).then_some(())?;
    let numbers = data_fields_of(bits)?.into_iter().map(|(number, _)| number).collect();
    let at = bits.pos;
    let connectors = bits.bit_long()?;
    (0..10_000).contains(&connectors).then_some(())?;
    *count_at = (at, bits.pos);
    Some((numbers, connectors))
}

/// Bit range and value of a pipe's or structure's connector count (see
/// [`part_record`]).
pub(crate) fn connector_count(data: &[u8], start: usize, end: usize, structure: bool) -> Option<(usize, usize, i32)> {
    let mut bits = Bits::new(data, start, end);
    part_head(&mut bits)?;
    if structure {
        for _ in 0..4 {
            point(&mut bits)?;
        }
        bits.skip(2)?;
    } else {
        bits.skip(26)?;
    }
    let mut at = (0, 0);
    let (_, count) = part_record(&mut bits, &mut at)?;
    Some((at.0, at.1, count))
}

/// The values a pipe caches after its first block of doubles (see the
/// module docs): `B, BL k, k x 40 bits, BL, BL, BL m, m x entry`, entry
/// `2 bits, BL key, BD, 4 bits`. Every key holds a `BD` (type 0x02).
// ponytail: the 40-bit runs and the bits around each entry's key are read
// past by their sizes (the same in every sample).
fn cached_values(bits: &mut Bits) -> Option<()> {
    bits.bit()?;
    let runs = usize::try_from(bits.bit_long()?).ok().filter(|k| k * 40 <= bits.remaining())?;
    bits.skip(40 * runs)?;
    bits.bit_long()?;
    bits.bit_long()?;
    let entries = usize::try_from(bits.bit_long()?).ok().filter(|m| m * 42 <= bits.remaining())?;
    for _ in 0..entries {
        bits.skip(2)?;
        (bits.bit_long()? as u32 >> 24 == 0x02).then_some(())?;
        bits.bit_double()?;
        bits.skip(4)?;
    }
    Some(())
}

/// A pipe read by its fields: the numbers of its part data fields in record
/// order (`None` for other fields), then its start and end.
fn pipe_record(data: &[u8], start: usize, end: usize) -> Option<(Vec<Option<f64>>, Vector3, Vector3)> {
    let mut bits = Bits::new(data, start, end);
    part_head(&mut bits)?;
    // ponytail: 26 bits, the same in every sample.
    bits.skip(26)?;
    let (numbers, connectors) = part_record(&mut bits, &mut (0, 0))?;
    (0..=2).contains(&connectors).then_some(())?;
    for _ in 0..3 {
        bits.bit_long()?;
    }
    bits.skip(128 + 1)?;
    for _ in 0..8 {
        bits.bit_double()?;
    }
    cached_values(&mut bits)?;
    for _ in 0..5 {
        bits.bit_double()?;
    }
    bits.skip(1)?;
    Some((numbers, point(&mut bits)?, point(&mut bits)?))
}

/// The part definition a pipe or structure record names: its handle
/// stream opens with four references (its layer's and style's hard
/// pointers, then soft and hard owner) and then names the
/// `AECC_NETWORK_PART_DEF` it applies, hard-pointer kind 5. The sizes a
/// record does not keep itself live in that definition (a pipe's
/// `PipeInnerDiameter`, a structure's `StructInnerDiameter`), so a pipe's
/// inverts are its ends lowered by half of its inner diameter.
pub(super) fn part_definition(refs: &[(u64, u8)]) -> Option<u64> {
    match refs {
        [(.., 5), (.., 5), (.., 4), (.., 3), (definition, 5), ..] => Some(*definition),
        _ => None,
    }
}

/// Number fields of the part data a pipe keeps itself (flow coefficients,
/// minimum curve radius and the like).
pub(super) fn pipe_fields(data: &[u8], start: usize, end: usize, strings: &[String]) -> Vec<PartField> {
    let Some((numbers, ..)) = pipe_record(data, start, end) else {
        return Vec::new();
    };
    let groups = data_fields(strings);
    if groups.len() != numbers.len() {
        return Vec::new();
    }
    groups
        .into_iter()
        .zip(numbers)
        .filter_map(|((code, context, unit, _), value)| Some(PartField { code, context, unit, value: value? }))
        .collect()
}

/// Start and end of a pipe, read by its fields (see the module docs).
pub(super) fn pipe_ends(data: &[u8], start: usize, end: usize) -> Option<(Vector3, Vector3)> {
    pipe_record(data, start, end).map(|(_, a, b)| (a, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bits as written, padded to whole bytes.
    fn bytes(bits: &str) -> Vec<u8> {
        let mut bits = bits.to_string();
        while bits.len() % 8 != 0 {
            bits.push('0');
        }
        bits.as_bytes()
            .chunks(8)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 2).unwrap())
            .collect()
    }

    fn raw(v: f64) -> String {
        v.to_le_bytes().iter().map(|b| format!("{b:08b}")).collect()
    }

    fn bd(v: f64) -> String {
        format!("00{}", raw(v))
    }

    fn rc(v: u8) -> String {
        format!("{v:08b}")
    }

    fn bl(v: u32) -> String {
        match v {
            0 => "10".to_string(),
            1..=255 => format!("01{}", rc(v as u8)),
            _ => format!("00{}", v.to_le_bytes().iter().map(|b| rc(*b)).collect::<String>()),
        }
    }

    fn bs(v: u16) -> String {
        match v {
            0 => "10".to_string(),
            1..=255 => format!("01{}", rc(v as u8)),
            _ => format!("00{}", v.to_le_bytes().iter().map(|b| rc(*b)).collect::<String>()),
        }
    }

    /// One part record field after the list's `BL 0`: kind, the two bits,
    /// the type and its value bits.
    fn field(kind: u16, two: &str, ty: u8, value: &str) -> String {
        format!("{}{}{}{}{}{two}{}{}{}{value}{}", bl(0), bl(80), bs(0x04BE), bl(0), bs(kind), bl(0), rc(8), rc(ty), bl(1))
    }

    fn group(code: &str, context: &str, unit: &str, text: Option<&str>) -> Vec<String> {
        let mut g: Vec<String> = ["AeccNetworkPartDataField", "", code, "", context, unit].map(String::from).to_vec();
        g.extend(text.map(String::from));
        g
    }

    #[test]
    fn part_definition_fields_read_by_their_types() {
        // A text, a raw number, a flag, an integer and a short-form number.
        let fields = [
            field(301, "00", 0x82, "0"),
            field(104, "10", 0x80, &format!("1{}", bd(450.0))),
            field(401, "00", 0x81, "10"),
            field(304, "00", 0x81, &format!("0{}", bl(4))),
            field(105, "10", 0x80, "101"),
        ];
        let bits = format!("{}{}{}{}000000000{}{}{}{}{}", bl(80), bl(0), bl(80), bl(80), bs(0x04BE), bl(80), bl(80), bl(5), fields.concat());
        let data = bytes(&bits);
        let mut strings = Vec::new();
        strings.extend(group("PrtNm", "Catalog_PartName", "", Some("Pipe")));
        strings.extend(group("PID", "PipeInnerDiameter", "mm", None));
        strings.extend(group("HPrtF", "Catalog_PartHide", "", None));
        strings.extend(group("CntTy", "Content_Type", "", None));
        strings.extend(group("WTh", "WallThickness", "mm", None));
        let fields = part_fields(&data, 0, bits.len(), &strings);
        let got: Vec<(&str, f64)> = fields.iter().map(|f| (f.code.as_str(), f.value)).collect();
        assert_eq!(got, [("PID", 450.0), ("WTh", 1.0)]);
        // The slots are the BD of each number: 66 bits raw, 2 bits short.
        let spans: Vec<usize> = number_field_slots(&data, 0, bits.len(), &strings).iter().map(|s| s.3 .1 - s.3 .0).collect();
        assert_eq!(spans, [66, 2]);
        // Fewer text groups than fields: not this layout.
        assert!(part_fields(&data, 0, bits.len(), &strings[..6]).is_empty());
    }

    #[test]
    fn a_pipe_names_its_part_definition_after_its_four_head_references() {
        let refs = [(4589u64, 5u8), (4843, 5), (0, 4), (0, 3), (5738, 5), (1243, 5), (4256, 4)];
        assert_eq!(part_definition(&refs), Some(5738));
        // Fewer references, or another shape, name nothing.
        assert_eq!(part_definition(&refs[..4]), None);
        assert_eq!(part_definition(&[(1, 5), (2, 5), (3, 5), (4, 3), (5, 5)]), None);
    }

    /// A pipe record: class header without keys, 010, GUID, 26 bits, its
    /// part record (one number field), the list's end, 3 x BL 0, GUID,
    /// 1 bit, 8 x BD 1.0, `values`, 5 x BD 0.0, 1 bit, then the ends.
    fn pipe(values: &str, ends: &[f64; 6]) -> String {
        let head = format!("{}{}{}{}{}{}010101{}010{}", bl(80), bl(0), bl(80), bl(80), bl(0), bl(0), rc(0), "0".repeat(128));
        let record = format!("{}{}{}{}{}{}", bl(80), bs(0x04BE), bl(80), bl(80), bl(1), field(104, "10", 0x80, &format!("1{}", bd(2.5))));
        let ends: String = ends.iter().map(|v| bd(*v)).collect();
        format!(
            "{head}{}{record}{}{}{}{}{}{values}{}0{ends}",
            "0".repeat(26),
            bl(0),
            [bl(0), bl(0), bl(0)].concat(),
            "0".repeat(129),
            "01".repeat(8),
            "",
            "10".repeat(5)
        )
    }

    #[test]
    fn pipe_record_reads_fields_and_ends() {
        // Without cached values.
        let values = format!("0{}{}{}{}", bl(0), bl(0), bl(0), bl(0));
        let bits = pipe(&values, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let data = bytes(&bits);
        let (numbers, a, b) = pipe_record(&data, 0, bits.len()).unwrap();
        assert_eq!(numbers, [Some(2.5)]);
        assert_eq!((a.x, a.y, a.z, b.x, b.y, b.z), (1.0, 2.0, 3.0, 4.0, 5.0, 6.0));
    }

    #[test]
    fn pipe_record_reads_past_cached_values() {
        // One 40-bit run and two cached doubles (a raw one and 0.0).
        let entry = |v: &str| format!("00{}{v}0010", bl(0x0200_404C));
        let values = format!("0{}{}{}{}{}{}{}", bl(1), "0".repeat(40), bl(0), bl(0), bl(2), entry(&bd(2.43)), entry("10"));
        let bits = pipe(&values, &[0.0, 0.0, -5.0, 100.0, 0.0, -6.0]);
        let data = bytes(&bits);
        let (a, b) = pipe_ends(&data, 0, bits.len()).unwrap();
        assert_eq!((a.x, a.y, a.z, b.x, b.y, b.z), (0.0, 0.0, -5.0, 100.0, 0.0, -6.0));
        // A cached value under another key type: not this layout.
        let values = format!("0{}{}{}{}00{}{}0010", bl(0), bl(0), bl(0), bl(1), bl(0x0400_404C), bd(2.43));
        let bits = pipe(&values, &[0.0, 0.0, -5.0, 100.0, 0.0, -6.0]);
        assert_eq!(pipe_ends(&bytes(&bits), 0, bits.len()), None);
    }
}
