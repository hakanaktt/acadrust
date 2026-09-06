use acadrust::entities::Line;
use acadrust::tables::Layer;
use acadrust::xdata::{ExtendedDataRecord, XDataValue};
use acadrust::{CadDocument, EntityType, Handle};

#[test]
fn rename_layer_updates_references_and_current_handle() {
    let mut document = CadDocument::new();
    let layer = Layer::new("TEST");
    document.layers.add(layer).unwrap();
    let mut line = Line::new();
    line.common.layer = "test".to_string();
    let entity_handle = document.add_entity(EntityType::Line(line)).unwrap();
    document.header.current_layer_name = "Test".to_string();
    document.header.current_layer_handle = Handle::NULL;

    let layer_handle = document.rename_layer("test", "Renamed").unwrap();

    assert!(layer_handle.is_valid());
    assert_eq!(document.layers.get("Renamed").unwrap().name, "Renamed");
    assert_eq!(
        document.get_entity(entity_handle).unwrap().common().layer,
        "Renamed"
    );
    assert_eq!(document.header.current_layer_name, "Renamed");
    assert_eq!(document.header.current_layer_handle, layer_handle);
}

#[test]
fn rename_layer_allows_unicode_case_change() {
    let mut document = CadDocument::new();
    document.layers.add(Layer::new("é")).unwrap();

    document.rename_layer("é", "É").unwrap();

    assert_eq!(document.layers.get("é").unwrap().name, "É");
}

#[test]
fn rename_layer_rejections_leave_document_unchanged() {
    let mut document = CadDocument::new();
    document.layers.add(Layer::new("First")).unwrap();
    document.layers.add(Layer::new("Second")).unwrap();
    let mut line = Line::new();
    line.common.layer = "First".to_string();
    let entity_handle = document.add_entity(EntityType::Line(line)).unwrap();

    assert!(document.rename_layer("First", "Second").is_err());
    assert!(document.rename_layer("0", "Zero").is_err());

    assert!(document.layers.contains("First"));
    assert!(document.layers.contains("Second"));
    assert_eq!(
        document.get_entity(entity_handle).unwrap().common().layer,
        "First"
    );
}

#[test]
fn rename_layer_updates_saved_state_and_xdata_references() {
    let mut document = CadDocument::new();
    document.layers.add(Layer::new("First")).unwrap();
    document.header.current_layer_name = "First".to_string();
    document.capture_layer_state("Saved", "");
    let mut line = Line::new();
    let mut record = ExtendedDataRecord::new("APP");
    record.add_value(XDataValue::LayerName("fIrSt".to_string()));
    line.common.extended_data.add_record(record);
    let entity_handle = document.add_entity(EntityType::Line(line)).unwrap();

    document.rename_layer("First", "Renamed").unwrap();

    assert_eq!(
        document.layer_state("Saved").unwrap().current_layer,
        "Renamed"
    );
    let values = &document
        .get_entity(entity_handle)
        .unwrap()
        .common()
        .extended_data
        .records()[0]
        .values;
    assert!(matches!(&values[0], XDataValue::LayerName(name) if name == "Renamed"));
}

#[test]
fn rename_layer_keeps_raw_xdata_when_handle_is_preserved() {
    let mut document = CadDocument::new();
    let mut layer = Layer::new("First");
    layer.handle = Handle::new(0x100);
    document.layers.add(layer).unwrap();
    let app_handle = document.app_ids.get("ACAD").unwrap().handle.value();
    let mut line = Line::new();
    let mut record = ExtendedDataRecord::new("ACAD");
    record.add_value(XDataValue::LayerName("First".to_string()));
    line.common.extended_data.add_record(record);
    line.common
        .extended_data
        .raw_dwg_eed
        .push((app_handle, vec![3, 0]));
    let entity_handle = document.add_entity(EntityType::Line(line)).unwrap();

    document.rename_layer("First", "Renamed").unwrap();

    assert_eq!(
        document
            .get_entity(entity_handle)
            .unwrap()
            .common()
            .extended_data
            .raw_dwg_eed
            .len(),
        1
    );
}

#[test]
fn rename_layer_drops_raw_xdata_when_handle_is_repaired() {
    let mut document = CadDocument::new();
    document.layers.add(Layer::new("First")).unwrap();
    let app_handle = document.app_ids.get("ACAD").unwrap().handle.value();
    let mut line = Line::new();
    let mut record = ExtendedDataRecord::new("ACAD");
    record.add_value(XDataValue::LayerName("First".to_string()));
    line.common.extended_data.add_record(record);
    line.common
        .extended_data
        .raw_dwg_eed
        .push((app_handle, vec![3, 0]));
    let entity_handle = document.add_entity(EntityType::Line(line)).unwrap();

    document.rename_layer("First", "Renamed").unwrap();

    assert!(document
        .get_entity(entity_handle)
        .unwrap()
        .common()
        .extended_data
        .raw_dwg_eed
        .is_empty());
}
