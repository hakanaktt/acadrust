//! Closed source values must be diagnosed before legacy enum defaulting.
use crate::error::{DxfError, Result};

pub(crate) fn checked(value: i64, allowed: &[i16], field: &str) -> Result<i16> {
    if let Ok(code) = i16::try_from(value) {
        if allowed.contains(&code) {
            return Ok(code);
        }
    }
    Err(DxfError::Parse(format!("invalid {field} value {value}")))
}
pub(crate) fn dwg(
    reader: &super::dwg::dwg_stream_readers::merged_reader::DwgMergedReader,
    value: i16,
    allowed: &[i16],
    field: &str,
) -> i16 {
    if let Err(error) = checked(i64::from(value), allowed, field) {
        reader.invalidate(error.to_string());
    }
    value
}
