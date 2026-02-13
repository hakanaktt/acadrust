//! Non-graphical object writers for the DWG object writer.
//!
//! Mirrors ACadSharp's `DwgObjectWriter.Objects.cs`.

use crate::error::Result;
use crate::io::dwg::object_type::DwgObjectType;
use crate::io::dwg::reference_type::DwgReferenceType;
use crate::io::dwg::writer::stream_writer::IDwgStreamWriter;
use crate::objects::{
    BookColor, Dictionary, DictionaryVariable, DictionaryWithDefault, Group,
    ImageDefinition, ImageDefinitionReactor, Layout, MLineStyle, MultiLeaderStyle,
    PlaceHolder, PlotSettings, RasterVariables, Scale, SortEntitiesTable,
    WipeoutVariables, XRecord,
};
use crate::types::Handle;

use super::DwgObjectWriter;

impl DwgObjectWriter {
    // -----------------------------------------------------------------------
    // DICTIONARY (0x2A) — listed type
    // -----------------------------------------------------------------------

    pub(super) fn write_dictionary(
        &mut self,
        dict: &Dictionary,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = dict.handle.value();
        let (mut writer, _) = self.create_object_writer();

        // Collect reactor handles as u64 slice
        let reactor_handles: Vec<Handle> = dict.reactors.clone();

        self.write_common_non_entity_data(
            &mut *writer,
            DwgObjectType::Dictionary,
            handle,
            owner_handle,
            &reactor_handles,
            dict.xdictionary_handle,
        )?;

        // BL: number of entries
        writer.write_bit_long(dict.entries.len() as i32)?;

        // R14 only: unknown byte
        if self.sio.r13_14_only {
            writer.write_byte(0)?;
        }

        // R2000+: cloning flags (BS) + hard owner flag (RC)
        if self.sio.r2000_plus {
            writer.write_bit_short(dict.duplicate_cloning)?;
            writer.write_byte(if dict.hard_owner { 1 } else { 0 })?;
        }

        // Entry names + handles
        for (name, entry_handle) in &dict.entries {
            writer.write_variable_text(name)?;
            writer.handle_reference_typed(
                if dict.hard_owner {
                    DwgReferenceType::HardOwnership
                } else {
                    DwgReferenceType::SoftOwnership
                },
                entry_handle.value(),
            )?;
        }

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // DICTIONARY WITH DEFAULT — unlisted type ("ACDBDICTIONARYWDFLT")
    // -----------------------------------------------------------------------

    pub(super) fn write_dictionary_with_default(
        &mut self,
        dict: &DictionaryWithDefault,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = dict.handle.value();
        let (mut writer, _) = self.create_object_writer();

        self.write_common_non_entity_data_unlisted(
            &mut *writer,
            "ACDBDICTIONARYWDFLT",
            handle,
            owner_handle,
            &[], // reactors
            None, // xdictionary
        )?;

        // Same dictionary body as regular dictionary
        writer.write_bit_long(dict.entries.len() as i32)?;

        if self.sio.r13_14_only {
            writer.write_byte(0)?;
        }

        if self.sio.r2000_plus {
            writer.write_bit_short(dict.duplicate_cloning)?;
            writer.write_byte(if dict.hard_owner { 1 } else { 0 })?;
        }

        for (name, entry_handle) in &dict.entries {
            writer.write_variable_text(name)?;
            writer.handle_reference_typed(
                if dict.hard_owner {
                    DwgReferenceType::HardOwnership
                } else {
                    DwgReferenceType::SoftOwnership
                },
                entry_handle.value(),
            )?;
        }

        // H 7: default entry handle (hard pointer)
        writer.handle_reference_typed(
            DwgReferenceType::HardPointer,
            dict.default_handle.value(),
        )?;

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // DICTIONARYVAR — unlisted type ("DICTIONARYVAR")
    // -----------------------------------------------------------------------

    pub(super) fn write_dictionary_variable(
        &mut self,
        dv: &DictionaryVariable,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = dv.handle.value();
        let (mut writer, _) = self.create_object_writer();

        self.write_common_non_entity_data_unlisted(
            &mut *writer,
            "DICTIONARYVAR",
            handle,
            owner_handle,
            &[], // reactors
            None, // xdictionary
        )?;

        // RC: integer value (schema_number — always 0)
        writer.write_byte(dv.schema_number as u8)?;

        // TV: string value
        writer.write_variable_text(&dv.value)?;

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // XRECORD (0x4F) — listed type
    // -----------------------------------------------------------------------

    pub(super) fn write_xrecord(
        &mut self,
        xrecord: &XRecord,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = xrecord.handle.value();
        let (mut writer, _) = self.create_object_writer();

        self.write_common_non_entity_data(
            &mut *writer,
            DwgObjectType::XRecord,
            handle,
            owner_handle,
            &[], // reactors
            None, // xdictionary
        )?;

        // Serialize entries to a raw byte buffer (little-endian).
        let data_bytes = self.serialize_xrecord_data(&xrecord);

        // BL: number of data bytes
        writer.write_bit_long(data_bytes.len() as i32)?;

        // Raw data bytes
        if !data_bytes.is_empty() {
            writer.write_bytes(&data_bytes)?;
        }

        // R2000+: BS cloning flags
        if self.sio.r2000_plus {
            writer.write_bit_short(xrecord.cloning_flags.to_value())?;
        }

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    /// Serialize XRecord entries to a raw little-endian byte buffer.
    fn serialize_xrecord_data(&self, xrecord: &XRecord) -> Vec<u8> {
        use crate::objects::XRecordValue;
        let mut buf = Vec::new();

        for entry in &xrecord.entries {
            // RS: group code (little-endian)
            let code = entry.code as i16;
            buf.extend_from_slice(&code.to_le_bytes());

            match &entry.value {
                XRecordValue::String(s) => {
                    // Write as length-prefixed UTF-8 (codepage string in DWG)
                    Self::write_string_to_buffer(&mut buf, s);
                }
                XRecordValue::Point3D(x, y, z) => {
                    buf.extend_from_slice(&x.to_le_bytes());
                    buf.extend_from_slice(&y.to_le_bytes());
                    buf.extend_from_slice(&z.to_le_bytes());
                }
                XRecordValue::Double(v) => {
                    buf.extend_from_slice(&v.to_le_bytes());
                }
                XRecordValue::Byte(v) => {
                    buf.push(*v);
                }
                XRecordValue::Bool(v) => {
                    buf.push(if *v { 1 } else { 0 });
                }
                XRecordValue::Int16(v) => {
                    buf.extend_from_slice(&v.to_le_bytes());
                }
                XRecordValue::Int32(v) => {
                    buf.extend_from_slice(&v.to_le_bytes());
                }
                XRecordValue::Int64(v) => {
                    buf.extend_from_slice(&v.to_le_bytes());
                }
                XRecordValue::Handle(h) => {
                    // Write handle as 8-byte value
                    let val = h.value();
                    buf.extend_from_slice(&val.to_le_bytes());
                }
                XRecordValue::Chunk(data) => {
                    // Write byte count + raw bytes
                    buf.push(data.len() as u8);
                    buf.extend_from_slice(data);
                }
            }
        }

        buf
    }

    /// Write a string into a raw buffer (length-prefixed, codepage).
    fn write_string_to_buffer(buf: &mut Vec<u8>, s: &str) {
        let bytes = s.as_bytes();
        let len = bytes.len() as u16;
        buf.extend_from_slice(&len.to_le_bytes());
        buf.extend_from_slice(bytes);
    }

    // -----------------------------------------------------------------------
    // PLOTSETTINGS — unlisted type ("PLOTSETTINGS")
    // -----------------------------------------------------------------------

    pub(super) fn write_plot_settings_obj(
        &mut self,
        ps: &PlotSettings,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = ps.handle.value();
        let (mut writer, _) = self.create_object_writer();

        self.write_common_non_entity_data_unlisted(
            &mut *writer,
            "PLOTSETTINGS",
            handle,
            owner_handle,
            &[], // reactors
            None, // xdictionary
        )?;

        self.write_plot_settings_data(&mut *writer, ps)?;

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    /// Write plot settings data (shared by PlotSettings and Layout).
    fn write_plot_settings_data(
        &self,
        writer: &mut dyn IDwgStreamWriter,
        ps: &PlotSettings,
    ) -> Result<()> {
        // TV: page name
        writer.write_variable_text(&ps.page_name)?;

        // TV: system printer name
        writer.write_variable_text(&ps.printer_name)?;

        // BS: flags
        writer.write_bit_short(ps.flags.to_bits() as i16)?;

        // BD: margins (left, bottom, right, top)
        writer.write_bit_double(ps.margins.left)?;
        writer.write_bit_double(ps.margins.bottom)?;
        writer.write_bit_double(ps.margins.right)?;
        writer.write_bit_double(ps.margins.top)?;

        // BD: paper width, height
        writer.write_bit_double(ps.paper_width)?;
        writer.write_bit_double(ps.paper_height)?;

        // TV: paper size
        writer.write_variable_text(&ps.paper_size)?;

        // BD: plot origin x, y
        writer.write_bit_double(ps.origin_x)?;
        writer.write_bit_double(ps.origin_y)?;

        // BS: paper units
        writer.write_bit_short(ps.paper_units.to_code())?;

        // BS: paper rotation
        writer.write_bit_short(ps.rotation.to_code())?;

        // BS: plot type
        writer.write_bit_short(ps.plot_type.to_code())?;

        // BD: window lower left x, y; upper right x, y
        writer.write_bit_double(ps.plot_window.lower_left_x)?;
        writer.write_bit_double(ps.plot_window.lower_left_y)?;
        writer.write_bit_double(ps.plot_window.upper_right_x)?;
        writer.write_bit_double(ps.plot_window.upper_right_y)?;

        // R13-R2000: plot view name (TV)
        if !self.sio.r2004_plus {
            writer.write_variable_text(&ps.plot_view_name)?;
        }

        // BD: numerator scale (real world units)
        writer.write_bit_double(ps.scale_numerator)?;

        // BD: denominator scale (drawing units)
        writer.write_bit_double(ps.scale_denominator)?;

        // TV: stylesheet
        writer.write_variable_text(&ps.current_style_sheet)?;

        // BS: scaled fit
        writer.write_bit_short(ps.scale_type.to_code())?;

        // BD: standard scale factor
        writer.write_bit_double(ps.scale_type.scale_factor())?;

        // 2BD: paper image origin (always 0,0)
        writer.write_bit_double(0.0)?;
        writer.write_bit_double(0.0)?;

        // R2004+: shade plot mode, resolution, DPI
        if self.sio.r2004_plus {
            writer.write_bit_short(ps.shade_plot_mode.to_code())?;
            writer.write_bit_short(ps.shade_plot_resolution.to_code())?;
            writer.write_bit_short(ps.shade_plot_dpi)?;

            // H: plot view handle (hard pointer) — 0 (none)
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?;
        }

        // R2007+: visual style handle (soft pointer) — 0 (none)
        if self.sio.r2007_plus {
            writer.handle_reference_typed(DwgReferenceType::SoftPointer, 0)?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // LAYOUT (0x52) — listed type
    // -----------------------------------------------------------------------

    pub(super) fn write_layout(
        &mut self,
        layout: &Layout,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = layout.handle.value();
        let (mut writer, _) = self.create_object_writer();

        let reactor_handles: Vec<Handle> = layout.reactors.clone();

        self.write_common_non_entity_data(
            &mut *writer,
            DwgObjectType::Layout,
            handle,
            owner_handle,
            &reactor_handles,
            layout.xdictionary_handle,
        )?;

        // Write plot settings portion (Layout inherits from PlotSettings)
        self.write_plot_settings_data(&mut *writer, &layout.plot_settings)?;

        // TV: layout name
        writer.write_variable_text(&layout.name)?;

        // BL: tab order
        writer.write_bit_long(layout.tab_order)?;

        // BS: layout flags
        writer.write_bit_short(layout.flags)?;

        // 3BD: UCS origin
        writer.write_bit_double(layout.ucs_origin.0)?;
        writer.write_bit_double(layout.ucs_origin.1)?;
        writer.write_bit_double(layout.ucs_origin.2)?;

        // 2RD: min limits
        writer.write_raw_double(layout.min_limits.0)?;
        writer.write_raw_double(layout.min_limits.1)?;

        // 2RD: max limits
        writer.write_raw_double(layout.max_limits.0)?;
        writer.write_raw_double(layout.max_limits.1)?;

        // 3BD: insertion base point
        writer.write_bit_double(layout.insertion_base.0)?;
        writer.write_bit_double(layout.insertion_base.1)?;
        writer.write_bit_double(layout.insertion_base.2)?;

        // 3BD: UCS X axis
        writer.write_bit_double(layout.ucs_x_axis.0)?;
        writer.write_bit_double(layout.ucs_x_axis.1)?;
        writer.write_bit_double(layout.ucs_x_axis.2)?;

        // 3BD: UCS Y axis
        writer.write_bit_double(layout.ucs_y_axis.0)?;
        writer.write_bit_double(layout.ucs_y_axis.1)?;
        writer.write_bit_double(layout.ucs_y_axis.2)?;

        // BD: elevation
        writer.write_bit_double(layout.elevation)?;

        // BS: UCS orthographic type
        writer.write_bit_short(layout.ucs_ortho_type)?;

        // 3BD: min extents
        writer.write_bit_double(layout.min_extents.0)?;
        writer.write_bit_double(layout.min_extents.1)?;
        writer.write_bit_double(layout.min_extents.2)?;

        // 3BD: max extents
        writer.write_bit_double(layout.max_extents.0)?;
        writer.write_bit_double(layout.max_extents.1)?;
        writer.write_bit_double(layout.max_extents.2)?;

        // R2004+: number of viewports (BL)
        if self.sio.r2004_plus {
            writer.write_bit_long(layout.viewport_handles.len() as i32)?;
        }

        // Handles
        // H: paper space block record handle (soft pointer)
        writer.handle_reference_typed(
            DwgReferenceType::SoftPointer,
            layout.block_record.value(),
        )?;

        // H: active viewport handle (soft pointer)
        writer.handle_reference_typed(
            DwgReferenceType::SoftPointer,
            layout.viewport.value(),
        )?;

        // H: base UCS handle (hard pointer)
        writer.handle_reference_typed(
            DwgReferenceType::HardPointer,
            layout.base_ucs.value(),
        )?;

        // H: named UCS handle (hard pointer)
        writer.handle_reference_typed(
            DwgReferenceType::HardPointer,
            layout.named_ucs.value(),
        )?;

        // R2004+: viewport handles (soft pointers)
        if self.sio.r2004_plus {
            for vh in &layout.viewport_handles {
                writer.handle_reference_typed(
                    DwgReferenceType::SoftPointer,
                    vh.value(),
                )?;
            }
        }

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // GROUP (0x48) — listed type
    // -----------------------------------------------------------------------

    pub(super) fn write_group(
        &mut self,
        group: &Group,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = group.handle.value();
        let (mut writer, _) = self.create_object_writer();

        self.write_common_non_entity_data(
            &mut *writer,
            DwgObjectType::Group,
            handle,
            owner_handle,
            &[], // reactors
            None, // xdictionary
        )?;

        // TV: description
        writer.write_variable_text(&group.description)?;

        // BS: unnamed flag (1 if unnamed)
        writer.write_bit_short(if group.is_unnamed() { 1 } else { 0 })?;

        // BS: selectable
        writer.write_bit_short(if group.selectable { 1 } else { 0 })?;

        // BL: number of entity handles
        writer.write_bit_long(group.entities.len() as i32)?;

        // H: hard pointer per entity
        for entity_handle in &group.entities {
            writer.handle_reference_typed(
                DwgReferenceType::HardPointer,
                entity_handle.value(),
            )?;
        }

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // MLINESTYLE (0x49) — listed type
    // -----------------------------------------------------------------------

    pub(super) fn write_mline_style(
        &mut self,
        style: &MLineStyle,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = style.handle.value();
        let (mut writer, _) = self.create_object_writer();

        self.write_common_non_entity_data(
            &mut *writer,
            DwgObjectType::MlineStyle,
            handle,
            owner_handle,
            &[], // reactors
            None, // xdictionary
        )?;

        // TV: name
        writer.write_variable_text(&style.name)?;

        // TV: description
        writer.write_variable_text(&style.description)?;

        // BS: flags (DWG uses same bit layout as DXF via to_bits())
        writer.write_bit_short(style.flags.to_bits() as i16)?;

        // CMC: fill color
        writer.write_cm_color(style.fill_color)?;

        // BD: start angle
        writer.write_bit_double(style.start_angle)?;

        // BD: end angle
        writer.write_bit_double(style.end_angle)?;

        // RC: number of elements (lines)
        writer.write_byte(style.elements.len() as u8)?;

        // Per element
        for element in &style.elements {
            // BD: offset
            writer.write_bit_double(element.offset)?;

            // CMC: color
            writer.write_cm_color(element.color)?;

            // R2018+: linetype handle (hard pointer)
            if self.sio.r2018_plus {
                let ltype_handle = self.resolve_linetype_handle(&element.linetype);
                writer.handle_reference_typed(
                    DwgReferenceType::HardPointer,
                    ltype_handle,
                )?;
            } else {
                // Pre-R2018: BS linetype index (0 = BYLAYER)
                writer.write_bit_short(0)?;
            }
        }

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // IMAGE DEFINITION — unlisted type ("IMAGEDEF")
    // -----------------------------------------------------------------------

    pub(super) fn write_image_definition(
        &mut self,
        imgdef: &ImageDefinition,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = imgdef.handle.value();
        let (mut writer, _) = self.create_object_writer();

        self.write_common_non_entity_data_unlisted(
            &mut *writer,
            "IMAGEDEF",
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // BL: class version
        writer.write_bit_long(imgdef.class_version)?;

        // 2RD: image size in pixels
        writer.write_raw_double(imgdef.size_in_pixels.0 as f64)?;
        writer.write_raw_double(imgdef.size_in_pixels.1 as f64)?;

        // TV: file path
        writer.write_variable_text(&imgdef.file_name)?;

        // B: is loaded
        writer.write_bit(imgdef.is_loaded)?;

        // RC: resolution units
        writer.write_byte(imgdef.resolution_unit.to_code() as u8)?;

        // 2RD: pixel size (default size of one pixel in ACAD units)
        writer.write_raw_double(imgdef.pixel_size.0)?;
        writer.write_raw_double(imgdef.pixel_size.1)?;

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // IMAGE DEFINITION REACTOR — unlisted type ("IMAGEDEF_REACTOR")
    // -----------------------------------------------------------------------

    pub(super) fn write_image_definition_reactor(
        &mut self,
        reactor: &ImageDefinitionReactor,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = reactor.handle.value();
        let (mut writer, _) = self.create_object_writer();

        self.write_common_non_entity_data_unlisted(
            &mut *writer,
            "IMAGEDEF_REACTOR",
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // BL: class version (always 2)
        writer.write_bit_long(2)?;

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // MULTILEADERSTYLE — unlisted type ("MLEADERSTYLE")
    // -----------------------------------------------------------------------

    pub(super) fn write_mleader_style(
        &mut self,
        style: &MultiLeaderStyle,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = style.handle.value();
        let (mut writer, _) = self.create_object_writer();

        self.write_common_non_entity_data_unlisted(
            &mut *writer,
            "MLEADERSTYLE",
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // R2010+: BS version = 2
        if self.sio.r2010_plus {
            writer.write_bit_short(2)?;
        }

        // BS: content type
        writer.write_bit_short(style.content_type as i16)?;

        // BS: multileader draw order
        writer.write_bit_short(style.multileader_draw_order as i16)?;

        // BS: leader draw order
        writer.write_bit_short(style.leader_draw_order as i16)?;

        // BL: max leader segment points
        writer.write_bit_long(style.max_leader_points)?;

        // BD: first segment angle
        writer.write_bit_double(style.first_segment_angle)?;

        // BD: second segment angle
        writer.write_bit_double(style.second_segment_angle)?;

        // BS: path type
        writer.write_bit_short(style.path_type as i16)?;

        // CMC: line color
        writer.write_cm_color(style.line_color)?;

        // H: leader line type handle (hard pointer)
        writer.handle_reference_typed(
            DwgReferenceType::HardPointer,
            style.line_type_handle.map_or(0, |h| h.value()),
        )?;

        // BL: line weight
        writer.write_bit_long(style.line_weight.value() as i32)?;

        // B: enable landing
        writer.write_bit(style.enable_landing)?;

        // BD: landing gap
        writer.write_bit_double(style.landing_gap)?;

        // B: enable dogleg
        writer.write_bit(style.enable_dogleg)?;

        // BD: landing distance
        writer.write_bit_double(style.landing_distance)?;

        // TV: description
        writer.write_variable_text(&style.description)?;

        // H: arrowhead block handle (hard pointer)
        writer.handle_reference_typed(
            DwgReferenceType::HardPointer,
            style.arrowhead_handle.map_or(0, |h| h.value()),
        )?;

        // BD: arrowhead size
        writer.write_bit_double(style.arrowhead_size)?;

        // TV: default text contents
        writer.write_variable_text(&style.default_text)?;

        // H: text style handle (hard pointer)
        writer.handle_reference_typed(
            DwgReferenceType::HardPointer,
            style.text_style_handle.map_or(0, |h| h.value()),
        )?;

        // BS: text left attachment
        writer.write_bit_short(style.text_left_attachment as i16)?;

        // BS: text right attachment
        writer.write_bit_short(style.text_right_attachment as i16)?;

        // BS: text angle type
        writer.write_bit_short(style.text_angle_type as i16)?;

        // BS: text alignment
        writer.write_bit_short(style.text_alignment as i16)?;

        // CMC: text color
        writer.write_cm_color(style.text_color)?;

        // BD: text height
        writer.write_bit_double(style.text_height)?;

        // B: text frame enabled
        writer.write_bit(style.text_frame)?;

        // B: always align text left
        writer.write_bit(style.text_always_left)?;

        // BD: align space
        writer.write_bit_double(style.align_space)?;

        // H: block content handle (hard pointer)
        writer.handle_reference_typed(
            DwgReferenceType::HardPointer,
            style.block_content_handle.map_or(0, |h| h.value()),
        )?;

        // CMC: block content color
        writer.write_cm_color(style.block_content_color)?;

        // 3BD: block content scale (x, y, z)
        writer.write_bit_double(style.block_content_scale_x)?;
        writer.write_bit_double(style.block_content_scale_y)?;
        writer.write_bit_double(style.block_content_scale_z)?;

        // B: enable block scale
        writer.write_bit(style.enable_block_scale)?;

        // BD: block content rotation
        writer.write_bit_double(style.block_content_rotation)?;

        // B: enable block rotation
        writer.write_bit(style.enable_block_rotation)?;

        // BS: block content connection type
        writer.write_bit_short(style.block_content_connection as i16)?;

        // BD: scale factor
        writer.write_bit_double(style.scale_factor)?;

        // B: property changed (overwrite property value)
        writer.write_bit(style.property_changed)?;

        // B: is annotative
        writer.write_bit(style.is_annotative)?;

        // BD: break gap size
        writer.write_bit_double(style.break_gap_size)?;

        // R2010+: attachment direction, text bottom/top attachment
        if self.sio.r2010_plus {
            // BS: attachment direction
            writer.write_bit_short(style.text_attachment_direction as i16)?;

            // BS: text bottom attachment
            writer.write_bit_short(style.text_bottom_attachment as i16)?;

            // BS: text top attachment
            writer.write_bit_short(style.text_top_attachment as i16)?;
        }

        // R2013+: unknown flag (write false)
        if self.sio.r2013_plus {
            writer.write_bit(false)?;
        }

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // SCALE — unlisted type ("SCALE")
    // -----------------------------------------------------------------------

    pub(super) fn write_scale(
        &mut self,
        scale: &Scale,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = scale.handle.value();
        let (mut writer, _) = self.create_object_writer();

        self.write_common_non_entity_data_unlisted(
            &mut *writer,
            "SCALE",
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // BS: unknown (ODA writes 0)
        writer.write_bit_short(0)?;

        // TV: name
        writer.write_variable_text(&scale.name)?;

        // BD: paper units
        writer.write_bit_double(scale.paper_units)?;

        // BD: drawing units
        writer.write_bit_double(scale.drawing_units)?;

        // B: is unit scale
        writer.write_bit(scale.is_unit_scale)?;

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // SORT ENTITIES TABLE — unlisted type ("SORTENTSTABLE")
    // -----------------------------------------------------------------------

    pub(super) fn write_sort_entities_table(
        &mut self,
        table: &SortEntitiesTable,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = table.handle.value();
        let (mut writer, _) = self.create_object_writer();

        self.write_common_non_entity_data_unlisted(
            &mut *writer,
            "SORTENTSTABLE",
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // H: block owner handle (soft pointer) — written FIRST
        writer.handle_reference_typed(
            DwgReferenceType::SoftPointer,
            table.block_owner_handle.value(),
        )?;

        // BL: number of entries
        let entries: Vec<_> = table.entries().collect();
        writer.write_bit_long(entries.len() as i32)?;

        // For each entry:
        for entry in &entries {
            // Sort handle: written as raw handle in main bit stream (code 0)
            // This is unique — uses handle_reference (default/undefined type)
            // which writes to main stream, not handle stream
            writer.handle_reference(entry.sort_handle.value())?;

            // Entity handle: soft pointer (written to handle stream)
            writer.handle_reference_typed(
                DwgReferenceType::SoftPointer,
                entry.entity_handle.value(),
            )?;
        }

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // RASTER VARIABLES — unlisted type ("RASTERVARIABLES")
    // -----------------------------------------------------------------------

    pub(super) fn write_raster_variables(
        &mut self,
        rv: &RasterVariables,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = rv.handle.value();
        let (mut writer, _) = self.create_object_writer();

        self.write_common_non_entity_data_unlisted(
            &mut *writer,
            "RASTERVARIABLES",
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // BL: class version
        writer.write_bit_long(rv.class_version)?;

        // BS: display frame
        writer.write_bit_short(rv.display_image_frame)?;

        // BS: image quality
        writer.write_bit_short(rv.image_quality)?;

        // BS: units
        writer.write_bit_short(rv.units)?;

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // BOOK COLOR (DBCOLOR) — unlisted type ("DBCOLOR")
    // -----------------------------------------------------------------------

    pub(super) fn write_book_color(
        &mut self,
        bc: &BookColor,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = bc.handle.value();
        let (mut writer, _) = self.create_object_writer();

        self.write_common_non_entity_data_unlisted(
            &mut *writer,
            "DBCOLOR",
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // BS: color index (always 0 per C# reference)
        writer.write_bit_short(0)?;

        // R2004+: true color + flags + color_name / book_name
        if self.sio.r2004_plus {
            // BL: true color (BGRA packed as u32) — write 0 (no true color)
            writer.write_bit_long(0)?;

            // RC: flags byte — bit 0 = has color_name, bit 1 = has book_name
            let has_color = !bc.color_name.is_empty();
            let has_book = !bc.book_name.is_empty();
            let flags: u8 = (if has_color { 1 } else { 0 })
                | (if has_book { 2 } else { 0 });
            writer.write_byte(flags)?;

            if has_color {
                writer.write_variable_text(&bc.color_name)?;
            }
            if has_book {
                writer.write_variable_text(&bc.book_name)?;
            }
        }

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // PLACEHOLDER (0x50) — listed type
    // -----------------------------------------------------------------------

    pub(super) fn write_placeholder(
        &mut self,
        ph: &PlaceHolder,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = ph.handle.value();
        let (mut writer, _) = self.create_object_writer();

        self.write_common_non_entity_data(
            &mut *writer,
            DwgObjectType::AcDbPlaceholder,
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // Empty body — no data beyond common non-entity data

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // WIPEOUT VARIABLES — unlisted type ("WIPEOUTVARIABLES")
    // -----------------------------------------------------------------------

    pub(super) fn write_wipeout_variables(
        &mut self,
        wv: &WipeoutVariables,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = wv.handle.value();
        let (mut writer, _) = self.create_object_writer();

        self.write_common_non_entity_data_unlisted(
            &mut *writer,
            "WIPEOUTVARIABLES",
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // BS: display frame
        writer.write_bit_short(wv.display_frame)?;

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }
}
