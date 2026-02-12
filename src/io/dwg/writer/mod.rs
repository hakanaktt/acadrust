//! DWG writer modules.
//!
//! This module contains the writer side of the DWG I/O system:
//!
//! - [`stream_writer`] — the `IDwgStreamWriter` trait
//! - [`stream_writer_base`] — the concrete implementation with version-specific logic
//! - [`merged_writer`] — the R2007+ and pre-R2007 multiplexed writers
//! - [`header_writer`] — header section writer
//! - [`classes_writer`] — classes section writer
//! - [`handle_writer`] — handle/object map section writer
//! - [`preview_writer`] — preview/thumbnail section writer
//! - [`app_info_writer`] — application info section writer (R2004+)
//! - [`aux_header_writer`] — auxiliary header section writer
//! - [`file_header_writer`] — file header assemblers (AC15/AC18)

pub mod app_info_writer;
pub mod aux_header_writer;
pub mod classes_writer;
pub mod dwg_writer;
pub mod file_header_writer;
pub mod handle_writer;
pub mod header_writer;
pub mod merged_writer;
pub mod object_writer;
pub mod preview_writer;
pub mod stream_writer;
pub mod stream_writer_base;
pub mod summary_info_writer;

pub use merged_writer::{DwgMergedStreamWriter, DwgMergedStreamWriterAC14};
pub use stream_writer::IDwgStreamWriter;
pub use stream_writer_base::{get_stream_writer, DwgStreamWriterBase};
pub use header_writer::DwgHeaderWriter;
pub use classes_writer::DwgClassesWriter;
pub use handle_writer::DwgHandleWriter;
pub use preview_writer::DwgPreviewWriter;
pub use app_info_writer::DwgAppInfoWriter;
pub use aux_header_writer::DwgAuxHeaderWriter;
pub use file_header_writer::{
    IDwgFileHeaderWriter, DwgFileHeaderWriterAC15, DwgFileHeaderWriterAC18,
};
pub use dwg_writer::DwgWriter;
pub use object_writer::DwgObjectWriter;
pub use summary_info_writer::DwgSummaryInfoWriter;
