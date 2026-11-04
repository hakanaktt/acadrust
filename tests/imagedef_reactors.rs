//! IMAGEDEF keeps its `{ACAD_REACTORS}` list (one IMAGEDEF_REACTOR per
//! image) through DWG and DXF round trips.

use std::io::Cursor;

use acadrust::entities::{EntityType, RasterImage};
use acadrust::objects::{ImageDefinition, ImageDefinitionReactor, ObjectType};
use acadrust::{
    CadDocument, DwgReader, DwgWriter, DxfReader, DxfVersion, DxfWriter, Handle, Vector3,
};

/// Definition D, image I using it, reactor R owned by I, `D.reactors = [R]`.
fn image_document() -> (CadDocument, Handle, Handle, Handle) {
    let (mut doc, def, image, reactor_handle) = image_document_without_list();
    definition_mut(&mut doc, def).reactors = vec![reactor_handle];
    (doc, def, image, reactor_handle)
}

/// As `image_document`, but the definition's reactor list is left empty.
fn image_document_without_list() -> (CadDocument, Handle, Handle, Handle) {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut definition = ImageDefinition::with_dimensions("photo.png", 64, 32);
    definition.handle = doc.allocate_handle();
    definition.owner = doc.header.named_objects_dict_handle;
    let def = definition.handle;
    doc.objects
        .insert(def, ObjectType::ImageDefinition(definition));
    if let Some(ObjectType::Dictionary(root)) =
        doc.objects.get_mut(&doc.header.named_objects_dict_handle)
    {
        root.add_entry("IMAGE_TEST", def);
    }

    let mut image = RasterImage::new("photo.png", Vector3::ZERO, 64.0, 32.0);
    image.definition_handle = Some(def);
    let image = doc.add_entity(EntityType::RasterImage(image)).unwrap();

    let mut reactor = ImageDefinitionReactor::new(image);
    reactor.handle = doc.allocate_handle();
    reactor.owner = image;
    let reactor_handle = reactor.handle;
    doc.objects
        .insert(reactor_handle, ObjectType::ImageDefinitionReactor(reactor));
    if let Some(EntityType::RasterImage(image)) = doc.get_entity_mut(image) {
        image.definition_reactor_handle = Some(reactor_handle);
    }
    (doc, def, image, reactor_handle)
}

fn definition(doc: &CadDocument, handle: Handle) -> &ImageDefinition {
    match doc.objects.get(&handle) {
        Some(ObjectType::ImageDefinition(definition)) => definition,
        other => panic!("IMAGEDEF {handle:?} missing: {other:?}"),
    }
}

fn definition_mut(doc: &mut CadDocument, handle: Handle) -> &mut ImageDefinition {
    match doc.objects.get_mut(&handle) {
        Some(ObjectType::ImageDefinition(definition)) => definition,
        _ => panic!("IMAGEDEF {handle:?} missing"),
    }
}

fn reactor(doc: &CadDocument, handle: Handle) -> &ImageDefinitionReactor {
    match doc.objects.get(&handle) {
        Some(ObjectType::ImageDefinitionReactor(reactor)) => reactor,
        other => panic!("IMAGEDEF_REACTOR {handle:?} missing: {other:?}"),
    }
}

fn dwg_round_trip(doc: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(doc).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn dxf_text(doc: &CadDocument) -> String {
    String::from_utf8(DxfWriter::new(doc).write_to_vec().unwrap()).unwrap()
}

fn dxf_round_trip(doc: &CadDocument) -> CadDocument {
    DxfReader::from_reader(Cursor::new(dxf_text(doc).into_bytes()))
        .unwrap()
        .read()
        .unwrap()
}

fn image_reactor_handle(doc: &CadDocument, image: Handle) -> Option<Handle> {
    match doc.get_entity(image) {
        Some(EntityType::RasterImage(image)) => image.definition_reactor_handle,
        other => panic!("IMAGE {image:?} missing: {other:?}"),
    }
}

/// The IMAGEDEF record's reactor handles in DXF text (empty without a group).
fn dxf_imagedef_reactors(text: &str, def: Handle) -> Vec<String> {
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let start = lines
        .windows(4)
        .position(|w| w[1] == "IMAGEDEF" && w[3] == format!("{:X}", def.value()))
        .expect("IMAGEDEF record");
    let record: Vec<&str> = lines[start + 4..]
        .chunks_exact(2)
        .map(|pair| pair[1])
        .take_while(|value| *value != "AcDbRasterImageDef")
        .collect();
    match record.iter().position(|value| *value == "{ACAD_REACTORS") {
        Some(open) => record[open + 1..]
            .iter()
            .take_while(|value| **value != "}")
            .map(|value| value.to_string())
            .collect(),
        None => Vec::new(),
    }
}

/// A file that carries the group (as AutoCAD writes it) keeps it through
/// DXF -> DXF and DXF -> DWG -> DXF. Uses only the text, no struct field.
#[test]
fn imagedef_reactor_group_from_dxf_is_written_back() {
    let (doc, def, _, reactor_handle) = image_document_without_list();
    let reactor_hex = format!("{:X}", reactor_handle.value());
    let text = dxf_text(&doc);
    let mut lines: Vec<&str> = text.lines().map(str::trim).collect();
    let start = lines
        .windows(4)
        .position(|w| w[1] == "IMAGEDEF" && w[3] == format!("{:X}", def.value()))
        .expect("IMAGEDEF record");
    // The writer lists the reactors it derives for a definition without a
    // list; replace that group with the file's own.
    if lines[start + 5] == "{ACAD_REACTORS" {
        let close = (start + 5..lines.len())
            .step_by(2)
            .find(|&k| lines[k] == "}")
            .expect("reactor group end");
        lines.drain(start + 4..close + 1);
    }
    let group = [
        "102",
        "{ACAD_REACTORS",
        "330",
        reactor_hex.as_str(),
        "102",
        "}",
    ];
    lines.splice(start + 4..start + 4, group);
    let source = lines.join("\n") + "\n";
    let expected = vec![reactor_hex.clone()];
    assert_eq!(dxf_imagedef_reactors(&source, def), expected);

    let loaded = DxfReader::from_reader(Cursor::new(source.into_bytes()))
        .unwrap()
        .read()
        .unwrap();
    assert_eq!(
        dxf_imagedef_reactors(&dxf_text(&loaded), def),
        expected,
        "DXF -> DXF"
    );
    let via_dwg = dwg_round_trip(&loaded);
    assert_eq!(
        dxf_imagedef_reactors(&dxf_text(&via_dwg), def),
        expected,
        "DXF -> DWG -> DXF"
    );
}

#[test]
fn imagedef_reactors_survive_dwg_round_trip() {
    let (doc, def, image, reactor_handle) = image_document();
    let loaded = dwg_round_trip(&doc);
    assert_eq!(definition(&loaded, def).reactors, vec![reactor_handle]);
    assert_eq!(
        definition(&loaded, def).owner,
        doc.header.named_objects_dict_handle
    );
    assert_eq!(reactor(&loaded, reactor_handle).owner, image);
    assert_eq!(reactor(&loaded, reactor_handle).image_handle, image);
    assert_eq!(image_reactor_handle(&loaded, image), Some(reactor_handle));

    // A second trip writes the list once, from the field.
    let again = dwg_round_trip(&loaded);
    assert_eq!(definition(&again, def).reactors, vec![reactor_handle]);
}

/// A definition whose list was emptied is written with the reactors the
/// drawing gives it: its dictionary and each image's reactor.
#[test]
fn emptied_imagedef_reactors_are_derived_after_dwg_load() {
    let (doc, def, _, reactor_handle) = image_document();
    let mut loaded = dwg_round_trip(&doc);
    definition_mut(&mut loaded, def).reactors.clear();
    let again = dwg_round_trip(&loaded);
    let owner = definition(&again, def).owner;
    assert_eq!(definition(&again, def).reactors, vec![owner, reactor_handle]);
}

#[test]
fn imagedef_without_reactors_gets_derived_ones() {
    let (mut doc, def, _, reactor_handle) = image_document();
    definition_mut(&mut doc, def).reactors.clear();
    let owner = definition(&doc, def).owner;
    let expected = vec![owner, reactor_handle];
    assert_eq!(definition(&dwg_round_trip(&doc), def).reactors, expected);
    assert_eq!(definition(&dxf_round_trip(&doc), def).reactors, expected);
}

#[test]
fn imagedef_reactors_survive_dxf_round_trip() {
    let (doc, def, image, reactor_handle) = image_document();
    assert_eq!(
        dxf_imagedef_reactors(&dxf_text(&doc), def),
        vec![format!("{:X}", reactor_handle.value())]
    );

    let loaded = dxf_round_trip(&doc);
    assert_eq!(definition(&loaded, def).reactors, vec![reactor_handle]);
    assert_eq!(
        definition(&loaded, def).owner,
        doc.header.named_objects_dict_handle
    );
    assert_eq!(reactor(&loaded, reactor_handle).owner, image);
    assert_eq!(reactor(&loaded, reactor_handle).image_handle, image);
}
