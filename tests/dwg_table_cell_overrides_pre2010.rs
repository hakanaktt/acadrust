//! TABLE cell overrides set through the property flags (content color,
//! background, text height, alignment) survive a pre-R2010 DWG save, and are
//! flagged in the DXF cell override flag, which AutoCAD needs to apply them.

use std::io::Cursor;

use acadrust::entities::{CellStyle, CellStylePropertyFlags as P, EntityType, Table, TableCell};
use acadrust::types::{Color, DxfVersion, Handle, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfWriter};

fn overridden_cell_doc(version: DxfVersion) -> (CadDocument, Handle) {
    let mut doc = CadDocument::with_version(version);
    let mut table = Table::new(Vector3::ZERO, 2, 2);
    let mut cell = TableCell::text("lab");
    let mut style = CellStyle::new();
    style.content_color = Color::from_index(1);
    style.background_color = Color::from_index(30);
    style.fill_enabled = true;
    style.text_height = 0.5;
    style.alignment = 9;
    style.property_flags = P::CONTENT_COLOR | P::BACKGROUND_COLOR | P::TEXT_HEIGHT | P::ALIGNMENT;
    cell.style = Some(style);
    table.rows[1].cells[0] = cell;
    let handle = doc.add_entity(EntityType::Table(Box::new(table))).unwrap();
    (doc, handle)
}

/// Each cell's override flag (91, 177 before R2007) in the DXF of `doc`.
fn dxf_cell_flags(doc: &CadDocument, version: DxfVersion) -> Vec<i32> {
    let dxf = String::from_utf8(DxfWriter::new(doc).write_to_vec().unwrap()).unwrap();
    let lines: Vec<&str> = dxf.lines().map(str::trim).collect();
    let pairs: Vec<(&str, &str)> = lines.chunks(2).map(|p| (p[0], p[1])).collect();
    let start = pairs
        .iter()
        .position(|p| *p == ("0", "ACAD_TABLE"))
        .unwrap();
    let flag_code = if version >= DxfVersion::AC1021 {
        "91"
    } else {
        "177"
    };
    // First override flag after each cell's 171 type group.
    let mut flags = Vec::new();
    let mut in_cell = false;
    for (code, value) in &pairs[start + 1..] {
        match *code {
            "0" => break,
            "171" => in_cell = true,
            c if c == flag_code && in_cell => {
                flags.push(value.parse::<i32>().unwrap());
                in_cell = false;
            }
            _ => {}
        }
    }
    flags
}

#[test]
fn dxf_cell_override_flag_names_property_flag_overrides() {
    for version in [DxfVersion::AC1018, DxfVersion::AC1021, DxfVersion::AC1032] {
        let (doc, _) = overridden_cell_doc(version);
        let flags = dxf_cell_flags(&doc, version);
        // alignment 0x01, background 0x02 | 0x04, content color 0x08, text height 0x20
        assert!(
            flags.iter().any(|f| f & 0x2F == 0x2F),
            "{version:?}: no cell flags its overrides: {flags:?}"
        );
    }
}

#[test]
fn dxf_cell_override_flag_ignores_a_stored_r2010_flag_word() {
    // An R2010+ DWG read leaves the R2010 property bits in `override_flags`
    // (0x200 = background color), which are not legacy bits (0x200 = left
    // border color).
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut table = Table::new(Vector3::ZERO, 2, 2);
    let mut cell = TableCell::text("lab");
    let mut style = CellStyle::new();
    style.override_flags = P::BACKGROUND_COLOR.bits() as i32;
    cell.style = Some(style);
    table.rows[1].cells[0] = cell;
    doc.add_entity(EntityType::Table(Box::new(table))).unwrap();
    let flags = dxf_cell_flags(&doc, DxfVersion::AC1032);
    assert!(
        flags.iter().all(|f| f & 0x200 == 0),
        "R2010 flag word in a cell flag: {flags:?}"
    );
}

#[test]
fn cell_overrides_from_property_flags_survive_pre_r2010_dwg() {
    for version in [DxfVersion::AC1018, DxfVersion::AC1021] {
        let (doc, handle) = overridden_cell_doc(version);

        let dwg = DwgWriter::write_to_vec(&doc).unwrap();
        let back = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
        let Some(EntityType::Table(table)) = back.get_entity(handle) else {
            panic!("{version:?}: table lost");
        };
        let style = table.rows[1].cells[0]
            .style
            .as_ref()
            .unwrap_or_else(|| panic!("{version:?}: cell style lost"));
        for property in [
            P::CONTENT_COLOR,
            P::BACKGROUND_COLOR,
            P::TEXT_HEIGHT,
            P::ALIGNMENT,
        ] {
            assert!(style.sets(property), "{version:?}: {property:?} not set");
        }
        assert_eq!(style.content_color, Color::from_index(1), "{version:?}");
        assert_eq!(style.background_color, Color::from_index(30), "{version:?}");
        assert!(style.fill_enabled, "{version:?}: fill");
        assert_eq!(style.text_height, 0.5, "{version:?}");
        assert_eq!(style.alignment, 9, "{version:?}");
    }
}
