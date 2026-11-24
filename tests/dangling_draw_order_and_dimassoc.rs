//! A draw-order entry for an erased entity (or for none) and the DIMASSOC
//! of an erased dimension are left out on write: they point at nothing a
//! reader can resolve, and older readers crash on them.

use acadrust::entities::{EntityType, Line};
use acadrust::objects::{
    AssocDimensionAssociation, AssociativeData, AssociativeObject, ObjectType, SortEntitiesTable,
};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, Handle};
use std::io::Cursor;

fn dimassoc(doc: &mut CadDocument, dimension: Handle) -> Handle {
    let mut object = AssociativeObject::new("DIMASSOC", "AcDbDimAssoc");
    object.handle = doc.allocate_handle();
    object.data = AssociativeData::DimensionAssociation(AssocDimensionAssociation {
        associativity: 1,
        dimension,
        ..AssocDimensionAssociation::default()
    });
    let handle = object.handle;
    doc.objects.insert(handle, ObjectType::Associative(object));
    handle
}

fn drawing(version: DxfVersion) -> (CadDocument, Handle) {
    let mut doc = CadDocument::with_version(version);
    let line = doc
        .add_entity(EntityType::Line(Line::from_points(
            Vector3::ZERO,
            Vector3::new(10.0, 0.0, 0.0),
        )))
        .unwrap();
    let erased = Handle::new(0x7FFF0);
    let model = doc.block_records.get("*Model_Space").unwrap().handle;
    let mut table = SortEntitiesTable::for_block(model);
    table.handle = doc.allocate_handle();
    table.owner_handle = doc.ensure_extension_dictionary(model);
    table.add_entry(line, Handle::new(0x7FFF1));
    table.add_entry(Handle::NULL, Handle::new(0x7FFF2));
    table.add_entry(erased, Handle::new(0x7FFF3));
    doc.objects
        .insert(table.handle, ObjectType::SortEntitiesTable(table));
    // Any live entity stands in for the dimension of the kept association.
    dimassoc(&mut doc, line);
    dimassoc(&mut doc, erased);
    (doc, line)
}

fn check(read: &CadDocument, line: Handle, what: &str) {
    let entities: Vec<Handle> = read
        .objects
        .values()
        .filter_map(|object| match object {
            ObjectType::SortEntitiesTable(table) => Some(table),
            _ => None,
        })
        .flat_map(|table| table.entries().map(|entry| entry.entity_handle))
        .collect();
    assert_eq!(entities, [line], "{what}: draw-order entries");
    let dimensions: Vec<Handle> = read
        .objects
        .values()
        .filter_map(|object| match object {
            ObjectType::Associative(AssociativeObject {
                data: AssociativeData::DimensionAssociation(association),
                ..
            }) => Some(association.dimension),
            _ => None,
        })
        .collect();
    assert_eq!(dimensions, [line], "{what}: DIMASSOC objects");
}

#[test]
fn dangling_draw_order_entries_and_orphan_dimassocs_are_not_written() {
    for version in [DxfVersion::AC1024, DxfVersion::AC1032] {
        let (doc, line) = drawing(version);
        let dwg = DwgWriter::write_to_vec(&doc).unwrap();
        let read = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
        check(&read, line, &format!("dwg {version:?}"));
        let dxf = DxfWriter::new(&doc).write_to_vec().unwrap();
        let read = DxfReader::from_reader(Cursor::new(dxf))
            .unwrap()
            .read()
            .unwrap();
        check(&read, line, &format!("dxf {version:?}"));
    }
}
