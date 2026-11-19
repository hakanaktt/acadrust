//! Corridors: the definition of a corridor (its baseline, region and
//! assembly), written out of date for the application to rebuild.

use super::super::bits::Bits;
use super::profile::new_identifier;
use super::site::default_style_of;
use super::support::*;
use crate::classes::arch::ArchEntityData;
use crate::entities::{EntityCommon, EntityType, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::Handle;
use crate::CadDocument;

/// Style tree key of the corridor style collection.
const KEY_CORRIDOR_STYLES: i32 = 0xEB2;
/// The class word after the class head of a corridor that has not been
/// built (a built corridor's differs).
const UNBUILT: i32 = 0x4000_8353;
/// The geometry stamp key (`BL 2`, two values; written as 0, as for
/// alignments).
const STAMP: i32 = 0x0B00_1081;
/// Default sampling frequencies of a region: along tangents and along
/// curves, then three more settings (0, 25 and 10 in every sample).
const FREQUENCY: f64 = 25.0;
const FREQUENCY_SETTINGS: [f64; 3] = [0.0, 25.0, 10.0];
/// The run that closes the record after the baseline's identifier.
const TAIL: &str = "10101010101010010000001010100001011111000000100";

fn arch(doc: &CadDocument, h: Handle) -> Option<&ArchEntityData> {
    match doc.get_entity(h)? {
        EntityType::Extended(x) => match &x.data {
            ExtendedEntityData::Arch(a) => Some(a),
            _ => None,
        },
        _ => None,
    }
}

/// What a corridor takes from its assembly.
struct Assembly {
    /// The group block: `BL 0, BL 0, BL group count, BL 1`, then the
    /// groups (nested object heads and nodes), as the assembly stores it.
    groups: Vec<u8>,
    group_names: Vec<String>,
    name: String,
    subassemblies: Vec<u64>,
    code_set: u64,
}

/// An assembly record:
///
/// ```text
/// BL 80, 0, 80, 80, 0, BL 0x10025F04, BL, 4 bits, RC n keys, keys
/// 120 zero bits, 3BD location, 18 bits, identifier (GUID)
/// group block (BL 0, BL 0, BL n groups, BL 1, the groups), 36 bits
/// references: soft 0, hard owner 0, style (hard), per subassembly
///   {subassembly (soft), soft 0}, code set style (hard)
/// texts: "AeccImpAssembly", "", name, "", "", "", per group {name, "",
///   "", ""}
/// ```
fn read_assembly(a: &ArchEntityData) -> Option<Assembly> {
    (a.dxf_name == "AECC_ASSEMBLY").then_some(())?;
    let (data, start, end) = a.main_stream()?;
    let mut b = Bits::new(data, start, end);
    for want in [80, 0, 80, 80, 0] {
        (b.bit_long()? == want).then_some(())?;
    }
    b.bit_long()?;
    b.bit_long()?;
    b.skip(4)?;
    for _ in 0..b.bits_value(8)? {
        super::super::alignment::key(&mut b)?;
    }
    b.skip(120)?;
    for _ in 0..3 {
        b.bit_double()?;
    }
    b.skip(18 + 128)?;
    let from = b.pos;
    b.bit_long()?;
    b.bit_long()?;
    let group_count = b.bit_long()?;
    let to = end.checked_sub(36).filter(|t| *t > from)?;
    let groups = (from..to).map(|i| data[i / 8] >> (7 - i % 8) & 1).collect();
    let strings = a.decode().strings;
    let group_names = (0..group_count as usize).map(|i| strings.get(6 + 4 * i).cloned()).collect::<Option<Vec<_>>>()?;
    let refs = a.typed_handles();
    (refs.len() >= 4).then_some(())?;
    let subassemblies = refs[3..refs.len() - 1].iter().step_by(2).map(|r| r.0).collect();
    Some(Assembly { groups, group_names, name: strings.get(2)?.clone(), subassemblies, code_set: refs[refs.len() - 1].0 })
}

/// Writes `bits` (0/1 values).
fn raw(w: &mut DwgMergedWriter, bits: &[u8]) {
    for b in bits {
        w.write_bit(*b == 1);
    }
}

/// The corridor record of an unbuilt corridor, one baseline and one
/// region:
///
/// ```text
/// BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x40008353, 4 bits 0101
/// RC 1, the geometry stamp (BL 2, BL 0, BL 0)
/// 2 bits 01, 128 zero bits, 2 bits 01, RC 82
/// BL 0 (no item), BL 0 (no surface), BL 0, BL 0 (no outline), BL 0
/// 2 bits 01, RC 32, RC 0x48 (layout), RC (baselines << 3 | 2), 1 bit 0,
/// 6 bits region count, BL 0 (no station)
/// region: identifier, the assembly's group block, BD start station,
///   BD end station, 2 bits 00, BD frequency along tangents, BD along
///   curves, BL 0, BD 0, BD 25, BD 10, identifier, BL 0 x 3, BL 0 (no
///   target), BL 0 (no station), 3 bits 110, BL 0
/// baseline: BL 0, B 1, 9 bits 000000101, identifier, then [`TAIL`]
/// references: soft 0, style (hard), code set style (hard), hard 0,
///   alignment, profile, assembly (hard), per subassembly
///   {subassembly (soft), soft 0}
/// texts: "AeccImpCorridor", "", name, "", "", per assembly group {name,
///   "", "", ""}, the assembly's name, "", the region's name, the
///   baseline's name
/// ```
///
/// The identifiers are version-4 GUIDs. Written with the samples'
/// identifiers, stations, frequencies and assemblies (one and two groups),
/// the record equals theirs bit for bit (WC13, WC24 to WC27).
#[allow(clippy::too_many_arguments)]
fn write_corridor(doc: &mut CadDocument, common: EntityCommon, names: [&str; 3], style: u64, alignment: Handle, profile: Handle, assembly: Handle, asm: &Assembly, stations: (f64, f64), ids: [[u8; 16]; 3]) -> Result<Handle, DxfError> {
    let [name, baseline, region] = names;
    let mut refs = vec![soft_pointer(0), hard_pointer(style), hard_pointer(asm.code_set), hard_pointer(0), hard_pointer(alignment.value()), hard_pointer(profile.value()), hard_pointer(assembly.value())];
    for s in &asm.subassemblies {
        refs.extend([soft_pointer(*s), soft_pointer(0)]);
    }
    let mut strings: Vec<String> = ["AeccImpCorridor", "", name, "", ""].iter().map(|s| s.to_string()).collect();
    for g in &asm.group_names {
        strings.extend([g.clone(), String::new(), String::new(), String::new()]);
    }
    strings.extend([asm.name.clone(), String::new(), region.to_string(), baseline.to_string()]);
    let byte = |w: &mut DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    put_entity(
        doc,
        "AECC_CORRIDOR",
        common,
        |w| {
            for v in [80, 0, 80, 80, 0, 0x1002_5F04, UNBUILT] {
                w.write_bit_long(v);
            }
            bits(w, "0101");
            byte(w, 1);
            for v in [STAMP, 2, 0, 0] {
                w.write_bit_long(v);
            }
            bits(w, "01");
            bits(w, &"0".repeat(128));
            bits(w, "01");
            byte(w, 82);
            for _ in 0..5 {
                w.write_bit_long(0);
            }
            bits(w, "01");
            byte(w, 32);
            byte(w, 0x48);
            byte(w, 1 << 3 | 2);
            bits(w, &format!("0{:06b}", 1));
            w.write_bit_long(0);
            w.write_bytes(&ids[0]);
            raw(w, &asm.groups);
            w.write_bit_double(stations.0);
            w.write_bit_double(stations.1);
            bits(w, "00");
            w.write_bit_double(FREQUENCY);
            w.write_bit_double(FREQUENCY);
            w.write_bit_long(0);
            for v in FREQUENCY_SETTINGS {
                w.write_bit_double(v);
            }
            w.write_bytes(&ids[1]);
            for _ in 0..5 {
                w.write_bit_long(0);
            }
            bits(w, "110");
            w.write_bit_long(0);
            w.write_bit_long(0);
            bits(w, "1000000101");
            w.write_bytes(&ids[2]);
            bits(w, TAIL);
        },
        Body { refs, strings },
    )
}

/// Adds a corridor `name` along `alignment` and its `profile` with one
/// baseline (`baseline`) and one region (`region`) over the alignment's
/// stations that applies `assembly` every 25 units, on `layer` (layer "0"
/// when `None`). The corridor is written out of date, as the
/// application's own corridor before its first rebuild: it holds no
/// stations, applied sections or feature lines until it is rebuilt. The
/// drawing must have a corridor style; the code set style is the
/// assembly's.
#[allow(clippy::too_many_arguments)]
pub fn add_corridor(doc: &mut CadDocument, name: &str, baseline: &str, region: &str, alignment: Handle, profile: Handle, assembly: Handle, layer: Option<&str>) -> Result<Handle, DxfError> {
    if name.is_empty() || baseline.is_empty() || region.is_empty() {
        return Err(invalid("a corridor needs a name, a baseline name and a region name"));
    }
    let al = arch(doc, alignment).filter(|a| a.dxf_name == "AECC_ALIGNMENT").ok_or_else(|| invalid("alignment not found"))?;
    let pr = arch(doc, profile).filter(|a| a.dxf_name == "AECC_PROFILE").ok_or_else(|| invalid("profile not found"))?;
    if !al.typed_handles().iter().any(|r| r.0 == profile.value()) || pr.typed_handles().get(12).map(|r| r.0) != Some(alignment.value()) {
        return Err(invalid("the profile is not one of the alignment's"));
    }
    let start = al.alignment_start_station().ok_or_else(|| invalid("the alignment has no start station"))?;
    let length: f64 = al.alignment_pieces().iter().map(|(_, p)| p.windows(2).map(|w| w[0].distance(&w[1])).sum::<f64>()).sum();
    let asm = arch(doc, assembly).and_then(read_assembly).ok_or_else(|| invalid("the assembly is not one this crate reads"))?;
    let style = default_style_of(doc, KEY_CORRIDOR_STYLES).ok_or_else(|| invalid("drawing has no corridor styles"))?;
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let handle = doc.allocate_handle();
    let common = EntityCommon { handle, owner_handle: doc.header.model_space_block_handle, layer: layer.into(), layer_handle: Some(layer_handle), ..EntityCommon::default() };
    let ids = [1, 2, 3].map(|k| new_identifier(handle.value() ^ k));
    write_corridor(doc, common, [name, baseline, region], style, alignment, profile, assembly, &asm, (start, start + length), ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::{DxfVersion, Vector2};

    /// A stand-in assembly record of the grammar `read_assembly` reads:
    /// one group, one subassembly.
    fn assembly(doc: &mut CadDocument, sub: u64, code_set: u64) -> Handle {
        let handle = doc.allocate_handle();
        let common = EntityCommon { handle, owner_handle: doc.header.model_space_block_handle, layer: "0".into(), layer_handle: doc.layers.get("0").map(|l| l.handle), ..EntityCommon::default() };
        let object_head = |w: &mut DwgMergedWriter| {
            w.write_bit_long(80);
            w.write_bit_long(80);
            w.write_raw_long(0x04BE_0000);
        };
        let refs = vec![soft_pointer(0), soft_pointer(0), hard_pointer(0), soft_pointer(sub), soft_pointer(0), hard_pointer(code_set)];
        let strings = ["AeccImpAssembly", "", "ASM1", "", "", "", "Group - (1)", "", "", ""].iter().map(|s| s.to_string()).collect();
        put_entity(doc, "AECC_ASSEMBLY", common, |w| {
            for v in [80, 0, 80, 80, 0, 0x1002_5F04, 0x4000_0353] {
                w.write_bit_long(v);
            }
            bits(w, "0010");
            bits(w, "00000000");
            bits(w, &"0".repeat(120));
            for v in [0.0, -50.0, 0.0] {
                w.write_bit_double(v);
            }
            bits(w, &"0".repeat(18));
            w.write_bytes(&[7; 16]);
            for v in [0, 0, 1, 1] {
                w.write_bit_long(v);
            }
            object_head(w);
            object_head(w);
            w.write_bit_long(0);
            w.write_bit_double(0.0);
            w.write_bit_double(0.0);
            w.write_bit_long(0);
            bits(w, &"0".repeat(36));
        }, Body { refs, strings }).unwrap();
        handle
    }

    #[test]
    fn corridors_take_their_assembly_and_range() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        let p = crate::classes::civil::add_profile(&mut doc, al, "P1", &[v(0.0, 10.0), v(100.0, 15.0)], &[], None).unwrap();
        let [sub, code_set, style] = [(); 3].map(|_| doc.allocate_handle());
        let asm = assembly(&mut doc, sub.value(), code_set.value());
        // No corridor style yet.
        assert!(add_corridor(&mut doc, "COR1", "BL1", "RG1", al, p, asm, None).is_err());
        let tree = root_node(&doc, super::super::tree::KEY_STYLES).unwrap();
        let head = doc.allocate_handle();
        put_object(&mut doc, "AECC_TREE_NODE", head, tree, Vec::new(), |w| {
            w.write_bit_long(KEY_CORRIDOR_STYLES);
            w.write_bit_long(1);
            w.write_bit_long(super::super::tree::DEFAULT_STYLE_ID);
            w.write_bit_long(0);
            bits(w, "010");
        }, Body { refs: vec![soft_owner(style.value())], strings: vec![String::new()] }).unwrap();
        add_node_child(&mut doc, tree, KEY_CORRIDOR_STYLES, head, "").unwrap();
        assert!(add_corridor(&mut doc, "COR1", "BL1", "RG1", al, al, asm, None).is_err());
        let c = add_corridor(&mut doc, "COR1", "BL1", "RG1", al, p, asm, None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-corridor-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let a = arch(&back, c).unwrap();
        let refs: Vec<u64> = a.typed_handles().iter().map(|r| r.0).collect();
        assert_eq!(refs, vec![0, style.value(), code_set.value(), 0, al.value(), p.value(), asm.value(), sub.value(), 0]);
        let strings = a.decode().strings;
        assert_eq!(strings[2], "COR1");
        assert_eq!(&strings[strings.len() - 4..], &["ASM1", "", "RG1", "BL1"]);
        // The region runs over the alignment: its end station is the only
        // full double after the copied group block.
        let (data, start, end) = a.main_stream().unwrap();
        let mut b = Bits::new(data, start, end);
        let found = (start..end).any(|p| {
            b.pos = p;
            b.bits_value(2) == Some(0) && b.raw_double() == Some(100.0)
        });
        assert!(found);
    }
}
