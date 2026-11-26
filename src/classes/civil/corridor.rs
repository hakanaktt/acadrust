//! Corridor feature lines.
//!
//! A corridor body stores its feature lines as groups, one per point code
//! in the order of the corridor's code list:
//!
//! ```text
//! BL  segment count
//! segment count x { BL n, n x { BD station, BD x, BD y, BD z, B } }
//! ```
//!
//! Points are in world coordinates, one record per corridor station. A
//! corridor with several baselines keeps one such run of groups per
//! baseline, after that baseline's region and station records (see
//! [`walk`]).

use super::bits::Bits;
use crate::types::Vector3;

type Groups = Vec<Vec<Vec<(f64, Vector3)>>>;

/// How a region places stations along horizontal curves (region flags
/// 0x1000 and 0x400; a region written before its corridor was ever saved
/// with a setting holds neither and is read as at increment).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum CurveStations {
    #[default]
    AtIncrement,
    ByCurvature,
    Both,
}

/// How a region places stations along offset target curves (target flags
/// 8 and 16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TargetCurveStations {
    #[default]
    None,
    AtIncrement,
    ByCurvature,
}

/// Where a region applies its assembly.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RegionFrequency {
    pub along_tangents: f64,
    pub along_curves: f64,
    pub along_spirals: f64,
    pub along_profile_curves: f64,
    pub curves: CurveStations,
    /// Mid-ordinate distance along curves (by curvature).
    pub curve_mid_ordinate: f64,
    pub at_horizontal_geometry_points: bool,
    pub at_superelevation_critical_points: bool,
    pub at_profile_geometry_points: bool,
    pub at_profile_high_low_points: bool,
    pub at_offset_target_geometry_points: bool,
    pub adjacent_to_offset_target_start_end: bool,
    pub along_target_curves: f64,
    pub target_curves: TargetCurveStations,
    pub target_curve_mid_ordinate: f64,
}

impl Default for RegionFrequency {
    /// The application's values for a new region.
    fn default() -> Self {
        Self {
            along_tangents: 25.0,
            along_curves: 25.0,
            along_spirals: 25.0,
            along_profile_curves: 10.0,
            curves: CurveStations::AtIncrement,
            curve_mid_ordinate: 0.1,
            at_horizontal_geometry_points: false,
            at_superelevation_critical_points: false,
            at_profile_geometry_points: false,
            at_profile_high_low_points: false,
            at_offset_target_geometry_points: false,
            adjacent_to_offset_target_start_end: false,
            along_target_curves: 25.0,
            target_curves: TargetCurveStations::None,
            target_curve_mid_ordinate: 0.1,
        }
    }
}

/// What a subassembly target parameter takes: its list in the region's
/// target entry (5, 8 and 9).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TargetKind {
    /// A surface (one object).
    Surface,
    /// Width or offset: alignments, polylines, feature lines.
    Offset,
    /// Elevation or slope: profiles, feature lines, 3D polylines.
    Elevation,
}

impl TargetKind {
    pub(super) fn list(self) -> usize {
        match self {
            TargetKind::Surface => 5,
            TargetKind::Offset => 8,
            TargetKind::Elevation => 9,
        }
    }
    fn of_list(list: usize) -> Option<Self> {
        [TargetKind::Surface, TargetKind::Offset, TargetKind::Elevation].into_iter().find(|k| k.list() == list)
    }
}

/// The objects a region gives one target parameter of one subassembly.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RegionTarget {
    pub subassembly: u64,
    /// The parameter's logical name (`TargetHA`, `TargetVA`, `TargetDTM`
    /// for stock subassemblies).
    pub name: String,
    pub kind: TargetKind,
    pub objects: Vec<u64>,
    /// Use the farthest of several offset or elevation targets (the
    /// nearest otherwise).
    pub farthest: bool,
}

/// One region of a baseline.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CorridorRegion {
    pub name: String,
    pub assembly: u64,
    pub start: f64,
    pub end: f64,
    pub frequency: RegionFrequency,
    pub targets: Vec<RegionTarget>,
}

/// One baseline of a corridor.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CorridorBaseline {
    pub name: String,
    /// The feature line of a baseline drawn along one (0 otherwise); the
    /// alignment and profile are then the ones made for it.
    pub feature_line: u64,
    pub alignment: u64,
    pub profile: u64,
    pub regions: Vec<CorridorRegion>,
    /// Stations added by hand, with their descriptions, in station order.
    pub stations: Vec<(f64, String)>,
    /// Whether the record holds the baseline's built stations.
    pub built: bool,
}

/// A corridor surface as its corridor record defines it.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CorridorSurfaceDefinition {
    pub name: String,
    pub build: bool,
    /// Link codes (in name order) and whether each is added as a breakline.
    pub link_codes: Vec<(String, bool)>,
    /// Feature line codes (in name order) and the flag each carries.
    pub feature_line_codes: Vec<(String, bool)>,
    /// 0 none, 1 top links, 2 bottom links.
    pub overhang_correction: i32,
}

/// A surface boundary or mask (kinds with the 0x40000000 flag hold a
/// polygon: 0x40000002 outside, 0x40000000 or 0x40000001 inside,
/// 0x40000004 mask; 0x80000002 the corridor extents).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CorridorBoundary {
    pub surface: String,
    pub name: String,
    pub description: String,
    pub kind: u32,
    pub corners: Vec<Vector3>,
    /// Feature line components of a boundary drawn along feature lines.
    pub components: usize,
}

/// What a corridor record defines: its settings, surfaces and baselines.
/// Names and references are read for a corridor that holds no built
/// stations (their texts and references follow the built ones otherwise).
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CorridorDefinition {
    pub name: String,
    pub description: String,
    pub rebuild_automatic: bool,
    /// Regions locked to the alignment's geometry (to stations otherwise).
    pub geometry_lock: bool,
    pub maximum_triangle_side: f64,
    pub surfaces: Vec<CorridorSurfaceDefinition>,
    pub boundaries: Vec<CorridorBoundary>,
    pub baselines: Vec<CorridorBaseline>,
}

/// Feature lines of a corridor per baseline: each baseline's groups, one
/// per point code in the order of the corridor's code list, each group
/// holding that code's segments. Empty for a record [`walk`] does not read.
pub(super) fn baselines(data: &[u8], start: usize, end: usize) -> Vec<Groups> {
    walk(&mut Bits::new(data, start, end)).map(|found| found.groups).unwrap_or_default()
}

/// A corridor surface's head after its GUID:
///
/// ```text
/// B, B built, B, BL n (link codes), n x B (the code is added as a breakline)
/// BL 0, BL m (feature line codes), m x B
/// BL overhang correction (0 none, 1 top links, 2 bottom links)
/// ```
///
/// Older files write the link count in its one-byte form (`01 00000nnn`).
fn surface(bits: &mut Bits) -> Option<CorridorSurfaceDefinition> {
    bits.bit()?;
    let build = bits.bit()?;
    bits.bit()?;
    let codes = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining())?;
    let link_codes = (0..codes).map(|_| Some((String::new(), bits.bit()?))).collect::<Option<_>>()?;
    // BL 0 in every record; a record with another value is not this layout.
    (bits.bit_long()? == 0).then_some(())?;
    let feature_codes = usize::try_from(bits.bit_long()?).ok().filter(|n| *n <= bits.remaining())?;
    let feature_line_codes = (0..feature_codes).map(|_| Some((String::new(), bits.bit()?))).collect::<Option<_>>()?;
    let overhang_correction = bits.bit_long()?;
    Some(CorridorSurfaceDefinition { name: String::new(), build, link_codes, feature_line_codes, overhang_correction })
}

/// The surfaces' boundaries and masks: `BL n` (one per surface that has
/// any, in surface order), `n x { BL k, k x { BL kind, BL m, items } }`.
/// Kinds with a flag above the low 16 bits hold `m x 3BD` corners:
/// 0x80000002 the corridor extents (its outline in some files, none in
/// others), 0x40000002 an outside and 0x40000000 (0x40000001 in older
/// files) an inside boundary from a polygon, 0x40000004 a mask. Kinds
/// without flags, 2 outside and 1 inside, are drawn along feature lines and
/// hold `m` components:
///
/// ```text
/// BL, BL feature line, B reversed, BD start station, BD end station,
/// B start at the line's start, B end at its end, B, GUID
/// ```
///
/// Returns the boundaries of each list.
fn outlines(bits: &mut Bits) -> Option<Vec<Vec<CorridorBoundary>>> {
    let mut lists = Vec::new();
    for _ in 0..bits.bit_long()? {
        let mut list = Vec::new();
        for _ in 0..bits.bit_long()? {
            let kind = bits.bit_long()? as u32;
            let m = usize::try_from(bits.bit_long()?).ok().filter(|m| m * 6 <= bits.remaining())?;
            let mut boundary = CorridorBoundary { surface: String::new(), name: String::new(), description: String::new(), kind, corners: Vec::new(), components: 0 };
            if kind < 0x1_0000 {
                for _ in 0..m {
                    bits.bit_long()?;
                    bits.bit_long()?;
                    bits.bit()?;
                    bits.bit_double()?;
                    bits.bit_double()?;
                    bits.skip(3)?;
                    bits.skip(128)?;
                }
                boundary.components = m;
            } else {
                for _ in 0..m {
                    boundary.corners.push(Vector3::new(bits.bit_double()?, bits.bit_double()?, bits.bit_double()?));
                }
            }
            list.push(boundary);
        }
        lists.push(list);
    }
    Some(lists)
}

/// What a corridor without baselines holds after its `RC 32` in place of
/// the layout, the baselines and the closing run (CB01, CB02).
pub(super) const BARE_END: &str = "010101010101010101010100001011111000000100";

/// Corridor keys holding one value per region, in region order.
pub(super) const KEY_ALONG_TARGET_CURVES: u32 = 0x0C04_A621;
pub(super) const KEY_CURVE_MID_ORDINATE: u32 = 0x0C04_A61E;
pub(super) const KEY_TARGET_CURVE_MID_ORDINATE: u32 = 0x0C04_A620;

/// What [`walk`] reads: the feature line groups of each baseline, the
/// definition, and the counts naming it needs.
#[derive(Default)]
struct Walked {
    groups: Vec<Groups>,
    definition: CorridorDefinition,
    /// No baseline holds built stations.
    unbuilt: bool,
    /// Keys whose value is a text (type 0x04).
    text_keys: usize,
    /// Feature line code settings (`BL n` after the 82).
    codes: usize,
    outline_lists: Vec<Vec<CorridorBoundary>>,
    /// Per region in corridor order: nodes of each assembly group.
    region_groups: Vec<Vec<usize>>,
    /// Per region: its target entries, each a list of (list, object count)
    /// per target item, `None` for an item that holds no object.
    region_items: Vec<Vec<Vec<Option<(usize, usize, bool)>>>>,
}

/// The record read field by field:
///
/// ```text
/// head:     BL 80, 0, 80, 80, 0, BL, BL, 4 bits, RC n, n x key,
///           2 bits, GUID (zero in newer files), 2 bits, RC 82, BL n,
///           n x { 2 bits, BL } (feature line code settings), BL surfaces,
///           surfaces x { GUID, head (see [`surface`]) }, BD maximum
///           triangle side length, outlines (see [`outlines`]),
///           BL 0, B rebuild automatically, B, RC 32, RC layout (older
///           form: RC 32, RC, 2 bits, RC layout), RC baselines (top
///           5 bits), 1 bit, 6 bits region count, BL record count (both of
///           the first baseline's first region)
/// layout:   0x28: regions are locked to the alignment geometry, each region
///           block holds the identifier of an alignment element before its
///           start and its end station; 0x48: locked to stations, it does
///           not
/// baseline: [after the first: 2 bits, BL n, n x BD (the previous
///           baseline's added stations), BL region count, BL record count],
///           regions, then B, GUID, BL n, n x feature line group, B, B, GUID
/// region:   records (see [`station`]), then its block (see [`region`]),
///           which gives the next region's record count, 1 after the last;
///           the block's target entries hold as many lists as the region's
///           last applied subassembly has words, less 8 (10
///           of 18 in the samples)
/// ```
///
/// A corridor that holds no built station (its first record count is 0:
/// not rebuilt since its definition was made) has no records and no
/// feature line groups:
///
/// ```text
/// baseline: [after the first: BL region count, BL 0], region blocks
///           (without their leading B), then BL 0, B 1, 9 bits 000000101,
///           GUID, BL n, n x BD added stations
/// end:      45 bits after the last baseline
/// ```
///
/// The 2027 generator writes the same layout with two differences: an
/// unbuilt region block ends with the region's index (0, 1, 2, ... in
/// region order) where the older files end with 0, and it keeps writing
/// BL 0 before the baseline tail. A baseline whose region count is 0 (its
/// regions were deleted after the corridor was built) ends with a
/// different, so far unmapped tail that carries the baseline's added
/// stations; such a record still gives no definition.
///
/// Keys 0x0C04A621, 0x0C04A61E and 0x0C04A620 hold one double per region
/// in corridor order: the frequency along offset target curves and the mid
/// ordinates along curves and along target curves (written once a region
/// changes one of them; 25, 0.1 and 0.1 without them).
// ponytail: the 4-bit run, the 2-bit runs around the head's GUID (01 before
// it, 01 or 11 after it), the bit after the rebuild flag, the 2 bits
// opening each later built baseline (00, 01, 10 or 11), the low 3 bits of
// the second RC and the bit after it are skipped by length.
fn walk(bits: &mut Bits) -> Option<Walked> {
    let mut w = Walked::default();
    for want in [80, 0, 80, 80, 0] {
        (bits.bit_long()? == want).then_some(())?;
    }
    bits.bit_long()?;
    bits.bit_long()?;
    bits.skip(4)?;
    let mut per_region: Vec<(u32, Vec<f64>)> = Vec::new();
    for _ in 0..bits.bits_value(8)? {
        let (key, value) = super::alignment::read_key(bits)?;
        match value {
            super::alignment::KeyValue::Doubles(v) if matches!(key, KEY_ALONG_TARGET_CURVES | KEY_CURVE_MID_ORDINATE | KEY_TARGET_CURVE_MID_ORDINATE) => per_region.push((key, v)),
            _ if key >> 24 == 0x04 => w.text_keys += 1,
            _ => {}
        }
    }
    bits.skip(2 + 128 + 2)?;
    (bits.raw_short_byte()? == 82).then_some(())?;
    w.codes = usize::try_from(bits.bit_long()?).ok()?;
    for _ in 0..w.codes {
        bits.skip(2)?;
        bits.bit_long()?;
    }
    let surfaces = bits.bit_long()?;
    (0..=64).contains(&surfaces).then_some(())?;
    for _ in 0..surfaces {
        bits.skip(128)?;
        w.definition.surfaces.push(surface(bits)?);
    }
    w.definition.maximum_triangle_side = bits.bit_double()?;
    w.outline_lists = outlines(bits)?;
    (bits.bit_long()? == 0).then_some(())?;
    // The bits before the 32 are 01 or 11. Bits 00 are the top of the 32 in
    // the form some records have (written before the corridor was rebuilt;
    // the application writes the other form once it rebuilds the record):
    // `RC 32, RC, 2 bits, RC layout`. The layout is the second RC: the
    // station-locked corridors of two such records hold 0x28, then 0x48.
    let at = bits.pos;
    let older = bits.bits_value(2)? == 0;
    bits.pos = at;
    if older {
        (bits.raw_short_byte()? == 32).then_some(())?;
        bits.raw_short_byte()?;
        bits.skip(2)?;
    } else {
        w.definition.rebuild_automatic = bits.bit()?;
        bits.bit()?;
        (bits.raw_short_byte()? == 32).then_some(())?;
    }
    // A corridor without baselines ends here with a run of its own (CB01,
    // CB02).
    if !older {
        let at = bits.pos;
        if BARE_END.bytes().all(|c| bits.bit() == Some(c == b'1')) && bits.remaining() == 0 {
            w.unbuilt = true;
            return Some(w);
        }
        bits.pos = at;
    }
    let guids = match bits.raw_short_byte()? {
        0x28 => true,
        0x48 => false,
        _ => return None,
    };
    w.definition.geometry_lock = guids;
    let baselines = bits.raw_short_byte()? >> 3;
    bits.skip(1)?;
    let mut regions = u32::from(bits.bits_value(6)?);
    let mut records = u32::try_from(bits.bit_long()?).ok()?;
    w.unbuilt = records == 0;
    for baseline in 0..baselines {
        if baseline > 0 {
            if !w.unbuilt {
                bits.skip(2)?;
                let stations = added_stations(bits)?;
                if let Some(last) = w.definition.baselines.last_mut() {
                    last.stations = stations;
                }
            }
            regions = u32::try_from(bits.bit_long()?).ok()?;
            records = u32::try_from(bits.bit_long()?).ok()?;
        }
        let mut line = CorridorBaseline { name: String::new(), feature_line: 0, alignment: 0, profile: 0, regions: Vec::new(), stations: Vec::new(), built: !w.unbuilt };
        for _ in 0..regions {
            let mut words = None;
            for index in 0..records {
                words = station(bits, index > 0)?.or(words);
            }
            let (next, found) = region(bits, guids, words, !w.unbuilt, &mut w)?;
            line.regions.push(found);
            records = next;
        }
        if w.unbuilt {
            // The count after the last region block is 0 in the older
            // files and the region's index (0, 1, ...) in the 2027 files;
            // the record gives BL 0 either way.
            (bits.bit_long()? == 0).then_some(())?;
            bits.bit()?;
            bits.skip(9 + 128)?;
            line.stations = added_stations(bits)?;
            w.definition.baselines.push(line);
            w.groups.push(Vec::new());
            continue;
        }
        // A last baseline whose regions give no record count has nothing
        // built: no feature line groups follow.
        if records == 0 && baseline + 1 == baselines {
            line.built = false;
            w.definition.baselines.push(line);
            w.groups.push(Vec::new());
            break;
        }
        (records == 1).then_some(())?;
        bits.bit()?;
        bits.skip(128)?;
        let n = bits.bit_long()?;
        w.groups.push((0..n).map(|_| fields_group(bits)).collect::<Option<Groups>>()?);
        bits.bit()?;
        bits.bit()?;
        bits.skip(128)?;
        w.definition.baselines.push(line);
    }
    // The settings kept in keys, one value per region.
    let all = w.definition.baselines.iter_mut().flat_map(|b| b.regions.iter_mut());
    for (index, region) in all.enumerate() {
        for (key, values) in &per_region {
            if let Some(v) = values.get(index) {
                let f = &mut region.frequency;
                match *key {
                    KEY_ALONG_TARGET_CURVES => f.along_target_curves = *v,
                    KEY_CURVE_MID_ORDINATE => f.curve_mid_ordinate = *v,
                    _ => f.target_curve_mid_ordinate = *v,
                }
            }
        }
    }
    Some(w)
}

/// `BL n, n x BD`: a baseline's added stations (descriptions are texts).
fn added_stations(bits: &mut Bits) -> Option<Vec<(f64, String)>> {
    let n = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 2 <= bits.remaining())?;
    (0..n).map(|_| Some((bits.bit_double()?, String::new()))).collect()
}

/// `BL 80, BL 80, BL 0x04BE0000`, the head of each nested object.
fn object(bits: &mut Bits) -> Option<()> {
    (bits.bit_long()? == 80).then_some(())?;
    object_rest(bits)
}

fn object_rest(bits: &mut Bits) -> Option<()> {
    (bits.bit_long()? == 80 && bits.bit_long()? == 0x04BE_0000).then_some(())
}

/// One station: `[after a region's first: B], BD station, GUID, BL, BL, BL,
/// BL`, assembly groups (see [`assembly`]), `BL n`, `n x` applied
/// subassembly (see [`body`]), `BL 19`. Returns the last body's word count.
fn station(bits: &mut Bits, after_first: bool) -> Option<Option<u32>> {
    if after_first {
        bits.bit()?;
    }
    bits.bit_double()?;
    bits.skip(128)?;
    for _ in 0..4 {
        bits.bit_long()?;
    }
    assembly(bits)?;
    let mut words = None;
    for _ in 0..bits.bit_long()? {
        words = Some(body(bits)?);
    }
    (bits.bit_long()? == 19).then_some(words)
}

/// The assembly's groups of nodes `object, object, BL kind, BD, BD`; after a node
/// `BL 80` opens the group's next node (the rest of its first object head
/// follows), `BL 0` ends, any other value up to 79 opens the next group (1,
/// 2 and 3 in the samples).
fn assembly(bits: &mut Bits) -> Option<Vec<usize>> {
    let mut groups = Vec::new();
    loop {
        object(bits)?;
        let mut nodes = 0;
        loop {
            object(bits)?;
            bits.bit_long()?;
            bits.bit_double()?;
            bits.bit_double()?;
            nodes += 1;
            match bits.bit_long()? {
                80 => object_rest(bits)?,
                0 => {
                    groups.push(nodes);
                    return Some(groups);
                }
                1..=79 => break,
                _ => return None,
            }
        }
        groups.push(nodes);
    }
}

/// Words of an applied subassembly before its end.
const WORDS: u32 = 18;

/// One applied subassembly: `object, BL 8, object`, its shape, its parameter
/// set, then 18 words, each a count of target items or `0x8004xxxx` with no
/// items, and `BL -1` (0 when the shape and parameter set are empty).
/// Returns the number of words.
///
/// ```text
/// shape:  BL n, n x { BL, BL, BD station, BD offset, BD elevation }
///         BL m, m x { BL code, B, B, BL k, k x RL point }
///         BL a, a x { BL code, B, B, BL k, k x RL link, BL }
/// ```
// ponytail: the word list is read as 18 words and its end; every sample
// (all versions, 39084 lists) has 18. A list ending earlier with -1 is still
// read.
fn body(bits: &mut Bits) -> Option<u32> {
    object(bits)?;
    (bits.bit_long()? == 8).then_some(())?;
    object(bits)?;
    for _ in 0..bits.bit_long()? {
        bits.bit_long()?;
        bits.bit_long()?;
        for _ in 0..3 {
            bits.bit_double()?;
        }
    }
    for areas in [false, true] {
        for _ in 0..bits.bit_long()? {
            bits.bit_long()?;
            bits.bit()?;
            bits.bit()?;
            for _ in 0..bits.bit_long()? {
                bits.raw_long()?;
            }
            if areas {
                bits.bit_long()?;
            }
        }
    }
    super::parameters::read_with(bits, &mut || Some(String::new()))?;
    let mut words = 0;
    loop {
        if words == WORDS {
            return matches!(bits.bit_long()?, -1 | 0).then_some(words);
        }
        let count = bits.bit_long()?;
        match count {
            -1 => return Some(words),
            0.. => {
                for _ in 0..count {
                    target(bits)?;
                }
            }
            // ponytail: the low half of the word is skipped (0 or 4 in the
            // samples).
            _ => ((count as u32) >> 16 == 0x8004).then_some(())?,
        }
        words += 1;
    }
}

/// A target item: `BL type, BL 0`, then by the type's low byte, less its
/// 0x20 flag
///
/// ```text
/// 0x12: BL value
/// 0x13: BD value
/// 0x14: nothing (a text in the text stream)
/// 0x15: object
/// 0x17: BL k, k x object, BL option (1 the farthest, 0 the nearest; the
///       type then carries 0x100)
/// ```
///
/// then `B, B choices, [BL n, n x BL], BL index`. Types carry flags above
/// the low byte (0x1015, 0x1017, 0x1117, 0x20001117, 0x1033 in the samples).
/// Returns, for an item holding objects, their count and whether the
/// farthest is used. Its texts: the parameter's name twice, two empty
/// texts and one per object.
fn target(bits: &mut Bits) -> Option<Option<(usize, bool)>> {
    let kind = bits.bit_long()? & 0xDF;
    (bits.bit_long()? == 0).then_some(())?;
    let mut objects = None;
    match kind {
        0x12 => {
            bits.bit_long()?;
        }
        0x13 => {
            bits.bit_double()?;
        }
        0x14 => {}
        0x15 => {
            object(bits)?;
            objects = Some((1, false));
        }
        0x17 => {
            let n = usize::try_from(bits.bit_long()?).ok()?;
            for _ in 0..n {
                object(bits)?;
            }
            objects = Some((n, bits.bit_long()? == 1));
        }
        _ => return None,
    }
    bits.bit()?;
    if bits.bit()? {
        for _ in 0..bits.bit_long()? {
            bits.bit_long()?;
        }
    }
    bits.bit_long()?;
    Some(objects)
}

/// Names and references of an unbuilt corridor's definition from its texts
/// and references (see [`walk`] for their order):
///
/// ```text
/// texts:      "AeccImpCorridor", "", name, description, one per text key,
///             one per feature line code, per surface its name and its
///             link and feature line codes, per
///             boundary list its surface's name and per boundary its name
///             and description, per baseline {per region {"", per assembly
///             group its name, "" and "", "" per node, the assembly's name,
///             "", per target item its texts, the region's name}, the
///             baseline's name, one description per added station}
/// references: soft 0, style, code set style, one per feature line code,
///             per surface {the surface (soft), style, 0, render material},
///             one per boundary, per baseline {feature line or 0,
///             alignment, profile, per region {assembly, per node
///             {subassembly, the one it hangs from or 0}, per target entry
///             {subassembly (hard), per target item its objects (soft)}}}
/// ```
fn name(w: &mut Walked, strings: &[String], refs: &[u64]) -> Option<()> {
    let taken = std::cell::Cell::new(0usize);
    let text = || {
        let t = strings.get(taken.get()).cloned();
        taken.set(taken.get() + 1);
        t
    };
    text()?;
    text()?;
    w.definition.name = text()?;
    w.definition.description = text()?;
    for _ in 0..w.text_keys + w.codes {
        text()?;
    }
    for s in &mut w.definition.surfaces {
        s.name = text()?;
        for code in s.link_codes.iter_mut().chain(&mut s.feature_line_codes) {
            code.0 = text()?;
        }
    }
    for list in &mut w.outline_lists {
        let surface = text()?;
        for b in list.iter_mut() {
            b.surface = surface.clone();
            b.name = text()?;
            b.description = text()?;
        }
    }
    let mut refs = refs.iter().copied();
    let mut next = || refs.next();
    for _ in 0..3 + w.codes + 4 * w.definition.surfaces.len() + w.outline_lists.iter().map(Vec::len).sum::<usize>() {
        next()?;
    }
    let mut index = 0;
    for line in &mut w.definition.baselines {
        line.feature_line = next()?;
        line.alignment = next()?;
        line.profile = next()?;
        for region in &mut line.regions {
            text()?;
            for nodes in w.region_groups.get(index)? {
                text()?;
                text()?;
                for _ in 0..2 * nodes {
                    text()?;
                }
            }
            region.assembly = next()?;
            for _ in 0..2 * w.region_groups[index].iter().sum::<usize>() {
                next()?;
            }
            text()?;
            text()?;
            let mut found = region.targets.iter_mut();
            for entry in w.region_items.get(index)? {
                let subassembly = next()?;
                for item in entry {
                    let (list, objects, _) = (*item)?;
                    let name = text()?;
                    for _ in 0..3 + objects {
                        text()?;
                    }
                    let objects = (0..objects).map(|_| next()).collect::<Option<Vec<_>>>()?;
                    if TargetKind::of_list(list).is_some() {
                        let t = found.next()?;
                        (t.subassembly, t.name, t.objects) = (subassembly, name, objects);
                    }
                }
            }
            region.name = text()?;
            index += 1;
        }
        // The 2027 generator repeats the feature line code texts after the
        // last region, before the baseline's name (the older files do not).
        // Which of the two layouts the record holds follows from how many
        // texts are left: the baseline's name plus one text per added
        // station without the codes, their number plus the codes with them.
        // A baseline whose added stations sit in the 2027 tail this reader
        // does not map yet gives no count (`known` below its true number),
        // so the codes are taken whenever more texts remain than the
        // name's alone.
        let remaining = strings.len() - taken.get();
        let known = line.stations.len();
        if remaining > known + 1 {
            for _ in 0..w.codes {
                text()?;
            }
        }
        line.name = text()?;
        for station in &mut line.stations {
            station.1 = text()?;
        }
    }
    let outlines = std::mem::take(&mut w.outline_lists);
    w.definition.boundaries = outlines.into_iter().flatten().collect();
    Some(())
}

/// The definition of a corridor. Names and references are filled for a
/// corridor that holds no built station.
pub(super) fn definition(data: &[u8], start: usize, end: usize, strings: &[String], refs: &[u64]) -> Option<CorridorDefinition> {
    let mut w = walk(&mut Bits::new(data, start, end))?;
    if w.unbuilt {
        name(&mut w, strings, refs)?;
    } else {
        let outlines = std::mem::take(&mut w.outline_lists);
        w.definition.boundaries = outlines.into_iter().flatten().collect();
    }
    Some(w.definition)
}

/// The corridor's point codes that have feature lines, in the order of its
/// code list (the order of each baseline's feature line groups): the texts
/// after the name, the description and one text per text key.
pub(super) fn point_codes(data: &[u8], start: usize, end: usize, strings: &[String]) -> Option<Vec<String>> {
    let w = walk(&mut Bits::new(data, start, end))?;
    let from = 4 + w.text_keys;
    strings.get(from..from + w.codes).map(<[String]>::to_vec)
}

/// A region's block after its stations, returning the record count it gives
/// for what follows and the region:
///
/// ```text
/// B, GUID (the assembly's identifier), BL, BL, BL, BL, assembly groups,
/// [GUID], BD start station, [GUID], BD end station, 2 bits,
/// BD frequency along tangents, BD along curves, BL flags, BD,
/// BD along spirals, BD along profile curves, GUID (the region's),
/// BL target flags, BL, BL, BL n, n x `lists` target lists (one entry per
/// subassembly with targets, in assembly order), BL m, m x BD station,
/// B, B, B, BL count
/// ```
///
/// Flags: 0x8 at horizontal geometry points, 0x80 at superelevation
/// critical points, 0x400 by curvature and 0x1000 at increment along
/// curves, 0x20000 at profile geometry points, 0x40000 at profile high and
/// low points. Target flags: 1 at offset target geometry points, 4 adjacent
/// to offset target start and end, 8 at increment and 16 by curvature
/// along target curves. A target entry's list 5 holds its surface targets,
/// 8 its offset and 9 its elevation targets.
// ponytail: the 2-bit run is skipped by length (00 in every sample).
fn region(bits: &mut Bits, guids: bool, words: Option<u32>, lead: bool, w: &mut Walked) -> Option<(u32, CorridorRegion)> {
    if lead {
        bits.bit()?;
    }
    bits.skip(128)?;
    for _ in 0..4 {
        bits.bit_long()?;
    }
    w.region_groups.push(assembly(bits)?);
    let mut ends = [0.0; 2];
    for end in &mut ends {
        if guids {
            bits.skip(128)?;
        }
        *end = bits.bit_double()?;
    }
    bits.skip(2)?;
    let mut f = RegionFrequency { along_tangents: bits.bit_double()?, along_curves: bits.bit_double()?, ..RegionFrequency::default() };
    let flags = bits.bit_long()?;
    f.curves = match flags & 0x1400 {
        0x400 => CurveStations::ByCurvature,
        0x1400 => CurveStations::Both,
        _ => CurveStations::AtIncrement,
    };
    f.at_horizontal_geometry_points = flags & 0x8 != 0;
    f.at_superelevation_critical_points = flags & 0x80 != 0;
    f.at_profile_geometry_points = flags & 0x2_0000 != 0;
    f.at_profile_high_low_points = flags & 0x4_0000 != 0;
    bits.bit_double()?;
    f.along_spirals = bits.bit_double()?;
    f.along_profile_curves = bits.bit_double()?;
    bits.skip(128)?;
    let target_flags = bits.bit_long()?;
    f.at_offset_target_geometry_points = target_flags & 1 != 0;
    f.adjacent_to_offset_target_start_end = target_flags & 4 != 0;
    f.target_curves = match target_flags & 0x18 {
        8 => TargetCurveStations::AtIncrement,
        16 => TargetCurveStations::ByCurvature,
        _ => TargetCurveStations::None,
    };
    for _ in 0..2 {
        bits.bit_long()?;
    }
    let entries = bits.bit_long()?;
    // An unbuilt region gives no word count: 18 words, 10 lists.
    let lists = match entries {
        0 => 0,
        _ => words.unwrap_or(WORDS).checked_sub(8)?,
    };
    let mut found = Vec::new();
    let mut items = Vec::new();
    for _ in 0..entries {
        let mut entry = Vec::new();
        for list in 0..lists {
            for _ in 0..bits.bit_long()? {
                let item = target(bits)?;
                entry.push(item.map(|(objects, farthest)| (list as usize, objects, farthest)));
                if let (Some(kind), Some((objects, farthest))) = (TargetKind::of_list(list as usize), item) {
                    found.push(RegionTarget { subassembly: 0, name: String::new(), kind, objects: vec![0; objects], farthest });
                }
            }
        }
        items.push(entry);
    }
    w.region_items.push(items);
    for _ in 0..bits.bit_long()? {
        bits.bit_double()?;
    }
    bits.skip(3)?;
    let next = u32::try_from(bits.bit_long()?).ok()?;
    Some((next, CorridorRegion { name: String::new(), assembly: 0, start: ends[0], end: ends[1], frequency: f, targets: found }))
}

/// A feature line group read by its counts: `BL n`, `n x { BL m, m x { BD
/// station, BD x, BD y, BD z, B } }`.
fn fields_group(bits: &mut Bits) -> Option<Vec<Vec<(f64, Vector3)>>> {
    let n = bits.bit_long()?;
    (0..n)
        .map(|_| {
            let n = usize::try_from(bits.bit_long()?).ok().filter(|n| n * 9 <= bits.remaining())?;
            (0..n)
                .map(|_| {
                    let station = bits.bit_double()?;
                    let point = Vector3::new(bits.bit_double()?, bits.bit_double()?, bits.bit_double()?);
                    bits.bit()?;
                    Some((station, point))
                })
                .collect()
        })
        .collect()
}

/// Feature line groups of every baseline, one after another.
pub(super) fn feature_line_groups(data: &[u8], start: usize, end: usize) -> Vec<Vec<Vec<(f64, Vector3)>>> {
    baselines(data, start, end).into_iter().flatten().collect()
}

/// Every feature line segment of a corridor.
pub(super) fn feature_lines(data: &[u8], start: usize, end: usize) -> Vec<Vec<(f64, Vector3)>> {
    feature_line_groups(data, start, end).into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bits_of(s: &str) -> Vec<u8> {
        let s: Vec<u8> = s.bytes().filter(|b| *b == b'0' || *b == b'1').collect();
        s.chunks(8).map(|c| c.iter().enumerate().fold(0u8, |a, (i, b)| a | ((b - b'0') << (7 - i)))).collect()
    }

    #[test]
    fn reads_a_boundary_along_feature_lines() {
        // One list with one outside boundary (kind 2) of two components:
        // {BL 0, BL 0, B 0, BD 0, BD 0, B 1, B 1, B 0, GUID} and the same
        // on feature line 1, reversed.
        let zeros = "0".repeat(128);
        let s = format!(
            "0100000001 0100000001 0100000010 0100000010 10 10 0 10 10 110 {zeros} 10 0100000001 1 10 10 110 {zeros}"
        );
        let b = bits_of(&s);
        let len = s.bytes().filter(|c| *c == b'0' || *c == b'1').count();
        let mut bits = Bits::new(&b, 0, len);
        let lists = outlines(&mut bits).unwrap();
        assert_eq!((lists.len(), lists[0][0].kind, lists[0][0].components), (1, 2, 2));
        assert_eq!(bits.pos, len);
    }

    #[test]
    fn reads_a_surface_with_two_link_codes_and_an_outline() {
        // B, B built, RC 0x20; two link codes, both breaklines; BL 0, one
        // feature line code; BL 0 overhang; then BL 0 and one list with one
        // extents outline of one corner.
        let s = "0 1 00100000 010 1 1 10 0100000001 1 10 10 01 00000001 01 00000001
                 00 00000010 00000000 00000000 10000000 01 00000001 10 10 10";
        let b = bits_of(s);
        let len = s.bytes().filter(|c| *c == b'0' || *c == b'1').count();
        let mut bits = Bits::new(&b, 0, len);
        let surface = surface(&mut bits).unwrap();
        let flags = |c: &[(String, bool)]| c.iter().map(|c| c.1).collect::<Vec<_>>();
        assert_eq!((surface.build, flags(&surface.link_codes), flags(&surface.feature_line_codes)), (true, vec![true, true], vec![true]));
        assert_eq!(bits.bit_double(), Some(0.0));
        assert_eq!(outlines(&mut bits).unwrap()[0][0].kind, 0x8000_0002);
        assert_eq!(bits.pos, len);
    }

    #[test]
    fn reads_assembly_groups_and_a_target() {
        let bl80 = "01 01010000";
        let rest = "00 00000000 00000000 10111110 00000100";
        let object = format!("{bl80} {bl80} {rest}");
        let zeros = "10 10 10";
        // Group 1 with two nodes (BL 80 continues it), BL 1, group 2 with
        // one node, BL 0; then a 0x1017 item with one object, BL 0, B, no
        // choices, BL index 3.
        let s = format!(
            "{object} {object} {zeros} {bl80} {bl80} {rest} {object} {zeros} 01 00000001
             {object} {object} {zeros} 10
             00 00010111 00010000 00000000 00000000 10 01 00000001 {object} 10 0 0 01 00000011"
        );
        let b = bits_of(&s);
        let len = s.bytes().filter(|b| *b == b'0' || *b == b'1').count();
        let mut bits = Bits::new(&b, 0, len);
        assert_eq!(assembly(&mut bits), Some(vec![2, 1]));
        assert_eq!(target(&mut bits), Some(Some((1, false))));
        assert_eq!(bits.pos, len);
    }
}
