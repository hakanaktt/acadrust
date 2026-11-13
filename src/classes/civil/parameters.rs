//! Parameter sets: the input values of a subassembly, stored by the
//! subassembly and again for each subassembly a corridor applies.
//!
//! ```text
//! BL 0, BL 0, 6 bits (011010)
//! BL n, n x integer item     (type 0x1012)
//! BL n, n x double item      (type 0x1013)
//! BL n, n x string item      (type 0x1014)
//! item: BL type, BL kind, value (BL integer, BD double, nothing for a
//!       string), B, B choices, [choices: BL n, n x BL choice value],
//!       BL index
//! ```
//!
//! Some subassemblies write the types with bit 0x20 set (0x1032, 0x1033,
//! 0x1034); the items are read alike.
//!
//! The texts are in the record's text stream, in item order: the name, its
//! display name and description (resource ids), an empty string, then for an
//! integer each choice's (name, display name) and for a string its value.

use super::bits::Bits;

const INTEGER: i32 = 0x1012;
const DOUBLE: i32 = 0x1013;
const STRING: i32 = 0x1014;

/// Value of one parameter.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ParameterValue {
    Integer(i32),
    Double(f64),
    Text(String),
}

/// One input parameter of a subassembly.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Parameter {
    pub name: String,
    /// Display name, a resource id for stock subassemblies.
    pub display_name: String,
    /// Description, a resource id for stock subassemblies.
    pub description: String,
    pub value: ParameterValue,
    /// The field after the item type (0, 9 or 16 in the samples; 9 for a
    /// slope).
    pub kind: i32,
    /// Named choices of an integer parameter: (name, display name, value).
    pub choices: Vec<(String, String, i32)>,
    /// The parameter's index in its subassembly's definition.
    pub index: i32,
}

/// The parameter set starting at `bits`, texts from `strings` (advanced past
/// the ones it uses).
pub(super) fn read(bits: &mut Bits, strings: &mut std::slice::Iter<String>) -> Option<Vec<Parameter>> {
    read_with(bits, &mut || strings.next().cloned())
}

/// The parameter set at `bits`, texts taken from `text` in order (a corridor
/// record's texts are not followed, so it passes empty ones).
pub(super) fn read_with(bits: &mut Bits, text: &mut dyn FnMut() -> Option<String>) -> Option<Vec<Parameter>> {
    (bits.bit_long()? == 0 && bits.bit_long()? == 0).then_some(())?;
    // ponytail: 6 bits, the same in every sample; name them once a record
    // varies them.
    (bits.bits_value(6)? == 0b01_1010).then_some(())?;
    let mut out = Vec::new();
    for kind in [INTEGER, DOUBLE, STRING] {
        let n = bits.bit_long()?;
        (0..=10_000).contains(&n).then_some(())?;
        for _ in 0..n {
            out.push(item(bits, text, kind)?);
        }
    }
    Some(out)
}

fn item(bits: &mut Bits, text: &mut dyn FnMut() -> Option<String>, kind: i32) -> Option<Parameter> {
    (bits.bit_long()? & !0x20 == kind).then_some(())?;
    let field = bits.bit_long()?;
    let value = match kind {
        INTEGER => Some(ParameterValue::Integer(bits.bit_long()?)),
        DOUBLE => Some(ParameterValue::Double(bits.bit_double()?)),
        _ => None,
    };
    bits.bit()?;
    let mut values = Vec::new();
    if bits.bit()? {
        let n = bits.bit_long()?;
        (0..=10_000).contains(&n).then_some(())?;
        for _ in 0..n {
            values.push(bits.bit_long()?);
        }
    }
    let index = bits.bit_long()?;
    let name = text()?;
    let display_name = text()?;
    let description = text()?;
    text()?;
    let choices = values
        .into_iter()
        .map(|v| Some((text()?, text()?, v)))
        .collect::<Option<Vec<_>>>()?;
    let value = match value {
        Some(v) => v,
        None => ParameterValue::Text(text()?),
    };
    Some(Parameter { name, display_name, description, value, kind: field, choices, index })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits_of(s: &str) -> Vec<u8> {
        let s: Vec<u8> = s.bytes().filter(|b| *b == b'0' || *b == b'1').collect();
        s.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | ((b - b'0') << (7 - i)))).collect()
    }

    #[test]
    fn reads_one_item_of_each_kind() {
        // BL 0, BL 0, 011010; integers: BL 1, {0x1012, BL 0, BL 1, B 0, B 1,
        // BL 2, BL 1, BL 0, BL 3}; doubles: BL 1, {0x1013, BL 16, BD 1.0,
        // B 0, B 0, BL 4}; strings: BL 1, {0x1014, BL 0, B 0, B 0, BL 0}.
        let b = bits_of(
            "10 10 011010 0100000001 00 00010010 00010000 00000000 00000000 10 0100000001 0 1 0100000010 0100000001 10 0100000011
             0100000001 00 00010011 00010000 00000000 00000000 0100010000 01 0 0 0100000100
             0100000001 00 00010100 00010000 00000000 00000000 10 0 0 10 0000000",
        );
        let texts: Vec<String> = ["CrownPtOnInside", "1", "2", "", "Yes", "Y", "No", "N", "Width", "3", "4", "", "Version", "5", "6", "", "R2019"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let read = read(&mut Bits::new(&b, 0, b.len() * 8), &mut texts.iter()).unwrap();
        assert_eq!(read.len(), 3);
        assert_eq!(read[0].value, ParameterValue::Integer(1));
        assert_eq!(read[0].choices, vec![("Yes".into(), "Y".into(), 1), ("No".into(), "N".into(), 0)]);
        assert_eq!(read[0].index, 3);
        assert_eq!((read[1].kind, read[1].value.clone(), read[1].index), (16, ParameterValue::Double(1.0), 4));
        assert_eq!(read[2].value, ParameterValue::Text("R2019".into()));
    }
}
