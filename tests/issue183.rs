//! `BLOCKPROPERTIESTABLE` tail layout (issue #183).
//!
//! In `AcDbBlockPropertiesTable`, DXF group 93 is a plain `BL` integer (the
//! next row ID counter), followed by three fixed boolean flags in groups
//! 290..=292 (`mustMatch`, `runtimeParametersOnly`, `disabledInDrawingEditor`).
//! Treating group 93 as the number of trailing boolean flags fails the strict
//! DWG read when 93 exceeds the remaining bits in the record, drops flags when
//! 93 < 3, and fabricates extra flags from byte padding when 3 < 93 <= 8.

use acadrust::{
    objects::{
        BlockPropertiesTable, BlockPropertiesTableField, BlockPropertiesTableValue,
        DynamicBlockData, ObjectType,
    },
    types::Handle,
    CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter,
};
use std::io::Cursor;

const TABLE_HANDLE: u64 = 0x25C;

fn document_with_table(
    column_count: i32,
    row_count: i32,
    group_93: i32,
    tail_flags: [bool; 3],
) -> CadDocument {
    let mut doc = DwgReader::from_stream(Cursor::new(
        include_bytes!("fixtures/dynamic-blocks/point.dwg").as_slice(),
    ))
    .read()
    .unwrap();

    let mut fields = vec![
        BlockPropertiesTableField {
            code: 90,
            value: BlockPropertiesTableValue::Integer(2),
        },
        BlockPropertiesTableField {
            code: 300,
            value: BlockPropertiesTableValue::Text("Issue183Table".into()),
        },
        BlockPropertiesTableField {
            code: 301,
            value: BlockPropertiesTableValue::Text(String::new()),
        },
        BlockPropertiesTableField {
            code: 91,
            value: BlockPropertiesTableValue::Integer(i64::from(column_count)),
        },
    ];

    for col in 0..column_count {
        fields.extend([
            BlockPropertiesTableField {
                code: 340,
                value: BlockPropertiesTableValue::Handle(Handle::NULL),
            },
            BlockPropertiesTableField {
                code: 170,
                value: BlockPropertiesTableValue::Integer(0),
            },
            BlockPropertiesTableField {
                code: 171,
                value: BlockPropertiesTableValue::Integer(-1),
            },
            BlockPropertiesTableField {
                code: 300,
                value: BlockPropertiesTableValue::Text(String::new()),
            },
            BlockPropertiesTableField {
                code: 301,
                value: BlockPropertiesTableValue::Text(format!("Col{col}")),
            },
            BlockPropertiesTableField {
                code: 90,
                value: BlockPropertiesTableValue::Integer(14 + i64::from(col)),
            },
            BlockPropertiesTableField {
                code: 170,
                value: BlockPropertiesTableValue::Integer(-9999),
            },
            BlockPropertiesTableField {
                code: 170,
                value: BlockPropertiesTableValue::Integer(-9999),
            },
            BlockPropertiesTableField {
                code: 290,
                value: BlockPropertiesTableValue::Boolean(false),
            },
            BlockPropertiesTableField {
                code: 291,
                value: BlockPropertiesTableValue::Boolean(true),
            },
            BlockPropertiesTableField {
                code: 292,
                value: BlockPropertiesTableValue::Boolean(true),
            },
            BlockPropertiesTableField {
                code: 293,
                value: BlockPropertiesTableValue::Boolean(col % 2 == 1),
            },
            BlockPropertiesTableField {
                code: 294,
                value: BlockPropertiesTableValue::Boolean(false),
            },
            BlockPropertiesTableField {
                code: 302,
                value: BlockPropertiesTableValue::Text(String::new()),
            },
            BlockPropertiesTableField {
                code: 340,
                value: BlockPropertiesTableValue::Handle(Handle::NULL),
            },
        ]);
    }

    fields.push(BlockPropertiesTableField {
        code: 92,
        value: BlockPropertiesTableValue::Integer(i64::from(row_count)),
    });

    for row in 0..row_count {
        fields.push(BlockPropertiesTableField {
            code: 90,
            value: BlockPropertiesTableValue::Integer(i64::from(row)),
        });
        for col in 0..column_count {
            fields.push(BlockPropertiesTableField {
                code: 170,
                value: BlockPropertiesTableValue::Integer(40),
            });
            fields.push(BlockPropertiesTableField {
                code: 140,
                value: BlockPropertiesTableValue::Real(f64::from(row * 10 + col + 1)),
            });
        }
    }

    fields.push(BlockPropertiesTableField {
        code: 93,
        value: BlockPropertiesTableValue::Integer(i64::from(group_93)),
    });
    for (index, flag) in tail_flags.into_iter().enumerate() {
        fields.push(BlockPropertiesTableField {
            code: 290 + index as i32,
            value: BlockPropertiesTableValue::Boolean(flag),
        });
    }

    let handle = Handle::new(TABLE_HANDLE);
    let ObjectType::DynamicBlock(object) = doc.objects.get_mut(&handle).unwrap() else {
        panic!("expected dynamic block object at {handle:?}");
    };
    object.dxf_name = "BLOCKPROPERTIESTABLE".into();
    object.cpp_class_name = "AcDbBlockPropertiesTable".into();
    object.data = DynamicBlockData::PropertiesTable(BlockPropertiesTable {
        parameter: Default::default(),
        fields,
    });

    let mut class = doc
        .classes
        .get_by_name("BLOCKPOINTPARAMETER")
        .unwrap()
        .clone();
    class.dxf_name = "BLOCKPROPERTIESTABLE".into();
    class.cpp_class_name = "AcDbBlockPropertiesTable".into();
    doc.classes.add_or_update(class);

    doc
}

fn table_data(doc: &CadDocument) -> &BlockPropertiesTable {
    match &doc.objects[&Handle::new(TABLE_HANDLE)] {
        ObjectType::DynamicBlock(object) => match &object.data {
            DynamicBlockData::PropertiesTable(table) => table,
            other => panic!("expected PropertiesTable, got {other:?}"),
        },
        other => panic!("expected DynamicBlock, got {other:?}"),
    }
}

#[test]
fn three_columns_ten_rows_with_group_93_equal_to_10_reads_strictly() {
    let doc = document_with_table(3, 10, 10, [false, true, false]);

    let dwg = DwgWriter::write_to_vec(&doc).expect("write dwg");
    let from_dwg = DwgReader::from_stream(Cursor::new(dwg))
        .read()
        .expect("strict DWG read of 3x10 BLOCKPROPERTIESTABLE with 93=10");
    assert_eq!(table_data(&from_dwg), table_data(&doc));

    let dxf = DxfWriter::new(&from_dwg).write_to_vec().expect("write dxf");
    let from_dxf = DxfReader::from_reader(Cursor::new(dxf))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    assert_eq!(table_data(&from_dxf), table_data(&doc));
}

#[test]
fn group_93_less_than_three_still_reads_all_three_tail_flags() {
    for group_93 in [0, 1, 2] {
        let doc = document_with_table(2, 2, group_93, [true, false, true]);
        let dwg = DwgWriter::write_to_vec(&doc).expect("write dwg");
        let decoded = DwgReader::from_stream(Cursor::new(dwg))
            .read()
            .expect("read dwg");
        let tail = &table_data(&decoded).fields[table_data(&decoded).fields.len() - 4..];
        assert_eq!(
            tail,
            &[
                BlockPropertiesTableField {
                    code: 93,
                    value: BlockPropertiesTableValue::Integer(i64::from(group_93)),
                },
                BlockPropertiesTableField {
                    code: 290,
                    value: BlockPropertiesTableValue::Boolean(true),
                },
                BlockPropertiesTableField {
                    code: 291,
                    value: BlockPropertiesTableValue::Boolean(false),
                },
                BlockPropertiesTableField {
                    code: 292,
                    value: BlockPropertiesTableValue::Boolean(true),
                },
            ],
            "group 93 = {group_93} must still preserve all three 290..=292 flags"
        );
        assert_eq!(table_data(&decoded), table_data(&doc));
    }
}

#[test]
fn group_93_within_trailing_padding_bits_does_not_fabricate_extra_flags() {
    let doc = document_with_table(2, 4, 4, [true, true, false]);
    let dwg = DwgWriter::write_to_vec(&doc).expect("write dwg");
    let decoded = DwgReader::from_stream(Cursor::new(dwg))
        .read()
        .expect("read dwg");
    assert!(
        !table_data(&decoded)
            .fields
            .iter()
            .skip_while(|f| f.code != 93)
            .any(|f| f.code > 292),
        "tail after group 93 must only contain 290..=292"
    );
    assert_eq!(table_data(&decoded), table_data(&doc));
}

#[test]
fn negative_or_large_group_93_is_preserved_as_an_integer_value() {
    for group_93 in [-1, 1000] {
        let doc = document_with_table(1, 2, group_93, [false, false, true]);
        let dwg = DwgWriter::write_to_vec(&doc).expect("write dwg");
        let decoded = DwgReader::from_stream(Cursor::new(dwg))
            .read()
            .expect("strict read must accept any BL in group 93");
        assert_eq!(table_data(&decoded), table_data(&doc));
    }
}
