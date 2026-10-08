//! # acadrust
//!
//! A pure Rust library for reading, writing, and inspecting CAD files in DXF
//! (ASCII and binary) and native binary DWG formats. DXF support spans R12
//! through R2018+; DWG support spans R12 through R2018+.
//!
//! ## Highlights
//!
//! - **48 top-level [`EntityType`] variants** for geometry, annotations,
//!   dimensions, meshes, solids, regions, bodies, and surfaces.
//! - **ACIS/SAT/SAB** parsing and writing, including solid history and primitive
//!   builders.
//! - **Tables and objects** for layers, styles, dictionaries, layouts, fields,
//!   materials, and associative data.
//! - **Failsafe reads** with bounded [`ReadDiagnostic`] values and aggregate
//!   [`ReadStats`] returned by [`DxfReader::read_with_stats`] and
//!   [`DwgReader::read_with_stats`].
//! - **Automatic code-page handling** for older drawings.
//! - Optional Serde serialization and 3D import support.
//!
//! ## Feature Flags
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `serde` | Enables `serde::Serialize` and `serde::Deserialize` implementations. |
//! | `import` | Enables STL, COLLADA, OBJ, glTF/GLB, and FBX importers. |
//!
//! ```toml
//! [dependencies]
//! acadrust = { version = "0.6.2", features = ["serde", "import"] }
//! ```
//!
//! ## Quick Start — DXF
//!
//! ```rust,no_run
//! use acadrust::DxfReader;
//! use acadrust::DxfWriter;
//!
//! # fn main() -> acadrust::Result<()> {
//! let doc = DxfReader::from_file("input.dxf")?.read()?;
//! println!("Entities: {}", doc.entities().count());
//! DxfWriter::new(&doc).write_to_file("output.dxf")?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Quick Start — DWG
//!
//! ```rust,no_run
//! use acadrust::{CadDocument, Color, DwgReader, DwgWriter, EntityType, Line};
//!
//! # fn main() -> acadrust::Result<()> {
//! let mut reader = DwgReader::from_file("input.dwg")?;
//! let doc = reader.read()?;
//! println!("Entities: {}", doc.entities().count());
//!
//! let mut output = CadDocument::new();
//! let mut line = Line::from_coords(0.0, 0.0, 0.0, 100.0, 50.0, 0.0);
//! line.common.color = Color::RED;
//! output.add_entity(EntityType::Line(line))?;
//! DwgWriter::write_to_file("output.dwg", &output)?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Failsafe Reading and Diagnostics
//!
//! Set [`DxfReaderConfiguration::failsafe`] to continue past recoverable
//! record errors. The returned [`ReadOutcome`] contains the document plus
//! source/decode counts and structured diagnostics; strict mode remains the
//! default.
//!
//! ```rust,no_run
//! use acadrust::{DxfReader, DxfReaderConfiguration};
//!
//! # fn main() -> acadrust::Result<()> {
//! let config = DxfReaderConfiguration { failsafe: true, ..Default::default() };
//! let outcome = DxfReader::from_file("drawing.dxf")?
//!     .with_configuration(config)
//!     .read_with_stats()?;
//! println!("{} entities, {} diagnostics", outcome.stats.output_entities,
//!     outcome.stats.diagnostics.len());
//! # Ok(())
//! # }
//! ```
//!
//! ## Optional 3D Imports
//!
//! With `features = ["import"]`, `import_file` auto-detects STL, COLLADA,
//! OBJ, glTF/GLB, and FBX by extension and converts them to a [`CadDocument`].
//!
//! ```rust,no_run
//! # fn main() -> acadrust::Result<()> {
//! # #[cfg(feature = "import")]
//! # {
//! use acadrust::{import_file, ImportConfig};
//!
//! let doc = import_file("model.glb", &ImportConfig::default())?;
//! println!("Imported {} entities", doc.entities().count());
//! # }
//! # Ok(())
//! # }
//! ```
//!
//! ## Serde
//!
//! With `features = ["serde"]`, document types implement Serde traits and can
//! be serialized to JSON using your chosen JSON crate.
//!
//! ```rust,no_run
//! # fn main() -> acadrust::Result<()> {
//! # #[cfg(feature = "serde")]
//! # {
//! use acadrust::{CadDocument, DxfReader};
//!
//! let doc = DxfReader::from_file("drawing.dxf")?.read()?;
//! let json = serde_json::to_string(&doc).unwrap();
//! let restored: CadDocument = serde_json::from_str(&json).unwrap();
//! assert_eq!(restored.entities().count(), doc.entities().count());
//! # }
//! # Ok(())
//! # }
//! ```
//!
//! ## Module Overview
//!
//! | Module | Contents |
//! |--------|----------|
//! | [`document`] | [`CadDocument`] — central drawing container |
//! | [`entities`] | 48 top-level entity variants |
//! | [`tables`] | Table entries such as [`Layer`] and [`LineType`] |
//! | [`objects`] | Non-graphical objects, dictionaries, layouts, and styles |
//! | [`types`] | Primitives such as [`Vector3`], [`Color`], and [`DxfVersion`] |
//! | [`io`] | DXF, DWG, and optional import readers and writers |
//! | [`entities::acis`] | ACIS/SAT/SAB parsing, writing, and primitive builders |
//! | [`notification`] | Structured parse notifications and recovery counts |
//!
//! ## File Version Support
//!
//! | Code | AutoCAD | DXF | DWG |
//! |------|---------|-----|-----|
//! | AC1009 | R12 | R/W | R/W |
//! | AC1012 | R13 | R/W | R/W |
//! | AC1014 | R14 | R/W | R/W |
//! | AC1015 | 2000 | R/W | R/W |
//! | AC1018 | 2004 | R/W | R/W |
//! | AC1021 | 2007 | R/W | R/W |
//! | AC1024 | 2010 | R/W | R/W |
//! | AC1027 | 2013 | R/W | R/W |
//! | AC1032 | 2018+ | R/W | R/W |
//!
//! `R/W` describes the format-level reader and writer paths. Entity
//! availability varies by file version. See the
//! [per-version compatibility matrix](https://github.com/hakanaktt/acadrust/blob/main/src/docs/entity_status_matrix.md),
//! which records tested fixtures and CAD-engine audit results rather than a
//! guarantee for every possible drawing.

#![allow(missing_docs)]
#![warn(rustdoc::missing_crate_level_docs)]

pub mod classes;
pub mod compound_file;
pub mod count;
pub mod document;
mod current_transparency;
pub mod nested_copy;
pub mod entities;
pub mod error;
pub mod fields;
pub mod io;
pub mod layer_state;
pub mod notification;
pub mod objects;
pub mod sheet_set;
pub mod tables;
pub mod types;
pub mod vba;
pub mod xdata;

// Re-export commonly used types
pub use error::{DxfError, Result};
pub use types::{
    BoundingBox2D, BoundingBox3D, Color, DxfVersion, Handle, LineWeight, Transparency, Vector2,
    Vector3,
};

// Re-export entity types
pub use entities::{
    Arc, Circle, Ellipse, EmbeddedEntity, Entity, EntityType, Line, LwPolyline, MText, Point,
    Polyline, ProxyGraphicRecord, ProxyGraphics, ProxyUnicodeText, Spline, Text,
};

// Re-export table types
pub use tables::{
    AppId, BlockRecord, DimStyle, Layer, LineType, Table, TableEntry, TextStyle, Ucs, VPort, View,
    VxTableRecord,
};

// Re-export document
pub use document::{
    CadDocument, Preview, PreviewFormat, SemanticEntityV1, SemanticInventoryV1, SemanticNodeV1,
    SemanticObjectV1, SemanticPartV1, SemanticReferenceV1, SemanticRelationshipKindV1,
    SemanticTableRecordV1, SolidHistoryGraph, SEMANTIC_INVENTORY_VERSION,
};
pub use layer_state::{LayerState, LayerStateLayer, LayerStateMask};

// Re-export I/O types
pub use io::dwg::{DwgReadOptions, DwgReader, DwgWriter};
pub use io::dxf::{DxfReader, DxfReaderConfiguration, DxfWriter};
pub use io::read::{
    push_read_diagnostic, ReadDiagnostic, ReadOutcome, ReadStage, ReadStats, SourceFormat,
    MAX_READ_DIAGNOSTICS,
};

// Re-export ACIS types
pub use entities::acis::primitives;
pub use entities::acis::{SabReader, SabWriter, SatParser, SatWriter};
pub use entities::acis::{SatDocument, SatHeader, SatPointer, SatRecord, SatToken, SatVersion};

// Re-export import types (when `import` feature is enabled)
#[cfg(feature = "import")]
pub use io::import::{
    import_file, ColladaImporter, FbxImporter, GltfImporter, ImportConfig, ImportFormat,
    ObjImporter, StlImporter,
};

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_cad_document_creation() {
        let doc = CadDocument::new();
        assert_eq!(doc.version, DxfVersion::AC1032);

        let doc2 = CadDocument::with_version(DxfVersion::AC1015);
        assert_eq!(doc2.version, DxfVersion::AC1015);
    }
}

mod drawing_variables;

mod hatch_origin;
