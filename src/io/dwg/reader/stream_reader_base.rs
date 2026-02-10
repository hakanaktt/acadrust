//! Base implementation of `IDwgStreamReader` with bit-level I/O.
//!
//! Mirrors ACadSharp's `DwgStreamReaderBase` (1026 lines of C#).
//!
//! This provides the core bit-manipulation implementations for reading
//! all DWG data types. Version-specific readers override a few methods.

use crate::error::{DxfError, Result};
use crate::io::dwg::object_type::DwgObjectType;
use crate::io::dwg::reference_type::DwgReferenceType;
use crate::types::{Color, DxfVersion, Transparency, Vector2, Vector3};

use super::stream_reader::IDwgStreamReader;

use std::io::{self, Cursor, Read, Seek, SeekFrom};

use encoding_rs::Encoding;

/// Base implementation of the bit-level DWG stream reader.
///
/// All version-specific behavior is handled by overriding individual methods
/// in the version-specific wrappers.
pub struct DwgStreamReaderBase {
    stream: Cursor<Vec<u8>>,
    bit_shift: u8,
    last_byte: u8,
    is_empty: bool,
    encoding: &'static Encoding,
    version: DxfVersion,
}

impl DwgStreamReaderBase {
    /// Create a new reader wrapping raw data bytes.
    pub fn new(data: Vec<u8>, version: DxfVersion) -> Self {
        Self {
            stream: Cursor::new(data),
            bit_shift: 0,
            last_byte: 0,
            is_empty: false,
            encoding: encoding_rs::WINDOWS_1252,
            version,
        }
    }

    /// Create from a stream position (for sub-stream use).
    pub fn new_at(data: Vec<u8>, version: DxfVersion, position: u64) -> Self {
        let mut reader = Self::new(data, version);
        reader.set_position(position);
        reader
    }

    /// Get the DXF version.
    pub fn version(&self) -> DxfVersion {
        self.version
    }

    /// Set the text encoding.
    pub fn set_encoding(&mut self, encoding: &'static Encoding) {
        self.encoding = encoding;
    }

    /// Get the text encoding.
    pub fn encoding(&self) -> &'static Encoding {
        self.encoding
    }

    /// Get the underlying data length.
    pub fn stream_length(&self) -> u64 {
        self.stream.get_ref().len() as u64
    }

    // ---------------------------------------------------------------
    // Internal helpers
    // ---------------------------------------------------------------

    /// Read a raw byte from the underlying stream (no bit-shift).
    fn read_raw_byte(&mut self) -> Result<u8> {
        let mut buf = [0u8; 1];
        self.stream
            .read_exact(&mut buf)
            .map_err(|e| DxfError::Io(e))?;
        Ok(buf[0])
    }

    /// Apply bit-shift to the last byte to get a "shifted" byte value.
    fn apply_shift_to_last_byte(&mut self) -> Result<u8> {
        let value = (self.last_byte as u16) << self.bit_shift;
        self.advance_byte();
        Ok((value as u8) | (self.last_byte >> (8 - self.bit_shift)))
    }

    /// Apply bit-shift to a buffer of raw bytes.
    fn apply_shift_to_arr(&mut self, arr: &mut [u8]) -> Result<()> {
        self.apply_shift_to_arr_at(arr, 0, arr.len())
    }

    /// Apply bit-shift reading `length` bytes from the stream into `arr`
    /// starting at `offset`.
    fn apply_shift_to_arr_at(
        &mut self,
        arr: &mut [u8],
        offset: usize,
        length: usize,
    ) -> Result<()> {
        let mut raw = vec![0u8; length];
        let n = self.stream.read(&mut raw)?;
        if n != length {
            return Err(DxfError::Io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "unexpected end of stream",
            )));
        }

        if self.bit_shift == 0 {
            arr[offset..offset + length].copy_from_slice(&raw);
            return Ok(());
        }

        let shift = 8 - self.bit_shift;
        for i in 0..length {
            let last_byte_value = (self.last_byte as u16) << self.bit_shift;
            self.last_byte = raw[i];
            let value = (last_byte_value as u8) | (self.last_byte >> shift);
            arr[offset + i] = value;
        }
        Ok(())
    }

    /// Read 3 bits (used by BitLongLong).
    fn read_3bits(&mut self) -> Result<u8> {
        let b1 = if self.read_bit()? { 1u8 } else { 0u8 };
        let b2 = (b1 << 1) | if self.read_bit()? { 1u8 } else { 0u8 };
        let b3 = (b2 << 1) | if self.read_bit()? { 1u8 } else { 0u8 };
        Ok(b3)
    }

    /// Read a handle's byte payload (big-endian, returned as u64 LE).
    fn read_handle_bytes(&mut self, length: usize) -> Result<u64> {
        if length > 8 {
            return Err(DxfError::InvalidFormat(format!(
                "Handle byte count {} exceeds maximum of 8",
                length
            )));
        }
        let mut raw = vec![0u8; length];
        let mut arr = [0u8; 8];

        let n = self.stream.read(&mut raw)?;
        if n < length {
            return Err(DxfError::Io(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "unexpected end of stream reading handle",
            )));
        }

        if self.bit_shift == 0 {
            // Set the array backwards (big-endian → little-endian)
            for i in 0..length {
                arr[length - 1 - i] = raw[i];
            }
        } else {
            let shift = 8 - self.bit_shift;
            for i in 0..length {
                let last_byte_value = (self.last_byte as u16) << self.bit_shift;
                self.last_byte = raw[i];
                let value = (last_byte_value as u8) | (self.last_byte >> shift);
                arr[length - 1 - i] = value;
            }
        }

        // Zero the remaining bytes
        for index in length..8 {
            arr[index] = 0;
        }

        Ok(u64::from_le_bytes(arr))
    }

    /// Convert Julian date to a floating-point timestamp.
    fn julian_to_timestamp(jdate: i32, milliseconds: i32) -> f64 {
        // Julian day 2440587.5 = Unix epoch (1970-01-01 00:00:00 UTC)
        let unix_time = (jdate as f64 - 2440587.5) * 86400.0;
        unix_time + (milliseconds as f64 / 1000.0)
    }

    /// Apply the flag-based position for string streams.
    fn apply_flag_to_position(&mut self, last_pos: i64) -> Result<(i64, i64)> {
        // If 1, then the "endbit" location should be decremented by 16 bytes
        let mut length = last_pos - 16;
        self.set_position_in_bits(length);

        // Short should be read at location endbit - 128 (bits)
        let mut str_data_size = self.read_raw_ushort()? as i64;

        // If this short has the 0x8000 bit set,
        // then decrement endbit by an additional 16 bytes,
        // strip the 0x8000 bit off of strDataSize, and read
        // the short at this new location, calling it hiSize.
        if (str_data_size & 0x8000) != 0 {
            length -= 16;
            self.set_position_in_bits(length);

            str_data_size &= 0x7FFF;

            let hi_size = self.read_raw_ushort()? as i64;
            // Set strDataSize to (strDataSize | (hiSize << 15))
            str_data_size += (hi_size & 0xFFFF) << 15;
        }

        Ok((length, str_data_size))
    }

    /// Read an LE i16 (with bit-shift if needed).
    fn read_short_le(&mut self) -> Result<i16> {
        let b0 = self.read_byte()? as u16;
        let b1 = self.read_byte()? as u16;
        Ok((b0 | (b1 << 8)) as i16)
    }

    /// Read an LE u16 (with bit-shift if needed).
    fn read_ushort_le(&mut self) -> Result<u16> {
        let b0 = self.read_byte()? as u16;
        let b1 = self.read_byte()? as u16;
        Ok(b0 | (b1 << 8))
    }

    /// Read an LE i32 (with bit-shift if needed).
    fn read_int_le(&mut self) -> Result<i32> {
        let b0 = self.read_byte()? as u32;
        let b1 = self.read_byte()? as u32;
        let b2 = self.read_byte()? as u32;
        let b3 = self.read_byte()? as u32;
        Ok((b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)) as i32)
    }

    /// Read an LE u32 (with bit-shift if needed).
    fn read_uint_le(&mut self) -> Result<u32> {
        let b0 = self.read_byte()? as u32;
        let b1 = self.read_byte()? as u32;
        let b2 = self.read_byte()? as u32;
        let b3 = self.read_byte()? as u32;
        Ok(b0 | (b1 << 8) | (b2 << 16) | (b3 << 24))
    }

    /// Read an LE u64 (with bit-shift if needed).
    fn read_ulong_le(&mut self) -> Result<u64> {
        let lo = self.read_uint_le()? as u64;
        let hi = self.read_uint_le()? as u64;
        Ok(lo | (hi << 32))
    }

    /// Read an LE f64 (with bit-shift if needed).
    fn read_double_le(&mut self) -> Result<f64> {
        let bytes = self.read_bytes(8)?;
        Ok(f64::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]))
    }

    /// Read a string of `length` bytes using the given encoding.
    fn read_string(&mut self, length: usize, encoding: &'static Encoding) -> Result<String> {
        let bytes = self.read_bytes(length)?;
        let (decoded, _, _) = encoding.decode(&bytes);
        Ok(decoded.into_owned())
    }
}

impl IDwgStreamReader for DwgStreamReaderBase {
    fn bit_shift(&self) -> u8 {
        self.bit_shift
    }

    fn set_bit_shift(&mut self, shift: u8) {
        self.bit_shift = shift;
    }

    fn is_empty(&self) -> bool {
        self.is_empty
    }

    fn position(&self) -> u64 {
        self.stream.position()
    }

    fn set_position(&mut self, pos: u64) {
        self.stream.set_position(pos);
        self.bit_shift = 0;
    }

    fn stream(&mut self) -> &mut dyn super::stream_reader::ReadSeek {
        &mut self.stream
    }

    fn advance_byte(&mut self) {
        self.last_byte = self.read_raw_byte().unwrap_or(0);
    }

    fn advance(&mut self, offset: usize) {
        if offset > 1 {
            let _ = self.stream.seek(SeekFrom::Current((offset - 1) as i64));
        }
        let _ = self.read_byte();
    }

    // ---------------------------------------------------------------
    // BIT CODES AND DATA DEFINITIONS
    // ---------------------------------------------------------------

    fn read_bit(&mut self) -> Result<bool> {
        if self.bit_shift == 0 {
            self.advance_byte();
            let result = (self.last_byte & 128) == 128;
            self.bit_shift = 1;
            return Ok(result);
        }

        let value = ((self.last_byte << self.bit_shift) & 128) == 128;

        self.bit_shift += 1;
        self.bit_shift &= 7;

        Ok(value)
    }

    fn read_bit_as_short(&mut self) -> Result<i16> {
        Ok(if self.read_bit()? { 1 } else { 0 })
    }

    fn read_2bits(&mut self) -> Result<u8> {
        let value;
        if self.bit_shift == 0 {
            self.advance_byte();
            value = self.last_byte >> 6;
            self.bit_shift = 2;
        } else if self.bit_shift == 7 {
            let last_value = (self.last_byte << 1) & 2;
            self.advance_byte();
            value = last_value | (self.last_byte >> 7);
            self.bit_shift = 1;
        } else {
            value = (self.last_byte >> (6 - self.bit_shift)) & 3;
            self.bit_shift += 2;
            self.bit_shift &= 7;
        }
        Ok(value)
    }

    fn read_bit_short(&mut self) -> Result<i16> {
        match self.read_2bits()? {
            0 => {
                // 00: A short (2 bytes) follows, little-endian order (LSB first)
                self.read_short_le()
            }
            1 => {
                // 01: An unsigned char (1 byte) follows
                if self.bit_shift == 0 {
                    self.advance_byte();
                    Ok(self.last_byte as i16)
                } else {
                    Ok(self.apply_shift_to_last_byte()? as i16)
                }
            }
            2 => {
                // 10: 0
                Ok(0)
            }
            3 => {
                // 11: 256
                Ok(256)
            }
            _ => unreachable!(),
        }
    }

    fn read_bit_short_as_bool(&mut self) -> Result<bool> {
        Ok(self.read_bit_short()? != 0)
    }

    fn read_bit_long(&mut self) -> Result<i32> {
        match self.read_2bits()? {
            0 => {
                // 00: A long (4 bytes) follows, little-endian order
                self.read_int_le()
            }
            1 => {
                // 01: An unsigned char (1 byte) follows
                if self.bit_shift == 0 {
                    self.advance_byte();
                    Ok(self.last_byte as i32)
                } else {
                    Ok(self.apply_shift_to_last_byte()? as i32)
                }
            }
            2 => {
                // 10: 0
                Ok(0)
            }
            _ => {
                // 11: not used
                Err(DxfError::Parse("Failed to read BitLong".into()))
            }
        }
    }

    fn read_bit_long_long(&mut self) -> Result<i64> {
        let mut value: u64 = 0;
        let size = self.read_3bits()?;

        for i in 0..size {
            let b = self.read_byte()? as u64;
            value += b << ((i as u64) << 3);
        }

        Ok(value as i64)
    }

    fn read_bit_double(&mut self) -> Result<f64> {
        match self.read_2bits()? {
            0 => self.read_double_le(),
            1 => Ok(1.0),
            2 => Ok(0.0),
            _ => Err(DxfError::Parse("Failed to read BitDouble".into())),
        }
    }

    fn read_bit_double_with_default(&mut self, def: f64) -> Result<f64> {
        let mut arr = def.to_le_bytes();

        match self.read_2bits()? {
            0 => {
                // 00: No more data present, use default.
                Ok(def)
            }
            1 => {
                // 01: 4 bytes patched into first 4 bytes of default
                if self.bit_shift == 0 {
                    self.advance_byte();
                    arr[0] = self.last_byte;
                    self.advance_byte();
                    arr[1] = self.last_byte;
                    self.advance_byte();
                    arr[2] = self.last_byte;
                    self.advance_byte();
                    arr[3] = self.last_byte;
                } else {
                    self.apply_shift_to_arr_at(&mut arr, 0, 4)?;
                }
                Ok(f64::from_le_bytes(arr))
            }
            2 => {
                // 10: 6 bytes — first 2 patch bytes [4..6]; last 4 patch bytes [0..4]
                if self.bit_shift == 0 {
                    self.advance_byte();
                    arr[4] = self.last_byte;
                    self.advance_byte();
                    arr[5] = self.last_byte;
                    self.advance_byte();
                    arr[0] = self.last_byte;
                    self.advance_byte();
                    arr[1] = self.last_byte;
                    self.advance_byte();
                    arr[2] = self.last_byte;
                    self.advance_byte();
                    arr[3] = self.last_byte;
                } else {
                    self.apply_shift_to_arr_at(&mut arr, 4, 2)?;
                    self.apply_shift_to_arr_at(&mut arr, 0, 4)?;
                }
                Ok(f64::from_le_bytes(arr))
            }
            3 => {
                // 11: A full RD follows
                self.read_double_le()
            }
            _ => unreachable!(),
        }
    }

    fn read_2bit_double(&mut self) -> Result<Vector2> {
        let x = self.read_bit_double()?;
        let y = self.read_bit_double()?;
        Ok(Vector2::new(x, y))
    }

    fn read_2bit_double_with_default(&mut self, def: Vector2) -> Result<Vector2> {
        let x = self.read_bit_double_with_default(def.x)?;
        let y = self.read_bit_double_with_default(def.y)?;
        Ok(Vector2::new(x, y))
    }

    fn read_3bit_double(&mut self) -> Result<Vector3> {
        let x = self.read_bit_double()?;
        let y = self.read_bit_double()?;
        let z = self.read_bit_double()?;
        Ok(Vector3::new(x, y, z))
    }

    fn read_3bit_double_with_default(&mut self, def: Vector3) -> Result<Vector3> {
        let x = self.read_bit_double_with_default(def.x)?;
        let y = self.read_bit_double_with_default(def.y)?;
        let z = self.read_bit_double_with_default(def.z)?;
        Ok(Vector3::new(x, y, z))
    }

    fn read_raw_char(&mut self) -> Result<u8> {
        self.read_byte()
    }

    fn read_raw_short(&mut self) -> Result<i16> {
        self.read_short_le()
    }

    fn read_raw_ushort(&mut self) -> Result<u16> {
        self.read_ushort_le()
    }

    fn read_raw_long(&mut self) -> Result<i32> {
        self.read_int_le()
    }

    fn read_raw_ulong(&mut self) -> Result<u64> {
        self.read_ulong_le()
    }

    fn read_raw_double(&mut self) -> Result<f64> {
        self.read_double_le()
    }

    fn read_2raw_double(&mut self) -> Result<Vector2> {
        let x = self.read_double_le()?;
        let y = self.read_double_le()?;
        Ok(Vector2::new(x, y))
    }

    fn read_3raw_double(&mut self) -> Result<Vector3> {
        let x = self.read_double_le()?;
        let y = self.read_double_le()?;
        let z = self.read_double_le()?;
        Ok(Vector3::new(x, y, z))
    }

    fn read_byte(&mut self) -> Result<u8> {
        if self.bit_shift == 0 {
            self.last_byte = self.read_raw_byte()?;
            return Ok(self.last_byte);
        }

        // Get the last bits from the last read byte
        let last_values = (self.last_byte as u16) << self.bit_shift;
        self.last_byte = self.read_raw_byte()?;
        Ok((last_values as u8) | (self.last_byte >> (8 - self.bit_shift)))
    }

    fn read_bytes(&mut self, length: usize) -> Result<Vec<u8>> {
        // Sanity check: prevent absurd allocations from corrupt data.
        if length > 16 * 1024 * 1024 {
            return Err(DxfError::InvalidFormat(format!(
                "Requested byte read of {} exceeds 16 MB sanity limit",
                length
            )));
        }
        let mut arr = vec![0u8; length];
        self.apply_shift_to_arr(&mut arr)?;
        Ok(arr)
    }

    fn read_modular_char(&mut self) -> Result<u64> {
        let mut shift = 0;
        let last_byte = self.read_byte()?;

        // Remove the flag
        let mut value = (last_byte & 0b0111_1111) as u64;

        if (last_byte & 0b1000_0000) != 0 {
            loop {
                shift += 7;
                let last = self.read_byte()?;
                value |= ((last & 0b0111_1111) as u64) << shift;

                // Check flag
                if (last & 0b1000_0000) == 0 {
                    break;
                }
            }
        }

        Ok(value)
    }

    fn read_signed_modular_char(&mut self) -> Result<i64> {
        let last_byte = self.read_byte()?;

        if (last_byte & 0b1000_0000) == 0 {
            // Single byte — drop the flags
            let mut value = (last_byte & 0b0011_1111) as i64;
            // Check the sign flag (bit 6)
            if (last_byte & 0b0100_0000) != 0 {
                value = -value;
            }
            return Ok(value);
        }

        // Multi-byte
        let mut total_shift = 0i32;
        let mut sum = (last_byte & 0b0111_1111) as i64;

        loop {
            total_shift += 7;
            let curr_byte = self.read_byte()?;

            if (curr_byte & 0b1000_0000) != 0 {
                sum |= ((curr_byte & 0b0111_1111) as i64) << total_shift;
            } else {
                // Last byte — drop flags and add value
                let mut value = sum | (((curr_byte & 0b0011_1111) as i64) << total_shift);

                // Check the sign flag
                if (curr_byte & 0b0100_0000) != 0 {
                    value = -value;
                }
                return Ok(value);
            }
        }
    }

    fn read_modular_short(&mut self) -> Result<i32> {
        let mut shift = 0b1111i32;

        // Read the bytes that form the short
        let b1 = self.read_byte()?;
        let b2 = self.read_byte()?;

        let mut flag = (b2 & 0b1000_0000) == 0;

        // Set the value in little endian
        let mut value = (b1 as i32) | (((b2 & 0b0111_1111) as i32) << 8);

        while !flag {
            // Read 2 more bytes
            let b1 = self.read_byte()?;
            let b2 = self.read_byte()?;

            // Check the flag
            flag = (b2 & 0b1000_0000) == 0;

            // Set the value in little endian
            value |= (b1 as i32) << shift;
            shift += 8;
            value |= ((b2 & 0b0111_1111) as i32) << shift;

            // Update the shift
            shift += 7;
        }

        Ok(value)
    }

    // ---------------------------------------------------------------
    // Handle references
    // ---------------------------------------------------------------

    fn handle_reference(&mut self) -> Result<u64> {
        let (handle, _) = self.handle_reference_typed(0)?;
        Ok(handle)
    }

    fn handle_reference_resolved(&mut self, reference_handle: u64) -> Result<u64> {
        let (handle, _) = self.handle_reference_typed(reference_handle)?;
        Ok(handle)
    }

    fn handle_reference_typed(
        &mut self,
        reference_handle: u64,
    ) -> Result<(u64, DwgReferenceType)> {
        // |CODE (4 bits)|COUNTER (4 bits)|HANDLE or OFFSET|
        let form = self.read_byte()?;

        // CODE of the reference
        let code = form >> 4;
        // COUNTER tells how many bytes of HANDLE follow
        let counter = (form & 0b0000_1111) as usize;

        // Get the reference type from the code
        let reference = DwgReferenceType::from_code(code)
            .unwrap_or(DwgReferenceType::Undefined);

        let initial_pos;

        match code {
            // 0x2, 0x3, 0x4, 0x5 — just read offset and use it as the result
            0..=5 => {
                initial_pos = self.read_handle_bytes(counter)?;
            }
            // 0x6 — result is reference handle + 1
            0x6 => {
                initial_pos = reference_handle.wrapping_add(1);
            }
            // 0x8 — result is reference handle - 1
            0x8 => {
                initial_pos = reference_handle.wrapping_sub(1);
            }
            // 0xA — result is reference handle plus offset
            0xA => {
                let offset = self.read_handle_bytes(counter)?;
                initial_pos = reference_handle.wrapping_add(offset);
            }
            // 0xC — result is reference handle minus offset
            0xC => {
                let offset = self.read_handle_bytes(counter)?;
                initial_pos = reference_handle.wrapping_sub(offset);
            }
            _ => {
                return Err(DxfError::Parse(format!(
                    "[HandleReference] invalid reference code with value: {code}"
                )));
            }
        }

        Ok((initial_pos, reference))
    }

    // ---------------------------------------------------------------
    // Text
    // ---------------------------------------------------------------

    fn read_text_unicode(&mut self) -> Result<String> {
        if self.version >= DxfVersion::AC1021 {
            // AC21+: Unicode text
            let text_length = self.read_short_le()?;
            if text_length == 0 {
                return Ok(String::new());
            }
            // Correct the text length by shifting 1 bit (each char is 2 bytes)
            let byte_length = (text_length << 1) as usize;
            let s = self.read_string(byte_length, encoding_rs::UTF_16LE)?;
            Ok(s.replace('\0', ""))
        } else {
            // Pre-AC21: code-page text
            let text_length = self.read_short_le()? as usize;
            let encoding_key = self.read_byte()?;
            if text_length == 0 {
                return Ok(String::new());
            }
            let enc = encoding_from_code_page(encoding_key);
            self.read_string(text_length, enc)
        }
    }

    fn read_variable_text(&mut self) -> Result<String> {
        if self.version >= DxfVersion::AC1021 {
            // AC21+: Unicode variable text
            let text_length = self.read_bit_short()?;
            if text_length == 0 {
                return Ok(String::new());
            }
            // Correct the text length by shifting 1 bit (each char is 2 bytes)
            let byte_length = (text_length << 1) as usize;
            let s = self.read_string(byte_length, encoding_rs::UTF_16LE)?;
            Ok(s.replace('\0', ""))
        } else {
            // Pre-AC21: code-page variable text
            let length = self.read_bit_short()? as usize;
            if length == 0 {
                return Ok(String::new());
            }
            let s = self.read_string(length, self.encoding)?;
            Ok(s.replace('\0', ""))
        }
    }

    // ---------------------------------------------------------------
    // Sentinel
    // ---------------------------------------------------------------

    fn read_sentinel(&mut self) -> Result<[u8; 16]> {
        let bytes = self.read_bytes(16)?;
        let mut sentinel = [0u8; 16];
        sentinel.copy_from_slice(&bytes);
        Ok(sentinel)
    }

    // ---------------------------------------------------------------
    // Colors
    // ---------------------------------------------------------------

    fn read_cm_color(&mut self) -> Result<Color> {
        if self.version >= DxfVersion::AC1018 {
            // AC18+ CMC encoding:
            // BS: color index (always 0)
            let _color_index = self.read_bit_short()?;
            // BL: RGB value
            let rgb = self.read_bit_long()? as u32;
            let arr = rgb.to_le_bytes();

            let color = if rgb == 0xC000_0000 {
                Color::ByLayer
            } else if (rgb & 0x0100_0000) != 0 {
                // Indexed color
                Color::Index(arr[0])
            } else {
                // True color
                Color::from_rgb(arr[2], arr[1], arr[0])
            };

            // RC: Color Byte
            let id = self.read_byte()?;

            // &1 => color name follows (TV)
            if (id & 1) == 1 {
                let _ = self.read_variable_text()?;
            }

            // &2 => book name follows (TV)
            if (id & 2) == 2 {
                let _ = self.read_variable_text()?;
            }

            Ok(color)
        } else {
            // Pre-AC18: BS color index
            let color_index = self.read_bit_short()?;
            Ok(Color::from_index(color_index))
        }
    }

    fn read_color_by_index(&mut self) -> Result<Color> {
        let idx = self.read_bit_short()?;
        Ok(Color::from_index(idx))
    }

    fn read_en_color(&mut self) -> Result<(Color, Transparency, bool)> {
        if self.version >= DxfVersion::AC1018 {
            // AC18+ entity color encoding
            let size = self.read_bit_short()?;

            if size == 0 {
                return Ok((Color::ByBlock, Transparency::OPAQUE, false));
            }

            let flags = (size as u16) & 0xFF00;
            let color;
            let mut transparency = Transparency::BY_LAYER;
            let mut is_book_color = false;

            // 0x4000: has AcDbColor reference (0x8000 is also set)
            if (flags & 0x4000) > 0 {
                color = Color::ByBlock;
                is_book_color = true;
            }
            // 0x8000: complex color (rgb)
            else if (flags & 0x8000) > 0 {
                let rgb = self.read_bit_long()? as u32;
                let arr = rgb.to_le_bytes();
                color = Color::from_rgb(arr[2], arr[1], arr[0]);
            } else {
                // Color index from lower 12 bits
                color = Color::from_index((size & 0x0FFF) as i16);
            }

            // 0x2000: transparency BL follows
            if (flags & 0x2000) > 0 {
                let value = self.read_bit_long()? as u32;
                transparency = Transparency::from_alpha_value(value);
            }

            Ok((color, transparency, is_book_color))
        } else {
            // Pre-AC18: BS color index, no transparency, no book-color
            let color_number = self.read_bit_short()?;
            Ok((Color::from_index(color_number), Transparency::BY_LAYER, false))
        }
    }

    // ---------------------------------------------------------------
    // Special types
    // ---------------------------------------------------------------

    fn read_object_type(&mut self) -> Result<DwgObjectType> {
        if self.version >= DxfVersion::AC1024 {
            // AC24+: 2-bit pair encoding
            let pair = self.read_2bits()?;
            let value = match pair {
                // Read the following byte
                0 => self.read_byte()? as i16,
                // Read following byte and add 0x1F0
                1 => 0x1F0 + self.read_byte()? as i16,
                // Read the following two raw bytes (raw short)
                2 | 3 => self.read_raw_short()?,
                _ => unreachable!(),
            };
            Ok(DwgObjectType::from_raw(value))
        } else {
            // Pre-AC24: BitShort
            let value = self.read_bit_short()?;
            Ok(DwgObjectType::from_raw(value))
        }
    }

    fn read_bit_extrusion(&mut self) -> Result<Vector3> {
        if self.version >= DxfVersion::AC1015 {
            // AC15+: 1-bit flag; if set → default extrusion (0,0,1)
            if self.read_bit()? {
                Ok(Vector3::UNIT_Z)
            } else {
                self.read_3bit_double()
            }
        } else {
            // Pre-AC15: always 3BD
            self.read_3bit_double()
        }
    }

    fn read_bit_thickness(&mut self) -> Result<f64> {
        if self.version >= DxfVersion::AC1015 {
            // AC15+: 1-bit flag; if set → 0.0
            if self.read_bit()? {
                Ok(0.0)
            } else {
                self.read_bit_double()
            }
        } else {
            // Pre-AC15: always BD
            self.read_bit_double()
        }
    }

    // ---------------------------------------------------------------
    // Date / time
    // ---------------------------------------------------------------

    fn read_8bit_julian_date(&mut self) -> Result<f64> {
        let jdate = self.read_int_le()?;
        let ms = self.read_int_le()?;
        Ok(Self::julian_to_timestamp(jdate, ms))
    }

    fn read_date_time(&mut self) -> Result<f64> {
        let jdate = self.read_bit_long()?;
        let ms = self.read_bit_long()?;
        Ok(Self::julian_to_timestamp(jdate, ms))
    }

    fn read_time_span(&mut self) -> Result<f64> {
        let hours = self.read_bit_long()? as f64;
        let milliseconds = self.read_bit_long()? as f64;
        Ok(hours * 3600.0 + milliseconds / 1000.0)
    }

    // ---------------------------------------------------------------
    // Stream position
    // ---------------------------------------------------------------

    fn position_in_bits(&self) -> i64 {
        let bit_position = self.stream.position() as i64 * 8;
        if self.bit_shift > 0 {
            bit_position + self.bit_shift as i64 - 8
        } else {
            bit_position
        }
    }

    fn set_position_in_bits(&mut self, position: i64) {
        self.set_position((position >> 3) as u64);
        self.bit_shift = (position & 7) as u8;

        if self.bit_shift > 0 {
            self.advance_byte();
        }
    }

    fn reset_shift(&mut self) -> Result<u16> {
        // Reset the shift value
        if self.bit_shift > 0 {
            self.bit_shift = 0;
        }

        self.advance_byte();
        let low = self.last_byte as u16;
        self.advance_byte();
        let high = self.last_byte as u16;

        Ok(low | (high << 8))
    }

    fn set_position_by_flag(&mut self, position: i64) -> Result<i64> {
        self.set_position_in_bits(position);

        // String stream present bit (last bit in pre-handles section).
        let flag = self.read_bit()?;

        let start_position;
        if flag {
            // String stream present
            let (length, size) = self.apply_flag_to_position(position)?;
            start_position = length - size;
            self.set_position_in_bits(start_position);
        } else {
            // Mark as empty — no string data
            self.is_empty = true;
            // Set the position to the end
            let len = self.stream_length();
            self.set_position(len);
            start_position = position;
        }

        Ok(start_position)
    }
}

/// Create a stream reader for the given DWG version.
///
/// This is the factory function mirroring ACadSharp's
/// `DwgStreamReaderBase.GetStreamHandler`.
pub fn get_stream_handler(version: DxfVersion, data: Vec<u8>) -> DwgStreamReaderBase {
    DwgStreamReaderBase::new(data, version)
}

/// Map a DWG code page byte to an encoding_rs encoding.
fn encoding_from_code_page(key: u8) -> &'static Encoding {
    // The DWG format stores a code page index byte.
    // Most common: 0x1E = Windows-1252 (Western), 0x00 = Windows-1252
    match key {
        0x00 | 0x01 | 0x1E => encoding_rs::WINDOWS_1252,
        0x02 => encoding_rs::WINDOWS_1250,    // Central European
        0x03 => encoding_rs::WINDOWS_1251,    // Cyrillic
        0x04 => encoding_rs::WINDOWS_1253,    // Greek
        0x05 => encoding_rs::WINDOWS_1254,    // Turkish
        0x06 => encoding_rs::WINDOWS_1255,    // Hebrew
        0x07 => encoding_rs::WINDOWS_1256,    // Arabic
        0x08 => encoding_rs::WINDOWS_1257,    // Baltic
        0x0A => encoding_rs::WINDOWS_874,     // Thai
        0x0B => encoding_rs::SHIFT_JIS,       // Japanese
        0x0C => encoding_rs::GBK,             // Simplified Chinese
        0x0D => encoding_rs::EUC_KR,          // Korean
        0x0E => encoding_rs::BIG5,            // Traditional Chinese
        _ => encoding_rs::WINDOWS_1252,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_reader(data: &[u8]) -> DwgStreamReaderBase {
        DwgStreamReaderBase::new(data.to_vec(), DxfVersion::AC1015)
    }

    /// Pack a 2-bit prefix code followed by value bytes into a contiguous bitstream.
    fn pack_2bit(code: u8, value: &[u8]) -> Vec<u8> {
        let mut bits: Vec<bool> = Vec::new();
        bits.push((code >> 1) & 1 == 1);
        bits.push(code & 1 == 1);
        for &b in value {
            for j in (0..8).rev() {
                bits.push((b >> j) & 1 == 1);
            }
        }
        bits_to_bytes(&bits)
    }

    /// Pack a 3-bit prefix code followed by value bytes into a contiguous bitstream.
    fn pack_3bit(code: u8, value: &[u8]) -> Vec<u8> {
        let mut bits: Vec<bool> = Vec::new();
        bits.push((code >> 2) & 1 == 1);
        bits.push((code >> 1) & 1 == 1);
        bits.push(code & 1 == 1);
        for &b in value {
            for j in (0..8).rev() {
                bits.push((b >> j) & 1 == 1);
            }
        }
        bits_to_bytes(&bits)
    }

    fn bits_to_bytes(bits: &[bool]) -> Vec<u8> {
        let mut result = Vec::new();
        for chunk in bits.chunks(8) {
            let mut byte = 0u8;
            for (i, &bit) in chunk.iter().enumerate() {
                if bit {
                    byte |= 1 << (7 - i);
                }
            }
            result.push(byte);
        }
        result
    }

    #[test]
    fn test_read_bit() {
        // 0b10110000 = 0xB0
        let mut reader = make_reader(&[0xB0]);
        assert!(reader.read_bit().unwrap());   // bit 7: 1
        assert!(!reader.read_bit().unwrap());  // bit 6: 0
        assert!(reader.read_bit().unwrap());   // bit 5: 1
        assert!(reader.read_bit().unwrap());   // bit 4: 1
        assert!(!reader.read_bit().unwrap());  // bit 3: 0
        assert!(!reader.read_bit().unwrap());  // bit 2: 0
        assert!(!reader.read_bit().unwrap());  // bit 1: 0
        assert!(!reader.read_bit().unwrap());  // bit 0: 0
    }

    #[test]
    fn test_read_2bits() {
        // 0b11010000 = 0xD0 → first 2 bits = 11 = 3
        let mut reader = make_reader(&[0xD0]);
        assert_eq!(reader.read_2bits().unwrap(), 3);
        // next 2 bits = 01 = 1
        assert_eq!(reader.read_2bits().unwrap(), 1);
    }

    #[test]
    fn test_read_bit_short_zero() {
        // 2-bit code 10 = zero
        // 0b10000000 = 0x80 (first 2 bits: 10)
        let mut reader = make_reader(&[0x80]);
        assert_eq!(reader.read_bit_short().unwrap(), 0);
    }

    #[test]
    fn test_read_bit_short_256() {
        // 2-bit code 11 = 256
        // 0b11000000 = 0xC0
        let mut reader = make_reader(&[0xC0]);
        assert_eq!(reader.read_bit_short().unwrap(), 256);
    }

    #[test]
    fn test_read_bit_short_one_byte() {
        // 2-bit code 01 = unsigned char follows, value = 0x42 (66)
        // Bitstream: 01 | 01000010 → packed into bytes
        let data = pack_2bit(0b01, &[0x42]);
        let mut reader = make_reader(&data);
        assert_eq!(reader.read_bit_short().unwrap(), 0x42);
    }

    #[test]
    fn test_read_bit_long_zero() {
        // 2-bit code 10 = zero
        let mut reader = make_reader(&[0x80]);
        assert_eq!(reader.read_bit_long().unwrap(), 0);
    }

    #[test]
    fn test_read_bit_double_zero() {
        // 2-bit code 10 = 0.0
        let mut reader = make_reader(&[0x80]);
        assert_eq!(reader.read_bit_double().unwrap(), 0.0);
    }

    #[test]
    fn test_read_bit_double_one() {
        // 2-bit code 01 = 1.0
        let mut reader = make_reader(&[0x40]);
        assert_eq!(reader.read_bit_double().unwrap(), 1.0);
    }

    #[test]
    fn test_read_byte_no_shift() {
        let mut reader = make_reader(&[0xAB]);
        assert_eq!(reader.read_byte().unwrap(), 0xAB);
    }

    #[test]
    fn test_read_modular_char_single() {
        // Single byte with high bit clear: 0x3F = 63
        let mut reader = make_reader(&[0x3F]);
        assert_eq!(reader.read_modular_char().unwrap(), 63);
    }

    #[test]
    fn test_read_modular_char_multi() {
        // Two bytes: 0x80 | 0x01 = 1, then 0x01 = (1 << 7) = 128
        // Total = 1 + 128 = 129
        let mut reader = make_reader(&[0x81, 0x01]);
        assert_eq!(reader.read_modular_char().unwrap(), 129);
    }

    #[test]
    fn test_read_signed_modular_char_positive() {
        // Single byte: 0x05 = 5 (bit 6 clear = positive)
        let mut reader = make_reader(&[0x05]);
        assert_eq!(reader.read_signed_modular_char().unwrap(), 5);
    }

    #[test]
    fn test_read_signed_modular_char_negative() {
        // Single byte: 0x45 → 0b01000101 → value = 0x05 & 0x3F = 5, sign bit set → -5
        let mut reader = make_reader(&[0x45]);
        assert_eq!(reader.read_signed_modular_char().unwrap(), -5);
    }

    #[test]
    fn test_read_modular_short() {
        // Simple case: two bytes, flag clear
        // b1=0x10, b2=0x00 → flag clear (0x00 & 0x80 == 0)
        // value = 0x10 | (0x00 << 8) = 16
        let mut reader = make_reader(&[0x10, 0x00]);
        assert_eq!(reader.read_modular_short().unwrap(), 16);
    }

    #[test]
    fn test_handle_reference_absolute() {
        // code=4 (SoftPointer), counter=1, handle byte=0x1A
        // form byte: 0x41 → code=4, counter=1
        let mut reader = make_reader(&[0x41, 0x1A]);
        let (handle, ref_type) = reader.handle_reference_typed(0).unwrap();
        assert_eq!(handle, 0x1A);
        assert_eq!(ref_type, DwgReferenceType::SoftPointer);
    }

    #[test]
    fn test_handle_reference_plus1() {
        // code=6, counter=0 → reference_handle + 1
        let mut reader = make_reader(&[0x60]);
        let handle = reader.handle_reference_resolved(0x100).unwrap();
        assert_eq!(handle, 0x101);
    }

    #[test]
    fn test_position_in_bits() {
        let mut reader = make_reader(&[0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(reader.position_in_bits(), 0);

        // Read one bit
        reader.read_bit().unwrap();
        // Position: stream=1 byte, shift=1
        // bits = 1*8 + 1 - 8 = 1
        assert_eq!(reader.position_in_bits(), 1);
    }

    #[test]
    fn test_set_position_in_bits() {
        let mut reader = make_reader(&[0x00, 0x00, 0xFF, 0xFF]);
        reader.set_position_in_bits(16);
        // Should be at byte 2, shift 0
        let b = reader.read_byte().unwrap();
        assert_eq!(b, 0xFF);
    }

    #[test]
    fn test_read_sentinel() {
        let sentinel_data: [u8; 16] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
        let mut reader = make_reader(&sentinel_data);
        let sentinel = reader.read_sentinel().unwrap();
        assert_eq!(sentinel, sentinel_data);
    }

    #[test]
    fn test_read_variable_text_empty() {
        // BS=0 (2-bit code 10 = 0)
        let mut reader = make_reader(&[0x80]);
        let text = reader.read_variable_text().unwrap();
        assert!(text.is_empty());
    }

    #[test]
    fn test_read_bit_long_long() {
        // 3 bits for size: 001 = 1 byte follows, value = 0x42
        // Bitstream: 001 | 01000010 → packed into bytes
        let data = pack_3bit(0b001, &[0x42]);
        let mut reader = make_reader(&data);
        assert_eq!(reader.read_bit_long_long().unwrap(), 0x42);
    }

    #[test]
    fn test_read_bit_extrusion_ac12() {
        // Pre-R2000: always reads 3BD
        // All three doubles are 0.0 (2-bit code 10 each)
        // 3 × "10" = 6 bits: 0b101010_00 = 0xA8
        let mut reader = DwgStreamReaderBase::new(vec![0xA8], DxfVersion::AC1014);
        let extrusion = reader.read_bit_extrusion().unwrap();
        assert_eq!(extrusion, Vector3::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn test_read_bit_extrusion_ac15_flag_set() {
        // R2000+: if first bit is 1 → extrusion is (0,0,1)
        // Byte 0x80 → first bit = 1
        let mut reader = DwgStreamReaderBase::new(vec![0x80], DxfVersion::AC1015);
        let extrusion = reader.read_bit_extrusion().unwrap();
        assert_eq!(extrusion, Vector3::UNIT_Z);
    }

    #[test]
    fn test_read_bit_extrusion_ac15_flag_clear() {
        // R2000+: if first bit is 0 → read 3BD
        // bit(0), then 3 × BD(10 = 0.0) = 7 bits: 0b0_101010_0
        // = 0b0101_0100 = 0x54, then pad byte
        let mut reader = DwgStreamReaderBase::new(vec![0x54, 0x00], DxfVersion::AC1015);
        let extrusion = reader.read_bit_extrusion().unwrap();
        assert_eq!(extrusion, Vector3::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn test_read_bit_thickness_ac15_flag_set() {
        // R2000+: if first bit is 1 → thickness is 0.0
        let mut reader = DwgStreamReaderBase::new(vec![0x80], DxfVersion::AC1015);
        assert_eq!(reader.read_bit_thickness().unwrap(), 0.0);
    }

    #[test]
    fn test_read_object_type_ac24() {
        // AC24+: 2-bit pair encoding
        // pair=0, then byte 0x01 → value = 1 (TEXT)
        let data = pack_2bit(0b00, &[0x01]);
        let mut reader = DwgStreamReaderBase::new(data, DxfVersion::AC1024);
        let ot = reader.read_object_type().unwrap();
        assert_eq!(ot, DwgObjectType::Text);
    }

    #[test]
    fn test_read_object_type_ac24_offset() {
        // pair=1, then byte 0x02 → value = 0x1F0 + 2 = 0x1F2 (AcadProxyEntity)
        let data = pack_2bit(0b01, &[0x02]);
        let mut reader = DwgStreamReaderBase::new(data, DxfVersion::AC1024);
        let ot = reader.read_object_type().unwrap();
        assert_eq!(ot, DwgObjectType::AcadProxyEntity);
    }

    #[test]
    fn test_read_bit_short_full_short() {
        // 2-bit code 00 = full short follows (LE)
        // Value 0x1234: LE bytes [0x34, 0x12]
        let data = pack_2bit(0b00, &[0x34, 0x12]);
        let mut reader = make_reader(&data);
        assert_eq!(reader.read_bit_short().unwrap(), 0x1234);
    }

    #[test]
    fn test_read_bit_long_full() {
        // 2-bit code 00 = full i32 follows (LE)
        // Value 0x12345678: LE bytes [0x78, 0x56, 0x34, 0x12]
        let data = pack_2bit(0b00, &[0x78, 0x56, 0x34, 0x12]);
        let mut reader = make_reader(&data);
        assert_eq!(reader.read_bit_long().unwrap(), 0x12345678);
    }

    #[test]
    fn test_read_bit_long_one_byte() {
        // 2-bit code 01 = unsigned char follows, value = 0xFF (255)
        let data = pack_2bit(0b01, &[0xFF]);
        let mut reader = make_reader(&data);
        assert_eq!(reader.read_bit_long().unwrap(), 255);
    }

    #[test]
    fn test_read_bit_double_full() {
        // 2-bit code 00 = full f64 follows (LE)
        let val_bytes = 3.14f64.to_le_bytes();
        let data = pack_2bit(0b00, &val_bytes);
        let mut reader = make_reader(&data);
        let result = reader.read_bit_double().unwrap();
        assert!((result - 3.14).abs() < 1e-15);
    }

    #[test]
    fn test_read_bit_double_with_default_no_change() {
        // 2-bit code 00 = use default
        let mut reader = make_reader(&[0x00]);
        assert_eq!(reader.read_bit_double_with_default(42.0).unwrap(), 42.0);
    }

    #[test]
    fn test_read_3bit_double() {
        // Three BD values, all 0.0: 10 10 10 = 6 bits
        // 0b101010_00 = 0xA8
        let mut reader = make_reader(&[0xA8]);
        let v = reader.read_3bit_double().unwrap();
        assert_eq!(v, Vector3::new(0.0, 0.0, 0.0));
    }

    #[test]
    fn test_read_raw_short() {
        let mut reader = make_reader(&[0x34, 0x12]);
        assert_eq!(reader.read_raw_short().unwrap(), 0x1234);
    }

    #[test]
    fn test_read_raw_long() {
        let mut reader = make_reader(&[0x78, 0x56, 0x34, 0x12]);
        assert_eq!(reader.read_raw_long().unwrap(), 0x12345678);
    }

    #[test]
    fn test_read_bytes_with_shift() {
        // Read a bit first to set shift=1, then read a byte
        // 0xFF, 0x80 → read 1 bit (=1), shift=1
        // Then read_byte: last_byte=0xFF, shift to next byte 0x80
        // Result: (0xFF << 1) | (0x80 >> 7) = 0xFE | 0x01 = 0xFF
        let mut reader = make_reader(&[0xFF, 0x80]);
        assert!(reader.read_bit().unwrap()); // bit 7 of 0xFF = 1
        let b = reader.read_byte().unwrap();
        assert_eq!(b, 0xFF);
    }

    #[test]
    fn test_handle_reference_minus1() {
        // code=8, counter=0 → reference_handle - 1
        let mut reader = make_reader(&[0x80]);
        let handle = reader.handle_reference_resolved(0x100).unwrap();
        assert_eq!(handle, 0xFF);
    }

    #[test]
    fn test_handle_reference_plus_offset() {
        // code=0xA, counter=1, offset byte=0x05 → reference_handle + 5
        let mut reader = make_reader(&[0xA1, 0x05]);
        let handle = reader.handle_reference_resolved(0x100).unwrap();
        assert_eq!(handle, 0x105);
    }

    #[test]
    fn test_handle_reference_minus_offset() {
        // code=0xC, counter=1, offset byte=0x05 → reference_handle - 5
        let mut reader = make_reader(&[0xC1, 0x05]);
        let handle = reader.handle_reference_resolved(0x100).unwrap();
        assert_eq!(handle, 0xFB);
    }

    #[test]
    fn test_read_en_color_ac18_indexed() {
        // AC18: BS color number with no flags → indexed color
        // BS value = 7: 2-bit code 01, then byte 7 (bit-packed)
        let data = pack_2bit(0b01, &[0x07]);
        let mut reader = DwgStreamReaderBase::new(data, DxfVersion::AC1018);
        let (color, transparency, is_book) = reader.read_en_color().unwrap();
        assert_eq!(color, Color::Index(7));
        assert_eq!(transparency, Transparency::BY_LAYER);
        assert!(!is_book);
    }

    #[test]
    fn test_read_en_color_pre_ac18() {
        // Pre-AC18: BS color index
        // BS value = 7: 2-bit code 01, then byte 7 (bit-packed)
        let data = pack_2bit(0b01, &[0x07]);
        let mut reader = DwgStreamReaderBase::new(data, DxfVersion::AC1015);
        let (color, transparency, is_book) = reader.read_en_color().unwrap();
        assert_eq!(color, Color::Index(7));
        assert_eq!(transparency, Transparency::BY_LAYER);
        assert!(!is_book);
    }

    #[test]
    fn test_encoding_from_code_page() {
        assert_eq!(encoding_from_code_page(0x00).name(), "windows-1252");
        assert_eq!(encoding_from_code_page(0x0B).name(), "Shift_JIS");
        assert_eq!(encoding_from_code_page(0x0C).name(), "GBK");
    }
}
