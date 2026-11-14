//! A `BREAKDATA` break point reference (`AcDbBreakPointRef`) carries a
//! sub-entity path: the object (`331`), the sub-entity type (`73`) and the
//! sub-entity index (`92`), written after the second point (`11`) as ODA
//! File Converter writes it for AutoCAD-saved drawings. The DXF reader and
//! writer keep that path, and the `331` of a reference does not take the
//! place of the dimension (`330`).

use std::io::Cursor;

use acadrust::entities::{EntityType, Line};
use acadrust::objects::{BreakData, BreakPointReference, DataObject, DataObjectData, ObjectType};
use acadrust::types::{Handle, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};

/// Each reference's sub-entity path as written into the DXF: the object's
/// index in `Fixture::objects`, then the `73` and `92` values.
const PATHS: [(usize, i32, i32); 2] = [(0, 1, 0), (1, 2, 3)];

struct Fixture {
    document: CadDocument,
    dimension: Handle,
    objects: [Handle; 2],
}

/// A document with a `BREAKDATA` that holds two point references with
/// distinct values. The dimension and the referenced objects are lines; the
/// test only needs live handles.
fn fixture() -> Fixture {
    let mut document = CadDocument::new();
    let mut line = |x: f64| {
        document
            .add_entity(EntityType::Line(Line::from_points(
                Vector3::new(x, 0.0, 0.0),
                Vector3::new(x, 1.0, 0.0),
            )))
            .unwrap()
    };
    let dimension = line(0.0);
    let objects = [line(1.0), line(2.0)];

    let mut object = DataObject::new(DataObjectData::BreakData(BreakData {
        version: 1,
        dimension_reference: dimension,
        point_references: vec![
            BreakPointReference {
                reference_type: 0,
                flags: 0,
                identifier: -1,
                first_point: Vector3::new(0.0, 0.0, 0.0),
                second_point: Vector3::new(1.0, 1.0, 0.0),
                ..BreakPointReference::default()
            },
            BreakPointReference {
                reference_type: 2,
                flags: 1,
                identifier: 5,
                first_point: Vector3::new(1.5, 2.5, 0.0),
                second_point: Vector3::new(3.5, 4.5, 0.0),
                ..BreakPointReference::default()
            },
        ],
        ..BreakData::default()
    }));
    object.handle = document.allocate_handle();
    document
        .objects
        .insert(object.handle, ObjectType::DataObject(object));
    Fixture {
        document,
        dimension,
        objects,
    }
}

fn hex(handle: Handle) -> String {
    format!("{:X}", handle.value())
}

/// The group code and value pairs of the `BREAKDATA` object in a DXF text,
/// from its `AcDbBreakData` subclass marker to the next object.
fn breakdata_pairs(text: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(text);
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let start = lines
        .chunks(2)
        .position(|pair| pair == ["100", "AcDbBreakData"])
        .expect("a BREAKDATA object in the DXF");
    lines[start * 2..]
        .chunks(2)
        .take_while(|pair| pair[0] != "0")
        .map(|pair| (pair[0].to_string(), pair[1].to_string()))
        .collect()
}

/// The pairs of each `AcDbBreakPointRef` of a `BREAKDATA`, without the
/// subclass marker.
fn point_references(pairs: &[(String, String)]) -> Vec<Vec<(String, String)>> {
    let mut references: Vec<Vec<(String, String)>> = Vec::new();
    for pair in pairs {
        if pair.0 == "100" && pair.1 == "AcDbBreakPointRef" {
            references.push(Vec::new());
        } else if let Some(reference) = references.last_mut() {
            reference.push(pair.clone());
        }
    }
    references
}

fn codes(pairs: &[(String, String)]) -> Vec<&str> {
    pairs.iter().map(|pair| pair.0.as_str()).collect()
}

fn value<'a>(pairs: &'a [(String, String)], code: &str) -> &'a str {
    &pairs
        .iter()
        .find(|pair| pair.0 == code)
        .unwrap_or_else(|| panic!("group {code} in {pairs:?}"))
        .1
}

/// The fixture written to DXF, with the sub-entity path of each
/// `AcDbBreakPointRef` set from `PATHS`: `331`, `73` and `92` after the `31`
/// of the second point, in the order ODA writes them. A path the writer
/// already wrote is replaced.
fn dxf_with_paths(fixture: &Fixture) -> Vec<u8> {
    let text = DxfWriter::new(&fixture.document).write_to_vec().unwrap();
    let text = String::from_utf8(text).unwrap();
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut paths = PATHS.iter();
    let mut in_breakdata = false;
    let mut in_reference = false;
    let mut patched = String::new();
    for pair in lines.chunks(2) {
        let (code, value) = (pair[0], pair.get(1).copied().unwrap_or(""));
        match (code.trim(), value.trim()) {
            ("100", "AcDbBreakData") => in_breakdata = true,
            ("0", _) => (in_breakdata, in_reference) = (false, false),
            ("100", "AcDbBreakPointRef") if in_breakdata => in_reference = true,
            ("331" | "73" | "92", _) if in_reference => continue,
            _ => {}
        }
        patched.push_str(code);
        patched.push_str(value);
        if in_reference && code.trim() == "31" {
            let (object, subentity_type, subentity_index) =
                paths.next().expect("one path per reference");
            patched.push_str(&format!(
                "331\n{}\n 73\n{subentity_type}\n 92\n{subentity_index}\n",
                hex(fixture.objects[*object])
            ));
        }
    }
    assert!(paths.next().is_none(), "every reference got its path");
    patched.into_bytes()
}

fn read_dxf(bytes: Vec<u8>) -> CadDocument {
    DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap()
}

fn break_data(document: &CadDocument) -> &BreakData {
    document
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::DataObject(DataObject {
                data: DataObjectData::BreakData(value),
                ..
            }) => Some(value),
            _ => None,
        })
        .expect("a BREAKDATA object in the document")
}

/// A BREAKDATA read from DXF writes each reference's sub-entity path back,
/// after the second point, with the values it was read with.
#[test]
fn dxf_round_trip_keeps_subentity_path() {
    let fixture = fixture();
    let document = read_dxf(dxf_with_paths(&fixture));
    let written = DxfWriter::new(&document).write_to_vec().unwrap();

    let pairs = breakdata_pairs(&written);
    let references = point_references(&pairs);
    assert_eq!(references.len(), 2, "both point references are written");
    for (reference, (object, subentity_type, subentity_index)) in references.iter().zip(PATHS) {
        assert_eq!(
            codes(reference),
            ["71", "72", "91", "10", "20", "30", "11", "21", "31", "331", "73", "92"],
            "group codes of the point reference"
        );
        assert_eq!(value(reference, "331"), hex(fixture.objects[object]));
        assert_eq!(value(reference, "73"), subentity_type.to_string());
        assert_eq!(value(reference, "92"), subentity_index.to_string());
    }
}

/// The `331` of a point reference names the reference's object; reading it
/// leaves the BREAKDATA's dimension (`330`) in place.
#[test]
fn dxf_reader_keeps_dimension_beside_subentity_objects() {
    let fixture = fixture();
    let document = read_dxf(dxf_with_paths(&fixture));
    assert_eq!(break_data(&document).dimension_reference, fixture.dimension);

    let written = DxfWriter::new(&document).write_to_vec().unwrap();
    let pairs = breakdata_pairs(&written);
    assert_eq!(value(&pairs, "330"), hex(fixture.dimension));
}

/// Control: the rest of each point reference reads the same with and
/// without a sub-entity path.
#[test]
fn point_reference_fields_read_beside_subentity_path() {
    let fixture = fixture();
    let expected = &break_data(&fixture.document).point_references;
    let document = read_dxf(dxf_with_paths(&fixture));
    let read = &break_data(&document).point_references;
    assert_eq!(read.len(), expected.len());
    for (read, expected) in read.iter().zip(expected) {
        assert_eq!(read.reference_type, expected.reference_type);
        assert_eq!(read.flags, expected.flags);
        assert_eq!(read.identifier, expected.identifier);
        assert_eq!(read.first_point, expected.first_point);
        assert_eq!(read.second_point, expected.second_point);
    }
}
