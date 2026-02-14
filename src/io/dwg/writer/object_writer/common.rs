//! Common entity/object data writing methods for the DWG object writer.
//!
//! Mirrors ACadSharp's `DwgObjectWriter.Common.cs`.
//!
//! Provides:
//! - `register_object` — finalize an object (size, CRC, add to handle map)
//! - `write_common_data` — object type + handle
//! - `write_common_entity_data` — entity mode, color, layer, linetype, etc.
//! - `write_common_non_entity_data` — owner, reactors, xdict
//! - `write_extended_data` — EED / XDATA

use crate::entities::EntityCommon;
use crate::error::Result;
use crate::io::dwg::crc;
use crate::io::dwg::object_type::DwgObjectType;
use crate::io::dwg::reference_type::DwgReferenceType;
use crate::io::dwg::writer::stream_writer::IDwgStreamWriter;
use crate::types::{Handle, Transparency};

use super::DwgObjectWriter;

impl DwgObjectWriter {
    // -----------------------------------------------------------------------
    // register_object — finalize written object data
    // -----------------------------------------------------------------------

    /// Finalize the object: write modular short size prefix + CRC16,
    /// and record its (handle, offset) pair in the handle map.
    pub(super) fn register_object(&mut self, handle: u64, object_data: Vec<u8>) {
        let position = self.objects_stream.len() as i64;

        // Write MS (Modular Short) size prefix
        Self::write_modular_short(&mut self.objects_stream, object_data.len() as u32);

        // R2010+: write handle stream size in bits as MC
        if self.sio.r2010_plus {
            // The handle stream size is encoded in the merged writer's
            // saved_position_in_bits after write_spear_shift.
            // For now we encode 0 (updated later if needed).
            // The actual handle size bits was saved during write_spear_shift.
            Self::write_modular_char(&mut self.objects_stream, self.last_handle_size_bits as i32);
        }

        // Write the object data
        let crc_start = self.objects_stream.len();
        self.objects_stream.extend_from_slice(&object_data);

        // CRC-8 (16-bit) over the object data (little-endian)
        let crc_val = crc::crc8(0xC0C1, &self.objects_stream[crc_start..]);
        self.objects_stream.push((crc_val & 0xFF) as u8);
        self.objects_stream.push((crc_val >> 8) as u8);

        // Record handle → offset
        self.handle_map.insert(handle, position);
    }

    /// Write a modular short (MS) to the output.
    fn write_modular_short(output: &mut Vec<u8>, mut value: u32) {
        loop {
            let low = (value & 0x7FFF) as u16;
            value >>= 15;
            if value != 0 {
                let encoded = low | 0x8000;
                output.push((encoded & 0xFF) as u8);
                output.push((encoded >> 8) as u8);
            } else {
                output.push((low & 0xFF) as u8);
                output.push((low >> 8) as u8);
                break;
            }
        }
    }

    /// Write a modular char (MC) to the output.
    fn write_modular_char(output: &mut Vec<u8>, value: i32) {
        let neg = value < 0;
        let mut uval = if neg { (-value) as u32 } else { value as u32 };

        loop {
            let mut byte = (uval & 0x7F) as u8;
            uval >>= 7;
            if uval != 0 {
                byte |= 0x80;
                output.push(byte);
            } else {
                if neg {
                    byte |= 0x40;
                }
                output.push(byte);
                break;
            }
        }
    }

    // -----------------------------------------------------------------------
    // write_common_data — object type + handle
    // -----------------------------------------------------------------------

    /// Write the object type code and handle. Resets the merged writer.
    pub(super) fn write_common_data_entity(
        &mut self,
        writer: &mut dyn IDwgStreamWriter,
        obj_type: DwgObjectType,
        handle: u64,
    ) -> Result<()> {
        writer.reset_stream()?;
        writer.write_object_type(obj_type.as_raw())?;

        // R2000-R2007: Save position for size-in-bits (patched later)
        if self.sio.r2000_plus && !self.sio.r2010_plus {
            writer.save_position_for_size()?;
        }

        // Object's own handle must go to the MAIN stream (not handle sub-stream).
        // The reader reads it from object_reader (main stream).
        writer.handle_reference_on_main(handle)?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // write_common_entity_data
    // -----------------------------------------------------------------------

    /// Write common entity data fields.
    ///
    /// Mirrors the reader's `read_common_entity_data()`.
    pub(super) fn write_common_entity_data(
        &mut self,
        writer: &mut dyn IDwgStreamWriter,
        obj_type: DwgObjectType,
        common: &EntityCommon,
        owner_handle: u64,
    ) -> Result<()> {
        self.write_common_data_entity(writer, obj_type, common.handle.value())?;

        // EED (extended entity data) — write empty for now
        self.write_extended_data(writer, common)?;

        // Graphic present flag (B) — always false
        writer.write_bit(false)?;

        // R13-R14: save position for size
        if self.sio.r13_14_only {
            writer.save_position_for_size()?;
        }

        // Entity mode (BB)
        // 0 = owner handle present, 1 = paper space, 2 = model space
        let entity_mode = if owner_handle == self.model_space_handle {
            2u8
        } else if owner_handle == self.paper_space_handle {
            1u8
        } else {
            0u8
        };
        writer.write_2bits(entity_mode)?;

        // When entity_mode == 0, write the owner handle
        if entity_mode == 0 {
            writer.handle_reference_typed(DwgReferenceType::SoftPointer, owner_handle)?;
        }

        // Reactors and XDictionary
        self.write_reactors_and_xdictionary(writer, common)?;

        // R13-R14: layer handle, linetype
        if self.sio.r13_14_only {
            // Layer handle
            writer.handle_reference_typed(DwgReferenceType::SoftPointer,
                self.resolve_layer_handle(&common.layer))?;
            // Isbylayerlt B
            writer.write_bit(true)?; // bylayer linetype for simplicity
        }

        // R13-R2000: prev/next entity handles (linked list)
        if !self.sio.r2004_plus {
            // no_links = true (no prev/next for simplicity)
            writer.write_bit(true)?;
        }

        // Color + transparency
        let _is_by_layer = common.transparency == Transparency::BY_LAYER;
        writer.write_en_color(common.color, common.transparency, false)?;

        // Linetype scale (BD)
        writer.write_bit_double(1.0)?; // default linetype scale

        // R2000+: linetype flags, layer handle, etc.
        if self.sio.r2000_plus {
            // Layer handle (H)
            writer.handle_reference_typed(DwgReferenceType::SoftPointer,
                self.resolve_layer_handle(&common.layer))?;

            // Linetype flags (BB): 00=bylayer
            writer.write_2bits(0)?; // bylayer

            if self.sio.r2007_plus {
                // Material flags (BB): 00=bylayer
                writer.write_2bits(0)?;
                // Shadow flags RC
                writer.write_byte(0)?;
            }

            // Plotstyle flags (BB): 00=bylayer
            writer.write_2bits(0)?;

            if self.sio.r2010_plus {
                // has full visual style (B), face visual style (B), edge visual style (B)
                writer.write_bit(false)?;
                writer.write_bit(false)?;
                writer.write_bit(false)?;
            }
        }

        // Invisibility (BS)
        writer.write_bit_short(if common.invisible { 1 } else { 0 })?;

        // Lineweight (RC)
        if self.sio.r2000_plus {
            writer.write_byte(common.line_weight.value() as u8)?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // write_common_non_entity_data
    // -----------------------------------------------------------------------

    /// Write common non-entity data fields (for table entries, objects).
    pub(super) fn write_common_non_entity_data(
        &mut self,
        writer: &mut dyn IDwgStreamWriter,
        obj_type: DwgObjectType,
        handle: u64,
        owner_handle: u64,
        reactors: &[Handle],
        xdictionary_handle: Option<Handle>,
    ) -> Result<()> {
        self.write_common_data_entity(writer, obj_type, handle)?;

        // EED — empty
        writer.write_bit_short(0)?;

        // R13-R14: save position for size
        if self.sio.r13_14_only {
            writer.save_position_for_size()?;
        }

        // Number of reactors (BL)
        writer.write_bit_long(reactors.len() as i32)?;

        // R2004+: xdictionary-missing flag (B)
        if self.sio.r2004_plus {
            writer.write_bit(xdictionary_handle.is_none())?;
        }

        // R2013+: has-ds-binary-data (B)
        if self.sio.r2013_plus {
            writer.write_bit(false)?;
        }

        // Owner handle (soft pointer)
        writer.handle_reference_typed(DwgReferenceType::SoftPointer, owner_handle)?;

        // Reactor handles
        for r in reactors {
            writer.handle_reference_typed(DwgReferenceType::HardPointer, r.value())?;
        }

        // XDictionary handle (hard owner)
        // For pre-R2004, there is no xdictionary-missing flag, so the handle
        // must ALWAYS be present (use handle 0 for "no xdictionary").
        if let Some(xdict) = xdictionary_handle {
            writer.handle_reference_typed(DwgReferenceType::HardOwnership, xdict.value())?;
        } else if !self.sio.r2004_plus {
            writer.handle_reference_typed(DwgReferenceType::HardOwnership, 0)?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // write_reactors_and_xdictionary (entity version)
    // -----------------------------------------------------------------------

    fn write_reactors_and_xdictionary(
        &self,
        writer: &mut dyn IDwgStreamWriter,
        common: &EntityCommon,
    ) -> Result<()> {
        // BL: number of reactors
        writer.write_bit_long(common.reactors.len() as i32)?;

        // R2004+: xdictionary-missing flag
        if self.sio.r2004_plus {
            writer.write_bit(common.xdictionary_handle.is_none())?;
        }

        // R2013+: has-ds-binary-data
        if self.sio.r2013_plus {
            writer.write_bit(false)?;
        }

        // Reactor handles
        for r in &common.reactors {
            writer.handle_reference_typed(DwgReferenceType::HardPointer, r.value())?;
        }

        // XDictionary handle
        // For pre-R2004, there is no xdictionary-missing flag, so the handle
        // must ALWAYS be present (use handle 0 for "no xdictionary").
        if let Some(xdict) = common.xdictionary_handle {
            writer.handle_reference_typed(DwgReferenceType::HardOwnership, xdict.value())?;
        } else if !self.sio.r2004_plus {
            writer.handle_reference_typed(DwgReferenceType::HardOwnership, 0)?;
        }

        Ok(())
    }

    // -----------------------------------------------------------------------
    // write_extended_data (EED)
    // -----------------------------------------------------------------------

    fn write_extended_data(
        &self,
        writer: &mut dyn IDwgStreamWriter,
        _common: &EntityCommon,
    ) -> Result<()> {
        // Write empty EED (BS: 0)
        // TODO: Support writing actual xdata when present
        writer.write_bit_short(0)?;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Resolve a layer name to its handle.
    pub(super) fn resolve_layer_handle(&self, layer_name: &str) -> u64 {
        self.layer_handles
            .get(&layer_name.to_uppercase())
            .copied()
            .unwrap_or(self.default_layer_handle)
    }

    /// Resolve a linetype name to its handle.
    pub(super) fn resolve_linetype_handle(&self, ltype_name: &str) -> u64 {
        self.linetype_handles
            .get(&ltype_name.to_uppercase())
            .copied()
            .unwrap_or(0)
    }

    /// Resolve a text style name to its handle.
    pub(super) fn resolve_textstyle_handle(&self, style_name: &str) -> u64 {
        self.textstyle_handles
            .get(&style_name.to_uppercase())
            .copied()
            .unwrap_or(self.default_textstyle_handle)
    }

    /// Resolve a block name to its handle.
    pub(super) fn resolve_block_handle(&self, block_name: &str) -> u64 {
        self.block_handles
            .get(&block_name.to_uppercase())
            .copied()
            .unwrap_or(0)
    }

    /// Resolve a dimension style name to its handle.
    pub(super) fn resolve_dimstyle_handle(&self, style_name: &str) -> u64 {
        self.dimstyle_handles
            .get(&style_name.to_uppercase())
            .copied()
            .unwrap_or(0)
    }

    /// Resolve a DXF class name to its class_number (for unlisted types).
    /// Returns 500 as fallback.
    pub(super) fn resolve_class_number(&self, dxf_class_name: &str) -> i16 {
        self.class_numbers
            .get(&dxf_class_name.to_uppercase())
            .copied()
            .unwrap_or(500)
    }

    /// Write common non-entity data fields for an **unlisted** (class-based) object type.
    ///
    /// Instead of passing a `DwgObjectType` enum, this uses the DXF class name
    /// to look up the class_number and write that as the object type code.
    pub(super) fn write_common_non_entity_data_unlisted(
        &mut self,
        writer: &mut dyn IDwgStreamWriter,
        dxf_class_name: &str,
        handle: u64,
        owner_handle: u64,
        reactors: &[Handle],
        xdictionary_handle: Option<Handle>,
    ) -> Result<()> {
        let class_number = self.resolve_class_number(dxf_class_name);

        // Write type code + handle
        writer.reset_stream()?;
        writer.write_object_type(class_number)?;

        if self.sio.r2000_plus && !self.sio.r2010_plus {
            writer.save_position_for_size()?;
        }

        writer.handle_reference(handle)?;

        // EED — empty
        writer.write_bit_short(0)?;

        // R13-R14: save position for size
        if self.sio.r13_14_only {
            writer.save_position_for_size()?;
        }

        // Number of reactors (BL)
        writer.write_bit_long(reactors.len() as i32)?;

        // R2004+: xdictionary-missing flag (B)
        if self.sio.r2004_plus {
            writer.write_bit(xdictionary_handle.is_none())?;
        }

        // R2013+: has-ds-binary-data (B)
        if self.sio.r2013_plus {
            writer.write_bit(false)?;
        }

        // Owner handle (soft pointer)
        writer.handle_reference_typed(DwgReferenceType::SoftPointer, owner_handle)?;

        // Reactor handles
        for r in reactors {
            writer.handle_reference_typed(DwgReferenceType::HardPointer, r.value())?;
        }

        // XDictionary handle (hard owner)
        // For pre-R2004, there is no xdictionary-missing flag, so the handle
        // must ALWAYS be present (use handle 0 for "no xdictionary").
        if let Some(xdict) = xdictionary_handle {
            writer.handle_reference_typed(DwgReferenceType::HardOwnership, xdict.value())?;
        } else if !self.sio.r2004_plus {
            writer.handle_reference_typed(DwgReferenceType::HardOwnership, 0)?;
        }

        Ok(())
    }

    /// Write common entity data for an **unlisted** (class-based) entity type.
    ///
    /// Instead of passing a `DwgObjectType` enum, this uses the DXF class
    /// name (e.g., "MULTILEADER", "IMAGE", "WIPEOUT") to look up the
    /// class_number and write that as the object type code.
    pub(super) fn write_common_entity_data_unlisted(
        &mut self,
        writer: &mut dyn IDwgStreamWriter,
        dxf_class_name: &str,
        common: &EntityCommon,
        owner_handle: u64,
    ) -> Result<()> {
        let class_number = self.resolve_class_number(dxf_class_name);

        // Write type code + handle (same as write_common_data_entity but with raw class_number)
        writer.reset_stream()?;
        writer.write_object_type(class_number)?;

        if self.sio.r2000_plus && !self.sio.r2010_plus {
            writer.save_position_for_size()?;
        }

        writer.handle_reference_on_main(common.handle.value())?;

        // Now write the standard entity header fields
        self.write_extended_data(writer, common)?;

        // Graphic present flag
        writer.write_bit(false)?;

        if self.sio.r13_14_only {
            writer.save_position_for_size()?;
        }

        // Entity mode
        let entity_mode = if owner_handle == self.model_space_handle {
            2u8
        } else if owner_handle == self.paper_space_handle {
            1u8
        } else {
            0u8
        };
        writer.write_2bits(entity_mode)?;

        if entity_mode == 0 {
            writer.handle_reference_typed(DwgReferenceType::SoftPointer, owner_handle)?;
        }

        self.write_reactors_and_xdictionary(writer, common)?;

        if self.sio.r13_14_only {
            writer.handle_reference_typed(
                DwgReferenceType::SoftPointer,
                self.resolve_layer_handle(&common.layer),
            )?;
            writer.write_bit(true)?;
        }

        if !self.sio.r2004_plus {
            writer.write_bit(true)?; // no_links
        }

        writer.write_en_color(common.color, common.transparency, false)?;
        writer.write_bit_double(1.0)?; // linetype scale

        if self.sio.r2000_plus {
            writer.handle_reference_typed(
                DwgReferenceType::SoftPointer,
                self.resolve_layer_handle(&common.layer),
            )?;
            writer.write_2bits(0)?; // linetype flags = bylayer

            if self.sio.r2007_plus {
                writer.write_2bits(0)?; // material flags
                writer.write_byte(0)?;  // shadow flags
            }

            writer.write_2bits(0)?; // plotstyle flags

            if self.sio.r2010_plus {
                writer.write_bit(false)?; // full visual style
                writer.write_bit(false)?; // face visual style
                writer.write_bit(false)?; // edge visual style
            }
        }

        writer.write_bit_short(if common.invisible { 1 } else { 0 })?;

        if self.sio.r2000_plus {
            writer.write_byte(common.line_weight.value() as u8)?;
        }

        Ok(())
    }
}
