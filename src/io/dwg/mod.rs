//! DWG binary file format support.
//!
//! Read and write ACAD's native binary format.  DWG files use
//! bit-granularity encoding, version-specific data layouts, and LZ77
//! compression (R2004+).
//!
//! # Reading
//!
//! ```rust,ignore
//! use acadrust::DwgReader;
//!
//! let doc = DwgReader::from_file("drawing.dwg")?.read()?;
//! ```
//!
//! # Writing
//!
//! ```rust,ignore
//! use acadrust::DwgWriter;
//!
//! DwgWriter::write_to_file("output.dwg", &doc)?;
//! ```
//!
//! ## Supported versions
//!
//! | DWG Version | ACAD | File format  |
//! |-------------|---------|-------------|
//! | AC1009      | R11/R12 | Pre-R13     |
//! | AC1012      | R13     | Linear      |
//! | AC1014      | R14     | Linear      |
//! | AC1015      | R2000   | Linear      |
//! | AC1018      | R2004   | Paged + LZ77 |
//! | AC1021      | R2007   | Paged + LZ77 |
//! | AC1024      | R2010   | Paged + LZ77 |
//! | AC1027      | R2013   | Paged + LZ77 |
//! | AC1032      | R2018   | Paged + LZ77 |

/// Text from a caught decoder panic, for diagnostics instead of an unwind.
pub(crate) fn unwind_message(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| {
            payload
                .downcast_ref::<&str>()
                .map(|message| (*message).to_string())
        })
        .unwrap_or_else(|| "Record decoder failed".to_string())
}

#[cfg(test)]
mod unwind_tests {
    #[test]
    fn unwind_message_uses_the_panic_text() {
        let owned: Box<dyn std::any::Any + Send> = Box::new("array count".to_string());
        assert_eq!(super::unwind_message(owned.as_ref()), "array count");
        let borrowed: Box<dyn std::any::Any + Send> = Box::new("truncated");
        assert_eq!(super::unwind_message(borrowed.as_ref()), "truncated");
    }
}

pub mod annotative_eed;
pub mod checksum;
pub mod compression;
pub mod compressor_ac21;
pub mod crc;
pub mod decompressor_ac18;
pub mod decompressor_ac21;
pub mod dwg21_metadata;
pub mod dwg_document_builder;
pub mod dwg_reader;
pub mod dwg_reference_type;
pub mod dwg_stream_readers;
pub mod dwg_stream_writers;
pub mod dwg_version;
pub mod dwg_writer;
pub mod eed_codec;
pub(crate) mod embedded_entity;
pub mod file_headers;
pub(crate) mod legacy_viewport;
mod mesh_budget;
mod parallel;
pub mod preview;
pub mod r12;
pub mod reed_solomon;
pub mod security;
mod shard_plan;
pub(crate) mod size_limits;
pub(crate) mod typeface_eed;

pub use dwg_reader::DwgReadOptions;
pub use dwg_reader::DwgReader;
pub use dwg_reference_type::DwgReferenceType;
pub use dwg_version::DwgVersion;
pub use dwg_writer::DwgWriter;
pub use mesh_budget::MeshFaceBudget;

pub(crate) fn sab_fingerprint<'a>(
    entries: impl IntoIterator<Item = (crate::Handle, &'a [u8])>,
) -> Vec<(u64, usize, u64)> {
    use std::hash::{Hash, Hasher};

    let mut fingerprint = Vec::new();
    for (handle, bytes) in entries {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        bytes.hash(&mut hasher);
        fingerprint.push((handle.value(), bytes.len(), hasher.finish()));
    }
    fingerprint.sort_unstable_by_key(|entry| entry.0);
    fingerprint
}
pub use file_headers::{DwgFileHeaderWriterAC15, DwgFileHeaderWriterAC18};
