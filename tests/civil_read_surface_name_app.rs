//! The name of a TIN surface the application wrote: a created-by name
//! stored before the definitions' texts (the empty shell of
//! `Surface-2.dwg` read "leightkadmin" — the created-by name — where
//! Civil 3D names the surface "EG").
//!
//! The application's text stream runs: two empty strings, the created-by
//! name, the definitions' names, the surface's name, its description.
//! This crate's records keep the name right after the definitions' texts.

use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::DwgReader;

/// The application's tutorial drawing, from `ACADRUST_CIVIL_TUTORIALS_DIR`.
fn app_drawing(name: &str) -> Option<acadrust::CadDocument> {
    let dir = std::env::var_os("ACADRUST_CIVIL_TUTORIALS_DIR")?;
    let path = std::path::Path::new(&dir).join(name);
    if !path.exists() {
        return None;
    }
    Some(DwgReader::from_file(&path).unwrap().read().unwrap())
}

fn surface_names(doc: &acadrust::CadDocument) -> Vec<Option<String>> {
    let mut out = Vec::new();
    for e in doc.entities() {
        if let EntityType::Extended(x) = e {
            if let ExtendedEntityData::Arch(a) = &x.data {
                if a.dxf_name == "AECC_TIN_SURFACE" {
                    out.push(a.surface_name());
                }
            }
        }
    }
    out
}

/// The empty shells of the tutorials' two surface-only drawings read "EG"
/// (2027 samples: one surface each, four bounding corners, no triangles).
/// Oracle: Civil 3D 2027 reads both as `TinSurface.Name` "EG"
/// (execute_civil_code, recorded captures cap.p2.oracle.surf.surface_2 and
/// cap.d17.o.surface.surface_1a; re-confirmed 2026-11-25).
#[test]
fn an_app_written_shells_name_is_not_its_created_by_name() {
    for drawing in ["Surface-1A.dwg", "Surface-2.dwg"] {
        let Some(doc) = app_drawing(drawing) else { continue };
        assert_eq!(surface_names(&doc), vec![Some("EG".to_string())], "{drawing}");
    }
}

/// A built surface's name sits after its definitions' names: the point
/// group, breaklines and boundary of `Surface-3.dwg` leave "XGND" after
/// them (2027 samples: "", "", "leightkadmin", "Existing Ground Points",
/// "XGND-Breakline", "XGND-Boundary", "XGND", ...).
#[test]
fn an_app_written_surfaces_name_follows_its_definitions_texts() {
    for (drawing, expected) in [("Surface-3.dwg", "XGND"), ("Surface-7.dwg", "XGND")] {
        let Some(doc) = app_drawing(drawing) else { continue };
        assert_eq!(surface_names(&doc), vec![Some(expected.to_string())], "{drawing}");
    }
}

/// A surface record whose definitions block this grammar does not read
/// (the multi-breakline preamble of `Corridor-1a.dwg`) names `None` — its
/// text boundary is unknown, and the created-by name or a definition's
/// name must not be read for it.
#[test]
fn a_surface_with_unreadable_definitions_names_none() {
    let Some(doc) = app_drawing("Corridor-1a.dwg") else { return };
    assert_eq!(surface_names(&doc), vec![None::<String>]);
}
