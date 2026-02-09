//! DWG bit-level stream readers.
//!
//! This module contains the reader side of the DWG bit-level I/O system:
//!
//! - [`stream_reader`] — the `IDwgStreamReader` trait
//! - [`stream_reader_base`] — the concrete implementation with version-specific logic
//! - [`merged_reader`] — the R2007+ multiplexed reader (main/text/handle streams)

pub mod merged_reader;
pub mod stream_reader;
pub mod stream_reader_base;

pub use merged_reader::DwgMergedReader;
pub use stream_reader::IDwgStreamReader;
pub use stream_reader_base::{get_stream_handler, DwgStreamReaderBase};
