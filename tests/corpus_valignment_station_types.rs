//! The corpus sweep of the VALIGNMENT station-type label groups: the
//! three station-type classes (station, minor station, horizontal
//! geometry point) decode through the public readers once they are
//! registered (see `ENTITY_CLASSES`), and the curve family's decode does
//! not drift.
//!
//! Oracle: the Civil Tutorials Drawings corpus (D013) — every
//! `AECC_VALIGNMENT_*_LABEL_GROUP` record across its drawings. Runs when
//! `ACADRUST_CIVIL_TUTORIALS_DIR` is set.

use acadrust::classes::civil::CivilKind;
use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::DwgReader;

#[test]
fn corpus_valignment_station_types_decode() {
    let Some(dir) = std::env::var_os("ACADRUST_CIVIL_TUTORIALS_DIR") else {
        println!("ACADRUST_CIVIL_TUTORIALS_DIR not set — nothing to sweep");
        return;
    };
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .expect("corpus directory")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "dwg"))
        .collect();
    files.sort();
    let mut station = 0usize;
    let mut minor = 0usize;
    let mut hgeompoint = 0usize;
    let mut curves = 0usize;
    let mut failures = 0usize;
    for path in &files {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        let mut reader = match DwgReader::from_file(path) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("FAIL {name}: open: {e}");
                failures += 1;
                continue;
            }
        };
        let doc = match reader.read() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("FAIL {name}: parse: {e}");
                failures += 1;
                continue;
            }
        };
        for e in doc.entities() {
            let EntityType::Extended(x) = e else { continue };
            let ExtendedEntityData::Arch(a) = &x.data else { continue };
            if !a.dxf_name.starts_with("AECC_VALIGNMENT_") || !a.dxf_name.ends_with("_LABEL_GROUP") {
                continue;
            }
            let at = |what: &str| {
                panic!("{name} {} {:#x}: {what}", a.dxf_name, x.common.handle.value());
            };
            // Every VALIGNMENT label group is a registered Label and its
            // settings read (the zero-drift pin for the curve family, the
            // decode pin for the station family).
            if a.civil_kind() != Some(CivilKind::Label) {
                at("not a registered Label class");
            }
            let Some(_anchor) = a.label_dimension_anchor() else {
                panic!("{name} {} {:#x}: settings do not read", a.dxf_name, x.common.handle.value());
            };
            match a.dxf_name.as_str() {
                "AECC_VALIGNMENT_STATION_LABEL_GROUP" => {
                    let stations = a.label_stations();
                    assert!(!stations.is_empty(), "{name}: no stations");
                    let mut sorted = stations.clone();
                    sorted.sort_by(|x, y| x.partial_cmp(y).unwrap());
                    assert_eq!(sorted, stations, "{name}: stations do not ascend");
                    station += 1;
                }
                "AECC_VALIGNMENT_MINOR_STATION_LABEL_GROUP" => {
                    let stations = a.label_stations();
                    assert!(!stations.is_empty(), "{name}: no stations");
                    let mut sorted = stations.clone();
                    sorted.sort_by(|x, y| x.partial_cmp(y).unwrap());
                    assert_eq!(sorted, stations, "{name}: stations do not ascend");
                    minor += 1;
                }
                "AECC_VALIGNMENT_HAGEOMPOINT_LABEL_GROUP" => {
                    assert!(a.label_stations().is_empty(), "{name}: station key in a geometry point group");
                    hgeompoint += 1;
                }
                _ => curves += 1,
            }
        }
    }
    println!(
        "swept: station {station}, minor station {minor}, geometry point {hgeompoint}, curve family {curves}, failures {failures}"
    );
    assert_eq!(station, 12, "12 station groups expected");
    assert_eq!(minor, 5, "5 minor station groups expected");
    assert_eq!(hgeompoint, 43, "43 geometry point groups expected");
    assert_eq!(curves, 255, "the curve family must not drift (255 records)");
    assert_eq!(failures, 0, "every corpus file must open and parse");
}
