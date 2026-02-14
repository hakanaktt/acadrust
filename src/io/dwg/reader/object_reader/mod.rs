//! DWG Object Reader — reads all entities, table entries, and objects
//! from the `AcDb:AcDbObjects` section.
//!
//! Mirrors ACadSharp's `DwgObjectReader` (~7200 lines of C#).
//!
//! # Architecture
//!
//! Objects in a DWG file are stored in arbitrary order, referenced through
//! a handle-based object map.  The reader seeds a queue from the header
//! handles, then processes each handle:
//!
//! 1. Look up the file offset in the handle map.
//! 2. Seek to that offset, read object size, set up sub-streams.
//! 3. Read object type, dispatch to the appropriate reader.
//! 4. Newly-discovered handles are enqueued for later processing.
//! 5. The resulting template is stored in the builder for later resolution.

pub mod common;
pub mod read_entities;
pub mod read_objects;
pub mod read_tables;
pub mod templates;

use std::collections::{HashMap, HashSet, VecDeque};

use crate::classes::DxfClass;
use crate::error::Result;
use crate::io::dwg::object_type::DwgObjectType;
use crate::io::dwg::reader::stream_reader::IDwgStreamReader;
use crate::io::dwg::reader::stream_reader_base::get_stream_handler;
use crate::io::dwg::section_io::SectionIO;
use crate::notification::{Notification, NotificationType};
use crate::types::DxfVersion;

use self::templates::CadTemplate;

/// The DWG object reader — reads all objects from the AcDb:AcDbObjects section.
///
/// Corresponds to ACadSharp's `DwgObjectReader` class.
pub struct DwgObjectReader {
    /// DWG version.
    version: DxfVersion,
    /// Version flags (pre-computed).
    sio: SectionIO,
    /// Raw section data (`AcDb:AcDbObjects`).
    data: Vec<u8>,
    /// Object map: handle → byte offset within `data`.
    handle_map: HashMap<u64, i64>,
    /// DXF class map: class_number → class info.
    class_map: HashMap<i16, DxfClass>,

    /// Queue of handles to read.
    handles: VecDeque<u64>,
    /// Handles already read (prevents duplicates / infinite loops).
    read_objects: HashSet<u64>,
    /// Handles for which a template already exists (from builder).
    existing_templates: HashSet<u64>,

    /// All templates produced by this reader.
    pub templates: Vec<CadTemplate>,

    /// Diagnostic / warning notifications.
    pub notifications: Vec<Notification>,

    /// Whether to continue on error instead of aborting.
    pub failsafe: bool,

    // --- Per-object state (set in get_entity_type, used by read methods) ---
    /// Bit position of the start of the current object data (after MS/MC header).
    object_initial_pos: i64,
    /// Size of the current object in bytes (from MS header, excluding CRC).
    object_size: u32,
}

impl DwgObjectReader {
    /// Create a new object reader.
    ///
    /// # Arguments
    /// - `version` — DWG version
    /// - `data` — raw `AcDb:AcDbObjects` section bytes
    /// - `handles` — initial queue of handles (from header handles collection)
    /// - `handle_map` — object map (handle → byte offset)
    /// - `classes` — DXF class collection (as a slice)
    pub fn new(
        version: DxfVersion,
        data: Vec<u8>,
        handles: VecDeque<u64>,
        handle_map: HashMap<u64, i64>,
        classes: &[DxfClass],
    ) -> Self {
        let class_map: HashMap<i16, DxfClass> = classes
            .iter()
            .map(|c| (c.class_number, c.clone()))
            .collect();

        Self {
            sio: SectionIO::new(version),
            version,
            data,
            handle_map,
            class_map,
            handles,
            read_objects: HashSet::new(),
            existing_templates: HashSet::new(),
            templates: Vec::new(),
            notifications: Vec::new(),
            failsafe: true,
            object_initial_pos: 0,
            object_size: 0,
        }
    }

    /// Read all entities, table entries, and objects from the section.
    ///
    /// Processes the handle queue until empty, building templates for each object.
    pub fn read(&mut self) -> Result<()> {
        while let Some(handle) = self.handles.pop_front() {
            // Skip if already read or already has a template.
            if self.read_objects.contains(&handle)
                || self.existing_templates.contains(&handle)
            {
                continue;
            }

            // Look up the file offset.
            let offset = match self.handle_map.get(&handle) {
                Some(&off) => off,
                None => {
                    continue;
                }
            };

            // Get the object type and set up sub-streams.
            let (obj_type, raw_type, streams) = match self.get_entity_type(offset) {
                Ok(v) => v,
                Err(e) => {
                    if !self.failsafe {
                        return Err(e);
                    }
                    self.notify(
                        &format!("Failed to get object type for handle {handle:#X}: {e}"),
                        NotificationType::Error,
                    );
                    continue;
                }
            };

            // Mark as read before dispatching to avoid infinite loops.
            self.read_objects.insert(handle);

            // Read the object.
            let template = match self.read_object(obj_type, raw_type, &mut streams.unwrap_or_default()) {
                Ok(Some(t)) => t,
                Ok(None) => {
                    self.notify(
                        &format!("Object type not implemented: {obj_type:?}"),
                        NotificationType::Warning,
                    );
                    continue;
                }
                Err(e) => {
                    if !self.failsafe {
                        return Err(e);
                    }
                    self.notify(
                        &format!("Failed to read object {obj_type:?} handle {handle:#X}: {e}"),
                        NotificationType::Error,
                    );
                    continue;
                }
            };

            // Enqueue all handle references from the template for further reading.
            self.enqueue_template_handles(&template);
            self.templates.push(template);
        }
        Ok(())
    }

    /// Extract all handle references from a template and enqueue any
    /// that haven't been seen yet.
    fn enqueue_template_handles(&mut self, template: &CadTemplate) {
        let handles = template.all_handles();
        for h in handles {
            if h != 0
                && !self.read_objects.contains(&h)
                && !self.existing_templates.contains(&h)
            {
                self.handles.push_back(h);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Sub-stream setup
    // -----------------------------------------------------------------------

    /// Read the MS/MC header, create sub-stream readers, and return the
    /// object type together with a `StreamSet` for dispatching.
    ///
    /// Also returns the raw i16 type code, which is needed for unlisted
    /// (class-based) types to look up the `DxfClass` in the class map.
    fn get_entity_type(&mut self, offset: i64) -> Result<(DwgObjectType, i16, Option<StreamSet>)> {
        // Create a CRC reader (position-based) for the raw data.
        let crc_reader = self.make_reader();
        let mut crc_reader = crc_reader;
        crc_reader.set_position(offset as u64);

        // MS : Size of object (not including CRC).
        let size = crc_reader.read_modular_short()? as u32;
        self.object_size = size;

        if size == 0 {
            return Ok((DwgObjectType::Invalid, -1, None));
        }

        let size_in_bits = (size as u64) << 3;

        let obj_type;
        let raw_type: i16;
        let streams;

        if self.sio.r2010_plus {
            // R2010+: MC handle stream size (unsigned).
            let handle_size = crc_reader.read_modular_char()? as u64;
            let handle_section_offset =
                crc_reader.position_in_bits() as u64 + size_in_bits - handle_size;

            // Main/object reader — starts at current CRC reader position.
            let mut object_reader = self.make_reader();
            object_reader.set_position_in_bits(crc_reader.position_in_bits());
            self.object_initial_pos = object_reader.position_in_bits();

            let (ot, rt) = object_reader.read_object_type_raw()?;
            obj_type = ot;
            raw_type = rt;

            // Handle sub-reader.
            let mut handles_reader = self.make_reader();
            handles_reader.set_position_in_bits(handle_section_offset as i64);

            // Text sub-reader (positioned by flag before handle section).
            let mut text_reader = self.make_reader();
            text_reader.set_position_by_flag(handle_section_offset as i64 - 1)?;

            streams = Some(StreamSet {
                object_reader,
                text_reader,
                handles_reader,
                merged: true,
                has_separate_text_reader: true,
                current_handle: 0,
            });
        } else {
            // Pre-R2010: no separate handle/text streams yet (set up in updateHandleReader).
            let mut object_reader = self.make_reader();
            object_reader.set_position_in_bits(crc_reader.position_in_bits());
            self.object_initial_pos = object_reader.position_in_bits();

            let (ot, rt) = object_reader.read_object_type_raw()?;
            obj_type = ot;
            raw_type = rt;

            // Handle reader is a clone (positioned later by updateHandleReader).
            let handles_reader = self.make_reader();
            // Text reader is unused for pre-R2007 (text is inline in object data).
            // For R2007, update_handle_reader will set up text_reader properly.

            streams = Some(StreamSet {
                object_reader,
                text_reader: self.make_reader(),
                handles_reader,
                merged: false,
                has_separate_text_reader: false,
                current_handle: 0,
            });
        }

        Ok((obj_type, raw_type, streams))
    }

    /// Create a fresh stream reader over the raw data (clones `self.data`).
    fn make_reader(&self) -> Box<dyn IDwgStreamReader> {
        Box::new(get_stream_handler(self.version, self.data.clone()))
    }

    // -----------------------------------------------------------------------
    // Handle enqueuing
    // -----------------------------------------------------------------------

    /// Read a handle reference from the handles reader and enqueue it for
    /// later processing if it hasn't been encountered yet.
    #[allow(dead_code)]
    fn handle_reference(
        handles_reader: &mut dyn IDwgStreamReader,
        ref_handle: u64,
        handles_queue: &mut VecDeque<u64>,
        read_objects: &HashSet<u64>,
        existing_templates: &HashSet<u64>,
    ) -> Result<u64> {
        let value = handles_reader.handle_reference_resolved(ref_handle)?;
        if value != 0
            && !read_objects.contains(&value)
            && !existing_templates.contains(&value)
        {
            handles_queue.push_back(value);
        }
        Ok(value)
    }

    // -----------------------------------------------------------------------
    // Type dispatch
    // -----------------------------------------------------------------------

    /// Dispatch to the type-specific reader method.
    fn read_object(
        &mut self,
        obj_type: DwgObjectType,
        raw_type: i16,
        streams: &mut StreamSet,
    ) -> Result<Option<CadTemplate>> {
        use DwgObjectType::*;

        let template = match obj_type {
            // Text entities
            Text => Some(self.read_text(streams)?),
            Attrib => Some(self.read_attribute(streams)?),
            Attdef => Some(self.read_attribute_definition(streams)?),

            // Block entities
            Block => Some(self.read_block(streams)?),
            Endblk => Some(self.read_end_block(streams)?),
            Seqend => Some(self.read_seqend(streams)?),

            // Insert
            Insert => Some(self.read_insert(streams)?),
            Minsert => Some(self.read_minsert(streams)?),

            // Vertices
            Vertex2D => Some(self.read_vertex_2d(streams)?),
            Vertex3D => Some(self.read_vertex_3d(streams)?),
            VertexPface => Some(self.read_vertex_3d(streams)?),
            VertexMesh => Some(self.read_vertex_3d(streams)?),
            VertexPfaceFace => Some(self.read_pface_vertex(streams)?),

            // Polylines
            Polyline2D => Some(self.read_polyline_2d(streams)?),
            Polyline3D => Some(self.read_polyline_3d(streams)?),
            PolylinePface => Some(self.read_polyface_mesh(streams)?),
            PolylineMesh => Some(self.read_polygon_mesh(streams)?),

            // Basic geometry
            Arc => Some(self.read_arc(streams)?),
            Circle => Some(self.read_circle(streams)?),
            Line => Some(self.read_line(streams)?),
            Point => Some(self.read_point(streams)?),
            Face3D => Some(self.read_3d_face(streams)?),
            Solid | Trace => Some(self.read_solid(streams)?),
            Shape => Some(self.read_shape(streams)?),

            // Dimensions
            DimensionOrdinate => Some(self.read_dim_ordinate(streams)?),
            DimensionLinear => Some(self.read_dim_linear(streams)?),
            DimensionAligned => Some(self.read_dim_aligned(streams)?),
            DimensionAng3Pt => Some(self.read_dim_angular_3pt(streams)?),
            DimensionAng2Ln => Some(self.read_dim_angular_2ln(streams)?),
            DimensionRadius => Some(self.read_dim_radius(streams)?),
            DimensionDiameter => Some(self.read_dim_diameter(streams)?),

            // Complex entities
            Viewport => Some(self.read_viewport(streams)?),
            Ellipse => Some(self.read_ellipse(streams)?),
            Spline => Some(self.read_spline(streams)?),
            Region => Some(self.read_modeler_geometry(streams, ModelerGeoType::Region)?),
            Solid3D => Some(self.read_solid_3d(streams)?),
            Body => Some(self.read_modeler_geometry(streams, ModelerGeoType::Body)?),
            Ray => Some(self.read_ray(streams)?),
            Xline => Some(self.read_xline(streams)?),
            Mtext => Some(self.read_mtext(streams)?),
            Leader => Some(self.read_leader(streams)?),
            Tolerance => Some(self.read_tolerance(streams)?),
            Mline => Some(self.read_mline(streams)?),
            LwPolyline => Some(self.read_lwpolyline(streams)?),
            Hatch => Some(self.read_hatch(streams)?),
            Ole2Frame => Some(self.read_ole2frame(streams)?),

            // Table control objects
            BlockControlObj => Some(self.read_block_control(streams)?),
            LayerControlObj => Some(self.read_table_control(streams, templates::TableControlType::LayerControl)?),
            StyleControlObj => Some(self.read_table_control(streams, templates::TableControlType::TextStyleControl)?),
            LtypeControlObj => Some(self.read_ltype_control(streams)?),
            ViewControlObj => Some(self.read_table_control(streams, templates::TableControlType::ViewControl)?),
            UcsControlObj => Some(self.read_table_control(streams, templates::TableControlType::UcsControl)?),
            VportControlObj => Some(self.read_table_control(streams, templates::TableControlType::VPortControl)?),
            AppidControlObj => Some(self.read_table_control(streams, templates::TableControlType::AppIdControl)?),
            DimstyleControlObj => Some(self.read_table_control(streams, templates::TableControlType::DimStyleControl)?),
            VpEntHdrCtrlObj => Some(self.read_table_control(streams, templates::TableControlType::ViewportEntityHeaderControl)?),

            // Table entries
            BlockHeader => Some(self.read_block_header(streams)?),
            Layer => Some(self.read_layer(streams)?),
            Style => Some(self.read_text_style(streams)?),
            Ltype => Some(self.read_ltype(streams)?),
            View => Some(self.read_view(streams)?),
            Ucs => Some(self.read_ucs(streams)?),
            Vport => Some(self.read_vport(streams)?),
            Appid => Some(self.read_appid(streams)?),
            Dimstyle => Some(self.read_dimstyle(streams)?),
            VpEntHdr => Some(self.read_viewport_entity_header(streams)?),

            // Non-graphical objects
            Dictionary => Some(self.read_dictionary(streams)?),
            Group => Some(self.read_group(streams)?),
            MlineStyle => Some(self.read_mline_style(streams)?),
            XRecord => Some(self.read_xrecord(streams)?),
            AcDbPlaceholder => Some(self.read_placeholder(streams)?),
            Layout => Some(self.read_layout(streams)?),

            // Proxy
            AcadProxyEntity => Some(self.read_proxy_entity(streams)?),
            AcadProxyObject => Some(self.read_proxy_object(streams)?),

            // Unknown / dummy
            OleFrame | Dummy => {
                let t = self.read_unknown_entity(streams)?;
                self.notify(
                    &format!("Unlisted object type {obj_type:?} read as UnknownEntity"),
                    NotificationType::Warning,
                );
                Some(t)
            }
            VbaProject | LongTransaction => {
                let t = self.read_unknown_non_graphical_object(streams)?;
                self.notify(
                    &format!("Unlisted object type {obj_type:?} read as GenericObject"),
                    NotificationType::Warning,
                );
                Some(t)
            }

            // Undefined / unknown codes
            Invalid | Undefined | Unknown9 | Unknown36 | Unknown37 | Unknown3A | Unknown3B => None,

            // Unlisted (class-based) types
            Unlisted => self.read_unlisted_type(raw_type, streams)?,
        };

        Ok(template)
    }

    /// Attempt to read a class-based (unlisted) object type.
    ///
    /// Looks up the raw type code in the class map to find the DXF class name,
    /// then dispatches to the appropriate reader method.
    fn read_unlisted_type(
        &mut self,
        raw_type: i16,
        streams: &mut StreamSet,
    ) -> Result<Option<CadTemplate>> {
        // Look up the DXF class by raw class number.
        let class = match self.class_map.get(&raw_type) {
            Some(c) => c.clone(),
            None => {
                self.notify(
                    &format!(
                        "Unknown class number {raw_type} — no DxfClass entry found"
                    ),
                    NotificationType::Warning,
                );
                return Ok(None);
            }
        };

        let dxf_name = class.dxf_name.to_uppercase();
        let is_entity = class.is_an_entity;

        let result = match dxf_name.as_str() {
            // ----- Entities -----
            "MESH" => Some(self.read_mesh(streams)?),
            "PDFUNDERLAY" | "DWFUNDERLAY" | "DGNUNDERLAY" => {
                let utype = match dxf_name.as_str() {
                    "PDFUNDERLAY" => crate::entities::underlay::UnderlayType::Pdf,
                    "DWFUNDERLAY" => crate::entities::underlay::UnderlayType::Dwf,
                    "DGNUNDERLAY" => crate::entities::underlay::UnderlayType::Dgn,
                    _ => unreachable!(),
                };
                Some(self.read_underlay(streams, utype)?)
            }
            "ACAD_TABLE" => Some(self.read_table_entity(streams)?),
            "IMAGE" => Some(self.read_cad_image(streams, false)?),
            "WIPEOUT" => Some(self.read_cad_image(streams, true)?),
            "MULTILEADER" | "ACDB_MLEADER_CLASS" => Some(self.read_multileader(streams)?),

            // ----- Non-graphical objects -----
            "ACDBDICTIONARYWDFLT" | "DICTIONARYWDFLT" => {
                Some(self.read_dictionary_with_default(streams)?)
            }
            "DICTIONARYVAR" => Some(self.read_dictionary_var(streams)?),
            "DBCOLOR" => Some(self.read_db_color(streams)?),
            "IMAGEDEF" => Some(self.read_image_definition(streams)?),
            "IMAGEDEF_REACTOR" => Some(self.read_image_definition_reactor(streams)?),
            "RASTERVARIABLES" => Some(self.read_raster_variables(streams)?),
            "SCALE" => Some(self.read_scale(streams)?),
            "SORTENTSTABLE" => Some(self.read_sort_entities_table(streams)?),
            "MLEADERSTYLE" => Some(self.read_mleader_style(streams)?),
            "VISUALSTYLE" => Some(self.read_visual_style(streams)?),
            "MATERIAL" => Some(self.read_material(streams)?),
            "PLOTSETTINGS" => Some(self.read_plot_settings(streams)?),
            "TABLESTYLE" => Some(self.read_table_style(streams)?),
            "PDFDEFINITION" | "DWFDEFINITION" | "DGNDEFINITION" => {
                Some(self.read_pdf_definition(streams)?)
            }
            "GEODATA" => Some(self.read_geodata(streams)?),
            "ACAD_EVALUATION_GRAPH" => Some(self.read_evaluation_graph(streams)?),
            "XRECORD" => Some(self.read_xrecord(streams)?),
            "ACDBPLACEHOLDER" | "PLACEHOLDER" => Some(self.read_acdb_placeholder(streams)?),
            "WIPEOUTVARIABLES" => Some(self.read_wipeout_variables(streams)?),

            // ----- Unknown / unimplemented -----
            _ => {
                if is_entity {
                    self.notify(
                        &format!("Unlisted entity type '{}' read as UnknownEntity", class.dxf_name),
                        NotificationType::Warning,
                    );
                    Some(self.read_unknown_entity(streams)?)
                } else {
                    self.notify(
                        &format!(
                            "Unlisted object type '{}' read as GenericObject",
                            class.dxf_name
                        ),
                        NotificationType::Warning,
                    );
                    Some(self.read_unknown_non_graphical_object(streams)?)
                }
            }
        };

        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    fn notify(&mut self, message: &str, ntype: NotificationType) {
        self.notifications.push(Notification {
            message: message.to_string(),
            notification_type: ntype,
        });
    }

    /// Update the text reader and the handle reader at the end-of-object
    /// position.  Called for R13-R14 / R2000-R2004 objects that embed the
    /// handle section offset inside the object data.
    fn update_handle_reader(&self, streams: &mut StreamSet) -> Result<()> {
        // RL: Size of object data in bits (the "endbit" of the pre-handles section).
        let size = streams.object_reader.read_raw_long()? as i64;
        let end_bits = size + self.object_initial_pos;

        // Position the handle reader at the end of the main data.
        streams.handles_reader.set_position_in_bits(end_bits);

        // For R2007 (AC1021), also set up a text reader.
        if self.version == DxfVersion::AC1021 {
            let mut text_reader = self.make_reader();
            text_reader.set_position_by_flag(end_bits - 1)?;
            streams.text_reader = text_reader;
            streams.has_separate_text_reader = true;
        }

        streams.merged = true;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Stream set — holds the three sub-readers for an object
// ---------------------------------------------------------------------------

/// Holds the three sub-stream readers for a single object.
///
/// For R2007+ (when `merged` is true), reads are routed:
/// - handle_reference → `handles_reader`
/// - text reads → `text_reader`
/// - everything else → `object_reader`
pub struct StreamSet {
    pub object_reader: Box<dyn IDwgStreamReader>,
    pub text_reader: Box<dyn IDwgStreamReader>,
    pub handles_reader: Box<dyn IDwgStreamReader>,
    /// Whether this stream set is a merged multi-stream.
    pub merged: bool,
    /// Whether the text reader is a separate stream (R2007+ only).
    /// For pre-R2007, text is inline in the object data.
    pub has_separate_text_reader: bool,
    /// Handle of the current object being read.
    ///
    /// Used as the reference for relative handle codes (0x6, 0x8, 0xA, 0xC)
    /// in the handle section. Set after reading the object's own handle in
    /// `read_common_entity_data` / `read_common_non_entity_data`.
    pub current_handle: u64,
}

impl Default for StreamSet {
    fn default() -> Self {
        Self {
            object_reader: Box::new(get_stream_handler(DxfVersion::AC1015, Vec::new())),
            text_reader: Box::new(get_stream_handler(DxfVersion::AC1015, Vec::new())),
            handles_reader: Box::new(get_stream_handler(DxfVersion::AC1015, Vec::new())),
            merged: false,
            has_separate_text_reader: false,
            current_handle: 0,
        }
    }
}

impl StreamSet {
    /// Read a handle reference from the handles sub-reader.
    ///
    /// Resolves relative handle codes (0x6, 0x8, 0xA, 0xC) against
    /// `current_handle` — the handle of the object currently being read.
    pub fn handle_ref(&mut self) -> Result<u64> {
        self.handles_reader.handle_reference_resolved(self.current_handle)
    }

    /// Read variable text from the text sub-reader.
    ///
    /// For R2007+ when `has_separate_text_reader` is true, reads from the
    /// dedicated text sub-stream. For pre-R2007, text is inline in the
    /// object data — read from `object_reader`.
    pub fn read_text(&mut self) -> Result<String> {
        if self.has_separate_text_reader {
            self.text_reader.read_variable_text()
        } else {
            self.object_reader.read_variable_text()
        }
    }

    /// Read CmColor from the object reader.
    pub fn read_cm_color(&mut self) -> Result<crate::types::Color> {
        self.object_reader.read_cm_color()
    }
}

/// Modeler geometry sub-type for dispatch.
#[derive(Debug, Clone, Copy)]
pub enum ModelerGeoType {
    Region,
    Body,
    Solid3D,
}
