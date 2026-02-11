//! Non-graphical object readers for the DWG object reader.
//!
//! Mirrors ACadSharp's `DwgObjectReader.Objects.cs`.

use crate::error::Result;

use super::templates::*;
use super::{DwgObjectReader, StreamSet};

impl DwgObjectReader {
    // -----------------------------------------------------------------------
    // DICTIONARY
    // -----------------------------------------------------------------------

    pub(super) fn read_dictionary(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        let dict_data = self.read_common_dictionary_data(streams)?;
        Ok(CadTemplate::DictionaryObj {
            common: common_tmpl,
            dict_data,
        })
    }

    #[allow(dead_code)]
    pub(super) fn read_dictionary_with_default(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        let dict_data = self.read_common_dictionary_data(streams)?;

        // Default entry handle (H — hard pointer).
        let default_handle = streams.handle_ref()?;

        Ok(CadTemplate::DictWithDefault {
            common: common_tmpl,
            dict_default_data: CadDictWithDefaultTemplateData {
                dict_data: dict_data,
                default_entry_handle: default_handle,
            },
        })
    }

    /// Shared dictionary data reader (matches readCommonDictionary in C#).
    fn read_common_dictionary_data(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadDictionaryTemplateData> {

        // BL: number of entries.
        let num_entries = streams.object_reader.read_bit_long()? as usize;

        // R14 only: unknown byte (RC).
        if self.sio.r13_14_only {
            let _unknown = streams.object_reader.read_raw_char()?;
        }

        // R2000+: cloning flags (BS), hard owner flag (RC).
        if self.sio.r2000_plus {
            let _cloning = streams.object_reader.read_bit_short()?;
            let _hard_owner = streams.object_reader.read_raw_char()?;
        }

        let mut data = CadDictionaryTemplateData::default();

        for _ in 0..num_entries {
            let name = streams.read_text()?;
            let handle = streams.handle_ref()?;
            if handle != 0 && !name.is_empty() {
                data.entries.push((name, handle));
            }
        }

        Ok(data)
    }

    // -----------------------------------------------------------------------
    // DICTIONARYVAR
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub(super) fn read_dictionary_var(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        // RC: integer value (discarded).
        let _int_val = streams.object_reader.read_raw_char()?;

        // TV: string value.
        let _value = streams.read_text()?;

        Ok(CadTemplate::GenericObject {
            common: common_tmpl,
        })
    }

    // -----------------------------------------------------------------------
    // GROUP
    // -----------------------------------------------------------------------

    pub(super) fn read_group(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        let mut group_data = CadGroupTemplateData::default();

        // TV: description.
        let _description = streams.read_text()?;

        // BS: unnamed flag (1 = unnamed).
        let _unnamed = streams.object_reader.read_bit_short()?;

        // BS: selectable.
        let _selectable = streams.object_reader.read_bit_short()?;

        // BL: number of entity handles.
        let num_handles = streams.object_reader.read_bit_long()? as usize;

        // Entity handles.
        for _ in 0..num_handles {
            let h = streams.handle_ref()?;
            group_data.entity_handles.push(h);
        }

        Ok(CadTemplate::GroupObj {
            common: common_tmpl,
            group_data,
        })
    }

    // -----------------------------------------------------------------------
    // MLINESTYLE
    // -----------------------------------------------------------------------

    pub(super) fn read_mline_style(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        let mut style_data = CadMLineStyleTemplateData::default();

        // TV: name.
        let _name = streams.read_text()?;

        // TV: description.
        let _description = streams.read_text()?;

        // BS: flags (DWG bit-mapped flags → DXF flags mapped in builder).
        let _flags = streams.object_reader.read_bit_short()?;

        // CMC: fill color.
        let _fill_color = streams.object_reader.read_cm_color()?;

        // BD: start angle.
        let _start_angle = streams.object_reader.read_bit_double()?;

        // BD: end angle.
        let _end_angle = streams.object_reader.read_bit_double()?;

        // RC: number of lines (elements).
        let num_lines = streams.object_reader.read_raw_char()? as usize;

        for _ in 0..num_lines {
            // BD: offset.
            let _offset = streams.object_reader.read_bit_double()?;

            // CMC: color.
            let _color = streams.object_reader.read_cm_color()?;

            // R2018+: linetype handle (H — hard pointer).
            if self.sio.r2018_plus {
                let h = streams.handle_ref()?;
                style_data.element_linetype_handles.push(h);
            } else {
                // BS: linetype index (pre-R2018).
                let _ltype_index = streams.object_reader.read_bit_short()?;
                style_data.element_linetype_handles.push(0);
            }
        }

        Ok(CadTemplate::MLineStyleObj {
            common: common_tmpl,
            mls_data: style_data,
        })
    }

    // -----------------------------------------------------------------------
    // LAYOUT (includes readPlotSettings call)
    // -----------------------------------------------------------------------

    pub(super) fn read_layout(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        let mut layout_data = CadLayoutTemplateData::default();

        // Read plot settings portion.
        self.read_plot_settings_data(streams)?;

        // TV: layout name.
        let _name = streams.read_text()?;

        // BL: tab order.
        let _tab_order = streams.object_reader.read_bit_long()?;

        // BS: layout flags.
        let _flags = streams.object_reader.read_bit_short()?;

        // 3BD: UCS origin.
        let _origin = streams.object_reader.read_3bit_double()?;

        // 2RD: min limits.
        let _min_limits = streams.object_reader.read_2raw_double()?;

        // 2RD: max limits.
        let _max_limits = streams.object_reader.read_2raw_double()?;

        // 3BD: insertion base point.
        let _ins_base = streams.object_reader.read_3bit_double()?;

        // 3BD: UCS X axis.
        let _ucs_x = streams.object_reader.read_3bit_double()?;

        // 3BD: UCS Y axis.
        let _ucs_y = streams.object_reader.read_3bit_double()?;

        // BD: elevation.
        let _elevation = streams.object_reader.read_bit_double()?;

        // BS: UCS orthographic type.
        let _ortho_type = streams.object_reader.read_bit_short()?;

        // 3BD: min extents.
        let _min_extents = streams.object_reader.read_3bit_double()?;

        // 3BD: max extents.
        let _max_extents = streams.object_reader.read_3bit_double()?;

        // R2004+: number of viewports.
        let num_viewports = if self.sio.r2004_plus {
            streams.object_reader.read_bit_long()? as usize
        } else {
            0
        };

        // Handles.
        // Paper space block handle (soft pointer).
        layout_data.block_record_handle = streams.handle_ref()?;

        // Active viewport handle (soft pointer).
        layout_data.viewport_handle = streams.handle_ref()?;

        // Base UCS handle (hard pointer).
        layout_data.base_ucs_handle = streams.handle_ref()?;

        // Named UCS handle (hard pointer).
        layout_data.named_ucs_handle = streams.handle_ref()?;

        // R2004+: viewport handles.
        if self.sio.r2004_plus {
            for _ in 0..num_viewports {
                let _h = streams.handle_ref()?;
                // viewport handles not stored individually in template
            }
        }

        Ok(CadTemplate::LayoutObj {
            common: common_tmpl,
            layout_data,
        })
    }

    // -----------------------------------------------------------------------
    // PLOTSETTINGS (standalone object)
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub(super) fn read_plot_settings(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        // Read the shared plot settings data.
        self.read_plot_settings_data(streams)?;

        Ok(CadTemplate::PlotSettingsObj {
            common: common_tmpl,
        })
    }

    /// Shared plot settings data reader (matches readPlotSettings(PlotSettings) in C#).
    fn read_plot_settings_data(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<()> {

        // TV: page name.
        let _page_name = streams.read_text()?;

        // TV: system printer name.
        let _printer = streams.read_text()?;

        // BS: flags.
        let _flags = streams.object_reader.read_bit_short()?;

        // BD: margins (left, bottom, right, top).
        let _margin_left = streams.object_reader.read_bit_double()?;
        let _margin_bottom = streams.object_reader.read_bit_double()?;
        let _margin_right = streams.object_reader.read_bit_double()?;
        let _margin_top = streams.object_reader.read_bit_double()?;

        // BD: paper width, height.
        let _paper_width = streams.object_reader.read_bit_double()?;
        let _paper_height = streams.object_reader.read_bit_double()?;

        // TV: paper size.
        let _paper_size = streams.read_text()?;

        // BD: plot origin x, y.
        let _origin_x = streams.object_reader.read_bit_double()?;
        let _origin_y = streams.object_reader.read_bit_double()?;

        // BS: paper units.
        let _paper_units = streams.object_reader.read_bit_short()?;

        // BS: paper rotation.
        let _rotation = streams.object_reader.read_bit_short()?;

        // BS: plot type.
        let _plot_type = streams.object_reader.read_bit_short()?;

        // BD: window lower left x, y; upper right x, y.
        let _ll_x = streams.object_reader.read_bit_double()?;
        let _ll_y = streams.object_reader.read_bit_double()?;
        let _ur_x = streams.object_reader.read_bit_double()?;
        let _ur_y = streams.object_reader.read_bit_double()?;

        // R13-R2000: plot view name (TV).
        if !self.sio.r2004_plus {
            let _view_name = streams.read_text()?;
        }

        // BD: numerator scale (real world units).
        let _num_scale = streams.object_reader.read_bit_double()?;

        // BD: denominator scale (drawing units).
        let _den_scale = streams.object_reader.read_bit_double()?;

        // TV: stylesheet.
        let _stylesheet = streams.read_text()?;

        // BS: scaled fit.
        let _scaled_fit = streams.object_reader.read_bit_short()?;

        // BD: standard scale factor.
        let _std_scale = streams.object_reader.read_bit_double()?;

        // 2BD: paper image origin.
        let _img_origin_x = streams.object_reader.read_bit_double()?;
        let _img_origin_y = streams.object_reader.read_bit_double()?;

        // R2004+: shade plot mode, resolution, DPI.
        if self.sio.r2004_plus {
            let _shade_mode = streams.object_reader.read_bit_short()?;
            let _shade_res = streams.object_reader.read_bit_short()?;
            let _shade_dpi = streams.object_reader.read_bit_short()?;

            // H: plot view handle (hard pointer) — discarded.
            let _plot_view = streams.handle_ref()?;
        }

        // R2007+: visual style handle (soft pointer) — discarded.
        if self.sio.r2007_plus {
            let _visual_style = streams.handle_ref()?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // XRECORD
    // -----------------------------------------------------------------------

    pub(super) fn read_xrecord(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        // BL: numdatabytes — data length.
        let num_data_bytes = streams.object_reader.read_bit_long()? as usize;

        // Calculate end offset for data section.
        let start_pos = streams.object_reader.position_in_bits();
        let end_pos = start_pos + (num_data_bytes as i64 * 8);

        // Read data pairs (RS group code + value based on type).
        while streams.object_reader.position_in_bits() < end_pos {
            let group_code = streams.object_reader.read_raw_short()?;

            // Read value based on group code type.
            // GroupCodeValueType lookup from DXF group codes.
            match Self::group_code_value_type(group_code) {
                GroupCodeType::String => {
                    let _val = streams.object_reader.read_text_unicode()?;
                }
                GroupCodeType::Point3D => {
                    let _x = streams.object_reader.read_bit_double()?;
                    let _y = streams.object_reader.read_bit_double()?;
                    let _z = streams.object_reader.read_bit_double()?;
                }
                GroupCodeType::Double => {
                    let _val = streams.object_reader.read_bit_double()?;
                }
                GroupCodeType::Int16 => {
                    let _val = streams.object_reader.read_bit_short()?;
                }
                GroupCodeType::Int32 => {
                    let _val = streams.object_reader.read_bit_long()?;
                }
                GroupCodeType::Int64 => {
                    let _val = streams.object_reader.read_bit_long_long()?;
                }
                GroupCodeType::Handle => {
                    let _val = streams.object_reader.handle_reference()?;
                }
                GroupCodeType::Bool => {
                    let _val = streams.object_reader.read_bit()?;
                }
                GroupCodeType::Chunk => {
                    let len = streams.object_reader.read_raw_char()? as usize;
                    let _val = streams.object_reader.read_bytes(len)?;
                }
                GroupCodeType::Unknown => {
                    // Skip unknown codes.
                    break;
                }
            }
        }

        // R2000+: cloning flags (BS).
        if self.sio.r2000_plus {
            let _cloning = streams.object_reader.read_bit_short()?;
        }

        Ok(CadTemplate::XRecordObj {
            common: common_tmpl,
        })
    }

    /// Map a DXF group code to its value type for XRecord reading.
    fn group_code_value_type(code: i16) -> GroupCodeType {
        match code {
            0..=9 => GroupCodeType::String,
            10..=39 => GroupCodeType::Point3D,
            40..=59 => GroupCodeType::Double,
            60..=79 => GroupCodeType::Int16,
            90..=99 => GroupCodeType::Int32,
            100..=102 => GroupCodeType::String,
            105 => GroupCodeType::Handle,
            110..=149 => GroupCodeType::Double,
            160..=169 => GroupCodeType::Int64,
            170..=179 => GroupCodeType::Int16,
            210..=239 => GroupCodeType::Double,
            270..=279 => GroupCodeType::Int16,
            280..=289 => GroupCodeType::Int16,
            290..=299 => GroupCodeType::Bool,
            300..=309 => GroupCodeType::String,
            310..=319 => GroupCodeType::Chunk,
            320..=329 => GroupCodeType::Handle,
            330..=369 => GroupCodeType::Handle,
            370..=379 => GroupCodeType::Int16,
            380..=389 => GroupCodeType::Int16,
            390..=399 => GroupCodeType::Handle,
            400..=409 => GroupCodeType::Int16,
            410..=419 => GroupCodeType::String,
            420..=429 => GroupCodeType::Int32,
            430..=439 => GroupCodeType::String,
            440..=449 => GroupCodeType::Int32,
            450..=459 => GroupCodeType::Int32,
            460..=469 => GroupCodeType::Double,
            470..=479 => GroupCodeType::String,
            480..=481 => GroupCodeType::Handle,
            999 => GroupCodeType::String,
            1000..=1009 => GroupCodeType::String,
            1010..=1059 => GroupCodeType::Double,
            1060..=1070 => GroupCodeType::Int16,
            1071 => GroupCodeType::Int32,
            _ => GroupCodeType::Unknown,
        }
    }

    // -----------------------------------------------------------------------
    // SCALE
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub(super) fn read_scale(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        // BS: unknown (ODA writes 0) — discarded.
        let _unknown = streams.object_reader.read_bit_short()?;

        // TV: name.
        let _name = streams.read_text()?;

        // BD: paper units.
        let _paper_units = streams.object_reader.read_bit_double()?;

        // BD: drawing units (divided by 10 in C# source).
        let _drawing_units = streams.object_reader.read_bit_double()?;

        // B: is unit scale.
        let _is_unit = streams.object_reader.read_bit()?;

        Ok(CadTemplate::ScaleObj {
            common: common_tmpl,
        })
    }

    // -----------------------------------------------------------------------
    // SORTENTSTABLE
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub(super) fn read_sort_entities_table(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        let mut sort_data = CadSortEntsTableTemplateData::default();

        // Block owner handle (soft pointer).
        sort_data.block_owner_handle = streams.handle_ref()?;

        // BL: number of entries.
        let num_entries = streams.object_reader.read_bit_long()? as usize;

        // Read entries: sort handle (from object stream) + entity handle (from handle stream).
        for _ in 0..num_entries {
            let sort_handle = streams.object_reader.handle_reference()?;
            let entity_handle = streams.handle_ref()?;
            sort_data.sort_handle_pairs.push((sort_handle, entity_handle));
        }

        Ok(CadTemplate::SortEntsTableObj {
            common: common_tmpl,
            sort_data,
        })
    }

    // -----------------------------------------------------------------------
    // IMAGE_DEF
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub(super) fn read_image_definition(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        // BL: class version.
        let _version = streams.object_reader.read_bit_long()?;

        // 2RD: image size (pixels).
        let _size = streams.object_reader.read_2raw_double()?;

        // TV: file name.
        let _file_name = streams.read_text()?;

        // B: is loaded.
        let _is_loaded = streams.object_reader.read_bit()?;

        // RC: units.
        let _units = streams.object_reader.read_raw_char()?;

        // 2RD: default size (pixel size in AutoCAD units).
        let _default_size = streams.object_reader.read_2raw_double()?;

        Ok(CadTemplate::ImageDefObj {
            common: common_tmpl,
            imgdef_data: CadImageDefTemplateData {},
        })
    }

    // -----------------------------------------------------------------------
    // IMAGE_DEF_REACTOR
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub(super) fn read_image_definition_reactor(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        // BL: class version.
        let _version = streams.object_reader.read_bit_long()?;

        Ok(CadTemplate::ImageDefReactorObj {
            common: common_tmpl,
            reactor_data: CadImageDefReactorTemplateData {
                image_handle: 0,
            },
        })
    }

    // -----------------------------------------------------------------------
    // MLEADERSTYLE
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub(super) fn read_mleader_style(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        let mut mleader_data = CadMLeaderStyleTemplateData::default();

        // R2010+: version (BS = 2).
        if self.sio.r2010_plus {
            let _version = streams.object_reader.read_bit_short()?;
        }

        // BS: content type.
        let _content_type = streams.object_reader.read_bit_short()?;

        // BS: multi-leader draw order.
        let _ml_draw_order = streams.object_reader.read_bit_short()?;

        // BS: leader draw order.
        let _leader_draw_order = streams.object_reader.read_bit_short()?;

        // BL: max leader segments points.
        let _max_segments = streams.object_reader.read_bit_long()?;

        // BD: first segment angle constraint.
        let _first_angle = streams.object_reader.read_bit_double()?;

        // BD: second segment angle constraint.
        let _second_angle = streams.object_reader.read_bit_double()?;

        // BS: path type.
        let _path_type = streams.object_reader.read_bit_short()?;

        // CMC: line color.
        let _line_color = streams.object_reader.read_cm_color()?;

        // H: leader line type handle (hard pointer).
        mleader_data.leader_line_type_handle = streams.handle_ref()?;

        // BL: leader line weight.
        let _line_weight = streams.object_reader.read_bit_long()?;

        // B: enable landing.
        let _landing = streams.object_reader.read_bit()?;

        // BD: landing gap.
        let _landing_gap = streams.object_reader.read_bit_double()?;

        // B: enable dogleg.
        let _dogleg = streams.object_reader.read_bit()?;

        // BD: landing distance.
        let _landing_distance = streams.object_reader.read_bit_double()?;

        // TV: description.
        let _description = streams.read_text()?;

        // H: arrowhead handle (hard pointer).
        mleader_data.arrowhead_handle = streams.handle_ref()?;

        // BD: arrowhead size.
        let _arrow_size = streams.object_reader.read_bit_double()?;

        // TV: default text contents.
        let _default_text = streams.read_text()?;

        // H: text style handle (hard pointer).
        mleader_data.mtext_style_handle = streams.handle_ref()?;

        // BS: text left attachment.
        let _text_left = streams.object_reader.read_bit_short()?;

        // BS: text right attachment.
        let _text_right = streams.object_reader.read_bit_short()?;

        // BS: text angle.
        let _text_angle = streams.object_reader.read_bit_short()?;

        // BS: text alignment.
        let _text_alignment = streams.object_reader.read_bit_short()?;

        // CMC: text color.
        let _text_color = streams.object_reader.read_cm_color()?;

        // BD: text height.
        let _text_height = streams.object_reader.read_bit_double()?;

        // B: text frame.
        let _text_frame = streams.object_reader.read_bit()?;

        // B: text align always left.
        let _align_left = streams.object_reader.read_bit()?;

        // BD: align space.
        let _align_space = streams.object_reader.read_bit_double()?;

        // H: block content handle (hard pointer).
        mleader_data.block_content_handle = streams.handle_ref()?;

        // CMC: block content color.
        let _block_color = streams.object_reader.read_cm_color()?;

        // 3BD: block content scale (x, y, z separate BD).
        let _scale_x = streams.object_reader.read_bit_double()?;
        let _scale_y = streams.object_reader.read_bit_double()?;
        let _scale_z = streams.object_reader.read_bit_double()?;

        // B: enable block content scale.
        let _enable_scale = streams.object_reader.read_bit()?;

        // BD: block content rotation.
        let _rotation = streams.object_reader.read_bit_double()?;

        // B: enable block content rotation.
        let _enable_rotation = streams.object_reader.read_bit()?;

        // BS: block content connection.
        let _connection = streams.object_reader.read_bit_short()?;

        // BD: scale factor.
        let _scale_factor = streams.object_reader.read_bit_double()?;

        // B: overwrite property value.
        let _overwrite = streams.object_reader.read_bit()?;

        // B: is annotative.
        let _annotative = streams.object_reader.read_bit()?;

        // BD: break size.
        let _break_size = streams.object_reader.read_bit_double()?;

        // R2010+: text attachment direction, text top/bottom attachment.
        if self.sio.r2010_plus {
            let _text_attach_dir = streams.object_reader.read_bit_short()?;
            let _text_top_attach = streams.object_reader.read_bit_short()?;
        }

        // R2013+: unknown flag.
        if self.sio.r2013_plus {
            let _unknown = streams.object_reader.read_bit()?;
        }

        Ok(CadTemplate::MLeaderStyleObj {
            common: common_tmpl,
            mls_style_data: mleader_data,
        })
    }

    // -----------------------------------------------------------------------
    // VISUALSTYLE (stub — complex, incomplete in C# source too)
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub(super) fn read_visual_style(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        // Partial read — C# source marks this as incomplete.
        // Read name and type only.
        let _name = streams.read_text()?;
        // The rest is very complex and marked TODO in ACadSharp.
        Ok(CadTemplate::GenericObject {
            common: common_tmpl,
        })
    }

    // -----------------------------------------------------------------------
    // MATERIAL (stub — very complex)
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub(super) fn read_material(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        // Material is extremely complex with many sub-structures.
        // Read basics.
        let _name = streams.read_text()?;
        let _description = streams.read_text()?;
        // Full material reading requires ~200+ lines; stub for now.
        Ok(CadTemplate::GenericObject {
            common: common_tmpl,
        })
    }

    // -----------------------------------------------------------------------
    // PLACEHOLDER
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub(super) fn read_acdb_placeholder(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        Ok(CadTemplate::GenericObject {
            common: common_tmpl,
        })
    }

    // -----------------------------------------------------------------------
    // RASTERVARIABLES
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub(super) fn read_raster_variables(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        // BL: class version.
        let _version = streams.object_reader.read_bit_long()?;

        // BS: display frame.
        let _frame = streams.object_reader.read_bit_short()?;

        // BS: display quality.
        let _quality = streams.object_reader.read_bit_short()?;

        // BS: units.
        let _units = streams.object_reader.read_bit_short()?;

        Ok(CadTemplate::GenericObject {
            common: common_tmpl,
        })
    }

    // -----------------------------------------------------------------------
    // DBCOLOR (BookColor)
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub(super) fn read_db_color(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        // BS: color index.
        let _color_index = streams.object_reader.read_bit_short()?;

        // R2004+: true color + flags + names.
        if self.sio.r2004_plus {
            let _true_color = streams.object_reader.read_bit_long()?;
            let flags = streams.object_reader.read_raw_char()?;

            if (flags & 1) > 0 {
                let _color_name = streams.read_text()?;
            }
            if (flags & 2) > 0 {
                let _book_name = streams.read_text()?;
            }
        }

        Ok(CadTemplate::GenericObject {
            common: common_tmpl,
        })
    }

    // -----------------------------------------------------------------------
    // PDF_DEFINITION (PdfUnderlayDefinition)
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub(super) fn read_pdf_definition(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        // TV: file path.
        let _file = streams.read_text()?;

        // TV: page.
        let _page = streams.read_text()?;

        Ok(CadTemplate::GenericObject {
            common: common_tmpl,
        })
    }

    // -----------------------------------------------------------------------
    // TABLESTYLE (stub — very complex r2007+ format)
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub(super) fn read_table_style(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;
        // TableStyle is extremely complex especially for R2007+.
        // Basic pre-R2007 reading:
        if !self.sio.r2007_plus {
            let _description = streams.read_text()?;
            let _flow_direction = streams.object_reader.read_bit_short()?;
            let _flags = streams.object_reader.read_bit_short()?;
            let _h_margin = streams.object_reader.read_bit_double()?;
            let _v_margin = streams.object_reader.read_bit_double()?;
            let _suppress_title = streams.object_reader.read_bit()?;
            let _suppress_header = streams.object_reader.read_bit()?;
            // 3 row/cell styles (data, title, header).
            // Each has CMC text color, CMC bg color, B fill, 6 border styles.
            // Then handle for text style.
        }
        Ok(CadTemplate::GenericObject {
            common: common_tmpl,
        })
    }

    // -----------------------------------------------------------------------
    // WIPEOUTVARIABLES (simple)
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub(super) fn read_wipeout_variables(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        // BS: display frame.
        let _display_frame = streams.object_reader.read_bit_short()?;

        Ok(CadTemplate::GenericObject {
            common: common_tmpl,
        })
    }
}

/// Group code value types for XRecord reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GroupCodeType {
    String,
    Point3D,
    Double,
    Int16,
    Int32,
    Int64,
    Handle,
    Bool,
    Chunk,
    Unknown,
}
