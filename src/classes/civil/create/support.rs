//! Classes, the root tree node, and the record writer every civil writer
//! shares.

use super::super::bits::Bits;
use crate::classes::arch::{ArchEntityData, ArchObject};
use crate::classes::{DxfClass, ProxyFlags};
use crate::entities::{EntityCommon, EntityType, ExtendedEntity, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::io::dwg::dwg_stream_writers::{object_writer::DwgObjectWriter, DwgMergedWriter};
use crate::objects::ObjectType;
use crate::types::{DxfVersion, Handle};
use crate::CadDocument;

/// Version word that opens every civil record body.
pub(super) const VERSION: i16 = 0x04BE;

/// Key of a node in the root tree node.
pub(super) const KEY_POINT_GROUPS: i32 = 32;
pub(super) const KEY_POINTS: i32 = 34;
pub(super) const KEY_ALL_POINTS_QUERY: i32 = 75;

/// Name of the root tree node in the named objects dictionary.
const ROOT: &str = "Root";
/// The root's texts. The second names a collection the reference
/// application keeps under the root; it is the same in every sample.
const ROOT_STRINGS: [&str; 2] = ["", "Pipe Network Interference Set Collection"];

struct Class {
    dxf: &'static str,
    cpp: &'static str,
    module: &'static str,
    proxy_flags: u16,
    entity: bool,
}

/// Civil classes a writer registers, with the class table values the
/// reference application gives them. The application name is only the
/// module name; the application accepts it (see docs/civil-write.md).
const CLASSES: &[Class] = &[
    Class { dxf: "AECC_TREE_NODE", cpp: "AeccDbTreeNode", module: "AeccvBase130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_POINT_NODE", cpp: "AeccDbPointsNode", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_POINT_GROUP_NODE", cpp: "AeccDbPointGroupsNode", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_POINT_PG", cpp: "AeccDbPG", module: "AeccLand130", proxy_flags: 0x37d, entity: false },
    Class { dxf: "AECC_QUERY_STATEMENT", cpp: "AeccDbQueryStatement", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_SETTINGS_NODE", cpp: "AeccDbSettingsNode", module: "AeccvBase130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_CLASS_NODE", cpp: "AeccDbClassNode", module: "AeccvBase130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_ROOT_SETTINGS_NODE", cpp: "AeccDbRootSettingsNode", module: "AeccvBase130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_USER_DEFINED_ATTRIBUTE_CLASSIFICATION", cpp: "AeccDbUserDefinedAttributeClassification", module: "AeccvBase130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_POINT_STYLE", cpp: "AeccDbPointStyle", module: "AeccLand130", proxy_flags: 0x81, entity: false },
    Class { dxf: "AECC_COGO_POINT", cpp: "AeccDbCogoPoint", module: "AeccLand130", proxy_flags: 0x33c, entity: true },
];

pub(super) fn invalid(message: &str) -> DxfError {
    DxfError::Parse(message.into())
}

/// Registers `dxf` (once) and returns its DWG type code.
pub(super) fn register(doc: &mut CadDocument, dxf: &str) -> Result<i16, DxfError> {
    if let Some(class) = doc.classes.get_by_name(dxf) {
        return Ok(class.class_number);
    }
    let class = CLASSES.iter().find(|c| c.dxf == dxf).ok_or_else(|| invalid("unknown civil class"))?;
    let mut entry = if class.entity { DxfClass::new_entity(class.dxf, class.cpp) } else { DxfClass::new(class.dxf, class.cpp) };
    entry.application_name = class.module.into();
    entry.proxy_flags = ProxyFlags(class.proxy_flags);
    entry.dwg_version = 25;
    doc.classes.add_or_update(entry);
    doc.classes.get_by_name(dxf).map(|c| c.class_number).ok_or_else(|| invalid("civil class was not registered"))
}

/// One reference of a record body and its kind.
#[derive(Clone, Copy)]
pub(super) struct Ref(pub DwgReferenceType, pub u64);

pub(super) fn soft_pointer(h: u64) -> Ref {
    Ref(DwgReferenceType::SoftPointer, h)
}
pub(super) fn hard_pointer(h: u64) -> Ref {
    Ref(DwgReferenceType::HardPointer, h)
}
pub(super) fn soft_owner(h: u64) -> Ref {
    Ref(DwgReferenceType::SoftOwnership, h)
}

/// A record body: fields go to `w`, references and texts are listed here
/// and written after the fields, in order.
pub(super) struct Body {
    pub refs: Vec<Ref>,
    pub strings: Vec<String>,
}

/// Writes the constant bit run `bits` ("0"/"1" characters).
pub(super) fn bits(w: &mut DwgMergedWriter, bits: &str) {
    for b in bits.bytes() {
        w.write_bit(b == b'1');
    }
}

/// BL in its one-byte form even for 0 (some fields are always written so).
pub(super) fn bl_byte(w: &mut DwgMergedWriter, value: u8) {
    w.write_2bits(1);
    w.write_byte(value);
}

fn finish(w: &mut DwgMergedWriter, body: &Body) {
    for r in &body.refs {
        w.write_handle(r.0, r.1);
    }
    for s in &body.strings {
        w.write_variable_text(s);
    }
}

/// Encodes and stores (or replaces) a civil dictionary object.
pub(super) fn put_object(
    doc: &mut CadDocument,
    dxf: &str,
    handle: Handle,
    owner: Handle,
    reactors: Vec<Handle>,
    fields: impl FnOnce(&mut DwgMergedWriter),
    body: Body,
) -> Result<(), DxfError> {
    let type_code = register(doc, dxf)?;
    let mut writer = DwgObjectWriter::new(doc)?;
    let (raw, handle_bits) = writer.encode_civil_object(type_code, handle, owner, &reactors, |w| {
        w.write_bit_short(VERSION);
        fields(w);
        finish(w, &body);
    });
    let mut object = ArchObject::new(dxf, handle, owner);
    object.cpp_class_name = CLASSES.iter().find(|c| c.dxf == dxf).map(|c| c.cpp).unwrap_or("").into();
    object.reactors = reactors;
    object.raw_dwg_data = Some(raw);
    object.raw_dwg_handle_bits = handle_bits;
    object.raw_dwg_version = Some(DxfVersion::AC1032);
    doc.objects.insert(handle, ObjectType::Arch(object));
    Ok(())
}

/// Encodes a civil entity and stores it (replacing one with the same handle).
pub(super) fn put_entity(
    doc: &mut CadDocument,
    dxf: &str,
    common: EntityCommon,
    fields: impl FnOnce(&mut DwgMergedWriter),
    body: Body,
) -> Result<Handle, DxfError> {
    let type_code = register(doc, dxf)?;
    let mut writer = DwgObjectWriter::new(doc)?;
    let (raw, handle_bits) = writer.encode_architecture_entity(type_code, &common, |w| {
        w.write_bit_short(VERSION);
        fields(w);
        finish(w, &body);
    });
    let cpp = CLASSES.iter().find(|c| c.dxf == dxf).map(|c| c.cpp).unwrap_or("");
    let mut data = ArchEntityData::from_parts(dxf, cpp, common.graphic_data.as_deref());
    data.dwg_type_code = type_code;
    data.dwg_handle_bits = handle_bits;
    data.dwg_source_version = Some(DxfVersion::AC1032);
    data.raw_dwg_data = Some(raw);
    let handle = common.handle;
    let entity = EntityType::Extended(Box::new(ExtendedEntity { common, data: ExtendedEntityData::Arch(data) }));
    if let Some(slot) = doc.get_entity_mut(handle) {
        *slot = entity;
        return Ok(handle);
    }
    doc.add_entity(entity).map_err(|_| invalid("civil entity was not added"))
}

/// The civil object at `handle`.
pub(super) fn object<'a>(doc: &'a CadDocument, handle: Handle) -> Option<&'a ArchObject> {
    match doc.objects.get(&handle) {
        Some(ObjectType::Arch(a)) => Some(a),
        _ => None,
    }
}

/// Main stream (data, first body bit after the version word, end) of a
/// civil object.
pub(super) fn fields_of(object: &ArchObject) -> Option<(&[u8], usize, usize)> {
    let (data, start, end) = object.main_stream()?;
    let mut b = Bits::new(data, start, end);
    (b.bit_short()? == VERSION).then_some(())?;
    Some((data, b.pos, end))
}

/// The root tree node, `None` when the drawing has none.
pub(super) fn root(doc: &CadDocument) -> Option<Handle> {
    doc.named_objects_dictionary()?.get(ROOT)
}

/// Keys and nodes of the root tree node, in stored order.
pub(super) fn root_entries(doc: &CadDocument) -> Option<Vec<(i32, Handle)>> {
    let node = object(doc, root(doc)?)?;
    let (data, start, end) = fields_of(node)?;
    let mut b = Bits::new(data, start, end);
    (b.bit_long()? == 1).then_some(())?;
    let n = usize::try_from(b.bit_long()?).ok()?;
    let keys = (0..n).map(|_| b.bit_long()).collect::<Option<Vec<_>>>()?;
    let handles = node.decode().handles;
    (handles.len() >= n).then_some(())?;
    Some(keys.into_iter().zip(handles).map(|(k, h)| (k, Handle::new(h))).collect())
}

/// Node the root tree node keeps under `key`.
pub(super) fn root_node(doc: &CadDocument, key: i32) -> Option<Handle> {
    root_entries(doc)?.into_iter().find(|(k, _)| *k == key).map(|(_, h)| h)
}

fn write_root(doc: &mut CadDocument, handle: Handle, entries: &[(i32, Handle)], strings: Vec<String>) -> Result<(), DxfError> {
    let nod = doc.header.named_objects_dict_handle;
    let keys: Vec<i32> = entries.iter().map(|e| e.0).collect();
    put_object(
        doc,
        "AECC_TREE_NODE",
        handle,
        nod,
        vec![nod],
        |w| {
            w.write_bit_long(1);
            w.write_bit_long(keys.len() as i32);
            for k in &keys {
                w.write_bit_long(*k);
            }
            w.write_bit_long(0);
            bits(w, "010");
        },
        Body { refs: entries.iter().map(|e| soft_owner(e.1.value())).collect(), strings },
    )
}

/// Adds (or repoints) the root entry `key`.
pub(super) fn set_root_entry(doc: &mut CadDocument, key: i32, node: Handle) -> Result<(), DxfError> {
    let root = ensure_civil(doc)?;
    let mut entries = root_entries(doc).ok_or_else(|| invalid("root tree node cannot be read"))?;
    match entries.iter_mut().find(|e| e.0 == key) {
        Some(e) => e.1 = node,
        // The application stores the entries by hash bucket (key mod 8),
        // a new key last in its bucket (WP00 -> WP01 adds 32, 34, 75 so).
        None => {
            let bucket = |k: i32| k as u32 % 8;
            let at = entries.iter().position(|e| bucket(e.0) > bucket(key)).unwrap_or(entries.len());
            entries.insert(at, (key, node));
        }
    }
    let strings = object(doc, root).map(|o| o.decode().strings).unwrap_or_default();
    write_root(doc, root, &entries, strings)
}

/// Makes the drawing a civil drawing: adds the root tree node (and its
/// class) when the drawing has none. Returns the root.
///
/// The reference application builds its settings and style tree itself
/// when it opens a drawing whose root lacks them (docs/civil-write.md), so
/// nothing else is written here.
pub fn ensure_civil(doc: &mut CadDocument) -> Result<Handle, DxfError> {
    if let Some(root) = root(doc) {
        return Ok(root);
    }
    let handle = doc.allocate_handle();
    write_root(doc, handle, &[], ROOT_STRINGS.iter().map(|s| s.to_string()).collect())?;
    doc.named_objects_dictionary_mut().ok_or_else(|| invalid("drawing has no named objects dictionary"))?.add_entry(ROOT, handle);
    Ok(handle)
}

