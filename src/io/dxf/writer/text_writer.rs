//! ASCII DXF writer

use std::io::Write;
use crate::error::Result;
use crate::types::Handle;
use super::stream_writer::DxfStreamWriter;

/// ASCII DXF stream writer
pub struct DxfTextWriter<W: Write> {
    writer: W,
}

impl<W: Write> DxfTextWriter<W> {
    /// Create a new ASCII DXF writer
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
    
    /// Write a DXF code with proper formatting (right-aligned in 3-character field)
    fn write_code(&mut self, code: i32) -> Result<()> {
        if code < 10 {
            writeln!(self.writer, "  {}", code)?;
        } else if code < 100 {
            writeln!(self.writer, " {}", code)?;
        } else {
            writeln!(self.writer, "{}", code)?;
        }
        Ok(())
    }
    
    /// Get the inner writer
    pub fn into_inner(self) -> W {
        self.writer
    }
}

impl<W: Write> DxfStreamWriter for DxfTextWriter<W> {
    fn write_string(&mut self, code: i32, value: &str) -> Result<()> {
        self.write_code(code)?;
        writeln!(self.writer, "{}", value)?;
        Ok(())
    }
    
    fn write_byte(&mut self, code: i32, value: u8) -> Result<()> {
        self.write_code(code)?;
        writeln!(self.writer, "{}", value)?;
        Ok(())
    }
    
    fn write_i16(&mut self, code: i32, value: i16) -> Result<()> {
        self.write_code(code)?;
        writeln!(self.writer, "{}", value)?;
        Ok(())
    }
    
    fn write_i32(&mut self, code: i32, value: i32) -> Result<()> {
        self.write_code(code)?;
        writeln!(self.writer, "{}", value)?;
        Ok(())
    }
    
    fn write_i64(&mut self, code: i32, value: i64) -> Result<()> {
        self.write_code(code)?;
        writeln!(self.writer, "{}", value)?;
        Ok(())
    }
    
    fn write_double(&mut self, code: i32, value: f64) -> Result<()> {
        self.write_code(code)?;
        // Format with sufficient precision, trimming unnecessary trailing zeros
        // but always including at least one decimal place
        if value == value.trunc() {
            writeln!(self.writer, "{:.1}", value)?;
        } else {
            // Use enough precision for CAD data
            let formatted = format!("{:.15}", value);
            let trimmed = formatted.trim_end_matches('0');
            let trimmed = if trimmed.ends_with('.') {
                format!("{}0", trimmed)
            } else {
                trimmed.to_string()
            };
            writeln!(self.writer, "{}", trimmed)?;
        }
        Ok(())
    }
    
    fn write_bool(&mut self, code: i32, value: bool) -> Result<()> {
        self.write_code(code)?;
        writeln!(self.writer, "{}", if value { 1 } else { 0 })?;
        Ok(())
    }
    
    fn write_handle(&mut self, code: i32, handle: Handle) -> Result<()> {
        self.write_code(code)?;
        writeln!(self.writer, "{:X}", handle.value())?;
        Ok(())
    }
    
    fn write_binary(&mut self, code: i32, data: &[u8]) -> Result<()> {
        self.write_code(code)?;
        for byte in data {
            write!(self.writer, "{:02X}", byte)?;
        }
        writeln!(self.writer)?;
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
    use crate::types::Vector3;
    use super::super::stream_writer::DxfStreamWriterExt;
    
    #[test]
    fn test_write_string() {
        let mut buf = Vec::new();
        {
            let mut writer = DxfTextWriter::new(&mut buf);
            writer.write_string(0, "LINE").unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert_eq!(output, "  0\nLINE\n");
    }
    
    #[test]
    fn test_write_code_formatting() {
        let mut buf = Vec::new();
        {
            let mut writer = DxfTextWriter::new(&mut buf);
            writer.write_i16(5, 100).unwrap();
            writer.write_i16(62, 7).unwrap();
            writer.write_i16(100, 1).unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        // Codes should be right-aligned in 3-character field
        assert!(output.starts_with("  5\n"));
        assert!(output.contains(" 62\n"));
        assert!(output.contains("100\n"));
    }
    
    #[test]
    fn test_write_point3d() {
        let mut buf = Vec::new();
        {
            let mut writer = DxfTextWriter::new(&mut buf);
            writer.write_point3d(10, Vector3::new(1.0, 2.0, 3.0)).unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains(" 10\n"));
        assert!(output.contains("1.0\n"));
        assert!(output.contains(" 20\n"));
        assert!(output.contains("2.0\n"));
        assert!(output.contains(" 30\n"));
        assert!(output.contains("3.0\n"));
    }
    
    #[test]
    fn test_write_handle() {
        let mut buf = Vec::new();
        {
            let mut writer = DxfTextWriter::new(&mut buf);
            writer.write_handle(5, Handle::new(255)).unwrap();
        }
        let output = String::from_utf8(buf).unwrap();
        assert!(output.contains("FF\n"));
    }
}
