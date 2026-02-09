//! DWG bit-level stream writers.
//!
//! This module contains the writer side of the DWG bit-level I/O system:
//!
//! - [`stream_writer`] — the `IDwgStreamWriter` trait
//! - [`stream_writer_base`] — the concrete implementation with version-specific logic
//! - [`merged_writer`] — the R2007+ and pre-R2007 multiplexed writers

pub mod merged_writer;
pub mod stream_writer;
pub mod stream_writer_base;

pub use merged_writer::{DwgMergedStreamWriter, DwgMergedStreamWriterAC14};
pub use stream_writer::IDwgStreamWriter;
pub use stream_writer_base::{get_stream_writer, DwgStreamWriterBase};
