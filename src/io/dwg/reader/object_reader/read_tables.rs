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

        // Update handles.
        if !self.sio.r2004_plus {
            self.update_handle_reader(streams)?;
        }

        // Null handle (H).
        let _null_handle = streams.handles_reader.handle_reference()?;

        // Entry handles.
        let mut table_data = CadTableTemplateData::default();
        for _ in 0..num_entries {
            let h = streams.handles_reader.handle_reference()?;
            table_data.entry_handles.push(h);
        }

        Ok(CadTemplate::TableControl {
            common: common_tmpl,
            table_data,
            table_type,
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

        // Update handles.
        if !self.sio.r2004_plus {
            self.update_handle_reader(streams)?;
        }

        // Null handle (H).
        let _null_handle = streams.handles_reader.handle_reference()?;

        let mut table_data = CadTableTemplateData::default();
        for _ in 0..num_entries {
            let h = streams.handles_reader.handle_reference()?;
            table_data.entry_handles.push(h);
        }

        // ByLayer handle.
        let _bylayer = streams.handles_reader.handle_reference()?;
        // ByBlock handle.
        let _byblock = streams.handles_reader.handle_reference()?;

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

        // 64-flag (B).
        let _flag64 = streams.object_reader.read_bit()?;

        // Xref + (BS).
        let _xref_index = streams.object_reader.read_bit_short()?;

        // Xref dependent (B).
        let _xref_dep = streams.object_reader.read_bit()?;

        // Anonymous (B).
        let _anonymous = streams.object_reader.read_bit()?;

        // Has attributes (B).
        let _has_attr = streams.object_reader.read_bit()?;

        // Is xref (B).
        let _is_xref = streams.object_reader.read_bit()?;

        // Is xref overlay (B).
        let _is_xref_overlay = streams.object_reader.read_bit()?;

        // R2000+: load xref (B).
        if self.sio.r2000_plus {
            let _loaded = streams.object_reader.read_bit()?;
        }

        // R2004+: owned object count (BL).
        let owned_count = if self.sio.r2004_plus {
            streams.object_reader.read_bit_long()? as usize
        } else {
            0
        };

        // Base point (3BD).
        let _base_point = streams.object_reader.read_3bit_double()?;

        // Xref path name (TV).
        let _xref_path = streams.read_text()?;

        // R2000+: insert count (skip reading until 0).
        if self.sio.r2000_plus {
            loop {
                let insert_count = streams.object_reader.read_raw_char()?;
                if insert_count == 0 {
                    break;
                }
            }
        }

        // Description (TV).
        let _description = streams.read_text()?;

        // R2000+: preview data (BL + bytes).
        if self.sio.r2000_plus {
            let preview_size = streams.object_reader.read_bit_long()? as usize;
            if preview_size > 0 {
                let _preview = streams.object_reader.read_bytes(preview_size)?;
            }
        }

        // R2007+: insert units (BS), explodable (B), scale uniformly (B).
        if self.sio.r2007_plus {
            let _units = streams.object_reader.read_bit_short()?;
            let _explodable = streams.object_reader.read_bit()?;
            let _scale_uniform = streams.object_reader.read_bit()?;
        }

        // Update handles.
        if !self.sio.r2004_plus {
            self.update_handle_reader(streams)?;
        }

        // Block control handle.
        let _block_ctrl = streams.handles_reader.handle_reference()?;

        // Reactors handled by common data.

        // R2000+ entity handles (owned objects).
        if self.sio.r2004_plus {
            for _ in 0..owned_count {
                let h = streams.handles_reader.handle_reference()?;
                block_data.owned_object_handles.push(h);
            }
        } else {
            block_data.first_entity_handle = streams.handles_reader.handle_reference()?;
            block_data.last_entity_handle = streams.handles_reader.handle_reference()?;
        }

        // Block entity handle.
        block_data.block_entity_handle = streams.handles_reader.handle_reference()?;

        // R2000+:
        if self.sio.r2000_plus {
            // Layout handle (hard pointer).
            block_data.layout_handle = streams.handles_reader.handle_reference()?;
        }

        // End block entity handle.
        block_data.end_block_handle = streams.handles_reader.handle_reference()?;

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
        let _name = streams.read_text()?;

        // 64-flag (B).
        let _flag64 = streams.object_reader.read_bit()?;

        // Xref + (BS).
        let _xref_index = streams.object_reader.read_bit_short()?;

        // Xref dependent (B).
        let _is_xref = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // R13-R14: frozen (B), on (B), frozen-in-new (B), locked (B).
        if self.sio.r13_14_only {
            let _frozen = streams.object_reader.read_bit()?;
            let _on = streams.object_reader.read_bit()?;
            let _frozen_new = streams.object_reader.read_bit()?;
            let _locked = streams.object_reader.read_bit()?;
        }

        // R2000+: Values (BS) containing layer flags + color index.
        if self.sio.r2000_plus {
            let _values = streams.object_reader.read_bit_short()?;
        }

        // Color (CMC).
        let _color = streams.object_reader.read_cm_color()?;

        // Update handles.
        if !self.sio.r2004_plus {
            self.update_handle_reader(streams)?;
        }

        // Layer control handle (soft back pointer).
        let _layer_ctrl = streams.handles_reader.handle_reference()?;

        // Reactors handled by common data.

        // Xdict handled by common data.

        // Xref handle.
        let _xref_handle = streams.handles_reader.handle_reference()?;

        // R2000+: plotstyle handle.
        if self.sio.r2000_plus {
            layer_data.plotstyle_handle = streams.handles_reader.handle_reference()?;
        }

        // R2007+: material handle.
        if self.sio.r2007_plus {
            layer_data.material_handle = streams.handles_reader.handle_reference()?;
        }

        // Linetype handle.
        layer_data.linetype_handle = streams.handles_reader.handle_reference()?;

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

        // Name (TV).
        let _name = streams.read_text()?;

        // 64-flag (B).
        let _flag64 = streams.object_reader.read_bit()?;

        // Xref + (BS).
        let _xref_index = streams.object_reader.read_bit_short()?;

        // Xref dependent (B).
        let _is_xref = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // Is vertical (B).
        let _is_vertical = streams.object_reader.read_bit()?;

        // Is shape file (B).
        let _is_shape_file = streams.object_reader.read_bit()?;

        // Fixed height (BD).
        let _height = streams.object_reader.read_bit_double()?;

        // Width factor (BD).
        let _width_factor = streams.object_reader.read_bit_double()?;

        // Oblique angle (BD).
        let _oblique_angle = streams.object_reader.read_bit_double()?;

        // Generation flags (RC).
        let _gen_flags = streams.object_reader.read_raw_char()?;

        // Last height used (BD).
        let _last_height = streams.object_reader.read_bit_double()?;

        // Font file name (TV).
        let _font_file = streams.read_text()?;

        // Big font file name (TV).
        let _big_font_file = streams.read_text()?;

        // Update handles.
        if !self.sio.r2004_plus {
            self.update_handle_reader(streams)?;
        }

        // Style control object handle.
        let _ctrl_handle = streams.handles_reader.handle_reference()?;

        Ok(CadTemplate::GenericTableEntry {
            common: common_tmpl,
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
        let _name = streams.read_text()?;

        // 64-flag (B).
        let _flag64 = streams.object_reader.read_bit()?;

        // Xref + (BS).
        let _xref_index = streams.object_reader.read_bit_short()?;

        // Xref dependent (B).
        let _is_xref = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // Description (TV).
        let _description = streams.read_text()?;

        // Pattern length (BD).
        ltype_data.total_len = streams.object_reader.read_bit_double()?;

        // Alignment (RC).
        let _alignment = streams.object_reader.read_raw_char()?;

        // Number of dashes (RC).
        let num_dashes = streams.object_reader.read_raw_char()? as usize;

        for _i in 0..num_dashes {
            // Dash length (BD).
            let _dash_length = streams.object_reader.read_bit_double()?;

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

        // Update handles.
        if !self.sio.r2004_plus {
            self.update_handle_reader(streams)?;
        }

        // LType control object handle.
        ltype_data.ltype_control_handle = streams.handles_reader.handle_reference()?;

        // Segment handles (shape file refs).
        for _ in 0..num_dashes {
            let h = streams.handles_reader.handle_reference()?;
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
        let _name = streams.read_text()?;

        // 64-flag (B).
        let _flag64 = streams.object_reader.read_bit()?;

        // Xref + (BS).
        let _xref_index = streams.object_reader.read_bit_short()?;

        // Xref dependent (B).
        let _is_xref = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // View height (BD).
        let _height = streams.object_reader.read_bit_double()?;

        // View width (BD).
        let _width = streams.object_reader.read_bit_double()?;

        // View center (2RD).
        let _center = streams.object_reader.read_2raw_double()?;

        // View target (3BD).
        let _target = streams.object_reader.read_3bit_double()?;

        // View direction (3BD).
        let _direction = streams.object_reader.read_3bit_double()?;

        // Twist angle (BD).
        let _twist = streams.object_reader.read_bit_double()?;

        // Lens length (BD).
        let _lens = streams.object_reader.read_bit_double()?;

        // Front clip (BD).
        let _front = streams.object_reader.read_bit_double()?;

        // Back clip (BD).
        let _back = streams.object_reader.read_bit_double()?;

        // UCSFOLLOW (B).
        let _ufi = streams.object_reader.read_bit()?;

        // Front on (B) + Back on (B).
        let _front_on = streams.object_reader.read_bit()?;
        let _back_on = streams.object_reader.read_bit()?;

        // Render mode / UCS at Origin (R2000+).
        if self.sio.r2000_plus {
            let _render_mode = streams.object_reader.read_raw_char()?;
            let _associated_ucs = streams.object_reader.read_bit()?;
            // UCS info.
            let _ucs_origin = streams.object_reader.read_3bit_double()?;
            let _ucs_x = streams.object_reader.read_3bit_double()?;
            let _ucs_y = streams.object_reader.read_3bit_double()?;
            let _ucs_elevation = streams.object_reader.read_bit_double()?;
            let _ucs_ortho = streams.object_reader.read_bit_short()?;
        }

        if self.sio.r2007_plus {
            // Camera plottable (B).
            let _cam = streams.object_reader.read_bit()?;
        }

        // Update handles.
        if !self.sio.r2004_plus {
            self.update_handle_reader(streams)?;
        }

        // View control object handle.
        let _ctrl_handle = streams.handles_reader.handle_reference()?;

        if self.sio.r2000_plus {
            // UCS handle if associated.
            view_data.ucs_handle = streams.handles_reader.handle_reference()?;
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

        // Name (TV).
        let _name = streams.read_text()?;

        // 64-flag (B).
        let _flag64 = streams.object_reader.read_bit()?;

        // Xref + (BS).
        let _xref_index = streams.object_reader.read_bit_short()?;

        // Xref dependent (B).
        let _is_xref = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // Origin (3BD).
        let _origin = streams.object_reader.read_3bit_double()?;

        // X direction (3BD).
        let _x_dir = streams.object_reader.read_3bit_double()?;

        // Y direction (3BD).
        let _y_dir = streams.object_reader.read_3bit_double()?;

        // R2000+: elevation, ortho type, num ortho origins.
        if self.sio.r2000_plus {
            let _elevation = streams.object_reader.read_bit_double()?;
            let _ortho_type = streams.object_reader.read_bit_short()?;
            // Ortho origin array (BC count + 3BD each).
            // Actually: BS ortho_view_type first, then origin
        }

        // Update handles.
        if !self.sio.r2004_plus {
            self.update_handle_reader(streams)?;
        }

        // UCS control object handle.
        let _ctrl_handle = streams.handles_reader.handle_reference()?;

        Ok(CadTemplate::GenericTableEntry {
            common: common_tmpl,
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
        let _name = streams.read_text()?;

        // 64-flag (B).
        let _flag64 = streams.object_reader.read_bit()?;

        // Xref + (BS).
        let _xref_index = streams.object_reader.read_bit_short()?;

        // Xref dependent (B).
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

        // Update handles.
        if !self.sio.r2004_plus {
            self.update_handle_reader(streams)?;
        }

        // VPort control object handle.
        vport_data.vport_control_handle = streams.handles_reader.handle_reference()?;

        // R2007+: background, visual style, sun handles.
        if self.sio.r2007_plus {
            vport_data.background_handle = streams.handles_reader.handle_reference()?;
            vport_data.style_handle = streams.handles_reader.handle_reference()?;
            vport_data.sun_handle = streams.handles_reader.handle_reference()?;
        }

        if self.sio.r2000_plus {
            vport_data.named_ucs_handle = streams.handles_reader.handle_reference()?;
            vport_data.base_ucs_handle = streams.handles_reader.handle_reference()?;
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

        // Name (TV).
        let _name = streams.read_text()?;

        // 64-flag (B).
        let _flag64 = streams.object_reader.read_bit()?;

        // Xref + (BS).
        let _xref_index = streams.object_reader.read_bit_short()?;

        // Xref dependent (B).
        let _is_xref = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // Unknown (RC).
        let _unknown = streams.object_reader.read_raw_char()?;

        // Update handles.
        if !self.sio.r2004_plus {
            self.update_handle_reader(streams)?;
        }

        // AppId control object handle.
        let _ctrl_handle = streams.handles_reader.handle_reference()?;

        Ok(CadTemplate::GenericTableEntry {
            common: common_tmpl,
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
        let _name = streams.read_text()?;

        // 64-flag (B).
        let _flag64 = streams.object_reader.read_bit()?;

        // Xref + (BS).
        let _xref_index = streams.object_reader.read_bit_short()?;

        // Xref dependent (B).
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

        // Update handles.
        if !self.sio.r2004_plus {
            self.update_handle_reader(streams)?;
        }

        // DimStyle control object handle.
        let _ctrl_handle = streams.handles_reader.handle_reference()?;

        // R13-R14: block name handles resolved from dimblk_name/dimblk1_name/dimblk2_name.

        // R2000+: Handle references for various sub-objects.
        if self.sio.r2000_plus {
            dimstyle_data.dimtxsty_handle = streams.handles_reader.handle_reference()?;
            dimstyle_data.dimldrblk_handle = streams.handles_reader.handle_reference()?;
            dimstyle_data.dimblk_handle = streams.handles_reader.handle_reference()?;
            dimstyle_data.dimblk1_handle = streams.handles_reader.handle_reference()?;
            dimstyle_data.dimblk2_handle = streams.handles_reader.handle_reference()?;
        }

        // R2007+: dimline type handles.
        if self.sio.r2007_plus {
            dimstyle_data.dimltype_handle = streams.handles_reader.handle_reference()?;
            dimstyle_data.dimltex1_handle = streams.handles_reader.handle_reference()?;
            dimstyle_data.dimltex2_handle = streams.handles_reader.handle_reference()?;
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

        // 64-flag (B).
        let _flag64 = streams.object_reader.read_bit()?;

        // Xref + (BS).
        let _xref_index = streams.object_reader.read_bit_short()?;

        // Xref dependent (B).
        let _is_xref = self.read_xref_dependant_bit(&mut *streams.object_reader)?;

        // 1-flag (B) — read as bit.
        let _flag1 = streams.object_reader.read_bit()?;

        // Update handles.
        if !self.sio.r2004_plus {
            self.update_handle_reader(streams)?;
        }

        // VP_ENT_HDR control object handle.
        let _ctrl_handle = streams.handles_reader.handle_reference()?;

        // Viewport entity handle.
        let _vp_entity = if !self.sio.r2004_plus {
            streams.handles_reader.handle_reference()?
        } else {
            0
        };

        Ok(CadTemplate::GenericTableEntry {
            common: common_tmpl,
        })
    }
}
