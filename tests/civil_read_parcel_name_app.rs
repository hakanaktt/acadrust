//! The name of a parcel the application wrote. Records that store none
//! (2027 sample `Parcel-2C.dwg`: names decode empty) keep their style's
//! name template instead, which the parcel's number and its style's name
//! evaluate ("ROAD (LOCAL): 100").
//!
//! A parcel references its style as its fourth body handle; the style's
//! record keeps its name, then the name template
//! (`<[167804933(...)]> : <[16809985]>` — style name, number).

use acadrust::classes::civil::parcel_name;
use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::types::Handle;
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

fn parcel_names(doc: &acadrust::CadDocument) -> Vec<(i32, Option<String>)> {
    let mut out = Vec::new();
    for e in doc.entities() {
        if let EntityType::Extended(x) = e {
            if let ExtendedEntityData::Arch(a) = &x.data {
                if a.dxf_name == "AECC_PARCEL" && a.parcel_site().is_some() {
                    let number = a.parcel_face().map(|f| f.0).unwrap_or(-1);
                    out.push((number, parcel_name(doc, Handle::new(x.common.handle.value()))));
                }
            }
        }
    }
    out
}

/// Every parcel of `Parcel-2C.dwg` reads the name Civil 3D shows. Oracle:
/// Civil 3D 2027, `Parcel.Name` through the site's `GetParcelIds`
/// (execute_civil_code, recorded captures cap.p4.oracle.parcel2 and
/// cap.d010.parcel2c.oracle, 2026-11-25): "ROAD (LOCAL): 100" and
/// "SINGLE-FAMILY: <number>" for all eleven parcels — the style's upper
/// case name ("Road (Local)" with the template's upper-case option), then
/// the number.
#[test]
fn an_app_written_parcels_name_comes_from_its_styles_name_template() {
    let Some(doc) = app_drawing("Parcel-2C.dwg") else { return };
    let names = parcel_names(&doc);
    assert_eq!(names.len(), 11);
    for (number, name) in names {
        let name = name.unwrap_or_else(|| panic!("parcel #{number} names None"));
        let expected = if number == 100 { format!("ROAD (LOCAL): {number}") } else { format!("SINGLE-FAMILY: {number}") };
        assert_eq!(name, expected, "parcel #{number}");
    }
}

/// A record that stores its name reads it directly (the application's
/// older form, `Labels-Parcels.dwg`: the evaluated name sits in the
/// record's texts — "Road : 9", "Property : 1" — and the record keeps no
/// number for the template to evaluate).
#[test]
fn a_parcels_stored_name_reads_directly() {
    let Some(doc) = app_drawing("Labels-Parcels.dwg") else { return };
    let names = parcel_names(&doc);
    assert_eq!(names.len(), 54);
    let named: Vec<Option<&str>> = names.iter().map(|(_, name)| name.as_deref()).collect();
    assert!(named.iter().all(|name| name.is_some()));
    assert!(named.contains(&Some("Road : 9")));
    assert!(named.contains(&Some("Property : 1")));
}

/// A style without a name of its own (this crate's default parcel style)
/// evaluates to no name: the application keeps no name for it in the
/// record either.
#[test]
fn a_styleless_default_parcels_name_reads_none() {
    let mut doc = acadrust::CadDocument::new();
    acadrust::classes::civil::ensure_civil(&mut doc).unwrap();
    let site = acadrust::classes::civil::add_site(&mut doc, "S", "").unwrap();
    let parcel = acadrust::classes::civil::add_parcel(
        &mut doc,
        site,
        &[acadrust::types::Vector2::new(0.0, 0.0), acadrust::types::Vector2::new(100.0, 0.0), acadrust::types::Vector2::new(100.0, 100.0)],
        None,
    )
    .unwrap();
    assert_eq!(parcel_name(&doc, parcel), None);
}
