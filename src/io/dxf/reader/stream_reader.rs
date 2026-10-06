//! DXF stream reader trait and common types

use crate::error::Result;
use crate::io::dxf::{DxfCode, GroupCodeValueType};
use crate::types::Vector3;

/// Typed value of a DXF code pair
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CodePairValue {
    /// No typed value (string-only codes)
    None,
    /// Integer value (for Int16/Int32/Int64/Byte group codes)
    Int(i64),
    /// Floating-point value (for Double group codes)
    Double(f64),
    /// Boolean value (for Bool group codes)
    Bool(bool),
}

/// A DXF code/value pair
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DxfCodePair {
    /// The DXF group code
    pub code: i32,

    /// The DXF code enum
    #[allow(dead_code)]
    pub dxf_code: DxfCode,

    /// String representation of the value
    pub value_string: String,

    /// Typed value (integer, double, or boolean)
    typed_value: CodePairValue,
}

#[derive(Debug, Clone, Default)]
pub struct DxfStreamContext {
    pub source_offset: Option<u64>,
    pub source_line: Option<usize>,
    pub record_type: Option<String>,
    pub record_handle: Option<u64>,
}

impl DxfCodePair {
    /// Create a new code/value pair
    pub fn new(code: i32, value_string: String) -> Self {
        let dxf_code = DxfCode::from_i32(code);
        // Use the raw group code here: DxfCode intentionally has only named
        // variants for a subset of the legal ranges, so routing through the
        // enum turns valid values such as XRecord code 290 into `Invalid`.
        let value_type = GroupCodeValueType::from_raw_code(code);

        // Parse value based on type
        let typed_value = match value_type {
            GroupCodeValueType::Int16
            | GroupCodeValueType::Int32
            | GroupCodeValueType::Int64
            | GroupCodeValueType::Byte => match value_string.trim().parse::<i64>() {
                Ok(v) => CodePairValue::Int(v),
                Err(_) => CodePairValue::None,
            },
            GroupCodeValueType::Double => match value_string.trim().parse::<f64>() {
                Ok(v) => CodePairValue::Double(v),
                Err(_) => CodePairValue::None,
            },
            GroupCodeValueType::Bool => match value_string.trim().parse::<i32>() {
                Ok(v) => CodePairValue::Bool(v != 0),
                Err(_) => CodePairValue::None,
            },
            _ => CodePairValue::None,
        };

        Self {
            code,
            dxf_code,
            value_string,
            typed_value,
        }
    }

    /// Create a code/value pair with a pre-computed typed value.
    /// Skips string→typed parsing, used by the binary DXF reader.
    pub(crate) fn new_typed(code: i32, value_string: String, typed_value: CodePairValue) -> Self {
        let dxf_code = DxfCode::from_i32(code);
        Self {
            code,
            dxf_code,
            value_string,
            typed_value,
        }
    }

    /// Get value as string
    #[allow(dead_code)]
    pub fn as_string(&self) -> &str {
        &self.value_string
    }

    /// Get value as integer
    #[allow(dead_code)]
    pub fn as_int(&self) -> Option<i64> {
        match self.typed_value {
            CodePairValue::Int(v) => Some(v),
            _ => None,
        }
    }

    /// Get value as i16
    pub fn as_i16(&self) -> Option<i16> {
        match self.typed_value {
            CodePairValue::Int(v) => i16::try_from(v).ok(),
            // Codes 290-299 are typed as booleans, but many readers consume
            // them as 0/1 flags through this accessor.
            CodePairValue::Bool(v) => Some(v as i16),
            _ => None,
        }
    }

    /// Get value as i32
    #[allow(dead_code)]
    pub fn as_i32(&self) -> Option<i32> {
        match self.typed_value {
            CodePairValue::Int(v) => i32::try_from(v).ok(),
            CodePairValue::Bool(v) => Some(v as i32),
            _ => None,
        }
    }

    /// Get value as a 32-bit *bit pattern*, accepting the unsigned spelling.
    ///
    /// A colour code (420, 421, and the `AcCmColor` values MLEADER carries in
    /// its 90-series codes) is a packed word, not a quantity: producers set a
    /// method byte in the high position, `0xC2` for a true colour, so a plain
    /// RGB such as `0xC2FF8800` is written by some producers as the unsigned
    /// `3271526400` and by others as the signed `-1023440896`. Only the second
    /// fits an `i32`, so [`as_i32`](Self::as_i32) drops the first, the colour
    /// never reaches the entity, and it falls back to its ACI index, which for
    /// a true-coloured entity is usually 256 (`ByLayer`).
    ///
    /// Reinterpreting the unsigned spelling as its two's complement keeps both,
    /// and the downstream decoders already mask the method byte off. Codes that
    /// carry a real number keep using `as_i32`: a flags word of three billion is
    /// a malformed file, not a negative count.
    pub fn as_i32_bits(&self) -> Option<i32> {
        match self.typed_value {
            CodePairValue::Int(v) => i32::try_from(v)
                .ok()
                .or_else(|| u32::try_from(v).ok().map(|bits| bits as i32)),
            _ => None,
        }
    }

    /// Get value as double
    pub fn as_double(&self) -> Option<f64> {
        match self.typed_value {
            CodePairValue::Double(v) => Some(v),
            _ => None,
        }
    }

    /// Get value as boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self.typed_value {
            CodePairValue::Bool(v) => Some(v),
            _ => None,
        }
    }

    /// Get value as handle (hex string to u64)
    #[allow(dead_code)]
    pub fn as_handle(&self) -> Option<u64> {
        u64::from_str_radix(self.value_string.trim(), 16).ok()
    }
}

/// Trait for reading DXF code/value pairs from a stream
pub trait DxfStreamReader {
    /// Read the next code/value pair
    fn read_pair(&mut self) -> Result<Option<DxfCodePair>>;

    /// Peek at the next code without consuming it
    #[allow(dead_code)]
    fn peek_code(&mut self) -> Result<Option<i32>>;

    /// Push a pair back to be read again on next read_pair call
    fn push_back(&mut self, pair: DxfCodePair);

    /// Reset the reader to the beginning
    #[allow(dead_code)]
    fn reset(&mut self) -> Result<()>;

    /// Set the character encoding for non-UTF8 strings.
    ///
    /// Called after reading $DWGCODEPAGE from the header.
    /// Implementations that don't support encoding changes can ignore this.
    fn set_encoding(&mut self, _encoding: &'static encoding_rs::Encoding) {
        // Default: no-op
    }

    fn diagnostic_context(&self) -> DxfStreamContext {
        DxfStreamContext::default()
    }

    fn record_xdata(&mut self, _on: bool) {}

    fn take_recorded_xdata(&mut self) -> (Option<String>, Vec<DxfCodePair>) {
        (None, Vec::new())
    }
}

pub(crate) struct XDataRecorder {
    inner: Box<dyn DxfStreamReader>,
    pending: Vec<DxfCodePair>,
    recording: bool,
    in_xdata: bool,
    handle: Option<String>,
    recorded: Vec<DxfCodePair>,
}

impl XDataRecorder {
    pub(crate) fn new(inner: Box<dyn DxfStreamReader>) -> Self {
        Self {
            inner,
            pending: Vec::new(),
            recording: false,
            in_xdata: false,
            handle: None,
            recorded: Vec::new(),
        }
    }
}

impl DxfStreamReader for XDataRecorder {
    fn read_pair(&mut self) -> Result<Option<DxfCodePair>> {
        if let Some(pair) = self.pending.pop() {
            return Ok(Some(pair));
        }
        let pair = self.inner.read_pair()?;
        if self.recording {
            if let Some(pair) = &pair {
                if pair.code == 1001 {
                    self.in_xdata = true;
                } else if !(1000..=1071).contains(&pair.code) {
                    self.in_xdata = false;
                }
                if self.in_xdata {
                    self.recorded.push(pair.clone());
                } else if pair.code == 5 && self.handle.is_none() {
                    self.handle = Some(pair.value_string.trim().to_string());
                }
            }
        }
        Ok(pair)
    }

    fn peek_code(&mut self) -> Result<Option<i32>> {
        match self.pending.last() {
            Some(pair) => Ok(Some(pair.code)),
            None => self.inner.peek_code(),
        }
    }

    fn push_back(&mut self, pair: DxfCodePair) {
        self.pending.push(pair);
    }

    fn reset(&mut self) -> Result<()> {
        self.pending.clear();
        self.inner.reset()
    }

    fn set_encoding(&mut self, encoding: &'static encoding_rs::Encoding) {
        self.inner.set_encoding(encoding);
    }

    fn diagnostic_context(&self) -> DxfStreamContext {
        self.inner.diagnostic_context()
    }

    fn record_xdata(&mut self, on: bool) {
        self.recording = on;
        self.in_xdata = false;
        self.handle = None;
        self.recorded.clear();
    }

    fn take_recorded_xdata(&mut self) -> (Option<String>, Vec<DxfCodePair>) {
        (self.handle.take(), std::mem::take(&mut self.recorded))
    }
}

/// Helper for reading 3D points from consecutive code pairs
pub struct PointReader {
    x: Option<f64>,
    y: Option<f64>,
    z: Option<f64>,
    group: Option<usize>,
}

impl PointReader {
    /// Create a new point reader
    pub fn new() -> Self {
        Self {
            x: None,
            y: None,
            z: None,
            group: None,
        }
    }

    /// Add a coordinate value
    pub fn add_coordinate(&mut self, pair: &DxfCodePair) -> bool {
        if let Some(axis) = GroupCodeValueType::coordinate_axis_raw(pair.code) {
            let coord_group = GroupCodeValueType::coordinate_group_raw(pair.code);

            // If this is a new group, reset
            if self.group.is_some() && self.group != coord_group {
                return false;
            }

            self.group = coord_group;

            if let Some(value) = pair.as_double() {
                match axis {
                    0 => self.x = Some(value),
                    1 => self.y = Some(value),
                    2 => self.z = Some(value),
                    _ => return false,
                }
                return true;
            }
        }
        false
    }

    /// Check if we have a complete point
    #[allow(dead_code)]
    pub fn is_complete(&self) -> bool {
        self.x.is_some() && self.y.is_some()
    }

    /// Get the point (returns Vector3 with z=0 if z not provided)
    pub fn get_point(&self) -> Option<Vector3> {
        if let (Some(x), Some(y)) = (self.x, self.y) {
            Some(Vector3::new(x, y, self.z.unwrap_or(0.0)))
        } else {
            None
        }
    }

    /// Reset the reader
    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.x = None;
        self.y = None;
        self.z = None;
        self.group = None;
    }
}

impl Default for PointReader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Code 420 is an Int32 group code, so `DxfCodePair::new` parses its text
    /// into `CodePairValue::Int` and the accessors below see an `i64`.
    fn true_color_pair(text: &str) -> DxfCodePair {
        DxfCodePair::new(420, text.to_string())
    }

    #[test]
    fn as_i32_bits_keeps_what_as_i32_already_kept() {
        for text in [
            "0",
            "16746496",
            "-1023440896",
            "-1",
            "2147483647",
            "-2147483648",
        ] {
            let pair = true_color_pair(text);

            assert_eq!(pair.as_i32_bits(), pair.as_i32(), "spelled {text}");
            assert!(pair.as_i32_bits().is_some(), "spelled {text}");
        }
    }

    /// The whole point: the unsigned half of the 32-bit range, which `as_i32`
    /// rejects, comes back as the same bit pattern read signed.
    #[test]
    fn as_i32_bits_accepts_the_unsigned_half_of_the_range() {
        for (unsigned, signed) in [
            ("2147483648", i32::MIN),
            ("3271526400", -1_023_440_896), // 0xC2FF8800, a true colour
            ("4294967295", -1),
        ] {
            let pair = true_color_pair(unsigned);

            assert_eq!(pair.as_i32(), None, "as_i32 should still reject {unsigned}");
            assert_eq!(pair.as_i32_bits(), Some(signed), "spelled {unsigned}");
        }
    }

    /// Past `u32::MAX` there is no bit pattern to reinterpret, and a code that
    /// carries no integer at all has nothing to offer either. Both must stay
    /// `None` rather than wrap into a plausible-looking colour.
    #[test]
    fn as_i32_bits_refuses_what_is_not_a_32_bit_pattern() {
        assert_eq!(true_color_pair("4294967296").as_i32_bits(), None);
        assert_eq!(true_color_pair("-2147483649").as_i32_bits(), None);
        assert_eq!(true_color_pair("not a number").as_i32_bits(), None);
        // A Double code parses into `CodePairValue::Double`, not `Int`.
        assert_eq!(DxfCodePair::new(40, "1.5".to_string()).as_i32_bits(), None);
    }
}
