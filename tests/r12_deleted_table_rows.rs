//! #282: an AC1009 table row flagged deleted (flag 128, empty name) keeps
//! stale values. They are not validated like a live row's, so a stale one
//! does not abort the read.

use std::io::Cursor;

use acadrust::entities::EntityType;
use acadrust::io::dwg::crc::{crc16, CRC16_SEED};
use acadrust::io::dwg::DwgReadOptions;
use acadrust::tables::{Layer, VPort};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion};

const LAYER_ROW: usize = 41;
const VPORT_ROW: usize = 253;
const LAYER_NAME: &str = "STALELAYER";
const VPORT_NAME: &str = "STALEVPORT";

/// An R12 drawing with one line, an unused layer and an extra viewport.
fn drawing() -> Vec<u8> {
    let mut document = CadDocument::with_version(DxfVersion::AC1009);
    document
        .add_entity(EntityType::Line(acadrust::Line::from_coords(
            0.0, 0.0, 0.0, 1.0, 1.0, 0.0,
        )))
        .unwrap();
    document.layers.add(Layer::new(LAYER_NAME)).unwrap();
    document.vports.add(VPort::new(VPORT_NAME)).unwrap();
    DwgWriter::write_to_vec(&document).unwrap()
}

/// Start of the table row named `name`: its flag byte, then the 32-byte name.
fn row_start(data: &[u8], name: &str) -> usize {
    let mut field = [0u8; 32];
    field[..name.len()].copy_from_slice(name.as_bytes());
    let at = data
        .windows(32)
        .position(|window| window == field)
        .unwrap_or_else(|| panic!("row {name} not written"));
    at - 1
}

/// Applies `edit` to the row, then recomputes its CRC.
fn edit_row(data: &mut [u8], name: &str, size: usize, edit: impl FnOnce(&mut [u8])) {
    let start = row_start(data, name);
    let row = &mut data[start..start + size];
    edit(row);
    let checksum = crc16(CRC16_SEED, &row[..size - 2]);
    row[size - 2..].copy_from_slice(&checksum.to_le_bytes());
}

/// Flag 128 and an empty name: the row is deleted.
fn mark_deleted(row: &mut [u8]) {
    row[0] = 128;
    row[1..33].fill(0);
}

/// Layer row: flag, name, usage, colour (short), linetype index (short).
fn set_layer_linetype(row: &mut [u8], index: i16) {
    row[37..39].copy_from_slice(&index.to_le_bytes());
}

/// VPORT row: flag, name, usage, then the lower-left corner's X (double).
fn set_vport_first_double(row: &mut [u8], value: f64) {
    row[35..43].copy_from_slice(&value.to_le_bytes());
}

fn read(data: Vec<u8>) -> acadrust::Result<CadDocument> {
    DwgReader::from_stream(Cursor::new(data)).read()
}

fn lines(document: &CadDocument) -> usize {
    document
        .entities()
        .filter(|entity| matches!(entity, EntityType::Line(_)))
        .count()
}

#[test]
fn the_unedited_drawing_reads() {
    let document = read(drawing()).unwrap();
    assert_eq!(lines(&document), 1);
    assert!(document.layers.get(LAYER_NAME).is_some());
}

#[test]
fn a_deleted_layer_row_with_a_stale_linetype_index_does_not_abort_the_read() {
    let mut data = drawing();
    edit_row(&mut data, LAYER_NAME, LAYER_ROW, |row| {
        mark_deleted(row);
        set_layer_linetype(row, 28);
    });
    let document = read(data).unwrap();
    assert_eq!(lines(&document), 1);
    assert!(document.layers.get(LAYER_NAME).is_none());
}

#[test]
fn a_deleted_vport_row_with_a_non_finite_double_does_not_abort_the_read() {
    let mut data = drawing();
    edit_row(&mut data, VPORT_NAME, VPORT_ROW, |row| {
        mark_deleted(row);
        set_vport_first_double(row, f64::NAN);
    });
    let document = read(data).unwrap();
    assert_eq!(lines(&document), 1);
    assert!(document.vports.get(VPORT_NAME).is_none());
}

/// Guard: a live row is still validated.
#[test]
fn a_live_layer_row_with_a_stale_linetype_index_is_still_an_error() {
    let mut data = drawing();
    edit_row(&mut data, LAYER_NAME, LAYER_ROW, |row| {
        set_layer_linetype(row, 28)
    });
    let error = read(data).unwrap_err().to_string();
    assert!(error.contains("linetype index 28"), "{error}");
}

/// A failsafe read whose AC1009 read failed as a whole must not report the
/// record stream as read.
#[test]
fn a_failed_failsafe_ac1009_read_does_not_report_the_record_stream_as_read() {
    let mut data = drawing();
    edit_row(&mut data, LAYER_NAME, LAYER_ROW, |row| {
        set_layer_linetype(row, 28)
    });
    let outcome =
        DwgReader::from_stream_with_options(Cursor::new(data), DwgReadOptions::failsafe())
            .read_with_stats()
            .unwrap();
    assert_eq!(outcome.document.entities().count(), 0);
    assert!(!outcome.stats.record_stream_read);
    assert!(!outcome.stats.has_usable_drawing_data());
}
