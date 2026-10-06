//! An `AcCmColor` true colour written in its unsigned spelling must survive a
//! DXF read.
//!
//! Group codes 420 and 421 carry a packed word, not a quantity: producers set a
//! method byte in the high position, `0xC2` for a true colour. A plain orange
//! is therefore written `16746496` (0xFF8800) by a producer that omits the
//! method byte, `-1023440896` by one that writes the full word signed, and
//! `3271526400` by one that writes it unsigned. Only the first two fit an
//! `i32`, so the reader used to drop the third: the colour never reached the
//! entity and it fell back to its ACI index, which for a true-coloured entity
//! is usually 256 (`ByLayer`).

use std::io::Cursor;

use acadrust::entities::{EntityType, Line};
use acadrust::tables::Layer;
use acadrust::types::{Color, DxfVersion};
use acadrust::{CadDocument, DxfReader, DxfWriter};

const ORANGE: Color = Color::Rgb {
    r: 255,
    g: 136,
    b: 0,
};

/// A DXF holding one orange line on one orange layer, with every code 420
/// value respelled as `spelling`.
fn dxf_with_true_color_spelled(spelling: &str) -> Vec<u8> {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    document
        .layers
        .add(Layer::with_color("Tinted", ORANGE))
        .expect("layer");

    let mut line = Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    line.common.layer = "Tinted".to_string();
    line.common.color = ORANGE;
    document.add_entity(EntityType::Line(line)).expect("entity");

    let written = DxfWriter::new(&document).write_to_vec().expect("DXF write");
    let text = String::from_utf8(written).expect("DXF is ASCII");
    let mut out = String::with_capacity(text.len());
    let mut respell_next = false;

    for line in text.split_inclusive('\n') {
        if respell_next {
            let terminator = if line.ends_with("\r\n") { "\r\n" } else { "\n" };
            out.push_str(spelling);
            out.push_str(terminator);
            respell_next = false;
            continue;
        }

        respell_next = line.trim() == "420";
        out.push_str(line);
    }

    assert!(out.contains(spelling), "no code 420 was respelled");

    out.into_bytes()
}

fn read_colors(bytes: Vec<u8>) -> (Color, Color) {
    let document = DxfReader::from_reader(Cursor::new(bytes))
        .expect("DXF reader")
        .read()
        .expect("DXF read");
    let entity = document.entities().next().expect("one entity");
    let entity_color = match entity {
        EntityType::Line(line) => line.common.color,
        other => panic!("expected a LINE, got {other:?}"),
    };

    (
        document.layers.get("Tinted").expect("layer").color,
        entity_color,
    )
}

#[test]
fn a_true_color_written_unsigned_reaches_the_layer_and_the_entity() {
    // 0xC2FF8800: the full AcCmColor word, method byte included, written as an
    // unsigned 32-bit value. This is the spelling that used to be dropped.
    let (layer, entity) = read_colors(dxf_with_true_color_spelled("3271526400"));

    assert_eq!(layer, ORANGE);
    assert_eq!(entity, ORANGE);
}

#[test]
fn the_signed_and_bare_spellings_of_the_same_color_still_read() {
    // -1023440896 is the same word written signed, 16746496 the bare 24 bits.
    // Both already fitted an `i32`, so they guard against the fix narrowing
    // what the reader accepts.
    for spelling in ["-1023440896", "16746496"] {
        let (layer, entity) = read_colors(dxf_with_true_color_spelled(spelling));

        assert_eq!(layer, ORANGE, "layer, spelled {spelling}");
        assert_eq!(entity, ORANGE, "entity, spelled {spelling}");
    }
}
