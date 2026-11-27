//! A dynamic block's rotate action (BLOCKROTATEACTION) is an object of the
//! class `AcDbBlockRotateAction`. `AcDbBlockRotationAction` is the subclass
//! marker of the action's DXF record, not its class name.
//!
//! The default class table and the DXF reader's name table both gave the
//! class the marker's name, and a class read from a DXF does not replace an
//! entry of the default table, so every DWG written by the library declared
//! BLOCKROTATEACTION under the marker's name, whatever the source file said.

use acadrust::{
    objects::{
        BlockActionWithBasePoint, BlockBasePointAction, BlockConnection, DynamicBlockData,
        DynamicBlockObject, ObjectType,
    },
    types::DxfVersion,
    CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter,
};
use std::io::Cursor;

const CLASS: &str = "AcDbBlockRotateAction";

fn connection(name: &str) -> BlockConnection {
    BlockConnection {
        code: 0,
        name: name.into(),
    }
}

/// A drawing with one rotate action under the named object dictionary.
fn drawing(version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::with_version(version);
    let handle = doc.allocate_handle();
    let root = doc.header.named_objects_dict_handle;
    let mut object = DynamicBlockObject::new("BLOCKROTATEACTION", CLASS);
    object.handle = handle;
    object.owner = root;
    // Two connections for the base point and one for the angle.
    object.data = DynamicBlockData::RotateAction(BlockBasePointAction {
        action: BlockActionWithBasePoint {
            connections: vec![connection("BaseX"), connection("BaseY")],
            ..Default::default()
        },
        connections: vec![connection("Angle")],
        dwg_tail_byte: 0,
    });
    doc.objects.insert(handle, ObjectType::DynamicBlock(object));
    if let Some(ObjectType::Dictionary(named)) = doc.objects.get_mut(&root) {
        named.add_entry("ROTATE", handle);
    }
    doc
}

/// The class name the document's class table gives BLOCKROTATEACTION.
fn class_name(doc: &CadDocument) -> String {
    doc.classes
        .iter()
        .find(|class| class.dxf_name.eq_ignore_ascii_case("BLOCKROTATEACTION"))
        .map(|class| class.cpp_class_name.clone())
        .expect("BLOCKROTATEACTION is in the class table")
}

fn has_rotate_action(doc: &CadDocument) -> bool {
    doc.objects.values().any(|object| {
        matches!(
            object,
            ObjectType::DynamicBlock(DynamicBlockObject {
                data: DynamicBlockData::RotateAction(_),
                ..
            })
        )
    })
}

fn read_dwg(bytes: Vec<u8>) -> CadDocument {
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn read_dxf(text: &str) -> CadDocument {
    DxfReader::from_reader(Cursor::new(text.as_bytes().to_vec()))
        .unwrap()
        .read()
        .unwrap()
}

#[test]
fn a_dwg_declares_the_rotate_action_class_by_its_name() {
    for version in [DxfVersion::AC1024, DxfVersion::AC1032] {
        let back = read_dwg(DwgWriter::write_to_vec(&drawing(version)).unwrap());
        assert!(
            has_rotate_action(&back),
            "{version:?}: the action is read back"
        );
        assert_eq!(class_name(&back), CLASS, "{version:?}");
    }
}

#[test]
fn a_dxf_declares_the_rotate_action_class_by_its_name() {
    let dxf = String::from_utf8(
        DxfWriter::new(&drawing(DxfVersion::AC1032))
            .write_to_vec()
            .unwrap(),
    )
    .unwrap();
    let back = read_dxf(&dxf);
    assert!(has_rotate_action(&back), "the action is read back");
    assert_eq!(class_name(&back), CLASS);
}

/// A DXF whose CLASSES section names the class correctly, as the file a
/// drafting program writes does: the DWG written from it keeps that name,
/// and the object read from it carries it too.
#[test]
fn a_dwg_written_from_a_dxf_keeps_the_class_the_dxf_names() {
    let dxf = String::from_utf8(
        DxfWriter::new(&drawing(DxfVersion::AC1032))
            .write_to_vec()
            .unwrap(),
    )
    .unwrap();
    // Name the class correctly in CLASSES only; the record keeps its
    // subclass marker AcDbBlockRotationAction.
    let mut lines: Vec<String> = dxf.lines().map(str::to_string).collect();
    let at = lines
        .windows(2)
        .position(|pair| pair[0].trim() == "1" && pair[1].trim() == "BLOCKROTATEACTION")
        .expect("BLOCKROTATEACTION is in CLASSES");
    assert_eq!(
        lines[at + 2].trim(),
        "2",
        "the class name follows the DXF name"
    );
    lines[at + 3] = CLASS.to_string();
    let dxf = lines.join("\n");

    let doc = read_dxf(&dxf);
    let object_class = doc.objects.values().find_map(|object| match object {
        ObjectType::DynamicBlock(object)
            if matches!(object.data, DynamicBlockData::RotateAction(_)) =>
        {
            Some(object.cpp_class_name.clone())
        }
        _ => None,
    });
    assert_eq!(
        object_class.as_deref(),
        Some(CLASS),
        "the object read from the DXF"
    );

    for version in [DxfVersion::AC1024, DxfVersion::AC1032] {
        let mut doc = read_dxf(&dxf);
        doc.version = version;
        let back = read_dwg(DwgWriter::write_to_vec(&doc).unwrap());
        assert!(
            has_rotate_action(&back),
            "{version:?}: the action is read back"
        );
        assert_eq!(class_name(&back), CLASS, "{version:?}");
    }
}
