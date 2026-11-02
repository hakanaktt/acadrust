//! A TABLE block cell read from an R2010+ DWG is a block cell again, so a
//! later DXF save still writes its block (`340`).

use std::io::Cursor;

use acadrust::entities::{CellType, Circle, EntityType, Table, TableCell};
use acadrust::tables::BlockRecord;
use acadrust::types::{DxfVersion, Handle, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

fn drawing(version: DxfVersion) -> (CadDocument, Handle, Handle) {
    let mut doc = CadDocument::with_version(version);
    let mut record = BlockRecord::new("LABCELL");
    record.handle = doc.allocate_handle();
    record.block_entity_handle = doc.allocate_handle();
    record.block_end_handle = doc.allocate_handle();
    let block = record.handle;
    let circle = Circle::from_center_radius(Vector3::ZERO, 1.0);
    record
        .entity_handles
        .push(doc.add_entity(EntityType::Circle(circle)).unwrap());
    doc.block_records.add(record).unwrap();
    let mut table = Table::new(Vector3::ZERO, 2, 2);
    table.rows[0].cells[0] = TableCell::text("name");
    table.rows[0].cells[1] = TableCell::block(block);
    let handle = doc.add_entity(EntityType::Table(Box::new(table))).unwrap();
    (doc, handle, block)
}

fn block_cell(doc: &CadDocument, handle: Handle) -> (CellType, Option<Handle>) {
    let Some(EntityType::Table(table)) = doc.get_entity(handle) else {
        panic!("table lost");
    };
    let cell = &table.rows[0].cells[1];
    (
        cell.cell_type,
        cell.contents.first().and_then(|c| c.block_handle),
    )
}

#[test]
fn r2010_plus_dwg_block_cell_keeps_its_type_and_block_through_dxf() {
    for version in [DxfVersion::AC1024, DxfVersion::AC1027, DxfVersion::AC1032] {
        let (doc, handle, block) = drawing(version);
        let dwg = DwgWriter::write_to_vec(&doc).unwrap();
        let from_dwg = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
        assert_eq!(
            block_cell(&from_dwg, handle),
            (CellType::Block, Some(block)),
            "{version:?} DWG"
        );
        let dxf = DxfWriter::new(&from_dwg).write_to_vec().unwrap();
        let from_dxf = DxfReader::from_reader(Cursor::new(dxf))
            .unwrap()
            .read()
            .unwrap();
        assert_eq!(
            block_cell(&from_dxf, handle),
            (CellType::Block, Some(block)),
            "{version:?} DWG then DXF"
        );
    }
}
