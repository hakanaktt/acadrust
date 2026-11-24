//! Corridors: the definition of a corridor (its baselines, regions,
//! frequencies, added stations and targets), written out of date for the
//! application to rebuild.

use super::super::bits::Bits;
use super::super::corridor::{
    CorridorBoundary, CorridorBaseline, CorridorDefinition, CorridorRegion, CurveStations, RegionFrequency, TargetCurveStations, TargetKind, KEY_ALONG_TARGET_CURVES, KEY_CURVE_MID_ORDINATE,
    KEY_TARGET_CURVE_MID_ORDINATE,
};
use super::profile::new_identifier;
use super::site::default_style_of;
use super::support::*;
use crate::classes::arch::ArchEntityData;
use crate::entities::{EntityCommon, EntityType, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::Handle;
use crate::CadDocument;

use super::default_styles::{ensure_corridor_styles, KEY_CORRIDOR_STYLES};
use super::tree::ensure_surface_tree;
/// Boundary kinds: a polygon inside or outside boundary, a mask, the
/// corridor extents.
pub const POLYGON_INSIDE: u32 = 0x4000_0000;
pub const POLYGON_OUTSIDE: u32 = 0x4000_0002;
pub const MASK: u32 = 0x4000_0004;
pub const EXTENTS: u32 = 0x8000_0002;
/// The class word after the class head of a corridor that has not been
/// built (a built corridor's differs).
const UNBUILT: i32 = 0x4000_8353;
/// The geometry stamp key (`BL 2`, two values; written as 0, as for
/// alignments).
const STAMP: i32 = 0x0B00_1081;
/// Keys a corridor with targets holds: an empty list of doubles and a text
/// (`[]`).
const KEY_TARGETS: i32 = 0x0C04_A61C;
const KEY_TARGET_TEXT: i32 = 0x0404_A627;
/// The run that closes the record after the last baseline's added stations.
const END: &str = "101010101010010000001010100001011111000000100";
/// The lists of a target entry (one per subassembly with targets).
const TARGET_LISTS: usize = 10;

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
    identifier: [u8; 16],
    /// The group block: `BL 0, BL 0, BL group count, BL 1`, then the
    /// groups (nested object heads and nodes), as the assembly stores it.
    groups: Vec<u8>,
    /// Name and node count of each group.
    group_names: Vec<(String, usize)>,
    name: String,
    /// `(subassembly, the subassembly it hangs from or 0)` per node.
    subassemblies: Vec<(u64, u64)>,
    code_set: u64,
}

/// An assembly record:
///
/// ```text
/// BL 80, 0, 80, 80, 0, BL 0x10025F04, BL, 4 bits, RC n keys, keys
/// 120 zero bits, 3BD location, 18 bits, identifier (GUID)
/// group block (BL 0, BL 0, BL n groups, per group BL k and k nodes of
/// 108 bits, BL point, BL, BL), 36 bits
/// references: soft 0, hard owner 0, style (hard), per node {subassembly,
///   the subassembly it hangs from or 0} (soft), code set style (hard)
/// texts: "AeccImpAssembly", "", name, "", "", "", per group {name, "",
///   then "", "" per node}
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
    b.skip(18)?;
    let mut identifier = [0u8; 16];
    for byte in &mut identifier {
        *byte = b.bits_value(8)?;
    }
    let from = b.pos;
    b.bit_long()?;
    b.bit_long()?;
    let group_count = b.bit_long()?;
    let mut sizes = Vec::new();
    for _ in 0..group_count {
        let k = usize::try_from(b.bit_long()?).ok()?;
        for _ in 0..k {
            b.skip(108)?;
            for _ in 0..3 {
                b.bit_long()?;
            }
        }
        sizes.push(k);
    }
    let to = end.checked_sub(36).filter(|t| *t > from)?;
    let groups = (from..to).map(|i| data[i / 8] >> (7 - i % 8) & 1).collect();
    let strings = a.decode().strings;
    let mut at = 6;
    let mut group_names = Vec::new();
    for k in sizes {
        group_names.push((strings.get(at)?.clone(), k));
        at += 2 + 2 * k;
    }
    let refs = a.typed_handles();
    (refs.len() >= 4 && refs.len() % 2 == 0).then_some(())?;
    let subassemblies = refs[3..refs.len() - 1].chunks(2).map(|p| (p[0].0, p[1].0)).collect();
    Some(Assembly { identifier, groups, group_names, name: strings.get(2)?.clone(), subassemblies, code_set: refs[refs.len() - 1].0 })
}

/// Writes `bits` (0/1 values).
fn raw(w: &mut DwgMergedWriter, bits: &[u8]) {
    for b in bits {
        w.write_bit(*b == 1);
    }
}

fn byte(w: &mut DwgMergedWriter, v: u8) {
    bits(w, &format!("{v:08b}"));
}

/// `BL 80, BL 80, BL 0x04BE0000`, the head of a nested object.
fn object_head(w: &mut DwgMergedWriter) {
    w.write_bit_long(80);
    w.write_bit_long(80);
    w.write_bit_long(0x04BE_0000);
}

/// A region's flags and target flags (see the reader's `region`).
fn region_flags(f: &RegionFrequency) -> (i32, i32) {
    let mut flags = match f.curves {
        CurveStations::AtIncrement => 0x1000,
        CurveStations::ByCurvature => 0x400,
        CurveStations::Both => 0x1400,
    };
    for (on, bit) in [(f.at_horizontal_geometry_points, 0x8), (f.at_superelevation_critical_points, 0x80), (f.at_profile_geometry_points, 0x2_0000), (f.at_profile_high_low_points, 0x4_0000)] {
        if on {
            flags |= bit;
        }
    }
    let mut target = match f.target_curves {
        TargetCurveStations::None => 0,
        TargetCurveStations::AtIncrement => 8,
        TargetCurveStations::ByCurvature => 16,
    };
    if f.at_offset_target_geometry_points {
        target |= 1;
    }
    if f.adjacent_to_offset_target_start_end {
        target |= 4;
    }
    (flags, target)
}

/// One region block of an unbuilt corridor:
///
/// ```text
/// the assembly's identifier, its group block, BD start, BD end, 2 bits 00,
/// BD frequency along tangents, BD along curves, BL flags, BD 0,
/// BD along spirals, BD along profile curves, identifier, BL target flags,
/// BL 0, BL 0, BL n target entries, n x 10 lists {BL k, k x item},
/// BL 0, 3 bits 110, BL 0
///
/// A region locked to its alignment's geometry has the identifier of its
/// alignment's anchor (kind 582) before each of BD start and BD end (KH01).
///
/// item:  BL 0x1015, BL 0, object head, B 0, B 0, BL 0 (a surface), or
///        BL 0x1017 (0x1117 for the farthest), BL 0, BL k, k x object head,
///        BL 1 farthest / 0 nearest, B 0, B 0, BL 0
/// ```
fn write_region(w: &mut DwgMergedWriter, r: &CorridorRegion, asm: &Assembly, entries: &[Vec<Vec<&super::super::corridor::RegionTarget>>], id: &[u8; 16], lock: Option<&[[u8; 16]; 2]>) {
    w.write_bytes(&asm.identifier);
    raw(w, &asm.groups);
    for (i, station) in [r.start, r.end].into_iter().enumerate() {
        if let Some(ids) = lock {
            w.write_bytes(&ids[i]);
        }
        w.write_bit_double(station);
    }
    bits(w, "00");
    let f = &r.frequency;
    let (flags, target_flags) = region_flags(f);
    w.write_bit_double(f.along_tangents);
    w.write_bit_double(f.along_curves);
    w.write_bit_long(flags);
    w.write_bit_double(0.0);
    w.write_bit_double(f.along_spirals);
    w.write_bit_double(f.along_profile_curves);
    w.write_bytes(id);
    w.write_bit_long(target_flags);
    w.write_bit_long(0);
    w.write_bit_long(0);
    w.write_bit_long(entries.len() as i32);
    for entry in entries {
        for list in entry {
            w.write_bit_long(list.len() as i32);
            for t in list {
                if t.kind == TargetKind::Surface {
                    w.write_bit_long(0x1015);
                    w.write_bit_long(0);
                    object_head(w);
                } else {
                    w.write_bit_long(if t.farthest { 0x1117 } else { 0x1017 });
                    w.write_bit_long(0);
                    w.write_bit_long(t.objects.len() as i32);
                    for _ in &t.objects {
                        object_head(w);
                    }
                    w.write_bit_long(i32::from(t.farthest));
                }
                bits(w, "00");
                w.write_bit_long(0);
            }
        }
    }
    w.write_bit_long(0);
    bits(w, "110");
    w.write_bit_long(0);
}

/// A region's targets per subassembly of its assembly (in node order), each
/// in its kind's list.
fn target_entries<'a>(r: &'a CorridorRegion, asm: &Assembly) -> Result<Vec<Vec<Vec<&'a super::super::corridor::RegionTarget>>>, DxfError> {
    for t in &r.targets {
        if !asm.subassemblies.iter().any(|(s, _)| *s == t.subassembly) {
            return Err(invalid("a target's subassembly is not one of its region's assembly"));
        }
        if t.objects.is_empty() || (t.kind == TargetKind::Surface && t.objects.len() != 1) {
            return Err(invalid("a target needs its objects (one surface for a surface target)"));
        }
    }
    let mut entries = Vec::new();
    for (sub, _) in &asm.subassemblies {
        let mine: Vec<_> = r.targets.iter().filter(|t| t.subassembly == *sub).collect();
        if mine.is_empty() {
            continue;
        }
        let mut lists = vec![Vec::new(); TARGET_LISTS];
        for t in mine {
            lists[t.kind.list()].push(t);
        }
        entries.push(lists);
    }
    Ok(entries)
}

/// A corridor's keys in the order the application writes them: the
/// targets' list, the stamp, the per-region settings and the targets' text.
fn write_keys(w: &mut DwgMergedWriter, regions: &[&CorridorRegion], targets: bool) {
    let defaults = RegionFrequency::default();
    let per_region = regions.iter().any(|r| {
        let f = &r.frequency;
        (f.along_target_curves, f.curve_mid_ordinate, f.target_curve_mid_ordinate) != (defaults.along_target_curves, defaults.curve_mid_ordinate, defaults.target_curve_mid_ordinate)
    });
    byte(w, 1 + 2 * u8::from(targets) + 3 * u8::from(per_region));
    let doubles = |w: &mut DwgMergedWriter, key: u32, value: &dyn Fn(&RegionFrequency) -> f64| {
        w.write_bit_long(key as i32);
        w.write_bit_long(regions.len() as i32);
        for r in regions {
            w.write_bit_double(value(&r.frequency));
        }
    };
    if targets {
        w.write_bit_long(KEY_TARGETS);
        w.write_bit_long(0);
    }
    for v in [STAMP, 2, 0, 0] {
        w.write_bit_long(v);
    }
    if per_region {
        doubles(w, KEY_ALONG_TARGET_CURVES, &|f| f.along_target_curves);
        doubles(w, KEY_CURVE_MID_ORDINATE, &|f| f.curve_mid_ordinate);
    }
    if targets {
        w.write_bit_long(KEY_TARGET_TEXT);
    }
    if per_region {
        doubles(w, KEY_TARGET_CURVE_MID_ORDINATE, &|f| f.target_curve_mid_ordinate);
    }
}

/// The styles a corridor record refers to.
#[derive(Clone, Copy)]
struct Styles {
    corridor: u64,
    surface: u64,
    material: u64,
    /// The code set style of a corridor without regions (the drawing's
    /// default).
    code_set: u64,
}

/// The boundaries grouped per surface, in surface order (a list per
/// surface that has any).
fn boundary_lists(def: &CorridorDefinition) -> Vec<Vec<&CorridorBoundary>> {
    def.surfaces.iter().map(|s| def.boundaries.iter().filter(|b| b.surface == s.name).collect::<Vec<_>>()).filter(|l| !l.is_empty()).collect()
}

/// The corridor record of an unbuilt corridor:
///
/// ```text
/// BL 80, 0, 80, 80, 0, BL 0x10025F04, BL 0x40008353, 4 bits 0101
/// RC n keys, keys (see [`write_keys`])
/// 2 bits 01, 128 zero bits, 2 bits 01, RC 82
/// BL 0 (no code settings), BL n surfaces, n x {identifier, B 0, B build,
/// B 1, BL n link codes, n x B breakline, BL 0, BL m feature line codes,
/// m x B, BL overhang correction}, BD maximum triangle side, BL n
/// boundary lists (one per surface with boundaries, in surface order),
/// n x {BL k, k x {BL kind, BL m, m x 3BD corner}}, BL 0
/// B rebuild automatically, B 1, RC 32, RC 0x48 (locked to stations;
/// 0x28 locked to the alignments' geometry),
/// RC (baselines << 3 | 2), 1 bit 0, 6 bits the first baseline's region
/// count, BL 0 (no station)
/// baseline: [after the first: BL region count, BL 0], its regions (see
///   [`write_region`]), BL 0, B 1, 9 bits 000000101, identifier, BL n,
///   n x BD added station
/// [`END`]
/// references: soft 0, style (hard), code set style (hard), per surface
///   {soft 0 (its surface is made by the rebuild), surface style, 0,
///   render material (hard)}, hard 0 per boundary, per baseline
///   {feature line or 0, alignment, profile (hard), per region {assembly (hard), the
///   assembly's node pairs {subassembly, the subassembly it hangs from or
///   0} (soft), per target entry {subassembly (hard), its target objects
///   (soft)}}}
/// texts: "AeccImpCorridor", "", name, description, ["[]" with targets],
///   per surface {its name, its link and feature line codes}, per boundary
///   list {the surface's name, per boundary {name, description}}, per
///   baseline {per region {"", per assembly group {name, "", then "",
///   "" per node}, the assembly's name, "", per target {its name twice, "",
///   "", "" per object}, the region's name}, the baseline's name, one
///   description per added station}
/// ```
///
/// The identifiers are version-4 GUIDs; a region starts with its
/// assembly's. Written with the samples' identifiers, stations,
/// frequencies and assemblies, the record equals theirs bit for bit (WC13,
/// WC24 to WC27 with a region flag word of 0, the form of a corridor never
/// saved after an edit).
fn write_corridor(doc: &mut CadDocument, common: EntityCommon, def: &CorridorDefinition, styles: Styles, asms: &[Vec<Assembly>], locks: &[[[u8; 16]; 2]]) -> Result<Handle, DxfError> {
    let seed = common.handle.value();
    let regions: Vec<&CorridorRegion> = def.baselines.iter().flat_map(|b| &b.regions).collect();
    let mut entries = Vec::new();
    for (b, line) in def.baselines.iter().enumerate() {
        for (r, region) in line.regions.iter().enumerate() {
            entries.push(target_entries(region, &asms[b][r])?);
        }
    }
    let targets = entries.iter().any(|e| !e.is_empty());
    let code_set = asms.first().and_then(|a| a.first()).map_or(styles.code_set, |a| a.code_set);
    let mut refs = vec![soft_pointer(0), hard_pointer(styles.corridor), hard_pointer(code_set)];
    let mut strings: Vec<String> = ["AeccImpCorridor", "", &def.name, &def.description].iter().map(|s| s.to_string()).collect();
    if targets {
        strings.push("[]".into());
    }
    for surface in &def.surfaces {
        refs.extend([soft_pointer(0), hard_pointer(styles.surface), hard_pointer(0), hard_pointer(styles.material)]);
        strings.push(surface.name.clone());
        strings.extend(surface.link_codes.iter().chain(&surface.feature_line_codes).map(|c| c.0.clone()));
    }
    let lists = boundary_lists(def);
    for list in &lists {
        strings.push(list[0].surface.clone());
        for b in list {
            refs.push(hard_pointer(0));
            strings.extend([b.name.clone(), b.description.clone()]);
        }
    }
    let mut index = 0;
    for (b, line) in def.baselines.iter().enumerate() {
        refs.extend([hard_pointer(line.feature_line), hard_pointer(line.alignment), hard_pointer(line.profile)]);
        for (r, region) in line.regions.iter().enumerate() {
            let asm = &asms[b][r];
            refs.push(hard_pointer(region.assembly));
            for (s, parent) in &asm.subassemblies {
                refs.extend([soft_pointer(*s), soft_pointer(*parent)]);
            }
            strings.push(String::new());
            for (g, k) in &asm.group_names {
                strings.extend([g.clone(), String::new()]);
                strings.extend(std::iter::repeat(String::new()).take(2 * k));
            }
            strings.extend([asm.name.clone(), String::new()]);
            for entry in &entries[index] {
                refs.push(hard_pointer(entry.iter().flatten().next().map_or(0, |t| t.subassembly)));
                for t in entry.iter().flatten() {
                    refs.extend(t.objects.iter().map(|o| soft_pointer(*o)));
                    strings.extend([t.name.clone(), t.name.clone(), String::new(), String::new()]);
                    strings.extend(std::iter::repeat(String::new()).take(t.objects.len()));
                }
            }
            strings.push(region.name.clone());
            index += 1;
        }
        strings.push(line.name.clone());
        strings.extend(line.stations.iter().map(|s| s.1.clone()));
    }
    put_entity(
        doc,
        "AECC_CORRIDOR",
        common,
        |w| {
            for v in [80, 0, 80, 80, 0, 0x1002_5F04, UNBUILT] {
                w.write_bit_long(v);
            }
            bits(w, "0101");
            write_keys(w, &regions, targets);
            bits(w, "01");
            bits(w, &"0".repeat(128));
            bits(w, "01");
            byte(w, 82);
            w.write_bit_long(0);
            w.write_bit_long(def.surfaces.len() as i32);
            for (i, surface) in def.surfaces.iter().enumerate() {
                w.write_bytes(&new_identifier(seed ^ (0x200 + i as u64)));
                w.write_bit(false);
                w.write_bit(surface.build);
                w.write_bit(true);
                let flags = |w: &mut DwgMergedWriter, codes: &[(String, bool)]| {
                    w.write_bit_long(codes.len() as i32);
                    codes.iter().for_each(|c| w.write_bit(c.1));
                };
                flags(w, &surface.link_codes);
                w.write_bit_long(0);
                flags(w, &surface.feature_line_codes);
                w.write_bit_long(surface.overhang_correction);
            }
            w.write_bit_double(def.maximum_triangle_side);
            w.write_bit_long(lists.len() as i32);
            for list in &lists {
                w.write_bit_long(list.len() as i32);
                for b in list {
                    w.write_bit_long(b.kind as i32);
                    w.write_bit_long(b.corners.len() as i32);
                    for c in &b.corners {
                        w.write_bit_double(c.x);
                        w.write_bit_double(c.y);
                        w.write_bit_double(c.z);
                    }
                }
            }
            w.write_bit_long(0);
            w.write_bit(def.rebuild_automatic);
            w.write_bit(true);
            byte(w, 32);
            if def.baselines.is_empty() {
                bits(w, crate::classes::civil::corridor::BARE_END);
                return;
            }
            byte(w, if def.geometry_lock { 0x28 } else { 0x48 });
            byte(w, (def.baselines.len() as u8) << 3 | 2);
            bits(w, &format!("0{:06b}", def.baselines[0].regions.len()));
            w.write_bit_long(0);
            let mut index = 0;
            for (b, line) in def.baselines.iter().enumerate() {
                if b > 0 {
                    w.write_bit_long(line.regions.len() as i32);
                    w.write_bit_long(0);
                }
                for (r, region) in line.regions.iter().enumerate() {
                    let id = new_identifier(seed ^ (0x100 + index as u64));
                    write_region(w, region, &asms[b][r], &entries[index], &id, locks.get(index));
                    index += 1;
                }
                w.write_bit_long(0);
                bits(w, "1000000101");
                w.write_bytes(&new_identifier(seed ^ (0x1_0000 + b as u64)));
                w.write_bit_long(line.stations.len() as i32);
                for s in &line.stations {
                    w.write_bit_double(s.0);
                }
            }
            bits(w, END);
        },
        Body { refs, strings },
    )
}

/// The length of an alignment's elements (an arc's chords taken back to
/// its arc by its curvature).
pub(super) fn alignment_length(al: &ArchEntityData) -> f64 {
    let arc = |chord: f64, k: f64| if k.abs() < 1e-12 { chord } else { 2.0 / k.abs() * (chord * k.abs() / 2.0).min(1.0).asin() };
    al.alignment_pieces_with_curvature().iter().map(|(_, k, p)| p.windows(2).map(|w| arc(w[0].distance(&w[1]), *k)).sum::<f64>()).sum()
}

/// Adds the corridor `def` defines on `layer` (layer "0" when `None`; a
/// corridor without baselines takes neither surfaces nor a geometry lock):
/// baselines along an alignment and one of its profiles, each with
/// regions applying an assembly of the drawing over a station range with
/// their frequencies and targets, and the stations added by hand (sorted
/// per baseline). The corridor is written out of date, as the
/// application's own corridor before its first rebuild: it holds no
/// stations, applied sections or feature lines until it is rebuilt. The
/// corridor style is the drawing's default (written when the drawing has
/// none); the code set style is the first region's assembly's. Region and
/// target names and handles come from `def`; its surfaces are written with
/// their codes (sorted by name), build flag and overhang correction, their
/// boundaries and masks drawn from polygons (closed when open) or along the
/// corridor extents, and the maximum triangle side length; the rebuild
/// makes each built surface. `built`, the feature line fields and the
/// feature line boundaries are not written (a boundary along feature lines
/// is refused). With `geometry_lock` every region is locked to its
/// alignment's geometry: each baseline's alignment, one this crate writes,
/// is written again with an anchor at each region's start and end, whose
/// identifiers the region keeps. A baseline along a feature line
/// names it with an alignment and profile that follow it (the
/// application's own makes them hidden); the rebuild follows the alignment
/// and profile.
pub fn add_corridor_with(doc: &mut CadDocument, def: &CorridorDefinition, layer: Option<&str>) -> Result<Handle, DxfError> {
    if def.name.is_empty() || def.baselines.len() > 31 || (def.baselines.is_empty() && (!def.surfaces.is_empty() || !def.boundaries.is_empty() || def.geometry_lock)) {
        return Err(invalid("a corridor needs a name and up to 31 baselines (surfaces and a geometry lock only with baselines)"));
    }
    for b in &def.boundaries {
        let polygon = matches!(b.kind, POLYGON_INSIDE | POLYGON_OUTSIDE | MASK) && b.corners.len() >= 3;
        if !(polygon || (b.kind == EXTENTS && b.corners.is_empty())) || !def.surfaces.iter().any(|s| s.name == b.surface) {
            return Err(invalid("a boundary needs its surface and a polygon (or none for the corridor extents)"));
        }
    }
    if def.surfaces.iter().any(|s| s.name.is_empty()) || def.surfaces.iter().enumerate().any(|(i, s)| def.surfaces[..i].iter().any(|t| t.name == s.name)) {
        return Err(invalid("corridor surfaces need distinct names"));
    }
    let mut asms = Vec::new();
    for line in &def.baselines {
        if line.name.is_empty() || line.regions.is_empty() || line.regions.len() > 63 {
            return Err(invalid("a baseline needs a name, an alignment and 1 to 63 regions"));
        }
        if line.feature_line != 0 && arch(doc, Handle::new(line.feature_line)).map(|a| a.dxf_name.as_str()) != Some("AECC_FEATURE_LINE") {
            return Err(invalid("feature line not found"));
        }
        let al = arch(doc, Handle::new(line.alignment)).filter(|a| a.dxf_name == "AECC_ALIGNMENT").ok_or_else(|| invalid("alignment not found"))?;
        let pr = arch(doc, Handle::new(line.profile)).filter(|a| a.dxf_name == "AECC_PROFILE").ok_or_else(|| invalid("profile not found"))?;
        if !al.typed_handles().iter().any(|r| r.0 == line.profile) || pr.typed_handles().get(12).map(|r| r.0) != Some(line.alignment) {
            return Err(invalid("the profile is not one of the alignment's"));
        }
        let start = al.alignment_start_station().ok_or_else(|| invalid("the alignment has no start station"))?;
        let end = start + alignment_length(al);
        // Spirals are measured along their chords: a little slack.
        let slack = 1e-3 * (end - start) + 1e-6;
        let mut list = Vec::new();
        for r in &line.regions {
            if r.name.is_empty() || !(start - slack..=end + slack).contains(&r.start) || !(r.start < r.end && r.end <= end + slack) {
                return Err(invalid("a region needs a name and a station range on its alignment"));
            }
            list.push(arch(doc, Handle::new(r.assembly)).and_then(read_assembly).ok_or_else(|| invalid("a region's assembly is not one this crate reads"))?);
        }
        asms.push(list);
    }
    ensure_corridor_styles(doc)?;
    let corridor = default_style_of(doc, KEY_CORRIDOR_STYLES).ok_or_else(|| invalid("drawing has no corridor styles"))?;
    let mut surface = 0;
    if !def.surfaces.is_empty() {
        if super::tin::default_style(doc).is_none() {
            ensure_surface_tree(doc)?;
        }
        surface = super::tin::default_style(doc).ok_or_else(|| invalid("drawing has no surface styles"))?;
    }
    let material = match doc.objects.get(&doc.header.acad_material_dict_handle) {
        Some(crate::objects::ObjectType::Dictionary(d)) => d.entries.iter().find(|e| e.0.eq_ignore_ascii_case("ByLayer")).map_or(0, |e| e.1.value()),
        _ => 0,
    };
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let mut def = def.clone();
    for line in &mut def.baselines {
        line.stations.sort_by(|a, b| a.0.total_cmp(&b.0));
    }
    for s in &mut def.surfaces {
        s.link_codes.sort_by(|a, b| a.0.cmp(&b.0));
        s.feature_line_codes.sort_by(|a, b| a.0.cmp(&b.0));
    }
    for b in &mut def.boundaries {
        if b.corners.first() != b.corners.last() {
            b.corners.push(b.corners[0]);
        }
    }
    let handle = doc.allocate_handle();
    let mut locks = Vec::new();
    if def.geometry_lock {
        for line in &def.baselines {
            let mut anchors = Vec::new();
            for r in &line.regions {
                let ids = [0x300, 0x301].map(|salt| new_identifier(handle.value() ^ (salt + 2 * locks.len() as u64)));
                anchors.extend([(ids[0], r.start), (ids[1], r.end)]);
                locks.push(ids);
            }
            super::alignment::add_anchors(doc, Handle::new(line.alignment), super::alignment::LOCK_ANCHOR, &anchors)?;
        }
    }
    let common = EntityCommon { handle, owner_handle: doc.header.model_space_block_handle, layer: layer.into(), layer_handle: Some(layer_handle), ..EntityCommon::default() };
    let mut code_set = 0;
    if def.baselines.is_empty() {
        super::default_styles::ensure_assembly_styles(doc)?;
        code_set = default_style_of(doc, super::default_styles::KEY_CODE_SET_STYLES).ok_or_else(|| invalid("drawing has no code set styles"))?;
    }
    write_corridor(doc, common, &def, Styles { corridor, surface, material, code_set }, &asms, &locks)
}

/// Adds a corridor `name` along `alignment` and its `profile` with one
/// baseline (`baseline`) and one region (`region`) over the alignment's
/// stations that applies `assembly` every 25 units (see
/// [`add_corridor_with`]).
#[allow(clippy::too_many_arguments)]
pub fn add_corridor(doc: &mut CadDocument, name: &str, baseline: &str, region: &str, alignment: Handle, profile: Handle, assembly: Handle, layer: Option<&str>) -> Result<Handle, DxfError> {
    let al = arch(doc, alignment).filter(|a| a.dxf_name == "AECC_ALIGNMENT").ok_or_else(|| invalid("alignment not found"))?;
    let start = al.alignment_start_station().ok_or_else(|| invalid("the alignment has no start station"))?;
    let end = start + alignment_length(al);
    let region = CorridorRegion { name: region.into(), assembly: assembly.value(), start, end, frequency: RegionFrequency::default(), targets: Vec::new() };
    let line = CorridorBaseline { name: baseline.into(), feature_line: 0, alignment: alignment.value(), profile: profile.value(), regions: vec![region], stations: Vec::new(), built: false };
    add_corridor_with(doc, &CorridorDefinition { name: name.into(), baselines: vec![line], ..CorridorDefinition::default() }, layer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classes::civil::{CorridorBoundary, CorridorSurfaceDefinition, RegionTarget, TargetKind};
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
            w.write_bit_long(0x04BE_0000);
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
        let [sub, code_set] = [(); 2].map(|_| doc.allocate_handle());
        let asm = assembly(&mut doc, sub.value(), code_set.value());
        assert!(add_corridor(&mut doc, "COR1", "BL1", "RG1", al, al, asm, None).is_err());
        let c = add_corridor(&mut doc, "COR1", "BL1", "RG1", al, p, asm, None).unwrap();
        // A drawing without corridor styles gets the default one.
        let style = default_style_of(&doc, KEY_CORRIDOR_STYLES).unwrap();
        assert_eq!(object(&doc, Handle::new(style)).unwrap().dxf_name, "AECC_CORRIDOR_STYLE");
        let path = std::env::temp_dir().join(format!("civil-corridor-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let a = arch(&back, c).unwrap();
        let refs: Vec<u64> = a.typed_handles().iter().map(|r| r.0).collect();
        assert_eq!(refs, vec![0, style, code_set.value(), 0, al.value(), p.value(), asm.value(), sub.value(), 0]);
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
    #[test]
    fn corridor_definitions_read_back() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        let p = crate::classes::civil::add_profile(&mut doc, al, "P1", &[v(0.0, 10.0), v(100.0, 15.0)], &[], None).unwrap();
        let al2 = crate::classes::civil::add_alignment(&mut doc, "AL2", &[v(0.0, 9.0), v(100.0, 9.0)], None).unwrap();
        let p2 = crate::classes::civil::add_profile(&mut doc, al2, "P2", &[v(0.0, 1.0), v(100.0, 2.0)], &[], None).unwrap();
        let [sub, code_set] = [(); 2].map(|_| doc.allocate_handle());
        let asm = assembly(&mut doc, sub.value(), code_set.value());
        let target = RegionTarget { subassembly: sub.value(), name: "TargetHA".into(), kind: TargetKind::Offset, objects: vec![al2.value()], farthest: true };
        let frequency = RegionFrequency { along_tangents: 10.0, curves: CurveStations::Both, curve_mid_ordinate: 2.0, at_profile_high_low_points: true, target_curves: TargetCurveStations::ByCurvature, ..RegionFrequency::default() };
        let r1 = CorridorRegion { name: "RG1".into(), assembly: asm.value(), start: 0.0, end: 40.0, frequency, targets: vec![target] };
        let r2 = CorridorRegion { name: "RG2".into(), assembly: asm.value(), start: 50.0, end: 100.0, frequency: RegionFrequency::default(), targets: vec![] };
        let b1 = CorridorBaseline { name: "BL1".into(), feature_line: 0, alignment: al.value(), profile: p.value(), regions: vec![r1, r2], stations: vec![(25.5, "A".into())], built: false };
        let mut b2 = b1.clone();
        (b2.name, b2.alignment, b2.profile, b2.stations) = ("BL2".into(), al2.value(), p2.value(), vec![]);
        b2.regions.truncate(1);
        let def = CorridorDefinition { name: "COR1".into(), description: "D".into(), rebuild_automatic: true, baselines: vec![b1, b2], ..CorridorDefinition::default() };
        let c = add_corridor_with(&mut doc, &def, None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-corridor-def-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(arch(&back, c).unwrap().corridor_definition(), Some(def));
    }

    #[test]
    fn corridor_surfaces_read_back() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        let p = crate::classes::civil::add_profile(&mut doc, al, "P1", &[v(0.0, 10.0), v(100.0, 15.0)], &[], None).unwrap();
        let [sub, code_set] = [(); 2].map(|_| doc.allocate_handle());
        let asm = assembly(&mut doc, sub.value(), code_set.value());
        let mut def = CorridorDefinition { name: "COR1".into(), maximum_triangle_side: 50.0, ..CorridorDefinition::default() };
        let region = CorridorRegion { name: "RG1".into(), assembly: asm.value(), start: 0.0, end: 100.0, frequency: RegionFrequency::default(), targets: vec![] };
        // A baseline along a feature line, built along the alignment that follows it.
        let site = crate::classes::civil::add_site(&mut doc, "S", "").unwrap();
        let line = crate::classes::civil::add_feature_line(&mut doc, site, "FL1", &[crate::types::Vector3::new(0.0, 0.0, 10.0), crate::types::Vector3::new(100.0, 0.0, 15.0)], None).unwrap();
        let mut along = CorridorBaseline { name: "BL1".into(), feature_line: al.value(), alignment: al.value(), profile: p.value(), regions: vec![region], stations: vec![], built: false };
        assert!(add_corridor_with(&mut doc, &CorridorDefinition { baselines: vec![along.clone()], ..def.clone() }, None).is_err());
        along.feature_line = line.value();
        def.baselines.push(along);
        let surface = |name: &str, build| CorridorSurfaceDefinition { name: name.into(), build, link_codes: vec![("Top".into(), true), ("Datum".into(), false)], feature_line_codes: vec![], overhang_correction: 1 };
        def.surfaces = vec![surface("CS1", true), surface("CS2", false)];
        let corner = |x, y| crate::types::Vector3::new(x, y, 0.0);
        let boundary = |surface: &str, name: &str, kind, corners| CorridorBoundary { surface: surface.into(), name: name.into(), description: String::new(), kind, corners, components: 0 };
        def.boundaries = vec![boundary("CS2", "B1", EXTENTS, vec![]), boundary("CS2", "M1", MASK, vec![corner(10.0, 0.0), corner(20.0, 0.0), corner(20.0, 5.0)])];
        let c = add_corridor_with(&mut doc, &def, None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-corridor-surfaces-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        // Codes come back in name order, polygons closed.
        def.surfaces.iter_mut().for_each(|s| s.link_codes.reverse());
        def.boundaries[1].corners.push(corner(10.0, 0.0));
        assert_eq!(arch(&back, c).unwrap().corridor_definition(), Some(def));
    }

    #[test]
    fn regions_lock_to_the_alignment_geometry() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        let p = crate::classes::civil::add_profile(&mut doc, al, "P1", &[v(0.0, 10.0), v(100.0, 15.0)], &[], None).unwrap();
        let [sub, code_set] = [(); 2].map(|_| doc.allocate_handle());
        let asm = assembly(&mut doc, sub.value(), code_set.value());
        let region = |name: &str, start, end| CorridorRegion { name: name.into(), assembly: asm.value(), start, end, frequency: RegionFrequency::default(), targets: vec![] };
        let line = CorridorBaseline { name: "BL1".into(), feature_line: 0, alignment: al.value(), profile: p.value(), regions: vec![region("RG1", 0.0, 40.0), region("RG2", 50.0, 90.0)], stations: vec![], built: false };
        let def = CorridorDefinition { name: "COR1".into(), geometry_lock: true, baselines: vec![line], ..CorridorDefinition::default() };
        let c = add_corridor_with(&mut doc, &def, None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-corridor-lock-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(arch(&back, c).unwrap().corridor_definition(), Some(def));
        // The alignment holds an anchor at each region end besides its
        // profile's.
        let mut locked: Vec<f64> = arch(&back, al).unwrap().alignment_anchors().unwrap().iter().filter(|a| a.kind == 582).map(|a| a.station).collect();
        locked.sort_by(f64::total_cmp);
        assert_eq!(locked, vec![0.0, 40.0, 50.0, 90.0]);
    }

    /// A corridor without baselines (CB01): written with the drawing's
    /// default code set style, read back with its name and no baselines.
    #[test]
    fn corridors_without_baselines() {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let def = CorridorDefinition { name: "C1".into(), description: String::new(), rebuild_automatic: false, geometry_lock: false, maximum_triangle_side: 0.0, surfaces: vec![], boundaries: vec![], baselines: vec![] };
        let h = add_corridor_with(&mut doc, &def, None).unwrap();
        assert!(add_corridor_with(&mut doc, &CorridorDefinition { geometry_lock: true, ..def.clone() }, None).is_err());
        let path = std::env::temp_dir().join(format!("civil-corridor-bare-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let read = arch(&back, h).unwrap().corridor_definition().unwrap();
        assert_eq!((read.name.as_str(), read.baselines.len()), ("C1", 0));
        assert_ne!(arch(&back, h).unwrap().typed_handles()[2].0, 0);
    }
}
