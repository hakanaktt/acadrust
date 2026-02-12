//! Non-graphical object writers for the DWG object writer.
//!
//! Mirrors ACadSharp's `DwgObjectWriter.Objects.cs`.

use crate::error::Result;
use crate::io::dwg::object_type::DwgObjectType;
use crate::io::dwg::reference_type::DwgReferenceType;
use crate::objects::Dictionary;
use crate::types::Handle;

use super::DwgObjectWriter;

impl DwgObjectWriter {
    // -----------------------------------------------------------------------
    // DICTIONARY
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
    // PLACEHOLDER (generic non-graphical object)
    // -----------------------------------------------------------------------

    pub(super) fn write_generic_object(
        &mut self,
        obj_type: DwgObjectType,
        handle: u64,
        owner_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_object_writer();
        self.write_common_non_entity_data(
            &mut *writer,
            obj_type,
            handle,
            owner_handle,
            &[],
            None,
        )?;

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }
}
