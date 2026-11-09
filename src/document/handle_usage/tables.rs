use super::*;
use crate::document::HeaderVariables;
use crate::tables::*;
pub(super) fn visit(d: &CadDocument, u: &mut HandleUsage) -> Result<()> {
    let h = &d.header;
    let HeaderVariables {
        legacy: _,
        required_versions: _,
        acad_maintenance_version: _,
        associate_dimensions: _,
        update_dimensions_while_dragging: _,
        ortho_mode: _,
        fill_mode: _,
        quick_text_mode: _,
        mirror_text: _,
        regen_mode: _,
        limit_check: _,
        paper_space_limit_check: _,
        polyline_linetype_generation: _,
        paper_space_linetype_scaling: _,
        show_model_space: _,
        user_timer: _,
        sketch_type: _,
        world_view: _,
        retain_xref_visibility: _,
        display_silhouette: _,
        create_ellipse_as_polyline: _,
        spline_frame: _,
        delete_objects: _,
        record_solid_history: _,
        show_solid_history: _,
        drag_mode: _,
        blip_mode: _,
        attribute_request: _,
        attribute_dialog: _,
        linear_unit_format: _,
        linear_unit_precision: _,
        angular_unit_format: _,
        angular_unit_precision: _,
        insertion_units: _,
        attribute_visibility: _,
        point_display_mode: _,
        user_int1: _,
        user_int2: _,
        user_int3: _,
        user_int4: _,
        user_int5: _,
        coords_mode: _,
        object_snap_mode: _,
        pick_style: _,
        spline_type: _,
        spline_segments: _,
        spline_segs_surface: _,
        surface_u_density: _,
        surface_v_density: _,
        surface_type: _,
        surface_tab1: _,
        surface_tab2: _,
        shade_edge: _,
        shade_diffuse: _,
        max_active_viewports: _,
        isolines: _,
        multiline_justification: _,
        text_quality: _,
        sort_entities: _,
        index_control: _,
        hide_text: _,
        xclip_frame: _,
        dwf_frame: _,
        dgn_frame: _,
        halo_gap: _,
        obscured_color: _,
        obscured_linetype: _,
        intersection_display: _,
        intersection_color: _,
        interference_color: _,
        real_world_scale: _,
        light_glyph_display: _,
        tile_model_light_synch: _,
        polysolid_width: _,
        polysolid_height: _,
        dwf_3d_precision: _,
        dim_fixed_ext_line_length: _,
        dim_fixed_ext_line_on: _,
        dim_arc_symbol: _,
        dim_jog_angle: _,
        dim_text_fill: _,
        dim_text_fill_color: _,
        dim_text_direction: _,
        dim_alt_mzf: _,
        dim_alt_mzs: _,
        dim_mzf: _,
        dim_mzs: _,
        cast_shadow: _,
        ucs_ortho_origins: _,
        paper_ucs_ortho_origins: _,
        dimension_associativity: _,
        project_name: _,
        linetype_scale: _,
        text_height: _,
        trace_width: _,
        sketch_increment: _,
        sketch_tolerance: _,
        thickness: _,
        point_display_size: _,
        polyline_width: _,
        current_entity_linetype_scale: _,
        view_twist: _,
        fillet_radius: _,
        chamfer_distance_a: _,
        chamfer_distance_b: _,
        chamfer_length: _,
        chamfer_angle: _,
        angle_base: _,
        angle_direction: _,
        elevation: _,
        paper_elevation: _,
        facet_resolution: _,
        multiline_scale: _,
        user_real1: _,
        user_real2: _,
        user_real3: _,
        user_real4: _,
        user_real5: _,
        viewport_scale_factor: _,
        current_annotation_scale: _,
        annotation_scale_value: _,
        shadow_plane_location: _,
        loft_angle1: _,
        loft_angle2: _,
        loft_magnitude1: _,
        loft_magnitude2: _,
        loft_param: _,
        loft_normals: _,
        latitude: _,
        longitude: _,
        north_direction: _,
        timezone: _,
        steps_per_second: _,
        step_size: _,
        lens_length: _,
        camera_height: _,
        camera_display: _,
        current_entity_color: _,
        current_line_weight: _,
        current_plotstyle_type: _,
        end_caps: _,
        join_style: _,
        lineweight_display: _,
        xedit: _,
        extended_names: _,
        plotstyle_mode: _,
        ole_startup: _,
        dim_scale: _,
        dim_arrow_size: _,
        dim_ext_line_offset: _,
        dim_line_increment: _,
        dim_ext_line_extension: _,
        dim_rounding: _,
        dim_line_extension: _,
        dim_tolerance_plus: _,
        dim_tolerance_minus: _,
        dim_text_height: _,
        dim_center_mark: _,
        dim_tick_size: _,
        dim_alt_scale: _,
        dim_linear_scale: _,
        dim_text_vertical_pos: _,
        dim_tolerance_scale: _,
        dim_line_gap: _,
        dim_alt_rounding: _,
        dim_tolerance: _,
        dim_limits: _,
        dim_text_inside_horizontal: _,
        dim_text_outside_horizontal: _,
        dim_suppress_ext1: _,
        dim_suppress_ext2: _,
        dim_text_above: _,
        dim_zero_suppression: _,
        dim_alt_zero_suppression: _,
        dim_alternate_units: _,
        dim_alt_decimal_places: _,
        dim_force_line_inside: _,
        dim_separate_arrows: _,
        dim_force_text_inside: _,
        dim_suppress_outside_ext: _,
        dim_line_color: _,
        dim_ext_line_color: _,
        dim_text_color: _,
        dim_angular_decimal_places: _,
        dim_decimal_places: _,
        dim_tolerance_decimal_places: _,
        dim_alt_units_format: _,
        dim_alt_tolerance_decimal_places: _,
        dim_angular_units: _,
        dim_fraction_format: _,
        dim_linear_unit_format: _,
        dim_decimal_separator: _,
        dim_text_movement: _,
        dim_horizontal_justification: _,
        dim_suppress_line1: _,
        dim_suppress_line2: _,
        dim_tolerance_justification: _,
        dim_tolerance_zero_suppression: _,
        dim_alt_tolerance_zero_suppression: _,
        dim_alt_tolerance_zero_tight: _,
        dim_fit: _,
        dim_fit_r13: _,
        dim_unit: _,
        unit_mode: _,
        dim_user_positioned_text: _,
        dim_post: _,
        dim_alt_post: _,
        dim_arrow_block: _,
        dim_arrow_block1: _,
        dim_arrow_block2: _,
        dim_leader_arrow_block: _,
        model_space_insertion_base: _,
        model_space_extents_min: _,
        model_space_extents_max: _,
        model_space_limits_min: _,
        model_space_limits_max: _,
        paper_space_insertion_base: _,
        paper_space_extents_min: _,
        paper_space_extents_max: _,
        paper_space_limits_min: _,
        paper_space_limits_max: _,
        ucs_base: _,
        model_space_ucs_name: _,
        paper_space_ucs_name: _,
        model_space_ucs_origin: _,
        model_space_ucs_x_axis: _,
        model_space_ucs_y_axis: _,
        paper_space_ucs_origin: _,
        paper_space_ucs_x_axis: _,
        paper_space_ucs_y_axis: _,
        ucs_ortho_ref: _,
        ucs_ortho_view: _,
        paper_ucs_ortho_ref: _,
        paper_ucs_ortho_view: _,
        paper_ucs_base: _,
        handle_seed: _,
        current_layer_handle: _,
        current_text_style_handle: _,
        current_linetype_handle: _,
        current_dimstyle_handle: _,
        current_multiline_style_handle: _,
        current_material_handle: _,
        dim_text_style_handle: _,
        dim_linetype_handle: _,
        dim_linetype1_handle: _,
        dim_linetype2_handle: _,
        dim_arrow_block_handle: _,
        dim_arrow_block1_handle: _,
        dim_arrow_block2_handle: _,
        dim_leader_arrow_block_handle: _,
        dim_text_style_name: _,
        dim_linetype_name: _,
        dim_linetype1_name: _,
        dim_linetype2_name: _,
        dim_line_weight: _,
        dim_ext_line_weight: _,
        block_control_handle: _,
        layer_control_handle: _,
        style_control_handle: _,
        linetype_control_handle: _,
        view_control_handle: _,
        ucs_control_handle: _,
        vport_control_handle: _,
        appid_control_handle: _,
        dimstyle_control_handle: _,
        vpent_hdr_control_handle: _,
        current_vx_handle: _,
        named_objects_dict_handle: _,
        acad_group_dict_handle: _,
        acad_mlinestyle_dict_handle: _,
        acad_layout_dict_handle: _,
        acad_plotsettings_dict_handle: _,
        acad_plotstylename_dict_handle: _,
        acad_material_dict_handle: _,
        acad_color_dict_handle: _,
        acad_visualstyle_dict_handle: _,
        model_space_block_handle: _,
        paper_space_block_handle: _,
        bylayer_linetype_handle: _,
        byblock_linetype_handle: _,
        continuous_linetype_handle: _,
        create_date_julian: _,
        update_date_julian: _,
        universal_create_date_julian: _,
        universal_update_date_julian: _,
        total_editing_time: _,
        user_elapsed_time: _,
        fingerprint_guid: _,
        version_guid: _,
        menu_name: _,
        code_page: _,
        last_saved_by: _,
        hyperlink_base: _,
        stylesheet: _,
        measurement: _,
        proxy_graphics: _,
        tree_depth: _,
        multiline_style: _,
        current_linetype_name: _,
        current_layer_name: _,
        current_text_style_name: _,
        current_dimstyle_name: _,
        current_table_style_name: _,
        current_mleader_style_name: _,
    } = h;
    u.protect(h.ucs_ortho_ref);
    u.protect(h.paper_ucs_ortho_ref);
    u.protect(h.current_layer_handle);
    u.protect(h.current_text_style_handle);
    u.protect(h.current_linetype_handle);
    u.protect(h.current_dimstyle_handle);
    u.protect(h.current_multiline_style_handle);
    u.protect(h.current_material_handle);
    u.protect(h.dim_text_style_handle);
    u.protect(h.dim_linetype_handle);
    u.protect(h.dim_linetype1_handle);
    u.protect(h.dim_linetype2_handle);
    u.protect(h.dim_arrow_block_handle);
    u.protect(h.dim_arrow_block1_handle);
    u.protect(h.dim_arrow_block2_handle);
    u.protect(h.dim_leader_arrow_block_handle);
    u.protect(h.block_control_handle);
    u.protect(h.layer_control_handle);
    u.protect(h.style_control_handle);
    u.protect(h.linetype_control_handle);
    u.protect(h.view_control_handle);
    u.protect(h.ucs_control_handle);
    u.protect(h.vport_control_handle);
    u.protect(h.appid_control_handle);
    u.protect(h.dimstyle_control_handle);
    u.protect(h.vpent_hdr_control_handle);
    u.protect(h.current_vx_handle);
    u.protect(h.named_objects_dict_handle);
    u.protect(h.acad_group_dict_handle);
    u.protect(h.acad_mlinestyle_dict_handle);
    u.protect(h.acad_layout_dict_handle);
    u.protect(h.acad_plotsettings_dict_handle);
    u.protect(h.acad_plotstylename_dict_handle);
    u.protect(h.acad_material_dict_handle);
    u.protect(h.acad_color_dict_handle);
    u.protect(h.acad_visualstyle_dict_handle);
    u.protect(h.model_space_block_handle);
    u.protect(h.paper_space_block_handle);
    u.protect(h.bylayer_linetype_handle);
    u.protect(h.byblock_linetype_handle);
    u.protect(h.continuous_linetype_handle);
    d.layers.reservation_fields();
    if d.layers.handle() != h.layer_control_handle || d.layers.has_stale_keys() {
        return Err(u.bad("layers.control_or_keys", Some(d.layers.handle())));
    }
    u.primary(d.layers.handle(), "layers.control")?;
    for v in d.layers.iter() {
        u.location = AuditLocation::TableEntry {
            table: "layers".into(),
            name: v.name().into(),
            handle: v.handle(),
        };
        u.primary(v.handle(), "layers.handle")?;
        let Layer {
            handle: _,
            name: _,
            flags: _,
            color: _,
            color_name: _,
            book_name: _,
            line_type: _,
            line_weight: _,
            plot_style: _,
            is_plottable: _,
            transparency: _,
            description: _,
            material: _,
            plotstyle_handle: _,
            xref_block_record_handle: _,
        } = v;
        u.protect(v.material);
        u.protect(v.plotstyle_handle);
        u.protect(v.xref_block_record_handle);
    }
    u.location = AuditLocation::Document;
    d.line_types.reservation_fields();
    if d.line_types.handle() != h.linetype_control_handle || d.line_types.has_stale_keys() {
        return Err(u.bad("line_types.control_or_keys", Some(d.line_types.handle())));
    }
    u.primary(d.line_types.handle(), "line_types.control")?;
    for v in d.line_types.iter() {
        u.location = AuditLocation::TableEntry {
            table: "line_types".into(),
            name: v.name().into(),
            handle: v.handle(),
        };
        u.primary(v.handle(), "line_types.handle")?;
        let LineType {
            handle: _,
            name: _,
            description: _,
            elements: _,
            pattern_length: _,
            alignment: _,
            xref_dependent: _,
            xref_block_record_handle: _,
        } = v;
        u.protect(v.xref_block_record_handle);
        for e in &v.elements {
            let LineTypeElement {
                length: _,
                complex: _,
            } = e;
            if let Some(c) = &e.complex {
                let LineTypeComplexData {
                    content: _,
                    style_handle: _,
                    scale: _,
                    rotation: _,
                    absolute_rotation: _,
                    upright: _,
                    source_flags: _,
                    offset: _,
                } = c;
                u.protect(c.style_handle);
                match &c.content {
                    LineTypeComplexContent::Shape { shape_number: _ }
                    | LineTypeComplexContent::Text { text: _ } => {}
                }
            }
        }
    }
    u.location = AuditLocation::Document;
    d.text_styles.reservation_fields();
    if d.text_styles.handle() != h.style_control_handle || d.text_styles.has_stale_keys() {
        return Err(u.bad("text_styles.control_or_keys", Some(d.text_styles.handle())));
    }
    u.primary(d.text_styles.handle(), "text_styles.control")?;
    for v in d.text_styles.iter() {
        u.location = AuditLocation::TableEntry {
            table: "text_styles".into(),
            name: v.name().into(),
            handle: v.handle(),
        };
        u.primary(v.handle(), "text_styles.handle")?;
        let TextStyle {
            handle: _,
            name: _,
            flags: _,
            height: _,
            width_factor: _,
            oblique_angle: _,
            last_height: _,
            font_file: _,
            big_font_file: _,
            true_type_font: _,
            true_type_font_flags: _,
            xref_dependent: _,
            xref_block_record_handle: _,
            is_shape_file: _,
            is_vertical: _,
            annotative: _,
        } = v;
        u.protect(v.xref_block_record_handle);
    }
    u.location = AuditLocation::Document;
    d.block_records.reservation_fields();
    if d.block_records.handle() != h.block_control_handle || d.block_records.has_stale_keys() {
        return Err(u.bad(
            "block_records.control_or_keys",
            Some(d.block_records.handle()),
        ));
    }
    u.primary(d.block_records.handle(), "block_records.control")?;
    for v in d.block_records.iter() {
        u.location = AuditLocation::TableEntry {
            table: "block_records".into(),
            name: v.name().into(),
            handle: v.handle(),
        };
        u.primary(v.handle(), "block_records.handle")?;
        let BlockRecord {
            handle: _,
            block_entity_handle: _,
            block_end_handle: _,
            name: _,
            flags: _,
            layout: _,
            units: _,
            explodable: _,
            scale_uniformly: _,
            entity_handles: _,
            xref_path: _,
            description: _,
            insert_count_bytes: _,
            preview_data: _,
            insert_handles: _,
            base_point: _,
        } = v;
        u.protect(v.layout);
        u.refs(&v.entity_handles);
        u.refs(&v.insert_handles);
        for (marker, end) in [(v.block_entity_handle, false), (v.block_end_handle, true)] {
            if !marker.is_null() {
                u.primary(marker, "block_record.marker")?;
                u.markers.insert(marker, (v.handle, end));
            }
        }
    }
    u.location = AuditLocation::Document;
    d.dim_styles.reservation_fields();
    if d.dim_styles.handle() != h.dimstyle_control_handle || d.dim_styles.has_stale_keys() {
        return Err(u.bad("dim_styles.control_or_keys", Some(d.dim_styles.handle())));
    }
    u.primary(d.dim_styles.handle(), "dim_styles.control")?;
    for v in d.dim_styles.iter() {
        u.location = AuditLocation::TableEntry {
            table: "dim_styles".into(),
            name: v.name().into(),
            handle: v.handle(),
        };
        u.primary(v.handle(), "dim_styles.handle")?;
        let DimStyle {
            handle: _,
            name: _,
            xref_reference: _,
            xref_resolved: _,
            xref_dependent: _,
            xref_handle: _,
            dimclrd: _,
            dimclrd_true_color: _,
            dimdle: _,
            dimdli: _,
            dimgap: _,
            dimlwd: _,
            dimsd1: _,
            dimsd2: _,
            dimclre: _,
            dimclre_true_color: _,
            dimexe: _,
            dimexo: _,
            dimlwe: _,
            dimse1: _,
            dimse2: _,
            dimfxl: _,
            dimfxlon: _,
            dimasz: _,
            dimblk: _,
            dimblk1: _,
            dimblk2: _,
            dimldrblk: _,
            dimblk_name: _,
            dimblk1_name: _,
            dimblk2_name: _,
            dimsah: _,
            dimcen: _,
            dimtsz: _,
            dimclrt: _,
            dimclrt_true_color: _,
            dimtxt: _,
            dimjust: _,
            dimtad: _,
            dimtvp: _,
            dimtih: _,
            dimtoh: _,
            dimtix: _,
            dimsoxd: _,
            dimtfill: _,
            dimtfillclr: _,
            dimtfillclr_true_color: _,
            dimtmove: _,
            dimtxtdirection: _,
            dimaltmzf: _,
            dimaltmzs: _,
            dimmzf: _,
            dimmzs: _,
            dimtxsty_handle: _,
            dimtxsty: _,
            dimscale: _,
            dimlfac: _,
            dimlunit: _,
            dimdec: _,
            dimrnd: _,
            dimdsep: _,
            dimzin: _,
            dimaunit: _,
            dimadec: _,
            dimazin: _,
            dimalt: _,
            dimaltf: _,
            dimaltd: _,
            dimaltu: _,
            dimalttd: _,
            dimaltrnd: _,
            dimapost: _,
            dimaltz: _,
            dimalttz: _,
            dimtol: _,
            dimlim: _,
            dimtp: _,
            dimtm: _,
            dimtdec: _,
            dimtfac: _,
            dimtolj: _,
            dimtzin: _,
            dimatfit: _,
            dimtofl: _,
            dimupt: _,
            dimfit: _,
            dimpost: _,
            dimfrac: _,
            dimarcsym: _,
            dimjogang: _,
            dimltex_handle: _,
            dimltex1_handle: _,
            dimltex2_handle: _,
            dimunit: _,
            annotative: _,
        } = v;
        u.protect(v.xref_handle);
        u.protect(v.dimblk);
        u.protect(v.dimblk1);
        u.protect(v.dimblk2);
        u.protect(v.dimldrblk);
        u.protect(v.dimtxsty_handle);
        u.protect(v.dimltex_handle);
        u.protect(v.dimltex1_handle);
        u.protect(v.dimltex2_handle);
    }
    u.location = AuditLocation::Document;
    d.app_ids.reservation_fields();
    if d.app_ids.handle() != h.appid_control_handle || d.app_ids.has_stale_keys() {
        return Err(u.bad("app_ids.control_or_keys", Some(d.app_ids.handle())));
    }
    u.primary(d.app_ids.handle(), "app_ids.control")?;
    for v in d.app_ids.iter() {
        u.location = AuditLocation::TableEntry {
            table: "app_ids".into(),
            name: v.name().into(),
            handle: v.handle(),
        };
        u.primary(v.handle(), "app_ids.handle")?;
        let AppId { handle: _, name: _ } = v;
    }
    u.location = AuditLocation::Document;
    d.views.reservation_fields();
    if d.views.handle() != h.view_control_handle || d.views.has_stale_keys() {
        return Err(u.bad("views.control_or_keys", Some(d.views.handle())));
    }
    u.primary(d.views.handle(), "views.control")?;
    for v in d.views.iter() {
        u.location = AuditLocation::TableEntry {
            table: "views".into(),
            name: v.name().into(),
            handle: v.handle(),
        };
        u.primary(v.handle(), "views.handle")?;
        let View {
            handle: _,
            name: _,
            center: _,
            height: _,
            width: _,
            direction: _,
            target: _,
            lens_length: _,
            front_clip: _,
            back_clip: _,
            twist_angle: _,
            perspective: _,
            front_clipping: _,
            back_clipping: _,
            front_clip_at_eye: _,
            render_mode: _,
            use_default_lights: _,
            default_lighting_type: _,
            brightness: _,
            contrast: _,
            ambient_color: _,
            paper_space: _,
            ucs_associated: _,
            ucs_origin: _,
            ucs_x_axis: _,
            ucs_y_axis: _,
            ucs_elevation: _,
            ucs_ortho_type: _,
            camera_plottable: _,
            xref_reference: _,
            xref_resolved: _,
            xref_dependent: _,
            xref_handle: _,
            background_handle: _,
            live_section_handle: _,
            visual_style_handle: _,
            sun_handle: _,
            named_ucs_handle: _,
            base_ucs_handle: _,
        } = v;
        u.protect(v.xref_handle);
        u.protect(v.background_handle);
        u.protect(v.live_section_handle);
        u.protect(v.visual_style_handle);
        u.protect(v.sun_handle);
        u.protect(v.named_ucs_handle);
        u.protect(v.base_ucs_handle);
    }
    u.location = AuditLocation::Document;
    d.vports.reservation_fields();
    if d.vports.handle() != h.vport_control_handle || d.vports.has_stale_keys() {
        return Err(u.bad("vports.control_or_keys", Some(d.vports.handle())));
    }
    u.primary(d.vports.handle(), "vports.control")?;
    for v in d.vports.iter() {
        u.location = AuditLocation::TableEntry {
            table: "vports".into(),
            name: v.name().into(),
            handle: v.handle(),
        };
        u.primary(v.handle(), "vports.handle")?;
        let VPort {
            handle: _,
            name: _,
            lower_left: _,
            upper_right: _,
            view_center: _,
            snap_base: _,
            snap_spacing: _,
            grid_spacing: _,
            view_direction: _,
            view_target: _,
            view_height: _,
            aspect_ratio: _,
            lens_length: _,
            view_twist: _,
            front_clip: _,
            back_clip: _,
            ucsfollow: _,
            circle_zoom: _,
            fast_zoom: _,
            grid_on: _,
            snap_on: _,
            snap_style: _,
            snap_isopair: _,
            snap_rotation: _,
            render_mode: _,
            perspective: _,
            front_clipping: _,
            back_clipping: _,
            front_clip_at_eye: _,
            ucsicon_lower: _,
            ucsicon_origin: _,
            use_default_lights: _,
            default_lighting_type: _,
            brightness: _,
            contrast: _,
            ambient_color: _,
            ucs_at_origin: _,
            ucs_per_viewport: _,
            ucs_origin: _,
            ucs_x_axis: _,
            ucs_y_axis: _,
            ucs_elevation: _,
            ucs_ortho_type: _,
            grid_flags: _,
            grid_major: _,
            xref_reference: _,
            xref_resolved: _,
            xref_dependent: _,
            xref_handle: _,
            background_handle: _,
            visual_style_handle: _,
            sun_handle: _,
            named_ucs_handle: _,
            base_ucs_handle: _,
        } = v;
        u.protect(v.xref_handle);
        u.protect(v.background_handle);
        u.protect(v.visual_style_handle);
        u.protect(v.sun_handle);
        u.protect(v.named_ucs_handle);
        u.protect(v.base_ucs_handle);
    }
    u.location = AuditLocation::Document;
    d.ucss.reservation_fields();
    if d.ucss.handle() != h.ucs_control_handle || d.ucss.has_stale_keys() {
        return Err(u.bad("ucss.control_or_keys", Some(d.ucss.handle())));
    }
    u.primary(d.ucss.handle(), "ucss.control")?;
    for v in d.ucss.iter() {
        u.location = AuditLocation::TableEntry {
            table: "ucss".into(),
            name: v.name().into(),
            handle: v.handle(),
        };
        u.primary(v.handle(), "ucss.handle")?;
        let Ucs {
            handle: _,
            name: _,
            origin: _,
            x_axis: _,
            y_axis: _,
            elevation: _,
            ortho_view_type: _,
            ortho_type: _,
            named_ucs_handle: _,
            base_ucs_handle: _,
            xref_reference: _,
            xref_resolved: _,
            xref_dependent: _,
            xref_handle: _,
        } = v;
        u.protect(v.named_ucs_handle);
        u.protect(v.base_ucs_handle);
        u.protect(v.xref_handle);
    }
    u.location = AuditLocation::Document;
    d.vx_table.reservation_fields();
    if d.vx_table.handle() != h.vpent_hdr_control_handle || d.vx_table.has_stale_keys() {
        return Err(u.bad("vx_table.control_or_keys", Some(d.vx_table.handle())));
    }
    u.primary(d.vx_table.handle(), "vx_table.control")?;
    for v in d.vx_table.iter() {
        u.location = AuditLocation::TableEntry {
            table: "vx_table".into(),
            name: v.name().into(),
            handle: v.handle(),
        };
        u.primary(v.handle(), "vx_table.handle")?;
        let VxTableRecord {
            handle: _,
            name: _,
            is_xref_reference: _,
            is_xref_resolved: _,
            is_xref_dependent: _,
            xref_handle: _,
            is_on: _,
            viewport: _,
            previous_entry: _,
            legacy_viewport_entity_address: _,
            legacy_viewport_index: _,
            legacy_previous_entry_index: _,
        } = v;
        u.protect(v.xref_handle);
        u.protect(v.viewport);
        u.protect(v.previous_entry);
    }
    u.location = AuditLocation::Document;
    Ok(())
}

// Nested scalar carriers are classified here with the rest of the census.
const _: fn(&super::super::LegacyHeaderState) = |v| {
    let super::super::LegacyHeaderState {
        view_center: _,
        view_height: _,
        view_aspect_ratio: _,
        view_direction: _,
        view_target: _,
        snap_mode: _,
        snap_spacing: _,
        snap_base: _,
        snap_angle: _,
        snap_style: _,
        snap_isopair: _,
        grid_mode: _,
        grid_spacing: _,
        axis_mode: _,
        axis_spacing: _,
        circle_zoom: _,
        fast_zoom: _,
        front_clip: _,
        back_clip: _,
        view_mode: _,
        flatland: _,
        ucs_icon: _,
        unit_mode: _,
        user_unit_ratios: _,
        user_unit_names: _,
    } = v;
};
const _: fn(&crate::tables::block_record::BlockFlags) = |v| {
    let crate::tables::block_record::BlockFlags {
        anonymous: _,
        has_attributes: _,
        is_xref: _,
        is_xref_overlay: _,
        is_external: _,
        is_xref_unloaded: _,
    } = v;
};
const _: fn(&LayerFlags) = |v| {
    let LayerFlags {
        frozen: _,
        locked: _,
        frozen_in_new_viewport: _,
        off: _,
        xref_dependent: _,
    } = v;
};
const _: fn(&TextGenerationFlags) = |v| {
    let TextGenerationFlags {
        backward: _,
        upside_down: _,
    } = v;
};

#[cfg(test)]
mod slot_tests {
    use super::*;
    const HIGH: u64 = (1u64 << 53) + 17;
    #[test]
    fn all_table_reference_fields_protect_dangling_targets() {
        {
            let mut d = CadDocument::new();
            let mut v = Layer::new("sentinel");
            v.handle = Handle::new(700);
            v.material = Handle::new(HIGH);
            d.layers.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Layer.material");
        }
        {
            let mut d = CadDocument::new();
            let mut v = Layer::new("sentinel");
            v.handle = Handle::new(700);
            v.plotstyle_handle = Handle::new(HIGH);
            d.layers.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Layer.plotstyle_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = Layer::new("sentinel");
            v.handle = Handle::new(700);
            v.xref_block_record_handle = Handle::new(HIGH);
            d.layers.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Layer.xref_block_record_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = LineType::new("sentinel");
            v.handle = Handle::new(700);
            v.xref_block_record_handle = Handle::new(HIGH);
            d.line_types.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "LineType.xref_block_record_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = TextStyle::new("sentinel");
            v.handle = Handle::new(700);
            v.xref_block_record_handle = Handle::new(HIGH);
            d.text_styles.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "TextStyle.xref_block_record_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = BlockRecord::new("sentinel");
            v.handle = Handle::new(700);
            v.layout = Handle::new(HIGH);
            d.block_records.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "BlockRecord.layout");
        }
        {
            let mut d = CadDocument::new();
            let mut v = DimStyle::new("sentinel");
            v.handle = Handle::new(700);
            v.xref_handle = Handle::new(HIGH);
            d.dim_styles.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "DimStyle.xref_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = DimStyle::new("sentinel");
            v.handle = Handle::new(700);
            v.dimblk = Handle::new(HIGH);
            d.dim_styles.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "DimStyle.dimblk");
        }
        {
            let mut d = CadDocument::new();
            let mut v = DimStyle::new("sentinel");
            v.handle = Handle::new(700);
            v.dimblk1 = Handle::new(HIGH);
            d.dim_styles.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "DimStyle.dimblk1");
        }
        {
            let mut d = CadDocument::new();
            let mut v = DimStyle::new("sentinel");
            v.handle = Handle::new(700);
            v.dimblk2 = Handle::new(HIGH);
            d.dim_styles.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "DimStyle.dimblk2");
        }
        {
            let mut d = CadDocument::new();
            let mut v = DimStyle::new("sentinel");
            v.handle = Handle::new(700);
            v.dimldrblk = Handle::new(HIGH);
            d.dim_styles.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "DimStyle.dimldrblk");
        }
        {
            let mut d = CadDocument::new();
            let mut v = DimStyle::new("sentinel");
            v.handle = Handle::new(700);
            v.dimtxsty_handle = Handle::new(HIGH);
            d.dim_styles.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "DimStyle.dimtxsty_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = DimStyle::new("sentinel");
            v.handle = Handle::new(700);
            v.dimltex_handle = Handle::new(HIGH);
            d.dim_styles.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "DimStyle.dimltex_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = DimStyle::new("sentinel");
            v.handle = Handle::new(700);
            v.dimltex1_handle = Handle::new(HIGH);
            d.dim_styles.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "DimStyle.dimltex1_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = DimStyle::new("sentinel");
            v.handle = Handle::new(700);
            v.dimltex2_handle = Handle::new(HIGH);
            d.dim_styles.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "DimStyle.dimltex2_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = View::new("sentinel");
            v.handle = Handle::new(700);
            v.xref_handle = Handle::new(HIGH);
            d.views.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "View.xref_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = View::new("sentinel");
            v.handle = Handle::new(700);
            v.background_handle = Handle::new(HIGH);
            d.views.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "View.background_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = View::new("sentinel");
            v.handle = Handle::new(700);
            v.live_section_handle = Handle::new(HIGH);
            d.views.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "View.live_section_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = View::new("sentinel");
            v.handle = Handle::new(700);
            v.visual_style_handle = Handle::new(HIGH);
            d.views.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "View.visual_style_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = View::new("sentinel");
            v.handle = Handle::new(700);
            v.sun_handle = Handle::new(HIGH);
            d.views.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "View.sun_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = View::new("sentinel");
            v.handle = Handle::new(700);
            v.named_ucs_handle = Handle::new(HIGH);
            d.views.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "View.named_ucs_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = View::new("sentinel");
            v.handle = Handle::new(700);
            v.base_ucs_handle = Handle::new(HIGH);
            d.views.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "View.base_ucs_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = VPort::new("sentinel");
            v.handle = Handle::new(700);
            v.xref_handle = Handle::new(HIGH);
            d.vports.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "VPort.xref_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = VPort::new("sentinel");
            v.handle = Handle::new(700);
            v.background_handle = Handle::new(HIGH);
            d.vports.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "VPort.background_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = VPort::new("sentinel");
            v.handle = Handle::new(700);
            v.visual_style_handle = Handle::new(HIGH);
            d.vports.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "VPort.visual_style_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = VPort::new("sentinel");
            v.handle = Handle::new(700);
            v.sun_handle = Handle::new(HIGH);
            d.vports.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "VPort.sun_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = VPort::new("sentinel");
            v.handle = Handle::new(700);
            v.named_ucs_handle = Handle::new(HIGH);
            d.vports.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "VPort.named_ucs_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = VPort::new("sentinel");
            v.handle = Handle::new(700);
            v.base_ucs_handle = Handle::new(HIGH);
            d.vports.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "VPort.base_ucs_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = Ucs::new("sentinel");
            v.handle = Handle::new(700);
            v.named_ucs_handle = Handle::new(HIGH);
            d.ucss.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Ucs.named_ucs_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = Ucs::new("sentinel");
            v.handle = Handle::new(700);
            v.base_ucs_handle = Handle::new(HIGH);
            d.ucss.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Ucs.base_ucs_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = Ucs::new("sentinel");
            v.handle = Handle::new(700);
            v.xref_handle = Handle::new(HIGH);
            d.ucss.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "Ucs.xref_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = VxTableRecord::new("sentinel");
            v.handle = Handle::new(700);
            v.xref_handle = Handle::new(HIGH);
            d.vx_table.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "VxTableRecord.xref_handle");
        }
        {
            let mut d = CadDocument::new();
            let mut v = VxTableRecord::new("sentinel");
            v.handle = Handle::new(700);
            v.viewport = Handle::new(HIGH);
            d.vx_table.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "VxTableRecord.viewport");
        }
        {
            let mut d = CadDocument::new();
            let mut v = VxTableRecord::new("sentinel");
            v.handle = Handle::new(700);
            v.previous_entry = Handle::new(HIGH);
            d.vx_table.add(v).unwrap();
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "VxTableRecord.previous_entry");
        }
    }
    #[test]
    fn stale_table_keys_and_control_aliases_are_errors() {
        let mut d = CadDocument::new();
        d.layers.get_mut("0").unwrap().name = "changed".into();
        assert!(visit(&d, &mut HandleUsage::new()).is_err());
        let mut d = CadDocument::new();
        d.header.layer_control_handle = Handle::new(HIGH);
        assert!(visit(&d, &mut HandleUsage::new()).is_err());
    }
}

#[cfg(test)]
mod remaining_slot_tests {
    use super::*;
    #[test]
    fn complex_linetype_and_block_member_slots_are_protected() {
        for inserts in [false, true] {
            let mut d = CadDocument::new();
            let b = d.block_records.get_mut("*Model_Space").unwrap();
            if inserts {
                b.insert_handles.push(Handle::new(9000));
            } else {
                b.entity_handles.push(Handle::new(9000));
            }
            let mut u = HandleUsage::new();
            visit(&d, &mut u).unwrap();
            assert_eq!(u.max, 9000);
        }
        let mut d = CadDocument::new();
        let mut v = LineType::new("complex");
        v.handle = Handle::new(700);
        v.elements.push(LineTypeElement {
            length: 1.,
            complex: Some(LineTypeComplexData {
                content: LineTypeComplexContent::Shape { shape_number: 0 },
                style_handle: Handle::new(9000),
                scale: 1.,
                rotation: 0.,
                absolute_rotation: false,
                upright: false,
                source_flags: 0,
                offset: [0.; 2],
            }),
        });
        d.line_types.add(v).unwrap();
        let mut u = HandleUsage::new();
        visit(&d, &mut u).unwrap();
        assert_eq!(u.max, 9000);
    }
}
