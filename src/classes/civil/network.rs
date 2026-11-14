//! Pipe network parts: pipe ends, structure positions and the sizes their
//! part definitions carry.
//!
//! A part definition (`AECC_NETWORK_PART_DEF`) stores one record per data
//! field, in the order of the fields in its text stream:
//!
//! ```text
//! strings: "AeccNetworkPartDataField", "", code, description, context, unit[, text value]
//! record:  marker ... BD value (number fields), 8 bits, next record's marker
//! ```
//!
//! The marker carries the class version (`... 80 ...` from 2018, `... 70
//! ...` in Civil 3D 2013 drawings), so a number field's value is the raw
//! double that ends 8 bits before the next record's marker. Text fields
//! keep their value in the text stream.
//!
//! A pipe (`AECC_PIPE`) and a structure (`AECC_STRUCTURE`) start alike:
//!
//! ```text
//! class header and keys (super::alignment), 3 bits (010), GUID
//! structure: 3BD insertion point
//! pipe:      26 bits, BL 80, BS 0x04BE, BL 80, BL 80, BL n, BL 0,
//!            n x { field, BL separator (0, then 1 or 2 after the last) },
//!            3 x BL, GUID, 1 bit, 17 x BD, 2 bits, 3BD start, 3BD end
//! field:     BL 80, BS 0x04BE, BL 0, BS kind, BL, BL, 16 bits,
//!            B has number, [BD number], BL 1
//! ```
//!
//! The pipe's fields are the part data the pipe keeps itself, in the order
//! of its text stream's `AeccNetworkPartDataField` groups. A field's kind is
//! 101 for a unitless number, 104 for a length and 301 for a text, whose
//! value is in the text stream.

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

/// Record markers of the 2018 and the 2013 class versions.
const MARKERS: [&[u8]; 2] = [b"01100101010000001011111", b"01100101000110000000100"];

pub(super) fn part_fields(data: &[u8], start: usize, end: usize, strings: &[String]) -> Vec<PartField> {
    let bit = |p: usize| (data[p / 8] >> (7 - p % 8)) & 1;
    let mut markers = Vec::new();
    for pos in start..end {
        if MARKERS.iter().any(|m| pos + m.len() <= end && m.iter().enumerate().all(|(i, c)| bit(pos + i) == c - b'0')) {
            markers.push(pos);
        }
    }
    // Record `k` ends at marker `k`.
    data_fields(strings)
        .into_iter()
        .zip(markers)
        .filter(|((.., text), _)| !text)
        .filter_map(|((code, context, unit, _), marker)| {
            let value = Bits::new(data, marker.checked_sub(72)?, end).raw_double()?;
            (value.is_finite() && (value == 0.0 || (1e-9..1e12).contains(&value.abs()))).then_some(PartField {
                code,
                context,
                unit,
                value,
            })
        })
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

/// A structure's insertion point, read by the record's fields.
pub(super) fn structure_at(data: &[u8], start: usize, end: usize) -> Option<Vector3> {
    let mut bits = Bits::new(data, start, end);
    part_head(&mut bits)?;
    point(&mut bits)
}

/// A pipe read by its fields: the numbers of its part data fields in record
/// order (`None` for a text field), then its start and end.
fn pipe_record(data: &[u8], start: usize, end: usize) -> Option<(Vec<Option<f64>>, Vector3, Vector3)> {
    let mut bits = Bits::new(data, start, end);
    part_head(&mut bits)?;
    // ponytail: 26 bits, the same in every sample.
    bits.skip(26)?;
    let nested = |bits: &mut Bits| Some(bits.bit_long()? == 80 && bits.bit_short()? == 0x04BE);
    nested(&mut bits)?.then_some(())?;
    (bits.bit_long()? == 80 && bits.bit_long()? == 80).then_some(())?;
    let n = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 60 <= bits.remaining())?;
    let mut separator = bits.bit_long()?;
    let mut numbers = Vec::with_capacity(n);
    for _ in 0..n {
        (separator == 0 && nested(&mut bits)?).then_some(())?;
        bits.bit_long()?;
        bits.bit_short()?;
        bits.bit_long()?;
        bits.bit_long()?;
        bits.skip(16)?;
        numbers.push(if bits.bit()? { Some(bits.bit_double()?) } else { None });
        (bits.bit_long()? == 1).then_some(())?;
        separator = bits.bit_long()?;
    }
    matches!(separator, 1 | 2).then_some(())?;
    for _ in 0..3 {
        bits.bit_long()?;
    }
    bits.skip(128 + 1)?;
    for _ in 0..17 {
        bits.bit_double()?;
    }
    bits.skip(2)?;
    Some((numbers, point(&mut bits)?, point(&mut bits)?))
}

/// Number fields of the part data a pipe keeps itself (flow coefficients,
/// minimum curve radius and the like).
pub(super) fn pipe_fields(data: &[u8], start: usize, end: usize, strings: &[String]) -> Vec<PartField> {
    let Some((numbers, ..)) = pipe_record(data, start, end) else {
        return Vec::new();
    };
    data_fields(strings)
        .into_iter()
        .zip(numbers)
        .filter_map(|((code, context, unit, _), value)| Some(PartField { code, context, unit, value: value? }))
        .collect()
}

/// Start and end of a pipe, read by its fields (see the module docs) or,
/// for a record laid out otherwise, as two `3BD` after six `BD 1.0` and two
/// bits. Other records follow the same bits; theirs hold unit vectors, a zero
/// length or garbage, which a pipe's ends do not.
pub(super) fn pipe_ends(data: &[u8], start: usize, end: usize) -> Option<(Vector3, Vector3)> {
    if let Some((_, a, b)) = pipe_record(data, start, end) {
        return Some((a, b));
    }
    const LEAD: &[u8] = b"01010101010100";
    let bit = |p: usize| (data[p / 8] >> (7 - p % 8)) & 1;
    let plausible = |v: f64| v.is_finite() && (v == 0.0 || (1e-9..1e9).contains(&v.abs()));
    let unit = |p: &Vector3| ((p.x * p.x + p.y * p.y + p.z * p.z).sqrt() - 1.0).abs() < 1e-9;
    let mut found = None;
    for pos in start..end.saturating_sub(LEAD.len()) {
        if !LEAD.iter().enumerate().all(|(i, c)| bit(pos + i) == c - b'0') {
            continue;
        }
        let mut bits = Bits::new(data, pos + LEAD.len(), end);
        let mut read = || -> Option<Vector3> {
            let v = Vector3::new(bits.bit_double()?, bits.bit_double()?, bits.bit_double()?);
            [v.x, v.y, v.z].into_iter().all(plausible).then_some(v)
        };
        let (Some(a), Some(b)) = (read(), read()) else { continue };
        if unit(&a) || unit(&b) || a.distance(&b) < 1e-6 {
            continue;
        }
        if found.is_some() {
            // Two candidates: not sure which is the pipe.
            return None;
        }
        found = Some((a, b));
    }
    found
}

/// A structure's insertion point: the first `3BD` whose plan position lies
/// inside the structure's extents.
pub(super) fn structure_position(data: &[u8], start: usize, end: usize, min: [f64; 2], max: [f64; 2]) -> Option<Vector3> {
    let inside = |x: f64, y: f64| x >= min[0] - 1e-6 && x <= max[0] + 1e-6 && y >= min[1] - 1e-6 && y <= max[1] + 1e-6;
    (start..end).find_map(|pos| {
        let mut bits = Bits::new(data, pos, end);
        // Full-width doubles only: the short forms match random bits.
        let mut full = || -> Option<f64> {
            let at = bits.pos;
            let v = bits.bit_double()?;
            (bits.pos - at == 66).then_some(v)
        };
        let (x, y) = (full()?, full()?);
        let z = bits.bit_double()?;
        (inside(x, y) && z.is_finite()).then(|| Vector3::new(x, y, z))
    })
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

    #[test]
    fn number_fields_end_eight_bits_before_the_next_marker() {
        for marker in MARKERS {
            let marker = std::str::from_utf8(marker).unwrap();
            // Record k ends at marker k: a text field, two number fields,
            // then the last record.
            let bits = format!(
                "{}{m}{}00{}00010000{m}{}00{}00010000{m}{}",
                "1".repeat(20),
                "1".repeat(7),
                raw(100.0),
                "1".repeat(7),
                raw(25.0),
                "1".repeat(20),
                m = marker
            );
            let data = bytes(&bits);
            let mut strings: Vec<String> = Vec::new();
            for (code, context, unit, text) in [
                ("PrtNm", "Catalog_PartName", "", Some("Pipe")),
                ("PID", "PipeInnerDiameter", "mm", None),
                ("WTh", "WallThickness", "mm", None),
                ("CSP", "CustomSize", "", Some("Model")),
            ] {
                strings.extend(["AeccNetworkPartDataField", "", code, "", context, unit].map(String::from));
                strings.extend(text.map(String::from));
            }
            let fields = part_fields(&data, 0, bits.len(), &strings);
            let got: Vec<(&str, f64)> = fields.iter().map(|f| (f.context.as_str(), f.value)).collect();
            assert_eq!(got, [("PipeInnerDiameter", 100.0), ("WallThickness", 25.0)], "{marker}");
        }
    }

    #[test]
    fn pipe_ends_skip_unit_vectors() {
        // A record with a unit vector, then the pipe's ends.
        let lead = "01010101010100";
        let bits = format!(
            "{lead}{}{}{}{}{}{}11{lead}{}{}{}{}{}{}",
            bd(0.0), bd(-8.0), bd(0.0), bd(1.0), bd(0.0), bd(0.0),
            bd(100.0), bd(200.0), bd(-3.5), bd(160.0), bd(210.0), bd(-4.0)
        );
        let data = bytes(&bits);
        let (a, b) = pipe_ends(&data, 0, bits.len()).unwrap();
        assert_eq!((a.x, a.y, a.z), (100.0, 200.0, -3.5));
        assert_eq!((b.x, b.y, b.z), (160.0, 210.0, -4.0));
    }

    #[test]
    fn pipe_record_reads_fields_and_ends() {
        // Class header without keys, 010, GUID, 26 bits, nested head, one
        // number field (2.5), the list's end, 3 x BL 0, GUID, 1 bit,
        // 17 x BD 1.0, 2 bits, then the ends.
        let l80 = "0101010000";
        let head = format!("{l80}10{l80}{l80}10 10 010101 00000000 010{}", "0".repeat(128));
        let nested = format!("{l80}001011111000000100");
        let field = format!("{nested}10 0101100101 10 10 {} 1 {} 0100000001", "0".repeat(16), bd(2.5));
        let ends: String = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0].iter().map(|v| bd(*v)).collect();
        let bits = format!(
            "{head}{}{nested}{l80}{l80}0100000001 10 {field} 0100000010 10 10 10 {} {} 00 {ends}",
            "0".repeat(26),
            "0".repeat(129),
            "01".repeat(17)
        )
        .replace(' ', "");
        let data = bytes(&bits);
        let (numbers, a, b) = pipe_record(&data, 0, bits.len()).unwrap();
        assert_eq!(numbers, [Some(2.5)]);
        assert_eq!((a.x, a.y, a.z, b.x, b.y, b.z), (1.0, 2.0, 3.0, 4.0, 5.0, 6.0));
    }
}
