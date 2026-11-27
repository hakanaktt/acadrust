//! Multi-view block records: the definition (`AecDbMvBlockDef`) and its
//! placements (`AecDbMvBlockRef`).
//!
//! A multi-view block definition names one or more views, each a pair of a
//! display representation and a native BLOCK_RECORD the view displays. A
//! reference places one definition in model space: the class body carries the
//! view scales and cut parameters, the handle stream points at the definition
//! and its view blocks, and the text stream carries the definition name and
//! the reference's GUID.
//!
//! The class bodies the application writes are recorded runs (constant across
//! the corpus for the references, one recorded byte per view for the
//! definition); a rebuild carries them verbatim, exactly as the reference
//! application stored them. Forms the typed fields cannot hold are refused.

use super::decode::{open_entity, open_object, read_entity_tag, read_object_tag, AecPlacement};
use super::{ArchEntityData, ArchObject};
use crate::classes::{DxfClass, ProxyFlags};
use crate::entities::{EntityCommon, EntityType, ExtendedEntity, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::io::dwg::dwg_stream_writers::object_writer::DwgObjectWriter;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::objects::{Dictionary, ObjectType};
use crate::tables::{Layer, TableEntry};
use crate::types::{DxfVersion, Handle};
use crate::CadDocument;

/// Class schema of `AecDbMvBlockRef`.
pub const MV_BLOCK_REF_SCHEMA: i16 = 87;
/// Class schema of `AecDbMvBlockDef`.
pub const MV_BLOCK_DEF_SCHEMA: i16 = 86;

/// Application name the reference application registers both classes under.
const APPLICATION: &str = "AecBase80|Product Desc: Autodesk Architectural Desktop 2007 |Company: Autodesk,Inc. |WEB Address: www.autodesk.com";

/// Name of the named-objects dictionary that holds the definitions.
const DEFINITIONS: &str = "AEC_MVBLOCK_DEFS";

/// The recorded parameter run of a class body: `bits` bits, MSB-first out of
/// `bytes` (trailing pad bits in the last byte are zero).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ClassBody {
    pub bytes: Vec<u8>,
    pub bits: u64,
}

impl ClassBody {
    fn write(&self, w: &mut DwgMergedWriter) {
        for bit in 0..self.bits {
            let byte = self.bytes[(bit / 8) as usize];
            w.write_bit(byte & (1 << (7 - (bit % 8))) != 0);
        }
    }

    fn validate(&self) -> Result<(), DxfError> {
        if self.bits < 64 || self.bits > self.bytes.len() as u64 * 8 {
            return Err(DxfError::Parse(
                "multi-view block class body is not a recorded run".into(),
            ));
        }
        if self.bytes.is_empty() {
            return Err(DxfError::Parse("multi-view block class body is empty".into()));
        }
        Ok(())
    }
}

/// One view of a [`MvBlockDef`]: its display representation and the native
/// block the view displays.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MvBlockView {
    pub display_rep: Handle,
    pub block: Handle,
}

/// A multi-view block definition (`AEC_MVBLOCK_DEF`, `AecDbMvBlockDef`).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MvBlockDef {
    pub name: String,
    pub views: Vec<MvBlockView>,
    /// The record's full text stream, in stored order.
    pub texts: Vec<String>,
    /// The recorded class-body parameter run.
    pub body: ClassBody,
}

/// A multi-view block reference (`AEC_MVBLOCK_REF`, `AecDbMvBlockRef`).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MvBlockRef {
    /// The definition name, the record's first text.
    pub name: String,
    /// The reference GUID, the entity identifier.
    pub guid: String,
    pub placement: AecPlacement,
    /// The `AEC_MVBLOCK_DEF` this reference places.
    pub definition: Handle,
    /// The view block records, in stored order (plan first in the corpus).
    pub view_blocks: Vec<Handle>,
    /// The record's full text stream, in stored order.
    pub texts: Vec<String>,
    /// The recorded class-body parameter run.
    pub body: ClassBody,
}

impl MvBlockRef {
    /// Read the shared record fields of a multi-view block reference.
    ///
    /// `None` when the record is another class, uses a different schema, or
    /// does not follow the stored layout (references, texts, placement).
    pub fn decode(data: &ArchEntityData) -> Option<MvBlockRef> {
        if data.dxf_name != "AEC_MVBLOCK_REF" {
            return None;
        }
        let decoded = data.decode();
        if decoded.schema != Some(MV_BLOCK_REF_SCHEMA) {
            return None;
        }
        let placement = decoded.placement.clone()?;
        let texts = decoded.strings;
        if texts.len() < 3 || texts[0].is_empty() {
            return None;
        }
        let typed = data.typed_handles();
        // Handle stream: null hard ownership (the entity prefix's), then the
        // definition and one hard pointer per view block.
        const HARD_OWNERSHIP: u8 = 3;
        const HARD_POINTER: u8 = 5;
        if typed.len() < 4 || typed[0] != (0, HARD_OWNERSHIP) {
            return None;
        }
        if typed[1..].iter().any(|(_, kind)| *kind != HARD_POINTER) {
            return None;
        }
        let definition = Handle::new(typed[1].0);
        let view_blocks: Vec<Handle> =
            typed[2..].iter().map(|(value, _)| Handle::new(*value)).collect();
        let body = ref_class_body(data)?;
        Some(MvBlockRef {
            name: texts[0].clone(),
            guid: placement.identifier.clone(),
            placement,
            definition,
            view_blocks,
            texts,
            body,
        })
    }
}

impl MvBlockDef {
    /// Read the shared record fields of a multi-view block definition.
    ///
    /// `None` when the record is another class, uses a different schema, or
    /// does not follow the stored layout.
    pub fn decode(object: &ArchObject) -> Option<MvBlockDef> {
        if object.dxf_name != "AEC_MVBLOCK_DEF" {
            return None;
        }
        let decoded = object.decode();
        if decoded.schema != Some(MV_BLOCK_DEF_SCHEMA) {
            return None;
        }
        let texts = decoded.strings;
        if texts.len() < 2 || texts[0].is_empty() {
            return None;
        }
        let typed = object.typed_handles();
        // Handle stream: display-rep/view-block hard-pointer pairs, closed by
        // a null hard pointer.
        const HARD_POINTER: u8 = 5;
        let last = *typed.last()?;
        if last != (0, HARD_POINTER) || typed.len() < 3 || typed.len() % 2 == 0 {
            return None;
        }
        if typed[..typed.len() - 1].iter().any(|(_, kind)| *kind != HARD_POINTER) {
            return None;
        }
        let views: Vec<MvBlockView> = typed[..typed.len() - 1]
            .chunks(2)
            .map(|pair| MvBlockView {
                display_rep: Handle::new(pair[0].0),
                block: Handle::new(pair[1].0),
            })
            .collect();
        let body = def_class_body(object)?;
        Some(MvBlockDef {
            name: texts[0].clone(),
            views,
            texts,
            body,
        })
    }
}

/// The class-body bits of a reference record: after the entity envelope and
/// its trailing prefix marker, to the end of the main stream.
fn ref_class_body(data: &ArchEntityData) -> Option<ClassBody> {
    let mut reader = open_entity(data).ok()?;
    let _ = read_entity_tag(&mut reader);
    if reader.check_error().is_err() || reader.read_bit_short() != 80 {
        return None;
    }
    main_bits(&reader)
}

/// The class-body bits of a definition record: after the object prefix (the
/// two name texts, a clear byte, and the closing marker) to the main end.
fn def_class_body(object: &ArchObject) -> Option<ClassBody> {
    let mut reader = open_object(object).ok()?;
    let _ = read_object_tag(&mut reader);
    if reader.read_bit_short() != 80 {
        return None;
    }
    let _ = reader.read_variable_text();
    if reader.read_bit_short() != 80 {
        return None;
    }
    let _ = reader.read_variable_text();
    if reader.read_bit() || reader.read_byte() != 0 || reader.read_bit_short() != 80 {
        return None;
    }
    reader.check_error().ok()?;
    main_bits(&reader)
}

/// The remaining main-stream bits, MSB-first into bytes (zero pad).
fn main_bits(reader: &crate::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader) -> Option<ClassBody> {
    let all = reader.raw_merged_data();
    let start = reader.position_in_bits();
    let bits = reader.main_remaining_bits();
    if start < 0 || bits < 0 {
        return None;
    }
    let (start, end) = (start as u64, (start + bits) as u64);
    if end > all.len() as u64 * 8 {
        return None;
    }
    let mut bytes = Vec::with_capacity((bits as usize + 7) / 8);
    let mut bit = start;
    while bit < end {
        let mut byte = 0u8;
        for shift in (0..8).rev() {
            if bit < end {
                let position = bit as usize;
                byte |= ((all[position / 8] >> (7 - (position % 8))) & 1) << shift;
                bit += 1;
            }
        }
        bytes.push(byte);
    }
    Some(ClassBody { bytes, bits: bits as u64 })
}

fn invalid(message: &str) -> DxfError {
    DxfError::Parse(message.into())
}

fn register_class(
    doc: &mut CadDocument,
    dxf: &str,
    cpp: &str,
    entity: bool,
) -> Result<i16, DxfError> {
    if let Some(class) = doc.classes.get_by_name(dxf) {
        return Ok(class.class_number);
    }
    let mut class = if entity {
        DxfClass::new_entity(dxf, cpp)
    } else {
        DxfClass::new(dxf, cpp)
    };
    class.application_name = APPLICATION.into();
    class.proxy_flags = ProxyFlags(if entity { 893 } else { 129 });
    class.dwg_version = 21;
    doc.classes.add_or_update(class);
    doc.classes
        .get_by_name(dxf)
        .map(|class| class.class_number)
        .ok_or_else(|| invalid("multi-view block class was not registered"))
}

/// Writes the entity envelope the AEC classes share: the class tag, the name
/// and identifier texts, the placement frame, and the closing marker and null
/// ownership the handle stream opens with.
fn entity_prefix(
    w: &mut DwgMergedWriter,
    schema: i16,
    texts: &[String],
    placement: &AecPlacement,
) {
    w.write_bit_short(80);
    w.write_bit(true);
    w.write_bit(true);
    w.write_bit_short(schema);
    w.write_bit_short(80);
    w.write_variable_text(&texts[0]);
    w.write_bit_short(80);
    w.write_bit_short(0);
    w.write_byte(1);
    w.write_bit(false);
    w.write_bit_short(80);
    w.write_variable_text(&texts[1]);
    for axis in [
        placement.origin,
        placement.x_axis,
        placement.y_axis,
        placement.z_axis,
    ] {
        w.write_bit_double(axis.x);
        w.write_bit_double(axis.y);
        w.write_bit_double(axis.z);
    }
    w.write_variable_text(&texts[2]);
    w.write_handle(DwgReferenceType::HardOwnership, 0);
    w.write_bit_short(80);
}

/// The `AEC_MVBLOCK_DEFS` dictionary under the named objects dictionary,
/// created when the drawing has none. Returns the dictionary's handle.
fn definitions_dictionary(doc: &mut CadDocument) -> Result<Handle, DxfError> {
    let nod = doc.header.named_objects_dict_handle;
    if let Some(handle) = doc
        .named_objects_dictionary()
        .and_then(|dict| dict.get(DEFINITIONS))
    {
        return Ok(handle);
    }
    let handle = doc.allocate_handle();
    let mut dictionary = Dictionary::new();
    dictionary.handle = handle;
    dictionary.owner = nod;
    doc.objects
        .insert(handle, ObjectType::Dictionary(dictionary));
    let Some(ObjectType::Dictionary(nod_dict)) = doc.objects.get_mut(&nod) else {
        return Err(invalid("drawing has no named objects dictionary"));
    };
    nod_dict.add_entry(DEFINITIONS, handle);
    Ok(handle)
}

/// Writes a multi-view block definition into the drawing and lists it in the
/// `AEC_MVBLOCK_DEFS` dictionary under its name.
///
/// Every view's display representation and block must exist in the drawing;
/// the class body must be a recorded run (a rebuild carries it verbatim; a
/// definition the crate has no recorded run for is refused, not invented).
pub fn add_mv_block_def(doc: &mut CadDocument, def: &MvBlockDef) -> Result<Handle, DxfError> {
    if def.name.trim().is_empty() {
        return Err(invalid("multi-view block definition name must not be empty"));
    }
    if def.views.is_empty() {
        return Err(invalid("multi-view block definition has no views"));
    }
    if def.texts.len() < 2 || def.texts[0] != def.name {
        return Err(invalid("multi-view block definition texts do not match its name"));
    }
    def.body.validate()?;
    for view in &def.views {
        // The display representation may be any stored object of the
        // drawing; the block must be a real record.
        let block = view.block;
        if block != Handle::NULL && doc.block_records.iter().all(|r| r.handle != block) {
            return Err(invalid("multi-view block view names no block record"));
        }
    }
    let type_code = register_class(doc, "AEC_MVBLOCK_DEF", "AecDbMvBlockDef", false)?;
    let dictionary = definitions_dictionary(doc)?;
    let handle = doc.allocate_handle();
    let views = def.views.clone();
    let texts = def.texts.clone();
    let body = def.body.clone();
    let mut writer = DwgObjectWriter::new(doc)?;
    let (raw, handle_bits) = writer.encode_architecture_object(type_code, handle, dictionary, |w| {
        // The object prefix: class tag, name twice, a clear byte, marker.
        w.write_bit_short(80);
        w.write_bit(true);
        w.write_bit(true);
        w.write_bit_short(MV_BLOCK_DEF_SCHEMA);
        w.write_bit_short(80);
        w.write_variable_text(&texts[0]);
        w.write_bit_short(80);
        w.write_variable_text(&texts[1]);
        w.write_bit(false);
        w.write_byte(0);
        w.write_bit_short(80);
        body.write(w);
        for view in &views {
            w.write_handle(DwgReferenceType::HardPointer, view.display_rep.value());
            w.write_handle(DwgReferenceType::HardPointer, view.block.value());
        }
        w.write_handle(DwgReferenceType::HardPointer, 0);
        for text in &texts[2..] {
            w.write_variable_text(text);
        }
    });
    let mut object = ArchObject::new("AEC_MVBLOCK_DEF", handle, dictionary);
    object.cpp_class_name = "AecDbMvBlockDef".into();
    object.reactors.push(dictionary);
    object.raw_dwg_data = Some(raw);
    object.raw_dwg_handle_bits = handle_bits;
    object.raw_dwg_version = Some(DxfVersion::AC1032);
    doc.objects.insert(handle, ObjectType::Arch(object));
    if let Some(ObjectType::Dictionary(dict)) = doc.objects.get_mut(&dictionary) {
        dict.insert(def.name.clone(), handle);
    }
    Ok(handle)
}

/// Places a multi-view block definition in model space.
///
/// The definition must exist in the drawing and every view block must name a
/// block record. `layer` selects the entity layer; the layer must exist.
/// The class body must be a recorded run: the application writes the view
/// scales and cut parameters itself, and a rebuild carries them verbatim.
pub fn add_mv_block_ref(doc: &mut CadDocument, reference: &MvBlockRef, layer: Option<&str>) -> Result<Handle, DxfError> {
    if reference.view_blocks.is_empty() {
        return Err(invalid("multi-view block reference names no view block"));
    }
    if reference.texts.len() < 3
        || reference.texts[0] != reference.name
        || reference.texts[1] != reference.guid
    {
        return Err(invalid("multi-view block reference texts do not match its fields"));
    }
    reference.body.validate()?;
    let definition = reference.definition;
    let known = matches!(doc.objects.get(&definition), Some(ObjectType::Arch(a)) if a.dxf_name == "AEC_MVBLOCK_DEF");
    if !known {
        return Err(invalid("multi-view block reference names no definition"));
    }
    for block in &reference.view_blocks {
        if doc.block_records.iter().all(|r| r.handle != *block) {
            return Err(invalid("multi-view block reference names no block record"));
        }
    }
    let layer_name = layer.unwrap_or("0").to_string();
    if layer_name.trim().is_empty() {
        return Err(invalid("multi-view block reference layer must not be empty"));
    }
    let layer_handle = match doc.layers.get(&layer_name) {
        Some(layer) => layer.handle,
        None => {
            let mut layer = Layer::new(layer_name.clone());
            layer.set_handle(doc.allocate_handle());
            let handle = layer.handle;
            doc.layers
                .add(layer)
                .map_err(|error| invalid(&error.to_string()))?;
            handle
        }
    };
    let type_code = register_class(doc, "AEC_MVBLOCK_REF", "AecDbMvBlockRef", true)?;
    let common = EntityCommon {
        handle: doc.allocate_handle(),
        owner_handle: doc.header.model_space_block_handle,
        layer: layer_name,
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    let texts = reference.texts.clone();
    let body = reference.body.clone();
    let view_blocks = reference.view_blocks.clone();
    let placement = reference.placement.clone();
    let mut writer = DwgObjectWriter::new(doc)?;
    let (raw, handle_bits) = writer.encode_architecture_entity(type_code, &common, |w| {
        entity_prefix(w, MV_BLOCK_REF_SCHEMA, &texts, &placement);
        body.write(w);
        w.write_handle(DwgReferenceType::HardPointer, definition.value());
        for block in &view_blocks {
            w.write_handle(DwgReferenceType::HardPointer, block.value());
        }
        for text in &texts[3..] {
            w.write_variable_text(text);
        }
    });
    let mut data = ArchEntityData::from_parts("AEC_MVBLOCK_REF", "AecDbMvBlockRef", None);
    data.dwg_type_code = type_code;
    data.dwg_handle_bits = handle_bits;
    data.dwg_source_version = Some(DxfVersion::AC1032);
    data.raw_dwg_data = Some(raw);
    let handle = common.handle;
    doc.add_entity(EntityType::Extended(Box::new(ExtendedEntity {
        common,
        data: ExtendedEntityData::Arch(data),
    })))
    .map_err(|_| invalid("multi-view block reference was not added"))?;
    Ok(handle)
}
