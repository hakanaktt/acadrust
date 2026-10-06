use acadrust::entities::acis::primitives::build_box;
use acadrust::entities::solid3d::{AcisData, Body, Region, Solid3D};
use acadrust::entities::surface::{Surface, SurfaceKind};
use acadrust::entities::EntityType;
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter};
use std::ffi::OsString;
use std::io::Cursor;

struct RawEnvironment {
    raw_all: Option<OsString>,
    raw_exclude: Option<OsString>,
}

impl RawEnvironment {
    fn new() -> Self {
        let saved = Self {
            raw_all: std::env::var_os("ACADRUST_RAW_ALL"),
            raw_exclude: std::env::var_os("ACADRUST_RAW_EXCLUDE"),
        };
        std::env::remove_var("ACADRUST_RAW_ALL");
        std::env::remove_var("ACADRUST_RAW_EXCLUDE");
        saved
    }
}

impl Drop for RawEnvironment {
    fn drop(&mut self) {
        for (name, value) in [
            ("ACADRUST_RAW_ALL", &self.raw_all),
            ("ACADRUST_RAW_EXCLUDE", &self.raw_exclude),
        ] {
            if let Some(value) = value {
                std::env::set_var(name, value);
            } else {
                std::env::remove_var(name);
            }
        }
    }
}

fn source_document(version: DxfVersion) -> CadDocument {
    let mut document = CadDocument::with_version(version);
    let sat = build_box([0.0, 0.0, 0.0], 2.0, 3.0, 4.0).to_sat_string();
    document
        .add_entity(EntityType::Solid3D(Solid3D::from_sat(&sat)))
        .unwrap();
    let sat = build_box([10.0, 0.0, 0.0], 3.0, 4.0, 5.0).to_sat_string();
    document
        .add_entity(EntityType::Region(Region::from_sat(&sat)))
        .unwrap();
    let sat = build_box([20.0, 0.0, 0.0], 4.0, 5.0, 6.0).to_sat_string();
    document
        .add_entity(EntityType::Body(Body::from_sat(&sat)))
        .unwrap();
    let sat = build_box([30.0, 0.0, 0.0], 5.0, 6.0, 7.0).to_sat_string();
    let mut surface = Surface::new(SurfaceKind::Plane);
    surface.acis_data = AcisData::from_sat(&sat);
    document.add_entity(EntityType::Surface(Box::new(surface))).unwrap();
    document
}

fn read(bytes: Vec<u8>) -> CadDocument {
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn geometry(document: &CadDocument) -> Vec<(u64, Vec<u8>)> {
    let mut entries: Vec<_> = document
        .entities()
        .filter_map(|entity| {
            let acis = match entity {
                EntityType::Solid3D(entity) => &entity.acis_data,
                EntityType::Region(entity) => &entity.acis_data,
                EntityType::Body(entity) => &entity.acis_data,
                EntityType::Surface(entity) => &entity.acis_data,
                _ => return None,
            };
            Some((entity.common().handle.value(), acis.sab_data.clone()))
        })
        .collect();
    entries.sort_unstable_by_key(|(handle, _)| *handle);
    entries
}

// Keep environment-dependent cases in one integration test so they run in an
// isolated process without affecting tests that use the normal writer.
#[test]
fn raw_records_preserve_external_geometry_with_mixed_exclusions() {
    let _environment = RawEnvironment::new();
    for version in [DxfVersion::AC1027, DxfVersion::AC1032] {
        std::env::remove_var("ACADRUST_RAW_ALL");
        std::env::remove_var("ACADRUST_RAW_EXCLUDE");
        let source = DwgWriter::write_to_vec(&source_document(version)).unwrap();
        std::env::set_var("ACADRUST_RAW_ALL", "1");
        let document = read(source);
        let expected = geometry(&document);
        assert_eq!(expected.len(), 4);
        assert!(expected.iter().all(|(_, bytes)| !bytes.is_empty()));

        for exclude in ["", "38", "PLANESURFACE", "ENTITIES"] {
            std::env::set_var("ACADRUST_RAW_EXCLUDE", exclude);
            let actual = read(DwgWriter::write_to_vec(&document).unwrap());
            assert_eq!(
                geometry(&actual),
                expected,
                "SAB changed for {version:?} with exclusion {exclude:?}"
            );
        }

        std::env::remove_var("ACADRUST_RAW_EXCLUDE");
        let mut converted = document.clone();
        converted.version = DxfVersion::AC1024;
        let actual = read(DwgWriter::write_to_vec(&converted).unwrap());
        assert_eq!(geometry(&actual), expected, "version conversion lost SAB");

        std::env::remove_var("ACADRUST_RAW_ALL");
        let mut edited = document;
        let solid = edited
            .entities_mut()
            .find_map(|entity| match entity {
                EntityType::Solid3D(solid) => Some(solid),
                _ => None,
            })
            .unwrap();
        solid.set_sat_document(&build_box([0.0, 0.0, 0.0], 9.0, 8.0, 7.0));
        let edited_handle = solid.common.handle.value();
        let actual = geometry(&read(DwgWriter::write_to_vec(&edited).unwrap()));
        assert_eq!(actual.len(), expected.len());
        for ((handle, bytes), (old_handle, old_bytes)) in actual.iter().zip(&expected) {
            assert_eq!(handle, old_handle);
            if *handle == edited_handle {
                assert!(!bytes.is_empty());
                assert_ne!(bytes, old_bytes, "edited geometry reused stale AcDs data");
            } else {
                assert_eq!(bytes, old_bytes);
            }
        }
    }
}
