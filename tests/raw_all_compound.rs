use acadrust::entities::{AttributeEntity, EntityType, Insert, Polyline2D, Polyline3D, Vertex2D};
use acadrust::io::dwg::dwg_stream_readers::object_reader::DwgObjectReader;
use acadrust::io::dwg::dwg_stream_writers::object_writer::DwgObjectWriter;
use acadrust::tables::BlockRecord;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion, Handle, Vector3};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::Cursor;

struct RawEnvironment {
    values: [Option<OsString>; 2],
}

impl RawEnvironment {
    fn new() -> Self {
        let values = [
            std::env::var_os("ACADRUST_RAW_ALL"),
            std::env::var_os("ACADRUST_RAW_EXCLUDE"),
        ];
        std::env::remove_var("ACADRUST_RAW_ALL");
        std::env::remove_var("ACADRUST_RAW_EXCLUDE");
        Self { values }
    }
}

impl Drop for RawEnvironment {
    fn drop(&mut self) {
        for (name, value) in ["ACADRUST_RAW_ALL", "ACADRUST_RAW_EXCLUDE"]
            .iter()
            .zip(&self.values)
        {
            if let Some(value) = value {
                std::env::set_var(name, value);
            } else {
                std::env::remove_var(name);
            }
        }
    }
}

fn source_document(version: DxfVersion) -> (CadDocument, [Handle; 3]) {
    let mut document = CadDocument::with_version(version);
    let polyline3d = document
        .add_entity(EntityType::Polyline3D(Polyline3D::from_points(vec![
            Vector3::new(1.0, 2.0, 3.0),
            Vector3::new(4.0, 5.0, 6.0),
            Vector3::new(7.0, 8.0, 9.0),
        ])))
        .unwrap();

    let mut polyline = Polyline2D::new();
    polyline.add_vertex(Vertex2D::new(Vector3::new(2.0, 3.0, 0.0)));
    polyline.add_vertex(
        Vertex2D::new(Vector3::new(5.0, 7.0, 0.0))
            .with_bulge(0.5)
            .with_width(1.0, 2.0),
    );
    polyline.add_vertex(Vertex2D::new(Vector3::new(11.0, 13.0, 0.0)));
    let polyline2d = document
        .add_entity(EntityType::Polyline2D(polyline))
        .unwrap();

    let mut block = BlockRecord::new("AttributedBlock");
    block.handle = document.allocate_handle();
    document.block_records.add(block).unwrap();
    let mut insert = Insert::new("AttributedBlock", Vector3::new(20.0, 30.0, 0.0));
    insert
        .attributes
        .push(AttributeEntity::simple("FIRST", "first value"));
    insert
        .attributes
        .push(AttributeEntity::simple("SECOND", "second value"));
    let insert = document.add_entity(EntityType::Insert(insert)).unwrap();
    (document, [polyline3d, polyline2d, insert])
}

fn read(bytes: Vec<u8>) -> CadDocument {
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn assert_geometry(expected: &CadDocument, actual: &CadDocument, handles: [Handle; 3]) {
    for handle in handles {
        match (
            expected.get_entity(handle).unwrap(),
            actual.get_entity(handle).unwrap(),
        ) {
            (EntityType::Polyline3D(expected), EntityType::Polyline3D(actual)) => {
                let points = |polyline: &Polyline3D| {
                    polyline
                        .vertices
                        .iter()
                        .map(|vertex| vertex.position)
                        .collect::<Vec<_>>()
                };
                assert_eq!(points(actual), points(expected));
            }
            (EntityType::Polyline2D(expected), EntityType::Polyline2D(actual)) => {
                assert_eq!(actual.vertices, expected.vertices);
            }
            (EntityType::Insert(expected), EntityType::Insert(actual)) => {
                assert_eq!(actual.insert_point, expected.insert_point);
                let attributes = |insert: &Insert| {
                    insert
                        .attributes
                        .iter()
                        .map(|attribute| (attribute.tag.clone(), attribute.value.clone()))
                        .collect::<Vec<_>>()
                };
                assert_eq!(attributes(actual), attributes(expected));
            }
            (expected, actual) => panic!("entity changed type: {expected:?} -> {actual:?}"),
        }
    }
}

fn child_inventory(document: &CadDocument) -> BTreeMap<(u64, i16), BTreeSet<u64>> {
    let (bytes, handles, _, _) = DwgObjectWriter::new(document).unwrap().write();
    let reader = DwgObjectReader::new(
        bytes,
        document.version,
        handles
            .into_iter()
            .map(|(handle, offset)| (handle, i64::from(offset)))
            .collect(),
    )
    .unwrap();
    let mut inventory = BTreeMap::<_, BTreeSet<_>>::new();
    for handle in reader.handles() {
        let offset = reader.offset_for(handle).unwrap() as usize;
        let (kind, mut record) = reader.read_record_at(offset).unwrap();
        if matches!(kind, 2 | 6 | 10 | 11) {
            let common = reader.read_common_entity_data(&mut record, kind);
            assert_eq!(common.common.handle, handle);
            inventory
                .entry((common.owner_handle, kind))
                .or_default()
                .insert(handle);
        }
    }
    inventory
}

// All environment-dependent cases share one test in this integration-test
// process, isolated from the normal writer tests.
#[test]
fn compound_exclusions_preserve_geometry_and_do_not_leave_orphaned_children() {
    let _environment = RawEnvironment::new();
    for version in [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1021,
        DxfVersion::AC1032,
    ] {
        std::env::remove_var("ACADRUST_RAW_ALL");
        std::env::remove_var("ACADRUST_RAW_EXCLUDE");
        let (source, handles) = source_document(version);
        let source_bytes = DwgWriter::write_to_vec(&source).unwrap();
        std::env::set_var("ACADRUST_RAW_ALL", "1");
        let document = read(source_bytes);
        assert_geometry(&source, &document, handles);
        let [polyline3d, polyline2d, insert] = handles.map(|handle| handle.value());
        let expected_counts = BTreeMap::from([
            ((polyline3d, 11), 3),
            ((polyline3d, 6), 1),
            ((polyline2d, 10), 3),
            ((polyline2d, 6), 1),
            ((insert, 2), 2),
            ((insert, 6), 1),
        ]);

        for exclude in ["", "11", "16", "10", "15", "2", "7", "6", "ENTITIES"] {
            std::env::set_var("ACADRUST_RAW_EXCLUDE", exclude);
            let counts: BTreeMap<_, _> = child_inventory(&document)
                .into_iter()
                .map(|(owner_and_type, children)| (owner_and_type, children.len()))
                .collect();
            assert_eq!(
                counts, expected_counts,
                "missing or orphaned children for {version:?} with exclusion {exclude:?}"
            );
            let actual = read(DwgWriter::write_to_vec(&document).unwrap());
            assert_geometry(&document, &actual, handles);
        }
    }
}
