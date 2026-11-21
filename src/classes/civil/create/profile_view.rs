//! Profile views and the profile label groups a view shows.

use super::super::profile::ProfileViewFrame;
use super::alignment::{rewritable, rewrite_with_views};
use super::profile::identifier;
use super::site::default_style_of;
use super::support::*;
use super::bands::{BandKind, ProfileViewBand, Resolved};
use super::tree::{ensure_point_tree, style, Component, Shared, Style, Value as StyleValue, DEFAULT_STYLE_ID, KEY_STYLES};
use crate::classes::civil::alignment::KeyValue;
use crate::classes::civil::style::{view_key, view_settings_from};
use crate::classes::civil::{ProfileViewStyleSettings, ProfileViewTicks};
use crate::classes::arch::ArchEntityData;
use crate::classes::civil::ProfilePiece;
use crate::entities::{EntityCommon, EntityType, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::{Color, Handle, Vector2};
use crate::CadDocument;

/// Style tree key of the profile view style collection.
const KEY_PROFILE_VIEW_STYLES: i32 = 53;
/// Style tree key of the profile label set collection.
const KEY_PROFILE_LABEL_SETS: i32 = 96;

/// The two class words after `BL 80` and two bits `11` that every graph
/// record of the samples holds.
const GRAPH_WORDS: [i32; 2] = [357_877_592, 142_348_576];
/// The graph keys: empty lists (types 0x0C, 0x0B and 0x19, `BL 0`) and
/// the byte key 0x0301BA08 = 1.
const GRAPH_KEYS: [u32; 6] = [0x0C01_BA06, 0x0C01_BA05, 0x0B01_BA07, 0x0301_BA08, 0x1901_BA0A, 0x0C01_BA04];
/// The view's range word, stored twice: the same in every view with an
/// automatic range, plus [`USER_STATIONS`] and [`USER_ELEVATIONS`] for a
/// user-specified station or elevation range.
const VIEW_RANGE_WORD: u32 = 0x8000_0420;
const USER_STATIONS: u32 = 0x1000;
const USER_ELEVATIONS: u32 = 0x2000;
/// The title the default view style gives a view: the parent alignment's
/// name through a format code.
const TITLE: &str = "Profile View of  <[67194883(2449)]>";
/// Vertical exaggeration of the default view style.
const VERTICAL_SCALE: f64 = 10.0;
/// The view style key holding the vertical exaggeration (VS11 to VS13).
const KEY_EXAGGERATION: u32 = view_key::EXAGGERATION;
/// Text ids this crate gives the view styles it adds (the first one, then
/// one more than the highest in the collection).
const FIRST_STYLE_TEXT_ID: u32 = 0x8003_0001;
/// The default range: the stations extend this far before the
/// alignment's start and past its end; the elevations, rounded out to
/// [`ELEVATION_STEP`], this far below and above the profiles.
const STATION_MARGIN: f64 = 200.0;
/// The automatic stations are rounded out to this step (an alignment
/// ending at 211.12 gives a view to 500, PB04).
const STATION_STEP: f64 = 100.0;
const ELEVATION_STEP: f64 = 10.0;
const ELEVATION_MARGIN: f64 = 20.0;

/// A value of the view's graph record that follows its range.
#[derive(Clone, Copy)]
enum Value {
    Width,
    HalfWidth,
    Height,
    HalfHeight,
    StationMin,
    StationMax,
    ElevationMin,
    ElevationMax,
    Fixed(f64),
}

impl Value {
    fn of(self, f: &ProfileViewFrame) -> f64 {
        let (w, h) = (f.station_max - f.station_min, f.elevation_max - f.elevation_min);
        match self {
            Value::Width => w,
            Value::HalfWidth => w / 2.0,
            Value::Height => h,
            Value::HalfHeight => h / 2.0,
            Value::StationMin => f.station_min,
            Value::StationMax => f.station_max,
            Value::ElevationMin => f.elevation_min,
            Value::ElevationMax => f.elevation_max,
            Value::Fixed(v) => v,
        }
    }
}

/// A piece of the view's graph record after the vertical scale.
enum Piece {
    /// A constant run (the same in every sample).
    Bits(&'static str),
    /// A `BD` from the view's range.
    Value(Value),
    /// The view title (`BL` byte count, UTF-16 with its terminator).
    Title,
    /// An axis title in the same form.
    Text(&'static str),
    /// `BL` the number of network parts drawn in the view.
    Parts,
}

/// The graph record after the vertical scale: the graph's title and
/// frame, then its four axes (left and right titled "Elevation", bottom
/// and top titled "Station": lengths, tick spacings and title positions
/// from the range, a title offset of 1/12 and height of 1/48, the left
/// title turned 270 degrees), the range again and four values 10. The
/// constant runs are the default view style's settings: WV01 to WV04
/// differ only in the values named here.
// ponytail: the runs between the values are not split into fields; name
// them once a view with other graph settings is written.
const GRAPH: &[Piece] = &[
    Piece::Bits("10100000010000000000100000000000000000"),
    Piece::Title,
    Piece::Bits("10"),
    Piece::Value(Value::HalfWidth),
    Piece::Value(Value::Height),
    Piece::Bits("1010"),
    Piece::Value(Value::Fixed(1.0 / 6.0)),
    Piece::Bits("10"),
    Piece::Value(Value::Fixed(1.0 / 12.0)),
    Piece::Bits("10100100001000"),
    Piece::Value(Value::StationMin),
    Piece::Value(Value::ElevationMin),
    Piece::Bits("10"),
    Piece::Value(Value::StationMin),
    Piece::Value(Value::ElevationMin),
    Piece::Bits("10"),
    Piece::Value(Value::StationMax),
    Piece::Value(Value::ElevationMax),
    Piece::Bits("100100000110010000000110101010"),
    Piece::Value(Value::Height),
    Piece::Bits("1010"),
    Piece::Value(Value::Height),
    Piece::Bits("101010100100000100"),
    Piece::Text("Elevation"),
    Piece::Bits("1010"),
    Piece::Value(Value::HalfHeight),
    Piece::Bits("10"),
    Piece::Value(Value::Fixed(-1.0 / 12.0)),
    Piece::Bits("1010"),
    Piece::Value(Value::Fixed(1.0 / 48.0)),
    Piece::Bits("10"),
    Piece::Value(Value::Fixed(4.712389)),
    Piece::Bits("0100001000010000001010101010101010101010101010010000001000000000000000001010101010101010101001000001010100000100"),
    Piece::Value(Value::Width),
    Piece::Bits("101010"),
    Piece::Value(Value::Height),
    Piece::Bits("1010"),
    Piece::Value(Value::Height),
    Piece::Bits("101010100100000100"),
    Piece::Text("Elevation"),
    Piece::Bits("10"),
    Piece::Value(Value::Width),
    Piece::Value(Value::HalfHeight),
    Piece::Bits("10"),
    Piece::Value(Value::Fixed(1.0 / 12.0)),
    Piece::Bits("1010"),
    Piece::Value(Value::Fixed(1.0 / 48.0)),
    Piece::Bits("100001001010110110000001001001001101111110110010000111111001001111110100001000010000100010"),
    Piece::Value(Value::Height),
    Piece::Bits("10"),
    Piece::Value(Value::Width),
    Piece::Bits("1010"),
    Piece::Value(Value::Width),
    Piece::Bits("10101010100100000100"),
    Piece::Text("Station"),
    Piece::Bits("10"),
    Piece::Value(Value::HalfWidth),
    Piece::Value(Value::Height),
    Piece::Bits("1010"),
    Piece::Value(Value::Fixed(1.0 / 12.0)),
    Piece::Bits("10"),
    Piece::Value(Value::Fixed(1.0 / 48.0)),
    Piece::Bits("10100100001000010001000010101010101010101010101010010000001000000000000000001010101010101010101001000001010100100000101010"),
    Piece::Value(Value::Width),
    Piece::Bits("1010"),
    Piece::Value(Value::Width),
    Piece::Bits("10101010100100000100"),
    Piece::Text("Station"),
    Piece::Bits("10"),
    Piece::Value(Value::HalfWidth),
    Piece::Bits("101010"),
    Piece::Value(Value::Fixed(-1.0 / 12.0)),
    Piece::Bits("10"),
    Piece::Value(Value::Fixed(1.0 / 48.0)),
    Piece::Bits("10100100000010"),
    Piece::Value(Value::StationMin),
    Piece::Value(Value::ElevationMin),
    Piece::Bits("10"),
    Piece::Value(Value::StationMin),
    Piece::Value(Value::ElevationMin),
    Piece::Bits("10"),
    Piece::Value(Value::StationMax),
    Piece::Value(Value::ElevationMax),
    Piece::Bits("100101010100"),
    Piece::Value(Value::Fixed(10.0)),
    Piece::Value(Value::Fixed(10.0)),
    Piece::Value(Value::Fixed(10.0)),
    Piece::Value(Value::Fixed(10.0)),
    Piece::Bits("1010"),
    Piece::Parts,
    Piece::Bits("10101010010000000110010000001010010000001110010000010010100001011111000000100"),
];

/// A network part drawn in a profile view
/// (`AECC_GRAPH_PROFILE_NETWORK_PART`, in model space): a constant run
/// (DU01, DW01: every one the same); references soft 0 and the part
/// (hard); texts "AeccImpGraphProfileNetworkPart", "", "". The view lists
/// it among its items and at the end of its references.
fn write_drawn_part(doc: &mut CadDocument, common: EntityCommon, part: u64) -> Result<Handle, DxfError> {
    const RUN: &str = "01010100001001010100000101010000100000000100010111110000001000010000010010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001011111000000100";
    let strings = ["AeccImpGraphProfileNetworkPart", "", ""].iter().map(|s| s.to_string()).collect();
    put_entity(doc, "AECC_GRAPH_PROFILE_NETWORK_PART", common, |w| bits(w, RUN), Body { refs: vec![soft_pointer(0), hard_pointer(part)], strings })
}

pub(super) fn text(w: &mut DwgMergedWriter, s: &str) {
    let mut bytes: Vec<u8> = s.encode_utf16().flat_map(u16::to_le_bytes).collect();
    bytes.extend([0, 0]);
    w.write_bit_long(bytes.len() as i32);
    w.write_bytes(&bytes);
}

pub(super) fn point(w: &mut DwgMergedWriter, p: Vector2) {
    w.write_bit_double(p.x);
    w.write_bit_double(p.y);
    w.write_bit_double(0.0);
}

/// A profile a view draws: its record and its style.
struct Shown {
    handle: u64,
    style: u64,
}

/// The profile view record (`AECC_PROFILE_VIEW`):
///
/// ```text
/// BL 80, 2 bits 11, the class words, BL 5, BL 6, the graph keys,
/// BL 0 (byte form), 121 zero bits
/// 3BD origin, 9 BD (identity), BD 1
/// RC n profiles, n x 5 bits (11001 the first, 10000 the next ones)
/// BL 0, B 1, 3BD origin, BL 2, BL text id
/// 3BD min, 3BD min, 3BD max, twice (station, elevation, 0)
/// BL 528, the title (BL byte count, UTF-16), BL 0
/// 7 BD (half width, height, 0, 0, 1/6, 0, 1/12)
/// BL 0, 0, 8, 1, 2, text id
/// 3BD origin, BD 0, 0, 0, 1, BD vertical scale, then [`GRAPH`]
/// references: soft 0, hard owner 0, style (hard), alignment (hard), per
///   profile {profile (soft), its style (hard), hard 0}, hard 0 x 4,
///   soft 0, hard 0
/// texts: "", "", name, "", name, ""
/// ```
///
/// Written with the samples' origins, ranges and profiles the record
/// equals theirs bit for bit.
#[allow(clippy::too_many_arguments)]
fn write_view(doc: &mut CadDocument, common: EntityCommon, name: &str, frame: &ProfileViewFrame, user: (bool, bool), style: u64, alignment: Handle, profiles: &[Shown], bands: &[Resolved], parts: &[u64]) -> Result<Handle, DxfError> {
    let word = (VIEW_RANGE_WORD | if user.0 { USER_STATIONS } else { 0 } | if user.1 { USER_ELEVATIONS } else { 0 }) as i32;
    let mut refs: Vec<Ref> = bands.iter().map(|_| hard_pointer(0)).collect();
    refs.extend([soft_pointer(0), Ref(DwgReferenceType::HardOwnership, 0), hard_pointer(style), hard_pointer(alignment.value())]);
    for p in profiles {
        refs.extend([soft_pointer(p.handle), hard_pointer(p.style), hard_pointer(0)]);
    }
    for g in parts {
        refs.extend([soft_pointer(*g), hard_pointer(0), hard_pointer(0)]);
    }
    for b in bands {
        refs.extend([hard_pointer(b.style), soft_pointer(b.band.data_source.map_or(0, |h| h.value())), hard_pointer(alignment.value()), hard_pointer(b.profile1), hard_pointer(b.profile2)]);
    }
    refs.extend([hard_pointer(0), hard_pointer(0), hard_pointer(0), hard_pointer(0), soft_pointer(0), hard_pointer(0)]);
    refs.extend(parts.iter().map(|g| soft_pointer(*g)));
    let strings = ["", "", name, "", name, ""].iter().map(|s| s.to_string()).collect();
    let origin = frame.origin;
    let lo = Vector2::new(frame.station_min, frame.elevation_min);
    let hi = Vector2::new(frame.station_max, frame.elevation_max);
    put_entity(
        doc,
        "AECC_PROFILE_VIEW",
        common,
        |w| {
            w.write_bit_long(80);
            bits(w, "11");
            for v in GRAPH_WORDS {
                w.write_bit_long(v);
            }
            w.write_bit_long(5);
            w.write_bit_long(GRAPH_KEYS.len() as i32);
            for k in GRAPH_KEYS {
                w.write_bit_long(k as i32);
                let values = super::bands::key_values(k, bands);
                match k >> 24 {
                    3 => bits(w, "00000001"),
                    0x0C => {
                        w.write_bit_long(values.len() as i32);
                        values.iter().for_each(|v| w.write_bit_double(*v));
                    }
                    _ => {
                        w.write_bit_long(values.len() as i32);
                        values.iter().for_each(|v| w.write_bit_long(*v as i32));
                    }
                }
            }
            bl_byte(w, 0);
            bits(w, &"0".repeat(121));
            point(w, origin);
            for v in [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 1.0] {
                w.write_bit_double(v);
            }
            bits(w, &format!("{:08b}", profiles.len() + parts.len()));
            for i in 0..profiles.len() + parts.len() {
                bits(w, if i == 0 { "11001" } else { "10000" });
            }
            w.write_bit_long(bands.len() as i32);
            super::bands::write_items(w, bands);
            w.write_bit(true);
            point(w, origin);
            w.write_bit_long(2);
            w.write_bit_long(word);
            for p in [lo, lo, hi, lo, lo, hi] {
                point(w, p);
            }
            w.write_bit_long(528);
            text(w, TITLE);
            w.write_bit_long(0);
            for v in [(hi.x - lo.x) / 2.0, hi.y - lo.y, 0.0, 0.0, 1.0 / 6.0, 0.0, 1.0 / 12.0] {
                w.write_bit_double(v);
            }
            for v in [0, 0, 8, 1, 2, word] {
                w.write_bit_long(v);
            }
            point(w, origin);
            for v in [0.0, 0.0, 0.0, 1.0, frame.vertical_scale] {
                w.write_bit_double(v);
            }
            for piece in GRAPH {
                match piece {
                    Piece::Bits(b) => bits(w, b),
                    Piece::Value(v) => w.write_bit_double(v.of(frame)),
                    Piece::Title => text(w, TITLE),
                    Piece::Text(t) => text(w, t),
                    Piece::Parts => w.write_bit_long(parts.len() as i32),
                }
            }
        },
        Body { refs, strings },
    )
}

/// The run every profile label group opens with (the four kinds share it).
/// The run band label groups open with too.
pub(super) const GROUP_HEADER: &str = HEADER;
const HEADER: &str = "01010100001001010100000101010000100000000100010111110000001000010000010010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000101010000010101000000000000000000000010111110000001000011111111111111111111111100000000010";
/// A label of a line label group, before and after its tangent's entity
/// id (`BL`).
const LINE: (&str, &str) = ("010000001100000110101111111100000000000110100000000000001100001111110000000000000000100000000000000000000000000000000000000000000000000011100000001111110000110001111111000000000000000001", "000000000100000000100010000000000000000000000100000010000011000111111100000000000000000100000110101111111100000000000110101010");
/// A label of a crest or sag curve label group, around its curve's id.
const CURVE: (&str, &str) = ("01000000100000011010111111110000000000011010000000000000110100111111000000000000000001", "000000000100000001000010000000000000000000000100000010000011010011111100000000000000000100000110101111111100000000000110101010");
/// The two labels of a PVI label group: at the profile's start (around its
/// first tangent's id) and at its end (around its last tangent's id).
const PVI_START: (&str, &str) = ("01000000110000110011111111000000000000000001", "0000011010111111110000000000011010000000000000110010111111000000000000000001100000000001000000001100100000000000000000000001000000110000110010111111000000000000000001000011001111111100000000000000000100000110101111111100000000000110101010");
const PVI_END: (&str, &str) = ("01000000110000011010111111110000000000011010000000000000110011111111000000000000000001100000110010111111000000000000000001", "0000000001000000001100100000000000000000000001000000110000110010111111000000000000000001000011001111111100000000000000000100000110101111111100000000000110101010");
/// What follows the labels, by kind of group.
const LINE_END: &str = "0100000001000001001010110100000000000000111101010001010100000001000000000000000000000000000000000000000000000000001100000000111111101000010101010101010101010101010101010101010101010101101101010011111100010101010101010101010101010101010101010101010101110001010011111111110100101010000010101000000000000000000000010111110000001000001011111000000100";
const CURVE_END: &str = "0100000001000001001110110100000000000000111101010001010100000001000000000000000000000000000000000000000000000000001100000000111111101000010101010101010101010101010101010101010101010101101101010011111100010101010101010101010101010101010101010101010101110001010011111111110100101010000010101000000000000000000000010111110000001000001011111000000100";
const PVI_END_GROUP: &str = "01000000010000010100101101000000000000001111010100010101000000010000000000000000000000000000000000000000000000000011000000001111110000000000000000000000000000000000000000000000000001011001010000001000010101010101010101010101010101010101010101010101101101010011111100010101010101010101010101010101010101010101010101110001010011111111110100101010000010101000000000000000000000010111110000001000001011111000000100";

/// The profile label groups a view gets for each profile.
#[derive(Clone, Copy)]
enum Group {
    Line,
    Pvi,
    Sag,
    Crest,
}

impl Group {
    const ALL: [Group; 4] = [Group::Line, Group::Pvi, Group::Sag, Group::Crest];

    fn dxf(self) -> &'static str {
        match self {
            Group::Line => "AECC_VALIGNMENT_LINE_LABEL_GROUP",
            Group::Pvi => "AECC_VALIGNMENT_PVI_LABEL_GROUP",
            Group::Sag => "AECC_VALIGNMENT_SAGCURVE_LABEL_GROUP",
            Group::Crest => "AECC_VALIGNMENT_CRESTCURVE_LABEL_GROUP",
        }
    }

    fn class_text(self) -> &'static str {
        match self {
            Group::Line => "AeccImpVAlignmentLineLabeling",
            Group::Pvi => "AeccImpVAlignmentPVILabeling",
            Group::Sag => "AeccImpVAlignmentSagCurveLabeling",
            Group::Crest => "AeccImpVAlignmentCrestCurveLabeling",
        }
    }

    /// Index of the group's label style among the profile label set's
    /// references (line, PVI, sag curve, crest curve after a null one).
    fn style_index(self) -> usize {
        match self {
            Group::Line => 1,
            Group::Pvi => 2,
            Group::Sag => 3,
            Group::Crest => 4,
        }
    }
}

/// A profile label group (an entity in model space):
///
/// ```text
/// HEADER, BL n, n labels (the kind's template around the labelled
/// entity's id), the kind's end
/// references: soft 0, soft 0, profile (soft), label style (hard), view
///   (soft), per label {profile (hard), view (soft)}
/// texts: the kind's class text, four empty texts
/// ```
///
/// The groups of WV01, WV02 and WV03 (two and four line labels, the PVI
/// labels, none to two curve labels) are reproduced bit for bit.
fn write_group(doc: &mut CadDocument, common: EntityCommon, kind: Group, ids: &[i32], profile: u64, style: u64, view: u64) -> Result<Handle, DxfError> {
    let mut refs = vec![soft_pointer(0), soft_pointer(0), soft_pointer(profile), hard_pointer(style), soft_pointer(view)];
    for _ in ids {
        refs.extend([hard_pointer(profile), soft_pointer(view)]);
    }
    let mut strings = vec![kind.class_text().to_string()];
    strings.extend((0..4).map(|_| String::new()));
    put_entity(
        doc,
        kind.dxf(),
        common,
        |w| {
            bits(w, HEADER);
            w.write_bit_long(ids.len() as i32);
            for (i, id) in ids.iter().enumerate() {
                let (before, after) = match kind {
                    Group::Line => LINE,
                    Group::Sag | Group::Crest => CURVE,
                    Group::Pvi if i == 0 => PVI_START,
                    Group::Pvi => PVI_END,
                };
                bits(w, before);
                w.write_bit_long(*id);
                bits(w, after);
            }
            bits(
                w,
                match kind {
                    Group::Line => LINE_END,
                    Group::Pvi => PVI_END_GROUP,
                    Group::Sag | Group::Crest => CURVE_END,
                },
            );
        },
        Body { refs, strings },
    )
}

/// The settings of the application's default profile view style, in
/// stored order: the graph's title, axes, ticks, grid and annotation
/// settings (the 16 keys of type 0x10 refer to the text style "Standard";
/// the type 0x18 texts are label format codes).
fn view_style_keys() -> Vec<(u32, StyleValue)> {
    vec![
        (0x1001_6114, StyleValue::Reference(Shared::Standard)),
        (0x1001_6301, StyleValue::Reference(Shared::Standard)),
        (0x0301_9001, StyleValue::Byte(0)),
        (0x0301_650A, StyleValue::Byte(0)),
        (0x0D01_6429, StyleValue::Long(2)),
        (0x0901_6320, StyleValue::Point([0.0, 0.0, 0.0])),
        (0x0201_630C, StyleValue::Double(100.0)),
        (0x0201_620D, StyleValue::Double(1.0 / 48.0)),
        (0x0201_610E, StyleValue::Double(2.5)),
        (0x0101_600F, StyleValue::Long(3)),
        (0x1801_640A, StyleValue::Wide("<[33607712(2097|2474|2387|2433|2593|2609|2657|2674|2689|2706|2721|2707|2803|2817)]>")),
        (0x0D01_6228, StyleValue::Long(2)),
        (0x0D01_6327, StyleValue::Long(2)),
        (0x0301_6507, StyleValue::Byte(1)),
        (0x0D01_6426, StyleValue::Long(5)),
        (0x0201_6408, StyleValue::Double(1.0 / 120.0)),
        (0x0201_631A, StyleValue::Double(0.0)),
        (0x1801_630B, StyleValue::Wide("<[33607712(2097|2474|2387|2433|2593|2609|2657|2674|2689|2706|2721|2707|2803|2817)]>")),
        (0x0201_6309, StyleValue::Double(1.0 / 240.0)),
        (0x0101_600C, StyleValue::Long(2)),
        (0x0400_1008, StyleValue::Text("")),
        (0x1801_610A, StyleValue::Wide("<[33652743(2097|2387|2433|2593|2609|2657|2803|2817)]>")),
        (0x0202_1124, StyleValue::Double(1.0 / 120.0)),
        (0x0D01_6500, StyleValue::Long(8)),
        (0x0D01_6412, StyleValue::Long(5)),
        (0x0901_621B, StyleValue::Point([0.0, 0.0, 0.0])),
        (0x0201_6207, StyleValue::Double(1.0 / 48.0)),
        (0x0D01_6203, StyleValue::Long(1)),
        (0x0201_6108, StyleValue::Double(1.0 / 120.0)),
        (0x0201_6009, StyleValue::Double(1.0 / 48.0)),
        (0x1001_6419, StyleValue::Reference(Shared::Standard)),
        (0x0301_6501, StyleValue::Byte(0)),
        (0x0201_6424, StyleValue::Double(1.0 / 120.0)),
        (0x0201_6413, StyleValue::Double(0.0)),
        (0x0901_6416, StyleValue::Point([0.0, 0.0, 0.0])),
        (0x0D01_6310, StyleValue::Long(5)),
        (0x0201_6215, StyleValue::Double(0.0)),
        (0x0D01_6211, StyleValue::Long(5)),
        (0x0D01_6112, StyleValue::Long(5)),
        (0x0201_6017, StyleValue::Double(2.0)),
        (0x1001_6119, StyleValue::Reference(Shared::Standard)),
        (0x1801_6302, StyleValue::Wide("Station")),
        (0x0901_6204, StyleValue::Point([1.0 / 12.0, 0.0, 0.0])),
        (0x0201_6113, StyleValue::Double(4.712389)),
        (0x0901_6116, StyleValue::Point([0.0, 0.0, 0.0])),
        (0x0D01_6010, StyleValue::Long(5)),
        (0x0201_6014, StyleValue::Double(0.0)),
        (0x0400_1010, StyleValue::Text("")),
        (0x0100_1017, StyleValue::Long(53)),
        (0x1001_6314, StyleValue::Reference(Shared::Standard)),
        (0x0301_650C, StyleValue::Byte(1)),
        (0x0201_640D, StyleValue::Double(1.0 / 48.0)),
        (0x0201_631F, StyleValue::Double(0.0)),
        (0x0201_630E, StyleValue::Double(25.0)),
        (0x0201_620F, StyleValue::Double(1.0 / 120.0)),
        (0x0901_6003, StyleValue::Point([0.0, 1.0 / 6.0, 0.0])),
        (0x0101_6011, StyleValue::Long(2)),
        (0x1002_111E, StyleValue::Reference(Shared::Standard)),
        (0x1001_6201, StyleValue::Reference(Shared::Standard)),
        (0x0301_6509, StyleValue::Byte(1)),
        (0x0D01_6428, StyleValue::Long(2)),
        (0x0D01_6505, StyleValue::Long(0)),
        (0x0D01_6329, StyleValue::Long(2)),
        (0x0201_620C, StyleValue::Double(10.0)),
        (0x0201_610D, StyleValue::Double(1.0 / 48.0)),
        (0x0101_600E, StyleValue::Long(2)),
        (0x1801_630A, StyleValue::Wide("<[33607712(2097|2474|2387|2433|2593|2609|2657|2674|2689|2706|2721|2707|2803|2817)]>")),
        (0x1001_641E, StyleValue::Reference(Shared::Standard)),
        (0x0D01_6227, StyleValue::Long(2)),
        (0x0D01_6128, StyleValue::Long(2)),
        (0x0301_6506, StyleValue::Byte(1)),
        (0x0D01_6502, StyleValue::Long(32)),
        (0x0901_641B, StyleValue::Point([0.0, 0.0, 0.0])),
        (0x0201_6407, StyleValue::Double(1.0 / 48.0)),
        (0x0D01_6403, StyleValue::Long(1)),
        (0x0D01_6326, StyleValue::Long(5)),
        (0x0201_6308, StyleValue::Double(1.0 / 120.0)),
        (0x0201_621A, StyleValue::Double(0.0)),
        (0x1801_620B, StyleValue::Wide("<[33652743(2097|2387|2433|2593|2609|2657|2803|2817)]>")),
        (0x0201_6209, StyleValue::Double(1.0 / 240.0)),
        (0x0201_600B, StyleValue::Double(1.0 / 12.0)),
        (0x0D01_6007, StyleValue::Long(1)),
        (0x1802_1125, StyleValue::Wide("<[33640457(2097|2387|2433|2593|2609|2657|2803|2817)]>")),
        (0x0D02_1129, StyleValue::Long(2)),
        (0x0202_1123, StyleValue::Double(1.0 / 240.0)),
        (0x0301_6503, StyleValue::Byte(1)),
        (0x0201_6415, StyleValue::Double(0.0)),
        (0x0D01_6411, StyleValue::Long(5)),
        (0x0D01_6312, StyleValue::Long(5)),
        (0x0901_611B, StyleValue::Point([0.0, 0.0, 0.0])),
        (0x0201_6107, StyleValue::Double(1.0 / 48.0)),
        (0x0D01_6103, StyleValue::Long(1)),
        (0x1801_600A, StyleValue::Wide("Profile View of  <[67194883(2449)]>")),
        (0x0301_6008, StyleValue::Byte(0)),
        (0x0201_6019, StyleValue::Double(2.0)),
        (0x1801_6425, StyleValue::Wide("<[33607712(2097|2474|2387|2433|2593|2609|2657|2674|2689|2706|2721|2707|2803|2817)]>")),
        (0x1001_6319, StyleValue::Reference(Shared::Standard)),
        (0x0D02_1126, StyleValue::Long(5)),
        (0x0201_6423, StyleValue::Double(1.0 / 240.0)),
        (0x0901_6404, StyleValue::Point([0.0, -0.08333333333333333, 0.0])),
        (0x0201_6324, StyleValue::Double(1.0 / 120.0)),
        (0x0201_6313, StyleValue::Double(0.0)),
        (0x0901_6316, StyleValue::Point([0.0, 0.0, 0.0])),
        (0x0D01_6210, StyleValue::Long(5)),
        (0x0201_6115, StyleValue::Double(0.0)),
        (0x0D01_6111, StyleValue::Long(5)),
        (0x0201_6016, StyleValue::Double(0.0)),
        (0x0902_1120, StyleValue::Point([0.0, 0.0, 0.0])),
        (0x0201_640F, StyleValue::Double(1.0 / 120.0)),
        (0x1801_6202, StyleValue::Wide("Elevation")),
        (0x0901_6104, StyleValue::Point([-0.08333333333333333, 0.0, 0.0])),
        (0x0201_6013, StyleValue::Double(0.0)),
        (0x1600_101F, StyleValue::Time),
        (0x1001_6214, StyleValue::Reference(Shared::Standard)),
        (0x1001_6401, StyleValue::Reference(Shared::Standard)),
        (0x0301_650B, StyleValue::Byte(1)),
        (0x0901_6420, StyleValue::Point([0.0, 0.0, 0.0])),
        (0x0201_640C, StyleValue::Double(100.0)),
        (0x0201_630D, StyleValue::Double(1.0 / 48.0)),
        (0x0201_620E, StyleValue::Double(2.5)),
        (0x0201_610F, StyleValue::Double(1.0 / 120.0)),
        (0x1001_6101, StyleValue::Reference(Shared::Standard)),
        (0x1001_6002, StyleValue::Reference(Shared::Standard)),
        (0x0301_6508, StyleValue::Byte(0)),
        (0x0D01_6427, StyleValue::Long(2)),
        (0x0D01_6328, StyleValue::Long(2)),
        (0x0D01_6504, StyleValue::Long(1)),
        (0x0201_641A, StyleValue::Double(0.0)),
        (0x1801_640B, StyleValue::Wide("<[33607712(2097|2474|2387|2433|2593|2609|2657|2674|2689|2706|2721|2707|2803|2817)]>")),
        (0x0201_6409, StyleValue::Double(1.0 / 240.0)),
        (0x0201_610C, StyleValue::Double(10.0)),
        (0x0101_600D, StyleValue::Long(2)),
        (0x0400_101A, StyleValue::Text("AeccDbGraphProfile")),
        (0x1801_620A, StyleValue::Wide("<[33652743(2097|2387|2433|2593|2609|2657|2803|2817)]>")),
        (0x1001_631E, StyleValue::Reference(Shared::Standard)),
        (0x0D01_6127, StyleValue::Long(2)),
        (0x0901_631B, StyleValue::Point([0.0, 0.0, 0.0])),
        (0x0201_6307, StyleValue::Double(1.0 / 48.0)),
        (0x0D01_6303, StyleValue::Long(1)),
        (0x0201_6208, StyleValue::Double(1.0 / 120.0)),
        (0x0201_611A, StyleValue::Double(0.0)),
        (0x1801_610B, StyleValue::Wide("<[33652743(2097|2387|2433|2593|2609|2657|2803|2817)]>")),
        (0x0201_6109, StyleValue::Double(1.0 / 240.0)),
        (0x0D01_6006, StyleValue::Long(0)),
        (0x0201_600A, StyleValue::Double(2.0)),
        (0x0D01_6410, StyleValue::Long(5)),
        (0x0201_6315, StyleValue::Double(0.0)),
        (0x0D01_6311, StyleValue::Long(5)),
        (0x0D01_6212, StyleValue::Long(5)),
        (0x0201_6018, StyleValue::Double(2.0)),
        (0x1801_6325, StyleValue::Wide("<[33607712(2097|2474|2387|2433|2593|2609|2657|2674|2689|2706|2721|2707|2803|2817)]>")),
        (0x1001_6219, StyleValue::Reference(Shared::Standard)),
        (0x0202_111F, StyleValue::Double(0.0)),
        (0x1801_6402, StyleValue::Wide("Station")),
        (0x0201_6323, StyleValue::Double(1.0 / 240.0)),
        (0x0901_6304, StyleValue::Point([0.0, 1.0 / 12.0, 0.0])),
        (0x0201_6213, StyleValue::Double(1.5707963)),
        (0x0901_6216, StyleValue::Point([0.0, 0.0, 0.0])),
        (0x0D01_6110, StyleValue::Long(5)),
        (0x0201_6015, StyleValue::Double(0.0)),
        (0x1001_6414, StyleValue::Reference(Shared::Standard)),
        (0x0201_641F, StyleValue::Double(0.0)),
        (0x0201_640E, StyleValue::Double(25.0)),
        (0x0201_630F, StyleValue::Double(1.0 / 120.0)),
        (0x1801_6102, StyleValue::Wide("Elevation")),
        (0x0101_6012, StyleValue::Long(2)),
        (0x0201_6001, StyleValue::Double(10.0)),
        (0x1600_101E, StyleValue::Time),
    ]
}

/// The plan components of the default profile view style: `(index,
/// visible, color index)`, 256 for ByLayer; lineweight and linetype
/// ByBlock, linetype scale 1.
const VIEW_STYLE_COMPONENTS: [(i32, bool, u16); 36] = [(0, true, 4), (1, true, 3), (2, true, 3), (3, true, 3), (4, false, 3), (5, true, 3), (6, true, 3), (7, true, 3), (8, true, 3), (9, true, 3), (10, false, 3), (11, true, 3), (12, true, 3), (13, true, 2), (14, true, 2), (15, true, 2), (16, false, 2), (17, true, 2), (18, true, 2), (19, true, 2), (20, true, 2), (21, true, 2), (22, false, 2), (23, true, 2), (24, true, 2), (37, true, 9), (38, false, 8), (39, true, 9), (40, false, 8), (44, true, 4), (47, true, 2), (48, true, 2), (49, true, 2), (50, true, 2), (53, false, 6), (54, true, 256)];

/// A profile view style (`AECC_PROFILE_VIEW_STYLE`): the style layout of
/// `tree.rs` with [`view_style_keys`] and the plan components
/// [`VIEW_STYLE_COMPONENTS`], no class data. With these the collection
/// (style tree key 53, listing the default style) and the default style
/// (id 30) equal WV01's records apart from the dates and the users.
#[allow(clippy::too_many_arguments)]
fn view_style(doc: &mut CadDocument, handle: Handle, owner: Handle, id: i32, children: &[(i32, Handle)], own: u64, layer: u64, by_block: u64) -> Result<(), DxfError> {
    view_style_named(doc, handle, owner, id, "", &ProfileViewStyleSettings::default(), children, own, layer, by_block)
}

/// The application's default profile view style settings (the values of
/// [`view_style_keys`]; text styles 0, the drawing's "Standard").
impl Default for ProfileViewStyleSettings {
    fn default() -> Self {
        let keys = view_style_keys();
        let get = |key: u32| match keys.iter().find(|(k, _)| *k == key)?.1 {
            StyleValue::Double(v) => Some(KeyValue::Double(v)),
            StyleValue::Long(v) => Some(KeyValue::Long(v)),
            StyleValue::Byte(v) => Some(KeyValue::Byte(v)),
            StyleValue::Point(p) => Some(KeyValue::Vector(p)),
            StyleValue::Wide(t) => Some(KeyValue::Text(t.to_string())),
            _ => None,
        };
        view_settings_from(&get, &|_| Some(0)).expect("the default profile view style holds every setting")
    }
}

/// The keys of `settings` (see [`view_key`]): a text style 0 is the
/// drawing's "Standard".
fn view_settings_values(s: &ProfileViewStyleSettings) -> Vec<(u32, StyleValue)> {
    use view_key::*;
    let style_of = |h: u64| StyleValue::Reference(if h == 0 { Shared::Standard } else { Shared::Record(h) });
    let point = |p: [f64; 2]| StyleValue::Point([p[0], p[1], 0.0]);
    let flag = |b: bool| StyleValue::Byte(u8::from(b));
    let t = &s.title;
    let mut out = vec![
        (EXAGGERATION, StyleValue::Double(s.vertical_exaggeration)),
        (RIGHT_TO_LEFT, flag(s.right_to_left)),
        (TITLE_TEXT, StyleValue::WideString(t.text.clone())),
        (TITLE_STYLE, style_of(t.text_style)),
        (TITLE_OFFSET, point(t.offset)),
        (TITLE_LOCATION, StyleValue::Long(t.location)),
        (TITLE_JUSTIFICATION, StyleValue::Long(t.justification)),
        (TITLE_BORDER, flag(t.border)),
        (TITLE_BORDER_GAP, StyleValue::Double(t.border_gap)),
        (TITLE_HEIGHT, StyleValue::Double(t.text_height)),
    ];
    for (keys, values) in [(PADDING, s.grid.padding), (AXIS_OFFSET, s.grid.axis_offset)] {
        out.extend(keys.into_iter().zip(values).map(|(k, v)| (k, StyleValue::Double(v))));
    }
    for (keys, l) in GRID_LINES.into_iter().zip([s.grid.horizontal, s.grid.vertical]) {
        out.extend(keys.into_iter().zip([l.omit_in_padding, l.clip_to_highest_profile, l.clip]).map(|(k, v)| (k, flag(v))));
    }
    for (g, a) in (1..).zip([&s.left, &s.right, &s.top, &s.bottom]) {
        if let Some(k) = SHOW[g as usize - 1] {
            out.push((k, flag(a.show_ticks_and_labels)));
        }
        let at = |ty: u32, item: u32| axis(ty, g, item);
        let tt = &a.title;
        out.extend([
            (at(0x18, TITLE[0]), StyleValue::WideString(tt.text.clone())),
            (at(0x10, TITLE[1]), style_of(tt.text_style)),
            (at(0x0D, TITLE[2]), StyleValue::Long(tt.location)),
            (at(0x09, TITLE[3]), point(tt.offset)),
            (at(0x02, TITLE[4]), StyleValue::Double(tt.text_height)),
            (at(0x02, TITLE[5]), StyleValue::Double(tt.rotation)),
        ]);
        let ticks: [(Option<&ProfileViewTicks>, [u32; 8]); 3] = [(Some(&a.major), TICKS[0]), (Some(&a.minor), TICKS[1]), (a.geometry.as_ref().filter(|_| g >= 3), TICKS[2])];
        for (k, items) in ticks.into_iter().filter_map(|(k, items)| Some((k?, items))) {
            if items[0] != 0 {
                out.push((at(0x02, items[0]), StyleValue::Double(k.interval)));
            }
            out.extend([
                (at(0x02, items[1]), StyleValue::Double(k.size)),
                (at(0x02, items[2]), StyleValue::Double(k.text_height)),
                (at(0x18, items[3]), StyleValue::WideString(k.label.clone())),
                (at(0x10, items[4]), style_of(k.text_style)),
                (at(0x09, items[5]), point(k.offset)),
                (at(0x02, items[6]), StyleValue::Double(k.rotation)),
                (at(0x0D, items[7]), StyleValue::Long(k.justification)),
            ]);
        }
    }
    out
}

/// [`view_style`] named `name` (with a text id) with `settings`.
#[allow(clippy::too_many_arguments)]
fn view_style_named(doc: &mut CadDocument, handle: Handle, owner: Handle, id: i32, name: &str, settings: &ProfileViewStyleSettings, children: &[(i32, Handle)], own: u64, layer: u64, by_block: u64) -> Result<(), DxfError> {
    let plan = VIEW_STYLE_COMPONENTS
        .iter()
        .map(|&(index, visible, color)| Component {
            index,
            visible,
            color: if color == 256 { Color::ByLayer } else { Color::Index(color as u8) },
            lineweight: -2,
            linetype: by_block,
            scale: 1.0,
        })
        .collect();
    let mut values = view_settings_values(settings);
    let keys: Vec<(u32, StyleValue)> = view_style_keys()
        .into_iter()
        .map(|(key, value)| match values.iter().position(|(k, _)| *k == key) {
            Some(i) => values.swap_remove(i),
            None => (key, value),
        })
        .collect();
    let st = Style { name, dxf: "AECC_PROFILE_VIEW_STYLE", id, children, keys: &keys, own, pointer_table: &[], lists: [plan, Vec::new(), Vec::new(), Vec::new()], class_refs: Vec::new() };
    style(doc, handle, owner, st, layer, |_| {})
}

/// Writes the profile view style collection and its default style when
/// the drawing lacks them (and the point tree of a drawing without a
/// style tree).
fn ensure_view_style_tree(doc: &mut CadDocument) -> Result<(), DxfError> {
    if root_node(doc, KEY_STYLES).is_none() && !ensure_point_tree(doc)? {
        return Err(invalid("drawing has a civil tree without a style tree"));
    }
    let styles = root_node(doc, KEY_STYLES).ok_or_else(|| invalid("drawing has no style tree"))?;
    let tree = read_node(doc, styles).ok_or_else(|| invalid("style tree cannot be read"))?;
    if tree.children.iter().any(|c| c.0 == KEY_PROFILE_VIEW_STYLES) {
        return Ok(());
    }
    let layer = doc.layers.get("0").map(|l| l.handle.value()).ok_or_else(|| invalid("drawing has no layer 0"))?;
    let by_block = doc.line_types.get("ByBlock").map(|l| l.handle.value()).ok_or_else(|| invalid("drawing lacks the ByBlock linetype"))?;
    let [head, default] = [(); 2].map(|_| doc.allocate_handle());
    view_style(doc, head, styles, KEY_PROFILE_VIEW_STYLES, &[(DEFAULT_STYLE_ID, default)], default.value(), layer, by_block)?;
    view_style(doc, default, head, DEFAULT_STYLE_ID, &[], 0, layer, by_block)?;
    add_node_child(doc, styles, KEY_PROFILE_VIEW_STYLES, head, "")
}

fn arch(doc: &CadDocument, h: u64) -> Option<&ArchEntityData> {
    match doc.get_entity(Handle::new(h))? {
        EntityType::Extended(x) => match &x.data {
            ExtendedEntityData::Arch(a) => Some(a),
            _ => None,
        },
        _ => None,
    }
}

/// The ids a profile's four groups label: its tangents (link order), its
/// first and last tangent (the PVI labels), its sag curves and its crest
/// curves. A curve is a crest where the grade falls across it.
fn labelled(profile: &ArchEntityData) -> [Vec<i32>; 4] {
    let ids = profile.profile_entity_ids();
    let tangents: Vec<i32> = ids.iter().filter(|e| !e.1).map(|e| e.0).collect();
    let grades: Vec<f64> = profile
        .profile_pieces()
        .iter()
        .filter(|p| p.0 == ProfilePiece::Tangent)
        .map(|(_, p)| (p[p.len() - 1].y - p[0].y) / (p[p.len() - 1].x - p[0].x))
        .collect();
    let (mut sag, mut crest) = (Vec::new(), Vec::new());
    let mut seen = 0;
    for (id, curve, _) in &ids {
        if !curve {
            seen += 1;
            continue;
        }
        // Between the tangent before (`seen - 1`) and the one after.
        match (grades.get(seen.max(1) - 1), grades.get(seen)) {
            (Some(a), Some(b)) if b > a => sag.push(*id),
            _ => crest.push(*id),
        }
    }
    let ends = match (tangents.first(), tangents.last()) {
        (Some(a), Some(b)) => vec![*a, *b],
        _ => Vec::new(),
    };
    [tangents, ends, sag, crest]
}

/// The range a new view gives the alignment and its profiles (see
/// [`STATION_MARGIN`]); WV01 (alignment 0-100, profile 10-20: -200-300,
/// -10-40) and WV03 (0-300, 2-24.5: -200-500, -20-50).
fn default_range(alignment: &ArchEntityData, profiles: &[&ArchEntityData], origin: Vector2, stations: Option<(f64, f64)>, vertical_scale: f64) -> Option<ProfileViewFrame> {
    let start = alignment.alignment_start_station()?;
    let length: f64 = alignment.alignment_pieces().iter().map(|(_, p)| p.windows(2).map(|w| w[0].distance(&w[1])).sum::<f64>()).sum();
    let (s0, s1) = stations.unwrap_or((((start - STATION_MARGIN) / STATION_STEP).floor() * STATION_STEP, ((start + length + STATION_MARGIN) / STATION_STEP).ceil() * STATION_STEP));
    // The elevations of the profiles within the shown stations: their
    // segments clipped to the station range.
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for line in profiles.iter().flat_map(|p| p.profile_polylines()) {
        for w in line.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (x0, x1) = (a.x.max(s0), b.x.min(s1));
            if x0 > x1 {
                continue;
            }
            for x in [x0, x1] {
                let y = if b.x == a.x { a.y } else { a.y + (b.y - a.y) * (x - a.x) / (b.x - a.x) };
                lo = lo.min(y);
                hi = hi.max(y);
            }
        }
    }
    (lo.is_finite() && hi.is_finite()).then_some(())?;
    Some(ProfileViewFrame {
        origin,
        station_min: s0,
        station_max: s1,
        elevation_min: (lo / ELEVATION_STEP).floor() * ELEVATION_STEP - ELEVATION_MARGIN,
        elevation_max: (hi / ELEVATION_STEP).ceil() * ELEVATION_STEP + ELEVATION_MARGIN,
        vertical_scale,
    })
}

/// The vertical exaggeration a profile view style holds.
fn exaggeration_of(doc: &CadDocument, style: u64) -> Option<f64> {
    let o = object(doc, Handle::new(style)).filter(|o| o.dxf_name == "AECC_PROFILE_VIEW_STYLE")?;
    let (data, start, end) = fields_of(o)?;
    let mut b = super::super::bits::Bits::new(data, start, end);
    b.bit_long()?;
    let n = b.bit_long()?;
    for _ in 0..n {
        b.bit_long()?;
    }
    (b.bit_long()? == 0 && b.bit_long()? == 0 && b.bit()?).then_some(())?;
    for _ in 0..b.bits_value(8)? {
        if let (KEY_EXAGGERATION, crate::classes::civil::alignment::KeyValue::Double(v)) = crate::classes::civil::alignment::read_key(&mut b)? {
            return (v.is_finite() && v > 0.0).then_some(v);
        }
    }
    None
}

/// Adds a profile view style `name` to the drawing: the default style's
/// settings with vertical exaggeration `exaggeration`, under a text id in
/// the profile view style collection (written with the default style when
/// the drawing lacks it).
pub fn add_profile_view_style(doc: &mut CadDocument, name: &str, exaggeration: f64) -> Result<Handle, DxfError> {
    add_profile_view_style_with(doc, name, &ProfileViewStyleSettings { vertical_exaggeration: exaggeration, ..Default::default() })
}

/// [`add_profile_view_style`] with every setting given (start from
/// `ProfileViewStyleSettings::default()`, the application's default
/// style). Text styles are handles of the drawing's text styles, 0 for
/// "Standard"; the top and left/right axes' geometry ticks are not
/// written (the top axis has no tick switch, the side axes no geometry
/// ticks).
pub fn add_profile_view_style_with(doc: &mut CadDocument, name: &str, settings: &ProfileViewStyleSettings) -> Result<Handle, DxfError> {
    let axes = [&settings.left, &settings.right, &settings.top, &settings.bottom];
    if name.is_empty() || !(settings.vertical_exaggeration.is_finite() && settings.vertical_exaggeration > 0.0) {
        return Err(invalid("a profile view style needs a name and a positive vertical exaggeration"));
    }
    if axes.iter().any(|a| !(a.major.interval > 0.0 && a.minor.interval > 0.0)) {
        return Err(invalid("profile view style tick intervals must be positive"));
    }
    let styles = [settings.title.text_style].into_iter().chain(axes.iter().flat_map(|a| [a.title.text_style, a.major.text_style, a.minor.text_style].into_iter().chain(a.geometry.as_ref().map(|g| g.text_style))));
    for h in styles.collect::<Vec<_>>() {
        if h != 0 && !doc.text_styles.iter().any(|t| t.handle.value() == h) {
            return Err(invalid("a profile view style text style is not a text style of the drawing"));
        }
    }
    ensure_view_style_tree(doc)?;
    let styles = root_node(doc, KEY_STYLES).ok_or_else(|| invalid("drawing has no style tree"))?;
    let head = read_node(doc, styles).and_then(|t| t.children.iter().find(|c| c.0 == KEY_PROFILE_VIEW_STYLES).map(|c| Handle::new(c.1))).ok_or_else(|| invalid("drawing has no profile view styles"))?;
    let collection = read_node(doc, head).ok_or_else(|| invalid("profile view style collection cannot be read"))?;
    if collection.names.iter().any(|n| n.eq_ignore_ascii_case(name)) {
        return Err(invalid("a profile view style of that name exists"));
    }
    let id = collection.children.iter().map(|c| c.0 as u32).filter(|k| *k & 0xFFFF_0000 == FIRST_STYLE_TEXT_ID & 0xFFFF_0000).max().map_or(FIRST_STYLE_TEXT_ID, |k| k + 1);
    let layer = doc.layers.get("0").map(|l| l.handle.value()).ok_or_else(|| invalid("drawing has no layer 0"))?;
    let by_block = doc.line_types.get("ByBlock").map(|l| l.handle.value()).ok_or_else(|| invalid("drawing lacks the ByBlock linetype"))?;
    let handle = doc.allocate_handle();
    view_style_named(doc, handle, head, id as i32, name, settings, &[], 0, layer, by_block)?;
    add_node_child(doc, head, id as i32, handle, name)?;
    Ok(handle)
}

/// Adds a profile view of `alignment` showing all its profiles, its lower
/// left corner at `origin`, on `layer` (layer "0" when `None`), with the
/// drawing's default profile view style and the default range (stations
/// 200 before the alignment's start to 200 past its end, the profiles'
/// elevations rounded out to 10 with 20 more below and above). When the
/// drawing has a default profile label set, each profile gets its four
/// label groups (lines, PVIs, sag and crest curves) in the view. The
/// alignment must be one this crate writes (it is written again with the
/// view in its list) and have a profile. A drawing without a profile
/// view style gets the default one.
pub fn add_profile_view(doc: &mut CadDocument, alignment: Handle, name: &str, origin: Vector2, layer: Option<&str>) -> Result<Handle, DxfError> {
    add_profile_view_with_range(doc, alignment, name, origin, None, None, layer)
}

/// [`add_profile_view`] with user-specified ranges: `stations` (start,
/// end) and `elevations` (minimum, maximum) replace the automatic ones
/// when given; the automatic elevations then come from the profiles
/// within the given stations. The view's range word records which range
/// the user gave.
pub fn add_profile_view_with_range(doc: &mut CadDocument, alignment: Handle, name: &str, origin: Vector2, stations: Option<(f64, f64)>, elevations: Option<(f64, f64)>, layer: Option<&str>) -> Result<Handle, DxfError> {
    add_profile_view_with_style(doc, alignment, name, origin, None, stations, elevations, layer)
}

/// [`add_profile_view_with_range`] with the profile view style `style`
/// (the drawing's default when `None`); the view takes its vertical
/// exaggeration.
#[allow(clippy::too_many_arguments)]
pub fn add_profile_view_with_style(doc: &mut CadDocument, alignment: Handle, name: &str, origin: Vector2, style: Option<Handle>, stations: Option<(f64, f64)>, elevations: Option<(f64, f64)>, layer: Option<&str>) -> Result<Handle, DxfError> {
    add_view(doc, alignment, name, origin, style, stations, elevations, &[], layer)
}

/// [`add_profile_view_with_style`] with `bands` (below the view first, in
/// the given order, then those above), each drawn with the drawing's
/// default band style of its kind (a drawing without band styles gets the
/// default ones) and given its label group. Only profile data bands are
/// written above the view.
pub fn add_profile_view_with_bands(doc: &mut CadDocument, alignment: Handle, name: &str, origin: Vector2, style: Option<Handle>, bands: &[ProfileViewBand], layer: Option<&str>) -> Result<Handle, DxfError> {
    add_view(doc, alignment, name, origin, style, None, None, bands, layer)
}

#[allow(clippy::too_many_arguments)]
fn add_view(doc: &mut CadDocument, alignment: Handle, name: &str, origin: Vector2, style: Option<Handle>, stations: Option<(f64, f64)>, elevations: Option<(f64, f64)>, bands: &[ProfileViewBand], layer: Option<&str>) -> Result<Handle, DxfError> {
    // A geometry band item holds the band's number in four bits.
    if bands.len() > 15 {
        return Err(invalid("a profile view holds at most 15 bands"));
    }
    if bands.iter().any(|b| !(b.weeding.is_finite() && b.weeding >= 0.0)) {
        return Err(invalid("a band's weeding is a finite length, zero or more"));
    }
    if bands.iter().any(|b| !(b.gap.is_finite() && b.gap >= 0.0)) {
        return Err(invalid("a band gap is a finite length, zero or more"));
    }
    if name.is_empty() {
        return Err(invalid("a profile view needs a name"));
    }
    if !origin.x.is_finite() || !origin.y.is_finite() {
        return Err(invalid("a profile view needs a finite origin"));
    }
    let (written, profiles) = rewritable(doc, alignment, |h| identifier(doc, h)).ok_or_else(|| invalid("the alignment is not one this crate writes"))?;
    if profiles.is_empty() {
        return Err(invalid("the alignment has no profile"));
    }
    let a = arch(doc, alignment.value()).ok_or_else(|| invalid("alignment not found"))?;
    let shown: Vec<&ArchEntityData> = profiles.iter().filter_map(|p| arch(doc, p.handle)).collect();
    if stations.is_some_and(|(lo, hi)| !(lo.is_finite() && hi.is_finite() && lo < hi)) {
        return Err(invalid("a station range runs from a lower to a higher finite station"));
    }
    let style = match style {
        Some(s) => Some((s.value(), exaggeration_of(doc, s.value()).ok_or_else(|| invalid("not a profile view style"))?)),
        None => None,
    };
    let mut frame = default_range(a, &shown, origin, stations, style.map_or(VERTICAL_SCALE, |s| s.1)).ok_or_else(|| invalid("the profiles have no elevations within the stations"))?;
    if let Some((lo, hi)) = elevations {
        if !(lo.is_finite() && hi.is_finite() && lo < hi) {
            return Err(invalid("an elevation range runs from a lower to a higher finite elevation"));
        }
        (frame.elevation_min, frame.elevation_max) = (lo, hi);
    }
    let list: Vec<Shown> = profiles.iter().zip(&shown).map(|(p, a)| Shown { handle: p.handle, style: a.typed_handles().get(11).map_or(0, |r| r.0) }).collect();
    let labels: Vec<[Vec<i32>; 4]> = shown.iter().map(|p| labelled(p)).collect();
    ensure_view_style_tree(doc)?;
    super::labels::ensure_label_styles(doc)?;
    let style = match style {
        Some(s) => s.0,
        None => default_style_of(doc, KEY_PROFILE_VIEW_STYLES).ok_or_else(|| invalid("drawing has no profile view styles"))?,
    };
    if !bands.is_empty() {
        super::band_styles::ensure_band_styles(doc)?;
    }
    let first = list[0].handle;
    let mut resolved = Vec::new();
    for b in bands.iter().filter(|b| !b.top).chain(bands.iter().filter(|b| b.top)) {
        let profile = |p: Option<Handle>| p.map_or(Ok(first), |p| if list.iter().any(|s| s.handle == p.value()) { Ok(p.value()) } else { Err(invalid("a band's profile is not a profile of the alignment")) });
        let style = default_style_of(doc, b.kind.style_key()).ok_or_else(|| invalid("drawing has no band style of that kind"))?;
        if let Some(source) = b.data_source {
            let group = || super::sample_lines::read_group(doc, source).is_some_and(|g| g.alignment == alignment.value());
            let network = || super::network::read_network(doc, source).is_some();
            let fits = match b.kind {
                BandKind::SectionalData => group(),
                BandKind::PipeNetwork => network(),
                _ => false,
            };
            fits.then_some(()).ok_or_else(|| invalid("a band's data source is a sample line group of the view's alignment (sectional data) or a gravity pipe network (pipe network)"))?;
        }
        resolved.push(Resolved { band: b.clone(), style, profile1: profile(b.profile1)?, profile2: profile(b.profile2.or(b.profile1))? });
    }
    let label_styles = default_style_of(doc, KEY_PROFILE_LABEL_SETS).and_then(|set| object(doc, Handle::new(set))).map(|o| o.decode().handles);
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let owner = doc.header.model_space_block_handle;
    let common = |handle: Handle| EntityCommon { handle, owner_handle: owner, layer: layer.into(), layer_handle: Some(layer_handle), ..EntityCommon::default() };
    let handle = doc.allocate_handle();
    let mut groups = Vec::new();
    if let Some(styles) = &label_styles {
        for (p, ids) in list.iter().zip(&labels) {
            for (kind, ids) in Group::ALL.into_iter().zip(ids) {
                if let Some(s) = styles.get(kind.style_index()).copied().filter(|s| *s != 0) {
                    groups.push((doc.allocate_handle(), kind, ids.clone(), p.handle, s));
                }
            }
        }
    }
    let mut views = written.views.clone();
    views.push(handle.value());
    rewrite_with_views(doc, alignment, written, &profiles, &profiles, Some(&views))?;
    // The parts of the pipe network bands' networks are drawn in the
    // view: each network once, its pipes then its structures (DU01).
    let mut parts: Vec<u64> = Vec::new();
    for b in resolved.iter().filter(|b| b.band.kind == BandKind::PipeNetwork) {
        if let Some(n) = b.band.data_source.and_then(|n| super::network::read_network(doc, n)) {
            for h in n.pipes.iter().chain(&n.structures) {
                if !parts.contains(h) {
                    parts.push(*h);
                }
            }
        }
    }
    let mut drawn = Vec::new();
    for part in &parts {
        let h = doc.allocate_handle();
        drawn.push(write_drawn_part(doc, common(h), *part)?.value());
    }
    write_view(doc, common(handle), name, &frame, (stations.is_some(), elevations.is_some()), style, alignment, &list, &resolved, &drawn)?;
    for (h, kind, ids, profile, s) in groups {
        write_group(doc, common(h), kind, &ids, profile, s, handle.value())?;
    }
    // A band that hides its labels has no label group (BVPD-SL, BVHG-SL).
    for (i, b) in resolved.iter().enumerate().filter(|(_, b)| b.band.show_labels) {
        let h = doc.allocate_handle();
        super::bands::write_group(doc, common(h), b, i as i32 + 1, handle.value(), &frame, alignment)?;
    }
    Ok(handle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::DxfVersion;

    fn entity(doc: &CadDocument, h: Handle) -> ArchEntityData {
        arch(doc, h.value()).cloned().unwrap()
    }

    fn round_trip(doc: &CadDocument, tag: &str) -> CadDocument {
        let path = std::env::temp_dir().join(format!("civil-view-{tag}-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        back
    }

    #[test]
    fn a_new_drawing_gets_a_view_with_the_default_style_and_range() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        assert!(add_profile_view(&mut doc, al, "PV", v(0.0, -100.0), None).is_err());
        crate::classes::civil::add_profile(&mut doc, al, "P1", &[v(0.0, 10.0), v(50.0, 20.0), v(100.0, 15.0)], &[0.0], None).unwrap();
        assert!(add_profile_view(&mut doc, al, "", v(0.0, -100.0), None).is_err());
        let first = add_profile_view(&mut doc, al, "PV - (1)", v(0.0, -100.0), None).unwrap();
        let second = add_profile_view(&mut doc, al, "PV - (2)", v(50.0, -300.0), None).unwrap();
        let back = round_trip(&doc, "new");
        let frame = entity(&back, first).profile_view_frame().unwrap();
        assert_eq!(frame, ProfileViewFrame { origin: v(0.0, -100.0), station_min: -200.0, station_max: 300.0, elevation_min: -10.0, elevation_max: 40.0, vertical_scale: 10.0 });
        assert_eq!(entity(&back, second).profile_view_frame().unwrap().origin, v(50.0, -300.0));
        // The alignment lists its views after its profile, oldest first.
        let refs: Vec<u64> = entity(&back, al).typed_handles()[7..].iter().map(|r| r.0).collect();
        assert_eq!(refs, vec![first.value(), second.value()]);
        let style = default_style_of(&back, KEY_PROFILE_VIEW_STYLES).unwrap();
        assert_eq!(object(&back, Handle::new(style)).unwrap().dxf_name, "AECC_PROFILE_VIEW_STYLE");
        assert_eq!(entity(&back, first).typed_handles()[2].0, style);
        // A new drawing gets the default profile label set: four label
        // groups per view.
        let groups = back.entities().filter(|e| matches!(e, EntityType::Extended(x) if matches!(&x.data, ExtendedEntityData::Arch(a) if a.dxf_name.contains("LABEL_GROUP")))).count();
        assert_eq!(groups, 8);
    }

    #[test]
    fn views_with_bands_read_their_frame_back() {
        use super::super::bands::{BandKind, ProfileViewBand};
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        crate::classes::civil::add_profile(&mut doc, al, "P1", &[v(0.0, 10.0), v(50.0, 20.0), v(100.0, 15.0)], &[0.0], None).unwrap();
        let bands: Vec<ProfileViewBand> = [BandKind::ProfileData, BandKind::VerticalGeometry, BandKind::HorizontalGeometry].into_iter().map(ProfileViewBand::new).collect();
        let view = add_profile_view_with_bands(&mut doc, al, "PV", v(0.0, -100.0), None, &bands, None).unwrap();
        let back = round_trip(&doc, "bands");
        // Each band item opens with its gap (a BD, not 0 here).
        let frame = entity(&back, view).profile_view_frame().unwrap();
        assert_eq!((frame.origin, frame.station_min, frame.station_max), (v(0.0, -100.0), -200.0, 300.0));
    }

    /// Superelevation, sectional data and pipe network bands below the
    /// view and geometry bands above it get their style collections and an
    /// empty label group each (labels for the geometry bands).
    #[test]
    fn views_take_every_band_kind_above_and_below() {
        use super::super::bands::{BandKind, ProfileViewBand};
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        crate::classes::civil::add_profile(&mut doc, al, "P1", &[v(0.0, 10.0), v(100.0, 15.0)], &[], None).unwrap();
        let mut bands: Vec<ProfileViewBand> = [BandKind::Superelevation, BandKind::SectionalData, BandKind::PipeNetwork, BandKind::VerticalGeometry, BandKind::HorizontalGeometry].into_iter().map(ProfileViewBand::new).collect();
        bands[3].top = true;
        bands[4].top = true;
        let view = add_profile_view_with_bands(&mut doc, al, "PV", v(0.0, -100.0), None, &bands, None).unwrap();
        let back = round_trip(&doc, "kinds");
        assert!(entity(&back, view).profile_view_frame().is_some());
        let groups: Vec<String> = back.entities().filter_map(|e| match e {
            crate::entities::EntityType::Extended(x) => match &x.data {
                crate::entities::ExtendedEntityData::Arch(a) if a.dxf_name.ends_with("BAND_LABELGROUP") => Some(a.dxf_name.clone()),
                _ => None,
            },
            _ => None,
        }).collect();
        assert_eq!(groups.len(), 5);
        assert!(groups.iter().any(|g| g == "AECC_SECTIONALDTABAND_LABELGROUP"));
    }

    /// A band that hides its labels gets no label group; weeding and
    /// stagger are written into the item and the group.
    #[test]
    fn bands_hiding_their_labels_have_no_group() {
        use super::super::bands::{BandKind, BandStagger, ProfileViewBand};
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        crate::classes::civil::add_profile(&mut doc, al, "P1", &[v(0.0, 10.0), v(100.0, 15.0)], &[], None).unwrap();
        let mut shown = ProfileViewBand::new(BandKind::ProfileData);
        shown.weeding = 50.0;
        shown.stagger = BandStagger::Left;
        let mut hidden = ProfileViewBand::new(BandKind::HorizontalGeometry);
        hidden.show_labels = false;
        let view = add_profile_view_with_bands(&mut doc, al, "PV", v(0.0, -100.0), None, &[shown, hidden], None).unwrap();
        let back = round_trip(&doc, "hidden");
        assert!(entity(&back, view).profile_view_frame().is_some());
        let groups = back.entities().filter(|e| matches!(e, crate::entities::EntityType::Extended(x) if matches!(&x.data, crate::entities::ExtendedEntityData::Arch(a) if a.dxf_name.ends_with("BAND_LABELGROUP")))).count();
        assert_eq!(groups, 1);
        let mut bad = ProfileViewBand::new(BandKind::ProfileData);
        bad.weeding = -1.0;
        assert!(add_profile_view_with_bands(&mut doc, al, "PV2", v(0.0, -300.0), None, &[bad], None).is_err());
    }

    #[test]
    fn views_take_a_style_with_its_vertical_exaggeration() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
        crate::classes::civil::add_profile(&mut doc, al, "P1", &[v(0.0, 10.0), v(50.0, 20.0), v(100.0, 15.0)], &[0.0], None).unwrap();
        assert!(add_profile_view_style(&mut doc, "VS5", 0.0).is_err());
        let s5 = add_profile_view_style(&mut doc, "VS5", 5.0).unwrap();
        let s20 = add_profile_view_style(&mut doc, "VS20", 20.0).unwrap();
        assert!(add_profile_view_style(&mut doc, "vs5", 3.0).is_err());
        assert!(add_profile_view_with_style(&mut doc, al, "PV", v(0.0, 0.0), Some(al), None, None, None).is_err());
        let view = add_profile_view_with_style(&mut doc, al, "PV5", v(0.0, -100.0), Some(s5), None, None, None).unwrap();
        let back = round_trip(&doc, "styles");
        assert_eq!((exaggeration_of(&back, s5.value()), exaggeration_of(&back, s20.value())), (Some(5.0), Some(20.0)));
        assert_eq!(entity(&back, view).typed_handles()[2].0, s5.value());
        assert_eq!(entity(&back, view).profile_view_frame().unwrap().vertical_scale, 5.0);
        // The collection lists both under text ids, after the default.
        let styles = read_node(&back, root_node(&back, KEY_STYLES).unwrap()).unwrap();
        let head = styles.children.iter().find(|c| c.0 == KEY_PROFILE_VIEW_STYLES).unwrap().1;
        let collection = read_node(&back, Handle::new(head)).unwrap();
        assert_eq!(collection.names, ["VS5", "VS20"]);
        assert_eq!(object(&back, s5).unwrap().decode().strings[..2], ["VS5".to_string(), String::new()]);
    }

    #[test]
    fn styles_take_every_setting() {
        let defaults = ProfileViewStyleSettings::default();
        // The default settings write the default style's keys unchanged.
        let keys = view_style_keys();
        for (k, v) in view_settings_values(&defaults) {
            let (_, d) = keys.iter().find(|(key, _)| *key == k).unwrap();
            assert_eq!(format!("{:?}", style_value(&v)), format!("{:?}", style_value(d)), "{k:08X}");
        }
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let mut s = defaults.clone();
        s.right_to_left = true;
        s.title.text = "Long view".into();
        s.title.border = true;
        s.grid.padding = [1.0, 2.0, 3.0, 4.0];
        s.grid.vertical.clip = false;
        s.left.show_ticks_and_labels = false;
        s.bottom.major.interval = 50.0;
        s.bottom.minor.label = "<[33607712(2097)]>".into();
        s.right.title.location = 2;
        s.top.geometry.as_mut().unwrap().justification = 0;
        s.top.title.text_style = doc.text_styles.get("Standard").unwrap().handle.value() + 1_000_000;
        assert!(add_profile_view_style_with(&mut doc, "S", &s).is_err());
        s.top.title.text_style = 0;
        s.left.minor.interval = 0.0;
        assert!(add_profile_view_style_with(&mut doc, "S", &s).is_err());
        s.left.minor.interval = 2.5;
        let h = add_profile_view_style_with(&mut doc, "S", &s).unwrap();
        let back = round_trip(&doc, "settings");
        let read = object(&back, h).unwrap().profile_view_style_settings().unwrap();
        let standard = back.text_styles.get("Standard").unwrap().handle.value();
        assert_eq!(read.top.title.text_style, standard);
        s.title.text_style = standard;
        for a in [&mut s.left, &mut s.right, &mut s.top, &mut s.bottom] {
            a.title.text_style = standard;
            for t in [Some(&mut a.major), Some(&mut a.minor), a.geometry.as_mut()].into_iter().flatten() {
                t.text_style = standard;
            }
        }
        assert_eq!(read, s);
    }

    fn style_value(v: &StyleValue) -> Option<KeyValue> {
        match v {
            StyleValue::Double(v) => Some(KeyValue::Double(*v)),
            StyleValue::Long(v) => Some(KeyValue::Long(*v)),
            StyleValue::Byte(v) => Some(KeyValue::Byte(*v)),
            StyleValue::Point(p) => Some(KeyValue::Vector(*p)),
            StyleValue::Wide(t) => Some(KeyValue::Text(t.to_string())),
            StyleValue::WideString(t) => Some(KeyValue::Text(t.clone())),
            StyleValue::Reference(r) => Some(KeyValue::Long(match r {
                Shared::Standard => 0,
                _ => -1,
            })),
            _ => None,
        }
    }

    #[test]
    fn views_get_label_groups_from_the_default_label_set() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(300.0, 0.0)], None).unwrap();
        let pvis = [v(0.0, 10.0), v(100.0, 20.0), v(200.0, 5.0), v(300.0, 15.0)];
        let p = crate::classes::civil::add_profile(&mut doc, al, "P1", &pvis, &[40.0, 30.0], None).unwrap();
        // The default label set a new drawing gets refers to the line, PVI,
        // sag and crest curve label styles after a null reference.
        let view = add_profile_view(&mut doc, al, "PV", v(0.0, -100.0), None).unwrap();
        let back = round_trip(&doc, "labels");
        let set = default_style_of(&back, KEY_PROFILE_LABEL_SETS).unwrap();
        let styles: Vec<u64> = object(&back, Handle::new(set)).unwrap().decode().handles[1..5].to_vec();
        let ids = entity(&back, p).profile_entity_ids();
        let tangents: Vec<i32> = ids.iter().filter(|e| !e.1).map(|e| e.0).collect();
        let curves: Vec<i32> = ids.iter().filter(|e| e.1).map(|e| e.0).collect();
        let group = |dxf: &str| {
            back.entities()
                .find_map(|e| match e {
                    EntityType::Extended(x) => match &x.data {
                        ExtendedEntityData::Arch(a) if a.dxf_name == dxf => Some(a.clone()),
                        _ => None,
                    },
                    _ => None,
                })
                .unwrap()
        };
        let line = group("AECC_VALIGNMENT_LINE_LABEL_GROUP");
        assert_eq!(line.label_entity_ids(), tangents);
        assert_eq!(line.typed_handles()[2..5].iter().map(|r| r.0).collect::<Vec<_>>(), vec![p.value(), styles[0], view.value()]);
        // The PVI labels sit at the profile's ends; the first curve (grade
        // falling) is a crest, the second a sag.
        let pvi = group("AECC_VALIGNMENT_PVI_LABEL_GROUP");
        assert_eq!(pvi.typed_handles().len(), 5 + 2 * 2);
        assert_eq!(group("AECC_VALIGNMENT_CRESTCURVE_LABEL_GROUP").label_entity_ids(), vec![curves[0]]);
        assert_eq!(group("AECC_VALIGNMENT_SAGCURVE_LABEL_GROUP").label_entity_ids(), vec![curves[1]]);
        assert_eq!(group("AECC_VALIGNMENT_SAGCURVE_LABEL_GROUP").typed_handles()[3].0, styles[2]);
    }

    #[test]
    fn views_keep_a_user_range() {
        let v = Vector2::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let al = crate::classes::civil::add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(300.0, 0.0)], None).unwrap();
        crate::classes::civil::add_profile(&mut doc, al, "P1", &[v(0.0, 10.0), v(150.0, 45.0), v(300.0, 12.0)], &[0.0], None).unwrap();
        assert!(add_profile_view_with_range(&mut doc, al, "X", v(0.0, 0.0), Some((50.0, 50.0)), None, None).is_err());
        assert!(add_profile_view_with_range(&mut doc, al, "X", v(0.0, 0.0), None, Some((5.0, f64::NAN)), None).is_err());
        let a = add_profile_view_with_range(&mut doc, al, "A", v(0.0, -200.0), Some((0.0, 60.0)), None, None).unwrap();
        let b = add_profile_view_with_range(&mut doc, al, "B", v(400.0, -200.0), Some((-20.0, 320.0)), Some((-5.5, 30.25)), None).unwrap();
        let back = round_trip(&doc, "range");
        let fa = entity(&back, a).profile_view_frame().unwrap();
        // Elevations 10 to 24 within stations 0 to 60: 10..30, 20 more each side.
        assert_eq!((fa.station_min, fa.station_max, fa.elevation_min, fa.elevation_max), (0.0, 60.0, -10.0, 50.0));
        let fb = entity(&back, b).profile_view_frame().unwrap();
        assert_eq!((fb.origin, fb.station_min, fb.station_max, fb.elevation_min, fb.elevation_max), (v(400.0, -200.0), -20.0, 320.0, -5.5, 30.25));
    }
}
