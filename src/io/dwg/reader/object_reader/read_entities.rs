//! Entity reading methods for the DWG object reader.
//!
//! Each method mirrors the corresponding `read*` method in ACadSharp's
//! `DwgObjectReader.cs` and `DwgObjectReader.Entities.cs`.

use crate::entities::*;
use crate::error::Result;
use crate::types::{Color, Handle, Vector2, Vector3};

use super::templates::*;
use super::{DwgObjectReader, ModelerGeoType, StreamSet};

impl DwgObjectReader {
    // -----------------------------------------------------------------------
    // Text / Attrib / Attdef
    // -----------------------------------------------------------------------

    /// Read a TEXT entity.
    pub(super) fn read_text(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;
        let (text, text_tmpl) = self.read_common_text_data(streams, entity_common)?;
        Ok(CadTemplate::TextEntity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            text_data: text_tmpl,
            entity: EntityType::Text(text),
        })
    }

    /// Read an ATTRIB entity.
    pub(super) fn read_attribute(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;
        let (attrib, text_tmpl) = self.read_common_att_data(streams, entity_common, false)?;
        Ok(CadTemplate::TextEntity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            text_data: text_tmpl,
            entity: EntityType::AttributeEntity(attrib),
        })
    }

    /// Read an ATTDEF entity.
    pub(super) fn read_attribute_definition(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;
        let (attdef, text_tmpl) = self.read_common_attdef_data(streams, entity_common)?;
        Ok(CadTemplate::TextEntity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            text_data: text_tmpl,
            entity: EntityType::AttributeDefinition(attdef),
        })
    }

    // -----------------------------------------------------------------------
    // Common text data
    // -----------------------------------------------------------------------

    fn read_common_text_data(
        &mut self,
        streams: &mut StreamSet,
        common: EntityCommon,
    ) -> Result<(Text, CadTextEntityTemplateData)> {
        let mut text_tmpl = CadTextEntityTemplateData::default();

        // R13-R14 path
        let elevation;
        let insertion_point;
        let alignment_point;
        let normal;
        let _thickness;
        let oblique_angle;
        let rotation;
        let height;
        let width_factor;
        let value;
        let _generation;
        let horizontal;
        let vertical;

        if self.sio.r13_14_only {
            elevation = streams.object_reader.read_bit_double()?;
            let ip = streams.object_reader.read_2raw_double()?;
            insertion_point = Vector3::new(ip.x, ip.y, elevation);
            let ap = streams.object_reader.read_2raw_double()?;
            alignment_point = Vector3::new(ap.x, ap.y, elevation);
            normal = streams.object_reader.read_bit_extrusion()?;
            _thickness = streams.object_reader.read_bit_thickness()?;
            oblique_angle = streams.object_reader.read_bit_double()?;
            rotation = streams.object_reader.read_bit_double()?;
            height = streams.object_reader.read_bit_double()?;
            width_factor = streams.object_reader.read_bit_double()?;
            value = streams.read_text()?;
            _generation = streams.object_reader.read_bit_short()?;
            horizontal = streams.object_reader.read_bit_short()?;
            vertical = streams.object_reader.read_bit_short()?;
        } else {
            // R2000+
            let data_flags = streams.object_reader.read_raw_char()?;
            elevation = if (data_flags & 1) == 0 {
                streams.object_reader.read_raw_double()?
            } else {
                0.0
            };
            let ip = streams.object_reader.read_2raw_double()?;
            insertion_point = Vector3::new(ip.x, ip.y, elevation);

            alignment_point = if (data_flags & 2) == 0 {
                let ap = streams.object_reader.read_2raw_double()?;
                Vector3::new(ap.x, ap.y, elevation)
            } else {
                insertion_point
            };

            normal = streams.object_reader.read_bit_extrusion()?;
            _thickness = streams.object_reader.read_bit_thickness()?;

            oblique_angle = if (data_flags & 4) == 0 {
                streams.object_reader.read_raw_double()?
            } else {
                0.0
            };
            rotation = if (data_flags & 8) == 0 {
                streams.object_reader.read_raw_double()?
            } else {
                0.0
            };

            height = streams.object_reader.read_raw_double()?;
            width_factor = if (data_flags & 0x10) == 0 {
                streams.object_reader.read_raw_double()?
            } else {
                1.0
            };

            value = streams.read_text()?;
            _generation = if (data_flags & 0x20) == 0 {
                streams.object_reader.read_bit_short()?
            } else {
                0
            };
            horizontal = if (data_flags & 0x40) == 0 {
                streams.object_reader.read_bit_short()?
            } else {
                0
            };
            vertical = if (data_flags & 0x80) == 0 {
                streams.object_reader.read_bit_short()?
            } else {
                0
            };
        }

        // Style handle.
        text_tmpl.style_handle = streams.handles_reader.handle_reference()?;

        let h_align = match horizontal {
            0 => TextHorizontalAlignment::Left,
            1 => TextHorizontalAlignment::Center,
            2 => TextHorizontalAlignment::Right,
            3 => TextHorizontalAlignment::Aligned,
            4 => TextHorizontalAlignment::Middle,
            5 => TextHorizontalAlignment::Fit,
            _ => TextHorizontalAlignment::Left,
        };
        let v_align = match vertical {
            0 => TextVerticalAlignment::Baseline,
            1 => TextVerticalAlignment::Bottom,
            2 => TextVerticalAlignment::Middle,
            3 => TextVerticalAlignment::Top,
            _ => TextVerticalAlignment::Baseline,
        };

        let text = Text {
            common,
            value,
            insertion_point,
            alignment_point: Some(alignment_point),
            height,
            rotation,
            width_factor,
            oblique_angle,
            style: String::new(), // resolved later from style_handle
            horizontal_alignment: h_align,
            vertical_alignment: v_align,
            normal,
        };

        Ok((text, text_tmpl))
    }

    /// Read common attribute data (shared by ATTRIB).
    fn read_common_att_data(
        &mut self,
        streams: &mut StreamSet,
        common: EntityCommon,
        _is_definition: bool,
    ) -> Result<(AttributeEntity, CadTextEntityTemplateData)> {
        let mut text_tmpl = CadTextEntityTemplateData::default();

        // Read text base data.
        let (text, _) = self.read_common_text_data(streams, common.clone())?;

        // Tag (TV).
        let tag = streams.read_text()?;

        // Field length (BS).
        let field_length = streams.object_reader.read_bit_short()?;

        // Flags (RC).
        let flags_val = streams.object_reader.read_raw_char()?;
        let flags = AttributeFlags {
            invisible: (flags_val & 1) != 0,
            constant: (flags_val & 2) != 0,
            verify: (flags_val & 4) != 0,
            preset: (flags_val & 8) != 0,
            locked_position: false,
            annotative: false,
        };

        // R2007+: lock position (B).
        let lock_position = if self.sio.r2007_plus {
            streams.object_reader.read_bit()?
        } else {
            false
        };

        // R2010+: has MText (B).
        let _has_mtext = if self.sio.r2010_plus {
            streams.object_reader.read_bit()?
        } else {
            false
        };

        // Style handle.
        text_tmpl.style_handle = streams.handles_reader.handle_reference()?;

        let attrib = AttributeEntity {
            common: text.common,
            tag,
            value: text.value,
            insertion_point: text.insertion_point,
            alignment_point: text.alignment_point.unwrap_or(text.insertion_point),
            height: text.height,
            rotation: text.rotation,
            width_factor: text.width_factor,
            oblique_angle: text.oblique_angle,
            text_style: String::new(),
            text_generation_flags: 0,
            horizontal_alignment: attribute_definition::HorizontalAlignment::Left,
            vertical_alignment: attribute_definition::VerticalAlignment::Baseline,
            flags,
            field_length,
            normal: text.normal,
            mtext_flag: attribute_definition::MTextFlag::SingleLine,
            is_multiline: false,
            line_count: 0,
            attdef_handle: Handle::NULL,
            lock_position,
        };

        Ok((attrib, text_tmpl))
    }

    /// Read common attribute definition data.
    fn read_common_attdef_data(
        &mut self,
        streams: &mut StreamSet,
        common: EntityCommon,
    ) -> Result<(AttributeDefinition, CadTextEntityTemplateData)> {
        let mut text_tmpl = CadTextEntityTemplateData::default();

        // Read text base data.
        let (text, _) = self.read_common_text_data(streams, common.clone())?;

        // Tag (TV).
        let tag = streams.read_text()?;

        // Field length (BS).
        let field_length = streams.object_reader.read_bit_short()?;

        // Flags (RC).
        let flags_val = streams.object_reader.read_raw_char()?;
        let flags = AttributeFlags {
            invisible: (flags_val & 1) != 0,
            constant: (flags_val & 2) != 0,
            verify: (flags_val & 4) != 0,
            preset: (flags_val & 8) != 0,
            locked_position: false,
            annotative: false,
        };

        // Prompt (TV).
        let prompt = streams.read_text()?;

        // R2007+: lock position (B).
        let lock_position = if self.sio.r2007_plus {
            streams.object_reader.read_bit()?
        } else {
            false
        };

        // R2010+: has MText (B).
        let _has_mtext = if self.sio.r2010_plus {
            streams.object_reader.read_bit()?
        } else {
            false
        };

        // Style handle.
        text_tmpl.style_handle = streams.handles_reader.handle_reference()?;

        let attdef = AttributeDefinition {
            common: text.common,
            tag,
            prompt,
            default_value: text.value,
            insertion_point: text.insertion_point,
            alignment_point: text.alignment_point.unwrap_or(text.insertion_point),
            height: text.height,
            rotation: text.rotation,
            width_factor: text.width_factor,
            oblique_angle: text.oblique_angle,
            text_style: String::new(),
            text_generation_flags: 0,
            horizontal_alignment: attribute_definition::HorizontalAlignment::Left,
            vertical_alignment: attribute_definition::VerticalAlignment::Baseline,
            flags,
            field_length,
            normal: text.normal,
            mtext_flag: attribute_definition::MTextFlag::SingleLine,
            is_multiline: false,
            line_count: 0,
            lock_position,
        };

        Ok((attdef, text_tmpl))
    }

    // -----------------------------------------------------------------------
    // Block / EndBlock / Seqend
    // -----------------------------------------------------------------------

    pub(super) fn read_block(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let name = streams.read_text()?;

        let block = Block {
            common: entity_common,
            name,
            base_point: Vector3::ZERO,
            description: String::new(),
            xref_path: String::new(),
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::Block(block),
        })
    }

    pub(super) fn read_end_block(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let endblk = BlockEnd {
            common: entity_common,
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::BlockEnd(endblk),
        })
    }

    pub(super) fn read_seqend(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let seqend = Seqend {
            common: entity_common,
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::Seqend(seqend),
        })
    }

    // -----------------------------------------------------------------------
    // Insert / MInsert
    // -----------------------------------------------------------------------

    pub(super) fn read_insert(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        self.read_insert_common_data(streams, false)
    }

    pub(super) fn read_minsert(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        self.read_insert_common_data(streams, true)
    }

    fn read_insert_common_data(
        &mut self,
        streams: &mut StreamSet,
        is_minsert: bool,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let insert_point = streams.object_reader.read_3bit_double()?;
        let x_scale;
        let y_scale;
        let z_scale;

        if self.sio.r2000_plus {
            // R2000+: scales as DD with default 1.0
            let data_flags = streams.object_reader.read_2bits()?;
            x_scale = if (data_flags & 1) == 0 {
                streams.object_reader.read_raw_double()? // DD with default
            } else {
                1.0
            };
            y_scale = if (data_flags & 2) == 0 {
                streams.object_reader.read_bit_double_with_default(x_scale)?
            } else {
                x_scale
            };
            z_scale = streams.object_reader.read_bit_double_with_default(x_scale)?;
        } else {
            x_scale = streams.object_reader.read_bit_double()?;
            y_scale = streams.object_reader.read_bit_double()?;
            z_scale = streams.object_reader.read_bit_double()?;
        }

        let rotation = streams.object_reader.read_bit_double()?;
        let normal = streams.object_reader.read_bit_extrusion()?;

        let has_atts = streams.object_reader.read_bit()?;

        // Owned objects count (R2004+).
        let owned_count = if self.sio.r2004_plus && has_atts {
            streams.object_reader.read_bit_long()?
        } else {
            0
        };

        let mut column_count = 1u16;
        let mut row_count = 1u16;
        let mut column_spacing = 0.0;
        let mut row_spacing = 0.0;

        if is_minsert {
            column_count = streams.object_reader.read_bit_short()? as u16;
            row_count = streams.object_reader.read_bit_short()? as u16;
            column_spacing = streams.object_reader.read_bit_double()?;
            row_spacing = streams.object_reader.read_bit_double()?;
        }

        // Block header handle.
        let block_header_handle = streams.handles_reader.handle_reference()?;

        let mut insert_tmpl = CadInsertTemplateData {
            block_header_handle,
            has_atts,
            owned_objects_count: owned_count,
            ..Default::default()
        };

        // Attribute handles.
        if has_atts {
            if self.sio.r2004_plus {
                for _ in 0..owned_count {
                    let h = streams.handles_reader.handle_reference()?;
                    insert_tmpl.owned_objects_handles.push(h);
                }
            } else {
                insert_tmpl.first_attribute_handle =
                    streams.handles_reader.handle_reference()?;
                insert_tmpl.end_attribute_handle =
                    streams.handles_reader.handle_reference()?;
            }
            insert_tmpl.seqend_handle = streams.handles_reader.handle_reference()?;
        }

        let insert = Insert {
            common: entity_common,
            block_name: String::new(), // resolved later
            insert_point,
            x_scale,
            y_scale,
            z_scale,
            rotation,
            normal,
            column_count,
            row_count,
            column_spacing,
            row_spacing,
        };

        Ok(CadTemplate::Insert {
            common: common_tmpl,
            entity_data: ent_tmpl,
            insert_data: insert_tmpl,
            entity: EntityType::Insert(insert),
        })
    }

    // -----------------------------------------------------------------------
    // Vertices
    // -----------------------------------------------------------------------

    pub(super) fn read_vertex_2d(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let flags = streams.object_reader.read_raw_char()?;
        let pt = streams.object_reader.read_3bit_double()?;
        let start_width = streams.object_reader.read_bit_double()?;
        let end_width = if start_width < 0.0 {
            start_width.abs()
        } else {
            streams.object_reader.read_bit_double()?
        };
        let bulge = streams.object_reader.read_bit_double()?;
        let _id = if self.sio.r2010_plus {
            streams.object_reader.read_bit_long()?
        } else {
            0
        };
        let tangent = streams.object_reader.read_bit_double()?;

        let vertex = Vertex2D {
            location: pt,
            flags: VertexFlags::from_bits(flags),
            start_width: start_width.abs(),
            end_width,
            bulge,
            curve_tangent: tangent,
            id: 0,
        };

        // Vertices are wrapped as part of the Polyline template during
        // builder resolution; for now, store as a generic entity.
        let _ = vertex;
        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::Unknown(UnknownEntity {
                common: entity_common,
                dxf_name: "VERTEX_2D".to_string(),
            }),
        })
    }

    pub(super) fn read_vertex_3d(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let flags = streams.object_reader.read_raw_char()?;
        let pt = streams.object_reader.read_3bit_double()?;

        let _ = (flags, pt);
        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::Unknown(UnknownEntity {
                common: entity_common,
                dxf_name: "VERTEX_3D".to_string(),
            }),
        })
    }

    pub(super) fn read_pface_vertex(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let i1 = streams.object_reader.read_bit_short()?;
        let i2 = streams.object_reader.read_bit_short()?;
        let i3 = streams.object_reader.read_bit_short()?;
        let i4 = streams.object_reader.read_bit_short()?;

        let _ = (i1, i2, i3, i4);
        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::Unknown(UnknownEntity {
                common: entity_common,
                dxf_name: "VERTEX_PFACE_FACE".to_string(),
            }),
        })
    }

    // -----------------------------------------------------------------------
    // Polylines
    // -----------------------------------------------------------------------

    pub(super) fn read_polyline_2d(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let flags = streams.object_reader.read_bit_short()? as u16;
        let _curve_type = streams.object_reader.read_bit_short()?;
        let start_width = streams.object_reader.read_bit_double()?;
        let end_width = streams.object_reader.read_bit_double()?;
        let thickness = streams.object_reader.read_bit_thickness()?;
        let elevation = streams.object_reader.read_bit_double()?;
        let normal = streams.object_reader.read_bit_extrusion()?;

        // Owned objects (R2004+).
        let owned_count = if self.sio.r2004_plus {
            streams.object_reader.read_bit_long()?
        } else {
            0
        };

        let mut poly_tmpl = CadPolylineTemplateData::default();
        if self.sio.r2004_plus {
            for _ in 0..owned_count {
                let h = streams.handles_reader.handle_reference()?;
                poly_tmpl.owned_objects_handles.push(h);
            }
        } else {
            poly_tmpl.first_vertex_handle = streams.handles_reader.handle_reference()?;
            poly_tmpl.last_vertex_handle = streams.handles_reader.handle_reference()?;
        }
        poly_tmpl.seqend_handle = streams.handles_reader.handle_reference()?;

        let polyline = Polyline2D {
            common: entity_common,
            flags: PolylineFlags::from_bits(flags),
            smooth_surface: SmoothSurfaceType::None,
            start_width,
            end_width,
            elevation,
            thickness,
            normal,
            vertices: Vec::new(), // populated by builder
        };

        Ok(CadTemplate::Polyline {
            common: common_tmpl,
            entity_data: ent_tmpl,
            polyline_data: poly_tmpl,
            entity: EntityType::Polyline2D(polyline),
        })
    }

    pub(super) fn read_polyline_3d(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let _curve_flags = streams.object_reader.read_raw_char()?;
        let _spline_flags = streams.object_reader.read_raw_char()?;

        // Owned objects (R2004+).
        let owned_count = if self.sio.r2004_plus {
            streams.object_reader.read_bit_long()?
        } else {
            0
        };

        let mut poly_tmpl = CadPolylineTemplateData::default();
        if self.sio.r2004_plus {
            for _ in 0..owned_count {
                let h = streams.handles_reader.handle_reference()?;
                poly_tmpl.owned_objects_handles.push(h);
            }
        } else {
            poly_tmpl.first_vertex_handle = streams.handles_reader.handle_reference()?;
            poly_tmpl.last_vertex_handle = streams.handles_reader.handle_reference()?;
        }
        poly_tmpl.seqend_handle = streams.handles_reader.handle_reference()?;

        let polyline = Polyline3D {
            common: entity_common,
            flags: Polyline3DFlags::default(),
            smooth_type: polyline3d::SmoothSurfaceType::None,
            vertices: Vec::new(), // populated by builder
            ..Default::default()
        };

        Ok(CadTemplate::Polyline {
            common: common_tmpl,
            entity_data: ent_tmpl,
            polyline_data: poly_tmpl,
            entity: EntityType::Polyline3D(polyline),
        })
    }

    /// Read POLYFACE (PolylinePface type = 0x1D).
    pub(super) fn read_polyface_mesh(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let _num_vertices = streams.object_reader.read_bit_short()?;
        let _num_faces = streams.object_reader.read_bit_short()?;

        // Owned objects (R2004+).
        let owned_count = if self.sio.r2004_plus {
            streams.object_reader.read_bit_long()?
        } else {
            0
        };

        let mut poly_tmpl = CadPolylineTemplateData::default();
        if self.sio.r2004_plus {
            for _ in 0..owned_count {
                let h = streams.handles_reader.handle_reference()?;
                poly_tmpl.owned_objects_handles.push(h);
            }
        } else {
            poly_tmpl.first_vertex_handle = streams.handles_reader.handle_reference()?;
            poly_tmpl.last_vertex_handle = streams.handles_reader.handle_reference()?;
        }
        poly_tmpl.seqend_handle = streams.handles_reader.handle_reference()?;

        let mesh = PolyfaceMesh {
            common: entity_common,
            elevation: 0.0,
            flags: polyface_mesh::PolyfaceMeshFlags::empty(),
            normal: Vector3::UNIT_Z,
            start_width: 0.0,
            end_width: 0.0,
            smooth_surface: polyface_mesh::PolyfaceSmoothType::None,
            thickness: 0.0,
            vertices: Vec::new(),
            faces: Vec::new(),
            seqend_handle: None,
        };

        Ok(CadTemplate::Polyline {
            common: common_tmpl,
            entity_data: ent_tmpl,
            polyline_data: poly_tmpl,
            entity: EntityType::PolyfaceMesh(mesh),
        })
    }

    /// Read POLYGON MESH (PolylineMesh type = 0x1E).
    pub(super) fn read_polygon_mesh(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let flags = streams.object_reader.read_bit_short()?;
        let curve_type = streams.object_reader.read_bit_short()?;
        let m_count = streams.object_reader.read_bit_short()?;
        let n_count = streams.object_reader.read_bit_short()?;
        let m_density = streams.object_reader.read_bit_short()?;
        let n_density = streams.object_reader.read_bit_short()?;

        // Owned objects (R2004+).
        let owned_count = if self.sio.r2004_plus {
            streams.object_reader.read_bit_long()?
        } else {
            0
        };

        let mut poly_tmpl = CadPolylineTemplateData::default();
        if self.sio.r2004_plus {
            for _ in 0..owned_count {
                let h = streams.handles_reader.handle_reference()?;
                poly_tmpl.owned_objects_handles.push(h);
            }
        } else {
            poly_tmpl.first_vertex_handle = streams.handles_reader.handle_reference()?;
            poly_tmpl.last_vertex_handle = streams.handles_reader.handle_reference()?;
        }
        poly_tmpl.seqend_handle = streams.handles_reader.handle_reference()?;

        let smooth_type = match curve_type {
            5 => polygon_mesh::SurfaceSmoothType::Quadratic,
            6 => polygon_mesh::SurfaceSmoothType::Cubic,
            8 => polygon_mesh::SurfaceSmoothType::Bezier,
            _ => polygon_mesh::SurfaceSmoothType::NoSmooth,
        };

        let mesh = polygon_mesh::PolygonMesh {
            common: entity_common,
            flags: polygon_mesh::PolygonMeshFlags::from_bits(flags as i16).unwrap_or(polygon_mesh::PolygonMeshFlags::empty()),
            m_vertex_count: m_count,
            n_vertex_count: n_count,
            m_smooth_density: m_density,
            n_smooth_density: n_density,
            smooth_type,
            elevation: 0.0,
            normal: Vector3::UNIT_Z,
            vertices: Vec::new(),
        };

        Ok(CadTemplate::Polyline {
            common: common_tmpl,
            entity_data: ent_tmpl,
            polyline_data: poly_tmpl,
            entity: EntityType::PolygonMesh(mesh),
        })
    }

    // -----------------------------------------------------------------------
    // Basic geometry
    // -----------------------------------------------------------------------

    pub(super) fn read_arc(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let center = streams.object_reader.read_3bit_double()?;
        let radius = streams.object_reader.read_bit_double()?;
        let thickness = streams.object_reader.read_bit_thickness()?;
        let normal = streams.object_reader.read_bit_extrusion()?;
        let start_angle = streams.object_reader.read_bit_double()?;
        let end_angle = streams.object_reader.read_bit_double()?;

        let arc = Arc {
            common: entity_common,
            center,
            radius,
            start_angle,
            end_angle,
            thickness,
            normal,
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::Arc(arc),
        })
    }

    pub(super) fn read_circle(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let center = streams.object_reader.read_3bit_double()?;
        let radius = streams.object_reader.read_bit_double()?;
        let thickness = streams.object_reader.read_bit_thickness()?;
        let normal = streams.object_reader.read_bit_extrusion()?;

        let circle = Circle {
            common: entity_common,
            center,
            radius,
            thickness,
            normal,
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::Circle(circle),
        })
    }

    pub(super) fn read_line(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        // R13-R14: simple 3BD + 3BD + thickness + normal.
        // R2000+: optimised with data flags.
        let start;
        let end;
        let thickness;
        let normal;

        if self.sio.r13_14_only {
            start = streams.object_reader.read_3bit_double()?;
            end = streams.object_reader.read_3bit_double()?;
            thickness = streams.object_reader.read_bit_thickness()?;
            normal = streams.object_reader.read_bit_extrusion()?;
        } else {
            // R2000+
            let z_are_zero = streams.object_reader.read_bit()?;
            let x1 = streams.object_reader.read_raw_double()?;
            let x2 = streams.object_reader.read_bit_double_with_default(x1)?;
            let y1 = streams.object_reader.read_raw_double()?;
            let y2 = streams.object_reader.read_bit_double_with_default(y1)?;

            if z_are_zero {
                start = Vector3::new(x1, y1, 0.0);
                end = Vector3::new(x2, y2, 0.0);
            } else {
                let z1 = streams.object_reader.read_raw_double()?;
                let z2 = streams.object_reader.read_bit_double_with_default(z1)?;
                start = Vector3::new(x1, y1, z1);
                end = Vector3::new(x2, y2, z2);
            }

            thickness = streams.object_reader.read_bit_thickness()?;
            normal = streams.object_reader.read_bit_extrusion()?;
        }

        let line = Line {
            common: entity_common,
            start,
            end,
            thickness,
            normal,
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::Line(line),
        })
    }

    pub(super) fn read_point(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let location = streams.object_reader.read_3bit_double()?;
        let thickness = streams.object_reader.read_bit_thickness()?;
        let normal = streams.object_reader.read_bit_extrusion()?;
        let _x_axis_angle = streams.object_reader.read_bit_double()?;

        let point = Point {
            common: entity_common,
            location,
            thickness,
            normal,
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::Point(point),
        })
    }

    pub(super) fn read_3d_face(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let has_no_flags;
        let z_is_zero;
        let c1;
        let c2;
        let c3;
        let c4;
        let invisible_edge;

        if self.sio.r2000_plus {
            has_no_flags = streams.object_reader.read_bit()?;
            z_is_zero = streams.object_reader.read_bit()?;

            let x1 = streams.object_reader.read_raw_double()?;
            let y1 = streams.object_reader.read_raw_double()?;
            let z1 = if z_is_zero { 0.0 } else { streams.object_reader.read_raw_double()? };
            c1 = Vector3::new(x1, y1, z1);

            c2 = Vector3::new(
                streams.object_reader.read_bit_double_with_default(x1)?,
                streams.object_reader.read_bit_double_with_default(y1)?,
                streams.object_reader.read_bit_double_with_default(z1)?,
            );
            c3 = Vector3::new(
                streams.object_reader.read_bit_double_with_default(c2.x)?,
                streams.object_reader.read_bit_double_with_default(c2.y)?,
                streams.object_reader.read_bit_double_with_default(c2.z)?,
            );
            c4 = Vector3::new(
                streams.object_reader.read_bit_double_with_default(c3.x)?,
                streams.object_reader.read_bit_double_with_default(c3.y)?,
                streams.object_reader.read_bit_double_with_default(c3.z)?,
            );

            invisible_edge = if has_no_flags {
                0u8
            } else {
                streams.object_reader.read_bit_short()? as u8
            };
        } else {
            c1 = streams.object_reader.read_3bit_double()?;
            c2 = streams.object_reader.read_3bit_double()?;
            c3 = streams.object_reader.read_3bit_double()?;
            c4 = streams.object_reader.read_3bit_double()?;
            invisible_edge = streams.object_reader.read_bit_short()? as u8;
        }

        let face = Face3D {
            common: entity_common,
            first_corner: c1,
            second_corner: c2,
            third_corner: c3,
            fourth_corner: c4,
            invisible_edges: InvisibleEdgeFlags::from_bits(invisible_edge),
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::Face3D(face),
        })
    }

    pub(super) fn read_solid(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let thickness = streams.object_reader.read_bit_thickness()?;
        let elevation = streams.object_reader.read_bit_double()?;
        let c1_2d = streams.object_reader.read_2raw_double()?;
        let c2_2d = streams.object_reader.read_2raw_double()?;
        let c3_2d = streams.object_reader.read_2raw_double()?;
        let c4_2d = streams.object_reader.read_2raw_double()?;
        let normal = streams.object_reader.read_bit_extrusion()?;

        let solid = Solid {
            common: entity_common,
            first_corner: Vector3::new(c1_2d.x, c1_2d.y, elevation),
            second_corner: Vector3::new(c2_2d.x, c2_2d.y, elevation),
            third_corner: Vector3::new(c3_2d.x, c3_2d.y, elevation),
            fourth_corner: Vector3::new(c4_2d.x, c4_2d.y, elevation),
            normal,
            thickness,
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::Solid(solid),
        })
    }

    pub(super) fn read_shape(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let insertion_point = streams.object_reader.read_3bit_double()?;
        let size = streams.object_reader.read_bit_double()?;
        let rotation = streams.object_reader.read_bit_double()?;
        let width_factor = streams.object_reader.read_bit_double()?;
        let oblique_angle = streams.object_reader.read_bit_double()?;
        let thickness = streams.object_reader.read_bit_thickness()?;
        let normal = streams.object_reader.read_bit_extrusion()?;
        let shape_number = streams.object_reader.read_bit_short()? as i32;

        let shape_file_handle = streams.handles_reader.handle_reference()?;

        let shape = Shape {
            common: entity_common,
            insertion_point,
            size,
            shape_name: String::new(),
            shape_number,
            rotation,
            relative_x_scale: width_factor,
            oblique_angle,
            normal,
            thickness,
            style_name: String::new(),
            style_handle: None,
        };

        Ok(CadTemplate::Shape {
            common: common_tmpl,
            entity_data: ent_tmpl,
            shape_data: CadShapeTemplateData { shape_file_handle },
            entity: EntityType::Shape(shape),
        })
    }

    // -----------------------------------------------------------------------
    // Dimensions
    // -----------------------------------------------------------------------

    fn read_common_dimension_data(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<(CadTemplateCommon, CadEntityTemplateData, DimensionBase, CadDimensionTemplateData)> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let version = if self.sio.r2010_plus {
            streams.object_reader.read_raw_char()?
        } else {
            0
        };
        let normal = streams.object_reader.read_bit_extrusion()?;
        let text_midpoint = streams.object_reader.read_2raw_double()?;
        let elevation = streams.object_reader.read_bit_double()?;

        // R2000+: special flags.
        let _flags = if self.sio.r2000_plus {
            streams.object_reader.read_raw_char()?
        } else {
            0
        };

        let user_text = streams.read_text()?;
        let text_rotation = streams.object_reader.read_bit_double()?;
        let horizontal_direction = streams.object_reader.read_bit_double()?;

        // Insertion scale / rotation.
        let _insert_scale = streams.object_reader.read_3bit_double()?;
        let _insert_rotation = streams.object_reader.read_bit_double()?;

        // R2000+: attachment point, line spacing style/factor.
        let _attachment_point;
        let line_spacing_factor;
        if self.sio.r2000_plus {
            _attachment_point = streams.object_reader.read_bit_short()?;
            let _ls_style = streams.object_reader.read_bit_short()?;
            line_spacing_factor = streams.object_reader.read_bit_double()?;
            let _actual_measurement = streams.object_reader.read_bit_double()?;
        } else {
            _attachment_point = 5; // middle-center
            line_spacing_factor = 1.0;
        }

        let definition_point;
        if self.sio.r2007_plus {
            let _unknown = streams.object_reader.read_bit()?;
            let _has_style_override = streams.object_reader.read_bit()?;
        }

        definition_point = streams.object_reader.read_2raw_double()?;

        let dim_base = DimensionBase {
            common: entity_common,
            definition_point: Vector3::new(
                definition_point.x,
                definition_point.y,
                elevation,
            ),
            text_middle_point: Vector3::new(text_midpoint.x, text_midpoint.y, elevation),
            insertion_point: Vector3::ZERO,
            dimension_type: dimension::DimensionType::Linear,
            attachment_point: dimension::AttachmentPointType::MiddleCenter,
            text: String::new(),
            user_text: if user_text.is_empty() {
                None
            } else {
                Some(user_text)
            },
            normal,
            text_rotation,
            horizontal_direction,
            style_name: String::new(),
            actual_measurement: 0.0,
            version,
            block_name: String::new(),
            line_spacing_factor,
        };

        Ok((common_tmpl, ent_tmpl, dim_base, CadDimensionTemplateData::default()))
    }

    fn read_common_dimension_handles(
        &mut self,
        streams: &mut StreamSet,
        dim_tmpl: &mut CadDimensionTemplateData,
    ) -> Result<()> {

        // Dimension style handle.
        dim_tmpl.style_handle = streams.handles_reader.handle_reference()?;
        // Block handle (anonymous dim block).
        dim_tmpl.block_handle = streams.handles_reader.handle_reference()?;

        Ok(())
    }

    pub(super) fn read_dim_ordinate(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, mut base, mut dim_tmpl) =
            self.read_common_dimension_data(streams)?;

        let pt_10 = streams.object_reader.read_3bit_double()?;
        let pt_13 = streams.object_reader.read_3bit_double()?;
        let pt_14 = streams.object_reader.read_3bit_double()?;
        let is_type_x = streams.object_reader.read_raw_char()? as u8;

        self.read_common_dimension_handles(streams, &mut dim_tmpl)?;

        base.dimension_type = dimension::DimensionType::Ordinate;

        let dim = DimensionOrdinate {
            base,
            definition_point: pt_10,
            feature_location: pt_13,
            leader_endpoint: pt_14,
            is_ordinate_type_x: (is_type_x & 1) != 0,
        };

        Ok(CadTemplate::Dimension {
            common: common_tmpl,
            entity_data: ent_tmpl,
            dim_data: dim_tmpl,
            entity: EntityType::Dimension(Dimension::Ordinate(dim)),
        })
    }

    pub(super) fn read_dim_linear(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, mut base, mut dim_tmpl) =
            self.read_common_dimension_data(streams)?;

        let pt_13 = streams.object_reader.read_3bit_double()?;
        let pt_14 = streams.object_reader.read_3bit_double()?;
        let pt_10 = streams.object_reader.read_3bit_double()?;
        let rotation = streams.object_reader.read_bit_double()?;
        let ext_rotation = streams.object_reader.read_bit_double()?;

        self.read_common_dimension_handles(streams, &mut dim_tmpl)?;

        base.dimension_type = dimension::DimensionType::Linear;

        let dim = DimensionLinear {
            base,
            first_point: pt_13,
            second_point: pt_14,
            definition_point: pt_10,
            rotation,
            ext_line_rotation: ext_rotation,
        };

        Ok(CadTemplate::Dimension {
            common: common_tmpl,
            entity_data: ent_tmpl,
            dim_data: dim_tmpl,
            entity: EntityType::Dimension(Dimension::Linear(dim)),
        })
    }

    pub(super) fn read_dim_aligned(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, mut base, mut dim_tmpl) =
            self.read_common_dimension_data(streams)?;

        let pt_13 = streams.object_reader.read_3bit_double()?;
        let pt_14 = streams.object_reader.read_3bit_double()?;
        let pt_10 = streams.object_reader.read_3bit_double()?;
        let ext_rotation = streams.object_reader.read_bit_double()?;

        self.read_common_dimension_handles(streams, &mut dim_tmpl)?;

        base.dimension_type = dimension::DimensionType::Aligned;

        let dim = DimensionAligned {
            base,
            first_point: pt_13,
            second_point: pt_14,
            definition_point: pt_10,
            ext_line_rotation: ext_rotation,
        };

        Ok(CadTemplate::Dimension {
            common: common_tmpl,
            entity_data: ent_tmpl,
            dim_data: dim_tmpl,
            entity: EntityType::Dimension(Dimension::Aligned(dim)),
        })
    }

    pub(super) fn read_dim_angular_3pt(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, mut base, mut dim_tmpl) =
            self.read_common_dimension_data(streams)?;

        let pt_10 = streams.object_reader.read_3bit_double()?;
        let pt_13 = streams.object_reader.read_3bit_double()?;
        let pt_14 = streams.object_reader.read_3bit_double()?;
        let pt_15 = streams.object_reader.read_3bit_double()?;

        self.read_common_dimension_handles(streams, &mut dim_tmpl)?;

        base.dimension_type = dimension::DimensionType::Angular3Point;

        let dim = DimensionAngular3Pt {
            base,
            definition_point: pt_10,
            first_point: pt_13,
            second_point: pt_14,
            angle_vertex: pt_15,
        };

        Ok(CadTemplate::Dimension {
            common: common_tmpl,
            entity_data: ent_tmpl,
            dim_data: dim_tmpl,
            entity: EntityType::Dimension(Dimension::Angular3Pt(dim)),
        })
    }

    pub(super) fn read_dim_angular_2ln(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, mut base, mut dim_tmpl) =
            self.read_common_dimension_data(streams)?;

        let pt_16 = streams.object_reader.read_3bit_double()?;
        let pt_13 = streams.object_reader.read_3bit_double()?;
        let pt_14 = streams.object_reader.read_3bit_double()?;
        let pt_15 = streams.object_reader.read_3bit_double()?;
        let pt_10 = streams.object_reader.read_3bit_double()?;

        self.read_common_dimension_handles(streams, &mut dim_tmpl)?;

        base.dimension_type = dimension::DimensionType::Angular;

        let dim = DimensionAngular2Ln {
            base,
            dimension_arc: pt_16,
            first_point: pt_13,
            second_point: pt_14,
            angle_vertex: pt_15,
            definition_point: pt_10,
        };

        Ok(CadTemplate::Dimension {
            common: common_tmpl,
            entity_data: ent_tmpl,
            dim_data: dim_tmpl,
            entity: EntityType::Dimension(Dimension::Angular2Ln(dim)),
        })
    }

    pub(super) fn read_dim_radius(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, mut base, mut dim_tmpl) =
            self.read_common_dimension_data(streams)?;

        let pt_10 = streams.object_reader.read_3bit_double()?;
        let pt_15 = streams.object_reader.read_3bit_double()?;
        let leader_len = streams.object_reader.read_bit_double()?;

        self.read_common_dimension_handles(streams, &mut dim_tmpl)?;

        base.dimension_type = dimension::DimensionType::Radius;

        let dim = DimensionRadius {
            base,
            definition_point: pt_10,
            angle_vertex: pt_15,
            leader_length: leader_len,
        };

        Ok(CadTemplate::Dimension {
            common: common_tmpl,
            entity_data: ent_tmpl,
            dim_data: dim_tmpl,
            entity: EntityType::Dimension(Dimension::Radius(dim)),
        })
    }

    pub(super) fn read_dim_diameter(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, mut base, mut dim_tmpl) =
            self.read_common_dimension_data(streams)?;

        let pt_10 = streams.object_reader.read_3bit_double()?;
        let pt_15 = streams.object_reader.read_3bit_double()?;
        let leader_len = streams.object_reader.read_bit_double()?;

        self.read_common_dimension_handles(streams, &mut dim_tmpl)?;

        base.dimension_type = dimension::DimensionType::Diameter;

        let dim = DimensionDiameter {
            base,
            definition_point: pt_10,
            angle_vertex: pt_15,
            leader_length: leader_len,
        };

        Ok(CadTemplate::Dimension {
            common: common_tmpl,
            entity_data: ent_tmpl,
            dim_data: dim_tmpl,
            entity: EntityType::Dimension(Dimension::Diameter(dim)),
        })
    }

    // -----------------------------------------------------------------------
    // Complex entities
    // -----------------------------------------------------------------------

    pub(super) fn read_viewport(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let center = streams.object_reader.read_3bit_double()?;
        let width = streams.object_reader.read_bit_double()?;
        let height = streams.object_reader.read_bit_double()?;

        // R2000+: full viewport data.
        let mut vp = Viewport {
            common: entity_common,
            center,
            width,
            height,
            ..Viewport::default()
        };

        let mut vp_tmpl = CadViewportTemplateData::default();

        if self.sio.r2000_plus {
            vp.view_target = streams.object_reader.read_3bit_double()?;
            vp.view_direction = streams.object_reader.read_3bit_double()?;
            vp.twist_angle = streams.object_reader.read_bit_double()?;
            vp.view_height = streams.object_reader.read_bit_double()?;
            vp.lens_length = streams.object_reader.read_bit_double()?;
            vp.front_clip_z = streams.object_reader.read_bit_double()?;
            vp.back_clip_z = streams.object_reader.read_bit_double()?;
            vp.snap_angle = streams.object_reader.read_bit_double()?;

            let view_center = streams.object_reader.read_2raw_double()?;
            vp.view_center = Vector3::new(view_center.x, view_center.y, 0.0);

            let snap_base = streams.object_reader.read_2raw_double()?;
            vp.snap_base = Vector3::new(snap_base.x, snap_base.y, 0.0);

            let snap_spacing = streams.object_reader.read_2raw_double()?;
            vp.snap_spacing = Vector3::new(snap_spacing.x, snap_spacing.y, 0.0);

            let grid_spacing = streams.object_reader.read_2raw_double()?;
            vp.grid_spacing = Vector3::new(grid_spacing.x, grid_spacing.y, 0.0);

            vp.circle_sides = streams.object_reader.read_bit_short()?;

            if self.sio.r2000_plus {
                vp.grid_major = streams.object_reader.read_bit_short()?;
            }

            // Frozen layers count (BL).
            let frozen_count = streams.object_reader.read_bit_long()? as usize;

            let status_flags = streams.object_reader.read_bit_long()?;
            vp.status = ViewportStatusFlags::from_bits(status_flags);

            // Style name (TV).
            let _style = streams.read_text()?;

            // Render mode (RC).
            let rm = streams.object_reader.read_raw_char()?;
            vp.render_mode = ViewportRenderMode::from_value(rm as i16);

            // UCS per viewport (B).
            vp.ucs_per_viewport = streams.object_reader.read_bit()?;

            // UCS origin, X/Y axes.
            vp.ucs_origin = streams.object_reader.read_3bit_double()?;
            vp.ucs_x_axis = streams.object_reader.read_3bit_double()?;
            vp.ucs_y_axis = streams.object_reader.read_3bit_double()?;
            vp.elevation = streams.object_reader.read_bit_double()?;
            vp.ucs_ortho_type = streams.object_reader.read_bit_short()?;

            if self.sio.r2004_plus {
                vp.shade_plot_mode = streams.object_reader.read_bit_short()?;
            }

            if self.sio.r2007_plus {
                // Grid flags (BS).
                let gf = streams.object_reader.read_bit_short()? as u16;
                vp.grid_flags = GridFlags {
                    beyond_limits: (gf & 1) != 0,
                    adaptive: (gf & 2) != 0,
                    subdivision: (gf & 4) != 0,
                    follow_dynamic: (gf & 8) != 0,
                };

                vp.default_lighting = streams.object_reader.read_bit()?;
                vp.default_lighting_type = streams.object_reader.read_raw_char()? as i16;
                vp.brightness = streams.object_reader.read_bit_double()?;
                vp.contrast = streams.object_reader.read_bit_double()?;
                vp.ambient_color = streams.object_reader.read_raw_long()?;
            }

            // Handles.
            vp_tmpl.viewport_header_handle = streams.handles_reader.handle_reference()?;

            // Frozen layer handles.
            for _ in 0..frozen_count {
                let h = streams.handles_reader.handle_reference()?;
                vp_tmpl.frozen_layer_handles.push(h);
            }

            // Clip boundary handle.
            vp_tmpl.boundary_handle = streams.handles_reader.handle_reference()?;

            if self.sio.r2000_plus {
                vp_tmpl.named_ucs_handle = streams.handles_reader.handle_reference()?;
                vp_tmpl.base_ucs_handle = streams.handles_reader.handle_reference()?;
            }
        }

        Ok(CadTemplate::Viewport {
            common: common_tmpl,
            entity_data: ent_tmpl,
            viewport_data: vp_tmpl,
            entity: EntityType::Viewport(vp),
        })
    }

    pub(super) fn read_ellipse(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let center = streams.object_reader.read_3bit_double()?;
        let major_axis = streams.object_reader.read_3bit_double()?;
        let normal = streams.object_reader.read_3bit_double()?;
        let minor_axis_ratio = streams.object_reader.read_bit_double()?;
        let start_param = streams.object_reader.read_bit_double()?;
        let end_param = streams.object_reader.read_bit_double()?;

        let ellipse = Ellipse {
            common: entity_common,
            center,
            major_axis,
            minor_axis_ratio,
            start_parameter: start_param,
            end_parameter: end_param,
            normal,
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::Ellipse(ellipse),
        })
    }

    pub(super) fn read_spline(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let scenario;
        let degree;
        let mut sp_flags = SplineFlags {
            closed: false,
            periodic: false,
            rational: false,
            planar: false,
            linear: false,
        };

        if self.sio.r2013_plus {
            let flags1 = streams.object_reader.read_bit_long()?;
            let _knot_param = streams.object_reader.read_bit_long()?;
            degree = streams.object_reader.read_bit_long()?;
            // Determine scenario from flags.
            scenario = if (flags1 & 1) != 0 { 2 } else { 1 };
        } else {
            scenario = streams.object_reader.read_bit_short()? as i32;
            degree = streams.object_reader.read_bit_long()?;
        }

        #[allow(unused_assignments)]
        let mut num_fit_pts = 0i32;
        #[allow(unused_assignments)]
        let mut num_knots = 0i32;
        #[allow(unused_assignments)]
        let mut num_ctrl_pts = 0i32;
        #[allow(unused_assignments)]
        let mut weight_present = false;
        let mut knots = Vec::new();
        let mut control_points = Vec::new();
        let mut fit_points = Vec::new();
        let mut weights = Vec::new();
        let mut normal = Vector3::UNIT_Z;

        if scenario == 2 {
            // Fit point data.
            let fit_tol = streams.object_reader.read_bit_double()?;
            let _ = fit_tol;
            normal = streams.object_reader.read_3bit_double()?;
            let _tangent_start = streams.object_reader.read_3bit_double()?;
            let _tangent_end = streams.object_reader.read_3bit_double()?;
            num_fit_pts = streams.object_reader.read_bit_long()?;
            for _ in 0..num_fit_pts {
                fit_points.push(streams.object_reader.read_3bit_double()?);
            }
        } else if scenario == 1 {
            // Control point data.
            sp_flags.rational = streams.object_reader.read_bit()?;
            sp_flags.closed = streams.object_reader.read_bit()?;
            sp_flags.periodic = streams.object_reader.read_bit()?;
            let _knot_tol = streams.object_reader.read_bit_double()?;
            let _ctrl_tol = streams.object_reader.read_bit_double()?;
            num_knots = streams.object_reader.read_bit_long()?;
            num_ctrl_pts = streams.object_reader.read_bit_long()?;
            weight_present = streams.object_reader.read_bit()?;

            for _ in 0..num_knots {
                knots.push(streams.object_reader.read_bit_double()?);
            }
            for _ in 0..num_ctrl_pts {
                control_points.push(streams.object_reader.read_3bit_double()?);
                if weight_present {
                    weights.push(streams.object_reader.read_bit_double()?);
                }
            }
        }

        let spline = Spline {
            common: entity_common,
            degree,
            flags: sp_flags,
            knots,
            control_points,
            weights,
            fit_points,
            normal,
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::Spline(spline),
        })
    }

    pub(super) fn read_solid_3d(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        self.read_modeler_geometry(streams, ModelerGeoType::Solid3D)
    }

    pub(super) fn read_modeler_geometry(
        &mut self,
        streams: &mut StreamSet,
        geo_type: ModelerGeoType,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        // Version (RC).
        let acis_version = streams.object_reader.read_raw_char()?;

        // SAT/SAB data.
        let acis_data = if self.sio.r2007_plus {
            // R2007+: SAB binary chunks.
            let mut sab_data = Vec::new();
            loop {
                let chunk_len = streams.object_reader.read_bit_long()?;
                if chunk_len == 0 {
                    break;
                }
                let chunk = streams.object_reader.read_bytes(chunk_len as usize)?;
                sab_data.extend_from_slice(&chunk);
            }
            solid3d::AcisData {
                version: if acis_version >= 2 {
                    solid3d::AcisVersion::Version2
                } else {
                    solid3d::AcisVersion::Version1
                },
                sat_data: String::new(),
                sab_data,
                is_binary: true,
            }
        } else {
            // Pre-R2007: SAT text.
            let mut sat = String::new();
            loop {
                let line = streams.read_text()?;
                if line.is_empty() {
                    break;
                }
                sat.push_str(&line);
                sat.push('\n');
            }
            solid3d::AcisData {
                version: solid3d::AcisVersion::Version1,
                sat_data: sat,
                sab_data: Vec::new(),
                is_binary: false,
            }
        };

        // R2007+: has_ds_binary_data flag was already read.
        // R13-R14 wire/silhouette data.
        let _b = if !self.sio.r2000_plus {
            streams.object_reader.read_bit()?
        } else {
            false
        };

        let mut solid3d_tmpl = CadSolid3DTemplateData::default();

        // R2007+: history handle.
        if self.sio.r2007_plus {
            solid3d_tmpl.history_handle = streams.handles_reader.handle_reference()?;
        }

        match geo_type {
            ModelerGeoType::Solid3D => {
                let solid = Solid3D {
                    common: entity_common,
                    uid: String::new(),
                    point_of_reference: Vector3::ZERO,
                    acis_data,
                    wires: Vec::new(),
                    silhouettes: Vec::new(),
                    history_handle: None,
                };
                Ok(CadTemplate::Solid3D {
                    common: common_tmpl,
                    entity_data: ent_tmpl,
                    solid3d_data: solid3d_tmpl,
                    entity: EntityType::Solid3D(solid),
                })
            }
            ModelerGeoType::Region => {
                let region = Region {
                    common: entity_common,
                    uid: String::new(),
                    point_of_reference: Vector3::ZERO,
                    acis_data,
                    wires: Vec::new(),
                    silhouettes: Vec::new(),
                };
                Ok(CadTemplate::Entity {
                    common: common_tmpl,
                    entity_data: ent_tmpl,
                    entity: EntityType::Region(region),
                })
            }
            ModelerGeoType::Body => {
                let body = Body {
                    common: entity_common,
                    uid: String::new(),
                    point_of_reference: Vector3::ZERO,
                    acis_data,
                    wires: Vec::new(),
                    silhouettes: Vec::new(),
                };
                Ok(CadTemplate::Entity {
                    common: common_tmpl,
                    entity_data: ent_tmpl,
                    entity: EntityType::Body(body),
                })
            }
        }
    }

    pub(super) fn read_ray(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let base_point = streams.object_reader.read_3bit_double()?;
        let direction = streams.object_reader.read_3bit_double()?;

        let ray = Ray {
            common: entity_common,
            base_point,
            direction,
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::Ray(ray),
        })
    }

    pub(super) fn read_xline(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let base_point = streams.object_reader.read_3bit_double()?;
        let direction = streams.object_reader.read_3bit_double()?;

        let xline = XLine {
            common: entity_common,
            base_point,
            direction,
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::XLine(xline),
        })
    }

    pub(super) fn read_mtext(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let insertion_point = streams.object_reader.read_3bit_double()?;
        let normal = streams.object_reader.read_3bit_double()?;
        let direction = streams.object_reader.read_3bit_double()?;
        let rect_width = streams.object_reader.read_bit_double()?;

        let rect_height = if self.sio.r2007_plus {
            Some(streams.object_reader.read_bit_double()?)
        } else {
            None
        };

        let height = streams.object_reader.read_bit_double()?;
        let attachment = streams.object_reader.read_bit_short()?;
        let drawing_dir = streams.object_reader.read_bit_short()?;

        // Text value (potentially in chunks).
        let _ext_height = streams.object_reader.read_bit_double()?;
        let _ext_width = streams.object_reader.read_bit_double()?;
        let text_value = streams.read_text()?;

        // Line spacing (R2000+).
        let line_spacing = if self.sio.r2000_plus {
            let _ls_style = streams.object_reader.read_bit_short()?;
            streams.object_reader.read_bit_double()?
        } else {
            1.0
        };

        // Rotation angle.
        let rotation = direction.y.atan2(direction.x);

        // Style handle.
        let style_handle = streams.handles_reader.handle_reference()?;
        let text_tmpl = CadTextEntityTemplateData { style_handle };

        let attach_pt = match attachment {
            1 => mtext::AttachmentPoint::TopLeft,
            2 => mtext::AttachmentPoint::TopCenter,
            3 => mtext::AttachmentPoint::TopRight,
            4 => mtext::AttachmentPoint::MiddleLeft,
            5 => mtext::AttachmentPoint::MiddleCenter,
            6 => mtext::AttachmentPoint::MiddleRight,
            7 => mtext::AttachmentPoint::BottomLeft,
            8 => mtext::AttachmentPoint::BottomCenter,
            9 => mtext::AttachmentPoint::BottomRight,
            _ => mtext::AttachmentPoint::TopLeft,
        };

        let draw_dir = match drawing_dir {
            1 => mtext::DrawingDirection::LeftToRight,
            3 => mtext::DrawingDirection::TopToBottom,
            5 => mtext::DrawingDirection::ByStyle,
            _ => mtext::DrawingDirection::LeftToRight,
        };

        let mtext = MText {
            common: entity_common,
            value: text_value,
            insertion_point,
            height,
            rectangle_width: rect_width,
            rectangle_height: rect_height,
            rotation,
            style: String::new(),
            attachment_point: attach_pt,
            drawing_direction: draw_dir,
            line_spacing_factor: line_spacing,
            normal,
        };

        Ok(CadTemplate::TextEntity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            text_data: text_tmpl,
            entity: EntityType::MText(mtext),
        })
    }

    pub(super) fn read_leader(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let _unknown = streams.object_reader.read_bit()?;
        let annotation_type = streams.object_reader.read_bit_short()?;
        let path_type = streams.object_reader.read_bit_short()?;
        let num_pts = streams.object_reader.read_bit_long()? as usize;

        let mut vertices = Vec::with_capacity(num_pts);
        for _ in 0..num_pts {
            vertices.push(streams.object_reader.read_3bit_double()?);
        }

        let normal = streams.object_reader.read_3bit_double()?;
        let horizontal_direction = streams.object_reader.read_3bit_double()?;
        let block_offset = streams.object_reader.read_3bit_double()?;

        // R14+: end point.
        let _end_pt_proj = if self.sio.r13_14_only || self.sio.r2000_plus {
            streams.object_reader.read_3bit_double()?
        } else {
            Vector3::ZERO
        };

        // R2000+: dimension association data.
        let dimasz = if self.sio.r2000_plus {
            streams.object_reader.read_bit_double()?
        } else {
            0.0
        };

        let hookline_on = streams.object_reader.read_bit()?;
        let arrow_head_on = streams.object_reader.read_bit()?;

        let _arrowhead_size = if self.sio.r13_14_only {
            streams.object_reader.read_bit_double()?
        } else {
            0.0
        };

        let text_width = if self.sio.r13_14_only {
            streams.object_reader.read_bit_double()?
        } else {
            0.0
        };
        let text_height = if self.sio.r13_14_only {
            streams.object_reader.read_bit_double()?
        } else {
            0.0
        };

        let _color_val = streams.object_reader.read_bit_short()?;

        let mut leader_tmpl = CadLeaderTemplateData::default();
        leader_tmpl.dimasz = dimasz;
        // Annotation handle.
        leader_tmpl.annotation_handle = streams.handles_reader.handle_reference()?;
        // Dimstyle handle.
        leader_tmpl.dimstyle_handle = streams.handles_reader.handle_reference()?;

        let ldr_path = match path_type {
            0 => leader::LeaderPathType::StraightLine,
            1 => leader::LeaderPathType::Spline,
            _ => leader::LeaderPathType::StraightLine,
        };

        let creation_type = match annotation_type {
            0 => leader::LeaderCreationType::WithText,
            1 => leader::LeaderCreationType::WithTolerance,
            2 => leader::LeaderCreationType::WithBlock,
            _ => leader::LeaderCreationType::NoAnnotation,
        };

        let leader = Leader {
            common: entity_common,
            dimension_style: String::new(),
            arrow_enabled: arrow_head_on,
            path_type: ldr_path,
            creation_type,
            hookline_direction: leader::HooklineDirection::Same,
            hookline_enabled: hookline_on,
            text_height,
            text_width,
            vertices,
            override_color: Color::ByLayer,
            annotation_handle: Handle::NULL,
            normal,
            horizontal_direction,
            block_offset,
            annotation_offset: Vector3::ZERO,
        };

        Ok(CadTemplate::Leader {
            common: common_tmpl,
            entity_data: ent_tmpl,
            leader_data: leader_tmpl,
            entity: EntityType::Leader(leader),
        })
    }

    pub(super) fn read_tolerance(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let _version = streams.object_reader.read_bit_short()?;
        let text = streams.read_text()?;
        let insertion_point = streams.object_reader.read_3bit_double()?;
        let direction = streams.object_reader.read_3bit_double()?;

        // Dimstyle handle.
        let style_handle = streams.handles_reader.handle_reference()?;

        let tolerance = Tolerance {
            common: entity_common,
            insertion_point,
            direction,
            normal: Vector3::UNIT_Z,
            text,
            dimension_style_name: String::new(),
            dimension_style_handle: Some(Handle::new(style_handle)),
            text_height: 0.0,
            dimension_gap: 0.0,
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::Tolerance(tolerance),
        })
    }

    pub(super) fn read_mline(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let scale = streams.object_reader.read_bit_double()?;
        let justification = streams.object_reader.read_raw_char()?;
        let base_point = streams.object_reader.read_3bit_double()?;
        let normal = streams.object_reader.read_3bit_double()?;
        let flags = streams.object_reader.read_bit_short()?;
        let num_lines = streams.object_reader.read_raw_char()? as usize;
        let num_verts = streams.object_reader.read_bit_short()? as usize;

        let mut vertices = Vec::with_capacity(num_verts);
        for _ in 0..num_verts {
            let position = streams.object_reader.read_3bit_double()?;
            let dir = streams.object_reader.read_3bit_double()?;
            let miter = streams.object_reader.read_3bit_double()?;

            let mut segments = Vec::with_capacity(num_lines);
            for _ in 0..num_lines {
                let num_params = streams.object_reader.read_bit_short()? as usize;
                let mut params = Vec::with_capacity(num_params);
                for _ in 0..num_params {
                    params.push(streams.object_reader.read_bit_double()?);
                }
                let num_area = streams.object_reader.read_bit_short()? as usize;
                let mut area_fill = Vec::with_capacity(num_area);
                for _ in 0..num_area {
                    area_fill.push(streams.object_reader.read_bit_double()?);
                }
                segments.push(mline::MLineSegment {
                    parameters: params,
                    area_fill_parameters: area_fill,
                });
            }

            vertices.push(mline::MLineVertex {
                position,
                direction: dir,
                miter,
                segments,
            });
        }

        // MLineStyle handle.
        let _style_handle = streams.handles_reader.handle_reference()?;

        let just = match justification {
            0 => mline::MLineJustification::Top,
            1 => mline::MLineJustification::Zero,
            2 => mline::MLineJustification::Bottom,
            _ => mline::MLineJustification::Top,
        };

        let ml = MLine {
            common: entity_common,
            flags: mline::MLineFlags::from_bits_truncate(flags),
            justification: just,
            normal,
            scale_factor: scale,
            start_point: base_point,
            style_handle: None,
            style_name: String::new(),
            style_element_count: num_lines,
            vertices,
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::MLine(ml),
        })
    }

    pub(super) fn read_lwpolyline(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let flag = streams.object_reader.read_bit_short()?;
        let is_closed = (flag & 0x200) != 0;

        let const_width = if (flag & 4) != 0 {
            streams.object_reader.read_bit_double()?
        } else {
            0.0
        };
        let elevation = if (flag & 8) != 0 {
            streams.object_reader.read_bit_double()?
        } else {
            0.0
        };
        let thickness = if (flag & 2) != 0 {
            streams.object_reader.read_bit_double()?
        } else {
            0.0
        };
        let normal = if (flag & 1) != 0 {
            streams.object_reader.read_3bit_double()?
        } else {
            Vector3::UNIT_Z
        };

        let num_pts = streams.object_reader.read_bit_long()? as usize;
        if num_pts > 10_000_000 {
            return Err(crate::error::DxfError::Parse(format!(
                "LWPolyline point count {} is unreasonably large", num_pts
            )));
        }
        let num_bulges = if (flag & 0x10) != 0 {
            streams.object_reader.read_bit_long()? as usize
        } else {
            0
        };

        let _vertex_id_count = if self.sio.r2010_plus && (flag & 0x400) != 0 {
            streams.object_reader.read_bit_long()? as usize
        } else {
            0
        };

        let num_widths = if (flag & 0x20) != 0 {
            streams.object_reader.read_bit_long()? as usize
        } else {
            0
        };

        let mut pts = Vec::with_capacity(num_pts);
        for _ in 0..num_pts {
            let v = streams.object_reader.read_2raw_double()?;
            pts.push(v);
        }

        let mut bulges = Vec::with_capacity(num_bulges);
        for _ in 0..num_bulges {
            bulges.push(streams.object_reader.read_bit_double()?);
        }

        // Skip vertex IDs if present.
        for _ in 0.._vertex_id_count {
            let _id = streams.object_reader.read_bit_long()?;
        }

        let mut widths = Vec::with_capacity(num_widths);
        for _ in 0..num_widths {
            let sw = streams.object_reader.read_bit_double()?;
            let ew = streams.object_reader.read_bit_double()?;
            widths.push((sw, ew));
        }

        // Build vertices.
        let mut vertices = Vec::with_capacity(num_pts);
        for i in 0..num_pts {
            let loc = pts[i];
            let bulge = if i < bulges.len() { bulges[i] } else { 0.0 };
            let (sw, ew) = if i < widths.len() {
                widths[i]
            } else {
                (const_width, const_width)
            };
            vertices.push(LwVertex {
                location: loc,
                bulge,
                start_width: sw,
                end_width: ew,
            });
        }

        let lw = LwPolyline {
            common: entity_common,
            vertices,
            is_closed,
            constant_width: const_width,
            elevation,
            thickness,
            normal,
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::LwPolyline(lw),
        })
    }

    pub(super) fn read_hatch(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        // R2004+: is gradient fill flag (BL).
        let is_gradient = if self.sio.r2004_plus {
            streams.object_reader.read_bit_long()? != 0
        } else {
            false
        };

        let mut gradient = hatch::HatchGradientPattern::default();
        if is_gradient {
            gradient.reserved = streams.object_reader.read_bit_long()?;
            gradient.angle = streams.object_reader.read_bit_double()?;
            gradient.shift = streams.object_reader.read_bit_double()?;
            gradient.is_single_color = streams.object_reader.read_bit_long()? != 0;
            gradient.color_tint = streams.object_reader.read_bit_double()?;

            let num_colors = streams.object_reader.read_bit_long()? as usize;
            for _ in 0..num_colors {
                let value = streams.object_reader.read_bit_double()?;
                let color = streams.object_reader.read_cm_color()?;
                gradient.colors.push(hatch::GradientColorEntry { value, color });
            }
            gradient.name = streams.read_text()?;
            gradient.enabled = true;
        }

        let elevation = streams.object_reader.read_bit_double()?;
        let normal = streams.object_reader.read_3bit_double()?;
        let pattern_name = streams.read_text()?;
        let is_solid = streams.object_reader.read_bit()?;
        let is_associative = streams.object_reader.read_bit()?;

        // Boundary paths.
        let num_paths = streams.object_reader.read_bit_long()? as usize;
        let mut paths = Vec::with_capacity(num_paths);
        let mut all_boundary_handles: Vec<Vec<u64>> = Vec::with_capacity(num_paths);

        for _ in 0..num_paths {
            let path_type_flag = streams.object_reader.read_bit_long()? as u32;
            let is_polyline = (path_type_flag & 2) != 0;

            let mut edges = Vec::new();

            if is_polyline {
                let has_bulge = streams.object_reader.read_bit()?;
                let _is_closed = streams.object_reader.read_bit()?;
                let num_verts = streams.object_reader.read_bit_long()? as usize;
                let mut verts = Vec::with_capacity(num_verts);
                for _ in 0..num_verts {
                    let pt = streams.object_reader.read_2raw_double()?;
                    let bulge = if has_bulge {
                        streams.object_reader.read_bit_double()?
                    } else {
                        0.0
                    };
                    verts.push(Vector3::new(pt.x, pt.y, bulge));
                }
                edges.push(hatch::BoundaryEdge::Polyline(hatch::PolylineEdge {
                    vertices: verts,
                    is_closed: true,
                }));
            } else {
                let num_edges = streams.object_reader.read_bit_long()? as usize;
                for _ in 0..num_edges {
                    let etype = streams.object_reader.read_raw_char()?;
                    match etype {
                        1 => {
                            let s = streams.object_reader.read_2raw_double()?;
                            let e = streams.object_reader.read_2raw_double()?;
                            edges.push(hatch::BoundaryEdge::Line(hatch::LineEdge {
                                start: s,
                                end: e,
                            }));
                        }
                        2 => {
                            let c = streams.object_reader.read_2raw_double()?;
                            let r = streams.object_reader.read_bit_double()?;
                            let sa = streams.object_reader.read_bit_double()?;
                            let ea = streams.object_reader.read_bit_double()?;
                            let ccw = streams.object_reader.read_bit()?;
                            edges.push(hatch::BoundaryEdge::CircularArc(
                                hatch::CircularArcEdge {
                                    center: c,
                                    radius: r,
                                    start_angle: sa,
                                    end_angle: ea,
                                    counter_clockwise: ccw,
                                },
                            ));
                        }
                        3 => {
                            let c = streams.object_reader.read_2raw_double()?;
                            let maj = streams.object_reader.read_2raw_double()?;
                            let mr = streams.object_reader.read_bit_double()?;
                            let sa = streams.object_reader.read_bit_double()?;
                            let ea = streams.object_reader.read_bit_double()?;
                            let ccw = streams.object_reader.read_bit()?;
                            edges.push(hatch::BoundaryEdge::EllipticArc(
                                hatch::EllipticArcEdge {
                                    center: c,
                                    major_axis_endpoint: maj,
                                    minor_axis_ratio: mr,
                                    start_angle: sa,
                                    end_angle: ea,
                                    counter_clockwise: ccw,
                                },
                            ));
                        }
                        4 => {
                            let deg = streams.object_reader.read_bit_long()?;
                            let rational = streams.object_reader.read_bit()?;
                            let periodic = streams.object_reader.read_bit()?;
                            let n_knots = streams.object_reader.read_bit_long()? as usize;
                            let n_ctrl = streams.object_reader.read_bit_long()? as usize;
                            let mut knots = Vec::with_capacity(n_knots);
                            for _ in 0..n_knots {
                                knots.push(streams.object_reader.read_bit_double()?);
                            }
                            let mut ctrl = Vec::with_capacity(n_ctrl);
                            for _ in 0..n_ctrl {
                                let p = streams.object_reader.read_2raw_double()?;
                                let w = if rational {
                                    streams.object_reader.read_bit_double()?
                                } else {
                                    1.0
                                };
                                ctrl.push(Vector3::new(p.x, p.y, w));
                            }
                            let n_fit = if self.sio.r2010_plus {
                                streams.object_reader.read_bit_long()? as usize
                            } else {
                                0
                            };
                            let mut fit = Vec::with_capacity(n_fit);
                            for _ in 0..n_fit {
                                fit.push(streams.object_reader.read_2raw_double()?);
                            }
                            let st = if self.sio.r2010_plus && n_fit > 0 {
                                streams.object_reader.read_2raw_double()?
                            } else {
                                Vector2::ZERO
                            };
                            let et = if self.sio.r2010_plus && n_fit > 0 {
                                streams.object_reader.read_2raw_double()?
                            } else {
                                Vector2::ZERO
                            };
                            edges.push(hatch::BoundaryEdge::Spline(hatch::SplineEdge {
                                degree: deg,
                                rational,
                                periodic,
                                knots,
                                control_points: ctrl,
                                fit_points: fit,
                                start_tangent: st,
                                end_tangent: et,
                            }));
                        }
                        _ => {}
                    }
                }
            }

            // Boundary object handles count.
            let num_handles = streams.object_reader.read_bit_long()? as usize;
            let _handles_count = num_handles;
            // Handles are read later from the handles stream.
            all_boundary_handles.push(Vec::new());

            paths.push(hatch::BoundaryPath {
                flags: hatch::BoundaryPathFlags::from_bits(path_type_flag),
                edges,
                boundary_handles: Vec::new(), // resolved later
            });
        }

        // Pattern style/type.
        let style = streams.object_reader.read_bit_short()?;
        let pattern_type = streams.object_reader.read_bit_short()?;

        let mut pattern_angle = 0.0;
        let mut pattern_scale = 1.0;
        let mut is_double = false;
        let mut pattern_lines = Vec::new();

        if !is_solid {
            pattern_angle = streams.object_reader.read_bit_double()?;
            pattern_scale = streams.object_reader.read_bit_double()?;
            is_double = streams.object_reader.read_bit()?;

            let num_def_lines = streams.object_reader.read_bit_short()? as usize;
            for _ in 0..num_def_lines {
                let angle = streams.object_reader.read_bit_double()?;
                let bp = streams.object_reader.read_2raw_double()?;
                let offset = streams.object_reader.read_2raw_double()?;
                let num_dashes = streams.object_reader.read_bit_short()? as usize;
                let mut dashes = Vec::with_capacity(num_dashes);
                for _ in 0..num_dashes {
                    dashes.push(streams.object_reader.read_bit_double()?);
                }
                pattern_lines.push(hatch::HatchPatternLine {
                    angle,
                    base_point: bp,
                    offset,
                    dash_lengths: dashes,
                });
            }
        }

        // Pixel size (BD).
        let pixel_size = if streams.object_reader.read_bit()? {
            streams.object_reader.read_bit_double()?
        } else {
            0.0
        };

        // Seed points.
        let num_seeds = streams.object_reader.read_bit_long()? as usize;
        let mut seed_points = Vec::with_capacity(num_seeds);
        for _ in 0..num_seeds {
            seed_points.push(streams.object_reader.read_2raw_double()?);
        }

        // Read boundary handles from handles reader.
        let mut hatch_tmpl = CadHatchTemplateData::default();
        for _path_idx in 0..num_paths {
            // Boundary path handle count was stored implicitly. Let's read from handles stream.
            // In ACadSharp this is done with boundary object handles for each path.
            let num_handles = streams.object_reader.read_bit_long().unwrap_or(0) as usize;
            let mut handles = Vec::with_capacity(num_handles);
            for _ in 0..num_handles {
                handles.push(streams.handles_reader.handle_reference()?);
            }
            hatch_tmpl.boundary_handles.push(handles);
        }

        let hatch_style = match style {
            0 => hatch::HatchStyleType::Normal,
            1 => hatch::HatchStyleType::Outer,
            2 => hatch::HatchStyleType::Ignore,
            _ => hatch::HatchStyleType::Normal,
        };

        let pat_type = match pattern_type {
            0 => hatch::HatchPatternType::UserDefined,
            1 => hatch::HatchPatternType::Predefined,
            2 => hatch::HatchPatternType::Custom,
            _ => hatch::HatchPatternType::Predefined,
        };

        let hatch = Hatch {
            common: entity_common,
            elevation,
            normal,
            pattern: hatch::HatchPattern {
                name: pattern_name,
                description: String::new(),
                lines: pattern_lines,
            },
            is_solid,
            is_associative,
            pattern_type: pat_type,
            pattern_angle,
            pattern_scale,
            is_double,
            style: hatch_style,
            paths,
            seed_points,
            pixel_size,
            gradient_color: gradient,
        };

        Ok(CadTemplate::Hatch {
            common: common_tmpl,
            entity_data: ent_tmpl,
            hatch_data: hatch_tmpl,
            entity: EntityType::Hatch(hatch),
        })
    }

    pub(super) fn read_ole2frame(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let version = streams.object_reader.read_bit_short()?;

        // Data length.
        let length = streams.object_reader.read_bit_long()? as usize;
        let data = streams.object_reader.read_bytes(length)?;

        let ole = Ole2Frame {
            common: entity_common,
            version,
            source_application: String::new(),
            upper_left_corner: Vector3::ZERO,
            lower_right_corner: Vector3::ZERO,
            ole_object_type: ole2frame::OleObjectType::Embedded,
            is_paper_space: false,
            binary_data: data,
        };

        Ok(CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: EntityType::Ole2Frame(ole),
        })
    }

    // -----------------------------------------------------------------------
    // Multileader (stub — reads common data, skips body)
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub(super) fn read_multileader(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        // MultiLeader is extremely complex; read common entity data
        // and store as a partially-populated template.
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        // For now, create a minimal MultiLeader entity.
        // Full reading requires ~400 lines mirroring DwgObjectReader.readMultiLeader().
        let ml = MultiLeader::default();
        let mut ml = ml;
        ml.common = entity_common;

        Ok(CadTemplate::MultiLeader {
            common: common_tmpl,
            entity_data: ent_tmpl,
            mleader_data: CadMultiLeaderTemplateData::default(),
            entity: EntityType::MultiLeader(ml),
        })
    }
}
