//! DWG Header section writer.
//!
//! Writes all system variables to the `AcDb:Header` section in a DWG file.
//! The header section contains drawing settings, written in a specific
//! order that varies by DWG version. This is the exact mirror of the
//! header reader.
//!
//! Mirrors ACadSharp's `DwgHeaderWriter`.

use crate::document::HeaderVariables;
use crate::error::Result;
use crate::io::dwg::constants::sentinels;
use crate::io::dwg::header_handles::DwgHeaderHandlesCollection;
use crate::io::dwg::reference_type::DwgReferenceType;
use crate::io::dwg::section_io::SectionIO;
use crate::io::dwg::writer::stream_writer::IDwgStreamWriter;
use crate::io::dwg::writer::stream_writer_base::DwgStreamWriterBase;
use crate::io::dwg::writer::merged_writer::DwgMergedStreamWriter;
use crate::types::DxfVersion;

use byteorder::{LittleEndian, WriteBytesExt};

/// Writer for the DWG `AcDb:Header` section.
pub struct DwgHeaderWriter {
    version: DxfVersion,
}

impl DwgHeaderWriter {
    /// Create a new header writer.
    pub fn new(version: DxfVersion) -> Self {
        Self { version }
    }

    /// Write the header section, returning the raw section bytes.
    pub fn write(
        &self,
        header: &HeaderVariables,
        handles: &DwgHeaderHandlesCollection,
        maintenance_version: u8,
    ) -> Result<Vec<u8>> {
        let sio = SectionIO::new(self.version);
        let main_data: Vec<u8>;

        if sio.r2007_plus {
            let mut writer = DwgMergedStreamWriter::new(self.version);
            writer.save_position_for_size()?;

            Self::write_vars_before_handseed(&sio, &mut writer, header)?;
            // H: HANDSEED — written to the main stream, not the handle stream
            writer.main_writer_mut().handle_reference(header.handle_seed)?;
            Self::write_vars_after_handseed(&sio, &mut writer, header, handles)?;

            writer.write_spear_shift()?;
            main_data = writer.main_writer().data().to_vec();
            // For R2007+, the merged writer produces main+text+handles combined
            // The write_spear_shift merges them.
        } else {
            let mut writer = DwgStreamWriterBase::new(self.version);

            Self::write_vars_before_handseed(&sio, &mut writer, header)?;
            // H: HANDSEED
            writer.handle_reference(header.handle_seed)?;
            Self::write_vars_after_handseed(&sio, &mut writer, header, handles)?;

            writer.write_spear_shift()?;
            main_data = writer.into_data();
        }

        // Wrap with sentinels + CRC
        self.write_size_and_crc(&main_data, maintenance_version)
    }

    /// Build the final section bytes: start sentinel + size + CRC(data) + CRC value + end sentinel.
    fn write_size_and_crc(
        &self,
        main_data: &[u8],
        maintenance_version: u8,
    ) -> Result<Vec<u8>> {
        let sio = SectionIO::new(self.version);
        let mut output = Vec::new();

        // Start sentinel
        output.extend_from_slice(&sentinels::HEADER_START);

        // Build CRC-covered region: size + main_data
        let mut crc_region = Vec::new();
        // RL: Size of the section
        crc_region.write_i32::<LittleEndian>(main_data.len() as i32)?;

        // R2010/R2013 (only present if maintenance version > 3) or R2018+:
        if (sio.r2010_plus && maintenance_version > 3) || sio.r2018_plus {
            crc_region.write_i32::<LittleEndian>(0)?;
        }

        crc_region.extend_from_slice(main_data);

        // CRC-8 over the region
        let crc = crate::io::dwg::crc::crc8(0xC0C1, &crc_region);
        output.extend_from_slice(&crc_region);

        // RS: CRC
        output.write_u16::<LittleEndian>(crc)?;

        // End sentinel
        output.extend_from_slice(&sentinels::HEADER_END);

        Ok(output)
    }

    /// Convert a Unix timestamp (f64) back to (julian_day, milliseconds).
    fn timestamp_to_julian(timestamp: f64) -> (i32, i32) {
        let unix_days = timestamp / 86400.0;
        let jdate = (unix_days + 2440587.5) as i32;
        let remainder_secs = timestamp - (jdate as f64 - 2440587.5) * 86400.0;
        let ms = (remainder_secs * 1000.0) as i32;
        (jdate, ms)
    }

    /// Convert a time span (f64 seconds) back to (days, milliseconds).
    fn timespan_to_parts(seconds: f64) -> (i32, i32) {
        let hours = (seconds / 3600.0) as i32;
        let remainder_ms = ((seconds - hours as f64 * 3600.0) * 1000.0) as i32;
        (hours, remainder_ms)
    }

    /// Write all header variables before HANDSEED.
    fn write_vars_before_handseed(
        sio: &SectionIO,
        writer: &mut dyn IDwgStreamWriter,
        header: &HeaderVariables,
    ) -> Result<()> {
        // R2013+:
        if sio.r2013_plus {
            // BLL: REQUIREDVERSIONS
            writer.write_bit_long_long(header.required_versions)?;
        }

        // Common: Unknown defaults
        writer.write_bit_double(412148564080.0)?;
        writer.write_bit_double(1.0)?;
        writer.write_bit_double(1.0)?;
        writer.write_bit_double(1.0)?;
        writer.write_variable_text("m")?;
        writer.write_variable_text("")?;
        writer.write_variable_text("")?;
        writer.write_variable_text("")?;
        writer.write_bit_long(24)?;
        writer.write_bit_long(0)?;

        // R13-R14 Only:
        if sio.r13_14_only {
            writer.write_bit_short(0)?;
        }

        // Pre-2004 Only:
        if sio.r2004_pre {
            // H: viewport entity header (hard pointer)
            writer.handle_reference(0)?;
        }

        // Common:
        writer.write_bit(header.associate_dimensions)?;
        writer.write_bit(header.update_dimensions_while_dragging)?;

        // R13-R14 Only:
        if sio.r13_14_only {
            writer.write_bit(header.dim_sav)?;
        }

        // Common:
        writer.write_bit(header.polyline_linetype_generation)?;
        writer.write_bit(header.ortho_mode)?;
        writer.write_bit(header.regen_mode)?;
        writer.write_bit(header.fill_mode)?;
        writer.write_bit(header.quick_text_mode)?;
        writer.write_bit(header.paper_space_linetype_scaling)?;
        writer.write_bit(header.limit_check)?;

        // R13-R14 Only:
        if sio.r13_14_only {
            writer.write_bit(header.blip_mode)?;
        }

        // R2004+:
        if sio.r2004_plus {
            writer.write_bit(false)?; // Undocumented
        }

        // Common:
        writer.write_bit(header.user_timer)?;
        writer.write_bit(header.sketch_polylines)?;
        writer.write_bit(header.angle_direction != 0)?;
        writer.write_bit(header.spline_frame)?;

        // R13-R14 Only:
        if sio.r13_14_only {
            writer.write_bit(header.attribute_request)?;
            writer.write_bit(header.attribute_dialog)?;
        }

        // Common:
        writer.write_bit(header.mirror_text)?;
        writer.write_bit(header.world_view)?;

        // R13-R14 Only:
        if sio.r13_14_only {
            writer.write_bit(false)?; // WIREFRAME
        }

        // Common:
        writer.write_bit(header.show_model_space)?;
        writer.write_bit(header.paper_space_limit_check)?;
        writer.write_bit(header.retain_xref_visibility)?;

        // R13-R14 Only:
        if sio.r13_14_only {
            writer.write_bit(header.delete_objects)?;
        }

        // Common:
        writer.write_bit(header.display_silhouette)?;
        writer.write_bit(header.create_ellipse_as_polyline)?;
        writer.write_bit_short(if header.proxy_graphics != 0 { 1 } else { 0 })?;

        // R13-R14 Only:
        if sio.r13_14_only {
            writer.write_bit_short(header.drag_mode)?;
        }

        // Common:
        writer.write_bit_short(header.tree_depth)?;
        writer.write_bit_short(header.linear_unit_format)?;
        writer.write_bit_short(header.linear_unit_precision)?;
        writer.write_bit_short(header.angular_unit_format)?;
        writer.write_bit_short(header.angular_unit_precision)?;

        // R13-R14 Only:
        if sio.r13_14_only {
            writer.write_bit_short(header.object_snap_mode as i16)?;
        }

        // Common:
        writer.write_bit_short(header.attribute_visibility)?;

        // R13-R14 Only:
        if sio.r13_14_only {
            writer.write_bit_short(header.coords_mode)?;
        }

        // Common:
        writer.write_bit_short(header.point_display_mode)?;

        // R13-R14 Only:
        if sio.r13_14_only {
            writer.write_bit_short(header.pick_style)?;
        }

        // R2004+:
        if sio.r2004_plus {
            writer.write_bit_long(0)?;
            writer.write_bit_long(0)?;
            writer.write_bit_long(0)?;
        }

        // Common:
        writer.write_bit_short(header.user_int1)?;
        writer.write_bit_short(header.user_int2)?;
        writer.write_bit_short(header.user_int3)?;
        writer.write_bit_short(header.user_int4)?;
        writer.write_bit_short(header.user_int5)?;

        writer.write_bit_short(header.spline_segments)?;
        writer.write_bit_short(header.surface_u_density)?;
        writer.write_bit_short(header.surface_v_density)?;
        writer.write_bit_short(header.surface_type)?;
        writer.write_bit_short(header.surface_tab1)?;
        writer.write_bit_short(header.surface_tab2)?;
        writer.write_bit_short(header.spline_type)?;
        writer.write_bit_short(header.shade_edge)?;
        writer.write_bit_short(header.shade_diffuse)?;
        writer.write_bit_short(header.unit_mode)?;
        writer.write_bit_short(header.max_active_viewports)?;
        writer.write_bit_short(header.isolines)?;
        writer.write_bit_short(header.multiline_justification)?;
        writer.write_bit_short(header.text_quality)?;

        writer.write_bit_double(header.linetype_scale)?;
        writer.write_bit_double(header.text_height)?;
        writer.write_bit_double(header.trace_width)?;
        writer.write_bit_double(header.sketch_increment)?;
        writer.write_bit_double(header.fillet_radius)?;
        writer.write_bit_double(header.thickness)?;
        writer.write_bit_double(header.angle_base)?;
        writer.write_bit_double(header.point_display_size)?;
        writer.write_bit_double(header.polyline_width)?;
        writer.write_bit_double(header.user_real1)?;
        writer.write_bit_double(header.user_real2)?;
        writer.write_bit_double(header.user_real3)?;
        writer.write_bit_double(header.user_real4)?;
        writer.write_bit_double(header.user_real5)?;
        writer.write_bit_double(header.chamfer_distance_a)?;
        writer.write_bit_double(header.chamfer_distance_b)?;
        writer.write_bit_double(header.chamfer_length)?;
        writer.write_bit_double(header.chamfer_angle)?;
        writer.write_bit_double(header.facet_resolution)?;
        writer.write_bit_double(header.multiline_scale)?;
        writer.write_bit_double(header.current_entity_linetype_scale)?;

        // TV: MENUNAME
        writer.write_variable_text(&header.menu_name)?;

        // Common dates:
        let (jdate, ms) = Self::timestamp_to_julian(header.create_date_julian);
        writer.write_date_time(jdate, ms)?;
        let (jdate, ms) = Self::timestamp_to_julian(header.update_date_julian);
        writer.write_date_time(jdate, ms)?;

        // R2004+:
        if sio.r2004_plus {
            writer.write_bit_long(0)?;
            writer.write_bit_long(0)?;
            writer.write_bit_long(0)?;
        }

        // Common:
        let (days, tms) = Self::timespan_to_parts(header.total_editing_time);
        writer.write_time_span(days, tms)?;
        let (days, tms) = Self::timespan_to_parts(header.user_elapsed_time);
        writer.write_time_span(days, tms)?;

        // CMC: CECOLOR
        writer.write_cm_color(header.current_entity_color)?;

        Ok(())
    }

    /// Write all header variables/handles after HANDSEED.
    fn write_vars_after_handseed(
        sio: &SectionIO,
        writer: &mut dyn IDwgStreamWriter,
        header: &HeaderVariables,
        handles: &DwgHeaderHandlesCollection,
    ) -> Result<()> {
        // H: CLAYER (hard pointer)
        writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.clayer().unwrap_or(0))?;
        // H: TEXTSTYLE (hard pointer)
        writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.textstyle().unwrap_or(0))?;
        // H: CELTYPE (hard pointer)
        writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.celtype().unwrap_or(0))?;

        // R2007+ Only:
        if sio.r2007_plus {
            // H: CMATERIAL (hard pointer)
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.cmaterial().unwrap_or(0))?;
        }

        // Common:
        // H: DIMSTYLE (hard pointer)
        writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dimstyle().unwrap_or(0))?;
        // H: CMLSTYLE (hard pointer)
        writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.cmlstyle().unwrap_or(0))?;

        // R2000+ Only:
        if sio.r2000_plus {
            writer.write_bit_double(header.viewport_scale_factor)?;
        }

        // Common: Paper space
        writer.write_3bit_double(header.paper_space_insertion_base)?;
        writer.write_3bit_double(header.paper_space_extents_min)?;
        writer.write_3bit_double(header.paper_space_extents_max)?;
        writer.write_2raw_double(header.paper_space_limits_min)?;
        writer.write_2raw_double(header.paper_space_limits_max)?;
        writer.write_bit_double(header.paper_elevation)?;
        writer.write_3bit_double(header.paper_space_ucs_origin)?;
        writer.write_3bit_double(header.paper_space_ucs_x_axis)?;
        writer.write_3bit_double(header.paper_space_ucs_y_axis)?;

        // H: UCSNAME (PSPACE) (hard pointer)
        writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.ucsname_pspace().unwrap_or(0))?;

        // R2000+ Only:
        if sio.r2000_plus {
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.pucsorthoref().unwrap_or(0))?;
            writer.write_bit_short(header.paper_ucs_ortho_view)?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.pucsbase().unwrap_or(0))?;
            writer.write_3bit_double(header.paper_ucs_ortho_top)?;
            writer.write_3bit_double(header.paper_ucs_ortho_bottom)?;
            writer.write_3bit_double(header.paper_ucs_ortho_left)?;
            writer.write_3bit_double(header.paper_ucs_ortho_right)?;
            writer.write_3bit_double(header.paper_ucs_ortho_front)?;
            writer.write_3bit_double(header.paper_ucs_ortho_back)?;
        }

        // Common: Model space
        writer.write_3bit_double(header.model_space_insertion_base)?;
        writer.write_3bit_double(header.model_space_extents_min)?;
        writer.write_3bit_double(header.model_space_extents_max)?;
        writer.write_2raw_double(header.model_space_limits_min)?;
        writer.write_2raw_double(header.model_space_limits_max)?;
        writer.write_bit_double(header.elevation)?;
        writer.write_3bit_double(header.model_space_ucs_origin)?;
        writer.write_3bit_double(header.model_space_ucs_x_axis)?;
        writer.write_3bit_double(header.model_space_ucs_y_axis)?;

        // H: UCSNAME (MSPACE) (hard pointer)
        writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.ucsname_mspace().unwrap_or(0))?;

        // R2000+ Only:
        if sio.r2000_plus {
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.ucsorthoref().unwrap_or(0))?;
            writer.write_bit_short(header.ucs_ortho_view)?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.ucsbase().unwrap_or(0))?;
            writer.write_3bit_double(header.model_ucs_ortho_top)?;
            writer.write_3bit_double(header.model_ucs_ortho_bottom)?;
            writer.write_3bit_double(header.model_ucs_ortho_left)?;
            writer.write_3bit_double(header.model_ucs_ortho_right)?;
            writer.write_3bit_double(header.model_ucs_ortho_front)?;
            writer.write_3bit_double(header.model_ucs_ortho_back)?;

            // TV: DIMPOST / DIMAPOST
            writer.write_variable_text(&header.dim_post)?;
            writer.write_variable_text(&header.dim_alt_post)?;
        }

        // =================================================================
        // Dimension variables block
        // =================================================================

        // R13-R14 Only:
        if sio.r13_14_only {
            writer.write_bit(header.dim_tolerance)?;
            writer.write_bit(header.dim_limits)?;
            writer.write_bit(header.dim_text_inside_horizontal)?;
            writer.write_bit(header.dim_text_outside_horizontal)?;
            writer.write_bit(header.dim_suppress_ext1)?;
            writer.write_bit(header.dim_suppress_ext2)?;
            writer.write_bit(header.dim_alternate_units)?;
            writer.write_bit(header.dim_force_line_inside)?;
            writer.write_bit(header.dim_separate_arrows)?;
            writer.write_bit(header.dim_force_text_inside)?;
            writer.write_bit(header.dim_suppress_outside_ext)?;
            // RC: DIMALTD
            writer.write_byte(header.dim_alt_decimal_places as u8)?;
            // RC: DIMZIN
            writer.write_byte(header.dim_zero_suppression as u8)?;
            // B: DIMSD1
            writer.write_bit(header.dim_suppress_line1)?;
            // B: DIMSD2
            writer.write_bit(header.dim_suppress_line2)?;
            // RC: DIMTOLJ
            writer.write_byte(header.dim_tolerance_justification as u8)?;
            // RC: DIMJUST
            writer.write_byte(header.dim_horizontal_justification as u8)?;
            // RC: DIMFIT
            writer.write_byte(header.dim_fit as u8)?;
            // B: DIMUPT
            writer.write_bit(header.dim_user_positioned_text)?;
            // RC: DIMTZIN
            writer.write_byte(header.dim_tolerance_zero_suppression as u8)?;
            // RC: DIMALTZ
            writer.write_byte(header.dim_alt_tolerance_zero_suppression as u8)?;
            // RC: DIMALTTZ
            writer.write_byte(header.dim_alt_tolerance_zero_tight as u8)?;
            // RC: DIMTAD
            writer.write_byte(header.dim_text_above as u8)?;
            // BS: DIMUNIT
            writer.write_bit_short(0)?; // R14 only, not stored
            // BS: DIMAUNIT
            writer.write_bit_short(header.dim_angular_decimal_places)?;
            // BS: DIMDEC
            writer.write_bit_short(header.dim_decimal_places)?;
            // BS: DIMTDEC
            writer.write_bit_short(header.dim_tolerance_decimal_places)?;
            // BS: DIMALTU
            writer.write_bit_short(header.dim_alt_units_format)?;
            // BS: DIMALTTD
            writer.write_bit_short(header.dim_alt_tolerance_decimal_places)?;
            // H: DIMTXSTY (hard pointer)
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dimtxsty().unwrap_or(0))?;
        }

        // Common:
        writer.write_bit_double(header.dim_scale)?;
        writer.write_bit_double(header.dim_arrow_size)?;
        writer.write_bit_double(header.dim_ext_line_offset)?;
        writer.write_bit_double(header.dim_line_increment)?;
        writer.write_bit_double(header.dim_ext_line_extension)?;
        writer.write_bit_double(header.dim_rounding)?;
        writer.write_bit_double(header.dim_line_extension)?;
        writer.write_bit_double(header.dim_tolerance_plus)?;
        writer.write_bit_double(header.dim_tolerance_minus)?;

        // R2007+ Only:
        if sio.r2007_plus {
            writer.write_bit_double(header.dim_fixed_ext_line_length)?;
            writer.write_bit_double(header.dim_jog_angle)?;
            writer.write_bit_short(header.dim_text_fill_mode)?;
            writer.write_cm_color(header.dim_text_fill_color)?;
        }

        // R2000+ Only:
        if sio.r2000_plus {
            writer.write_bit(header.dim_tolerance)?;
            writer.write_bit(header.dim_limits)?;
            writer.write_bit(header.dim_text_inside_horizontal)?;
            writer.write_bit(header.dim_text_outside_horizontal)?;
            writer.write_bit(header.dim_suppress_ext1)?;
            writer.write_bit(header.dim_suppress_ext2)?;
            writer.write_bit_short(header.dim_text_above)?;
            writer.write_bit_short(header.dim_zero_suppression)?;
            writer.write_bit_short(header.dim_alt_zero_suppression)?;
        }

        // R2007+ Only:
        if sio.r2007_plus {
            writer.write_bit_short(header.dim_arc_symbol_position)?;
        }

        // Common:
        writer.write_bit_double(header.dim_text_height)?;
        writer.write_bit_double(header.dim_center_mark)?;
        writer.write_bit_double(header.dim_tick_size)?;
        writer.write_bit_double(header.dim_alt_scale)?;
        writer.write_bit_double(header.dim_linear_scale)?;
        writer.write_bit_double(header.dim_text_vertical_pos)?;
        writer.write_bit_double(header.dim_tolerance_scale)?;
        writer.write_bit_double(header.dim_line_gap)?;

        // R13-R14 Only:
        if sio.r13_14_only {
            writer.write_variable_text(&header.dim_post)?;
            writer.write_variable_text(&header.dim_alt_post)?;
            writer.write_variable_text(&header.dim_arrow_block)?;
            writer.write_variable_text(&header.dim_arrow_block1)?;
            writer.write_variable_text(&header.dim_arrow_block2)?;
        }

        // R2000+ Only:
        if sio.r2000_plus {
            writer.write_bit_double(header.dim_alt_rounding)?;
            writer.write_bit(header.dim_alternate_units)?;
            writer.write_bit_short(header.dim_alt_decimal_places)?;
            writer.write_bit(header.dim_force_line_inside)?;
            writer.write_bit(header.dim_separate_arrows)?;
            writer.write_bit(header.dim_force_text_inside)?;
            writer.write_bit(header.dim_suppress_outside_ext)?;
        }

        // Common:
        writer.write_cm_color(header.dim_line_color)?;
        writer.write_cm_color(header.dim_ext_line_color)?;
        writer.write_cm_color(header.dim_text_color)?;

        // R2000+ Only:
        if sio.r2000_plus {
            writer.write_bit_short(header.dim_angular_decimal_places)?;
            writer.write_bit_short(header.dim_decimal_places)?;
            writer.write_bit_short(header.dim_tolerance_decimal_places)?;
            writer.write_bit_short(header.dim_alt_units_format)?;
            writer.write_bit_short(header.dim_alt_tolerance_decimal_places)?;
            writer.write_bit_short(header.dim_angular_units)?;
            writer.write_bit_short(header.dim_fraction_format)?;
            writer.write_bit_short(header.dim_linear_unit_format)?;
            writer.write_bit_short(header.dim_decimal_separator as i16)?;
            writer.write_bit_short(header.dim_text_movement)?;
            writer.write_bit_short(header.dim_horizontal_justification)?;
            writer.write_bit(header.dim_suppress_line1)?;
            writer.write_bit(header.dim_suppress_line2)?;
            writer.write_bit_short(header.dim_tolerance_justification)?;
            writer.write_bit_short(header.dim_tolerance_zero_suppression)?;
            writer.write_bit_short(header.dim_alt_tolerance_zero_suppression)?;
            writer.write_bit_short(header.dim_alt_tolerance_zero_tight)?;
            writer.write_bit(header.dim_user_positioned_text)?;
            writer.write_bit_short(header.dim_fit)?;
        }

        // R2007+ Only:
        if sio.r2007_plus {
            writer.write_bit(header.dim_ext_line_length_fixed)?;
        }

        // R2010+ Only:
        if sio.r2010_plus {
            writer.write_bit(header.dim_text_direction)?;
            writer.write_bit_double(header.dim_alt_mzf)?;
            writer.write_variable_text(&header.dim_alt_mzs)?;
            writer.write_bit_double(header.dim_mzf)?;
            writer.write_variable_text(&header.dim_mzs)?;
        }

        // R2000+ Only:
        if sio.r2000_plus {
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dimtxsty().unwrap_or(0))?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dimldrblk().unwrap_or(0))?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dimblk().unwrap_or(0))?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dimblk1().unwrap_or(0))?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dimblk2().unwrap_or(0))?;
        }

        // R2007+ Only:
        if sio.r2007_plus {
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dimltype().unwrap_or(0))?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dimltex1().unwrap_or(0))?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dimltex2().unwrap_or(0))?;
        }

        // R2000+ Only:
        if sio.r2000_plus {
            writer.write_bit_short(header.dim_line_weight)?;
            writer.write_bit_short(header.dim_ext_line_weight)?;
        }

        // =================================================================
        // Table control object handles
        // =================================================================

        writer.handle_reference_typed(DwgReferenceType::HardOwnership, handles.block_control_object().unwrap_or(0))?;
        writer.handle_reference_typed(DwgReferenceType::HardOwnership, handles.layer_control_object().unwrap_or(0))?;
        writer.handle_reference_typed(DwgReferenceType::HardOwnership, handles.style_control_object().unwrap_or(0))?;
        writer.handle_reference_typed(DwgReferenceType::HardOwnership, handles.linetype_control_object().unwrap_or(0))?;
        writer.handle_reference_typed(DwgReferenceType::HardOwnership, handles.view_control_object().unwrap_or(0))?;
        writer.handle_reference_typed(DwgReferenceType::HardOwnership, handles.ucs_control_object().unwrap_or(0))?;
        writer.handle_reference_typed(DwgReferenceType::HardOwnership, handles.vport_control_object().unwrap_or(0))?;
        writer.handle_reference_typed(DwgReferenceType::HardOwnership, handles.appid_control_object().unwrap_or(0))?;
        writer.handle_reference_typed(DwgReferenceType::HardOwnership, handles.dimstyle_control_object().unwrap_or(0))?;

        // R13-R15 Only:
        if sio.r13_15_only {
            writer.handle_reference_typed(DwgReferenceType::HardOwnership, handles.viewport_entity_header_control_object().unwrap_or(0))?;
        }

        // Common:
        writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dictionary_acad_group().unwrap_or(0))?;
        writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dictionary_acad_mlinestyle().unwrap_or(0))?;
        writer.handle_reference_typed(DwgReferenceType::HardOwnership, handles.dictionary_named_objects().unwrap_or(0))?;

        // R2000+ Only:
        if sio.r2000_plus {
            writer.write_bit_short(header.stacked_text_alignment)?;
            writer.write_bit_short(header.stacked_text_size_percentage)?;
            writer.write_variable_text(&header.hyperlink_base)?;
            writer.write_variable_text(&header.stylesheet)?;

            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dictionary_layouts().unwrap_or(0))?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dictionary_plotsettings().unwrap_or(0))?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dictionary_plotstyles().unwrap_or(0))?;
        }

        // R2004+:
        if sio.r2004_plus {
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dictionary_materials().unwrap_or(0))?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dictionary_colors().unwrap_or(0))?;
        }

        // R2007+:
        if sio.r2007_plus {
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dictionary_visualstyle().unwrap_or(0))?;
            if sio.r2013_plus {
                writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dictionary_visualstyle().unwrap_or(0))?;
            }
        }

        // R2000+: Flags word
        if sio.r2000_plus {
            let mut flags: i32 = (header.current_line_weight as i32) & 0x1F;
            flags |= (header.end_caps as i32) & 0x60;
            flags |= (header.join_style as i32) & 0x180;
            if !header.lineweight_display {
                flags |= 0x200;
            }
            if !header.xedit {
                flags |= 0x400;
            }
            if header.extended_names {
                flags |= 0x800;
            }
            if header.plotstyle_mode {
                flags |= 0x2000;
            }
            if header.ole_startup {
                flags |= 0x4000;
            }

            writer.write_bit_long(flags)?;

            writer.write_bit_short(header.insertion_units)?;
            writer.write_bit_short(header.current_plotstyle_type)?;

            if header.current_plotstyle_type == 3 {
                writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.cpsnid().unwrap_or(0))?;
            }

            writer.write_variable_text(&header.fingerprint_guid)?;
            writer.write_variable_text(&header.version_guid)?;
        }

        // R2004+:
        if sio.r2004_plus {
            writer.write_byte(header.sort_entities as u8)?;
            writer.write_byte(header.index_control as u8)?;
            writer.write_byte(header.hide_text as u8)?;
            writer.write_byte(header.xclip_frame as u8)?;
            writer.write_byte(header.dimension_associativity as u8)?;
            writer.write_byte(header.halo_gap as u8)?;
            writer.write_bit_short(header.obscured_color)?;
            writer.write_bit_short(header.intersection_color)?;
            writer.write_byte(header.obscured_linetype as u8)?;
            writer.write_byte(header.intersection_display as u8)?;

            writer.write_variable_text(&header.project_name)?;
        }

        // Common block record handles:
        writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.paper_space().unwrap_or(0))?;
        writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.model_space().unwrap_or(0))?;
        writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.bylayer().unwrap_or(0))?;
        writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.byblock().unwrap_or(0))?;
        writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.continuous().unwrap_or(0))?;

        // R2007+:
        if sio.r2007_plus {
            writer.write_bit(header.camera_display)?;

            writer.write_bit_long(0)?;
            writer.write_bit_long(0)?;
            writer.write_bit_double(0.0)?;

            writer.write_bit_double(header.steps_per_second)?;
            writer.write_bit_double(header.step_size)?;
            writer.write_bit_double(header.dw3d_precision)?;
            writer.write_bit_double(header.lens_length)?;
            writer.write_bit_double(header.camera_height)?;
            writer.write_byte(header.solids_retain_history as u8)?;
            writer.write_byte(header.show_solids_history as u8)?;
            writer.write_bit_double(header.swept_solid_width)?;
            writer.write_bit_double(header.swept_solid_height)?;
            writer.write_bit_double(header.loft_angle1)?;
            writer.write_bit_double(header.loft_angle2)?;
            writer.write_bit_double(header.loft_magnitude1)?;
            writer.write_bit_double(header.loft_magnitude2)?;
            writer.write_bit_short(header.loft_param)?;
            writer.write_byte(header.loft_normals as u8)?;
            writer.write_bit_double(header.latitude)?;
            writer.write_bit_double(header.longitude)?;
            writer.write_bit_double(header.north_direction)?;
            writer.write_bit_long(header.timezone)?;
            writer.write_byte(header.light_glyph_display as u8)?;
            writer.write_byte(header.tile_model_light_synch as u8)?;
            writer.write_byte(header.dwf_frame as u8)?;
            writer.write_byte(header.dgn_frame as u8)?;

            writer.write_bit(false)?; // unknown

            // CMC: INTERFERECOLOR
            writer.write_cm_color(crate::types::Color::Index(header.intersection_color as u8))?;

            // Handles:
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.interfereobjvs().unwrap_or(0))?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.interferevpvs().unwrap_or(0))?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, handles.dragvs().unwrap_or(0))?;

            writer.write_byte(header.shadow_mode as u8)?;
            writer.write_bit_double(header.shadow_plane_location)?;
        }

        // R14+: Unknown trailing shorts
        if sio.version() >= DxfVersion::AC1014 {
            writer.write_bit_short(-1)?;
            writer.write_bit_short(-1)?;
            writer.write_bit_short(-1)?;
            writer.write_bit_short(-1)?;

            if sio.r2004_plus {
                writer.write_bit_long(0)?;
                writer.write_bit_long(0)?;
                writer.write_bit(false)?;
            }
        }

        Ok(())
    }
}
