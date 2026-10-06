use std::io::Cursor;

use acadrust::objects::{
    AssocArrayActionBody, AssocArrayItem, AssocArrayModifyActionBody, AssocDimensionAssociation,
    AssociativeData, AssociativeObject, ObjectType, PlaceHolder,
};
use acadrust::tables::TextStyle;
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

fn dwg_roundtrip(document: &CadDocument) -> CadDocument {
    DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(document).unwrap()))
        .read()
        .unwrap()
}

fn dxf_roundtrip(document: &CadDocument) -> CadDocument {
    DxfReader::from_reader(Cursor::new(
        DxfWriter::new(document).write_to_vec().unwrap(),
    ))
    .unwrap()
    .read()
    .unwrap()
}

#[test]
fn retained_dimassoc_record_observes_added_extension_dictionary() {
    let mut document = CadDocument::new();
    let handle = document.allocate_handle();
    let mut object = AssociativeObject::new("DIMASSOC", "AcDbDimAssoc");
    object.handle = handle;
    object.owner = document.header.named_objects_dict_handle;
    object.data = AssociativeData::DimensionAssociation(AssocDimensionAssociation::default());
    document
        .objects
        .insert(handle, ObjectType::Associative(object));
    let mut source = dwg_roundtrip(&document);
    let record = source.ensure_xrecord(handle, "REVIEW_DATA");
    let dictionary = source.extension_dictionary_handle(handle).unwrap();

    let saved = dwg_roundtrip(&source);
    let ObjectType::Associative(object) = &saved.objects[&handle] else {
        panic!("Dimension association missing");
    };
    assert_eq!(object.xdictionary_handle, Some(dictionary));
    assert_eq!(saved.xrecord(handle, "REVIEW_DATA").unwrap().handle, record);
}

#[test]
fn retained_dimassoc_record_observes_existing_extension_dictionary_edits() {
    let mut document = CadDocument::new();
    let handle = document.allocate_handle();
    let mut object = AssociativeObject::new("DIMASSOC", "AcDbDimAssoc");
    object.handle = handle;
    object.owner = document.header.named_objects_dict_handle;
    object.data = AssociativeData::DimensionAssociation(AssocDimensionAssociation::default());
    document
        .objects
        .insert(handle, ObjectType::Associative(object));
    let dictionary = document.ensure_extension_dictionary(handle);
    let record = document.ensure_xrecord(handle, "ORIGINAL");
    let mut source = dwg_roundtrip(&document);
    let before = source.objects[&dictionary].clone();
    let extra = source.ensure_xrecord(handle, "ADDED");
    assert_ne!(source.objects[&dictionary], before);

    let saved = dwg_roundtrip(&source);
    assert_eq!(saved.extension_dictionary_handle(handle), Some(dictionary));
    assert_eq!(saved.xrecord(handle, "ORIGINAL").unwrap().handle, record);
    assert_eq!(saved.xrecord(handle, "ADDED").unwrap().handle, extra);
}

#[test]
fn truetype_font_flags_survive_saves_and_typeface_edits() {
    const FONT_FLAGS: i32 = 0x0300_0022;
    for version in [
        DxfVersion::AC1015,
        DxfVersion::AC1018,
        DxfVersion::AC1021,
        DxfVersion::AC1032,
    ] {
        let mut document = CadDocument::with_version(version);
        let mut style = TextStyle::with_truetype("StyledFont", "Arial");
        style.handle = document.allocate_handle();
        style.true_type_font_flags = FONT_FLAGS;
        document.text_styles.add(style).unwrap();
        let mut loaded = dwg_roundtrip(&document);
        for expected_face in ["Arial", "Tahoma"] {
            loaded
                .text_styles
                .get_mut("StyledFont")
                .unwrap()
                .true_type_font = expected_face.to_string();
            for saved in [dwg_roundtrip(&loaded), dxf_roundtrip(&loaded)] {
                let style = saved.text_styles.get("StyledFont").unwrap();
                assert_eq!(style.true_type_font, expected_face);
                assert_eq!(style.true_type_font_flags, FONT_FLAGS, "{version:?}");
                let from_dxf = dwg_roundtrip(&dxf_roundtrip(&saved));
                assert_eq!(
                    from_dxf
                        .text_styles
                        .get("StyledFont")
                        .unwrap()
                        .true_type_font_flags,
                    FONT_FLAGS,
                    "DXF to DWG {version:?}"
                );
            }
        }
    }
}

#[test]
fn array_body_item_handles_participate_in_reference_remapping() {
    for modify in [false, true] {
        let mut document = CadDocument::new();
        let colliding = document.layers.get("0").unwrap().handle;
        let mut target = PlaceHolder::new();
        target.handle = colliding;
        let target_owner = Handle::new(0x1234_5678);
        target.owner = target_owner;
        document
            .objects
            .insert(colliding, ObjectType::PlaceHolder(target));
        let body = AssocArrayActionBody {
            items: vec![AssocArrayItem {
                first_handle: Some(colliding),
                second_handle: Some(colliding),
                ..Default::default()
            }],
            ..Default::default()
        };
        let mut object = if modify {
            AssociativeObject::new(
                "ACDBASSOCARRAYMODIFYACTIONBODY",
                "AcDbAssocArrayModifyActionBody",
            )
        } else {
            AssociativeObject::new("ACDBASSOCARRAYACTIONBODY", "AcDbAssocArrayActionBody")
        };
        object.data = if modify {
            AssociativeData::ArrayModifyActionBody(AssocArrayModifyActionBody {
                body,
                ..Default::default()
            })
        } else {
            AssociativeData::ArrayActionBody(body)
        };
        assert!(object.references_handle(colliding));
        let handle = document.allocate_handle();
        object.handle = handle;
        document
            .objects
            .insert(handle, ObjectType::Associative(object));

        document.resolve_references();
        let remapped = document
            .objects
            .iter()
            .find_map(|(handle, object)| {
                matches!(object, ObjectType::PlaceHolder(value) if value.owner == target_owner)
                    .then_some(*handle)
            })
            .unwrap();
        assert_ne!(remapped, colliding);
        let ObjectType::Associative(object) = &document.objects[&handle] else {
            panic!()
        };
        let items = match &object.data {
            AssociativeData::ArrayActionBody(body) => &body.items,
            AssociativeData::ArrayModifyActionBody(value) => &value.body.items,
            _ => panic!("Array action body missing"),
        };
        assert_eq!(items[0].first_handle, Some(remapped));
        assert_eq!(items[0].second_handle, Some(remapped));
        assert!(object.references_handle(remapped));
        assert!(!object.references_handle(colliding));
    }
}

#[cfg(feature = "serde")]
#[test]
fn truetype_font_flags_default_for_older_serialized_styles() {
    let mut value = serde_json::to_value(TextStyle::new("Legacy")).unwrap();
    value
        .as_object_mut()
        .unwrap()
        .remove("true_type_font_flags");
    let style: TextStyle = serde_json::from_value(value).unwrap();
    assert_eq!(style.true_type_font_flags, 34);
}
