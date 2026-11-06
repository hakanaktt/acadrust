#![allow(clippy::field_reassign_with_default)] // Mutation is the behavior under test.
use super::*;
use crate::types::Handle;
const HIGH: u64 = (1u64 << 53) + 17;

#[test]
fn header_reference_slots_protect_dangling_values() {
    {
        let mut d = CadDocument::new();
        d.header.ucs_ortho_ref = Handle::new(HIGH);
        assert!(census(&d).unwrap() >= HIGH, "header.ucs_ortho_ref");
    }
    {
        let mut d = CadDocument::new();
        d.header.paper_ucs_ortho_ref = Handle::new(HIGH);
        assert!(census(&d).unwrap() >= HIGH, "header.paper_ucs_ortho_ref");
    }
    {
        let mut d = CadDocument::new();
        d.header.current_layer_handle = Handle::new(HIGH);
        assert!(census(&d).unwrap() >= HIGH, "header.current_layer_handle");
    }
    {
        let mut d = CadDocument::new();
        d.header.current_text_style_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.current_text_style_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.current_linetype_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.current_linetype_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.current_dimstyle_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.current_dimstyle_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.current_multiline_style_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.current_multiline_style_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.current_material_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.current_material_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.dim_text_style_handle = Handle::new(HIGH);
        assert!(census(&d).unwrap() >= HIGH, "header.dim_text_style_handle");
    }
    {
        let mut d = CadDocument::new();
        d.header.dim_linetype_handle = Handle::new(HIGH);
        assert!(census(&d).unwrap() >= HIGH, "header.dim_linetype_handle");
    }
    {
        let mut d = CadDocument::new();
        d.header.dim_linetype1_handle = Handle::new(HIGH);
        assert!(census(&d).unwrap() >= HIGH, "header.dim_linetype1_handle");
    }
    {
        let mut d = CadDocument::new();
        d.header.dim_linetype2_handle = Handle::new(HIGH);
        assert!(census(&d).unwrap() >= HIGH, "header.dim_linetype2_handle");
    }
    {
        let mut d = CadDocument::new();
        d.header.dim_arrow_block_handle = Handle::new(HIGH);
        assert!(census(&d).unwrap() >= HIGH, "header.dim_arrow_block_handle");
    }
    {
        let mut d = CadDocument::new();
        d.header.dim_arrow_block1_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.dim_arrow_block1_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.dim_arrow_block2_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.dim_arrow_block2_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.dim_leader_arrow_block_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.dim_leader_arrow_block_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.current_vx_handle = Handle::new(HIGH);
        assert!(census(&d).unwrap() >= HIGH, "header.current_vx_handle");
    }
    {
        let mut d = CadDocument::new();
        d.header.named_objects_dict_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.named_objects_dict_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.acad_group_dict_handle = Handle::new(HIGH);
        assert!(census(&d).unwrap() >= HIGH, "header.acad_group_dict_handle");
    }
    {
        let mut d = CadDocument::new();
        d.header.acad_mlinestyle_dict_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.acad_mlinestyle_dict_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.acad_layout_dict_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.acad_layout_dict_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.acad_plotsettings_dict_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.acad_plotsettings_dict_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.acad_plotstylename_dict_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.acad_plotstylename_dict_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.acad_material_dict_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.acad_material_dict_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.acad_color_dict_handle = Handle::new(HIGH);
        assert!(census(&d).unwrap() >= HIGH, "header.acad_color_dict_handle");
    }
    {
        let mut d = CadDocument::new();
        d.header.acad_visualstyle_dict_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.acad_visualstyle_dict_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.model_space_block_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.model_space_block_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.paper_space_block_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.paper_space_block_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.bylayer_linetype_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.bylayer_linetype_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.byblock_linetype_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.byblock_linetype_handle"
        );
    }
    {
        let mut d = CadDocument::new();
        d.header.continuous_linetype_handle = Handle::new(HIGH);
        assert!(
            census(&d).unwrap() >= HIGH,
            "header.continuous_linetype_handle"
        );
    }
}

#[test]
fn private_reference_slots_protect_orphans() {
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.vx_control_entries.push(h);
        assert!(census(&d).unwrap() >= HIGH, "vx_control_entries");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.xdic_by_handle.insert(h, Handle::NULL);
        assert!(census(&d).unwrap() >= HIGH, "xdic_by_handle.owner");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.xdic_by_handle.insert(Handle::NULL, h);
        assert!(census(&d).unwrap() >= HIGH, "xdic_by_handle.target");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.reactors_by_handle.insert(h, vec![]);
        assert!(census(&d).unwrap() >= HIGH, "reactors_by_handle.owner");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.reactors_by_handle.insert(Handle::NULL, vec![h]);
        assert!(census(&d).unwrap() >= HIGH, "reactors_by_handle.target");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.block_entity_handles.insert(h, vec![]);
        assert!(census(&d).unwrap() >= HIGH, "block_entity_handles.owner");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.block_entity_handles.insert(Handle::NULL, vec![h]);
        assert!(census(&d).unwrap() >= HIGH, "block_entity_handles.child");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.context_scales.insert(h, Handle::NULL);
        assert!(census(&d).unwrap() >= HIGH, "context_scales.key");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.context_scales.insert(Handle::NULL, h);
        assert!(census(&d).unwrap() >= HIGH, "context_scales.value");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.block_representations.insert(h, Handle::NULL);
        assert!(census(&d).unwrap() >= HIGH, "block_representations.key");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.block_representations.insert(Handle::NULL, h);
        assert!(census(&d).unwrap() >= HIGH, "block_representations.value");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.view_rep_refs.insert(h, vec![]);
        assert!(census(&d).unwrap() >= HIGH, "view_rep_refs.key");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.view_rep_refs.insert(Handle::NULL, vec![h]);
        assert!(census(&d).unwrap() >= HIGH, "view_rep_refs.value");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.section_view_reps.push(h);
        assert!(census(&d).unwrap() >= HIGH, "section_view_reps");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.acis_sab_handles.push(h);
        assert!(census(&d).unwrap() >= HIGH, "acis_sab_handles");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.dwg_data_store_handles.insert(h);
        assert!(census(&d).unwrap() >= HIGH, "dwg_data_store_handles");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.r12_table_usage.insert(h, 0);
        assert!(census(&d).unwrap() >= HIGH, "r12_table_usage");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.r12_block_tags.insert(h, 0);
        assert!(census(&d).unwrap() >= HIGH, "r12_block_tags");
    }
    {
        let mut d = CadDocument::new();
        d.raw_record_owners.insert(HIGH, 0);
        assert!(census(&d).unwrap() >= HIGH, "raw_record_owners.child");
    }
    {
        let mut d = CadDocument::new();
        d.raw_record_owners.insert(0, HIGH);
        assert!(census(&d).unwrap() >= HIGH, "raw_record_owners.owner");
    }
    {
        let mut d = CadDocument::new();
        d.raw_acds_fingerprint.push((HIGH, 0, 0));
        assert!(census(&d).unwrap() >= HIGH, "raw_acds_fingerprint");
    }
    {
        let mut d = CadDocument::new();
        let h = Handle::new(HIGH);
        d.object_xdata.insert(h, crate::xdata::ExtendedData::new());
        assert!(census(&d).unwrap() >= HIGH, "object_xdata.owner");
    }
}

#[test]
fn scalar_fingerprints_do_not_raise_floor() {
    let mut d = CadDocument::new();
    let before = census(&d).unwrap();
    d.raw_acds_fingerprint.push((0, usize::MAX, u64::MAX));
    assert_eq!(census(&d).unwrap(), before);
}

#[test]
fn raw_sources_refuse_without_mutation() {
    let mut d = CadDocument::new();
    let next = d.next_handle;
    let seed = d.header.handle_seed;
    d.raw_acds_data = Some(std::sync::Arc::new(vec![]));
    assert!(matches!(
        d.try_reserve_handles(1),
        Err(HandleReservationError::UninspectableData { .. })
    ));
    assert_eq!(d.next_handle, next);
    assert_eq!(d.header.handle_seed, seed);
    assert!(d.raw_acds_data.as_ref().unwrap().is_empty());
    assert!(d.try_reserve_handles(0).unwrap().is_empty());
}
#[test]
fn entity_index_corruption_is_not_repaired() {
    let mut d = CadDocument::new();
    d.entity_index.insert(Handle::new(HIGH), usize::MAX);
    assert!(matches!(
        census(&d),
        Err(HandleReservationError::InconsistentDocument { .. })
    ));
    assert_eq!(d.entity_index[&Handle::new(HIGH)], usize::MAX);
}

#[test]
fn failure_and_success_preserve_private_raw_nan_and_history_state() {
    use crate::entities::{EntityType, Line};
    use crate::notification::NotificationType;
    for case in 0..4 {
        let mut d = CadDocument::new();
        let mut line = Line::new();
        line.start.x = f64::from_bits(0x7ff8000000000042);
        let h = d.add_entity(EntityType::Line(line)).unwrap();
        let rec = d.begin_entity_change_recording();
        d.get_entity_mut(h).unwrap();
        d.notifications
            .notify(NotificationType::Warning, "existing diagnostic");
        match case {
            0 => d.header.handle_seed = u64::MAX,
            1 => d.layers.get_mut("0").unwrap().name = "stale".into(),
            2 => d.raw_acds_data = Some(std::sync::Arc::new(vec![1, 2, 3])),
            _ => {}
        }
        let before = format!("{:?}", d);
        let next = d.next_handle;
        let seed = d.header.handle_seed;
        let image = d
            .entities
            .iter()
            .find(|e| e.common().handle == h)
            .unwrap()
            .clone();
        let raw = d.raw_acds_data.clone();
        let order = rec.inner.lock().unwrap().order.clone();
        let before_count = rec.inner.lock().unwrap().before.len();
        let result = d.try_reserve_handles(2);
        if case < 3 {
            assert!(result.is_err());
            assert_eq!(format!("{:?}", d), before);
            assert_eq!((d.next_handle, d.header.handle_seed), (next, seed));
        } else {
            let r = result.unwrap();
            let end = r.get(1).unwrap().value() + 1;
            assert_eq!((d.next_handle, d.header.handle_seed), (end, end));
            d.next_handle = next;
            d.header.handle_seed = seed;
            assert_eq!(format!("{:?}", d), before);
        }
        assert!(std::sync::Arc::ptr_eq(
            &image,
            d.entities.iter().find(|e| e.common().handle == h).unwrap()
        ));
        let EntityType::Line(l) = d.get_entity(h).unwrap() else {
            unreachable!()
        };
        assert_eq!(l.start.x.to_bits(), 0x7ff8000000000042);
        if let Some(raw) = raw {
            assert!(std::sync::Arc::ptr_eq(
                &raw,
                d.raw_acds_data.as_ref().unwrap()
            ));
        }
        assert!(std::sync::Arc::ptr_eq(
            &rec,
            &d.active_entity_change_recorder().unwrap()
        ));
        assert_eq!(rec.inner.lock().unwrap().order, order);
        assert_eq!(rec.inner.lock().unwrap().before.len(), before_count);
        d.end_entity_change_recording();
    }
}
#[test]
fn virtual_block_alias_accepts_only_matching_kind_and_owner() {
    use crate::entities::{Block, EntityType, Line};
    for wrong in [false, true] {
        let mut d = CadDocument::new();
        let br = d.header.model_space_block_handle;
        let marker = Handle::new(700);
        d.block_records
            .get_mut("*Model_Space")
            .unwrap()
            .block_entity_handle = marker;
        let mut e = if wrong {
            EntityType::Line(Line::new())
        } else {
            EntityType::Block(Block::new("*Model_Space", crate::types::Vector3::ZERO))
        };
        e.common_mut().handle = marker;
        e.common_mut().owner_handle = br;
        let slot = d.entities.len();
        d.entities.push(std::sync::Arc::new(e));
        d.entity_index.insert(marker, slot);
        assert_eq!(census(&d).is_ok(), !wrong);
    }
}

#[test]
fn private_raw_stores_are_all_refused_without_discard() {
    for case in 0..5 {
        let mut d = CadDocument::new();
        match case {
            0 => {
                d.r12_header_raw = Some(vec![]);
            }
            1 => {
                d.eed_by_handle.insert(Handle::new(700), vec![]);
                d.object_xdata
                    .insert(Handle::new(700), crate::xdata::ExtendedData::new());
            }
            2 => {
                d.original_objects.insert(
                    Handle::new(700),
                    crate::objects::ObjectType::Dictionary(crate::objects::Dictionary::new()),
                );
            }
            3 => {
                d.raw_records.insert(
                    700,
                    (
                        0,
                        std::sync::Arc::new(crate::entities::RawRecord {
                            data: vec![1, 2],
                            handle_bits: 0,
                            version: d.version,
                        }),
                    ),
                );
            }
            4 => {
                d.block_visibility_params.insert(
                    Handle::new(700),
                    crate::objects::BlockVisibilityParameter::default(),
                );
            }
            _ => unreachable!(),
        }
        let before = format!("{:?}", d);
        assert!(matches!(
            d.try_reserve_handles(1),
            Err(HandleReservationError::UninspectableData { .. })
        ));
        assert_eq!(format!("{:?}", d), before);
    }
}
#[test]
fn missing_index_and_cross_domain_duplicate_are_errors() {
    use crate::entities::{EntityType, Line};
    let mut d = CadDocument::new();
    let h = d.add_entity(EntityType::Line(Line::new())).unwrap();
    let slot = d.entity_index.remove(&h).unwrap();
    d.entity_index.insert(Handle::new(HIGH), slot);
    assert!(census(&d).is_err());
    let mut d = CadDocument::new();
    let mut x = crate::objects::XRecord::new();
    x.handle = d.layers.get("0").unwrap().handle;
    d.objects
        .insert(x.handle, crate::objects::ObjectType::XRecord(x));
    assert!(matches!(
        census(&d),
        Err(HandleReservationError::InconsistentDocument { .. })
    ));
}

#[test]
fn success_is_map_order_independent_and_clones_are_independent() {
    let original = CadDocument::new();
    let mut a = original.clone();
    let mut b = original.clone();
    let entries: Vec<_> = b.objects.drain().collect();
    for (k, v) in entries.into_iter().rev() {
        b.objects.insert(k, v);
    }
    let r1 = a.try_reserve_handles(4).unwrap();
    let r2 = b.try_reserve_handles(4).unwrap();
    assert_eq!(
        r1.into_iter().collect::<Vec<_>>(),
        r2.into_iter().collect::<Vec<_>>()
    );
    assert_eq!(
        original.header.handle_seed,
        CadDocument::new().header.handle_seed
    );
}
#[test]
fn representable_exhausted_state_is_atomic_on_next_request() {
    let mut d = CadDocument::new();
    d.next_handle = u64::MAX - 1;
    let r = d.try_reserve_handles(1).unwrap();
    assert_eq!(r.get(0), Some(Handle::new(u64::MAX - 1)));
    assert_eq!(d.next_handle, u64::MAX);
    assert_eq!(d.header.handle_seed, u64::MAX);
    let before = format!("{:?}", d);
    assert_eq!(
        d.try_reserve_handles(1).unwrap_err(),
        HandleReservationError::Exhausted
    );
    assert_eq!(format!("{:?}", d), before);
    assert!(d.try_reserve_handles(0).unwrap().is_empty());
}

#[test]
fn nested_errors_name_the_exact_storage_slot() {
    let mut d = CadDocument::new();
    let mut x = crate::objects::XRecord::new();
    x.handle = Handle::new(700);
    x.entries
        .push(crate::objects::XRecordEntry::string(340, "uninspectable"));
    d.objects
        .insert(x.handle, crate::objects::ObjectType::XRecord(x));
    let HandleReservationError::UninspectableData { field_path, .. } =
        d.try_reserve_handles(1).unwrap_err()
    else {
        panic!("wrong error category")
    };
    assert!(field_path.contains("entries[0]"), "{field_path}");
    assert!(field_path.contains("objects["), "{field_path}");
}

#[test]
fn remaining_side_stores_are_explicit_coverage_refusals() {
    use crate::objects::{DgnLsComponent, DgnLsComponentType, DgnLsDefinition};
    for case in 0..4 {
        let mut d = CadDocument::new();
        match case {
            0 => {
                d.dgn_ls_definitions.insert(
                    Handle::new(700),
                    DgnLsDefinition {
                        handle: Handle::new(700),
                        name: String::new(),
                        root_component: Handle::NULL,
                    },
                );
            }
            1 => {
                d.dgn_ls_components.insert(
                    Handle::new(700),
                    DgnLsComponent {
                        handle: Handle::new(700),
                        component_type: DgnLsComponentType::Internal,
                        description: String::new(),
                        refs: vec![],
                        scale: 1.,
                    },
                );
            }
            2 => {
                d.fields.insert(
                    Handle::new(700),
                    super::super::FieldDef {
                        handle: Handle::new(700),
                        owner: Handle::NULL,
                        evaluator: String::new(),
                        code: String::new(),
                        objects: vec![],
                    },
                );
            }
            3 => {
                d.section_view_style = Some(crate::entities::SectionViewStyle {
                    show_arrows: false,
                    show_plane_line: false,
                    show_end_lines: false,
                    arrow_size: 0.,
                    arrow_extension: 0.,
                    label_height: 0.,
                    label_offset: 0.,
                    label_position: 0,
                    arrow_position: 0,
                    end_line_length: 0.,
                    end_line_overshoot: 0.,
                    arrow_start_handle: 0,
                    arrow_end_handle: 0,
                    arrow_is_default: true,
                });
            }
            _ => unreachable!(),
        }
        let before = format!("{:?}", d);
        assert!(matches!(
            d.try_reserve_handles(1),
            Err(HandleReservationError::UninspectableData { .. })
        ));
        assert_eq!(format!("{:?}", d), before);
    }
}
