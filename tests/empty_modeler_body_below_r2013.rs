//! Below R2013 a modeler body with no data (3DSOLID, REGION, BODY, SURFACE)
//! is written as an ACIS document with a header and no records, the way the
//! reference application saves it.
//!
//! Below R2013 the body is stored in the entity record. The writer used to
//! emit only the leading `acis_empty` flag for an empty body, and ODA refuses
//! such a DWG ("Empty ACIS not allowed") in R2000, R2004 and R2010. R2013+
//! stores the body in the AcDs section and keeps its own empty-body layout
//! (`write_acis_empty`), which ODA accepts.
//!
//! The drawings are synthetic: entities built here with no SAT or SAB.

use std::io::Cursor;

use acadrust::entities::acis::{SabReader, SatDocument};
use acadrust::entities::solid3d::AcisData;
use acadrust::entities::surface::{Surface, SurfaceKind};
use acadrust::entities::{Body, EntityType, Region, Solid3D};
use acadrust::io::dwg::dwg_stream_readers::object_reader::DwgObjectReader;
use acadrust::io::dwg::dwg_stream_writers::object_writer::DwgObjectWriter;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion, Handle};

/// Versions that store the modeler body in the entity record.
const INLINE: [DxfVersion; 5] = [
    DxfVersion::AC1014,
    DxfVersion::AC1015,
    DxfVersion::AC1018,
    DxfVersion::AC1021,
    DxfVersion::AC1024,
];

/// A drawing of `version` with an empty 3DSOLID, REGION and BODY, and from
/// R2007 an empty plane surface.
fn document(version: DxfVersion) -> (CadDocument, Vec<Handle>) {
    let mut document = CadDocument::with_version(version);
    let mut entities = vec![
        EntityType::Solid3D(Solid3D::new()),
        EntityType::Region(Region::new()),
        EntityType::Body(Body::new()),
    ];
    if version >= DxfVersion::AC1021 {
        entities.push(EntityType::Surface(Box::new(Surface::new(
            SurfaceKind::Plane,
        ))));
    }
    let handles = entities
        .into_iter()
        .map(|entity| document.add_entity(entity).unwrap())
        .collect();
    (document, handles)
}

fn acis(entity: &EntityType) -> &AcisData {
    match entity {
        EntityType::Solid3D(entity) => &entity.acis_data,
        EntityType::Region(entity) => &entity.acis_data,
        EntityType::Body(entity) => &entity.acis_data,
        EntityType::Surface(entity) => &entity.acis_data,
        _ => panic!("not a modeler entity"),
    }
}

/// The modeler document of a body read back, SAT or SAB.
fn modeler_document(acis: &AcisData) -> SatDocument {
    if acis.is_binary {
        SabReader::read(&acis.sab_data).expect("a SAB document")
    } else {
        SatDocument::parse(&acis.sat_data).expect("a SAT document")
    }
}

/// The record of each empty body starts with `acis_empty = 0` followed by
/// an ACIS document: SAT (version 1) up to R2000, SAB (version 2) from R2004.
#[test]
fn empty_body_record_carries_an_acis_document() {
    for version in INLINE {
        let (document, handles) = document(version);
        let (bytes, records, _, _) = DwgObjectWriter::new(&document).unwrap().write();
        let offsets = records
            .into_iter()
            .map(|(h, offset)| (h, i64::from(offset)))
            .collect();
        let objects = DwgObjectReader::new(bytes, version, offsets).unwrap();
        for handle in handles {
            let offset = objects.offset_for(handle.value()).expect("record written");
            let (type_code, mut reader) = objects.read_record_at(offset as usize).unwrap();
            objects.read_common_entity_data(&mut reader, type_code);
            assert!(
                !reader.read_bit(),
                "{version:?} {handle:?}: acis_empty flag instead of a document"
            );
            reader.read_bit(); // unknown
            let expected = if version >= DxfVersion::AC1018 { 2 } else { 1 };
            assert_eq!(reader.read_bit_short(), expected, "{version:?} {handle:?}");
            reader.check_error().unwrap();
        }
    }
}

/// Read back, each body is an ACIS document without records.
#[test]
fn empty_body_reads_back_as_an_empty_acis_document() {
    for version in INLINE {
        let (document, handles) = document(version);
        let bytes = DwgWriter::write_to_vec(&document).unwrap();
        let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        for handle in handles {
            let entity = read
                .get_entity(handle)
                .unwrap_or_else(|| panic!("{version:?} {handle:?}: entity lost"));
            let acis = acis(entity);
            assert!(
                acis.has_data(),
                "{version:?} {handle:?}: written as the acis_empty flag"
            );
            assert!(
                modeler_document(acis).records.is_empty(),
                "{version:?} {handle:?}"
            );
        }
    }
}

/// R2013+ keeps the AcDs layout: an empty body has no AcDs record and reads
/// back empty.
#[test]
fn r2013_empty_body_keeps_the_acds_layout() {
    for version in [DxfVersion::AC1027, DxfVersion::AC1032] {
        let (document, handles) = document(version);
        let bytes = DwgWriter::write_to_vec(&document).unwrap();
        let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        for handle in handles {
            let entity = read
                .get_entity(handle)
                .unwrap_or_else(|| panic!("{version:?} {handle:?}: entity lost"));
            assert!(!acis(entity).has_data(), "{version:?} {handle:?}");
        }
    }
}
