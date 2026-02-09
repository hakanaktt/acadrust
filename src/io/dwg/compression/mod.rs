//! LZ77 compression and decompression for DWG file format.
//!
//! The DWG format uses two LZ77 variants:
//! - **AC18** (R2004, R2010, R2013, R2018): Standard LZ77 variant
//! - **AC21** (R2007): Different opcode format

pub mod lz77_ac18;
pub mod lz77_ac21;

use crate::error::Result;

/// Trait for compressing data.
///
/// Mirrors ACadSharp's `ICompressor` interface.
pub trait Compressor {
    /// Compress a source buffer starting at `offset` for `total_size` bytes.
    fn compress(&self, source: &[u8], offset: usize, total_size: usize) -> Result<Vec<u8>>;
}

/// Trait for decompressing data.
pub trait Decompressor {
    /// Decompress a source buffer, returning a buffer of `decompressed_size` bytes.
    fn decompress(&self, source: &[u8], decompressed_size: usize) -> Result<Vec<u8>>;
}
