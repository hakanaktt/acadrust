//! Assemblies and stock basic lane subassemblies.

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

/// Style tree keys: assembly styles and code set styles.
const KEY_ASSEMBLY_STYLES: i32 = 0x118;
const KEY_CODE_SET_STYLES: i32 = 0xEA0;
/// Root key of the name templates.
const KEY_NAME_TEMPLATES: i32 = 0x8C;

/// The run between an assembly's location and its identifier.
const ASSEMBLY_LOCATION_TAIL: &str = "011010100110101001";
/// A group of one subassembly in an assembly's group block: its index
/// word and the heads of its nested objects (the same in every group of
/// the samples).
const GROUP: (usize, &str) = (122, "405505400000be045415000002f8128");
/// The run after the group block, ending with the version word.
const ASSEMBLY_TAIL: &str = "101010101000000010001011111000000100";

/// Where a subassembly's script comes from: the library list the record
/// keeps (a JSON array of paths, typically under a content folder
/// variable), the resolved path of the library and the module holding its
/// resource texts. The texts are stored as given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubassemblySource {
    pub library: String,
    pub path: String,
    pub resource_module: String,
}

/// The side of the assembly's baseline a subassembly lies on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Right,
    Left,
}

/// The stock basic lane: a lane of `width` falling at `slope` (a ratio,
/// negative downward) from the attachment point, `depth` thick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BasicLane {
    pub side: Side,
    pub width: f64,
    pub depth: f64,
    pub slope: f64,
}

/// A piece of the basic lane record.
enum Lane {
    Run(usize, &'static str),
    /// The attachment point (`3BD`, the assembly's location).
    Origin,
    /// The lane's layout values: 0 and 3 the outer x, 1 the top outer
    /// elevation, 2 the bottom inner elevation, 4 the bottom outer one.
    Point(usize),
    Side,
    Depth,
    Slope,
    Width,
}

/// The record of a basic lane: the stock script's record with its
/// parameters (side, depth, slope, width), its layout points (inner top at
/// the origin, outer top, outer bottom, inner bottom) and its origin as
/// fields; the runs between them (the script's codes, links and shapes)
/// are the same for every basic lane of the samples.
const LANE: &[Lane] = &[
    Lane::Run(315, "542541508045f0210540c1ed004040834010101fd00404400000000000000000000000000000000"),
    Lane::Origin,
    Lane::Run(124, "6a6a5085415000002f81104406a901a"),
    Lane::Point(0),
    Lane::Run(0, ""),
    Lane::Point(1),
    Lane::Run(16, "406a"),
    Lane::Point(2),
    Lane::Run(14, "4068"),
    Lane::Point(3),
    Lane::Run(0, ""),
    Lane::Point(4),
    Lane::Run(556, "411029020000000001000000a408040000000c000001029020300000002000000a4080800000000000001014064100000000004000000080000000c000002a6901048400002"),
    Lane::Side,
    Lane::Run(88, "5029014050304c40000110"),
    Lane::Depth,
    Lane::Run(56, "10304c40000109"),
    Lane::Slope,
    Lane::Run(56, "10404c40000110"),
    Lane::Width,
    Lane::Run(403, "102404141000008aaaaaaaa8000004808ffffffffa9030000000000000000000000000000000000000000000004881a917c08"),
];

fn run(w: &mut DwgMergedWriter, len: usize, hex: &str) {
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

/// What a written assembly holds.
struct Assembly {
    location: Vector3,
    id: [u8; 16],
    style: u64,
    code_set: u64,
    subassemblies: Vec<u64>,
}

/// The assembly record (`AECC_ASSEMBLY`, an entity in model space):
///
/// ```text
/// BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x40000353, 4 bits 0010, RC 0
/// (no keys), 120 zero bits, 3BD location, 18 bits, 16-byte identifier,
/// BL 0, BL 0, BL n groups, n x {group run, BL 0}, BL 0, 36 bits ending
/// with the version word
/// references: soft 0, hard owner 0, the assembly style (hard), per
///   subassembly {subassembly, soft 0} (soft), the code set style (hard)
/// texts: "AeccImpAssembly", "", name, "", "", "", per group {"Group -
///   (i)", "", "", ""}
/// ```
///
/// Every subassembly written here is a group of its own.
fn write_assembly(doc: &mut CadDocument, common: EntityCommon, name: &str, a: &Assembly) -> Result<Handle, DxfError> {
    let mut refs = vec![soft_pointer(0), Ref(DwgReferenceType::HardOwnership, 0), hard_pointer(a.style)];
    for s in &a.subassemblies {
        refs.extend([soft_pointer(*s), soft_pointer(0)]);
    }
    refs.push(hard_pointer(a.code_set));
    let mut strings: Vec<String> = ["AeccImpAssembly", "", name, "", "", ""].iter().map(|s| s.to_string()).collect();
    for i in 0..a.subassemblies.len() {
        strings.extend([format!("Group - ({})", i + 1), String::new(), String::new(), String::new()]);
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
            w.write_bit_long(a.subassemblies.len() as i32);
            for _ in &a.subassemblies {
                run(w, GROUP.0, GROUP.1);
                w.write_bit_long(0);
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
    let asm = Assembly {
        location: Vector3::new(p[0], p[1], p[2]),
        id,
        style: refs[2].0,
        code_set: refs[refs.len() - 1].0,
        subassemblies: refs[3..refs.len() - 1].iter().step_by(2).map(|r| r.0).collect(),
    };
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

fn entity_common(doc: &mut CadDocument, layer: Option<&str>) -> Result<EntityCommon, DxfError> {
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    Ok(EntityCommon { handle: doc.allocate_handle(), owner_handle: doc.header.model_space_block_handle, layer: layer.into(), layer_handle: Some(layer_handle), ..EntityCommon::default() })
}

/// Adds an empty assembly named `name` at `location` on `layer` (layer
/// "0" when `None`), with the drawing's default assembly style and code
/// set style (the drawing must have both).
pub fn add_assembly(doc: &mut CadDocument, name: &str, location: Vector3, layer: Option<&str>) -> Result<Handle, DxfError> {
    if name.is_empty() {
        return Err(invalid("an assembly needs a name"));
    }
    if !(location.x.is_finite() && location.y.is_finite() && location.z.is_finite()) {
        return Err(invalid("an assembly needs a finite location"));
    }
    let style = default_style_of(doc, KEY_ASSEMBLY_STYLES).ok_or_else(|| invalid("drawing has no assembly styles"))?;
    let code_set = default_style_of(doc, KEY_CODE_SET_STYLES).ok_or_else(|| invalid("drawing has no code set styles"))?;
    let common = entity_common(doc, layer)?;
    let id = new_identifier(common.handle.value());
    write_assembly(doc, common, name, &Assembly { location, id, style, code_set, subassemblies: Vec::new() })
}

/// The name templates root (root key 0x8C) and its node, which names an
/// assembly's groups, when the drawing lacks them.
fn ensure_name_templates(doc: &mut CadDocument) -> Result<(), DxfError> {
    if root_node(doc, KEY_NAME_TEMPLATES).is_some() {
        return Ok(());
    }
    let root = root(doc).ok_or_else(|| invalid("drawing has no civil root"))?;
    let [head, node] = [(); 2].map(|_| doc.allocate_handle());
    put_object(doc, "AECC_NAME_TEMPLATES_ROOT", head, root, Vec::new(), |w| run(w, 69, "631012003800024808"), Body { refs: vec![soft_owner(node.value()), soft_pointer(0)], strings: vec![String::new()] })?;
    put_object(doc, "AECC_NAME_TEMPLATES_NODE", node, head, Vec::new(), |w| run(w, 91, "200380002920280a0480a02"), Body { refs: vec![soft_pointer(0)], strings: vec![String::new(), "Group - (<[67112990(2449)]>)".to_string()] })?;
    set_root_entry(doc, KEY_NAME_TEMPLATES, head)
}

/// Adds a stock basic lane named `name` to `assembly` as a group of its
/// own, attached at the assembly's location, on `layer` (layer "0" when
/// `None`). The subassembly takes the drawing's default subassembly style
/// (written when the drawing has none) and the assembly's code set style;
/// `source` names the stock library as the caller's installation has it.
/// The assembly must be one this crate writes.
pub fn add_basic_lane(doc: &mut CadDocument, assembly: Handle, name: &str, lane: &BasicLane, source: &SubassemblySource, layer: Option<&str>) -> Result<Handle, DxfError> {
    if name.is_empty() {
        return Err(invalid("a subassembly needs a name"));
    }
    if !(lane.width.is_finite() && lane.width > 0.0 && lane.depth.is_finite() && lane.depth >= 0.0 && lane.slope.is_finite()) {
        return Err(invalid("a basic lane needs a positive width, a depth of 0 or more and a finite slope"));
    }
    let (asm_common, asm_name, mut asm) = read_assembly(doc, assembly)?;
    super::default_styles::ensure_subassembly_styles(doc)?;
    ensure_name_templates(doc)?;
    let style = default_style_of(doc, super::default_styles::KEY_SUBASSEMBLY_STYLES).ok_or_else(|| invalid("drawing has no subassembly styles"))?;
    let common = entity_common(doc, layer)?;
    let handle = common.handle;
    let x = if lane.side == Side::Left { -lane.width } else { lane.width };
    let top = lane.width * lane.slope;
    let points = [x, top, -lane.depth, x, top - lane.depth];
    let refs = vec![soft_pointer(0), Ref(DwgReferenceType::HardOwnership, 0), soft_pointer(assembly.value()), hard_pointer(style), soft_pointer(handle.value()), hard_pointer(asm.code_set)];
    let strings: Vec<String> = [
        "AeccImpSubassembly", "", "", &source.library, "{\"docs_permanent_id\": \"\"}", "1C93F66D5FE047B1FF8D78D80555436A", name, "", &source.path, "Subassembly.BasicLane", "", "", "", &source.resource_module, "",
        "Crown", "ETW", "Crown_SubBase", "ETW_SubBase", "Top", "Pave", "Datum", "SubBase", "Pave1",
        "Side", "505", "506", "", "Right", "507", "Left", "508", "Depth", "511", "512", "", "Slope", "513", "514", "", "Width", "509", "510", "", "Version", "503", "504", "", "R2019", "",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let origin = asm.location;
    put_entity(
        doc,
        "AECC_SUBASSEMBLY",
        common,
        |w| {
            for piece in LANE {
                match piece {
                    Lane::Run(len, hex) => run(w, *len, hex),
                    Lane::Origin => point3(w, origin),
                    Lane::Point(i) => w.write_bit_double(points[*i]),
                    Lane::Side => w.write_bit_long(if lane.side == Side::Left { 1 } else { 0 }),
                    Lane::Depth => w.write_bit_double(lane.depth),
                    Lane::Slope => w.write_bit_double(lane.slope),
                    Lane::Width => w.write_bit_double(lane.width),
                }
            }
        },
        Body { refs, strings },
    )?;
    asm.subassemblies.push(handle.value());
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
    fn assemblies_take_basic_lanes() {
        let source = SubassemblySource { library: "[ \"lanes.dll\" ]".into(), path: "lanes.dll".into(), resource_module: "lanes-rc.dll".into() };
        let mut empty = CadDocument::new();
        empty.version = DxfVersion::AC1032;
        assert!(add_assembly(&mut empty, "A", Vector3::new(0.0, 0.0, 0.0), None).is_err());
        let mut doc = drawing();
        let a = add_assembly(&mut doc, "ASM1", Vector3::new(20.0, -60.0, 0.0), None).unwrap();
        let bad = BasicLane { side: Side::Right, width: 0.0, depth: 0.5, slope: -0.02 };
        assert!(add_basic_lane(&mut doc, a, "X", &bad, &source, None).is_err());
        let r = add_basic_lane(&mut doc, a, "LaneR", &BasicLane { side: Side::Right, width: 12.0, depth: 0.67, slope: -0.02 }, &source, None).unwrap();
        let l = add_basic_lane(&mut doc, a, "LaneL", &BasicLane { side: Side::Left, width: 10.0, depth: 0.5, slope: -0.03 }, &source, None).unwrap();
        assert!(root_node(&doc, KEY_NAME_TEMPLATES).is_some());
        let path = std::env::temp_dir().join(format!("civil-assembly-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let mut back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let (_, name, asm) = read_assembly(&mut back, a).unwrap();
        assert_eq!((name.as_str(), asm.location, asm.subassemblies.clone()), ("ASM1", Vector3::new(20.0, -60.0, 0.0), vec![r.value(), l.value()]));
        let lane = match back.get_entity(l) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => a.clone(),
                _ => panic!(),
            },
            _ => panic!(),
        };
        let strings = lane.decode().strings;
        assert_eq!((strings[3].as_str(), strings[6].as_str(), strings[8].as_str(), strings[13].as_str()), ("[ \"lanes.dll\" ]", "LaneL", "lanes.dll", "lanes-rc.dll"));
        assert_eq!(lane.typed_handles()[2].0, a.value());
    }
}
