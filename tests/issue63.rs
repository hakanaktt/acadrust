//! Regression tests for issue #63.
//!
//! Two DXF-writer defects found when round-tripping a real-world DWG:
//!
//! 1. The named-object dictionary's `ACAD_FIELD` entry was written with a
//!    soft-pointer group code (350) instead of a hard-owner reference (360),
//!    pointing at an object downstream CAD applications treated as erasable —
//!    BricsCAD refused to save the resulting file with
//!    "Object was erased: (114D)". AutoCAD, ODA and BricsCAD all emit code
//!    360 for ACAD_FIELD, ACAD_LAYOUT and ACAD_PLOTSTYLENAME.
//!
//! 2. Every heavy 2D polyline was re-serialized as legacy
//!    POLYLINE/VERTEX/SEQEND instead of being down-saved to LWPOLYLINE for
//!    R2000+ output — a large structural divergence from the source drawing.

use std::io::Cursor;

use acadrust::entities::{EntityType, Polyline2D, PolylineFlags, Vertex2D};
use acadrust::objects::{FieldList, ObjectType};
use acadrust::types::{DxfVersion, Handle, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};

fn write_text(doc: &CadDocument) -> String {
    String::from_utf8(
        DxfWriter::new(doc)
            .write_to_vec()
            .expect("DXF write failed"),
    )
    .unwrap()
}

fn read_bytes(bytes: Vec<u8>) -> CadDocument {
    DxfReader::from_reader(Cursor::new(bytes))
        .expect("DXF reader init failed")
        .read()
        .expect("DXF read failed")
}

fn has_record(text: &str, record_type: &str) -> bool {
    text.contains(&format!("  0\r\n{}\r\n", record_type))
}

/// Find the `3 / KEY / <code> / <handle>` quadruple for a dictionary entry
/// and return (group_code, target_handle).
fn dictionary_entry(text: &str, key: &str) -> (u16, u64) {
    let lines: Vec<&str> = text.split("\r\n").collect();
    for i in 1..lines.len() {
        if lines[i] == key && lines[i - 1].trim() == "3" {
            let code: u16 = lines[i + 1].trim().parse().expect("group code");
            let handle = u64::from_str_radix(lines[i + 2].trim(), 16).expect("handle");
            return (code, handle);
        }
    }
    panic!("dictionary entry {:?} not found", key);
}

// ── Issue 1: ACAD_FIELD hard-owner pointer ──────────────────────────────

#[test]
fn acad_field_nod_entry_uses_hard_owner_pointer() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let nod_handle = doc.header.named_objects_dict_handle;
    assert!(
        !nod_handle.is_null(),
        "default document must have a root NOD"
    );

    let field_list_handle = Handle::new(0x114D);
    let field_list = FieldList {
        handle: field_list_handle,
        owner: nod_handle,
        ..Default::default()
    };
    doc.objects
        .insert(field_list_handle, ObjectType::FieldList(field_list));

    if let Some(ObjectType::Dictionary(nod)) = doc.objects.get_mut(&nod_handle) {
        nod.add_entry("ACAD_FIELD", field_list_handle);
    } else {
        panic!("root NOD missing from objects");
    }

    let output = write_text(&doc);

    // The ACAD_FIELD entry must be a hard-owner reference (360), and it must
    // point at the real FIELDLIST object rather than a placeholder.
    let (code, target) = dictionary_entry(&output, "ACAD_FIELD");
    assert_eq!(code, 360, "ACAD_FIELD must use hard-owner code 360");
    assert_eq!(target, 0x114D, "ACAD_FIELD must point at the FIELDLIST");
    assert!(
        has_record(&output, "FIELDLIST"),
        "FIELDLIST object must be written"
    );
}

#[test]
fn acad_layout_and_plotstylename_nod_entries_match_bricscad() {
    // Updated per issue #51: BricsCAD's own DXF export writes ALL NOD
    // entries (including ACAD_LAYOUT and ACAD_PLOTSTYLENAME) as soft
    // pointers (350); hard-owning the plot style dictionary made BricsCAD's
    // audit reject the layers' PlotStyleName references. Only ACAD_FIELD
    // stays a hard owner (360) - covered by its dedicated test below.
    let doc = CadDocument::with_version(DxfVersion::AC1032);
    let output = write_text(&doc);

    for key in ["ACAD_LAYOUT", "ACAD_PLOTSTYLENAME"] {
        let (code, _) = dictionary_entry(&output, key);
        assert_eq!(code, 350, "{} must use soft-pointer code 350", key);
    }
    // Soft-pointer entries must stay soft.
    let (group_code, _) = dictionary_entry(&output, "ACAD_GROUP");
    assert_eq!(group_code, 350, "ACAD_GROUP stays a soft pointer");
}

// ── Issue 2: LWPOLYLINE down-save ───────────────────────────────────────

fn plain_polyline() -> Polyline2D {
    let mut pl = Polyline2D::new();
    pl.start_width = 0.5;
    pl.elevation = 2.0;
    pl.close();

    let mut v0 = Vertex2D::new(Vector3::new(0.0, 0.0, 2.0));
    v0.bulge = 0.5;
    pl.add_vertex(v0);

    let mut v1 = Vertex2D::new(Vector3::new(10.0, 0.0, 2.0));
    v1.start_width = 1.0;
    v1.end_width = 0.25;
    pl.add_vertex(v1);

    pl.add_vertex(Vertex2D::new(Vector3::new(10.0, 10.0, 2.0)));
    pl
}

#[test]
fn plain_polyline2d_downsaves_to_lwpolyline_r2000_plus() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.add_entity(EntityType::Polyline2D(plain_polyline()))
        .unwrap();

    let output = write_text(&doc);
    assert!(
        has_record(&output, "LWPOLYLINE"),
        "expected LWPOLYLINE output"
    );
    assert!(
        !has_record(&output, "POLYLINE"),
        "legacy POLYLINE must not be written"
    );
    assert!(
        !has_record(&output, "VERTEX"),
        "VERTEX records must not be written"
    );
    assert!(
        !has_record(&output, "SEQEND"),
        "SEQEND records must not be written"
    );

    // Round-trip fidelity: geometry, bulge, widths and elevation survive the
    // down-save with the defaults baked into each vertex.
    let rt = read_bytes(DxfWriter::new(&doc).write_to_vec().unwrap());
    assert_eq!(rt.entity_count(), 1);
    for entity in rt.entities() {
        match entity {
            EntityType::LwPolyline(lw) => {
                assert!(lw.is_closed);
                assert_eq!(lw.vertices.len(), 3);
                assert_eq!(lw.elevation, 2.0);
                assert_eq!(lw.vertices[0].location.x, 0.0);
                assert_eq!(lw.vertices[0].location.y, 0.0);
                assert_eq!(lw.vertices[0].bulge, 0.5);
                assert_eq!(lw.vertices[0].start_width, 0.5, "default width baked in");
                assert_eq!(
                    lw.vertices[1].start_width, 1.0,
                    "per-vertex width preserved"
                );
                assert_eq!(lw.vertices[1].end_width, 0.25);
                assert_eq!(lw.vertices[2].start_width, 0.5, "default width baked in");
                assert_eq!(lw.vertices[2].end_width, 0.0);
            }
            other => panic!("expected LwPolyline after down-save, got {:?}", other),
        }
    }
}

#[test]
fn spline_fit_polyline2d_keeps_legacy_form() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut pl = plain_polyline();
    pl.flags = pl.flags | PolylineFlags::SPLINE_FIT;
    doc.add_entity(EntityType::Polyline2D(pl)).unwrap();

    let output = write_text(&doc);
    assert!(
        has_record(&output, "POLYLINE"),
        "fitted polylines keep the legacy form"
    );
    assert!(has_record(&output, "VERTEX"));
    assert!(has_record(&output, "SEQEND"));
    assert!(!has_record(&output, "LWPOLYLINE"));
}

#[test]
fn polyface_mesh_flags_keep_legacy_form() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut pl = plain_polyline();
    pl.flags = pl.flags | PolylineFlags::POLYFACE_MESH;
    doc.add_entity(EntityType::Polyline2D(pl)).unwrap();

    let output = write_text(&doc);
    assert!(has_record(&output, "POLYLINE"));
    assert!(!has_record(&output, "LWPOLYLINE"));
}

#[test]
fn r14_output_keeps_legacy_polyline_form() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1014);
    doc.add_entity(EntityType::Polyline2D(plain_polyline()))
        .unwrap();

    let output = write_text(&doc);
    assert!(
        has_record(&output, "POLYLINE"),
        "pre-R2000 output keeps legacy POLYLINE"
    );
    assert!(has_record(&output, "VERTEX"));
    assert!(has_record(&output, "SEQEND"));
    assert!(!has_record(&output, "LWPOLYLINE"));
}

#[test]
fn empty_polyline2d_still_downsaves_without_children() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.add_entity(EntityType::Polyline2D(Polyline2D::new()))
        .unwrap();

    let output = write_text(&doc);
    assert!(has_record(&output, "LWPOLYLINE"));
    assert!(!has_record(&output, "SEQEND"));
}
