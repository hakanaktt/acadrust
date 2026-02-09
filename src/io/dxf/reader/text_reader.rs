//! DXF ASCII text reader

use super::stream_reader::{DxfCodePair, DxfStreamReader};
use crate::error::{DxfError, Result};
use encoding_rs::Encoding;
use std::io::{BufReader, Read, Seek, SeekFrom};

/// DXF ASCII text file reader
pub struct DxfTextReader<R: Read + Seek> {
    reader: BufReader<R>,
    line_number: usize,
    peeked_pair: Option<DxfCodePair>,
    /// Non-UTF8 fallback encoding.  `None` means use Latin-1 (byte-to-char).
    encoding: Option<&'static Encoding>,
}

impl<R: Read + Seek> DxfTextReader<R> {
    /// Create a new DXF text reader
    pub fn new(reader: BufReader<R>) -> Result<Self> {
        Ok(Self {
            reader,
            line_number: 0,
            peeked_pair: None,
            encoding: None,
        })
    }
    
    /// Read a single line from the stream, handling non-UTF8 bytes gracefully.
    /// Uses the configured encoding for fallback, or Latin-1 if none set.
    fn read_line(&mut self) -> Result<Option<String>> {
        let mut bytes = Vec::new();
        
        // Read bytes until newline
        loop {
            let mut byte = [0u8; 1];
            match self.reader.read(&mut byte) {
                Ok(0) => {
                    // EOF
                    if bytes.is_empty() {
                        return Ok(None);
                    }
                    break;
                }
                Ok(_) => {
                    if byte[0] == b'\n' {
                        break;
                    }
                    bytes.push(byte[0]);
                }
                Err(e) => return Err(e.into()),
            }
        }
        
        self.line_number += 1;
        
        // Try UTF-8 first, then use configured encoding or Latin-1 fallback
        let line = match String::from_utf8(bytes.clone()) {
            Ok(s) => s,
            Err(_) => {
                if let Some(enc) = self.encoding {
                    let (decoded, _, _) = enc.decode(&bytes);
                    decoded.into_owned()
                } else {
                    // Latin-1 is a 1:1 mapping of bytes 0-255 to Unicode code points
                    bytes.iter().map(|&b| b as char).collect()
                }
            }
        };
        
        // Trim whitespace and newlines (including \r)
        let trimmed = line.trim().to_string();
        Ok(Some(trimmed))
    }
    
    /// Read a code/value pair from the stream
    fn read_pair_internal(&mut self) -> Result<Option<DxfCodePair>> {
        // Read code line
        let code_line = match self.read_line()? {
            Some(line) => line,
            None => return Ok(None),
        };
        
        // Parse code
        let code = code_line.trim().parse::<i32>()
            .map_err(|_| DxfError::Parse(format!("Invalid DXF code at line {}: '{}'", self.line_number, code_line)))?;
        
        // Read value line
        let value_line = match self.read_line()? {
            Some(line) => line,
            None => return Err(DxfError::Parse(format!("Unexpected EOF after code {} at line {}", code, self.line_number))),
        };
        
        // Process special character sequences in strings
        let value = self.process_string_value(&value_line);
        
        Ok(Some(DxfCodePair::new(code, value)))
    }
    
    /// Process special character sequences in DXF strings
    fn process_string_value(&self, value: &str) -> String {
        value
            .replace("^J", "\n")
            .replace("^M", "\r")
            .replace("^I", "\t")
            .replace("^ ", "^")
    }
}

impl<R: Read + Seek> DxfStreamReader for DxfTextReader<R> {
    fn read_pair(&mut self) -> Result<Option<DxfCodePair>> {
        // If we have a peeked pair, return it
        if let Some(pair) = self.peeked_pair.take() {
            return Ok(Some(pair));
        }
        
        self.read_pair_internal()
    }
    
    fn peek_code(&mut self) -> Result<Option<i32>> {
        // If we already have a peeked pair, return its code
        if let Some(ref pair) = self.peeked_pair {
            return Ok(Some(pair.code));
        }
        
        // Read the next pair and store it
        if let Some(pair) = self.read_pair_internal()? {
            let code = pair.code;
            self.peeked_pair = Some(pair);
            Ok(Some(code))
        } else {
            Ok(None)
        }
    }

    fn push_back(&mut self, pair: DxfCodePair) {
        self.peeked_pair = Some(pair);
    }
    
    fn reset(&mut self) -> Result<()> {
        self.reader.seek(SeekFrom::Start(0))?;
        self.line_number = 0;
        self.peeked_pair = None;
        Ok(())
    }

    fn set_encoding(&mut self, encoding: &'static Encoding) {
        self.encoding = Some(encoding);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    
    #[test]
    fn test_read_simple_pair() {
        let data = "0\nSECTION\n";
        let cursor = Cursor::new(data.as_bytes());
        let buf_reader = BufReader::new(cursor);
        let mut reader = DxfTextReader::new(buf_reader).unwrap();
        
        let pair = reader.read_pair().unwrap().unwrap();
        assert_eq!(pair.code, 0);
        assert_eq!(pair.value_string, "SECTION");
    }
    
    #[test]
    fn test_read_integer_pair() {
        let data = "70\n42\n";
        let cursor = Cursor::new(data.as_bytes());
        let buf_reader = BufReader::new(cursor);
        let mut reader = DxfTextReader::new(buf_reader).unwrap();
        
        let pair = reader.read_pair().unwrap().unwrap();
        assert_eq!(pair.code, 70);
        assert_eq!(pair.as_int(), Some(42));
    }
    
    #[test]
    fn test_read_double_pair() {
        let data = "10\n123.456\n";
        let cursor = Cursor::new(data.as_bytes());
        let buf_reader = BufReader::new(cursor);
        let mut reader = DxfTextReader::new(buf_reader).unwrap();
        
        let pair = reader.read_pair().unwrap().unwrap();
        assert_eq!(pair.code, 10);
        assert_eq!(pair.as_double(), Some(123.456));
    }
    
    #[test]
    fn test_peek_code() {
        let data = "0\nSECTION\n2\nHEADER\n";
        let cursor = Cursor::new(data.as_bytes());
        let buf_reader = BufReader::new(cursor);
        let mut reader = DxfTextReader::new(buf_reader).unwrap();
        
        // Peek should return 0
        assert_eq!(reader.peek_code().unwrap(), Some(0));
        
        // Read should return the same pair
        let pair = reader.read_pair().unwrap().unwrap();
        assert_eq!(pair.code, 0);
        
        // Next peek should return 2
        assert_eq!(reader.peek_code().unwrap(), Some(2));
    }
    
    #[test]
    fn test_special_characters() {
        let data = "1\nLine1^JLine2^MLine3\n";
        let cursor = Cursor::new(data.as_bytes());
        let buf_reader = BufReader::new(cursor);
        let mut reader = DxfTextReader::new(buf_reader).unwrap();
        
        let pair = reader.read_pair().unwrap().unwrap();
        assert_eq!(pair.value_string, "Line1\nLine2\rLine3");
    }
}


