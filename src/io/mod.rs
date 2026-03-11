//! File I/O for DXF, DWG, and SVG formats.
//!
//! | Sub-module | Capabilities |
//! |------------|------------------------------------------------------|
//! | [`dxf`]    | Read/write ASCII and Binary DXF (R12 – R2018+)      |
//! | [`dwg`]    | Read/write native DWG binary (R13 – R2018)          |
//! | [`svg`]    | Export to Scalable Vector Graphics (SVG)            |
//!
//! The top-level re-exports [`DxfReader`], [`DxfWriter`], [`DwgReader`],
//! [`DwgWriter`], and [`SvgWriter`] for quick access.

pub mod dxf;
pub mod dwg;
pub mod svg;

pub use dxf::{DxfReader, DxfWriter};
pub use dwg::{DwgReader, DwgWriter};
pub use svg::SvgWriter;

