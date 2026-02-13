//! Shared test utilities for acadrust integration tests.
//!
//! Consolidates duplicated helpers (path resolution, read helpers, version
//! constants, entity counting, roundtrip testing) into a single module
//! that all test crates import via `mod common;`.

#![allow(dead_code)]

pub mod builders;
pub mod comparison;

use acadrust::entities::EntityType;
use acadrust::io::dxf::{DxfReader, DxfReaderConfiguration};
use acadrust::io::dwg::{DwgReader, DwgReaderConfiguration};
use acadrust::types::DxfVersion;
use acadrust::CadDocument;
use std::collections::BTreeMap;
use std::path::PathBuf;

// ===========================================================================
// Sample path resolution
// ===========================================================================

/// Resolve path to an ACadSharp-master reference sample DWG.
///
/// ```ignore
/// let p = sample_dwg_path("AC1015");
/// // → "<manifest>/ACadSharp-master/samples/sample_AC1015.dwg"
/// ```
pub fn sample_dwg_path(version: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("ACadSharp-master")
        .join("samples")
        .join(format!("sample_{version}.dwg"))
}

/// Resolve path to an ACadSharp-master reference sample DXF.
///
/// `format` is `"ascii"` or `"binary"`.
///
/// ```ignore
/// let p = sample_dxf_path("AC1015", "ascii");
/// // → "<manifest>/ACadSharp-master/samples/sample_AC1015_ascii.dxf"
/// ```
pub fn sample_dxf_path(version: &str, format: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("ACadSharp-master")
        .join("samples")
        .join(format!("sample_{version}_{format}.dxf"))
}

/// Resolve path to a file in the `reference_samples/` directory at the crate root.
pub fn reference_sample_path(filename: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("reference_samples")
        .join(filename)
}

/// Resolve path into the `test_output/` directory, creating it if needed.
pub fn test_output_path(filename: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("test_output");
    let _ = std::fs::create_dir_all(&dir);
    dir.join(filename)
}

// ===========================================================================
// Version constants
// ===========================================================================

/// All DWG/DXF versions we have reference samples for.
pub const ALL_VERSIONS: [(DxfVersion, &str); 8] = [
    (DxfVersion::AC1012, "R13"),
    (DxfVersion::AC1014, "R14"),
    (DxfVersion::AC1015, "2000"),
    (DxfVersion::AC1018, "2004"),
    (DxfVersion::AC1021, "2007"),
    (DxfVersion::AC1024, "2010"),
    (DxfVersion::AC1027, "2013"),
    (DxfVersion::AC1032, "2018"),
];

/// Versions the DXF writer can target.
pub const DXF_WRITABLE_VERSIONS: [DxfVersion; 8] = [
    DxfVersion::AC1012,
    DxfVersion::AC1014,
    DxfVersion::AC1015,
    DxfVersion::AC1018,
    DxfVersion::AC1021,
    DxfVersion::AC1024,
    DxfVersion::AC1027,
    DxfVersion::AC1032,
];

/// DWG version strings that have reference sample files.
pub const DWG_SAMPLE_VERSIONS: [&str; 7] = [
    "AC1014", "AC1015", "AC1018", "AC1021", "AC1024", "AC1027", "AC1032",
];

/// DXF version strings that have reference sample files (both ascii & binary).
pub const DXF_SAMPLE_VERSIONS: [&str; 7] = [
    "AC1009", "AC1015", "AC1018", "AC1021", "AC1024", "AC1027", "AC1032",
];

// ===========================================================================
// Read helpers
// ===========================================================================

/// Read a DWG file in failsafe mode (errors logged but not fatal).
pub fn read_dwg(path: &str) -> CadDocument {
    let config = DwgReaderConfiguration {
        failsafe: true,
        ..Default::default()
    };
    DwgReader::from_file(path)
        .unwrap_or_else(|e| panic!("Cannot open DWG {path}: {e:?}"))
        .with_config(config)
        .read()
        .unwrap_or_else(|e| panic!("Failed to read DWG {path}: {e:?}"))
}

/// Read a DWG file in strict mode (errors are fatal).
pub fn read_dwg_strict(path: &str) -> Result<CadDocument, Box<dyn std::error::Error>> {
    let config = DwgReaderConfiguration {
        failsafe: false,
        ..Default::default()
    };
    let doc = DwgReader::from_file(path)?.with_config(config).read()?;
    Ok(doc)
}

/// Read a DXF file in failsafe mode.
pub fn read_dxf(path: &str) -> CadDocument {
    let config = DxfReaderConfiguration { failsafe: true };
    DxfReader::from_file(path)
        .unwrap_or_else(|e| panic!("Cannot open DXF {path}: {e:?}"))
        .with_configuration(config)
        .read()
        .unwrap_or_else(|e| panic!("Failed to read DXF {path}: {e:?}"))
}

/// Read a DXF file in strict mode (no failsafe).
pub fn read_dxf_strict(path: &str) -> CadDocument {
    DxfReader::from_file(path)
        .unwrap_or_else(|e| panic!("Cannot open DXF {path}: {e:?}"))
        .read()
        .unwrap_or_else(|e| panic!("Failed to read DXF {path}: {e:?}"))
}

/// Read a DWG reference sample by version string (e.g. `"AC1015"`).
pub fn read_sample_dwg(version: &str) -> CadDocument {
    let path = sample_dwg_path(version);
    read_dwg(path.to_str().unwrap())
}

/// Read a DXF reference sample by version string and format.
pub fn read_sample_dxf(version: &str, format: &str) -> CadDocument {
    let path = sample_dxf_path(version, format);
    read_dxf(path.to_str().unwrap())
}

// ===========================================================================
// Entity utilities
// ===========================================================================

/// Return the DXF-style type name string for an EntityType variant.
pub fn entity_type_name(e: &EntityType) -> &'static str {
    e.as_entity().entity_type()
}

/// Build a sorted frequency map of entity type names.
pub fn entity_type_histogram(doc: &CadDocument) -> BTreeMap<&'static str, usize> {
    let mut map = BTreeMap::new();
    for e in doc.entities() {
        *map.entry(entity_type_name(e)).or_insert(0) += 1;
    }
    map
}

/// Build a sorted frequency map with owned String keys.
pub fn entity_type_counts(doc: &CadDocument) -> BTreeMap<String, usize> {
    let mut map = BTreeMap::new();
    for e in doc.entities() {
        *map.entry(entity_type_name(e).to_string()).or_insert(0) += 1;
    }
    map
}

/// Count entities in the document.
pub fn entity_count(doc: &CadDocument) -> usize {
    doc.entities().count()
}

// ===========================================================================
// Table name collectors
// ===========================================================================

/// Collect sorted layer names.
pub fn layer_names(doc: &CadDocument) -> Vec<String> {
    let mut names: Vec<_> = doc.layers.iter().map(|l| l.name.clone()).collect();
    names.sort();
    names
}

/// Collect sorted line-type names.
pub fn linetype_names(doc: &CadDocument) -> Vec<String> {
    let mut names: Vec<_> = doc.line_types.iter().map(|lt| lt.name.clone()).collect();
    names.sort();
    names
}

/// Collect sorted text-style names.
pub fn textstyle_names(doc: &CadDocument) -> Vec<String> {
    let mut names: Vec<_> = doc.text_styles.iter().map(|ts| ts.name.clone()).collect();
    names.sort();
    names
}

/// Collect sorted block-record names.
pub fn block_record_names(doc: &CadDocument) -> Vec<String> {
    let mut names: Vec<_> = doc.block_records.iter().map(|br| br.name.clone()).collect();
    names.sort();
    names
}

// ===========================================================================
// DXF write + read-back
// ===========================================================================

/// Write a document to a DXF file and read it back in failsafe mode.
pub fn write_and_read_back_dxf(doc: &CadDocument, path: &str) -> CadDocument {
    acadrust::DxfWriter::new(doc.clone())
        .write_to_file(path)
        .unwrap_or_else(|e| panic!("Failed to write DXF {path}: {e:?}"));
    read_dxf(path)
}

/// Roundtrip: write doc as DXF, read back, return the read-back document.
/// Uses a temp file in `test_output/`.
pub fn roundtrip_dxf(doc: &CadDocument, label: &str) -> CadDocument {
    let path = test_output_path(&format!("roundtrip_{label}.dxf"));
    write_and_read_back_dxf(doc, path.to_str().unwrap())
}

// ===========================================================================
// DWG write + read-back
// ===========================================================================

/// Write a document to DWG bytes via `DwgWriter::write`.
pub fn write_dwg_bytes(doc: &CadDocument) -> Vec<u8> {
    acadrust::io::dwg::DwgWriter::write(doc)
        .unwrap_or_else(|e| panic!("DwgWriter::write failed: {e:?}"))
}

/// Write a document to DWG bytes, then read it back in failsafe mode.
/// Returns the re-read document.
pub fn roundtrip_dwg(doc: &CadDocument, label: &str) -> CadDocument {
    let bytes = write_dwg_bytes(doc);

    // Also persist to disk for debugging/manual inspection.
    let path = test_output_path(&format!("roundtrip_{label}.dwg"));
    std::fs::write(&path, &bytes)
        .unwrap_or_else(|e| panic!("Failed to write DWG to {}: {e}", path.display()));

    // Read back from bytes
    let cursor = std::io::Cursor::new(bytes);
    let config = DwgReaderConfiguration { failsafe: true, ..Default::default() };
    DwgReader::from_reader(cursor)
        .unwrap_or_else(|e| panic!("DwgReader::from_reader failed for {label}: {e:?}"))
        .with_config(config)
        .read()
        .unwrap_or_else(|e| panic!("DwgReader::read failed for {label}: {e:?}"))
}

/// Write a document to DWG bytes and attempt to read back.
/// Returns `Ok(doc)` on success, `Err(msg)` on failure — never panics.
pub fn try_roundtrip_dwg(doc: &CadDocument, label: &str) -> std::result::Result<CadDocument, String> {
    let bytes = match acadrust::io::dwg::DwgWriter::write(doc) {
        Ok(b) => b,
        Err(e) => return Err(format!("DwgWriter::write failed for {label}: {e:?}")),
    };

    // Persist for debugging.
    let path = test_output_path(&format!("roundtrip_{label}.dwg"));
    let _ = std::fs::write(&path, &bytes);

    let cursor = std::io::Cursor::new(bytes);
    let config = DwgReaderConfiguration { failsafe: true, ..Default::default() };
    DwgReader::from_reader(cursor)
        .map_err(|e| format!("DwgReader::from_reader failed for {label}: {e:?}"))?
        .with_config(config)
        .read()
        .map_err(|e| format!("DwgReader::read failed for {label}: {e:?}"))
}
