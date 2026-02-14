//! Merged DWG stream writer for R2007+ (AC1021+).
//!
//! Mirrors ACadSharp's `DwgMergedStreamWriter`.
//!
//! In DWG versions AC1021+, written object data is split into three
//! separate sections:
//! - **Main data** — system variables, entity properties, etc.
//! - **String data** — all text values (TV / TU)
//! - **Handle data** — all handle references (H)
//!
//! `WriteSpearShift` merges the three streams into one: main + text
//! (with flag/size) + handles.

use crate::error::{DxfError, Result};
use crate::io::dwg::reference_type::DwgReferenceType;
use crate::types::{Color, DxfVersion, Transparency, Vector2, Vector3};

use super::stream_writer::IDwgStreamWriter;
use super::stream_writer_base::DwgStreamWriterBase;

/// R2007+ merged writer with three sub-streams (main/text/handles).
pub struct DwgMergedStreamWriter {
    main: DwgStreamWriterBase,
    text: DwgStreamWriterBase,
    handle: DwgStreamWriterBase,
    saved_position: bool,
    position_in_bits: i64,
    saved_position_in_bits: i64,
}

impl DwgMergedStreamWriter {
    /// Create a new merged writer.
    pub fn new(version: DxfVersion) -> Self {
        Self {
            main: DwgStreamWriterBase::new(version),
            text: DwgStreamWriterBase::new(version),
            handle: DwgStreamWriterBase::new(version),
            saved_position: false,
            position_in_bits: 0,
            saved_position_in_bits: 0,
        }
    }

    /// Get the main writer.
    pub fn main_writer(&self) -> &DwgStreamWriterBase {
        &self.main
    }

    /// Get the main writer mutably.
    pub fn main_writer_mut(&mut self) -> &mut DwgStreamWriterBase {
        &mut self.main
    }

    /// Get the text writer mutably.
    pub fn text_writer_mut(&mut self) -> &mut DwgStreamWriterBase {
        &mut self.text
    }

    /// Get the handle writer mutably.
    pub fn handle_writer_mut(&mut self) -> &mut DwgStreamWriterBase {
        &mut self.handle
    }

    /// Consume and return a tuple of (main_data, text_data, handle_data).
    pub fn into_data(self) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        (
            self.main.into_data(),
            self.text.into_data(),
            self.handle.into_data(),
        )
    }
}

impl IDwgStreamWriter for DwgMergedStreamWriter {
    fn stream(&mut self) -> &mut dyn super::stream_writer::ReadWriteSeek {
        self.main.stream()
    }

    fn position_in_bits(&self) -> i64 {
        self.main.position_in_bits()
    }

    fn saved_position_in_bits(&self) -> i64 {
        self.saved_position_in_bits
    }

    // ---------------------------------------------------------------
    // Main-delegated writes
    // ---------------------------------------------------------------

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.main.write_bytes(bytes)
    }

    fn write_bytes_at(&mut self, bytes: &[u8], offset: usize, length: usize) -> Result<()> {
        self.main.write_bytes_at(bytes, offset, length)
    }

    fn write_int(&mut self, value: i32) -> Result<()> {
        self.main.write_int(value)
    }

    fn write_raw_long(&mut self, value: i32) -> Result<()> {
        self.main.write_raw_long(value)
    }

    fn write_raw_short(&mut self, value: i16) -> Result<()> {
        self.main.write_raw_short(value)
    }

    fn write_raw_ushort(&mut self, value: u16) -> Result<()> {
        self.main.write_raw_ushort(value)
    }

    fn write_raw_double(&mut self, value: f64) -> Result<()> {
        self.main.write_raw_double(value)
    }

    fn write_byte(&mut self, value: u8) -> Result<()> {
        self.main.write_byte(value)
    }

    fn write_bit(&mut self, value: bool) -> Result<()> {
        self.main.write_bit(value)
    }

    fn write_2bits(&mut self, value: u8) -> Result<()> {
        self.main.write_2bits(value)
    }

    fn write_bit_short(&mut self, value: i16) -> Result<()> {
        self.main.write_bit_short(value)
    }

    fn write_bit_long(&mut self, value: i32) -> Result<()> {
        self.main.write_bit_long(value)
    }

    fn write_bit_long_long(&mut self, value: i64) -> Result<()> {
        self.main.write_bit_long_long(value)
    }

    fn write_bit_double(&mut self, value: f64) -> Result<()> {
        self.main.write_bit_double(value)
    }

    fn write_bit_double_with_default(&mut self, def: f64, value: f64) -> Result<()> {
        self.main.write_bit_double_with_default(def, value)
    }

    fn write_2bit_double(&mut self, value: Vector2) -> Result<()> {
        self.main.write_2bit_double(value)
    }

    fn write_2bit_double_with_default(&mut self, def: Vector2, value: Vector2) -> Result<()> {
        self.main.write_2bit_double_with_default(def, value)
    }

    fn write_3bit_double(&mut self, value: Vector3) -> Result<()> {
        self.main.write_3bit_double(value)
    }

    fn write_3bit_double_with_default(&mut self, def: Vector3, value: Vector3) -> Result<()> {
        self.main.write_3bit_double_with_default(def, value)
    }

    fn write_2raw_double(&mut self, value: Vector2) -> Result<()> {
        self.main.write_2raw_double(value)
    }

    fn write_cm_color(&mut self, value: Color) -> Result<()> {
        self.main.write_cm_color(value)
    }

    fn write_en_color(
        &mut self,
        color: Color,
        transparency: Transparency,
        is_book_color: bool,
    ) -> Result<()> {
        self.main.write_en_color(color, transparency, is_book_color)
    }

    fn write_bit_extrusion(&mut self, normal: Vector3) -> Result<()> {
        self.main.write_bit_extrusion(normal)
    }

    fn write_bit_thickness(&mut self, thickness: f64) -> Result<()> {
        self.main.write_bit_thickness(thickness)
    }

    fn write_date_time(&mut self, jdate: i32, ms: i32) -> Result<()> {
        self.main.write_date_time(jdate, ms)
    }

    fn write_8bit_julian_date(&mut self, jdate: i32, ms: i32) -> Result<()> {
        self.main.write_8bit_julian_date(jdate, ms)
    }

    fn write_time_span(&mut self, days: i32, ms: i32) -> Result<()> {
        self.main.write_time_span(days, ms)
    }

    fn write_object_type(&mut self, value: i16) -> Result<()> {
        self.main.write_object_type(value)
    }

    // ---------------------------------------------------------------
    // Text → text writer
    // ---------------------------------------------------------------

    fn write_variable_text(&mut self, value: &str) -> Result<()> {
        self.text.write_variable_text(value)
    }

    fn write_text_unicode(&mut self, value: &str) -> Result<()> {
        self.text.write_text_unicode(value)
    }

    // ---------------------------------------------------------------
    // Handle references → handle writer
    // ---------------------------------------------------------------

    fn handle_reference(&mut self, handle: u64) -> Result<()> {
        self.handle.handle_reference(handle)
    }

    fn handle_reference_typed(
        &mut self,
        ref_type: DwgReferenceType,
        handle: u64,
    ) -> Result<()> {
        self.handle.handle_reference_typed(ref_type, handle)
    }

    /// Write a handle reference to the MAIN stream (not handle sub-stream).
    /// Used for the object's own handle.
    fn handle_reference_on_main(&mut self, handle: u64) -> Result<()> {
        self.main.handle_reference(handle)
    }

    // ---------------------------------------------------------------
    // Stream control
    // ---------------------------------------------------------------

    fn write_spear_shift(&mut self) -> Result<()> {
        let main_size_bits = self.main.position_in_bits();
        let text_size_bits = self.text.position_in_bits();

        self.main.write_spear_shift()?;

        if self.saved_position {
            let mut main_text_total_bits = (main_size_bits + text_size_bits + 1) as i32;
            if text_size_bits > 0 {
                main_text_total_bits += 16;
                if text_size_bits >= 0x8000 {
                    main_text_total_bits += 16;
                    if text_size_bits >= 0x4000_0000 {
                        main_text_total_bits += 16;
                    }
                }
            }

            // BUG2 fix: use merged writer's saved position, not base writer's
            let saved_pos = self.position_in_bits;
            self.main.set_position_in_bits(saved_pos)?;
            // Write the total size in bits
            self.main.write_raw_long(main_text_total_bits)?;
            self.main.write_shift_value()?;
        }

        self.main.set_position_in_bits(main_size_bits)?;

        if text_size_bits > 0 {
            self.text.write_spear_shift()?;
            let text_data = self.text.data().to_vec();
            self.main.write_bytes(&text_data)?;
            self.main.write_spear_shift()?;
            self.main
                .set_position_in_bits(main_size_bits + text_size_bits)?;
            self.main.set_position_by_flag(text_size_bits)?;
            self.main.write_bit(true)?;
        } else {
            self.main.write_bit(false)?;
        }

        self.handle.write_spear_shift()?;
        // BUG3 fix: store handle start OFFSET (size computed later in finalize)
        self.saved_position_in_bits = self.main.position_in_bits();
        let handle_data = self.handle.data().to_vec();
        self.main.write_bytes(&handle_data)?;
        self.main.write_spear_shift()?;

        Ok(())
    }

    fn reset_stream(&mut self) -> Result<()> {
        self.main.reset_stream()?;
        self.text.reset_stream()?;
        self.handle.reset_stream()?;
        Ok(())
    }

    fn save_position_for_size(&mut self) -> Result<()> {
        self.saved_position = true;
        self.position_in_bits = self.main.position_in_bits();
        // Save position for the size-in-bits
        self.main.write_raw_long(0)?;
        Ok(())
    }

    fn set_position_in_bits(&mut self, _pos_in_bits: i64) -> Result<()> {
        Err(DxfError::Parse(
            "set_position_in_bits not supported on merged writer".into(),
        ))
    }

    fn set_position_by_flag(&mut self, _pos: i64) -> Result<()> {
        Err(DxfError::Parse(
            "set_position_by_flag not supported on merged writer".into(),
        ))
    }

    fn write_shift_value(&mut self) -> Result<()> {
        Err(DxfError::Parse(
            "write_shift_value not supported on merged writer".into(),
        ))
    }
}

/// Pre-R2007 merged writer (AC14 style).
///
/// Mirrors ACadSharp's `DwgmMergedStreamWriterAC14`.
///
/// In pre-R2007, there is no separate text stream — text goes to the
/// main stream. Only two streams: main and handles.
pub struct DwgMergedStreamWriterAC14 {
    main: DwgStreamWriterBase,
    handle: DwgStreamWriterBase,
    saved_position: bool,
    position_in_bits: i64,
    saved_position_in_bits: i64,
}

impl DwgMergedStreamWriterAC14 {
    /// Create a new pre-R2007 merged writer.
    pub fn new(version: DxfVersion) -> Self {
        Self {
            main: DwgStreamWriterBase::new(version),
            handle: DwgStreamWriterBase::new(version),
            saved_position: false,
            position_in_bits: 0,
            saved_position_in_bits: 0,
        }
    }

    /// Get the main writer.
    pub fn main_writer(&self) -> &DwgStreamWriterBase {
        &self.main
    }

    /// Get the main writer mutably.
    pub fn main_writer_mut(&mut self) -> &mut DwgStreamWriterBase {
        &mut self.main
    }

    /// Get the handle writer mutably.
    pub fn handle_writer_mut(&mut self) -> &mut DwgStreamWriterBase {
        &mut self.handle
    }
}

impl IDwgStreamWriter for DwgMergedStreamWriterAC14 {
    fn stream(&mut self) -> &mut dyn super::stream_writer::ReadWriteSeek {
        self.main.stream()
    }

    fn position_in_bits(&self) -> i64 {
        self.main.position_in_bits()
    }

    fn saved_position_in_bits(&self) -> i64 {
        self.saved_position_in_bits
    }

    // ---------------------------------------------------------------
    // Main-delegated writes
    // ---------------------------------------------------------------

    fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.main.write_bytes(bytes)
    }

    fn write_bytes_at(&mut self, bytes: &[u8], offset: usize, length: usize) -> Result<()> {
        self.main.write_bytes_at(bytes, offset, length)
    }

    fn write_int(&mut self, value: i32) -> Result<()> {
        self.main.write_int(value)
    }

    fn write_raw_long(&mut self, value: i32) -> Result<()> {
        self.main.write_raw_long(value)
    }

    fn write_raw_short(&mut self, value: i16) -> Result<()> {
        self.main.write_raw_short(value)
    }

    fn write_raw_ushort(&mut self, value: u16) -> Result<()> {
        self.main.write_raw_ushort(value)
    }

    fn write_raw_double(&mut self, value: f64) -> Result<()> {
        self.main.write_raw_double(value)
    }

    fn write_byte(&mut self, value: u8) -> Result<()> {
        self.main.write_byte(value)
    }

    fn write_bit(&mut self, value: bool) -> Result<()> {
        self.main.write_bit(value)
    }

    fn write_2bits(&mut self, value: u8) -> Result<()> {
        self.main.write_2bits(value)
    }

    fn write_bit_short(&mut self, value: i16) -> Result<()> {
        self.main.write_bit_short(value)
    }

    fn write_bit_long(&mut self, value: i32) -> Result<()> {
        self.main.write_bit_long(value)
    }

    fn write_bit_long_long(&mut self, value: i64) -> Result<()> {
        self.main.write_bit_long_long(value)
    }

    fn write_bit_double(&mut self, value: f64) -> Result<()> {
        self.main.write_bit_double(value)
    }

    fn write_bit_double_with_default(&mut self, def: f64, value: f64) -> Result<()> {
        self.main.write_bit_double_with_default(def, value)
    }

    fn write_2bit_double(&mut self, value: Vector2) -> Result<()> {
        self.main.write_2bit_double(value)
    }

    fn write_2bit_double_with_default(&mut self, def: Vector2, value: Vector2) -> Result<()> {
        self.main.write_2bit_double_with_default(def, value)
    }

    fn write_3bit_double(&mut self, value: Vector3) -> Result<()> {
        self.main.write_3bit_double(value)
    }

    fn write_3bit_double_with_default(&mut self, def: Vector3, value: Vector3) -> Result<()> {
        self.main.write_3bit_double_with_default(def, value)
    }

    fn write_2raw_double(&mut self, value: Vector2) -> Result<()> {
        self.main.write_2raw_double(value)
    }

    // Text goes to MAIN in AC14
    fn write_variable_text(&mut self, value: &str) -> Result<()> {
        self.main.write_variable_text(value)
    }

    fn write_text_unicode(&mut self, value: &str) -> Result<()> {
        self.main.write_text_unicode(value)
    }

    fn write_cm_color(&mut self, value: Color) -> Result<()> {
        self.main.write_cm_color(value)
    }

    fn write_en_color(
        &mut self,
        color: Color,
        transparency: Transparency,
        is_book_color: bool,
    ) -> Result<()> {
        self.main.write_en_color(color, transparency, is_book_color)
    }

    fn write_bit_extrusion(&mut self, normal: Vector3) -> Result<()> {
        self.main.write_bit_extrusion(normal)
    }

    fn write_bit_thickness(&mut self, thickness: f64) -> Result<()> {
        self.main.write_bit_thickness(thickness)
    }

    fn write_date_time(&mut self, jdate: i32, ms: i32) -> Result<()> {
        self.main.write_date_time(jdate, ms)
    }

    fn write_8bit_julian_date(&mut self, jdate: i32, ms: i32) -> Result<()> {
        self.main.write_8bit_julian_date(jdate, ms)
    }

    fn write_time_span(&mut self, days: i32, ms: i32) -> Result<()> {
        self.main.write_time_span(days, ms)
    }

    fn write_object_type(&mut self, value: i16) -> Result<()> {
        self.main.write_object_type(value)
    }

    // ---------------------------------------------------------------
    // Handle references → handle writer
    // ---------------------------------------------------------------

    fn handle_reference(&mut self, handle: u64) -> Result<()> {
        self.handle.handle_reference(handle)
    }

    fn handle_reference_typed(
        &mut self,
        ref_type: DwgReferenceType,
        handle: u64,
    ) -> Result<()> {
        self.handle.handle_reference_typed(ref_type, handle)
    }

    /// Write a handle reference to the MAIN stream (not handle sub-stream).
    /// Used for the object's own handle.
    fn handle_reference_on_main(&mut self, handle: u64) -> Result<()> {
        self.main.handle_reference(handle)
    }

    // ---------------------------------------------------------------
    // Stream control
    // ---------------------------------------------------------------

    fn write_spear_shift(&mut self) -> Result<()> {
        let pos = self.main.position_in_bits();

        if self.saved_position {
            self.main.write_spear_shift()?;
            // BUG2 fix: use merged writer's saved position, not base writer's
            let saved_pos = self.position_in_bits;
            self.main.set_position_in_bits(saved_pos)?;
            self.main.write_raw_long(pos as i32)?;
            self.main.write_shift_value()?;
            self.main.set_position_in_bits(pos)?;
        }

        self.handle.write_spear_shift()?;
        let handle_data = self.handle.data().to_vec();
        self.main.write_bytes(&handle_data)?;
        self.main.write_spear_shift()?;

        Ok(())
    }

    fn reset_stream(&mut self) -> Result<()> {
        self.main.reset_stream()?;
        self.handle.reset_stream()?;
        Ok(())
    }

    fn save_position_for_size(&mut self) -> Result<()> {
        self.saved_position = true;
        self.position_in_bits = self.main.position_in_bits();
        self.main.write_raw_long(0)?;
        Ok(())
    }

    fn set_position_in_bits(&mut self, pos_in_bits: i64) -> Result<()> {
        self.main.set_position_in_bits(pos_in_bits)
    }

    fn set_position_by_flag(&mut self, pos: i64) -> Result<()> {
        self.main.set_position_by_flag(pos)
    }

    fn write_shift_value(&mut self) -> Result<()> {
        self.main.write_shift_value()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merged_writer_text_to_text_stream() {
        let mut w = DwgMergedStreamWriter::new(DxfVersion::AC1021);
        w.write_bit_short(42).unwrap();
        w.write_variable_text("Hello").unwrap();
        w.handle_reference(0x1A).unwrap();

        // Main has the bit short
        assert!(w.main.position_in_bits() > 0);
        // Text has the variable text
        assert!(w.text.position_in_bits() > 0);
        // Handle has the handle
        assert!(w.handle.position_in_bits() > 0);
    }

    #[test]
    fn test_ac14_writer_text_to_main() {
        let mut w = DwgMergedStreamWriterAC14::new(DxfVersion::AC1015);
        let main_before = w.main.position_in_bits();
        w.write_variable_text("Test").unwrap();
        // Text goes to main stream
        assert!(w.main.position_in_bits() > main_before);
    }
}
