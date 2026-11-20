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
}

/// A band of a profile view, drawn with the drawing's default band style
/// of its kind.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProfileViewBand {
    pub kind: BandKind,
    /// Above the view; only profile data bands are written above.
    pub top: bool,
    /// The profiles the band reads; the view's first profile when `None`.
    pub profile1: Option<Handle>,
    pub profile2: Option<Handle>,
    /// Gap between the view and the band (or the band before).
    pub gap: f64,
    pub label_at_start: bool,
    pub label_at_end: bool,
}

impl ProfileViewBand {
    /// A band below the view with the application's defaults: a gap of
    /// half an inch in feet, labels at the start and end stations except
    /// for vertical geometry bands.
    pub fn new(kind: BandKind) -> Self {
        let ends = kind != BandKind::VerticalGeometry;
        ProfileViewBand { kind, top: false, profile1: None, profile2: None, gap: 0.5 / 12.0, label_at_start: ends, label_at_end: ends }
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
        }
    }

    fn dxf(self) -> &'static str {
        match self {
            BandKind::ProfileData => "AECC_PROFILEDATABAND_LABELGROUP",
            BandKind::VerticalGeometry => "AECC_VERTICALGEOMETRYBAND_LABELGROUP",
            BandKind::HorizontalGeometry => "AECC_HORIZONTALGEOMETRYBAND_LABELGROUP",
        }
    }

    fn class_text(self) -> &'static str {
        match self {
            BandKind::ProfileData => "AeccImpProfileDataBandLabeling",
            BandKind::VerticalGeometry => "AeccImpVerticalGeometryBandLabeling",
            BandKind::HorizontalGeometry => "AeccImpHorizontalGeometryBandLabeling",
        }
    }

    /// The word of the kind's geometry band items (2 vertical, 3
    /// horizontal).
    fn word(self) -> i32 {
        match self {
            BandKind::ProfileData => 1,
            BandKind::VerticalGeometry => 2,
            BandKind::HorizontalGeometry => 3,
        }
    }
}

/// A profile data band item after its gap (`BD`): bit 4 label at end, bit
/// 5 label at start, bit 9 below the view; above the view bits 921 and
/// 923 are 1 and 0.
const DATA_ITEM: (usize, &str) = (1056, "0c404000000000000000059408111111111111913f0000000000000000000000000000000000000000000000000000000000000000243801010000008040000030100000100400000501000001804000007010000020040000090100000280400000b0100000300400000d01000007404000101000000000000165000000000000003940");
/// A vertical or horizontal geometry band item after its gap: bits 4 and
/// 5 as above, the band kind word in bits 436 to 439 (2 vertical, 3
/// horizontal).
const GEOMETRY_ITEM: (usize, &str) = (572, "0c404000000000000000059408111111111111913f00000000000000000000000000000000000000000000000000000000000000002903000000000000165000000000000003940");

fn hex_bits((len, hex): (usize, &str)) -> Vec<bool> {
    hex.chars().flat_map(|c| {
        let v = c.to_digit(16).unwrap_or(0);
        (0..4).rev().map(move |i| v >> i & 1 == 1)
    }).take(len).collect()
}

/// Writes the view's band items: each `BD gap` and its kind's template.
pub(super) fn write_items(w: &mut DwgMergedWriter, bands: &[Resolved]) {
    for b in bands {
        w.write_bit_double(b.band.gap);
        let mut bits = hex_bits(if b.band.kind == BandKind::ProfileData { DATA_ITEM } else { GEOMETRY_ITEM });
        bits[4] = b.band.label_at_end;
        bits[5] = b.band.label_at_start;
        match b.band.kind {
            BandKind::ProfileData => {
                bits[9] = !b.band.top;
                bits[921] = b.band.top;
                bits[923] = !b.band.top;
            }
            kind => {
                let word = kind.word() as u8;
                for i in 0..4 {
                    bits[436 + i] = word >> (3 - i) & 1 == 1;
                }
            }
        }
        for bit in bits {
            w.write_bit(bit);
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
}

const RUN_ENDS: &str = "00000000010000000001100100000000000000000000";
const RUN_MINOR: &str = "00000000010000000010100100000000000000000000";
const RUN_POINT: &str = "00000000010000000100100100000000000000000000";
const RUN_VERTICAL: &str = "00000000010000000111100100000000000000000000";
const LIST_ENDS: &[u32] = &[0x0101_5013, 0x0200_D020, 0x0300_D622, 0x0300_D623];
const LIST_MINOR: &[u32] = &[0x0101_5013, 0x0200_D020, 0x0200_D620];
const LIST_POINT: &[u32] = &[0x0100_D068, 0x0100_D61E, 0x0100_D621, 0x0101_5013, 0x0200_D61D];
const LIST_VERTICAL: &[u32] = &[0x0100_FC31, 0x0101_5013];

/// Station spacing of the default profile data band style's labels.
const MAJOR: f64 = 100.0;
const MINOR: f64 = 25.0;

/// The run every band label group opens with (that of the profile label
/// groups).
const HEADER: &str = super::profile_view::GROUP_HEADER;

/// What follows a group's labels, around its band number (`BL`, 1 for the
/// first band, the bands below the view first).
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

fn write_label(w: &mut DwgMergedWriter, l: &Label) {
    w.write_bit_long(l.keys.len() as i32);
    for (key, value) in &l.keys {
        w.write_bit_long(*key as i32);
        match value {
            KeyValue::Long(v) => w.write_bit_long(*v),
            KeyValue::Double(v) => w.write_bit_double(*v),
            KeyValue::Byte(v) => w.write_byte(*v),
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
/// references: soft 0, soft 0, view (soft), hard 0, per label view (hard)
/// texts: the kind's class text, three empty texts
/// ```
///
/// Profile data bands label the view's stations and the alignment's
/// horizontal geometry points, vertical geometry bands the profile's
/// entities; horizontal geometry bands are written without labels (the
/// application fills them once the view or its bands change).
pub(super) fn write_group(doc: &mut CadDocument, common: EntityCommon, b: &Resolved, number: i32, view: u64, frame: &ProfileViewFrame, alignment: Handle) -> Result<Handle, DxfError> {
    let a = super::network::arch_entity(doc, alignment).ok_or_else(|| invalid("alignment not found"))?;
    let labels: Vec<Label> = match b.band.kind {
        BandKind::ProfileData => {
            let points = geometry_points(a).unwrap_or_default();
            data_labels(frame, &b.band, number, &points).into_iter().map(|l| l.1).collect()
        }
        BandKind::VerticalGeometry => super::network::arch_entity(doc, Handle::new(b.profile1)).map(|p| vertical_labels(p, number)).unwrap_or_default(),
        BandKind::HorizontalGeometry => Vec::new(),
    };
    let mut refs = vec![soft_pointer(0), soft_pointer(0), soft_pointer(view), hard_pointer(0)];
    refs.extend(labels.iter().map(|_| hard_pointer(view)));
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
            for l in &labels {
                write_label(w, l);
            }
            bits(w, before);
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
}
