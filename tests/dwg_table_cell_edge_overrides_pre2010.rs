//! Edge overrides of a legacy TABLE cell (AcDbTable cell data below R2010).
//!
//! ODA's Open Design Specification (20.4.96.1) gives each edge (top, right,
//! bottom, left) a color bit 0x40 << i, a line weight bit 0x400 << i and a
//! visibility bit 0x4000 << i, and an edge set in the virtual edge byte
//! (RC 178) carries no border data. The legacy override word keeps that
//! meaning through DWG, DXF and the conversion to the R2010+ cell style.

use std::io::Cursor;

use acadrust::entities::table::{BorderPropertyFlags, CellEdgeFlags, CellStyle};
use acadrust::entities::{EntityType, Table};
use acadrust::types::{Color, DxfVersion, Handle, LineWeight, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

const TOP_VISIBILITY: i32 = 0x4000;
const RIGHT_WEIGHT: i32 = 0x800;
const BOTTOM_WEIGHT: i32 = 0x1000;
const LEFT_VISIBILITY: i32 = 0x20000;

fn legacy_style(flags: i32) -> CellStyle {
    let mut style = CellStyle::new();
    style.override_flags = flags;
    style.legacy_override_bits = true;
    style
}

fn table_document(version: DxfVersion, table: Table) -> (CadDocument, Handle) {
    let mut document = CadDocument::with_version(version);
    let handle = document
        .add_entity(EntityType::Table(Box::new(table)))
        .unwrap();
    (document, handle)
}

fn table(document: &CadDocument, handle: Handle) -> &Table {
    let Some(EntityType::Table(table)) = document.get_entity(handle) else {
        panic!("table lost");
    };
    table
}

fn dwg_roundtrip(document: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(document).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn dxf_text(document: &CadDocument) -> String {
    String::from_utf8(DxfWriter::new(document).write_to_vec().unwrap()).unwrap()
}

/// The group codes of the first cell of the first ACAD_TABLE, from its 171
/// to the next 171.
fn first_cell_codes(dxf: &str) -> Vec<i32> {
    let lines: Vec<&str> = dxf.lines().map(str::trim).collect();
    let pairs: Vec<(&str, &str)> = lines.chunks(2).map(|p| (p[0], p[1])).collect();
    let start = pairs
        .iter()
        .position(|p| *p == ("0", "ACAD_TABLE"))
        .unwrap();
    pairs[start + 1..]
        .iter()
        .map(|(code, _)| code.parse::<i32>().unwrap())
        .skip_while(|code| *code != 171)
        .enumerate()
        .take_while(|(i, code)| *i == 0 || *code != 171)
        .map(|(_, code)| code)
        .collect()
}

#[test]
fn edge_visibility_and_virtual_edges_survive_a_pre_r2010_dwg_save() {
    for version in [DxfVersion::AC1018, DxfVersion::AC1021] {
        let mut source = Table::new(Vector3::ZERO, 2, 2);
        for (row, column, text) in [(0, 0, "a"), (0, 1, "b"), (1, 0, "c"), (1, 1, "d")] {
            source.set_cell_text(row, column, text);
        }
        // Text height, the visibility of the top and left edges, the bottom
        // line weight: no line weight on top and left, no visibility at the
        // bottom.
        let flags = 0x20 | TOP_VISIBILITY | LEFT_VISIBILITY | BOTTOM_WEIGHT;
        let mut style = legacy_style(flags);
        style.text_height = 2.5;
        style.top_border.invisible = true;
        style.left_border.invisible = true;
        style.bottom_border.line_weight = LineWeight::from_value(35);
        source.rows[0].cells[0].style = Some(style);
        // Top and left colors; the left edge is virtual (the cell on its left
        // holds the shared edge), so its color is not stored.
        let mut style = legacy_style(0x40 | 0x200);
        style.top_border.color = Color::from_index(3);
        style.left_border.color = Color::from_index(5);
        source.rows[0].cells[1].style = Some(style);
        source.rows[0].cells[1].virtual_edge = CellEdgeFlags::LEFT.bits() as i16;
        let (document, handle) = table_document(version, source);

        let loaded = dwg_roundtrip(&document);
        let back = table(&loaded, handle);
        let texts: Vec<_> = back
            .rows
            .iter()
            .flat_map(|row| row.cells.iter().map(|cell| cell.text_value().to_string()))
            .collect();
        assert_eq!(texts, ["a", "b", "c", "d"], "{version:?}");

        let first = &back.rows[0].cells[0];
        let style = first.style.as_ref().expect("first cell style");
        assert_eq!(style.override_flags, flags, "{version:?}");
        assert_eq!(style.text_height, 2.5, "{version:?}");
        assert!(style.top_border.invisible, "{version:?}: top visibility");
        assert!(style.left_border.invisible, "{version:?}: left visibility");
        assert_eq!(
            style.bottom_border.line_weight,
            LineWeight::from_value(35),
            "{version:?}"
        );
        // The edges with border data are named in the cell's edge flags.
        assert_eq!(
            first.edge_flags,
            (CellEdgeFlags::TOP | CellEdgeFlags::BOTTOM | CellEdgeFlags::LEFT).bits() as u8,
            "{version:?}"
        );

        let second = &back.rows[0].cells[1];
        let style = second.style.as_ref().expect("second cell style");
        assert_eq!(second.virtual_edge, CellEdgeFlags::LEFT.bits() as i16);
        assert_eq!(style.top_border.color, Color::from_index(3), "{version:?}");
        assert_eq!(
            style.left_border.color,
            Color::ByBlock,
            "{version:?}: a virtual edge stores no color"
        );
    }
}

#[test]
fn dxf_edge_visibility_keeps_its_own_bit() {
    for version in [DxfVersion::AC1018, DxfVersion::AC1021] {
        let mut source = Table::new(Vector3::ZERO, 1, 1);
        source.set_cell_text(0, 0, "cell");
        // The right line weight and the left visibility.
        let mut style = legacy_style(RIGHT_WEIGHT | LEFT_VISIBILITY);
        style.right_border.line_weight = LineWeight::from_value(50);
        style.left_border.invisible = true;
        source.rows[0].cells[0].style = Some(style);
        let (document, handle) = table_document(version, source);

        // 275 is the right line weight, 285 its visibility; 278 and 288 the
        // left ones.
        let dxf = dxf_text(&document);
        let codes = first_cell_codes(&dxf);
        for (code, written) in [(275, true), (285, false), (278, false), (288, true)] {
            assert_eq!(
                codes.contains(&code),
                written,
                "{version:?}: {code} in {codes:?}"
            );
        }

        let read = DxfReader::from_reader(Cursor::new(dxf.into_bytes()))
            .unwrap()
            .read()
            .unwrap();
        let style = table(&read, handle).rows[0].cells[0].style.clone().unwrap();
        assert_eq!(
            style.override_flags,
            RIGHT_WEIGHT | LEFT_VISIBILITY,
            "{version:?}: DXF read"
        );
        assert!(style.left_border.invisible);

        // And from DXF on to a pre-R2010 DWG.
        assert_eq!(read.version, version);
        let loaded = dwg_roundtrip(&read);
        let back = table(&loaded, handle).rows[0].cells[0]
            .style
            .clone()
            .unwrap();
        assert_eq!(
            back.override_flags,
            RIGHT_WEIGHT | LEFT_VISIBILITY,
            "{version:?}: DWG read"
        );
        assert_eq!(back.right_border.line_weight, LineWeight::from_value(50));
        assert!(back.left_border.invisible, "{version:?}");
    }
}

#[test]
fn legacy_weight_and_visibility_convert_to_their_own_r2010_properties() {
    for version in [DxfVersion::AC1024, DxfVersion::AC1032] {
        let mut source = Table::new(Vector3::ZERO, 1, 1);
        source.set_cell_text(0, 0, "cell");
        let mut style = legacy_style(RIGHT_WEIGHT | LEFT_VISIBILITY);
        style.right_border.line_weight = LineWeight::from_value(50);
        style.left_border.invisible = true;
        source.rows[0].cells[0].style = Some(style);
        let (document, handle) = table_document(version, source);

        let loaded = dwg_roundtrip(&document);
        let style = table(&loaded, handle).rows[0].cells[0]
            .style
            .clone()
            .unwrap();
        assert_eq!(
            style.applied_border_edges,
            CellEdgeFlags::RIGHT | CellEdgeFlags::LEFT,
            "{version:?}"
        );
        assert_eq!(
            style.right_border.override_flags,
            BorderPropertyFlags::LINE_WEIGHT,
            "{version:?}"
        );
        assert_eq!(style.right_border.line_weight, LineWeight::from_value(50));
        assert_eq!(
            style.left_border.override_flags,
            BorderPropertyFlags::INVISIBILITY,
            "{version:?}"
        );
        assert!(style.left_border.invisible, "{version:?}");
    }
}
