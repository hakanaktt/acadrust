//! Entity-specific writing methods for the DWG object writer.
//!
//! Mirrors ACadSharp's `DwgObjectWriter.Entities.cs`.
//!
//! Each method mirrors the corresponding `read_*` method in the object reader
//! to produce exactly the same binary layout.

use crate::entities::*;
use crate::entities::attribute_definition::AttributeFlags;
use crate::error::Result;
use crate::io::dwg::object_type::DwgObjectType;
use crate::io::dwg::reference_type::DwgReferenceType;
use crate::types::Vector3;
use crate::types::Handle;

use super::DwgObjectWriter;

impl DwgObjectWriter {
    /// Dispatch entity writing based on entity type.
    pub(super) fn write_entity(
        &mut self,
        entity: &EntityType,
        owner_handle: u64,
    ) -> Result<()> {
        match entity {
            EntityType::Line(e) => self.write_line(e, owner_handle),
            EntityType::Circle(e) => self.write_circle(e, owner_handle),
            EntityType::Arc(e) => self.write_arc(e, owner_handle),
            EntityType::Point(e) => self.write_point(e, owner_handle),
            EntityType::Ellipse(e) => self.write_ellipse(e, owner_handle),
            EntityType::Text(e) => self.write_text(e, owner_handle),
            EntityType::MText(e) => self.write_mtext(e, owner_handle),
            EntityType::AttributeEntity(e) => self.write_attribute(e, owner_handle),
            EntityType::AttributeDefinition(e) => self.write_attribute_definition(e, owner_handle),
            EntityType::Solid(e) => self.write_solid(e, owner_handle),
            EntityType::Face3D(e) => self.write_3d_face(e, owner_handle),
            EntityType::Ray(e) => self.write_ray(e, owner_handle),
            EntityType::XLine(e) => self.write_xline(e, owner_handle),
            EntityType::LwPolyline(e) => self.write_lwpolyline(e, owner_handle),
            EntityType::Spline(e) => self.write_spline(e, owner_handle),
            EntityType::Insert(e) => self.write_insert_composite(e, owner_handle),
            EntityType::Shape(e) => self.write_shape(e, owner_handle),
            EntityType::Tolerance(e) => self.write_tolerance(e, owner_handle),
            EntityType::Leader(e) => self.write_leader(e, owner_handle),
            EntityType::Dimension(e) => self.write_dimension(e, owner_handle),
            EntityType::Viewport(e) => self.write_viewport_entity(e, owner_handle),
            EntityType::Block(e) => self.write_block(e, owner_handle),
            EntityType::BlockEnd(e) => self.write_block_end(e, owner_handle),
            EntityType::Seqend(e) => self.write_seqend(e, owner_handle),
            // Composite polyline entities — write parent + children + SEQEND
            EntityType::Polyline2D(e) => self.write_polyline_2d_composite(e, owner_handle),
            EntityType::Polyline3D(e) => self.write_polyline_3d_composite(e, owner_handle),
            EntityType::PolyfaceMesh(e) => self.write_polyface_mesh_composite(e, owner_handle),
            EntityType::PolygonMesh(e) => self.write_polygon_mesh_composite(e, owner_handle),
            // Entities not yet supported for writing — skip silently
            _ => Ok(()),
        }
    }

    // -----------------------------------------------------------------------
    // Composite polyline writers — write parent + child vertices + SEQEND
    // -----------------------------------------------------------------------

    /// Assign unique handles to child objects if they don't have one yet.
    /// Returns the next available handle value.
    fn next_available_handle(&self) -> u64 {
        self.handle_map
            .keys()
            .last()
            .map(|&h| h + 1)
            .unwrap_or(0x100)
    }

    /// Write a complete Polyline2D: parent polyline + Vertex2D children + SEQEND.
    fn write_polyline_2d_composite(
        &mut self,
        polyline: &Polyline2D,
        owner_handle: u64,
    ) -> Result<()> {
        let polyline_handle = polyline.common.handle.value();

        // Allocate handles for vertices and SEQEND
        let mut next_h = self.next_available_handle();
        let mut vertex_handles = Vec::with_capacity(polyline.vertices.len());
        let mut vertex_commons = Vec::with_capacity(polyline.vertices.len());

        for _v in &polyline.vertices {
            let h = next_h;
            next_h += 1;
            vertex_handles.push(h);
            let mut vc = EntityCommon::new();
            vc.handle = Handle::new(h);
            vc.layer = polyline.common.layer.clone();
            vc.color = polyline.common.color;
            vertex_commons.push(vc);
        }

        let seqend_h = next_h;
        let mut seqend_common = EntityCommon::new();
        seqend_common.handle = Handle::new(seqend_h);
        seqend_common.layer = polyline.common.layer.clone();

        // 1. Write the parent polyline with references to children
        self.write_polyline_2d(polyline, owner_handle, &vertex_handles, seqend_h)?;

        // 2. Write each vertex as a separate entity
        for (i, vertex) in polyline.vertices.iter().enumerate() {
            self.write_vertex_2d(vertex, &vertex_commons[i], polyline_handle)?;
        }

        // 3. Write SEQEND
        let seqend = Seqend { common: seqend_common };
        self.write_seqend(&seqend, polyline_handle)?;

        Ok(())
    }

    /// Write a complete Polyline3D: parent polyline + Vertex3D children + SEQEND.
    fn write_polyline_3d_composite(
        &mut self,
        polyline: &Polyline3D,
        owner_handle: u64,
    ) -> Result<()> {
        let polyline_handle = polyline.common.handle.value();

        let mut next_h = self.next_available_handle();
        let mut vertex_handles = Vec::with_capacity(polyline.vertices.len());
        let mut vertex_commons = Vec::with_capacity(polyline.vertices.len());

        for v in &polyline.vertices {
            let h = if v.handle.value() != 0 { v.handle.value() } else { let h = next_h; next_h += 1; h };
            vertex_handles.push(h);
            let mut vc = EntityCommon::new();
            vc.handle = Handle::new(h);
            vc.layer = polyline.common.layer.clone();
            vc.color = polyline.common.color;
            vertex_commons.push(vc);
        }

        let seqend_h = next_h;
        let mut seqend_common = EntityCommon::new();
        seqend_common.handle = Handle::new(seqend_h);
        seqend_common.layer = polyline.common.layer.clone();

        self.write_polyline_3d(polyline, owner_handle, &vertex_handles, seqend_h)?;

        for (i, vertex) in polyline.vertices.iter().enumerate() {
            self.write_vertex_3d_polyline(vertex, &vertex_commons[i], polyline_handle)?;
        }

        let seqend = Seqend { common: seqend_common };
        self.write_seqend(&seqend, polyline_handle)?;

        Ok(())
    }

    /// Write a complete PolyfaceMesh: parent + PolyfaceVertex children + PolyfaceFace children + SEQEND.
    fn write_polyface_mesh_composite(
        &mut self,
        mesh: &PolyfaceMesh,
        owner_handle: u64,
    ) -> Result<()> {
        let mesh_handle = mesh.common.handle.value();

        let mut next_h = self.next_available_handle();
        let total_children = mesh.vertices.len() + mesh.faces.len();
        let mut child_handles = Vec::with_capacity(total_children);

        // Allocate handles for vertices
        let mut vertex_commons = Vec::with_capacity(mesh.vertices.len());
        for v in &mesh.vertices {
            let h = if v.common.handle.value() != 0 { v.common.handle.value() } else { let h = next_h; next_h += 1; h };
            child_handles.push(h);
            let mut vc = v.common.clone();
            vc.handle = Handle::new(h);
            if vc.layer.is_empty() || vc.layer == "0" {
                vc.layer = mesh.common.layer.clone();
            }
            vertex_commons.push(vc);
        }

        // Allocate handles for faces
        let mut face_commons = Vec::with_capacity(mesh.faces.len());
        for f in &mesh.faces {
            let h = if f.common.handle.value() != 0 { f.common.handle.value() } else { let h = next_h; next_h += 1; h };
            child_handles.push(h);
            let mut fc = f.common.clone();
            fc.handle = Handle::new(h);
            if fc.layer.is_empty() || fc.layer == "0" {
                fc.layer = mesh.common.layer.clone();
            }
            face_commons.push(fc);
        }

        let seqend_h = next_h;
        let mut seqend_common = EntityCommon::new();
        seqend_common.handle = Handle::new(seqend_h);
        seqend_common.layer = mesh.common.layer.clone();

        // 1. Write parent polyface mesh
        self.write_polyface_mesh(mesh, owner_handle, &child_handles, seqend_h)?;

        // 2. Write vertex position entities (VertexPface = 0x0D)
        for (i, vertex) in mesh.vertices.iter().enumerate() {
            self.write_pface_vertex(vertex, &vertex_commons[i], mesh_handle)?;
        }

        // 3. Write face record entities (VertexPfaceFace = 0x0E)
        for (i, face) in mesh.faces.iter().enumerate() {
            let mut face_copy = face.clone();
            face_copy.common = face_commons[i].clone();
            self.write_pface_face(&face_copy, mesh_handle)?;
        }

        // 4. Write SEQEND
        let seqend = Seqend { common: seqend_common };
        self.write_seqend(&seqend, mesh_handle)?;

        Ok(())
    }

    /// Write a complete PolygonMesh: parent + PolygonMeshVertex children + SEQEND.
    fn write_polygon_mesh_composite(
        &mut self,
        mesh: &crate::entities::PolygonMeshEntity,
        owner_handle: u64,
    ) -> Result<()> {
        let mesh_handle = mesh.common.handle.value();

        let mut next_h = self.next_available_handle();
        let mut vertex_handles = Vec::with_capacity(mesh.vertices.len());
        let mut vertex_commons = Vec::with_capacity(mesh.vertices.len());

        for v in &mesh.vertices {
            let h = if v.common.handle.value() != 0 { v.common.handle.value() } else { let h = next_h; next_h += 1; h };
            vertex_handles.push(h);
            let mut vc = v.common.clone();
            vc.handle = Handle::new(h);
            if vc.layer.is_empty() || vc.layer == "0" {
                vc.layer = mesh.common.layer.clone();
            }
            vertex_commons.push(vc);
        }

        let seqend_h = next_h;
        let mut seqend_common = EntityCommon::new();
        seqend_common.handle = Handle::new(seqend_h);
        seqend_common.layer = mesh.common.layer.clone();

        // 1. Write parent polygon mesh
        self.write_polygon_mesh(mesh, owner_handle, &vertex_handles, seqend_h)?;

        // 2. Write vertex entities (VertexMesh = 0x0C)
        for (i, vertex) in mesh.vertices.iter().enumerate() {
            self.write_vertex_mesh(vertex, &vertex_commons[i], mesh_handle)?;
        }

        // 3. Write SEQEND
        let seqend = Seqend { common: seqend_common };
        self.write_seqend(&seqend, mesh_handle)?;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // LINE
    // -----------------------------------------------------------------------

    fn write_line(&mut self, line: &Line, owner_handle: u64) -> Result<()> {
        let (mut writer, _version) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Line,
            &line.common,
            owner_handle,
        )?;

        if self.sio.r13_14_only {
            writer.write_3bit_double(line.start)?;
            writer.write_3bit_double(line.end)?;
        } else {
            // R2000+: optimized encoding
            let z_are_zero = line.start.z == 0.0 && line.end.z == 0.0;
            writer.write_bit(z_are_zero)?;
            writer.write_raw_double(line.start.x)?;
            writer.write_bit_double_with_default(line.start.x, line.end.x)?;
            writer.write_raw_double(line.start.y)?;
            writer.write_bit_double_with_default(line.start.y, line.end.y)?;
            if !z_are_zero {
                writer.write_raw_double(line.start.z)?;
                writer.write_bit_double_with_default(line.start.z, line.end.z)?;
            }
        }

        writer.write_bit_thickness(line.thickness)?;
        writer.write_bit_extrusion(line.normal)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, line.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // CIRCLE
    // -----------------------------------------------------------------------

    fn write_circle(&mut self, circle: &Circle, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Circle,
            &circle.common,
            owner_handle,
        )?;

        writer.write_3bit_double(circle.center)?;
        writer.write_bit_double(circle.radius)?;
        writer.write_bit_thickness(circle.thickness)?;
        writer.write_bit_extrusion(circle.normal)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, circle.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // ARC
    // -----------------------------------------------------------------------

    fn write_arc(&mut self, arc: &Arc, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Arc,
            &arc.common,
            owner_handle,
        )?;

        writer.write_3bit_double(arc.center)?;
        writer.write_bit_double(arc.radius)?;
        writer.write_bit_thickness(arc.thickness)?;
        writer.write_bit_extrusion(arc.normal)?;
        writer.write_bit_double(arc.start_angle)?;
        writer.write_bit_double(arc.end_angle)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, arc.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // POINT
    // -----------------------------------------------------------------------

    fn write_point(&mut self, point: &Point, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Point,
            &point.common,
            owner_handle,
        )?;

        writer.write_3bit_double(point.location)?;
        writer.write_bit_thickness(point.thickness)?;
        writer.write_bit_extrusion(point.normal)?;
        writer.write_bit_double(0.0)?; // x_axis_angle

        writer.write_spear_shift()?;
        self.finalize_entity(writer, point.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // ELLIPSE
    // -----------------------------------------------------------------------

    fn write_ellipse(&mut self, ellipse: &Ellipse, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Ellipse,
            &ellipse.common,
            owner_handle,
        )?;

        writer.write_3bit_double(ellipse.center)?;
        writer.write_3bit_double(ellipse.major_axis)?;
        writer.write_3bit_double(ellipse.normal)?;
        writer.write_bit_double(ellipse.minor_axis_ratio)?;
        writer.write_bit_double(ellipse.start_parameter)?;
        writer.write_bit_double(ellipse.end_parameter)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, ellipse.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // TEXT
    // -----------------------------------------------------------------------

    fn write_text(&mut self, text: &Text, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Text,
            &text.common,
            owner_handle,
        )?;

        let align_pt = text.alignment_point.unwrap_or(text.insertion_point);
        self.write_text_data(
            &mut *writer,
            text.insertion_point,
            align_pt,
            text.alignment_point.is_some(),
            text.normal,
            text.oblique_angle,
            text.rotation,
            text.height,
            text.width_factor,
            &text.value,
            0, // generation_flags
            text.horizontal_alignment as i16,
            text.vertical_alignment as i16,
        )?;

        // Style handle (hard pointer)
        writer.handle_reference_typed(
            DwgReferenceType::HardPointer,
            self.resolve_textstyle_handle(&text.style),
        )?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, text.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Shared text-entity data writer
    // -----------------------------------------------------------------------

    /// Write text-entity body data shared by TEXT, ATTRIB and ATTDEF.
    ///
    /// The binary layout mirrors `read_common_text_data` in the object reader.
    /// For R2000+ the DataFlags byte uses the convention:
    ///   bit SET   → field has its DEFAULT value → do NOT write the field
    ///   bit CLEAR → field is NON-default      → write the field
    fn write_text_data(
        &self,
        writer: &mut dyn crate::io::dwg::writer::stream_writer::IDwgStreamWriter,
        insertion_point: Vector3,
        alignment_point: Vector3,
        has_alignment_point: bool,
        normal: Vector3,
        oblique_angle: f64,
        rotation: f64,
        height: f64,
        width_factor: f64,
        value: &str,
        generation_flags: i16,
        horizontal_alignment: i16,
        vertical_alignment: i16,
    ) -> Result<()> {
        if self.sio.r13_14_only {
            writer.write_bit_double(insertion_point.z)?; // elevation
            writer.write_2raw_double(crate::types::Vector2::new(
                insertion_point.x,
                insertion_point.y,
            ))?;
            writer.write_2raw_double(crate::types::Vector2::new(
                alignment_point.x,
                alignment_point.y,
            ))?;
            writer.write_bit_extrusion(normal)?;
            writer.write_bit_thickness(0.0)?;
            writer.write_bit_double(oblique_angle)?;
            writer.write_bit_double(rotation)?;
            writer.write_bit_double(height)?;
            writer.write_bit_double(width_factor)?;
            writer.write_variable_text(value)?;
            writer.write_bit_short(generation_flags)?;
            writer.write_bit_short(horizontal_alignment)?;
            writer.write_bit_short(vertical_alignment)?;
        } else {
            // R2000+: DataFlags byte — bit SET = default value (skip)
            let elevation = insertion_point.z;
            let mut data_flags: u8 = 0;
            if elevation == 0.0 {
                data_flags |= 0x01;
            }
            if !has_alignment_point
                || (alignment_point.x == insertion_point.x
                    && alignment_point.y == insertion_point.y)
            {
                data_flags |= 0x02;
            }
            if oblique_angle == 0.0 {
                data_flags |= 0x04;
            }
            if rotation == 0.0 {
                data_flags |= 0x08;
            }
            if width_factor == 1.0 {
                data_flags |= 0x10;
            }
            if generation_flags == 0 {
                data_flags |= 0x20;
            }
            if horizontal_alignment == 0 {
                data_flags |= 0x40;
            }
            if vertical_alignment == 0 {
                data_flags |= 0x80;
            }

            writer.write_byte(data_flags)?;

            if data_flags & 0x01 == 0 {
                writer.write_raw_double(elevation)?;
            }
            writer.write_2raw_double(crate::types::Vector2::new(
                insertion_point.x,
                insertion_point.y,
            ))?;
            if data_flags & 0x02 == 0 {
                writer.write_2bit_double_with_default(
                    crate::types::Vector2::new(insertion_point.x, insertion_point.y),
                    crate::types::Vector2::new(alignment_point.x, alignment_point.y),
                )?;
            }
            writer.write_bit_extrusion(normal)?;
            writer.write_bit_thickness(0.0)?;

            if data_flags & 0x04 == 0 {
                writer.write_raw_double(oblique_angle)?;
            }
            if data_flags & 0x08 == 0 {
                writer.write_raw_double(rotation)?;
            }
            writer.write_raw_double(height)?;
            if data_flags & 0x10 == 0 {
                writer.write_raw_double(width_factor)?;
            }
            writer.write_variable_text(value)?;
            if data_flags & 0x20 == 0 {
                writer.write_bit_short(generation_flags)?;
            }
            if data_flags & 0x40 == 0 {
                writer.write_bit_short(horizontal_alignment)?;
            }
            if data_flags & 0x80 == 0 {
                writer.write_bit_short(vertical_alignment)?;
            }
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // ATTRIB
    // -----------------------------------------------------------------------

    /// Write an ATTRIB entity (block attribute instance).
    ///
    /// Binary layout mirrors `read_common_att_data`:
    ///   common_entity_data → text_data → Tag(TV) → FieldLength(BS) →
    ///   Flags(RC) → LockPosition(B, R2007+) → hasMText(B, R2010+) →
    ///   StyleHandle(H)
    fn write_attribute(
        &mut self,
        attrib: &AttributeEntity,
        owner_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Attrib,
            &attrib.common,
            owner_handle,
        )?;

        // Text base data
        self.write_text_data(
            &mut *writer,
            attrib.insertion_point,
            attrib.alignment_point,
            true, // attributes always store alignment point
            attrib.normal,
            attrib.oblique_angle,
            attrib.rotation,
            attrib.height,
            attrib.width_factor,
            &attrib.value,
            attrib.text_generation_flags,
            attrib.horizontal_alignment as i16,
            attrib.vertical_alignment as i16,
        )?;

        // Tag (TV)
        writer.write_variable_text(&attrib.tag)?;
        // Field length (BS) — always 0 (unused)
        writer.write_bit_short(0)?;
        // Flags (RC) — NOT bit-pair coded
        let flags_byte = Self::attribute_flags_to_byte(&attrib.flags);
        writer.write_byte(flags_byte)?;

        // R2007+: Lock position flag (B)
        if self.sio.r2007_plus {
            writer.write_bit(attrib.lock_position)?;
        }

        // R2010+: has MText flag (B)
        if self.sio.r2010_plus {
            writer.write_bit(false)?; // single-line attributes
        }

        // Style handle (hard pointer)
        writer.handle_reference_typed(
            DwgReferenceType::HardPointer,
            self.resolve_textstyle_handle(&attrib.text_style),
        )?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, attrib.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // ATTDEF
    // -----------------------------------------------------------------------

    /// Write an ATTDEF entity (block attribute definition).
    ///
    /// Binary layout mirrors `read_common_attdef_data`:
    ///   common_entity_data → text_data → Tag(TV) → FieldLength(BS) →
    ///   Flags(RC) → Prompt(TV) → LockPosition(B, R2007+) →
    ///   hasMText(B, R2010+) → StyleHandle(H)
    fn write_attribute_definition(
        &mut self,
        attdef: &AttributeDefinition,
        owner_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Attdef,
            &attdef.common,
            owner_handle,
        )?;

        // Text base data (default_value is the text value)
        self.write_text_data(
            &mut *writer,
            attdef.insertion_point,
            attdef.alignment_point,
            true, // attribute defs always store alignment point
            attdef.normal,
            attdef.oblique_angle,
            attdef.rotation,
            attdef.height,
            attdef.width_factor,
            &attdef.default_value,
            attdef.text_generation_flags,
            attdef.horizontal_alignment as i16,
            attdef.vertical_alignment as i16,
        )?;

        // Tag (TV)
        writer.write_variable_text(&attdef.tag)?;
        // Field length (BS) — always 0 (unused)
        writer.write_bit_short(0)?;
        // Flags (RC) — NOT bit-pair coded
        let flags_byte = Self::attribute_flags_to_byte(&attdef.flags);
        writer.write_byte(flags_byte)?;

        // Prompt (TV) — only in ATTDEF, not in ATTRIB
        writer.write_variable_text(&attdef.prompt)?;

        // R2007+: Lock position flag (B)
        if self.sio.r2007_plus {
            writer.write_bit(attdef.lock_position)?;
        }

        // R2010+: has MText flag (B)
        if self.sio.r2010_plus {
            writer.write_bit(false)?;
        }

        // Style handle (hard pointer)
        writer.handle_reference_typed(
            DwgReferenceType::HardPointer,
            self.resolve_textstyle_handle(&attdef.text_style),
        )?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, attdef.common.handle.value());
        Ok(())
    }

    /// Convert `AttributeFlags` to the raw byte encoding used in the DWG format.
    fn attribute_flags_to_byte(flags: &AttributeFlags) -> u8 {
        let mut v: u8 = 0;
        if flags.invisible { v |= 1; }
        if flags.constant  { v |= 2; }
        if flags.verify    { v |= 4; }
        if flags.preset    { v |= 8; }
        v
    }

    // -----------------------------------------------------------------------
    // MTEXT
    // -----------------------------------------------------------------------

    fn write_mtext(&mut self, mtext: &MText, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Mtext,
            &mtext.common,
            owner_handle,
        )?;

        writer.write_3bit_double(mtext.insertion_point)?;
        writer.write_3bit_double(mtext.normal)?;

        // Direction — compute from rotation and normal
        let direction = if mtext.rotation == 0.0 {
            Vector3::UNIT_X
        } else {
            Vector3::new(mtext.rotation.cos(), mtext.rotation.sin(), 0.0)
        };
        writer.write_3bit_double(direction)?;

        writer.write_bit_double(mtext.rectangle_width)?;
        writer.write_bit_double(mtext.height)?;
        writer.write_bit_short(mtext.attachment_point as i16)?;
        writer.write_bit_short(mtext.drawing_direction as i16)?;

        // Text value
        writer.write_variable_text(&mtext.value)?;
        // Extra text string (empty)
        writer.write_variable_text("")?;

        writer.write_bit_double(mtext.line_spacing_factor)?;
        writer.write_bit_short(0)?; // line spacing style (0 = at least)

        // Style handle
        writer.handle_reference_typed(
            DwgReferenceType::HardPointer,
            self.resolve_textstyle_handle(&mtext.style),
        )?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, mtext.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // SOLID
    // -----------------------------------------------------------------------

    fn write_solid(&mut self, solid: &Solid, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Solid,
            &solid.common,
            owner_handle,
        )?;

        writer.write_bit_thickness(solid.thickness)?;
        writer.write_bit_double(solid.first_corner.z)?;
        writer.write_2raw_double(crate::types::Vector2::new(
            solid.first_corner.x,
            solid.first_corner.y,
        ))?;
        writer.write_2raw_double(crate::types::Vector2::new(
            solid.second_corner.x,
            solid.second_corner.y,
        ))?;
        writer.write_2raw_double(crate::types::Vector2::new(
            solid.third_corner.x,
            solid.third_corner.y,
        ))?;
        writer.write_2raw_double(crate::types::Vector2::new(
            solid.fourth_corner.x,
            solid.fourth_corner.y,
        ))?;
        writer.write_bit_extrusion(solid.normal)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, solid.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // 3DFACE
    // -----------------------------------------------------------------------

    fn write_3d_face(&mut self, face: &Face3D, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Face3D,
            &face.common,
            owner_handle,
        )?;

        if self.sio.r2000_plus {
            let has_no_flags = face.invisible_edges.bits() == 0;
            let z_is_zero = face.first_corner.z == 0.0;
            writer.write_bit(has_no_flags)?;
            writer.write_bit(z_is_zero)?;

            writer.write_raw_double(face.first_corner.x)?;
            writer.write_raw_double(face.first_corner.y)?;
            if !z_is_zero {
                writer.write_raw_double(face.first_corner.z)?;
            }

            writer.write_bit_double_with_default(
                face.first_corner.x,
                face.second_corner.x,
            )?;
            writer.write_bit_double_with_default(
                face.first_corner.y,
                face.second_corner.y,
            )?;
            writer.write_bit_double_with_default(
                face.first_corner.z,
                face.second_corner.z,
            )?;

            writer.write_bit_double_with_default(
                face.second_corner.x,
                face.third_corner.x,
            )?;
            writer.write_bit_double_with_default(
                face.second_corner.y,
                face.third_corner.y,
            )?;
            writer.write_bit_double_with_default(
                face.second_corner.z,
                face.third_corner.z,
            )?;

            writer.write_bit_double_with_default(
                face.third_corner.x,
                face.fourth_corner.x,
            )?;
            writer.write_bit_double_with_default(
                face.third_corner.y,
                face.fourth_corner.y,
            )?;
            writer.write_bit_double_with_default(
                face.third_corner.z,
                face.fourth_corner.z,
            )?;

            if !has_no_flags {
                writer.write_bit_short(face.invisible_edges.bits() as i16)?;
            }
        } else {
            writer.write_3bit_double(face.first_corner)?;
            writer.write_3bit_double(face.second_corner)?;
            writer.write_3bit_double(face.third_corner)?;
            writer.write_3bit_double(face.fourth_corner)?;
            writer.write_bit_short(face.invisible_edges.bits() as i16)?;
        }

        writer.write_spear_shift()?;
        self.finalize_entity(writer, face.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // RAY
    // -----------------------------------------------------------------------

    fn write_ray(&mut self, ray: &Ray, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Ray,
            &ray.common,
            owner_handle,
        )?;

        writer.write_3bit_double(ray.base_point)?;
        writer.write_3bit_double(ray.direction)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, ray.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // XLINE
    // -----------------------------------------------------------------------

    fn write_xline(&mut self, xline: &XLine, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Xline,
            &xline.common,
            owner_handle,
        )?;

        writer.write_3bit_double(xline.base_point)?;
        writer.write_3bit_double(xline.direction)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, xline.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // LWPOLYLINE
    // -----------------------------------------------------------------------

    fn write_lwpolyline(&mut self, lwpoly: &LwPolyline, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::LwPolyline,
            &lwpoly.common,
            owner_handle,
        )?;

        let num_pts = lwpoly.vertices.len();

        // Flags (BS)
        let mut flags: i16 = 0;
        if lwpoly.is_closed {
            flags |= 0x200;
        }
        writer.write_bit_short(flags)?;

        let has_constant_width = lwpoly.constant_width != 0.0;
        let has_elevation = lwpoly.elevation != 0.0;
        let has_thickness = lwpoly.thickness != 0.0;
        let has_normal = lwpoly.normal != Vector3::UNIT_Z;
        let has_bulges = lwpoly.vertices.iter().any(|v| v.bulge != 0.0);
        let has_widths = lwpoly
            .vertices
            .iter()
            .any(|v| v.start_width != 0.0 || v.end_width != 0.0);

        if has_constant_width {
            writer.write_bit_double(lwpoly.constant_width)?;
        }
        if has_elevation {
            writer.write_bit_double(lwpoly.elevation)?;
        }
        if has_thickness {
            writer.write_bit_thickness(lwpoly.thickness)?;
        }
        if has_normal {
            writer.write_bit_extrusion(lwpoly.normal)?;
        }

        // Number of points (BL)
        writer.write_bit_long(num_pts as i32)?;

        // Number of bulges (BL)
        let bulge_count = if has_bulges { num_pts } else { 0 };
        writer.write_bit_long(bulge_count as i32)?;

        // Number of width pairs (BL)
        let width_count = if has_widths { num_pts } else { 0 };
        writer.write_bit_long(width_count as i32)?;

        // Points (2RD / 2DD)
        for (i, v) in lwpoly.vertices.iter().enumerate() {
            if i == 0 {
                writer.write_2raw_double(crate::types::Vector2::new(
                    v.location.x,
                    v.location.y,
                ))?;
            } else {
                let prev = &lwpoly.vertices[i - 1];
                writer.write_2bit_double_with_default(
                    crate::types::Vector2::new(prev.location.x, prev.location.y),
                    crate::types::Vector2::new(v.location.x, v.location.y),
                )?;
            }
        }

        // Bulges (BD each)
        if has_bulges {
            for v in &lwpoly.vertices {
                writer.write_bit_double(v.bulge)?;
            }
        }

        // Width pairs (2BD each)
        if has_widths {
            for v in &lwpoly.vertices {
                writer.write_bit_double(v.start_width)?;
                writer.write_bit_double(v.end_width)?;
            }
        }

        writer.write_spear_shift()?;
        self.finalize_entity(writer, lwpoly.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // SPLINE
    // -----------------------------------------------------------------------

    fn write_spline(&mut self, spline: &Spline, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Spline,
            &spline.common,
            owner_handle,
        )?;

        let scenario = if !spline.fit_points.is_empty() { 2 } else { 1 };

        // R2013+: flags as BL
        if self.sio.r2013_plus {
            let mut flag_bits: i32 = 0;
            if spline.flags.closed {
                flag_bits |= 1;
            }
            if spline.flags.periodic {
                flag_bits |= 2;
            }
            if spline.flags.rational {
                flag_bits |= 4;
            }
            if spline.flags.planar {
                flag_bits |= 8;
            }
            if spline.flags.linear {
                flag_bits |= 16;
            }
            writer.write_bit_long(flag_bits)?;
            writer.write_bit_long(0)?; // knot parametrization
        }

        writer.write_bit_long(spline.degree)?;

        if scenario == 2 {
            // Fit data
            writer.write_bit_double(0.0)?; // fit tolerance
            writer.write_3bit_double(Vector3::ZERO)?; // start tangent
            writer.write_3bit_double(Vector3::ZERO)?; // end tangent
            writer.write_bit_long(spline.fit_points.len() as i32)?;
            for pt in &spline.fit_points {
                writer.write_3bit_double(*pt)?;
            }
        } else {
            // Control point data
            writer.write_bit(spline.flags.rational)?;
            writer.write_bit(spline.flags.closed)?;
            writer.write_bit(spline.flags.periodic)?;

            writer.write_bit_double(1e-10)?; // knot tolerance
            writer.write_bit_double(1e-10)?; // control point tolerance

            writer.write_bit_long(spline.knots.len() as i32)?;
            writer.write_bit_long(spline.control_points.len() as i32)?;
            let has_weights = !spline.weights.is_empty()
                && spline.weights.len() == spline.control_points.len();
            writer.write_bit(has_weights)?;

            for k in &spline.knots {
                writer.write_bit_double(*k)?;
            }

            for (i, cp) in spline.control_points.iter().enumerate() {
                writer.write_3bit_double(*cp)?;
                if has_weights {
                    writer.write_bit_double(spline.weights[i])?;
                }
            }
        }

        writer.write_spear_shift()?;
        self.finalize_entity(writer, spline.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // INSERT (with attribute support)
    // -----------------------------------------------------------------------

    /// Write a simple INSERT entity with no attributes.
    fn write_insert_simple(&mut self, insert: &Insert, owner_handle: u64) -> Result<()> {
        self.write_insert_inner(insert, owner_handle, false, &[], 0)
    }

    /// Core INSERT writer used by both the simple and composite paths.
    ///
    /// When `has_attribs` is true, after the block-header handle it writes
    /// the attribute-handle chain and SEQEND handle.
    fn write_insert_inner(
        &mut self,
        insert: &Insert,
        owner_handle: u64,
        has_attribs: bool,
        attrib_handles: &[u64],
        seqend_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Insert,
            &insert.common,
            owner_handle,
        )?;

        writer.write_3bit_double(insert.insert_point)?;

        if self.sio.r2000_plus {
            // Scale flags (BB)
            let sx = insert.x_scale;
            let sy = insert.y_scale;
            let sz = insert.z_scale;
            if sx == 1.0 && sy == 1.0 && sz == 1.0 {
                writer.write_2bits(3)?; // default scale (1,1,1)
            } else {
                writer.write_2bits(0)?; // explicit scales
                writer.write_bit_double(sx)?;
                writer.write_bit_double_with_default(sx, sy)?;
                writer.write_bit_double_with_default(sx, sz)?;
            }
        } else {
            writer.write_3bit_double(Vector3::new(
                insert.x_scale,
                insert.y_scale,
                insert.z_scale,
            ))?;
        }

        writer.write_bit_double(insert.rotation)?;
        writer.write_bit_extrusion(insert.normal)?;

        // Has ATTRIBs flag (B)
        writer.write_bit(has_attribs)?;

        // R2004+: Owned object count (BL) — only when has_attribs
        if self.sio.r2004_plus && has_attribs {
            writer.write_bit_long(attrib_handles.len() as i32)?;
        }

        // Block header handle (hard pointer)
        writer.handle_reference_typed(
            DwgReferenceType::HardPointer,
            self.resolve_block_handle(&insert.block_name),
        )?;

        // Attribute handles
        if has_attribs {
            if self.sio.r2004_plus {
                // R2004+: owned handles list (hard ownership)
                for &ah in attrib_handles {
                    writer.handle_reference_typed(
                        DwgReferenceType::HardOwnership,
                        ah,
                    )?;
                }
            } else {
                // R13–R2000: first and last attribute handles (soft pointer)
                if let Some(&first) = attrib_handles.first() {
                    writer.handle_reference_typed(
                        DwgReferenceType::SoftPointer,
                        first,
                    )?;
                }
                if let Some(&last) = attrib_handles.last() {
                    writer.handle_reference_typed(
                        DwgReferenceType::SoftPointer,
                        last,
                    )?;
                }
            }
            // SEQEND handle (hard ownership)
            writer.handle_reference_typed(
                DwgReferenceType::HardOwnership,
                seqend_handle,
            )?;
        }

        writer.write_spear_shift()?;
        self.finalize_entity(writer, insert.common.handle.value());
        Ok(())
    }

    /// Write a complete INSERT: parent INSERT + ATTRIB children + SEQEND.
    ///
    /// If the insert has no attributes, writes a simple insert.
    fn write_insert_composite(
        &mut self,
        insert: &Insert,
        owner_handle: u64,
    ) -> Result<()> {
        if insert.attributes.is_empty() {
            return self.write_insert_simple(insert, owner_handle);
        }

        let insert_handle = insert.common.handle.value();

        // Allocate handles for attribute children and SEQEND
        let mut next_h = self.next_available_handle();
        let mut attrib_handles = Vec::with_capacity(insert.attributes.len());
        let mut attrib_commons = Vec::with_capacity(insert.attributes.len());

        for _att in &insert.attributes {
            let h = next_h;
            next_h += 1;
            attrib_handles.push(h);
            let mut ac = EntityCommon::new();
            ac.handle = Handle::new(h);
            ac.layer = insert.common.layer.clone();
            ac.color = insert.common.color;
            attrib_commons.push(ac);
        }

        let seqend_h = next_h;
        let mut seqend_common = EntityCommon::new();
        seqend_common.handle = Handle::new(seqend_h);
        seqend_common.layer = insert.common.layer.clone();

        // 1. Write the parent INSERT with attribute references
        self.write_insert_inner(
            insert,
            owner_handle,
            true,
            &attrib_handles,
            seqend_h,
        )?;

        // 2. Write each ATTRIB as a separate entity owned by the insert
        for (i, attrib) in insert.attributes.iter().enumerate() {
            let mut att_clone = attrib.clone();
            att_clone.common = attrib_commons[i].clone();
            self.write_attribute(&att_clone, insert_handle)?;
        }

        // 3. Write SEQEND
        let seqend = Seqend { common: seqend_common };
        self.write_seqend(&seqend, insert_handle)?;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // SHAPE
    // -----------------------------------------------------------------------

    fn write_shape(&mut self, shape: &Shape, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Shape,
            &shape.common,
            owner_handle,
        )?;

        writer.write_3bit_double(shape.insertion_point)?;
        writer.write_bit_double(shape.size)?;
        writer.write_bit_double(shape.rotation)?;
        writer.write_bit_double(shape.relative_x_scale)?;
        writer.write_bit_double(shape.oblique_angle)?;
        writer.write_bit_double(shape.thickness)?;
        writer.write_bit_short(shape.shape_number as i16)?;
        writer.write_bit_extrusion(shape.normal)?;

        // Style handle
        writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, shape.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // TOLERANCE
    // -----------------------------------------------------------------------

    fn write_tolerance(&mut self, tolerance: &Tolerance, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Tolerance,
            &tolerance.common,
            owner_handle,
        )?;

        writer.write_bit_short(0)?; // unknown purpose version
        writer.write_bit_double(tolerance.text_height)?;
        writer.write_variable_text(&tolerance.text)?;
        writer.write_3bit_double(tolerance.insertion_point)?;
        writer.write_3bit_double(tolerance.direction)?;
        writer.write_bit_extrusion(tolerance.normal)?;

        // Dimstyle handle
        let dimstyle_handle = tolerance
            .dimension_style_handle
            .map(|h| h.value())
            .unwrap_or(0);
        writer.handle_reference_typed(DwgReferenceType::HardPointer, dimstyle_handle)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, tolerance.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // LEADER
    // -----------------------------------------------------------------------

    fn write_leader(&mut self, leader: &Leader, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Leader,
            &leader.common,
            owner_handle,
        )?;

        writer.write_bit(leader.arrow_enabled)?;
        writer.write_bit_short(leader.path_type as i16)?;
        writer.write_bit_short(leader.creation_type as i16)?;
        writer.write_bit_short(leader.hookline_direction as i16)?;
        writer.write_bit(leader.hookline_enabled)?;
        writer.write_bit_double(leader.text_height)?;
        writer.write_bit_double(leader.text_width)?;

        writer.write_bit_long(leader.vertices.len() as i32)?;
        for v in &leader.vertices {
            writer.write_3bit_double(*v)?;
        }

        // R14+: last vertex override + dim base point
        if self.sio.version() >= crate::types::DxfVersion::AC1014 {
            writer.write_3bit_double(
                leader.vertices.last().copied().unwrap_or(Vector3::ZERO),
            )?;
            writer.write_3bit_double(leader.annotation_offset)?;
        }

        writer.write_3bit_double(leader.normal)?;

        // Handles: ANN_HANDLE (soft pointer), DIMSTYLE (hard pointer)
        writer.handle_reference_typed(
            DwgReferenceType::SoftPointer,
            leader.annotation_handle.value(),
        )?;
        writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, leader.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // DIMENSION writers — all 7 sub-types
    // -----------------------------------------------------------------------

    /// Dispatch a `Dimension` enum to the appropriate sub-type writer.
    fn write_dimension(
        &mut self,
        dim: &Dimension,
        owner_handle: u64,
    ) -> Result<()> {
        match dim {
            Dimension::Ordinate(d) => self.write_dim_ordinate(d, owner_handle),
            Dimension::Linear(d) => self.write_dim_linear(d, owner_handle),
            Dimension::Aligned(d) => self.write_dim_aligned(d, owner_handle),
            Dimension::Angular3Pt(d) => self.write_dim_angular_3pt(d, owner_handle),
            Dimension::Angular2Ln(d) => self.write_dim_angular_2ln(d, owner_handle),
            Dimension::Radius(d) => self.write_dim_radius(d, owner_handle),
            Dimension::Diameter(d) => self.write_dim_diameter(d, owner_handle),
        }
    }

    /// Write common dimension base data shared by all 7 dimension types.
    ///
    /// Binary layout mirrors `read_common_dimension_data` in the reader:
    ///   R2010+: Version(RC) → Normal(3BD) → TextMidPt(2RD) → Elevation(BD)
    ///   → R2000+ Flags(RC) → UserText(TV) → TextRotation(BD) →
    ///   HorizontalDir(BD) → InsertScale(3BD) → InsertRotation(BD)
    ///   → R2000+: AttachPoint(BS) + LSStyle(BS) + LSFactor(BD) + ActualMeas(BD)
    ///   → R2007+: Unknown(B) + FlipArrow1(B) + FlipArrow2(B)
    ///   → InsertionPt(2RD) → Handles: DIMSTYLE + BLOCK
    fn write_dimension_common_data(
        &self,
        writer: &mut dyn crate::io::dwg::writer::stream_writer::IDwgStreamWriter,
        base: &DimensionBase,
    ) -> Result<()> {
        // R2010+: Version (RC)
        if self.sio.r2010_plus {
            writer.write_byte(base.version)?;
        }

        // Extrusion normal (3BD)
        writer.write_3bit_double(base.normal)?;

        // Text middle point (2RD) — XY only
        writer.write_2raw_double(crate::types::Vector2::new(
            base.text_middle_point.x,
            base.text_middle_point.y,
        ))?;

        // Elevation (BD) — Z coord shared by ECS points (11, 12, 16)
        writer.write_bit_double(base.definition_point.z)?;

        // R2000+: flags byte
        if self.sio.r2000_plus {
            // bit 0: NOT user-defined location → write 0x01 when NOT user-defined
            // For simplicity, default to 0 (user-defined location)
            writer.write_byte(0)?;
        }

        // User text (TV)
        let user_text = base.user_text.as_deref().unwrap_or(&base.text);
        writer.write_variable_text(user_text)?;

        // Text rotation (BD)
        writer.write_bit_double(base.text_rotation)?;

        // Horizontal direction (BD)
        writer.write_bit_double(base.horizontal_direction)?;

        // Insert scale (3BD) — hardcoded (1,1,1) per C# reference
        writer.write_3bit_double(Vector3::new(1.0, 1.0, 1.0))?;

        // Insert rotation (BD) — hardcoded 0
        writer.write_bit_double(0.0)?;

        // R2000+: attachment point, line spacing style/factor, actual measurement
        if self.sio.r2000_plus {
            writer.write_bit_short(base.attachment_point as i16)?;
            writer.write_bit_short(0)?; // line spacing style (0 = at least)
            writer.write_bit_double(base.line_spacing_factor)?;
            writer.write_bit_double(base.actual_measurement)?;
        }

        // R2007+: unknown + flip arrows
        if self.sio.r2007_plus {
            writer.write_bit(false)?; // unknown
            writer.write_bit(false)?; // flip arrow 1
            writer.write_bit(false)?; // flip arrow 2
        }

        // Insertion point (2RD) — XY of group code 12
        writer.write_2raw_double(crate::types::Vector2::new(
            base.insertion_point.x,
            base.insertion_point.y,
        ))?;

        Ok(())
    }

    /// Write the common dimension handle references: DIMSTYLE + anonymous BLOCK.
    fn write_dimension_handles(
        &self,
        writer: &mut dyn crate::io::dwg::writer::stream_writer::IDwgStreamWriter,
        base: &DimensionBase,
    ) -> Result<()> {
        // DIMSTYLE handle (hard pointer)
        writer.handle_reference_typed(
            DwgReferenceType::HardPointer,
            self.resolve_dimstyle_handle(&base.style_name),
        )?;
        // Anonymous block handle (hard pointer)
        writer.handle_reference_typed(
            DwgReferenceType::HardPointer,
            self.resolve_block_handle(&base.block_name),
        )?;
        Ok(())
    }

    // ----- Ordinate -----

    fn write_dim_ordinate(
        &mut self,
        dim: &DimensionOrdinate,
        owner_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::DimensionOrdinate,
            &dim.base.common,
            owner_handle,
        )?;

        self.write_dimension_common_data(&mut *writer, &dim.base)?;

        // Type-specific: 3BD pt10, 3BD pt13, 3BD pt14, RC flag
        writer.write_3bit_double(dim.definition_point)?;
        writer.write_3bit_double(dim.feature_location)?;
        writer.write_3bit_double(dim.leader_endpoint)?;
        writer.write_byte(if dim.is_ordinate_type_x { 1 } else { 0 })?;

        self.write_dimension_handles(&mut *writer, &dim.base)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, dim.base.common.handle.value());
        Ok(())
    }

    // ----- Linear -----

    fn write_dim_linear(
        &mut self,
        dim: &DimensionLinear,
        owner_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::DimensionLinear,
            &dim.base.common,
            owner_handle,
        )?;

        self.write_dimension_common_data(&mut *writer, &dim.base)?;

        // Type-specific: aligned fields + rotation
        // 3BD pt13 (first point)
        writer.write_3bit_double(dim.first_point)?;
        // 3BD pt14 (second point)
        writer.write_3bit_double(dim.second_point)?;
        // 3BD pt10 (definition point on dim line)
        writer.write_3bit_double(dim.definition_point)?;
        // BD ext_line_rotation
        writer.write_bit_double(dim.ext_line_rotation)?;
        // BD rotation (Linear only, not in Aligned)
        writer.write_bit_double(dim.rotation)?;

        self.write_dimension_handles(&mut *writer, &dim.base)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, dim.base.common.handle.value());
        Ok(())
    }

    // ----- Aligned -----

    fn write_dim_aligned(
        &mut self,
        dim: &DimensionAligned,
        owner_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::DimensionAligned,
            &dim.base.common,
            owner_handle,
        )?;

        self.write_dimension_common_data(&mut *writer, &dim.base)?;

        // Type-specific: 3BD pt13, 3BD pt14, 3BD pt10, BD ext_rotation
        writer.write_3bit_double(dim.first_point)?;
        writer.write_3bit_double(dim.second_point)?;
        writer.write_3bit_double(dim.definition_point)?;
        writer.write_bit_double(dim.ext_line_rotation)?;

        self.write_dimension_handles(&mut *writer, &dim.base)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, dim.base.common.handle.value());
        Ok(())
    }

    // ----- Angular 3-Point -----

    fn write_dim_angular_3pt(
        &mut self,
        dim: &DimensionAngular3Pt,
        owner_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::DimensionAng3Pt,
            &dim.base.common,
            owner_handle,
        )?;

        self.write_dimension_common_data(&mut *writer, &dim.base)?;

        // Type-specific: 3BD pt10, 3BD pt13, 3BD pt14, 3BD pt15
        writer.write_3bit_double(dim.definition_point)?;
        writer.write_3bit_double(dim.first_point)?;
        writer.write_3bit_double(dim.second_point)?;
        writer.write_3bit_double(dim.angle_vertex)?;

        self.write_dimension_handles(&mut *writer, &dim.base)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, dim.base.common.handle.value());
        Ok(())
    }

    // ----- Angular 2-Line -----

    fn write_dim_angular_2ln(
        &mut self,
        dim: &DimensionAngular2Ln,
        owner_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::DimensionAng2Ln,
            &dim.base.common,
            owner_handle,
        )?;

        self.write_dimension_common_data(&mut *writer, &dim.base)?;

        // Type-specific: 2RD pt16, 3BD pt13, 3BD pt14, 3BD pt15, 3BD pt10
        writer.write_2raw_double(crate::types::Vector2::new(
            dim.dimension_arc.x,
            dim.dimension_arc.y,
        ))?;
        writer.write_3bit_double(dim.first_point)?;
        writer.write_3bit_double(dim.second_point)?;
        writer.write_3bit_double(dim.angle_vertex)?;
        writer.write_3bit_double(dim.definition_point)?;

        self.write_dimension_handles(&mut *writer, &dim.base)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, dim.base.common.handle.value());
        Ok(())
    }

    // ----- Radius -----

    fn write_dim_radius(
        &mut self,
        dim: &DimensionRadius,
        owner_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::DimensionRadius,
            &dim.base.common,
            owner_handle,
        )?;

        self.write_dimension_common_data(&mut *writer, &dim.base)?;

        // Type-specific: 3BD pt10, 3BD pt15, BD leader_length
        writer.write_3bit_double(dim.definition_point)?;
        writer.write_3bit_double(dim.angle_vertex)?;
        writer.write_bit_double(dim.leader_length)?;

        self.write_dimension_handles(&mut *writer, &dim.base)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, dim.base.common.handle.value());
        Ok(())
    }

    // ----- Diameter -----

    fn write_dim_diameter(
        &mut self,
        dim: &DimensionDiameter,
        owner_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::DimensionDiameter,
            &dim.base.common,
            owner_handle,
        )?;

        self.write_dimension_common_data(&mut *writer, &dim.base)?;

        // Type-specific: 3BD pt10, 3BD pt15, BD leader_length
        writer.write_3bit_double(dim.definition_point)?;
        writer.write_3bit_double(dim.angle_vertex)?;
        writer.write_bit_double(dim.leader_length)?;

        self.write_dimension_handles(&mut *writer, &dim.base)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, dim.base.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // VIEWPORT entity
    // -----------------------------------------------------------------------

    fn write_viewport_entity(
        &mut self,
        viewport: &Viewport,
        owner_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Viewport,
            &viewport.common,
            owner_handle,
        )?;

        writer.write_3bit_double(viewport.center)?;
        writer.write_bit_double(viewport.width)?;
        writer.write_bit_double(viewport.height)?;

        if self.sio.r2000_plus {
            writer.write_3bit_double(viewport.view_target)?;
            writer.write_3bit_double(viewport.view_direction)?;
            writer.write_bit_double(viewport.twist_angle)?;
            writer.write_bit_double(viewport.view_height)?;
            writer.write_bit_double(viewport.lens_length)?;
            writer.write_bit_double(viewport.front_clip_z)?;
            writer.write_bit_double(viewport.back_clip_z)?;
            writer.write_bit_double(viewport.snap_angle)?;
            writer.write_2bit_double(crate::types::Vector2::new(
                viewport.view_center.x,
                viewport.view_center.y,
            ))?;
            writer.write_2bit_double(crate::types::Vector2::new(
                viewport.snap_base.x,
                viewport.snap_base.y,
            ))?;
            writer.write_2bit_double(crate::types::Vector2::new(
                viewport.snap_spacing.x,
                viewport.snap_spacing.y,
            ))?;
            writer.write_2bit_double(crate::types::Vector2::new(
                viewport.grid_spacing.x,
                viewport.grid_spacing.y,
            ))?;
            writer.write_bit_short(viewport.circle_sides)?;

            if self.sio.r2007_plus {
                writer.write_bit_short(viewport.grid_flags.to_bits())?;
                writer.write_bit_short(viewport.grid_major)?;
            }

            // Frozen layer count (BL)
            writer.write_bit_long(viewport.frozen_layers.len() as i32)?;

            writer.write_bit_long(viewport.status.to_bits())?;
            writer.write_variable_text("")?; // stylesheet
            writer.write_byte(viewport.render_mode as u8)?;
            writer.write_bit(viewport.ucs_icon_visible)?;
            writer.write_bit(viewport.ucs_per_viewport)?;
            writer.write_3bit_double(viewport.ucs_origin)?;
            writer.write_3bit_double(viewport.ucs_x_axis)?;
            writer.write_3bit_double(viewport.ucs_y_axis)?;
            writer.write_bit_double(viewport.elevation)?;
            writer.write_bit_short(viewport.ucs_ortho_type)?;

            if self.sio.r2004_plus {
                writer.write_bit_short(viewport.shade_plot_mode)?;
            }

            if self.sio.r2007_plus {
                writer.write_bit(viewport.default_lighting)?;
                writer.write_byte(viewport.default_lighting_type as u8)?;
                writer.write_bit_double(viewport.brightness)?;
                writer.write_bit_double(viewport.contrast)?;
                writer.write_cm_color(crate::types::Color::from_index(
                    (viewport.ambient_color & 0xFF) as i16,
                ))?;
            }
        }

        writer.write_spear_shift()?;
        self.finalize_entity(writer, viewport.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // BLOCK / ENDBLK / SEQEND
    // -----------------------------------------------------------------------

    fn write_block(&mut self, block: &Block, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Block,
            &block.common,
            owner_handle,
        )?;

        writer.write_variable_text(&block.name)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, block.common.handle.value());
        Ok(())
    }

    fn write_block_end(&mut self, block_end: &BlockEnd, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Endblk,
            &block_end.common,
            owner_handle,
        )?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, block_end.common.handle.value());
        Ok(())
    }

    fn write_seqend(&mut self, seqend: &Seqend, owner_handle: u64) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Seqend,
            &seqend.common,
            owner_handle,
        )?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, seqend.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // VERTEX 2D  (DwgObjectType::Vertex2D = 0x0A)
    // -----------------------------------------------------------------------

    pub(super) fn write_vertex_2d(
        &mut self,
        vertex: &Vertex2D,
        common: &crate::entities::EntityCommon,
        owner_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Vertex2D,
            common,
            owner_handle,
        )?;

        // RC: flags
        writer.write_byte(vertex.flags.bits())?;

        // 3BD: point
        writer.write_3bit_double(vertex.location)?;

        // BD: start_width — negative means same as end_width
        let sw = if vertex.start_width == vertex.end_width && vertex.start_width != 0.0 {
            -vertex.start_width
        } else {
            vertex.start_width
        };
        writer.write_bit_double(sw)?;

        // BD: end_width (only if start_width >= 0, i.e. they differ)
        if sw >= 0.0 {
            writer.write_bit_double(vertex.end_width)?;
        }

        // BD: bulge
        writer.write_bit_double(vertex.bulge)?;

        // BL: vertex id (R2010+ only)
        if self.sio.r2010_plus {
            writer.write_bit_long(vertex.id)?;
        }

        // BD: curve fit tangent direction
        writer.write_bit_double(vertex.curve_tangent)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // VERTEX 3D POLYLINE  (DwgObjectType::Vertex3D = 0x0B)
    // -----------------------------------------------------------------------

    pub(super) fn write_vertex_3d_polyline(
        &mut self,
        vertex: &Vertex3DPolyline,
        common: &crate::entities::EntityCommon,
        owner_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Vertex3D,
            common,
            owner_handle,
        )?;

        // RC: flags
        writer.write_byte(vertex.flags as u8)?;

        // 3BD: point
        writer.write_3bit_double(vertex.position)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // VERTEX MESH  (DwgObjectType::VertexMesh = 0x0C)
    // -----------------------------------------------------------------------

    pub(super) fn write_vertex_mesh(
        &mut self,
        vertex: &crate::entities::PolygonMeshVertex,
        common: &crate::entities::EntityCommon,
        owner_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::VertexMesh,
            common,
            owner_handle,
        )?;

        // RC: flags
        writer.write_byte(vertex.flags as u8)?;

        // 3BD: point
        writer.write_3bit_double(vertex.location)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // VERTEX PFACE  (DwgObjectType::VertexPface = 0x0D)
    // Same format as Vertex3D — position vertex in a polyface mesh
    // -----------------------------------------------------------------------

    pub(super) fn write_pface_vertex(
        &mut self,
        vertex: &crate::entities::PolyfaceVertex,
        common: &crate::entities::EntityCommon,
        owner_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::VertexPface,
            common,
            owner_handle,
        )?;

        // RC: flags
        writer.write_byte(vertex.flags.bits() as u8)?;

        // 3BD: point
        writer.write_3bit_double(vertex.location)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // VERTEX PFACE FACE  (DwgObjectType::VertexPfaceFace = 0x0E)
    // Face record referencing vertices by index
    // -----------------------------------------------------------------------

    pub(super) fn write_pface_face(
        &mut self,
        face: &crate::entities::PolyfaceFace,
        owner_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::VertexPfaceFace,
            &face.common,
            owner_handle,
        )?;

        // 4 × BS: face vertex indices
        writer.write_bit_short(face.index1)?;
        writer.write_bit_short(face.index2)?;
        writer.write_bit_short(face.index3)?;
        writer.write_bit_short(face.index4)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, face.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // POLYLINE 2D  (DwgObjectType::Polyline2D = 0x0F)
    // -----------------------------------------------------------------------

    fn write_polyline_2d(
        &mut self,
        polyline: &Polyline2D,
        owner_handle: u64,
        vertex_handles: &[u64],
        seqend_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Polyline2D,
            &polyline.common,
            owner_handle,
        )?;

        // BS: flags
        writer.write_bit_short(polyline.flags.bits() as i16)?;

        // BS: curve type (smooth surface type)
        writer.write_bit_short(polyline.smooth_surface as i16)?;

        // BD: start width
        writer.write_bit_double(polyline.start_width)?;

        // BD: end width
        writer.write_bit_double(polyline.end_width)?;

        // BT: thickness
        writer.write_bit_thickness(polyline.thickness)?;

        // BD: elevation
        writer.write_bit_double(polyline.elevation)?;

        // BE: normal (extrusion)
        writer.write_bit_extrusion(polyline.normal)?;

        // Owned objects (R2004+)
        if self.sio.r2004_plus {
            // BL: owned object count
            writer.write_bit_long(vertex_handles.len() as i32)?;
        }

        // Handle references for owned vertices
        if self.sio.r2004_plus {
            for &vh in vertex_handles {
                writer.handle_reference_typed(DwgReferenceType::HardOwnership, vh)?;
            }
        } else {
            // Pre-R2004: first vertex handle + last vertex handle
            let first = vertex_handles.first().copied().unwrap_or(0);
            let last = vertex_handles.last().copied().unwrap_or(0);
            writer.handle_reference_typed(DwgReferenceType::HardPointer, first)?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, last)?;
        }

        // H: SEQEND handle
        writer.handle_reference_typed(DwgReferenceType::SoftPointer, seqend_handle)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, polyline.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // POLYLINE 3D  (DwgObjectType::Polyline3D = 0x10)
    // -----------------------------------------------------------------------

    fn write_polyline_3d(
        &mut self,
        polyline: &Polyline3D,
        owner_handle: u64,
        vertex_handles: &[u64],
        seqend_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::Polyline3D,
            &polyline.common,
            owner_handle,
        )?;

        // RC: curve flags — derive from Polyline3DFlags
        let mut curve_flags: u8 = 0;
        if polyline.flags.closed {
            curve_flags |= 1;
        }
        if polyline.flags.spline_fit {
            curve_flags |= 4;
        }
        writer.write_byte(curve_flags)?;

        // RC: spline flags — smooth type
        let spline_flags: u8 = polyline.smooth_type as u8;
        writer.write_byte(spline_flags)?;

        // Owned objects (R2004+)
        if self.sio.r2004_plus {
            writer.write_bit_long(vertex_handles.len() as i32)?;
        }

        if self.sio.r2004_plus {
            for &vh in vertex_handles {
                writer.handle_reference_typed(DwgReferenceType::HardOwnership, vh)?;
            }
        } else {
            let first = vertex_handles.first().copied().unwrap_or(0);
            let last = vertex_handles.last().copied().unwrap_or(0);
            writer.handle_reference_typed(DwgReferenceType::HardPointer, first)?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, last)?;
        }

        // H: SEQEND handle
        writer.handle_reference_typed(DwgReferenceType::SoftPointer, seqend_handle)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, polyline.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // POLYFACE MESH  (DwgObjectType::PolylinePface = 0x1D)
    // -----------------------------------------------------------------------

    fn write_polyface_mesh(
        &mut self,
        mesh: &PolyfaceMesh,
        owner_handle: u64,
        child_handles: &[u64],
        seqend_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::PolylinePface,
            &mesh.common,
            owner_handle,
        )?;

        // BS: number of vertices
        writer.write_bit_short(mesh.vertices.len() as i16)?;

        // BS: number of faces
        writer.write_bit_short(mesh.faces.len() as i16)?;

        // Owned objects (R2004+)
        if self.sio.r2004_plus {
            writer.write_bit_long(child_handles.len() as i32)?;
        }

        if self.sio.r2004_plus {
            for &ch in child_handles {
                writer.handle_reference_typed(DwgReferenceType::HardOwnership, ch)?;
            }
        } else {
            let first = child_handles.first().copied().unwrap_or(0);
            let last = child_handles.last().copied().unwrap_or(0);
            writer.handle_reference_typed(DwgReferenceType::HardPointer, first)?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, last)?;
        }

        // H: SEQEND handle
        writer.handle_reference_typed(DwgReferenceType::SoftPointer, seqend_handle)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, mesh.common.handle.value());
        Ok(())
    }

    // -----------------------------------------------------------------------
    // POLYGON MESH  (DwgObjectType::PolylineMesh = 0x1E)
    // -----------------------------------------------------------------------

    fn write_polygon_mesh(
        &mut self,
        mesh: &crate::entities::PolygonMeshEntity,
        owner_handle: u64,
        vertex_handles: &[u64],
        seqend_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_entity_writer();
        self.write_common_entity_data(
            &mut *writer,
            DwgObjectType::PolylineMesh,
            &mesh.common,
            owner_handle,
        )?;

        // BS: flags
        writer.write_bit_short(mesh.flags.bits())?;

        // BS: curve type (smooth surface type)
        let curve_type: i16 = match mesh.smooth_type {
            crate::entities::SurfaceSmoothType::Quadratic => 5,
            crate::entities::SurfaceSmoothType::Cubic => 6,
            crate::entities::SurfaceSmoothType::Bezier => 8,
            _ => 0,
        };
        writer.write_bit_short(curve_type)?;

        // BS: M vertex count
        writer.write_bit_short(mesh.m_vertex_count)?;

        // BS: N vertex count
        writer.write_bit_short(mesh.n_vertex_count)?;

        // BS: M smooth density
        writer.write_bit_short(mesh.m_smooth_density)?;

        // BS: N smooth density
        writer.write_bit_short(mesh.n_smooth_density)?;

        // Owned objects (R2004+)
        if self.sio.r2004_plus {
            writer.write_bit_long(vertex_handles.len() as i32)?;
        }

        if self.sio.r2004_plus {
            for &vh in vertex_handles {
                writer.handle_reference_typed(DwgReferenceType::HardOwnership, vh)?;
            }
        } else {
            let first = vertex_handles.first().copied().unwrap_or(0);
            let last = vertex_handles.last().copied().unwrap_or(0);
            writer.handle_reference_typed(DwgReferenceType::HardPointer, first)?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, last)?;
        }

        // H: SEQEND handle
        writer.handle_reference_typed(DwgReferenceType::SoftPointer, seqend_handle)?;

        writer.write_spear_shift()?;
        self.finalize_entity(writer, mesh.common.handle.value());
        Ok(())
    }
}
