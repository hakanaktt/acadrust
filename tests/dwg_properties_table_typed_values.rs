//! A dynamic block's properties table (BLOCKPROPERTIESTABLE) is read from
//! DWG with every column's typed values.
//!
//! After its number (90), each column of the table has two typed values,
//! each stored the way a row's cell is: a type (BS), then the value in that
//! type's form; type -9999 is an empty value with nothing after it. The same
//! groups appear in that order in a DXF of the table (170 type, then the
//! value's group). The reader took the two types for plain numbers and gave
//! up on an empty row cell:
//!
//! - a column with a text value left its string in the stream, so every later
//!   string moved by one and the last one was not read;
//! - a row cell of type -9999 ended the read, so the table was not read at
//!   all.
//!
//! The table here is built in code and written by the DWG writer, which
//! stores the groups one after another, each in its type's form.

use acadrust::{
    objects::{
        BlockPropertiesTable, BlockPropertiesTableField as Field,
        BlockPropertiesTableValue as Value, DynamicBlockData, DynamicBlockObject, ObjectType,
    },
    types::DxfVersion,
    CadDocument, DwgReader, DwgWriter, Handle,
};
use std::io::Cursor;

fn int(code: i32, value: i64) -> Field {
    Field {
        code,
        value: Value::Integer(value),
    }
}

fn text(code: i32, value: &str) -> Field {
    Field {
        code,
        value: Value::Text(value.into()),
    }
}

fn flag(code: i32, value: bool) -> Field {
    Field {
        code,
        value: Value::Boolean(value),
    }
}

fn handle(code: i32) -> Field {
    Field {
        code,
        value: Value::Handle(Handle::NULL),
    }
}

/// One column: the parameter it sets (none here), its numbers and names,
/// the two typed values `values`, five flags, the format text and the object
/// it owns (none).
fn column(kind: &str, id: i64, values: &[Field]) -> Vec<Field> {
    let mut fields = vec![
        handle(340),
        int(170, 0),
        int(171, -1),
        text(300, ""),
        text(301, kind),
        int(90, id),
    ];
    fields.extend_from_slice(values);
    fields.extend((290..=294).map(|code| flag(code, code == 291 || code == 292)));
    fields.push(text(302, ""));
    fields.push(handle(340));
    fields
}

/// A table of two columns and three rows. The first column has an empty
/// value and a text one; the second has two empty values. The last row's
/// size is empty.
fn table_fields() -> Vec<Field> {
    let mut fields = vec![int(90, 2), text(300, "Sizes"), text(301, ""), int(91, 2)];
    fields.extend(column(
        "UserVariable",
        3,
        &[int(170, -9999), int(170, 1), text(300, "Custom")],
    ));
    fields.extend(column(
        "UpdatedDistance",
        4,
        &[int(170, -9999), int(170, -9999)],
    ));
    fields.push(int(92, 3));
    for (row, name, size) in [
        (11, "Small", Some(12.0)),
        (12, "Medium", Some(18.5)),
        (13, "Unsized", None),
    ] {
        fields.extend([int(90, row), int(170, 1), text(300, name)]);
        match size {
            Some(size) => fields.extend([
                int(170, 40),
                Field {
                    code: 140,
                    value: Value::Real(size),
                },
            ]),
            None => fields.push(int(170, -9999)),
        }
    }
    fields.extend([
        int(93, 14),
        flag(290, false),
        flag(291, true),
        flag(292, false),
    ]);
    fields
}

fn drawing(version: DxfVersion, fields: Vec<Field>) -> CadDocument {
    let mut doc = CadDocument::with_version(version);
    let root = doc.header.named_objects_dict_handle;
    let handle = doc.allocate_handle();
    let mut object = DynamicBlockObject::new("BLOCKPROPERTIESTABLE", "AcDbBlockPropertiesTable");
    object.handle = handle;
    object.owner = root;
    object.data = DynamicBlockData::PropertiesTable(BlockPropertiesTable {
        fields,
        ..Default::default()
    });
    doc.objects.insert(handle, ObjectType::DynamicBlock(object));
    if let Some(ObjectType::Dictionary(named)) = doc.objects.get_mut(&root) {
        named.add_entry("SIZES", handle);
    }
    doc
}

/// The groups of the one properties table in DWG bytes, as read.
fn fields_of(dwg: Vec<u8>) -> Vec<Field> {
    let doc = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
    doc.objects
        .values()
        .find_map(|object| match object {
            ObjectType::DynamicBlock(DynamicBlockObject {
                data: DynamicBlockData::PropertiesTable(table),
                ..
            }) => Some(table.fields.clone()),
            _ => None,
        })
        .expect("the properties table is read as one")
}

#[test]
fn a_properties_table_is_read_with_its_columns_typed_values() {
    for version in [DxfVersion::AC1024, DxfVersion::AC1032] {
        let dwg = DwgWriter::write_to_vec(&drawing(version, table_fields())).unwrap();
        assert_eq!(fields_of(dwg), table_fields(), "{version:?}");
    }
}

/// A table whose cells all have a value: the column's text value is the
/// only thing the reader has to find, and the strings after it (the row
/// names) stay where they are.
#[test]
fn a_columns_text_value_does_not_shift_the_strings_after_it() {
    let mut fields = table_fields();
    let empty = fields
        .iter()
        .rposition(|field| *field == int(170, -9999))
        .unwrap();
    fields.splice(
        empty..=empty,
        [
            int(170, 40),
            Field {
                code: 140,
                value: Value::Real(20.0),
            },
        ],
    );
    let dwg = DwgWriter::write_to_vec(&drawing(DxfVersion::AC1032, fields.clone())).unwrap();
    let back = fields_of(dwg);
    let names = |fields: &[Field]| -> Vec<String> {
        fields
            .iter()
            .filter_map(|field| match (&field.code, &field.value) {
                (300, Value::Text(name)) => Some(name.clone()),
                _ => None,
            })
            .collect()
    };
    assert_eq!(
        names(&back),
        ["Sizes", "", "Custom", "", "Small", "Medium", "Unsized"]
    );
    assert_eq!(back, fields);
}

/// A row cell with no value (type -9999) is read as such, and the rows after
/// it too.
#[test]
fn an_empty_cell_does_not_end_the_table() {
    let mut fields = table_fields();
    // Make the second row's size the empty one, so a row follows it.
    let at = fields
        .iter()
        .position(|field| field.code == 140 && field.value == Value::Real(18.5))
        .unwrap();
    fields.splice(at - 1..=at, [int(170, -9999)]);
    let dwg = DwgWriter::write_to_vec(&drawing(DxfVersion::AC1032, fields.clone())).unwrap();
    assert_eq!(fields_of(dwg), fields);
}
