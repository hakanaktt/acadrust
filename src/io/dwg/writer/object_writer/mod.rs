//! DWG Object Writer — writes all objects (entities, table entries, non-graphical
//! objects) to the objects section of a DWG file.
//!
//! Mirrors ACadSharp's `DwgObjectWriter` class.
//!
//! The writer serializes every database object into a contiguous byte stream.
//! Each object is wrapped in a *Modular Short* (MS) size prefix followed by
//! its payload and a CRC-8 (16-bit) trailer. The (handle → offset) pairs are
//! recorded in a handle map so the *handle section writer* can emit the offset
//! table afterwards.

mod common;
mod write_entities;
mod write_objects;
mod write_tables;

use std::collections::{BTreeMap, HashMap};
use std::io::SeekFrom;

use crate::document::CadDocument;
use crate::error::Result;
use crate::io::dwg::section_io::SectionIO;
use crate::io::dwg::writer::merged_writer::{DwgMergedStreamWriter, DwgMergedStreamWriterAC14};
use crate::io::dwg::writer::stream_writer::IDwgStreamWriter;
use crate::objects::ObjectType;
use crate::tables::TableEntry;
use crate::types::DxfVersion;

use write_tables::TableControlType;

/// DWG Object Writer — responsible for encoding all database objects into
/// the objects section byte stream.
pub struct DwgObjectWriter {
    /// Version-flag helper.
    pub(super) sio: SectionIO,
    /// Accumulated objects data (MS + payload + CRC for each object).
    pub(super) objects_stream: Vec<u8>,
    /// Map of handle → byte offset within `objects_stream`.
    pub(super) handle_map: BTreeMap<u64, i64>,
    /// Bit-size of the last handle stream (for R2010+ MC encoding).
    pub(super) last_handle_size_bits: i64,

    // Well-known handles -------------------------------------------------
    pub(super) model_space_handle: u64,
    pub(super) paper_space_handle: u64,

    // Default fallback handles -------------------------------------------
    pub(super) default_layer_handle: u64,
    pub(super) default_textstyle_handle: u64,

    // Lookup tables: name (upper-cased) → handle -------------------------
    pub(super) layer_handles: HashMap<String, u64>,
    pub(super) linetype_handles: HashMap<String, u64>,
    pub(super) textstyle_handles: HashMap<String, u64>,
    pub(super) block_handles: HashMap<String, u64>,
    pub(super) dimstyle_handles: HashMap<String, u64>,
}

impl DwgObjectWriter {
    // -----------------------------------------------------------------------
    // Constructor
    // -----------------------------------------------------------------------

    /// Create a new `DwgObjectWriter` for the given version and document.
    pub fn new(version: DxfVersion, doc: &CadDocument) -> Self {
        let sio = SectionIO::new(version);

        // Build name→handle lookup maps from the document's tables.
        let mut layer_handles = HashMap::new();
        let mut default_layer_handle = 0u64;
        for layer in doc.layers.iter() {
            let h = layer.handle.value();
            let key = layer.name().to_uppercase();
            if key == "0" {
                default_layer_handle = h;
            }
            layer_handles.insert(key, h);
        }

        let mut linetype_handles = HashMap::new();
        for lt in doc.line_types.iter() {
            linetype_handles.insert(lt.name().to_uppercase(), lt.handle.value());
        }

        let mut textstyle_handles = HashMap::new();
        let mut default_textstyle_handle = 0u64;
        for ts in doc.text_styles.iter() {
            let h = ts.handle.value();
            let key = ts.name().to_uppercase();
            if key == "STANDARD" {
                default_textstyle_handle = h;
            }
            textstyle_handles.insert(key, h);
        }

        let mut block_handles = HashMap::new();
        for br in doc.block_records.iter() {
            block_handles.insert(br.name().to_uppercase(), br.handle.value());
        }

        let mut dimstyle_handles = HashMap::new();
        for ds in doc.dim_styles.iter() {
            dimstyle_handles.insert(ds.name().to_uppercase(), ds.handle.value());
        }

        let model_space_handle = doc.header.model_space_block_handle.value();
        let paper_space_handle = doc.header.paper_space_block_handle.value();

        DwgObjectWriter {
            sio,
            objects_stream: Vec::with_capacity(64 * 1024),
            handle_map: BTreeMap::new(),
            last_handle_size_bits: 0,
            model_space_handle,
            paper_space_handle,
            default_layer_handle,
            default_textstyle_handle,
            layer_handles,
            linetype_handles,
            textstyle_handles,
            block_handles,
            dimstyle_handles,
        }
    }

    // -----------------------------------------------------------------------
    // Public entry point
    // -----------------------------------------------------------------------

    /// Write all objects from the document and return:
    ///   - the objects section data (`Vec<u8>`)
    ///   - the handle → offset map (`BTreeMap<u64, i64>`)
    pub fn write(mut self, doc: &CadDocument) -> Result<(Vec<u8>, BTreeMap<u64, i64>)> {
        // 1. Table controls ------------------------------------------------
        self.write_table_controls(doc)?;

        // 2. Table entries --------------------------------------------------
        self.write_table_entries(doc)?;

        // 3. Block entities (BLOCK, owned entities, ENDBLK) -----------------
        self.write_block_contents(doc)?;

        // 4. Non-graphical objects ------------------------------------------
        self.write_nongraphical_objects(doc)?;

        Ok((self.objects_stream, self.handle_map))
    }

    // -----------------------------------------------------------------------
    // Create / finalize helpers (used by write_entities, write_tables, …)
    // -----------------------------------------------------------------------

    /// Create a fresh merged writer for an entity.
    pub(super) fn create_entity_writer(&self) -> (Box<dyn IDwgStreamWriter>, DxfVersion) {
        let version = self.sio.version();
        if self.sio.r2007_plus {
            (Box::new(DwgMergedStreamWriter::new(version)), version)
        } else {
            (Box::new(DwgMergedStreamWriterAC14::new(version)), version)
        }
    }

    /// Create a fresh merged writer for a non-entity object.
    pub(super) fn create_object_writer(&self) -> (Box<dyn IDwgStreamWriter>, DxfVersion) {
        // Same underlying implementation as entity writer.
        self.create_entity_writer()
    }

    /// Finalize an entity: extract the written data and register it.
    pub(super) fn finalize_entity(
        &mut self,
        mut writer: Box<dyn IDwgStreamWriter>,
        handle: u64,
    ) {
        let handle_bits = writer.saved_position_in_bits();
        self.last_handle_size_bits = handle_bits;
        let data = Self::extract_writer_data(&mut *writer);
        self.register_object(handle, data);
    }

    /// Finalize a non-entity object (same implementation).
    pub(super) fn finalize_object(
        &mut self,
        mut writer: Box<dyn IDwgStreamWriter>,
        handle: u64,
    ) {
        let handle_bits = writer.saved_position_in_bits();
        self.last_handle_size_bits = handle_bits;
        let data = Self::extract_writer_data(&mut *writer);
        self.register_object(handle, data);
    }

    /// Extract all bytes from the merged writer's main stream.
    ///
    /// After `write_spear_shift()` the handle (and text) sub-streams have
    /// been merged into the main stream, so we simply read it from
    /// position 0 to the end.
    fn extract_writer_data(writer: &mut dyn IDwgStreamWriter) -> Vec<u8> {
        let stream = writer.stream();
        let end = stream.seek(SeekFrom::End(0)).unwrap_or(0);
        if end == 0 {
            return Vec::new();
        }
        stream.seek(SeekFrom::Start(0)).unwrap();
        let mut data = vec![0u8; end as usize];
        let _ = stream.read_exact(&mut data);
        data
    }

    // -----------------------------------------------------------------------
    // Internal orchestration helpers
    // -----------------------------------------------------------------------

    /// Write the nine table control objects.
    fn write_table_controls(&mut self, doc: &CadDocument) -> Result<()> {
        let hdr = &doc.header;

        // BLOCK_CONTROL
        let block_entry_handles: Vec<u64> = doc
            .block_records
            .iter()
            .map(|b| b.handle.value())
            .collect();
        self.write_table_control(
            TableControlType::BlockControl,
            crate::io::dwg::object_type::DwgObjectType::BlockControlObj,
            hdr.block_control_handle.value(),
            0, // owned by root
            &block_entry_handles,
        )?;

        // LAYER_CONTROL
        let layer_entry_handles: Vec<u64> =
            doc.layers.iter().map(|l| l.handle.value()).collect();
        self.write_table_control(
            TableControlType::LayerControl,
            crate::io::dwg::object_type::DwgObjectType::LayerControlObj,
            hdr.layer_control_handle.value(),
            0,
            &layer_entry_handles,
        )?;

        // STYLE_CONTROL (text styles)
        let style_entry_handles: Vec<u64> =
            doc.text_styles.iter().map(|s| s.handle.value()).collect();
        self.write_table_control(
            TableControlType::TextStyleControl,
            crate::io::dwg::object_type::DwgObjectType::StyleControlObj,
            hdr.style_control_handle.value(),
            0,
            &style_entry_handles,
        )?;

        // LTYPE_CONTROL (special: extra ByLayer/ByBlock handles)
        let ltype_entry_handles: Vec<u64> = doc
            .line_types
            .iter()
            .map(|lt| lt.handle.value())
            .collect();
        self.write_ltype_control(
            hdr.linetype_control_handle.value(),
            0,
            &ltype_entry_handles,
            hdr.bylayer_linetype_handle.value(),
            hdr.byblock_linetype_handle.value(),
        )?;

        // VIEW_CONTROL
        let view_entry_handles: Vec<u64> =
            doc.views.iter().map(|v| v.handle.value()).collect();
        self.write_table_control(
            TableControlType::ViewControl,
            crate::io::dwg::object_type::DwgObjectType::ViewControlObj,
            hdr.view_control_handle.value(),
            0,
            &view_entry_handles,
        )?;

        // UCS_CONTROL
        let ucs_entry_handles: Vec<u64> =
            doc.ucss.iter().map(|u| u.handle.value()).collect();
        self.write_table_control(
            TableControlType::UcsControl,
            crate::io::dwg::object_type::DwgObjectType::UcsControlObj,
            hdr.ucs_control_handle.value(),
            0,
            &ucs_entry_handles,
        )?;

        // VPORT_CONTROL
        let vport_entry_handles: Vec<u64> =
            doc.vports.iter().map(|v| v.handle.value()).collect();
        self.write_table_control(
            TableControlType::VPortControl,
            crate::io::dwg::object_type::DwgObjectType::VportControlObj,
            hdr.vport_control_handle.value(),
            0,
            &vport_entry_handles,
        )?;

        // APPID_CONTROL
        let appid_entry_handles: Vec<u64> =
            doc.app_ids.iter().map(|a| a.handle.value()).collect();
        self.write_table_control(
            TableControlType::AppIdControl,
            crate::io::dwg::object_type::DwgObjectType::AppidControlObj,
            hdr.appid_control_handle.value(),
            0,
            &appid_entry_handles,
        )?;

        // DIMSTYLE_CONTROL
        let dimstyle_entry_handles: Vec<u64> = doc
            .dim_styles
            .iter()
            .map(|d| d.handle.value())
            .collect();
        self.write_table_control(
            TableControlType::DimStyleControl,
            crate::io::dwg::object_type::DwgObjectType::DimstyleControlObj,
            hdr.dimstyle_control_handle.value(),
            0,
            &dimstyle_entry_handles,
        )?;

        Ok(())
    }

    /// Write all table entries.
    fn write_table_entries(&mut self, doc: &CadDocument) -> Result<()> {
        let hdr = &doc.header;

        // Layers
        let layer_ctrl = hdr.layer_control_handle.value();
        let layers: Vec<_> = doc.layers.iter().cloned().collect();
        for layer in &layers {
            self.write_layer(layer, layer_ctrl)?;
        }

        // Text Styles
        let style_ctrl = hdr.style_control_handle.value();
        let styles: Vec<_> = doc.text_styles.iter().cloned().collect();
        for style in &styles {
            self.write_text_style(style, style_ctrl)?;
        }

        // Line Types
        let ltype_ctrl = hdr.linetype_control_handle.value();
        let ltypes: Vec<_> = doc.line_types.iter().cloned().collect();
        for lt in &ltypes {
            self.write_linetype(lt, ltype_ctrl)?;
        }

        // Application IDs
        let appid_ctrl = hdr.appid_control_handle.value();
        let appids: Vec<_> = doc.app_ids.iter().cloned().collect();
        for appid in &appids {
            self.write_appid(appid, appid_ctrl)?;
        }

        // Dimension Styles
        let dimstyle_ctrl = hdr.dimstyle_control_handle.value();
        let dimstyles: Vec<_> = doc.dim_styles.iter().cloned().collect();
        for ds in &dimstyles {
            self.write_dimstyle(ds, dimstyle_ctrl)?;
        }

        // VPorts
        let vport_ctrl = hdr.vport_control_handle.value();
        let vports: Vec<_> = doc.vports.iter().cloned().collect();
        for vp in &vports {
            self.write_vport(vp, vport_ctrl)?;
        }

        // Views
        let view_ctrl = hdr.view_control_handle.value();
        let views: Vec<_> = doc.views.iter().cloned().collect();
        for v in &views {
            self.write_view(v, view_ctrl)?;
        }

        Ok(())
    }

    /// Write block contents: for each block record, write BLOCK entity,
    /// owned entities, and ENDBLK entity.
    fn write_block_contents(&mut self, doc: &CadDocument) -> Result<()> {
        let block_ctrl = doc.header.block_control_handle.value();
        let blocks: Vec<_> = doc.block_records.iter().cloned().collect();

        for block in &blocks {
            let owner_handle = block.handle.value();

            // Collect entity handles for the block header
            let entity_handles: Vec<u64> = block
                .entities
                .iter()
                .map(|e| e.common().handle.value())
                .collect();

            // Write block header (BLOCK_RECORD)
            self.write_block_header(
                block,
                block_ctrl,
                &entity_handles,
                block.block_entity_handle.value(),
                block.block_end_handle.value(),
                block.layout.value(),
            )?;

            // Write owned entities
            for entity in &block.entities {
                self.write_entity(entity, owner_handle)?;
            }
        }

        Ok(())
    }

    /// Write non-graphical objects (dictionaries, etc.).
    fn write_nongraphical_objects(&mut self, doc: &CadDocument) -> Result<()> {
        let objects: Vec<_> = doc.objects.values().collect();
        for obj in objects {
            match obj {
                ObjectType::Dictionary(dict) => {
                    // Owner handle: look up from the dictionary's own info;
                    // root dictionary is owned by handle 0.
                    let owner_h = 0u64; // simplified — root dict owned by 0
                    self.write_dictionary(dict, owner_h)?;
                }
                // Other object types — skip for now
                _ => {}
            }
        }
        Ok(())
    }
}
