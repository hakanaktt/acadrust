//! Gravity pipe networks: the network, its structures and its pipes.
//!
//! A network part refers to a family of the drawing's parts list and to
//! the part definition (`AECC_NETWORK_PART_DEF`) of its size, which holds
//! the catalog data of that size. The writer places the parts list's null
//! structure and its 12 inch corrugated metal pipe, whose part definitions
//! it writes when the drawing lacks them.

use super::assembly::{ensure_name_template, entity_common, run};
use super::network_content::*;
use super::network_families::{PIPES, STRUCTURES};
use super::network_sizes::{PIPE_SIZES, SIZE_TEMPLATES};
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
/// Root key of the part catalog node, the parts lists' owner.
const KEY_PART_CATALOG: i32 = 0x136;
const STRUCTURE_TEMPLATE: (i32, &str) = (0x125, "Structure - (<[67112990(2449)]>)");
const PIPE_TEMPLATE: (i32, &str) = (0x126, "Pipe - (<[67112990(2449)]>)");

/// A part family the writer places (one size of it): its name, its id in
/// the catalog, the key of its size's part definition and that
/// definition's record and texts, a structure's texts and record runs
/// (the runs after its axes up to its identifier and after it), and the
/// body its definition's blocks hold.
pub(super) struct Family {
    pub(super) name: &'static str,
    pub(super) family: &'static str,
    pub(super) key: &'static str,
    pub(super) definition: (usize, &'static str),
    pub(super) definition_strings: &'static [&'static str],
    pub(super) strings: &'static [&'static str],
    pub(super) record: Option<((usize, &'static str), (usize, &'static str))>,
    pub(super) body: FamilyBody,
}

/// The body of a structure size, about the insertion point (its top).
// ponytail: the application's bodies are the catalog's generated solids;
// the writer writes a cylinder or box of the size's extents instead.
pub(super) enum FamilyBody {
    /// Pipes: none.
    None,
    /// A sphere of this radius, in one block the definition refers to twice.
    Sphere(f64),
    /// A cylinder down from the insertion point, in two blocks.
    Cylinder { radius: f64, depth: f64 },
    /// A box, in two blocks.
    Box { min: [f64; 3], max: [f64; 3] },
}

const NULL_STRUCTURE: Family = Family {
    name: "Null Structure",
    family: "D45435F0-874A-4754-BF5E-FE9D18BE870F",
    key: "D45435F0874A4754BF5EFE9D18BE870FSBSH0SBSD0",
    definition: STRUCTURE_DEFINITION,
    definition_strings: STRUCTURE_DEFINITION_STRINGS,
    strings: STRUCTURE_STRINGS,
    record: None,
    body: FamilyBody::Sphere(0.5),
};
const CORRUGATED_PIPE_12: Family = Family {
    name: "Corrugated Metal Pipe",
    family: "481DD6EF-B926-428E-99B7-A5BE30A17E59",
    key: "481DD6EFB926428E99B7A5BE30A17E591815E08D5B6A42B087113F4839A38301",
    definition: PIPE_DEFINITION,
    definition_strings: PIPE_DEFINITION_STRINGS,
    strings: PIPE_STRINGS,
    record: None,
    body: FamilyBody::None,
};

pub(super) fn texts(t: &[&str]) -> Vec<String> {
    t.iter().map(|s| s.to_string()).collect()
}

pub(super) fn hard_owner(h: u64) -> Ref {
    Ref(DwgReferenceType::HardOwnership, h)
}

pub(super) fn arch_entity(doc: &CadDocument, handle: Handle) -> Option<&ArchEntityData> {
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

/// The part definition of `part`, written when the drawing lacks it.
fn ensure_definition(doc: &mut CadDocument, part: &Family) -> Result<Handle, DxfError> {
    ensure_definition_with(doc, part.key, part.definition, part.definition_strings, |doc| body_refs(doc, &part.body))
}

/// The references a part definition makes to the blocks of `body`: none
/// for a pipe, one block twice for a sphere, two blocks otherwise.
pub(super) fn body_refs(doc: &mut CadDocument, body: &FamilyBody) -> Result<Vec<Ref>, DxfError> {
    use crate::entities::acis::primitives::{build_box, build_cylinder, build_sphere};
    let solid = || match *body {
        FamilyBody::None => None,
        FamilyBody::Sphere(r) => Some(build_sphere([0.0, 0.0, -r], r)),
        FamilyBody::Cylinder { radius, depth } => Some(build_cylinder([0.0, 0.0, -depth], radius, depth)),
        FamilyBody::Box { min, max } => Some(build_box([(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0, (min[2] + max[2]) / 2.0], max[0] - min[0], max[1] - min[1], max[2] - min[2])),
    };
    Ok(match body {
        FamilyBody::None => Vec::new(),
        FamilyBody::Sphere(_) => vec![hard_pointer(body_block(doc, solid())?.value()); 2],
        _ => vec![hard_pointer(body_block(doc, solid())?.value()), hard_pointer(body_block(doc, solid())?.value())],
    })
}

/// The part definition keyed `key`, written when the drawing lacks it, under
/// the named-objects entry `AECC_NETWORK_PART_DEFS` (a dictionary keyed by
/// size; gravity and pressure parts share it) with the references `refs`
/// makes. The record does not open with the version word.
pub(super) fn ensure_definition_with(
    doc: &mut CadDocument,
    key: &str,
    definition: (usize, &str),
    strings: &[&str],
    refs: impl FnOnce(&mut CadDocument) -> Result<Vec<Ref>, DxfError>,
) -> Result<Handle, DxfError> {
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
            if let Some(h) = d.get(key) {
                return Ok(h);
            }
        }
        _ => return Err(invalid("the part definitions entry is not a dictionary")),
    }
    let refs = refs(doc)?;
    let handle = doc.allocate_handle();
    put_plain_object(doc, "AECC_NETWORK_PART_DEF", handle, dict, vec![dict], |w| run(w, definition.0, definition.1), Body { refs, strings: texts(strings) })?;
    if let Some(ObjectType::Dictionary(d)) = doc.objects.get_mut(&dict) {
        d.add_entry(key, handle);
    }
    Ok(handle)
}

/// An anonymous block (`*U<n>`) holding a part's body: a solid sphere of
/// `radius` touching the insertion point from below, on layer 0. The
/// application places it at each part of the size and takes the part's
/// extents from it.
fn body_block(doc: &mut CadDocument, body: Option<crate::entities::acis::SatDocument>) -> Result<Handle, DxfError> {
    use crate::entities::{Block, BlockEnd, Solid3D};
    use crate::tables::BlockRecord;
    let name = (0..).map(|i| format!("*U{i}")).find(|n| !doc.block_records.contains(n)).unwrap_or_default();
    let (record, start, end) = (doc.allocate_handle(), doc.allocate_handle(), doc.allocate_handle());
    let mut br = BlockRecord::new(&name);
    br.handle = record;
    br.block_entity_handle = start;
    br.block_end_handle = end;
    br.flags.anonymous = true;
    doc.block_records.add(br).map_err(|_| invalid("cannot add the part body's block"))?;
    let mut marker = Block::new(&name, Vector3::ZERO);
    marker.common.handle = start;
    marker.common.owner_handle = record;
    doc.add_entity(EntityType::Block(marker))?;
    let mut solid = Solid3D::new();
    solid.common.owner_handle = record;
    if let Some(body) = body {
        solid.set_sat_document(&body);
    }
    doc.add_entity(EntityType::Solid3D(solid))?;
    let mut tail = BlockEnd::new();
    tail.common.handle = end;
    tail.common.owner_handle = record;
    doc.add_entity(EntityType::BlockEnd(tail))?;
    Ok(record)
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
pub(super) struct Network {
    common: EntityCommon,
    name: String,
    list: u64,
    pub pipes: Vec<u64>,
    pub structures: Vec<u64>,
}

pub(super) fn read_network(doc: &CadDocument, network: Handle) -> Option<Network> {
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
fn place(doc: &mut CadDocument, network: Handle, part: &Family, structure: bool, write: impl FnOnce(&mut CadDocument, EntityCommon, Vec<Ref>, usize) -> Result<Handle, DxfError>) -> Result<Handle, DxfError> {
    place_with(doc, network, part, structure, None, write)
}

/// [`place`] with the part definition `definition` (the family's own one
/// when `None`).
pub(super) fn place_with(doc: &mut CadDocument, network: Handle, part: &Family, structure: bool, definition: Option<Handle>, write: impl FnOnce(&mut CadDocument, EntityCommon, Vec<Ref>, usize) -> Result<Handle, DxfError>) -> Result<Handle, DxfError> {
    let mut net = read_network(doc, network).ok_or_else(|| invalid("not a pipe network"))?;
    let (style, rules) = family_style(doc, Handle::new(net.list), part.family).ok_or_else(|| invalid("the network's parts list lacks the part's family"))?;
    let definition = match definition {
        Some(h) => h,
        None => ensure_definition(doc, part)?,
    };
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
/// STRUCTURE_RECORD, identifier (see [`write_id`]), STRUCTURE_TAIL
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
                run_range(w, STRUCTURE_RECORD.1, 0, STRUCTURE_RECORD.0 - 2);
                write_id(w, &id);
                run_range(w, STRUCTURE_TAIL.1, 0, OUTLINE.0);
                // The cached plan outline: a circle of radius 0.5 about
                // the insertion point, four quarter arcs from the top
                // going counterclockwise (x, y, bulge, flag).
                let bulge = (std::f64::consts::PI / 8.0).tan();
                for (dx, dy) in [(0.0, 0.5), (-0.5, 0.0), (0.0, -0.5), (0.5, 0.0)] {
                    for v in [position.x + dx, position.y + dy, bulge] {
                        w.write_2bits(0);
                        w.write_raw_double(v);
                    }
                    w.write_bit(false);
                }
                run_range(w, STRUCTURE_TAIL.1, OUTLINE.1, STRUCTURE_TAIL.0);
            },
            Body { refs, strings },
        )
    })
}

/// Names of the structure and pipe families the writer places (one size
/// each, the size the application's default parts list gives a new part).
pub fn network_part_families() -> Vec<&'static str> {
    std::iter::once(NULL_STRUCTURE.name).chain(STRUCTURES.iter().map(|f| f.name)).chain(PIPES.iter().map(|f| f.name)).collect()
}

/// Adds a structure of the family `name` (one of
/// [`network_part_families`]) to `network` at `position`, turned by
/// `rotation` (radians) about Z. A family other than the null structure
/// is written as the application writes it at the origin:
///
/// ```text
/// STRUCTURE_HEAD, 3BD insertion point, 3 x 3BD axes, the family's run up
/// to the identifier, identifier, the family's run after it (part record
/// and the outline and extents the application caches for the origin)
/// texts: the family's, with the name "Structure - (n)"
/// ```
///
/// The application recomputes the cached outline, extents, rim and sump
/// from the insertion point when it opens the drawing.
pub fn add_structure_of_family(doc: &mut CadDocument, network: Handle, name: &str, position: Vector3, rotation: f64) -> Result<Handle, DxfError> {
    if name == NULL_STRUCTURE.name {
        return add_structure(doc, network, position, rotation);
    }
    let family = STRUCTURES.iter().find(|f| f.name == name).ok_or_else(|| invalid("no structure family of that name"))?;
    family.record.ok_or_else(|| invalid("the family has no structure record"))?;
    if ![position.x, position.y, position.z, rotation].iter().all(|v| v.is_finite()) {
        return Err(invalid("a structure needs a finite position and rotation"));
    }
    place(doc, network, family, true, |doc, common, refs, n| write_structure_record(doc, family, common, refs, n, position, rotation))
}

/// The record of a structure of `family` (not the null structure): see
/// [`add_structure_of_family`].
pub(super) fn write_structure_record(doc: &mut CadDocument, family: &Family, common: EntityCommon, refs: Vec<Ref>, n: usize, position: Vector3, rotation: f64) -> Result<Handle, DxfError> {
    let (before, after) = family.record.ok_or_else(|| invalid("the family has no structure record"))?;
    let id = new_identifier(common.handle.value());
    let mut strings = texts(family.strings);
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
            run(w, before.0, before.1);
            write_id(w, &id);
            run(w, after.0, after.1);
        },
        Body { refs, strings },
    )
}

/// Adds a pipe of the family `name` (one of [`network_part_families`]) from
/// `start` to `end` to `network`, not connected to structures; its record
/// is a corrugated metal pipe's (see [`add_pipe`]) with the family's name.
pub fn add_pipe_of_family(doc: &mut CadDocument, network: Handle, name: &str, start: Vector3, end: Vector3) -> Result<Handle, DxfError> {
    let family = PIPES.iter().find(|f| f.name == name).ok_or_else(|| invalid("no pipe family of that name"))?;
    write_pipe(doc, network, family, start, end)
}

/// A pipe size of the catalog (see [`add_pipe_of_size`]): its family's
/// name, catalog row, size name, the template its part definition starts
/// from (an index into `SIZE_TEMPLATES`; none for the family's own record)
/// and the changes to that definition: (bit, bits removed, bits inserted,
/// their hex).
pub(super) struct PipeSize {
    pub(super) family: &'static str,
    pub(super) row: &'static str,
    pub(super) name: &'static str,
    pub(super) template: Option<usize>,
    pub(super) patch: &'static [(u16, u16, u16, &'static str)],
}

/// A part definition a pipe size starts from where its family's own record
/// lists its fields in another order (the application writes the sizes it
/// adds to a family that way).
pub(super) struct PipeSizeTemplate {
    pub(super) family: &'static str,
    pub(super) key_suffix: &'static str,
    pub(super) definition: (usize, &'static str),
    pub(super) definition_strings: &'static [&'static str],
}

/// The pipe sizes of the default catalog for the pipe family `family` (one
/// of [`network_part_families`]), by the names the application gives them
/// ("15 inch Corrugated Metal Pipe", "36 x 24 inch Concrete Box Culvert").
pub fn network_pipe_sizes(family: &str) -> Vec<&'static str> {
    PIPE_SIZES.iter().filter(|s| s.family == family).map(|s| s.name).collect()
}

/// Adds a pipe of the catalog size `size` (one of [`network_pipe_sizes`])
/// from `start` to `end` to `network`, not connected to structures; the
/// parts list need not hold the size. Its part definition, keyed by family
/// id, catalog row and the family's flow and material fields:
///
/// ```text
/// the family's definition with the size's changes: the size's values
/// (inner diameter or width and height, wall thickness, body sizes) and
/// the cross section geometry the catalog computes from them
/// texts: the family's, with the row id and the size name
/// ```
pub fn add_pipe_of_size(doc: &mut CadDocument, network: Handle, size: &str, start: Vector3, end: Vector3) -> Result<Handle, DxfError> {
    let sized = PIPE_SIZES.iter().find(|s| s.name == size).ok_or_else(|| invalid("no pipe size of that name"))?;
    let family = PIPES.iter().find(|f| f.name == sized.family).ok_or_else(|| invalid("no pipe family of that name"))?;
    let (suffix, (bits, hex), template_strings) = match sized.template {
        Some(i) => {
            let t = &SIZE_TEMPLATES[i];
            (t.key_suffix, t.definition, t.definition_strings)
        }
        None => (&family.key[64..], family.definition, family.definition_strings),
    };
    let key = format!("{}{}{suffix}", family.family.replace('-', ""), sized.row.replace('-', ""));
    let definition = patched(bits, hex, sized.patch);
    let mut strings: Vec<&str> = template_strings.to_vec();
    for (context, value) in [("Catalog_BasicTableRowID", sized.row), ("Catalog_PartSizeName", sized.name)] {
        let i = strings.iter().position(|s| *s == context).ok_or_else(|| invalid("the family's definition lacks a size field"))?;
        strings[i + 2] = value;
    }
    let definition = ensure_definition_with(doc, &key, (definition.0, &definition.1), &strings, |_| Ok(Vec::new()))?;
    place_with(doc, network, family, false, Some(definition), |doc, common, refs, n| write_pipe_record(doc, family, common, refs, n, start, end))
}

/// The `bits` long run `hex` with `patch` applied (in bit order): at each
/// bit, the bits removed replaced by those inserted.
fn patched(bits: usize, hex: &str, patch: &[(u16, u16, u16, &str)]) -> (usize, String) {
    let bit = |h: &str, i: usize| h.as_bytes().get(i / 4).and_then(|c| (*c as char).to_digit(16)).is_some_and(|v| v >> (3 - i % 4) & 1 == 1);
    let mut out: Vec<bool> = Vec::with_capacity(bits + 512);
    let mut at = 0;
    for (pos, removed, inserted, ins) in patch {
        out.extend((at..*pos as usize).map(|i| bit(hex, i)));
        out.extend((0..*inserted as usize).map(|i| bit(ins, i)));
        at = *pos as usize + *removed as usize;
    }
    out.extend((at..bits).map(|i| bit(hex, i)));
    let hex = out.chunks(4).map(|c| char::from_digit(c.iter().enumerate().fold(0, |v, (i, b)| v | (u32::from(*b) << (3 - i))), 16).unwrap_or('0')).collect();
    (out.len(), hex)
}

/// Adds a 12 inch corrugated metal pipe from `start` to `end` to
/// `network`, not connected to structures. The pipe record (`AECC_PIPE`):
///
/// ```text
/// PIPE_HEAD, identifier (see [`write_id`]), PIPE_RECORD, 3BD start,
/// 3BD end, PIPE_TAIL
/// references and texts as a structure's (name "Pipe - (n)")
/// ```
///
/// The application recomputes the length, slope and extents from the ends.
pub fn add_pipe(doc: &mut CadDocument, network: Handle, start: Vector3, end: Vector3) -> Result<Handle, DxfError> {
    write_pipe(doc, network, &CORRUGATED_PIPE_12, start, end)
}

fn write_pipe(doc: &mut CadDocument, network: Handle, family: &Family, start: Vector3, end: Vector3) -> Result<Handle, DxfError> {
    if ![start.x, start.y, start.z, end.x, end.y, end.z].iter().all(|v| v.is_finite()) || start.distance(&end) < 1e-9 {
        return Err(invalid("a pipe needs two distinct finite ends"));
    }
    place(doc, network, family, false, |doc, common, refs, n| write_pipe_record(doc, family, common, refs, n, start, end))
}

fn write_pipe_record(doc: &mut CadDocument, family: &Family, common: EntityCommon, refs: Vec<Ref>, n: usize, start: Vector3, end: Vector3) -> Result<Handle, DxfError> {
    {
        let id = new_identifier(common.handle.value());
        let mut strings = texts(PIPE_STRINGS);
        strings[2] = family.name.to_string();
        strings[6] = format!("Pipe - ({n})");
        put_entity(
            doc,
            "AECC_PIPE",
            common,
            |w| {
                run_range(w, PIPE_HEAD.1, 0, PIPE_HEAD.0 - 2);
                write_id(w, &id);
                run(w, PIPE_RECORD.0, PIPE_RECORD.1);
                for v in [start.x, start.y, start.z, end.x, end.y, end.z] {
                    w.write_bit_double(v);
                }
                run(w, PIPE_TAIL.0, PIPE_TAIL.1);
            },
            Body { refs, strings },
        )
    }
}

/// A part's identifier: a version-4 GUID (Windows byte order), then the
/// two bits `10`. The constant run before it (`PIPE_HEAD`,
/// `STRUCTURE_RECORD`) is written without its last two bits, the start of
/// the sample's own identifier. Any other two bits after the GUID make the
/// application regenerate the drawing endlessly or discard the part.
fn write_id(w: &mut crate::io::dwg::dwg_stream_writers::DwgMergedWriter, id: &[u8; 16]) {
    id.iter().for_each(|b| w.write_byte(*b));
    w.write_bit(true);
    w.write_bit(false);
}

/// Bit range of the cached plan outline in `STRUCTURE_TAIL`.
const OUTLINE: (usize, usize) = (473, 1269);

/// Bits `from..to` of the constant run `hex`.
pub(super) fn run_range(w: &mut crate::io::dwg::dwg_stream_writers::DwgMergedWriter, hex: &str, from: usize, to: usize) {
    for i in from..to {
        let v = hex.as_bytes().get(i / 4).and_then(|c| (*c as char).to_digit(16)).unwrap_or(0);
        w.write_bit(v >> (3 - i % 4) & 1 == 1);
    }
}

/// A record's reference as written, from its kind code.
pub(super) fn typed(r: (u64, u8)) -> Ref {
    let kind = match r.1 {
        2 => DwgReferenceType::SoftOwnership,
        3 => DwgReferenceType::HardOwnership,
        4 => DwgReferenceType::SoftPointer,
        5 => DwgReferenceType::HardPointer,
        _ => DwgReferenceType::Undefined,
    };
    Ref(kind, r.0)
}

/// Index of a part's first connector reference: after hard 0, hard 0,
/// soft 0, hard owner 0, part definition and style.
const FIRST_CONNECTOR: usize = 6;

/// A pipe or structure with its connector count raised to `connectors.len()`
/// and `connectors` as its connector references (after the style),
/// rewriting the record's count field and keeping every other bit.
fn set_connectors(doc: &mut CadDocument, part: Handle, structure: bool, connectors: &[u64]) -> Result<(), DxfError> {
    let refused = || invalid("the part's record is not laid out as a pipe or structure");
    let a = arch_entity(doc, part).ok_or_else(refused)?;
    let (data, start, end) = a.main_stream().ok_or_else(refused)?;
    let (at, after, count) = crate::classes::civil::network_connector_count(data, start, end, structure).ok_or_else(refused)?;
    let count = usize::try_from(count).map_err(|_| refused())?;
    let mut refs: Vec<Ref> = a.typed_handles().into_iter().map(typed).collect();
    (refs.len() >= FIRST_CONNECTOR + count).then_some(()).ok_or_else(refused)?;
    refs.splice(FIRST_CONNECTOR..FIRST_CONNECTOR + count, connectors.iter().map(|h| hard_pointer(*h)));
    let bit = |p: usize| data[p / 8] >> (7 - p % 8) & 1 == 1;
    let (head, tail): (Vec<bool>, Vec<bool>) = ((start..at).map(bit).collect(), (after..end).map(bit).collect());
    let strings = a.decode().strings;
    let dxf = a.dxf_name.clone();
    let common = doc.get_entity(part).ok_or_else(refused)?.common().clone();
    put_entity(
        doc,
        &dxf,
        common,
        |w| {
            head.iter().for_each(|b| w.write_bit(*b));
            w.write_bit_long(connectors.len() as i32);
            tail.iter().for_each(|b| w.write_bit(*b));
        },
        Body { refs, strings },
    )?;
    Ok(())
}

/// The connector references of a part (see [`set_connectors`]).
fn connectors_of(doc: &CadDocument, part: Handle, structure: bool) -> Option<Vec<u64>> {
    let a = arch_entity(doc, part)?;
    let (data, start, end) = a.main_stream()?;
    let (.., count) = crate::classes::civil::network_connector_count(data, start, end, structure)?;
    let refs = a.typed_handles();
    Some(refs.get(FIRST_CONNECTOR..FIRST_CONNECTOR + usize::try_from(count).ok()?)?.iter().map(|r| r.0).collect())
}

/// A connector (`AECC_NETWORK_PART_CONNECTOR`, an entity on the network's
/// layer) of `part`, paired with the connector `pair` of the part it meets
/// (0 for an open pipe end):
///
/// ```text
/// CONNECTOR (constant)
/// references: soft 0, hard owner 0, pair (soft), part (hard)
/// texts: "AeccImpNetworkPartConnector", "", ""
/// ```
fn write_connector(doc: &mut CadDocument, handle: Handle, layer: &str, pair: u64, part: Handle) -> Result<Handle, DxfError> {
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let common = EntityCommon { handle, owner_handle: doc.header.model_space_block_handle, layer: layer.into(), layer_handle: Some(layer_handle), ..EntityCommon::default() };
    put_entity(
        doc,
        "AECC_NETWORK_PART_CONNECTOR",
        common,
        |w| run(w, CONNECTOR.0, CONNECTOR.1),
        Body { refs: vec![soft_pointer(0), hard_owner(0), soft_pointer(pair), hard_pointer(part.value())], strings: texts(CONNECTOR_STRINGS) },
    )
}

/// Connects the ends of `pipe` to the structures `start` and `end` (an end
/// given `None` stays open). The pipe must have no connections yet and the
/// structures must be in its network.
///
/// A connected pipe has two connectors, start then end; a structure has
/// one per pipe it meets, in the order the pipes were connected. Each
/// connector is paired with the one of the part it meets; an open end's
/// connector is paired with none. The structures keep the positions they
/// have: the application places a connected structure on the lowest pipe
/// end it meets when the network is edited.
pub fn connect_pipe(doc: &mut CadDocument, pipe: Handle, start: Option<Handle>, end: Option<Handle>) -> Result<(), DxfError> {
    let p = arch_entity(doc, pipe).filter(|a| a.dxf_name == "AECC_PIPE").ok_or_else(|| invalid("not a pipe"))?;
    let network = p.typed_handles().get(6).map(|r| r.0);
    if start.is_none() && end.is_none() {
        return Err(invalid("a connection needs a structure"));
    }
    if start.is_some() && start == end {
        return Err(invalid("a pipe cannot meet one structure at both ends"));
    }
    if connectors_of(doc, pipe, false).ok_or_else(|| invalid("the pipe's record is not laid out as a pipe"))?.len() != 0 {
        return Err(invalid("the pipe is connected already"));
    }
    for s in [start, end].into_iter().flatten() {
        let st = arch_entity(doc, s).filter(|a| a.dxf_name == "AECC_STRUCTURE").ok_or_else(|| invalid("not a structure"))?;
        let at = connectors_of(doc, s, true).ok_or_else(|| invalid("the structure's record is not laid out as a structure"))?.len();
        if st.typed_handles().get(FIRST_CONNECTOR + at).map(|r| r.0) != network {
            return Err(invalid("the structure is not in the pipe's network"));
        }
    }
    let layer = doc.get_entity(pipe).map(|e| e.common().layer.clone()).unwrap_or_default();
    let ends = [doc.allocate_handle(), doc.allocate_handle()];
    for (own, structure) in ends.into_iter().zip([start, end]) {
        let pair = match structure {
            Some(s) => {
                let other = doc.allocate_handle();
                write_connector(doc, other, &layer, own.value(), s)?;
                let mut list = connectors_of(doc, s, true).unwrap_or_default();
                list.push(other.value());
                set_connectors(doc, s, true, &list)?;
                other.value()
            }
            None => 0,
        };
        write_connector(doc, own, &layer, pair, pipe)?;
    }
    set_connectors(doc, pipe, false, &[ends[0].value(), ends[1].value()])
}

/// Names a pipe or structure `name` (what the drawing's part list shows).
/// The writer names parts `Pipe - (n)` / `Structure - (n)` by their place
/// in the network; the application may rename a part afterwards (it
/// suffixes a duplicate with ` (1)`) — this writes that name.
///
/// The name goes to the slot the record's own layout pins: the text after
/// the record's `AeccNetworkPartDataField` groups, the one slot rule the
/// read and the write side share. Records this crate writes list no
/// fields, so the name
/// stays at `strings[6]`; the application's carriers hold it past their
/// field groups (Pipe Networks-3B census: no-group pipes with an extra
/// description text `strings[7]`, three number-field groups `strings[25]`,
/// a headwall's one text-field group `strings[13]`), so a re-saved
/// app-written record keeps the name in its slot and a renamed part can be
/// renamed again whatever text its slot holds. A record whose layout does
/// not pin the slot is refused: the slot is never invented.
///
/// The record keeps every bit; only the name text changes. Names are not
/// checked for uniqueness: the application resolves a duplicate on open.
pub fn set_network_part_name(doc: &mut CadDocument, part: Handle, name: &str) -> Result<(), DxfError> {
    let refused = || invalid("not a pipe network part");
    if name.trim().is_empty() {
        return Err(invalid("a part name needs text"));
    }
    let a = arch_entity(doc, part).filter(|a| a.dxf_name == "AECC_PIPE" || a.dxf_name == "AECC_STRUCTURE").ok_or_else(refused)?;
    let (data, start, end) = a.main_stream().ok_or_else(refused)?;
    let mut strings = a.decode().strings;
    let structure = a.dxf_name == "AECC_STRUCTURE";
    let slot = super::super::network::part_name_slot(&data, start, end, &strings, structure).ok_or_else(refused)?;
    strings[slot] = name.to_string();
    let dxf = a.dxf_name.clone();
    let common = doc.get_entity(part).ok_or_else(refused)?.common().clone();
    let bits: Vec<bool> = (start..end).map(|p| data[p / 8] >> (7 - p % 8) & 1 == 1).collect();
    let refs: Vec<Ref> = a.typed_handles().into_iter().map(typed).collect();
    put_entity(doc, &dxf, common, |w| bits.iter().for_each(|b| w.write_bit(*b)), Body { refs, strings })?;
    Ok(())
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
        put_object(doc, "AECC_NETWORK_PART_CATALOG_DEF_NODE", catalog, root, Vec::new(), |_| {}, Body { refs: vec![soft_owner(list.value())], strings: Vec::new() }).unwrap();
        put_object(doc, "AECC_NETWORK_PART_LIST", list, catalog, Vec::new(), |_| {}, Body { refs: Vec::new(), strings: vec![String::new()] }).unwrap();
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
        // styles.
        let mut fresh = CadDocument::new();
        fresh.version = DxfVersion::AC1032;
        let n0 = add_pipe_network(&mut fresh, "N", None).unwrap();
        let list = parts_list(&fresh).unwrap();
        assert_eq!(read_network(&fresh, n0).unwrap().list, list.value());
        assert_eq!(fresh.objects.values().filter(|o| matches!(o, ObjectType::Arch(a) if a.dxf_name == "AECC_NETWORK_PART_FAMILY_ITEM" && a.owner == list)).count(), 33);
        assert!(family_style(&fresh, list, NULL_STRUCTURE.family).is_some());
        let s0 = add_structure(&mut fresh, n0, v(0.0, 0.0, 0.0), 0.0).unwrap();
        // A part's identifier (365 bits in for a structure at the origin):
        // a version-4 GUID, then `10`.
        let a = arch_entity(&fresh, s0).unwrap();
        let (d, start, end) = a.main_stream().unwrap();
        let bits: String = (start..end).map(|i| if d[i / 8] >> (7 - i % 8) & 1 == 1 { '1' } else { '0' }).collect();
        assert_eq!((&bits[365 + 56..365 + 60], &bits[365 + 64..365 + 66], &bits[365 + 128..365 + 130]), ("0100", "10", "10"));
        // The size's body: an anonymous block the definition refers to twice.
        let def = Handle::new(a.typed_handles()[4].0);
        let body = object(&fresh, def).unwrap().typed_handles();
        assert_eq!(body.len(), 2);
        let block = fresh.block_records.iter().find(|b| b.handle.value() == body[0].0).unwrap();
        assert!(block.name.starts_with("*U") && block.flags.anonymous);
        assert!(block.entity_handles.iter().any(|h| matches!(fresh.get_entity(*h), Some(EntityType::Solid3D(_)))));
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
        // Connections: two connectors on the pipe (start, end), one per
        // pipe on a structure, each paired with the other part's.
        let p2 = add_pipe(&mut doc, n, v(100.0, 20.0, -3.0), v(150.0, 20.0, -4.0)).unwrap();
        assert!(connect_pipe(&mut doc, p1, None, None).is_err());
        assert!(connect_pipe(&mut doc, p1, Some(s1), Some(s1)).is_err());
        assert!(connect_pipe(&mut doc, p1, Some(p2), None).is_err(), "not a structure");
        connect_pipe(&mut doc, p1, Some(s1), Some(s2)).unwrap();
        connect_pipe(&mut doc, p2, Some(s2), None).unwrap();
        assert!(connect_pipe(&mut doc, p1, Some(s1), None).is_err(), "connected already");
        let path = std::env::temp_dir().join(format!("civil-networks-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let net = read_network(&back, n).unwrap();
        assert_eq!(net.name, "NW1");
        assert_eq!(net.structures, vec![s1.value(), s2.value()]);
        assert_eq!(net.pipes, vec![p1.value(), p2.value()]);
        let s = arch_entity(&back, s1).unwrap();
        assert_eq!(s.structure_position(), Some(v(10.0, 20.0, 3.0)));
        assert!((s.network_part_rotation().unwrap() - 0.5).abs() < 1e-12);
        assert_eq!(s.decode().strings[6], "Structure - (1)");
        assert_eq!(arch_entity(&back, s2).unwrap().decode().strings[6], "Structure - (2)");
        let p = arch_entity(&back, p1).unwrap();
        assert_eq!(p.pipe_ends(), Some((v(10.0, 20.0, -2.0), v(100.0, 20.0, -3.0))));
        let refs: Vec<u64> = p.typed_handles().iter().map(|r| r.0).collect();
        assert_eq!((refs[5], refs[8], refs[12]), (styles[2], n.value(), styles[3]));
        let pair = |c: u64| arch_entity(&back, Handle::new(c)).unwrap().typed_handles().iter().map(|r| r.0).collect::<Vec<u64>>();
        let ends = connectors_of(&back, p1, false).unwrap();
        assert_eq!(ends.len(), 2);
        assert_eq!(connectors_of(&back, s1, true).unwrap(), vec![pair(ends[0])[2]]);
        assert_eq!(pair(pair(ends[0])[2])[3], s1.value());
        let at_s2 = connectors_of(&back, s2, true).unwrap();
        assert_eq!(at_s2, vec![pair(ends[1])[2], pair(connectors_of(&back, p2, false).unwrap()[0])[2]]);
        assert_eq!(pair(connectors_of(&back, p2, false).unwrap()[1])[2], 0, "open end");
        assert_eq!(arch_entity(&back, s2).unwrap().structure_position(), Some(v(100.0, 20.0, 2.0)));
        // One part definition per size, keyed by it.
        let defs = back.named_objects_dictionary().and_then(|d| d.get(PART_DEFS)).unwrap();
        let ObjectType::Dictionary(d) = back.objects.get(&defs).unwrap() else { panic!() };
        assert_eq!(d.len(), 2);
        assert_eq!(d.get(CORRUGATED_PIPE_12.key), Some(Handle::new(refs[4])));
        assert_eq!(object(&back, Handle::new(refs[4])).unwrap().decode().strings[7], "Inner Pipe Diameter");
        // The name templates of structures and pipes, beside an assembly's.
        let mut with_group = doc.clone();
        ensure_name_template(&mut with_group, (0x0E80, "Group - (<[67112990(2449)]>)")).unwrap();
        let head = root_node(&with_group, 0x8C).unwrap();
        assert_eq!(object(&with_group, head).unwrap().typed_handles().len(), 4);
    }

    #[test]
    fn structures_and_pipes_of_other_families() {
        let v = Vector3::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let n = add_pipe_network(&mut doc, "NF", None).unwrap();
        assert!(network_part_families().contains(&"Concentric Cylindrical Structure"));
        let s = add_structure_of_family(&mut doc, n, "Concentric Cylindrical Structure", v(50.0, 0.0, 0.0), 0.0).unwrap();
        let t = add_structure_of_family(&mut doc, n, "Rectangular Junction Structure NF", v(100.0, 0.0, 0.0), 0.0).unwrap();
        let p = add_pipe_of_family(&mut doc, n, "PVC Pipe", v(50.0, 0.0, -4.0), v(100.0, 0.0, -4.5)).unwrap();
        assert!(add_structure_of_family(&mut doc, n, "PVC Pipe", v(0.0, 0.0, 0.0), 0.0).is_err());
        assert!(add_pipe_of_family(&mut doc, n, "Null Structure", v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0)).is_err());
        connect_pipe(&mut doc, p, Some(s), Some(t)).unwrap();
        let path = std::env::temp_dir().join(format!("civil-families-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(arch_entity(&back, s).unwrap().structure_position(), Some(v(50.0, 0.0, 0.0)));
        assert_eq!(arch_entity(&back, t).unwrap().structure_position(), Some(v(100.0, 0.0, 0.0)));
        let pipe = arch_entity(&back, p).unwrap();
        assert_eq!(pipe.pipe_ends(), Some((v(50.0, 0.0, -4.0), v(100.0, 0.0, -4.5))));
        assert_eq!(pipe.decode().strings[2], "PVC Pipe");
        // Each family's size has its own definition, a structure's with two body blocks.
        let def = object(&back, Handle::new(arch_entity(&back, s).unwrap().typed_handles()[4].0)).unwrap();
        let blocks: Vec<u64> = def.typed_handles().iter().map(|r| r.0).filter(|h| back.block_records.iter().any(|b| b.handle.value() == *h)).collect();
        assert_eq!(blocks.len(), 2);
        assert_ne!(blocks[0], blocks[1]);
    }

    #[test]
    fn pipes_of_catalog_sizes_get_their_own_part_definitions() {
        // A patch replaces, removes and inserts bits in order.
        assert_eq!(patched(8, "a5", &[(1, 2, 1, "8"), (6, 0, 4, "f")]), (11, "cfa".to_string()));
        let v = Vector3::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let n = add_pipe_network(&mut doc, "N", None).unwrap();
        let sizes = network_pipe_sizes("Corrugated Metal Pipe");
        assert_eq!(sizes.len(), 25);
        assert!(network_pipe_sizes("Concrete Box Culvert").contains(&"36 x 24 inch Concrete Box Culvert"));
        assert!(add_pipe_of_size(&mut doc, n, "13 inch Corrugated Metal Pipe", v(0.0, 0.0, 0.0), v(1.0, 0.0, 0.0)).is_err());
        let p = add_pipe_of_size(&mut doc, n, "15 inch Corrugated Metal Pipe", v(0.0, 0.0, -5.0), v(100.0, 0.0, -6.0)).unwrap();
        let q = add_pipe_of_size(&mut doc, n, "15 inch Corrugated Metal Pipe", v(0.0, 20.0, -5.0), v(100.0, 20.0, -6.0)).unwrap();
        let path = std::env::temp_dir().join(format!("civil-pipe-sizes-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        // Both pipes share the size's definition, keyed by family and row.
        let defs = match back.named_objects_dictionary().and_then(|d| d.get(PART_DEFS)).and_then(|h| back.objects.get(&h)) {
            Some(ObjectType::Dictionary(d)) => d.clone(),
            _ => panic!("no part definitions"),
        };
        let key = "481DD6EFB926428E99B7A5BE30A17E598AEAAAC3CFFE4D85B96403680E46A993";
        let def = defs.get(key).unwrap();
        for pipe in [p, q] {
            assert_eq!(Handle::new(arch_entity(&back, pipe).unwrap().typed_handles()[4].0), def);
        }
        let texts = object(&back, def).unwrap().decode().strings;
        assert!(texts.iter().any(|t| t == "15 inch Corrugated Metal Pipe"));
        assert!(texts.iter().any(|t| t == "8AEAAAC3-CFFE-4D85-B964-03680E46A993"));
        // The inner diameter field (a raw double 154 bits in) holds the
        // size's 15 inches.
        let (data, start, _) = object(&back, def).unwrap().main_stream().unwrap();
        let bytes: Vec<u8> = (0..8).map(|k| (0..8).fold(0u8, |b, i| b << 1 | (data[(start + 154 + 8 * k + i) / 8] >> (7 - (start + 154 + 8 * k + i) % 8) & 1))).collect();
        assert_eq!(f64::from_le_bytes(bytes.try_into().unwrap()), 15.0);
    }

    /// Bit-string helpers for application-shaped carrier records (the
    /// encodings the read side's module docs describe).
    fn bl(v: u32) -> String {
        match v {
            0 => "10".to_string(),
            1..=255 => format!("01{v:08b}"),
            _ => format!("00{}", v.to_le_bytes().iter().map(|b| format!("{b:08b}")).collect::<String>()),
        }
    }

    fn bs(v: u16) -> String {
        match v {
            0 => "10".to_string(),
            1..=255 => format!("01{v:08b}"),
            _ => format!("00{}", v.to_le_bytes().iter().map(|b| format!("{b:08b}")).collect::<String>()),
        }
    }

    fn bd(v: f64) -> String {
        format!("00{}", v.to_le_bytes().iter().map(|b| format!("{b:08b}")).collect::<String>())
    }

    /// One part-record field after the list's `BL 0`: kind, the two bits,
    /// the type and its value bits.
    fn field(kind: u16, two: &str, ty: u8, value: &str) -> String {
        format!("{}{}{}{}{}{two}{}{}{}{value}{}", bl(0), bl(80), bs(0x04BE), bl(0), bs(kind), bl(0), "00001000", format!("{ty:08b}"), bl(1))
    }

    /// A carrier record's head: class header without keys, the 3-bit tag
    /// and the GUID.
    fn part_head() -> String {
        format!("{}{}{}{}{}{}01010100000000010{}", bl(80), bl(0), bl(80), bl(80), bl(0), bl(0), "0".repeat(128))
    }

    /// A structure's part record whose field list holds `fields` (`n`
    /// fields).
    fn structure_record(fields: &str, n: u32) -> String {
        format!("{}{}{}{}{}{}{}{fields}{}{}", part_head(), "0".repeat(794), bl(80), bs(0x04BE), bl(80), bl(80), bl(n), bl(0), "0".repeat(256))
    }

    /// A pipe's part record whose field list holds `fields` (`n` fields).
    fn pipe_record(fields: &str, n: u32) -> String {
        format!("{}{}{}{}{}{}{}{fields}{}{}", part_head(), "0".repeat(26), bl(80), bs(0x04BE), bl(80), bl(80), bl(n), bl(0), "0".repeat(256))
    }

    /// A bit string as written.
    fn bits_of(bits: &str) -> Vec<bool> {
        bits.chars().map(|c| c == '1').collect()
    }

    /// One `AeccNetworkPartDataField` group of a text stream.
    fn group(code: &str, desc: &str, context: &str, unit: &str, text: Option<&str>) -> Vec<String> {
        let mut g: Vec<String> = ["AeccNetworkPartDataField", "", code, desc, context, unit].map(String::from).to_vec();
        g.extend(text.map(String::from));
        g
    }

    /// Re-bodies a part with an application-shaped record: `bits` and
    /// `strings` as given, references kept.
    fn rebody(doc: &mut CadDocument, part: Handle, bits: Vec<bool>, strings: Vec<String>) {
        let a = arch_entity(doc, part).unwrap();
        let refs: Vec<Ref> = a.typed_handles().into_iter().map(typed).collect();
        let dxf = a.dxf_name.clone();
        let common = doc.get_entity(part).unwrap().common().clone();
        put_entity(doc, &dxf, common, |w| bits.iter().for_each(|b| w.write_bit(*b)), Body { refs, strings }).unwrap();
    }

    /// A record's main-stream bits as written.
    fn record_bits(doc: &CadDocument, part: Handle) -> Vec<bool> {
        let a = arch_entity(doc, part).unwrap();
        let (data, start, end) = a.main_stream().unwrap();
        (start..end).map(|p| data[p / 8] >> (7 - p % 8) & 1 == 1).collect()
    }

    /// The application's carriers hold the name where the record's layout
    /// pins it — past the `AeccNetworkPartDataField` groups its field list
    /// frames (Pipe Networks-3B census: a no-group pipe with an extra
    /// description text 7, a headwall's one text-field group 13, a
    /// catalog-size pipe's three number-field groups 25; this crate's own
    /// no-group records 6). The slot is the read side's too, so a renamed
    /// part keeps reading its name and can be renamed again whatever its
    /// slot holds; a record whose layout pins no slot is refused (#675,
    /// the reviews of #693).
    #[test]
    fn part_names_go_to_the_slot_the_record_pins() {
        let v = Vector3::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let n = add_pipe_network(&mut doc, "NW-SLOTS", None).unwrap();
        let s = add_structure(&mut doc, n, v(0.0, 0.0, 0.0), 0.0).unwrap();
        let p1 = add_pipe(&mut doc, n, v(0.0, 0.0, -2.0), v(50.0, 0.0, -2.0)).unwrap();
        let p2 = add_pipe(&mut doc, n, v(50.0, 0.0, -2.0), v(100.0, 0.0, -2.0)).unwrap();

        // This crate's own records list no fields: the name stays at
        // strings[6], and a part can be renamed again and again, the
        // second time to a name without the part-list prefix.
        set_network_part_name(&mut doc, s, "Storm 1").unwrap();
        set_network_part_name(&mut doc, s, "Storm 2").unwrap();
        set_network_part_name(&mut doc, p1, "Storm 1").unwrap();
        set_network_part_name(&mut doc, p1, "Storm 2").unwrap();
        assert_eq!(arch_entity(&doc, s).unwrap().decode().strings[6], "Storm 2");
        assert_eq!(arch_entity(&doc, p1).unwrap().decode().strings[6], "Storm 2");

        // The application's normal pipe: one description text before the
        // `AeccNetworkPartRecord` word (the writer's extra "leightk" text
        // in the sample), so the name sits one text later. The writer's
        // zero-field record bits stand.
        let mut normal = texts(PIPE_STRINGS);
        normal.insert(3, "leightk".into());
        let bits = record_bits(&doc, p2);
        rebody(&mut doc, p2, bits, normal);
        assert_eq!(arch_entity(&doc, p2).unwrap().decode().strings[7], "Pipe - (1)");
        set_network_part_name(&mut doc, p2, "Storm 1").unwrap();
        set_network_part_name(&mut doc, p2, "Storm 2").unwrap();
        assert_eq!(arch_entity(&doc, p2).unwrap().decode().strings[7], "Storm 2");

        // The headwall's carrier: one text-field group at the writer's
        // name slot (its "Reinforced Concrete" value is the group's
        // seventh text), the name after it.
        let mut headwall: Vec<String> = ["AeccImpStructure", "", "44 x 6 x 37 inch Concrete Rectangular Headwall", "AeccNetworkPartRecord", "", ""].iter().map(|s| s.to_string()).collect();
        headwall.extend(group("Mat", "Material", "Material_Type", "", Some("Reinforced Concrete")));
        headwall.extend(["Storm 1", "XC_STORM", "EG"].map(String::from));
        rebody(&mut doc, s, bits_of(&structure_record(&field(301, "00", 0x82, "0"), 1)), headwall);
        set_network_part_name(&mut doc, s, "MAIN-01").unwrap();
        assert_eq!(arch_entity(&doc, s).unwrap().decode().strings[13], "MAIN-01");
        set_network_part_name(&mut doc, s, "Storm 2").unwrap();
        assert_eq!(arch_entity(&doc, s).unwrap().decode().strings[13], "Storm 2");
        assert_eq!(arch_entity(&doc, s).unwrap().decode().strings[12], "Reinforced Concrete");

        // A catalog-size pipe: three number-field groups, the name past
        // them.
        let mut sized: Vec<String> = ["AeccImpPipe", "", "24 inch Concrete Pipe", "leightk", "AeccNetworkPartRecord", "", ""].iter().map(|s| s.to_string()).collect();
        sized.extend(group("ACDW", "Darcy Weisbach Factor", "FlowAnalysis_DarcyWeisbach", "", None));
        sized.extend(group("ACHW", "Hazen Williams Coefficient", "FlowAnalysis_HazenWilliams", "", None));
        sized.extend(group("MCR", "Minimum Curve Radius", "MinCurveRadius", "in", None));
        sized.extend(["MAIN-02", "XC_STORM", "EG", "Catalog Item"].map(String::from));
        let fields = [&0.013f64, &100.0, &75.0].map(|d| field(104, "10", 0x80, &format!("1{}", bd(*d)))).concat();
        rebody(&mut doc, p2, bits_of(&pipe_record(&fields, 3)), sized);
        set_network_part_name(&mut doc, p2, "MAIN-02").unwrap();
        assert_eq!(arch_entity(&doc, p2).unwrap().decode().strings[25], "MAIN-02");

        // The read side reads the same slots: a renamed part keeps its
        // name.
        assert_eq!(arch_entity(&doc, s).unwrap().network_part_name().as_deref(), Some("Storm 2"));
        assert_eq!(arch_entity(&doc, p2).unwrap().network_part_name().as_deref(), Some("MAIN-02"));

        // The re-saved records keep the names in their slots.
        let path = std::env::temp_dir().join(format!("civil-part-slots-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let a = arch_entity(&back, s).unwrap();
        assert_eq!((a.decode().strings[6].as_str(), a.decode().strings[12].as_str(), a.decode().strings[13].as_str()), ("AeccNetworkPartDataField", "Reinforced Concrete", "Storm 2"));
        let a = arch_entity(&back, p2).unwrap();
        assert_eq!((a.decode().strings[7].as_str(), a.decode().strings[25].as_str()), ("AeccNetworkPartDataField", "MAIN-02"));
        assert_eq!(arch_entity(&back, s).unwrap().network_part_name().as_deref(), Some("Storm 2"));
        assert_eq!(arch_entity(&back, p2).unwrap().network_part_name().as_deref(), Some("MAIN-02"));

        // A record without the `AeccNetworkPartRecord` word, and a record
        // whose field list does not read: the layout pins no slot, the
        // rename is refused.
        let bits = record_bits(&doc, p1);
        rebody(&mut doc, p1, bits, vec![String::new(); 8]);
        assert!(set_network_part_name(&mut doc, p1, "Storm 3").is_err());
        rebody(&mut doc, p1, vec![false; 512], texts(PIPE_STRINGS));
        assert!(set_network_part_name(&mut doc, p1, "Storm 3").is_err());
    }
}
