//! Accepted SDK-based mapping assumption: arc-length dimensions use the angular
//! annotation context payload. This synthesized regression is not a native CAD
//! capture and does not claim host editing/regeneration coverage.
use std::io::Cursor;

use acadrust::entities::{Dimension, DimensionArc, EntityType};
use acadrust::objects::{
    Dictionary, DimContext, DimSubtype, ObjectContextData, ObjectContextKind, ObjectType, Scale,
};
use acadrust::types::{DxfVersion, Vector2, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, Handle};

fn child_dictionary(document: &mut CadDocument, owner: Handle, key: &str) -> Handle {
    let handle = document.allocate_handle();
    let mut dictionary = Dictionary::new();
    dictionary.handle = handle;
    dictionary.owner = owner;
    dictionary.reactors.push(owner);
    dictionary.hard_owner = true;
    document
        .objects
        .insert(handle, ObjectType::Dictionary(dictionary));
    let Some(ObjectType::Dictionary(parent)) = document.objects.get_mut(&owner) else {
        panic!("parent dictionary");
    };
    parent.insert(key, handle);
    handle
}

fn fixture() -> CadDocument {
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    document.register_object_context_class("ACDB_ANGDIMOBJECTCONTEXTDATA_CLASS");
    document
        .dim_styles
        .get_mut("Standard")
        .expect("style")
        .annotative = true;
    let mut arc = DimensionArc::default();
    arc.first_extension_point = Vector3::new(10.0, 0.0, 0.0);
    arc.second_extension_point = Vector3::new(0.0, 10.0, 0.0);
    arc.definition_point = Vector3::new(12.0, 12.0, 0.0);
    arc.arc_end_parameter = std::f64::consts::FRAC_PI_2;
    arc.has_leader = true;
    arc.first_leader_point = Vector3::new(13.0, 13.0, 0.0);
    arc.second_leader_point = Vector3::new(15.0, 15.0, 0.0);
    let owner = document
        .add_entity(EntityType::Dimension(Box::new(Dimension::Arc(arc))))
        .expect("arc");
    let extension = document.ensure_extension_dictionary(owner);
    let manager = child_dictionary(&mut document, extension, "AcDbContextDataManager");
    let contexts = child_dictionary(&mut document, manager, "ACDB_ANNOTATIONSCALES");
    let scales = document.ensure_subdictionary("ACAD_SCALELIST");
    for (index, ratio) in [20.0, 50.0].into_iter().enumerate() {
        let scale_handle = document.allocate_handle();
        let mut scale = Scale::new(&format!("1:{ratio}"), 1.0, ratio);
        scale.handle = scale_handle;
        scale.owner_handle = scales;
        document
            .objects
            .insert(scale_handle, ObjectType::Scale(scale));
        let Some(ObjectType::Dictionary(dictionary)) = document.objects.get_mut(&scales) else {
            panic!("scale dictionary");
        };
        dictionary.insert(format!("*A{index}"), scale_handle);
        let handle = document.allocate_handle();
        let context = ObjectContextData {
            handle,
            owner_handle: contexts,
            reactors: vec![contexts],
            xdictionary_handle: None,
            class_version: 4,
            has_file_to_extension_dictionary: true,
            uses_legacy_mleader_context_base: false,
            is_default: index == 0,
            scale: scale_handle,
            kind: ObjectContextKind::Dim(DimContext {
                def_pt: Vector2::new(ratio, ratio + 1.0),
                is_def_textloc: index == 0,
                text_rotation: 0.25,
                block: Handle::NULL,
                b293: false,
                dimtofl: true,
                dimosxd: false,
                dimatfit: true,
                dimtix: false,
                dimtmove: true,
                override_code: 0b10101,
                has_arrow2: true,
                flip_arrow2: false,
                flip_arrow1: true,
                subtype: DimSubtype::Angular {
                    arc_pt: Vector3::new(ratio + 2.0, ratio + 3.0, 2.0),
                },
            }),
        };
        document
            .objects
            .insert(handle, ObjectType::ObjectContextData(context));
        let Some(ObjectType::Dictionary(dictionary)) = document.objects.get_mut(&contexts) else {
            panic!("context dictionary");
        };
        dictionary.insert(format!("*A{index}"), handle);
    }
    document
}

fn arc(document: &CadDocument) -> &DimensionArc {
    document
        .entities()
        .find_map(|entity| match entity {
            EntityType::Dimension(dimension) => match dimension.as_ref() {
                Dimension::Arc(arc) => Some(arc),
                _ => None,
            },
            _ => None,
        })
        .expect("arc-length dimension survived")
}

fn snapshots(document: &CadDocument) -> Vec<(String, bool, ObjectContextKind)> {
    let mut owner = arc(document)
        .base
        .common
        .xdictionary_handle
        .expect("extension");
    for key in ["AcDbContextDataManager", "ACDB_ANNOTATIONSCALES"] {
        let Some(ObjectType::Dictionary(dictionary)) = document.objects.get(&owner) else {
            panic!("dictionary");
        };
        owner = dictionary.get(key).expect("linked dictionary");
    }
    let Some(ObjectType::Dictionary(dictionary)) = document.objects.get(&owner) else {
        panic!("contexts");
    };
    let mut snapshots = Vec::new();
    for (_, handle) in &dictionary.entries {
        let Some(ObjectType::ObjectContextData(context)) = document.objects.get(handle) else {
            panic!("context");
        };
        assert_eq!(context.class_name(), "ACDB_ANGDIMOBJECTCONTEXTDATA_CLASS");
        let Some(ObjectType::Scale(scale)) = document.objects.get(&context.scale) else {
            panic!("scale");
        };
        snapshots.push((scale.name.clone(), context.is_default, context.kind.clone()));
    }
    snapshots.sort_by(|a, b| a.0.cmp(&b.0));
    snapshots
}

#[test]
fn arc_length_with_two_angular_contexts_survives_dwg_and_both_dxf_forms() {
    let source = fixture();
    let expected = snapshots(&source);
    assert_eq!(expected.len(), 2);
    let dwg = DwgWriter::write_to_vec(&source).expect("DWG write");
    let mut readbacks = vec![DwgReader::from_stream(Cursor::new(dwg))
        .read()
        .expect("strict DWG read")];
    for binary in [false, true] {
        let mut writer = DxfWriter::new(&source);
        writer.set_binary(binary);
        let bytes = writer.write_to_vec().expect("DXF write");
        readbacks.push(
            DxfReader::from_reader(Cursor::new(bytes))
                .expect("DXF reader")
                .read()
                .expect("DXF read"),
        );
    }
    for readback in readbacks {
        assert_eq!(snapshots(&readback), expected);
        assert!(
            readback
                .dim_styles
                .get("Standard")
                .expect("style survived")
                .annotative,
            "the contexts must keep their annotative style policy"
        );
        let result = arc(&readback);
        let original = arc(&source);
        assert_eq!(result.definition_point, original.definition_point);
        assert_eq!(result.first_extension_point, original.first_extension_point);
        assert_eq!(
            result.second_extension_point,
            original.second_extension_point
        );
        assert_eq!(result.has_leader, original.has_leader);
        assert_eq!(result.first_leader_point, original.first_leader_point);
        assert_eq!(result.second_leader_point, original.second_leader_point);
        assert_eq!(result.measurement(), original.measurement());
    }
}
