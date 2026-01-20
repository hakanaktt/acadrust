//! DXF section writers
//!
//! This module contains writers for each section of a DXF file:
//! HEADER, CLASSES, TABLES, BLOCKS, ENTITIES, and OBJECTS.

use crate::document::CadDocument;
use crate::entities::*;
use crate::error::Result;
use crate::objects::{
    Dictionary, DictionaryVariable, Group, ImageDefinition, Layout, MLineStyle, MultiLeaderStyle,
    ObjectType, PlotSettings, Scale, SortEntitiesTable, TableStyle, XRecord,
};
use crate::tables::*;
use crate::types::{Color, Handle};
use crate::xdata::{ExtendedData, XDataValue};

use super::stream_writer::{DxfStreamWriter, DxfStreamWriterExt};

/// Standard table handles (well-known values used by AutoCAD)
/// These are consistent across DXF files for interoperability
const HANDLE_VPORT_TABLE: u64 = 0x8;
const HANDLE_LTYPE_TABLE: u64 = 0x5;
const HANDLE_LAYER_TABLE: u64 = 0x2;
const HANDLE_STYLE_TABLE: u64 = 0x3;
const HANDLE_VIEW_TABLE: u64 = 0x6;
const HANDLE_UCS_TABLE: u64 = 0x7;
const HANDLE_APPID_TABLE: u64 = 0x9;
const HANDLE_DIMSTYLE_TABLE: u64 = 0xA;
const HANDLE_BLOCK_RECORD_TABLE: u64 = 0x1;

/// Writes all DXF sections
pub struct SectionWriter<'a, W: DxfStreamWriter> {
    writer: &'a mut W,
}

impl<'a, W: DxfStreamWriter> SectionWriter<'a, W> {
    /// Create a new section writer
    pub fn new(writer: &'a mut W) -> Self {
        Self { writer }
    }

    /// Write the HEADER section
    pub fn write_header(&mut self, document: &CadDocument) -> Result<()> {
        self.writer.write_section_start("HEADER")?;

        // Write essential header variables
        self.write_header_variable("$ACADVER", |w| {
            w.write_string(1, document.version.to_dxf_string())
        })?;

        // Maintenance version (required by some readers)
        self.write_header_variable("$ACADMAINTVER", |w| {
            w.write_i16(70, 0)
        })?;

        // Code page - ANSI_1252 for Western European
        self.write_header_variable("$DWGCODEPAGE", |w| {
            w.write_string(3, "ANSI_1252")
        })?;

        self.write_header_variable("$HANDSEED", |w| {
            w.write_handle(5, Handle::new(document.next_handle()))
        })?;

        // Drawing extents
        self.write_header_variable("$EXTMIN", |w| {
            w.write_double(10, 0.0)?;
            w.write_double(20, 0.0)?;
            w.write_double(30, 0.0)
        })?;

        self.write_header_variable("$EXTMAX", |w| {
            w.write_double(10, 0.0)?;
            w.write_double(20, 0.0)?;
            w.write_double(30, 0.0)
        })?;

        // Drawing limits
        self.write_header_variable("$LIMMIN", |w| {
            w.write_double(10, 0.0)?;
            w.write_double(20, 0.0)
        })?;

        self.write_header_variable("$LIMMAX", |w| {
            w.write_double(10, 12.0)?;
            w.write_double(20, 9.0)
        })?;

        // Insertion base point
        self.write_header_variable("$INSBASE", |w| {
            w.write_double(10, 0.0)?;
            w.write_double(20, 0.0)?;
            w.write_double(30, 0.0)
        })?;

        // Current layer
        self.write_header_variable("$CLAYER", |w| w.write_string(8, "0"))?;

        // Current color
        self.write_header_variable("$CECOLOR", |w| w.write_i16(62, 256))?;

        // Current linetype
        self.write_header_variable("$CELTYPE", |w| w.write_string(6, "ByLayer"))?;

        // Current lineweight
        self.write_header_variable("$CELWEIGHT", |w| w.write_i16(370, -1))?;

        // Measurement (0=English, 1=Metric)
        self.write_header_variable("$MEASUREMENT", |w| w.write_i16(70, 1))?;

        // Units
        self.write_header_variable("$INSUNITS", |w| w.write_i16(70, 0))?;

        self.writer.write_section_end()?;
        Ok(())
    }

    /// Write a header variable
    fn write_header_variable<F>(&mut self, name: &str, write_value: F) -> Result<()>
    where
        F: FnOnce(&mut W) -> Result<()>,
    {
        self.writer.write_string(9, name)?;
        write_value(self.writer)
    }

    /// Write the CLASSES section
    pub fn write_classes(&mut self, _document: &CadDocument) -> Result<()> {
        self.writer.write_section_start("CLASSES")?;

        self.writer.write_section_end()?;
        Ok(())
    }
    
    /// Write a single CLASS definition (for custom object types only)
    #[allow(dead_code)]
    fn write_class(
        &mut self,
        dxf_name: &str,
        cpp_name: &str,
        app_name: &str,
        proxy_flags: i32,
        instance_count: i32,
        was_proxy: bool,
        is_entity: bool,
    ) -> Result<()> {
        self.writer.write_string(0, "CLASS")?;
        self.writer.write_string(1, dxf_name)?;      // DXF class name
        self.writer.write_string(2, cpp_name)?;       // C++ class name
        self.writer.write_string(3, app_name)?;       // Application name
        self.writer.write_i32(90, proxy_flags)?;      // Proxy capabilities flag
        self.writer.write_i32(91, instance_count)?;   // Instance count (informational)
        self.writer.write_byte(280, if was_proxy { 1 } else { 0 })?;  // Was-a-proxy flag
        self.writer.write_byte(281, if is_entity { 1 } else { 0 })?;  // Is-an-entity flag
        Ok(())
    }

    /// Write the TABLES section
    pub fn write_tables(&mut self, document: &CadDocument) -> Result<()> {
        self.writer.write_section_start("TABLES")?;

        // Write tables in the standard order
        self.write_vport_table(document)?;
        self.write_ltype_table(document)?;
        self.write_layer_table(document)?;
        self.write_style_table(document)?;
        self.write_view_table(document)?;
        self.write_ucs_table(document)?;
        self.write_appid_table(document)?;
        self.write_dimstyle_table(document)?;
        self.write_block_record_table(document)?;

        self.writer.write_section_end()?;
        Ok(())
    }

    /// Write VPORT table
    fn write_vport_table(&mut self, document: &CadDocument) -> Result<()> {
        self.write_table_header("VPORT", document.vports.len(), Handle::new(HANDLE_VPORT_TABLE))?;

        for vport in document.vports.iter() {
            self.write_vport_entry(vport, Handle::new(HANDLE_VPORT_TABLE))?;
        }

        self.write_table_end()?;
        Ok(())
    }

    fn write_vport_entry(&mut self, vport: &VPort, owner: Handle) -> Result<()> {
        self.writer.write_string(0, "VPORT")?;
        self.write_common_table_data(vport.handle(), owner)?;
        self.writer.write_subclass("AcDbSymbolTableRecord")?;
        self.writer.write_subclass("AcDbViewportTableRecord")?;
        self.writer.write_string(2, vport.name())?;
        self.writer.write_i16(70, 0)?;

        // Viewport center
        self.writer.write_double(10, vport.view_center.x)?;
        self.writer.write_double(20, vport.view_center.y)?;

        // Viewport size
        self.writer.write_double(40, vport.view_height)?;
        self.writer.write_double(41, vport.aspect_ratio)?;

        // View target point
        self.writer.write_double(12, vport.view_target.x)?;
        self.writer.write_double(22, vport.view_target.y)?;
        self.writer.write_double(32, vport.view_target.z)?;

        // View direction
        self.writer.write_double(13, vport.view_direction.x)?;
        self.writer.write_double(23, vport.view_direction.y)?;
        self.writer.write_double(33, vport.view_direction.z)?;

        // View twist angle
        self.writer.write_double(51, 0.0)?;

        // Lens length
        self.writer.write_double(42, vport.lens_length)?;

        // Front/back clipping
        self.writer.write_double(43, 0.0)?;
        self.writer.write_double(44, 0.0)?;

        // View mode
        self.writer.write_i16(71, 0)?;

        // Circle zoom
        self.writer.write_i16(72, 1000)?;

        // Fast zoom
        self.writer.write_i16(73, 1)?;

        // UCSICON
        self.writer.write_i16(74, 3)?;

        // Snap on
        self.writer.write_i16(75, 0)?;

        // Grid on
        self.writer.write_i16(76, 0)?;

        // Snap style
        self.writer.write_i16(77, 0)?;

        // Snap isopair
        self.writer.write_i16(78, 0)?;

        Ok(())
    }

    /// Write LTYPE table
    fn write_ltype_table(&mut self, document: &CadDocument) -> Result<()> {
        self.write_table_header("LTYPE", document.line_types.len(), Handle::new(HANDLE_LTYPE_TABLE))?;

        for ltype in document.line_types.iter() {
            self.write_ltype_entry(ltype, Handle::new(HANDLE_LTYPE_TABLE))?;
        }

        self.write_table_end()?;
        Ok(())
    }

    fn write_ltype_entry(&mut self, ltype: &LineType, owner: Handle) -> Result<()> {
        self.writer.write_string(0, "LTYPE")?;
        self.write_common_table_data(ltype.handle(), owner)?;
        self.writer.write_subclass("AcDbSymbolTableRecord")?;
        self.writer.write_subclass("AcDbLinetypeTableRecord")?;
        self.writer.write_string(2, ltype.name())?;
        self.writer.write_i16(70, 0)?;
        self.writer.write_string(3, &ltype.description)?;
        self.writer.write_i16(72, 65)?; // Alignment code (always 65)
        self.writer.write_i16(73, ltype.elements.len() as i16)?;
        self.writer.write_double(40, ltype.pattern_length)?;

        for element in &ltype.elements {
            self.writer.write_double(49, element.length)?;
            self.writer.write_i16(74, 0)?;
        }

        Ok(())
    }

    /// Write LAYER table
    fn write_layer_table(&mut self, document: &CadDocument) -> Result<()> {
        self.write_table_header("LAYER", document.layers.len(), Handle::new(HANDLE_LAYER_TABLE))?;

        for layer in document.layers.iter() {
            self.write_layer_entry(layer, Handle::new(HANDLE_LAYER_TABLE))?;
        }

        self.write_table_end()?;
        Ok(())
    }

    fn write_layer_entry(&mut self, layer: &Layer, owner: Handle) -> Result<()> {
        self.writer.write_string(0, "LAYER")?;
        self.write_common_table_data(layer.handle(), owner)?;
        self.writer.write_subclass("AcDbSymbolTableRecord")?;
        self.writer.write_subclass("AcDbLayerTableRecord")?;
        self.writer.write_string(2, layer.name())?;

        // Flags
        let mut flags: i16 = 0;
        if layer.is_frozen() {
            flags |= 1;
        }
        if layer.is_locked() {
            flags |= 4;
        }
        self.writer.write_i16(70, flags)?;

        // Color (negative if layer is off)
        let color_index = match layer.color {
            Color::Index(i) => i as i16,
            Color::ByLayer => 7,
            Color::ByBlock => 0,
            Color::Rgb { .. } => 7,
        };
        if !layer.is_off() {
            self.writer.write_i16(62, color_index)?;
        } else {
            self.writer.write_i16(62, -color_index)?;
        }

        // Linetype name
        self.writer.write_string(6, &layer.line_type)?;

        // Lineweight
        self.writer.write_i16(370, layer.line_weight.value())?;

        // Plot flag (code 290 is Bool type - single byte in binary)
        self.writer
            .write_bool(290, layer.is_plottable)?;

        Ok(())
    }

    /// Write STYLE table (text styles)
    fn write_style_table(&mut self, document: &CadDocument) -> Result<()> {
        self.write_table_header("STYLE", document.text_styles.len(), Handle::new(HANDLE_STYLE_TABLE))?;

        for style in document.text_styles.iter() {
            self.write_style_entry(style, Handle::new(HANDLE_STYLE_TABLE))?;
        }

        self.write_table_end()?;
        Ok(())
    }

    fn write_style_entry(&mut self, style: &TextStyle, owner: Handle) -> Result<()> {
        self.writer.write_string(0, "STYLE")?;
        self.write_common_table_data(style.handle(), owner)?;
        self.writer.write_subclass("AcDbSymbolTableRecord")?;
        self.writer.write_subclass("AcDbTextStyleTableRecord")?;
        self.writer.write_string(2, style.name())?;
        self.writer.write_i16(70, 0)?;
        self.writer.write_double(40, style.height)?;
        self.writer.write_double(41, style.width_factor)?;
        self.writer.write_double(50, style.oblique_angle)?;
        self.writer.write_i16(71, 0)?; // Text generation flags
        self.writer.write_double(42, style.height)?; // Last height used
        self.writer.write_string(3, &style.font_file)?;
        self.writer.write_string(4, &style.big_font_file)?;

        Ok(())
    }

    /// Write VIEW table
    fn write_view_table(&mut self, document: &CadDocument) -> Result<()> {
        self.write_table_header("VIEW", document.views.len(), Handle::new(HANDLE_VIEW_TABLE))?;

        for view in document.views.iter() {
            self.write_view_entry(view, Handle::new(HANDLE_VIEW_TABLE))?;
        }

        self.write_table_end()?;
        Ok(())
    }

    fn write_view_entry(&mut self, view: &View, owner: Handle) -> Result<()> {
        self.writer.write_string(0, "VIEW")?;
        self.write_common_table_data(view.handle(), owner)?;
        self.writer.write_subclass("AcDbSymbolTableRecord")?;
        self.writer.write_subclass("AcDbViewTableRecord")?;
        self.writer.write_string(2, view.name())?;
        self.writer.write_i16(70, 0)?;
        self.writer.write_double(40, view.height)?;
        self.writer.write_double(10, view.center.x)?;
        self.writer.write_double(20, view.center.y)?;
        self.writer.write_double(41, view.width)?;
        self.writer.write_double(11, view.direction.x)?;
        self.writer.write_double(21, view.direction.y)?;
        self.writer.write_double(31, view.direction.z)?;
        self.writer.write_double(12, view.target.x)?;
        self.writer.write_double(22, view.target.y)?;
        self.writer.write_double(32, view.target.z)?;
        self.writer.write_double(42, view.lens_length)?;
        self.writer.write_double(43, view.front_clip)?;
        self.writer.write_double(44, view.back_clip)?;
        self.writer.write_double(50, view.twist_angle)?;

        Ok(())
    }

    /// Write UCS table
    fn write_ucs_table(&mut self, document: &CadDocument) -> Result<()> {
        self.write_table_header("UCS", document.ucss.len(), Handle::new(HANDLE_UCS_TABLE))?;

        for ucs in document.ucss.iter() {
            self.write_ucs_entry(ucs, Handle::new(HANDLE_UCS_TABLE))?;
        }

        self.write_table_end()?;
        Ok(())
    }

    fn write_ucs_entry(&mut self, ucs: &Ucs, owner: Handle) -> Result<()> {
        self.writer.write_string(0, "UCS")?;
        self.write_common_table_data(ucs.handle(), owner)?;
        self.writer.write_subclass("AcDbSymbolTableRecord")?;
        self.writer.write_subclass("AcDbUCSTableRecord")?;
        self.writer.write_string(2, ucs.name())?;
        self.writer.write_i16(70, 0)?;
        self.writer.write_double(10, ucs.origin.x)?;
        self.writer.write_double(20, ucs.origin.y)?;
        self.writer.write_double(30, ucs.origin.z)?;
        self.writer.write_double(11, ucs.x_axis.x)?;
        self.writer.write_double(21, ucs.x_axis.y)?;
        self.writer.write_double(31, ucs.x_axis.z)?;
        self.writer.write_double(12, ucs.y_axis.x)?;
        self.writer.write_double(22, ucs.y_axis.y)?;
        self.writer.write_double(32, ucs.y_axis.z)?;

        Ok(())
    }

    /// Write APPID table
    fn write_appid_table(&mut self, document: &CadDocument) -> Result<()> {
        self.write_table_header("APPID", document.app_ids.len(), Handle::new(HANDLE_APPID_TABLE))?;

        for appid in document.app_ids.iter() {
            self.write_appid_entry(appid, Handle::new(HANDLE_APPID_TABLE))?;
        }

        self.write_table_end()?;
        Ok(())
    }

    fn write_appid_entry(&mut self, appid: &AppId, owner: Handle) -> Result<()> {
        self.writer.write_string(0, "APPID")?;
        self.write_common_table_data(appid.handle(), owner)?;
        self.writer.write_subclass("AcDbSymbolTableRecord")?;
        self.writer.write_subclass("AcDbRegAppTableRecord")?;
        self.writer.write_string(2, appid.name())?;
        self.writer.write_i16(70, 0)?;

        Ok(())
    }

    /// Write DIMSTYLE table
    fn write_dimstyle_table(&mut self, document: &CadDocument) -> Result<()> {
        self.write_table_header("DIMSTYLE", document.dim_styles.len(), Handle::new(HANDLE_DIMSTYLE_TABLE))?;
        self.writer.write_subclass("AcDbDimStyleTable")?;

        for dimstyle in document.dim_styles.iter() {
            self.write_dimstyle_entry(dimstyle, Handle::new(HANDLE_DIMSTYLE_TABLE))?;
        }

        self.write_table_end()?;
        Ok(())
    }

    fn write_dimstyle_entry(&mut self, dimstyle: &DimStyle, owner: Handle) -> Result<()> {
        self.writer.write_string(0, "DIMSTYLE")?;
        self.writer.write_handle(105, dimstyle.handle())?;
        self.writer.write_handle(330, owner)?;
        self.writer.write_subclass("AcDbSymbolTableRecord")?;
        self.writer.write_subclass("AcDbDimStyleTableRecord")?;
        self.writer.write_string(2, dimstyle.name())?;
        self.writer.write_i16(70, 0)?;

        // Dimension style properties - using the actual short field names
        self.writer.write_double(40, dimstyle.dimscale)?; // Scale factor
        self.writer.write_double(41, dimstyle.dimasz)?; // Arrow size
        self.writer.write_double(42, dimstyle.dimexo)?; // Extension line offset
        self.writer.write_double(44, dimstyle.dimexe)?; // Extension line extension
        self.writer.write_double(140, dimstyle.dimtxt)?; // Text height

        self.writer.write_i16(176, dimstyle.dimclrd)?; // Dimension line color
        self.writer.write_i16(177, dimstyle.dimclre)?; // Extension line color
        self.writer.write_i16(178, dimstyle.dimclrt)?; // Dimension text color

        Ok(())
    }

    /// Write BLOCK_RECORD table
    fn write_block_record_table(&mut self, document: &CadDocument) -> Result<()> {
        self.write_table_header("BLOCK_RECORD", document.block_records.len(), Handle::new(HANDLE_BLOCK_RECORD_TABLE))?;

        for block_record in document.block_records.iter() {
            self.write_block_record_entry(block_record, Handle::new(HANDLE_BLOCK_RECORD_TABLE))?;
        }

        self.write_table_end()?;
        Ok(())
    }

    fn write_block_record_entry(&mut self, block_record: &BlockRecord, owner: Handle) -> Result<()> {
        self.writer.write_string(0, "BLOCK_RECORD")?;
        self.write_common_table_data(block_record.handle(), owner)?;
        self.writer.write_subclass("AcDbSymbolTableRecord")?;
        self.writer.write_subclass("AcDbBlockTableRecord")?;
        self.writer.write_string(2, block_record.name())?;
        self.writer.write_i16(70, block_record.units)?;
        self.writer
            .write_byte(280, if block_record.explodable { 1 } else { 0 })?;
        self.writer.write_i16(
            281,
            if block_record.scale_uniformly { 1 } else { 0 },
        )?;

        Ok(())
    }

    /// Write table header
    fn write_table_header(&mut self, name: &str, count: usize, table_handle: Handle) -> Result<()> {
        self.writer.write_string(0, "TABLE")?;
        self.writer.write_string(2, name)?;
        self.writer.write_handle(5, table_handle)?;
        self.writer.write_handle(330, Handle::new(0))?; // Tables owned by document root (handle 0)
        self.writer.write_subclass("AcDbSymbolTable")?;
        self.writer.write_i16(70, count as i16)?;
        Ok(())
    }

    /// Write table end
    fn write_table_end(&mut self) -> Result<()> {
        self.writer.write_string(0, "ENDTAB")
    }

    /// Write common table entry data
    fn write_common_table_data(&mut self, handle: Handle, owner: Handle) -> Result<()> {
        self.writer.write_handle(5, handle)?;
        self.writer.write_handle(330, owner)?;
        Ok(())
    }

    /// Write the BLOCKS section
    pub fn write_blocks(&mut self, document: &CadDocument) -> Result<()> {
        self.writer.write_section_start("BLOCKS")?;

        for block_record in document.block_records.iter() {
            self.write_block_definition(block_record)?;
        }

        self.writer.write_section_end()?;
        Ok(())
    }

    /// Write a complete block definition (BLOCK...entities...ENDBLK)
    fn write_block_definition(&mut self, block_record: &BlockRecord) -> Result<()> {
        let owner = block_record.handle();
        
        // Determine block flags
        let flags: i16 = if block_record.is_model_space() { 
            2 // Model space flag
        } else { 
            0 
        };
        
        // Write BLOCK entity
        self.writer.write_string(0, "BLOCK")?;
        self.writer.write_handle(5, block_record.block_entity_handle)?;
        self.writer.write_handle(330, owner)?;
        self.writer.write_subclass("AcDbEntity")?;
        // Paper space flag (group code 67) - 1 for paper space
        if block_record.is_paper_space() {
            self.writer.write_i16(67, 1)?;
        }
        self.writer.write_string(8, "0")?;
        self.writer.write_subclass("AcDbBlockBegin")?;
        self.writer.write_string(2, block_record.name())?;
        self.writer.write_i16(70, flags)?;
        self.writer.write_double(10, 0.0)?;
        self.writer.write_double(20, 0.0)?;
        self.writer.write_double(30, 0.0)?;
        self.writer.write_string(3, block_record.name())?;
        // Group code 1 is XRef path (empty for normal blocks)
        self.writer.write_string(1, "")?;

        // Write entities in the block (only for non-model/paper space blocks)
        if !block_record.is_model_space() && !block_record.is_paper_space() {
            for entity in &block_record.entities {
                self.write_entity_with_owner(entity, owner)?;
            }
        }

        // Write ENDBLK entity
        self.writer.write_string(0, "ENDBLK")?;
        self.writer.write_handle(5, block_record.block_end_handle)?;
        self.writer.write_handle(330, owner)?;
        self.writer.write_subclass("AcDbEntity")?;
        // Paper space flag for ENDBLK too
        if block_record.is_paper_space() {
            self.writer.write_i16(67, 1)?;
        }
        self.writer.write_string(8, "0")?;
        self.writer.write_subclass("AcDbBlockEnd")?;

        Ok(())
    }

    /// Write the ENTITIES section
    pub fn write_entities(&mut self, document: &CadDocument) -> Result<()> {
        self.writer.write_section_start("ENTITIES")?;

        // Write entities from model space block record
        if let Some(model_space) = document.block_records.get("*Model_Space") {
            let owner = model_space.handle();
            for entity in &model_space.entities {
                self.write_entity_with_owner(entity, owner)?;
            }
        }

        // Also write standalone entities (owned by model space by default)
        let model_space_handle = document.block_records.get("*Model_Space")
            .map(|b| b.handle())
            .unwrap_or(Handle::new(0x1F));
        for entity in document.entities() {
            self.write_entity_with_owner(entity, model_space_handle)?;
        }

        self.writer.write_section_end()?;
        Ok(())
    }

    /// Write an entity with explicit owner
    fn write_entity_with_owner(&mut self, entity: &EntityType, owner: Handle) -> Result<()> {
        match entity {
            EntityType::Point(e) => self.write_point(e, owner),
            EntityType::Line(e) => self.write_line(e, owner),
            EntityType::Circle(e) => self.write_circle(e, owner),
            EntityType::Arc(e) => self.write_arc(e, owner),
            EntityType::Ellipse(e) => self.write_ellipse(e, owner),
            EntityType::Polyline(e) => self.write_polyline(e, owner),
            EntityType::Polyline2D(e) => self.write_polyline2d(e, owner),
            EntityType::Polyline3D(e) => self.write_polyline3d(e, owner),
            EntityType::LwPolyline(e) => self.write_lwpolyline(e, owner),
            EntityType::Text(e) => self.write_text(e, owner),
            EntityType::MText(e) => self.write_mtext(e, owner),
            EntityType::Spline(e) => self.write_spline(e, owner),
            EntityType::Dimension(dim) => self.write_dimension(dim, owner),
            EntityType::Hatch(e) => self.write_hatch(e, owner),
            EntityType::Solid(e) => self.write_solid(e, owner),
            EntityType::Face3D(e) => self.write_face3d(e, owner),
            EntityType::Insert(e) => self.write_insert(e, owner),
            EntityType::Block(e) => self.write_block_entity(e, owner),
            EntityType::BlockEnd(e) => self.write_block_end(e, owner),
            EntityType::Ray(e) => self.write_ray(e, owner),
            EntityType::XLine(e) => self.write_xline(e, owner),
            EntityType::Viewport(e) => self.write_viewport(e, owner),
            EntityType::AttributeDefinition(e) => self.write_attdef(e, owner),
            EntityType::AttributeEntity(e) => self.write_attrib(e, owner),
            EntityType::Leader(e) => self.write_leader(e, owner),
            EntityType::MultiLeader(e) => self.write_multileader(e, owner),
            EntityType::MLine(e) => self.write_mline(e, owner),
            EntityType::Mesh(e) => self.write_mesh(e, owner),
            EntityType::RasterImage(e) => self.write_raster_image(e, owner),
            EntityType::Solid3D(e) => self.write_solid3d(e, owner),
            EntityType::Region(e) => self.write_region(e, owner),
            EntityType::Body(e) => self.write_body(e, owner),
            EntityType::Table(e) => self.write_acad_table(e, owner),
            EntityType::Tolerance(e) => self.write_tolerance(e, owner),
            EntityType::PolyfaceMesh(e) => self.write_polyface_mesh(e, owner),
            EntityType::Wipeout(e) => self.write_wipeout(e, owner),
            EntityType::Shape(e) => self.write_shape(e, owner),
            EntityType::Underlay(e) => self.write_underlay(e, owner),
        }
    }

    /// Write common entity data with owner
    fn write_common_entity_data(&mut self, entity: &dyn Entity, owner: Handle) -> Result<()> {
        self.writer.write_handle(5, entity.handle())?;
        self.writer.write_handle(330, owner)?;
        self.writer.write_subclass("AcDbEntity")?;
        self.writer.write_string(8, entity.layer())?;

        // Write color only if not ByLayer (default)
        let color = entity.color();
        if color != Color::ByLayer {
            self.writer.write_color(62, color)?;
        }

        // Write lineweight if not default
        let lw = entity.line_weight();
        if lw != crate::types::LineWeight::ByLayer {
            self.writer.write_i16(370, lw.value())?;
        }

        // Write visibility
        if entity.is_invisible() {
            self.writer.write_i16(60, 1)?;
        }

        Ok(())
    }

    /// Write POINT entity
    fn write_point(&mut self, point: &Point, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("POINT")?;
        self.write_common_entity_data(point, owner)?;
        self.writer.write_subclass("AcDbPoint")?;
        self.writer.write_point3d(10, point.location)?;
        if point.thickness != 0.0 {
            self.writer.write_double(39, point.thickness)?;
        }
        Ok(())
    }

    /// Write LINE entity
    fn write_line(&mut self, line: &Line, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("LINE")?;
        self.write_common_entity_data(line, owner)?;
        self.writer.write_subclass("AcDbLine")?;
        self.writer.write_point3d(10, line.start)?;
        self.writer.write_point3d(11, line.end)?;
        if line.thickness != 0.0 {
            self.writer.write_double(39, line.thickness)?;
        }
        Ok(())
    }

    /// Write CIRCLE entity
    fn write_circle(&mut self, circle: &Circle, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("CIRCLE")?;
        self.write_common_entity_data(circle, owner)?;
        self.writer.write_subclass("AcDbCircle")?;
        self.writer.write_point3d(10, circle.center)?;
        self.writer.write_double(40, circle.radius)?;
        if circle.thickness != 0.0 {
            self.writer.write_double(39, circle.thickness)?;
        }
        Ok(())
    }

    /// Write ARC entity
    fn write_arc(&mut self, arc: &Arc, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("ARC")?;
        self.write_common_entity_data(arc, owner)?;
        self.writer.write_subclass("AcDbCircle")?;
        self.writer.write_point3d(10, arc.center)?;
        self.writer.write_double(40, arc.radius)?;
        if arc.thickness != 0.0 {
            self.writer.write_double(39, arc.thickness)?;
        }
        self.writer.write_subclass("AcDbArc")?;
        self.writer.write_double(50, arc.start_angle.to_degrees())?;
        self.writer.write_double(51, arc.end_angle.to_degrees())?;
        Ok(())
    }

    /// Write ELLIPSE entity
    fn write_ellipse(&mut self, ellipse: &Ellipse, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("ELLIPSE")?;
        self.write_common_entity_data(ellipse, owner)?;
        self.writer.write_subclass("AcDbEllipse")?;
        self.writer.write_point3d(10, ellipse.center)?;
        self.writer.write_point3d(11, ellipse.major_axis)?;
        self.writer.write_double(40, ellipse.minor_axis_ratio)?;
        self.writer.write_double(41, ellipse.start_parameter)?;
        self.writer.write_double(42, ellipse.end_parameter)?;
        Ok(())
    }

    /// Write POLYLINE entity (3D polyline)
    fn write_polyline(&mut self, polyline: &Polyline, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("POLYLINE")?;
        self.write_common_entity_data(polyline, owner)?;
        self.writer.write_subclass("AcDb3dPolyline")?;

        let mut flags: i16 = 8; // 3D polyline flag
        if polyline.is_closed() {
            flags |= 1;
        }
        self.writer.write_i16(70, flags)?;

        // VERTEX and SEQEND are owned by the polyline entity
        let polyline_handle = polyline.common.handle;
        // Generate sub-entity handles based on polyline handle
        let base_handle = polyline_handle.value();

        // Write vertices
        for (i, vertex) in polyline.vertices.iter().enumerate() {
            let vertex_handle = Handle::new(base_handle + i as u64 + 1);
            self.writer.write_entity_type("VERTEX")?;
            self.writer.write_handle(5, vertex_handle)?;
            self.writer.write_handle(330, polyline_handle)?;
            self.writer.write_subclass("AcDbEntity")?;
            self.writer.write_string(8, &polyline.common.layer)?;
            self.writer.write_subclass("AcDbVertex")?;
            self.writer.write_subclass("AcDb3dPolylineVertex")?;
            self.writer.write_point3d(10, vertex.location)?;
            self.writer.write_i16(70, 32)?; // 3D polyline vertex
        }

        // Write SEQEND
        let seqend_handle = Handle::new(base_handle + polyline.vertices.len() as u64 + 1);
        self.writer.write_entity_type("SEQEND")?;
        self.writer.write_handle(5, seqend_handle)?;
        self.writer.write_handle(330, polyline_handle)?;
        self.writer.write_subclass("AcDbEntity")?;
        self.writer.write_subclass("AcDbSequenceEnd")?;
        self.writer.write_string(8, &polyline.common.layer)?;

        Ok(())
    }
    
    /// Write POLYLINE entity (2D polyline)
    fn write_polyline2d(&mut self, polyline: &Polyline2D, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("POLYLINE")?;
        self.write_common_entity_data(polyline, owner)?;
        self.writer.write_subclass("AcDb2dPolyline")?;

        // Entities follow flag (VERTEX records follow)
        self.writer.write_i16(66, 1)?;

        self.writer.write_i16(70, polyline.flags.bits() as i16)?;
        
        if polyline.elevation != 0.0 {
            self.writer.write_double(30, polyline.elevation)?;
        }
        if polyline.thickness != 0.0 {
            self.writer.write_double(39, polyline.thickness)?;
        }
        if polyline.start_width != 0.0 {
            self.writer.write_double(40, polyline.start_width)?;
        }
        if polyline.end_width != 0.0 {
            self.writer.write_double(41, polyline.end_width)?;
        }

        // VERTEX and SEQEND are owned by the polyline entity
        let polyline_handle = polyline.common.handle;
        let base_handle = polyline_handle.value();

        // Write vertices
        for (i, vertex) in polyline.vertices.iter().enumerate() {
            let vertex_handle = Handle::new(base_handle + i as u64 + 1);
            self.writer.write_entity_type("VERTEX")?;
            self.writer.write_handle(5, vertex_handle)?;
            self.writer.write_handle(330, polyline_handle)?;
            self.writer.write_subclass("AcDbEntity")?;
            self.writer.write_string(8, &polyline.common.layer)?;
            self.writer.write_subclass("AcDbVertex")?;
            self.writer.write_subclass("AcDb2dVertex")?;
            self.writer.write_point3d(10, vertex.location)?;
            if vertex.start_width != 0.0 {
                self.writer.write_double(40, vertex.start_width)?;
            }
            if vertex.end_width != 0.0 {
                self.writer.write_double(41, vertex.end_width)?;
            }
            if vertex.bulge != 0.0 {
                self.writer.write_double(42, vertex.bulge)?;
            }
            self.writer.write_i16(70, vertex.flags.bits() as i16)?;
        }

        // Write SEQEND
        let seqend_handle = Handle::new(base_handle + polyline.vertices.len() as u64 + 1);
        self.writer.write_entity_type("SEQEND")?;
        self.writer.write_handle(5, seqend_handle)?;
        self.writer.write_handle(330, polyline_handle)?;
        self.writer.write_subclass("AcDbEntity")?;
        self.writer.write_subclass("AcDbSequenceEnd")?;
        self.writer.write_string(8, &polyline.common.layer)?;

        Ok(())
    }

    /// Write LWPOLYLINE entity
    fn write_lwpolyline(&mut self, lwpoly: &LwPolyline, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("LWPOLYLINE")?;
        self.write_common_entity_data(lwpoly, owner)?;
        self.writer.write_subclass("AcDbPolyline")?;
        self.writer.write_i32(90, lwpoly.vertices.len() as i32)?;

        let mut flags: i16 = 0;
        if lwpoly.is_closed {
            flags |= 1;
        }
        self.writer.write_i16(70, flags)?;

        self.writer.write_double(38, lwpoly.elevation)?;
        if lwpoly.thickness != 0.0 {
            self.writer.write_double(39, lwpoly.thickness)?;
        }

        for vertex in &lwpoly.vertices {
            self.writer.write_double(10, vertex.location.x)?;
            self.writer.write_double(20, vertex.location.y)?;
            // Always write start width, end width, and bulge (default to 0.0 if not set)
            self.writer.write_double(40, vertex.start_width)?;
            self.writer.write_double(41, vertex.end_width)?;
            self.writer.write_double(42, vertex.bulge)?;
        }

        Ok(())
    }

    /// Write TEXT entity
    fn write_text(&mut self, text: &Text, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("TEXT")?;
        self.write_common_entity_data(text, owner)?;
        self.writer.write_subclass("AcDbText")?;
        self.writer.write_point3d(10, text.insertion_point)?;
        self.writer.write_double(40, text.height)?;
        self.writer.write_string(1, &text.value)?;
        if text.rotation != 0.0 {
            self.writer.write_double(50, text.rotation.to_degrees())?;
        }
        if text.width_factor != 1.0 {
            self.writer.write_double(41, text.width_factor)?;
        }
        if text.oblique_angle != 0.0 {
            self.writer.write_double(51, text.oblique_angle)?;
        }
        self.writer.write_string(7, &text.style)?;
        self.writer.write_i16(72, text.horizontal_alignment as i16)?;
        if let Some(align_pt) = text.alignment_point {
            self.writer.write_point3d(11, align_pt)?;
        }
        self.writer.write_subclass("AcDbText")?;
        self.writer.write_i16(73, text.vertical_alignment as i16)?;
        Ok(())
    }

    /// Write MTEXT entity
    fn write_mtext(&mut self, mtext: &MText, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("MTEXT")?;
        self.write_common_entity_data(mtext, owner)?;
        self.writer.write_subclass("AcDbMText")?;
        self.writer.write_point3d(10, mtext.insertion_point)?;
        self.writer.write_double(40, mtext.height)?;
        self.writer.write_double(41, mtext.rectangle_width)?;
        self.writer.write_i16(71, mtext.attachment_point as i16)?;
        self.writer.write_i16(72, mtext.drawing_direction as i16)?;

        // Write text value (may need to be split for long text)
        let text = &mtext.value;
        if text.len() > 250 {
            // Split into chunks
            let mut remaining = text.as_str();
            while remaining.len() > 250 {
                let (chunk, rest) = remaining.split_at(250);
                self.writer.write_string(3, chunk)?;
                remaining = rest;
            }
            self.writer.write_string(1, remaining)?;
        } else {
            self.writer.write_string(1, text)?;
        }

        self.writer.write_string(7, &mtext.style)?;
        if mtext.rotation != 0.0 {
            self.writer.write_double(50, mtext.rotation.to_degrees())?;
        }
        self.writer.write_double(44, mtext.line_spacing_factor)?;
        Ok(())
    }

    /// Write SPLINE entity
    fn write_spline(&mut self, spline: &Spline, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("SPLINE")?;
        self.write_common_entity_data(spline, owner)?;
        self.writer.write_subclass("AcDbSpline")?;

        // Normal vector
        self.writer.write_double(210, 0.0)?;
        self.writer.write_double(220, 0.0)?;
        self.writer.write_double(230, 1.0)?;

        // Flags
        let mut flags: i16 = 0;
        if spline.flags.closed {
            flags |= 1;
        }
        if spline.flags.periodic {
            flags |= 2;
        }
        if spline.flags.rational {
            flags |= 4;
        }
        self.writer.write_i16(70, flags)?;

        self.writer.write_i16(71, spline.degree as i16)?;
        self.writer.write_i16(72, spline.knots.len() as i16)?;
        self.writer
            .write_i16(73, spline.control_points.len() as i16)?;
        self.writer.write_i16(74, spline.fit_points.len() as i16)?;

        // Knot tolerance, control point tolerance, fit tolerance
        self.writer.write_double(42, 0.0000001)?;
        self.writer.write_double(43, 0.0000001)?;
        self.writer.write_double(44, 0.0000001)?;

        // Knots
        for knot in &spline.knots {
            self.writer.write_double(40, *knot)?;
        }

        // Control points
        for point in &spline.control_points {
            self.writer.write_point3d(10, *point)?;
        }

        // Fit points
        for point in &spline.fit_points {
            self.writer.write_point3d(11, *point)?;
        }

        Ok(())
    }

    /// Write DIMENSION entity
    fn write_dimension(&mut self, dimension: &Dimension, owner: Handle) -> Result<()> {
        match dimension {
            Dimension::Aligned(dim) => self.write_dimension_aligned(dim, owner),
            Dimension::Linear(dim) => self.write_dimension_linear(dim, owner),
            Dimension::Radius(dim) => self.write_dimension_radius(dim, owner),
            Dimension::Diameter(dim) => self.write_dimension_diameter(dim, owner),
            Dimension::Angular2Ln(dim) => self.write_dimension_angular_2line(dim, owner),
            Dimension::Angular3Pt(dim) => self.write_dimension_angular_3point(dim, owner),
            Dimension::Ordinate(dim) => self.write_dimension_ordinate(dim, owner),
        }
    }

    fn write_dimension_base(&mut self, base: &DimensionBase, type_flags: i16, owner: Handle) -> Result<()> {
        self.writer.write_handle(5, base.common.handle)?;
        self.writer.write_handle(330, owner)?;
        self.writer.write_subclass("AcDbEntity")?;
        self.writer.write_string(8, &base.common.layer)?;
        self.writer.write_subclass("AcDbDimension")?;
        self.writer.write_string(2, &base.block_name)?;
        self.writer.write_point3d(10, base.definition_point)?;
        self.writer.write_point3d(11, base.text_middle_point)?;
        self.writer.write_i16(70, type_flags)?;
        self.writer.write_double(53, base.text_rotation)?;
        self.writer.write_string(3, &base.style_name)?;
        if !base.text.is_empty() {
            self.writer.write_string(1, &base.text)?;
        }
        Ok(())
    }

    fn write_dimension_aligned(&mut self, dim: &DimensionAligned, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("DIMENSION")?;
        self.write_dimension_base(&dim.base, 1, owner)?; // Aligned = 1
        self.writer.write_subclass("AcDbAlignedDimension")?;
        self.writer.write_point3d(13, dim.first_point)?;
        self.writer.write_point3d(14, dim.second_point)?;
        Ok(())
    }

    fn write_dimension_linear(&mut self, dim: &DimensionLinear, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("DIMENSION")?;
        self.write_dimension_base(&dim.base, 0, owner)?; // Linear = 0
        self.writer.write_subclass("AcDbAlignedDimension")?;
        self.writer.write_point3d(13, dim.first_point)?;
        self.writer.write_point3d(14, dim.second_point)?;
        self.writer.write_double(50, dim.rotation)?;
        self.writer.write_subclass("AcDbRotatedDimension")?;
        Ok(())
    }

    fn write_dimension_radius(&mut self, dim: &DimensionRadius, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("DIMENSION")?;
        self.write_dimension_base(&dim.base, 4, owner)?; // Radius = 4
        self.writer.write_subclass("AcDbRadialDimension")?;
        self.writer.write_point3d(15, dim.angle_vertex)?;
        self.writer.write_double(40, dim.leader_length)?;
        Ok(())
    }

    fn write_dimension_diameter(&mut self, dim: &DimensionDiameter, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("DIMENSION")?;
        self.write_dimension_base(&dim.base, 3, owner)?; // Diameter = 3
        self.writer.write_subclass("AcDbDiametricDimension")?;
        self.writer.write_point3d(15, dim.angle_vertex)?;
        self.writer.write_double(40, dim.leader_length)?;
        Ok(())
    }

    fn write_dimension_angular_2line(&mut self, dim: &DimensionAngular2Ln, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("DIMENSION")?;
        self.write_dimension_base(&dim.base, 2, owner)?; // Angular = 2
        self.writer.write_subclass("AcDb2LineAngularDimension")?;
        self.writer.write_point3d(13, dim.first_point)?;
        self.writer.write_point3d(14, dim.second_point)?;
        self.writer.write_point3d(15, dim.angle_vertex)?;
        self.writer.write_point3d(16, dim.definition_point)?;
        Ok(())
    }

    fn write_dimension_angular_3point(&mut self, dim: &DimensionAngular3Pt, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("DIMENSION")?;
        self.write_dimension_base(&dim.base, 5, owner)?; // 3-point angular = 5
        self.writer.write_subclass("AcDb3PointAngularDimension")?;
        self.writer.write_point3d(13, dim.first_point)?;
        self.writer.write_point3d(14, dim.second_point)?;
        self.writer.write_point3d(15, dim.angle_vertex)?;
        Ok(())
    }

    fn write_dimension_ordinate(&mut self, dim: &DimensionOrdinate, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("DIMENSION")?;
        let type_flags = if dim.is_ordinate_type_x { 64 } else { 128 }; // Ordinate with X/Y flag
        self.write_dimension_base(&dim.base, 6 | type_flags, owner)?;
        self.writer.write_subclass("AcDbOrdinateDimension")?;
        self.writer.write_point3d(13, dim.feature_location)?;
        self.writer.write_point3d(14, dim.leader_endpoint)?;
        Ok(())
    }

    /// Write HATCH entity
    fn write_hatch(&mut self, hatch: &Hatch, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("HATCH")?;
        self.write_common_entity_data(hatch, owner)?;
        self.writer.write_subclass("AcDbHatch")?;

        // Elevation point
        self.writer.write_double(10, 0.0)?;
        self.writer.write_double(20, 0.0)?;
        self.writer.write_double(30, hatch.elevation)?;

        // Normal vector
        self.writer.write_double(210, hatch.normal.x)?;
        self.writer.write_double(220, hatch.normal.y)?;
        self.writer.write_double(230, hatch.normal.z)?;

        // Pattern name
        self.writer.write_string(2, &hatch.pattern.name)?;

        // Solid fill flag
        self.writer.write_i16(70, if hatch.is_solid { 1 } else { 0 })?;

        // Associative flag
        self.writer
            .write_i16(71, if hatch.is_associative { 1 } else { 0 })?;

        // Number of boundary paths
        self.writer.write_i32(91, hatch.paths.len() as i32)?;

        // Write boundary paths
        for path in &hatch.paths {
            self.write_hatch_boundary_path(path)?;
        }

        // Pattern style
        self.writer.write_i16(75, hatch.style as i16)?;
        self.writer.write_i16(76, hatch.pattern_type as i16)?;

        if !hatch.is_solid {
            self.writer
                .write_double(52, hatch.pattern_angle.to_degrees())?;
            self.writer.write_double(41, hatch.pattern_scale)?;
            self.writer.write_i16(77, if hatch.is_double { 1 } else { 0 })?;

            // Pattern definition lines
            self.writer
                .write_i16(78, hatch.pattern.lines.len() as i16)?;
            for line in &hatch.pattern.lines {
                self.writer.write_double(53, line.angle.to_degrees())?;
                self.writer.write_double(43, line.base_point.x)?;
                self.writer.write_double(44, line.base_point.y)?;
                self.writer.write_double(45, line.offset.x)?;
                self.writer.write_double(46, line.offset.y)?;
                self.writer.write_i16(79, line.dash_lengths.len() as i16)?;
                for dash in &line.dash_lengths {
                    self.writer.write_double(49, *dash)?;
                }
            }
        }

        // Seed points
        self.writer.write_i32(98, hatch.seed_points.len() as i32)?;
        for seed in &hatch.seed_points {
            self.writer.write_double(10, seed.x)?;
            self.writer.write_double(20, seed.y)?;
        }

        Ok(())
    }

    fn write_hatch_boundary_path(&mut self, path: &BoundaryPath) -> Result<()> {
        self.writer.write_i32(92, get_boundary_path_bits(&path.flags) as i32)?;

        if !path.flags.is_polyline() {
            self.writer.write_i32(93, path.edges.len() as i32)?;
        }

        for edge in &path.edges {
            self.write_hatch_edge(edge)?;
        }

        // Associated entities (boundary handles)
        self.writer.write_i32(97, 0)?;

        Ok(())
    }

    fn write_hatch_edge(&mut self, edge: &BoundaryEdge) -> Result<()> {
        match edge {
            BoundaryEdge::Line(line_edge) => {
                self.writer.write_i16(72, 1)?; // Line type
                self.writer.write_double(10, line_edge.start.x)?;
                self.writer.write_double(20, line_edge.start.y)?;
                self.writer.write_double(11, line_edge.end.x)?;
                self.writer.write_double(21, line_edge.end.y)?;
            }
            BoundaryEdge::CircularArc(arc) => {
                self.writer.write_i16(72, 2)?; // Arc type
                self.writer.write_double(10, arc.center.x)?;
                self.writer.write_double(20, arc.center.y)?;
                self.writer.write_double(40, arc.radius)?;
                self.writer
                    .write_double(50, arc.start_angle.to_degrees())?;
                self.writer.write_double(51, arc.end_angle.to_degrees())?;
                self.writer
                    .write_i16(73, if arc.counter_clockwise { 1 } else { 0 })?;
            }
            BoundaryEdge::EllipticArc(ellipse) => {
                self.writer.write_i16(72, 3)?; // Ellipse type
                self.writer.write_double(10, ellipse.center.x)?;
                self.writer.write_double(20, ellipse.center.y)?;
                self.writer.write_double(11, ellipse.major_axis_endpoint.x)?;
                self.writer.write_double(21, ellipse.major_axis_endpoint.y)?;
                self.writer.write_double(40, ellipse.minor_axis_ratio)?;
                self.writer.write_double(50, ellipse.start_angle)?;
                self.writer.write_double(51, ellipse.end_angle)?;
                self.writer
                    .write_i16(73, if ellipse.counter_clockwise { 1 } else { 0 })?;
            }
            BoundaryEdge::Spline(spline) => {
                self.writer.write_i16(72, 4)?; // Spline type
                self.writer
                    .write_i16(73, if spline.rational { 1 } else { 0 })?;
                self.writer
                    .write_i16(74, if spline.periodic { 1 } else { 0 })?;
                self.writer.write_i32(94, spline.degree)?;
                self.writer.write_i32(95, spline.knots.len() as i32)?;
                self.writer
                    .write_i32(96, spline.control_points.len() as i32)?;
                for knot in &spline.knots {
                    self.writer.write_double(40, *knot)?;
                }
                for point in &spline.control_points {
                    self.writer.write_double(10, point.x)?;
                    self.writer.write_double(20, point.y)?;
                }
            }
            BoundaryEdge::Polyline(poly) => {
                let has_bulge = poly.has_bulge();
                self.writer.write_i16(72, if has_bulge { 1 } else { 0 })?;
                self.writer
                    .write_i16(73, if poly.is_closed { 1 } else { 0 })?;
                self.writer.write_i32(93, poly.vertices.len() as i32)?;
                for vertex in &poly.vertices {
                    self.writer.write_double(10, vertex.x)?;
                    self.writer.write_double(20, vertex.y)?;
                    if has_bulge {
                        self.writer.write_double(42, vertex.z)?; // z stores bulge
                    }
                }
            }
        }
        Ok(())
    }

    /// Write SOLID entity
    fn write_solid(&mut self, solid: &Solid, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("SOLID")?;
        self.write_common_entity_data(solid, owner)?;
        self.writer.write_subclass("AcDbTrace")?;
        self.writer.write_point3d(10, solid.first_corner)?;
        self.writer.write_point3d(11, solid.second_corner)?;
        self.writer.write_point3d(12, solid.third_corner)?;
        self.writer.write_point3d(13, solid.fourth_corner)?;
        if solid.thickness != 0.0 {
            self.writer.write_double(39, solid.thickness)?;
        }
        Ok(())
    }

    /// Write 3DFACE entity
    fn write_face3d(&mut self, face: &Face3D, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("3DFACE")?;
        self.write_common_entity_data(face, owner)?;
        self.writer.write_subclass("AcDbFace")?;
        self.writer.write_point3d(10, face.first_corner)?;
        self.writer.write_point3d(11, face.second_corner)?;
        self.writer.write_point3d(12, face.third_corner)?;
        self.writer.write_point3d(13, face.fourth_corner)?;
        if face.invisible_edges != InvisibleEdgeFlags::NONE {
            let edge_bits = get_invisible_edge_bits(&face.invisible_edges);
            self.writer.write_i16(70, edge_bits as i16)?;
        }
        Ok(())
    }

    /// Write INSERT entity
    fn write_insert(&mut self, insert: &Insert, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("INSERT")?;
        self.write_common_entity_data(insert, owner)?;
        self.writer.write_subclass("AcDbBlockReference")?;
        self.writer.write_string(2, &insert.block_name)?;
        self.writer.write_point3d(10, insert.insert_point)?;
        if insert.x_scale != 1.0 {
            self.writer.write_double(41, insert.x_scale)?;
        }
        if insert.y_scale != 1.0 {
            self.writer.write_double(42, insert.y_scale)?;
        }
        if insert.z_scale != 1.0 {
            self.writer.write_double(43, insert.z_scale)?;
        }
        if insert.rotation != 0.0 {
            self.writer.write_double(50, insert.rotation.to_degrees())?;
        }
        if insert.column_count > 1 {
            self.writer.write_i16(70, insert.column_count as i16)?;
        }
        if insert.row_count > 1 {
            self.writer.write_i16(71, insert.row_count as i16)?;
        }
        if insert.column_spacing != 0.0 {
            self.writer.write_double(44, insert.column_spacing)?;
        }
        if insert.row_spacing != 0.0 {
            self.writer.write_double(45, insert.row_spacing)?;
        }
        Ok(())
    }

    /// Write BLOCK entity
    fn write_block_entity(&mut self, block: &Block, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("BLOCK")?;
        self.write_common_entity_data(block, owner)?;
        self.writer.write_subclass("AcDbBlockBegin")?;
        self.writer.write_string(2, &block.name)?;
        self.writer.write_i16(70, 0)?; // Block flags
        self.writer.write_point3d(10, block.base_point)?;
        self.writer.write_string(3, &block.name)?;
        if !block.xref_path.is_empty() {
            self.writer.write_string(1, &block.xref_path)?;
        }
        if !block.description.is_empty() {
            self.writer.write_string(4, &block.description)?;
        }
        Ok(())
    }

    /// Write ENDBLK entity
    fn write_block_end(&mut self, block_end: &BlockEnd, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("ENDBLK")?;
        self.write_common_entity_data(block_end, owner)?;
        self.writer.write_subclass("AcDbBlockEnd")?;
        Ok(())
    }

    /// Write RAY entity
    fn write_ray(&mut self, ray: &Ray, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("RAY")?;
        self.write_common_entity_data(ray, owner)?;
        self.writer.write_subclass("AcDbRay")?;
        self.writer.write_point3d(10, ray.base_point)?;
        self.writer.write_point3d(11, ray.direction)?;
        Ok(())
    }

    /// Write XLINE entity
    fn write_xline(&mut self, xline: &XLine, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("XLINE")?;
        self.write_common_entity_data(xline, owner)?;
        self.writer.write_subclass("AcDbXline")?;
        self.writer.write_point3d(10, xline.base_point)?;
        self.writer.write_point3d(11, xline.direction)?;
        Ok(())
    }

    /// Write POLYLINE (3D) entity
    fn write_polyline3d(&mut self, polyline: &Polyline3D, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("POLYLINE")?;
        self.write_common_entity_data(polyline, owner)?;
        self.writer.write_subclass("AcDb3dPolyline")?;

        // Entities follow flag (VERTEX records follow)
        self.writer.write_i16(66, 1)?;
        
        // Dummy point with elevation (ACadSharp pattern)
        self.writer.write_double(10, 0.0)?;
        self.writer.write_double(20, 0.0)?;
        self.writer.write_double(30, polyline.elevation)?;
        
        // Polyline flags (bit 8 = 3D polyline)
        self.writer.write_i16(70, polyline.flags.to_bits() as i16)?;
        
        // Write vertices
        let polyline_handle = polyline.handle();
        let base_handle = polyline_handle.value();
        for (index, vertex) in polyline.vertices.iter().enumerate() {
            let vertex_handle = if vertex.handle.is_null() {
                Handle::new(base_handle + index as u64 + 1)
            } else {
                vertex.handle
            };
            self.writer.write_entity_type("VERTEX")?;
            self.writer.write_handle(5, vertex_handle)?;
            self.writer.write_handle(330, polyline_handle)?;
            self.writer.write_subclass("AcDbEntity")?;
            self.writer.write_string(8, &vertex.layer)?;
            self.writer.write_subclass("AcDbVertex")?;
            self.writer.write_subclass("AcDb3dPolylineVertex")?;
            self.writer.write_point3d(10, vertex.position)?;
            self.writer.write_i16(70, vertex.flags as i16)?;
        }
        
        // SEQEND
        self.writer.write_entity_type("SEQEND")?;
        let seqend_handle = Handle::new(base_handle + polyline.vertices.len() as u64 + 1);
        self.writer.write_handle(5, seqend_handle)?;
        self.writer.write_handle(330, polyline_handle)?;
        self.writer.write_subclass("AcDbEntity")?;
        self.writer.write_subclass("AcDbSequenceEnd")?;
        self.writer.write_string(8, &polyline.common.layer)?;
        
        Ok(())
    }

    /// Write VIEWPORT entity
    fn write_viewport(&mut self, viewport: &Viewport, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("VIEWPORT")?;
        self.write_common_entity_data(viewport, owner)?;
        self.writer.write_subclass("AcDbViewport")?;
        
        // Center point
        self.writer.write_point3d(10, viewport.center)?;
        
        // Width and height
        self.writer.write_double(40, viewport.width)?;
        self.writer.write_double(41, viewport.height)?;
        
        // Viewport ID
        self.writer.write_i16(68, viewport.id)?;
        
        // Status
        self.writer.write_i32(90, viewport.status.to_bits())?;
        
        // View center
        self.writer.write_double(12, viewport.view_center.x)?;
        self.writer.write_double(22, viewport.view_center.y)?;
        
        // Snap base
        self.writer.write_double(13, viewport.snap_base.x)?;
        self.writer.write_double(23, viewport.snap_base.y)?;
        
        // Snap spacing
        self.writer.write_double(14, viewport.snap_spacing.x)?;
        self.writer.write_double(24, viewport.snap_spacing.y)?;
        
        // Grid spacing
        self.writer.write_double(15, viewport.grid_spacing.x)?;
        self.writer.write_double(25, viewport.grid_spacing.y)?;
        
        // View direction
        self.writer.write_double(16, viewport.view_direction.x)?;
        self.writer.write_double(26, viewport.view_direction.y)?;
        self.writer.write_double(36, viewport.view_direction.z)?;
        
        // View target
        self.writer.write_double(17, viewport.view_target.x)?;
        self.writer.write_double(27, viewport.view_target.y)?;
        self.writer.write_double(37, viewport.view_target.z)?;
        
        // Lens length
        self.writer.write_double(42, viewport.lens_length)?;
        
        // Front and back clipping
        self.writer.write_double(43, viewport.front_clip_z)?;
        self.writer.write_double(44, viewport.back_clip_z)?;
        
        // View height
        self.writer.write_double(45, viewport.view_height)?;
        
        // Snap and twist angles
        self.writer.write_double(50, viewport.snap_angle)?;
        self.writer.write_double(51, viewport.twist_angle)?;
        
        // Circle sides
        self.writer.write_i16(72, viewport.circle_sides)?;
        
        // Render mode
        self.writer.write_byte(281, viewport.render_mode.to_value() as u8)?;
        
        Ok(())
    }

    /// Write ATTDEF entity
    fn write_attdef(&mut self, attdef: &AttributeDefinition, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("ATTDEF")?;
        self.write_common_entity_data(attdef, owner)?;
        self.writer.write_subclass("AcDbText")?;
        
        // Insertion point
        self.writer.write_point3d(10, attdef.insertion_point)?;
        
        // Text height
        self.writer.write_double(40, attdef.height)?;
        
        // Default value
        self.writer.write_string(1, &attdef.default_value)?;
        
        // Rotation
        self.writer.write_double(50, attdef.rotation.to_degrees())?;
        
        // Width factor
        self.writer.write_double(41, attdef.width_factor)?;
        
        // Oblique angle
        self.writer.write_double(51, attdef.oblique_angle.to_degrees())?;
        
        // Text style
        self.writer.write_string(7, &attdef.text_style)?;
        
        // Text generation flags
        self.writer.write_i16(71, attdef.text_generation_flags)?;
        
        // Horizontal alignment
        self.writer.write_i16(72, attdef.horizontal_alignment.to_value())?;
        
        // Alignment point
        self.writer.write_point3d(21, attdef.alignment_point)?;
        
        // Normal
        self.writer.write_point3d(230, attdef.normal)?;
        
        self.writer.write_subclass("AcDbAttributeDefinition")?;
        
        // Tag
        self.writer.write_string(2, &attdef.tag)?;
        
        // Attribute flags
        self.writer.write_i16(70, attdef.flags.to_bits() as i16)?;
        
        // Field length
        self.writer.write_i16(73, attdef.field_length)?;
        
        // Vertical alignment
        self.writer.write_i16(74, attdef.vertical_alignment.to_value())?;
        
        // Prompt
        self.writer.write_string(3, &attdef.prompt)?;
        
        Ok(())
    }

    /// Write ATTRIB entity
    fn write_attrib(&mut self, attrib: &AttributeEntity, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("ATTRIB")?;
        self.write_common_entity_data(attrib, owner)?;
        self.writer.write_subclass("AcDbText")?;
        
        // Insertion point
        self.writer.write_point3d(10, attrib.insertion_point)?;
        
        // Text height
        self.writer.write_double(40, attrib.height)?;
        
        // Value
        self.writer.write_string(1, &attrib.value)?;
        
        // Rotation
        self.writer.write_double(50, attrib.rotation.to_degrees())?;
        
        // Width factor
        self.writer.write_double(41, attrib.width_factor)?;
        
        // Oblique angle
        self.writer.write_double(51, attrib.oblique_angle.to_degrees())?;
        
        // Text style
        self.writer.write_string(7, &attrib.text_style)?;
        
        // Text generation flags
        self.writer.write_i16(71, attrib.text_generation_flags)?;
        
        // Horizontal alignment
        self.writer.write_i16(72, attrib.horizontal_alignment.to_value())?;
        
        // Alignment point
        self.writer.write_point3d(21, attrib.alignment_point)?;
        
        // Normal
        self.writer.write_point3d(230, attrib.normal)?;
        
        self.writer.write_subclass("AcDbAttribute")?;
        
        // Tag
        self.writer.write_string(2, &attrib.tag)?;
        
        // Attribute flags
        self.writer.write_i16(70, attrib.flags.to_bits() as i16)?;
        
        // Field length
        self.writer.write_i16(73, attrib.field_length)?;
        
        // Vertical alignment
        self.writer.write_i16(74, attrib.vertical_alignment.to_value())?;
        
        Ok(())
    }

    /// Write LEADER entity
    fn write_leader(&mut self, leader: &Leader, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("LEADER")?;
        self.write_common_entity_data(leader, owner)?;
        self.writer.write_subclass("AcDbLeader")?;
        
        // Dimension style
        self.writer.write_string(3, &leader.dimension_style)?;
        
        // Arrow head flag
        self.writer.write_i16(71, if leader.arrow_enabled { 1 } else { 0 })?;
        
        // Path type
        self.writer.write_i16(72, leader.path_type.to_value())?;
        
        // Creation type
        self.writer.write_i16(73, leader.creation_type.to_value())?;
        
        // Hookline direction
        self.writer.write_i16(74, leader.hookline_direction.to_value())?;
        
        // Hookline flag
        self.writer.write_i16(75, if leader.hookline_enabled { 1 } else { 0 })?;
        
        // Text height
        self.writer.write_double(40, leader.text_height)?;
        
        // Text width
        self.writer.write_double(41, leader.text_width)?;
        
        // Number of vertices
        self.writer.write_i16(76, leader.vertices.len() as i16)?;
        
        // Vertices
        for vertex in &leader.vertices {
            self.writer.write_point3d(10, *vertex)?;
        }
        
        // Normal
        self.writer.write_point3d(230, leader.normal)?;
        
        // Horizontal direction
        self.writer.write_point3d(211, leader.horizontal_direction)?;
        
        // Block offset
        self.writer.write_point3d(212, leader.block_offset)?;
        
        // Annotation offset
        self.writer.write_point3d(213, leader.annotation_offset)?;
        
        Ok(())
    }

    /// Write the OBJECTS section
    pub fn write_objects(&mut self, document: &CadDocument) -> Result<()> {
        self.writer.write_section_start("OBJECTS")?;

        // Write root dictionary
        let mut root_dict = Dictionary::new();
        root_dict.handle = Handle::new(document.next_handle());
        self.write_dictionary(&root_dict)?;

        // Write other objects
        for object in document.objects.values() {
            match object {
                ObjectType::Dictionary(dict) => self.write_dictionary(dict)?,
                ObjectType::Layout(layout) => self.write_layout(layout)?,
                ObjectType::XRecord(xrecord) => self.write_xrecord(xrecord)?,
                ObjectType::Group(group) => self.write_group(group)?,
                ObjectType::MLineStyle(mlinestyle) => self.write_mlinestyle(mlinestyle)?,
                ObjectType::ImageDefinition(imagedef) => self.write_image_definition(imagedef)?,
                ObjectType::PlotSettings(plotsettings) => self.write_plot_settings(plotsettings)?,
                ObjectType::MultiLeaderStyle(style) => self.write_multileader_style(style)?,
                ObjectType::TableStyle(style) => self.write_table_style(style)?,
                ObjectType::Scale(scale) => self.write_scale(scale)?,
                ObjectType::SortEntitiesTable(table) => self.write_sort_entities_table(table)?,
                ObjectType::DictionaryVariable(var) => self.write_dictionary_variable(var)?,
                ObjectType::Unknown { .. } => {}
            }
        }

        self.writer.write_section_end()?;
        Ok(())
    }

    fn write_dictionary(&mut self, dict: &Dictionary) -> Result<()> {
        self.writer.write_string(0, "DICTIONARY")?;
        self.writer.write_handle(5, dict.handle)?;
        self.writer.write_handle(330, dict.owner)?;
        self.writer.write_subclass("AcDbDictionary")?;
        self.writer
            .write_byte(280, if dict.hard_owner { 1 } else { 0 })?;
        self.writer.write_byte(281, dict.duplicate_cloning as u8)?;

        for (key, handle) in &dict.entries {
            self.writer.write_string(3, key)?;
            self.writer.write_handle(350, *handle)?;
        }

        Ok(())
    }

    fn write_layout(&mut self, layout: &Layout) -> Result<()> {
        self.writer.write_string(0, "LAYOUT")?;
        self.writer.write_handle(5, layout.handle)?;
        self.writer.write_handle(330, layout.owner)?;
        self.writer.write_subclass("AcDbPlotSettings")?;

        // Minimal plot settings
        self.writer.write_string(1, "")?; // Page setup name
        self.writer.write_string(2, "")?; // Printer/plotter name
        self.writer.write_string(4, "")?; // Paper size
        self.writer.write_string(6, "")?; // Plot view name
        self.writer.write_double(40, 0.0)?; // Left margin
        self.writer.write_double(41, 0.0)?; // Bottom margin
        self.writer.write_double(42, 0.0)?; // Right margin
        self.writer.write_double(43, 0.0)?; // Top margin
        self.writer.write_double(44, 0.0)?; // Paper width
        self.writer.write_double(45, 0.0)?; // Paper height
        self.writer.write_double(46, 0.0)?; // Plot origin X
        self.writer.write_double(47, 0.0)?; // Plot origin Y
        self.writer.write_double(48, 0.0)?; // Plot window X1
        self.writer.write_double(49, 0.0)?; // Plot window Y1
        self.writer.write_double(140, 0.0)?; // Plot window X2
        self.writer.write_double(141, 0.0)?; // Plot window Y2
        self.writer.write_double(142, 1.0)?; // Numerator of custom print scale
        self.writer.write_double(143, 1.0)?; // Denominator of custom print scale
        self.writer.write_i16(70, layout.flags)?;
        self.writer.write_i16(72, 0)?; // Plot paper units
        self.writer.write_i16(73, 0)?; // Plot rotation
        self.writer.write_i16(74, 0)?; // Plot type

        self.writer.write_subclass("AcDbLayout")?;
        self.writer.write_string(1, &layout.name)?;
        self.writer.write_i16(70, layout.flags)?;
        self.writer.write_i16(71, layout.tab_order)?;
        self.writer.write_double(10, layout.min_limits.0)?;
        self.writer.write_double(20, layout.min_limits.1)?;
        self.writer.write_double(11, layout.max_limits.0)?;
        self.writer.write_double(21, layout.max_limits.1)?;
        self.writer.write_double(12, layout.insertion_base.0)?;
        self.writer.write_double(22, layout.insertion_base.1)?;
        self.writer.write_double(32, layout.insertion_base.2)?;
        self.writer.write_double(14, layout.min_extents.0)?;
        self.writer.write_double(24, layout.min_extents.1)?;
        self.writer.write_double(34, layout.min_extents.2)?;
        self.writer.write_double(15, layout.max_extents.0)?;
        self.writer.write_double(25, layout.max_extents.1)?;
        self.writer.write_double(35, layout.max_extents.2)?;
        self.writer.write_handle(330, layout.block_record)?;
        self.writer.write_handle(331, layout.viewport)?;

        Ok(())
    }

    fn write_xrecord(&mut self, xrecord: &XRecord) -> Result<()> {
        use crate::objects::XRecordValue;

        self.writer.write_string(0, "XRECORD")?;
        self.writer.write_handle(5, xrecord.handle)?;
        self.writer.write_handle(330, xrecord.owner)?;
        self.writer.write_subclass("AcDbXrecord")?;
        self.writer.write_byte(280, xrecord.cloning_flags.to_code() as u8)?;

        // Write each entry's group code and value
        for entry in xrecord.iter() {
            match &entry.value {
                XRecordValue::String(s) => {
                    self.writer.write_string(entry.code, s)?;
                }
                XRecordValue::Double(d) => {
                    self.writer.write_double(entry.code, *d)?;
                }
                XRecordValue::Int16(i) => {
                    self.writer.write_i16(entry.code, *i)?;
                }
                XRecordValue::Int32(i) => {
                    self.writer.write_i32(entry.code, *i)?;
                }
                XRecordValue::Int64(i) => {
                    // Write as i32, truncating if needed (DXF doesn't have native i64 codes for all ranges)
                    self.writer.write_i32(entry.code, *i as i32)?;
                }
                XRecordValue::Byte(b) => {
                    self.writer.write_i16(entry.code, *b as i16)?;
                }
                XRecordValue::Bool(b) => {
                    self.writer.write_i16(entry.code, if *b { 1 } else { 0 })?;
                }
                XRecordValue::Handle(h) => {
                    self.writer.write_handle(entry.code, *h)?;
                }
                XRecordValue::Point3D(x, y, z) => {
                    self.writer.write_double(entry.code, *x)?;
                    self.writer.write_double(entry.code + 10, *y)?;
                    self.writer.write_double(entry.code + 20, *z)?;
                }
                XRecordValue::Chunk(data) => {
                    self.writer.write_binary(entry.code, data)?;
                }
            }
        }

        Ok(())
    }

    fn write_group(&mut self, group: &Group) -> Result<()> {
        self.writer.write_string(0, "GROUP")?;
        self.writer.write_handle(5, group.handle)?;
        self.writer.write_handle(330, group.owner)?;
        self.writer.write_subclass("AcDbGroup")?;

        // Group description (code 300)
        self.writer.write_string(300, &group.description)?;

        // Unnamed flag (code 70) - 1 if unnamed, 0 if named
        self.writer
            .write_i16(70, if group.is_unnamed() { 1 } else { 0 })?;

        // Selectable flag (code 71)
        self.writer
            .write_i16(71, if group.selectable { 1 } else { 0 })?;

        // Entity handles (code 340)
        for entity_handle in group.iter() {
            self.writer.write_handle(340, *entity_handle)?;
        }

        Ok(())
    }

    fn write_mlinestyle(&mut self, style: &MLineStyle) -> Result<()> {
        self.writer.write_string(0, "MLINESTYLE")?;
        self.writer.write_handle(5, style.handle)?;
        self.writer.write_handle(330, style.owner)?;
        self.writer.write_subclass("AcDbMlineStyle")?;

        // Style name (code 2)
        self.writer.write_string(2, &style.name)?;

        // Flags (code 70)
        self.writer.write_i16(70, style.flags.to_bits() as i16)?;

        // Description (code 3)
        self.writer.write_string(3, &style.description)?;

        // Fill color (code 62)
        let fill_color_index = match style.fill_color {
            Color::ByLayer => 256,
            Color::ByBlock => 0,
            Color::Index(i) => i as i16,
            Color::Rgb { .. } => 256, // Fall back to ByLayer for RGB color
        };
        self.writer.write_i16(62, fill_color_index)?;

        // Start angle (code 51)
        self.writer.write_double(51, style.start_angle)?;

        // End angle (code 52)
        self.writer.write_double(52, style.end_angle)?;

        // Number of elements (code 71)
        self.writer.write_i16(71, style.element_count() as i16)?;

        // Write each element
        for element in style.iter() {
            // Element offset (code 49)
            self.writer.write_double(49, element.offset)?;

            // Element color (code 62)
            let elem_color_index = match element.color {
                Color::ByLayer => 256,
                Color::ByBlock => 0,
                Color::Index(i) => i as i16,
                Color::Rgb { .. } => 256,
            };
            self.writer.write_i16(62, elem_color_index)?;

            // Element linetype (code 6)
            self.writer.write_string(6, &element.linetype)?;
        }

        Ok(())
    }

    fn write_image_definition(&mut self, imagedef: &ImageDefinition) -> Result<()> {
        self.writer.write_string(0, "IMAGEDEF")?;
        self.writer.write_handle(5, imagedef.handle)?;
        self.writer.write_handle(330, imagedef.owner)?;
        self.writer.write_subclass("AcDbRasterImageDef")?;

        // Class version (code 90)
        self.writer.write_i32(90, imagedef.class_version)?;

        // File name (code 1)
        self.writer.write_string(1, &imagedef.file_name)?;

        // Image size in pixels (codes 10, 20)
        self.writer
            .write_double(10, imagedef.size_in_pixels.0 as f64)?;
        self.writer
            .write_double(20, imagedef.size_in_pixels.1 as f64)?;

        // Default pixel size (codes 11, 21)
        self.writer.write_double(11, imagedef.pixel_size.0)?;
        self.writer.write_double(21, imagedef.pixel_size.1)?;

        // Is loaded (code 280)
        self.writer
            .write_byte(280, if imagedef.is_loaded { 1 } else { 0 })?;

        // Resolution units (code 281)
        self.writer
            .write_byte(281, imagedef.resolution_unit.to_code() as u8)?;

        Ok(())
    }

    fn write_plot_settings(&mut self, settings: &PlotSettings) -> Result<()> {
        self.writer.write_string(0, "PLOTSETTINGS")?;
        self.writer.write_handle(5, settings.handle)?;
        self.writer.write_handle(330, settings.owner)?;
        self.writer.write_subclass("AcDbPlotSettings")?;

        // Page setup name (code 1)
        self.writer.write_string(1, &settings.page_name)?;

        // Printer/plotter name (code 2)
        self.writer.write_string(2, &settings.printer_name)?;

        // Paper size (code 4)
        self.writer.write_string(4, &settings.paper_size)?;

        // Plot view name (code 6)
        self.writer.write_string(6, &settings.plot_view_name)?;

        // Style sheet (code 7)
        self.writer.write_string(7, &settings.current_style_sheet)?;

        // Unprintable margins (codes 40-43)
        self.writer.write_double(40, settings.margins.left)?;
        self.writer.write_double(41, settings.margins.bottom)?;
        self.writer.write_double(42, settings.margins.right)?;
        self.writer.write_double(43, settings.margins.top)?;

        // Paper size (codes 44, 45)
        self.writer.write_double(44, settings.paper_width)?;
        self.writer.write_double(45, settings.paper_height)?;

        // Plot origin (codes 46, 47)
        self.writer.write_double(46, settings.origin_x)?;
        self.writer.write_double(47, settings.origin_y)?;

        // Plot window (codes 48, 49, 140, 141)
        self.writer
            .write_double(48, settings.plot_window.lower_left_x)?;
        self.writer
            .write_double(49, settings.plot_window.lower_left_y)?;
        self.writer
            .write_double(140, settings.plot_window.upper_right_x)?;
        self.writer
            .write_double(141, settings.plot_window.upper_right_y)?;

        // Custom scale (codes 142, 143)
        self.writer.write_double(142, settings.scale_numerator)?;
        self.writer.write_double(143, settings.scale_denominator)?;

        // Flags (code 70)
        self.writer.write_i16(70, settings.flags.to_bits() as i16)?;

        // Paper units (code 72)
        self.writer.write_i16(72, settings.paper_units.to_code())?;

        // Rotation (code 73)
        self.writer.write_i16(73, settings.rotation.to_code())?;

        // Plot type (code 74)
        self.writer.write_i16(74, settings.plot_type.to_code())?;

        // Standard scale type (code 75)
        self.writer.write_i16(75, settings.scale_type.to_code())?;

        // Shade plot mode (code 76)
        self.writer
            .write_i16(76, settings.shade_plot_mode.to_code())?;

        // Shade plot resolution level (code 77)
        self.writer
            .write_i16(77, settings.shade_plot_resolution.to_code())?;

        // Shade plot custom DPI (code 78)
        self.writer.write_i16(78, settings.shade_plot_dpi)?;

        Ok(())
    }

    /// Write MultiLeaderStyle object
    fn write_multileader_style(&mut self, style: &MultiLeaderStyle) -> Result<()> {
        self.writer.write_string(0, "MLEADERSTYLE")?;
        self.writer.write_handle(5, style.handle)?;
        self.writer.write_handle(330, style.owner_handle)?;
        self.writer.write_subclass("AcDbMLeaderStyle")?;

        // Content type
        self.writer.write_i16(170, style.content_type as i16)?;

        // Draw mleader order type
        self.writer.write_i16(171, style.multileader_draw_order as i16)?;

        // Draw leader order type
        self.writer.write_i16(172, style.leader_draw_order as i16)?;

        // Max leader points
        self.writer.write_i32(90, style.max_leader_points)?;

        // First segment angle constraint
        self.writer.write_double(40, style.first_segment_angle)?;

        // Second segment angle constraint
        self.writer.write_double(41, style.second_segment_angle)?;

        // Leader line type
        self.writer.write_i16(173, style.path_type as i16)?;

        // Leader line color
        self.write_color_i32(91, style.line_color)?;

        // Leader line type handle
        if let Some(h) = style.line_type_handle {
            self.writer.write_handle(340, h)?;
        }

        // Leader line weight
        self.writer.write_i16(92, style.line_weight.value())?;

        // Enable landing
        self.writer.write_bool(290, style.enable_landing)?;

        // Landing gap
        self.writer.write_double(42, style.landing_gap)?;

        // Enable dogleg
        self.writer.write_bool(291, style.enable_dogleg)?;

        // Dogleg length
        self.writer.write_double(43, style.landing_distance)?;

        // Style name
        self.writer.write_string(3, &style.name)?;

        // Arrow head block handle
        if let Some(h) = style.arrowhead_handle {
            self.writer.write_handle(341, h)?;
        }

        // Arrow head size
        self.writer.write_double(44, style.arrowhead_size)?;

        // Default mtext contents
        self.writer.write_string(300, &style.default_text)?;

        // Text style handle
        if let Some(h) = style.text_style_handle {
            self.writer.write_handle(342, h)?;
        }

        // Text left attachment type
        self.writer.write_i16(174, style.text_left_attachment as i16)?;

        // Text angle type
        self.writer.write_i16(175, style.text_angle_type as i16)?;

        // Text alignment type
        self.writer.write_i16(176, style.text_alignment as i16)?;

        // Text right attachment type
        self.writer.write_i16(178, style.text_right_attachment as i16)?;

        // Text color
        self.write_color_i32(93, style.text_color)?;

        // Text height
        self.writer.write_double(45, style.text_height)?;

        // Enable frame text
        self.writer.write_bool(292, style.text_frame)?;

        // Text always left justify
        self.writer.write_bool(297, style.text_always_left)?;

        // Align space
        self.writer.write_double(46, style.align_space)?;

        // Block content handle
        if let Some(h) = style.block_content_handle {
            self.writer.write_handle(343, h)?;
        }

        // Block content color
        self.write_color_i32(94, style.block_content_color)?;

        // Block content scale (x, y, z)
        self.writer.write_double(47, style.block_content_scale_x)?;
        self.writer.write_double(49, style.block_content_scale_y)?;
        self.writer.write_double(140, style.block_content_scale_z)?;

        // Enable block content scale
        self.writer.write_bool(293, style.enable_block_scale)?;

        // Block content rotation
        self.writer.write_double(141, style.block_content_rotation)?;

        // Enable block content rotation
        self.writer.write_bool(294, style.enable_block_rotation)?;

        // Block content connection type
        self.writer.write_i16(177, style.block_content_connection as i16)?;

        // Scale factor
        self.writer.write_double(142, style.scale_factor)?;

        // Property changed flag
        self.writer.write_bool(295, style.property_changed)?;

        // Is annotative
        self.writer.write_bool(296, style.is_annotative)?;

        // Break gap size
        self.writer.write_double(143, style.break_gap_size)?;

        Ok(())
    }

    /// Write TableStyle object
    fn write_table_style(&mut self, style: &TableStyle) -> Result<()> {
        self.writer.write_string(0, "TABLESTYLE")?;
        self.writer.write_handle(5, style.handle)?;
        self.writer.write_handle(330, style.owner_handle)?;
        self.writer.write_subclass("AcDbTableStyle")?;

        // Version
        self.writer.write_byte(280, style.version as u8)?;

        // Description
        if !style.description.is_empty() {
            self.writer.write_string(3, &style.description)?;
        }

        // Flow direction
        self.writer.write_i16(70, style.flow_direction as i16)?;

        // Flags
        self.writer.write_i16(71, style.flags.bits())?;

        // Horizontal margin
        self.writer.write_double(40, style.horizontal_margin)?;

        // Vertical margin
        self.writer.write_double(41, style.vertical_margin)?;

        // Title suppressed
        self.writer.write_bool(280, style.title_suppressed)?;

        // Header suppressed
        self.writer.write_bool(281, style.header_suppressed)?;

        // Write cell style info for data row
        self.write_table_cell_style("DATA", &style.data_row_style)?;

        // Write cell style info for header row
        self.write_table_cell_style("HEADER", &style.header_row_style)?;

        // Write cell style info for title row
        self.write_table_cell_style("TITLE", &style.title_row_style)?;

        Ok(())
    }

    /// Helper to write table cell style
    fn write_table_cell_style(&mut self, name: &str, style: &crate::objects::RowCellStyle) -> Result<()> {
        // Cell type indicator - simplified for basic support
        self.writer.write_string(7, &style.text_style_name)?;
        self.writer.write_double(140, style.text_height)?;
        self.writer.write_i16(170, style.alignment as i16)?;
        
        // Text color
        self.write_color_i16(62, style.text_color)?;

        // Fill color
        self.write_color_i16(63, style.fill_color)?;

        // Fill enabled
        self.writer.write_bool(283, style.fill_enabled)?;

        let _ = name; // Name is for future use in extended format
        
        Ok(())
    }

    /// Write Scale object
    fn write_scale(&mut self, scale: &Scale) -> Result<()> {
        self.writer.write_string(0, "SCALE")?;
        self.writer.write_handle(5, scale.handle)?;
        self.writer.write_handle(330, scale.owner_handle)?;
        self.writer.write_subclass("AcDbScale")?;

        // Scale name
        self.writer.write_string(300, &scale.name)?;

        // Paper units
        self.writer.write_double(140, scale.paper_units)?;

        // Drawing units
        self.writer.write_double(141, scale.drawing_units)?;

        // Is unit scale
        self.writer.write_bool(290, scale.is_unit_scale)?;

        Ok(())
    }

    /// Write SortEntitiesTable object
    fn write_sort_entities_table(&mut self, table: &SortEntitiesTable) -> Result<()> {
        self.writer.write_string(0, "SORTENTSTABLE")?;
        self.writer.write_handle(5, table.handle)?;
        self.writer.write_handle(330, table.owner_handle)?;
        self.writer.write_subclass("AcDbSortentsTable")?;

        // Block owner handle
        self.writer.write_handle(330, table.block_owner_handle)?;

        // Write all entries
        for entry in table.entries() {
            self.writer.write_handle(331, entry.entity_handle)?;
            self.writer.write_handle(5, entry.sort_handle)?;
        }

        Ok(())
    }

    /// Write DictionaryVariable object
    fn write_dictionary_variable(&mut self, var: &DictionaryVariable) -> Result<()> {
        self.writer.write_string(0, "DICTIONARYVAR")?;
        self.writer.write_handle(5, var.handle)?;
        self.writer.write_handle(330, var.owner_handle)?;
        self.writer.write_subclass("DictionaryVariables")?;

        // Schema number
        self.writer.write_byte(280, var.schema_number as u8)?;

        // Value
        self.writer.write_string(1, &var.value)?;

        Ok(())
    }

    /// Helper to write color as i32 (true color format)
    fn write_color_i32(&mut self, code: i32, color: Color) -> Result<()> {
        match color {
            Color::ByLayer => self.writer.write_i32(code, 256)?,
            Color::ByBlock => self.writer.write_i32(code, 0)?,
            Color::Index(i) => self.writer.write_i32(code, i as i32)?,
            Color::Rgb { r, g, b } => {
                let rgb = ((r as i32) << 16) | ((g as i32) << 8) | (b as i32);
                self.writer.write_i32(code, rgb)?;
            }
        }
        Ok(())
    }

    /// Helper to write color as i16 (index format)
    fn write_color_i16(&mut self, code: i32, color: Color) -> Result<()> {
        match color {
            Color::ByLayer => self.writer.write_i16(code, 256)?,
            Color::ByBlock => self.writer.write_i16(code, 0)?,
            Color::Index(i) => self.writer.write_i16(code, i as i16)?,
            Color::Rgb { .. } => self.writer.write_i16(code, 7)?, // Default to white/black
        }
        Ok(())
    }

    /// Write extended data (XDATA)
    #[allow(dead_code)]
    fn write_xdata(&mut self, xdata: &ExtendedData) -> Result<()> {
        if xdata.is_empty() {
            return Ok(());
        }

        for record in xdata.records() {
            self.writer.write_string(1001, &record.application_name)?;

            for value in &record.values {
                match value {
                    XDataValue::String(s) => {
                        self.writer.write_string(1000, s)?;
                    }
                    XDataValue::ControlString(s) => {
                        self.writer.write_string(1002, s)?;
                    }
                    XDataValue::LayerName(s) => {
                        self.writer.write_string(1003, s)?;
                    }
                    XDataValue::BinaryData(data) => {
                        self.writer.write_binary(1004, data)?;
                    }
                    XDataValue::Handle(h) => {
                        self.writer.write_handle(1005, *h)?;
                    }
                    XDataValue::Point3D(p) => {
                        self.writer.write_double(1010, p.x)?;
                        self.writer.write_double(1020, p.y)?;
                        self.writer.write_double(1030, p.z)?;
                    }
                    XDataValue::Position3D(p) => {
                        self.writer.write_double(1011, p.x)?;
                        self.writer.write_double(1021, p.y)?;
                        self.writer.write_double(1031, p.z)?;
                    }
                    XDataValue::Displacement3D(p) => {
                        self.writer.write_double(1012, p.x)?;
                        self.writer.write_double(1022, p.y)?;
                        self.writer.write_double(1032, p.z)?;
                    }
                    XDataValue::Direction3D(p) => {
                        self.writer.write_double(1013, p.x)?;
                        self.writer.write_double(1023, p.y)?;
                        self.writer.write_double(1033, p.z)?;
                    }
                    XDataValue::Real(r) => {
                        self.writer.write_double(1040, *r)?;
                    }
                    XDataValue::Distance(d) => {
                        self.writer.write_double(1041, *d)?;
                    }
                    XDataValue::ScaleFactor(s) => {
                        self.writer.write_double(1042, *s)?;
                    }
                    XDataValue::Integer16(i) => {
                        self.writer.write_i16(1070, *i)?;
                    }
                    XDataValue::Integer32(i) => {
                        self.writer.write_i32(1071, *i)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Write MULTILEADER entity
    fn write_multileader(&mut self, mleader: &crate::entities::MultiLeader, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("MULTILEADER")?;
        self.write_common_entity_data(mleader, owner)?;
        self.writer.write_subclass("AcDbMLeader")?;

        // Class version (hardcoded to 2 for R2010+)
        self.writer.write_i16(270, 2)?;

        // Context data - write the annotation context
        self.writer.write_subclass("CONTEXT_DATA{")?;

        // Content scale
        self.writer.write_double(40, mleader.context.scale_factor)?;

        // Content base point
        self.writer.write_point3d(10, mleader.context.content_base_point)?;

        // Text height
        self.writer.write_double(41, mleader.context.text_height)?;

        // Arrow head size
        self.writer.write_double(140, mleader.context.arrowhead_size)?;

        // Landing gap
        self.writer.write_double(145, mleader.context.landing_gap)?;

        // Has text contents (code 290-299 is Bool type - single byte in binary)
        self.writer.write_bool(290, mleader.context.has_text_contents)?;

        // Has block contents
        self.writer.write_bool(296, mleader.context.has_block_contents)?;

        // Text direction
        self.writer.write_point3d(110, mleader.context.text_direction)?;

        // Text location
        self.writer.write_point3d(111, mleader.context.text_location)?;

        // Text normal
        self.writer.write_point3d(112, mleader.context.text_normal)?;

        // Leader roots
        for root in &mleader.context.leader_roots {
            // Leader root connection point
            self.writer.write_point3d(10, root.connection_point)?;

            // Leader root direction
            self.writer.write_point3d(11, root.direction)?;

            // Number of leader lines
            self.writer.write_i16(302, root.lines.len() as i16)?;

            for line in &root.lines {
                // Leader line index
                self.writer.write_i16(304, line.index as i16)?;

                // Number of points
                self.writer.write_i16(305, line.points.len() as i16)?;

                // Points
                for pt in &line.points {
                    self.writer.write_point3d(10, *pt)?;
                }
            }
        }

        self.writer.write_string(301, "}")?; // End CONTEXT_DATA

        // Main properties
        // Content type
        self.writer.write_i16(170, mleader.content_type as i16)?;

        // Style handle
        if let Some(h) = mleader.style_handle {
            self.writer.write_handle(340, h)?;
        }

        // Path type
        self.writer.write_i16(171, mleader.path_type as i16)?;

        // Leader line color
        self.writer.write_color(91, mleader.line_color)?;

        // Leader line weight
        self.writer.write_i16(171, mleader.line_weight.value())?;

        // Enable landing (code 290-299 is Bool type)
        self.writer.write_bool(290, mleader.enable_landing)?;

        // Enable dogleg
        self.writer.write_bool(291, mleader.enable_dogleg)?;

        // Dogleg length
        self.writer.write_double(41, mleader.dogleg_length)?;

        // Arrowhead size
        self.writer.write_double(42, mleader.arrowhead_size)?;

        // Text style handle
        if let Some(h) = mleader.text_style_handle {
            self.writer.write_handle(342, h)?;
        }

        // Text left attachment type
        self.writer.write_i16(173, mleader.text_left_attachment as i16)?;

        // Text right attachment type
        self.writer.write_i16(95, mleader.text_right_attachment as i16)?;

        // Text angle type
        self.writer.write_i16(174, mleader.text_angle_type as i16)?;

        // Text alignment type
        self.writer.write_i16(175, mleader.text_alignment as i16)?;

        // Text color
        self.writer.write_color(92, mleader.text_color)?;

        // Text frame (code 292 is Bool type)
        self.writer.write_bool(292, mleader.text_frame)?;

        // Block content handle
        if let Some(h) = mleader.block_content_handle {
            self.writer.write_handle(343, h)?;
        }

        // Block content color
        self.writer.write_color(93, mleader.block_content_color)?;

        // Block content scale
        self.writer.write_point3d(10, mleader.block_scale)?;

        // Block content rotation
        self.writer.write_double(43, mleader.block_rotation)?;

        // Block content connection type
        self.writer.write_i16(176, mleader.block_connection_type as i16)?;

        // Enable annotation scale (code 293 is Bool type)
        self.writer.write_bool(293, mleader.enable_annotation_scale)?;

        // Text direction negative (code 294 is Bool type)
        self.writer.write_bool(294, mleader.text_direction_negative)?;

        // Text align in IPE
        self.writer.write_i16(178, mleader.text_align_in_ipe)?;

        // Text attachment point
        self.writer.write_i16(179, mleader.text_attachment_point as i16)?;

        // Scale factor
        self.writer.write_double(45, mleader.scale_factor)?;

        Ok(())
    }

    /// Write MLINE entity
    fn write_mline(&mut self, mline: &crate::entities::MLine, owner: Handle) -> Result<()> {
        
        self.writer.write_entity_type("MLINE")?;
        self.write_common_entity_data(mline, owner)?;
        self.writer.write_subclass("AcDbMline")?;

        // Style name
        self.writer.write_string(2, &mline.style_name)?;

        // Style handle
        if let Some(h) = mline.style_handle {
            self.writer.write_handle(340, h)?;
        }

        // Scale factor
        self.writer.write_double(40, mline.scale_factor)?;

        // Justification
        self.writer.write_i16(70, mline.justification as i16)?;

        // Flags
        self.writer.write_i16(71, mline.flags.bits())?;

        // Number of vertices
        self.writer.write_i16(72, mline.vertices.len() as i16)?;

        // Number of style elements
        self.writer.write_i16(73, mline.style_element_count as i16)?;

        // Start point
        self.writer.write_point3d(10, mline.start_point)?;

        // Normal
        self.writer.write_point3d(210, mline.normal)?;

        // Vertices
        for vertex in &mline.vertices {
            // Position
            self.writer.write_point3d(11, vertex.position)?;

            // Direction
            self.writer.write_point3d(12, vertex.direction)?;

            // Miter
            self.writer.write_point3d(13, vertex.miter)?;

            // Segments for each element
            for segment in &vertex.segments {
                // Number of parameters
                self.writer.write_i16(74, segment.parameters.len() as i16)?;

                // Parameters
                for param in &segment.parameters {
                    self.writer.write_double(41, *param)?;
                }

                // Number of area fill parameters
                self.writer.write_i16(75, segment.area_fill_parameters.len() as i16)?;

                // Area fill parameters
                for param in &segment.area_fill_parameters {
                    self.writer.write_double(42, *param)?;
                }
            }
        }

        Ok(())
    }

    /// Write MESH entity
    fn write_mesh(&mut self, mesh: &crate::entities::Mesh, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("MESH")?;
        self.write_common_entity_data(mesh, owner)?;
        self.writer.write_subclass("AcDbSubDMesh")?;

        // Version
        self.writer.write_i16(71, mesh.version)?;

        // Blend crease
        self.writer.write_i16(72, if mesh.blend_crease { 1 } else { 0 })?;

        // Subdivision level
        self.writer.write_i32(91, mesh.subdivision_level)?;

        // Vertex count
        self.writer.write_i32(92, mesh.vertices.len() as i32)?;

        // Vertices
        for v in &mesh.vertices {
            self.writer.write_point3d(10, *v)?;
        }

        // Face count (face list size = count of all indices + face size prefixes)
        let face_list_size: i32 = mesh.faces.iter().map(|f| 1 + f.vertices.len() as i32).sum();
        self.writer.write_i32(93, face_list_size)?;

        // Face data: each face is: vertex_count, v0, v1, v2, ...
        for face in &mesh.faces {
            self.writer.write_i32(90, face.vertices.len() as i32)?;
            for vi in &face.vertices {
                self.writer.write_i32(90, *vi as i32)?;
            }
        }

        // Edge count
        self.writer.write_i32(94, (mesh.edges.len() * 2) as i32)?;

        // Edges: start_index, end_index pairs
        for edge in &mesh.edges {
            self.writer.write_i32(90, edge.start as i32)?;
            self.writer.write_i32(90, edge.end as i32)?;
        }

        // Edge crease count
        let creased_edges: Vec<_> = mesh.edges.iter().enumerate()
            .filter(|(_, e)| e.has_crease())
            .collect();
        self.writer.write_i32(95, creased_edges.len() as i32)?;

        // Edge creases: index, crease_value pairs
        for (idx, edge) in creased_edges {
            self.writer.write_i32(90, idx as i32)?;
            self.writer.write_double(140, edge.crease_value())?;
        }

        Ok(())
    }

    /// Write IMAGE (RasterImage) entity
    fn write_raster_image(&mut self, image: &crate::entities::RasterImage, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("IMAGE")?;
        self.write_common_entity_data(image, owner)?;
        self.writer.write_subclass("AcDbRasterImage")?;

        // Class version
        self.writer.write_i32(90, image.class_version)?;

        // Insertion point
        self.writer.write_point3d(10, image.insertion_point)?;

        // U vector (size of single pixel in world)
        self.writer.write_point3d(11, image.u_vector)?;

        // V vector
        self.writer.write_point3d(12, image.v_vector)?;

        // Image size in pixels
        self.writer.write_double(13, image.size.x)?;
        self.writer.write_double(23, image.size.y)?;

        // Image definition handle
        if let Some(h) = image.definition_handle {
            self.writer.write_handle(340, h)?;
        }

        // Display properties
        self.writer.write_i16(70, image.flags.bits())?;

        // Clipping boundary on
        self.writer.write_byte(280, if image.clipping_enabled { 1 } else { 0 })?;

        // Brightness
        self.writer.write_byte(281, image.brightness)?;

        // Contrast
        self.writer.write_byte(282, image.contrast)?;

        // Fade
        self.writer.write_byte(283, image.fade)?;

        // Image definition reactor handle
        if let Some(h) = image.definition_reactor_handle {
            self.writer.write_handle(360, h)?;
        }

        // Clipping boundary type
        self.writer.write_i16(71, image.clip_boundary.clip_type as i16)?;

        // Number of clip boundary vertices
        self.writer.write_i16(91, image.clip_boundary.vertices.len() as i16)?;

        // Clip boundary vertices
        for v in &image.clip_boundary.vertices {
            self.writer.write_double(14, v.x)?;
            self.writer.write_double(24, v.y)?;
        }

        Ok(())
    }

    /// Write 3DSOLID entity
    fn write_solid3d(&mut self, solid: &Solid3D, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("3DSOLID")?;
        self.write_common_entity_data(solid, owner)?;
        self.writer.write_subclass("AcDbModelerGeometry")?;

        // Version
        self.writer.write_i16(70, solid.acis_data.version as i16)?;

        // Write ACIS data
        self.write_acis_data(&solid.acis_data)?;

        self.writer.write_subclass("AcDb3dSolid")?;

        // History handle
        if let Some(h) = solid.history_handle {
            self.writer.write_handle(350, h)?;
        }

        Ok(())
    }

    /// Write REGION entity
    fn write_region(&mut self, region: &Region, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("REGION")?;
        self.write_common_entity_data(region, owner)?;
        self.writer.write_subclass("AcDbModelerGeometry")?;

        // Version
        self.writer.write_i16(70, region.acis_data.version as i16)?;

        // Write ACIS data
        self.write_acis_data(&region.acis_data)?;

        Ok(())
    }

    /// Write BODY entity
    fn write_body(&mut self, body: &Body, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("BODY")?;
        self.write_common_entity_data(body, owner)?;
        self.writer.write_subclass("AcDbModelerGeometry")?;

        // Version
        self.writer.write_i16(70, body.acis_data.version as i16)?;

        // Write ACIS data
        self.write_acis_data(&body.acis_data)?;

        Ok(())
    }

    /// Write ACIS data (shared by Solid3D, Region, Body)
    fn write_acis_data(&mut self, acis: &AcisData) -> Result<()> {
        // Write ACIS data as 255-byte chunks using group code 1
        // Final chunk uses group code 3
        let data = &acis.sat_data;
        let chunk_size = 255;
        let chunks: Vec<&str> = data.as_bytes()
            .chunks(chunk_size)
            .map(|c| std::str::from_utf8(c).unwrap_or(""))
            .collect();

        if chunks.is_empty() {
            // Write empty string with group code 1
            self.writer.write_string(1, "")?;
        } else {
            // Write all chunks except the last with group code 1
            for chunk in chunks.iter().take(chunks.len().saturating_sub(1)) {
                self.writer.write_string(1, chunk)?;
            }
            // Write last chunk with group code 3
            if let Some(last) = chunks.last() {
                self.writer.write_string(3, last)?;
            }
        }

        Ok(())
    }

    /// Write ACAD_TABLE entity
    fn write_acad_table(&mut self, table: &table::Table, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("ACAD_TABLE")?;
        self.write_common_entity_data(table, owner)?;
        self.writer.write_subclass("AcDbBlockReference")?;

        // Block record handle
        if let Some(h) = table.block_record_handle {
            self.writer.write_handle(2, h)?;
        }

        // Insertion point
        self.writer.write_point3d(10, table.insertion_point)?;

        self.writer.write_subclass("AcDbTable")?;

        // Table style handle
        if let Some(h) = table.table_style_handle {
            self.writer.write_handle(342, h)?;
        }

        // Data version
        self.writer.write_byte(280, table.data_version as u8)?;

        // Horizontal direction
        self.writer.write_point3d(11, table.horizontal_direction)?;

        // Number of rows
        self.writer.write_i16(91, table.rows.len() as i16)?;

        // Number of columns
        self.writer.write_i16(92, table.columns.len() as i16)?;

        // Override flags
        let mut override_flags = 0i16;
        if table.override_flag { override_flags |= 1; }
        if table.override_border_color { override_flags |= 2; }
        if table.override_border_line_weight { override_flags |= 4; }
        if table.override_border_visibility { override_flags |= 8; }
        self.writer.write_i16(93, override_flags)?;

        // Row heights
        for row in &table.rows {
            self.writer.write_double(141, row.height)?;
        }

        // Column widths
        for col in &table.columns {
            self.writer.write_double(142, col.width)?;
        }

        // Write cells
        for row in &table.rows {
            for cell in &row.cells {
                self.write_table_cell(cell)?;
            }
        }

        // Break options
        self.writer.write_i16(94, table.break_options.bits() as i16)?;
        self.writer.write_i16(95, table.break_flow_direction as i16)?;
        self.writer.write_double(143, table.break_spacing)?;

        Ok(())
    }

    /// Write table cell data
    fn write_table_cell(&mut self, cell: &TableCell) -> Result<()> {
        // Cell type
        self.writer.write_i16(171, cell.cell_type as i16)?;

        // Cell state flags
        self.writer.write_i16(172, cell.state.bits() as i16)?;

        // Cell flags
        self.writer.write_i16(173, cell.flag as i16)?;

        // Merged dimensions
        self.writer.write_i16(174, cell.merged as i16)?;
        self.writer.write_i16(175, cell.merge_width as i16)?;
        self.writer.write_i16(176, cell.merge_height as i16)?;

        // Virtual edge flag
        self.writer.write_i16(177, cell.virtual_edge)?;

        // Rotation
        self.writer.write_double(144, cell.rotation)?;

        // Contents count
        self.writer.write_i16(179, cell.contents.len() as i16)?;

        // Write cell contents
        for content in &cell.contents {
            self.writer.write_i16(170, content.content_type as i16)?;

            // Write value based on type
            match content.value.value_type {
                CellValueType::String => {
                    self.writer.write_string(1, &content.value.text)?;
                }
                CellValueType::Double => {
                    self.writer.write_double(140, content.value.numeric_value)?;
                }
                CellValueType::Long => {
                    self.writer.write_i16(90, content.value.numeric_value as i16)?;
                }
                _ => {}
            }

            // Format string
            if !content.value.format.is_empty() {
                self.writer.write_string(300, &content.value.format)?;
            }

            // Block handle
            if let Some(h) = content.block_handle {
                self.writer.write_handle(340, h)?;
            }
        }

        // Cell style
        if let Some(ref style) = cell.style {
            self.writer.write_color(62, style.content_color)?;
            self.writer.write_double(140, style.text_height)?;
            self.writer.write_double(144, style.rotation)?;
            self.writer.write_i16(170, style.alignment as i16)?;
        }

        Ok(())
    }

    /// Write a Tolerance entity
    fn write_tolerance(&mut self, tolerance: &Tolerance, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("TOLERANCE")?;
        self.write_common_entity_data(tolerance, owner)?;
        self.writer.write_subclass("AcDbFcf")?;

        // Dimension style name
        self.writer.write_string(3, &tolerance.dimension_style_name)?;

        // Insertion point
        self.writer.write_double(10, tolerance.insertion_point.x)?;
        self.writer.write_double(20, tolerance.insertion_point.y)?;
        self.writer.write_double(30, tolerance.insertion_point.z)?;

        // Normal vector
        self.writer.write_double(210, tolerance.normal.x)?;
        self.writer.write_double(220, tolerance.normal.y)?;
        self.writer.write_double(230, tolerance.normal.z)?;

        // Direction vector
        self.writer.write_double(11, tolerance.direction.x)?;
        self.writer.write_double(21, tolerance.direction.y)?;
        self.writer.write_double(31, tolerance.direction.z)?;

        // Tolerance text
        self.writer.write_string(1, &tolerance.text)?;

        Ok(())
    }

    /// Write a PolyfaceMesh entity
    fn write_polyface_mesh(&mut self, mesh: &PolyfaceMesh, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("POLYLINE")?;
        self.write_common_entity_data(mesh, owner)?;
        self.writer.write_subclass("AcDbPolyFaceMesh")?;

        // Entities follow flag (VERTEX records follow)
        self.writer.write_i16(66, 1)?;

        // Dummy point with elevation (ACadSharp pattern)
        self.writer.write_double(10, 0.0)?;
        self.writer.write_double(20, 0.0)?;
        self.writer.write_double(30, mesh.elevation)?;
        
        // Polyline flags (64 = polyface mesh) - MUST be before 71/72
        self.writer.write_i16(70, mesh.flags.bits())?;
        
        // Vertex count - MUST come before smooth surface type
        self.writer.write_i16(71, mesh.vertex_count() as i16)?;
        // Face count - MUST come before smooth surface type  
        self.writer.write_i16(72, mesh.face_count() as i16)?;

        // Write vertices
        let mesh_handle = mesh.common.handle;
        let base_handle = mesh_handle.value();
        for (index, vertex) in mesh.vertices.iter().enumerate() {
            let vertex_handle = if vertex.common.handle.is_null() {
                Handle::new(base_handle + index as u64 + 1)
            } else {
                vertex.common.handle
            };
            self.writer.write_entity_type("VERTEX")?;
            self.writer.write_handle(5, vertex_handle)?;
            self.writer.write_handle(330, mesh.common.handle)?;
            self.writer.write_subclass("AcDbEntity")?;
            self.writer.write_string(8, &vertex.common.layer)?;
            self.writer.write_subclass("AcDbVertex")?;
            self.writer.write_subclass("AcDbPolyFaceMeshVertex")?;

            self.writer.write_double(10, vertex.location.x)?;
            self.writer.write_double(20, vertex.location.y)?;
            self.writer.write_double(30, vertex.location.z)?;

            let flags = vertex.flags | PolyfaceVertexFlags::POLYGON_MESH;
            self.writer.write_i16(70, flags.bits())?;
        }

        // Write faces
        for (index, face) in mesh.faces.iter().enumerate() {
            let face_handle = if face.common.handle.is_null() {
                Handle::new(base_handle + mesh.vertices.len() as u64 + index as u64 + 1)
            } else {
                face.common.handle
            };
            self.writer.write_entity_type("VERTEX")?;
            self.writer.write_handle(5, face_handle)?;
            self.writer.write_handle(330, mesh.common.handle)?;
            self.writer.write_subclass("AcDbEntity")?;
            self.writer.write_string(8, &face.common.layer)?;
            self.writer.write_subclass("AcDbFaceRecord")?;

            // Dummy position
            self.writer.write_double(10, 0.0)?;
            self.writer.write_double(20, 0.0)?;
            self.writer.write_double(30, 0.0)?;

            let flags = face.flags | PolyfaceVertexFlags::POLYFACE_MESH;
            self.writer.write_i16(70, flags.bits())?; // Face record flag

            // Vertex indices
            let indices = face.vertex_indices();
            if indices.len() >= 3 {
                self.writer.write_i16(71, indices[0])?;
                self.writer.write_i16(72, indices[1])?;
                self.writer.write_i16(73, indices[2])?;
                if indices.len() >= 4 {
                    self.writer.write_i16(74, indices[3])?;
                }
            }
        }

        // Write SEQEND
        self.writer.write_entity_type("SEQEND")?;
        let seqend_handle = mesh.seqend_handle.unwrap_or_else(|| {
            Handle::new(base_handle + mesh.vertices.len() as u64 + mesh.faces.len() as u64 + 1)
        });
        self.writer.write_handle(5, seqend_handle)?;
        self.writer.write_handle(330, mesh.common.handle)?;
        self.writer.write_subclass("AcDbEntity")?;
        self.writer.write_subclass("AcDbSequenceEnd")?;
        self.writer.write_string(8, &mesh.common.layer)?;

        Ok(())
    }

    /// Write a Wipeout entity
    fn write_wipeout(&mut self, wipeout: &Wipeout, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("WIPEOUT")?;
        self.write_common_entity_data(wipeout, owner)?;
        self.writer.write_subclass("AcDbWipeout")?;

        // Class version
        self.writer.write_i32(90, wipeout.class_version)?;

        // Insertion point
        self.writer.write_double(10, wipeout.insertion_point.x)?;
        self.writer.write_double(20, wipeout.insertion_point.y)?;
        self.writer.write_double(30, wipeout.insertion_point.z)?;

        // U-vector
        self.writer.write_double(11, wipeout.u_vector.x)?;
        self.writer.write_double(21, wipeout.u_vector.y)?;
        self.writer.write_double(31, wipeout.u_vector.z)?;

        // V-vector
        self.writer.write_double(12, wipeout.v_vector.x)?;
        self.writer.write_double(22, wipeout.v_vector.y)?;
        self.writer.write_double(32, wipeout.v_vector.z)?;

        // Size
        self.writer.write_double(13, wipeout.size.x)?;
        self.writer.write_double(23, wipeout.size.y)?;

        // Display flags
        self.writer.write_i16(70, wipeout.flags.bits())?;

        // Clipping
        self.writer.write_byte(280, if wipeout.clipping_enabled { 1 } else { 0 })?;
        self.writer.write_byte(281, wipeout.brightness)?;
        self.writer.write_byte(282, wipeout.contrast)?;
        self.writer.write_byte(283, wipeout.fade)?;

        // Clip boundary type
        self.writer.write_i16(71, wipeout.clip_type as i16)?;

        // Clip boundary count
        self.writer.write_i32(91, wipeout.clip_boundary_vertices.len() as i32)?;

        // Clip boundary vertices
        for v in &wipeout.clip_boundary_vertices {
            self.writer.write_double(14, v.x)?;
            self.writer.write_double(24, v.y)?;
        }

        Ok(())
    }

    /// Write a Shape entity
    fn write_shape(&mut self, shape: &Shape, owner: Handle) -> Result<()> {
        self.writer.write_entity_type("SHAPE")?;
        self.write_common_entity_data(shape, owner)?;
        self.writer.write_subclass("AcDbShape")?;

        // Thickness
        if shape.thickness.abs() > 1e-10 {
            self.writer.write_double(39, shape.thickness)?;
        }

        // Insertion point
        self.writer.write_double(10, shape.insertion_point.x)?;
        self.writer.write_double(20, shape.insertion_point.y)?;
        self.writer.write_double(30, shape.insertion_point.z)?;

        // Size
        self.writer.write_double(40, shape.size)?;

        // Shape name
        self.writer.write_string(2, &shape.shape_name)?;

        // Rotation
        if shape.rotation.abs() > 1e-10 {
            self.writer.write_double(50, shape.rotation.to_degrees())?;
        }

        // Relative X scale
        if (shape.relative_x_scale - 1.0).abs() > 1e-10 {
            self.writer.write_double(41, shape.relative_x_scale)?;
        }

        // Oblique angle
        if shape.oblique_angle.abs() > 1e-10 {
            self.writer.write_double(51, shape.oblique_angle.to_degrees())?;
        }

        // Normal
        if shape.has_custom_normal() {
            self.writer.write_double(210, shape.normal.x)?;
            self.writer.write_double(220, shape.normal.y)?;
            self.writer.write_double(230, shape.normal.z)?;
        }

        Ok(())
    }

    /// Write an Underlay entity (PDF, DWF, or DGN)
    fn write_underlay(&mut self, underlay: &Underlay, owner: Handle) -> Result<()> {
        self.writer.write_entity_type(underlay.entity_name())?;
        self.write_common_entity_data(underlay, owner)?;
        self.writer.write_subclass("AcDbUnderlayReference")?;

        // Definition handle
        self.writer.write_handle(340, underlay.definition_handle)?;

        // Insertion point
        self.writer.write_double(10, underlay.insertion_point.x)?;
        self.writer.write_double(20, underlay.insertion_point.y)?;
        self.writer.write_double(30, underlay.insertion_point.z)?;

        // Scale factors
        self.writer.write_double(41, underlay.x_scale)?;
        self.writer.write_double(42, underlay.y_scale)?;
        self.writer.write_double(43, underlay.z_scale)?;

        // Rotation
        self.writer.write_double(50, underlay.rotation.to_degrees())?;

        // Normal
        self.writer.write_double(210, underlay.normal.x)?;
        self.writer.write_double(220, underlay.normal.y)?;
        self.writer.write_double(230, underlay.normal.z)?;

        // Flags
        self.writer.write_byte(280, underlay.flags.bits())?;

        // Contrast
        self.writer.write_byte(281, underlay.contrast)?;

        // Fade
        self.writer.write_byte(282, underlay.fade)?;

        // Clip boundary vertices count
        self.writer.write_i32(91, underlay.clip_boundary_vertices.len() as i32)?;

        // Clip boundary vertices
        for v in &underlay.clip_boundary_vertices {
            self.writer.write_double(11, v.x)?;
            self.writer.write_double(21, v.y)?;
        }

        Ok(())
    }
}

/// Helper to extract invisible edge bits
fn get_invisible_edge_bits(flags: &InvisibleEdgeFlags) -> u8 {
    let mut bits = 0u8;
    if flags.is_first_invisible() { bits |= 1; }
    if flags.is_second_invisible() { bits |= 2; }
    if flags.is_third_invisible() { bits |= 4; }
    if flags.is_fourth_invisible() { bits |= 8; }
    bits
}

/// Helper to extract boundary path flag bits
fn get_boundary_path_bits(flags: &BoundaryPathFlags) -> u32 {
    let mut bits = 0u32;
    if flags.is_external() { bits |= 1; }
    if flags.is_polyline() { bits |= 2; }
    if flags.is_derived() { bits |= 4; }
    bits
}
