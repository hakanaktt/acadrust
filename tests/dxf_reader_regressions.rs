
use std::io::Cursor;

use acadrust::entities::attribute_definition::{
    AttributeDefinition, AttributeFlags, HorizontalAlignment, VerticalAlignment,
};
use acadrust::entities::EntityType;
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};

fn dxf_roundtrip(doc: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(doc).write_to_vec().expect("DXF write failed");
    DxfReader::from_reader(Cursor::new(bytes))
        .expect("DXF reader init failed")
        .read()
        .expect("DXF read failed")
}

#[test]
fn attdef_text_and_attribute_fields_survive_dxf_roundtrip() {
    let mut a = AttributeDefinition::new("TAG".into(), "Prompt".into(), "Default".into());
    a.insertion_point = Vector3::new(1.0, 2.0, 0.0);
    a.alignment_point = Vector3::new(3.0, 4.0, 0.0);
    a.height = 2.0;
    a.width_factor = 0.8;
    a.oblique_angle = 15f64.to_radians();
    a.text_style = "Standard".into();
    a.text_generation_flags = 2;
    a.horizontal_alignment = HorizontalAlignment::Center;
    a.vertical_alignment = VerticalAlignment::Middle;
    a.flags = AttributeFlags {
        invisible: true,
        constant: true,
        ..Default::default()
    };
    a.field_length = 12;
    a.normal = Vector3::new(0.0, 0.0, -1.0);

    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.add_entity(EntityType::AttributeDefinition(a)).unwrap();
    let rt = dxf_roundtrip(&doc);
    let b = rt
        .entities()
        .find_map(|e| match e {
            EntityType::AttributeDefinition(a) => Some(a.clone()),
            _ => None,
        })
        .expect("ATTDEF missing");

    assert_eq!(b.alignment_point, Vector3::new(3.0, 4.0, 0.0));
    assert_eq!(b.width_factor, 0.8);
    assert!((b.oblique_angle - 15f64.to_radians()).abs() < 1e-9);
    assert_eq!(b.text_style, "Standard");
    assert_eq!(b.text_generation_flags, 2);
    assert_eq!(b.horizontal_alignment, HorizontalAlignment::Center);
    assert_eq!(b.vertical_alignment, VerticalAlignment::Middle);
    assert!(b.flags.invisible && b.flags.constant && !b.flags.verify);
    assert_eq!(b.field_length, 12);
    assert_eq!(b.normal, Vector3::new(0.0, 0.0, -1.0));
}

#[test]
fn helix_left_handed_survives_dxf_roundtrip() {
    use acadrust::entities::Helix;
    let mut h = Helix::new();
    h.handedness = false;
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.add_entity(EntityType::Helix(Box::new(h))).unwrap();
    let rt = dxf_roundtrip(&doc);
    let h = rt
        .entities()
        .find_map(|e| match e {
            EntityType::Helix(h) => Some(h.clone()),
            _ => None,
        })
        .expect("HELIX missing");
    assert!(!h.handedness);
}

#[test]
fn light_boolean_flags_survive_dxf_roundtrip() {
    use acadrust::entities::Light;
    let mut l = Light::new();
    l.status = true;
    l.plot_glyph = true;
    l.use_attenuation_limits = true;
    l.cast_shadows = true;
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.add_entity(EntityType::Light(l)).unwrap();
    let rt = dxf_roundtrip(&doc);
    let l = rt
        .entities()
        .find_map(|e| match e {
            EntityType::Light(l) => Some(l.clone()),
            _ => None,
        })
        .expect("LIGHT missing");
    assert!(l.status && l.plot_glyph && l.use_attenuation_limits && l.cast_shadows);
}

#[test]
fn underlay_rotation_is_degrees_on_the_wire() {
    use acadrust::entities::underlay::{Underlay, UnderlayType};
    let mut u = Underlay::new(UnderlayType::Pdf);
    u.rotation = 0.5; // radians
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.add_entity(EntityType::Underlay(u)).unwrap();
    let rt = dxf_roundtrip(&doc);
    let u = rt
        .entities()
        .find_map(|e| match e {
            EntityType::Underlay(u) => Some(u.clone()),
            _ => None,
        })
        .expect("UNDERLAY missing");
    assert!((u.rotation - 0.5).abs() < 1e-9, "rotation = {}", u.rotation);
}

#[test]
fn leader_annotation_link_vectors_and_color_survive_dxf_roundtrip() {
    use acadrust::entities::Leader;
    use acadrust::types::{Color, Handle};
    let mut l = Leader::new();
    l.vertices = vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(5.0, 5.0, 0.0)];
    l.annotation_handle = Handle::new(0x2A);
    l.horizontal_direction = Vector3::new(0.0, 1.0, 0.0);
    l.block_offset = Vector3::new(1.0, 2.0, 3.0);
    l.annotation_offset = Vector3::new(4.0, 5.0, 6.0);
    l.override_color = Color::from_index(3);
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.add_entity(EntityType::Leader(l)).unwrap();
    let rt = dxf_roundtrip(&doc);
    let l = rt
        .entities()
        .find_map(|e| match e {
            EntityType::Leader(l) => Some(l.clone()),
            _ => None,
        })
        .expect("LEADER missing");
    assert_eq!(l.annotation_handle, Handle::new(0x2A));
    assert_eq!(l.horizontal_direction, Vector3::new(0.0, 1.0, 0.0));
    assert_eq!(l.block_offset, Vector3::new(1.0, 2.0, 3.0));
    assert_eq!(l.annotation_offset, Vector3::new(4.0, 5.0, 6.0));
    assert_eq!(l.override_color, Color::from_index(3));
}

#[test]
fn view_border_and_section_symbol_keep_their_kind_in_dxf_entities_section() {
    use acadrust::entities::{SectionSymbol, ViewBorder};
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.add_entity(EntityType::ViewBorder(ViewBorder::default())).unwrap();
    doc.add_entity(EntityType::SectionSymbol(SectionSymbol::default())).unwrap();
    let rt = dxf_roundtrip(&doc);
    assert!(rt.entities().any(|e| matches!(e, EntityType::ViewBorder(_))));
    assert!(rt.entities().any(|e| matches!(e, EntityType::SectionSymbol(_))));
}

#[test]
fn table_merged_ranges_are_rebuilt_after_dxf_roundtrip() {
    use acadrust::entities::table::{CellRange, Table};
    let mut t = Table::new(Vector3::new(0.0, 0.0, 0.0), 3, 3);
    t.merge_cells(CellRange::new(0, 0, 1, 1));
    assert_eq!(t.merged_ranges.len(), 1);
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.add_entity(EntityType::Table(Box::new(t))).unwrap();
    let rt = dxf_roundtrip(&doc);
    let t = rt
        .entities()
        .find_map(|e| match e {
            EntityType::Table(t) => Some(t.clone()),
            _ => None,
        })
        .expect("TABLE missing");
    assert_eq!(t.merged_ranges, vec![CellRange::new(0, 0, 1, 1)]);
}
