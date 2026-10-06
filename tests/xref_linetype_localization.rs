use std::io::Cursor;

use acadrust::entities::{EntityType, Line};
use acadrust::nested_copy::NestedCopyMode;
use acadrust::tables::{BlockRecord, Layer, LineType};
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn assert_localized_linetype_survives_detach(
    mode: NestedCopyMode,
    local_linetype_name: &str,
    local_layer_name: &str,
) {
    for version in [
        DxfVersion::AC1018,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        let mut document = CadDocument::with_version(version);
        let mut xref = BlockRecord::new("X");
        xref.handle = document.allocate_handle();
        xref.block_entity_handle = document.allocate_handle();
        xref.block_end_handle = document.allocate_handle();
        xref.flags.is_xref = true;
        xref.xref_path = "X.dwg".into();
        let xref_handle = xref.handle;
        document.block_records.add(xref).unwrap();

        let mut line_type = LineType::dashed();
        line_type.name = "X|Dash".into();
        line_type.handle = document.allocate_handle();
        line_type.xref_dependent = true;
        line_type.xref_block_record_handle = xref_handle;
        let expected_elements = line_type.elements.clone();
        document.line_types.add_or_replace(line_type);

        let mut layer = Layer::new("X|Detail");
        layer.handle = document.allocate_handle();
        layer.line_type = "X|Dash".into();
        layer.flags.xref_dependent = true;
        layer.xref_block_record_handle = xref_handle;
        document.layers.add_or_replace(layer);

        let mut line = Line::from_coords(0.0, 0.0, 0.0, 1.0, 0.0, 0.0);
        line.common.layer = "X|Detail".into();
        line.common.linetype = "X|Dash".into();
        let mut copied_entities = vec![EntityType::Line(line)];
        let names = document.nested_copy_symbol_names(mode);
        assert_eq!(
            document.localize_nested_copy_symbols(&mut copied_entities, &names),
            0
        );
        let common = copied_entities[0].common();
        assert_eq!(common.layer, local_layer_name);
        assert_eq!(common.linetype, local_linetype_name);

        let local = document.line_types.get(local_linetype_name).unwrap();
        assert!(!local.xref_dependent);
        assert_eq!(local.xref_block_record_handle, Handle::NULL);
        assert_eq!(local.elements, expected_elements);
        let source = document.line_types.get("X|Dash").unwrap();
        assert!(source.xref_dependent);
        assert_eq!(source.xref_block_record_handle, xref_handle);

        for entity in copied_entities {
            document.add_entity(entity).unwrap();
        }
        // Remove the source xref and its symbols as a host does on detach.
        document.block_records.remove("X").unwrap();
        document.line_types.remove("X|Dash").unwrap();
        document.layers.remove("X|Detail").unwrap();

        let bytes = DwgWriter::write_to_vec(&document).unwrap();
        let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        assert!(read.block_records.get("X").is_none());
        assert!(read
            .block_records
            .iter()
            .all(|record| record.handle != xref_handle));
        let local = read.line_types.get(local_linetype_name).unwrap();
        assert!(!local.xref_dependent);
        assert_eq!(
            local.xref_block_record_handle,
            Handle::NULL,
            "{version:?}, {mode:?}: local linetype retains a detached xref handle"
        );
        assert_eq!(local.elements, expected_elements);
        let copied = read
            .entities()
            .find(|entity| matches!(entity, EntityType::Line(_)))
            .unwrap();
        assert_eq!(copied.common().layer, local_layer_name);
        assert_eq!(copied.common().linetype, local_linetype_name);
    }
}

#[test]
fn inserted_linetype_is_independent_of_detached_xref() {
    assert_localized_linetype_survives_detach(NestedCopyMode::Insert, "Dash", "Detail");
}

#[test]
fn bound_linetype_is_independent_of_detached_xref() {
    assert_localized_linetype_survives_detach(NestedCopyMode::Bind, "X$0$Dash", "X$0$Detail");
}
