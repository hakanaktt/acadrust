//! DWG round-trip coverage for UNDERLAY references (PDF / DWF / DGN).
//!
//! Locks in the AcDbUnderlayReference encoder/decoder pair: the object-data
//! fields (normal, insertion, rotation, x/y/z scale, display flags, contrast,
//! fade), the mid-record definition handle drawn from the handle stream, and
//! the bit-long-counted raw clip-boundary vertices. A desync in either the
//! writer field order or the class registration drops the entity, so each
//! flavour must survive a byte round-trip across DWG versions.

use std::io::Cursor;

use acadrust::entities::underlay::{Underlay, UnderlayDisplayFlags, UnderlayType};
use acadrust::entities::EntityType;
use acadrust::objects::{ObjectType, UnderlayDefinition};
use acadrust::types::{DxfVersion, Handle, Vector2, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

fn dwg_roundtrip(doc: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(doc).expect("DWG write failed");
    DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG read failed")
}

fn dxf_roundtrip(doc: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(doc)
        .write_to_vec()
        .expect("DXF write failed");
    DxfReader::from_reader(Cursor::new(bytes))
        .expect("DXF reader init failed")
        .read()
        .expect("DXF read failed")
}

/// Build a fully-populated underlay of the given flavour.
fn sample_underlay(kind: UnderlayType) -> Underlay {
    let mut u = Underlay::new(kind);
    u.normal = Vector3::new(0.0, 0.0, 1.0);
    u.insertion_point = Vector3::new(10.0, 20.0, 5.0);
    u.rotation = 0.5;
    u.x_scale = 2.0;
    u.y_scale = 3.0;
    u.z_scale = 1.5;
    u.flags = UnderlayDisplayFlags::CLIPPING | UnderlayDisplayFlags::ON;
    u.contrast = 75;
    u.fade = 20;
    u.definition_handle = Handle::new(0x2A);
    u.clip_boundary_vertices = vec![
        Vector2::new(0.0, 0.0),
        Vector2::new(100.0, 0.0),
        Vector2::new(100.0, 50.0),
    ];
    u
}

/// Round-trip an underlay through DWG at `version` and pull it back out.
fn roundtrip_underlay(version: DxfVersion, kind: UnderlayType) -> Underlay {
    let mut doc = CadDocument::with_version(version);
    doc.add_entity(EntityType::Underlay(sample_underlay(kind)))
        .unwrap();
    let rt = dwg_roundtrip(&doc);
    let found = rt.entities().find_map(|e| match e {
        EntityType::Underlay(u) => Some(u.clone()),
        _ => None,
    });
    found.expect("UNDERLAY missing after DWG roundtrip")
}

fn assert_fields(u: &Underlay, kind: UnderlayType, label: &str) {
    assert_eq!(u.underlay_type, kind, "{label}: underlay type");
    assert_eq!(
        u.insertion_point,
        Vector3::new(10.0, 20.0, 5.0),
        "{label}: insertion point"
    );
    assert_eq!(u.rotation, 0.5, "{label}: rotation");
    assert_eq!(u.x_scale, 2.0, "{label}: x scale");
    assert_eq!(u.y_scale, 3.0, "{label}: y scale");
    assert_eq!(u.z_scale, 1.5, "{label}: z scale");
    assert_eq!(
        u.flags,
        UnderlayDisplayFlags::CLIPPING | UnderlayDisplayFlags::ON,
        "{label}: display flags"
    );
    assert_eq!(u.contrast, 75, "{label}: contrast");
    assert_eq!(u.fade, 20, "{label}: fade");
    assert_eq!(
        u.definition_handle,
        Handle::new(0x2A),
        "{label}: def handle"
    );
    assert_eq!(
        u.clip_boundary_vertices.len(),
        3,
        "{label}: clip vertex count"
    );
    assert_eq!(
        u.clip_boundary_vertices[2],
        Vector2::new(100.0, 50.0),
        "{label}: last clip vertex"
    );
}

#[test]
fn underlay_pdf_dwg_roundtrip_all_versions() {
    for version in [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1024,
        DxfVersion::AC1032,
    ] {
        let u = roundtrip_underlay(version, UnderlayType::Pdf);
        assert_fields(&u, UnderlayType::Pdf, &format!("PDF {version:?}"));
    }
}

/// Build a document carrying a single underlay definition object.
fn definition_document(version: DxfVersion, kind: UnderlayType) -> CadDocument {
    let mut doc = CadDocument::with_version(version);
    let mut def = UnderlayDefinition::new(kind);
    def.handle = Handle::new(0x400);
    def.owner_handle = Handle::new(0x0C); // named-object dictionary
    def.file_path = "C:/refs/site-plan.pdf".to_string();
    def.page_name = "Sheet1".to_string();
    doc.objects
        .insert(def.handle, ObjectType::UnderlayDefinition(def));
    doc
}

fn extract_definition(doc: &CadDocument) -> UnderlayDefinition {
    doc.objects
        .values()
        .find_map(|o| match o {
            ObjectType::UnderlayDefinition(d) => Some(d.clone()),
            _ => None,
        })
        .expect("underlay definition missing after roundtrip")
}

fn assert_def(d: &UnderlayDefinition, kind: UnderlayType, label: &str) {
    assert_eq!(d.underlay_type, kind, "{label}: def underlay type");
    assert_eq!(
        d.file_path, "C:/refs/site-plan.pdf",
        "{label}: def file path"
    );
    assert_eq!(d.page_name, "Sheet1", "{label}: def page name");
    assert_eq!(d.handle, Handle::new(0x400), "{label}: def handle");
}

#[test]
fn underlay_definition_dwg_roundtrip() {
    for kind in [UnderlayType::Pdf, UnderlayType::Dwf, UnderlayType::Dgn] {
        let doc = definition_document(DxfVersion::AC1032, kind);
        let d = extract_definition(&dwg_roundtrip(&doc));
        assert_def(&d, kind, &format!("DWG {kind:?}"));
    }
}

#[test]
fn underlay_definition_dxf_roundtrip() {
    for kind in [UnderlayType::Pdf, UnderlayType::Dwf, UnderlayType::Dgn] {
        let doc = definition_document(DxfVersion::AC1032, kind);
        let d = extract_definition(&dxf_roundtrip(&doc));
        assert_def(&d, kind, &format!("DXF {kind:?}"));
    }
}

#[test]
fn underlay_flavours_preserved() {
    // The reference bitstream is identical for all three; only the resolved
    // DXF class name distinguishes them, so verify each flavour survives.
    for kind in [UnderlayType::Pdf, UnderlayType::Dwf, UnderlayType::Dgn] {
        let u = roundtrip_underlay(DxfVersion::AC1032, kind);
        assert_fields(&u, kind, &format!("{kind:?}"));
    }
}
