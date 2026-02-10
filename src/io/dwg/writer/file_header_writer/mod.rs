//! DWG file header writer infrastructure.
//!
//! Provides the `IDwgFileHeaderWriter` trait and version-specific implementations:
//! - `DwgFileHeaderWriterAC15` — R13–R2000 sequential layout
//! - `DwgFileHeaderWriterAC18` — R2004+ page-based layout with LZ77

mod writer_ac15;
mod writer_ac18;

pub use writer_ac15::DwgFileHeaderWriterAC15;
pub use writer_ac18::DwgFileHeaderWriterAC18;

use crate::error::Result;

/// Trait for assembling section data into the final DWG file.
///
/// Mirrors ACadSharp's `IDwgFileHeaderWriter`.
pub trait IDwgFileHeaderWriter {
    /// The offset of the handle/object-map section relative to the file start.
    fn handle_section_offset(&self) -> i32;

    /// Register a named section's data for inclusion in the output file.
    ///
    /// `name` — section name (e.g., "AcDb:Header", "AcDb:Classes").
    /// `data` — raw section bytes (uncompressed or pre-compressed depending on writer).
    /// `is_compressed` — whether to apply LZ77 compression (AC18+ only).
    /// `decomp_size` — max decompressed page size (default 0x7400).
    fn add_section(
        &mut self,
        name: &str,
        data: Vec<u8>,
        is_compressed: bool,
        decomp_size: usize,
    ) -> Result<()>;

    /// Assemble all registered sections and write the complete DWG file.
    fn write_file(&mut self) -> Result<Vec<u8>>;
}
