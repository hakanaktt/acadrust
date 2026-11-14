//! A table built through the R2010+ cell model, saved below R2010.
//!
//! The legacy AcDbTable cell (ODA's Open Design Specification, 20.4.96.1)
//! has one override word in legacy bits (0x01 alignment, 0x02 no fill,
//! 0x04 background, 0x08 content color, 0x10 text style, 0x20 text height,
//! per edge color 0x40, line weight 0x400 and visibility 0x4000, shifted by
//! the edge) and no row, column or table cell styles. The word is built
//! from the values resolved for the cell (content, cell, row, column,
//! table), and the cell type and value handle (344) come from the cell's
//! contents.

use std::io::Cursor;

use acadrust::entities::table::{
    BorderPropertyFlags, CellContent, CellEdgeFlags, CellStyle, CellStylePropertyFlags as P,
    CellType, TableCell,
};
use acadrust::entities::{EntityType, Table};
use acadrust::fields::NewField;
use acadrust::tables::BlockRecord;
use acadrust::types::{Color, DxfVersion, Handle, LineWeight, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfWriter};

const VERSIONS: [DxfVersion; 2] = [DxfVersion::AC1018, DxfVersion::AC1021];

const ALIGNMENT: i32 = 0x01;
const BACKGROUND: i32 = 0x02 | 0x04;
const CONTENT_COLOR: i32 = 0x08;
const TEXT_HEIGHT: i32 = 0x20;
const TOP_COLOR: i32 = 0x40;
const LEFT_COLOR: i32 = 0x200;
const BOTTOM_WEIGHT: i32 = 0x1000;
const TOP_VISIBILITY: i32 = 0x4000;
const BOTTOM_VISIBILITY: i32 = 0x10000;

/// The cell style an R2010+ read gives a cell: the four margins and the two
/// spacings overridden (as AutoCAD stores them), plus `extra`.
fn r2010_style(extra: P) -> CellStyle {
    let mut style = CellStyle::new();
    style.override_flags = (P::MARGIN_LEFT
        | P::MARGIN_TOP
        | P::MARGIN_RIGHT
        | P::MARGIN_BOTTOM
        | P::MARGIN_HORIZONTAL_SPACING
        | P::MARGIN_VERTICAL_SPACING
        | extra)
        .bits() as i32;
    style
}

/// A style that overrides only `properties`.
fn overriding(properties: P) -> CellStyle {
    let mut style = CellStyle::new();
    style.override_flags = properties.bits() as i32;
    style
}

/// A style that overrides `property` of the border on `edge`.
fn border_override(edge: CellEdgeFlags, property: BorderPropertyFlags) -> CellStyle {
    let mut style = CellStyle::new();
    style.applied_border_edges = edge;
    let border = match edge {
        CellEdgeFlags::TOP => &mut style.top_border,
        CellEdgeFlags::RIGHT => &mut style.right_border,
        CellEdgeFlags::BOTTOM => &mut style.bottom_border,
        CellEdgeFlags::LEFT => &mut style.left_border,
        _ => {
            let mut border = acadrust::entities::table::CellBorder::new();
            border.override_flags = property;
            style.additional_borders.push((edge.bits(), border));
            return style;
        }
    };
    border.override_flags = property;
    style
}

fn text_table(rows: usize, columns: usize) -> Table {
    let mut table = Table::new(Vector3::ZERO, rows, columns);
    for row in 0..rows {
        for column in 0..columns {
            table.set_cell_text(row, column, &format!("{row}{column}"));
        }
    }
    table
}

fn document(version: DxfVersion, table: Table) -> (CadDocument, Handle) {
    let mut document = CadDocument::with_version(version);
    let handle = document
        .add_entity(EntityType::Table(Box::new(table)))
        .unwrap();
    (document, handle)
}

fn dwg_roundtrip(document: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(document).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn table(document: &CadDocument, handle: Handle) -> &Table {
    let Some(EntityType::Table(table)) = document.get_entity(handle) else {
        panic!("table lost");
    };
    table
}

/// The legacy override word each cell is read back with (0 without style).
fn words(table: &Table) -> Vec<Vec<i32>> {
    table
        .rows
        .iter()
        .map(|row| {
            row.cells
                .iter()
                .map(|cell| cell.style.as_ref().map_or(0, |style| style.override_flags))
                .collect()
        })
        .collect()
}

fn style(table: &Table, row: usize, column: usize) -> &CellStyle {
    table.rows[row].cells[column]
        .style
        .as_ref()
        .unwrap_or_else(|| panic!("cell {row},{column} has no style"))
}

#[test]
fn r2010_cell_style_bits_are_not_written_as_legacy_bits() {
    for version in VERSIONS {
        let mut source = text_table(1, 1);
        // Margins 0x400..0x2000 and spacings 0x20000/0x40000 have no place
        // in the legacy cell; the alignment (0x10 here) is legacy bit 0x01.
        let mut cell_style = r2010_style(P::ALIGNMENT);
        cell_style.alignment = 6;
        source.rows[0].cells[0].style = Some(cell_style);
        let (document, handle) = document(version, source);

        let loaded = dwg_roundtrip(&document);
        let back = table(&loaded, handle);
        assert_eq!(words(back), [[ALIGNMENT]], "{version:?}");
        assert_eq!(style(back, 0, 0).alignment, 6, "{version:?}");
        assert_eq!(back.rows[0].cells[0].text_value(), "00", "{version:?}");
    }
}

#[test]
fn row_column_and_table_overrides_land_in_each_cell() {
    for version in VERSIONS {
        let mut source = text_table(2, 2);
        // Row 0: text height 2.5; column 1: content color 3; the table:
        // background 30. Cell (0, 0)'s content overrides the text height
        // itself (1.25), cell (1, 1)'s style overrides the content color (5).
        let mut row = overriding(P::TEXT_HEIGHT);
        row.text_height = 2.5;
        source.rows[0].style = Some(row);
        let mut column = overriding(P::CONTENT_COLOR);
        column.content_color = Color::from_index(3);
        source.columns[1].style = Some(column);
        let mut base = overriding(P::BACKGROUND_COLOR);
        base.background_color = Color::from_index(30);
        base.fill_enabled = true;
        source.base_style = Some(base);
        let content = &mut source.rows[0].cells[0].contents[0];
        content.format_override_flags = P::TEXT_HEIGHT.bits() as i32;
        content.text_height = 1.25;
        let mut own = r2010_style(P::CONTENT_COLOR);
        own.content_color = Color::from_index(5);
        source.rows[1].cells[1].style = Some(own);
        let (document, handle) = document(version, source);

        let loaded = dwg_roundtrip(&document);
        let back = table(&loaded, handle);
        assert_eq!(
            words(back),
            [
                [
                    TEXT_HEIGHT | BACKGROUND,
                    TEXT_HEIGHT | CONTENT_COLOR | BACKGROUND
                ],
                [BACKGROUND, CONTENT_COLOR | BACKGROUND],
            ],
            "{version:?}"
        );
        assert_eq!(style(back, 0, 0).text_height, 1.25, "{version:?}");
        assert_eq!(style(back, 0, 1).text_height, 2.5, "{version:?}");
        assert_eq!(
            style(back, 0, 1).content_color,
            Color::from_index(3),
            "{version:?}"
        );
        assert_eq!(
            style(back, 1, 1).content_color,
            Color::from_index(5),
            "{version:?}"
        );
        for (row, column) in [(0, 0), (0, 1), (1, 0), (1, 1)] {
            let cell = style(back, row, column);
            assert!(cell.fill_enabled, "{version:?} {row},{column}");
            assert_eq!(
                cell.background_color,
                Color::from_index(30),
                "{version:?} {row},{column}"
            );
        }
    }
}

#[test]
fn border_overrides_land_on_the_edges_they_cover() {
    for version in VERSIONS {
        let mut source = text_table(2, 2);
        // Row 0's bottom line weight: the bottom edge of both its cells.
        let mut row = border_override(CellEdgeFlags::BOTTOM, BorderPropertyFlags::LINE_WEIGHT);
        row.bottom_border.line_weight = LineWeight::from_value(35);
        source.rows[0].style = Some(row);
        // Column 0's left color: the left edge of both its cells.
        let mut column = border_override(CellEdgeFlags::LEFT, BorderPropertyFlags::COLOR);
        column.left_border.color = Color::from_index(1);
        source.columns[0].style = Some(column);
        // The table's inside horizontal edges are hidden: the bottom of row
        // 0 and the top of row 1.
        let mut base = border_override(
            CellEdgeFlags::INSIDE_HORIZONTAL,
            BorderPropertyFlags::INVISIBILITY,
        );
        base.additional_borders[0].1.invisible = true;
        source.base_style = Some(base);
        // Cell (1, 1)'s own top color.
        let mut own = border_override(CellEdgeFlags::TOP, BorderPropertyFlags::COLOR);
        own.top_border.color = Color::from_index(4);
        source.rows[1].cells[1].style = Some(own);
        let (document, handle) = document(version, source);

        let loaded = dwg_roundtrip(&document);
        let back = table(&loaded, handle);
        assert_eq!(
            words(back),
            [
                [
                    LEFT_COLOR | BOTTOM_WEIGHT | BOTTOM_VISIBILITY,
                    BOTTOM_WEIGHT | BOTTOM_VISIBILITY,
                ],
                [LEFT_COLOR | TOP_VISIBILITY, TOP_COLOR | TOP_VISIBILITY],
            ],
            "{version:?}"
        );
        for column in 0..2 {
            let cell = style(back, 0, column);
            assert_eq!(
                cell.bottom_border.line_weight,
                LineWeight::from_value(35),
                "{version:?} 0,{column}"
            );
            assert!(cell.bottom_border.invisible, "{version:?} 0,{column}");
            assert!(style(back, 1, column).top_border.invisible, "{version:?}");
        }
        for row in 0..2 {
            assert_eq!(
                style(back, row, 0).left_border.color,
                Color::from_index(1),
                "{version:?} {row},0"
            );
        }
        assert_eq!(
            style(back, 1, 1).top_border.color,
            Color::from_index(4),
            "{version:?}"
        );
    }
}

#[test]
fn a_block_content_is_written_as_a_block_cell() {
    for version in VERSIONS {
        let mut document = CadDocument::with_version(version);
        let mut record = BlockRecord::new("MARK");
        record.handle = document.allocate_handle();
        let block = record.handle;
        document.block_records.add(record).unwrap();
        let mut source = text_table(1, 2);
        // The cell type says text, the content is a block.
        let mut cell = TableCell::new();
        cell.contents.push(CellContent::block(block));
        assert_eq!(cell.cell_type, CellType::Text);
        source.rows[0].cells[1] = cell;
        let handle = document
            .add_entity(EntityType::Table(Box::new(source)))
            .unwrap();

        let loaded = dwg_roundtrip(&document);
        let back = &table(&loaded, handle).rows[0].cells[1];
        assert_eq!(back.cell_type, CellType::Block, "{version:?}");
        assert_eq!(back.value_handle, Some(block), "{version:?}");
        assert_eq!(
            back.contents.first().and_then(|c| c.block_handle),
            Some(block),
            "{version:?}"
        );
    }
}

#[test]
fn a_field_content_is_the_cell_value_handle() {
    for version in VERSIONS {
        let (mut document, handle) = document(version, text_table(1, 2));
        let field = document
            .build_table_block(
                handle,
                vec![(
                    0,
                    1,
                    "%<\\_FldIdx 0>%".into(),
                    vec![NewField::new("\\AcExpr (2+3)", "5")],
                )],
            )
            .unwrap()[0]
            .unwrap();

        let loaded = dwg_roundtrip(&document);
        let back = &table(&loaded, handle).rows[0].cells[1];
        assert_eq!(back.cell_type, CellType::Text, "{version:?}");
        assert_eq!(back.value_handle, Some(field), "{version:?}");
    }
}

/// The group codes and values of each cell of the first ACAD_TABLE, from
/// its 171 to the next 171.
fn dxf_cells(dxf: &str) -> Vec<Vec<(i32, String)>> {
    let lines: Vec<&str> = dxf.lines().map(str::trim).collect();
    let pairs: Vec<(i32, &str)> = lines
        .chunks(2)
        .map(|p| (p[0].parse::<i32>().unwrap(), p[1]))
        .collect();
    let start = pairs.iter().position(|p| *p == (0, "ACAD_TABLE")).unwrap();
    let mut cells: Vec<Vec<(i32, String)>> = Vec::new();
    for (code, value) in &pairs[start + 1..] {
        match code {
            0 => break,
            171 => cells.push(vec![(*code, value.to_string())]),
            _ => {
                if let Some(cell) = cells.last_mut() {
                    cell.push((*code, value.to_string()));
                }
            }
        }
    }
    cells
}

#[test]
fn dxf_cell_follows_the_legacy_word() {
    for version in VERSIONS {
        let mut source = text_table(1, 1);
        let mut cell_style = r2010_style(P::ALIGNMENT);
        cell_style.alignment = 6;
        source.rows[0].cells[0].style = Some(cell_style);
        let mut row = overriding(P::TEXT_HEIGHT);
        row.text_height = 2.5;
        source.rows[0].style = Some(row);
        let (document, _) = document(version, source);

        let dxf = String::from_utf8(DxfWriter::new(&document).write_to_vec().unwrap()).unwrap();
        let cells = dxf_cells(&dxf);
        let cell = &cells[0];
        let flag_code = if version >= DxfVersion::AC1021 {
            91
        } else {
            177
        };
        let flag = cell
            .iter()
            .find(|(code, _)| *code == flag_code)
            .map(|(_, value)| value.parse::<i32>().unwrap());
        assert_eq!(flag, Some(ALIGNMENT | TEXT_HEIGHT), "{version:?}: {cell:?}");
        let value = |wanted: i32| {
            cell.iter()
                .find(|(code, _)| *code == wanted)
                .map(|(_, value)| value.parse::<f64>().unwrap())
        };
        assert_eq!(value(170), Some(6.0), "{version:?}: {cell:?}");
        assert_eq!(value(140), Some(2.5), "{version:?}: {cell:?}");
        // No line weight or visibility group: the margins are not edges.
        for code in [275, 276, 278, 279, 285, 286, 288, 289] {
            assert!(
                cell.iter().all(|(c, _)| *c != code),
                "{version:?}: group {code} in {cell:?}"
            );
        }
    }
}
