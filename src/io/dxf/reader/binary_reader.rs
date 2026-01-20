//! DXF binary reader

use super::stream_reader::{DxfCodePair, DxfStreamReader};
use crate::error::{DxfError, Result};
use std::io::{BufReader, Read, Seek, SeekFrom};

/// Sentinel for binary DXF files
pub const BINARY_SENTINEL: &[u8] = b"AutoCAD Binary DXF\r\n\x1a\x00";

/// DXF binary file reader
pub struct DxfBinaryReader<R: Read + Seek> {
    reader: BufReader<R>,
    position: u64,
    peeked_pair: Option<DxfCodePair>,
    /// True for pre-AC1012 format (single-byte group codes)
    /// False for AC1012+ format (two-byte group codes)
    use_single_byte_codes: bool,
}

impl<R: Read + Seek> DxfBinaryReader<R> {
    /// Create a new DXF binary reader
    pub fn new(mut reader: BufReader<R>) -> Result<Self> {
        // Verify sentinel
        let mut sentinel = vec![0u8; BINARY_SENTINEL.len()];
        reader.read_exact(&mut sentinel)?;
        
        if sentinel != BINARY_SENTINEL {
            return Err(DxfError::Parse("Invalid binary DXF sentinel".to_string()));
        }
        
        // Detect format by checking the first group code
        // In pre-AC1012, after sentinel we have: [code_byte][string...]
        // In AC1012+, we have: [code_lo][code_hi][string...]
        // The first code should be 0 (for SECTION), so:
        // - Pre-AC1012: byte 0 = 0x00, byte 1 = 'S' (0x53)
        // - AC1012+: byte 0 = 0x00, byte 1 = 0x00, byte 2 = 'S' (0x53)
        let mut probe = [0u8; 2];
        reader.read_exact(&mut probe)?;
        reader.seek(SeekFrom::Start(BINARY_SENTINEL.len() as u64))?;
        
        // If second byte is printable ASCII (like 'S' for SECTION), it's pre-AC1012
        let use_single_byte_codes = probe[0] == 0 && probe[1] >= 0x20 && probe[1] < 0x7F;
        
        Ok(Self {
            reader,
            position: BINARY_SENTINEL.len() as u64,
            peeked_pair: None,
            use_single_byte_codes,
        })
    }
    
    /// Read a code/value pair from the binary stream
    fn read_pair_internal(&mut self) -> Result<Option<DxfCodePair>> {
        let code = if self.use_single_byte_codes {
            // Pre-AC1012: single byte codes, with 255 as escape for extended codes
            let mut code_byte = [0u8; 1];
            match self.reader.read_exact(&mut code_byte) {
                Ok(_) => {},
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
                Err(e) => return Err(e.into()),
            }
            self.position += 1;
            
            if code_byte[0] == 255 {
                // Extended code: next 2 bytes are the actual code
                let mut ext_code = [0u8; 2];
                self.reader.read_exact(&mut ext_code)?;
                self.position += 2;
                i16::from_le_bytes(ext_code) as i32
            } else {
                code_byte[0] as i32
            }
        } else {
            // AC1012+: 2-byte codes, little-endian
            let mut code_bytes = [0u8; 2];
            match self.reader.read_exact(&mut code_bytes) {
                Ok(_) => {},
                Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(None),
                Err(e) => return Err(e.into()),
            }
            self.position += 2;
            i16::from_le_bytes(code_bytes) as i32
        };
        
        // Read value based on code type
        let value = self.read_value_for_code(code)?;
        
        Ok(Some(DxfCodePair::new(code, value)))
    }
    
    /// Read a value from the binary stream based on the group code
    fn read_value_for_code(&mut self, code: i32) -> Result<String> {
        use crate::io::dxf::{DxfCode, GroupCodeValueType};
        
        let dxf_code = DxfCode::from_i32(code);
        let value_type = GroupCodeValueType::from_code(dxf_code);
        
        match value_type {
            GroupCodeValueType::String => {
                // Null-terminated string
                let mut bytes = Vec::new();
                loop {
                    let mut byte = [0u8; 1];
                    self.reader.read_exact(&mut byte)?;
                    self.position += 1;
                    
                    if byte[0] == 0 {
                        break;
                    }
                    bytes.push(byte[0]);
                }
                
                // Try UTF-8 first, then fall back to lossy conversion for Windows-1252/CP1252
                match String::from_utf8(bytes.clone()) {
                    Ok(s) => Ok(s),
                    Err(_) => {
                        // Fall back to lossy conversion (replaces invalid bytes with replacement char)
                        Ok(String::from_utf8_lossy(&bytes).into_owned())
                    }
                }
            }
            
            GroupCodeValueType::Double => {
                // 8-byte double
                let mut bytes = [0u8; 8];
                self.reader.read_exact(&mut bytes)?;
                self.position += 8;
                
                let value = f64::from_le_bytes(bytes);
                Ok(value.to_string())
            }
            
            GroupCodeValueType::Int16 | GroupCodeValueType::Byte => {
                // 2-byte integer
                let mut bytes = [0u8; 2];
                self.reader.read_exact(&mut bytes)?;
                self.position += 2;
                
                let value = i16::from_le_bytes(bytes);
                Ok(value.to_string())
            }
            
            GroupCodeValueType::Int32 => {
                // 4-byte integer
                let mut bytes = [0u8; 4];
                self.reader.read_exact(&mut bytes)?;
                self.position += 4;
                
                let value = i32::from_le_bytes(bytes);
                Ok(value.to_string())
            }
            
            GroupCodeValueType::Int64 => {
                // 8-byte integer
                let mut bytes = [0u8; 8];
                self.reader.read_exact(&mut bytes)?;
                self.position += 8;
                
                let value = i64::from_le_bytes(bytes);
                Ok(value.to_string())
            }
            
            GroupCodeValueType::Bool => {
                // 1-byte boolean
                let mut byte = [0u8; 1];
                self.reader.read_exact(&mut byte)?;
                self.position += 1;
                
                Ok(if byte[0] != 0 { "1" } else { "0" }.to_string())
            }
            
            GroupCodeValueType::BinaryData | GroupCodeValueType::Handle => {
                // Null-terminated hex string
                let mut bytes = Vec::new();
                loop {
                    let mut byte = [0u8; 1];
                    self.reader.read_exact(&mut byte)?;
                    self.position += 1;
                    
                    if byte[0] == 0 {
                        break;
                    }
                    bytes.push(byte[0]);
                }
                
                // Handles should be ASCII hex, but use lossy for robustness
                Ok(String::from_utf8_lossy(&bytes).into_owned())
            }
            
            _ => {
                // Default to string - use lossy for Windows-1252 compatibility
                let mut bytes = Vec::new();
                loop {
                    let mut byte = [0u8; 1];
                    self.reader.read_exact(&mut byte)?;
                    self.position += 1;
                    
                    if byte[0] == 0 {
                        break;
                    }
                    bytes.push(byte[0]);
                }
                
                Ok(String::from_utf8_lossy(&bytes).into_owned())
            }
        }
    }
}

impl<R: Read + Seek> DxfStreamReader for DxfBinaryReader<R> {
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
        self.position = 0;
        self.peeked_pair = None;
        
        // Re-verify sentinel
        let mut sentinel = vec![0u8; BINARY_SENTINEL.len()];
        self.reader.read_exact(&mut sentinel)?;
        
        if sentinel != BINARY_SENTINEL {
            return Err(DxfError::Parse("Invalid binary DXF sentinel".to_string()));
        }
        
        // Re-detect format (should be same as before, but just re-skip the probe bytes)
        let mut probe = [0u8; 2];
        self.reader.read_exact(&mut probe)?;
        self.reader.seek(SeekFrom::Start(BINARY_SENTINEL.len() as u64))?;
        
        self.position = BINARY_SENTINEL.len() as u64;
        Ok(())
    }
}

