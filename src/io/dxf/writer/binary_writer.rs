//! Binary DXF writer

use super::stream_writer::DxfStreamWriter;
use crate::error::Result;
use crate::types::Handle;
use byteorder::{LittleEndian, WriteBytesExt};
use std::io::Write;

/// Binary DXF sentinel
const BINARY_DXF_SENTINEL: &[u8] = b"AutoCAD Binary DXF\r\n\x1a\x00";

/// Binary DXF stream writer
pub struct DxfBinaryWriter<W: Write> {
    writer: W,
    /// Stack buffer for formatting handles without heap allocation.
    hex_buf: [u8; 17],
    encoding: Option<crate::io::dxf::code_page::LegacyEncoding>,
}

impl<W: Write> DxfBinaryWriter<W> {
    /// Create a new binary DXF writer
    pub fn new(mut writer: W) -> Result<Self> {
        // Write the binary sentinel at the start
        writer.write_all(BINARY_DXF_SENTINEL)?;
        Ok(Self {
            writer,
            hex_buf: [0u8; 17],
            encoding: None,
        })
    }

    /// Select the declared code page for a pre-2007 DXF.
    pub fn set_encoding(&mut self, encoding: crate::io::dxf::code_page::LegacyEncoding) {
        self.encoding = Some(encoding);
    }

    /// Write a DXF code as 16-bit little-endian
    fn write_code(&mut self, code: i32) -> Result<()> {
        self.writer.write_i16::<LittleEndian>(code as i16)?;
        Ok(())
    }

    /// Write a null-terminated string
    fn write_null_string(&mut self, value: &str) -> Result<()> {
        // A NUL terminates a binary DXF string. Drop embedded NULs so they
        // cannot turn the remainder of the value into a new group code.
        let sanitized;
        let value = if value.contains('\0') {
            sanitized = value.replace('\0', "");
            sanitized.as_str()
        } else {
            value
        };
        if let Some(encoding) = self.encoding {
            self.writer
                .write_all(&crate::io::dxf::code_page::encode_legacy_string(
                    value, encoding,
                ))?;
        } else {
            self.writer.write_all(value.as_bytes())?;
        }
        self.writer.write_u8(0)?;
        Ok(())
    }

    /// Preserved DXF pairs store their values as text, but their binary
    /// representation still follows the group code's numeric or chunk type.
    /// Return false only for values that retain the null-string layout.
    fn write_typed_text(&mut self, code: i32, value: &str) -> Result<bool> {
        use crate::error::DxfError;
        use crate::io::dxf::GroupCodeValueType;

        let invalid = || DxfError::Parse(format!("Invalid text value for binary DXF group {code}"));
        let text = value.trim();
        match GroupCodeValueType::from_raw_code(code) {
            GroupCodeValueType::String | GroupCodeValueType::Handle => return Ok(false),
            GroupCodeValueType::Bool => {
                self.write_bool(code, text.parse::<i64>().map_err(|_| invalid())? != 0)?
            }
            GroupCodeValueType::Byte | GroupCodeValueType::Int16 => {
                self.write_i16(code, text.parse().map_err(|_| invalid())?)?
            }
            GroupCodeValueType::Int32 => {
                self.write_i32(code, text.parse().map_err(|_| invalid())?)?
            }
            GroupCodeValueType::Int64 => {
                self.write_i64(code, text.parse().map_err(|_| invalid())?)?
            }
            GroupCodeValueType::Double | GroupCodeValueType::Point3D => {
                self.write_double(code, text.parse().map_err(|_| invalid())?)?
            }
            GroupCodeValueType::BinaryData => {
                if text.len() % 2 != 0 {
                    return Err(invalid());
                }
                let data = text
                    .as_bytes()
                    .chunks_exact(2)
                    .map(|pair| {
                        let high = (pair[0] as char).to_digit(16).ok_or_else(invalid)?;
                        let low = (pair[1] as char).to_digit(16).ok_or_else(invalid)?;
                        Ok(((high << 4) | low) as u8)
                    })
                    .collect::<Result<Vec<_>>>()?;
                self.write_binary(code, &data)?;
            }
            GroupCodeValueType::None => return Err(DxfError::InvalidDxfCode(code)),
        }
        Ok(true)
    }

    /// Write an integer using the binary width required by its group code.
    /// Callers often have a wider or narrower Rust integer available than the
    /// DXF field requires, so the group-code type is the source of truth.
    fn write_integer(&mut self, code: i32, value: i64) -> Result<()> {
        use crate::error::DxfError;
        use crate::io::dxf::GroupCodeValueType;

        let too_big = || {
            DxfError::Parse(format!(
                "value {value} does not fit the width binary DXF gives group {code}"
            ))
        };
        match GroupCodeValueType::from_raw_code(code) {
            GroupCodeValueType::Bool => {
                let v = u8::try_from(value)
                    .ok()
                    .filter(|v| *v <= 1)
                    .ok_or_else(too_big)?;
                self.write_code(code)?;
                self.writer.write_u8(v)?;
            }
            // Binary DXF stores Byte groups in the same two-byte slot as
            // Int16 groups (the reader follows this convention as well).
            GroupCodeValueType::Byte | GroupCodeValueType::Int16 => {
                let v = i16::try_from(value).map_err(|_| too_big())?;
                self.write_code(code)?;
                self.writer.write_i16::<LittleEndian>(v)?;
            }
            GroupCodeValueType::Int32 => {
                let v = i32::try_from(value).map_err(|_| too_big())?;
                self.write_code(code)?;
                self.writer.write_i32::<LittleEndian>(v)?;
            }
            GroupCodeValueType::Int64 => {
                self.write_code(code)?;
                self.writer.write_i64::<LittleEndian>(value)?;
            }
            GroupCodeValueType::Double => {
                self.write_code(code)?;
                self.writer.write_f64::<LittleEndian>(value as f64)?;
            }
            _ => return Err(DxfError::InvalidDxfCode(code)),
        }
        Ok(())
    }

    /// Get the inner writer
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> DxfStreamWriter for DxfBinaryWriter<W> {
    fn is_binary(&self) -> bool {
        true
    }

    fn write_string(&mut self, code: i32, value: &str) -> Result<()> {
        if self.write_typed_text(code, value)? {
            return Ok(());
        }
        self.write_code(code)?;
        // Sanitize embedded newlines to DXF paragraph markers, matching the
        // ASCII writer.  While binary DXF uses null-terminated strings (so raw
        // newlines won't structurally break the file), many ACAD consumers
        // still expect \P instead of literal line breaks in string values.
        if value.contains('\n') || value.contains('\r') {
            let sanitized = value
                .replace("\r\n", "\\P")
                .replace('\r', "\\P")
                .replace('\n', "\\P");
            self.write_null_string(&sanitized)?;
        } else {
            self.write_null_string(value)?;
        }
        Ok(())
    }

    fn write_xrecord_string(&mut self, code: i32, value: &str) -> Result<()> {
        if self.write_typed_text(code, value)? {
            return Ok(());
        }
        self.write_code(code)?;
        self.write_null_string(value)
    }

    fn write_byte(&mut self, code: i32, value: u8) -> Result<()> {
        self.write_integer(code, i64::from(value))
    }

    fn write_i16(&mut self, code: i32, value: i16) -> Result<()> {
        self.write_integer(code, i64::from(value))
    }

    fn write_i32(&mut self, code: i32, value: i32) -> Result<()> {
        self.write_integer(code, i64::from(value))
    }

    fn write_i64(&mut self, code: i32, value: i64) -> Result<()> {
        self.write_integer(code, value)
    }

    fn write_double(&mut self, code: i32, value: f64) -> Result<()> {
        self.write_code(code)?;
        self.writer.write_f64::<LittleEndian>(value)?;
        Ok(())
    }

    fn write_bool(&mut self, code: i32, value: bool) -> Result<()> {
        use crate::io::dxf::GroupCodeValueType;

        // Logical flags also occur in integer groups, including history
        // flags 280/281. Their binary width follows the group code, not the
        // Rust value type; only groups 290-299 use a single byte.
        let value = u8::from(value);
        match GroupCodeValueType::from_raw_code(code) {
            GroupCodeValueType::Bool => {
                self.write_code(code)?;
                self.writer.write_u8(value)?;
                Ok(())
            }
            GroupCodeValueType::Byte | GroupCodeValueType::Int16 => {
                self.write_i16(code, i16::from(value))
            }
            GroupCodeValueType::Int32 => self.write_i32(code, i32::from(value)),
            GroupCodeValueType::Int64 => self.write_i64(code, i64::from(value)),
            GroupCodeValueType::Double => self.write_double(code, f64::from(value)),
            _ => Err(crate::error::DxfError::InvalidDxfCode(code)),
        }
    }

    fn write_handle(&mut self, code: i32, handle: Handle) -> Result<()> {
        self.write_code(code)?;
        // Handles are written as hex strings even in binary DXF
        // Format directly into stack buffer to avoid heap allocation
        let val = handle.value();
        if val == 0 {
            self.writer.write_all(b"0\0")?;
        } else {
            let mut pos = 16usize;
            let mut v = val;
            while v > 0 {
                pos -= 1;
                let digit = (v & 0xF) as u8;
                self.hex_buf[pos] = if digit < 10 {
                    b'0' + digit
                } else {
                    b'A' + digit - 10
                };
                v >>= 4;
            }
            let hex_len = 16 - pos;
            self.hex_buf.copy_within(pos..16, 0);
            self.hex_buf[hex_len] = 0; // null terminator
            self.writer.write_all(&self.hex_buf[..hex_len + 1])?;
        }
        Ok(())
    }

    fn write_binary(&mut self, code: i32, data: &[u8]) -> Result<()> {
        // Each binary group has a one-byte length. Repeat the group instead
        // of truncating the length when a caller supplies a larger payload.
        if data.is_empty() {
            self.write_code(code)?;
            self.writer.write_u8(0)?;
        }
        for chunk in data.chunks(255) {
            self.write_code(code)?;
            self.writer.write_u8(chunk.len() as u8)?;
            self.writer.write_all(chunk)?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_binary_sentinel() {
        let mut buf = Vec::new();
        {
            let _writer = DxfBinaryWriter::new(&mut buf).unwrap();
        }
        assert!(buf.starts_with(BINARY_DXF_SENTINEL));
    }

    #[test]
    fn test_write_string() {
        let mut buf = Vec::new();
        {
            let mut writer = DxfBinaryWriter::new(&mut buf).unwrap();
            writer.write_string(0, "LINE").unwrap();
        }
        let sentinel_len = BINARY_DXF_SENTINEL.len();
        // After sentinel: code (2 bytes) + string + null
        assert_eq!(buf[sentinel_len..sentinel_len + 2], [0, 0]); // code 0 as little-endian
        assert_eq!(&buf[sentinel_len + 2..sentinel_len + 6], b"LINE");
        assert_eq!(buf[sentinel_len + 6], 0); // null terminator
    }

    #[test]
    fn test_write_double() {
        let mut buf = Vec::new();
        {
            let mut writer = DxfBinaryWriter::new(&mut buf).unwrap();
            writer.write_double(10, 1.5).unwrap();
        }
        let sentinel_len = BINARY_DXF_SENTINEL.len();
        // code (2 bytes) + f64 (8 bytes)
        assert_eq!(buf[sentinel_len..sentinel_len + 2], [10, 0]); // code 10 as little-endian
                                                                  // 1.5 as f64 little-endian
        let expected: [u8; 8] = 1.5f64.to_le_bytes();
        assert_eq!(&buf[sentinel_len + 2..sentinel_len + 10], &expected);
    }

    #[test]
    fn test_write_i16() {
        let mut buf = Vec::new();
        {
            let mut writer = DxfBinaryWriter::new(&mut buf).unwrap();
            writer.write_i16(62, 7).unwrap();
        }
        let sentinel_len = BINARY_DXF_SENTINEL.len();
        assert_eq!(buf[sentinel_len..sentinel_len + 2], [62, 0]); // code 62
        assert_eq!(buf[sentinel_len + 2..sentinel_len + 4], [7, 0]); // value 7
    }

    #[test]
    fn test_write_string_newline_sanitization() {
        let mut buf = Vec::new();
        {
            let mut writer = DxfBinaryWriter::new(&mut buf).unwrap();
            writer.write_string(1, "Hello\r\nWorld\nFoo\rBar").unwrap();
        }
        let sentinel_len = BINARY_DXF_SENTINEL.len();
        // code (2 bytes) + sanitized string + null
        let str_start = sentinel_len + 2;
        let str_end = buf[str_start..].iter().position(|&b| b == 0).unwrap() + str_start;
        let written = std::str::from_utf8(&buf[str_start..str_end]).unwrap();
        assert_eq!(written, "Hello\\PWorld\\PFoo\\PBar");
    }

    #[test]
    fn embedded_nuls_do_not_split_binary_strings() {
        for encoding in [None, Some(encoding_rs::WINDOWS_1252.into())] {
            for xrecord in [false, true] {
                for (value, sanitized) in [
                    ("", ""),
                    ("\0", ""),
                    ("\0\0", ""),
                    ("\0caf\u{e9}\0 au\0 lait\0", "caf\u{e9} au lait"),
                ] {
                    let mut writer = DxfBinaryWriter::new(Vec::new()).unwrap();
                    if let Some(encoding) = encoding {
                        writer.set_encoding(encoding);
                    }
                    let code = if xrecord { 1_i16 } else { 1000 };
                    if xrecord {
                        writer.write_xrecord_string(i32::from(code), value).unwrap();
                    } else {
                        writer.write_string(i32::from(code), value).unwrap();
                    }
                    writer.write_string(0, "EOF").unwrap();

                    let mut expected = BINARY_DXF_SENTINEL.to_vec();
                    expected.extend_from_slice(&code.to_le_bytes());
                    if encoding.is_some() {
                        let (bytes, _, errors) = encoding_rs::WINDOWS_1252.encode(sanitized);
                        assert!(!errors);
                        expected.extend_from_slice(&bytes);
                    } else {
                        expected.extend_from_slice(sanitized.as_bytes());
                    }
                    expected.extend_from_slice(b"\0\0\0EOF\0");
                    assert_eq!(writer.into_inner(), expected);
                }
            }
        }
    }

    #[test]
    fn nul_filtering_preserves_each_string_paths_newline_behavior() {
        for (xrecord, expected_string) in [
            (false, b"one\\Ptwo\\Pthree".as_slice()),
            (true, b"one\r\ntwo\nthree".as_slice()),
        ] {
            let mut writer = DxfBinaryWriter::new(Vec::new()).unwrap();
            if xrecord {
                writer
                    .write_xrecord_string(1, "one\0\r\ntwo\nthree\0")
                    .unwrap();
            } else {
                writer.write_string(1, "one\0\r\ntwo\nthree\0").unwrap();
            }
            let mut expected = BINARY_DXF_SENTINEL.to_vec();
            expected.extend_from_slice(&1_i16.to_le_bytes());
            expected.extend_from_slice(expected_string);
            expected.push(0);
            assert_eq!(writer.into_inner(), expected);
        }
    }
}

#[cfg(test)]
mod overflow_tests {
    use super::*;

    fn writer() -> DxfBinaryWriter<Vec<u8>> {
        DxfBinaryWriter::new(Vec::new()).unwrap()
    }

    /// Group 73 is a 16-bit group, so 70000 does not fit. Casting it down
    /// would write 4464 and nobody would know.
    #[test]
    fn a_value_too_big_for_a_16_bit_group_is_an_error() {
        assert!(writer().write_i32(73, 70_000).is_err());
        assert!(writer().write_i64(73, i64::from(i16::MIN) - 1).is_err());
        assert!(writer().write_i16(73, i16::MAX).is_ok());
    }

    /// Groups 90-99 are 32-bit.
    #[test]
    fn a_value_too_big_for_a_32_bit_group_is_an_error() {
        assert!(writer().write_i64(90, i64::from(i32::MAX) + 1).is_err());
        assert!(writer().write_i64(90, i64::from(i32::MIN)).is_ok());
    }

    /// Groups 290-299 are one byte holding a flag.
    #[test]
    fn a_value_that_is_not_a_flag_is_an_error_in_a_one_byte_group() {
        assert!(writer().write_i32(290, 2).is_err());
        assert!(writer().write_i32(290, 257).is_err());
        assert!(writer().write_i32(290, 1).is_ok());
        assert!(writer().write_bool(290, true).is_ok());
    }

    /// A value that fits keeps the width the code fixes.
    #[test]
    fn a_value_that_fits_is_written_unchanged() {
        let mut w = writer();
        w.write_i32(73, -2).unwrap();
        let out = w.into_inner();
        let n = BINARY_DXF_SENTINEL.len();
        assert_eq!(out[n..], [73, 0, 0xFE, 0xFF]);
    }
}
