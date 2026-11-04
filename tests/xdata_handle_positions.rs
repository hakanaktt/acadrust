//! An XDATA handle that names no written record keeps its place in DXF: it is written as 0, so
//! the values after it keep their positions (XDATA is read by position).

use std::io::Cursor;

use acadrust::entities::{EntityType, Line};
use acadrust::tables::AppId;
use acadrust::types::Vector3;
use acadrust::xdata::{ExtendedDataRecord, XDataValue};
use acadrust::{CadDocument, DxfReader, DxfWriter, Handle};

const APP: &str = "SURVEY_APP";

fn text(s: &str) -> XDataValue {
    XDataValue::String(s.into())
}

/// Write a LINE with `first`, the handle `pick` chooses, `last` as its XDATA to DXF and read the
/// values back, next to the handle that was picked.
fn round_trip(pick: impl FnOnce(&CadDocument) -> Handle) -> (Handle, Vec<XDataValue>) {
    let mut doc = CadDocument::new();
    let mut app = AppId::new(APP);
    app.handle = doc.allocate_handle();
    doc.app_ids.add(app).expect("appid");
    let handle = pick(&doc);
    let mut line = Line::from_points(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0));
    let mut record = ExtendedDataRecord::new(APP);
    record.values = vec![text("first"), XDataValue::Handle(handle), text("last")];
    line.common.extended_data.add_record(record);
    let h = doc.add_entity(EntityType::Line(line)).expect("line");

    let bytes = DxfWriter::new(&doc).write_to_vec().expect("dxf");
    let back = DxfReader::from_reader(Cursor::new(bytes))
        .expect("reader")
        .read()
        .expect("read");
    let values = back
        .get_entity(h)
        .and_then(|e| e.common().extended_data.get_record(APP))
        .map(|r| r.values.clone())
        .expect("the record");
    (handle, values)
}

#[test]
fn a_null_handle_keeps_the_later_values_in_place() {
    let (_, values) = round_trip(|_| Handle::NULL);
    assert_eq!(
        values,
        vec![text("first"), XDataValue::Handle(Handle::NULL), text("last")]
    );
}

#[test]
fn a_dangling_handle_is_cleared_and_keeps_the_later_values_in_place() {
    let (_, values) = round_trip(|_| Handle::new(0xFF_FFFF));
    assert_eq!(
        values,
        vec![text("first"), XDataValue::Handle(Handle::NULL), text("last")]
    );
}

#[test]
fn a_written_records_handle_stays() {
    let (layer, values) = round_trip(|doc| doc.layers.get("0").expect("layer 0").handle);
    assert_eq!(
        values,
        vec![text("first"), XDataValue::Handle(layer), text("last")]
    );
}
