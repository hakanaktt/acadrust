//! A block record's reactors (an associative array's dependencies point back
//! from its anonymous blocks) can be set through the public API and survive
//! DXF and DWG.

use acadrust::tables::BlockRecord;
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};
use std::io::Cursor;

fn with_reactor() -> (CadDocument, acadrust::Handle) {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut record = BlockRecord::new("*U1");
    record.handle = doc.allocate_handle();
    let handle = record.handle;
    doc.block_records.add(record).unwrap();
    // Any live object will do as the reactor; the named object dictionary is one.
    let reactor = doc.header.named_objects_dict_handle;
    doc.add_object_reactor(handle, reactor);
    doc.add_object_reactor(handle, reactor);
    assert_eq!(doc.object_reactors(handle), &[reactor], "added once");
    (doc, reactor)
}

fn check(read: &CadDocument, reactor: acadrust::Handle, what: &str) {
    let record = read.block_records.get("*U1").expect("block record");
    assert_eq!(read.object_reactors(record.handle), &[reactor], "{what}");
}

#[test]
fn block_record_reactors_round_trip_dxf_and_dwg() {
    let (doc, reactor) = with_reactor();
    let dxf = DxfWriter::new(&doc).write_to_vec().unwrap();
    let text = String::from_utf8_lossy(&dxf);
    let record = text
        .find("\r\n*U1\r\n")
        .or_else(|| text.find("\n*U1\n"))
        .expect("record");
    let head = &text[..record];
    let start = head.rfind("BLOCK_RECORD").expect("record start");
    assert!(head[start..].contains("{ACAD_REACTORS"), "dxf text");
    let read = DxfReader::from_reader(Cursor::new(dxf))
        .unwrap()
        .read()
        .unwrap();
    check(&read, reactor, "dxf");
    let dwg = DwgWriter::write_to_vec(&doc).unwrap();
    let read = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
    check(&read, reactor, "dwg");
}

#[test]
fn objects_without_reactors_report_none() {
    let doc = CadDocument::with_version(DxfVersion::AC1032);
    assert!(doc
        .object_reactors(doc.header.named_objects_dict_handle)
        .is_empty());
}
