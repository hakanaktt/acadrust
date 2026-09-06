//! Repro for issue #64: a DXF round-trip writes a ByBlock linetype handle
//! as the DIMSTYLE text style (group 340).

use acadrust::entities::EntityType;
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DxfReader, DxfWriter};

#[test]
fn repro_issue64() {
    // Input mirroring the reporter's file: a Standard text style at #11,
    // a ByBlock linetype at #14 (which collides with the handle that
    // acadrust's DEFAULT Standard dimstyle uses for its text style), and
    // no Standard dimstyle of its own (the default one survives).
    let mut doc = CadDocument::with_version(DxfVersion::AC1024);

    // The file's own records replace the defaults at these handles.
    doc.text_styles.get_mut("Standard").unwrap().handle = Handle::new(0x11);
    doc.line_types.get_mut("ByBlock").unwrap().handle = Handle::new(0x14);
    // No Standard DIMSTYLE in the input: the default one survives, still
    // pointing its dimtxsty at the DEFAULT text style handle (0x14) - which
    // the input has now given to the ByBlock linetype.
    doc.add_entity(EntityType::Line(acadrust::entities::Line::from_coords(
        0.0, 0.0, 0.0, 1.0, 1.0, 0.0,
    )))
    .unwrap();

    let input = std::env::temp_dir().join("issue64_input.dxf");
    DxfWriter::new(&doc).write_to_file(&input).unwrap();

    // The round-trip from the issue
    let loaded = DxfReader::from_file(&input).unwrap().read().unwrap();
    let output = std::env::temp_dir().join("issue64_output.dxf");
    DxfWriter::new(&loaded).write_to_file(&output).unwrap();

    let ts_handle = loaded.text_styles.get("Standard").unwrap().handle;
    let ltype_handle = loaded.line_types.get("ByBlock").unwrap().handle;
    let dimtxsty = loaded.dim_styles.get("Standard").unwrap().dimtxsty_handle;
    println!("text style Standard = {ts_handle:?}");
    println!("ByBlock LTYPE       = {ltype_handle:?}");
    println!("DIMSTYLE dimtxsty   = {dimtxsty:?}");
    assert_ne!(
        dimtxsty, ltype_handle,
        "DIMSTYLE text style points at the ByBlock linetype"
    );

    // The output record must reference the text style, not the linetype.
    let text = std::fs::read_to_string(&output).unwrap();
    let lines: Vec<&str> = text.split("\r\n").collect();
    let mut dimstyle_340 = String::new();
    for i in 0..lines.len() - 3 {
        if lines[i] == "DIMSTYLE" && lines[i - 1] == "  0" {
            let mut j = i;
            while j < lines.len() - 1 {
                if lines[j].trim() == "340" {
                    dimstyle_340 = lines[j + 1].trim().to_string();
                    break;
                }
                if lines[j] == "  0" && j > i {
                    break;
                }
                j += 1;
            }
            break;
        }
    }
    println!("output DIMSTYLE 340 -> {dimstyle_340}");
    assert_ne!(
        dimstyle_340,
        format!("{:X}", ltype_handle.value()),
        "DIMSTYLE 340 points at the ByBlock linetype"
    );
}

#[test]
fn issue64_root_dict_handle_resolved_on_read() {
    use acadrust::objects::{Dictionary, ObjectType};

    // The reporter's file keeps its NAMED OBJECTS DICTIONARY at #A while
    // an unrelated dictionary (ACAD_GROUP) sits at #C - the handle the
    // header is seeded with and that the DXF reader used to leave stale.
    // The writer must emit the real root dictionary first in the OBJECTS
    // section or consumers audit away its children as orphans.
    let mut doc = CadDocument::with_version(DxfVersion::AC1024);

    let mut nod = match doc.objects.remove(&Handle::new(0x0C)).unwrap() {
        ObjectType::Dictionary(d) => d,
        _ => panic!("expected default NOD at #C"),
    };
    nod.handle = Handle::new(0x0A);
    doc.objects
        .insert(Handle::new(0x0A), ObjectType::Dictionary(nod));

    let decoy = Dictionary {
        handle: Handle::new(0x0C),
        owner: Handle::new(0x0A),
        ..Dictionary::new()
    };
    doc.objects
        .insert(Handle::new(0x0C), ObjectType::Dictionary(decoy));

    for object in doc.objects.values_mut() {
        if let ObjectType::Dictionary(dict) = object {
            if dict.owner == Handle::new(0x0C) {
                dict.owner = Handle::new(0x0A);
            }
        }
    }
    assert_eq!(
        doc.header.named_objects_dict_handle,
        Handle::new(0x0C),
        "header still carries the stale default"
    );

    let input = std::env::temp_dir().join("issue64_root_input.dxf");
    DxfWriter::new(&doc).write_to_file(&input).unwrap();
    let loaded = DxfReader::from_file(&input).unwrap().read().unwrap();

    assert_eq!(
        loaded.header.named_objects_dict_handle,
        Handle::new(0x0A),
        "reader must resolve the root dictionary"
    );
    match loaded.objects.get(&Handle::new(0x0A)) {
        Some(ObjectType::Dictionary(dict)) => {
            assert!(dict.owner.is_null(), "root dictionary owns itself")
        }
        _ => panic!("root dict handle must point at a dictionary"),
    }

    let output = std::env::temp_dir().join("issue64_root_output.dxf");
    DxfWriter::new(&loaded).write_to_file(&output).unwrap();

    let text = std::fs::read_to_string(&output).unwrap();
    let lines: Vec<&str> = text.split("\r\n").collect();
    let mut first_object = None;
    for i in 0..lines.len() - 3 {
        if lines[i] == "  0" && lines[i + 1] == "SECTION" && lines[i + 3] == "OBJECTS" {
            first_object = Some((
                lines[i + 5].trim().to_string(),
                lines[i + 7].trim().to_string(),
            ));
            break;
        }
    }
    let (name, handle) = first_object.expect("OBJECTS section");
    assert_eq!(name, "DICTIONARY", "root dictionary must be first");
    assert_eq!(handle, "A", "first object is the real root dictionary");
}
