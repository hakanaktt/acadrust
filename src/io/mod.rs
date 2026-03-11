//! File I/O for DXF, DWG, SVG, and PDF formats.
//!
//! | Sub-module | Capabilities |
//! |------------|------------------------------------------------------|
//! | [`dxf`]    | Read/write ASCII and Binary DXF (R12 – R2018+)      |
//! | [`dwg`]    | Read/write native DWG binary (R13 – R2018)          |
//! | [`svg`]    | Export to Scalable Vector Graphics (SVG)            |
//! | [`pdf`]    | Export to Portable Document Format (PDF 1.4)        |
//!
//! The top-level re-exports [`DxfReader`], [`DxfWriter`], [`DwgReader`],
//! [`DwgWriter`], [`SvgWriter`], and [`PdfWriter`] for quick access.

pub mod dxf;
pub mod dwg;
pub mod svg;
pub mod pdf;

pub use dxf::{DxfReader, DxfWriter};
pub use dwg::{DwgReader, DwgWriter};
pub use svg::SvgWriter;
pub use pdf::PdfWriter;

