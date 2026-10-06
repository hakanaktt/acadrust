use acadrust::entities::table::{CellStylePropertyFlags, CellValue};
use acadrust::entities::{Insert, Line, MText, Table};
use acadrust::fields::{self, FieldContext, NewField};
use acadrust::sheet_set::SheetSetDatabase;
use acadrust::tables::{BlockRecord, TextStyle};
use acadrust::types::{Color, Handle, Vector3};
use acadrust::{count, CadDocument, EntityType};

struct Context {
    sheet_set: SheetSetDatabase,
}

impl FieldContext for Context {
    fn now_julian(&self) -> f64 {
        2460000.0
    }

    fn sheet_sets(&self, f: &mut dyn FnMut(&SheetSetDatabase) -> Option<String>) -> Option<String> {
        f(&self.sheet_set)
    }
}

fn table_texts(doc: &CadDocument, table: Handle) -> Vec<String> {
    let EntityType::Table(table) = doc.get_entity(table).unwrap() else {
        panic!("expected table");
    };
    doc.block_records
        .get(&table.block_name)
        .unwrap()
        .entity_handles
        .iter()
        .filter_map(|handle| match doc.get_entity(*handle) {
            Some(EntityType::MText(text)) => Some(text.value.clone()),
            _ => None,
        })
        .collect()
}

fn table_text_formats(doc: &CadDocument, table: Handle) -> Vec<(Color, String)> {
    let EntityType::Table(table) = doc.get_entity(table).unwrap() else {
        panic!("expected table");
    };
    doc.block_records
        .get(&table.block_name)
        .unwrap()
        .entity_handles
        .iter()
        .filter_map(|handle| match doc.get_entity(*handle) {
            Some(EntityType::MText(text)) => Some((text.common.color, text.style.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn plotting_evaluates_against_the_open_sheet_set() {
    let mut doc = CadDocument::new();
    let mut sheet_set = SheetSetDatabase::new("Open set name", "");
    sheet_set.path = Some("C:\\nonexistent-field-regression-directory\\review.dst".into());
    let code = format!(
        "\\AcSm.16.2 Database(\"{}\").SheetSet(\"{}\").Name",
        sheet_set.path.as_ref().unwrap(),
        sheet_set.sheet_set().id()
    );
    let ctx = Context { sheet_set };
    let host = doc.add_entity(EntityType::MText(MText::new())).unwrap();
    doc.set_text_field(
        host,
        "%<\\_FldIdx 0>%",
        vec![NewField::new(code, "cached name")],
    )
    .unwrap();

    assert_eq!(
        fields::resolve(&doc, host, &ctx).as_deref(),
        Some("Open set name")
    );
    assert_eq!(doc.stamp_plot_fields(&ctx), [host]);
    let EntityType::MText(text) = doc.get_entity(host).unwrap() else {
        panic!("expected field host");
    };
    assert_eq!(text.value, "Open set name");
}

#[test]
fn table_block_renders_numeric_and_formatted_cell_values() {
    let mut doc = CadDocument::new();
    let mut table = Table::new(Vector3::ZERO, 1, 3);
    for col in 0..3 {
        table.set_cell_text(0, col, "");
    }
    table.rows[0].cells[0].contents[0].value = CellValue::number(42.0);
    table.rows[0].cells[1].contents[0].value = CellValue::integer(7);
    table.rows[0].cells[2].contents[0].value = CellValue::number(3.0);
    table.rows[0].cells[2].contents[0].value.formatted_value = "3.00 mm".into();
    let handle = doc.add_entity(EntityType::Table(Box::new(table))).unwrap();

    assert!(doc.refresh_table_block(handle));
    assert_eq!(table_texts(&doc, handle), ["42", "7", "3.00 mm"]);
    assert!(!doc.refresh_table_block(handle));
}

#[test]
fn table_block_refreshes_cell_color_and_text_style() {
    let mut doc = CadDocument::new();
    let mut text_style = TextStyle::new("Alternate");
    text_style.handle = doc.allocate_handle();
    doc.text_styles.add(text_style).unwrap();
    let mut table = Table::new(Vector3::ZERO, 1, 1);
    table.set_cell_text(0, 0, "formatted text");
    let content = &mut table.rows[0].cells[0].contents[0];
    content.format_override_flags =
        (CellStylePropertyFlags::CONTENT_COLOR | CellStylePropertyFlags::TEXT_STYLE).bits() as i32;
    content.color = Color::from_index(1);
    content.text_style_name = "Standard".into();
    let handle = doc.add_entity(EntityType::Table(Box::new(table))).unwrap();

    assert!(doc.refresh_table_block(handle));
    assert_eq!(table_text_formats(&doc, handle), [(Color::from_index(1), "Standard".into())]);
    assert!(!doc.refresh_table_block(handle));

    let EntityType::Table(table) = doc.get_entity_mut(handle).unwrap() else {
        panic!("expected table");
    };
    table.rows[0].cells[0].contents[0].color = Color::from_index(2);
    assert!(doc.refresh_table_block(handle));
    assert_eq!(table_text_formats(&doc, handle), [(Color::from_index(2), "Standard".into())]);
    assert!(!doc.refresh_table_block(handle));

    let EntityType::Table(table) = doc.get_entity_mut(handle).unwrap() else {
        panic!("expected table");
    };
    table.rows[0].cells[0].contents[0].text_style_name = "Alternate".into();
    assert!(doc.refresh_table_block(handle));
    assert_eq!(table_text_formats(&doc, handle), [(Color::from_index(2), "Alternate".into())]);
    assert!(!doc.refresh_table_block(handle));
}

#[test]
fn count_keeps_references_with_different_normals() {
    let mut doc = CadDocument::new();
    let mut block = BlockRecord::new("CountNormals");
    block.handle = doc.allocate_handle();
    let owner = block.handle;
    doc.block_records.add(block).unwrap();
    let mut line = Line::from_points(Vector3::ZERO, Vector3::new(1.0, 2.0, 3.0));
    line.common.owner_handle = owner;
    doc.add_entity(EntityType::Line(line)).unwrap();
    let first = Insert::new("CountNormals", Vector3::new(10.0, 20.0, 30.0));
    let mut tilted = first.clone();
    tilted.normal = Vector3::UNIT_Y;
    assert_ne!(
        count::insert_outline(&doc, &first, 0),
        count::insert_outline(&doc, &tilted, 0)
    );
    let first_handle = doc.add_entity(EntityType::Insert(first.clone())).unwrap();
    doc.add_entity(EntityType::Insert(tilted)).unwrap();
    let duplicate_handle = doc.add_entity(EntityType::Insert(first)).unwrap();

    let instances = count::block_instances(&doc, None);
    assert_eq!(
        count::count_of(&instances, "CountNormals", &Default::default()),
        2
    );
    let duplicate = instances
        .iter()
        .find(|instance| instance.handle == duplicate_handle)
        .unwrap();
    assert_eq!(duplicate.duplicate_of, Some(first_handle));
}

#[test]
fn table_formulas_keep_existing_aggregate_functions() {
    let mut doc = CadDocument::new();
    let mut table = Table::new(Vector3::ZERO, 1, 3);
    table.set_cell_text(0, 0, "3");
    table.set_cell_text(0, 1, "7");
    table.set_cell_text(0, 2, "text");
    let handle = doc.add_entity(EntityType::Table(Box::new(table))).unwrap();
    let ctx = Context {
        sheet_set: SheetSetDatabase::new("", ""),
    };

    for (function, expected) in [("Min", "3"), ("Max", "7"), ("Product", "21"), ("Mean", "5")] {
        let code = format!("\\AcExpr ({function}(A1:C1))");
        assert_eq!(
            fields::evaluate_code(&doc, &code, &[], Some(handle), &ctx).as_deref(),
            Some(expected)
        );
    }
}

#[test]
fn field_in_an_empty_table_cell_survives_refresh_and_serialization() {
    use acadrust::entities::table::TableCellContentType;
    use acadrust::{DwgReader, DwgWriter, DxfReader, DxfWriter};
    use std::io::Cursor;

    let mut doc = CadDocument::new();
    let table = doc.add_entity(EntityType::Table(Box::new(Table::new(Vector3::ZERO, 1, 1)))).unwrap();
    let fields = doc.build_table_block(table, vec![(
        0, 0, "%<\\_FldIdx 0>%".into(), vec![NewField::new("\\AcExpr (2+3)", "5")],
    )]).unwrap();
    let field = fields[0].unwrap();
    let EntityType::Table(value) = doc.get_entity(table).unwrap() else { panic!("expected table") };
    let content = &value.rows[0].cells[0].contents[0];
    assert_eq!(content.content_type, TableCellContentType::Field);
    assert_eq!(content.field_handle, Some(field));
    assert_eq!(content.value.display(), "5");
    assert!(!doc.refresh_table_block(table));
    assert_eq!(table_texts(&doc, table), ["5"]);

    let dxf = DxfWriter::new(&doc).write_to_vec().unwrap();
    let dwg = DwgWriter::write_to_vec(&doc).unwrap();
    for loaded in [
        DxfReader::from_reader(Cursor::new(dxf)).unwrap().read().unwrap(),
        DwgReader::from_stream(Cursor::new(dwg)).read().unwrap(),
    ] {
        let EntityType::Table(value) = loaded.get_entity(table).unwrap() else { panic!("expected table") };
        let content = &value.rows[0].cells[0].contents[0];
        assert_eq!(content.content_type, TableCellContentType::Field);
        assert_eq!(content.field_handle, Some(field));
    }
}
