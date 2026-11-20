//! Assemblies and subassemblies built from subassembly packages.

use super::profile::new_identifier;
use super::site::default_style_of;
use super::support::*;
use crate::classes::civil::bits::Bits;
use crate::entities::{EntityCommon, EntityType, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::{Handle, Vector3};
use crate::CadDocument;
use crate::classes::civil::pkt::{enum_default, number_default, PktDirection, PktLayout, PktPackage, PktSide, PktValue};

use super::default_styles::{ensure_assembly_styles, KEY_ASSEMBLY_STYLES, KEY_CODE_SET_STYLES};
/// Root key of the name templates.
const KEY_NAME_TEMPLATES: i32 = 0x8C;

/// The run between an assembly's location and its identifier.
const ASSEMBLY_LOCATION_TAIL: &str = "011010100110101001";
/// The two attachment records that open every node of an assembly's
/// group block (`BS 80, BS 80, BS 0, B 1, BL 240, 5 bits 00100` each; the
/// same in every node of the samples).
const NODE_HEAD: &str = "010101000001010100000000000000000000001011111000000100010101000001010100000000000000000000001011111000000100";
/// The run after the group block, ending with the version word.
const ASSEMBLY_TAIL: &str = "101010101000000010001011111000000100";

pub(super) fn run(w: &mut DwgMergedWriter, len: usize, hex: &str) {
    let mut left = len;
    for c in hex.chars() {
        let v = c.to_digit(16).unwrap_or(0);
        for i in (0..4).rev() {
            if left == 0 {
                return;
            }
            w.write_bit(v >> i & 1 == 1);
            left -= 1;
        }
    }
}

fn point3(w: &mut DwgMergedWriter, p: Vector3) {
    for v in [p.x, p.y, p.z] {
        w.write_bit_double(v);
    }
}

/// A subassembly in an assembly's group: itself, the subassembly it is
/// attached to (0 for the baseline) and the index of that subassembly's
/// point it hangs from.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Node {
    subassembly: u64,
    parent: u64,
    point: u32,
}

/// What a written assembly holds.
struct Assembly {
    location: Vector3,
    id: [u8; 16],
    style: u64,
    code_set: u64,
    /// Each group starts with a subassembly at the baseline; the others
    /// hang from earlier subassemblies of the group, in the order added.
    groups: Vec<Vec<Node>>,
}

/// The assembly record (`AECC_ASSEMBLY`, an entity in model space):
///
/// ```text
/// BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x40000353, 4 bits 0010, RC 0
/// (no keys), 120 zero bits, 3BD location, 18 bits, 16-byte identifier,
/// BL 0, BL 0, BL n groups, n x {BL k nodes, k x node}, BL 0, 36 bits
/// ending with the version word
/// node: NODE_HEAD, BL index of the parent's point it hangs from (0 at the
///   baseline), BL 0, BL 0
/// references: soft 0, hard owner 0, the assembly style (hard), per node
///   in group order {subassembly, the subassembly it hangs from or 0}
///   (soft), the code set style (hard)
/// texts: "AeccImpAssembly", "", name, "", "", "", per group {"Group -
///   (i)", "", then "", "" per node}
/// ```
fn write_assembly(doc: &mut CadDocument, common: EntityCommon, name: &str, a: &Assembly) -> Result<Handle, DxfError> {
    let mut refs = vec![soft_pointer(0), Ref(DwgReferenceType::HardOwnership, 0), hard_pointer(a.style)];
    for n in a.groups.iter().flatten() {
        refs.extend([soft_pointer(n.subassembly), soft_pointer(n.parent)]);
    }
    refs.push(hard_pointer(a.code_set));
    let mut strings: Vec<String> = ["AeccImpAssembly", "", name, "", "", ""].iter().map(|s| s.to_string()).collect();
    for (i, g) in a.groups.iter().enumerate() {
        strings.extend([format!("Group - ({})", i + 1), String::new()]);
        strings.extend(std::iter::repeat(String::new()).take(2 * g.len()));
    }
    put_entity(
        doc,
        "AECC_ASSEMBLY",
        common,
        |w| {
            for v in [80, 0, 80, 80, 0, 0x1002_5F04, 0x4000_0353] {
                w.write_bit_long(v);
            }
            bits(w, "0010");
            w.write_byte(0);
            bits(w, &"0".repeat(120));
            point3(w, a.location);
            bits(w, ASSEMBLY_LOCATION_TAIL);
            w.write_bytes(&a.id);
            w.write_bit_long(0);
            w.write_bit_long(0);
            w.write_bit_long(a.groups.len() as i32);
            for g in &a.groups {
                w.write_bit_long(g.len() as i32);
                for n in g {
                    bits(w, NODE_HEAD);
                    w.write_bit_long(n.point as i32);
                    w.write_bit_long(0);
                    w.write_bit_long(0);
                }
            }
            w.write_bit_long(0);
            bits(w, ASSEMBLY_TAIL);
        },
        Body { refs, strings },
    )
}

fn main_bits(doc: &CadDocument, handle: Handle) -> Option<Vec<u8>> {
    match doc.get_entity(handle)? {
        EntityType::Extended(x) => match &x.data {
            ExtendedEntityData::Arch(a) => a.main_stream().map(|(d, s, e)| (s..e).map(|i| d[i / 8] >> (7 - i % 8) & 1).collect()),
            _ => None,
        },
        _ => None,
    }
}

/// An assembly this crate writes: what it holds, when writing it from
/// that gives back its record.
fn read_assembly(doc: &mut CadDocument, handle: Handle) -> Result<(EntityCommon, String, Assembly), DxfError> {
    let refused = || invalid("the assembly is not one this crate writes");
    let Some(EntityType::Extended(x)) = doc.get_entity(handle) else { return Err(invalid("assembly not found")) };
    let ExtendedEntityData::Arch(a) = &x.data else { return Err(refused()) };
    if a.dxf_name != "AECC_ASSEMBLY" {
        return Err(refused());
    }
    let common = x.common.clone();
    let (data, start, end) = a.main_stream().ok_or_else(refused)?;
    let mut b = Bits::new(data, start, end);
    b.skip(114 + 120).ok_or_else(refused)?;
    let mut p = [0.0; 3];
    for v in &mut p {
        *v = b.bit_double().ok_or_else(refused)?;
    }
    b.skip(ASSEMBLY_LOCATION_TAIL.len()).ok_or_else(refused)?;
    let mut id = [0u8; 16];
    for v in &mut id {
        *v = b.bits_value(8).ok_or_else(refused)?;
    }
    let refs = a.typed_handles();
    let name = a.decode().strings.get(2).cloned().ok_or_else(refused)?;
    (refs.len() >= 4 && refs.len() % 2 == 0).then_some(()).ok_or_else(refused)?;
    // Group block: BL 0, BL 0, BL groups, per group BL k and k nodes.
    let mut pairs = refs[3..refs.len() - 1].chunks(2);
    let mut groups = Vec::new();
    let mut node = |b: &mut Bits| -> Option<Node> {
        b.skip(NODE_HEAD.len())?;
        let point = u32::try_from(b.bit_long()?).ok()?;
        b.bit_long()?;
        b.bit_long()?;
        let pair = pairs.next()?;
        Some(Node { subassembly: pair[0].0, parent: pair[1].0, point })
    };
    (b.bit_long() == Some(0) && b.bit_long() == Some(0)).then_some(()).ok_or_else(refused)?;
    for _ in 0..b.bit_long().ok_or_else(refused)? {
        let k = b.bit_long().ok_or_else(refused)?;
        groups.push((0..k).map(|_| node(&mut b)).collect::<Option<Vec<_>>>().ok_or_else(refused)?);
    }
    let asm = Assembly { location: Vector3::new(p[0], p[1], p[2]), id, style: refs[2].0, code_set: refs[refs.len() - 1].0, groups };
    let original = doc.get_entity(handle).cloned();
    let before = main_bits(doc, handle);
    write_assembly(doc, common.clone(), &name, &asm)?;
    if main_bits(doc, handle) != before {
        if let (Some(o), Some(slot)) = (original, doc.get_entity_mut(handle)) {
            *slot = o;
        }
        return Err(refused());
    }
    Ok((common, name, asm))
}

pub(super) fn entity_common(doc: &mut CadDocument, layer: Option<&str>) -> Result<EntityCommon, DxfError> {
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    Ok(EntityCommon { handle: doc.allocate_handle(), owner_handle: doc.header.model_space_block_handle, layer: layer.into(), layer_handle: Some(layer_handle), ..EntityCommon::default() })
}

/// Adds an empty assembly named `name` at `location` on `layer` (layer
/// "0" when `None`), with the drawing's default assembly style and code
/// set style; a drawing without them gets the application's defaults.
pub fn add_assembly(doc: &mut CadDocument, name: &str, location: Vector3, layer: Option<&str>) -> Result<Handle, DxfError> {
    if name.is_empty() {
        return Err(invalid("an assembly needs a name"));
    }
    if !(location.x.is_finite() && location.y.is_finite() && location.z.is_finite()) {
        return Err(invalid("an assembly needs a finite location"));
    }
    ensure_assembly_styles(doc)?;
    let style = default_style_of(doc, KEY_ASSEMBLY_STYLES).ok_or_else(|| invalid("drawing has no assembly styles"))?;
    let code_set = default_style_of(doc, KEY_CODE_SET_STYLES).ok_or_else(|| invalid("drawing has no code set styles"))?;
    let common = entity_common(doc, layer)?;
    let id = new_identifier(common.handle.value());
    write_assembly(doc, common, name, &Assembly { location, id, style, code_set, groups: Vec::new() })
}

/// Name template id of an assembly's groups.
const GROUP_TEMPLATE: (i32, &str) = (0x0E80, "Group - (<[67112990(2449)]>)");

/// Adds the name template `(id, text)` to the name templates root (root
/// key 0x8C), writing the root when the drawing lacks it. A root lists its
/// nodes (`AECC_NAME_TEMPLATES_NODE`), one per kind of named object:
///
/// ```text
/// root: BL 0x8C, BL n, n x BL id, BL 0, BL 32, 3 bits 001
///       references: n x node (soft owner), soft 0; texts: ""
/// node: BL id, BL 0, BL 0, BL 32, 3 bits 001, BL 1, BL next counter,
///       BL 1, BL 1
///       references: soft 0; texts: "", the template
/// ```
///
/// A new node's counter is 2 (the first name is made with it).
pub(super) fn ensure_name_template(doc: &mut CadDocument, template: (i32, &str)) -> Result<(), DxfError> {
    ensure_name_template_from(doc, template, 2).map(|_| ())
}

/// The node of name template `id` and its next counter.
fn name_node(doc: &CadDocument, id: i32) -> Option<(Handle, i32)> {
    let head = root_node(doc, KEY_NAME_TEMPLATES)?;
    object(doc, head)?.typed_handles().iter().filter(|r| r.0 != 0).find_map(|r| {
        let (d, s, e) = object(doc, Handle::new(r.0)).and_then(fields_of)?;
        let mut b = Bits::new(d, s, e);
        (b.bit_long()? == id).then_some(())?;
        for want in [0, 0, 32] {
            (b.bit_long()? == want).then_some(())?;
        }
        (b.bits_value(3)? == 0b001 && b.bit_long()? == 1).then_some(())?;
        Some((Handle::new(r.0), b.bit_long()?))
    })
}

/// Takes `count` names of template `(id, text)` (adding the template with
/// counter 1 when the drawing lacks it): returns the first counter value
/// and writes the node with the counter after the last.
pub(super) fn take_names(doc: &mut CadDocument, template: (i32, &str), count: i32) -> Result<i32, DxfError> {
    ensure_name_template_from(doc, template, 1)?;
    let (node, next) = name_node(doc, template.0).ok_or_else(|| invalid("a name template node is not one this crate reads"))?;
    let owner = object(doc, node).map(|o| o.owner).ok_or_else(|| invalid("name template node not found"))?;
    write_name_node(doc, node, owner, template, next + count)?;
    Ok(next)
}

fn write_name_node(doc: &mut CadDocument, node: Handle, owner: Handle, template: (i32, &str), next: i32) -> Result<(), DxfError> {
    put_object(
        doc,
        "AECC_NAME_TEMPLATES_NODE",
        node,
        owner,
        Vec::new(),
        |w| {
            for v in [template.0, 0, 0, 32] {
                w.write_bit_long(v);
            }
            bits(w, "001");
            for v in [1, next, 1, 1] {
                w.write_bit_long(v);
            }
        },
        Body { refs: vec![soft_pointer(0)], strings: vec![String::new(), template.1.to_string()] },
    )
}

/// [`ensure_name_template`] with the counter `next` for a new node.
fn ensure_name_template_from(doc: &mut CadDocument, template: (i32, &str), next: i32) -> Result<(), DxfError> {
    let head = root_node(doc, KEY_NAME_TEMPLATES);
    let mut nodes: Vec<(i32, Handle)> = Vec::new();
    if let Some(head) = head {
        for r in object(doc, head).map(|o| o.typed_handles()).unwrap_or_default() {
            if r.0 == 0 {
                continue;
            }
            let id = object(doc, Handle::new(r.0)).and_then(fields_of).and_then(|(d, s, e)| Bits::new(d, s, e).bit_long()).ok_or_else(|| invalid("a name template node is not one this crate reads"))?;
            nodes.push((id, Handle::new(r.0)));
        }
        if nodes.iter().any(|n| n.0 == template.0) {
            return Ok(());
        }
    }
    let root = root(doc).ok_or_else(|| invalid("drawing has no civil root"))?;
    let head_handle = head.unwrap_or_else(|| doc.allocate_handle());
    let node = doc.allocate_handle();
    write_name_node(doc, node, head_handle, template, next)?;
    nodes.push((template.0, node));
    put_object(
        doc,
        "AECC_NAME_TEMPLATES_ROOT",
        head_handle,
        root,
        Vec::new(),
        |w| {
            w.write_bit_long(KEY_NAME_TEMPLATES);
            bl_byte(w, nodes.len() as u8);
            for n in &nodes {
                w.write_bit_long(n.0);
            }
            bits(w, "100100100000001");
        },
        Body { refs: nodes.iter().map(|n| soft_owner(n.1.value())).chain([soft_pointer(0)]).collect(), strings: vec![String::new()] },
    )?;
    if head.is_none() {
        set_root_entry(doc, KEY_NAME_TEMPLATES, head_handle)?;
    }
    Ok(())
}

/// Where a package subassembly's package lies, as the host stores it:
/// `library` the library list (a JSON array of paths, a leading folder
/// typically written as an environment variable), `path` the package's
/// resolved path. Both are stored as given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PktSource {
    pub library: String,
    pub path: String,
}

/// The class keys of a package subassembly (type 4, no value): the four
/// W-K01 to W-K05 carry; a stock subassembly (SA1) lacks 0x0404D01D.
const SUBASSEMBLY_KEYS: [i32; 4] = [0x0404_D01E, 0x0404_D020, 0x0404_D01D, 0x0404_D01F];
/// The run between the record's scale and its point list (the same in
/// W-K01 to W-K05 and in the stock SA1).
const SUBASSEMBLY_HEAD_TAIL: (usize, &str) = (62, "085415000002f810");
/// The run after the parameter set, to the end of the body (the same in
/// W-K01 to W-K05 and in the stock SA1).
const SUBASSEMBLY_TAIL: (usize, &str) = (341, "aaaaaaaa0000012023fffffffea40c0000000000000000000000000000000000000000000012206a45f020");
/// Parameter item types: integer, double.
const ITEM_INTEGER: i32 = 0x1012;
const ITEM_DOUBLE: i32 = 0x1013;

/// MD5 (RFC 1321) of `data`: the host names a package subassembly's
/// script by the MD5 of its package file.
fn md5(data: &[u8]) -> [u8; 16] {
    const S: [u32; 64] = [7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21];
    let k: Vec<u32> = (0..64).map(|i| ((i as f64 + 1.0).sin().abs() * 4_294_967_296.0) as u32).collect();
    let mut h = [0x6745_2301u32, 0xEFCD_AB89, 0x98BA_DCFE, 0x1032_5476];
    let mut msg = data.to_vec();
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&((data.len() as u64).wrapping_mul(8)).to_le_bytes());
    for chunk in msg.chunks(64) {
        let m: Vec<u32> = chunk.chunks(4).map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
        let [mut a, mut b, mut c, mut d] = h;
        for i in 0..64 {
            let (f, g) = match i / 16 {
                0 => ((b & c) | (!b & d), i),
                1 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                2 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let t = d;
            d = c;
            c = b;
            b = b.wrapping_add(a.wrapping_add(f).wrapping_add(k[i]).wrapping_add(m[g]).rotate_left(S[i]));
            a = t;
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d]) {
            *x = x.wrapping_add(y);
        }
    }
    let mut out = [0u8; 16];
    for (i, v) in h.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&v.to_le_bytes());
    }
    out
}

/// One parameter of a subassembly record: its item type, the type's
/// field, value, choices, declaration index and texts.
struct Item {
    name: String,
    display: String,
    kind: i32,
    field: i32,
    integer: i32,
    double: f64,
    choices: Vec<(String, String, i32)>,
    index: i32,
}

/// The input parameters of `pkg` as the record lists them: integers, then
/// doubles, each by name; indices count the inputs in declaration order
/// (targets and a Side of None left out).
fn items(pkg: &PktPackage, side: PktSide, values: &[(String, PktValue)]) -> Result<Vec<Item>, DxfError> {
    let mut out = Vec::new();
    let inputs = pkg.parameters.iter().filter(|p| p.direction == PktDirection::In && !p.is_target());
    for (index, p) in inputs.enumerate() {
        let given = values.iter().find(|v| v.0.eq_ignore_ascii_case(&p.name)).map(|v| &v.1);
        let display = p.display_name().to_string();
        let item = |kind, field, integer, double, choices| Item { name: p.name.clone(), display: display.clone(), kind, field, integer, double, choices, index: index as i32 };
        match p.type_name.as_str() {
            "asw:EnumType" => {
                let d = p.default.as_deref().and_then(enum_default).ok_or_else(|| invalid("a package enumeration without its default"))?;
                if p.name == "Side" && d.0 == -1 {
                    continue;
                }
                let group = d.2.ok_or_else(|| invalid("a package enumeration without its group"))?;
                let choices: Vec<(String, String, i32)> = if group == "Side" {
                    vec![("Left".into(), "Left".into(), 1), ("Right".into(), "Right".into(), 0)]
                } else {
                    pkg.enum_groups.iter().find(|g| g.name == group).ok_or_else(|| invalid("a package enumeration names no group"))?.items.clone()
                };
                let key = if p.name == "Side" {
                    if side == PktSide::Left {
                        1
                    } else {
                        0
                    }
                } else {
                    match given {
                        Some(PktValue::Integer(k)) => *k,
                        None => d.0,
                        _ => return Err(invalid("an enumeration takes an integer key")),
                    }
                };
                if !choices.iter().any(|c| c.2 == key) {
                    return Err(invalid("the key is not one of the enumeration's"));
                }
                out.push(item(ITEM_INTEGER, 0, key, 0.0, choices));
            }
            t @ ("x:Double" | "asw:Grade" | "asw:Slope") => {
                let field = match t {
                    "asw:Grade" => 9,
                    "asw:Slope" => 10,
                    _ => 16,
                };
                let v = match given {
                    Some(PktValue::Double(x)) => *x,
                    None => number_default(p.default.as_deref().unwrap_or("")).parse().map_err(|_| invalid("a package number without its default"))?,
                    _ => return Err(invalid("a number parameter takes a double")),
                };
                out.push(item(ITEM_DOUBLE, field, 0, v, Vec::new()));
            }
            _ => return Err(invalid("a package parameter type the record writer does not know")),
        }
    }
    // ponytail: names compared by bytes; the samples' names sort alike
    // case-blind; check a mixed-case pair against the host if one appears.
    out.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.name.cmp(&b.name)));
    Ok(out)
}

/// The record of a package subassembly (`AECC_SUBASSEMBLY`, an entity in
/// model space):
///
/// ```text
/// BL 80, 0, 80, 80, 0, BL 0x10025F04, 010101, RC 4 keys (0x0404D01E,
/// 0x0404D020, 0x0404D01D, 0x0404D01F), BL 0 (one-byte form), 121 zero bits,
/// 3BD origin, 9 BD rotation (identity), BD 1, 62 bits,
/// BL n points, n x {BL code count, B 1, B 0, BL 0, BD offset, BD elevation},
/// BL n links, n x {BL code count, B 1, B 0, BL n, n x RL point},
/// BL n shapes, n x {BL code count, B 1, B 0, BL n, n x RL link, BL 0},
/// parameter set: BL 0, BL 0, 011010, BL n integers {BL 0x1012, BL 0,
///   BL value, B 0, B 1, BL n, n x BL choice key, BL index}, BL n doubles
///   {BL 0x1013, BL 16 | 9 grade | 10 slope, BD value, B 0, B 0, BL
///   index}, BL 0 texts,
/// 341 bits
/// references: soft 0, hard owner 0, the assembly (soft), subassembly
///   style (hard), the subassembly (soft), code set style (hard)
/// texts: "AeccImpSubassembly", "", "", library list, "{"docs_permanent_id":
///   ""}", version, the package file's MD5 (upper-case hex), name, "",
///   path, "Subassembly.<package name>", 5 x "", the point, link and
///   shape codes, per parameter {name, display name, "", ""} and its
///   choices {name, display name}, ""
/// ```
fn subassembly_record(w: &mut DwgMergedWriter, origin: Vector3, layout: &PktLayout, items: &[Item]) {
    for v in [80, 0, 80, 80, 0, 0x1002_5F04] {
        w.write_bit_long(v);
    }
    bits(w, "010101");
    w.write_byte(SUBASSEMBLY_KEYS.len() as u8);
    for k in SUBASSEMBLY_KEYS {
        w.write_bit_long(k);
    }
    // the closing BL 0 in its one-byte form, as the host writes it
    bits(w, "0100000000");
    bits(w, &"0".repeat(121));
    point3(w, origin);
    for v in [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0] {
        w.write_bit_double(v);
    }
    run(w, SUBASSEMBLY_HEAD_TAIL.0, SUBASSEMBLY_HEAD_TAIL.1);
    let head = |w: &mut DwgMergedWriter, codes: usize| {
        w.write_bit_long(codes as i32);
        w.write_bit(true);
        w.write_bit(false);
    };
    w.write_bit_long(layout.points.len() as i32);
    for p in &layout.points {
        head(w, p.codes.len());
        w.write_bit_long(0);
        w.write_bit_double(p.offset);
        w.write_bit_double(p.elevation);
    }
    w.write_bit_long(layout.links.len() as i32);
    for l in &layout.links {
        head(w, l.codes.len());
        w.write_bit_long(l.points.len() as i32);
        for i in &l.points {
            w.write_raw_long(*i as i32);
        }
    }
    w.write_bit_long(layout.shapes.len() as i32);
    for s in &layout.shapes {
        head(w, s.codes.len());
        w.write_bit_long(s.links.len() as i32);
        for i in &s.links {
            w.write_raw_long(*i as i32);
        }
        w.write_bit_long(0);
    }
    w.write_bit_long(0);
    w.write_bit_long(0);
    bits(w, "011010");
    for kind in [ITEM_INTEGER, ITEM_DOUBLE] {
        let these: Vec<&Item> = items.iter().filter(|i| i.kind == kind).collect();
        w.write_bit_long(these.len() as i32);
        for i in these {
            w.write_bit_long(i.kind);
            w.write_bit_long(i.field);
            if kind == ITEM_INTEGER {
                w.write_bit_long(i.integer);
            } else {
                w.write_bit_double(i.double);
            }
            w.write_bit(false);
            w.write_bit(!i.choices.is_empty());
            if !i.choices.is_empty() {
                w.write_bit_long(i.choices.len() as i32);
                for c in &i.choices {
                    w.write_bit_long(c.2);
                }
            }
            w.write_bit_long(i.index);
        }
    }
    w.write_bit_long(0);
    run(w, SUBASSEMBLY_TAIL.0, SUBASSEMBLY_TAIL.1);
}

/// Adds a subassembly built from the package `pkt` to `assembly`, as a
/// group of its own attached at the assembly's location, on `layer`
/// (layer "0" when `None`). Its geometry is the package's layout on
/// `side` with `values` (see `pkt::layout`); `source` says where the
/// package lies, which must hold the bytes `pkt::write_pkt` writes for
/// `pkt` (the record keeps their MD5). The subassembly takes the drawing's
/// default subassembly style (written when the drawing has none) and the
/// assembly's code set style. The assembly must be one this crate writes.
#[allow(clippy::too_many_arguments)]
pub fn add_pkt_subassembly(doc: &mut CadDocument, assembly: Handle, name: &str, source: &PktSource, pkt: &PktPackage, side: PktSide, values: &[(String, PktValue)], layer: Option<&str>) -> Result<Handle, DxfError> {
    place_subassembly(doc, assembly, None, name, source, pkt, side, values, layer)
}

/// Adds a package subassembly attached to `point` (an index into the
/// parent's layout points) of the subassembly `parent` of the same
/// assembly; it joins the parent's group, after its last member, and the
/// host places its layout (on `side`) at that point. Otherwise as
/// [`add_pkt_subassembly`].
#[allow(clippy::too_many_arguments)]
pub fn add_pkt_subassembly_on(doc: &mut CadDocument, parent: Handle, point: usize, name: &str, source: &PktSource, pkt: &PktPackage, side: PktSide, values: &[(String, PktValue)], layer: Option<&str>) -> Result<Handle, DxfError> {
    let Some(EntityType::Extended(x)) = doc.get_entity(parent) else { return Err(invalid("parent subassembly not found")) };
    let ExtendedEntityData::Arch(a) = &x.data else { return Err(invalid("not a subassembly")) };
    let assembly = a.subassembly_assembly().filter(|_| a.dxf_name == "AECC_SUBASSEMBLY").ok_or_else(|| invalid("not a subassembly of an assembly"))?;
    a.subassembly_shape().and_then(|s| s.points.get(point).copied()).ok_or_else(|| invalid("the parent has no such point"))?;
    let point = u32::try_from(point).map_err(|_| invalid("the parent has no such point"))?;
    place_subassembly(doc, Handle::new(assembly), Some((parent, point)), name, source, pkt, side, values, layer)
}

#[allow(clippy::too_many_arguments)]
fn place_subassembly(doc: &mut CadDocument, assembly: Handle, attach: Option<(Handle, u32)>, name: &str, source: &PktSource, pkt: &PktPackage, side: PktSide, values: &[(String, PktValue)], layer: Option<&str>) -> Result<Handle, DxfError> {
    if name.is_empty() {
        return Err(invalid("a subassembly needs a name"));
    }
    let layout = crate::classes::civil::pkt::layout(pkt, side, values)?;
    if layout.points.is_empty() {
        return Err(invalid("the package lays out no points"));
    }
    let items = items(pkt, side, values)?;
    let id: String = md5(&crate::classes::civil::pkt::write_pkt(pkt)?).iter().map(|b| format!("{b:02X}")).collect();
    let (asm_common, asm_name, mut asm) = read_assembly(doc, assembly)?;
    let group = match attach {
        Some((parent, ..)) => Some(asm.groups.iter().position(|g| g.iter().any(|n| n.subassembly == parent.value())).ok_or_else(|| invalid("the parent is not in the assembly"))?),
        None => None,
    };
    super::default_styles::ensure_subassembly_styles(doc)?;
    ensure_name_template(doc, GROUP_TEMPLATE)?;
    let style = default_style_of(doc, super::default_styles::KEY_SUBASSEMBLY_STYLES).ok_or_else(|| invalid("drawing has no subassembly styles"))?;
    let common = entity_common(doc, layer)?;
    let handle = common.handle;
    let refs = vec![soft_pointer(0), Ref(DwgReferenceType::HardOwnership, 0), soft_pointer(assembly.value()), hard_pointer(style), soft_pointer(handle.value()), hard_pointer(asm.code_set)];
    let class = format!("Subassembly.{}", pkt.name);
    let mut strings: Vec<String> = ["AeccImpSubassembly", "", "", &source.library, "{\"docs_permanent_id\": \"\"}", &pkt.version, &id, name, "", &source.path, &class, "", "", "", "", ""].iter().map(|s| s.to_string()).collect();
    strings.extend(layout.codes.iter().cloned());
    for i in &items {
        strings.extend([i.name.clone(), i.display.clone(), String::new(), String::new()]);
        for c in &i.choices {
            strings.extend([c.0.clone(), c.1.clone()]);
        }
    }
    strings.push(String::new());
    // An attached subassembly keeps the assembly's location as its origin
    // too (the host stores where it was inserted; the group block places
    // it at its parent's point).
    let origin = asm.location;
    put_entity(doc, "AECC_SUBASSEMBLY", common, |w| subassembly_record(w, origin, &layout, &items), Body { refs, strings })?;
    match (attach, group) {
        (Some((parent, point)), Some(g)) => asm.groups[g].push(Node { subassembly: handle.value(), parent: parent.value(), point }),
        _ => asm.groups.push(vec![Node { subassembly: handle.value(), parent: 0, point: 0 }]),
    }
    write_assembly(doc, asm_common, &asm_name, &asm)?;
    Ok(handle)
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::{DxfVersion, Vector2};
    use super::super::tree::{DEFAULT_STYLE_ID, KEY_STYLES};

    /// A drawing with a style tree and stand-in assembly and code set
    /// style collections with a default style each.
    fn drawing() -> CadDocument {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        crate::classes::civil::add_alignment(&mut doc, "AL0", &[Vector2::new(0.0, -50.0), Vector2::new(100.0, -50.0)], None).unwrap();
        let styles = root_node(&doc, KEY_STYLES).unwrap();
        for key in [KEY_ASSEMBLY_STYLES, KEY_CODE_SET_STYLES] {
            let [head, style] = [(); 2].map(|_| doc.allocate_handle());
            for (h, owner, k, children) in [(head, styles, key, vec![(DEFAULT_STYLE_ID, style)]), (style, head, DEFAULT_STYLE_ID, vec![])] {
                let refs = children.iter().map(|c| soft_owner(c.1.value())).collect();
                put_object(&mut doc, "AECC_TREE_NODE", h, owner, Vec::new(), |w| {
                    w.write_bit_long(k);
                    w.write_bit_long(children.len() as i32);
                    for c in &children {
                        w.write_bit_long(c.0);
                    }
                    w.write_bit_long(0);
                    bits(w, "010");
                }, Body { refs, strings: vec![String::new()] })
                .unwrap();
            }
            add_node_child(&mut doc, styles, key, head, "").unwrap();
        }
        doc
    }

    #[test]
    fn md5_matches_rfc_1321() {
        let hex = |d: &[u8]| md5(d).iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(hex(&[b'a'; 1000]), "cabe45dcc9ae5b66ba86600cca6b8ba8");
    }

    #[test]
    fn assemblies_take_package_subassemblies() {
        let pkg = crate::classes::civil::pkt::tests::lane();
        let source = PktSource { library: "[ \"lane.pkt\" ]".into(), path: "lane.pkt".into() };
        let mut empty = CadDocument::new();
        empty.version = DxfVersion::AC1032;
        // A drawing without the styles gets the defaults, with the link,
        // shape, feature line and marker styles the code set draws with.
        add_assembly(&mut empty, "A", Vector3::new(0.0, 0.0, 0.0), None).unwrap();
        let tree = read_node(&empty, root_node(&empty, KEY_STYLES).unwrap()).unwrap();
        for (key, dxf) in [(KEY_ASSEMBLY_STYLES, "AECC_ASSEMBLY_STYLE"), (KEY_CODE_SET_STYLES, "AECC_ROADWAY_STYLE_SET"), (0x11C, "AECC_ROADWAYLINK_STYLE"), (0x11D, "AECC_ROADWAYSHAPE_STYLE"), (0x11B, "AECC_ROADWAYMARKER_STYLE"), (0x80, "AECC_FEATURELINE_STYLE")] {
            assert!(tree.children.iter().any(|c| c.0 == key), "{key:#x}");
            assert_eq!(object(&empty, Handle::new(default_style_of(&empty, key).unwrap())).unwrap().dxf_name, dxf);
        }
        let code_set = object(&empty, Handle::new(default_style_of(&empty, KEY_CODE_SET_STYLES).unwrap())).unwrap();
        let link = default_style_of(&empty, 0x11C).unwrap();
        assert!(code_set.typed_handles().iter().any(|r| r.0 == link));
        // The default code set lists no codes; uncoded and unlisted codes
        // draw with the default marker, link and shape styles.
        let [marker, shape] = [0x11B, 0x11D].map(|k| default_style_of(&empty, k).unwrap());
        assert!(code_set.code_set().is_empty());
        assert_eq!(code_set.code_set_defaults(), [marker, link, shape, marker, link, shape]);
        let mut doc = drawing();
        let a = add_assembly(&mut doc, "ASM1", Vector3::new(20.0, -60.0, 0.0), None).unwrap();
        let wide = [("Width".to_string(), PktValue::Double(4.0)), ("Depth".to_string(), PktValue::Double(0.25)), ("Slope".to_string(), PktValue::Double(-0.03))];
        assert!(add_pkt_subassembly(&mut doc, a, "X", &source, &pkg, PktSide::Right, &[("Width".into(), PktValue::Integer(1))], None).is_err());
        let r = add_pkt_subassembly(&mut doc, a, "LaneR", &source, &pkg, PktSide::Right, &wide, None).unwrap();
        let l = add_pkt_subassembly(&mut doc, a, "LaneL", &source, &pkg, PktSide::Left, &[], None).unwrap();
        assert!(root_node(&doc, KEY_NAME_TEMPLATES).is_some());
        let path = std::env::temp_dir().join(format!("civil-assembly-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let mut back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let (_, name, asm) = read_assembly(&mut back, a).unwrap();
        let subs: Vec<u64> = asm.groups.iter().flatten().map(|n| n.subassembly).collect();
        assert_eq!((name.as_str(), asm.location, subs), ("ASM1", Vector3::new(20.0, -60.0, 0.0), vec![r.value(), l.value()]));
        let record = |h: Handle| match back.get_entity(h) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => a.clone(),
                _ => panic!(),
            },
            _ => panic!(),
        };
        let lane = record(r);
        let strings = lane.decode().strings;
        assert_eq!((strings[3].as_str(), strings[7].as_str(), strings[9].as_str(), strings[10].as_str()), ("[ \"lane.pkt\" ]", "LaneR", "lane.pkt", "Subassembly.Lane"));
        assert_eq!(strings[6].len(), 32);
        assert_eq!(&strings[16..24], ["Crown", "ETW", "ETW_Datum", "Crown_Datum", "Top", "Pave", "Datum", "Pave"]);
        assert_eq!(&strings[24..32], ["Side", "Side", "", "", "Left", "Left", "Right", "Right"]);
        assert_eq!(lane.typed_handles()[2].0, a.value());
        // the stored shape and parameters read back
        let (data, start, end) = lane.main_stream().unwrap();
        let shape = crate::classes::civil::subassembly::find(data, start, end, &strings).unwrap();
        assert_eq!(shape.points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(), [(0.0, 0.0), (4.0, -0.12), (4.0, -0.37), (0.0, -0.49)]);
        assert_eq!((shape.links.clone(), shape.areas.clone()), (vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 0]], vec![vec![0, 1, 2, 3]]));
        assert_eq!(shape.link_codes[0], ["Top", "Pave"]);
        let params = crate::classes::civil::subassembly::parameters(data, start, end, &strings).unwrap();
        let v: Vec<(String, crate::classes::civil::ParameterValue, i32)> = params.into_iter().map(|p| (p.name, p.value, p.index)).collect();
        use crate::classes::civil::ParameterValue::{Double, Integer};
        assert_eq!(v, [("Side".into(), Integer(0), 0), ("Depth".into(), Double(0.25), 2), ("Slope".into(), Double(-0.03), 3), ("Width".into(), Double(4.0), 1)]);
    }

    #[test]
    fn subassemblies_hang_from_points_of_their_group() {
        let pkg = crate::classes::civil::pkt::tests::lane();
        let source = PktSource { library: "[ \"lane.pkt\" ]".into(), path: "lane.pkt".into() };
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let a = add_assembly(&mut doc, "A", Vector3::new(20.0, -60.0, 0.0), None).unwrap();
        let mut chain = Vec::new();
        for side in [PktSide::Right, PktSide::Left] {
            let lane = add_pkt_subassembly(&mut doc, a, "Lane", &source, &pkg, side, &[], None).unwrap();
            let edge = add_pkt_subassembly_on(&mut doc, lane, 1, "Edge", &source, &pkg, side, &[], None).unwrap();
            let outer = add_pkt_subassembly_on(&mut doc, edge, 1, "Outer", &source, &pkg, side, &[], None).unwrap();
            chain.push([lane, edge, outer]);
        }
        let inner = add_pkt_subassembly_on(&mut doc, chain[0][0], 2, "Inner", &source, &pkg, PktSide::Right, &[], None).unwrap();
        assert!(add_pkt_subassembly_on(&mut doc, chain[0][0], 99, "X", &source, &pkg, PktSide::Right, &[], None).is_err(), "no such point");
        assert!(add_pkt_subassembly_on(&mut doc, a, 0, "X", &source, &pkg, PktSide::Right, &[], None).is_err(), "not a subassembly");
        let path = std::env::temp_dir().join(format!("civil-chain-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let mut back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        // Two groups: each starts at the baseline, the others hang from
        // earlier members in the order added.
        let (_, _, asm) = read_assembly(&mut back, a).unwrap();
        let node = |s: Handle, p: u64, point: u32| Node { subassembly: s.value(), parent: p, point };
        let [r, l] = [chain[0], chain[1]];
        assert_eq!(asm.groups, vec![
            vec![node(r[0], 0, 0), node(r[1], r[0].value(), 1), node(r[2], r[1].value(), 1), node(inner, r[0].value(), 2)],
            vec![node(l[0], 0, 0), node(l[1], l[0].value(), 1), node(l[2], l[1].value(), 1)],
        ]);
        let Some(EntityType::Extended(x)) = back.get_entity(a) else { panic!() };
        let ExtendedEntityData::Arch(record) = &x.data else { panic!() };
        let layout = record.assembly_layout().unwrap();
        let read: Vec<(u64, u64, u32)> = layout.attachments.iter().map(|t| (t.subassembly, t.parent, t.point)).collect();
        let want: Vec<(u64, u64, u32)> = asm.groups.iter().flatten().map(|n| (n.subassembly, n.parent, n.point)).collect();
        assert_eq!(read, want);
        assert_eq!(record.decode().strings.iter().filter(|t| t.starts_with("Group - (")).count(), 2);
    }

}
