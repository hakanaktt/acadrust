//! DXF ASCII text reader

use super::stream_reader::{DxfCodePair, DxfStreamContext, DxfStreamReader};
use crate::error::{DxfError, Result};
use encoding_rs::Encoding;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};

/// DXF ASCII text file reader
pub struct DxfTextReader<R: Read + Seek> {
    reader: BufReader<R>,
    line_number: usize,
    stream_offset: u64,
    peeked_pair: Option<DxfCodePair>,
    /// Non-UTF8 fallback encoding.  `None` means use Latin-1 (byte-to-char).
    encoding: Option<&'static Encoding>,
    /// Reusable buffer for the code line, to avoid per-line allocation.
    line_buf: Vec<u8>,
    /// Reusable buffer for the value line. Kept separate from `line_buf` so
    /// neither read has to surrender its allocation to the other.
    value_buf: Vec<u8>,
    context: DxfStreamContext,
}

impl<R: Read + Seek> DxfTextReader<R> {
    /// Create a new DXF text reader
    pub fn new(reader: BufReader<R>) -> Result<Self> {
        Ok(Self {
            reader,
            line_number: 0,
            stream_offset: 0,
            peeked_pair: None,
            encoding: None,
            line_buf: Vec::with_capacity(256),
            value_buf: Vec::with_capacity(256),
            context: DxfStreamContext::default(),
        })
    }

    /// Read the value line into the reusable `value_buf`, keeping the buffer's
    /// allocation. Returns `Ok(true)` if a line was read, `Ok(false)` on EOF.
    fn read_value_line_raw(&mut self) -> Result<bool> {
        self.value_buf.clear();
        let bytes_read = self.reader.read_until(b'\n', &mut self.value_buf)?;
        if bytes_read == 0 {
            return Ok(false);
        }

        self.line_number += 1;
        self.stream_offset = self.stream_offset.saturating_add(bytes_read as u64);

        // Strip trailing \n and \r in-place
        if self.value_buf.last() == Some(&b'\n') {
            self.value_buf.pop();
        }
        if self.value_buf.last() == Some(&b'\r') {
            self.value_buf.pop();
        }
        Ok(true)
    }

    /// Decode `value_buf` into the owned `String` the code pair carries,
    /// handling non-UTF8 bytes via the configured encoding (Latin-1 if none).
    ///
    /// One allocation per code pair — the `String` the pair must own anyway.
    /// The previous version allocated twice: once to hand `line_buf`'s
    /// allocation to a `String` (forcing a fresh buffer for the next line),
    /// and again to copy that `String` in `process_string_value`.
    fn decode_value_string(&self) -> String {
        match std::str::from_utf8(&self.value_buf) {
            Ok(text) => self.process_string_value(text),
            Err(_) => {
                let decoded = if let Some(enc) = self.encoding {
                    // DXF carries junk bytes often enough that a malformed
                    // sequence is not worth failing the read over; take the
                    // replacement characters and keep going.
                    let (decoded, _, _) = enc.decode(&self.value_buf);
                    decoded.into_owned()
                } else {
                    // Fallback to Latin-1: each byte is its own code point.
                    self.value_buf.iter().map(|&b| b as char).collect()
                };
                // Already owned, so only re-copy when there is an escape to expand.
                if decoded.contains('^') {
                    self.process_string_value(&decoded)
                } else {
                    decoded
                }
            }
        }
    }

    /// Read a line into the reusable `line_buf` without allocating a String.
    /// Returns `Ok(true)` if a line was read, `Ok(false)` on EOF.
    fn read_line_raw(&mut self) -> Result<bool> {
        self.line_buf.clear();
        let bytes_read = self.reader.read_until(b'\n', &mut self.line_buf)?;
        if bytes_read == 0 {
            return Ok(false);
        }
        self.line_number += 1;
        self.stream_offset = self.stream_offset.saturating_add(bytes_read as u64);
        if self.line_buf.last() == Some(&b'\n') {
            self.line_buf.pop();
        }
        if self.line_buf.last() == Some(&b'\r') {
            self.line_buf.pop();
        }
        Ok(true)
    }

    /// Read a code/value pair from the stream
    fn read_pair_internal(&mut self) -> Result<Option<DxfCodePair>> {
        let pair_offset = Some(self.stream_offset);
        self.context.source_offset = pair_offset;
        self.context.source_line = Some(self.line_number.saturating_add(1));

        // Read code line into reusable buffer (no String allocation needed)
        if !self.read_line_raw()? {
            return Ok(None);
        }

        // Parse code directly from byte buffer
        let code_str = std::str::from_utf8(&self.line_buf).map_err(|_| {
            DxfError::Parse(format!(
                "Invalid UTF-8 in code at line {}",
                self.line_number
            ))
        })?;
        let code = code_str.trim().parse::<i32>().map_err(|_| {
            DxfError::Parse(format!(
                "Invalid DXF code at line {}: '{}'",
                self.line_number, code_str
            ))
        })?;

        // Read the value line into its own reusable buffer, then decode once.
        if !self.read_value_line_raw()? {
            return Err(DxfError::Parse(format!(
                "Unexpected EOF after code {} at line {}",
                code, self.line_number
            )));
        }
        let value = self.decode_value_string();

        let pair = DxfCodePair::new(code, value);
        self.observe_pair(&pair, pair_offset);
        Ok(Some(pair))
    }

    fn observe_pair(&mut self, pair: &DxfCodePair, pair_offset: Option<u64>) {
        self.context.source_offset = pair_offset;
        self.context.source_line = Some(self.line_number.saturating_sub(1));
        if pair.code == 0 {
            self.context.record_type = Some(pair.value_string.clone());
            self.context.record_handle = None;
        } else if pair.code == 105
            || (pair.code == 5 && self.context.record_type.as_deref() != Some("DIMSTYLE"))
        {
            self.context.record_handle = pair.as_handle();
        }
    }

    /// Process special character sequences in DXF strings
    fn process_string_value(&self, value: &str) -> String {
        // Fast path: most DXF values contain no escape sequences
        if !value.contains('^') {
            return value.to_string();
        }
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
        self.stream_offset = 0;
        self.peeked_pair = None;
        self.context = DxfStreamContext::default();
        Ok(())
    }

    fn set_encoding(&mut self, encoding: &'static Encoding) {
        self.encoding = Some(encoding);
    }

    fn diagnostic_context(&self) -> DxfStreamContext {
        self.context.clone()
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
