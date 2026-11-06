#![allow(clippy::field_reassign_with_default)] // Deliberate independent public-field mutations.
use acadrust::CadDocument;

#[test]
fn zero_is_total_noop() {
    let mut doc = CadDocument::new();
    doc.header.handle_seed = u64::MAX;
    let range = doc.try_reserve_handles(0).unwrap();
    assert!(range.is_empty());
    assert_eq!(range.len(), 0);
    assert_eq!(range.get(0), None);
    assert_eq!(range.into_iter().len(), 0);
    assert_eq!(doc.header.handle_seed, u64::MAX);
}

#[test]
fn constructor_defaults_are_supported() {
    let mut doc = CadDocument::new();
    let seed = doc.header.handle_seed;
    let range = doc.try_reserve_handles(2).unwrap();
    assert!(range.get(0).unwrap().value() >= seed);
    let end = range.get(1).unwrap().value() + 1;
    assert_eq!(doc.header.handle_seed, end);
    assert_eq!(
        doc.try_reserve_handles(1).unwrap().get(0).unwrap().value(),
        end
    );
}

#[test]
fn dangling_xrecord_target_is_never_reserved() {
    use acadrust::objects::{ObjectType, XRecord, XRecordEntry, XRecordValue};
    use acadrust::types::Handle;
    let mut doc = CadDocument::new();
    let mut x = XRecord::new();
    x.handle = Handle::new(700);
    x.entries.push(XRecordEntry::new(
        340,
        XRecordValue::Handle(Handle::new(9000)),
    ));
    doc.objects.insert(x.handle, ObjectType::XRecord(x));
    assert!(doc.try_reserve_handles(1).unwrap().get(0).unwrap().value() > 9000);
}

#[test]
fn unused_ranges_are_consumed_and_new_public_references_are_seen() {
    use acadrust::{
        objects::{ObjectType, XRecord},
        types::Handle,
    };
    let mut d = CadDocument::new();
    let r = d.try_reserve_handles(3).unwrap();
    let end = r.get(2).unwrap().value() + 1;
    assert_eq!(d.allocate_handle().value(), end);
    let mut x = XRecord::new();
    x.handle = Handle::new(9000);
    d.objects.insert(x.handle, ObjectType::XRecord(x));
    assert!(d.try_reserve_handles(1).unwrap().get(0).unwrap().value() > 9000);
}
#[test]
fn clone_insert_lookup_remove_restore_is_independent() {
    use acadrust::entities::{EntityType, Line};
    let original = CadDocument::new();
    let mut d = original.clone();
    let h = d.try_reserve_handles(1).unwrap().get(0).unwrap();
    let mut l = Line::new();
    l.common.handle = h;
    assert_eq!(d.add_entity(EntityType::Line(l)).unwrap(), h);
    assert!(original.get_entity(h).is_none());
    let image = d.remove_entity_arc(h).unwrap();
    assert!(d.get_entity(h).is_none());
    assert_eq!(d.restore_entity_arc(image), Some(h));
    assert!(d.get_entity(h).is_some());
}

#[test]
fn audit_clean_dangling_reference_still_reserves_its_number() {
    use acadrust::objects::{ObjectType, XRecord, XRecordEntry};
    use acadrust::types::Handle;
    let mut d = CadDocument::new();
    let own = d.allocate_handle();
    let dangling = Handle::new(d.next_handle());
    let mut x = XRecord::named("guard");
    x.handle = own;
    x.owner = d.header.named_objects_dict_handle;
    x.add_entry(XRecordEntry::handle(340, dangling));
    d.objects.insert(own, ObjectType::XRecord(x));
    d.named_objects_dictionary_mut()
        .unwrap()
        .add_entry("guard", own);
    assert_eq!(d.audit().error_count(), 0);
    assert!(d.try_reserve_handles(1).unwrap().get(0).unwrap().value() > dangling.value());
}
#[test]
fn stale_table_floor_is_reconciled_without_repair() {
    use acadrust::{tables::Layer, types::Handle};
    let mut d = CadDocument::new();
    let mut layer = Layer::new("high");
    layer.handle = Handle::new(9000);
    d.layers.add(layer.clone()).unwrap();
    assert!(d.try_reserve_handles(1).unwrap().get(0).unwrap().value() > 9000);
    assert_eq!(d.layers.get("high"), Some(&layer));
}

fn assert_public_coverage_refusal(mut d: CadDocument) {
    let before = format!("{d:?}");
    let counters = (d.next_handle(), d.header.handle_seed);
    assert!(matches!(
        d.try_reserve_handles(1),
        Err(acadrust::HandleReservationError::UninspectableData { .. })
    ));
    assert_eq!((d.next_handle(), d.header.handle_seed), counters);
    assert_eq!(format!("{d:?}"), before);
}

#[test]
fn every_common_raw_carrier_refuses_without_discard() {
    use acadrust::entities::{EntityType, Line, R12EntityMetadata, RawRecord};
    for case in 0..4 {
        let mut d = CadDocument::new();
        let version = d.version;
        let h = d.add_entity(EntityType::Line(Line::new())).unwrap();
        let c = d.get_entity_mut(h).unwrap().common_mut();
        match case {
            0 => {
                c.raw_record = Some(std::sync::Arc::new(RawRecord {
                    data: vec![],
                    handle_bits: 0,
                    version,
                }))
            }
            1 => c.graphic_data = Some(vec![]),
            2 => {
                c.r12_metadata = Some(R12EntityMetadata {
                    viewport: None,
                    child_viewports: vec![],
                })
            }
            3 => c.has_ds_data = true,
            _ => unreachable!(),
        }
        assert_public_coverage_refusal(d);
    }
}

#[test]
fn layout_style_and_order_raw_replays_refuse_even_when_empty() {
    use acadrust::objects::{Layout, ObjectType, SortEntitiesTable, TableStyle};
    for case in 0..3 {
        let mut d = CadDocument::new();
        let h = d.allocate_handle();
        let o = match case {
            0 => {
                let mut v = Layout::new("raw");
                v.handle = h;
                v.raw_plot_settings_codes = Some(vec![]);
                ObjectType::Layout(v)
            }
            1 => {
                let mut v = TableStyle::default();
                v.handle = h;
                v.raw_dxf_codes = Some(vec![]);
                ObjectType::TableStyle(v)
            }
            2 => {
                let mut v = SortEntitiesTable::new();
                v.handle = h;
                v.raw_dxf_codes = Some(vec![]);
                ObjectType::SortEntitiesTable(v)
            }
            _ => unreachable!(),
        };
        d.objects.insert(h, o);
        assert_public_coverage_refusal(d);
    }
}

#[test]
fn all_declared_text_field_carriers_refuse() {
    use acadrust::entities::{
        AttributeDefinition, AttributeEntity, Dimension, DimensionAligned, EntityType, MText, Text,
    };
    let mut text = Text::new();
    text.value = "%<field>".into();
    let mut mtext = MText::new();
    mtext.value = "%<field>".into();
    let mut attr = AttributeEntity::default();
    attr.value = "%<field>".into();
    let mut def = AttributeDefinition::default();
    def.default_value = "%<field>".into();
    let mut dim = DimensionAligned::default();
    dim.base.text = "%<field>".into();
    let mut user = DimensionAligned::default();
    user.base.user_text = Some("%<field>".into());
    for e in [
        EntityType::Text(text),
        EntityType::MText(Box::new(mtext)),
        EntityType::AttributeEntity(attr),
        EntityType::AttributeDefinition(def),
        EntityType::Dimension(Box::new(Dimension::Aligned(dim))),
        EntityType::Dimension(Box::new(Dimension::Aligned(user))),
    ] {
        let mut d = CadDocument::new();
        d.add_entity(e).unwrap();
        assert_public_coverage_refusal(d);
    }
    let mut d = CadDocument::new();
    let h = d.allocate_handle();
    let mut style = acadrust::objects::MultiLeaderStyle::default();
    style.handle = h;
    style.default_text = "%<field>".into();
    d.objects
        .insert(h, acadrust::objects::ObjectType::MultiLeaderStyle(style));
    assert_public_coverage_refusal(d);
}

#[test]
fn multiline_attribute_flags_and_embedded_payloads_refuse_independently() {
    use acadrust::entities::{AttributeDefinition, AttributeEntity, EntityType, MText, MTextFlag};
    for definition in [false, true] {
        for embedded in [false, true] {
            let e = if definition {
                let mut v = AttributeDefinition::default();
                if embedded {
                    v.embedded_mtext = Some(Box::new(MText::new()));
                } else {
                    v.mtext_flag = MTextFlag::MultiLine;
                }
                EntityType::AttributeDefinition(v)
            } else {
                let mut v = AttributeEntity::default();
                if embedded {
                    v.embedded_mtext = Some(Box::new(MText::new()));
                } else {
                    v.mtext_flag = MTextFlag::MultiLine;
                }
                EntityType::AttributeEntity(v)
            };
            let mut d = CadDocument::new();
            d.add_entity(e).unwrap();
            assert_public_coverage_refusal(d);
        }
    }
}

#[test]
fn null_primary_identity_refuses_but_zero_count_does_not_certify_it() {
    use acadrust::{
        objects::{ObjectType, XRecord},
        types::Handle,
    };
    let mut d = CadDocument::new();
    d.objects
        .insert(Handle::NULL, ObjectType::XRecord(XRecord::new()));
    let before = format!("{d:?}");
    assert!(matches!(
        d.try_reserve_handles(1),
        Err(acadrust::HandleReservationError::InconsistentDocument { .. })
    ));
    assert_eq!(format!("{d:?}"), before);
    assert!(d.try_reserve_handles(0).unwrap().is_empty());
    assert_eq!(format!("{d:?}"), before);
}

#[test]
fn identity_at_exact_old_next_handle_is_not_reissued() {
    use acadrust::{
        objects::{ObjectType, XRecord},
        types::Handle,
    };
    let mut d = CadDocument::new();
    let occupied = Handle::new(d.next_handle());
    let mut x = XRecord::new();
    x.handle = occupied;
    d.objects.insert(occupied, ObjectType::XRecord(x.clone()));
    assert!(d.try_reserve_handles(1).unwrap().get(0).unwrap().value() > occupied.value());
    assert_eq!(d.objects.get(&occupied), Some(&ObjectType::XRecord(x)));
}

#[test]
fn signed_attribute_thickness_is_scalar_and_preserved() {
    use acadrust::entities::{AttributeDefinition, AttributeEntity, EntityType};
    for thickness in [-3.25, 2.5] {
        let mut d = CadDocument::new();
        let mut definition =
            AttributeDefinition::new("scalar".into(), "prompt".into(), "value".into());
        definition.thickness = thickness;
        let mut attribute = AttributeEntity::new("scalar".into(), "value".into());
        attribute.thickness = thickness;
        let def = d
            .add_entity(EntityType::AttributeDefinition(definition))
            .unwrap();
        let attr = d
            .add_entity(EntityType::AttributeEntity(attribute))
            .unwrap();
        let before = (
            d.get_entity(def).unwrap().clone(),
            d.get_entity(attr).unwrap().clone(),
        );
        let r = d.try_reserve_handles(1).unwrap();
        assert!(r.get(0).unwrap().value() > attr.value().max(def.value()));
        assert_eq!(d.get_entity(def).unwrap(), &before.0);
        assert_eq!(d.get_entity(attr).unwrap(), &before.1);
    }
}

#[test]
fn new_map_object_data_classes_remain_uninspectable() {
    use acadrust::objects::{
        ClassObject, ClassObjectData, ObjectDataDescriptor, ObjectDataDictionary, ObjectDataRecord,
        ObjectType,
    };
    use acadrust::types::Handle;
    let kinds = [
        ClassObjectData::ObjectDataDictionary(ObjectDataDictionary {
            entries: vec![("dangling".into(), Handle::new(9000))],
            ..Default::default()
        }),
        ClassObjectData::ObjectDataDescriptor(ObjectDataDescriptor::default()),
        ClassObjectData::ObjectDataRecord(ObjectDataRecord {
            table: Handle::new(9000),
            payload: vec![1, 2, 3],
        }),
    ];
    for kind in kinds {
        let mut d = CadDocument::new();
        let mut v = ClassObject::new(kind);
        v.handle = d.allocate_handle();
        d.objects.insert(v.handle, ObjectType::ClassObject(v));
        assert_public_coverage_refusal(d);
    }
}
