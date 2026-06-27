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
    pub dxf_code: DxfCode,
    
    /// String representation of the value
    pub value_string: String,
    
    /// Typed value (integer, double, or boolean)
    typed_value: CodePairValue,
}

impl DxfCodePair {
    /// Create a new code/value pair
    pub fn new(code: i32, value_string: String) -> Self {
        let dxf_code = DxfCode::from_i32(code);
        // Deferred string->number parsing for ASCII DXF readers to avoid parsing overhead
        // for values that are never accessed as numbers.
        Self {
            code,
            dxf_code,
            value_string,
            typed_value: CodePairValue::None,
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
            _ => self.value_string.trim().parse::<i64>().ok(),
        }
    }

    /// Get value as i16
    pub fn as_i16(&self) -> Option<i16> {
        match self.typed_value {
            CodePairValue::Int(v) => i16::try_from(v).ok(),
            _ => self.value_string.trim().parse::<i64>().ok().and_then(|v| i16::try_from(v).ok()),
        }
    }

    /// Get value as i32
    #[allow(dead_code)]
    pub fn as_i32(&self) -> Option<i32> {
        match self.typed_value {
            CodePairValue::Int(v) => i32::try_from(v).ok(),
            _ => self.value_string.trim().parse::<i64>().ok().and_then(|v| i32::try_from(v).ok()),
        }
    }
    
    /// Get value as double
    pub fn as_double(&self) -> Option<f64> {
        match self.typed_value {
            CodePairValue::Double(v) => Some(v),
            _ => self.value_string.trim().parse::<f64>().ok(),
        }
    }
    
    /// Get value as boolean
    pub fn as_bool(&self) -> Option<bool> {
        match self.typed_value {
            CodePairValue::Bool(v) => Some(v),
            _ => self.value_string.trim().parse::<i32>().ok().map(|v| v != 0),
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
        if let Some(axis) = GroupCodeValueType::coordinate_axis(pair.dxf_code) {
            let coord_group = GroupCodeValueType::coordinate_group(pair.dxf_code);
            
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


