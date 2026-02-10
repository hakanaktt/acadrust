//! DWG Header section reader.
//!
//! Reads all system variables from the `AcDb:Header` section in a DWG file.
//! The header section contains ~500 drawing settings, organized in a specific
//! read order that varies by DWG version.
//!
//! Mirrors ACadSharp's `DwgHeaderReader`.

use crate::document::HeaderVariables;
use crate::error::Result;
use crate::io::dwg::constants::sentinels;
use crate::io::dwg::header_handles::DwgHeaderHandlesCollection;
use crate::io::dwg::reader::merged_reader::DwgMergedReader;
use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
use crate::io::dwg::reader::stream_reader_base::get_stream_handler;
use crate::io::dwg::section_io::SectionIO;
use crate::types::DxfVersion;

/// Reader for the DWG `AcDb:Header` section.
///
/// Reads the complete set of system variables (header variables) and
/// all handle references from the header section.
pub struct DwgHeaderReader {
    /// Raw section data bytes (decompressed/decrypted by caller).
    data: Vec<u8>,
    /// DWG version being read.
    version: DxfVersion,
}

impl DwgHeaderReader {
    /// Create a new header reader.
    ///
    /// # Arguments
    /// * `version` - The DWG version being read.
    /// * `data` - Raw section bytes (already decompressed/decrypted).
    pub fn new(version: DxfVersion, data: Vec<u8>) -> Self {
        Self { data, version }
    }

    /// Read all header variables and handle references.
    ///
    /// Returns a tuple of (HeaderVariables, DwgHeaderHandlesCollection).
    pub fn read(
        &self,
        acad_maintenance_version: i32,
    ) -> Result<(HeaderVariables, DwgHeaderHandlesCollection)> {
        let mut header = HeaderVariables::default();
        let mut handles = DwgHeaderHandlesCollection::new();
        let sio = SectionIO::new(self.version);

        let mut main_reader = get_stream_handler(self.version, self.data.clone());

        // Start sentinel:
        // 0xCF,0x7B,0x1F,0x23,0xFD,0xDE,0x38,0xA9,0x5F,0x7C,0x68,0xB8,0x4E,0x6D,0x33,0x5F
        let sentinel = main_reader.read_sentinel()?;
        if let Some(expected) = sentinels::start_sentinel("AcDb:Header") {
            SectionIO::check_sentinel(&sentinel, expected);
        }

        // RL: Size of the section
        let size = main_reader.read_raw_long()? as i64;

        // R2010/R2013 (only present if the maintenance version is greater than 3!) or R2018+:
        if (sio.r2010_plus && acad_maintenance_version > 3) || sio.r2018_plus {
            // Unknown (4 byte long), might be part of a 64-bit size
            let _unknown = main_reader.read_raw_long()?;
        }

        let initial_pos = main_reader.position_in_bits();

        // +R2007 Only:
        if sio.r2007_plus {
            // RL: Size in bits
            let size_in_bits = main_reader.read_raw_long()? as i64;
            let last_position_in_bits = initial_pos + size_in_bits - 1;

            // Setup the text handler for versions 2007 and above
            let mut text_reader = get_stream_handler(self.version, self.data.clone());
            // Set the position and use the flag
            text_reader.set_position_by_flag(last_position_in_bits)?;

            // Setup the handler for the references for versions 2007 and above
            let mut reference_reader = get_stream_handler(self.version, self.data.clone());
            // Set the position and jump the flag
            reference_reader.set_position_in_bits(last_position_in_bits + 1);

            let mut merged =
                DwgMergedReader::new(main_reader, text_reader, reference_reader, self.version);

            // Read everything before HANDSEED
            Self::read_vars_before_handseed(&sio, &mut merged, &mut header, &mut handles)?;

            // H: HANDSEED The next handle - reads from main data stream, not handle stream
            header.handle_seed = merged.main_reader_mut().handle_reference()?;

            // Read everything after HANDSEED
            Self::read_vars_after_handseed(&sio, &mut merged, &mut header, &mut handles)?;

            // End sentinel: use a fresh reader positioned at the end
            let end_pos = initial_pos + size * 8;
            let mut end_reader = get_stream_handler(self.version, self.data.clone());
            end_reader.set_position_in_bits(end_pos);
            end_reader.reset_shift()?;
            let sentinel = end_reader.read_sentinel()?;
            if let Some(expected) = sentinels::end_sentinel("AcDb:Header") {
                SectionIO::check_sentinel(&sentinel, expected);
            }
        } else {
            // Pre-R2007 path: single reader

            // Read everything before HANDSEED
            Self::read_vars_before_handseed(
                &sio,
                &mut main_reader,
                &mut header,
                &mut handles,
            )?;

            // H: HANDSEED
            header.handle_seed = main_reader.handle_reference()?;

            // Read everything after HANDSEED
            Self::read_vars_after_handseed(
                &sio,
                &mut main_reader,
                &mut header,
                &mut handles,
            )?;

            // End sentinel: set position at end of section
            main_reader.set_position_in_bits(initial_pos + size * 8);
            main_reader.reset_shift()?;
            let sentinel = main_reader.read_sentinel()?;
            if let Some(expected) = sentinels::end_sentinel("AcDb:Header") {
                SectionIO::check_sentinel(&sentinel, expected);
            }
        }

        Ok((header, handles))
    }

    /// Read all header variables from the start through CECOLOR (before HANDSEED).
    fn read_vars_before_handseed(
        sio: &SectionIO,
        reader: &mut dyn IDwgStreamReader,
        header: &mut HeaderVariables,
        _handles: &mut DwgHeaderHandlesCollection,
    ) -> Result<()> {
        // R2013+:
        if sio.r2013_plus {
            // BLL: Variabele REQUIREDVERSIONS, default value 0, read only
            header.required_versions = reader.read_bit_long_long()?;
        }

        // Common:
        // BD: Unknown, default value 412148564080.0
        reader.read_bit_double()?;
        // BD: Unknown, default value 1.0
        reader.read_bit_double()?;
        // BD: Unknown, default value 1.0
        reader.read_bit_double()?;
        // BD: Unknown, default value 1.0
        reader.read_bit_double()?;
        // TV: Unknown text string, default "m"
        reader.read_variable_text()?;
        // TV: Unknown text string, default ""
        reader.read_variable_text()?;
        // TV: Unknown text string, default ""
        reader.read_variable_text()?;
        // TV: Unknown text string, default ""
        reader.read_variable_text()?;
        // BL: Unknown long, default value 24L
        reader.read_bit_long()?;
        // BL: Unknown long, default value 0L
        reader.read_bit_long()?;

        // R13-R14 Only:
        if sio.r13_14_only {
            // BS: Unknown short, default value 0
            reader.read_bit_short()?;
        }

        // Pre-2004 Only:
        if sio.r2004_pre {
            // H: Handle of the current viewport entity header (hard pointer)
            reader.handle_reference()?;
        }

        // Common:
        // B: DIMASO
        header.associate_dimensions = reader.read_bit()?;
        // B: DIMSHO
        header.update_dimensions_while_dragging = reader.read_bit()?;

        // R13-R14 Only:
        if sio.r13_14_only {
            // B: DIMSAV Undocumented
            header.dim_sav = reader.read_bit()?;
        }

        // Common:
        // B: PLINEGEN
        header.polyline_linetype_generation = reader.read_bit()?;
        // B: ORTHOMODE
        header.ortho_mode = reader.read_bit()?;
        // B: REGENMODE
        header.regen_mode = reader.read_bit()?;
        // B: FILLMODE
        header.fill_mode = reader.read_bit()?;
        // B: QTEXTMODE
        header.quick_text_mode = reader.read_bit()?;
        // B: PSLTSCALE
        header.paper_space_linetype_scaling = reader.read_bit()?;
        // B: LIMCHECK
        header.limit_check = reader.read_bit()?;

        // R13-R14 Only (stored in registry from R15 onwards):
        if sio.r13_14_only {
            // B: BLIPMODE
            header.blip_mode = reader.read_bit()?;
        }
        // R2004+:
        if sio.r2004_plus {
            // B: Undocumented
            reader.read_bit()?;
        }

        // Common:
        // B: USRTIMER (User timer on/off)
        header.user_timer = reader.read_bit()?;
        // B: SKPOLY
        header.sketch_polylines = reader.read_bit()?;
        // B: ANGDIR
        header.angle_direction = reader.read_bit_as_short()?;
        // B: SPLFRAME
        header.spline_frame = reader.read_bit()?;

        // R13-R14 Only (stored in registry from R15 onwards):
        if sio.r13_14_only {
            // B: ATTREQ
            header.attribute_request = reader.read_bit()?;
            // B: ATTDIA
            header.attribute_dialog = reader.read_bit()?;
        }

        // Common:
        // B: MIRRTEXT
        header.mirror_text = reader.read_bit()?;
        // B: WORLDVIEW
        header.world_view = reader.read_bit()?;

        // R13-R14 Only:
        if sio.r13_14_only {
            // B: WIREFRAME Undocumented
            reader.read_bit()?;
        }

        // Common:
        // B: TILEMODE
        header.show_model_space = reader.read_bit()?;
        // B: PLIMCHECK
        header.paper_space_limit_check = reader.read_bit()?;
        // B: VISRETAIN
        header.retain_xref_visibility = reader.read_bit()?;

        // R13-R14 Only (stored in registry from R15 onwards):
        if sio.r13_14_only {
            // B: DELOBJ
            header.delete_objects = reader.read_bit()?;
        }

        // Common:
        // B: DISPSILH
        header.display_silhouette = reader.read_bit()?;
        // B: PELLIPSE (not present in DXF)
        header.create_ellipse_as_polyline = reader.read_bit()?;
        // BS: PROXYGRAPHICS
        header.proxy_graphics = if reader.read_bit_short_as_bool()? { 1 } else { 0 };

        // R13-R14 Only (stored in registry from R15 onwards):
        if sio.r13_14_only {
            // BS: DRAGMODE
            header.drag_mode = reader.read_bit_short()?;
        }

        // Common:
        // BS: TREEDEPTH
        header.tree_depth = reader.read_bit_short()?;
        // BS: LUNITS
        header.linear_unit_format = reader.read_bit_short()?;
        // BS: LUPREC
        let linear_unit_precision = reader.read_bit_short()?;
        if linear_unit_precision >= 0 && linear_unit_precision <= 8 {
            header.linear_unit_precision = linear_unit_precision;
        }
        // BS: AUNITS
        header.angular_unit_format = reader.read_bit_short()?;
        // BS: AUPREC
        let angular_unit_precision = reader.read_bit_short()?;
        if angular_unit_precision >= 0 && angular_unit_precision <= 8 {
            header.angular_unit_precision = angular_unit_precision;
        }

        // R13-R14 Only (stored in registry from R15 onwards):
        if sio.r13_14_only {
            // BS: OSMODE
            header.object_snap_mode = reader.read_bit_short()? as i32;
        }

        // Common:
        // BS: ATTMODE
        header.attribute_visibility = reader.read_bit_short()?;

        // R13-R14 Only (stored in registry from R15 onwards):
        if sio.r13_14_only {
            // BS: COORDS
            header.coords_mode = reader.read_bit_short()?;
        }

        // Common:
        // BS: PDMODE
        header.point_display_mode = reader.read_bit_short()?;

        // R13-R14 Only (stored in registry from R15 onwards):
        if sio.r13_14_only {
            // BS: PICKSTYLE
            header.pick_style = reader.read_bit_short()?;
        }

        // R2004+:
        if sio.r2004_plus {
            // BL: Unknown
            reader.read_bit_long()?;
            // BL: Unknown
            reader.read_bit_long()?;
            // BL: Unknown
            reader.read_bit_long()?;
        }

        // Common:
        // BS: USERI1
        header.user_int1 = reader.read_bit_short()?;
        // BS: USERI2
        header.user_int2 = reader.read_bit_short()?;
        // BS: USERI3
        header.user_int3 = reader.read_bit_short()?;
        // BS: USERI4
        header.user_int4 = reader.read_bit_short()?;
        // BS: USERI5
        header.user_int5 = reader.read_bit_short()?;

        // BS: SPLINESEGS
        header.spline_segments = reader.read_bit_short()?;
        // BS: SURFU
        header.surface_u_density = reader.read_bit_short()?;
        // BS: SURFV
        header.surface_v_density = reader.read_bit_short()?;
        // BS: SURFTYPE
        header.surface_type = reader.read_bit_short()?;
        // BS: SURFTAB1
        header.surface_tab1 = reader.read_bit_short()?;
        // BS: SURFTAB2
        header.surface_tab2 = reader.read_bit_short()?;
        // BS: SPLINETYPE
        header.spline_type = reader.read_bit_short()?;
        // BS: SHADEDGE
        header.shade_edge = reader.read_bit_short()?;
        // BS: SHADEDIF
        header.shade_diffuse = reader.read_bit_short()?;
        // BS: UNITMODE
        header.unit_mode = reader.read_bit_short()?;
        // BS: MAXACTVP
        header.max_active_viewports = reader.read_bit_short()?;
        // BS: ISOLINES
        let isolines = reader.read_bit_short()?;
        if isolines >= 0 && isolines <= 2047 {
            header.isolines = isolines;
        }
        // BS: CMLJUST
        header.multiline_justification = reader.read_bit_short()?;
        // BS: TEXTQLTY
        let text_quality = reader.read_bit_short()?;
        if text_quality >= 0 && text_quality <= 100 {
            header.text_quality = text_quality;
        }

        // BD: LTSCALE
        header.linetype_scale = reader.read_bit_double()?;
        // BD: TEXTSIZE
        header.text_height = reader.read_bit_double()?;
        // BD: TRACEWID
        header.trace_width = reader.read_bit_double()?;
        // BD: SKETCHINC
        header.sketch_increment = reader.read_bit_double()?;
        // BD: FILLETRAD
        header.fillet_radius = reader.read_bit_double()?;
        // BD: THICKNESS
        header.thickness = reader.read_bit_double()?;
        // BD: ANGBASE
        header.angle_base = reader.read_bit_double()?;
        // BD: PDSIZE
        header.point_display_size = reader.read_bit_double()?;
        // BD: PLINEWID
        header.polyline_width = reader.read_bit_double()?;
        // BD: USERR1
        header.user_real1 = reader.read_bit_double()?;
        // BD: USERR2
        header.user_real2 = reader.read_bit_double()?;
        // BD: USERR3
        header.user_real3 = reader.read_bit_double()?;
        // BD: USERR4
        header.user_real4 = reader.read_bit_double()?;
        // BD: USERR5
        header.user_real5 = reader.read_bit_double()?;
        // BD: CHAMFERA
        header.chamfer_distance_a = reader.read_bit_double()?;
        // BD: CHAMFERB
        header.chamfer_distance_b = reader.read_bit_double()?;
        // BD: CHAMFERC
        header.chamfer_length = reader.read_bit_double()?;
        // BD: CHAMFERD
        header.chamfer_angle = reader.read_bit_double()?;
        // BD: FACETRES
        let facet_resolution = reader.read_bit_double()?;
        if facet_resolution > 0.0 && facet_resolution <= 10.0 {
            header.facet_resolution = facet_resolution;
        }
        // BD: CMLSCALE
        header.multiline_scale = reader.read_bit_double()?;
        // BD: CELTSCALE
        header.current_entity_linetype_scale = reader.read_bit_double()?;

        // TV: MENUNAME
        header.menu_name = reader.read_variable_text()?;

        // Common:
        // BL: TDCREATE (Julian day)
        // BL: TDCREATE (Milliseconds into the day)
        header.create_date_julian = reader.read_date_time()?;
        // BL: TDUPDATE (Julian day)
        // BL: TDUPDATE (Milliseconds into the day)
        header.update_date_julian = reader.read_date_time()?;

        // R2004+:
        if sio.r2004_plus {
            // BL: Unknown
            reader.read_bit_long()?;
            // BL: Unknown
            reader.read_bit_long()?;
            // BL: Unknown
            reader.read_bit_long()?;
        }

        // Common:
        // BL: TDINDWG (Days)
        // BL: TDINDWG (Milliseconds into the day)
        header.total_editing_time = reader.read_time_span()?;
        // BL: TDUSRTIMER (Days)
        // BL: TDUSRTIMER (Milliseconds into the day)
        header.user_elapsed_time = reader.read_time_span()?;

        // CMC: CECOLOR
        header.current_entity_color = reader.read_cm_color()?;

        // Note: HANDSEED is read separately between the two halves
        Ok(())
    }

    /// Read all header variables/handles after HANDSEED.
    fn read_vars_after_handseed(
        sio: &SectionIO,
        reader: &mut dyn IDwgStreamReader,
        header: &mut HeaderVariables,
        handles: &mut DwgHeaderHandlesCollection,
    ) -> Result<()> {
        // H: CLAYER (hard pointer)
        handles.set_clayer(Some(reader.handle_reference()?));
        // H: TEXTSTYLE (hard pointer)
        handles.set_textstyle(Some(reader.handle_reference()?));
        // H: CELTYPE (hard pointer)
        handles.set_celtype(Some(reader.handle_reference()?));

        // R2007+ Only:
        if sio.r2007_plus {
            // H: CMATERIAL (hard pointer)
            handles.set_cmaterial(Some(reader.handle_reference()?));
        }

        // Common:
        // H: DIMSTYLE (hard pointer)
        handles.set_dimstyle(Some(reader.handle_reference()?));
        // H: CMLSTYLE (hard pointer)
        handles.set_cmlstyle(Some(reader.handle_reference()?));

        // R2000+ Only:
        if sio.r2000_plus {
            // BD: PSVPSCALE
            header.viewport_scale_factor = reader.read_bit_double()?;
        }

        // Common:
        // 3BD: INSBASE (PSPACE)
        header.paper_space_insertion_base = reader.read_3bit_double()?;
        // 3BD: EXTMIN (PSPACE)
        header.paper_space_extents_min = reader.read_3bit_double()?;
        // 3BD: EXTMAX (PSPACE)
        header.paper_space_extents_max = reader.read_3bit_double()?;
        // 2RD: LIMMIN (PSPACE)
        header.paper_space_limits_min = reader.read_2raw_double()?;
        // 2RD: LIMMAX (PSPACE)
        header.paper_space_limits_max = reader.read_2raw_double()?;
        // BD: ELEVATION (PSPACE)
        header.paper_elevation = reader.read_bit_double()?;
        // 3BD: UCSORG (PSPACE)
        header.paper_space_ucs_origin = reader.read_3bit_double()?;
        // 3BD: UCSXDIR (PSPACE)
        header.paper_space_ucs_x_axis = reader.read_3bit_double()?;
        // 3BD: UCSYDIR (PSPACE)
        header.paper_space_ucs_y_axis = reader.read_3bit_double()?;

        // H: UCSNAME (PSPACE) (hard pointer)
        handles.set_ucsname_pspace(Some(reader.handle_reference()?));

        // R2000+ Only:
        if sio.r2000_plus {
            // H: PUCSORTHOREF (hard pointer)
            handles.set_pucsorthoref(Some(reader.handle_reference()?));

            // BS: PUCSORTHOVIEW
            header.paper_ucs_ortho_view = reader.read_bit_short()?;

            // H: PUCSBASE (hard pointer)
            handles.set_pucsbase(Some(reader.handle_reference()?));

            // 3BD: PUCSORGTOP
            header.paper_ucs_ortho_top = reader.read_3bit_double()?;
            // 3BD: PUCSORGBOTTOM
            header.paper_ucs_ortho_bottom = reader.read_3bit_double()?;
            // 3BD: PUCSORGLEFT
            header.paper_ucs_ortho_left = reader.read_3bit_double()?;
            // 3BD: PUCSORGRIGHT
            header.paper_ucs_ortho_right = reader.read_3bit_double()?;
            // 3BD: PUCSORGFRONT
            header.paper_ucs_ortho_front = reader.read_3bit_double()?;
            // 3BD: PUCSORGBACK
            header.paper_ucs_ortho_back = reader.read_3bit_double()?;
        }

        // Common:
        // 3BD: INSBASE (MSPACE)
        header.model_space_insertion_base = reader.read_3bit_double()?;
        // 3BD: EXTMIN (MSPACE)
        header.model_space_extents_min = reader.read_3bit_double()?;
        // 3BD: EXTMAX (MSPACE)
        header.model_space_extents_max = reader.read_3bit_double()?;
        // 2RD: LIMMIN (MSPACE)
        header.model_space_limits_min = reader.read_2raw_double()?;
        // 2RD: LIMMAX (MSPACE)
        header.model_space_limits_max = reader.read_2raw_double()?;
        // BD: ELEVATION (MSPACE)
        header.elevation = reader.read_bit_double()?;
        // 3BD: UCSORG (MSPACE)
        header.model_space_ucs_origin = reader.read_3bit_double()?;
        // 3BD: UCSXDIR (MSPACE)
        header.model_space_ucs_x_axis = reader.read_3bit_double()?;
        // 3BD: UCSYDIR (MSPACE)
        header.model_space_ucs_y_axis = reader.read_3bit_double()?;

        // H: UCSNAME (MSPACE) (hard pointer)
        handles.set_ucsname_mspace(Some(reader.handle_reference()?));

        // R2000+ Only:
        if sio.r2000_plus {
            // H: UCSORTHOREF (hard pointer)
            handles.set_ucsorthoref(Some(reader.handle_reference()?));

            // BS: UCSORTHOVIEW
            header.ucs_ortho_view = reader.read_bit_short()?;

            // H: UCSBASE (hard pointer)
            handles.set_ucsbase(Some(reader.handle_reference()?));

            // 3BD: UCSORGTOP
            header.model_ucs_ortho_top = reader.read_3bit_double()?;
            // 3BD: UCSORGBOTTOM
            header.model_ucs_ortho_bottom = reader.read_3bit_double()?;
            // 3BD: UCSORGLEFT
            header.model_ucs_ortho_left = reader.read_3bit_double()?;
            // 3BD: UCSORGRIGHT
            header.model_ucs_ortho_right = reader.read_3bit_double()?;
            // 3BD: UCSORGFRONT
            header.model_ucs_ortho_front = reader.read_3bit_double()?;
            // 3BD: UCSORGBACK
            header.model_ucs_ortho_back = reader.read_3bit_double()?;

            // TV: DIMPOST
            header.dim_post = reader.read_variable_text()?;
            // TV: DIMAPOST
            header.dim_alt_post = reader.read_variable_text()?;
        }

        // ===================================================================
        // Dimension variables block
        // ===================================================================

        // R13-R14 Only:
        if sio.r13_14_only {
            // B: DIMTOL
            header.dim_tolerance = reader.read_bit()?;
            // B: DIMLIM
            header.dim_limits = reader.read_bit()?;
            // B: DIMTIH
            header.dim_text_inside_horizontal = reader.read_bit()?;
            // B: DIMTOH
            header.dim_text_outside_horizontal = reader.read_bit()?;
            // B: DIMSE1
            header.dim_suppress_ext1 = reader.read_bit()?;
            // B: DIMSE2
            header.dim_suppress_ext2 = reader.read_bit()?;
            // B: DIMALT
            header.dim_alternate_units = reader.read_bit()?;
            // B: DIMTOFL
            header.dim_force_line_inside = reader.read_bit()?;
            // B: DIMSAH
            header.dim_separate_arrows = reader.read_bit()?;
            // B: DIMTIX
            header.dim_force_text_inside = reader.read_bit()?;
            // B: DIMSOXD
            header.dim_suppress_outside_ext = reader.read_bit()?;
            // RC: DIMALTD
            header.dim_alt_decimal_places = reader.read_raw_char()? as i16;
            // RC: DIMZIN
            header.dim_zero_suppression = reader.read_raw_char()? as i16;
            // B: DIMSD1
            header.dim_suppress_line1 = reader.read_bit()?;
            // B: DIMSD2
            header.dim_suppress_line2 = reader.read_bit()?;
            // RC: DIMTOLJ
            header.dim_tolerance_justification = reader.read_raw_char()? as i16;
            // RC: DIMJUST
            header.dim_horizontal_justification = reader.read_raw_char()? as i16;
            // RC: DIMFIT
            header.dim_fit = reader.read_raw_char()? as i16;
            // B: DIMUPT
            header.dim_user_positioned_text = reader.read_bit()?;
            // RC: DIMTZIN
            header.dim_tolerance_zero_suppression = reader.read_raw_char()? as i16;
            // RC: DIMALTZ
            header.dim_alt_tolerance_zero_suppression = reader.read_raw_char()? as i16;
            // RC: DIMALTTZ
            header.dim_alt_tolerance_zero_tight = reader.read_raw_char()? as i16;
            // RC: DIMTAD
            header.dim_text_above = reader.read_raw_char()? as i16;
            // BS: DIMUNIT
            reader.read_bit_short()?; // dim_unit (R14 only, not stored)
            // BS: DIMAUNIT
            header.dim_angular_decimal_places = reader.read_bit_short()?;
            // BS: DIMDEC
            header.dim_decimal_places = reader.read_bit_short()?;
            // BS: DIMTDEC
            header.dim_tolerance_decimal_places = reader.read_bit_short()?;
            // BS: DIMALTU
            header.dim_alt_units_format = reader.read_bit_short()?;
            // BS: DIMALTTD
            header.dim_alt_tolerance_decimal_places = reader.read_bit_short()?;
            // H: DIMTXSTY (hard pointer)
            handles.set_dimtxsty(Some(reader.handle_reference()?));
        }

        // Common:
        // BD: DIMSCALE
        header.dim_scale = reader.read_bit_double()?;
        // BD: DIMASZ
        header.dim_arrow_size = reader.read_bit_double()?;
        // BD: DIMEXO
        header.dim_ext_line_offset = reader.read_bit_double()?;
        // BD: DIMDLI
        header.dim_line_increment = reader.read_bit_double()?;
        // BD: DIMEXE
        header.dim_ext_line_extension = reader.read_bit_double()?;
        // BD: DIMRND
        header.dim_rounding = reader.read_bit_double()?;
        // BD: DIMDLE
        header.dim_line_extension = reader.read_bit_double()?;
        // BD: DIMTP
        header.dim_tolerance_plus = reader.read_bit_double()?;
        // BD: DIMTM
        header.dim_tolerance_minus = reader.read_bit_double()?;

        // R2007+ Only:
        if sio.r2007_plus {
            // BD: DIMFXL
            header.dim_fixed_ext_line_length = reader.read_bit_double()?;
            // BD: DIMJOGANG
            let dim_jog_angle = reader.read_bit_double()?;
            let rounded = (dim_jog_angle * 1_000_000.0).round() / 1_000_000.0;
            // Clamp between ~5 degrees and ~90 degrees
            if rounded > 0.0873 && rounded < std::f64::consts::FRAC_PI_2 {
                header.dim_jog_angle = dim_jog_angle;
            }
            // BS: DIMTFILL
            header.dim_text_fill_mode = reader.read_bit_short()?;
            // CMC: DIMTFILLCLR
            header.dim_text_fill_color = reader.read_cm_color()?;
        }

        // R2000+ Only:
        if sio.r2000_plus {
            // B: DIMTOL
            header.dim_tolerance = reader.read_bit()?;
            // B: DIMLIM
            header.dim_limits = reader.read_bit()?;
            // B: DIMTIH
            header.dim_text_inside_horizontal = reader.read_bit()?;
            // B: DIMTOH
            header.dim_text_outside_horizontal = reader.read_bit()?;
            // B: DIMSE1
            header.dim_suppress_ext1 = reader.read_bit()?;
            // B: DIMSE2
            header.dim_suppress_ext2 = reader.read_bit()?;
            // BS: DIMTAD
            header.dim_text_above = reader.read_bit_short()?;
            // BS: DIMZIN
            header.dim_zero_suppression = reader.read_bit_short()?;
            // BS: DIMAZIN
            header.dim_alt_zero_suppression = reader.read_bit_short()?;
        }

        // R2007+ Only:
        if sio.r2007_plus {
            // BS: DIMARCSYM
            header.dim_arc_symbol_position = reader.read_bit_short()?;
        }

        // Common:
        // BD: DIMTXT
        header.dim_text_height = reader.read_bit_double()?;
        // BD: DIMCEN
        header.dim_center_mark = reader.read_bit_double()?;
        // BD: DIMTSZ
        header.dim_tick_size = reader.read_bit_double()?;
        // BD: DIMALTF
        header.dim_alt_scale = reader.read_bit_double()?;
        // BD: DIMLFAC
        header.dim_linear_scale = reader.read_bit_double()?;
        // BD: DIMTVP
        header.dim_text_vertical_pos = reader.read_bit_double()?;
        // BD: DIMTFAC
        header.dim_tolerance_scale = reader.read_bit_double()?;
        // BD: DIMGAP
        header.dim_line_gap = reader.read_bit_double()?;

        // R13-R14 Only:
        if sio.r13_14_only {
            // T: DIMPOST
            header.dim_post = reader.read_variable_text()?;
            // T: DIMAPOST
            header.dim_alt_post = reader.read_variable_text()?;
            // T: DIMBLK
            header.dim_arrow_block = reader.read_variable_text()?;
            // T: DIMBLK1
            header.dim_arrow_block1 = reader.read_variable_text()?;
            // T: DIMBLK2
            header.dim_arrow_block2 = reader.read_variable_text()?;
        }

        // R2000+ Only:
        if sio.r2000_plus {
            // BD: DIMALTRND
            header.dim_alt_rounding = reader.read_bit_double()?;
            // B: DIMALT
            header.dim_alternate_units = reader.read_bit()?;
            // BS: DIMALTD
            header.dim_alt_decimal_places = reader.read_bit_short()?;
            // B: DIMTOFL
            header.dim_force_line_inside = reader.read_bit()?;
            // B: DIMSAH
            header.dim_separate_arrows = reader.read_bit()?;
            // B: DIMTIX
            header.dim_force_text_inside = reader.read_bit()?;
            // B: DIMSOXD
            header.dim_suppress_outside_ext = reader.read_bit()?;
        }

        // Common:
        // CMC: DIMCLRD
        header.dim_line_color = reader.read_cm_color()?;
        // CMC: DIMCLRE
        header.dim_ext_line_color = reader.read_cm_color()?;
        // CMC: DIMCLRT
        header.dim_text_color = reader.read_cm_color()?;

        // R2000+ Only:
        if sio.r2000_plus {
            // BS: DIMADEC
            header.dim_angular_decimal_places = reader.read_bit_short()?;
            // BS: DIMDEC
            header.dim_decimal_places = reader.read_bit_short()?;
            // BS: DIMTDEC
            header.dim_tolerance_decimal_places = reader.read_bit_short()?;
            // BS: DIMALTU
            header.dim_alt_units_format = reader.read_bit_short()?;
            // BS: DIMALTTD
            header.dim_alt_tolerance_decimal_places = reader.read_bit_short()?;
            // BS: DIMAUNIT
            header.dim_angular_units = reader.read_bit_short()?;
            // BS: DIMFRAC
            header.dim_fraction_format = reader.read_bit_short()?;
            // BS: DIMLUNIT
            header.dim_linear_unit_format = reader.read_bit_short()?;
            // BS: DIMDSEP
            header.dim_decimal_separator = reader.read_bit_short()? as u8 as char;
            // BS: DIMTMOVE
            header.dim_text_movement = reader.read_bit_short()?;
            // BS: DIMJUST
            header.dim_horizontal_justification = reader.read_bit_short()?;
            // B: DIMSD1
            header.dim_suppress_line1 = reader.read_bit()?;
            // B: DIMSD2
            header.dim_suppress_line2 = reader.read_bit()?;
            // BS: DIMTOLJ
            header.dim_tolerance_justification = reader.read_bit_short()?;
            // BS: DIMTZIN
            header.dim_tolerance_zero_suppression = reader.read_bit_short()?;
            // BS: DIMALTZ
            header.dim_alt_tolerance_zero_suppression = reader.read_bit_short()?;
            // BS: DIMALTTZ
            header.dim_alt_tolerance_zero_tight = reader.read_bit_short()?;
            // B: DIMUPT
            header.dim_user_positioned_text = reader.read_bit()?;
            // BS: DIMATFIT
            header.dim_fit = reader.read_bit_short()?;
        }

        // R2007+ Only:
        if sio.r2007_plus {
            // B: DIMFXLON
            header.dim_ext_line_length_fixed = reader.read_bit()?;
        }

        // R2010+ Only:
        if sio.r2010_plus {
            // B: DIMTXTDIRECTION
            header.dim_text_direction = reader.read_bit()?;
            // BD: DIMALTMZF
            header.dim_alt_mzf = reader.read_bit_double()?;
            // T: DIMALTMZS
            header.dim_alt_mzs = reader.read_variable_text()?;
            // BD: DIMMZF
            header.dim_mzf = reader.read_bit_double()?;
            // T: DIMMZS
            header.dim_mzs = reader.read_variable_text()?;
        }

        // R2000+ Only:
        if sio.r2000_plus {
            // H: DIMTXSTY (hard pointer)
            handles.set_dimtxsty(Some(reader.handle_reference()?));
            // H: DIMLDRBLK (hard pointer)
            handles.set_dimldrblk(Some(reader.handle_reference()?));
            // H: DIMBLK (hard pointer)
            handles.set_dimblk(Some(reader.handle_reference()?));
            // H: DIMBLK1 (hard pointer)
            handles.set_dimblk1(Some(reader.handle_reference()?));
            // H: DIMBLK2 (hard pointer)
            handles.set_dimblk2(Some(reader.handle_reference()?));
        }

        // R2007+ Only:
        if sio.r2007_plus {
            // H: DIMLTYPE (hard pointer)
            handles.set_dimltype(Some(reader.handle_reference()?));
            // H: DIMLTEX1 (hard pointer)
            handles.set_dimltex1(Some(reader.handle_reference()?));
            // H: DIMLTEX2 (hard pointer)
            handles.set_dimltex2(Some(reader.handle_reference()?));
        }

        // R2000+ Only:
        if sio.r2000_plus {
            // BS: DIMLWD
            header.dim_line_weight = reader.read_bit_short()?;
            // BS: DIMLWE
            header.dim_ext_line_weight = reader.read_bit_short()?;
        }

        // ===================================================================
        // Table control object handles
        // ===================================================================

        // H: BLOCK CONTROL OBJECT (hard owner)
        handles.set_block_control_object(Some(reader.handle_reference()?));
        // H: LAYER CONTROL OBJECT (hard owner)
        handles.set_layer_control_object(Some(reader.handle_reference()?));
        // H: STYLE CONTROL OBJECT (hard owner)
        handles.set_style_control_object(Some(reader.handle_reference()?));
        // H: LINETYPE CONTROL OBJECT (hard owner)
        handles.set_linetype_control_object(Some(reader.handle_reference()?));
        // H: VIEW CONTROL OBJECT (hard owner)
        handles.set_view_control_object(Some(reader.handle_reference()?));
        // H: UCS CONTROL OBJECT (hard owner)
        handles.set_ucs_control_object(Some(reader.handle_reference()?));
        // H: VPORT CONTROL OBJECT (hard owner)
        handles.set_vport_control_object(Some(reader.handle_reference()?));
        // H: APPID CONTROL OBJECT (hard owner)
        handles.set_appid_control_object(Some(reader.handle_reference()?));
        // H: DIMSTYLE CONTROL OBJECT (hard owner)
        handles.set_dimstyle_control_object(Some(reader.handle_reference()?));

        // R13-R15 Only:
        if sio.r13_15_only {
            // H: VIEWPORT ENTITY HEADER CONTROL OBJECT (hard owner)
            handles.set_viewport_entity_header_control_object(Some(reader.handle_reference()?));
        }

        // Common:
        // H: DICTIONARY (ACAD_GROUP) (hard pointer)
        handles.set_dictionary_acad_group(Some(reader.handle_reference()?));
        // H: DICTIONARY (ACAD_MLINESTYLE) (hard pointer)
        handles.set_dictionary_acad_mlinestyle(Some(reader.handle_reference()?));
        // H: DICTIONARY (NAMED OBJECTS) (hard owner)
        handles.set_dictionary_named_objects(Some(reader.handle_reference()?));

        // R2000+ Only:
        if sio.r2000_plus {
            // BS: TSTACKALIGN, default = 1 (not present in DXF)
            header.stacked_text_alignment = reader.read_bit_short()?;
            // BS: TSTACKSIZE, default = 70 (not present in DXF)
            header.stacked_text_size_percentage = reader.read_bit_short()?;

            // TV: HYPERLINKBASE
            header.hyperlink_base = reader.read_variable_text()?;
            // TV: STYLESHEET
            header.stylesheet = reader.read_variable_text()?;

            // H: DICTIONARY (LAYOUTS) (hard pointer)
            handles.set_dictionary_layouts(Some(reader.handle_reference()?));
            // H: DICTIONARY (PLOTSETTINGS) (hard pointer)
            handles.set_dictionary_plotsettings(Some(reader.handle_reference()?));
            // H: DICTIONARY (PLOTSTYLES) (hard pointer)
            handles.set_dictionary_plotstyles(Some(reader.handle_reference()?));
        }

        // R2004+:
        if sio.r2004_plus {
            // H: DICTIONARY (MATERIALS) (hard pointer)
            handles.set_dictionary_materials(Some(reader.handle_reference()?));
            // H: DICTIONARY (COLORS) (hard pointer)
            handles.set_dictionary_colors(Some(reader.handle_reference()?));
        }

        // R2007+:
        if sio.r2007_plus {
            // H: DICTIONARY (VISUALSTYLE) (hard pointer)
            handles.set_dictionary_visualstyle(Some(reader.handle_reference()?));

            // R2013+:
            if sio.r2013_plus {
                // H: UNKNOWN (hard pointer) - overwrites DICTIONARY_VISUALSTYLE
                handles.set_dictionary_visualstyle(Some(reader.handle_reference()?));
            }
        }

        // ===================================================================
        // R2000+ options flags word and final settings
        // ===================================================================

        // R2000+:
        if sio.r2000_plus {
            // BL: Flags
            let flags = reader.read_bit_long()?;
            // CELWEIGHT Flags & 0x001F
            header.current_line_weight = (flags & 0x1F) as i16;
            // ENDCAPS Flags & 0x0060
            header.end_caps = (flags & 0x60) as i16;
            // JOINSTYLE Flags & 0x0180
            header.join_style = (flags & 0x180) as i16;
            // LWDISPLAY !(Flags & 0x0200)
            header.lineweight_display = (flags & 0x200) == 1;
            // XEDIT !(Flags & 0x0400)
            header.xedit = (flags & 0x400) == 1;
            // EXTNAMES Flags & 0x0800
            header.extended_names = (flags & 0x800) != 0;
            // PSTYLEMODE Flags & 0x2000
            header.plotstyle_mode = (flags & 0x2000) != 0;
            // OLESTARTUP Flags & 0x4000
            header.ole_startup = (flags & 0x4000) != 0;

            // BS: INSUNITS
            header.insertion_units = reader.read_bit_short()?;
            // BS: CEPSNTYPE
            header.current_plotstyle_type = reader.read_bit_short()?;

            if header.current_plotstyle_type == 3 {
                // H: CPSNID (present only if CEPSNTYPE == 3) (hard pointer)
                handles.set_cpsnid(Some(reader.handle_reference()?));
            }

            // TV: FINGERPRINTGUID
            header.fingerprint_guid = reader.read_variable_text()?;
            // TV: VERSIONGUID
            header.version_guid = reader.read_variable_text()?;
        }

        // R2004+:
        if sio.r2004_plus {
            // RC: SORTENTS
            header.sort_entities = reader.read_byte()? as i16;
            // RC: INDEXCTL
            header.index_control = reader.read_byte()? as i16;
            // RC: HIDETEXT
            header.hide_text = reader.read_byte()? as i16;
            // RC: XCLIPFRAME, before R2010 the value can be 0 or 1 only
            header.xclip_frame = reader.read_byte()? as i16;
            // RC: DIMASSOC
            header.dimension_associativity = reader.read_byte()? as i16;
            // RC: HALOGAP
            header.halo_gap = reader.read_byte()? as i16;
            // BS: OBSCUREDCOLOR
            header.obscured_color = reader.read_bit_short()?;
            // BS: INTERSECTIONCOLOR
            header.intersection_color = reader.read_bit_short()?;
            // RC: OBSCUREDLTYPE
            header.obscured_linetype = reader.read_byte()? as i16;
            // RC: INTERSECTIONDISPLAY
            header.intersection_display = reader.read_byte()? as i16;

            // TV: PROJECTNAME
            header.project_name = reader.read_variable_text()?;
        }

        // ===================================================================
        // Common block record handles
        // ===================================================================

        // H: BLOCK_RECORD (*PAPER_SPACE) (hard pointer)
        handles.set_paper_space(Some(reader.handle_reference()?));
        // H: BLOCK_RECORD (*MODEL_SPACE) (hard pointer)
        handles.set_model_space(Some(reader.handle_reference()?));
        // H: LTYPE (BYLAYER) (hard pointer)
        handles.set_bylayer(Some(reader.handle_reference()?));
        // H: LTYPE (BYBLOCK) (hard pointer)
        handles.set_byblock(Some(reader.handle_reference()?));
        // H: LTYPE (CONTINUOUS) (hard pointer)
        handles.set_continuous(Some(reader.handle_reference()?));

        // ===================================================================
        // R2007+ camera/3D/light/geo settings
        // ===================================================================

        // R2007+:
        if sio.r2007_plus {
            // B: CAMERADISPLAY
            header.camera_display = reader.read_bit()?;

            // BL: unknown
            reader.read_bit_long()?;
            // BL: unknown
            reader.read_bit_long()?;
            // BD: unknown
            reader.read_bit_double()?;

            // BD: STEPSPERSEC
            let steps_per_second = reader.read_bit_double()?;
            if steps_per_second >= 1.0 && steps_per_second <= 30.0 {
                header.steps_per_second = steps_per_second;
            }
            // BD: STEPSIZE
            header.step_size = reader.read_bit_double()?;
            // BD: 3DDWFPREC
            header.dw3d_precision = reader.read_bit_double()?;
            // BD: LENSLENGTH
            header.lens_length = reader.read_bit_double()?;
            // BD: CAMERAHEIGHT
            header.camera_height = reader.read_bit_double()?;
            // RC: SOLIDHIST
            header.solids_retain_history = reader.read_raw_char()? as i16;
            // RC: SHOWHIST
            header.show_solids_history = reader.read_raw_char()? as i16;
            // BD: PSOLWIDTH
            header.swept_solid_width = reader.read_bit_double()?;
            // BD: PSOLHEIGHT
            header.swept_solid_height = reader.read_bit_double()?;
            // BD: LOFTANG1
            header.loft_angle1 = reader.read_bit_double()?;
            // BD: LOFTANG2
            header.loft_angle2 = reader.read_bit_double()?;
            // BD: LOFTMAG1
            header.loft_magnitude1 = reader.read_bit_double()?;
            // BD: LOFTMAG2
            header.loft_magnitude2 = reader.read_bit_double()?;
            // BS: LOFTPARAM
            header.loft_param = reader.read_bit_short()?;
            // RC: LOFTNORMALS
            header.loft_normals = reader.read_raw_char()? as i16;
            // BD: LATITUDE
            header.latitude = reader.read_bit_double()?;
            // BD: LONGITUDE
            header.longitude = reader.read_bit_double()?;
            // BD: NORTHDIRECTION
            header.north_direction = reader.read_bit_double()?;
            // BL: TIMEZONE
            header.timezone = reader.read_bit_long()?;
            // RC: LIGHTGLYPHDISPLAY
            header.light_glyph_display = reader.read_raw_char()? as i16;
            // RC: TILEMODELIGHTSYNCH ??
            header.tile_model_light_synch = reader.read_raw_char()? as i16;
            // RC: DWFFRAME
            header.dwf_frame = reader.read_raw_char()? as i16;
            // RC: DGNFRAME
            header.dgn_frame = reader.read_raw_char()? as i16;

            // B: unknown
            reader.read_bit()?;

            // CMC: INTERFERECOLOR
            header.intersection_color = {
                let c = reader.read_cm_color()?;
                match c {
                    crate::types::Color::Index(i) => i as i16,
                    _ => 257,
                }
            };

            // H: INTERFEREOBJVS (hard pointer)
            handles.set_interfereobjvs(Some(reader.handle_reference()?));
            // H: INTERFEREVPVS (hard pointer)
            handles.set_interferevpvs(Some(reader.handle_reference()?));
            // H: DRAGVS (hard pointer)
            handles.set_dragvs(Some(reader.handle_reference()?));

            // RC: CSHADOW
            header.shadow_mode = reader.read_byte()? as i16;
            // BD: SHADOWPLANELOCATION
            header.shadow_plane_location = reader.read_bit_double()?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_reader_creation() {
        let reader = DwgHeaderReader::new(DxfVersion::AC1015, vec![]);
        assert_eq!(reader.version, DxfVersion::AC1015);
    }
}
