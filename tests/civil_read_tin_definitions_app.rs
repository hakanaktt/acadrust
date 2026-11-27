//! The definitions of a TIN surface the application wrote: their texts
//! sit one slot later than this crate's records keep them. The
//! application stores the created-by name between the two empty strings
//! and the texts (2027 samples: "", "", "leightkadmin", then the
//! definitions' names); records this crate write start the texts right
//! after the empty strings. Which layout a record keeps follows from the
//! record's own text-stream length against the definitions' text need —
//! the application's stream runs two empty strings, the created-by name,
//! the definitions' texts, two texts per elevation range, the surface's
//! name, its description and one empty string.
//!
//! Started at the created-by slot, every definition was assigned the
//! previous definition's name (`Surface-3.dwg`'s boundary read
//! "XGND-Breakline" where Civil 3D names it "XGND-Boundary").

use acadrust::classes::civil::{TinBoundaryType, TinOperation};
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

fn surfaces(doc: &acadrust::CadDocument) -> Vec<acadrust::classes::arch::ArchEntityData> {
    doc.entities()
        .filter_map(|e| match e {
            EntityType::Extended(x) => match &x.data {
                ExtendedEntityData::Arch(d) if d.dxf_name == "AECC_TIN_SURFACE" => Some(d.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// `Surface-3.dwg`'s three definitions read their own names, in build
/// order, with their sources (the point group, the 33 breakline objects,
/// the boundary's one). The expected texts are the record's stored
/// strings (2027 sample: "", "", "leightkadmin", "Existing Ground
/// Points", "XGND-Breakline", "XGND-Boundary", "XGND", "Existing ground
/// surface") — each definition takes the one at its slot, not its
/// neighbor's. The surface's name stays "XGND".
#[test]
fn a_built_surfaces_definitions_read_their_own_texts() {
    let Some(doc) = app_drawing("Surface-3.dwg") else { return };
    let mut found = Vec::new();
    for data in surfaces(&doc) {
        let (defs, auto_rebuild) = data.tin_definitions().expect("the definitions read");
        assert!(!auto_rebuild);
        assert_eq!(data.surface_name().as_deref(), Some("XGND"));
        found = defs;
    }
    assert_eq!(found.len(), 3);
    assert_eq!(found[0].operation, TinOperation::PointGroup);
    assert_eq!(found[0].texts, vec!["Existing Ground Points".to_string()]);
    assert_eq!(found[0].sources.len(), 1);
    assert!(matches!(found[1].operation, TinOperation::Breaklines(_, _)));
    assert_eq!(found[1].texts, vec!["XGND-Breakline".to_string()]);
    assert_eq!(found[1].sources.len(), 33);
    match &found[2].operation {
        TinOperation::Boundary { boundary_type, non_destructive, lines } => {
            assert_eq!(boundary_type, &TinBoundaryType::Outer);
            assert_eq!(*non_destructive, false);
            assert_eq!(lines.objects, 1);
        }
        other => panic!("the third definition is {other:?}"),
    }
    assert_eq!(found[2].texts, vec!["XGND-Boundary".to_string()]);
    assert_eq!(found[2].sources.len(), 1);
}

/// The elevation-analysis surfaces whose definitions the grammar did not
/// read before their ranges were mapped (`Intro-2.dwg`, `Parcel-1E.dwg`,
/// `Parcel-3A.dwg`, eight ranges each) now read their definitions' own
/// texts: the two range texts per range sit between the definitions'
/// texts and the surface's name, and the stream length against the
/// definitions' need still places the created-by slot.
#[test]
fn the_elevation_surfaces_definitions_read_after_their_ranges() {
    for drawing in ["Intro-2.dwg", "Parcel-1E.dwg", "Parcel-3A.dwg"] {
        let Some(doc) = app_drawing(drawing) else { return };
        let surfaces = surfaces(&doc);
        assert_eq!(surfaces.len(), 1, "{drawing}");
        let (defs, _) = surfaces[0].tin_definitions().expect("the definitions read");
        assert_eq!(data_texts(&defs[0]), vec!["Existing Ground Points".to_string()], "{drawing}");
        assert_eq!(defs[0].operation, TinOperation::PointGroup);
        assert_eq!(surfaces[0].surface_name().as_deref(), Some("XGND"), "{drawing}");
    }
    let doc = app_drawing("Intro-2.dwg").unwrap();
    let defs = surfaces(&doc)[0].tin_definitions().unwrap().0;
    assert_eq!(defs.len(), 2);
    assert!(matches!(defs[1].operation, TinOperation::Boundary { .. }));
    assert_eq!(defs[1].texts, vec!["Outer Boundary".to_string()]);
}

/// A surface the application's corridor created keeps no created-by name
/// — its stream has the crate layout's length, and its one definition
/// takes the empty text slot there (`Corridor-5b.dwg`'s four corridor
/// surfaces; the surface's name is the corridor surface's, right after
/// the definitions' texts).
#[test]
fn a_corridor_created_surfaces_definition_keeps_an_empty_text() {
    let Some(doc) = app_drawing("Corridor-5b.dwg") else { return };
    let named: Vec<(Option<String>, Vec<Vec<String>>)> = surfaces(&doc)
        .iter()
        .filter_map(|d| {
            let (defs, _) = d.tin_definitions()?;
            Some((d.surface_name(), defs.iter().map(|x| x.texts.clone()).collect()))
        })
        .collect();
    assert!(named.contains(&(
        Some("Corridor - (1) Top".to_string()),
        vec![vec![String::new()]]
    )));
    assert!(named.iter().all(|(n, _)| n.is_some()), "no corridor surface lost its name");
}

/// A grading group's surface keeps the definition's empty text slot at
/// its own position, not the created-by name (`Grading-3A.dwg`,
/// `Grading-4.dwg`, `Grading-5.dwg` read "leightkadmin" there before).
#[test]
fn a_grading_groups_definition_keeps_its_empty_slot() {
    for drawing in ["Grading-3A.dwg", "Grading-4.dwg", "Grading-5.dwg"] {
        let Some(doc) = app_drawing(drawing) else { return };
        let read: Vec<Vec<Vec<String>>> = surfaces(&doc)
            .iter()
            .filter_map(|d| d.tin_definitions())
            .map(|(defs, _)| defs.iter().map(|x| x.texts.clone()).collect())
            .collect();
        assert!(
            read.contains(&vec![vec![String::new()]]),
            "{drawing}: a single grading-group definition with its empty text slot"
        );
    }
}

/// An import keeps its two texts — the file and the format name — and the
/// snapshot and boundary definitions after it keep theirs
/// (`Labels-Surface.dwg`: "", "", "yed", the composite file, "Composite",
/// "Boundary1", the surface's "Composite", ...).
#[test]
fn an_import_keeps_its_name_and_file_texts() {
    let Some(doc) = app_drawing("Labels-Surface.dwg") else { return };
    let surfaces = surfaces(&doc);
    assert_eq!(surfaces.len(), 1);
    let (defs, _) = surfaces[0].tin_definitions().expect("the definitions read");
    assert_eq!(defs.len(), 3);
    match &defs[0].operation {
        TinOperation::Import { .. } => {}
        other => panic!("the first definition is {other:?}"),
    }
    assert_eq!(
        defs[0].texts,
        vec!["..\\Composite_Surface.xml".to_string(), "Composite".to_string()]
    );
    // Kind 40 keeps the stored value (1 in drawings of older releases).
    assert!(matches!(defs[1].operation, TinOperation::Kind40(_)));
    assert!(defs[1].texts.is_empty());
    assert_eq!(defs[2].texts, vec!["Boundary1".to_string()]);
    assert_eq!(surfaces[0].surface_name().as_deref(), Some("Composite"));
}

fn data_texts(d: &acadrust::classes::civil::TinDefinition) -> Vec<String> {
    d.texts.clone()
}
