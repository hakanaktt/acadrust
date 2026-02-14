//! Table control object and table entry readers for the DWG object reader.
//!
//! Mirrors ACadSharp's `DwgObjectReader` table-related methods.

use crate::error::Result;

use super::templates::*;
use super::{DwgObjectReader, StreamSet};

impl DwgObjectReader {
    // -----------------------------------------------------------------------
    // Table control objects (generic)
    // -----------------------------------------------------------------------

    /// Read a table control object (BLOCK_CONTROL, LAYER_CONTROL, etc.).
    ///
    /// Corresponds to ACadSharp `readDocumentTable()`.
    pub(super) fn read_table_control(
        &mut self,
        streams: &mut StreamSet,
        table_type: TableControlType,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        // BL: number of entries.
        let num_entries = streams.object_reader.read_bit_long()? as usize;

        // Entry handles (soft owner).
        let mut table_data = CadTableTemplateData::default();
        for _ in 0..num_entries {
            let h = streams.handle_ref()?;
            table_data.entry_handles.push(h);
        }

        Ok(CadTemplate::TableControl {
            common: common_tmpl,
            table_data,
            table_type,
        })
    }

    /// Read the BLOCK_CONTROL object (special: has extra Model_Space/Paper_Space handles).
    ///
    /// The DWG spec stores *Model_Space and *Paper_Space handles separately
    /// after the regular block entry handles.
    pub(super) fn read_block_control(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        // BL: number of entries (excludes *Model_Space and *Paper_Space).
        let num_entries = streams.object_reader.read_bit_long()? as usize;

        // Regular entry handles (soft owner).
        let mut table_data = CadTableTemplateData::default();
        for _ in 0..num_entries {
            let h = streams.handle_ref()?;
            table_data.entry_handles.push(h);
        }

        // *Model_Space handle (hard owner) — add to entry list.
        let model_space = streams.handle_ref()?;
        table_data.entry_handles.push(model_space);

        // *Paper_Space handle (hard owner) — add to entry list.
        let paper_space = streams.handle_ref()?;
        table_data.entry_handles.push(paper_space);

        Ok(CadTemplate::TableControl {
            common: common_tmpl,
            table_data,
            table_type: TableControlType::BlockControl,
        })
    }

    /// Read the LTYPE_CONTROL object (special: has extra ByLayer/ByBlock handles).
    pub(super) fn read_ltype_control(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        // BL: number of entries.
        let num_entries = streams.object_reader.read_bit_long()? as usize;

        let mut table_data = CadTableTemplateData::default();
        for _ in 0..num_entries {
            let h = streams.handle_ref()?;
            table_data.entry_handles.push(h);
        }

        // ByLayer handle.
        let _bylayer = streams.handle_ref()?;
        // ByBlock handle.
        let _byblock = streams.handle_ref()?;

        Ok(CadTemplate::TableControl {
            common: common_tmpl,
            table_data,
            table_type: TableControlType::LineTypeControl,
        })
    }

    // -----------------------------------------------------------------------
    // BLOCK_HEADER (BLOCK_RECORD) table entry
    // -----------------------------------------------------------------------

    pub(super) fn read_block_header(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        let mut block_data = CadBlockRecordTemplateData::default();

        // Name (TV).
        let _name = streams.read_text()?;

        // Xref dependent (B) — matches C# readXrefDependantBit.
        let _xref_dep = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // Anonymous (B).
        let _anonymous = streams.object_reader.read_bit()?;

        // Has attributes (B).
        let _has_attr = streams.object_reader.read_bit()?;

        // Is xref (B).
        let is_xref = streams.object_reader.read_bit()?;

        // Is xref overlay (B).
        let is_xref_overlay = streams.object_reader.read_bit()?;

        // R2000+: load xref (B).
        if self.sio.r2000_plus {
            let _loaded = streams.object_reader.read_bit()?;
        }

        // R2004+: owned object count (BL) — only if NOT xref/overlay.
        let owned_count = if self.sio.r2004_plus && !is_xref && !is_xref_overlay {
            streams.object_reader.read_bit_long()? as usize
        } else {
            0
        };

        // Base point (3BD).
        let _base_point = streams.object_reader.read_3bit_double()?;

        // Xref path name (TV).
        let _xref_path = streams.read_text()?;

        // R2000+: insert count, description, preview data.
        let mut insert_count = 0usize;
        if self.sio.r2000_plus {
            // Insert Count: a sequence of non-zero RC's followed by a terminating 0 RC.
            loop {
                let rc = streams.object_reader.read_raw_char()?;
                if rc == 0 {
                    break;
                }
                insert_count += 1;
            }

            // Block Description (TV).
            let _description = streams.read_text()?;

            // Size of preview data (BL) + bytes.
            let preview_size = streams.object_reader.read_bit_long()? as usize;
            if preview_size > 0 {
                let _preview = streams.object_reader.read_bytes(preview_size)?;
            }
        }

        // R2007+: insert units (BS), explodable (B), block scaling (RC).
        if self.sio.r2007_plus {
            let _units = streams.object_reader.read_bit_short()?;
            let _explodable = streams.object_reader.read_bit()?;
            let _can_scale = streams.object_reader.read_raw_char()?;
        }

        // --- Handle references (from handles_reader) ---

        // Block control handle (NULL, hard pointer).
        let _block_ctrl = streams.handle_ref()?;

        // Block entity handle (BeginBlock, hard owner).
        block_data.block_entity_handle = streams.handle_ref()?;

        // R13-R2000 (not xref): first and last entity handles.
        if !self.sio.r2004_plus && !is_xref && !is_xref_overlay {
            block_data.first_entity_handle = streams.handle_ref()?;
            block_data.last_entity_handle = streams.handle_ref()?;
        }

        // R2004+ (not xref): owned object handles.
        if self.sio.r2004_plus && !is_xref && !is_xref_overlay {
            for _ in 0..owned_count {
                let h = streams.handle_ref()?;
                block_data.owned_object_handles.push(h);
            }
        }

        // End block entity handle (ENDBLK, hard owner).
        block_data.end_block_handle = streams.handle_ref()?;

        // R2000+: insert handles + layout handle.
        if self.sio.r2000_plus {
            for _ in 0..insert_count {
                let _insert = streams.handle_ref()?;
            }
            // Layout Handle (hard pointer).
            block_data.layout_handle = streams.handle_ref()?;
        }

        Ok(CadTemplate::BlockHeader {
            common: common_tmpl,
            block_data,
        })
    }

    // -----------------------------------------------------------------------
    // LAYER table entry
    // -----------------------------------------------------------------------

    pub(super) fn read_layer(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        let mut layer_data = CadLayerTemplateData::default();

        // Name (TV).
        layer_data.name = streams.read_text()?;

        // Xref dependent (B) — matches C# readXrefDependantBit.
        let _is_xref = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // R13-R14: frozen (B), on (B), frozen-in-new (B), locked (B).
        if self.sio.r13_14_only {
            layer_data.frozen = streams.object_reader.read_bit()?;
            layer_data.is_on = streams.object_reader.read_bit()?;
            let _frozen_new = streams.object_reader.read_bit()?;
            layer_data.locked = streams.object_reader.read_bit()?;
            layer_data.is_plottable = true; // default for R13-R14
        }

        // R2000+: Values (BS) containing layer flags + color + lineweight.
        if self.sio.r2000_plus {
            let values = streams.object_reader.read_bit_short()?;
            layer_data.frozen = (values & 0x01) != 0;
            layer_data.is_on = (values & 0x02) == 0; // bit 1 set = off
            // bit 2 = frozen in new VP (not stored separately)
            layer_data.locked = (values & 0x08) != 0;
            layer_data.is_plottable = (values & 0x10) != 0;
            layer_data.line_weight_raw = ((values & 0x3E0) >> 5) as u8;
        }

        // Color (CMC).
        layer_data.color = streams.object_reader.read_cm_color()?;

        // External reference block handle (hard pointer).
        // (Misnamed as "layer control handle" in some references; this is
        //  actually the xref block handle per ACadSharp/ODA spec.)
        let _xref_block = streams.handle_ref()?;

        // R2000+: plotstyle handle.
        if self.sio.r2000_plus {
            layer_data.plotstyle_handle = streams.handle_ref()?;
        }

        // R2007+: material handle.
        if self.sio.r2007_plus {
            layer_data.material_handle = streams.handle_ref()?;
        }

        // Linetype handle.
        layer_data.linetype_handle = streams.handle_ref()?;

        Ok(CadTemplate::LayerEntry {
            common: common_tmpl,
            layer_data,
        })
    }

    // -----------------------------------------------------------------------
    // TEXT_STYLE table entry
    // -----------------------------------------------------------------------

    pub(super) fn read_text_style(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        let mut ts_data = CadTextStyleTemplateData::default();

        // Name (TV).
        ts_data.name = streams.read_text()?;

        // Xref dependent (B) — matches C# readXrefDependantBit.
        let _is_xref = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // Is vertical (B).
        ts_data.is_vertical = streams.object_reader.read_bit()?;

        // Is shape file (B).
        ts_data.is_shape_file = streams.object_reader.read_bit()?;

        // Fixed height (BD).
        ts_data.height = streams.object_reader.read_bit_double()?;

        // Width factor (BD).
        ts_data.width_factor = streams.object_reader.read_bit_double()?;

        // Oblique angle (BD).
        ts_data.oblique_angle = streams.object_reader.read_bit_double()?;

        // Generation flags (RC).
        ts_data.gen_flags = streams.object_reader.read_raw_char()?;

        // Last height used (BD).
        ts_data.last_height = streams.object_reader.read_bit_double()?;

        // Font file name (TV).
        ts_data.font_file = streams.read_text()?;

        // Big font file name (TV).
        ts_data.big_font_file = streams.read_text()?;

        // Style control object handle.
        ts_data.style_control_handle = streams.handle_ref()?;

        Ok(CadTemplate::TextStyleEntry {
            common: common_tmpl,
            textstyle_data: ts_data,
        })
    }

    // -----------------------------------------------------------------------
    // LTYPE table entry
    // -----------------------------------------------------------------------

    pub(super) fn read_ltype(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        let mut ltype_data = CadLineTypeTemplateData::default();

        // Name (TV).
        ltype_data.name = streams.read_text()?;

        // Xref dependent (B) — matches C# readXrefDependantBit.
        let _is_xref = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // Description (TV).
        ltype_data.description = streams.read_text()?;

        // Pattern length (BD).
        ltype_data.total_len = streams.object_reader.read_bit_double()?;

        // Alignment (RC).
        ltype_data.alignment = streams.object_reader.read_raw_char()?;

        // Number of dashes (RC).
        let num_dashes = streams.object_reader.read_raw_char()? as usize;

        for _i in 0..num_dashes {
            // Dash length (BD).
            let dash_length = streams.object_reader.read_bit_double()?;
            ltype_data.dash_lengths.push(dash_length);

            // Complex shape code (BS).
            let shape_code = streams.object_reader.read_bit_short()?;

            // X offset (RD).
            let _x_offset = streams.object_reader.read_raw_double()?;

            // Y offset (RD).
            let _y_offset = streams.object_reader.read_raw_double()?;

            // Scale (BD).
            let _scale = streams.object_reader.read_bit_double()?;

            // Rotation (BD).
            let _rotation = streams.object_reader.read_bit_double()?;

            // Shape flag (BS).
            let _shape_flag = streams.object_reader.read_bit_short()?;

            // R2004+: has text.
            if self.sio.r2004_plus && shape_code != 0 {
                // Segment text (TV or null if not text).
                // This reads the text area for text-in-linetype segments.
            }
        }

        // R2004+: segment handles (string area).
        // Segment text strings.
        let _strings_area = if self.sio.r2004_plus {
            // In R2004+, there is a strings area for shape text.
            for _ in 0..num_dashes {
                // Text string (TV) for the segment (empty if none).
                let _seg_text = streams.read_text()?;
            }
            true
        } else {
            false
        };

        // LType control object handle.
        ltype_data.ltype_control_handle = streams.handle_ref()?;

        // Segment handles (shape file refs).
        for _ in 0..num_dashes {
            let h = streams.handle_ref()?;
            ltype_data.segment_handles.push(h);
        }

        Ok(CadTemplate::LineTypeEntry {
            common: common_tmpl,
            ltype_data,
        })
    }

    // -----------------------------------------------------------------------
    // VIEW table entry
    // -----------------------------------------------------------------------

    pub(super) fn read_view(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        let mut view_data = CadViewTemplateData::default();

        // Name (TV).
        view_data.name = streams.read_text()?;

        // Xref dependent (B) — matches C# readXrefDependantBit.
        let _is_xref = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // View height (BD).
        view_data.height = streams.object_reader.read_bit_double()?;

        // View width (BD).
        view_data.width = streams.object_reader.read_bit_double()?;

        // View center (2RD).
        let center = streams.object_reader.read_2raw_double()?;
        view_data.center = center;

        // View target (3BD).
        view_data.target = streams.object_reader.read_3bit_double()?;

        // View direction (3BD).
        view_data.direction = streams.object_reader.read_3bit_double()?;

        // Twist angle (BD).
        view_data.twist_angle = streams.object_reader.read_bit_double()?;

        // Lens length (BD).
        view_data.lens_length = streams.object_reader.read_bit_double()?;

        // Front clip (BD).
        view_data.front_clip = streams.object_reader.read_bit_double()?;

        // Back clip (BD).
        view_data.back_clip = streams.object_reader.read_bit_double()?;

        // UCSFOLLOW (B).
        let _ufi = streams.object_reader.read_bit()?;

        // Front on (B) + Back on (B).
        let _front_on = streams.object_reader.read_bit()?;
        let _back_on = streams.object_reader.read_bit()?;

        // Render mode / UCS at Origin (R2000+).
        let mut associated_ucs = false;
        if self.sio.r2000_plus {
            let _render_mode = streams.object_reader.read_raw_char()?;
            associated_ucs = streams.object_reader.read_bit()?;
            // UCS info (always present when R2000+, regardless of associated flag).
            let _ucs_origin = streams.object_reader.read_3bit_double()?;
            let _ucs_x = streams.object_reader.read_3bit_double()?;
            let _ucs_y = streams.object_reader.read_3bit_double()?;
            let _ucs_elevation = streams.object_reader.read_bit_double()?;
            let _ucs_ortho = streams.object_reader.read_bit_short()?;
        }

        // View control object handle (external reference block handle).
        let _ctrl_handle = streams.handle_ref()?;

        if self.sio.r2007_plus {
            // Camera plottable (B) — read from object_reader, after ctrl handle.
            let _cam = streams.object_reader.read_bit()?;
            // Background handle (H 332 soft pointer).
            let _background = streams.handle_ref()?;
            // Visual style handle (H 348 hard pointer).
            let _visual_style = streams.handle_ref()?;
            // Sun handle (H 361 hard owner).
            let _sun = streams.handle_ref()?;
        }

        if self.sio.r2000_plus && associated_ucs {
            // UCS handle (H 346).
            view_data.ucs_handle = streams.handle_ref()?;
            // Named UCS handle (H 345).
            let _named_ucs = streams.handle_ref()?;
        }

        if self.sio.r2007_plus {
            // Live section handle (H 334 soft pointer).
            let _live_section = streams.handle_ref()?;
        }

        Ok(CadTemplate::ViewEntry {
            common: common_tmpl,
            view_data,
        })
    }

    // -----------------------------------------------------------------------
    // UCS table entry
    // -----------------------------------------------------------------------

    pub(super) fn read_ucs(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        let mut ucs_data = CadUcsTemplateData::default();

        // Name (TV).
        ucs_data.name = streams.read_text()?;

        // Xref dependent (B) — matches C# readXrefDependantBit.
        let _is_xref = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // Origin (3BD).
        ucs_data.origin = streams.object_reader.read_3bit_double()?;

        // X direction (3BD).
        ucs_data.x_dir = streams.object_reader.read_3bit_double()?;

        // Y direction (3BD).
        ucs_data.y_dir = streams.object_reader.read_3bit_double()?;

        // R2000+: elevation, ortho type, num ortho origins.
        if self.sio.r2000_plus {
            let _elevation = streams.object_reader.read_bit_double()?;
            let _ortho_type = streams.object_reader.read_bit_short()?;
            // Ortho origin array (BC count + 3BD each).
            // Actually: BS ortho_view_type first, then origin
        }

        // UCS control object handle (external reference block handle).
        ucs_data.ucs_control_handle = streams.handle_ref()?;

        // R2000+: Base UCS handle, Named UCS handle.
        if self.sio.r2000_plus {
            let _base_ucs = streams.handle_ref()?;
            let _named_ucs = streams.handle_ref()?;
        }

        Ok(CadTemplate::UcsEntry {
            common: common_tmpl,
            ucs_data,
        })
    }

    // -----------------------------------------------------------------------
    // VPORT table entry
    // -----------------------------------------------------------------------

    pub(super) fn read_vport(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        let mut vport_data = CadVPortTemplateData::default();

        // Name (TV).
        vport_data.name = streams.read_text()?;

        // Xref dependent (B) — matches C# readXrefDependantBit.
        let _is_xref = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // View height (BD).
        let _view_height = streams.object_reader.read_bit_double()?;

        // Aspect ratio (BD).
        let _aspect_ratio = streams.object_reader.read_bit_double()?;

        // View center (2RD).
        let _view_center = streams.object_reader.read_2raw_double()?;

        // View target (3BD).
        let _view_target = streams.object_reader.read_3bit_double()?;

        // View direction (3BD).
        let _view_direction = streams.object_reader.read_3bit_double()?;

        // Twist angle (BD).
        let _twist = streams.object_reader.read_bit_double()?;

        // Lens length (BD).
        let _lens = streams.object_reader.read_bit_double()?;

        // Front clip (BD).
        let _front_clip = streams.object_reader.read_bit_double()?;

        // Back clip (BD).
        let _back_clip = streams.object_reader.read_bit_double()?;

        // View mode (UCSFOLLOW + FRONT_ON + BACK_ON + PERSP + etc.).
        let _view_mode = streams.object_reader.read_bit_long()?;

        // Render mode (RC).
        let _render_mode = streams.object_reader.read_raw_char()?;

        // R2000+: additional settings.
        if self.sio.r2000_plus {
            // Use default lighting (B).
            let _default_lighting = streams.object_reader.read_bit()?;

            // Default lighting type (RC).
            let _dl_type = streams.object_reader.read_raw_char()?;

            // Brightness (BD).
            let _brightness = streams.object_reader.read_bit_double()?;

            // Contrast (BD).
            let _contrast = streams.object_reader.read_bit_double()?;

            // Ambient color (RL).
            let _ambient = streams.object_reader.read_raw_long()?;
        }

        // Lower left corner (2RD).
        let _lower_left = streams.object_reader.read_2raw_double()?;

        // Upper right corner (2RD).
        let _upper_right = streams.object_reader.read_2raw_double()?;

        // UCSFOLLOW (B).
        let _ucsfollow = streams.object_reader.read_bit()?;

        // Circle sides (BS).
        let _circle_sides = streams.object_reader.read_bit_short()?;

        // Snap (B/BD).
        let _snap_on = streams.object_reader.read_bit()?;
        let _snap_style = streams.object_reader.read_bit()?;
        let _snap_isopair = streams.object_reader.read_bit_short()?;
        let _snap_rotation = streams.object_reader.read_bit_double()?;
        let _snap_base = streams.object_reader.read_2raw_double()?;
        let _snap_spacing = streams.object_reader.read_2raw_double()?;

        // Grid (B/BD).
        let _grid_on = streams.object_reader.read_bit()?;
        let _grid_spacing = streams.object_reader.read_2raw_double()?;

        // UCS flags (R2000+).
        if self.sio.r2000_plus {
            // UCS at origin (B).
            let _ucs_per_vp = streams.object_reader.read_bit()?;
            let _ucs_origin = streams.object_reader.read_3bit_double()?;
            let _ucs_x = streams.object_reader.read_3bit_double()?;
            let _ucs_y = streams.object_reader.read_3bit_double()?;
            let _ucs_elevation = streams.object_reader.read_bit_double()?;
            let _ucs_ortho_type = streams.object_reader.read_bit_short()?;
        }

        if self.sio.r2007_plus {
            // Grid flags (BS).
            let _grid_flags = streams.object_reader.read_bit_short()?;
            let _grid_major = streams.object_reader.read_bit_short()?;
        }

        // VPort control object handle.
        vport_data.vport_control_handle = streams.handle_ref()?;

        // R2007+: background, visual style, sun handles.
        if self.sio.r2007_plus {
            vport_data.background_handle = streams.handle_ref()?;
            vport_data.style_handle = streams.handle_ref()?;
            vport_data.sun_handle = streams.handle_ref()?;
        }

        if self.sio.r2000_plus {
            vport_data.named_ucs_handle = streams.handle_ref()?;
            vport_data.base_ucs_handle = streams.handle_ref()?;
        }

        Ok(CadTemplate::VPortEntry {
            common: common_tmpl,
            vport_data,
        })
    }

    // -----------------------------------------------------------------------
    // APPID table entry
    // -----------------------------------------------------------------------

    pub(super) fn read_appid(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        let mut appid_data = CadAppIdTemplateData::default();

        // Name (TV).
        appid_data.name = streams.read_text()?;

        // Xref dependent (B) — matches C# readXrefDependantBit.
        let _is_xref = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // Unknown (RC).
        let _unknown = streams.object_reader.read_raw_char()?;

        // AppId control object handle.
        appid_data.appid_control_handle = streams.handle_ref()?;

        Ok(CadTemplate::AppIdEntry {
            common: common_tmpl,
            appid_data,
        })
    }

    // -----------------------------------------------------------------------
    // DIMSTYLE table entry
    // -----------------------------------------------------------------------

    pub(super) fn read_dimstyle(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        let mut dimstyle_data = CadDimStyleTemplateData::default();

        // Name (TV).
        dimstyle_data.name = streams.read_text()?;

        // Xref dependent (B) — matches C# readXrefDependantBit.
        let _is_xref = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // All the dimension style variables (R13-R14 layout).
        if self.sio.r13_14_only {
            // R13-R14: all variables in a specific order.
            let _dimtol = streams.object_reader.read_bit()?;
            let _dimlim = streams.object_reader.read_bit()?;
            let _dimtih = streams.object_reader.read_bit()?;
            let _dimtoh = streams.object_reader.read_bit()?;
            let _dimse1 = streams.object_reader.read_bit()?;
            let _dimse2 = streams.object_reader.read_bit()?;
            let _dimalt = streams.object_reader.read_bit()?;
            let _dimtofl = streams.object_reader.read_bit()?;
            let _dimsah = streams.object_reader.read_bit()?;
            let _dimtix = streams.object_reader.read_bit()?;
            let _dimsoxd = streams.object_reader.read_bit()?;
            let _dimaltd = streams.object_reader.read_raw_char()?;
            let _dimzin = streams.object_reader.read_raw_char()?;
            let _dimsd1 = streams.object_reader.read_bit()?;
            let _dimsd2 = streams.object_reader.read_bit()?;
            let _dimtolj = streams.object_reader.read_raw_char()?;
            let _dimjust = streams.object_reader.read_raw_char()?;
            let _dimfit = streams.object_reader.read_raw_char()?;
            let _dimupt = streams.object_reader.read_bit()?;
            let _dimtzin = streams.object_reader.read_raw_char()?;
            let _dimaltz = streams.object_reader.read_raw_char()?;
            let _dimalttz = streams.object_reader.read_raw_char()?;
            let _dimtad = streams.object_reader.read_raw_char()?;
            let _dimunit = streams.object_reader.read_bit_short()?;
            let _dimaunit = streams.object_reader.read_bit_short()?;
            let _dimdec = streams.object_reader.read_bit_short()?;
            let _dimtdec = streams.object_reader.read_bit_short()?;
            let _dimaltu = streams.object_reader.read_bit_short()?;
            let _dimalttd = streams.object_reader.read_bit_short()?;
            dimstyle_data.dimtxsty_handle = 0; // read from handles later
            let _dimscale = streams.object_reader.read_bit_double()?;
            let _dimasz = streams.object_reader.read_bit_double()?;
            let _dimexo = streams.object_reader.read_bit_double()?;
            let _dimdli = streams.object_reader.read_bit_double()?;
            let _dimexe = streams.object_reader.read_bit_double()?;
            let _dimrnd = streams.object_reader.read_bit_double()?;
            let _dimdle = streams.object_reader.read_bit_double()?;
            let _dimtp = streams.object_reader.read_bit_double()?;
            let _dimtm = streams.object_reader.read_bit_double()?;
            let _dimtxt = streams.object_reader.read_bit_double()?;
            let _dimcen = streams.object_reader.read_bit_double()?;
            let _dimtsz = streams.object_reader.read_bit_double()?;
            let _dimaltf = streams.object_reader.read_bit_double()?;
            let _dimlfac = streams.object_reader.read_bit_double()?;
            let _dimtvp = streams.object_reader.read_bit_double()?;
            let _dimtfac = streams.object_reader.read_bit_double()?;
            let _dimgap = streams.object_reader.read_bit_double()?;
            let _dimpost = streams.read_text()?;
            let _dimapost = streams.read_text()?;
            dimstyle_data.dimblk_name = streams.read_text()?;
            dimstyle_data.dimblk1_name = streams.read_text()?;
            dimstyle_data.dimblk2_name = streams.read_text()?;
            let _dimclrd = streams.object_reader.read_bit_short()?;
            let _dimclre = streams.object_reader.read_bit_short()?;
            let _dimclrt = streams.object_reader.read_bit_short()?;
        }

        // R2000+ layout.
        if self.sio.r2000_plus {
            let _dimpost = streams.read_text()?;
            let _dimapost = streams.read_text()?;
            let _dimscale = streams.object_reader.read_bit_double()?;
            let _dimasz = streams.object_reader.read_bit_double()?;
            let _dimexo = streams.object_reader.read_bit_double()?;
            let _dimdli = streams.object_reader.read_bit_double()?;
            let _dimexe = streams.object_reader.read_bit_double()?;
            let _dimrnd = streams.object_reader.read_bit_double()?;
            let _dimdle = streams.object_reader.read_bit_double()?;
            let _dimtp = streams.object_reader.read_bit_double()?;
            let _dimtm = streams.object_reader.read_bit_double()?;

            // R2007+: dimfxl, dimfxlon, dimjogang, dimtfill, dimtfillclr.
            if self.sio.r2007_plus {
                let _dimfxl = streams.object_reader.read_bit_double()?;
                let _dimfxlon = streams.object_reader.read_bit()?;
                let _dimjogang = streams.object_reader.read_bit_double()?;
                let _dimtfill = streams.object_reader.read_bit_short()?;
                let _dimtfillclr = streams.object_reader.read_cm_color()?;
            }

            let _dimtol = streams.object_reader.read_bit()?;
            let _dimlim = streams.object_reader.read_bit()?;
            let _dimtih = streams.object_reader.read_bit()?;
            let _dimtoh = streams.object_reader.read_bit()?;
            let _dimse1 = streams.object_reader.read_bit()?;
            let _dimse2 = streams.object_reader.read_bit()?;
            let _dimtad = streams.object_reader.read_bit_short()?;
            let _dimzin = streams.object_reader.read_bit_short()?;
            let _dimazin = streams.object_reader.read_bit_short()?;

            // R2007+: dimarcsym.
            if self.sio.r2007_plus {
                let _dimarcsym = streams.object_reader.read_bit_short()?;
            }

            let _dimtxt = streams.object_reader.read_bit_double()?;
            let _dimcen = streams.object_reader.read_bit_double()?;
            let _dimtsz = streams.object_reader.read_bit_double()?;
            let _dimaltf = streams.object_reader.read_bit_double()?;
            let _dimlfac = streams.object_reader.read_bit_double()?;
            let _dimtvp = streams.object_reader.read_bit_double()?;
            let _dimtfac = streams.object_reader.read_bit_double()?;
            let _dimgap = streams.object_reader.read_bit_double()?;
            let _dimaltrnd = streams.object_reader.read_bit_double()?;
            let _dimalt = streams.object_reader.read_bit()?;
            let _dimaltd = streams.object_reader.read_bit_short()?;
            let _dimtofl = streams.object_reader.read_bit()?;
            let _dimsah = streams.object_reader.read_bit()?;
            let _dimtix = streams.object_reader.read_bit()?;
            let _dimsoxd = streams.object_reader.read_bit()?;
            let _dimclrd = streams.object_reader.read_cm_color()?;
            let _dimclre = streams.object_reader.read_cm_color()?;
            let _dimclrt = streams.object_reader.read_cm_color()?;
            let _dimadec = streams.object_reader.read_bit_short()?;
            let _dimdec = streams.object_reader.read_bit_short()?;
            let _dimtdec = streams.object_reader.read_bit_short()?;
            let _dimaltu = streams.object_reader.read_bit_short()?;
            let _dimalttd = streams.object_reader.read_bit_short()?;
            let _dimaunit = streams.object_reader.read_bit_short()?;
            let _dimatfit = streams.object_reader.read_bit_short()?;
            let _dimunit = streams.object_reader.read_bit_short()?;
            let _dimlunit = streams.object_reader.read_bit_short()?;
            let _dimdsep = streams.object_reader.read_bit_short()?;
            let _dimtmove = streams.object_reader.read_bit_short()?;
            let _dimjust = streams.object_reader.read_bit_short()?;
            let _dimsd1 = streams.object_reader.read_bit()?;
            let _dimsd2 = streams.object_reader.read_bit()?;
            let _dimtolj = streams.object_reader.read_bit_short()?;
            let _dimtzin = streams.object_reader.read_bit_short()?;
            let _dimaltz = streams.object_reader.read_bit_short()?;
            let _dimalttz = streams.object_reader.read_bit_short()?;
            let _dimupt = streams.object_reader.read_bit()?;
            let _dimfit = streams.object_reader.read_bit_short()?;

            // R2007+: txtdirection.
            if self.sio.r2007_plus {
                let _txtdir = streams.object_reader.read_bit()?;
            }

            let _dimscale = streams.object_reader.read_bit_double()?;
            let _dimmzf = streams.object_reader.read_bit_double()?;
            let _dimmzs = streams.object_reader.read_bit_short()?;

            // R2010+: dim DIMFRAC/DIMLTYPE/...
            if self.sio.r2010_plus {
                // dimfrac
                let _dimfrac = streams.object_reader.read_bit_short()?;
            }
        }

        // DimStyle control object handle.
        let _ctrl_handle = streams.handle_ref()?;

        // R13-R14: block name handles resolved from dimblk_name/dimblk1_name/dimblk2_name.

        // Common: DIMTXSTY (340) text style handle.
        dimstyle_data.dimtxsty_handle = streams.handle_ref()?;

        // R2000+: Handle references for block sub-objects.
        if self.sio.r2000_plus {
            dimstyle_data.dimldrblk_handle = streams.handle_ref()?;
            dimstyle_data.dimblk_handle = streams.handle_ref()?;
            dimstyle_data.dimblk1_handle = streams.handle_ref()?;
            dimstyle_data.dimblk2_handle = streams.handle_ref()?;
        }

        // R2007+: dimline type handles.
        if self.sio.r2007_plus {
            dimstyle_data.dimltype_handle = streams.handle_ref()?;
            dimstyle_data.dimltex1_handle = streams.handle_ref()?;
            dimstyle_data.dimltex2_handle = streams.handle_ref()?;
        }

        Ok(CadTemplate::DimStyleEntry {
            common: common_tmpl,
            dimstyle_data,
        })
    }

    // -----------------------------------------------------------------------
    // VP_ENT_HDR table entry (viewport entity header)
    // -----------------------------------------------------------------------

    pub(super) fn read_viewport_entity_header(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        // Xref dependent (B) — matches C# readXrefDependantBit.
        let _is_xref = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // 1-flag (B) — read as bit.
        let _flag1 = streams.object_reader.read_bit()?;

        // VP_ENT_HDR control object handle.
        let _ctrl_handle = streams.handle_ref()?;

        // Viewport entity handle.
        let _vp_entity = if !self.sio.r2004_plus {
            streams.handle_ref()?
        } else {
            0
        };

        Ok(CadTemplate::GenericTableEntry {
            common: common_tmpl,
        })
    }
}
