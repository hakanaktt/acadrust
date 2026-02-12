//! Table control and table entry writers for the DWG object writer.
//!
//! Mirrors ACadSharp's `DwgObjectWriter` table methods and the reader's
//! `read_tables.rs`.

use crate::error::Result;
use crate::io::dwg::object_type::DwgObjectType;
use crate::io::dwg::reference_type::DwgReferenceType;
use crate::tables::*;

use super::DwgObjectWriter;

/// Which table control type is being written.
#[derive(Debug, Clone, Copy)]
pub enum TableControlType {
    BlockControl,
    LayerControl,
    TextStyleControl,
    LineTypeControl,
    ViewControl,
    UcsControl,
    VPortControl,
    AppIdControl,
    DimStyleControl,
}

impl DwgObjectWriter {
    // -----------------------------------------------------------------------
    // Generic table control (BLOCK_CONTROL, LAYER_CONTROL, etc.)
    // -----------------------------------------------------------------------

    pub(super) fn write_table_control(
        &mut self,
        _table_type: TableControlType,
        obj_type: DwgObjectType,
        handle: u64,
        owner_handle: u64,
        entry_handles: &[u64],
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

        // BL: number of entries
        writer.write_bit_long(entry_handles.len() as i32)?;

        // Entry handles (soft owner)
        for &eh in entry_handles {
            writer.handle_reference_typed(DwgReferenceType::SoftOwnership, eh)?;
        }

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    /// Write LTYPE_CONTROL (special: has extra ByLayer/ByBlock handles).
    pub(super) fn write_ltype_control(
        &mut self,
        handle: u64,
        owner_handle: u64,
        entry_handles: &[u64],
        bylayer_handle: u64,
        byblock_handle: u64,
    ) -> Result<()> {
        let (mut writer, _) = self.create_object_writer();
        self.write_common_non_entity_data(
            &mut *writer,
            DwgObjectType::LtypeControlObj,
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // BL: number of entries
        writer.write_bit_long(entry_handles.len() as i32)?;

        // Entry handles
        for &eh in entry_handles {
            writer.handle_reference_typed(DwgReferenceType::SoftOwnership, eh)?;
        }

        // ByLayer handle
        writer.handle_reference_typed(DwgReferenceType::SoftOwnership, bylayer_handle)?;
        // ByBlock handle
        writer.handle_reference_typed(DwgReferenceType::SoftOwnership, byblock_handle)?;

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // LAYER table entry
    // -----------------------------------------------------------------------

    pub(super) fn write_layer(
        &mut self,
        layer: &Layer,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = layer.handle.value();
        let (mut writer, _) = self.create_object_writer();
        self.write_common_non_entity_data(
            &mut *writer,
            DwgObjectType::Layer,
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // Name (TV)
        writer.write_variable_text(layer.name())?;

        // Xref dependent (B)
        writer.write_bit(false)?;

        if self.sio.r13_14_only {
            writer.write_bit(layer.flags.frozen)?;
            writer.write_bit(!layer.flags.off)?; // is_on
            let _frozen_new = false;
            writer.write_bit(_frozen_new)?;
            writer.write_bit(layer.flags.locked)?;
        }

        if self.sio.r2000_plus {
            let mut values: i16 = 0;
            if layer.flags.frozen {
                values |= 0x01;
            }
            if layer.flags.off {
                values |= 0x02; // off = bit 1 set
            }
            if layer.flags.locked {
                values |= 0x08;
            }
            if layer.is_plottable {
                values |= 0x10;
            }
            let lw = lineweight_to_index(layer.line_weight);
            values |= (lw as i16) << 5;
            writer.write_bit_short(values)?;
        }

        // Color (CMC)
        writer.write_cm_color(layer.color)?;

        // External reference block handle
        writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?;

        // R2000+: plotstyle handle
        if self.sio.r2000_plus {
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?;
        }

        // R2007+: material handle
        if self.sio.r2007_plus {
            writer.handle_reference_typed(
                DwgReferenceType::HardPointer,
                layer.material.value(),
            )?;
        }

        // Linetype handle
        writer.handle_reference_typed(
            DwgReferenceType::HardPointer,
            self.resolve_linetype_handle(&layer.line_type),
        )?;

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // TEXT_STYLE table entry
    // -----------------------------------------------------------------------

    pub(super) fn write_text_style(
        &mut self,
        style: &TextStyle,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = style.handle.value();
        let (mut writer, _) = self.create_object_writer();
        self.write_common_non_entity_data(
            &mut *writer,
            DwgObjectType::Style,
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // Name (TV)
        writer.write_variable_text(style.name())?;

        // Xref dependent (B)
        writer.write_bit(false)?;

        // Is vertical (B)
        writer.write_bit(false)?; // TextGenerationFlags has no vertical flag

        // Is shape file (B)
        writer.write_bit(false)?;

        // Fixed height (BD)
        writer.write_bit_double(style.height)?;

        // Width factor (BD)
        writer.write_bit_double(style.width_factor)?;

        // Oblique angle (BD)
        writer.write_bit_double(style.oblique_angle)?;

        // Generation flags (RC)
        let gen_flags: u8 = (if style.flags.backward { 2 } else { 0 })
            | (if style.flags.upside_down { 4 } else { 0 });
        writer.write_byte(gen_flags)?;

        // Last height used (BD)
        writer.write_bit_double(style.height)?;

        // Font file name (TV)
        writer.write_variable_text(&style.font_file)?;

        // Big font file name (TV)
        writer.write_variable_text(&style.big_font_file)?;

        // Style control object handle
        writer.handle_reference_typed(DwgReferenceType::HardPointer, owner_handle)?;

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // LTYPE table entry
    // -----------------------------------------------------------------------

    pub(super) fn write_linetype(
        &mut self,
        ltype: &LineType,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = ltype.handle.value();
        let (mut writer, _) = self.create_object_writer();
        self.write_common_non_entity_data(
            &mut *writer,
            DwgObjectType::Ltype,
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // Name (TV)
        writer.write_variable_text(ltype.name())?;

        // Xref dependent (B)
        writer.write_bit(false)?;

        // Description (TV)
        writer.write_variable_text(&ltype.description)?;

        // Pattern length (BD)
        writer.write_bit_double(ltype.pattern_length)?;

        // Alignment (RC) — always 'A'
        writer.write_byte(b'A')?;

        // Number of dashes (RC)
        let num_dashes = ltype.elements.len();
        writer.write_byte(num_dashes as u8)?;

        for elem in &ltype.elements {
            // Dash length (BD)
            writer.write_bit_double(elem.length)?;
            // Complex shape code (BS)
            writer.write_bit_short(0)?;
            // X offset (RD)
            writer.write_raw_double(0.0)?;
            // Y offset (RD)
            writer.write_raw_double(0.0)?;
            // Scale (BD)
            writer.write_bit_double(1.0)?;
            // Rotation (BD)
            writer.write_bit_double(0.0)?;
            // Shape flag (BS)
            writer.write_bit_short(0)?;
        }

        // R2004+: segment text strings
        if self.sio.r2004_plus {
            for _elem in &ltype.elements {
                writer.write_variable_text("")?;
            }
        }

        // LType control object handle
        writer.handle_reference_typed(DwgReferenceType::HardPointer, owner_handle)?;

        // Segment handles (one per dash)
        for _elem in &ltype.elements {
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?;
        }

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // BLOCK_HEADER (BLOCK_RECORD) table entry
    // -----------------------------------------------------------------------

    pub(super) fn write_block_header(
        &mut self,
        block: &BlockRecord,
        owner_handle: u64,
        entity_handles: &[u64],
        block_entity_handle: u64,
        end_block_handle: u64,
        layout_handle: u64,
    ) -> Result<()> {
        let handle = block.handle.value();
        let (mut writer, _) = self.create_object_writer();
        self.write_common_non_entity_data(
            &mut *writer,
            DwgObjectType::BlockHeader,
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // Name (TV)
        writer.write_variable_text(block.name())?;

        // Xref dependent (B)
        writer.write_bit(false)?;

        // Anonymous (B)
        writer.write_bit(block.flags.anonymous)?;

        // Has attributes (B)
        writer.write_bit(false)?;

        // Is xref (B)
        writer.write_bit(false)?;

        // Is xref overlay (B)
        writer.write_bit(false)?;

        // R2000+: load xref (B)
        if self.sio.r2000_plus {
            writer.write_bit(false)?;
        }

        // R2004+: owned object count
        if self.sio.r2004_plus {
            writer.write_bit_long(entity_handles.len() as i32)?;
        }

        // Base point (3BD)
        writer.write_3bit_double(crate::types::Vector3::ZERO)?;

        // Xref path name (TV)
        writer.write_variable_text("")?;

        // R2000+: insert count + description + preview
        if self.sio.r2000_plus {
            // Insert count: sequence of non-zero RC terminated by 0 RC
            writer.write_byte(0)?; // immediate terminator (0 inserts)

            // Description (TV)
            writer.write_variable_text("")?;

            // Preview data size (BL) + bytes
            writer.write_bit_long(0)?;
        }

        // R2007+: insert units, explodable, block scaling
        if self.sio.r2007_plus {
            writer.write_bit_short(block.units as i16)?;
            writer.write_bit(block.explodable)?;
            writer.write_byte(if block.scale_uniformly { 1 } else { 0 })?;
        }

        // Block control handle
        writer.handle_reference_typed(DwgReferenceType::HardPointer, owner_handle)?;

        // Block entity handle (BLOCK start)
        writer.handle_reference_typed(
            DwgReferenceType::HardOwnership,
            block_entity_handle,
        )?;

        // R13-R2000: first and last entity handles (not xref)
        if !self.sio.r2004_plus {
            let first = entity_handles.first().copied().unwrap_or(0);
            let last = entity_handles.last().copied().unwrap_or(0);
            writer.handle_reference_typed(DwgReferenceType::HardPointer, first)?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, last)?;
        }

        // R2004+: owned object handles
        if self.sio.r2004_plus {
            for &eh in entity_handles {
                writer.handle_reference_typed(DwgReferenceType::HardOwnership, eh)?;
            }
        }

        // End block entity handle (ENDBLK)
        writer.handle_reference_typed(DwgReferenceType::HardOwnership, end_block_handle)?;

        // R2000+: layout handle
        if self.sio.r2000_plus {
            // 0 insert handles (since we wrote 0 insert count)
            writer.handle_reference_typed(DwgReferenceType::HardPointer, layout_handle)?;
        }

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // APPID table entry
    // -----------------------------------------------------------------------

    pub(super) fn write_appid(
        &mut self,
        appid: &AppId,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = appid.handle.value();
        let (mut writer, _) = self.create_object_writer();
        self.write_common_non_entity_data(
            &mut *writer,
            DwgObjectType::Appid,
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // Name (TV)
        writer.write_variable_text(appid.name())?;

        // Xref dependent (B)
        writer.write_bit(false)?;

        // Unknown (RC)
        writer.write_byte(0)?;

        // AppId control object handle
        writer.handle_reference_typed(DwgReferenceType::HardPointer, owner_handle)?;

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // DIMSTYLE table entry (simplified — just name + handles)
    // -----------------------------------------------------------------------

    pub(super) fn write_dimstyle(
        &mut self,
        dimstyle: &DimStyle,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = dimstyle.handle.value();
        let (mut writer, _) = self.create_object_writer();
        self.write_common_non_entity_data(
            &mut *writer,
            DwgObjectType::Dimstyle,
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // Name (TV)
        writer.write_variable_text(dimstyle.name())?;

        // Xref dependent (B)
        writer.write_bit(false)?;

        // Write the dimension variables — this is version-specific.
        // For simplicity, we write the R2000+ layout which covers
        // the most common case.
        if self.sio.r2000_plus {
            // DIMPOST, DIMAPOST
            writer.write_variable_text("")?;
            writer.write_variable_text("")?;

            // Dimension style variables (BD/B/BS/CMC etc)
            writer.write_bit_double(dimstyle.dimscale)?;
            writer.write_bit_double(dimstyle.dimasz)?;
            writer.write_bit_double(dimstyle.dimexo)?;
            writer.write_bit_double(dimstyle.dimdli)?;
            writer.write_bit_double(dimstyle.dimexe)?;
            writer.write_bit_double(dimstyle.dimrnd)?;
            writer.write_bit_double(dimstyle.dimdle)?;
            writer.write_bit_double(dimstyle.dimtp)?;
            writer.write_bit_double(dimstyle.dimtm)?;

            // R2007+: extra fields
            if self.sio.r2007_plus {
                writer.write_bit_double(0.0)?; // dimfxl
                writer.write_bit(false)?;       // dimfxlon
                writer.write_bit_double(0.0)?; // dimjogang
                writer.write_bit_short(0)?;     // dimtfill
                writer.write_cm_color(crate::types::Color::ByBlock)?; // dimtfillclr
            }

            // Boolean flags
            writer.write_bit(dimstyle.dimtol)?;
            writer.write_bit(dimstyle.dimlim)?;
            writer.write_bit(dimstyle.dimtih)?;
            writer.write_bit(dimstyle.dimtoh)?;
            writer.write_bit(dimstyle.dimse1)?;
            writer.write_bit(dimstyle.dimse2)?;
            writer.write_bit_short(dimstyle.dimtad)?;
            writer.write_bit_short(dimstyle.dimzin)?;
            writer.write_bit_short(0)?; // dimazin

            if self.sio.r2007_plus {
                writer.write_bit_short(0)?; // dimarcsym
            }

            writer.write_bit_double(dimstyle.dimtxt)?;
            writer.write_bit_double(dimstyle.dimcen)?;
            writer.write_bit_double(dimstyle.dimtsz)?;
            writer.write_bit_double(dimstyle.dimaltf)?;
            writer.write_bit_double(dimstyle.dimlfac)?;
            writer.write_bit_double(dimstyle.dimtvp)?;
            writer.write_bit_double(dimstyle.dimtfac)?;
            writer.write_bit_double(dimstyle.dimgap)?;
            writer.write_bit_double(0.0)?; // dimaltrnd
            writer.write_bit(dimstyle.dimalt)?;
            writer.write_bit_short(dimstyle.dimaltd)?;
            writer.write_bit(dimstyle.dimtofl)?;
            writer.write_bit(dimstyle.dimsah)?;
            writer.write_bit(dimstyle.dimtix)?;
            writer.write_bit(dimstyle.dimsoxd)?;
            writer.write_cm_color(crate::types::Color::from_index(dimstyle.dimclrd))?;
            writer.write_cm_color(crate::types::Color::from_index(dimstyle.dimclre))?;
            writer.write_cm_color(crate::types::Color::from_index(dimstyle.dimclrt))?;
            writer.write_bit_short(0)?; // dimadec
            writer.write_bit_short(dimstyle.dimdec)?;
            writer.write_bit_short(dimstyle.dimtdec)?;
            writer.write_bit_short(dimstyle.dimaltu)?;
            writer.write_bit_short(dimstyle.dimalttd)?;
            writer.write_bit_short(dimstyle.dimaunit)?;
            writer.write_bit_short(0)?; // dimatfit
            writer.write_bit_short(dimstyle.dimunit)?;
            writer.write_bit_short(dimstyle.dimlunit)?;
            writer.write_bit_short(dimstyle.dimdsep)?;
            writer.write_bit_short(0)?; // dimtmove
            writer.write_bit_short(dimstyle.dimjust)?;
            writer.write_bit(dimstyle.dimsd1)?;
            writer.write_bit(dimstyle.dimsd2)?;
            writer.write_bit_short(dimstyle.dimtolj)?;
            writer.write_bit_short(dimstyle.dimtzin)?;
            writer.write_bit_short(dimstyle.dimaltz)?;
            writer.write_bit_short(dimstyle.dimalttz)?;
            writer.write_bit(dimstyle.dimupt)?;
            writer.write_bit_short(dimstyle.dimfit)?;

            if self.sio.r2007_plus {
                writer.write_bit(false)?; // txtdirection
            }

            writer.write_bit_double(dimstyle.dimscale)?; // dimscale (repeated)
            writer.write_bit_double(0.0)?; // dimmzf
            writer.write_bit_short(0)?;    // dimmzs

            if self.sio.r2010_plus {
                writer.write_bit_short(0)?; // dimfrac
            }
        } else {
            // R13-R14 layout (simplified)
            writer.write_bit(dimstyle.dimtol)?;
            writer.write_bit(dimstyle.dimlim)?;
            writer.write_bit(dimstyle.dimtih)?;
            writer.write_bit(dimstyle.dimtoh)?;
            writer.write_bit(dimstyle.dimse1)?;
            writer.write_bit(dimstyle.dimse2)?;
            writer.write_bit(dimstyle.dimalt)?;
            writer.write_bit(dimstyle.dimtofl)?;
            writer.write_bit(dimstyle.dimsah)?;
            writer.write_bit(dimstyle.dimtix)?;
            writer.write_bit(dimstyle.dimsoxd)?;
            writer.write_byte(dimstyle.dimaltd as u8)?;
            writer.write_byte(dimstyle.dimzin as u8)?;
            writer.write_bit(dimstyle.dimsd1)?;
            writer.write_bit(dimstyle.dimsd2)?;
            writer.write_byte(dimstyle.dimtolj as u8)?;
            writer.write_byte(dimstyle.dimjust as u8)?;
            writer.write_byte(dimstyle.dimfit as u8)?;
            writer.write_bit(dimstyle.dimupt)?;
            writer.write_byte(dimstyle.dimtzin as u8)?;
            writer.write_byte(dimstyle.dimaltz as u8)?;
            writer.write_byte(dimstyle.dimalttz as u8)?;
            writer.write_byte(dimstyle.dimtad as u8)?;
            writer.write_bit_short(dimstyle.dimunit)?;
            writer.write_bit_short(dimstyle.dimaunit)?;
            writer.write_bit_short(dimstyle.dimdec)?;
            writer.write_bit_short(dimstyle.dimtdec)?;
            writer.write_bit_short(dimstyle.dimaltu)?;
            writer.write_bit_short(dimstyle.dimalttd)?;
            // dimtxsty handle handled below
            writer.write_bit_double(dimstyle.dimscale)?;
            writer.write_bit_double(dimstyle.dimasz)?;
            writer.write_bit_double(dimstyle.dimexo)?;
            writer.write_bit_double(dimstyle.dimdli)?;
            writer.write_bit_double(dimstyle.dimexe)?;
            writer.write_bit_double(dimstyle.dimrnd)?;
            writer.write_bit_double(dimstyle.dimdle)?;
            writer.write_bit_double(dimstyle.dimtp)?;
            writer.write_bit_double(dimstyle.dimtm)?;
            writer.write_bit_double(dimstyle.dimtxt)?;
            writer.write_bit_double(dimstyle.dimcen)?;
            writer.write_bit_double(dimstyle.dimtsz)?;
            writer.write_bit_double(dimstyle.dimaltf)?;
            writer.write_bit_double(dimstyle.dimlfac)?;
            writer.write_bit_double(dimstyle.dimtvp)?;
            writer.write_bit_double(dimstyle.dimtfac)?;
            writer.write_bit_double(dimstyle.dimgap)?;
            writer.write_variable_text("")?; // dimpost
            writer.write_variable_text("")?; // dimapost
            writer.write_variable_text("")?; // dimblk
            writer.write_variable_text("")?; // dimblk1
            writer.write_variable_text("")?; // dimblk2
            writer.write_bit_short(dimstyle.dimclrd)?;
            writer.write_bit_short(dimstyle.dimclre)?;
            writer.write_bit_short(dimstyle.dimclrt)?;
        }

        // DimStyle control object handle
        writer.handle_reference_typed(DwgReferenceType::HardPointer, owner_handle)?;

        // DIMTXSTY handle (text style)
        writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?;

        // R2000+: block sub-object handles
        if self.sio.r2000_plus {
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?; // dimldrblk
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?; // dimblk
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?; // dimblk1
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?; // dimblk2
        }

        // R2007+: dimline type handles
        if self.sio.r2007_plus {
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?; // dimltype
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?; // dimltex1
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?; // dimltex2
        }

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // VPORT table entry
    // -----------------------------------------------------------------------

    pub(super) fn write_vport(
        &mut self,
        vport: &VPort,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = vport.handle.value();
        let (mut writer, _) = self.create_object_writer();
        self.write_common_non_entity_data(
            &mut *writer,
            DwgObjectType::Vport,
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // Name (TV)
        writer.write_variable_text(vport.name())?;

        // Xref dependent (B)
        writer.write_bit(false)?;

        // View height (BD)
        writer.write_bit_double(vport.view_height)?;

        // Aspect ratio (BD)
        writer.write_bit_double(vport.aspect_ratio)?;

        // View center (2RD)
        writer.write_2raw_double(vport.view_center)?;

        // View target (3BD)
        writer.write_3bit_double(vport.view_target)?;

        // View direction (3BD)
        writer.write_3bit_double(vport.view_direction)?;

        // Twist angle (BD)
        writer.write_bit_double(0.0)?;

        // Lens length (BD)
        writer.write_bit_double(vport.lens_length)?;

        // Front clip (BD)
        writer.write_bit_double(0.0)?;

        // Back clip (BD)
        writer.write_bit_double(0.0)?;

        // View mode (BL)
        writer.write_bit_long(0)?;

        // Render mode (RC)
        writer.write_byte(0)?;

        // R2000+
        if self.sio.r2000_plus {
            writer.write_bit(true)?;  // use default lighting
            writer.write_byte(1)?;     // default lighting type
            writer.write_bit_double(0.0)?; // brightness
            writer.write_bit_double(0.0)?; // contrast
            writer.write_raw_long(0)?;     // ambient color
        }

        // Lower-left corner (2RD)
        writer.write_2raw_double(vport.lower_left)?;

        // Upper-right corner (2RD)
        writer.write_2raw_double(vport.upper_right)?;

        // UCSFOLLOW (B)
        writer.write_bit(false)?;

        // Circle sides (BS)
        writer.write_bit_short(1000)?;

        // Snap settings
        writer.write_bit(false)?; // snap on
        writer.write_bit(false)?; // snap style
        writer.write_bit_short(0)?; // snap isopair
        writer.write_bit_double(0.0)?; // snap rotation
        writer.write_2raw_double(vport.snap_base)?;
        writer.write_2raw_double(vport.snap_spacing)?;

        // Grid settings
        writer.write_bit(false)?; // grid on
        writer.write_2raw_double(vport.grid_spacing)?;

        // UCS flags (R2000+)
        if self.sio.r2000_plus {
            writer.write_bit(false)?; // ucs per viewport
            writer.write_3bit_double(crate::types::Vector3::ZERO)?; // UCS origin
            writer.write_3bit_double(crate::types::Vector3::UNIT_X)?; // UCS X
            writer.write_3bit_double(crate::types::Vector3::UNIT_Y)?; // UCS Y
            writer.write_bit_double(0.0)?; // UCS elevation
            writer.write_bit_short(0)?;     // UCS ortho type
        }

        if self.sio.r2007_plus {
            writer.write_bit_short(0)?; // grid flags
            writer.write_bit_short(5)?; // grid major
        }

        // VPort control object handle
        writer.handle_reference_typed(DwgReferenceType::HardPointer, owner_handle)?;

        // R2007+: background, visual style, sun handles
        if self.sio.r2007_plus {
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?;
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?;
        }

        if self.sio.r2000_plus {
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?; // named UCS
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?; // base UCS
        }

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // VIEW table entry
    // -----------------------------------------------------------------------

    pub(super) fn write_view(
        &mut self,
        view: &View,
        owner_handle: u64,
    ) -> Result<()> {
        let handle = view.handle.value();
        let (mut writer, _) = self.create_object_writer();
        self.write_common_non_entity_data(
            &mut *writer,
            DwgObjectType::View,
            handle,
            owner_handle,
            &[],
            None,
        )?;

        // Name (TV)
        writer.write_variable_text(view.name())?;

        // Xref dependent (B)
        writer.write_bit(false)?;

        // View height, width (BD)
        writer.write_bit_double(view.height)?;
        writer.write_bit_double(view.width)?;

        // View center (2RD)
        writer.write_2raw_double(crate::types::Vector2::new(view.center.x, view.center.y))?;

        // View target (3BD)
        writer.write_3bit_double(view.target)?;

        // View direction (3BD)
        writer.write_3bit_double(view.direction)?;

        // Twist angle (BD)
        writer.write_bit_double(view.twist_angle)?;

        // Lens length (BD)
        writer.write_bit_double(view.lens_length)?;

        // Front clip, back clip (BD)
        writer.write_bit_double(view.front_clip)?;
        writer.write_bit_double(view.back_clip)?;

        // UCSFOLLOW, Front on, Back on (B)
        writer.write_bit(false)?;
        writer.write_bit(false)?;
        writer.write_bit(false)?;

        if self.sio.r2000_plus {
            writer.write_byte(0)?; // render mode
            writer.write_bit(false)?; // associated_ucs
            // UCS info
            writer.write_3bit_double(crate::types::Vector3::ZERO)?;
            writer.write_3bit_double(crate::types::Vector3::UNIT_X)?;
            writer.write_3bit_double(crate::types::Vector3::UNIT_Y)?;
            writer.write_bit_double(0.0)?; // UCS elevation
            writer.write_bit_short(0)?;     // ortho type
        }

        // View control object handle
        writer.handle_reference_typed(DwgReferenceType::HardPointer, owner_handle)?;

        if self.sio.r2007_plus {
            writer.write_bit(false)?; // camera plottable
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?; // background
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?; // visual style
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?; // sun
        }

        // associated_ucs = false, so no UCS handle

        if self.sio.r2007_plus {
            writer.handle_reference_typed(DwgReferenceType::HardPointer, 0)?; // live section
        }

        writer.write_spear_shift()?;
        self.finalize_object(writer, handle);
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a `LineWeight` to its 5-bit index value.
fn lineweight_to_index(lw: crate::types::LineWeight) -> u8 {
    match lw {
        crate::types::LineWeight::Default => 0x1F,
        crate::types::LineWeight::ByBlock => 0x1E,
        _ => {
            let val = lw.value();
            // Map the DXF lineweight value to the 5-bit index
            match val {
                0 => 0,
                5 => 1,
                9 => 2,
                13 => 3,
                15 => 4,
                18 => 5,
                20 => 6,
                25 => 7,
                30 => 8,
                35 => 9,
                40 => 10,
                50 => 11,
                53 => 12,
                60 => 13,
                70 => 14,
                80 => 15,
                90 => 16,
                100 => 17,
                106 => 18,
                120 => 19,
                140 => 20,
                158 => 21,
                200 => 22,
                211 => 23,
                _ => 0,
            }
        }
    }
}
