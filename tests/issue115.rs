use std::io::Cursor;

use acadrust::tables::Layer;
use acadrust::types::{Color, DxfVersion};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfWriter};

const RGB: Color = Color::from_rgb(200, 100, 50);

fn document() -> CadDocument {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    document.layers.add(Layer::with_color("L1", RGB)).unwrap();
    document
}

fn layer_aci(dxf: &[u8]) -> i16 {
    let normalized = String::from_utf8_lossy(dxf).replace("\r\n", "\n");
    let text = normalized.as_str();
    let table = text
        .split_once("2\nLAYER\n")
        .and_then(|(_, rest)| rest.split_once("0\nENDTAB"))
        .map(|(table, _)| table)
        .expect("LAYER table");
    let lines: Vec<_> = table.lines().map(str::trim).collect();
    let start = lines
        .windows(2)
        .position(|pair| pair[0] == "2" && pair[1] == "L1")
        .map(|index| index + 2)
        .expect("L1 layer record");
    lines[start..]
        .chunks(2)
        .take_while(|pair| pair.len() == 2 && pair[0] != "0")
        .find(|pair| pair[0] == "62")
        .and_then(|pair| pair[1].parse().ok())
        .expect("layer group 62")
}

#[test]
fn rgb_layer_writes_nearest_aci_and_true_color() {
    let document = document();
    let dxf = DxfWriter::new(&document).write_to_vec().unwrap();
    assert_eq!(layer_aci(&dxf), RGB.approximate_index());
    let text = String::from_utf8_lossy(&dxf).replace("\r\n", "\n");
    let lines: Vec<_> = text.lines().map(str::trim).collect();
    assert!(lines.windows(2).any(|pair| pair == ["420", "13132850"]));
}

#[test]
fn rgb_layer_keeps_nearest_aci_after_dwg_roundtrip() {
    let document = document();
    let dwg = DwgWriter::write_to_vec(&document).unwrap();
    let restored = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
    let dxf = DxfWriter::new(&restored).write_to_vec().unwrap();
    assert_eq!(layer_aci(&dxf), RGB.approximate_index());
}
