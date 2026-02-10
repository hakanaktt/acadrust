//! Common entity/object data readers.
//!
//! Mirrors ACadSharp's `DwgObjectReader` methods:
//! - `readCommonEntityData`
//! - `readCommonNonEntityData`
//! - `readExtendedData`
//! - `readReactorsAndDictionaryHandle`
//! - `readXrefDependantBit`

use crate::entities::EntityCommon;
use crate::error::Result;
use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
use crate::types::{Handle, LineWeight};
use crate::xdata::XDataValue;

use super::templates::{
    CadEntityTemplateData, CadTemplateCommon, EDataTemplate,
};
use super::{DwgObjectReader, StreamSet};

impl DwgObjectReader {
    // -----------------------------------------------------------------------
    // readCommonEntityData
    // -----------------------------------------------------------------------
    // readCommonEntityData
    // -----------------------------------------------------------------------

    /// Read common entity data fields.
    ///
    /// Corresponds to ACadSharp `readCommonEntityData()`.
    ///
    /// Populates:
    /// - `CadTemplateCommon` (handle, reactors, xdict, EED)
    /// - `CadEntityTemplateData` (layer, ltype, plotstyle, entity mode, etc.)
    /// - `EntityCommon` (color, line weight, transparency, invisibility, etc.)
    pub(super) fn read_common_entity_data(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<(CadTemplateCommon, CadEntityTemplateData, EntityCommon)> {
        let mut common_tmpl = CadTemplateCommon::default();
        let mut ent_tmpl = CadEntityTemplateData::default();
        let mut entity_common = EntityCommon::new();

        // R2000-R2007: Update handle reader position (RL: object size in bits).
        // This comes BEFORE handle+EED — matching C# readCommonData().
        if self.sio.r2000_plus && !self.sio.r2010_plus {
            self.update_handle_reader(streams)?;
        }

        // Handle (H).
        let handle = streams.object_reader.handle_reference()?;
        common_tmpl.handle = handle;
        entity_common.handle = Handle::new(handle);

        // EED (extended entity data).
        common_tmpl.edata = self.read_extended_data(streams)?;

        // Graphic present flag (B).
        let graphic_present = streams.object_reader.read_bit()?;
        if graphic_present {
            if self.sio.r2010_plus {
                // R2010+: BLL graphics size.
                let gfx_size = streams.object_reader.read_bit_long_long()? as usize;
                streams.object_reader.advance(gfx_size);
            } else {
                // R13-R2007: RL graphics size.
                let gfx_size = streams.object_reader.read_raw_long()? as usize;
                streams.object_reader.advance(gfx_size);
            }
        }

        // R13-R14: Update handle reader position before entity mode.
        if self.sio.r13_14_only {
            self.update_handle_reader(streams)?;
        }

        // Entity mode (BB).
        ent_tmpl.entity_mode = streams.object_reader.read_2bits()?;

        // When entity_mode == 0, the owner handle is present (soft pointer).
        // Matches C# readEntityMode: if (EntityMode == 0) OwnerHandle = handleReference.
        if ent_tmpl.entity_mode == 0 {
            common_tmpl.owner_handle = streams.handles_reader.handle_reference()?;
        }

        // Reactors and extended dictionary.
        let (reactor_handles, xdict_handle) =
            self.read_reactors_and_dictionary_handle(streams)?;
        common_tmpl.reactor_handles = reactor_handles;
        common_tmpl.xdict_handle = xdict_handle;

        // R13-R14: Layer handle, Linetype handle.
        if self.sio.r13_14_only {
            ent_tmpl.layer_handle = streams.handles_reader.handle_reference()?;
            // Isbylayerlt B: 1 if bylayer linetype (no handle), else 0 (read handle).
            let is_bylayer_lt = streams.object_reader.read_bit()?;
            if !is_bylayer_lt {
                ent_tmpl.linetype_handle = streams.handles_reader.handle_reference()?;
            }
        }

        // R13-R2000: prev/next entity handles (linked list).
        // A bit flag indicates whether the handles are present.
        if !self.sio.r2004_plus {
            let no_links = streams.object_reader.read_bit()?;
            if !no_links {
                ent_tmpl.prev_entity = streams.handles_reader.handle_reference()?;
                ent_tmpl.next_entity = streams.handles_reader.handle_reference()?;
            }
        }

        // Color, transparency, book-color flag.
        let (color, transparency, has_color_handle) = streams.object_reader.read_en_color()?;
        entity_common.color = color;
        entity_common.transparency = transparency;

        // R2004+ color handle.
        if self.sio.r2004_plus && has_color_handle {
            ent_tmpl.color_handle = streams.handles_reader.handle_reference()?;
        }

        // Linetype scale (BD).
        let linetype_scale = streams.object_reader.read_bit_double()?;
        let _ = linetype_scale; // stored on entity if needed

        // R2000+: Linetype flags, plotstyle flags, material flags, shadow flags.
        if self.sio.r2000_plus {
            // Layer handle (H).
            ent_tmpl.layer_handle = streams.handles_reader.handle_reference()?;

            // Linetype flags (BB): 00=bylayer, 01=byblock, 10=continuous, 11=handle.
            ent_tmpl.ltype_flags = streams.object_reader.read_2bits()?;

            if ent_tmpl.ltype_flags == 3 {
                ent_tmpl.linetype_handle = streams.handles_reader.handle_reference()?;
            }

            if self.sio.r2007_plus {
                // Material flags (BB).
                let material_flags = streams.object_reader.read_2bits()?;
                if material_flags == 3 {
                    ent_tmpl.material_handle = streams.handles_reader.handle_reference()?;
                }
                // Shadow flags RC (1 byte, not BB).
                let _shadow_flags = streams.object_reader.read_raw_char()?;
            }

            // Plotstyle flags (BB).
            let plotstyle_flags = streams.object_reader.read_2bits()?;
            if plotstyle_flags == 3 {
                ent_tmpl.plotstyle_handle = streams.handles_reader.handle_reference()?;
            }

            if self.sio.r2010_plus {
                // R2010+: has full visual style (B), face visual style (B), edge visual style (B).
                let has_full_vs = streams.object_reader.read_bit()?;
                let has_face_vs = streams.object_reader.read_bit()?;
                let has_edge_vs = streams.object_reader.read_bit()?;
                if has_full_vs {
                    let _full_vs = streams.handles_reader.handle_reference()?;
                }
                if has_face_vs {
                    let _face_vs = streams.handles_reader.handle_reference()?;
                }
                if has_edge_vs {
                    let _edge_vs = streams.handles_reader.handle_reference()?;
                }
            }
        }

        // Invisibility (BS).
        let invis_flags = streams.object_reader.read_bit_short()?;
        entity_common.invisible = (invis_flags & 1) != 0;

        // Lineweight (RC).
        if self.sio.r2000_plus {
            let lw = streams.object_reader.read_raw_char()?;
            entity_common.line_weight = LineWeight::from_value(lw as i16);
        }

        Ok((common_tmpl, ent_tmpl, entity_common))
    }

    // -----------------------------------------------------------------------
    // readCommonNonEntityData
    // -----------------------------------------------------------------------

    /// Read common non-entity data fields.
    ///
    /// Corresponds to ACadSharp `readCommonNonEntityData()`.
    pub(super) fn read_common_non_entity_data(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<CadTemplateCommon> {
        let mut common_tmpl = CadTemplateCommon::default();

        // R2000-R2007: Update handle reader position (RL: object size in bits).
        // This comes BEFORE handle+EED — matching C# readCommonData().
        if self.sio.r2000_plus && !self.sio.r2010_plus {
            self.update_handle_reader(streams)?;
        }

        // Handle (H).
        let handle = streams.object_reader.handle_reference()?;
        common_tmpl.handle = handle;

        // EED (extended entity data).
        common_tmpl.edata = self.read_extended_data(streams)?;

        // R13-R14: Update handle reader position (RL comes AFTER handle+EED).
        if self.sio.r13_14_only {
            self.update_handle_reader(streams)?;
        }

        // Number of reactors (BL).
        let num_reactors = streams.object_reader.read_bit_long()? as usize;

        // R2004+: xdictionary-missing flag (B).
        let xdict_missing = if self.sio.r2004_plus {
            streams.object_reader.read_bit()?
        } else {
            false
        };

        // R2013+: has-ds-binary-data flag (B).
        if self.sio.r2013_plus {
            let _has_ds = streams.object_reader.read_bit()?;
        }

        // Owner handle (H): soft pointer.
        common_tmpl.owner_handle = streams.handles_reader.handle_reference()?;

        // Reactor handles.
        for _ in 0..num_reactors {
            let rh = streams.handles_reader.handle_reference()?;
            common_tmpl.reactor_handles.push(rh);
        }

        // XDictionary handle (hard owner) if not missing.
        if !xdict_missing {
            common_tmpl.xdict_handle = streams.handles_reader.handle_reference()?;
        }

        Ok(common_tmpl)
    }

    // -----------------------------------------------------------------------
    // readReactorsAndDictionaryHandle (entity version)
    // -----------------------------------------------------------------------

    /// Read reactor handles and xdictionary handle for entities.
    ///
    /// Corresponds to ACadSharp `readReactorsAndDictionaryHandle()`.
    fn read_reactors_and_dictionary_handle(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<(Vec<u64>, u64)> {
        // BL: number of reactors.
        let num_reactors = streams.object_reader.read_bit_long()? as usize;

        // Sanity check: reactor count > 10000 almost certainly means a misaligned stream.
        if num_reactors > 10_000 {
            return Err(crate::error::DxfError::Parse(format!(
                "Reactor count {} is unreasonably large — stream likely misaligned",
                num_reactors
            )));
        }

        // R2004+: xdictionary-missing flag (B).
        let xdict_missing = if self.sio.r2004_plus {
            streams.object_reader.read_bit()?
        } else {
            false
        };

        // R2013+: has-ds-binary-data flag (B).
        if self.sio.r2013_plus {
            let _has_ds = streams.object_reader.read_bit()?;
        }

        // Reactor handles.
        let mut reactors = Vec::with_capacity(num_reactors);
        for _ in 0..num_reactors {
            let rh = streams.handles_reader.handle_reference()?;
            reactors.push(rh);
        }

        // XDictionary handle.
        let xdict = if !xdict_missing {
            streams.handles_reader.handle_reference()?
        } else {
            0
        };

        Ok((reactors, xdict))
    }

    // -----------------------------------------------------------------------
    // readExtendedData (EED)
    // -----------------------------------------------------------------------

    /// Read extended entity data (EED / XDATA).
    ///
    /// Corresponds to ACadSharp `readExtendedData()`.
    fn read_extended_data(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<EDataTemplate> {
        let mut edata = EDataTemplate::default();

        loop {
            // BS: size of next app group (0 = end of EED).
            let size = streams.object_reader.read_bit_short()? as i32;
            if size <= 0 {
                break;
            }

            // H: APPID handle.
            let app_handle = streams.object_reader.handle_reference()?;

            // Read `size` bytes of xdata.
            let end_pos = streams.object_reader.position().saturating_add(size as u64);
            let mut values = Vec::new();

            while (streams.object_reader.position() as i32) < end_pos as i32 {
                let code = streams.object_reader.read_raw_char()? as i32;
                let xval = match code {
                    // 0 = string (RC length + chars)
                    0 => {
                        let len = streams.object_reader.read_raw_char()? as usize;
                        let _codepage = streams.object_reader.read_raw_short()?;
                        let bytes = streams.object_reader.read_bytes(len)?;
                        let s = String::from_utf8_lossy(&bytes).to_string();
                        XDataValue::String(s)
                    }
                    // 1 = control string (opening or closing brace)
                    1 => {
                        let rc = streams.object_reader.read_raw_char()?;
                        if rc == b'{' {
                            XDataValue::ControlString("{".to_string())
                        } else {
                            XDataValue::ControlString("}".to_string())
                        }
                    }
                    // 2 = layer handle
                    2 => {
                        let h = streams.object_reader.handle_reference()?;
                        XDataValue::LayerName(format!("{h:#X}"))
                    }
                    // 3 = binary chunk
                    3 => {
                        let len = streams.object_reader.read_raw_char()? as usize;
                        let bytes = streams.object_reader.read_bytes(len)?;
                        XDataValue::BinaryData(bytes)
                    }
                    // 4 = entity handle
                    4 => {
                        let h = streams.object_reader.handle_reference()?;
                        XDataValue::Handle(Handle::new(h))
                    }
                    // 5 = real / 3D point
                    5 => {
                        let x = streams.object_reader.read_raw_double()?;
                        let y = streams.object_reader.read_raw_double()?;
                        let z = streams.object_reader.read_raw_double()?;
                        XDataValue::Point3D(crate::types::Vector3::new(x, y, z))
                    }
                    // 10 = real
                    10 => {
                        let r = streams.object_reader.read_raw_double()?;
                        XDataValue::Real(r)
                    }
                    // 11 = short
                    11 => {
                        let s = streams.object_reader.read_raw_short()?;
                        XDataValue::Integer16(s)
                    }
                    // 12 = long
                    12 => {
                        let l = streams.object_reader.read_raw_long()?;
                        XDataValue::Integer32(l)
                    }
                    _ => {
                        // Unknown code — skip remaining bytes.
                        let remaining = end_pos.saturating_sub(streams.object_reader.position());
                        if remaining > 0 {
                            streams.object_reader.advance(remaining as usize);
                        }
                        break;
                    }
                };
                values.push(xval);
            }

            edata.add(app_handle, values);
        }

        Ok(edata)
    }

    // -----------------------------------------------------------------------
    // readXrefDependantBit
    // -----------------------------------------------------------------------

    /// Read the xref-dependent bit(s) for a table entry.
    ///
    /// Matches C# `readXrefDependantBit(TableEntry)`:
    /// - **R2007+**: BS (xrefindex, bit 0x100 = XrefDependent).
    /// - **Pre-R2007**: B (64-flag/Referenced) + BS (xrefindex+1) + B (Xdep).
    pub(super) fn read_xref_dependant_bit(
        &self,
        reader: &mut dyn IDwgStreamReader,
    ) -> Result<bool> {
        if self.sio.r2007_plus {
            // R2007+: xrefindex BS — bit 0x100 indicates xref-dependent.
            let xrefindex = reader.read_bit_short()?;
            let is_xref = (xrefindex & 0x100) != 0;
            Ok(is_xref)
        } else {
            // Pre-R2007: 64-flag (B) + xrefindex+1 (BS) + Xdep (B).
            let _referenced = reader.read_bit()?; // 64-flag
            let _xrefindex = reader.read_bit_short()?; // xref index + 1
            let is_xref = reader.read_bit()?; // Xdep
            Ok(is_xref)
        }
    }

    // -----------------------------------------------------------------------
    // Unknown entity / object helpers
    // -----------------------------------------------------------------------

    /// Read an unrecognised entity as an unknown entity.
    pub(super) fn read_unknown_entity(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<super::templates::CadTemplate> {
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let entity = crate::entities::UnknownEntity {
            common: entity_common,
            dxf_name: String::new(),
        };

        Ok(super::templates::CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: crate::entities::EntityType::Unknown(entity),
        })
    }

    /// Read an unrecognised non-graphical object as a generic object.
    pub(super) fn read_unknown_non_graphical_object(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<super::templates::CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        Ok(super::templates::CadTemplate::GenericObject {
            common: common_tmpl,
        })
    }

    /// Read a proxy entity.
    pub(super) fn read_proxy_entity(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<super::templates::CadTemplate> {
        // Proxy entities share the same common data.
        let (common_tmpl, ent_tmpl, entity_common) =
            self.read_common_entity_data(streams)?;

        let entity = crate::entities::UnknownEntity {
            common: entity_common,
            dxf_name: "AcDbProxyEntity".to_string(),
        };

        Ok(super::templates::CadTemplate::Entity {
            common: common_tmpl,
            entity_data: ent_tmpl,
            entity: crate::entities::EntityType::Unknown(entity),
        })
    }

    /// Read a proxy object.
    pub(super) fn read_proxy_object(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<super::templates::CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        Ok(super::templates::CadTemplate::GenericObject {
            common: common_tmpl,
        })
    }

    /// Read a placeholder object.
    pub(super) fn read_placeholder(
        &mut self,
        streams: &mut StreamSet,
    ) -> Result<super::templates::CadTemplate> {
        let common_tmpl = self.read_common_non_entity_data(streams)?;

        Ok(super::templates::CadTemplate::GenericObject {
            common: common_tmpl,
        })
    }
}
