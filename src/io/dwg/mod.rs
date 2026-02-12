//! DWG file format reader/writer support.
//!
//! This module implements reading and writing of AutoCAD DWG binary files.
//! It follows the structure of the ACadSharp C# implementation.
//!
//! # Module Structure
//!
//! - [`constants`] — Magic numbers, sentinel bytes, section names, version-specific constants
//! - [`crc`] — CRC-8 (16-bit) and CRC-32 computation and stream handlers
//! - [`checksum`] — Adler-32, magic sequence generation, compression padding
//! - [`encryption`] — AC18+ data section page header encryption/decryption
//! - [`compression`] — LZ77 AC18 and AC21 compressors/decompressors
//! - [`reed_solomon`] — Byte de-interleaving for AC21 Reed-Solomon encoded data
//! - [`reference_type`] — DWG handle reference codes and resolution
//! - [`header_handles`] — Named handle collection for DWG file header references
//! - [`section_io`] — Version-conditional section reading/writing helpers
//! - [`file_header`] — DWG file header structures (AC15, AC18, AC21)
//! - [`object_type`] — DWG object type codes
//! - [`reader`] — Bit-level DWG stream readers
//! - [`writer`] — Bit-level DWG stream writers

pub mod builder;
pub mod checksum;
pub mod compression;
pub mod constants;
pub mod crc;
pub mod encryption;
pub mod file_header;
pub mod header_handles;
pub mod object_type;
pub mod reader;
pub mod reed_solomon;
pub mod reference_type;
pub mod section_io;
pub mod writer;

// Re-export commonly used types
pub use compression::{Compressor, Decompressor};
pub use file_header::{
    Dwg21CompressedMetadata, DwgFileHeader, DwgFileHeaderAC15, DwgFileHeaderAC18,
    DwgFileHeaderAC21, DwgLocalSectionMap, DwgSectionDescriptor, DwgSectionLocatorRecord,
};
pub use header_handles::DwgHeaderHandlesCollection;
pub use object_type::DwgObjectType;
pub use builder::DwgDocumentBuilder;
pub use reader::{DwgMergedReader, DwgReader, DwgReaderConfiguration, DwgStreamReaderBase, IDwgStreamReader};
pub use reference_type::{DwgReferenceType, HandleReference};
pub use section_io::SectionIO;
pub use writer::{
    DwgMergedStreamWriter, DwgMergedStreamWriterAC14, DwgStreamWriterBase, DwgWriter,
    IDwgStreamWriter,
};
