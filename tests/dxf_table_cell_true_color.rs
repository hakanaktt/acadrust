//! TABLE cell true colors (content, background, borders) survive a DXF save:
//! the index color is followed by its RGB (63 → 421, 64 → 422, 69 → 427).

use std::io::Cursor;

use acadrust::entities::{CellStyle, CellStylePropertyFlags as P, EntityType, Table, TableCell};
use acadrust::types::{Color, DxfVersion, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};

#[test]
fn cell_true_colors_survive_dxf() {
    let content = Color::from_rgb(10, 120, 200);
    let background = Color::from_rgb(250, 240, 5);
    let border = Color::from_rgb(1, 2, 3);
    for version in [DxfVersion::AC1021, DxfVersion::AC1027, DxfVersion::AC1032] {
        let mut doc = CadDocument::with_version(version);
        let mut table = Table::new(Vector3::ZERO, 2, 2);
        let mut cell = TableCell::text("lab");
        let mut style = CellStyle::new();
        style.content_color = content;
        style.background_color = background;
        style.fill_enabled = true;
        style.top_border.color = border;
        style.property_flags = P::CONTENT_COLOR | P::BACKGROUND_COLOR;
        cell.style = Some(style);
        table.rows[1].cells[0] = cell;
        let handle = doc.add_entity(EntityType::Table(Box::new(table))).unwrap();

        let dxf = DxfWriter::new(&doc).write_to_vec().unwrap();
        let back = DxfReader::from_reader(Cursor::new(dxf))
            .unwrap()
            .read()
            .unwrap();
        let Some(EntityType::Table(table)) = back.get_entity(handle) else {
            panic!("{version:?}: table lost");
        };
        let style = table.rows[1].cells[0].style.as_ref().unwrap();
        assert_eq!(style.content_color, content, "{version:?} content");
        assert_eq!(style.background_color, background, "{version:?} background");
        assert_eq!(style.top_border.color, border, "{version:?} top border");
    }
}
