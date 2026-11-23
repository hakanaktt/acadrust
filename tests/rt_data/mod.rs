//! Shared helpers for the data-cluster codec roundtrip tests (`rt_data_*`):
//! COGO points, point groups and queries, user-defined point properties,
//! TIN surfaces and definitions, sites and alignments on a site, feature
//! lines, and parcels / lot lines.
//!
//! Every test writes with the civil writers, saves the drawing as a DWG to a
//! unique scratch file, reopens it with the reader and asserts what
//! `docs/civil-write.md` documents for the written records.
#![allow(dead_code)]

use acadrust::classes::arch::{ArchEntityData, ArchObject};
use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::objects::ObjectType;
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter};

/// A new AC1032 drawing. The civil writers bootstrap their own support
/// records (style and settings tree) the first time one runs.
pub fn rt_data_drawing() -> CadDocument {
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1032;
    doc
}

/// Saves `doc` as a DWG to a unique scratch file, reopens it from the file
/// and removes the scratch file.
pub fn rt_data_roundtrip(doc: &CadDocument) -> CadDocument {
    static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("rt-data-{}-{n}.dwg", std::process::id()));
    DwgWriter::write_to_file(&path, doc).expect("write the scratch DWG");
    let back = DwgReader::from_file(&path)
        .expect("reopen the scratch DWG")
        .read()
        .expect("parse the scratch DWG");
    let _ = std::fs::remove_file(&path);
    back
}

/// The civil objects (`AECC_*` non-entity records) of one class name.
pub fn rt_data_objects(doc: &CadDocument, dxf_name: &str) -> Vec<ArchObject> {
    doc.objects
        .values()
        .filter_map(|o| match o {
            ObjectType::Arch(a) if a.dxf_name == dxf_name => Some(a.clone()),
            _ => None,
        })
        .collect()
}

/// The civil object of one class name with this handle.
pub fn rt_data_object(doc: &CadDocument, handle: Handle) -> Option<ArchObject> {
    match doc.objects.get(&handle) {
        Some(ObjectType::Arch(a)) => Some(a.clone()),
        _ => None,
    }
}

/// The civil entities of one class name with their handles.
pub fn rt_data_entities<'a>(doc: &'a CadDocument, dxf_name: &str) -> Vec<(Handle, &'a ArchEntityData)> {
    doc.entities()
        .filter_map(|e| match e {
            EntityType::Extended(x) => match &x.data {
                ExtendedEntityData::Arch(d) if d.dxf_name == dxf_name => Some((x.common.handle, d)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}
