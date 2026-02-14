//! Base implementation of `IDwgStreamWriter` with bit-level I/O.
//!
//! Mirrors ACadSharp's `DwgStreamWriterBase` (688 lines of C#) plus all
//! version-specific overrides (AC15, AC18, AC21, AC24) inlined via
//! version checks.
//!
//! This provides the core bit-manipulation implementations for writing
//! all DWG data types.

use crate::error::{DxfError, Result};
use crate::io::dwg::reference_type::DwgReferenceType;
use crate::types::{Color, DxfVersion, Transparency, Vector2, Vector3};

use super::stream_writer::IDwgStreamWriter;

use std::io::{self, Cursor, Read, Seek, SeekFrom, Write};

use encoding_rs::Encoding;

/// Base implementation of the bit-level DWG stream writer.
///
/// All version-specific behavior is dispatched via `self.version` checks.
pub struct DwgStreamWriterBase {
    stream: Cursor<Vec<u8>>,
    bit_shift: i32,
    last_byte: u8,
    encoding: &'static Encoding,
    version: DxfVersion,
    saved_position_in_bits: i64,
}

impl DwgStreamWriterBase {
    /// Create a new writer.
    pub fn new(version: DxfVersion) -> Self {
        Self {
            stream: Cursor::new(Vec::new()),
            bit_shift: 0,
            last_byte: 0,
            encoding: encoding_rs::WINDOWS_1252,
            version,
            saved_position_in_bits: 0,
        }
    }

    /// Create a writer wrapping existing data.
    pub fn from_data(data: Vec<u8>, version: DxfVersion) -> Self {
        Self {
            stream: Cursor::new(data),
            bit_shift: 0,
            last_byte: 0,
            encoding: encoding_rs::WINDOWS_1252,
            version,
            saved_position_in_bits: 0,
        }
    }

    /// Get the DXF version.
    pub fn version(&self) -> DxfVersion {
        self.version
    }

    /// Set the text encoding.
    pub fn set_encoding(&mut self, encoding: &'static Encoding) {
        self.encoding = encoding;
    }

    /// Get the written data.
    pub fn data(&self) -> &[u8] {
        self.stream.get_ref()
    }

    /// Consume the writer and return the written data.
    pub fn into_data(self) -> Vec<u8> {
        self.stream.into_inner()
    }

    /// Get the current byte position.
    pub fn position(&self) -> u64 {
        self.stream.position()
    }

    /// Get the stream length.
    pub fn stream_length(&self) -> u64 {
        self.stream.get_ref().len() as u64
    }

    // ---------------------------------------------------------------
    // Internal helpers
    // ---------------------------------------------------------------

    fn reset_shift(&mut self) {
        self.bit_shift = 0;
        self.last_byte = 0;
    }

    fn write_3bits(&mut self, value: u8) -> Result<()> {
        self.write_bit((value & 4) != 0)?;
        self.write_bit((value & 2) != 0)?;
        self.write_bit((value & 1) != 0)?;
        Ok(())
    }

    /// Compute the number of bytes needed to encode a handle.
    fn handle_byte_count(handle: u64) -> u8 {
        if handle == 0 {
            0
        } else if handle < 0x100 {
            1
        } else if handle < 0x1_0000 {
            2
        } else if handle < 0x100_0000 {
            3
        } else if handle < 0x1_0000_0000 {
            4
        } else if handle < 0x100_0000_0000 {
            5
        } else if handle < 0x1_0000_0000_0000 {
            6
        } else if handle < 0x100_0000_0000_0000 {
            7
        } else {
            8
        }
    }
}

impl IDwgStreamWriter for DwgStreamWriterBase {
    fn stream(&mut self) -> &mut dyn super::stream_writer::ReadWriteSeek {
        &mut self.stream
    }

    fn position_in_bits(&self) -> i64 {
        self.stream.position() as i64 * 8 + self.bit_shift as i64
    }

    fn saved_position_in_bits(&self) -> i64 {
        self.saved_position_in_bits
    }

    // ---------------------------------------------------------------
    // Raw writes
    // ---------------------------------------------------------------

    fn write_bytes(&mut self, arr: &[u8]) -> Result<()> {
        if self.bit_shift == 0 {
            for &b in arr {
                self.stream.write_all(&[b])?;
            }
            return Ok(());
        }

        let num = 8 - self.bit_shift;
        for &b in arr {
            let combined = self.last_byte | (b >> self.bit_shift);
            self.stream.write_all(&[combined])?;
            self.last_byte = b << num;
        }
        Ok(())
    }

    fn write_bytes_at(&mut self, arr: &[u8], offset: usize, length: usize) -> Result<()> {
        if self.bit_shift == 0 {
            for i in 0..length {
                self.stream.write_all(&[arr[offset + i]])?;
            }
            return Ok(());
        }

        let num = 8 - self.bit_shift;
        for i in 0..length {
            let b = arr[offset + i];
            let combined = self.last_byte | (b >> self.bit_shift);
            self.stream.write_all(&[combined])?;
            self.last_byte = b << num;
        }
        Ok(())
    }

    fn write_int(&mut self, value: i32) -> Result<()> {
        let bytes = value.to_le_bytes();
        self.write_bytes(&bytes)
    }

    fn write_raw_long(&mut self, value: i32) -> Result<()> {
        let bytes = value.to_le_bytes();
        self.write_bytes(&bytes)
    }

    fn write_raw_short(&mut self, value: i16) -> Result<()> {
        let bytes = value.to_le_bytes();
        self.write_bytes(&bytes)
    }

    fn write_raw_ushort(&mut self, value: u16) -> Result<()> {
        let bytes = value.to_le_bytes();
        self.write_bytes(&bytes)
    }

    fn write_raw_double(&mut self, value: f64) -> Result<()> {
        let bytes = value.to_le_bytes();
        self.write_bytes(&bytes)
    }

    fn write_byte(&mut self, value: u8) -> Result<()> {
        if self.bit_shift == 0 {
            self.stream.write_all(&[value])?;
            return Ok(());
        }

        let shift = 8 - self.bit_shift;
        let combined = self.last_byte | (value >> self.bit_shift);
        self.stream.write_all(&[combined])?;
        self.last_byte = value << shift;
        Ok(())
    }

    // ---------------------------------------------------------------
    // Bit-coded writes
    // ---------------------------------------------------------------

    fn write_bit(&mut self, value: bool) -> Result<()> {
        if self.bit_shift < 7 {
            if value {
                self.last_byte |= 1 << (7 - self.bit_shift);
            }
            self.bit_shift += 1;
            return Ok(());
        }

        // bit_shift == 7: this is the last bit in the byte
        if value {
            self.last_byte |= 1;
        }

        self.stream.write_all(&[self.last_byte])?;
        self.reset_shift();
        Ok(())
    }

    fn write_2bits(&mut self, value: u8) -> Result<()> {
        if self.bit_shift < 6 {
            self.last_byte |= value << (6 - self.bit_shift);
            self.bit_shift += 2;
        } else if self.bit_shift == 6 {
            self.last_byte |= value;
            self.stream.write_all(&[self.last_byte])?;
            self.reset_shift();
        } else {
            // bit_shift == 7: spans byte boundary
            self.last_byte |= value >> 1;
            self.stream.write_all(&[self.last_byte])?;
            self.last_byte = value << 7;
            self.bit_shift = 1;
        }
        Ok(())
    }

    fn write_bit_short(&mut self, value: i16) -> Result<()> {
        if value == 0 {
            self.write_2bits(2)?;
        } else if value > 0 && value < 256 {
            self.write_2bits(1)?;
            self.write_byte(value as u8)?;
        } else if value == 256 {
            self.write_2bits(3)?;
        } else {
            self.write_2bits(0)?;
            self.write_byte(value as u8)?;
            self.write_byte((value >> 8) as u8)?;
        }
        Ok(())
    }

    fn write_bit_long(&mut self, value: i32) -> Result<()> {
        if value == 0 {
            self.write_2bits(2)?;
            return Ok(());
        }

        if value > 0 && value < 256 {
            self.write_2bits(1)?;
            self.write_byte(value as u8)?;
            return Ok(());
        }

        self.write_2bits(0)?;
        self.write_byte(value as u8)?;
        self.write_byte((value >> 8) as u8)?;
        self.write_byte((value >> 16) as u8)?;
        self.write_byte((value >> 24) as u8)?;
        Ok(())
    }

    fn write_bit_long_long(&mut self, value: i64) -> Result<()> {
        let mut size: u8 = 0;
        let unsigned_value = value as u64;

        let mut hold = unsigned_value;
        while hold != 0 {
            hold >>= 8;
            size += 1;
        }

        self.write_3bits(size)?;

        hold = unsigned_value;
        for _ in 0..size {
            self.write_byte((hold & 0xFF) as u8)?;
            hold >>= 8;
        }
        Ok(())
    }

    fn write_bit_double(&mut self, value: f64) -> Result<()> {
        if value == 0.0 {
            self.write_2bits(2)?;
            return Ok(());
        }

        if value == 1.0 {
            self.write_2bits(1)?;
            return Ok(());
        }

        self.write_2bits(0)?;
        let bytes = value.to_le_bytes();
        self.write_bytes(&bytes)?;
        Ok(())
    }

    fn write_bit_double_with_default(&mut self, def: f64, value: f64) -> Result<()> {
        if def == value {
            // 00: No more data, use default.
            self.write_2bits(0)?;
            return Ok(());
        }

        let def_bytes = def.to_le_bytes();
        let value_bytes = value.to_le_bytes();

        // Compare the 2 sets of bytes by symmetry
        let mut first = 0;
        let mut last: i32 = 7;
        while last >= 0 && def_bytes[last as usize] == value_bytes[last as usize] {
            first += 1;
            last -= 1;
        }

        if first >= 4 {
            // 01: 4 bytes patched into first 4 bytes
            self.write_2bits(1)?;
            self.write_bytes_at(&value_bytes, 0, 4)?;
        } else if first >= 2 {
            // 10: 6 bytes — bytes [4..6] then [0..4]
            self.write_2bits(2)?;
            self.write_byte(value_bytes[4])?;
            self.write_byte(value_bytes[5])?;
            self.write_byte(value_bytes[0])?;
            self.write_byte(value_bytes[1])?;
            self.write_byte(value_bytes[2])?;
            self.write_byte(value_bytes[3])?;
        } else {
            // 11: Full RD
            self.write_2bits(3)?;
            self.write_bytes(&value_bytes)?;
        }
        Ok(())
    }

    fn write_2bit_double(&mut self, value: Vector2) -> Result<()> {
        self.write_bit_double(value.x)?;
        self.write_bit_double(value.y)?;
        Ok(())
    }

    fn write_2bit_double_with_default(&mut self, def: Vector2, value: Vector2) -> Result<()> {
        self.write_bit_double_with_default(def.x, value.x)?;
        self.write_bit_double_with_default(def.y, value.y)?;
        Ok(())
    }

    fn write_3bit_double(&mut self, value: Vector3) -> Result<()> {
        self.write_bit_double(value.x)?;
        self.write_bit_double(value.y)?;
        self.write_bit_double(value.z)?;
        Ok(())
    }

    fn write_3bit_double_with_default(&mut self, def: Vector3, value: Vector3) -> Result<()> {
        self.write_bit_double_with_default(def.x, value.x)?;
        self.write_bit_double_with_default(def.y, value.y)?;
        self.write_bit_double_with_default(def.z, value.z)?;
        Ok(())
    }

    fn write_2raw_double(&mut self, value: Vector2) -> Result<()> {
        self.write_raw_double(value.x)?;
        self.write_raw_double(value.y)?;
        Ok(())
    }

    // ---------------------------------------------------------------
    // Text (version-specific)
    // ---------------------------------------------------------------

    fn write_variable_text(&mut self, value: &str) -> Result<()> {
        if value.is_empty() {
            self.write_bit_short(0)?;
            return Ok(());
        }

        if self.version >= DxfVersion::AC1021 {
            // AC21+: Unicode
            let utf16: Vec<u16> = value.encode_utf16().collect();
            self.write_bit_short(utf16.len() as i16)?;
            let bytes: Vec<u8> = utf16.iter().flat_map(|ch| ch.to_le_bytes()).collect();
            self.write_bytes(&bytes)?;
        } else {
            // Pre-AC21: encoded text
            let (encoded, _, _) = self.encoding.encode(value);
            self.write_bit_short(encoded.len() as i16)?;
            self.write_bytes(&encoded)?;
        }
        Ok(())
    }

    fn write_text_unicode(&mut self, value: &str) -> Result<()> {
        if self.version >= DxfVersion::AC1021 {
            // AC21+: Unicode
            let utf16: Vec<u16> = value.encode_utf16().collect();
            self.write_raw_short((utf16.len() as i16) + 1)?;
            let bytes: Vec<u8> = utf16.iter().flat_map(|ch| ch.to_le_bytes()).collect();
            self.write_bytes(&bytes)?;
            // Null terminator (2 bytes for Unicode)
            self.stream.write_all(&[0, 0])?;
        } else {
            // Pre-AC21: encoded text
            let (encoded, _, _) = self.encoding.encode(value);
            self.write_raw_short((encoded.len() as i16) + 1)?;
            self.stream.write_all(&encoded)?;
            self.stream.write_all(&[0])?;
        }
        Ok(())
    }

    // ---------------------------------------------------------------
    // Handle references
    // ---------------------------------------------------------------

    fn handle_reference(&mut self, handle: u64) -> Result<()> {
        self.handle_reference_typed(DwgReferenceType::Undefined, handle)
    }

    fn handle_reference_typed(
        &mut self,
        ref_type: DwgReferenceType,
        handle: u64,
    ) -> Result<()> {
        let b = (ref_type as u8) << 4;
        let counter = Self::handle_byte_count(handle);

        self.write_byte(b | counter)?;

        // Write handle bytes in big-endian order
        let shift_start = (counter as u32).saturating_sub(1) * 8;
        for i in 0..counter {
            let shift = shift_start - (i as u32 * 8);
            self.write_byte(((handle >> shift) & 0xFF) as u8)?;
        }
        Ok(())
    }

    /// For the base writer, handle_reference_on_main is the same as
    /// handle_reference (there's no separate handle sub-stream).
    fn handle_reference_on_main(&mut self, handle: u64) -> Result<()> {
        self.handle_reference(handle)
    }

    // ---------------------------------------------------------------
    // Object type (version-specific)
    // ---------------------------------------------------------------

    fn write_object_type(&mut self, value: i16) -> Result<()> {
        if self.version >= DxfVersion::AC1024 {
            // AC24+: 2-bit pair encoding
            if value <= 255 {
                self.write_2bits(0)?;
                self.write_byte(value as u8)?;
            } else if value >= 0x1F0 && value <= 0x2EF {
                self.write_2bits(1)?;
                self.write_byte((value - 0x1F0) as u8)?;
            } else {
                self.write_2bits(2)?;
                let bytes = value.to_le_bytes();
                self.write_bytes(&bytes)?;
            }
        } else {
            // Pre-AC24: BitShort
            self.write_bit_short(value)?;
        }
        Ok(())
    }

    // ---------------------------------------------------------------
    // Colors (version-specific)
    // ---------------------------------------------------------------

    fn write_cm_color(&mut self, value: Color) -> Result<()> {
        if self.version >= DxfVersion::AC1018 {
            // AC18+ CMC encoding
            // BS: color index (always 0)
            self.write_bit_short(0)?;

            // Build the RGB BL value
            let mut arr = [0u8; 4];
            match value {
                Color::Rgb { r, g, b } => {
                    arr[2] = r;
                    arr[1] = g;
                    arr[0] = b;
                    arr[3] = 0xC2; // 0b1100_0010
                }
                Color::ByLayer => {
                    arr[3] = 0xC0; // 0b1100_0000
                }
                Color::Index(idx) => {
                    arr[3] = 0xC3; // 0b1100_0011
                    arr[0] = idx;
                }
                Color::ByBlock => {
                    arr[3] = 0xC0;
                }
            }

            let rgb = i32::from_le_bytes(arr);
            self.write_bit_long(rgb)?;

            // RC: Color Byte (no name, no book)
            self.write_byte(0)?;
        } else {
            // Pre-AC18: BS color index
            let index = match value {
                Color::ByBlock => 0,
                Color::ByLayer => 256,
                Color::Index(i) => i as i16,
                Color::Rgb { r: _, g: _, b: _ } => {
                    // Approximate to nearest ACI index — use 7 (white) as default
                    7
                }
            };
            self.write_bit_short(index)?;
        }
        Ok(())
    }

    fn write_en_color(
        &mut self,
        color: Color,
        transparency: Transparency,
        is_book_color: bool,
    ) -> Result<()> {
        if self.version >= DxfVersion::AC1018 {
            // AC18+ entity color encoding
            let is_by_block = matches!(color, Color::ByBlock);
            let is_by_layer = transparency == Transparency::BY_LAYER;
            let is_true_color = matches!(color, Color::Rgb { .. });

            if is_by_block && is_by_layer && !is_book_color {
                self.write_bit_short(0)?;
                return Ok(());
            }

            let mut size: u16 = 0;

            // 0x2000: transparency follows
            if !is_by_layer {
                size |= 0x2000;
            }

            // 0x4000: has AcDbColor reference
            if is_book_color {
                size |= 0x4000;
                size |= 0x8000;
            } else if is_true_color {
                // 0x8000: complex color (RGB)
                size |= 0x8000;
            } else {
                // Color index in lower bits
                let idx = match color {
                    Color::ByBlock => 0u16,
                    Color::ByLayer => 256,
                    Color::Index(i) => i as u16,
                    _ => 7,
                };
                size |= idx & 0x0FFF;
            }

            self.write_bit_short(size as i16)?;

            // Write RGB if true color
            if is_true_color {
                if let Color::Rgb { r, g, b } = color {
                    let arr = [b, g, r, 0xC2u8];
                    let rgb = u32::from_le_bytes(arr);
                    self.write_bit_long(rgb as i32)?;
                }
            }

            // Write transparency if present
            if !is_by_layer {
                let alpha_value = transparency.to_alpha_value();
                self.write_bit_long(alpha_value as i32)?;
            }
        } else {
            // Pre-AC18: just write CmColor
            self.write_cm_color(color)?;
        }
        Ok(())
    }

    // ---------------------------------------------------------------
    // Extrusion / Thickness (version-specific)
    // ---------------------------------------------------------------

    fn write_bit_extrusion(&mut self, normal: Vector3) -> Result<()> {
        if self.version >= DxfVersion::AC1015 {
            // AC15+: single bit + optional 3BD
            if normal == Vector3::UNIT_Z {
                self.write_bit(true)?;
            } else {
                self.write_bit(false)?;
                self.write_3bit_double(normal)?;
            }
        } else {
            // Pre-AC15: always 3BD
            self.write_3bit_double(normal)?;
        }
        Ok(())
    }

    fn write_bit_thickness(&mut self, thickness: f64) -> Result<()> {
        if self.version >= DxfVersion::AC1015 {
            // AC15+: single bit + optional BD
            if thickness == 0.0 {
                self.write_bit(true)?;
            } else {
                self.write_bit(false)?;
                self.write_bit_double(thickness)?;
            }
        } else {
            // Pre-AC15: always BD
            self.write_bit_double(thickness)?;
        }
        Ok(())
    }

    // ---------------------------------------------------------------
    // Date / time
    // ---------------------------------------------------------------

    fn write_date_time(&mut self, jdate: i32, ms: i32) -> Result<()> {
        self.write_bit_long(jdate)?;
        self.write_bit_long(ms)?;
        Ok(())
    }

    fn write_8bit_julian_date(&mut self, jdate: i32, ms: i32) -> Result<()> {
        self.write_raw_long(jdate)?;
        self.write_raw_long(ms)?;
        Ok(())
    }

    fn write_time_span(&mut self, days: i32, ms: i32) -> Result<()> {
        self.write_bit_long(days)?;
        self.write_bit_long(ms)?;
        Ok(())
    }

    // ---------------------------------------------------------------
    // Stream control
    // ---------------------------------------------------------------

    fn write_spear_shift(&mut self) -> Result<()> {
        if self.bit_shift > 0 {
            for _ in self.bit_shift..8 {
                self.write_bit(false)?;
            }
        }
        Ok(())
    }

    fn reset_stream(&mut self) -> Result<()> {
        self.stream.set_position(0);
        self.reset_shift();
        self.stream.get_mut().clear();
        Ok(())
    }

    fn save_position_for_size(&mut self) -> Result<()> {
        self.saved_position_in_bits = self.position_in_bits();
        self.write_raw_long(0)?;
        Ok(())
    }

    fn set_position_in_bits(&mut self, pos_in_bits: i64) -> Result<()> {
        let position = pos_in_bits / 8;
        self.bit_shift = (pos_in_bits % 8) as i32;
        self.stream.set_position(position as u64);

        if self.bit_shift > 0 {
            let mut buf = [0u8; 1];
            let n = self.stream.read(&mut buf)?;
            if n == 0 {
                return Err(DxfError::Io(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "end of stream in set_position_in_bits",
                )));
            }
            self.last_byte = buf[0];
            // Seek back so the next write overwrites this byte
            self.stream.seek(SeekFrom::Current(-1))?;
        } else {
            self.last_byte = 0;
        }

        Ok(())
    }

    fn set_position_by_flag(&mut self, pos: i64) -> Result<()> {
        if pos >= 0x8000 {
            if pos >= 0x4000_0000 {
                let v = ((pos >> 30) & 0xFFFF) as u16;
                self.write_bytes(&v.to_le_bytes())?;
                let v = (((pos >> 15) & 0x7FFF) | 0x8000) as u16;
                self.write_bytes(&v.to_le_bytes())?;
            } else {
                let v = ((pos >> 15) & 0xFFFF) as u16;
                self.write_bytes(&v.to_le_bytes())?;
            }

            let v = ((pos & 0x7FFF) | 0x8000) as u16;
            self.write_bytes(&v.to_le_bytes())?;
        } else {
            let v = pos as u16;
            self.write_bytes(&v.to_le_bytes())?;
        }
        Ok(())
    }

    fn write_shift_value(&mut self) -> Result<()> {
        if self.bit_shift > 0 {
            let position = self.stream.position();
            let mut buf = [0u8; 1];
            let n = self.stream.read(&mut buf)?;
            if n > 0 {
                let mask = 0xFF >> self.bit_shift;
                let curr_value = self.last_byte | (buf[0] & mask as u8);
                self.stream.seek(SeekFrom::Start(position))?;
                self.stream.write_all(&[curr_value])?;
            }
        }
        Ok(())
    }
}

/// Create a stream writer for the given DWG version.
pub fn get_stream_writer(version: DxfVersion) -> DwgStreamWriterBase {
    DwgStreamWriterBase::new(version)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_writer() -> DwgStreamWriterBase {
        DwgStreamWriterBase::new(DxfVersion::AC1015)
    }

    fn data(w: &DwgStreamWriterBase) -> Vec<u8> {
        w.data().to_vec()
    }

    #[test]
    fn test_write_bit_true() {
        let mut w = make_writer();
        w.write_bit(true).unwrap();
        w.write_spear_shift().unwrap();
        assert_eq!(data(&w), vec![0x80]);
    }

    #[test]
    fn test_write_bit_false() {
        let mut w = make_writer();
        w.write_bit(false).unwrap();
        w.write_spear_shift().unwrap();
        assert_eq!(data(&w), vec![0x00]);
    }

    #[test]
    fn test_write_2bits() {
        let mut w = make_writer();
        w.write_2bits(3).unwrap();
        w.write_spear_shift().unwrap();
        assert_eq!(data(&w), vec![0xC0]);
    }

    #[test]
    fn test_write_byte_no_shift() {
        let mut w = make_writer();
        w.write_byte(0xAB).unwrap();
        assert_eq!(data(&w), vec![0xAB]);
    }

    #[test]
    fn test_write_bit_short_zero() {
        let mut w = make_writer();
        w.write_bit_short(0).unwrap();
        w.write_spear_shift().unwrap();
        // 2-bit code 10 → 0x80
        assert_eq!(data(&w), vec![0x80]);
    }

    #[test]
    fn test_write_bit_short_256() {
        let mut w = make_writer();
        w.write_bit_short(256).unwrap();
        w.write_spear_shift().unwrap();
        // 2-bit code 11 → 0xC0
        assert_eq!(data(&w), vec![0xC0]);
    }

    #[test]
    fn test_write_bit_short_small() {
        let mut w = make_writer();
        w.write_bit_short(42).unwrap();
        w.write_spear_shift().unwrap();
        // 2-bit code 01, then byte 42
        // 0b01_101010 = 0x6A, remaining bits padded → 0x6A, 0x00... wait
        // Actually: write_2bits(1) → last_byte = 0b01_000000 = 0x40, bit_shift=2
        // write_byte(42=0x2A=0b00101010): bit_shift=2, shift=6
        //   combined = 0x40 | (0x2A >> 2) = 0x40 | 0x0A = 0x4A
        //   last_byte = 0x2A << 6 = 0x80
        // write_spear_shift: 6 false bits → writes 0x80
        assert_eq!(data(&w), vec![0x4A, 0x80]);
    }

    #[test]
    fn test_write_bit_long_zero() {
        let mut w = make_writer();
        w.write_bit_long(0).unwrap();
        w.write_spear_shift().unwrap();
        assert_eq!(data(&w), vec![0x80]);
    }

    #[test]
    fn test_write_bit_double_zero() {
        let mut w = make_writer();
        w.write_bit_double(0.0).unwrap();
        w.write_spear_shift().unwrap();
        assert_eq!(data(&w), vec![0x80]);
    }

    #[test]
    fn test_write_bit_double_one() {
        let mut w = make_writer();
        w.write_bit_double(1.0).unwrap();
        w.write_spear_shift().unwrap();
        assert_eq!(data(&w), vec![0x40]);
    }

    #[test]
    fn test_write_read_roundtrip_bit_short() {
        use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
        use crate::io::dwg::reader::stream_reader_base::DwgStreamReaderBase;

        for value in &[0i16, 1, 42, 127, 255, 256, -1, 0x1234, i16::MAX, i16::MIN] {
            let mut w = DwgStreamWriterBase::new(DxfVersion::AC1015);
            w.write_bit_short(*value).unwrap();
            w.write_spear_shift().unwrap();

            let mut r = DwgStreamReaderBase::new(w.into_data(), DxfVersion::AC1015);
            let read_val = r.read_bit_short().unwrap();
            assert_eq!(*value, read_val, "roundtrip failed for {value}");
        }
    }

    #[test]
    fn test_write_read_roundtrip_bit_long() {
        use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
        use crate::io::dwg::reader::stream_reader_base::DwgStreamReaderBase;

        for value in &[0i32, 1, 42, 255, 0x12345678, -1, i32::MAX] {
            let mut w = DwgStreamWriterBase::new(DxfVersion::AC1015);
            w.write_bit_long(*value).unwrap();
            w.write_spear_shift().unwrap();

            let mut r = DwgStreamReaderBase::new(w.into_data(), DxfVersion::AC1015);
            let read_val = r.read_bit_long().unwrap();
            assert_eq!(*value, read_val, "roundtrip failed for {value}");
        }
    }

    #[test]
    fn test_write_read_roundtrip_bit_double() {
        use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
        use crate::io::dwg::reader::stream_reader_base::DwgStreamReaderBase;

        for value in &[0.0f64, 1.0, 3.14, -42.5, f64::MAX, f64::MIN_POSITIVE] {
            let mut w = DwgStreamWriterBase::new(DxfVersion::AC1015);
            w.write_bit_double(*value).unwrap();
            w.write_spear_shift().unwrap();

            let mut r = DwgStreamReaderBase::new(w.into_data(), DxfVersion::AC1015);
            let read_val = r.read_bit_double().unwrap();
            assert_eq!(*value, read_val, "roundtrip failed for {value}");
        }
    }

    #[test]
    fn test_write_read_roundtrip_handle() {
        use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
        use crate::io::dwg::reader::stream_reader_base::DwgStreamReaderBase;

        for handle in &[0u64, 1, 0xFF, 0x1234, 0xABCDEF, 0x12345678] {
            let mut w = DwgStreamWriterBase::new(DxfVersion::AC1015);
            w.handle_reference_typed(DwgReferenceType::SoftPointer, *handle)
                .unwrap();
            w.write_spear_shift().unwrap();

            let mut r = DwgStreamReaderBase::new(w.into_data(), DxfVersion::AC1015);
            let (read_handle, ref_type) = r.handle_reference_typed(0).unwrap();
            assert_eq!(*handle, read_handle, "roundtrip failed for handle {handle}");
            assert_eq!(ref_type, DwgReferenceType::SoftPointer);
        }
    }

    #[test]
    fn test_write_bit_extrusion_ac15() {
        use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
        use crate::io::dwg::reader::stream_reader_base::DwgStreamReaderBase;

        // Default extrusion (0,0,1) → single bit
        let mut w = DwgStreamWriterBase::new(DxfVersion::AC1015);
        w.write_bit_extrusion(Vector3::UNIT_Z).unwrap();
        w.write_spear_shift().unwrap();

        let mut r = DwgStreamReaderBase::new(w.into_data(), DxfVersion::AC1015);
        assert_eq!(r.read_bit_extrusion().unwrap(), Vector3::UNIT_Z);
    }

    #[test]
    fn test_write_bit_thickness_ac15() {
        use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
        use crate::io::dwg::reader::stream_reader_base::DwgStreamReaderBase;

        // Zero thickness → single bit
        let mut w = DwgStreamWriterBase::new(DxfVersion::AC1015);
        w.write_bit_thickness(0.0).unwrap();
        w.write_spear_shift().unwrap();

        let mut r = DwgStreamReaderBase::new(w.into_data(), DxfVersion::AC1015);
        assert_eq!(r.read_bit_thickness().unwrap(), 0.0);
    }

    #[test]
    fn test_write_read_roundtrip_object_type_ac24() {
        use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
        use crate::io::dwg::reader::stream_reader_base::DwgStreamReaderBase;

        // Only test values that map to known DwgObjectType variants,
        // since Unlisted variant loses the original raw value.
        for value in &[1i16, 0x43, 0x1F2, 0x1F3] {
            let mut w = DwgStreamWriterBase::new(DxfVersion::AC1024);
            w.write_object_type(*value).unwrap();
            w.write_spear_shift().unwrap();

            let mut r = DwgStreamReaderBase::new(w.into_data(), DxfVersion::AC1024);
            let ot = r.read_object_type().unwrap();
            assert_eq!(ot.as_raw(), *value, "roundtrip failed for OT {value}");
        }
    }

    #[test]
    fn test_write_read_roundtrip_bit_long_long() {
        use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
        use crate::io::dwg::reader::stream_reader_base::DwgStreamReaderBase;

        for value in &[0i64, 1, 0xFF, 0x1234, 0xABCDEF] {
            let mut w = DwgStreamWriterBase::new(DxfVersion::AC1015);
            w.write_bit_long_long(*value).unwrap();
            w.write_spear_shift().unwrap();

            let mut r = DwgStreamReaderBase::new(w.into_data(), DxfVersion::AC1015);
            let read_val = r.read_bit_long_long().unwrap();
            assert_eq!(*value, read_val, "roundtrip failed for {value}");
        }
    }

    #[test]
    fn test_write_read_roundtrip_variable_text() {
        use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
        use crate::io::dwg::reader::stream_reader_base::DwgStreamReaderBase;

        // Pre-AC21 text
        let mut w = DwgStreamWriterBase::new(DxfVersion::AC1015);
        w.write_variable_text("Hello").unwrap();
        w.write_spear_shift().unwrap();

        let mut r = DwgStreamReaderBase::new(w.into_data(), DxfVersion::AC1015);
        assert_eq!(r.read_variable_text().unwrap(), "Hello");
    }

    #[test]
    fn test_write_read_roundtrip_variable_text_unicode() {
        use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
        use crate::io::dwg::reader::stream_reader_base::DwgStreamReaderBase;

        // AC21 Unicode text
        let mut w = DwgStreamWriterBase::new(DxfVersion::AC1021);
        w.write_variable_text("ABC").unwrap();
        w.write_spear_shift().unwrap();

        let mut r = DwgStreamReaderBase::new(w.into_data(), DxfVersion::AC1021);
        assert_eq!(r.read_variable_text().unwrap(), "ABC");
    }
}
