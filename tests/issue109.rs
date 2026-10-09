use std::io::Cursor;

use acadrust::entities::{EntityType, Hatch, HatchGradientPattern};
use acadrust::types::{Color, DxfVersion};
use acadrust::{CadDocument, DxfReader, DxfWriter};

fn gradient_document() -> CadDocument {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    let mut hatch = Hatch::new();
    hatch.is_solid = true;
    hatch.gradient_color = HatchGradientPattern {
        enabled: true,
        colors: vec![
            acadrust::entities::hatch::GradientColorEntry {
                value: 0.0,
                color: Color::Rgb { r: 0x12, g: 0x34, b: 0x56 },
                color_name: None,
                book_name: None,
            },
            acadrust::entities::hatch::GradientColorEntry {
                value: 1.0,
                color: Color::Rgb { r: 0x65, g: 0x43, b: 0x21 },
                color_name: None,
                book_name: None,
            },
        ],
        ..Default::default()
    };
    document.add_entity(EntityType::Hatch(hatch)).unwrap();
    document
}

#[test]
fn dxf_gradient_rgb_wins_when_it_follows_aci() {
    let text = String::from_utf8(DxfWriter::new(&gradient_document()).write_to_vec().unwrap())
        .unwrap()
        .replace("\r\n", "\n");
    let reordered = text
        .replace("\n63\n0\n421\n1193046\n", "\n63\n7\n421\n1193046\n")
        .replace("\n63\n0\n421\n6636321\n", "\n63\n7\n421\n6636321\n");
    let restored = DxfReader::from_reader(Cursor::new(reordered.into_bytes()))
        .unwrap()
        .read()
        .unwrap();
    let hatch = restored.entities().find_map(|entity| match entity {
        EntityType::Hatch(hatch) => Some(hatch),
        _ => None,
    }).unwrap();
    assert_eq!(hatch.gradient_color.colors[0].color, Color::Rgb { r: 0x12, g: 0x34, b: 0x56 });
    assert_eq!(hatch.gradient_color.colors[1].color, Color::Rgb { r: 0x65, g: 0x43, b: 0x21 });
}

#[test]
fn gradient_color_entries_default_metadata_is_empty() {
    let document = gradient_document();
    let hatch = document.entities().find_map(|entity| match entity {
        EntityType::Hatch(hatch) => Some(hatch),
        _ => None,
    }).unwrap();
    assert!(hatch.gradient_color.colors.iter().all(|entry| entry.color_name.is_none() && entry.book_name.is_none()));
}
