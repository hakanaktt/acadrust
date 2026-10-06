//! Regression tests for issue #66: post-read handle repair must allocate
//! above file-sourced handles and preserve object ownership when remapping.

use std::collections::HashSet;
use std::io::Cursor;

use acadrust::entities::{Circle, EntityType};
use acadrust::objects::{Dictionary, DictionaryVariable, ObjectType, XRecord};
use acadrust::tables::AppId;
use acadrust::types::Handle;
use acadrust::{CadDocument, DxfReader, DxfWriter, TableEntry};

fn read_dxf(bytes: Vec<u8>) -> CadDocument {
    DxfReader::from_reader(Cursor::new(bytes))
        .expect("create DXF reader")
        .read()
        .expect("read DXF")
}

fn assert_unique_identity_handles(document: &CadDocument) {
    let mut seen = HashSet::new();
    let mut insert = |kind: &str, handle: Handle| {
        if !handle.is_null() {
            assert!(
                seen.insert(handle),
                "duplicate identity handle {handle:?} found at {kind}"
            );
        }
    };

    macro_rules! insert_table {
        ($name:literal, $table:expr) => {{
            insert(concat!($name, " control"), $table.handle());
            for entry in $table.iter() {
                insert(concat!($name, " entry"), entry.handle());
            }
        }};
    }

    insert_table!("LAYER", document.layers);
    insert_table!("LTYPE", document.line_types);
    insert_table!("STYLE", document.text_styles);
    insert_table!("DIMSTYLE", document.dim_styles);
    insert_table!("APPID", document.app_ids);
    insert_table!("VIEW", document.views);
    insert_table!("VPORT", document.vports);
    insert_table!("UCS", document.ucss);
    insert_table!("VX", document.vx_table);

    insert("BLOCK_RECORD control", document.block_records.handle());
    for record in document.block_records.iter() {
        insert("BLOCK_RECORD entry", record.handle());
        insert("BLOCK marker", record.block_entity_handle);
        insert("ENDBLK marker", record.block_end_handle);
    }
    for entity in document.entities() {
        insert("entity", entity.common().handle);
    }
    for handle in document.objects.keys().copied() {
        insert("object", handle);
    }
}

#[test]
fn post_read_default_repair_allocates_above_file_handles() {
    let mut source = CadDocument::new();

    // These defaults must be absent from the source so the reader's own
    // initialize_defaults() entries survive and need collision repair.
    assert!(source.app_ids.remove("AcadAnnotative").is_some());
    assert!(source.app_ids.remove("AcCmTransparency").is_some());

    // Match the attachment: the surviving default APPIDs start at #1D/#1E,
    // while source-owned BLOCK/OBJECT records already occupy those handles.
    source
        .block_records
        .get_mut("*Paper_Space")
        .expect("paper space")
        .block_end_handle = Handle::new(0x1D);

    let mut xrecord = XRecord::named("SOURCE_OBJECT");
    xrecord.handle = Handle::new(0x1E);
    xrecord.owner = source.header.named_objects_dict_handle;
    source
        .objects
        .insert(xrecord.handle, ObjectType::XRecord(xrecord));

    for (name, handle) in [
        ("HATCHBACKGROUNDCOLOR", Handle::new(0x33)),
        ("EZDXF", Handle::new(0x34)),
    ] {
        let mut appid = AppId::new(name);
        appid.set_handle(handle);
        source.app_ids.add(appid).expect("unique APPID");
    }

    let mut circle = Circle::from_coords(0.0, 0.0, 0.0, 1.0);
    circle.common.handle = Handle::new(0x32);
    source
        .add_entity(EntityType::Circle(circle))
        .expect("add source entity");

    let input = DxfWriter::new(&source).write_to_vec().expect("write input");
    let loaded = read_dxf(input);

    assert_eq!(
        loaded
            .app_ids
            .get("HATCHBACKGROUNDCOLOR")
            .expect("source APPID")
            .handle,
        Handle::new(0x33)
    );
    assert_eq!(
        loaded.app_ids.get("EZDXF").expect("source APPID").handle,
        Handle::new(0x34)
    );
    assert_eq!(
        loaded
            .block_records
            .get("*Paper_Space")
            .expect("paper space")
            .block_end_handle,
        Handle::new(0x1D),
        "source ENDBLK handle must not be moved to preserve a synthesized default"
    );
    match loaded.objects.get(&Handle::new(0x1E)) {
        Some(ObjectType::XRecord(value)) => assert_eq!(value.handle, Handle::new(0x1E)),
        _ => panic!("source XRECORD handle must be preserved"),
    }

    let annotative = loaded
        .app_ids
        .get("AcadAnnotative")
        .expect("synthesized APPID")
        .handle;
    let transparency = loaded
        .app_ids
        .get("AcCmTransparency")
        .expect("synthesized APPID")
        .handle;
    for handle in [annotative, transparency] {
        assert!(
            ![
                Handle::new(0x1D),
                Handle::new(0x1E),
                Handle::new(0x33),
                Handle::new(0x34),
            ]
            .contains(&handle),
            "synthesized APPID reused a source identity handle"
        );
    }
    assert_unique_identity_handles(&loaded);

    let output = DxfWriter::new(&loaded)
        .write_to_vec()
        .expect("write round-trip");
    assert_unique_identity_handles(&read_dxf(output));
}

#[test]
fn explicit_entity_handle_does_not_lower_handle_seed() {
    let mut document = CadDocument::new();
    document.header.handle_seed = 0x100;

    let mut circle = Circle::from_coords(0.0, 0.0, 0.0, 1.0);
    circle.common.handle = Handle::new(0x40);
    document
        .add_entity(EntityType::Circle(circle))
        .expect("add explicit-handle entity");

    assert_eq!(document.header.handle_seed, 0x100);
    assert_eq!(document.allocate_handle(), Handle::new(0x100));
}

#[test]
fn dictionary_variable_owner_follows_remapped_dictionary() {
    let mut document = CadDocument::new();
    let colliding = Handle::new(0x100);
    let variable_handle = Handle::new(0x101);

    let mut circle = Circle::from_coords(0.0, 0.0, 0.0, 1.0);
    circle.common.handle = colliding;
    document
        .add_entity(EntityType::Circle(circle))
        .expect("add colliding entity");

    let mut dictionary = Dictionary::new();
    dictionary.handle = colliding;
    dictionary.add_entry("SETTING", variable_handle);
    document
        .objects
        .insert(colliding, ObjectType::Dictionary(dictionary));

    let mut variable = DictionaryVariable::new("SETTING", "1");
    variable.handle = variable_handle;
    variable.owner_handle = colliding;
    document
        .objects
        .insert(variable_handle, ObjectType::DictionaryVariable(variable));

    document.resolve_references();

    let remapped_dictionary = document
        .objects
        .iter()
        .find_map(|(handle, object)| match object {
            ObjectType::Dictionary(value)
                if value
                    .entries
                    .iter()
                    .any(|(name, target)| name == "SETTING" && *target == variable_handle) =>
            {
                Some(*handle)
            }
            _ => None,
        })
        .expect("remapped dictionary");
    assert_ne!(remapped_dictionary, colliding);

    match document.objects.get(&variable_handle) {
        Some(ObjectType::DictionaryVariable(variable)) => {
            assert_eq!(variable.owner_handle, remapped_dictionary);
        }
        _ => panic!("dictionary variable missing"),
    }
}
