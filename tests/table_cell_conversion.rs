use std::borrow::Cow;
use std::io::Cursor;

use acadrust::entities::table::{
    BorderPropertyFlags, CellEdgeFlags, CellStyle, CellStylePropertyFlags, TableCell,
};
use acadrust::entities::{EntityType, Table};
use acadrust::types::{Color, DxfVersion, Handle, LineWeight, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

fn dxf_document_with_style(version: DxfVersion, style: CellStyle) -> (CadDocument, Handle) {
    let mut document = CadDocument::with_version(version);
    let mut table = Table::new(Vector3::ZERO, 1, 1);
    table.set_cell_text(0, 0, "styled cell");
    table.rows[0].cells[0].style = Some(style);
    let handle = document
        .add_entity(EntityType::Table(Box::new(table)))
        .unwrap();
    let bytes = DxfWriter::new(&document).write_to_vec().unwrap();
    let loaded = DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap();
    (loaded, handle)
}

fn cell_style(document: &CadDocument, handle: Handle) -> &CellStyle {
    let Some(EntityType::Table(table)) = document.get_entity(handle) else {
        panic!("expected table");
    };
    table.rows[0].cells[0].style.as_ref().unwrap()
}

fn dwg_roundtrip(document: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(document).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

#[test]
fn legacy_cell_background_fill_survives_modern_dwg_conversion() {
    for version in [DxfVersion::AC1024, DxfVersion::AC1027, DxfVersion::AC1032] {
        for enabled in [false, true] {
            let mut style = CellStyle::new();
            style.override_flags = 0x02 | 0x04;
            style.background_color = Color::from_index(1);
            style.fill_enabled = enabled;
            let (source, handle) = dxf_document_with_style(version, style);
            let original = cell_style(&source, handle).clone();
            assert!(original.legacy_override_bits);
            assert_eq!(original.fill_enabled, enabled);
            assert_eq!(original.background_color, Color::from_index(1));

            let loaded = dwg_roundtrip(&source);
            let converted = cell_style(&loaded, handle);
            assert_eq!(converted.fill_enabled, enabled, "{version:?}");
            assert!(converted.sets(CellStylePropertyFlags::BACKGROUND_COLOR));
            assert_eq!(
                converted.background_color,
                if enabled {
                    Color::from_index(1)
                } else {
                    Color::ByBlock
                },
                "{version:?}"
            );
            assert_eq!(cell_style(&source, handle), &original);
        }
    }
}

#[test]
fn legacy_cell_borders_survive_modern_dwg_conversion() {
    for version in [DxfVersion::AC1024, DxfVersion::AC1027, DxfVersion::AC1032] {
        let mut style = CellStyle::new();
        style.override_flags = 0x40 | 0x800 | 0x100 | 0x1000;
        style.top_border.color = Color::from_index(3);
        style.right_border.line_weight = LineWeight::from_value(25);
        style.right_border.invisible = true;
        style.bottom_border.color = Color::from_index(5);
        style.bottom_border.line_weight = LineWeight::from_value(35);
        let (source, handle) = dxf_document_with_style(version, style);
        let original = cell_style(&source, handle).clone();
        assert!(original.legacy_override_bits);
        assert_eq!(original.top_border.color, Color::from_index(3));
        assert_eq!(
            original.right_border.line_weight,
            LineWeight::from_value(25)
        );
        assert!(original.right_border.invisible);
        assert_eq!(original.bottom_border.color, Color::from_index(5));
        assert_eq!(
            original.bottom_border.line_weight,
            LineWeight::from_value(35)
        );
        assert!(!original.bottom_border.invisible);

        let loaded = dwg_roundtrip(&source);
        let converted = cell_style(&loaded, handle);
        assert_eq!(
            converted.applied_border_edges,
            CellEdgeFlags::TOP | CellEdgeFlags::RIGHT | CellEdgeFlags::BOTTOM,
            "{version:?}"
        );
        assert_eq!(
            converted.top_border.override_flags,
            BorderPropertyFlags::COLOR
        );
        assert_eq!(converted.top_border.color, original.top_border.color);
        let weight_flags = BorderPropertyFlags::LINE_WEIGHT | BorderPropertyFlags::INVISIBILITY;
        assert_eq!(converted.right_border.override_flags, weight_flags);
        assert_eq!(
            converted.right_border.line_weight,
            original.right_border.line_weight
        );
        assert!(converted.right_border.invisible);
        assert_eq!(
            converted.bottom_border.override_flags,
            weight_flags | BorderPropertyFlags::COLOR
        );
        assert_eq!(converted.bottom_border.color, original.bottom_border.color);
        assert_eq!(
            converted.bottom_border.line_weight,
            original.bottom_border.line_weight
        );
        assert!(!converted.bottom_border.invisible);
        assert_eq!(
            converted.left_border.override_flags,
            BorderPropertyFlags::NONE
        );
        assert_eq!(cell_style(&source, handle), &original);
    }
}

#[test]
fn legacy_invisible_cell_borders_survive_modern_dwg_conversion() {
    for version in [DxfVersion::AC1024, DxfVersion::AC1027, DxfVersion::AC1032] {
        let mut document = CadDocument::with_version(version);
        let mut table = Table::new(Vector3::ZERO, 1, 1);
        table.set_cell_text(0, 0, "hidden border");
        let mut style = CellStyle::new();
        style.legacy_override_bits = true;
        style.override_flags = 0x800;
        style.right_border.line_weight = LineWeight::from_value(25);
        style.right_border.invisible = true;
        table.rows[0].cells[0].style = Some(style);
        let handle = document
            .add_entity(EntityType::Table(Box::new(table)))
            .unwrap();

        let loaded = dwg_roundtrip(&document);
        let converted = cell_style(&loaded, handle);
        assert_eq!(converted.applied_border_edges, CellEdgeFlags::RIGHT);
        assert_eq!(
            converted.right_border.override_flags,
            BorderPropertyFlags::LINE_WEIGHT | BorderPropertyFlags::INVISIBILITY
        );
        assert_eq!(
            converted.right_border.line_weight,
            LineWeight::from_value(25)
        );
        assert!(converted.right_border.invisible, "{version:?}");
    }
}

#[test]
fn binary_cells_keep_their_existing_layout() {
    let mut cell = TableCell::text("binary value");
    let mut style = CellStyle::new();
    style.override_flags = CellStylePropertyFlags::BACKGROUND_COLOR.bits() as i32;
    style.background_color = Color::from_index(1);
    style.applied_border_edges = CellEdgeFlags::TOP;
    style.top_border.color = Color::from_index(3);
    style.top_border.override_flags = BorderPropertyFlags::COLOR;
    cell.style = Some(style);

    let converted = cell.binary_layout();
    assert!(matches!(converted, Cow::Borrowed(_)));
    assert_eq!(converted.as_ref(), &cell);
}
