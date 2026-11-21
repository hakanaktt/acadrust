//! The bands of a profile view: the items the view lists and the label
//! group each band draws its labels with.

use super::super::profile::ProfileViewFrame;
use super::support::*;
use crate::classes::arch::ArchEntityData;
use crate::classes::civil::AlignmentPiece;
use crate::entities::EntityCommon;
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::types::Handle;
use crate::CadDocument;

/// What a profile view band shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BandKind {
    /// Stations and elevations of the profiles.
    ProfileData,
    /// The profile's tangents and curves.
    VerticalGeometry,
    /// The alignment's lines and curves.
    HorizontalGeometry,
    /// The alignment's superelevation.
    Superelevation,
    /// The sample lines of a sample line group.
    SectionalData,
    /// The parts of a pipe network.
    PipeNetwork,
}

/// A band of a profile view, drawn with the drawing's default band style
/// of its kind.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProfileViewBand {
    pub kind: BandKind,
    /// Above the view.
    pub top: bool,
    /// The profiles the band reads; the view's first profile when `None`.
    pub profile1: Option<Handle>,
    pub profile2: Option<Handle>,
    /// Gap between the view and the band (or the band before).
    pub gap: f64,
    pub label_at_start: bool,
    pub label_at_end: bool,
    /// Whether the band shows its labels.
    pub show_labels: bool,
    /// Weeding distance of the labels.
    pub weeding: f64,
    pub stagger: BandStagger,
    /// A sectional data band's sample line group (one of the view's
    /// alignment) or a pipe network band's gravity pipe network (its
    /// parts are drawn in the view); `None` for the other kinds.
    pub data_source: Option<Handle>,
}

/// How a band staggers its labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum BandStagger {
    None,
    Both,
    Right,
    Left,
}

impl ProfileViewBand {
    /// A band below the view with the application's defaults: a gap of
    /// half an inch in feet, labels at the start and end stations except
    /// for vertical geometry bands.
    pub fn new(kind: BandKind) -> Self {
        let ends = kind != BandKind::VerticalGeometry;
        ProfileViewBand { kind, top: false, profile1: None, profile2: None, gap: 0.5 / 12.0, label_at_start: ends, label_at_end: ends, show_labels: true, weeding: 100.0, stagger: BandStagger::None, data_source: None }
    }
}

/// A band with its style and profiles resolved.
pub(super) struct Resolved {
    pub band: ProfileViewBand,
    pub style: u64,
    pub profile1: u64,
    pub profile2: u64,
}

impl BandKind {
    /// Style tree key of the kind's band style collection.
    pub(super) fn style_key(self) -> i32 {
        match self {
            BandKind::ProfileData => super::band_styles::KEY_DATA_BANDS,
            BandKind::VerticalGeometry => super::band_styles::KEY_VERTICAL_GEOMETRY_BANDS,
            BandKind::HorizontalGeometry => super::band_styles::KEY_HORIZONTAL_GEOMETRY_BANDS,
            BandKind::Superelevation => super::band_styles::KEY_SUPERELEVATION_BANDS,
            BandKind::SectionalData => super::band_styles::KEY_SECTIONAL_DATA_BANDS,
            BandKind::PipeNetwork => super::band_styles::KEY_PIPE_NETWORK_BANDS,
        }
    }

    fn dxf(self) -> &'static str {
        match self {
            BandKind::ProfileData => "AECC_PROFILEDATABAND_LABELGROUP",
            BandKind::VerticalGeometry => "AECC_VERTICALGEOMETRYBAND_LABELGROUP",
            BandKind::HorizontalGeometry => "AECC_HORIZONTALGEOMETRYBAND_LABELGROUP",
            BandKind::Superelevation => "AECC_SUPERELEVATIONBAND_LABELGROUP",
            BandKind::SectionalData => "AECC_SECTIONALDTABAND_LABELGROUP",
            BandKind::PipeNetwork => "AECC_PIPENETWORKBAND_LABELGROUP",
        }
    }

    fn class_text(self) -> &'static str {
        match self {
            BandKind::ProfileData => "AeccImpProfileDataBandLabeling",
            BandKind::VerticalGeometry => "AeccImpVerticalGeometryBandLabeling",
            BandKind::HorizontalGeometry => "AeccImpHorizontalGeometryBandLabeling",
            BandKind::Superelevation => "AeccImpSuperElevationBandLabeling",
            BandKind::SectionalData => "AeccImpSectionalDataBandLabeling",
            BandKind::PipeNetwork => "AeccImpPipeNetworkBandLabeling",
        }
    }
}

/// An item after its gap (`BD`) opens with `BL` flags (0x01 below the
/// view, 0x10 label at start, 0x20 label at end, 0x100 labels shown),
/// `BD` weeding, `BL` stagger (0 none, 1 both, 2 right, 3 left) and `BD`
/// stagger line height (1/60); the rest is constant per kind but for the
/// patches below (BVPD-W, -SL, -ST, -SR, -SLF; BVHG-SL, -W). The template
/// holds these four fields in its first 168 bits.
const ITEM_HEAD: usize = 168;
const STAGGER_LINE_HEIGHT: f64 = 1.0 / 60.0;
/// A profile data band item: above the view bits 921 and 923 are 1 and 0.
const DATA_ITEM: (usize, &str) = (1056, "0c404000000000000000059408111111111111913f0000000000000000000000000000000000000000000000000000000000000000243801010000008040000030100000100400000501000001804000007010000020040000090100000280400000b0100000300400000d01000007404000101000000000000165000000000000003940");
/// The item of every other band after its gap (vertical and horizontal
/// geometry, superelevation, sectional data, pipe network: BV-SE, BV-SD,
/// BV-PN equal a vertical geometry item; above the view only the flags
/// change: BV-VGT, BV-HGT), the band's number in the view (bottom bands first, from 1)
/// in bits 436 to 439 (a lone horizontal geometry band: 1, HG3P; PB04: 2
/// and 3).
const GEOMETRY_ITEM: (usize, &str) = (572, "0c404000000000000000059408111111111111913f00000000000000000000000000000000000000000000000000000000000000002903000000000000165000000000000003940");

fn hex_bits((len, hex): (usize, &str)) -> Vec<bool> {
    hex.chars().flat_map(|c| {
        let v = c.to_digit(16).unwrap_or(0);
        (0..4).rev().map(move |i| v >> i & 1 == 1)
    }).take(len).collect()
}

/// Writes the view's band items: each `BD gap` and its kind's template.
pub(super) fn write_items(w: &mut DwgMergedWriter, bands: &[Resolved]) {
    for (n, b) in bands.iter().enumerate() {
        w.write_bit_double(b.band.gap);
        let mut bits = hex_bits(if b.band.kind == BandKind::ProfileData { DATA_ITEM } else { GEOMETRY_ITEM });
        let band = &b.band;
        let flags = i32::from(!band.top) | i32::from(band.label_at_start) << 4 | i32::from(band.label_at_end) << 5 | i32::from(band.show_labels) << 8;
        w.write_bit_long(flags);
        w.write_bit_double(band.weeding);
        w.write_bit_long(band.stagger as i32);
        w.write_bit_double(STAGGER_LINE_HEIGHT);
        match band.kind {
            BandKind::ProfileData => {
                bits[921] = band.top;
                bits[923] = !band.top;
            }
            _ => {
                let number = n as u8 + 1;
                for i in 0..4 {
                    bits[436 + i] = number >> (3 - i) & 1 == 1;
                }
            }
        }
        for bit in &bits[ITEM_HEAD..] {
            w.write_bit(*bit);
        }
    }
}

/// The band entries of the view's graph keys, by key: one value per band.
pub(super) fn key_values(key: u32, bands: &[Resolved]) -> Vec<f64> {
    bands
        .iter()
        .map(|b| match key {
            0x0C01_BA06 => 3.0,
            0x0C01_BA05 if b.band.kind == BandKind::HorizontalGeometry => -1.0,
            0x0C01_BA05 => 10.0,
            0x0C01_BA04 => 5.0,
            0x1901_BA0A => f64::from(0x1001_BA09),
            _ => 0.0,
        })
        .collect()
}

/// A label of a band label group: its keys, the 44-bit run of its kind and
/// its key list.
struct Label {
    keys: Vec<(u32, KeyValue)>,
    run: &'static str,
    list: &'static [u32],
}

enum KeyValue {
    Long(i32),
    Double(f64),
    Byte(u8),
    /// 128 zero bits.
    NullGuid,
    /// No value (the key's object is a reference of the label).
    Reference,
}

const RUN_ENDS: &str = "00000000010000000001100100000000000000000000";
const RUN_MINOR: &str = "00000000010000000010100100000000000000000000";
const RUN_POINT: &str = "00000000010000000100100100000000000000000000";
const RUN_VERTICAL: &str = "00000000010000000111100100000000000000000000";
const LIST_ENDS: &[u32] = &[0x0101_5013, 0x0200_D020, 0x0300_D622, 0x0300_D623];
const LIST_MINOR: &[u32] = &[0x0101_5013, 0x0200_D020, 0x0200_D620];
const LIST_POINT: &[u32] = &[0x0100_D068, 0x0100_D61E, 0x0100_D621, 0x0101_5013, 0x0200_D61D];
const LIST_VERTICAL: &[u32] = &[0x0100_FC31, 0x0101_5013];
const RUN_SAMPLE_LINE: &str = "00000000010000001101100100000000000000000000";
const RUN_INCREMENT: &str = "00000000010000001110100100000000000000000000";
const LIST_SAMPLE_LINE: &[u32] = &[0x0101_5013, 0x0200_D020, 0x0F01_7824, 0x0F01_7825, 0x1501_7821, 0x1501_7822];
const LIST_INCREMENT: &[u32] = &[0x0101_5013, 0x0200_D020, 0x0201_7823, 0x0F01_7824, 0x0F01_7825, 0x1501_7821, 0x1501_7822];
const RUN_PIPE: &str = "00000000010000001100100100000000000000000000";
const RUN_STRUCTURE: &str = "00000000010000001011100100000000000000000000";
const LIST_PIPE: &[u32] = &[0x0101_5013, 0x0F01_781C];
const LIST_STRUCTURE: &[u32] = &[0x0101_5013, 0x0200_D020, 0x0F01_781C];
const RUN_ELEMENT: &str = "00000000010000001000100100000000000000000000";
const RUN_CURVE_MIDDLE: &str = "00000000010000001111100100000000000000000000";
const LIST_ELEMENT: &[u32] = &[0x0100_D61E, 0x0100_D621, 0x0100_D731, 0x0101_5013, 0x0200_D020, 0x0200_D61D];
const LIST_CURVE_MIDDLE: &[u32] = &[0x0100_D61E, 0x0100_D621, 0x0100_D731, 0x0101_5013, 0x0200_D020];

/// The codes of a horizontal geometry point: the alignment's start and
/// end, a tangent-to-curve point and a curve-to-tangent point.
const POINT_START: i32 = 257;
const POINT_END: i32 = 258;
const POINT_CURVE_IN: i32 = 260;
const POINT_CURVE_OUT: i32 = 261;

/// Station spacing of the default profile data band style's labels.
const MAJOR: f64 = 100.0;
const MINOR: f64 = 25.0;

/// The run every band label group opens with (that of the profile label
/// groups).
const HEADER: &str = super::profile_view::GROUP_HEADER;

const STAGGER_AT: usize = 26;

/// What follows a group's labels, around its band number (`BL`, 1 for the
/// first band, the bands below the view first); the band's stagger (`BL`)
/// sits at bit [`STAGGER_AT`] (BVPD-ST, -SR, -SLF), where the runs hold 0.
fn tail(kind: BandKind) -> (&'static str, &'static str) {
    const AFTER: &str = "0001011111000000100";
    match kind {
        BandKind::ProfileData => ("1001010001010100000001101010100000010001000100010001000100010001000100010001000110010001001111111111010", AFTER),
        _ => ("10010100010101000000011010100001010101010101010101010101010101010101010101010110110101001111110000010001000100010001000100010001000100010001000110010001001111111111010", AFTER),
    }
}

/// The horizontal geometry points a profile data band labels: the
/// alignment's start (code 0x0200D004) and end (0x0200D005), and each
/// arc's start (0x0400D053), middle (0x0400D08E) and end (0x0400D054),
/// with the point's kind word (1 start, 2 end, 3 arc) and its fraction
/// along its element.
fn geometry_points(alignment: &ArchEntityData) -> Option<Vec<(f64, i32, i32, f64)>> {
    let start = alignment.alignment_start_station()?;
    let pieces = alignment.alignment_pieces_with_curvature();
    let mut out = vec![(start, 0x0200_D004, 1, 0.0)];
    let mut at = start;
    for (kind, curvature, pts) in &pieces {
        let (a, b) = (*pts.first()?, *pts.last()?);
        let chord = a.distance(&b);
        let length = match kind {
            AlignmentPiece::Line => chord,
            AlignmentPiece::Curve if *curvature != 0.0 => {
                let r = 1.0 / curvature.abs();
                let half = (chord / (2.0 * r)).clamp(-1.0, 1.0).asin();
                // An arc past half a turn bulges beyond its chord.
                let mid = pts[pts.len() / 2];
                let past_half = mid.distance(&crate::types::Vector2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)) > r;
                2.0 * r * if past_half { std::f64::consts::PI - half } else { half }
            }
            // debt: spirals' points are not labelled (no sample).
            _ => return None,
        };
        if *kind == AlignmentPiece::Curve {
            out.push((at, 0x0400_D053, 3, 0.0));
            out.push((at + length / 2.0, 0x0400_D08E, 3, 0.5));
            out.push((at + length, 0x0400_D054, 3, 1.0));
        }
        at += length;
    }
    out.push((at, 0x0200_D005, 2, 1.0));
    Some(out)
}

/// The labels of a profile data band over the view's stations: one each
/// minor interval (major ones every major interval), the start and end
/// stations when the band labels them, and the alignment's geometry
/// points; a geometry point takes the place of a station label at its
/// station.
fn data_labels(frame: &ProfileViewFrame, band: &ProfileViewBand, number: i32, points: &[(f64, i32, i32, f64)]) -> Vec<(f64, Label)> {
    let station = |s: f64| (0x0200_D020, KeyValue::Double(s));
    let mut out: Vec<(f64, Label)> = Vec::new();
    let near = |s: f64| points.iter().any(|p| (p.0 - s).abs() < 1e-6);
    if band.label_at_start {
        out.push((frame.station_min, Label { keys: vec![(0x0300_D622, KeyValue::Byte(1)), (0x0101_5013, KeyValue::Long(number)), station(frame.station_min)], run: RUN_ENDS, list: LIST_ENDS }));
    }
    let mut s = (frame.station_min / MINOR).floor() * MINOR + MINOR;
    while s < frame.station_max - 1e-9 {
        if !near(s) {
            let label = if (s / MAJOR).round() * MAJOR == s {
                Label { keys: vec![(0x0200_D61F, KeyValue::Double(MAJOR)), (0x0101_5013, KeyValue::Long(number)), station(s)], run: RUN_ENDS, list: LIST_ENDS }
            } else {
                Label { keys: vec![(0x0200_D61F, KeyValue::Double(MAJOR)), (0x0101_5013, KeyValue::Long(number)), (0x0200_D620, KeyValue::Double(MINOR)), station(s)], run: RUN_MINOR, list: LIST_MINOR }
            };
            out.push((s, label));
        }
        s += MINOR;
    }
    for &(s, code, kind, fraction) in points.iter().filter(|p| p.0 > frame.station_min && p.0 < frame.station_max) {
        let keys = vec![(0x0101_5013, KeyValue::Long(number)), (0x0200_D61D, KeyValue::Double(fraction)), station(s), (0x0100_D068, KeyValue::Long(code)), (0x0100_D61E, KeyValue::Long(kind))];
        out.push((s, Label { keys, run: RUN_POINT, list: LIST_POINT }));
    }
    if band.label_at_end {
        out.push((frame.station_max, Label { keys: vec![(0x0300_D623, KeyValue::Byte(1)), (0x0101_5013, KeyValue::Long(number)), station(frame.station_max)], run: RUN_ENDS, list: LIST_ENDS }));
    }
    out.sort_by(|a, b| a.0.total_cmp(&b.0));
    out
}

/// The labels of a vertical geometry band: one per profile entity in
/// identifier order, keyed 0x01017807 plus its index. Every label stores
/// its band's number (key 0x01015013).
fn vertical_labels(profile: &ArchEntityData, number: i32) -> Vec<Label> {
    let mut ids: Vec<i32> = profile.profile_entity_ids().iter().map(|e| e.0).collect();
    ids.sort_unstable();
    ids.into_iter()
        .enumerate()
        .map(|(i, id)| Label {
            keys: vec![(0x0101_7807 + i as u32, KeyValue::Long(i as i32)), (0x0101_5013, KeyValue::Long(number)), (0x0100_FC31, KeyValue::Long(id))],
            run: RUN_VERTICAL,
            list: LIST_VERTICAL,
        })
        .collect()
}

/// The labels of a horizontal geometry band (HG1 to HG3, PB04): per
/// element, at its middle station, its start and end point codes, the
/// previous element's start code, the signed radii (right of travel
/// positive, 0 at the alignment's ends and on lines) at its start, its end
/// and the previous element's start, whether that radius (the end's at the
/// alignment's start) turns left, how many curves lie up to it and its
/// number (the lines numbered first, then the curves); and one more label
/// at each curve's middle, numbered by curve. `None` for an alignment of
/// other pieces or where two lines or two curves meet.
fn horizontal_labels(alignment: &ArchEntityData, number: i32) -> Option<Vec<(f64, Label)>> {
    let mut at = alignment.alignment_start_station()?;
    // (start station, length, signed radius: 0 for a line)
    let mut elements = Vec::new();
    for (kind, curvature, pts) in alignment.alignment_pieces_with_curvature() {
        let (a, b) = (*pts.first()?, *pts.last()?);
        let chord = a.distance(&b);
        let (length, radius) = match kind {
            AlignmentPiece::Line => (chord, 0.0),
            AlignmentPiece::Curve if curvature != 0.0 => {
                let r = 1.0 / curvature.abs();
                let half = (chord / (2.0 * r)).clamp(-1.0, 1.0).asin();
                let mid = pts[pts.len() / 2];
                let past_half = mid.distance(&crate::types::Vector2::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0)) > r;
                (2.0 * r * if past_half { std::f64::consts::PI - half } else { half }, -1.0 / curvature)
            }
            _ => return None,
        };
        elements.push((at, length, radius));
        at += length;
    }
    let curve = |i: usize| elements[i].2 != 0.0;
    if elements.windows(2).any(|w| (w[0].2 != 0.0) == (w[1].2 != 0.0)) {
        return None;
    }
    let n = elements.len();
    let start_code = |i: usize| if i == 0 { POINT_START } else if curve(i) { POINT_CURVE_IN } else { POINT_CURVE_OUT };
    let end_code = |i: usize| if i + 1 == n { POINT_END } else { start_code(i + 1) };
    // The radius at an element's start and end: the curve's that the
    // point belongs to.
    let start_radius = |i: usize| if i == 0 { 0.0 } else if curve(i) { elements[i].2 } else { elements[i - 1].2 };
    let end_radius = |i: usize| if i + 1 == n { 0.0 } else if curve(i) { elements[i].2 } else { elements[i + 1].2 };
    let lines = (0..n).filter(|i| !curve(*i)).count() as i32;
    let mut out = Vec::new();
    let (mut line_no, mut curve_no) = (0, 0);
    for (i, &(start, length, _)) in elements.iter().enumerate() {
        let middle = start + length / 2.0;
        let numbered = if curve(i) {
            curve_no += 1;
            lines + curve_no
        } else {
            line_no += 1;
            line_no
        };
        let previous = i.checked_sub(1).unwrap_or(0);
        let turn = if start_radius(i) != 0.0 { start_radius(i) } else { end_radius(i) };
        if curve(i) {
            let keys = vec![(0x0101_5013, KeyValue::Long(number)), (0x0100_D731, KeyValue::Long(curve_no)), (0x0200_D020, KeyValue::Double(middle)), (0x0100_D621, KeyValue::Long(-1)), (0x0100_D61E, KeyValue::Long(curve_no))];
            out.push((middle, Label { keys, run: RUN_CURVE_MIDDLE, list: LIST_CURVE_MIDDLE }));
        }
        let keys = vec![
            (0x0101_782A, KeyValue::Long(end_code(i))),
            (0x0101_782F, KeyValue::Long(0)),
            (0x0301_782C, KeyValue::Byte(u8::from(turn < 0.0))),
            (0x0101_7829, KeyValue::Long(start_code(i))),
            (0x0201_782E, KeyValue::Double(end_radius(i))),
            (0x0101_782B, KeyValue::Long(start_code(previous))),
            (0x0101_5013, KeyValue::Long(number)),
            (0x0100_D731, KeyValue::Long(curve_no)),
            (0x0200_D020, KeyValue::Double(middle)),
            (0x0100_D621, KeyValue::Long(-1)),
            (0x0201_7830, KeyValue::Double(if i == 0 { 0.0 } else { start_radius(previous) })),
            (0x0100_D61E, KeyValue::Long(numbered)),
            if curve(i) { (0x0101_780C, KeyValue::Long(1)) } else { (0x0101_780B, KeyValue::Long(0)) },
            (0x0201_782D, KeyValue::Double(start_radius(i))),
        ];
        out.push((middle, Label { keys, run: RUN_ELEMENT, list: LIST_ELEMENT }));
    }
    Some(out)
}

/// The labels of a sectional data band reading the sample line group
/// `group` (its lines in station order): one per line at its station,
/// each line after the first followed by one more holding the previous
/// line's station (key 0x02017823). Each label refers to its line and the
/// group (keys 0x0F017824, 0x0F017825); keys 0x15017821/22 hold 128 zero
/// bits (DT02, DU01, E-SD1-edit).
fn sectional_labels(doc: &CadDocument, group: Handle, number: i32) -> Option<Vec<(Label, Vec<u64>)>> {
    let g = super::sample_lines::read_group(doc, group)?;
    let lines: Vec<(f64, u64)> = g.lines.iter().map(|l| Some((super::sample_lines::station(doc, *l)?, *l))).collect::<Option<_>>()?;
    let head = |keys: &mut Vec<(u32, KeyValue)>| keys.extend([(0x1501_7822, KeyValue::NullGuid), (0x0F01_7824, KeyValue::Reference), (0x1501_7821, KeyValue::NullGuid)]);
    let tail = |keys: &mut Vec<(u32, KeyValue)>, s: f64| keys.extend([(0x0101_5013, KeyValue::Long(number)), (0x0200_D020, KeyValue::Double(s)), (0x0F01_7825, KeyValue::Reference)]);
    let mut out = Vec::new();
    for (i, &(s, line)) in lines.iter().enumerate() {
        let mut keys = Vec::new();
        head(&mut keys);
        tail(&mut keys, s);
        out.push((Label { keys, run: RUN_SAMPLE_LINE, list: LIST_SAMPLE_LINE }, vec![line, group.value()]));
        if i > 0 {
            let mut keys = Vec::new();
            head(&mut keys);
            keys.push((0x0201_7823, KeyValue::Double(lines[i - 1].0)));
            tail(&mut keys, s);
            out.push((Label { keys, run: RUN_INCREMENT, list: LIST_INCREMENT }, vec![line, group.value()]));
        }
    }
    Some(out)
}

/// The labels of a pipe network band reading `network` in a view from
/// `station_min`: one per pipe, then one per structure, in the network's
/// order (DU01, DW01, E-PN1-edit). A pipe's label holds its slope (0x0201783B, the
/// drop from start to end over its plan length), the largest slope of the
/// network's pipes either way (0x0201783A), the stations of its ends on the
/// alignment (0x02017835, 0x02017836), the view's first station
/// (0x02017837), 1.0 (0x02017838), 0.0 (0x02017839) and a zero byte
/// (0x0301781D); a structure's its station (0x0200D020). Each refers to
/// its part (key 0x0F01781C).
fn network_labels(doc: &CadDocument, network: Handle, alignment: &ArchEntityData, station_min: f64, number: i32) -> Option<Vec<(Label, Vec<u64>)>> {
    let n = super::network::read_network(doc, network)?;
    let elements = alignment.alignment_elements()?;
    let start = alignment.alignment_start_station()?;
    let station = |p: crate::types::Vector3| super::alignment::station_of_point(&elements, start, crate::types::Vector2::new(p.x, p.y));
    let ends: Vec<(u64, crate::types::Vector3, crate::types::Vector3)> = n.pipes.iter().map(|h| super::network::arch_entity(doc, Handle::new(*h))?.pipe_ends().map(|(a, b)| (*h, a, b))).collect::<Option<_>>()?;
    let slope = |a: crate::types::Vector3, b: crate::types::Vector3| {
        let run = ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
        if run > 0.0 { (a.z - b.z) / run } else { 0.0 }
    };
    let steepest = ends.iter().map(|e| slope(e.1, e.2).abs()).fold(0.0, f64::max);
    let mut out = Vec::new();
    for &(h, a, b) in &ends {
        let keys = vec![
            (0x0201_783B, KeyValue::Double(slope(a, b))),
            (0x0201_7838, KeyValue::Double(1.0)),
            (0x0F01_781C, KeyValue::Reference),
            (0x0201_7835, KeyValue::Double(station(a)?)),
            (0x0201_783A, KeyValue::Double(steepest)),
            (0x0201_7837, KeyValue::Double(station_min)),
            (0x0301_781D, KeyValue::Byte(0)),
            (0x0201_7839, KeyValue::Double(0.0)),
            (0x0101_5013, KeyValue::Long(number)),
            (0x0201_7836, KeyValue::Double(station(b)?)),
        ];
        out.push((Label { keys, run: RUN_PIPE, list: LIST_PIPE }, vec![h]));
    }
    for &h in &n.structures {
        let p = super::network::arch_entity(doc, Handle::new(h))?.structure_position()?;
        let keys = vec![(0x0F01_781C, KeyValue::Reference), (0x0101_5013, KeyValue::Long(number)), (0x0200_D020, KeyValue::Double(station(p)?))];
        out.push((Label { keys, run: RUN_STRUCTURE, list: LIST_STRUCTURE }, vec![h]));
    }
    Some(out)
}

fn write_label(w: &mut DwgMergedWriter, l: &Label) {
    w.write_bit_long(l.keys.len() as i32);
    for (key, value) in &l.keys {
        w.write_bit_long(*key as i32);
        match value {
            KeyValue::Long(v) => w.write_bit_long(*v),
            KeyValue::Double(v) => w.write_bit_double(*v),
            KeyValue::Byte(v) => w.write_byte(*v),
            KeyValue::NullGuid => (0..128).for_each(|_| w.write_bit(false)),
            KeyValue::Reference => {}
        }
    }
    bits(w, l.run);
    w.write_bit_long(l.list.len() as i32);
    for k in l.list {
        w.write_bit_long(*k as i32);
    }
    w.write_bit_long(0);
    w.write_bit_long(0);
}

/// Writes the label group of the band `b`, the `number`th band of `view`:
///
/// ```text
/// HEADER, BL n, n labels { BL k, k keys, 44 bits, BL m, m key ids,
///   BL 0, BL 0 }, the kind's tail around BL band number
/// references: soft 0, soft 0, view (soft), hard 0, per label its objects
///   (soft) and the view (hard)
/// texts: the kind's class text, three empty texts
/// ```
///
/// Profile data bands label the view's stations and the alignment's
/// horizontal geometry points, vertical geometry bands the profile's
/// entities, horizontal geometry bands the alignment's elements (written
/// without labels for alignments [`horizontal_labels`] does not label:
/// the application fills them once the view or its bands change).
pub(super) fn write_group(doc: &mut CadDocument, common: EntityCommon, b: &Resolved, number: i32, view: u64, frame: &ProfileViewFrame, alignment: Handle) -> Result<Handle, DxfError> {
    let a = super::network::arch_entity(doc, alignment).ok_or_else(|| invalid("alignment not found"))?;
    let labels: Vec<(Label, Vec<u64>)> = match b.band.kind {
        BandKind::SectionalData => match b.band.data_source {
            Some(group) => sectional_labels(doc, group, number).ok_or_else(|| invalid("a sectional data band's sample line group cannot be read"))?,
            None => Vec::new(),
        },
        BandKind::PipeNetwork => match b.band.data_source {
            Some(network) => network_labels(doc, network, a, frame.station_min, number).ok_or_else(|| invalid("a pipe network band's network cannot be read"))?,
            None => Vec::new(),
        },
        _ => {
    let labels: Vec<Label> = match b.band.kind {
        BandKind::ProfileData => {
            let points = geometry_points(a).unwrap_or_default();
            data_labels(frame, &b.band, number, &points).into_iter().map(|l| l.1).collect()
        }
        BandKind::VerticalGeometry => super::network::arch_entity(doc, Handle::new(b.profile1)).map(|p| vertical_labels(p, number)).unwrap_or_default(),
        BandKind::HorizontalGeometry => horizontal_labels(a, number).unwrap_or_default().into_iter().map(|l| l.1).collect(),
        // Without a data source (superelevation: not written) these
        // bands have no labels.
        BandKind::Superelevation | BandKind::SectionalData | BandKind::PipeNetwork => Vec::new(),
    };
    labels.into_iter().map(|l| (l, Vec::new())).collect()
        }
    };
    let mut refs = vec![soft_pointer(0), soft_pointer(0), soft_pointer(view), hard_pointer(0)];
    for (_, objects) in &labels {
        refs.extend(objects.iter().map(|h| soft_pointer(*h)));
        refs.push(hard_pointer(view));
    }
    let mut strings = vec![b.band.kind.class_text().to_string()];
    strings.extend((0..3).map(|_| String::new()));
    let (before, after) = tail(b.band.kind);
    put_entity(
        doc,
        b.band.kind.dxf(),
        common,
        |w| {
            bits(w, HEADER);
            w.write_bit_long(labels.len() as i32);
            for (l, _) in &labels {
                write_label(w, l);
            }
            bits(w, &before[..STAGGER_AT]);
            w.write_bit_long(b.band.stagger as i32);
            bits(w, &before[STAGGER_AT + 2..]);
            w.write_bit_long(number);
            bits(w, after);
        },
        Body { refs, strings },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The profile data labels of PB04's view: stations -200 to 500 every
    /// 25 with the alignment's start, arc and end points in between, the
    /// start point (station 0) in place of the major station label there.
    #[test]
    fn profile_data_labels_of_a_view() {
        let frame = ProfileViewFrame { origin: crate::types::Vector2::new(0.0, -100.0), station_min: -200.0, station_max: 500.0, elevation_min: 0.0, elevation_max: 20.0, vertical_scale: 10.0 };
        let points = [(0.0, 0x0200_D004, 1, 0.0), (81.1146, 0x0400_D053, 3, 0.0), (99.6605, 0x0400_D08E, 3, 0.5), (118.2064, 0x0400_D054, 3, 1.0), (211.1243, 0x0200_D005, 2, 1.0)];
        let labels = data_labels(&frame, &ProfileViewBand::new(BandKind::ProfileData), 1, &points);
        assert_eq!(labels.len(), 33);
        assert_eq!((labels[0].0, labels[0].1.list), (-200.0, LIST_ENDS));
        assert_eq!((labels[8].0, labels[8].1.run), (0.0, RUN_POINT));
        assert_eq!((labels[32].0, labels[32].1.keys.len()), (500.0, 3));
        let mut b = ProfileViewBand::new(BandKind::ProfileData);
        b.label_at_start = false;
        assert_eq!(data_labels(&frame, &b, 1, &points).len(), 32);
    }

    /// The horizontal geometry labels of HG3P's alignment (line, left arc,
    /// line, right arc, line): an element label each and a middle label
    /// per arc, lines numbered first.
    #[test]
    fn horizontal_geometry_labels_of_an_alignment() {
        use super::super::alignment::{add_alignment_with_curves, VertexCurve};
        let v = crate::types::Vector2::new;
        let mut doc = CadDocument::new();
        let pts = [v(0.0, 0.0), v(100.0, 0.0), v(200.0, 60.0), v(300.0, 0.0)];
        let a = add_alignment_with_curves(&mut doc, "AL1", &pts, &[VertexCurve::Arc(144.4126919312708), VertexCurve::Arc(51.07935859793733)], None).unwrap();
        let labels = horizontal_labels(super::super::network::arch_entity(&doc, a).unwrap(), 1).unwrap();
        let key = |l: &Label, k: u32| l.keys.iter().find(|x| x.0 == k).map(|x| match x.1 {
            KeyValue::Long(i) => f64::from(i),
            KeyValue::Double(d) => d,
            KeyValue::Byte(b) => f64::from(b),
            KeyValue::NullGuid | KeyValue::Reference => 0.0,
        });
        assert_eq!(labels.len(), 7);
        assert_eq!((labels[1].1.list, key(&labels[1].1, 0x0100_D61E)), (LIST_CURVE_MIDDLE, Some(1.0)));
        let arc = &labels[2].1;
        assert_eq!((key(arc, 0x0101_7829), key(arc, 0x0101_782A), key(arc, 0x0100_D61E)), (Some(260.0), Some(261.0), Some(4.0)));
        assert!((key(arc, 0x0201_782D).unwrap() + 144.4126919312708).abs() < 1e-9);
        assert!((labels[2].0 - 99.0217174).abs() < 1e-6);
        assert_eq!((key(&labels[6].1, 0x0101_782A), key(&labels[6].1, 0x0301_782C)), (Some(258.0), Some(0.0)));
    }

    /// A sectional data band labels its group's lines in station order,
    /// each followed by the step from the line before.
    #[test]
    fn sectional_data_labels_of_a_group() {
        use super::super::alignment::{add_alignment_with_curves, VertexCurve};
        use super::super::sample_lines::{add_sample_line, add_sample_line_group};
        let v = crate::types::Vector2::new;
        let mut doc = CadDocument::new();
        let a = add_alignment_with_curves(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], &[] as &[VertexCurve], None).unwrap();
        let g = add_sample_line_group(&mut doc, a, "SLG1", None).unwrap();
        let far = add_sample_line(&mut doc, g, "SL1", 50.0, 10.0, 10.0, None).unwrap();
        let near = add_sample_line(&mut doc, g, "SL2", 20.0, 5.0, 7.0, None).unwrap();
        let labels = sectional_labels(&doc, g, 1).unwrap();
        let station = |l: &Label, k: u32| l.keys.iter().find_map(|x| match x { (key, KeyValue::Double(d)) if *key == k => Some(*d), _ => None });
        assert_eq!(labels.len(), 3);
        assert_eq!((station(&labels[0].0, 0x0200_D020), labels[0].1.clone()), (Some(20.0), vec![near.value(), g.value()]));
        assert_eq!((labels[2].0.list, station(&labels[2].0, 0x0201_7823), station(&labels[2].0, 0x0200_D020), labels[2].1[0]), (LIST_INCREMENT, Some(20.0), Some(50.0), far.value()));
    }

    /// A pipe network band labels the network's pipes (slopes, end
    /// stations, the largest slope either way) then its structures; a
    /// point beside an arc gets the arc's station.
    #[test]
    fn pipe_network_labels_of_a_network() {
        use super::super::alignment::{add_alignment_with_curves, VertexCurve};
        use super::super::network::{add_pipe, add_pipe_network, add_structure};
        use crate::types::Vector3;
        let v = crate::types::Vector2::new;
        let mut doc = CadDocument::new();
        let a = add_alignment_with_curves(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0), v(200.0, 60.0), v(300.0, 0.0)], &[VertexCurve::Arc(144.4126919312708), VertexCurve::Arc(51.07935859793733)], None).unwrap();
        let n = add_pipe_network(&mut doc, "NET1", None).unwrap();
        add_pipe(&mut doc, n, Vector3::new(0.0, 0.0, -5.0), Vector3::new(40.0, 0.0, -6.0)).unwrap();
        add_structure(&mut doc, n, Vector3::new(20.0, 5.0, 3.0), 0.0).unwrap();
        add_pipe(&mut doc, n, Vector3::new(40.0, 0.0, -6.0), Vector3::new(60.0, 2.0, -5.0)).unwrap();
        add_structure(&mut doc, n, Vector3::new(70.0, 5.0, 3.0), 0.0).unwrap();
        let al = super::super::network::arch_entity(&doc, a).unwrap();
        let labels = network_labels(&doc, n, al, -200.0, 1).unwrap();
        let value = |l: &Label, k: u32| l.keys.iter().find_map(|x| match x { (key, KeyValue::Double(d)) if *key == k => Some(*d), _ => None });
        assert_eq!(labels.iter().map(|l| l.0.list).collect::<Vec<_>>(), vec![LIST_PIPE, LIST_PIPE, LIST_STRUCTURE, LIST_STRUCTURE]);
        let up = -1.0 / 20.0f64.hypot(2.0);
        assert_eq!((value(&labels[0].0, 0x0201_783B), value(&labels[0].0, 0x0201_7836), value(&labels[0].0, 0x0201_7837)), (Some(0.025), Some(40.0), Some(-200.0)));
        assert!((value(&labels[1].0, 0x0201_783B).unwrap() - up).abs() < 1e-12 && (value(&labels[0].0, 0x0201_783A).unwrap() + up).abs() < 1e-12);
        // E-PN1-edit: the structure at (70, 5) sits at 70.34093655710718.
        assert!((value(&labels[3].0, 0x0200_D020).unwrap() - 70.34093655710718).abs() < 1e-9);
    }
}
