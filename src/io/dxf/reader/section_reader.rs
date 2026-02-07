//! DXF section readers

use super::stream_reader::{DxfStreamReader, PointReader};
use crate::document::CadDocument;
use crate::entities::*;
use crate::error::Result;
use crate::objects::{Dictionary, Layout, ObjectType};
use crate::tables::*;
use crate::tables::linetype::LineTypeElement;
use crate::types::*;
use crate::xdata::{ExtendedData, ExtendedDataRecord, XDataValue};

/// Section reader for parsing DXF sections
pub struct SectionReader<'a> {
    reader: &'a mut Box<dyn DxfStreamReader>,
}

impl<'a> SectionReader<'a> {
    /// Create a new section reader
    pub fn new(reader: &'a mut Box<dyn DxfStreamReader>) -> Self {
        Self { reader }
    }
    
    /// Read the HEADER section
    pub fn read_header(&mut self, _document: &mut CadDocument) -> Result<()> {
        // Read header variables until ENDSEC
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDSEC" {
                break;
            }
            
            // Header variables start with code 9 (variable name)
            if pair.code == 9 {
                let var_name = pair.value_string.clone();
                
                // Read the variable value(s)
                match var_name.as_str() {
                    "$ACADVER" => {
                        // Already read during version detection
                        self.reader.read_pair()?;
                    }
                    "$HANDSEED" => {
                        if let Some(value_pair) = self.reader.read_pair()? {
                            if let Some(handle) = value_pair.as_handle() {
                                // Store handle seed for later use
                                let _ = handle;
                            }
                        }
                    }
                    _ => {
                        // Skip unknown header variable
                        self.reader.read_pair()?;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Read the CLASSES section
    pub fn read_classes(&mut self, _document: &mut CadDocument) -> Result<()> {
        // Read classes until ENDSEC
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDSEC" {
                break;
            }
            
            // Classes are defined with code 0 = "CLASS"
            if pair.code == 0 && pair.value_string == "CLASS" {
                // Skip class definition for now
                while let Some(class_pair) = self.reader.read_pair()? {
                    if class_pair.code == 0 {
                        // Next entity - push back and break to outer loop
                        self.reader.push_back(class_pair);
                        break;
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Read the TABLES section
    pub fn read_tables(&mut self, document: &mut CadDocument) -> Result<()> {
        // Read tables until ENDSEC
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDSEC" {
                break;
            }
            
            // Tables start with code 0 = "TABLE"
            if pair.code == 0 && pair.value_string == "TABLE" {
                // Read table name (code 2)
                if let Some(name_pair) = self.reader.read_pair()? {
                    if name_pair.code == 2 {
                        match name_pair.value_string.as_str() {
                            "LAYER" => self.read_layer_table(document)?,
                            "LTYPE" => self.read_linetype_table(document)?,
                            "STYLE" => self.read_textstyle_table(document)?,
                            "BLOCK_RECORD" => self.read_block_record_table(document)?,
                            "DIMSTYLE" => self.read_dimstyle_table(document)?,
                            "APPID" => self.read_appid_table(document)?,
                            "VIEW" => self.read_view_table(document)?,
                            "VPORT" => self.read_vport_table(document)?,
                            "UCS" => self.read_ucs_table(document)?,
                            _ => {
                                // Skip unknown table
                                self.skip_to_endtab()?;
                            }
                        }
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Read the BLOCKS section
    pub fn read_blocks(&mut self, document: &mut CadDocument) -> Result<()> {
        // Read blocks until ENDSEC
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDSEC" {
                break;
            }

            // Blocks start with code 0 = "BLOCK"
            if pair.code == 0 && pair.value_string == "BLOCK" {
                self.read_block(document)?;
            }
        }

        Ok(())
    }

    /// Read a single BLOCK...ENDBLK definition
    fn read_block(&mut self, document: &mut CadDocument) -> Result<()> {
        use crate::entities::Block;
        use crate::types::Vector3;

        let mut block_name = String::new();
        let mut base_point = Vector3::new(0.0, 0.0, 0.0);
        let mut description = String::new();
        let mut xref_path = String::new();
        let mut layer = String::from("0");
        let mut handle = Handle::NULL;

        let mut point_reader = PointReader::new();

        // Read BLOCK entity properties
        while let Some(pair) = self.reader.read_pair()? {
            match pair.code {
                0 => {
                    // Start of next entity - put it back and break
                    self.reader.push_back(pair);
                    break;
                }
                2 => {
                    // Block name
                    block_name = pair.value_string.clone();
                }
                3 => {
                    // Block name (alternate)
                    if block_name.is_empty() {
                        block_name = pair.value_string.clone();
                    }
                }
                4 => {
                    // Description
                    description = pair.value_string.clone();
                }
                1 => {
                    // XRef path
                    xref_path = pair.value_string.clone();
                }
                5 => {
                    // Handle
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        handle = Handle::new(h);
                    }
                }
                8 => {
                    // Layer
                    layer = pair.value_string.clone();
                }
                10 | 20 | 30 => {
                    // Base point coordinates
                    point_reader.add_coordinate(&pair);
                    if let Some(pt) = point_reader.get_point() {
                        base_point = pt;
                    }
                }
                _ => {}
            }
        }

        // Create Block entity
        let mut block = Block::new(block_name.clone(), base_point);
        block.common.handle = handle;
        block.common.layer = layer.clone();
        block.description = description;
        block.xref_path = xref_path;

        // Find the corresponding BlockRecord and add entities to it
        let mut block_entities: Vec<EntityType> = Vec::new();

        // Read entities until ENDBLK
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                match pair.value_string.as_str() {
                    "ENDBLK" => {
                        // Read ENDBLK properties
                        let _block_end = self.read_block_end()?;

                        // Find the BlockRecord and add entities
                        if let Some(block_record) = document.block_records.get_mut(&block_name) {
                            block_record.entities = block_entities;
                        }

                        // Note: Block and BlockEnd are block definition markers, not drawing entities.
                        // They are not added to the document's main entity list.
                        // The block content is stored in the BlockRecord.

                        break;
                    }
                    "POINT" => {
                        if let Some(entity) = self.read_point()? {
                            block_entities.push(EntityType::Point(entity));
                        }
                    }
                    "LINE" => {
                        if let Some(entity) = self.read_line()? {
                            block_entities.push(EntityType::Line(entity));
                        }
                    }
                    "CIRCLE" => {
                        if let Some(entity) = self.read_circle()? {
                            block_entities.push(EntityType::Circle(entity));
                        }
                    }
                    "ARC" => {
                        if let Some(entity) = self.read_arc()? {
                            block_entities.push(EntityType::Arc(entity));
                        }
                    }
                    "ELLIPSE" => {
                        if let Some(entity) = self.read_ellipse()? {
                            block_entities.push(EntityType::Ellipse(entity));
                        }
                    }
                    "POLYLINE" => {
                        if let Some(entity) = self.read_polyline()? {
                            block_entities.push(EntityType::Polyline(entity));
                        }
                    }
                    "LWPOLYLINE" => {
                        if let Some(entity) = self.read_lwpolyline()? {
                            block_entities.push(EntityType::LwPolyline(entity));
                        }
                    }
                    "TEXT" => {
                        if let Some(entity) = self.read_text()? {
                            block_entities.push(EntityType::Text(entity));
                        }
                    }
                    "MTEXT" => {
                        if let Some(entity) = self.read_mtext()? {
                            block_entities.push(EntityType::MText(entity));
                        }
                    }
                    "SPLINE" => {
                        if let Some(entity) = self.read_spline()? {
                            block_entities.push(EntityType::Spline(entity));
                        }
                    }
                    _ => {
                        // Skip unknown entity type
                    }
                }
            }
        }

        Ok(())
    }

    /// Read ENDBLK entity
    fn read_block_end(&mut self) -> Result<BlockEnd> {
        use crate::entities::BlockEnd;

        let mut block_end = BlockEnd::new();
        let mut layer = String::from("0");
        let mut handle = Handle::NULL;

        while let Some(pair) = self.reader.read_pair()? {
            match pair.code {
                0 => {
                    // Next entity - push back and break
                    self.reader.push_back(pair);
                    break;
                }
                5 => {
                    // Handle
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        handle = Handle::new(h);
                    }
                }
                8 => {
                    // Layer
                    layer = pair.value_string.clone();
                }
                _ => {}
            }
        }

        block_end.common.handle = handle;
        block_end.common.layer = layer;

        Ok(block_end)
    }
    
    /// Read the ENTITIES section
    pub fn read_entities(&mut self, document: &mut CadDocument) -> Result<()> {
        // Read entities until ENDSEC
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDSEC" {
                break;
            }
            
            // Entities start with code 0
            if pair.code == 0 {
                let entity_type = pair.value_string.clone();
                
                match entity_type.as_str() {
                    "POINT" => {
                        if let Some(entity) = self.read_point()? {
                            let _ = document.add_entity(EntityType::Point(entity));
                        }
                    }
                    "LINE" => {
                        if let Some(entity) = self.read_line()? {
                            let _ = document.add_entity(EntityType::Line(entity));
                        }
                    }
                    "CIRCLE" => {
                        if let Some(entity) = self.read_circle()? {
                            let _ = document.add_entity(EntityType::Circle(entity));
                        }
                    }
                    "ARC" => {
                        if let Some(entity) = self.read_arc()? {
                            let _ = document.add_entity(EntityType::Arc(entity));
                        }
                    }
                    "ELLIPSE" => {
                        if let Some(entity) = self.read_ellipse()? {
                            let _ = document.add_entity(EntityType::Ellipse(entity));
                        }
                    }
                    "POLYLINE" => {
                        if let Some(entity) = self.read_polyline()? {
                            let _ = document.add_entity(EntityType::Polyline(entity));
                        }
                    }
                    "LWPOLYLINE" => {
                        if let Some(entity) = self.read_lwpolyline()? {
                            let _ = document.add_entity(EntityType::LwPolyline(entity));
                        }
                    }
                    "TEXT" => {
                        if let Some(entity) = self.read_text()? {
                            let _ = document.add_entity(EntityType::Text(entity));
                        }
                    }
                    "MTEXT" => {
                        if let Some(entity) = self.read_mtext()? {
                            let _ = document.add_entity(EntityType::MText(entity));
                        }
                    }
                    "SPLINE" => {
                        if let Some(entity) = self.read_spline()? {
                            let _ = document.add_entity(EntityType::Spline(entity));
                        }
                    }
                    "DIMENSION" => {
                        if let Some(entity) = self.read_dimension()? {
                            let _ = document.add_entity(EntityType::Dimension(entity));
                        }
                    }
                    "HATCH" => {
                        if let Some(entity) = self.read_hatch()? {
                            let _ = document.add_entity(EntityType::Hatch(entity));
                        }
                    }
                    "SOLID" => {
                        if let Some(entity) = self.read_solid()? {
                            let _ = document.add_entity(EntityType::Solid(entity));
                        }
                    }
                    "3DFACE" => {
                        if let Some(entity) = self.read_face3d()? {
                            let _ = document.add_entity(EntityType::Face3D(entity));
                        }
                    }
                    "INSERT" => {
                        if let Some(entity) = self.read_insert()? {
                            let _ = document.add_entity(EntityType::Insert(entity));
                        }
                    }
                    "RAY" => {
                        if let Some(entity) = self.read_ray()? {
                            let _ = document.add_entity(EntityType::Ray(entity));
                        }
                    }
                    "XLINE" => {
                        if let Some(entity) = self.read_xline()? {
                            let _ = document.add_entity(EntityType::XLine(entity));
                        }
                    }
                    "ATTDEF" => {
                        if let Some(entity) = self.read_attdef()? {
                            let _ = document.add_entity(EntityType::AttributeDefinition(entity));
                        }
                    }
                    "TOLERANCE" => {
                        if let Some(entity) = self.read_tolerance()? {
                            let _ = document.add_entity(EntityType::Tolerance(entity));
                        }
                    }
                    "SHAPE" => {
                        if let Some(entity) = self.read_shape()? {
                            let _ = document.add_entity(EntityType::Shape(entity));
                        }
                    }
                    "WIPEOUT" => {
                        if let Some(entity) = self.read_wipeout()? {
                            let _ = document.add_entity(EntityType::Wipeout(entity));
                        }
                    }
                    _ => {
                        // Skip unknown entity type
                    }
                }
            }
        }
        
        Ok(())
    }
    
    /// Read the OBJECTS section
    pub fn read_objects(&mut self, document: &mut CadDocument) -> Result<()> {
        // Read objects until ENDSEC
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDSEC" {
                break;
            }

            // Objects start with code 0
            if pair.code == 0 {
                match pair.value_string.as_str() {
                    "DICTIONARY" | "ACDBDICTIONARYWDFLT" => {
                        if let Some(obj) = self.read_dictionary()? {
                            document.objects.insert(obj.handle, ObjectType::Dictionary(obj));
                        }
                    }
                    "LAYOUT" => {
                        if let Some(obj) = self.read_layout()? {
                            document.objects.insert(obj.handle, ObjectType::Layout(obj));
                        }
                    }
                    _ => {
                        // Unknown object type - skip it
                        self.skip_unknown_object(&pair.value_string)?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Read a DICTIONARY object
    fn read_dictionary(&mut self) -> Result<Option<Dictionary>> {
        let mut dict = Dictionary::new();
        let mut current_key: Option<String> = None;

        while let Some(pair) = self.reader.read_pair()? {
            match pair.code {
                0 => {
                    // Next object - push back and break
                    self.reader.push_back(pair);
                    break;
                }
                5 => {
                    // Handle
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        dict.handle = Handle::new(h);
                    }
                }
                330 => {
                    // Owner handle
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        dict.owner = Handle::new(h);
                    }
                }
                281 => {
                    // Duplicate record cloning flag
                    if let Some(value) = pair.as_i16() {
                        dict.duplicate_cloning = value;
                    }
                }
                280 => {
                    // Hard owner flag
                    if let Some(value) = pair.as_i16() {
                        dict.hard_owner = value != 0;
                    }
                }
                3 => {
                    // Entry key (name)
                    current_key = Some(pair.value_string.clone());
                }
                350 | 360 => {
                    // Entry value (handle) - 350 is soft owner, 360 is hard owner
                    if let Some(key) = current_key.take() {
                        if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                            dict.add_entry(key, Handle::new(h));
                        }
                    }
                }
                _ => {}
            }
        }

        Ok(Some(dict))
    }

    /// Read a LAYOUT object
    fn read_layout(&mut self) -> Result<Option<Layout>> {
        let mut layout = Layout::new("");

        while let Some(pair) = self.reader.read_pair()? {
            match pair.code {
                0 => {
                    // Next object - push back and break
                    self.reader.push_back(pair);
                    break;
                }
                5 => {
                    // Handle
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        layout.handle = Handle::new(h);
                    }
                }
                330 => {
                    // Owner handle
                    if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                        layout.owner = Handle::new(h);
                    }
                }
                1 => {
                    // Layout name
                    layout.name = pair.value_string.clone();
                }
                70 => {
                    // Layout flags
                    if let Some(value) = pair.as_i16() {
                        layout.flags = value;
                    }
                }
                71 => {
                    // Tab order
                    if let Some(value) = pair.as_i16() {
                        layout.tab_order = value;
                    }
                }
                10 => {
                    // Min limits X
                    if let Some(value) = pair.as_double() {
                        layout.min_limits.0 = value;
                    }
                }
                20 => {
                    // Min limits Y
                    if let Some(value) = pair.as_double() {
                        layout.min_limits.1 = value;
                    }
                }
                11 => {
                    // Max limits X
                    if let Some(value) = pair.as_double() {
                        layout.max_limits.0 = value;
                    }
                }
                21 => {
                    // Max limits Y
                    if let Some(value) = pair.as_double() {
                        layout.max_limits.1 = value;
                    }
                }
                _ => {}
            }
        }

        Ok(Some(layout))
    }

    /// Skip an unknown object type
    fn skip_unknown_object(&mut self, _type_name: &str) -> Result<()> {
        // Read until next code 0
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }
        }
        Ok(())
    }
    
    /// Skip to ENDTAB
    fn skip_to_endtab(&mut self) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }
        }
        Ok(())
    }

    // ===== Table Readers =====

    /// Read LAYER table
    fn read_layer_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "LAYER" {
                if let Some(layer) = self.read_layer_entry()? {
                    let _ = document.layers.add(layer);
                }
            }
        }
        Ok(())
    }

    /// Read a single LAYER entry
    fn read_layer_entry(&mut self) -> Result<Option<Layer>> {
        let mut layer = Layer::new("0");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                // Next entity - push back and break
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                2 => layer.name = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        layer.color = Color::from_index(color_index);
                    }
                }
                6 => layer.line_type = pair.value_string.clone(),
                70 => {
                    if let Some(flags) = pair.as_i16() {
                        layer.flags.frozen = (flags & 1) != 0;
                        layer.flags.locked = (flags & 4) != 0;
                        layer.flags.off = (flags & 2) != 0;
                    }
                }
                290 => {
                    if let Some(plotting) = pair.as_bool() {
                        layer.is_plottable = plotting;
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        layer.line_weight = LineWeight::from_value(lw);
                    }
                }
                _ => {}
            }
        }

        Ok(Some(layer))
    }

    /// Read LTYPE table
    fn read_linetype_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "LTYPE" {
                if let Some(linetype) = self.read_linetype_entry()? {
                    let _ = document.line_types.add(linetype);
                }
            }
        }
        Ok(())
    }

    /// Read a single LTYPE entry
    fn read_linetype_entry(&mut self) -> Result<Option<LineType>> {
        let mut linetype = LineType::new("Continuous");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                2 => linetype.name = pair.value_string.clone(),
                3 => linetype.description = pair.value_string.clone(),
                73 => {
                    if let Some(count) = pair.as_i16() {
                        linetype.elements.reserve(count as usize);
                    }
                }
                40 => {
                    if let Some(length) = pair.as_double() {
                        linetype.pattern_length = length;
                    }
                }
                49 => {
                    if let Some(dash) = pair.as_double() {
                        linetype.elements.push(LineTypeElement { length: dash });
                    }
                }
                _ => {}
            }
        }

        Ok(Some(linetype))
    }

    /// Read STYLE table
    fn read_textstyle_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "STYLE" {
                if let Some(style) = self.read_textstyle_entry()? {
                    let _ = document.text_styles.add(style);
                }
            }
        }
        Ok(())
    }

    /// Read a single STYLE entry
    fn read_textstyle_entry(&mut self) -> Result<Option<TextStyle>> {
        let mut style = TextStyle::new("Standard");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                2 => style.name = pair.value_string.clone(),
                3 => style.font_file = pair.value_string.clone(),
                4 => style.big_font_file = pair.value_string.clone(),
                40 => {
                    if let Some(height) = pair.as_double() {
                        style.height = height;
                    }
                }
                41 => {
                    if let Some(width) = pair.as_double() {
                        style.width_factor = width;
                    }
                }
                50 => {
                    if let Some(angle) = pair.as_double() {
                        style.oblique_angle = angle;
                    }
                }
                71 => {
                    if let Some(flags) = pair.as_i16() {
                        style.flags.backward = (flags & 2) != 0;
                        style.flags.upside_down = (flags & 4) != 0;
                    }
                }
                _ => {}
            }
        }

        Ok(Some(style))
    }

    /// Read BLOCK_RECORD table
    fn read_block_record_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "BLOCK_RECORD" {
                if let Some(block_record) = self.read_block_record_entry()? {
                    let _ = document.block_records.add(block_record);
                }
            }
        }
        Ok(())
    }

    /// Read a single BLOCK_RECORD entry
    fn read_block_record_entry(&mut self) -> Result<Option<BlockRecord>> {
        let mut block_record = BlockRecord::new("*Model_Space");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                2 => block_record.name = pair.value_string.clone(),
                70 => {
                    if let Some(flags) = pair.as_i16() {
                        block_record.flags.anonymous = (flags & 1) != 0;
                        block_record.flags.has_attributes = (flags & 2) != 0;
                        block_record.flags.is_xref = (flags & 4) != 0;
                        block_record.flags.is_xref_overlay = (flags & 8) != 0;
                    }
                }
                280 => {
                    if let Some(units) = pair.as_i16() {
                        block_record.units = units;
                    }
                }
                _ => {}
            }
        }

        Ok(Some(block_record))
    }

    /// Read DIMSTYLE table
    fn read_dimstyle_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "DIMSTYLE" {
                if let Some(dimstyle) = self.read_dimstyle_entry()? {
                    let _ = document.dim_styles.add(dimstyle);
                }
            }
        }
        Ok(())
    }

    /// Read a single DIMSTYLE entry
    fn read_dimstyle_entry(&mut self) -> Result<Option<DimStyle>> {
        let mut dimstyle = DimStyle::new("Standard");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            if pair.code == 2 {
                dimstyle.name = pair.value_string.clone();
            }
        }

        Ok(Some(dimstyle))
    }

    /// Read APPID table
    fn read_appid_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "APPID" {
                if let Some(appid) = self.read_appid_entry()? {
                    let _ = document.app_ids.add(appid);
                }
            }
        }
        Ok(())
    }

    /// Read a single APPID entry
    fn read_appid_entry(&mut self) -> Result<Option<AppId>> {
        let mut appid = AppId::new("ACAD");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            if pair.code == 2 {
                appid.name = pair.value_string.clone();
            }
        }

        Ok(Some(appid))
    }

    /// Read VIEW table
    fn read_view_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "VIEW" {
                if let Some(view) = self.read_view_entry()? {
                    let _ = document.views.add(view);
                }
            }
        }
        Ok(())
    }

    /// Read a single VIEW entry
    fn read_view_entry(&mut self) -> Result<Option<View>> {
        let mut view = View::new("*Active");
        let mut center = PointReader::new();
        let mut target = PointReader::new();
        let mut direction = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                2 => view.name = pair.value_string.clone(),
                10 | 20 | 30 => { center.add_coordinate(&pair); }
                11 | 21 | 31 => { target.add_coordinate(&pair); }
                12 | 22 | 32 => { direction.add_coordinate(&pair); }
                40 => {
                    if let Some(height) = pair.as_double() {
                        view.height = height;
                    }
                }
                41 => {
                    if let Some(width) = pair.as_double() {
                        view.width = width;
                    }
                }
                _ => {}
            }
        }

        if let Some(pt) = center.get_point() {
            view.center = pt;
        }
        if let Some(pt) = target.get_point() {
            view.target = pt;
        }
        if let Some(pt) = direction.get_point() {
            view.direction = pt;
        }

        Ok(Some(view))
    }

    /// Read VPORT table
    fn read_vport_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "VPORT" {
                if let Some(vport) = self.read_vport_entry()? {
                    let _ = document.vports.add(vport);
                }
            }
        }
        Ok(())
    }

    /// Read a single VPORT entry
    fn read_vport_entry(&mut self) -> Result<Option<VPort>> {
        let mut vport = VPort::new("*Active");

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            if pair.code == 2 {
                vport.name = pair.value_string.clone();
            }
        }

        Ok(Some(vport))
    }

    /// Read UCS table
    fn read_ucs_table(&mut self, document: &mut CadDocument) -> Result<()> {
        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 && pair.value_string == "ENDTAB" {
                break;
            }

            if pair.code == 0 && pair.value_string == "UCS" {
                if let Some(ucs) = self.read_ucs_entry()? {
                    let _ = document.ucss.add(ucs);
                }
            }
        }
        Ok(())
    }

    /// Read a single UCS entry
    fn read_ucs_entry(&mut self) -> Result<Option<Ucs>> {
        let mut ucs = Ucs::new("World");
        let mut origin = PointReader::new();
        let mut x_axis = PointReader::new();
        let mut y_axis = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                2 => ucs.name = pair.value_string.clone(),
                10 | 20 | 30 => { origin.add_coordinate(&pair); }
                11 | 21 | 31 => { x_axis.add_coordinate(&pair); }
                12 | 22 | 32 => { y_axis.add_coordinate(&pair); }
                _ => {}
            }
        }

        if let Some(pt) = origin.get_point() {
            ucs.origin = pt;
        }
        if let Some(pt) = x_axis.get_point() {
            ucs.x_axis = pt;
        }
        if let Some(pt) = y_axis.get_point() {
            ucs.y_axis = pt;
        }

        Ok(Some(ucs))
    }

    // ===== Entity Readers =====

    /// Read a POINT entity
    fn read_point(&mut self) -> Result<Option<Point>> {
        let mut point = Point::new();
        let mut location = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => point.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        point.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        point.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { location.add_coordinate(&pair); }
                39 => {
                    if let Some(thickness) = pair.as_double() {
                        point.thickness = thickness;
                    }
                }
                _ => {}
            }
        }

        if let Some(pt) = location.get_point() {
            point.location = pt;
        }

        Ok(Some(point))
    }

    /// Read a LINE entity
    fn read_line(&mut self) -> Result<Option<Line>> {
        let mut line = Line::new();
        let mut start = PointReader::new();
        let mut end = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                // Push back the code 0 pair so it can be read by the caller
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => line.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        line.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        line.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { start.add_coordinate(&pair); }
                11 | 21 | 31 => { end.add_coordinate(&pair); }
                39 => {
                    if let Some(thickness) = pair.as_double() {
                        line.thickness = thickness;
                    }
                }
                // Extended data - read and store
                1001 => {
                    // Push back the pair and read XDATA
                    self.reader.push_back(pair);
                    let (extended_data, _next_pair) = self.read_extended_data()?;
                    line.common.extended_data = extended_data;
                }
                _ => {}
            }
        }

        if let Some(pt) = start.get_point() {
            line.start = pt;
        }
        if let Some(pt) = end.get_point() {
            line.end = pt;
        }

        Ok(Some(line))
    }

    /// Read a CIRCLE entity
    fn read_circle(&mut self) -> Result<Option<Circle>> {
        let mut circle = Circle::new();
        let mut center = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => circle.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        circle.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        circle.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { center.add_coordinate(&pair); }
                40 => {
                    if let Some(radius) = pair.as_double() {
                        circle.radius = radius;
                    }
                }
                39 => {
                    if let Some(thickness) = pair.as_double() {
                        circle.thickness = thickness;
                    }
                }
                _ => {}
            }
        }

        if let Some(pt) = center.get_point() {
            circle.center = pt;
        }

        Ok(Some(circle))
    }

    /// Read an ARC entity
    fn read_arc(&mut self) -> Result<Option<Arc>> {
        let mut arc = Arc::new();
        let mut center = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => arc.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        arc.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        arc.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { center.add_coordinate(&pair); }
                40 => {
                    if let Some(radius) = pair.as_double() {
                        arc.radius = radius;
                    }
                }
                50 => {
                    if let Some(angle) = pair.as_double() {
                        arc.start_angle = angle;
                    }
                }
                51 => {
                    if let Some(angle) = pair.as_double() {
                        arc.end_angle = angle;
                    }
                }
                39 => {
                    if let Some(thickness) = pair.as_double() {
                        arc.thickness = thickness;
                    }
                }
                _ => {}
            }
        }

        if let Some(pt) = center.get_point() {
            arc.center = pt;
        }

        Ok(Some(arc))
    }

    /// Read an ELLIPSE entity
    fn read_ellipse(&mut self) -> Result<Option<Ellipse>> {
        let mut ellipse = Ellipse::new();
        let mut center = PointReader::new();
        let mut major_axis = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => ellipse.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        ellipse.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        ellipse.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { center.add_coordinate(&pair); }
                11 | 21 | 31 => { major_axis.add_coordinate(&pair); }
                40 => {
                    if let Some(ratio) = pair.as_double() {
                        ellipse.minor_axis_ratio = ratio;
                    }
                }
                41 => {
                    if let Some(angle) = pair.as_double() {
                        ellipse.start_parameter = angle;
                    }
                }
                42 => {
                    if let Some(angle) = pair.as_double() {
                        ellipse.end_parameter = angle;
                    }
                }
                _ => {}
            }
        }

        if let Some(pt) = center.get_point() {
            ellipse.center = pt;
        }
        if let Some(pt) = major_axis.get_point() {
            ellipse.major_axis = pt;
        }

        Ok(Some(ellipse))
    }

    /// Read a POLYLINE entity
    fn read_polyline(&mut self) -> Result<Option<Polyline>> {
        use crate::entities::polyline::Vertex3D;

        let mut polyline = Polyline::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                // Check if it's a VERTEX or SEQEND
                if pair.value_string == "VERTEX" {
                    // Read vertex
                    let mut vertex_reader = PointReader::new();

                    while let Some(vpair) = self.reader.read_pair()? {
                        if vpair.code == 0 {
                            self.reader.push_back(vpair);
                            break;
                        }
                        match vpair.code {
                            10 | 20 | 30 => { vertex_reader.add_coordinate(&vpair); }
                            _ => {}
                        }
                    }

                    if let Some(pt) = vertex_reader.get_point() {
                        polyline.vertices.push(Vertex3D::new(pt));
                    }
                } else if pair.value_string == "SEQEND" {
                    // End of polyline - skip SEQEND properties
                    while let Some(seqend_pair) = self.reader.read_pair()? {
                        if seqend_pair.code == 0 {
                            self.reader.push_back(seqend_pair);
                            break;
                        }
                    }
                    break;
                } else {
                    // End of polyline, different entity - push back
                    self.reader.push_back(pair);
                    break;
                }
            } else {
                match pair.code {
                    8 => polyline.common.layer = pair.value_string.clone(),
                    62 => {
                        if let Some(color_index) = pair.as_i16() {
                            polyline.common.color = Color::from_index(color_index);
                        }
                    }
                    370 => {
                        if let Some(lw) = pair.as_i16() {
                            polyline.common.line_weight = LineWeight::from_value(lw);
                        }
                    }
                    70 => {
                        if let Some(flags) = pair.as_i16() {
                            if (flags & 1) != 0 {
                                polyline.close();
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(Some(polyline))
    }

    /// Read an LWPOLYLINE entity
    fn read_lwpolyline(&mut self) -> Result<Option<LwPolyline>> {
        use crate::entities::lwpolyline::LwVertex;
        use crate::types::Vector2;

        let mut lwpolyline = LwPolyline::new();
        let mut vertices_x: Vec<f64> = Vec::new();
        let mut vertices_y: Vec<f64> = Vec::new();
        let mut bulges: Vec<f64> = Vec::new();
        let mut widths_start: Vec<f64> = Vec::new();
        let mut widths_end: Vec<f64> = Vec::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => lwpolyline.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        lwpolyline.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        lwpolyline.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                70 => {
                    if let Some(flags) = pair.as_i16() {
                        lwpolyline.is_closed = (flags & 1) != 0;
                    }
                }
                38 => {
                    if let Some(elevation) = pair.as_double() {
                        lwpolyline.elevation = elevation;
                    }
                }
                10 => {
                    if let Some(x) = pair.as_double() {
                        vertices_x.push(x);
                    }
                }
                20 => {
                    if let Some(y) = pair.as_double() {
                        vertices_y.push(y);
                    }
                }
                42 => {
                    if let Some(bulge) = pair.as_double() {
                        bulges.push(bulge);
                    }
                }
                40 => {
                    if let Some(width) = pair.as_double() {
                        widths_start.push(width);
                    }
                }
                41 => {
                    if let Some(width) = pair.as_double() {
                        widths_end.push(width);
                    }
                }
                _ => {}
            }
        }

        // Build vertices from collected data
        for i in 0..vertices_x.len().min(vertices_y.len()) {
            let bulge = bulges.get(i).copied().unwrap_or(0.0);
            let start_width = widths_start.get(i).copied().unwrap_or(0.0);
            let end_width = widths_end.get(i).copied().unwrap_or(0.0);

            lwpolyline.vertices.push(LwVertex {
                location: Vector2::new(vertices_x[i], vertices_y[i]),
                bulge,
                start_width,
                end_width,
            });
        }

        Ok(Some(lwpolyline))
    }

    /// Read a TEXT entity
    fn read_text(&mut self) -> Result<Option<Text>> {
        let mut text = Text::new();
        let mut insertion = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => text.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        text.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        text.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { insertion.add_coordinate(&pair); }
                1 => text.value = pair.value_string.clone(),
                40 => {
                    if let Some(height) = pair.as_double() {
                        text.height = height;
                    }
                }
                50 => {
                    if let Some(rotation) = pair.as_double() {
                        text.rotation = rotation;
                    }
                }
                41 => {
                    if let Some(width_factor) = pair.as_double() {
                        text.width_factor = width_factor;
                    }
                }
                51 => {
                    if let Some(oblique) = pair.as_double() {
                        text.oblique_angle = oblique;
                    }
                }
                7 => text.style = pair.value_string.clone(),
                _ => {}
            }
        }

        if let Some(pt) = insertion.get_point() {
            text.insertion_point = pt;
        }

        Ok(Some(text))
    }

    /// Read an MTEXT entity
    fn read_mtext(&mut self) -> Result<Option<MText>> {
        let mut mtext = MText::new();
        let mut insertion = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => mtext.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        mtext.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        mtext.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { insertion.add_coordinate(&pair); }
                1 | 3 => {
                    // Text content (can be split across multiple codes)
                    mtext.value.push_str(&pair.value_string);
                }
                40 => {
                    if let Some(height) = pair.as_double() {
                        mtext.height = height;
                    }
                }
                41 => {
                    if let Some(width) = pair.as_double() {
                        mtext.rectangle_width = width;
                    }
                }
                50 => {
                    if let Some(rotation) = pair.as_double() {
                        mtext.rotation = rotation;
                    }
                }
                7 => mtext.style = pair.value_string.clone(),
                _ => {}
            }
        }

        if let Some(pt) = insertion.get_point() {
            mtext.insertion_point = pt;
        }

        Ok(Some(mtext))
    }

    /// Read a SPLINE entity
    fn read_spline(&mut self) -> Result<Option<Spline>> {
        let mut spline = Spline::new();
        let mut current_control_point = PointReader::new();
        let mut current_fit_point = PointReader::new();
        let mut reading_control = false;
        let mut reading_fit = false;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => spline.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        spline.common.color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        spline.common.line_weight = LineWeight::from_value(lw);
                    }
                }
                70 => {
                    if let Some(flags_val) = pair.as_i16() {
                        spline.flags.closed = (flags_val & 1) != 0;
                        spline.flags.periodic = (flags_val & 2) != 0;
                        spline.flags.rational = (flags_val & 4) != 0;
                    }
                }
                71 => {
                    if let Some(degree) = pair.as_i16() {
                        spline.degree = degree as i32;
                    }
                }
                40 => {
                    if let Some(knot) = pair.as_double() {
                        spline.knots.push(knot);
                    }
                }
                10 | 20 | 30 => {
                    // Control point coordinates
                    if pair.code == 10 {
                        // Save previous control point if complete
                        if reading_control {
                            if let Some(pt) = current_control_point.get_point() {
                                spline.control_points.push(pt);
                            }
                        }
                        current_control_point = PointReader::new();
                        reading_control = true;
                    }
                    current_control_point.add_coordinate(&pair);
                }
                11 | 21 | 31 => {
                    // Fit point coordinates
                    if pair.code == 11 {
                        // Save previous fit point if complete
                        if reading_fit {
                            if let Some(pt) = current_fit_point.get_point() {
                                spline.fit_points.push(pt);
                            }
                        }
                        current_fit_point = PointReader::new();
                        reading_fit = true;
                    }
                    current_fit_point.add_coordinate(&pair);
                }
                _ => {}
            }
        }

        // Save last control point if any
        if reading_control {
            if let Some(pt) = current_control_point.get_point() {
                spline.control_points.push(pt);
            }
        }

        // Save last fit point if any
        if reading_fit {
            if let Some(pt) = current_fit_point.get_point() {
                spline.fit_points.push(pt);
            }
        }

        Ok(Some(spline))
    }

    /// Read a DIMENSION entity
    fn read_dimension(&mut self) -> Result<Option<Dimension>> {
        use crate::entities::dimension::*;

        let mut dim_type = DimensionType::Linear;
        let mut definition_point = PointReader::new();
        let mut text_middle_point = PointReader::new();
        let mut insertion_point = PointReader::new();
        let mut first_point = PointReader::new();
        let mut second_point = PointReader::new();
        let mut third_point = PointReader::new();
        let mut fourth_point = PointReader::new();
        let mut text = String::new();
        let mut style_name = String::from("Standard");
        let mut layer = String::from("0");
        let mut color = Color::ByLayer;
        let mut line_weight = LineWeight::ByLayer;
        let mut rotation = 0.0;
        let mut actual_measurement = 0.0;
        let mut leader_length = 0.0;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        line_weight = LineWeight::from_value(lw);
                    }
                }
                70 => {
                    if let Some(type_val) = pair.as_i16() {
                        dim_type = match type_val & 0x0F {
                            0 => DimensionType::Linear,
                            1 => DimensionType::Aligned,
                            2 => DimensionType::Angular,
                            3 => DimensionType::Diameter,
                            4 => DimensionType::Radius,
                            5 => DimensionType::Angular3Point,
                            6 => DimensionType::Ordinate,
                            _ => DimensionType::Linear,
                        };
                    }
                }
                1 => text = pair.value_string.clone(),
                3 => style_name = pair.value_string.clone(),
                10 | 20 | 30 => { definition_point.add_coordinate(&pair); }
                11 | 21 | 31 => { text_middle_point.add_coordinate(&pair); }
                12 | 22 | 32 => { insertion_point.add_coordinate(&pair); }
                13 | 23 | 33 => { first_point.add_coordinate(&pair); }
                14 | 24 | 34 => { second_point.add_coordinate(&pair); }
                15 | 25 | 35 => { third_point.add_coordinate(&pair); }
                16 | 26 | 36 => { fourth_point.add_coordinate(&pair); }
                50 => {
                    if let Some(rot) = pair.as_double() {
                        rotation = rot;
                    }
                }
                42 => {
                    if let Some(measurement) = pair.as_double() {
                        actual_measurement = measurement;
                    }
                }
                40 => {
                    if let Some(length) = pair.as_double() {
                        leader_length = length;
                    }
                }
                _ => {}
            }
        }

        // Build the appropriate dimension type
        let pt1 = first_point.get_point().unwrap_or(Vector3::zero());
        let pt2 = second_point.get_point().unwrap_or(Vector3::zero());
        let pt3 = third_point.get_point().unwrap_or(Vector3::zero());
        let _pt4 = fourth_point.get_point().unwrap_or(Vector3::zero());

        let dimension = match dim_type {
            DimensionType::Aligned => {
                let mut dim = DimensionAligned::new(pt1, pt2);
                dim.base.common.layer = layer;
                dim.base.common.color = color;
                dim.base.common.line_weight = line_weight;
                dim.base.text = text;
                dim.base.style_name = style_name;
                dim.base.actual_measurement = actual_measurement;
                if let Some(def_pt) = definition_point.get_point() {
                    dim.definition_point = def_pt;
                }
                Dimension::Aligned(dim)
            }
            DimensionType::Linear => {
                let mut dim = DimensionLinear::rotated(pt1, pt2, rotation);
                dim.base.common.layer = layer;
                dim.base.common.color = color;
                dim.base.common.line_weight = line_weight;
                dim.base.text = text;
                dim.base.style_name = style_name;
                dim.base.actual_measurement = actual_measurement;
                if let Some(def_pt) = definition_point.get_point() {
                    dim.definition_point = def_pt;
                }
                Dimension::Linear(dim)
            }
            DimensionType::Radius => {
                let center = pt1;
                let chord_point = pt2;
                let mut dim = DimensionRadius::new(center, chord_point);
                dim.base.common.layer = layer;
                dim.base.common.color = color;
                dim.base.common.line_weight = line_weight;
                dim.base.text = text;
                dim.base.style_name = style_name;
                dim.base.actual_measurement = actual_measurement;
                dim.leader_length = leader_length;
                Dimension::Radius(dim)
            }
            DimensionType::Diameter => {
                let center = pt1;
                let point_on_arc = pt2;
                let mut dim = DimensionDiameter::new(center, point_on_arc);
                dim.base.common.layer = layer;
                dim.base.common.color = color;
                dim.base.common.line_weight = line_weight;
                dim.base.text = text;
                dim.base.style_name = style_name;
                dim.base.actual_measurement = actual_measurement;
                Dimension::Diameter(dim)
            }
            DimensionType::Angular => {
                // Angular2Ln: vertex, first_point, second_point
                let mut dim = DimensionAngular2Ln::new(pt1, pt2, pt3);
                dim.base.common.layer = layer;
                dim.base.common.color = color;
                dim.base.common.line_weight = line_weight;
                dim.base.text = text;
                dim.base.style_name = style_name;
                dim.base.actual_measurement = actual_measurement;
                Dimension::Angular2Ln(dim)
            }
            DimensionType::Angular3Point => {
                // Angular3Pt: center, first_point, second_point
                let mut dim = DimensionAngular3Pt::new(pt1, pt2, pt3);
                dim.base.common.layer = layer;
                dim.base.common.color = color;
                dim.base.common.line_weight = line_weight;
                dim.base.text = text;
                dim.base.style_name = style_name;
                dim.base.actual_measurement = actual_measurement;
                Dimension::Angular3Pt(dim)
            }
            DimensionType::Ordinate => {
                // Ordinate: feature_location, leader_endpoint
                // Use x_ordinate by default (could be determined from flags in real implementation)
                let mut dim = DimensionOrdinate::x_ordinate(pt1, pt2);
                dim.base.common.layer = layer;
                dim.base.common.color = color;
                dim.base.common.line_weight = line_weight;
                dim.base.text = text;
                dim.base.style_name = style_name;
                dim.base.actual_measurement = actual_measurement;
                Dimension::Ordinate(dim)
            }
        };

        Ok(Some(dimension))
    }

    /// Read a HATCH entity
    fn read_hatch(&mut self) -> Result<Option<Hatch>> {
        use crate::entities::hatch::*;

        let mut hatch = Hatch::new();
        let mut pattern_name = String::from("SOLID");
        let mut pattern_type = HatchPatternType::Predefined;
        let mut layer = String::from("0");
        let mut color = Color::ByLayer;
        let mut line_weight = LineWeight::ByLayer;
        let mut _num_boundary_paths = 0;
        let mut current_path_edges: Vec<BoundaryEdge> = Vec::new();
        let mut reading_boundary = false;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        line_weight = LineWeight::from_value(lw);
                    }
                }
                2 => pattern_name = pair.value_string.clone(),
                70 => {
                    if let Some(solid_fill) = pair.as_i16() {
                        hatch.is_solid = solid_fill != 0;
                    }
                }
                71 => {
                    if let Some(associative) = pair.as_i16() {
                        hatch.is_associative = associative != 0;
                    }
                }
                75 => {
                    if let Some(style) = pair.as_i16() {
                        hatch.style = match style {
                            0 => HatchStyleType::Normal,
                            1 => HatchStyleType::Outer,
                            2 => HatchStyleType::Ignore,
                            _ => HatchStyleType::Normal,
                        };
                    }
                }
                76 => {
                    if let Some(ptype) = pair.as_i16() {
                        pattern_type = match ptype {
                            0 => HatchPatternType::UserDefined,
                            1 => HatchPatternType::Predefined,
                            2 => HatchPatternType::Custom,
                            _ => HatchPatternType::Predefined,
                        };
                    }
                }
                91 => {
                    if let Some(num_paths) = pair.as_i32() {
                        _num_boundary_paths = num_paths;
                    }
                }
                92 => {
                    // Boundary path type flags - indicates start of a new boundary path
                    if reading_boundary && !current_path_edges.is_empty() {
                        // Save previous path
                        let path = BoundaryPath {
                            flags: BoundaryPathFlags::new(),
                            edges: current_path_edges.clone(),
                            boundary_handles: Vec::new(),
                        };
                        hatch.paths.push(path);
                        current_path_edges.clear();
                    }
                    reading_boundary = true;
                }
                72 => {
                    // Edge type - indicates start of a new edge
                    if let Some(edge_type) = pair.as_i16() {
                        match edge_type {
                            1 => {
                                // Line edge - will be populated by subsequent codes
                                current_path_edges.push(BoundaryEdge::Line(LineEdge {
                                    start: Vector2::new(0.0, 0.0),
                                    end: Vector2::new(0.0, 0.0),
                                }));
                            }
                            2 => {
                                // Circular arc edge
                                current_path_edges.push(BoundaryEdge::CircularArc(CircularArcEdge {
                                    center: Vector2::new(0.0, 0.0),
                                    radius: 0.0,
                                    start_angle: 0.0,
                                    end_angle: 0.0,
                                    counter_clockwise: true,
                                }));
                            }
                            3 => {
                                // Elliptic arc edge
                                current_path_edges.push(BoundaryEdge::EllipticArc(EllipticArcEdge {
                                    center: Vector2::new(0.0, 0.0),
                                    major_axis_endpoint: Vector2::new(1.0, 0.0),
                                    minor_axis_ratio: 1.0,
                                    start_angle: 0.0,
                                    end_angle: 0.0,
                                    counter_clockwise: true,
                                }));
                            }
                            4 => {
                                // Spline edge
                                current_path_edges.push(BoundaryEdge::Spline(SplineEdge {
                                    degree: 3,
                                    rational: false,
                                    periodic: false,
                                    knots: Vec::new(),
                                    control_points: Vec::new(),
                                    fit_points: Vec::new(),
                                    start_tangent: Vector2::new(0.0, 0.0),
                                    end_tangent: Vector2::new(0.0, 0.0),
                                }));
                            }
                            _ => {}
                        }
                    }
                }
                // Note: Full hatch reading would require reading all edge data (codes 10-40, etc.)
                // For now, we create a basic hatch structure
                _ => {}
            }
        }

        // Save last boundary path if any
        if reading_boundary && !current_path_edges.is_empty() {
            let path = BoundaryPath {
                flags: BoundaryPathFlags::new(),
                edges: current_path_edges,
                boundary_handles: Vec::new(),
            };
            hatch.paths.push(path);
        }

        hatch.common.layer = layer;
        hatch.common.color = color;
        hatch.common.line_weight = line_weight;
        hatch.pattern.name = pattern_name;
        hatch.pattern_type = pattern_type;

        Ok(Some(hatch))
    }

    /// Read a SOLID entity
    fn read_solid(&mut self) -> Result<Option<Solid>> {
        let mut corner1 = PointReader::new();
        let mut corner2 = PointReader::new();
        let mut corner3 = PointReader::new();
        let mut corner4 = PointReader::new();
        let mut layer = String::from("0");
        let mut color = Color::ByLayer;
        let mut line_weight = LineWeight::ByLayer;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { corner1.add_coordinate(&pair); }
                11 | 21 | 31 => { corner2.add_coordinate(&pair); }
                12 | 22 | 32 => { corner3.add_coordinate(&pair); }
                13 | 23 | 33 => { corner4.add_coordinate(&pair); }
                _ => {}
            }
        }

        let pt1 = corner1.get_point().unwrap_or(Vector3::zero());
        let pt2 = corner2.get_point().unwrap_or(Vector3::zero());
        let pt3 = corner3.get_point().unwrap_or(Vector3::zero());
        let pt4 = corner4.get_point().unwrap_or(pt3);

        let mut solid = Solid::new(pt1, pt2, pt3, pt4);
        solid.common.layer = layer;
        solid.common.color = color;
        solid.common.line_weight = line_weight;

        Ok(Some(solid))
    }

    /// Read a 3DFACE entity
    fn read_face3d(&mut self) -> Result<Option<Face3D>> {
        let mut corner1 = PointReader::new();
        let mut corner2 = PointReader::new();
        let mut corner3 = PointReader::new();
        let mut corner4 = PointReader::new();
        let mut layer = String::from("0");
        let mut color = Color::ByLayer;
        let mut line_weight = LineWeight::ByLayer;
        let mut invisible_flags = 0i16;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        line_weight = LineWeight::from_value(lw);
                    }
                }
                10 | 20 | 30 => { corner1.add_coordinate(&pair); }
                11 | 21 | 31 => { corner2.add_coordinate(&pair); }
                12 | 22 | 32 => { corner3.add_coordinate(&pair); }
                13 | 23 | 33 => { corner4.add_coordinate(&pair); }
                70 => {
                    if let Some(flags) = pair.as_i16() {
                        invisible_flags = flags;
                    }
                }
                _ => {}
            }
        }

        let pt1 = corner1.get_point().unwrap_or(Vector3::zero());
        let pt2 = corner2.get_point().unwrap_or(Vector3::zero());
        let pt3 = corner3.get_point().unwrap_or(Vector3::zero());
        let pt4 = corner4.get_point().unwrap_or(pt3);

        use crate::entities::face3d::InvisibleEdgeFlags;
        let mut invisible_edges = InvisibleEdgeFlags::new();
        invisible_edges.set_first_invisible((invisible_flags & 1) != 0);
        invisible_edges.set_second_invisible((invisible_flags & 2) != 0);
        invisible_edges.set_third_invisible((invisible_flags & 4) != 0);
        invisible_edges.set_fourth_invisible((invisible_flags & 8) != 0);

        let mut face = Face3D::new(pt1, pt2, pt3, pt4);
        face.common.layer = layer;
        face.common.color = color;
        face.common.line_weight = line_weight;
        face.invisible_edges = invisible_edges;

        Ok(Some(face))
    }

    /// Read an INSERT entity
    fn read_insert(&mut self) -> Result<Option<Insert>> {
        let mut block_name = String::new();
        let mut insertion = PointReader::new();
        let mut x_scale = 1.0;
        let mut y_scale = 1.0;
        let mut z_scale = 1.0;
        let mut rotation = 0.0;
        let mut column_count = 1u16;
        let mut row_count = 1u16;
        let mut column_spacing = 0.0;
        let mut row_spacing = 0.0;
        let mut layer = String::from("0");
        let mut color = Color::ByLayer;
        let mut line_weight = LineWeight::ByLayer;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        color = Color::from_index(color_index);
                    }
                }
                370 => {
                    if let Some(lw) = pair.as_i16() {
                        line_weight = LineWeight::from_value(lw);
                    }
                }
                2 => block_name = pair.value_string.clone(),
                10 | 20 | 30 => { insertion.add_coordinate(&pair); }
                41 => {
                    if let Some(sx) = pair.as_double() {
                        x_scale = sx;
                    }
                }
                42 => {
                    if let Some(sy) = pair.as_double() {
                        y_scale = sy;
                    }
                }
                43 => {
                    if let Some(sz) = pair.as_double() {
                        z_scale = sz;
                    }
                }
                50 => {
                    if let Some(rot) = pair.as_double() {
                        rotation = rot;
                    }
                }
                70 => {
                    if let Some(col_count) = pair.as_i16() {
                        column_count = col_count.max(1) as u16;
                    }
                }
                71 => {
                    if let Some(r_count) = pair.as_i16() {
                        row_count = r_count.max(1) as u16;
                    }
                }
                44 => {
                    if let Some(col_spacing_val) = pair.as_double() {
                        column_spacing = col_spacing_val;
                    }
                }
                45 => {
                    if let Some(row_spacing_val) = pair.as_double() {
                        row_spacing = row_spacing_val;
                    }
                }
                _ => {}
            }
        }

        let insert_point = insertion.get_point().unwrap_or(Vector3::zero());
        let mut insert = Insert::new(block_name, insert_point);
        insert.common.layer = layer;
        insert.common.color = color;
        insert.common.line_weight = line_weight;
        insert.x_scale = x_scale;
        insert.y_scale = y_scale;
        insert.z_scale = z_scale;
        insert.rotation = rotation;
        insert.column_count = column_count;
        insert.row_count = row_count;
        insert.column_spacing = column_spacing;
        insert.row_spacing = row_spacing;

        Ok(Some(insert))
    }

    /// Read a RAY entity
    fn read_ray(&mut self) -> Result<Option<Ray>> {
        let mut base_point = PointReader::new();
        let mut direction = PointReader::new();
        let mut layer = String::from("0");
        let mut color = Color::ByLayer;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        color = Color::from_index(color_index);
                    }
                }
                10 | 20 | 30 => { base_point.add_coordinate(&pair); }
                11 | 21 | 31 => { direction.add_coordinate(&pair); }
                _ => {}
            }
        }

        let bp = base_point.get_point().unwrap_or(Vector3::zero());
        let dir = direction.get_point().unwrap_or(Vector3::new(1.0, 0.0, 0.0));
        let mut ray = Ray::new(bp, dir);
        ray.common.layer = layer;
        ray.common.color = color;

        Ok(Some(ray))
    }

    /// Read an XLINE entity
    fn read_xline(&mut self) -> Result<Option<XLine>> {
        let mut base_point = PointReader::new();
        let mut direction = PointReader::new();
        let mut layer = String::from("0");
        let mut color = Color::ByLayer;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        color = Color::from_index(color_index);
                    }
                }
                10 | 20 | 30 => { base_point.add_coordinate(&pair); }
                11 | 21 | 31 => { direction.add_coordinate(&pair); }
                _ => {}
            }
        }

        let bp = base_point.get_point().unwrap_or(Vector3::zero());
        let dir = direction.get_point().unwrap_or(Vector3::new(1.0, 0.0, 0.0));
        let mut xline = XLine::new(bp, dir);
        xline.common.layer = layer;
        xline.common.color = color;

        Ok(Some(xline))
    }

    /// Read an ATTDEF entity
    fn read_attdef(&mut self) -> Result<Option<AttributeDefinition>> {
        let mut tag = String::new();
        let mut prompt = String::new();
        let mut default_value = String::new();
        let mut insertion_point = PointReader::new();
        let mut height = 0.0;
        let mut rotation = 0.0;
        let mut layer = String::from("0");
        let mut color = Color::ByLayer;

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        color = Color::from_index(color_index);
                    }
                }
                1 => default_value = pair.value_string.clone(),
                2 => tag = pair.value_string.clone(),
                3 => prompt = pair.value_string.clone(),
                10 | 20 | 30 => { insertion_point.add_coordinate(&pair); }
                40 => {
                    if let Some(h) = pair.as_double() {
                        height = h;
                    }
                }
                50 => {
                    if let Some(r) = pair.as_double() {
                        rotation = r;
                    }
                }
                _ => {}
            }
        }

        let mut attdef = AttributeDefinition::new(tag, prompt, default_value);
        attdef.insertion_point = insertion_point.get_point().unwrap_or(Vector3::zero());
        attdef.height = height;
        attdef.rotation = rotation;
        attdef.common.layer = layer;
        attdef.common.color = color;

        Ok(Some(attdef))
    }

    /// Read a TOLERANCE entity
    fn read_tolerance(&mut self) -> Result<Option<Tolerance>> {
        let mut tolerance = Tolerance::new();
        let mut insertion_point = PointReader::new();
        let mut direction = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => tolerance.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        tolerance.common.color = Color::from_index(color_index);
                    }
                }
                1 => tolerance.text = pair.value_string.clone(),
                3 => tolerance.dimension_style_name = pair.value_string.clone(),
                10 | 20 | 30 => { insertion_point.add_coordinate(&pair); }
                11 | 21 | 31 => { direction.add_coordinate(&pair); }
                _ => {}
            }
        }

        tolerance.insertion_point = insertion_point.get_point().unwrap_or(Vector3::zero());
        tolerance.direction = direction.get_point().unwrap_or(Vector3::new(1.0, 0.0, 0.0));

        Ok(Some(tolerance))
    }

    /// Read a SHAPE entity
    fn read_shape(&mut self) -> Result<Option<Shape>> {
        let mut shape = Shape::new();
        let mut insertion_point = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => shape.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        shape.common.color = Color::from_index(color_index);
                    }
                }
                2 => shape.shape_name = pair.value_string.clone(),
                10 | 20 | 30 => { insertion_point.add_coordinate(&pair); }
                40 => {
                    if let Some(s) = pair.as_double() {
                        shape.size = s;
                    }
                }
                50 => {
                    if let Some(r) = pair.as_double() {
                        shape.rotation = r;
                    }
                }
                _ => {}
            }
        }

        shape.insertion_point = insertion_point.get_point().unwrap_or(Vector3::zero());

        Ok(Some(shape))
    }

    /// Read a WIPEOUT entity
    fn read_wipeout(&mut self) -> Result<Option<Wipeout>> {
        let mut wipeout = Wipeout::new();
        let mut insertion_point = PointReader::new();
        let mut u_vector = PointReader::new();
        let mut v_vector = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            if pair.code == 0 {
                self.reader.push_back(pair);
                break;
            }

            match pair.code {
                8 => wipeout.common.layer = pair.value_string.clone(),
                62 => {
                    if let Some(color_index) = pair.as_i16() {
                        wipeout.common.color = Color::from_index(color_index);
                    }
                }
                10 | 20 | 30 => { insertion_point.add_coordinate(&pair); }
                11 | 21 | 31 => { u_vector.add_coordinate(&pair); }
                12 | 22 | 32 => { v_vector.add_coordinate(&pair); }
                14 => {
                    if let Some(x) = pair.as_double() {
                        wipeout.clip_boundary_vertices.push(Vector2::new(x, 0.0));
                    }
                }
                24 => {
                    if let Some(y) = pair.as_double() {
                        if let Some(last) = wipeout.clip_boundary_vertices.last_mut() {
                            last.y = y;
                        }
                    }
                }
                _ => {}
            }
        }

        wipeout.insertion_point = insertion_point.get_point().unwrap_or(Vector3::zero());
        wipeout.u_vector = u_vector.get_point().unwrap_or(Vector3::new(1.0, 0.0, 0.0));
        wipeout.v_vector = v_vector.get_point().unwrap_or(Vector3::new(0.0, 1.0, 0.0));

        Ok(Some(wipeout))
    }

    /// Read extended data (XDATA) from the current position
    /// Returns the extended data and the last pair read (which is not part of XDATA)
    fn read_extended_data(&mut self) -> Result<(ExtendedData, Option<super::stream_reader::DxfCodePair>)> {
        let mut xdata = ExtendedData::new();
        let mut current_record: Option<ExtendedDataRecord> = None;
        let mut point_reader = PointReader::new();

        while let Some(pair) = self.reader.read_pair()? {
            match pair.code {
                // Application name - start of new record
                1001 => {
                    // Save previous record if exists
                    if let Some(record) = current_record.take() {
                        xdata.add_record(record);
                    }
                    // Start new record
                    current_record = Some(ExtendedDataRecord::new(pair.value_string.clone()));
                }
                // String value
                1000 => {
                    if let Some(ref mut record) = current_record {
                        record.add_value(XDataValue::String(pair.value_string.clone()));
                    }
                }
                // Control string
                1002 => {
                    if let Some(ref mut record) = current_record {
                        record.add_value(XDataValue::ControlString(pair.value_string.clone()));
                    }
                }
                // Layer name
                1003 => {
                    if let Some(ref mut record) = current_record {
                        record.add_value(XDataValue::LayerName(pair.value_string.clone()));
                    }
                }
                // Binary data
                1004 => {
                    if let Some(ref mut record) = current_record {
                        // Parse hex string to bytes
                        let bytes: Vec<u8> = (0..pair.value_string.len())
                            .step_by(2)
                            .filter_map(|i| {
                                let end = (i + 2).min(pair.value_string.len());
                                u8::from_str_radix(&pair.value_string[i..end], 16).ok()
                            })
                            .collect();
                        record.add_value(XDataValue::BinaryData(bytes));
                    }
                }
                // Database handle
                1005 => {
                    if let Some(ref mut record) = current_record {
                        if let Ok(h) = u64::from_str_radix(&pair.value_string, 16) {
                            record.add_value(XDataValue::Handle(Handle::new(h)));
                        }
                    }
                }
                // 3D point (1010, 1020, 1030)
                1010 | 1020 | 1030 => {
                    if let Some(ref mut record) = current_record {
                        point_reader.add_coordinate(&pair);
                        if let Some(pt) = point_reader.get_point() {
                            record.add_value(XDataValue::Point3D(pt));
                        }
                    }
                }
                // 3D position (1011, 1021, 1031)
                1011 | 1021 | 1031 => {
                    if let Some(ref mut record) = current_record {
                        point_reader.add_coordinate(&pair);
                        if let Some(pt) = point_reader.get_point() {
                            record.add_value(XDataValue::Position3D(pt));
                        }
                    }
                }
                // 3D displacement (1012, 1022, 1032)
                1012 | 1022 | 1032 => {
                    if let Some(ref mut record) = current_record {
                        point_reader.add_coordinate(&pair);
                        if let Some(pt) = point_reader.get_point() {
                            record.add_value(XDataValue::Displacement3D(pt));
                        }
                    }
                }
                // 3D direction (1013, 1023, 1033)
                1013 | 1023 | 1033 => {
                    if let Some(ref mut record) = current_record {
                        point_reader.add_coordinate(&pair);
                        if let Some(pt) = point_reader.get_point() {
                            record.add_value(XDataValue::Direction3D(pt));
                        }
                    }
                }
                // Real value
                1040 => {
                    if let Some(ref mut record) = current_record {
                        if let Some(value) = pair.as_double() {
                            record.add_value(XDataValue::Real(value));
                        }
                    }
                }
                // Distance
                1041 => {
                    if let Some(ref mut record) = current_record {
                        if let Some(value) = pair.as_double() {
                            record.add_value(XDataValue::Distance(value));
                        }
                    }
                }
                // Scale factor
                1042 => {
                    if let Some(ref mut record) = current_record {
                        if let Some(value) = pair.as_double() {
                            record.add_value(XDataValue::ScaleFactor(value));
                        }
                    }
                }
                // 16-bit integer
                1070 => {
                    if let Some(ref mut record) = current_record {
                        if let Some(value) = pair.as_i16() {
                            record.add_value(XDataValue::Integer16(value));
                        }
                    }
                }
                // 32-bit integer
                1071 => {
                    if let Some(ref mut record) = current_record {
                        if let Some(value) = pair.as_i32() {
                            record.add_value(XDataValue::Integer32(value));
                        }
                    }
                }
                // Not XDATA - return what we have
                _ => {
                    // Save last record if exists
                    if let Some(record) = current_record.take() {
                        xdata.add_record(record);
                    }
                    return Ok((xdata, Some(pair)));
                }
            }
        }

        // End of file - save last record if exists
        if let Some(record) = current_record.take() {
            xdata.add_record(record);
        }

        Ok((xdata, None))
    }
}


