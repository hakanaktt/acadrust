use std::io::Cursor;

use acadrust::entities::{EntityType, Line};
use acadrust::objects::{BookColor, ObjectType};
use acadrust::tables::Layer;
use acadrust::types::{Color, DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

#[test]
fn layer_color_book_identity_survives_dwg_roundtrip() {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let mut layer = Layer::with_color("Named color", Color::from_rgb(12, 34, 56));
    layer.color_name = Some("Accent".to_string());
    layer.book_name = Some("Brand colors".to_string());
    document.layers.add(layer).unwrap();

    let bytes = DwgWriter::write_to_vec(&document).expect("DWG write");
    let roundtripped = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG read");
    let layer = roundtripped.layers.get("Named color").expect("layer");

    assert_eq!(layer.color, Color::from_rgb(12, 34, 56));
    assert_eq!(layer.color_name.as_deref(), Some("Accent"));
    assert_eq!(layer.book_name.as_deref(), Some("Brand colors"));
}

#[test]
fn layer_color_book_identity_survives_dxf_roundtrip() {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let mut layer = Layer::with_color("Named color", Color::from_rgb(12, 34, 56));
    layer.color_name = Some("Accent".to_string());
    layer.book_name = Some("Brand colors".to_string());
    document.layers.add(layer).unwrap();

    let bytes = DxfWriter::new(&document).write_to_vec().expect("DXF write");
    assert!(String::from_utf8_lossy(&bytes).contains("Brand colors$Accent"));
    let roundtripped = DxfReader::from_reader(Cursor::new(bytes))
        .expect("DXF reader")
        .read()
        .expect("DXF read");
    let layer = roundtripped.layers.get("Named color").expect("layer");

    assert_eq!(layer.color, Color::from_rgb(12, 34, 56));
    assert_eq!(layer.color_name.as_deref(), Some("Accent"));
    assert_eq!(layer.book_name.as_deref(), Some("Brand colors"));
}

#[test]
fn entity_color_book_identity_survives_dxf_roundtrip() {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let book_handle = Handle::new(0x900);
    document.objects.insert(
        book_handle,
        ObjectType::BookColor(BookColor {
            handle: book_handle,
            owner: Handle::NULL,
            color: Color::from_rgb(220, 100, 20),
            color_name: "Accent".to_string(),
            book_name: "Brand colors".to_string(),
        }),
    );
    let mut line = Line::from_coords(0.0, 0.0, 0.0, 1.0, 1.0, 0.0);
    line.common.color = Color::from_rgb(220, 100, 20);
    line.common.color_name = Some("Brand colors$Accent".to_string());
    let handle = document.add_entity(EntityType::Line(line)).unwrap();

    let bytes = DxfWriter::new(&document).write_to_vec().expect("DXF write");
    let roundtripped = DxfReader::from_reader(Cursor::new(bytes))
        .expect("DXF reader")
        .read()
        .expect("DXF read");
    let common = roundtripped.get_entity(handle).expect("line").common();

    assert_eq!(common.color, Color::from_rgb(220, 100, 20));
    assert_eq!(common.color_name.as_deref(), Some("Brand colors$Accent"));
    assert_eq!(common.color_book_handle, Some(book_handle));
}

#[test]
fn book_color_object_survives_dxf_roundtrip() {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let handle = Handle::new(0x900);
    document.objects.insert(
        handle,
        ObjectType::BookColor(BookColor {
            handle,
            owner: Handle::NULL,
            color: Color::from_rgb(220, 100, 20),
            color_name: "Accent".to_string(),
            book_name: "Brand colors".to_string(),
        }),
    );

    let bytes = DxfWriter::new(&document).write_to_vec().expect("DXF write");
    let roundtripped = DxfReader::from_reader(Cursor::new(bytes))
        .expect("DXF reader")
        .read()
        .expect("DXF read");
    let ObjectType::BookColor(color) = roundtripped.objects.get(&handle).expect("book color")
    else {
        panic!("book color object");
    };

    assert_eq!(color.color, Color::from_rgb(220, 100, 20));
    assert_eq!(color.color_name, "Accent");
    assert_eq!(color.book_name, "Brand colors");
}

#[test]
fn layer_state_restores_color_book_identity() {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let mut layer = Layer::with_color("Named color", Color::from_rgb(12, 34, 56));
    layer.handle = document.allocate_handle();
    layer.color_name = Some("Accent".to_string());
    layer.book_name = Some("Brand colors".to_string());
    document.layers.add(layer).unwrap();
    document.capture_layer_state("Named state", "");

    let layer = document.layers.get_mut("Named color").unwrap();
    layer.color = Color::Index(7);
    layer.color_name = None;
    layer.book_name = None;
    assert_eq!(document.restore_layer_state("Named state"), Some(2));
    let layer = document.layers.get("Named color").unwrap();

    assert_eq!(layer.color, Color::from_rgb(12, 34, 56));
    assert_eq!(layer.color_name.as_deref(), Some("Accent"));
    assert_eq!(layer.book_name.as_deref(), Some("Brand colors"));
}
