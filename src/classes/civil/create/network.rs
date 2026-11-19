//! Gravity pipe networks: the network, its structures and its pipes.
//!
//! A network part refers to a family of the drawing's parts list and to
//! the part definition (`AECC_NETWORK_PART_DEF`) of its size, which holds
//! the catalog data of that size. The writer places the parts list's null
//! structure and its 12 inch corrugated metal pipe, whose part definitions
//! it writes when the drawing lacks them.

use super::assembly::{ensure_name_template, entity_common, run};
use super::network_content::*;
use super::profile::new_identifier;
use super::support::*;
use crate::classes::arch::ArchEntityData;
use crate::entities::{EntityCommon, EntityType, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::objects::{Dictionary, ObjectType};
use crate::types::{Handle, Vector3};
use crate::CadDocument;

/// Named-objects entry holding the part definitions, keyed by size.
const PART_DEFS: &str = "AECC_NETWORK_PART_DEFS";
/// Root key of the format manager.
const KEY_FORMAT_MANAGER: i32 = 0x4C;
/// Root key of the part catalog node, the parts lists' owner.
const KEY_PART_CATALOG: i32 = 0x136;
const STRUCTURE_TEMPLATE: (i32, &str) = (0x125, "Structure - (<[67112990(2449)]>)");
const PIPE_TEMPLATE: (i32, &str) = (0x126, "Pipe - (<[67112990(2449)]>)");

/// A part the writer places: its family (by catalog id), the key of its
/// size's part definition and that definition's record.
struct Part {
    family: &'static str,
    key: &'static str,
    definition: (usize, &'static str),
    strings: &'static [&'static str],
    /// References of the definition: a structure's refer twice to the
    /// block of its body, written null (the application draws the part
    /// without it).
    references: usize,
}

const NULL_STRUCTURE: Part = Part {
    family: "D45435F0-874A-4754-BF5E-FE9D18BE870F",
    key: "D45435F0874A4754BF5EFE9D18BE870FSBSH0SBSD0",
    definition: STRUCTURE_DEFINITION,
    strings: STRUCTURE_DEFINITION_STRINGS,
    references: 2,
};
const CORRUGATED_PIPE_12: Part = Part {
    family: "481DD6EF-B926-428E-99B7-A5BE30A17E59",
    key: "481DD6EFB926428E99B7A5BE30A17E591815E08D5B6A42B087113F4839A38301",
    definition: PIPE_DEFINITION,
    strings: PIPE_DEFINITION_STRINGS,
    references: 0,
};

fn texts(t: &[&str]) -> Vec<String> {
    t.iter().map(|s| s.to_string()).collect()
}

fn hard_owner(h: u64) -> Ref {
    Ref(DwgReferenceType::HardOwnership, h)
}

fn arch_entity(doc: &CadDocument, handle: Handle) -> Option<&ArchEntityData> {
    match doc.get_entity(handle)? {
        EntityType::Extended(x) => match &x.data {
            ExtendedEntityData::Arch(a) => Some(a),
            _ => None,
        },
        _ => None,
    }
}

/// The drawing's first parts list (`AECC_NETWORK_PART_LIST`).
fn parts_list(doc: &CadDocument) -> Option<Handle> {
    doc.objects
        .iter()
        .filter(|(_, o)| matches!(o, ObjectType::Arch(a) if a.dxf_name == "AECC_NETWORK_PART_LIST"))
        .map(|(h, _)| *h)
        .min_by_key(|h| h.value())
}

/// The style and rules of a family of `list`: its family item names the
/// catalog id; its size filter (the item's first reference) refers to
/// `0, style, rules, ...`.
fn family_style(doc: &CadDocument, list: Handle, family: &str) -> Option<(u64, u64)> {
    let item = doc.objects.values().find_map(|o| match o {
        ObjectType::Arch(a) if a.dxf_name == "AECC_NETWORK_PART_FAMILY_ITEM" && a.owner == list && a.decode().strings.iter().any(|s| s == family) => Some(a),
        _ => None,
    })?;
    let filter = object(doc, Handle::new(*item.decode().handles.first()?))?;
    let refs = filter.decode().handles;
    Some((*refs.get(1)?, *refs.get(2)?)).filter(|(s, _)| *s != 0)
}

/// The application's default parts list (with its catalog node under the
/// root and the default pipe and structure styles) when the drawing has no
/// parts list. Its size filters name no rules.
fn ensure_parts_list(doc: &mut CadDocument) -> Result<Handle, DxfError> {
    if let Some(list) = parts_list(doc) {
        return Ok(list);
    }
    if root_node(doc, KEY_PART_CATALOG).is_some() {
        return Err(invalid("drawing has a part catalog without a parts list"));
    }
    super::default_styles::ensure_pipe_network_styles(doc)?;
    let root = ensure_civil(doc)?;
    let handles = super::templates::write_templates(doc, root, PARTS_LIST)?;
    set_root_entry(doc, KEY_PART_CATALOG, handles[0])?;
    Ok(handles[1])
}

/// The part definition of `part`, written when the drawing lacks it, under
/// the named-objects entry `AECC_NETWORK_PART_DEFS` (a dictionary keyed by
/// size). The record does not open with the version word.
fn ensure_definition(doc: &mut CadDocument, part: &Part) -> Result<Handle, DxfError> {
    let nod = doc.header.named_objects_dict_handle;
    let dict = match doc.named_objects_dictionary().and_then(|d| d.get(PART_DEFS)) {
        Some(h) => h,
        None => {
            let h = doc.allocate_handle();
            let mut d = Dictionary::new();
            d.handle = h;
            d.owner = nod;
            doc.objects.insert(h, ObjectType::Dictionary(d));
            match doc.objects.get_mut(&nod) {
                Some(ObjectType::Dictionary(n)) => n.add_entry(PART_DEFS, h),
                _ => return Err(invalid("drawing has no named objects dictionary")),
            }
            h
        }
    };
    match doc.objects.get(&dict) {
        Some(ObjectType::Dictionary(d)) => {
            if let Some(h) = d.get(part.key) {
                return Ok(h);
            }
        }
        _ => return Err(invalid("the part definitions entry is not a dictionary")),
    }
    let handle = doc.allocate_handle();
    put_plain_object(
        doc,
        "AECC_NETWORK_PART_DEF",
        handle,
        dict,
        vec![dict],
        |w| run(w, part.definition.0, part.definition.1),
        Body { refs: vec![hard_pointer(0); part.references], strings: texts(part.strings) },
    )?;
    if let Some(ObjectType::Dictionary(d)) = doc.objects.get_mut(&dict) {
        d.add_entry(part.key, handle);
    }
    Ok(handle)
}

/// `BL 80, BS 0x04BE`: opens a part record and each of its fields.
#[cfg(test)]
const FIELD_MARK: &str = "0101010000001011111000000100";

/// Whether the part catalog node (`AECC_NETWORK_PART_CATALOG_DEF_NODE`,
/// the parts list's owner) lists the fields of a null structure's part
/// definition. The node holds a part record whose fields are the union of
/// those of the drawing's part definitions:
///
/// ```text
/// ..., BL 80, BS 0x04BE, BL 80, BL 80, BL n, BL 0, n x field, ...
/// field: BL 80, BS 0x04BE, BL 0, BS kind, ... (75 bits for a number, 81
///        for a text: kind > 255)
/// texts: "", "AeccNetworkPartRecord", "", "", then per field
///        "AeccNetworkPartDataField", "", code, description, context,
///        unit[, default text]
/// ```
///
/// The application does not finish opening a drawing with a structure
/// whose fields the node lacks; the node also caches data after the
/// fields that grows with them (not decoded), so the writer does not add
/// them.
fn catalog_lists_structure_fields(doc: &CadDocument, list: Handle) -> bool {
    let Some(node) = object(doc, list).and_then(|l| object(doc, l.owner)).filter(|n| n.dxf_name == "AECC_NETWORK_PART_CATALOG_DEF_NODE") else {
        return false;
    };
    let strings = node.decode().strings;
    STRUCTURE_CATALOG_FIELDS.iter().all(|f| (0..strings.len()).any(|i| strings[i] == "AeccNetworkPartDataField" && strings.get(i + 2).map(String::as_str) == Some(f.0)))
}

/// The network record (`AECC_NETWORK`, an entity in model space):
///
/// ```text
/// NETWORK_EMPTY (no parts yet) or NETWORK_WITH_PARTS, BL pipe count,
/// BL structure count, NETWORK_END
/// references: 5 x hard 0, parts list (hard), soft 0, hard owner 0,
///   the pipes, the structures (hard)
/// texts: "AeccImpNetwork", "", name, "", structure name template, ...,
///   pipe name template, ...
/// ```
fn write_network(doc: &mut CadDocument, common: EntityCommon, name: &str, list: u64, pipes: &[u64], structures: &[u64]) -> Result<Handle, DxfError> {
    let mut refs = vec![hard_pointer(0); 5];
    refs.extend([hard_pointer(list), soft_pointer(0), hard_owner(0)]);
    refs.extend(pipes.iter().chain(structures).map(|h| hard_pointer(*h)));
    let mut strings = texts(NETWORK_STRINGS);
    strings[2] = name.to_string();
    let head = if pipes.is_empty() && structures.is_empty() { NETWORK_EMPTY } else { NETWORK_WITH_PARTS };
    put_entity(
        doc,
        "AECC_NETWORK",
        common,
        |w| {
            run(w, head.0, head.1);
            w.write_bit_long(pipes.len() as i32);
            w.write_bit_long(structures.len() as i32);
            run(w, NETWORK_END.0, NETWORK_END.1);
        },
        Body { refs, strings },
    )
}

/// A network read back from its record: entity data, name, parts list,
/// pipes and structures.
struct Network {
    common: EntityCommon,
    name: String,
    list: u64,
    pipes: Vec<u64>,
    structures: Vec<u64>,
}

fn read_network(doc: &CadDocument, network: Handle) -> Option<Network> {
    let a = arch_entity(doc, network).filter(|a| a.dxf_name == "AECC_NETWORK")?;
    let common = doc.get_entity(network)?.common().clone();
    let refs: Vec<u64> = a.typed_handles().iter().map(|r| r.0).collect();
    let kind = |h: &&u64| arch_entity(doc, Handle::new(**h)).map(|e| e.dxf_name.clone());
    let parts = refs.get(8..)?;
    Some(Network {
        common,
        name: a.decode().strings.get(2)?.clone(),
        list: *refs.get(5)?,
        pipes: parts.iter().filter(|h| kind(h).as_deref() == Some("AECC_PIPE")).copied().collect(),
        structures: parts.iter().filter(|h| kind(h).as_deref() == Some("AECC_STRUCTURE")).copied().collect(),
    })
}

/// Adds an empty gravity pipe network named `name` on `layer` (layer "0"
/// when `None`) that uses the drawing's first parts list; a drawing
/// without one gets the application's default parts list.
pub fn add_pipe_network(doc: &mut CadDocument, name: &str, layer: Option<&str>) -> Result<Handle, DxfError> {
    if name.is_empty() {
        return Err(invalid("a pipe network needs a name"));
    }
    let list = ensure_parts_list(doc)?;
    let common = entity_common(doc, layer)?;
    ensure_name_template(doc, STRUCTURE_TEMPLATE)?;
    ensure_name_template(doc, PIPE_TEMPLATE)?;
    write_network(doc, common, name, list.value(), &[], &[])
}

/// Writes a part with `write` (given its entity data, references and
/// number in the network) and adds it to the network's lists.
fn place(doc: &mut CadDocument, network: Handle, part: &Part, structure: bool, write: impl FnOnce(&mut CadDocument, EntityCommon, Vec<Ref>, usize) -> Result<Handle, DxfError>) -> Result<Handle, DxfError> {
    let mut net = read_network(doc, network).ok_or_else(|| invalid("not a pipe network"))?;
    // ponytail: a drawing written from scratch (no format manager, root
    // key 0x4C, and few root settings) does not open once it holds a part;
    // what the parts need from the application's tree is not known, so a
    // drawing without the format manager is refused.
    if root_node(doc, KEY_FORMAT_MANAGER).is_none() {
        return Err(invalid("drawing lacks the application's settings that pipe network parts need"));
    }
    let (style, rules) = family_style(doc, Handle::new(net.list), part.family).ok_or_else(|| invalid("the network's parts list lacks the part's family"))?;
    if structure && !catalog_lists_structure_fields(doc, Handle::new(net.list)) {
        return Err(invalid("the parts list's catalog does not list the structure's fields yet"));
    }
    let definition = ensure_definition(doc, part)?;
    let common = entity_common(doc, Some(&net.common.layer))?;
    let refs = vec![
        hard_pointer(0),
        hard_pointer(0),
        soft_pointer(0),
        hard_owner(0),
        hard_pointer(definition.value()),
        hard_pointer(style),
        hard_pointer(network.value()),
        soft_pointer(0),
        soft_pointer(0),
        hard_pointer(0),
        hard_pointer(rules),
    ];
    let number = if structure { net.structures.len() } else { net.pipes.len() } + 1;
    let handle = write(doc, common, refs, number)?;
    if structure {
        net.structures.push(handle.value());
    } else {
        net.pipes.push(handle.value());
    }
    write_network(doc, net.common, &net.name, net.list, &net.pipes, &net.structures)?;
    Ok(handle)
}

/// Adds a null structure to `network` at `position`, turned by `rotation`
/// (radians) about Z. The structure record (`AECC_STRUCTURE`):
///
/// ```text
/// STRUCTURE_HEAD, 3BD insertion point, 3 x 3BD axes (X, Y, Z),
/// STRUCTURE_RECORD, 16-byte id, STRUCTURE_TAIL
/// references: hard 0, hard 0, soft 0, hard owner 0, part definition,
///   style, network, soft 0, soft 0, hard 0, rules (the style and rules of
///   the family's size in the parts list)
/// texts: STRUCTURE_STRINGS with the name "Structure - (n)"
/// ```
///
/// The outline and extents cached in the record are the application's for
/// a structure at the origin; the application recomputes them, the rim
/// and the sump from the insertion point.
pub fn add_structure(doc: &mut CadDocument, network: Handle, position: Vector3, rotation: f64) -> Result<Handle, DxfError> {
    if ![position.x, position.y, position.z, rotation].iter().all(|v| v.is_finite()) {
        return Err(invalid("a structure needs a finite position and rotation"));
    }
    place(doc, network, &NULL_STRUCTURE, true, |doc, common, refs, n| {
        let id = new_identifier(common.handle.value());
        let mut strings = texts(STRUCTURE_STRINGS);
        strings[6] = format!("Structure - ({n})");
        let (c, s) = (rotation.cos(), rotation.sin());
        put_entity(
            doc,
            "AECC_STRUCTURE",
            common,
            |w| {
                run(w, STRUCTURE_HEAD.0, STRUCTURE_HEAD.1);
                for v in [position.x, position.y, position.z, c, s, 0.0, -s, c, 0.0, 0.0, 0.0, 1.0] {
                    w.write_bit_double(v);
                }
                run(w, STRUCTURE_RECORD.0, STRUCTURE_RECORD.1);
                id.iter().for_each(|b| w.write_byte(*b));
                run(w, STRUCTURE_TAIL.0, STRUCTURE_TAIL.1);
            },
            Body { refs, strings },
        )
    })
}

/// Adds a 12 inch corrugated metal pipe from `start` to `end` to
/// `network`, not connected to structures. The pipe record (`AECC_PIPE`):
///
/// ```text
/// PIPE_HEAD, 16-byte id, PIPE_RECORD, 3BD start, 3BD end, PIPE_TAIL
/// references and texts as a structure's (name "Pipe - (n)")
/// ```
///
/// The application recomputes the length, slope and extents from the ends.
pub fn add_pipe(doc: &mut CadDocument, network: Handle, start: Vector3, end: Vector3) -> Result<Handle, DxfError> {
    if ![start.x, start.y, start.z, end.x, end.y, end.z].iter().all(|v| v.is_finite()) || start.distance(&end) < 1e-9 {
        return Err(invalid("a pipe needs two distinct finite ends"));
    }
    place(doc, network, &CORRUGATED_PIPE_12, false, |doc, common, refs, n| {
        let id = new_identifier(common.handle.value());
        let mut strings = texts(PIPE_STRINGS);
        strings[6] = format!("Pipe - ({n})");
        put_entity(
            doc,
            "AECC_PIPE",
            common,
            |w| {
                run(w, PIPE_HEAD.0, PIPE_HEAD.1);
                id.iter().for_each(|b| w.write_byte(*b));
                run(w, PIPE_RECORD.0, PIPE_RECORD.1);
                for v in [start.x, start.y, start.z, end.x, end.y, end.z] {
                    w.write_bit_double(v);
                }
                run(w, PIPE_TAIL.0, PIPE_TAIL.1);
            },
            Body { refs, strings },
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classes::DxfClass;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::DxfVersion;

    /// A parts list with the two families the writer places, as the
    /// application lays it out: list, family item (catalog id among its
    /// texts, size filter first), size filter (`0, style, rules`).
    fn with_parts_list(doc: &mut CadDocument) -> [u64; 4] {
        let root = ensure_civil(doc).unwrap();
        for name in ["AECC_NETWORK_PART_CATALOG_DEF_NODE", "AECC_NETWORK_PART_LIST", "AECC_NETWORK_PART_FAMILY_ITEM", "AECC_PART_SIZE_FILTER", "AECC_PIPE_STYLE"] {
            doc.classes.add_or_update(DxfClass::new(name, name));
        }
        let [catalog, list] = [(); 2].map(|_| doc.allocate_handle());
        // A part record with the null structure's fields.
        let records: String = STRUCTURE_CATALOG_FIELDS.iter().map(|f| f.1).collect();
        let mut strings = texts(&["", "AeccNetworkPartRecord", "", ""]);
        strings.extend(STRUCTURE_CATALOG_FIELDS.iter().flat_map(|f| texts(f.2)));
        put_object(doc, "AECC_NETWORK_PART_CATALOG_DEF_NODE", catalog, root, Vec::new(), |w| bits(w, &format!("{FIELD_MARK}01010100000101010000{:010b}10{records}", 0b01_0000_0000 | STRUCTURE_CATALOG_FIELDS.len())), Body { refs: vec![soft_owner(list.value())], strings }).unwrap();
        put_object(doc, "AECC_NETWORK_PART_LIST", list, catalog, Vec::new(), |_| {}, Body { refs: Vec::new(), strings: vec![String::new()] }).unwrap();
        // A stand-in format manager (the parts' guard looks for it).
        let manager = doc.allocate_handle();
        put_object(doc, "AECC_PIPE_STYLE", manager, root, Vec::new(), |_| {}, Body { refs: Vec::new(), strings: Vec::new() }).unwrap();
        set_root_entry(doc, KEY_FORMAT_MANAGER, manager).unwrap();
        let mut styles = [0; 4];
        for (i, family) in [NULL_STRUCTURE.family, CORRUGATED_PIPE_12.family].into_iter().enumerate() {
            let [item, filter, style, rules] = [(); 4].map(|_| doc.allocate_handle());
            for h in [style, rules] {
                put_object(doc, "AECC_PIPE_STYLE", h, root, Vec::new(), |_| {}, Body { refs: Vec::new(), strings: Vec::new() }).unwrap();
            }
            put_object(doc, "AECC_NETWORK_PART_FAMILY_ITEM", item, list, Vec::new(), |_| {}, Body { refs: vec![soft_owner(filter.value()), soft_pointer(0)], strings: vec![String::new(), family.to_string()] }).unwrap();
            put_object(doc, "AECC_PART_SIZE_FILTER", filter, item, Vec::new(), |_| {}, Body { refs: vec![soft_pointer(0), hard_pointer(style.value()), hard_pointer(rules.value())], strings: Vec::new() }).unwrap();
            styles[2 * i] = style.value();
            styles[2 * i + 1] = rules.value();
        }
        styles
    }

    #[test]
    fn networks_list_their_parts_and_parts_keep_their_places() {
        let v = Vector3::new;
        // A drawing without a parts list gets the default one and its
        // styles; parts are refused there.
        let mut fresh = CadDocument::new();
        fresh.version = DxfVersion::AC1032;
        let n0 = add_pipe_network(&mut fresh, "N", None).unwrap();
        let list = parts_list(&fresh).unwrap();
        assert_eq!(read_network(&fresh, n0).unwrap().list, list.value());
        assert_eq!(fresh.objects.values().filter(|o| matches!(o, ObjectType::Arch(a) if a.dxf_name == "AECC_NETWORK_PART_FAMILY_ITEM" && a.owner == list)).count(), 33);
        assert!(family_style(&fresh, list, NULL_STRUCTURE.family).is_some());
        assert!(add_structure(&mut fresh, n0, v(0.0, 0.0, 0.0), 0.0).is_err());
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let styles = with_parts_list(&mut doc);
        assert!(add_pipe_network(&mut doc, "", None).is_err());
        let n = add_pipe_network(&mut doc, "NW1", None).unwrap();
        let s1 = add_structure(&mut doc, n, v(10.0, 20.0, 3.0), 0.5).unwrap();
        let s2 = add_structure(&mut doc, n, v(100.0, 20.0, 2.0), 0.0).unwrap();
        let p1 = add_pipe(&mut doc, n, v(10.0, 20.0, -2.0), v(100.0, 20.0, -3.0)).unwrap();
        assert!(add_pipe(&mut doc, n, v(1.0, 1.0, 1.0), v(1.0, 1.0, 1.0)).is_err());
        assert!(add_pipe(&mut doc, s1, v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0)).is_err(), "not a network");
        let path = std::env::temp_dir().join(format!("civil-networks-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let net = read_network(&back, n).unwrap();
        assert_eq!(net.name, "NW1");
        assert_eq!(net.structures, vec![s1.value(), s2.value()]);
        assert_eq!(net.pipes, vec![p1.value()]);
        let s = arch_entity(&back, s1).unwrap();
        assert_eq!(s.structure_position(), Some(v(10.0, 20.0, 3.0)));
        assert!((s.network_part_rotation().unwrap() - 0.5).abs() < 1e-12);
        assert_eq!(s.decode().strings[6], "Structure - (1)");
        assert_eq!(arch_entity(&back, s2).unwrap().decode().strings[6], "Structure - (2)");
        let p = arch_entity(&back, p1).unwrap();
        assert_eq!(p.pipe_ends(), Some((v(10.0, 20.0, -2.0), v(100.0, 20.0, -3.0))));
        let refs: Vec<u64> = p.typed_handles().iter().map(|r| r.0).collect();
        assert_eq!((refs[5], refs[6], refs[10]), (styles[2], n.value(), styles[3]));
        // One part definition per size, keyed by it.
        let defs = back.named_objects_dictionary().and_then(|d| d.get(PART_DEFS)).unwrap();
        let ObjectType::Dictionary(d) = back.objects.get(&defs).unwrap() else { panic!() };
        assert_eq!(d.len(), 2);
        assert_eq!(d.get(CORRUGATED_PIPE_12.key), Some(Handle::new(refs[4])));
        assert_eq!(object(&back, Handle::new(refs[4])).unwrap().decode().strings[7], "Inner Pipe Diameter");
        // A structure needs the catalog node to list its fields.
        let mut bare = doc.clone();
        let list = parts_list(&bare).unwrap();
        let catalog = object(&bare, list).unwrap().owner;
        let civil_root = root(&bare).unwrap();
        put_object(&mut bare, "AECC_NETWORK_PART_CATALOG_DEF_NODE", catalog, civil_root, Vec::new(), |_| {}, Body { refs: vec![soft_owner(list.value())], strings: texts(&["", "AeccNetworkPartRecord", "", ""]) }).unwrap();
        assert!(add_structure(&mut bare, n, v(0.0, 0.0, 0.0), 0.0).is_err());
        assert!(add_pipe(&mut bare, n, v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0)).is_ok());
        // The name templates of structures and pipes, beside an assembly's.
        let mut with_group = doc.clone();
        ensure_name_template(&mut with_group, (0x0E80, "Group - (<[67112990(2449)]>)")).unwrap();
        let head = root_node(&with_group, 0x8C).unwrap();
        assert_eq!(object(&with_group, head).unwrap().typed_handles().len(), 4);
    }
}
