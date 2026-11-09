use super::*;
use crate::objects::*;
fn style(v: &NamedTableCellStyle, u: &mut HandleUsage) {
    let NamedTableCellStyle {
        cell_style: _,
        id: _,
        style_type: _,
        name: _,
    } = v;
    let c = &v.cell_style;
    let TableCellStyleData {
        style_type: _,
        data_flags: _,
        property_override_flags: _,
        merge_flags: _,
        background_color: _,
        content_layout: _,
        content_format: _,
        margin_override_flags: _,
        vertical_margin: _,
        horizontal_margin: _,
        bottom_margin: _,
        right_margin: _,
        horizontal_spacing: _,
        vertical_spacing: _,
        borders: _,
    } = c;
    let f = &c.content_format;
    let TableContentFormat {
        property_override_flags: _,
        property_flags: _,
        value_data_type: _,
        value_unit_type: _,
        value_format_string: _,
        rotation: _,
        block_scale: _,
        cell_alignment: _,
        content_color: _,
        text_style: _,
        text_height: _,
    } = f;
    u.protect(f.text_style);
    for b in &c.borders {
        let TableGridFormat {
            index_mask: _,
            border: _,
            line_type: _,
        } = b;
        u.protect(b.line_type);
    }
}
fn context(v: &ObjectContextData, u: &mut HandleUsage) -> Result<()> {
    match &v.kind {
        ObjectContextKind::AnnotScale => {}
        ObjectContextKind::BlkRef {
            rotation: _,
            insertion: _,
            scale_factor: _,
        } => {}
        ObjectContextKind::Text {
            horizontal_mode: _,
            rotation: _,
            insertion: _,
            alignment: _,
        } => {}
        ObjectContextKind::MText(c) => {
            let MTextContext {
                attachment: _,
                x_axis_dir: _,
                insertion: _,
                rect_width: _,
                rect_height: _,
                extents_width: _,
                extents_height: _,
                column_type: _,
                columns: _,
            } = c;
            if let Some(cols) = &c.columns {
                let MTextColumns {
                    num_heights: _,
                    width: _,
                    gutter: _,
                    auto_height: _,
                    flow_reversed: _,
                    heights: _,
                } = cols;
            }
        }
        ObjectContextKind::Dim(d) => {
            let DimContext {
                def_pt: _,
                is_def_textloc: _,
                text_rotation: _,
                block: _,
                b293: _,
                dimtofl: _,
                dimosxd: _,
                dimatfit: _,
                dimtix: _,
                dimtmove: _,
                override_code: _,
                has_arrow2: _,
                flip_arrow2: _,
                flip_arrow1: _,
                subtype: _,
            } = d;
            u.protect(d.block);
            match &d.subtype {
                DimSubtype::Aligned { dimline_pt: _ }
                | DimSubtype::Angular { arc_pt: _ }
                | DimSubtype::Radial { first_arc_pt: _ } => {}
                DimSubtype::Diametric {
                    first_arc_pt: _,
                    def_pt: _,
                }
                | DimSubtype::RadialLarge {
                    ovr_center: _,
                    jog_point: _,
                }
                | DimSubtype::Ordinate {
                    feature_location_pt: _,
                    leader_endpt: _,
                } => {}
            }
        }
        ObjectContextKind::HatchScale(h) => super::entities::hatch_context(h, u),
        ObjectContextKind::HatchView(h) => {
            let HatchViewContext {
                hatch: _,
                view: _,
                view_normal: _,
                view_rotation: _,
                evaluate_hatch: _,
            } = h;
            u.protect(h.view);
            super::entities::hatch_context(&h.hatch, u);
        }
        ObjectContextKind::Opaque
        | ObjectContextKind::MLeader(_)
        | ObjectContextKind::Leader(_)
        | ObjectContextKind::MTextAttribute(_)
        | ObjectContextKind::Fcf { .. } => return Err(u.unsupported("object_context.kind")),
    }
    Ok(())
}
fn xrecord(v: &XRecord, u: &mut HandleUsage) -> Result<()> {
    if !v.entries_complete {
        return Err(u.unsupported("xrecord.entries_complete"));
    }
    if !v.raw_data.is_empty() {
        return Err(u.unsupported("xrecord.raw_data"));
    }
    if v.raw_dwg_data.is_some() {
        return Err(u.unsupported("xrecord.raw_dwg_data"));
    }

    for r in &v.object_references {
        let ProxyObjectReference { handle: _, kind: _ } = r;
        u.protect(r.handle);
    }
    for (index, e) in v.entries.iter().enumerate() {
        let path = || format!("xrecord.entries[{index}].value");
        let XRecordEntry { code: _, value: _ } = e;

        let kind = XRecordValueType::from_code(e.code);
        let handle_code = kind.is_handle() || matches!(e.code, 5 | 105 | 320..=329);
        match &e.value {
            XRecordValue::Handle(h) => {
                u.protect(*h);
                if !handle_code {
                    return Err(u.bad(&path(), Some(*h)));
                }
            }
            XRecordValue::String(_) => {
                if handle_code {
                    return Err(u.unsupported(&path()));
                }
                if kind != XRecordValueType::String {
                    return Err(u.bad(&path(), None));
                }
            }
            XRecordValue::Double(_) => {
                if kind != XRecordValueType::Double {
                    return Err(u.bad(&path(), None));
                }
            }
            XRecordValue::Int16(_) => {
                if kind != XRecordValueType::Int16 {
                    return Err(u.bad(&path(), None));
                }
            }
            XRecordValue::Int32(_) => {
                if kind != XRecordValueType::Int32 {
                    return Err(u.bad(&path(), None));
                }
            }
            XRecordValue::Int64(_) => {
                if kind != XRecordValueType::Int64 {
                    return Err(u.bad(&path(), None));
                }
            }
            XRecordValue::Byte(_) => {
                if kind != XRecordValueType::Byte {
                    return Err(u.bad(&path(), None));
                }
            }
            XRecordValue::Bool(_) => {
                if kind != XRecordValueType::Bool {
                    return Err(u.bad(&path(), None));
                }
            }
            XRecordValue::Point3D(_, _, _) => {
                if kind != XRecordValueType::Point3D {
                    return Err(u.bad(&path(), None));
                }
            }
            XRecordValue::Chunk(_) => {
                if kind != XRecordValueType::Chunk {
                    return Err(u.bad(&path(), None));
                }
            }
        }
    }
    Ok(())
}
pub(super) fn visit(key: Handle, o: &ObjectType, u: &mut HandleUsage) -> Result<()> {
    match o {
        ObjectType::Dictionary(v) => {
            if key != v.handle {
                return Err(u.bad("objects.key/intrinsic", Some(v.handle)));
            }
            u.primary(key, "objects.handle")?;
            let Dictionary {
                handle: _,
                owner: _,
                entries: _,
                hard_owner_entries: _,
                duplicate_cloning: _,
                hard_owner: _,
                reactors: _,
                xdictionary_handle: _,
            } = v;
            u.protect(v.owner);
            u.refs(&v.reactors);
            u.optional(v.xdictionary_handle);
            for (_, h) in &v.entries {
                u.protect(*h);
            }
        }
        ObjectType::Layout(v) => {
            if key != v.handle {
                return Err(u.bad("objects.key/intrinsic", Some(v.handle)));
            }
            u.primary(key, "objects.handle")?;
            let Layout {
                handle: _,
                owner: _,
                name: _,
                flags: _,
                tab_order: _,
                min_limits: _,
                max_limits: _,
                insertion_base: _,
                min_extents: _,
                max_extents: _,
                elevation: _,
                ucs_origin: _,
                ucs_x_axis: _,
                ucs_y_axis: _,
                ucs_ortho_type: _,
                block_record: _,
                viewport: _,
                viewports: _,
                base_ucs: _,
                named_ucs: _,
                reactors: _,
                xdictionary_handle: _,
                raw_plot_settings_codes: _,
                paper_width: _,
                paper_height: _,
                plot_rotation: _,
                plot_flags: _,
                plot_page_name: _,
                plot_printer_name: _,
                paper_size: _,
                plot_view_name: _,
                plot_style_sheet: _,
                plot_margin_left: _,
                plot_margin_bottom: _,
                plot_margin_right: _,
                plot_margin_top: _,
                plot_origin_x: _,
                plot_origin_y: _,
                plot_window_min_x: _,
                plot_window_min_y: _,
                plot_window_max_x: _,
                plot_window_max_y: _,
                plot_paper_units: _,
                plot_type: _,
                plot_scale_numerator: _,
                plot_scale_denominator: _,
                plot_scale_type: _,
                plot_scale_factor: _,
                paper_image_origin_x: _,
                paper_image_origin_y: _,
                shade_plot_mode: _,
                shade_plot_resolution: _,
                shade_plot_dpi: _,
                plot_view_handle: _,
                visual_style_handle: _,
            } = v;
            u.protect(v.owner);
            u.protect(v.block_record);
            u.protect(v.viewport);
            u.refs(&v.viewports);
            u.protect(v.base_ucs);
            u.protect(v.named_ucs);
            u.refs(&v.reactors);
            u.optional(v.xdictionary_handle);
            u.protect(v.plot_view_handle);
            u.protect(v.visual_style_handle);
            if v.raw_plot_settings_codes.is_some() {
                return Err(u.unsupported("layout.raw_plot_settings_codes"));
            }
        }
        ObjectType::XRecord(v) => {
            if key != v.handle {
                return Err(u.bad("objects.key/intrinsic", Some(v.handle)));
            }
            u.primary(key, "objects.handle")?;
            let XRecord {
                handle: _,
                owner: _,
                reactors: _,
                xdictionary_handle: _,
                name: _,
                cloning_flags: _,
                entries: _,
                object_references: _,
                preserve_object_reference_stream: _,
                entries_complete: _,
                raw_data: _,
                raw_dwg_data: _,
                raw_dwg_handle_bits: _,
                raw_dwg_version: _,
            } = v;
            u.protect(v.owner);
            u.refs(&v.reactors);
            u.optional(v.xdictionary_handle);
            xrecord(v, u)?;
        }
        ObjectType::MLineStyle(v) => {
            if key != v.handle {
                return Err(u.bad("objects.key/intrinsic", Some(v.handle)));
            }
            u.primary(key, "objects.handle")?;
            let MLineStyle {
                handle: _,
                owner: _,
                name: _,
                description: _,
                elements: _,
                start_angle: _,
                end_angle: _,
                fill_color: _,
                flags: _,
            } = v;
            u.protect(v.owner);
        }
        ObjectType::PlotSettings(v) => {
            if key != v.handle {
                return Err(u.bad("objects.key/intrinsic", Some(v.handle)));
            }
            u.primary(key, "objects.handle")?;
            v.reservation_fields();
            u.protect(v.owner);
            u.refs(&v.reactors);
            u.optional(v.xdictionary_handle);
            u.protect(v.plot_view_handle);
            u.protect(v.visual_style_handle);
        }
        ObjectType::MultiLeaderStyle(v) => {
            if key != v.handle {
                return Err(u.bad("objects.key/intrinsic", Some(v.handle)));
            }
            u.primary(key, "objects.handle")?;
            let MultiLeaderStyle {
                handle: _,
                owner_handle: _,
                name: _,
                description: _,
                path_type: _,
                line_color: _,
                line_type_handle: _,
                line_weight: _,
                enable_landing: _,
                enable_dogleg: _,
                landing_distance: _,
                landing_gap: _,
                arrowhead_handle: _,
                arrowhead_size: _,
                content_type: _,
                text_style_handle: _,
                text_left_attachment: _,
                text_right_attachment: _,
                text_top_attachment: _,
                text_bottom_attachment: _,
                text_attachment_direction: _,
                text_angle_type: _,
                text_alignment: _,
                text_color: _,
                text_height: _,
                text_frame: _,
                text_always_left: _,
                default_text: _,
                block_content_handle: _,
                block_content_color: _,
                block_content_connection: _,
                block_content_rotation: _,
                block_content_scale_x: _,
                block_content_scale_y: _,
                block_content_scale_z: _,
                enable_block_scale: _,
                enable_block_rotation: _,
                scale_factor: _,
                align_space: _,
                break_gap_size: _,
                max_leader_points: _,
                first_segment_angle: _,
                second_segment_angle: _,
                leader_draw_order: _,
                multileader_draw_order: _,
                is_annotative: _,
                property_changed: _,
                unknown_flag_298: _,
            } = v;
            u.protect(v.owner_handle);
            u.optional(v.line_type_handle);
            u.optional(v.arrowhead_handle);
            u.optional(v.text_style_handle);
            u.optional(v.block_content_handle);
            u.text(&v.default_text, "multileader_style.default_text")?;
        }
        ObjectType::TableStyle(v) => {
            if key != v.handle {
                return Err(u.bad("objects.key/intrinsic", Some(v.handle)));
            }
            u.primary(key, "objects.handle")?;
            let TableStyle {
                handle: _,
                owner_handle: _,
                name: _,
                description: _,
                version: _,
                flow_direction: _,
                flags: _,
                horizontal_margin: _,
                vertical_margin: _,
                title_suppressed: _,
                header_suppressed: _,
                data_row_style: _,
                header_row_style: _,
                title_row_style: _,
                modern_unknown_byte: _,
                modern_unknown_long1: _,
                modern_unknown_long2: _,
                modern_cell_style_handle: _,
                modern_style: _,
                modern_overrides: _,
                annotative: _,
                raw_dxf_codes: _,
            } = v;
            u.protect(v.owner_handle);
            u.protect(v.modern_cell_style_handle);
            if v.raw_dxf_codes.is_some() {
                return Err(u.unsupported("table_style.raw_dxf_codes"));
            }
            for r in [&v.data_row_style, &v.header_row_style, &v.title_row_style] {
                let RowCellStyle {
                    text_style_name: _,
                    text_style_handle: _,
                    text_height: _,
                    alignment: _,
                    text_color: _,
                    fill_color: _,
                    fill_enabled: _,
                    data_type: _,
                    unit_type: _,
                    format_string: _,
                    left_border: _,
                    right_border: _,
                    top_border: _,
                    bottom_border: _,
                    horizontal_inside_border: _,
                    vertical_inside_border: _,
                } = r;
                u.optional(r.text_style_handle);
            }
            if let Some(s) = &v.modern_style {
                style(s, u);
            }
            for (_, s) in &v.modern_overrides {
                style(s, u);
            }
        }
        ObjectType::Scale(v) => {
            if key != v.handle {
                return Err(u.bad("objects.key/intrinsic", Some(v.handle)));
            }
            u.primary(key, "objects.handle")?;
            let Scale {
                handle: _,
                owner_handle: _,
                name: _,
                paper_units: _,
                drawing_units: _,
                is_unit_scale: _,
                is_temporary: _,
            } = v;
            u.protect(v.owner_handle);
        }
        ObjectType::ObjectContextData(v) => {
            if key != v.handle {
                return Err(u.bad("objects.key/intrinsic", Some(v.handle)));
            }
            u.primary(key, "objects.handle")?;
            let ObjectContextData {
                handle: _,
                owner_handle: _,
                reactors: _,
                xdictionary_handle: _,
                class_version: _,
                has_file_to_extension_dictionary: _,
                uses_legacy_mleader_context_base: _,
                is_default: _,
                scale: _,
                kind: _,
            } = v;
            u.protect(v.owner_handle);
            u.refs(&v.reactors);
            u.optional(v.xdictionary_handle);
            u.protect(v.scale);
            context(v, u)?;
        }
        ObjectType::SortEntitiesTable(v) => {
            if key != v.handle {
                return Err(u.bad("objects.key/intrinsic", Some(v.handle)));
            }
            u.primary(key, "objects.handle")?;
            u.protect(v.owner_handle);
            u.protect(v.block_owner_handle);
            v.reservation_inspect(u)?;
        }
        ObjectType::DictionaryVariable(v) => {
            if key != v.handle {
                return Err(u.bad("objects.key/intrinsic", Some(v.handle)));
            }
            u.primary(key, "objects.handle")?;
            let DictionaryVariable {
                handle: _,
                owner_handle: _,
                schema_number: _,
                value: _,
                name: _,
            } = v;
            u.protect(v.owner_handle);
        }
        ObjectType::PlaceHolder(v) => {
            if key != v.handle {
                return Err(u.bad("objects.key/intrinsic", Some(v.handle)));
            }
            u.primary(key, "objects.handle")?;
            let PlaceHolder {
                handle: _,
                owner: _,
            } = v;
            u.protect(v.owner);
        }
        ObjectType::DictionaryWithDefault(v) => {
            if key != v.handle {
                return Err(u.bad("objects.key/intrinsic", Some(v.handle)));
            }
            u.primary(key, "objects.handle")?;
            let DictionaryWithDefault {
                handle: _,
                owner: _,
                entries: _,
                default_handle: _,
                duplicate_cloning: _,
                hard_owner: _,
            } = v;
            u.protect(v.owner);
            u.protect(v.default_handle);
            for (_, h) in &v.entries {
                u.protect(*h);
            }
        }
        ObjectType::Associative(v) => {
            if key != v.handle {
                return Err(u.bad("objects.key/intrinsic", Some(v.handle)));
            }
            u.primary(key, "objects.handle")?;
            let AssociativeObject {
                handle: _,
                owner: _,
                reactors: _,
                xdictionary_handle: _,
                dxf_name: _,
                cpp_class_name: _,
                data: _,
                source_version: _,
            } = v;
            u.protect(v.owner);
            u.refs(&v.reactors);
            u.optional(v.xdictionary_handle);
            match &v.data {
                AssociativeData::DimensionAssociation(a) => {
                    let AssocDimensionAssociation {
                        associativity: _,
                        trans_space: _,
                        rotated_type: _,
                        dimension: _,
                        references: _,
                    } = a;
                    u.protect(a.dimension);
                    for chain in &a.references {
                        for r in chain {
                            let AssocDimensionReference {
                                class_name: _,
                                osnap_type: _,
                                xrefs: _,
                                main_subent_type: _,
                                main_gs_marker: _,
                                xref_paths: _,
                                osnap_distance: _,
                                osnap_point: _,
                                intersection_objects: _,
                                intersection_subent_type: _,
                                intersection_gs_marker: _,
                                intersection_xref_paths: _,
                                has_last_point_reference: _,
                            } = r;
                            u.refs(&r.xrefs);
                            u.refs(&r.intersection_objects);
                        }
                    }
                }
                AssociativeData::Unknown => return Err(u.unsupported("associative.Unknown")),
                AssociativeData::Dependency(_) => {
                    return Err(u.unsupported("associative.Dependency"))
                }
                AssociativeData::ValueDependency(_) => {
                    return Err(u.unsupported("associative.ValueDependency"))
                }
                AssociativeData::GeomDependency(_) => {
                    return Err(u.unsupported("associative.GeomDependency"))
                }
                AssociativeData::SurfaceActionBody(_) => {
                    return Err(u.unsupported("associative.SurfaceActionBody"))
                }
                AssociativeData::Action(_) => return Err(u.unsupported("associative.Action")),
                AssociativeData::Network(_) => return Err(u.unsupported("associative.Network")),
                AssociativeData::AnnotationActionBody(_) => {
                    return Err(u.unsupported("associative.AnnotationActionBody"))
                }
                AssociativeData::PersSubentManager(_) => {
                    return Err(u.unsupported("associative.PersSubentManager"))
                }
                AssociativeData::EdgeActionParam(_) => {
                    return Err(u.unsupported("associative.EdgeActionParam"))
                }
                AssociativeData::ConstraintGroup(_) => {
                    return Err(u.unsupported("associative.ConstraintGroup"))
                }
                AssociativeData::Variable(_) => return Err(u.unsupported("associative.Variable")),
                AssociativeData::ActionParam(_) => {
                    return Err(u.unsupported("associative.ActionParam"))
                }
                AssociativeData::CompoundActionParam(_) => {
                    return Err(u.unsupported("associative.CompoundActionParam"))
                }
                AssociativeData::OsnapPointRefActionParam(_) => {
                    return Err(u.unsupported("associative.OsnapPointRefActionParam"))
                }
                AssociativeData::PointRefActionParam(_) => {
                    return Err(u.unsupported("associative.PointRefActionParam"))
                }
                AssociativeData::ObjectActionParam(_) => {
                    return Err(u.unsupported("associative.ObjectActionParam"))
                }
                AssociativeData::PathActionParam(_) => {
                    return Err(u.unsupported("associative.PathActionParam"))
                }
                AssociativeData::DimDependencyBody(_) => {
                    return Err(u.unsupported("associative.DimDependencyBody"))
                }
                AssociativeData::FaceActionParam(_) => {
                    return Err(u.unsupported("associative.FaceActionParam"))
                }
                AssociativeData::VertexActionParam(_) => {
                    return Err(u.unsupported("associative.VertexActionParam"))
                }
                AssociativeData::AsmBodyActionParam(_) => {
                    return Err(u.unsupported("associative.AsmBodyActionParam"))
                }
                AssociativeData::ArrayParameters(_) => {
                    return Err(u.unsupported("associative.ArrayParameters"))
                }
                AssociativeData::ArrayActionBody(_) => {
                    return Err(u.unsupported("associative.ArrayActionBody"))
                }
                AssociativeData::ArrayModifyActionBody(_) => {
                    return Err(u.unsupported("associative.ArrayModifyActionBody"))
                }
                AssociativeData::PersSubentManagerStatic(_) => {
                    return Err(u.unsupported("associative.PersSubentManagerStatic"))
                }
                AssociativeData::ViewRepActionBody(_) => {
                    return Err(u.unsupported("associative.ViewRepActionBody"))
                }
                AssociativeData::ViewObjectActionParam(_) => {
                    return Err(u.unsupported("associative.ViewObjectActionParam"))
                }
                AssociativeData::ViewRepHatchManager(_) => {
                    return Err(u.unsupported("associative.ViewRepHatchManager"))
                }
                AssociativeData::ViewRepHatchActionParam(_) => {
                    return Err(u.unsupported("associative.ViewRepHatchActionParam"))
                }
                AssociativeData::ViewLabelActionParam(_) => {
                    return Err(u.unsupported("associative.ViewLabelActionParam"))
                }
                AssociativeData::SmartCenterActionBody(_) => {
                    return Err(u.unsupported("associative.SmartCenterActionBody"))
                }
            }
        }
        ObjectType::Group(_) => return Err(u.unsupported("object.Group")),
        ObjectType::ImageDefinition(_) => return Err(u.unsupported("object.ImageDefinition")),
        ObjectType::UnderlayDefinition(_) => return Err(u.unsupported("object.UnderlayDefinition")),
        ObjectType::TableContent(_) => return Err(u.unsupported("object.TableContent")),
        ObjectType::TableTemplate(_) => return Err(u.unsupported("object.TableTemplate")),
        ObjectType::VisualStyle(_) => return Err(u.unsupported("object.VisualStyle")),
        ObjectType::Material(_) => return Err(u.unsupported("object.Material")),
        ObjectType::ImageDefinitionReactor(_) => {
            return Err(u.unsupported("object.ImageDefinitionReactor"))
        }
        ObjectType::GeoData(_) => return Err(u.unsupported("object.GeoData")),
        ObjectType::SpatialFilter(_) => return Err(u.unsupported("object.SpatialFilter")),
        ObjectType::RasterVariables(_) => return Err(u.unsupported("object.RasterVariables")),
        ObjectType::BookColor(_) => return Err(u.unsupported("object.BookColor")),
        ObjectType::WipeoutVariables(_) => return Err(u.unsupported("object.WipeoutVariables")),
        ObjectType::BlockVisibilityParameter(_) => {
            return Err(u.unsupported("object.BlockVisibilityParameter"))
        }
        ObjectType::DynamicBlock(_) => return Err(u.unsupported("object.DynamicBlock")),
        ObjectType::ClassObject(_) => return Err(u.unsupported("object.ClassObject")),
        ObjectType::DataObject(_) => return Err(u.unsupported("object.DataObject")),
        ObjectType::Field(_) => return Err(u.unsupported("object.Field")),
        ObjectType::FieldList(_) => return Err(u.unsupported("object.FieldList")),
        ObjectType::RegisteredClass(_) => return Err(u.unsupported("object.RegisteredClass")),
        ObjectType::DgnLineStyle(_) => return Err(u.unsupported("object.DgnLineStyle")),
        ObjectType::ProxyObject(_) => return Err(u.unsupported("object.ProxyObject")),
        ObjectType::Arch(_) => return Err(u.unsupported("object.Arch")),
        ObjectType::Unknown { .. } => return Err(u.unsupported("object.Unknown")),
    }
    Ok(())
}

// Nested scalar carriers are classified here with the rest of the census.
const _: fn(&MLineStyleElement) = |v| {
    let MLineStyleElement {
        offset: _,
        color: _,
        linetype: _,
    } = v;
};
const _: fn(&MLineStyleFlags) = |v| {
    let MLineStyleFlags {
        fill_on: _,
        display_joints: _,
        start_square_cap: _,
        start_inner_arcs_cap: _,
        start_round_cap: _,
        end_square_cap: _,
        end_inner_arcs_cap: _,
        end_round_cap: _,
    } = v;
};
const _: fn(&PaperMargin) = |v| {
    let PaperMargin {
        left: _,
        bottom: _,
        right: _,
        top: _,
    } = v;
};
const _: fn(&PlotFlags) = |v| {
    let PlotFlags {
        unknown_bits: _,
        plot_viewport_borders: _,
        show_plot_styles: _,
        plot_centered: _,
        plot_hidden: _,
        use_standard_scale: _,
        plot_plot_styles: _,
        scale_lineweights: _,
        print_lineweights: _,
        draw_viewports_first: _,
        model_type: _,
        update_paper: _,
        zoom_to_paper_on_update: _,
        initializing: _,
        prev_plot_init: _,
    } = v;
};
const _: fn(&PlotWindow) = |v| {
    let PlotWindow {
        lower_left_x: _,
        lower_left_y: _,
        upper_right_x: _,
        upper_right_y: _,
    } = v;
};
const _: fn(&TableCellBorder) = |v| {
    let TableCellBorder {
        property_flags: _,
        border_type: _,
        line_weight: _,
        color: _,
        is_invisible: _,
        double_line_spacing: _,
    } = v;
};

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)] // Independently mutate one carrier per case.
mod slot_tests {
    use super::*;
    const HIGH: u64 = (1u64 << 53) + 17;
    fn context_leaf() -> ObjectContextData {
        ObjectContextData {
            handle: Handle::NULL,
            owner_handle: Handle::NULL,
            reactors: vec![],
            xdictionary_handle: None,
            class_version: 4,
            has_file_to_extension_dictionary: true,
            uses_legacy_mleader_context_base: false,
            is_default: true,
            scale: Handle::NULL,
            kind: ObjectContextKind::AnnotScale,
        }
    }
    #[test]
    fn object_reference_slots_are_protected() {
        {
            let mut v = Dictionary::default();
            v.handle = Handle::new(700);
            v.owner = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Dictionary(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Dictionary.owner");
        }
        {
            let mut v = Dictionary::default();
            v.handle = Handle::new(700);
            v.reactors = vec![Handle::new(HIGH)];
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Dictionary(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Dictionary.reactors");
        }
        {
            let mut v = Dictionary::default();
            v.handle = Handle::new(700);
            v.xdictionary_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Dictionary(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Dictionary.xdictionary_handle");
        }
        {
            let mut v = Layout::new("sentinel");
            v.handle = Handle::new(700);
            v.owner = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Layout(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Layout.owner");
        }
        {
            let mut v = Layout::new("sentinel");
            v.handle = Handle::new(700);
            v.block_record = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Layout(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Layout.block_record");
        }
        {
            let mut v = Layout::new("sentinel");
            v.handle = Handle::new(700);
            v.viewport = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Layout(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Layout.viewport");
        }
        {
            let mut v = Layout::new("sentinel");
            v.handle = Handle::new(700);
            v.viewports = vec![Handle::new(HIGH)];
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Layout(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Layout.viewports");
        }
        {
            let mut v = Layout::new("sentinel");
            v.handle = Handle::new(700);
            v.base_ucs = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Layout(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Layout.base_ucs");
        }
        {
            let mut v = Layout::new("sentinel");
            v.handle = Handle::new(700);
            v.named_ucs = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Layout(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Layout.named_ucs");
        }
        {
            let mut v = Layout::new("sentinel");
            v.handle = Handle::new(700);
            v.reactors = vec![Handle::new(HIGH)];
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Layout(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Layout.reactors");
        }
        {
            let mut v = Layout::new("sentinel");
            v.handle = Handle::new(700);
            v.xdictionary_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Layout(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Layout.xdictionary_handle");
        }
        {
            let mut v = Layout::new("sentinel");
            v.handle = Handle::new(700);
            v.plot_view_handle = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Layout(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Layout.plot_view_handle");
        }
        {
            let mut v = Layout::new("sentinel");
            v.handle = Handle::new(700);
            v.visual_style_handle = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Layout(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Layout.visual_style_handle");
        }
        {
            let mut v = XRecord::default();
            v.handle = Handle::new(700);
            v.owner = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::XRecord(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "XRecord.owner");
        }
        {
            let mut v = XRecord::default();
            v.handle = Handle::new(700);
            v.reactors = vec![Handle::new(HIGH)];
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::XRecord(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "XRecord.reactors");
        }
        {
            let mut v = XRecord::default();
            v.handle = Handle::new(700);
            v.xdictionary_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::XRecord(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "XRecord.xdictionary_handle");
        }
        {
            let mut v = MLineStyle::default();
            v.handle = Handle::new(700);
            v.owner = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::MLineStyle(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "MLineStyle.owner");
        }
        {
            let mut v = PlotSettings::default();
            v.handle = Handle::new(700);
            v.owner = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::PlotSettings(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "PlotSettings.owner");
        }
        {
            let mut v = PlotSettings::default();
            v.handle = Handle::new(700);
            v.reactors = vec![Handle::new(HIGH)];
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::PlotSettings(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "PlotSettings.reactors");
        }
        {
            let mut v = PlotSettings::default();
            v.handle = Handle::new(700);
            v.xdictionary_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::PlotSettings(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "PlotSettings.xdictionary_handle");
        }
        {
            let mut v = PlotSettings::default();
            v.handle = Handle::new(700);
            v.plot_view_handle = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::PlotSettings(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "PlotSettings.plot_view_handle");
        }
        {
            let mut v = PlotSettings::default();
            v.handle = Handle::new(700);
            v.visual_style_handle = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::PlotSettings(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "PlotSettings.visual_style_handle");
        }
        {
            let mut v = MultiLeaderStyle::default();
            v.handle = Handle::new(700);
            v.owner_handle = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::MultiLeaderStyle(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "MultiLeaderStyle.owner_handle");
        }
        {
            let mut v = MultiLeaderStyle::default();
            v.handle = Handle::new(700);
            v.line_type_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::MultiLeaderStyle(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "MultiLeaderStyle.line_type_handle");
        }
        {
            let mut v = MultiLeaderStyle::default();
            v.handle = Handle::new(700);
            v.arrowhead_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::MultiLeaderStyle(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "MultiLeaderStyle.arrowhead_handle");
        }
        {
            let mut v = MultiLeaderStyle::default();
            v.handle = Handle::new(700);
            v.text_style_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::MultiLeaderStyle(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "MultiLeaderStyle.text_style_handle");
        }
        {
            let mut v = MultiLeaderStyle::default();
            v.handle = Handle::new(700);
            v.block_content_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::MultiLeaderStyle(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "MultiLeaderStyle.block_content_handle");
        }
        {
            let mut v = TableStyle::default();
            v.handle = Handle::new(700);
            v.owner_handle = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::TableStyle(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "TableStyle.owner_handle");
        }
        {
            let mut v = TableStyle::default();
            v.handle = Handle::new(700);
            v.modern_cell_style_handle = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::TableStyle(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "TableStyle.modern_cell_style_handle");
        }
        {
            let mut v = Scale::default();
            v.handle = Handle::new(700);
            v.owner_handle = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Scale(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Scale.owner_handle");
        }
        {
            let mut v = context_leaf();
            v.handle = Handle::new(700);
            v.owner_handle = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::ObjectContextData(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "ObjectContextData.owner_handle");
        }
        {
            let mut v = context_leaf();
            v.handle = Handle::new(700);
            v.reactors = vec![Handle::new(HIGH)];
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::ObjectContextData(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "ObjectContextData.reactors");
        }
        {
            let mut v = context_leaf();
            v.handle = Handle::new(700);
            v.xdictionary_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::ObjectContextData(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "ObjectContextData.xdictionary_handle");
        }
        {
            let mut v = context_leaf();
            v.handle = Handle::new(700);
            v.scale = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::ObjectContextData(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "ObjectContextData.scale");
        }
        {
            let mut v = SortEntitiesTable::default();
            v.handle = Handle::new(700);
            v.owner_handle = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::SortEntitiesTable(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "SortEntitiesTable.owner_handle");
        }
        {
            let mut v = SortEntitiesTable::default();
            v.handle = Handle::new(700);
            v.block_owner_handle = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::SortEntitiesTable(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "SortEntitiesTable.block_owner_handle");
        }
        {
            let mut v = DictionaryVariable::default();
            v.handle = Handle::new(700);
            v.owner_handle = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::DictionaryVariable(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "DictionaryVariable.owner_handle");
        }
        {
            let mut v = PlaceHolder::default();
            v.handle = Handle::new(700);
            v.owner = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::PlaceHolder(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "PlaceHolder.owner");
        }
        {
            let mut v = DictionaryWithDefault::default();
            v.handle = Handle::new(700);
            v.owner = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::DictionaryWithDefault(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "DictionaryWithDefault.owner");
        }
        {
            let mut v = DictionaryWithDefault::default();
            v.handle = Handle::new(700);
            v.default_handle = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::DictionaryWithDefault(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "DictionaryWithDefault.default_handle");
        }
        {
            let mut v = AssociativeObject::new("DIMASSOC", "AcDbDimAssoc");
            v.handle = Handle::new(700);
            v.owner = Handle::new(HIGH);
            v.data = AssociativeData::DimensionAssociation(AssocDimensionAssociation::default());
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Associative(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Associative.owner");
        }
        {
            let mut v = AssociativeObject::new("DIMASSOC", "AcDbDimAssoc");
            v.handle = Handle::new(700);
            v.reactors = vec![Handle::new(HIGH)];
            v.data = AssociativeData::DimensionAssociation(AssocDimensionAssociation::default());
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Associative(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Associative.reactors");
        }
        {
            let mut v = AssociativeObject::new("DIMASSOC", "AcDbDimAssoc");
            v.handle = Handle::new(700);
            v.xdictionary_handle = Some(Handle::new(HIGH));
            v.data = AssociativeData::DimensionAssociation(AssocDimensionAssociation::default());
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::Associative(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Associative.xdictionary_handle");
        }
    }
    #[test]
    fn xrecord_both_reference_representations_are_independent() {
        for vector in [false, true] {
            let mut v = XRecord::new();
            v.handle = Handle::new(700);
            if vector {
                v.preserve_object_reference_stream = false;
                v.object_references.push(ProxyObjectReference {
                    handle: Handle::new(HIGH),
                    kind: ProxyReferenceKind::SoftPointer,
                });
            } else {
                v.entries.push(XRecordEntry::new(
                    340,
                    XRecordValue::Handle(Handle::new(HIGH)),
                ));
            }
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::XRecord(v), &mut u).unwrap();
            assert_eq!(u.max, HIGH);
        }
    }
    #[test]
    fn xrecord_raw_duplicates_and_bad_codes_refuse() {
        for case in 0..5 {
            let mut v = XRecord::new();
            v.handle = Handle::new(700);
            match case {
                0 => v.entries_complete = false,
                1 => v.raw_data.push(0),
                2 => v.raw_dwg_data = Some(vec![]),
                3 => v.entries.push(XRecordEntry::string(340, "not a handle")),
                4 => v.entries.push(XRecordEntry::new(
                    90,
                    XRecordValue::Handle(Handle::new(HIGH)),
                )),
                _ => unreachable!(),
            }
            let mut u = HandleUsage::new();
            assert!(visit(v.handle, &ObjectType::XRecord(v), &mut u).is_err());
        }
    }
    #[test]
    fn sort_rank_is_scalar_but_entity_handle_is_protected() {
        let mut v = SortEntitiesTable::new();
        v.handle = Handle::new(700);
        v.add_entry(Handle::new(HIGH), Handle::new(u64::MAX));
        let mut u = HandleUsage::new();
        visit(v.handle, &ObjectType::SortEntitiesTable(v), &mut u).unwrap();
        assert_eq!(u.max, HIGH);
    }
    #[test]
    fn dimension_association_protects_all_inactive_chains() {
        for i in 0..4 {
            for intersect in [false, true] {
                let mut a = AssocDimensionAssociation::default();
                let mut r = AssocDimensionReference::default();
                if intersect {
                    r.intersection_objects.push(Handle::new(HIGH));
                } else {
                    r.xrefs.push(Handle::new(HIGH));
                }
                a.references[i].push(r);
                let mut v = AssociativeObject::new("DIMASSOC", "AcDbDimAssoc");
                v.handle = Handle::new(700);
                v.data = AssociativeData::DimensionAssociation(a);
                let mut u = HandleUsage::new();
                visit(v.handle, &ObjectType::Associative(v), &mut u).unwrap();
                assert_eq!(u.max, HIGH);
            }
        }
    }
    #[test]
    fn nested_table_style_handles_are_protected() {
        let mut v = TableStyle::default();
        v.handle = Handle::new(700);
        v.data_row_style.text_style_handle = Some(Handle::new(HIGH));
        let mut u = HandleUsage::new();
        visit(v.handle, &ObjectType::TableStyle(v), &mut u).unwrap();
        assert_eq!(u.max, HIGH);
    }
    #[test]
    fn context_dim_block_is_a_protected_reference() {
        let mut v = context_leaf();
        v.handle = Handle::new(700);
        let mut d = DimContext {
            def_pt: crate::types::Vector2::ZERO,
            is_def_textloc: false,
            text_rotation: 0.,
            block: Handle::NULL,
            b293: false,
            dimtofl: false,
            dimosxd: false,
            dimatfit: false,
            dimtix: false,
            dimtmove: false,
            override_code: 0,
            has_arrow2: false,
            flip_arrow2: false,
            flip_arrow1: false,
            subtype: DimSubtype::Aligned {
                dimline_pt: crate::types::Vector3::ZERO,
            },
        };
        d.block = Handle::new(HIGH);
        v.kind = ObjectContextKind::Dim(d);
        let mut u = HandleUsage::new();
        visit(v.handle, &ObjectType::ObjectContextData(v), &mut u).unwrap();
        assert_eq!(u.max, HIGH);
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)] // Independently mutate one carrier per case.
mod refusal_tests {
    use super::*;
    #[test]
    fn every_excluded_object_family_refuses() {
        let values = vec![
            ObjectType::Group(Group::default()),
            ObjectType::ImageDefinition(ImageDefinition::default()),
            ObjectType::UnderlayDefinition(UnderlayDefinition::default()),
            ObjectType::TableContent(crate::entities::Table::new(
                crate::types::Vector3::ZERO,
                1,
                1,
            )),
            ObjectType::TableTemplate(crate::entities::Table::new(
                crate::types::Vector3::ZERO,
                1,
                1,
            )),
            ObjectType::VisualStyle(VisualStyle::default()),
            ObjectType::Material(Material::default()),
            ObjectType::ImageDefinitionReactor(ImageDefinitionReactor::new(Handle::NULL)),
            ObjectType::GeoData(GeoData::default()),
            ObjectType::SpatialFilter(SpatialFilter::default()),
            ObjectType::RasterVariables(RasterVariables::default()),
            ObjectType::BookColor(BookColor::default()),
            ObjectType::WipeoutVariables(WipeoutVariables::default()),
            ObjectType::BlockVisibilityParameter(BlockVisibilityParameter::default()),
            ObjectType::DynamicBlock(DynamicBlockObject::new("PROBE", "Probe")),
            ObjectType::ClassObject(ClassObject::default()),
            ObjectType::DataObject(DataObject::default()),
            ObjectType::Field(Field::default()),
            ObjectType::FieldList(FieldList::default()),
            ObjectType::RegisteredClass(RegisteredClassObject::default()),
            ObjectType::DgnLineStyle(DgnLineStyleObject {
                handle: Handle::NULL,
                owner: Handle::NULL,
                reactors: vec![],
                xdictionary_handle: None,
                data: DgnLineStyleData::Registered {
                    dxf_name: "PROBE".into(),
                    properties: vec![],
                    payload: ProxyPayload::default(),
                    object_ids: vec![],
                },
            }),
            ObjectType::ProxyObject(ProxyObject::default()),
            ObjectType::Arch(crate::classes::arch::ArchObject::new(
                "PROBE",
                Handle::NULL,
                Handle::NULL,
            )),
        ];
        for o in values {
            let mut u = HandleUsage::new();
            assert!(
                matches!(
                    visit(Handle::new(700), &o, &mut u),
                    Err(HandleReservationError::UninspectableData { .. })
                ),
                "{o:?}"
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)] // Independently mutate one carrier per case.
mod more_nested_tests {
    use super::*;
    #[test]
    fn modern_style_references_in_default_and_overrides_are_protected() {
        for override_slot in [false, true] {
            for border in [false, true] {
                let mut v = TableStyle::default();
                v.handle = Handle::new(700);
                let mut c = NamedTableCellStyle::default();
                if border {
                    let mut b = TableGridFormat::default();
                    b.line_type = Handle::new(9000);
                    c.cell_style.borders.push(b);
                } else {
                    c.cell_style.content_format.text_style = Handle::new(9000);
                }
                if override_slot {
                    v.modern_overrides.push((1, c));
                } else {
                    v.modern_style = Some(c);
                }
                let mut u = HandleUsage::new();
                visit(v.handle, &ObjectType::TableStyle(v), &mut u).unwrap();
                assert_eq!(u.max, 9000);
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)] // Independently mutate one carrier per case.
mod context_refusal_tests {
    use super::*;
    #[test]
    fn all_excluded_context_kinds_refuse() {
        let z = crate::types::Vector3::ZERO;
        let kinds = vec![
            ObjectContextKind::Opaque,
            ObjectContextKind::MLeader(
                crate::entities::multileader::MultiLeaderAnnotContext::default(),
            ),
            ObjectContextKind::Leader(LeaderContext {
                points: vec![],
                x_direction: z,
                annotation_enabled: false,
                insertion_offset: z,
                endpoint_projection: z,
            }),
            ObjectContextKind::MTextAttribute(MTextAttributeContext {
                horizontal_mode: 0,
                rotation: 0.,
                insertion: crate::types::Vector2::ZERO,
                alignment: crate::types::Vector2::ZERO,
                enable_context: false,
                context: None,
            }),
            ObjectContextKind::Fcf {
                location: z,
                horizontal_direction: z,
            },
        ];
        for kind in kinds {
            let v = ObjectContextData {
                handle: Handle::new(700),
                owner_handle: Handle::NULL,
                reactors: vec![],
                xdictionary_handle: None,
                class_version: 4,
                has_file_to_extension_dictionary: true,
                uses_legacy_mleader_context_base: false,
                is_default: true,
                scale: Handle::NULL,
                kind,
            };
            assert!(matches!(
                visit(
                    v.handle,
                    &ObjectType::ObjectContextData(v),
                    &mut HandleUsage::new()
                ),
                Err(HandleReservationError::UninspectableData { .. })
            ));
        }
    }
    #[test]
    fn hatch_context_boundaries_and_view_links_are_protected() {
        for view in [false, true] {
            let high = Handle::new(9000);
            let mut b = crate::entities::BoundaryPath::new();
            if !view {
                b.boundary_handles.push(high);
            }
            let hatch = HatchScaleContext {
                pattern_lines: vec![],
                pattern_scale: 1.,
                pattern_base: crate::types::Vector3::ZERO,
                loops: vec![HatchLoopContext {
                    loop_type: 0,
                    supports_context: false,
                    boundary: Some(b),
                }],
            };
            let kind = if view {
                ObjectContextKind::HatchView(HatchViewContext {
                    hatch,
                    view: high,
                    view_normal: crate::types::Vector3::UNIT_Z,
                    view_rotation: 0.,
                    evaluate_hatch: false,
                })
            } else {
                ObjectContextKind::HatchScale(hatch)
            };
            let v = ObjectContextData {
                handle: Handle::new(700),
                owner_handle: Handle::NULL,
                reactors: vec![],
                xdictionary_handle: None,
                class_version: 4,
                has_file_to_extension_dictionary: true,
                uses_legacy_mleader_context_base: false,
                is_default: true,
                scale: Handle::NULL,
                kind,
            };
            let mut u = HandleUsage::new();
            visit(v.handle, &ObjectType::ObjectContextData(v), &mut u).unwrap();
            assert_eq!(u.max, 9000);
        }
    }
}

#[cfg(test)]
mod final_slot_tests {
    use super::*;
    #[test]
    fn dictionary_entry_and_association_dimension_slots_are_protected() {
        let mut a = Dictionary::new();
        a.handle = Handle::new(700);
        a.entries.push(("dangling".into(), Handle::new(9000)));
        let mut u = HandleUsage::new();
        visit(a.handle, &ObjectType::Dictionary(a), &mut u).unwrap();
        assert_eq!(u.max, 9000);
        let mut a = DictionaryWithDefault::new();
        a.handle = Handle::new(700);
        a.entries.push(("dangling".into(), Handle::new(9000)));
        let mut u = HandleUsage::new();
        visit(a.handle, &ObjectType::DictionaryWithDefault(a), &mut u).unwrap();
        assert_eq!(u.max, 9000);
        let mut a = AssociativeObject::new("DIMASSOC", "AcDbDimAssoc");
        a.handle = Handle::new(700);
        a.data = AssociativeData::DimensionAssociation(AssocDimensionAssociation {
            dimension: Handle::new(9000),
            ..Default::default()
        });
        let mut u = HandleUsage::new();
        visit(a.handle, &ObjectType::Associative(a), &mut u).unwrap();
        assert_eq!(u.max, 9000);
    }
    #[test]
    fn unknown_object_is_refused_even_with_empty_raw_data() {
        let a = ObjectType::Unknown {
            type_name: "PROBE".into(),
            handle: Handle::new(700),
            owner: Handle::NULL,
            raw_dxf_codes: None,
            raw_dwg_data: None,
            raw_dwg_handle_bits: 0,
            raw_dwg_version: None,
        };
        assert!(matches!(
            visit(Handle::new(700), &a, &mut HandleUsage::new()),
            Err(HandleReservationError::UninspectableData { .. })
        ));
    }
}
