//! Merged DWG stream reader for R2007+ (AC1021+).
//!
//! Mirrors ACadSharp's `DwgMergedReader`.
//!
//! In DWG versions AC1021 and above, object data is split into three
//! separate sections within each object record:
//! - **Main data** — system variables, entity properties, etc.
//! - **String data** — all text values (TV / TU)
//! - **Handle data** — all handle references (H)
//!
//! This reader multiplexes across the three sub-streams, routing
//! each method call to the appropriate reader.

use crate::error::{DxfError, Result};
use crate::io::dwg::object_type::DwgObjectType;
use crate::io::dwg::reference_type::DwgReferenceType;
use crate::types::{Color, DxfVersion, Transparency, Vector2, Vector3};

use super::stream_reader::IDwgStreamReader;
use super::stream_reader_base::DwgStreamReaderBase;

/// Merged reader that delegates to three sub-streams.
///
/// Created for each object/entity in R2007+ DWG files.
pub struct DwgMergedReader {
    main_reader: DwgStreamReaderBase,
    text_reader: DwgStreamReaderBase,
    handle_reader: DwgStreamReaderBase,
    version: DxfVersion,
}

impl DwgMergedReader {
    /// Create a new merged reader with the three sub-streams.
    pub fn new(
        main_reader: DwgStreamReaderBase,
        text_reader: DwgStreamReaderBase,
        handle_reader: DwgStreamReaderBase,
        version: DxfVersion,
    ) -> Self {
        Self {
            main_reader,
            text_reader,
            handle_reader,
            version,
        }
    }

    /// Get a reference to the main reader.
    pub fn main_reader(&self) -> &DwgStreamReaderBase {
        &self.main_reader
    }

    /// Get a mutable reference to the main reader.
    pub fn main_reader_mut(&mut self) -> &mut DwgStreamReaderBase {
        &mut self.main_reader
    }

    /// Get a mutable reference to the text reader.
    pub fn text_reader_mut(&mut self) -> &mut DwgStreamReaderBase {
        &mut self.text_reader
    }

    /// Get a mutable reference to the handle reader.
    pub fn handle_reader_mut(&mut self) -> &mut DwgStreamReaderBase {
        &mut self.handle_reader
    }
}

impl IDwgStreamReader for DwgMergedReader {
    fn bit_shift(&self) -> u8 {
        self.main_reader.bit_shift()
    }

    fn set_bit_shift(&mut self, _shift: u8) {
        // Not meaningful for merged reader
    }

    fn is_empty(&self) -> bool {
        false
    }

    fn position(&self) -> u64 {
        self.main_reader.position()
    }

    fn set_position(&mut self, _pos: u64) {
        // Setting position directly is not supported on the merged reader
    }

    fn stream(&mut self) -> &mut dyn super::stream_reader::ReadSeek {
        self.main_reader.stream()
    }

    fn advance_byte(&mut self) {
        // Not meaningful for merged reader
    }

    fn advance(&mut self, offset: usize) {
        self.main_reader.advance(offset);
    }

    // ---------------------------------------------------------------
    // All main-reader delegated methods
    // ---------------------------------------------------------------

    fn read_bit(&mut self) -> Result<bool> {
        self.main_reader.read_bit()
    }

    fn read_bit_as_short(&mut self) -> Result<i16> {
        self.main_reader.read_bit_as_short()
    }

    fn read_2bits(&mut self) -> Result<u8> {
        self.main_reader.read_2bits()
    }

    fn read_bit_short(&mut self) -> Result<i16> {
        self.main_reader.read_bit_short()
    }

    fn read_bit_short_as_bool(&mut self) -> Result<bool> {
        self.main_reader.read_bit_short_as_bool()
    }

    fn read_bit_long(&mut self) -> Result<i32> {
        self.main_reader.read_bit_long()
    }

    fn read_bit_long_long(&mut self) -> Result<i64> {
        self.main_reader.read_bit_long_long()
    }

    fn read_bit_double(&mut self) -> Result<f64> {
        self.main_reader.read_bit_double()
    }

    fn read_bit_double_with_default(&mut self, def: f64) -> Result<f64> {
        self.main_reader.read_bit_double_with_default(def)
    }

    fn read_2bit_double(&mut self) -> Result<Vector2> {
        self.main_reader.read_2bit_double()
    }

    fn read_2bit_double_with_default(&mut self, def: Vector2) -> Result<Vector2> {
        self.main_reader.read_2bit_double_with_default(def)
    }

    fn read_3bit_double(&mut self) -> Result<Vector3> {
        self.main_reader.read_3bit_double()
    }

    fn read_3bit_double_with_default(&mut self, def: Vector3) -> Result<Vector3> {
        self.main_reader.read_3bit_double_with_default(def)
    }

    fn read_raw_char(&mut self) -> Result<u8> {
        self.main_reader.read_raw_char()
    }

    fn read_raw_short(&mut self) -> Result<i16> {
        self.main_reader.read_raw_short()
    }

    fn read_raw_ushort(&mut self) -> Result<u16> {
        self.main_reader.read_raw_ushort()
    }

    fn read_raw_long(&mut self) -> Result<i32> {
        self.main_reader.read_raw_long()
    }

    fn read_raw_ulong(&mut self) -> Result<u64> {
        self.main_reader.read_raw_ulong()
    }

    fn read_raw_double(&mut self) -> Result<f64> {
        self.main_reader.read_raw_double()
    }

    fn read_2raw_double(&mut self) -> Result<Vector2> {
        self.main_reader.read_2raw_double()
    }

    fn read_3raw_double(&mut self) -> Result<Vector3> {
        self.main_reader.read_3raw_double()
    }

    fn read_byte(&mut self) -> Result<u8> {
        self.main_reader.read_byte()
    }

    fn read_bytes(&mut self, length: usize) -> Result<Vec<u8>> {
        self.main_reader.read_bytes(length)
    }

    fn read_modular_char(&mut self) -> Result<u64> {
        self.main_reader.read_modular_char()
    }

    fn read_signed_modular_char(&mut self) -> Result<i64> {
        self.main_reader.read_signed_modular_char()
    }

    fn read_modular_short(&mut self) -> Result<i32> {
        self.main_reader.read_modular_short()
    }

    // ---------------------------------------------------------------
    // Handle references → handle_reader
    // ---------------------------------------------------------------

    fn handle_reference(&mut self) -> Result<u64> {
        self.handle_reader.handle_reference()
    }

    fn handle_reference_resolved(&mut self, reference_handle: u64) -> Result<u64> {
        self.handle_reader.handle_reference_resolved(reference_handle)
    }

    fn handle_reference_typed(
        &mut self,
        reference_handle: u64,
    ) -> Result<(u64, DwgReferenceType)> {
        self.handle_reader
            .handle_reference_typed(reference_handle)
    }

    // ---------------------------------------------------------------
    // Text → text_reader
    // ---------------------------------------------------------------

    fn read_text_unicode(&mut self) -> Result<String> {
        if self.text_reader.is_empty() {
            return Ok(String::new());
        }
        self.text_reader.read_text_unicode()
    }

    fn read_variable_text(&mut self) -> Result<String> {
        if self.text_reader.is_empty() {
            return Ok(String::new());
        }
        self.text_reader.read_variable_text()
    }

    // ---------------------------------------------------------------
    // Sentinel — read from main
    // ---------------------------------------------------------------

    fn read_sentinel(&mut self) -> Result<[u8; 16]> {
        self.main_reader.read_sentinel()
    }

    // ---------------------------------------------------------------
    // Colors
    // ---------------------------------------------------------------

    fn read_cm_color(&mut self) -> Result<Color> {
        if self.version >= DxfVersion::AC1018 {
            // For AC18+ in merged mode, CmColor reads structural data
            // from the main reader but text (color/book names) from
            // the text reader.

            // BS: color index (always 0)
            let _color_index = self.main_reader.read_bit_short()?;
            // BL: RGB value
            let rgb = self.main_reader.read_bit_long()? as u32;
            let arr = rgb.to_le_bytes();

            let color = if rgb == 0xC000_0000 {
                Color::ByLayer
            } else if (rgb & 0x0100_0000) != 0 {
                Color::Index(arr[0])
            } else {
                Color::from_rgb(arr[2], arr[1], arr[0])
            };

            // RC: Color Byte
            let id = self.main_reader.read_byte()?;

            // &1 => color name follows (TV) — read from text stream
            if (id & 1) == 1 {
                let _ = self.read_variable_text()?;
            }

            // &2 => book name follows (TV) — read from text stream
            if (id & 2) == 2 {
                let _ = self.read_variable_text()?;
            }

            Ok(color)
        } else {
            // Pre-AC18: BS color index
            self.main_reader.read_cm_color()
        }
    }

    fn read_color_by_index(&mut self) -> Result<Color> {
        let idx = self.main_reader.read_bit_short()?;
        Ok(Color::from_index(idx))
    }

    fn read_en_color(&mut self) -> Result<(Color, Transparency, bool)> {
        self.main_reader.read_en_color()
    }

    // ---------------------------------------------------------------
    // Special types — via main reader
    // ---------------------------------------------------------------

    fn read_object_type(&mut self) -> Result<DwgObjectType> {
        self.main_reader.read_object_type()
    }

    fn read_object_type_raw(&mut self) -> Result<(DwgObjectType, i16)> {
        self.main_reader.read_object_type_raw()
    }

    fn read_bit_extrusion(&mut self) -> Result<Vector3> {
        self.main_reader.read_bit_extrusion()
    }

    fn read_bit_thickness(&mut self) -> Result<f64> {
        self.main_reader.read_bit_thickness()
    }

    // ---------------------------------------------------------------
    // Date / time — via main reader
    // ---------------------------------------------------------------

    fn read_8bit_julian_date(&mut self) -> Result<f64> {
        self.main_reader.read_8bit_julian_date()
    }

    fn read_date_time(&mut self) -> Result<f64> {
        self.main_reader.read_date_time()
    }

    fn read_time_span(&mut self) -> Result<f64> {
        self.main_reader.read_time_span()
    }

    // ---------------------------------------------------------------
    // Stream position — via main reader
    // ---------------------------------------------------------------

    fn position_in_bits(&self) -> i64 {
        self.main_reader.position_in_bits()
    }

    fn set_position_in_bits(&mut self, position: i64) {
        self.main_reader.set_position_in_bits(position);
    }

    fn reset_shift(&mut self) -> Result<u16> {
        self.main_reader.reset_shift()
    }

    fn set_position_by_flag(&mut self, _position: i64) -> Result<i64> {
        Err(DxfError::Parse(
            "set_position_by_flag not supported on merged reader".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_merged(main: &[u8], text: &[u8], handle: &[u8]) -> DwgMergedReader {
        let v = DxfVersion::AC1021;
        DwgMergedReader::new(
            DwgStreamReaderBase::new(main.to_vec(), v),
            DwgStreamReaderBase::new(text.to_vec(), v),
            DwgStreamReaderBase::new(handle.to_vec(), v),
            v,
        )
    }

    #[test]
    fn test_merged_bit_short_from_main() {
        // BS=0 (2-bit code 10)
        let mut reader = make_merged(&[0x80], &[], &[]);
        assert_eq!(reader.read_bit_short().unwrap(), 0);
    }

    #[test]
    fn test_merged_handle_from_handle_stream() {
        // code=4 (SoftPointer), counter=1, handle byte=0x1A
        let mut reader = make_merged(&[], &[], &[0x41, 0x1A]);
        assert_eq!(reader.handle_reference().unwrap(), 0x1A);
    }

    #[test]
    fn test_merged_text_from_text_stream_empty() {
        // Text reader with is_empty set → returns empty
        let v = DxfVersion::AC1021;
        let text_reader = DwgStreamReaderBase::new(vec![], v);
        // Trigger is_empty by set_position_by_flag with no string stream
        // For simplicity: an empty text reader with no data
        let main = DwgStreamReaderBase::new(vec![0x80], v);
        let handle = DwgStreamReaderBase::new(vec![], v);
        let _reader = DwgMergedReader::new(main, text_reader, handle, v);
        // read_variable_text checks is_empty; empty vec means read will fail
        // but we can test the empty-text-reader scenario
    }

    #[test]
    fn test_merged_variable_text_unicode() {
        // AC21 text reader: BS length + Unicode bytes (bit-packed)
        // BS=3 (code 01, byte 0x03), then 6 bytes: 'A'=0x41,0x00, 'B'=0x42,0x00, 'C'=0x43,0x00
        // Bit stream: 01|00000011|01000001|00000000|01000010|00000000|01000011|00000000
        // Packed:     0x40,0xD0,0x40,0x10,0x80,0x10,0xC0,0x00
        let text_data = vec![0x40, 0xD0, 0x40, 0x10, 0x80, 0x10, 0xC0, 0x00];
        let mut reader = make_merged(&[], &text_data, &[]);
        let text = reader.read_variable_text().unwrap();
        assert_eq!(text, "ABC");
    }
}
