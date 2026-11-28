//! The picture block of a table takes a cell's text height and margins from
//! the row and column overrides too, not only from the cell and the table
//! style. A table scaled as a whole states the text height on each row.

use acadrust::entities::table::{CellStyle, CellStylePropertyFlags as P};
use acadrust::entities::{MText, Table};
use acadrust::types::{Handle, Vector3};
use acadrust::{CadDocument, EntityType};

fn texts(doc: &CadDocument, table: Handle) -> Vec<MText> {
    let EntityType::Table(table) = doc.get_entity(table).unwrap() else {
        panic!("expected table");
    };
    doc.block_records
        .get(&table.block_name)
        .unwrap()
        .entity_handles
        .iter()
        .filter_map(|handle| match doc.get_entity(*handle) {
            Some(EntityType::MText(text)) => Some((**text).clone()),
            _ => None,
        })
        .collect()
}

fn overriding(flags: P) -> CellStyle {
    CellStyle {
        override_flags: flags.bits() as i32,
        ..CellStyle::new()
    }
}

fn table() -> Table {
    let mut table = Table::new(Vector3::ZERO, 2, 2);
    for (row, name) in [(0, ["A1", "B1"]), (1, ["A2", "B2"])] {
        table.rows[row].height = 10.0;
        for (col, text) in name.into_iter().enumerate() {
            table.set_cell_text(row, col, text);
        }
    }
    for column in &mut table.columns {
        column.width = 20.0;
    }
    table
}

#[test]
fn row_and_column_text_height_overrides_reach_the_picture() {
    let mut doc = CadDocument::new();
    let mut table = table();
    // Row 2 states a height; column B states another, which wins where they cross.
    let mut row = overriding(P::TEXT_HEIGHT);
    row.text_height = 2.0;
    table.rows[1].style = Some(row);
    let mut column = overriding(P::TEXT_HEIGHT);
    column.text_height = 3.0;
    table.columns[1].style = Some(column);
    // A cell's own content height still wins over both.
    let own = &mut table.rows[1].cells[1].contents[0];
    own.format_override_flags |= P::TEXT_HEIGHT.bits() as i32;
    own.text_height = 1.25;
    let handle = doc.add_entity(EntityType::Table(Box::new(table))).unwrap();

    assert!(doc.refresh_table_block(handle));
    let texts = texts(&doc, handle);
    let height = |name: &str| texts.iter().find(|t| t.value == name).unwrap().height;
    let plain = height("A1");
    assert!(plain > 0.0 && plain != 2.0 && plain != 3.0);
    assert_eq!(height("B1"), 3.0);
    assert_eq!(height("A2"), 2.0);
    assert_eq!(height("B2"), 1.25);
}

#[test]
fn margin_overrides_place_the_text_and_set_its_width() {
    let mut doc = CadDocument::new();
    let mut table = table();
    // Cell A1 states its own margins; row 2 states them for its cells.
    let margins = P::MARGIN_LEFT | P::MARGIN_TOP | P::MARGIN_RIGHT | P::MARGIN_BOTTOM;
    let mut own = overriding(margins | P::ALIGNMENT);
    own.alignment = 1;
    (
        own.margin_left,
        own.margin_top,
        own.margin_right,
        own.margin_bottom,
    ) = (1.0, 2.0, 3.0, 4.0);
    table.rows[0].cells[0].style = Some(own);
    let mut row = overriding(margins);
    (
        row.margin_left,
        row.margin_top,
        row.margin_right,
        row.margin_bottom,
    ) = (0.5, 0.75, 0.5, 0.75);
    table.rows[1].style = Some(row);
    let mut right_bottom = overriding(P::ALIGNMENT);
    right_bottom.alignment = 9;
    table.rows[1].cells[1].style = Some(right_bottom);
    let handle = doc.add_entity(EntityType::Table(Box::new(table))).unwrap();

    assert!(doc.refresh_table_block(handle));
    let texts = texts(&doc, handle);
    let text = |name: &str| texts.iter().find(|t| t.value == name).unwrap();
    let near = |a: f64, b: f64| (a - b).abs() < 1e-9;

    let a1 = text("A1");
    assert!(near(a1.insertion_point.x, 1.0), "{}", a1.insertion_point.x);
    assert!(near(a1.insertion_point.y, -2.0), "{}", a1.insertion_point.y);
    assert!(
        near(a1.rectangle_width, 20.0 - 1.0 - 3.0),
        "{}",
        a1.rectangle_width
    );

    let b2 = text("B2");
    assert!(
        near(b2.insertion_point.x, 40.0 - 0.5),
        "{}",
        b2.insertion_point.x
    );
    assert!(
        near(b2.insertion_point.y, -(20.0 - 0.75)),
        "{}",
        b2.insertion_point.y
    );
    assert!(
        near(b2.rectangle_width, 20.0 - 0.5 - 0.5),
        "{}",
        b2.rectangle_width
    );

    // Without an override the table style's margins stay in use.
    let b1 = text("B1");
    assert!(b1.rectangle_width > 20.0 - 1.0, "{}", b1.rectangle_width);
}
