//! Binary DXF writer

use std::io::Write;
use byteorder::{LittleEndian, WriteBytesExt};
use crate::error::Result;
use crate::types::Handle;
use super::stream_writer::DxfStreamWriter;

/// Binary DXF sentinel
const BINARY_DXF_SENTINEL: &[u8] = b"AutoCAD Binary DXF\r\n\x1a\x00";

/// Binary DXF stream writer
pub struct DxfBinaryWriter<W: Write> {
    writer: W,
}

impl<W: Write> DxfBinaryWriter<W> {
    /// Create a new binary DXF writer
    pub fn new(mut writer: W) -> Result<Self> {
        // Write the binary sentinel at the start
        writer.write_all(BINARY_DXF_SENTINEL)?;
        Ok(Self { writer })
    }
    
    /// Write a DXF code as 16-bit little-endian
    fn write_code(&mut self, code: i32) -> Result<()> {
        self.writer.write_i16::<LittleEndian>(code as i16)?;
        Ok(())
    }
    
    /// Write a null-terminated string
    fn write_null_string(&mut self, value: &str) -> Result<()> {
        self.writer.write_all(value.as_bytes())?;
        self.writer.write_u8(0)?;
        Ok(())
    }
    
    /// Get the inner writer
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> DxfStreamWriter for DxfBinaryWriter<W> {
    fn write_string(&mut self, code: i32, value: &str) -> Result<()> {
        self.write_code(code)?;
        self.write_null_string(value)?;
        Ok(())
    }
    
    fn write_byte(&mut self, code: i32, value: u8) -> Result<()> {
        self.write_code(code)?;
        // Group codes 280-289 are "Byte" type but written as Int16 in binary DXF
        self.writer.write_i16::<LittleEndian>(value as i16)?;
        Ok(())
    }
    
    fn write_i16(&mut self, code: i32, value: i16) -> Result<()> {
        self.write_code(code)?;
        self.writer.write_i16::<LittleEndian>(value)?;
        Ok(())
    }
    
    fn write_i32(&mut self, code: i32, value: i32) -> Result<()> {
        self.write_code(code)?;
        self.writer.write_i32::<LittleEndian>(value)?;
        Ok(())
    }
    
    fn write_i64(&mut self, code: i32, value: i64) -> Result<()> {
        self.write_code(code)?;
        self.writer.write_i64::<LittleEndian>(value)?;
        Ok(())
    }
    
    fn write_double(&mut self, code: i32, value: f64) -> Result<()> {
        self.write_code(code)?;
        self.writer.write_f64::<LittleEndian>(value)?;
        Ok(())
    }
    
    fn write_bool(&mut self, code: i32, value: bool) -> Result<()> {
        self.write_code(code)?;
        self.writer.write_u8(if value { 1 } else { 0 })?;
        Ok(())
    }
    
    fn write_handle(&mut self, code: i32, handle: Handle) -> Result<()> {
        self.write_code(code)?;
        // Handles are written as hex strings even in binary DXF
        let hex_str = format!("{:X}", handle.value());
        self.write_null_string(&hex_str)?;
        Ok(())
    }
    
    fn write_binary(&mut self, code: i32, data: &[u8]) -> Result<()> {
        self.write_code(code)?;
        // Write length as a byte, then the data
        self.writer.write_u8(data.len() as u8)?;
        self.writer.write_all(data)?;
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
        assert_eq!(buf[sentinel_len..sentinel_len+2], [0, 0]); // code 0 as little-endian
        assert_eq!(&buf[sentinel_len+2..sentinel_len+6], b"LINE");
        assert_eq!(buf[sentinel_len+6], 0); // null terminator
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
        assert_eq!(buf[sentinel_len..sentinel_len+2], [10, 0]); // code 10 as little-endian
        // 1.5 as f64 little-endian
        let expected: [u8; 8] = 1.5f64.to_le_bytes();
        assert_eq!(&buf[sentinel_len+2..sentinel_len+10], &expected);
    }
    
    #[test]
    fn test_write_i16() {
        let mut buf = Vec::new();
        {
            let mut writer = DxfBinaryWriter::new(&mut buf).unwrap();
            writer.write_i16(62, 7).unwrap();
        }
        let sentinel_len = BINARY_DXF_SENTINEL.len();
        assert_eq!(buf[sentinel_len..sentinel_len+2], [62, 0]); // code 62
        assert_eq!(buf[sentinel_len+2..sentinel_len+4], [7, 0]); // value 7
    }
}

