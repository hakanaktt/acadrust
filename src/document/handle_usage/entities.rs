use super::*;
use crate::entities::*;
use crate::xdata::{ExtendedData, ExtendedDataRecord, XDataValue};
pub(super) fn xdata(x: &ExtendedData, u: &mut HandleUsage) -> Result<()> {
    x.reservation_fields();
    if !x.raw_dwg_eed.is_empty() {
        return Err(u.unsupported("extended_data.raw_dwg_eed"));
    }
    for r in x.records() {
        let ExtendedDataRecord {
            application_name: _,
            values: _,
        } = r;
        for v in &r.values {
            match v {
                XDataValue::Handle(h) => u.protect(*h),
                XDataValue::String(_)
                | XDataValue::ControlString(_)
                | XDataValue::LayerName(_)
                | XDataValue::BinaryData(_)
                | XDataValue::Point3D(_)
                | XDataValue::Position3D(_)
                | XDataValue::Displacement3D(_)
                | XDataValue::Direction3D(_)
                | XDataValue::Real(_)
                | XDataValue::Distance(_)
                | XDataValue::ScaleFactor(_)
                | XDataValue::Integer16(_)
                | XDataValue::Integer32(_) => {}
            }
        }
    }
    Ok(())
}
fn common(c: &EntityCommon, u: &mut HandleUsage) -> Result<()> {
    let EntityCommon {
        handle: _,
        layer: _,
        layer_handle: _,
        color: _,
        line_weight: _,
        linetype: _,
        linetype_handle: _,
        linetype_scale: _,
        transparency: _,
        color_name: _,
        invisible: _,
        extended_data: _,
        graphic_data: _,
        reactors: _,
        xdictionary_handle: _,
        owner_handle: _,
        color_book_handle: _,
        full_visual_style_handle: _,
        face_visual_style_handle: _,
        edge_visual_style_handle: _,
        material_flags: _,
        material_handle: _,
        shadow_flags: _,
        plotstyle_flags: _,
        plotstyle_handle: _,
        entity_mode: _,
        r12_metadata: _,
        has_ds_data: _,
        raw_record: _,
    } = c;

    if c.raw_record.is_some() {
        return Err(u.unsupported("common.raw_record"));
    }
    if c.graphic_data.is_some() {
        return Err(u.unsupported("common.graphic_data"));
    }
    if c.r12_metadata.is_some() {
        return Err(u.unsupported("common.r12_metadata"));
    }
    if c.has_ds_data {
        return Err(u.unsupported("common.has_ds_data"));
    }

    u.optional(c.layer_handle);
    u.optional(c.linetype_handle);
    u.refs(&c.reactors);
    u.optional(c.xdictionary_handle);
    u.protect(c.owner_handle);
    u.optional(c.color_book_handle);
    u.optional(c.full_visual_style_handle);
    u.optional(c.face_visual_style_handle);
    u.optional(c.edge_visual_style_handle);
    u.optional(c.material_handle);
    u.optional(c.plotstyle_handle);
    xdata(&c.extended_data, u)
}
fn attribute(v: &AttributeEntity, u: &mut HandleUsage, inline: bool) -> Result<()> {
    let AttributeEntity {
        common: _,
        tag: _,
        value: _,
        insertion_point: _,
        alignment_point: _,
        height: _,
        thickness: _,
        rotation: _,
        width_factor: _,
        oblique_angle: _,
        text_style: _,
        text_generation_flags: _,
        horizontal_alignment: _,
        vertical_alignment: _,
        flags: _,
        field_length: _,
        normal: _,
        mtext_flag: _,
        is_multiline: _,
        line_count: _,
        embedded_mtext: _,
        attdef_handle: _,
        lock_position: _,
    } = v;

    if inline && !v.common.handle.is_null() {
        u.primary(v.common.handle, "common.handle")?;
    }
    common(&v.common, u)?;
    if v.embedded_mtext.is_some() || v.is_multiline || v.mtext_flag != MTextFlag::SingleLine {
        return Err(u.unsupported("attribute.multiline"));
    }
    u.protect(v.attdef_handle);
    u.text(&v.value, "attribute.value")
}
fn boundary(p: &BoundaryPath, u: &mut HandleUsage) {
    let BoundaryPath {
        flags: _,
        edges: _,
        boundary_handles: _,
    } = p;
    u.refs(&p.boundary_handles);
    for e in &p.edges {
        match e {
            BoundaryEdge::Line(_)
            | BoundaryEdge::CircularArc(_)
            | BoundaryEdge::EllipticArc(_)
            | BoundaryEdge::Spline(_)
            | BoundaryEdge::Polyline(_) => {}
        }
    }
}
pub(super) fn hatch_context(h: &crate::objects::HatchScaleContext, u: &mut HandleUsage) {
    let crate::objects::HatchScaleContext {
        pattern_lines: _,
        pattern_scale: _,
        pattern_base: _,
        loops: _,
    } = h;
    for l in &h.loops {
        let crate::objects::HatchLoopContext {
            loop_type: _,
            supports_context: _,
            boundary: _,
        } = l;
        if let Some(p) = &l.boundary {
            boundary(p, u);
        }
    }
}
fn dimension(d: &Dimension, u: &mut HandleUsage) -> Result<()> {
    match d {
        Dimension::Aligned(v) => {
            let DimensionAligned {
                base: _,
                first_point: _,
                second_point: _,
                definition_point: _,
                ext_line_rotation: _,
            } = v;
        }
        Dimension::Linear(v) => {
            let DimensionLinear {
                base: _,
                first_point: _,
                second_point: _,
                definition_point: _,
                rotation: _,
                ext_line_rotation: _,
            } = v;
        }
        Dimension::Radius(v) => {
            let DimensionRadius {
                base: _,
                definition_point: _,
                angle_vertex: _,
                leader_length: _,
            } = v;
        }
        Dimension::Diameter(v) => {
            let DimensionDiameter {
                base: _,
                definition_point: _,
                angle_vertex: _,
                leader_length: _,
            } = v;
        }
        Dimension::Angular2Ln(v) => {
            let DimensionAngular2Ln {
                base: _,
                dimension_arc: _,
                first_point: _,
                second_point: _,
                angle_vertex: _,
                definition_point: _,
            } = v;
        }
        Dimension::Angular3Pt(v) => {
            let DimensionAngular3Pt {
                base: _,
                definition_point: _,
                first_point: _,
                second_point: _,
                angle_vertex: _,
            } = v;
        }
        Dimension::Ordinate(v) => {
            let DimensionOrdinate {
                base: _,
                definition_point: _,
                feature_location: _,
                leader_endpoint: _,
                is_ordinate_type_x: _,
            } = v;
        }
        Dimension::Arc(v) => {
            let DimensionArc {
                base: _,
                definition_point: _,
                first_extension_point: _,
                second_extension_point: _,
                center_point: _,
                is_partial: _,
                arc_start_parameter: _,
                arc_end_parameter: _,
                has_leader: _,
                first_leader_point: _,
                second_leader_point: _,
            } = v;
        }
        Dimension::LargeRadial(v) => {
            let DimensionLargeRadial {
                base: _,
                definition_point: _,
                chord_point: _,
                jog_angle: _,
                override_center: _,
                jog_point: _,
            } = v;
        }
    }
    let b = d.base();
    let DimensionBase {
        common: _,
        definition_point: _,
        text_middle_point: _,
        insertion_point: _,
        dimension_type: _,
        attachment_point: _,
        text: _,
        user_text: _,
        normal: _,
        text_rotation: _,
        horizontal_direction: _,
        style_name: _,
        actual_measurement: _,
        version: _,
        block_name: _,
        line_spacing_factor: _,
        line_spacing_style: _,
        insertion_scale: _,
        insertion_rotation: _,
        dwg_unknown_bit: _,
        flip_arrow1: _,
        flip_arrow2: _,
        dwg_flags_byte: _,
        text_user_positioned: _,
    } = b;
    u.text(&b.text, "dimension.text")?;
    if let Some(t) = &b.user_text {
        u.text(t, "dimension.user_text")?;
    }
    Ok(())
}
pub(super) fn visit(e: &EntityType, u: &mut HandleUsage) -> Result<()> {
    common(e.common(), u)?;
    match e {
        EntityType::Point(v) => {
            let Point {
                common: _,
                location: _,
                thickness: _,
                normal: _,
                x_axis_angle: _,
            } = v;
        }
        EntityType::Line(v) => {
            let Line {
                common: _,
                start: _,
                end: _,
                thickness: _,
                normal: _,
            } = v;
        }
        EntityType::Circle(v) => {
            let Circle {
                common: _,
                center: _,
                radius: _,
                thickness: _,
                normal: _,
            } = v;
        }
        EntityType::Arc(v) => {
            let Arc {
                common: _,
                center: _,
                radius: _,
                start_angle: _,
                end_angle: _,
                thickness: _,
                normal: _,
            } = v;
        }
        EntityType::Ellipse(v) => {
            let Ellipse {
                common: _,
                center: _,
                major_axis: _,
                minor_axis_ratio: _,
                start_parameter: _,
                end_parameter: _,
                normal: _,
            } = v;
        }
        EntityType::Polyline(v) => {
            let Polyline {
                common: _,
                flags: _,
                vertices: _,
            } = v;
        }
        EntityType::Polyline2D(v) => {
            let Polyline2D {
                common: _,
                flags: _,
                smooth_surface: _,
                start_width: _,
                end_width: _,
                thickness: _,
                elevation: _,
                normal: _,
                vertices: _,
            } = v;
        }
        EntityType::Polyline3D(v) => {
            let Polyline3D {
                common: _,
                flags: _,
                smooth_type: _,
                default_start_width: _,
                default_end_width: _,
                mesh_m_count: _,
                mesh_n_count: _,
                smooth_m_density: _,
                smooth_n_density: _,
                elevation: _,
                normal: _,
                vertices: _,
            } = v;
            for (index, p) in v.vertices.iter().enumerate() {
                let Vertex3DPolyline {
                    handle: _,
                    layer: _,
                    position: _,
                    flags: _,
                } = p;
                if !p.handle.is_null() {
                    u.primary(p.handle, &format!("polyline3d.vertices[{index}].handle"))?;
                }
            }
        }
        EntityType::LwPolyline(v) => {
            let LwPolyline {
                common: _,
                vertices: _,
                is_closed: _,
                plinegen: _,
                constant_width: _,
                elevation: _,
                thickness: _,
                normal: _,
            } = v;
        }
        EntityType::Text(v) => {
            let Text {
                common: _,
                value: _,
                insertion_point: _,
                alignment_point: _,
                height: _,
                rotation: _,
                width_factor: _,
                oblique_angle: _,
                style: _,
                horizontal_alignment: _,
                vertical_alignment: _,
                normal: _,
                thickness: _,
                generation_flags: _,
            } = v;
            u.text(&v.value, "Text.value")?;
        }
        EntityType::MText(v) => {
            let MText {
                common: _,
                value: _,
                insertion_point: _,
                height: _,
                rectangle_width: _,
                rectangle_height: _,
                rotation: _,
                dwg_x_direction: _,
                style: _,
                attachment_point: _,
                drawing_direction: _,
                line_spacing_factor: _,
                line_spacing_style: _,
                normal: _,
                background_fill_flags: _,
                background_scale: _,
                background_color: _,
                background_transparency: _,
                is_annotative: _,
                column_data: _,
                extents_width: _,
                extents_height: _,
            } = v.as_ref();
            u.text(&v.value, "MText.value")?;
        }
        EntityType::Dimension(v) => {
            dimension(v, u)?;
        }
        EntityType::Hatch(v) => {
            let Hatch {
                common: _,
                is_mpolygon: _,
                elevation: _,
                normal: _,
                pattern: _,
                is_solid: _,
                is_associative: _,
                pattern_type: _,
                pattern_angle: _,
                pattern_scale: _,
                is_double: _,
                style: _,
                paths: _,
                seed_points: _,
                pixel_size: _,
                gradient_color: _,
                mpolygon_hatch_color: _,
                mpolygon_x_direction: _,
                mpolygon_invalid_loops: _,
            } = v.as_ref();
            for p in v.paths.iter().chain(v.mpolygon_invalid_loops.iter()) {
                boundary(p, u);
            }
        }
        EntityType::Solid(v) => {
            let Solid {
                common: _,
                first_corner: _,
                second_corner: _,
                third_corner: _,
                fourth_corner: _,
                normal: _,
                thickness: _,
                is_trace: _,
            } = v;
        }
        EntityType::Insert(v) => {
            v.reservation_fields();
            u.optional(v.view_rep_handle);
            for (index, a) in v.attributes.iter().enumerate() {
                let parent = u.prefix.clone();
                u.prefix = format!("{parent}insert.attributes[{index}].");
                attribute(a, u, true)?;
                u.prefix = parent;
            }
            if let Some(h) = v.seqend_handle {
                if !h.is_null() {
                    u.primary(h, "insert.seqend_handle")?;
                }
            }
        }
        EntityType::Block(v) => {
            let Block {
                common: _,
                name: _,
                base_point: _,
                description: _,
                xref_path: _,
            } = v;
        }
        EntityType::BlockEnd(v) => {
            let BlockEnd { common: _ } = v;
        }
        EntityType::Viewport(v) => {
            let Viewport {
                common: _,
                center: _,
                width: _,
                height: _,
                status: _,
                id: _,
                dxf_status: _,
                off_screen: _,
                view_center: _,
                snap_base: _,
                snap_spacing: _,
                grid_spacing: _,
                view_direction: _,
                view_target: _,
                lens_length: _,
                front_clip_z: _,
                back_clip_z: _,
                view_height: _,
                snap_angle: _,
                twist_angle: _,
                circle_sides: _,
                frozen_layers: _,
                clip_boundary_handle: _,
                render_mode: _,
                style_sheet: _,
                ucs_at_origin: _,
                ucs_per_viewport: _,
                ucs_icon_visible: _,
                ucs_origin: _,
                ucs_x_axis: _,
                ucs_y_axis: _,
                ucs_handle: _,
                base_ucs_handle: _,
                ucs_ortho_type: _,
                elevation: _,
                shade_plot_mode: _,
                grid_flags: _,
                grid_major: _,
                background_handle: _,
                shade_plot_handle: _,
                visual_style_handle: _,
                sun_handle: _,
                default_lighting: _,
                default_lighting_type: _,
                brightness: _,
                contrast: _,
                ambient_color: _,
                custom_scale: _,
            } = v.as_ref();
            u.refs(&v.frozen_layers);
            u.protect(v.clip_boundary_handle);
            u.protect(v.ucs_handle);
            u.protect(v.base_ucs_handle);
            u.protect(v.background_handle);
            u.protect(v.shade_plot_handle);
            u.protect(v.visual_style_handle);
            u.protect(v.sun_handle);
        }
        EntityType::AttributeDefinition(v) => {
            let AttributeDefinition {
                common: _,
                tag: _,
                prompt: _,
                default_value: _,
                insertion_point: _,
                alignment_point: _,
                height: _,
                thickness: _,
                rotation: _,
                width_factor: _,
                oblique_angle: _,
                text_style: _,
                text_generation_flags: _,
                horizontal_alignment: _,
                vertical_alignment: _,
                flags: _,
                field_length: _,
                normal: _,
                mtext_flag: _,
                is_multiline: _,
                line_count: _,
                embedded_mtext: _,
                lock_position: _,
            } = v;
            if v.embedded_mtext.is_some() || v.is_multiline || v.mtext_flag != MTextFlag::SingleLine
            {
                return Err(u.unsupported("attdef.multiline"));
            }
            u.text(&v.default_value, "attdef.default_value")?;
        }
        EntityType::AttributeEntity(v) => {
            attribute(v, u, false)?;
        }
        EntityType::MLine(v) => {
            let MLine {
                common: _,
                flags: _,
                justification: _,
                normal: _,
                scale_factor: _,
                start_point: _,
                style_handle: _,
                style_name: _,
                style_element_count: _,
                vertices: _,
            } = v;
            u.optional(v.style_handle);
        }
        EntityType::Seqend(v) => {
            let Seqend { common: _ } = v;
        }
        EntityType::Spline(_) => return Err(u.unsupported("entity.Spline")),
        EntityType::Helix(_) => return Err(u.unsupported("entity.Helix")),
        EntityType::Face3D(_) => return Err(u.unsupported("entity.Face3D")),
        EntityType::Ray(_) => return Err(u.unsupported("entity.Ray")),
        EntityType::XLine(_) => return Err(u.unsupported("entity.XLine")),
        EntityType::Leader(_) => return Err(u.unsupported("entity.Leader")),
        EntityType::MultiLeader(_) => return Err(u.unsupported("entity.MultiLeader")),
        EntityType::Mesh(_) => return Err(u.unsupported("entity.Mesh")),
        EntityType::RasterImage(_) => return Err(u.unsupported("entity.RasterImage")),
        EntityType::Solid3D(_) => return Err(u.unsupported("entity.Solid3D")),
        EntityType::Region(_) => return Err(u.unsupported("entity.Region")),
        EntityType::Body(_) => return Err(u.unsupported("entity.Body")),
        EntityType::Surface(_) => return Err(u.unsupported("entity.Surface")),
        EntityType::Table(_) => return Err(u.unsupported("entity.Table")),
        EntityType::Tolerance(_) => return Err(u.unsupported("entity.Tolerance")),
        EntityType::PolyfaceMesh(_) => return Err(u.unsupported("entity.PolyfaceMesh")),
        EntityType::Wipeout(_) => return Err(u.unsupported("entity.Wipeout")),
        EntityType::Shape(_) => return Err(u.unsupported("entity.Shape")),
        EntityType::Underlay(_) => return Err(u.unsupported("entity.Underlay")),
        EntityType::Ole2Frame(_) => return Err(u.unsupported("entity.Ole2Frame")),
        EntityType::PolygonMesh(_) => return Err(u.unsupported("entity.PolygonMesh")),
        EntityType::Light(_) => return Err(u.unsupported("entity.Light")),
        EntityType::SectionSymbol(_) => return Err(u.unsupported("entity.SectionSymbol")),
        EntityType::ViewBorder(_) => return Err(u.unsupported("entity.ViewBorder")),
        EntityType::Extended(_) => return Err(u.unsupported("entity.Extended")),
        EntityType::Unknown(_) => return Err(u.unsupported("entity.Unknown")),
    }
    Ok(())
}

// Nested scalar carriers are classified here with the rest of the census.
const _: fn(&AttributeFlags) = |v| {
    let AttributeFlags {
        invisible: _,
        constant: _,
        verify: _,
        preset: _,
        locked_position: _,
        annotative: _,
    } = v;
};
const _: fn(&CircularArcEdge) = |v| {
    let CircularArcEdge {
        center: _,
        radius: _,
        start_angle: _,
        end_angle: _,
        counter_clockwise: _,
    } = v;
};
const _: fn(&EllipticArcEdge) = |v| {
    let EllipticArcEdge {
        center: _,
        major_axis_endpoint: _,
        minor_axis_ratio: _,
        start_angle: _,
        end_angle: _,
        counter_clockwise: _,
    } = v;
};
const _: fn(&GradientColorEntry) = |v| {
    let GradientColorEntry {
        value: _,
        color: _,
        color_name: _,
        book_name: _,
    } = v;
};
const _: fn(&HatchGradientPattern) = |v| {
    let HatchGradientPattern {
        enabled: _,
        reserved: _,
        angle: _,
        shift: _,
        is_single_color: _,
        color_tint: _,
        colors: _,
        name: _,
    } = v;
};
const _: fn(&HatchPattern) = |v| {
    let HatchPattern {
        name: _,
        description: _,
        lines: _,
    } = v;
};
const _: fn(&HatchPatternLine) = |v| {
    let HatchPatternLine {
        angle: _,
        base_point: _,
        offset: _,
        dash_lengths: _,
    } = v;
};
const _: fn(&LineEdge) = |v| {
    let LineEdge { start: _, end: _ } = v;
};
const _: fn(&PolylineEdge) = |v| {
    let PolylineEdge {
        vertices: _,
        is_closed: _,
    } = v;
};
const _: fn(&SplineEdge) = |v| {
    let SplineEdge {
        degree: _,
        rational: _,
        periodic: _,
        knots: _,
        control_points: _,
        fit_points: _,
        start_tangent: _,
        end_tangent: _,
    } = v;
};
const _: fn(&LwVertex) = |v| {
    let LwVertex {
        location: _,
        bulge: _,
        start_width: _,
        end_width: _,
        vertex_id: _,
    } = v;
};
const _: fn(&MLineSegment) = |v| {
    let MLineSegment {
        parameters: _,
        area_fill_parameters: _,
    } = v;
};
const _: fn(&MLineVertex) = |v| {
    let MLineVertex {
        position: _,
        direction: _,
        miter: _,
        segments: _,
    } = v;
};
const _: fn(&MTextColumnData) = |v| {
    let MTextColumnData {
        column_type: _,
        column_count: _,
        flow_reversed: _,
        auto_height: _,
        width: _,
        gutter: _,
        heights: _,
    } = v;
};
const _: fn(&Vertex2D) = |v| {
    let Vertex2D {
        location: _,
        flags: _,
        start_width: _,
        end_width: _,
        bulge: _,
        curve_tangent: _,
        id: _,
    } = v;
};
const _: fn(&Vertex3D) = |v| {
    let Vertex3D {
        location: _,
        flags: _,
    } = v;
};
const _: fn(&Polyline3DFlags) = |v| {
    let Polyline3DFlags {
        closed: _,
        spline_fit: _,
        is_3d: _,
        is_3d_mesh: _,
        mesh_closed_n: _,
        is_polyface_mesh: _,
        linetype_continuous: _,
    } = v;
};
const _: fn(&GridFlags) = |v| {
    let GridFlags {
        beyond_limits: _,
        adaptive: _,
        subdivision: _,
        follow_dynamic: _,
    } = v;
};
const _: fn(&ViewportStatusFlags) = |v| {
    let ViewportStatusFlags {
        non_rectangular_clipping: _,
        is_off: _,
        is_on: _,
        perspective: _,
        front_clipping: _,
        back_clipping: _,
        ucs_follow: _,
        front_clip_not_at_eye: _,
        ucs_icon_visible: _,
        ucs_icon_at_origin: _,
        fast_zoom: _,
        snap_on: _,
        grid_on: _,
        isometric_snap: _,
        hide_plot: _,
        iso_pair_top: _,
        iso_pair_right: _,
        locked: _,
    } = v;
};

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)] // Independently mutate one carrier per case.
mod slot_tests {
    use super::*;
    #[test]
    fn every_common_handle_slot_is_protected() {
        const HIGH: u64 = (1u64 << 53) + 17;
        {
            let mut c = EntityCommon::default();
            c.layer_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            common(&c, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "layer_handle");
        }
        {
            let mut c = EntityCommon::default();
            c.linetype_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            common(&c, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "linetype_handle");
        }
        {
            let mut c = EntityCommon::default();
            c.reactors = vec![Handle::new(HIGH)];
            let mut u = HandleUsage::new();
            common(&c, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "reactors");
        }
        {
            let mut c = EntityCommon::default();
            c.xdictionary_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            common(&c, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "xdictionary_handle");
        }
        {
            let mut c = EntityCommon::default();
            c.owner_handle = Handle::new(HIGH);
            let mut u = HandleUsage::new();
            common(&c, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "owner_handle");
        }
        {
            let mut c = EntityCommon::default();
            c.color_book_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            common(&c, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "color_book_handle");
        }
        {
            let mut c = EntityCommon::default();
            c.full_visual_style_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            common(&c, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "full_visual_style_handle");
        }
        {
            let mut c = EntityCommon::default();
            c.face_visual_style_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            common(&c, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "face_visual_style_handle");
        }
        {
            let mut c = EntityCommon::default();
            c.edge_visual_style_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            common(&c, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "edge_visual_style_handle");
        }
        {
            let mut c = EntityCommon::default();
            c.material_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            common(&c, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "material_handle");
        }
        {
            let mut c = EntityCommon::default();
            c.plotstyle_handle = Some(Handle::new(HIGH));
            let mut u = HandleUsage::new();
            common(&c, &mut u).unwrap();
            assert_eq!(u.max, HIGH, "plotstyle_handle");
        }
    }
    #[test]
    fn raw_eed_duplicate_does_not_certify_coverage() {
        let mut x = ExtendedData::new();
        let mut r = ExtendedDataRecord::new("APP");
        r.values.push(XDataValue::Handle(Handle::new(9000)));
        x.add_record(r);
        x.raw_dwg_eed.push((9000, vec![]));
        let mut u = HandleUsage::new();
        assert!(matches!(
            xdata(&x, &mut u),
            Err(HandleReservationError::UninspectableData { .. })
        ));
    }
    #[test]
    fn literal_xdata_is_not_a_handle_scan() {
        let mut x = ExtendedData::new();
        let mut r = ExtendedDataRecord::new("APP");
        r.values.push(XDataValue::String("FFFFFFFFFFFFFFFF".into()));
        r.values.push(XDataValue::BinaryData(vec![255; 8]));
        r.values.push(XDataValue::Integer32(i32::MAX));
        x.add_record(r);
        let mut u = HandleUsage::new();
        xdata(&x, &mut u).unwrap();
        assert_eq!(u.max, 0);
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)] // Independently mutate one carrier per case.
mod refusal_tests {
    use super::*;
    #[test]
    fn every_excluded_entity_family_refuses() {
        let values = vec![
            EntityType::Spline(Spline::default()),
            EntityType::Helix(Box::default()),
            EntityType::Face3D(Face3D::new(
                crate::types::Vector3::ZERO,
                crate::types::Vector3::ZERO,
                crate::types::Vector3::ZERO,
                crate::types::Vector3::ZERO,
            )),
            EntityType::Ray(Ray::default()),
            EntityType::XLine(XLine::default()),
            EntityType::Leader(Leader::default()),
            EntityType::MultiLeader(Box::default()),
            EntityType::Mesh(Mesh::default()),
            EntityType::RasterImage(RasterImage::default()),
            EntityType::Solid3D(Solid3D::default()),
            EntityType::Region(Region::default()),
            EntityType::Body(Body::default()),
            EntityType::Surface(Box::new(Surface::new(SurfaceKind::Generic))),
            EntityType::Table(Box::new(crate::entities::Table::new(
                crate::types::Vector3::ZERO,
                1,
                1,
            ))),
            EntityType::Tolerance(Tolerance::default()),
            EntityType::PolyfaceMesh(PolyfaceMesh::default()),
            EntityType::Wipeout(Wipeout::default()),
            EntityType::Shape(Shape::default()),
            EntityType::Underlay(Underlay::default()),
            EntityType::Ole2Frame(Box::default()),
            EntityType::PolygonMesh(crate::entities::polygon_mesh::PolygonMesh::default()),
            EntityType::Light(Box::default()),
            EntityType::SectionSymbol(SectionSymbol::default()),
            EntityType::ViewBorder(ViewBorder::default()),
            EntityType::Extended(Box::new(ExtendedEntity {
                common: EntityCommon::default(),
                data: ExtendedEntityData::Camera {
                    view_handle: Handle::NULL,
                },
            })),
            EntityType::Unknown(UnknownEntity::new("PROBE")),
        ];
        for e in values {
            let mut u = HandleUsage::new();
            assert!(
                matches!(
                    visit(&e, &mut u),
                    Err(HandleReservationError::UninspectableData { .. })
                ),
                "{e:?}"
            );
        }
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)] // Independently mutate one carrier per case.
mod positive_tests {
    use super::*;
    #[test]
    fn every_supported_entity_family_and_dimension_variant_is_inspectable() {
        let z = crate::types::Vector3::ZERO;
        let values = vec![
            EntityType::Point(Point::default()),
            EntityType::Line(Line::default()),
            EntityType::Circle(Circle::default()),
            EntityType::Arc(Arc::default()),
            EntityType::Ellipse(Ellipse::default()),
            EntityType::Polyline(Polyline::default()),
            EntityType::Polyline2D(Polyline2D::default()),
            EntityType::Polyline3D(Polyline3D::default()),
            EntityType::LwPolyline(LwPolyline::default()),
            EntityType::Text(Text::default()),
            EntityType::MText(Box::default()),
            EntityType::Hatch(Box::default()),
            EntityType::Solid(Solid::new(z, z, z, z)),
            EntityType::Insert(Insert::new("B", z)),
            EntityType::Block(Block::new("B", z)),
            EntityType::BlockEnd(BlockEnd::default()),
            EntityType::Viewport(Box::default()),
            EntityType::AttributeDefinition(AttributeDefinition::default()),
            EntityType::AttributeEntity(AttributeEntity::default()),
            EntityType::MLine(MLine::default()),
            EntityType::Seqend(Seqend::default()),
            EntityType::Dimension(Box::new(Dimension::Aligned(DimensionAligned::default()))),
            EntityType::Dimension(Box::new(Dimension::Linear(DimensionLinear::default()))),
            EntityType::Dimension(Box::new(Dimension::Radius(DimensionRadius::default()))),
            EntityType::Dimension(Box::new(Dimension::Diameter(DimensionDiameter::default()))),
            EntityType::Dimension(Box::new(Dimension::Angular2Ln(
                DimensionAngular2Ln::default(),
            ))),
            EntityType::Dimension(Box::new(Dimension::Angular3Pt(
                DimensionAngular3Pt::default(),
            ))),
            EntityType::Dimension(Box::new(Dimension::Ordinate(DimensionOrdinate::default()))),
            EntityType::Dimension(Box::new(Dimension::Arc(DimensionArc::default()))),
            EntityType::Dimension(Box::new(Dimension::LargeRadial(
                DimensionLargeRadial::default(),
            ))),
        ];
        for e in values {
            visit(&e, &mut HandleUsage::new()).unwrap();
        }
    }
    #[test]
    fn subtype_handle_slots_and_inline_identities_are_accounted() {
        let high = Handle::new((1u64 << 53) + 17);
        let mut v = Viewport::new();
        v.frozen_layers.push(high);
        let mut u = HandleUsage::new();
        visit(&EntityType::Viewport(Box::new(v)), &mut u).unwrap();
        assert_eq!(u.max, high.value());
        let mut v = MLine::new();
        v.style_handle = Some(high);
        let mut u = HandleUsage::new();
        visit(&EntityType::MLine(v), &mut u).unwrap();
        assert_eq!(u.max, high.value());
        let mut v = AttributeEntity::default();
        v.attdef_handle = high;
        let mut u = HandleUsage::new();
        visit(&EntityType::AttributeEntity(v), &mut u).unwrap();
        assert_eq!(u.max, high.value());
        let mut v = Insert::new("B", crate::types::Vector3::ZERO);
        v.seqend_handle = Some(high);
        let mut u = HandleUsage::new();
        visit(&EntityType::Insert(v), &mut u).unwrap();
        assert_eq!(u.max, high.value());
        let mut v = Polyline3D::from_points(vec![crate::types::Vector3::ZERO]);
        v.vertices[0].handle = high;
        let mut u = HandleUsage::new();
        visit(&EntityType::Polyline3D(v.clone()), &mut u).unwrap();
        assert_eq!(u.max, high.value());
        v.vertices.push(v.vertices[0].clone());
        assert!(matches!(
            visit(&EntityType::Polyline3D(v), &mut HandleUsage::new()),
            Err(HandleReservationError::InconsistentDocument { .. })
        ));
    }
    #[test]
    fn fields_and_multiline_attributes_are_conservative_refusals() {
        let mut t = Text::new();
        t.value = "%<literal>".into();
        assert!(visit(&EntityType::Text(t), &mut HandleUsage::new()).is_err());
        let mut a = AttributeEntity::default();
        a.is_multiline = true;
        assert!(visit(&EntityType::AttributeEntity(a), &mut HandleUsage::new()).is_err());
        let mut d = DimensionAligned::default();
        d.base.user_text = Some("%<field>".into());
        assert!(visit(
            &EntityType::Dimension(Box::new(Dimension::Aligned(d))),
            &mut HandleUsage::new()
        )
        .is_err());
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod remaining_slot_tests {
    use super::*;
    #[test]
    fn every_direct_subtype_handle_slot_is_protected() {
        {
            let mut v = Insert::new("B", crate::types::Vector3::ZERO);
            v.view_rep_handle = Some(Handle::new(9000));
            let mut u = HandleUsage::new();
            visit(&EntityType::Insert(v), &mut u).unwrap();
            assert_eq!(u.max, 9000, "Insert.view_rep_handle");
        }
        {
            let mut v = Insert::new("B", crate::types::Vector3::ZERO);
            v.seqend_handle = Some(Handle::new(9000));
            let mut u = HandleUsage::new();
            visit(&EntityType::Insert(v), &mut u).unwrap();
            assert_eq!(u.max, 9000, "Insert.seqend_handle");
        }
        {
            let mut v = Viewport::default();
            v.frozen_layers = vec![Handle::new(9000)];
            let mut u = HandleUsage::new();
            visit(&EntityType::Viewport(Box::new(v)), &mut u).unwrap();
            assert_eq!(u.max, 9000, "Viewport.frozen_layers");
        }
        {
            let mut v = Viewport::default();
            v.clip_boundary_handle = Handle::new(9000);
            let mut u = HandleUsage::new();
            visit(&EntityType::Viewport(Box::new(v)), &mut u).unwrap();
            assert_eq!(u.max, 9000, "Viewport.clip_boundary_handle");
        }
        {
            let mut v = Viewport::default();
            v.ucs_handle = Handle::new(9000);
            let mut u = HandleUsage::new();
            visit(&EntityType::Viewport(Box::new(v)), &mut u).unwrap();
            assert_eq!(u.max, 9000, "Viewport.ucs_handle");
        }
        {
            let mut v = Viewport::default();
            v.base_ucs_handle = Handle::new(9000);
            let mut u = HandleUsage::new();
            visit(&EntityType::Viewport(Box::new(v)), &mut u).unwrap();
            assert_eq!(u.max, 9000, "Viewport.base_ucs_handle");
        }
        {
            let mut v = Viewport::default();
            v.background_handle = Handle::new(9000);
            let mut u = HandleUsage::new();
            visit(&EntityType::Viewport(Box::new(v)), &mut u).unwrap();
            assert_eq!(u.max, 9000, "Viewport.background_handle");
        }
        {
            let mut v = Viewport::default();
            v.shade_plot_handle = Handle::new(9000);
            let mut u = HandleUsage::new();
            visit(&EntityType::Viewport(Box::new(v)), &mut u).unwrap();
            assert_eq!(u.max, 9000, "Viewport.shade_plot_handle");
        }
        {
            let mut v = Viewport::default();
            v.visual_style_handle = Handle::new(9000);
            let mut u = HandleUsage::new();
            visit(&EntityType::Viewport(Box::new(v)), &mut u).unwrap();
            assert_eq!(u.max, 9000, "Viewport.visual_style_handle");
        }
        {
            let mut v = Viewport::default();
            v.sun_handle = Handle::new(9000);
            let mut u = HandleUsage::new();
            visit(&EntityType::Viewport(Box::new(v)), &mut u).unwrap();
            assert_eq!(u.max, 9000, "Viewport.sun_handle");
        }
        {
            let mut v = AttributeEntity::default();
            v.attdef_handle = Handle::new(9000);
            let mut u = HandleUsage::new();
            visit(&EntityType::AttributeEntity(v), &mut u).unwrap();
            assert_eq!(u.max, 9000, "AttributeEntity.attdef_handle");
        }
        {
            let mut v = MLine::default();
            v.style_handle = Some(Handle::new(9000));
            let mut u = HandleUsage::new();
            visit(&EntityType::MLine(v), &mut u).unwrap();
            assert_eq!(u.max, 9000, "MLine.style_handle");
        }
    }
    #[test]
    fn both_hatch_boundary_stores_protect_references() {
        for invalid in [false, true] {
            let mut v = Hatch::new();
            let mut p = BoundaryPath::new();
            p.boundary_handles.push(Handle::new(9000));
            if invalid {
                v.mpolygon_invalid_loops.push(p);
            } else {
                v.paths.push(p);
            }
            let mut u = HandleUsage::new();
            visit(&EntityType::Hatch(Box::new(v)), &mut u).unwrap();
            assert_eq!(u.max, 9000);
        }
    }
}
