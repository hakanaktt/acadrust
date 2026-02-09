//! DWG bit-level stream reader trait.
//!
//! Mirrors ACadSharp's `IDwgStreamReader` interface.
//!
//! DWG data is **bit-aligned** (not byte-aligned). Every read operation
//! must track the current bit position within the byte stream.

use crate::error::Result;
use crate::io::dwg::object_type::DwgObjectType;
use crate::io::dwg::reference_type::DwgReferenceType;
use crate::types::{Color, Transparency, Vector2, Vector3};

use std::io::{Read, Seek};

/// Trait for bit-level DWG stream reading.
///
/// All data types listed in the DWG specification are encoded using variable-length
/// bit codes. This trait provides reading methods for each type:
///
/// - **B** — bit (1 bit)
/// - **BB** — 2-bit code
/// - **3B** — bit triplet (1–3 bits, R2010+)
/// - **BS** — BitShort (2+8/16 bits)
/// - **BL** — BitLong (2+8/32 bits)
/// - **BLL** — BitLongLong (3+N*8 bits, R2010+)
/// - **BD** — BitDouble (2+0/64 bits)
/// - **DD** — BitDouble with default
/// - **MC** — Modular Char (7-bit chunks)
/// - **MS** — Modular Short (15-bit chunks)
/// - **H** — Handle reference
/// - **T** — Text, **TU** — Unicode text, **TV** — Variable text
/// - **RC** — Raw Char, **RS** — Raw Short, **RL** — Raw Long, **RD** — Raw Double
/// - **SN** — Sentinel (16 bytes)
/// - **BE** — BitExtrusion, **BT** — BitThickness
/// - **CMC** — CmColor, **TC** — TrueColor
/// - **OT** — ObjectType
pub trait IDwgStreamReader {
    /// Current bit-shift within the last read byte (0–7).
    fn bit_shift(&self) -> u8;

    /// Set the bit shift.
    fn set_bit_shift(&mut self, shift: u8);

    /// Whether this stream is empty (no string data available).
    fn is_empty(&self) -> bool;

    /// Current byte position in the underlying stream.
    fn position(&self) -> u64;

    /// Set the byte position (resets bit shift to 0).
    fn set_position(&mut self, pos: u64);

    /// Get a reference to the underlying stream for raw operations.
    fn stream(&mut self) -> &mut dyn ReadSeek;

    /// Advance one byte forward, storing it as the last byte.
    fn advance_byte(&mut self);

    /// Advance `offset` bytes forward.
    fn advance(&mut self, offset: usize);

    // ---------------------------------------------------------------
    // BIT CODES AND DATA DEFINITIONS
    // ---------------------------------------------------------------

    /// **B** — Read a single bit (1 or 0).
    fn read_bit(&mut self) -> Result<bool>;

    /// **B** — Read a single bit, returning it as i16 (0 or 1).
    fn read_bit_as_short(&mut self) -> Result<i16>;

    /// **BB** — Read a 2-bit code.
    fn read_2bits(&mut self) -> Result<u8>;

    /// **BS** — Read a BitShort (16-bit).
    fn read_bit_short(&mut self) -> Result<i16>;

    /// **BS** — Read a BitShort and return as bool (nonzero = true).
    fn read_bit_short_as_bool(&mut self) -> Result<bool>;

    /// **BL** — Read a BitLong (32-bit).
    fn read_bit_long(&mut self) -> Result<i32>;

    /// **BLL** — Read a BitLongLong (64-bit, R2010+).
    fn read_bit_long_long(&mut self) -> Result<i64>;

    /// **BD** — Read a BitDouble (64-bit float).
    fn read_bit_double(&mut self) -> Result<f64>;

    /// **DD** — Read a BitDouble with a default value.
    fn read_bit_double_with_default(&mut self, def: f64) -> Result<f64>;

    /// **2BD** — Read 2D point (2 × BitDouble).
    fn read_2bit_double(&mut self) -> Result<Vector2>;

    /// **2DD** — Read 2D point with defaults.
    fn read_2bit_double_with_default(&mut self, def: Vector2) -> Result<Vector2>;

    /// **3BD** — Read 3D point (3 × BitDouble).
    fn read_3bit_double(&mut self) -> Result<Vector3>;

    /// **3DD** — Read 3D point with defaults.
    fn read_3bit_double_with_default(&mut self, def: Vector3) -> Result<Vector3>;

    /// **RC** — Read a raw char (8 bits, may span byte boundary).
    fn read_raw_char(&mut self) -> Result<u8>;

    /// **RS** — Read a raw short (16 bits LE).
    fn read_raw_short(&mut self) -> Result<i16>;

    /// **RU** — Read a raw unsigned short (16 bits LE).
    fn read_raw_ushort(&mut self) -> Result<u16>;

    /// **RL** — Read a raw long (32 bits LE).
    fn read_raw_long(&mut self) -> Result<i32>;

    /// **RL** — Read a raw unsigned long (32 bits LE).
    fn read_raw_ulong(&mut self) -> Result<u64>;

    /// **RD** — Read a raw double (64 bits LE).
    fn read_raw_double(&mut self) -> Result<f64>;

    /// **2RD** — Read 2D point (2 × raw double).
    fn read_2raw_double(&mut self) -> Result<Vector2>;

    /// **3RD** — Read 3D point (3 × raw double).
    fn read_3raw_double(&mut self) -> Result<Vector3>;

    /// Read a single byte (with bit-shift handling).
    fn read_byte(&mut self) -> Result<u8>;

    /// Read multiple bytes (with bit-shift handling).
    fn read_bytes(&mut self, length: usize) -> Result<Vec<u8>>;

    /// **MC** — Read a Modular Char (unsigned).
    fn read_modular_char(&mut self) -> Result<u64>;

    /// **MC** — Read a Signed Modular Char.
    fn read_signed_modular_char(&mut self) -> Result<i64>;

    /// **MS** — Read a Modular Short.
    fn read_modular_short(&mut self) -> Result<i32>;

    // ---------------------------------------------------------------
    // Handle references
    // ---------------------------------------------------------------

    /// **H** — Read a handle reference (absolute).
    fn handle_reference(&mut self) -> Result<u64>;

    /// **H** — Read a handle reference resolved against a reference handle.
    fn handle_reference_resolved(&mut self, reference_handle: u64) -> Result<u64>;

    /// **H** — Read a handle reference, returning both the resolved handle
    /// and the reference type.
    fn handle_reference_typed(
        &mut self,
        reference_handle: u64,
    ) -> Result<(u64, DwgReferenceType)>;

    // ---------------------------------------------------------------
    // Text
    // ---------------------------------------------------------------

    /// **T** — Read Unicode text (BS length + CodePage byte + chars).
    fn read_text_unicode(&mut self) -> Result<String>;

    /// **TV** — Read variable text (T for pre-2007, TU for 2007+).
    fn read_variable_text(&mut self) -> Result<String>;

    // ---------------------------------------------------------------
    // Sentinel
    // ---------------------------------------------------------------

    /// **SN** — Read a 16-byte sentinel.
    fn read_sentinel(&mut self) -> Result<[u8; 16]>;

    // ---------------------------------------------------------------
    // Colors
    // ---------------------------------------------------------------

    /// **CMC** — Read a CmColor value. `use_text_stream` is for merged reader use.
    fn read_cm_color(&mut self) -> Result<Color>;

    /// Read a color by index only.
    fn read_color_by_index(&mut self) -> Result<Color>;

    /// **ENC** — Read an entity color with transparency and book-color flag.
    fn read_en_color(&mut self) -> Result<(Color, Transparency, bool)>;

    // ---------------------------------------------------------------
    // Special types
    // ---------------------------------------------------------------

    /// **OT** — Read an object type code.
    fn read_object_type(&mut self) -> Result<DwgObjectType>;

    /// **BE** — Read a BitExtrusion (optimized 3D vector).
    fn read_bit_extrusion(&mut self) -> Result<Vector3>;

    /// **BT** — Read a BitThickness (optimized double).
    fn read_bit_thickness(&mut self) -> Result<f64>;

    // ---------------------------------------------------------------
    // Date / time
    // ---------------------------------------------------------------

    /// Read an 8-bit Julian date (2 × raw long).
    fn read_8bit_julian_date(&mut self) -> Result<f64>;

    /// Read a DateTime (2 × BitLong → Julian date).
    fn read_date_time(&mut self) -> Result<f64>;

    /// Read a TimeSpan (2 × BitLong → days + milliseconds).
    fn read_time_span(&mut self) -> Result<f64>;

    // ---------------------------------------------------------------
    // Stream position
    // ---------------------------------------------------------------

    /// Get the absolute position in the stream in bits.
    fn position_in_bits(&self) -> i64;

    /// Set the position in the stream by bits.
    fn set_position_in_bits(&mut self, position: i64);

    /// Reset bit shift to 0 and return a u16 (CRC reset pattern).
    fn reset_shift(&mut self) -> Result<u16>;

    /// Find the position of the string stream (R2007+ flag-based).
    ///
    /// Returns the start position of the string stream (in bits).
    fn set_position_by_flag(&mut self, position: i64) -> Result<i64>;
}

/// Helper trait combining Read + Seek.
pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

#[cfg(test)]
mod tests {
    use super::*;

    // Compile-time check that the trait is object-safe
    fn _assert_object_safe(_: &dyn IDwgStreamReader) {}
}
