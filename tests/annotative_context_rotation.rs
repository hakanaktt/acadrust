//! The rotation of an annotative block or text representation is in radians in
//! memory, like `Insert::rotation` and what DWG stores, and in degrees in DXF
//! group 50.

use std::io::Cursor;

use acadrust::objects::{ObjectContextData, ObjectContextKind, ObjectType, Scale};
use acadrust::types::{DxfVersion, Vector2, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};

const BLKREF: &str = "ACDB_BLKREFOBJECTCONTEXTDATA_CLASS";
const TEXT: &str = "ACDB_TEXTOBJECTCONTEXTDATA_CLASS";

fn blkref() -> ObjectContextKind {
    ObjectContextKind::BlkRef {
        rotation: 30f64.to_radians(),
        insertion: Vector3::new(10.0, 10.0, 0.0),
        scale_factor: Vector3::new(50.0, 50.0, 50.0),
    }
}

fn text() -> ObjectContextKind {
    ObjectContextKind::Text {
        horizontal_mode: 0,
        rotation: 30f64.to_radians(),
        insertion: Vector2::new(20.0, 10.0),
        alignment: Vector2::new(0.0, 0.0),
    }
}

fn dxf_with(kind: ObjectContextKind) -> Vec<u8> {
    let mut doc = CadDocument::with_version(DxfVersion::AC1027);
    let class_name = match kind {
        ObjectContextKind::BlkRef { .. } => BLKREF,
        _ => TEXT,
    };
    doc.register_object_context_class(class_name);
    let scale = doc.allocate_handle();
    let mut leaf_scale = Scale::new("1:50", 1.0, 50.0);
    leaf_scale.handle = scale;
    doc.objects.insert(scale, ObjectType::Scale(leaf_scale));
    let owner = doc.allocate_handle();
    let handle = doc.allocate_handle();
    doc.objects.insert(
        handle,
        ObjectType::ObjectContextData(ObjectContextData {
            handle,
            owner_handle: owner,
            reactors: vec![owner],
            xdictionary_handle: None,
            class_version: 4,
            has_file_to_extension_dictionary: true,
            uses_legacy_mleader_context_base: false,
            is_default: true,
            scale,
            kind,
        }),
    );
    DxfWriter::new(&doc).write_to_vec().expect("DXF write")
}

/// The value line of group 50 in the object of `class`, which is the one
/// record of that name with group 0: the CLASSES section names it in group 1.
fn group_50_line(lines: &[String], class: &str) -> usize {
    let start = (1..lines.len())
        .find(|&at| lines[at].trim() == class && lines[at - 1].trim() == "0")
        .unwrap_or_else(|| panic!("no {class} object in the DXF"));
    (start + 1..lines.len() - 1)
        .step_by(2)
        .take_while(|&code| lines[code].trim() != "0")
        .find(|&code| lines[code].trim() == "50")
        .map(|code| code + 1)
        .unwrap_or_else(|| panic!("no group 50 in {class}"))
}

fn dxf_lines(bytes: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(bytes)
        .lines()
        .map(str::to_string)
        .collect()
}

fn written_degrees(kind: ObjectContextKind, class: &str) -> f64 {
    let lines = dxf_lines(&dxf_with(kind));
    lines[group_50_line(&lines, class)].trim().parse().unwrap()
}

/// What a DXF holding `degrees` in group 50 reads back as.
fn rotation_read(kind: ObjectContextKind, class: &str, degrees: f64) -> f64 {
    let mut lines = dxf_lines(&dxf_with(kind));
    let line = group_50_line(&lines, class);
    lines[line] = degrees.to_string();
    let doc = DxfReader::from_reader(Cursor::new(lines.join("\n").into_bytes()))
        .expect("DXF reader init")
        .read()
        .expect("DXF read");
    doc.objects
        .values()
        .find_map(|object| match object {
            ObjectType::ObjectContextData(context) => match context.kind {
                ObjectContextKind::BlkRef { rotation, .. }
                | ObjectContextKind::Text { rotation, .. } => Some(rotation),
                _ => None,
            },
            _ => None,
        })
        .expect("the context reads back")
}

fn assert_near(got: f64, expected: f64) {
    assert!((got - expected).abs() < 1e-9, "{got} != {expected}");
}

#[test]
fn a_block_representation_writes_its_rotation_in_degrees() {
    assert_near(written_degrees(blkref(), BLKREF), 30.0);
}

#[test]
fn a_text_representation_writes_its_rotation_in_degrees() {
    assert_near(written_degrees(text(), TEXT), 30.0);
}

#[test]
fn a_block_representation_reads_its_rotation_from_degrees() {
    assert_near(rotation_read(blkref(), BLKREF, 90.0), 90f64.to_radians());
}

#[test]
fn a_text_representation_reads_its_rotation_from_degrees() {
    assert_near(rotation_read(text(), TEXT, 90.0), 90f64.to_radians());
}
