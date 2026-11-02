//! Dimension object context data stores its block by NAME in DXF group 2.
//!
//! AutoCAD writes `2 *D1` in AcDbDimensionObjectContextData and refuses a
//! DXF with a hex handle there ("Invalid dimension block name"). The writer
//! wrote the handle, and the reader parsed only the hex form, so AutoCAD's
//! own files read with a null block.

use acadrust::objects::{
    DimContext, DimSubtype, ObjectContextData, ObjectContextKind, ObjectType, Scale,
};
use acadrust::tables::BlockRecord;
use acadrust::types::{DxfVersion, Vector2, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter, Handle};
use std::io::Cursor;

fn document() -> (CadDocument, Handle, Handle) {
    let mut doc = CadDocument::with_version(DxfVersion::AC1027);
    doc.register_object_context_class("ACDB_ALDIMOBJECTCONTEXTDATA_CLASS");

    let mut record = BlockRecord::new("*D1");
    record.handle = doc.allocate_handle();
    record.block_entity_handle = doc.allocate_handle();
    record.block_end_handle = doc.allocate_handle();
    let block = record.handle;
    doc.block_records.add(record).unwrap();

    let scale_h = doc.allocate_handle();
    let mut scale = Scale::new("1:50", 1.0, 50.0);
    scale.handle = scale_h;
    doc.objects.insert(scale_h, ObjectType::Scale(scale));

    let leaf = doc.allocate_handle();
    let owner = doc.header.named_objects_dict_handle;
    let ctx = ObjectContextData {
        handle: leaf,
        owner_handle: owner,
        reactors: vec![owner],
        xdictionary_handle: None,
        class_version: 3,
        has_file_to_extension_dictionary: false,
        uses_legacy_mleader_context_base: false,
        is_default: true,
        scale: scale_h,
        kind: ObjectContextKind::Dim(DimContext {
            def_pt: Vector2::new(5.0, 5.0),
            is_def_textloc: true,
            text_rotation: 0.0,
            block,
            b293: false,
            dimtofl: false,
            dimosxd: false,
            dimatfit: false,
            dimtix: false,
            dimtmove: false,
            override_code: 0,
            has_arrow2: false,
            flip_arrow2: false,
            flip_arrow1: false,
            subtype: DimSubtype::Aligned {
                dimline_pt: Vector3::new(0.0, 10.0, 0.0),
            },
        }),
    };
    doc.objects.insert(leaf, ObjectType::ObjectContextData(ctx));
    (doc, leaf, block)
}

fn write(doc: &CadDocument) -> String {
    String::from_utf8(DxfWriter::new(doc).write_to_vec().expect("write dxf")).unwrap()
}

fn read(text: &str) -> CadDocument {
    DxfReader::from_reader(Cursor::new(text.as_bytes().to_vec()))
        .expect("open dxf")
        .read()
        .expect("read dxf")
}

/// Line index of the group-2 value inside the context record `leaf`.
fn group2_line(text: &str, leaf: Handle) -> usize {
    let lines: Vec<&str> = text.lines().collect();
    let hex = format!("{:X}", leaf.value());
    let start = (0..lines.len() - 3)
        .find(|&i| {
            lines[i].trim() == "0"
                && lines[i + 1].trim() == "ACDB_ALDIMOBJECTCONTEXTDATA_CLASS"
                && lines[i + 3].trim() == hex
        })
        .expect("context record");
    (start + 2..lines.len() - 1)
        .step_by(2)
        .take_while(|&i| lines[i].trim() != "0")
        .find(|&i| lines[i].trim() == "2")
        .map(|i| i + 1)
        .expect("group 2")
}

fn block_of(doc: &CadDocument, leaf: Handle) -> Handle {
    match doc.objects.get(&leaf) {
        Some(ObjectType::ObjectContextData(ObjectContextData {
            kind: ObjectContextKind::Dim(d),
            ..
        })) => d.block,
        other => panic!("context lost: {other:?}"),
    }
}

/// `text` with the context record's group-2 value replaced by `value`.
fn with_group2(text: &str, leaf: Handle, value: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    lines[group2_line(text, leaf)] = value.to_owned();
    lines.join("\r\n") + "\r\n"
}

#[test]
fn dim_context_writes_the_block_name() {
    let (doc, leaf, _) = document();
    let text = write(&doc);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines[group2_line(&text, leaf)].trim(), "*D1");
}

#[test]
fn dim_context_reads_autocads_block_name_and_the_old_hex_handle() {
    let (doc, leaf, block) = document();
    let text = write(&doc);
    // AutoCAD's layout: the block name.
    assert_eq!(
        block_of(&read(&with_group2(&text, leaf, "*D1")), leaf),
        block
    );
    // Files written by older acadrust: a hex handle.
    let hex = format!("{:X}", block.value());
    assert_eq!(
        block_of(&read(&with_group2(&text, leaf, &hex)), leaf),
        block
    );
}
