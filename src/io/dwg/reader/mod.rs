//! DWG bit-level stream readers and section readers.
//!
//! This module contains the reader side of the DWG I/O system:
//!
//! ## Stream readers (Phase 2)
//! - [`stream_reader`] — the `IDwgStreamReader` trait
//! - [`stream_reader_base`] — the concrete implementation with version-specific logic
//! - [`merged_reader`] — the R2007+ multiplexed reader (main/text/handle streams)
//!
//! ## Section readers (Phase 4)
//! - [`header_reader`] — `AcDb:Header` section (system variables)
//! - [`classes_reader`] — `AcDb:Classes` section (DXF class definitions)
//! - [`handle_reader`] — `AcDb:Handles` section (object map)
//! - [`summary_info_reader`] — `AcDb:SummaryInfo` section (document metadata)
//! - [`preview_reader`] — `AcDb:Preview` section (thumbnail image)
//! - [`app_info_reader`] — `AcDb:AppInfo` section (application info)

pub mod merged_reader;
pub mod stream_reader;
pub mod stream_reader_base;

pub mod app_info_reader;
pub mod classes_reader;
pub mod dwg_reader;
pub mod handle_reader;
pub mod header_reader;
pub mod object_reader;
pub mod preview_reader;
pub mod summary_info_reader;

pub use merged_reader::DwgMergedReader;
pub use stream_reader::IDwgStreamReader;
pub use stream_reader_base::{get_stream_handler, DwgStreamReaderBase};

pub use app_info_reader::{AppInfo, DwgAppInfoReader};
pub use classes_reader::DwgClassesReader;
pub use dwg_reader::{DwgReader, DwgReaderConfiguration};
pub use handle_reader::DwgHandleReader;
pub use header_reader::DwgHeaderReader;
pub use preview_reader::DwgPreviewReader;
pub use summary_info_reader::DwgSummaryInfoReader;
