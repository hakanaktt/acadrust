//! An image definition lists its owner dictionary and the IMAGEDEF_REACTOR of
//! each image that uses it, and RASTERVARIABLES lists the named object
//! dictionary (DXF Reference: IMAGEDEF, RASTERVARIABLES). AutoCAD's External
//! References palette shows an image whose definition lists no reactor as
//! unreferenced, even though it draws it.

use std::io::Cursor;

use acadrust::entities::EntityType;
use acadrust::objects::ObjectType;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, Handle};

/// Two images sharing one definition, written by an application that lists
/// the reactors, as AutoCAD does.
fn source_dxf() -> String {
    let mut pairs: Vec<(i32, &str)> = vec![
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, "AC1015"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
    ];
    for (handle, reactor, x) in [("A0", "A2", "0.0"), ("A1", "A3", "20.0")] {
        pairs.extend([
            (0, "IMAGE"),
            (5, handle),
            (100, "AcDbEntity"),
            (8, "0"),
            (100, "AcDbRasterImage"),
            (90, "0"),
            (10, x),
            (20, "0.0"),
            (30, "0.0"),
            (11, "0.1"),
            (21, "0.0"),
            (31, "0.0"),
            (12, "0.0"),
            (22, "0.1"),
            (32, "0.0"),
            (13, "100.0"),
            (23, "50.0"),
            (340, "B0"),
            (70, "15"),
            (280, "0"),
            (281, "50"),
            (282, "50"),
            (283, "0"),
            (360, reactor),
        ]);
    }
    pairs.extend([
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "OBJECTS"),
        (0, "DICTIONARY"),
        (5, "C"),
        (330, "0"),
        (100, "AcDbDictionary"),
        (281, "1"),
        (3, "ACAD_IMAGE_DICT"),
        (350, "B1"),
        (3, "ACAD_IMAGE_VARS"),
        (350, "B4"),
        (0, "DICTIONARY"),
        (5, "B1"),
        (330, "C"),
        (100, "AcDbDictionary"),
        (281, "1"),
        (3, "picture"),
        (350, "B0"),
        (0, "IMAGEDEF"),
        (5, "B0"),
        (102, "{ACAD_REACTORS"),
        (330, "B1"),
        (330, "A2"),
        (330, "A3"),
        (102, "}"),
        (330, "B1"),
        (100, "AcDbRasterImageDef"),
        (90, "0"),
        (1, "C:\\pictures\\picture.png"),
        (10, "100.0"),
        (20, "50.0"),
        (11, "0.01"),
        (21, "0.01"),
        (280, "1"),
        (281, "0"),
    ]);
    for (reactor, image) in [("A2", "A0"), ("A3", "A1")] {
        pairs.extend([
            (0, "IMAGEDEF_REACTOR"),
            (5, reactor),
            (330, image),
            (100, "AcDbRasterImageDefReactor"),
            (90, "2"),
            (330, image),
        ]);
    }
    pairs.extend([
        (0, "RASTERVARIABLES"),
        (5, "B4"),
        (102, "{ACAD_REACTORS"),
        (330, "C"),
        (102, "}"),
        (330, "C"),
        (100, "AcDbRasterVariables"),
        (90, "0"),
        (70, "1"),
        (71, "1"),
        (72, "0"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ]);
    pairs
        .iter()
        .map(|(code, value)| format!("{code}\n{value}\n"))
        .collect()
}

fn read_dxf(text: &[u8]) -> CadDocument {
    DxfReader::from_reader(Cursor::new(text.to_vec()))
        .unwrap()
        .read()
        .unwrap()
}

/// The `{ACAD_REACTORS` handles of the object of type `kind` in DXF `text`.
fn dxf_reactors(text: &str, kind: &str) -> Vec<String> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let pairs: Vec<(&str, &str)> = lines
        .chunks(2)
        .map(|p| (p[0], p.get(1).copied().unwrap_or("")))
        .collect();
    let start = pairs
        .iter()
        .position(|p| *p == ("0", kind))
        .unwrap_or_else(|| panic!("no {kind}"));
    let mut reactors = Vec::new();
    let mut inside = false;
    for &(code, value) in &pairs[start + 1..] {
        match (code, value) {
            ("0", _) => break,
            ("102", "{ACAD_REACTORS") => inside = true,
            ("102", "}") => inside = false,
            ("330", handle) if inside => reactors.push(handle.to_string()),
            _ => {}
        }
    }
    reactors
}

/// The image definition's owner and the reactors of the images using it.
fn expected(doc: &CadDocument) -> (Vec<String>, Vec<String>) {
    let hex = |h: Handle| format!("{:X}", h.value());
    let definition = doc
        .objects
        .values()
        .find_map(|o| match o {
            ObjectType::ImageDefinition(d) => Some(d),
            _ => None,
        })
        .unwrap();
    let mut imagedef = vec![hex(definition.owner)];
    imagedef.extend(doc.entities().filter_map(|e| match e {
        EntityType::RasterImage(i) => i.definition_reactor_handle.map(hex),
        _ => None,
    }));
    let variables = doc
        .objects
        .values()
        .find_map(|o| match o {
            ObjectType::RasterVariables(v) => Some(v),
            _ => None,
        })
        .unwrap();
    (imagedef, vec![hex(variables.owner)])
}

#[test]
fn dxf_output_lists_image_definition_and_raster_variables_reactors() {
    let doc = read_dxf(source_dxf().as_bytes());
    let (imagedef, variables) = expected(&doc);
    assert_eq!(imagedef.len(), 3, "the dictionary and two reactors");
    let text = String::from_utf8(DxfWriter::new(&doc).write_to_vec().unwrap()).unwrap();
    assert_eq!(dxf_reactors(&text, "IMAGEDEF"), imagedef);
    assert_eq!(dxf_reactors(&text, "RASTERVARIABLES"), variables);
}

#[test]
fn dwg_output_lists_image_definition_and_raster_variables_reactors() {
    let doc = read_dxf(source_dxf().as_bytes());
    let dwg = DwgWriter::write_to_vec(&doc).unwrap();
    let back = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
    let (imagedef, variables) = expected(&back);
    assert_eq!(imagedef.len(), 3, "the dictionary and two reactors");
    // The DWG's reactors, as read back and written out again.
    let text = String::from_utf8(DxfWriter::new(&back).write_to_vec().unwrap()).unwrap();
    assert_eq!(dxf_reactors(&text, "IMAGEDEF"), imagedef);
    assert_eq!(dxf_reactors(&text, "RASTERVARIABLES"), variables);
}
