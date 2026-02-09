//! DWG bit-level stream writer trait.
//!
//! Mirrors ACadSharp's `IDwgStreamWriter` interface.
//!
//! Provides the write side of the DWG bit-level I/O system.
//! All data types are written using variable-length bit codes that
//! match the DWG specification.

use crate::error::Result;
use crate::io::dwg::reference_type::DwgReferenceType;
use crate::types::{Color, Transparency, Vector2, Vector3};

use std::io::{Read, Seek, Write};

/// Combined trait for streams that support Read + Write + Seek.
pub trait ReadWriteSeek: Read + Write + Seek {}
impl<T: Read + Write + Seek> ReadWriteSeek for T {}

/// Trait for bit-level DWG stream writing.
///
/// This is the write counterpart to `IDwgStreamReader`. All data
/// types are written using the same bit-level encoding as they
/// are read.
pub trait IDwgStreamWriter {
    /// Get a reference to the underlying stream.
    fn stream(&mut self) -> &mut dyn ReadWriteSeek;

    /// Current position in bits.
    fn position_in_bits(&self) -> i64;

    /// Position saved for size-in-bits patching.
    fn saved_position_in_bits(&self) -> i64;

    // ---------------------------------------------------------------
    // Raw writes
    // ---------------------------------------------------------------

    /// Write raw bytes with bit-shift handling.
    fn write_bytes(&mut self, bytes: &[u8]) -> Result<()>;

    /// Write a slice of bytes starting at `offset` for `length` bytes.
    fn write_bytes_at(&mut self, bytes: &[u8], offset: usize, length: usize) -> Result<()>;

    /// Write a raw i32 (4 bytes LE).
    fn write_int(&mut self, value: i32) -> Result<()>;

    /// Write a raw i32 (4 bytes LE, same as write_int).
    fn write_raw_long(&mut self, value: i32) -> Result<()>;

    /// Write a raw i16 (2 bytes LE).
    fn write_raw_short(&mut self, value: i16) -> Result<()>;

    /// Write a raw u16 (2 bytes LE).
    fn write_raw_ushort(&mut self, value: u16) -> Result<()>;

    /// Write a raw f64 (8 bytes LE).
    fn write_raw_double(&mut self, value: f64) -> Result<()>;

    /// Write a raw byte (8 bits).
    fn write_byte(&mut self, value: u8) -> Result<()>;

    // ---------------------------------------------------------------
    // Bit-coded writes
    // ---------------------------------------------------------------

    /// **B** — Write a single bit.
    fn write_bit(&mut self, value: bool) -> Result<()>;

    /// **BB** — Write a 2-bit code.
    fn write_2bits(&mut self, value: u8) -> Result<()>;

    /// **BS** — Write a BitShort.
    fn write_bit_short(&mut self, value: i16) -> Result<()>;

    /// **BL** — Write a BitLong.
    fn write_bit_long(&mut self, value: i32) -> Result<()>;

    /// **BLL** — Write a BitLongLong.
    fn write_bit_long_long(&mut self, value: i64) -> Result<()>;

    /// **BD** — Write a BitDouble.
    fn write_bit_double(&mut self, value: f64) -> Result<()>;

    /// **DD** — Write a BitDouble with default.
    fn write_bit_double_with_default(&mut self, def: f64, value: f64) -> Result<()>;

    /// **2BD** — Write 2D point (2 × BitDouble).
    fn write_2bit_double(&mut self, value: Vector2) -> Result<()>;

    /// **2DD** — Write 2D point with defaults.
    fn write_2bit_double_with_default(&mut self, def: Vector2, value: Vector2) -> Result<()>;

    /// **3BD** — Write 3D point (3 × BitDouble).
    fn write_3bit_double(&mut self, value: Vector3) -> Result<()>;

    /// **3DD** — Write 3D point with defaults.
    fn write_3bit_double_with_default(&mut self, def: Vector3, value: Vector3) -> Result<()>;

    /// **2RD** — Write 2D point (2 × raw double).
    fn write_2raw_double(&mut self, value: Vector2) -> Result<()>;

    // ---------------------------------------------------------------
    // Text
    // ---------------------------------------------------------------

    /// **TV** — Write variable-length text.
    fn write_variable_text(&mut self, value: &str) -> Result<()>;

    /// **TU** — Write Unicode text.
    fn write_text_unicode(&mut self, value: &str) -> Result<()>;

    // ---------------------------------------------------------------
    // Handle references
    // ---------------------------------------------------------------

    /// **H** — Write a handle reference with default type (Undefined).
    fn handle_reference(&mut self, handle: u64) -> Result<()>;

    /// **H** — Write a handle reference with a specific type.
    fn handle_reference_typed(
        &mut self,
        ref_type: DwgReferenceType,
        handle: u64,
    ) -> Result<()>;

    // ---------------------------------------------------------------
    // Object type
    // ---------------------------------------------------------------

    /// **OT** — Write an object type code.
    fn write_object_type(&mut self, value: i16) -> Result<()>;

    // ---------------------------------------------------------------
    // Colors
    // ---------------------------------------------------------------

    /// **CMC** — Write a CmColor value.
    fn write_cm_color(&mut self, value: Color) -> Result<()>;

    /// **ENC** — Write an entity color with transparency.
    fn write_en_color(
        &mut self,
        color: Color,
        transparency: Transparency,
        is_book_color: bool,
    ) -> Result<()>;

    // ---------------------------------------------------------------
    // Special types
    // ---------------------------------------------------------------

    /// **BE** — Write a BitExtrusion.
    fn write_bit_extrusion(&mut self, normal: Vector3) -> Result<()>;

    /// **BT** — Write a BitThickness.
    fn write_bit_thickness(&mut self, thickness: f64) -> Result<()>;

    // ---------------------------------------------------------------
    // Date / time
    // ---------------------------------------------------------------

    /// Write a date/time as two BL (Julian day + milliseconds).
    fn write_date_time(&mut self, jdate: i32, ms: i32) -> Result<()>;

    /// Write a date/time as two RL (8-bit Julian).
    fn write_8bit_julian_date(&mut self, jdate: i32, ms: i32) -> Result<()>;

    /// Write a time span as two BL (days + milliseconds).
    fn write_time_span(&mut self, days: i32, ms: i32) -> Result<()>;

    // ---------------------------------------------------------------
    // Stream control
    // ---------------------------------------------------------------

    /// Pad the current byte with zero bits if bit_shift > 0.
    fn write_spear_shift(&mut self) -> Result<()>;

    /// Reset the stream (truncate to empty).
    fn reset_stream(&mut self) -> Result<()>;

    /// Save the current position for later size patching.
    fn save_position_for_size(&mut self) -> Result<()>;

    /// Set the bit position in the stream.
    fn set_position_in_bits(&mut self, pos_in_bits: i64) -> Result<()>;

    /// Write the flag-encoded position for string stream.
    fn set_position_by_flag(&mut self, pos: i64) -> Result<()>;

    /// Write the shift value (merges last_byte with the byte on disk).
    fn write_shift_value(&mut self) -> Result<()>;
}
