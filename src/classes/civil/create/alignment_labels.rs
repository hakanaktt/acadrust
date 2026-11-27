//! Alignment label groups (a labeled alignment's station, minor station
//! and geometry point labels), written by their fields.
//!
//! A label group is an entity in model space whose class data is the
//! record [`labels.rs`](super::super::labels) reads: a class header, the
//! labels as keyed property sets with their overrides, then the group's
//! settings (a label set key, two words, five doubles, the band range)
//! and the kind's own tail (a station increment, or the geometry point
//! kinds), closed by `B, BS 0x04BE`. The record names no handle; the
//! alignment and the label style are the entity's references:
//!
//! ```text
//! references: soft 0, soft 0, alignment (soft), label style (hard),
//!   per label { alignment (hard) }
//! texts: the kind's implementation class text (`AeccImp*`), three empty texts
//! ```
//!
//! The form is the corpus's (Align-2, Align-6A, Labels-Alignments,
//! Parcel-1A): every label carries `2 bits 00, RC 1, BL flag` after its
//! keys and lists its label set's key ids; the first and last label of a
//! station group carry the begin/end marks instead of a station when the
//! alignment is labeled left to right. Records a drawing stores in
//! another form (nested labels, unknown override value types) are not
//! written: the typed fields below cannot hold them, and the corpus
//! builder leaves such a group as a gap.

use super::support::*;
use crate::entities::EntityCommon;
use crate::error::DxfError;
use crate::types::Handle;
use crate::CadDocument;

/// The kind of an alignment label group: which labeled set it places
/// labels for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AlignmentLabelGroupKind {
    /// Major station labels (`AECC_ALIGNMENT_STATION_LABEL_GROUP`).
    Station,
    /// Minor station labels (`AECC_ALIGNMENT_MINOR_STATION_LABEL_GROUP`).
    MinorStation,
    /// Geometry point labels (`AECC_ALIGNMENT_GEOMPOINT_LABEL_GROUP`).
    GeomPoint,
}

impl AlignmentLabelGroupKind {
    /// The entity class and its C++ name, per kind.
    pub fn dxf(self) -> &'static str {
        match self {
            Self::Station => "AECC_ALIGNMENT_STATION_LABEL_GROUP",
            Self::MinorStation => "AECC_ALIGNMENT_MINOR_STATION_LABEL_GROUP",
            Self::GeomPoint => "AECC_ALIGNMENT_GEOMPOINT_LABEL_GROUP",
        }
    }

    /// The class text the record carries (strings[0]): the implementation
    /// class's name — Civil creates the object through its `AeccImp*` class,
    /// and the enabler resolves that name at load.
    fn class_text(self) -> &'static str {
        match self {
            Self::Station => "AeccImpAlignmentStationLabeling",
            Self::MinorStation => "AeccImpAlignmentMinorStationLabeling",
            Self::GeomPoint => "AeccImpAlignmentGeomPointLabeling",
        }
    }

    /// The label set key the group's settings name (a bare key: type
    /// 0x0F carries no value).
    fn set_key(self) -> u32 {
        match self {
            Self::Station => 0x0F00_B401,
            Self::MinorStation => 0x0F00_B402,
            Self::GeomPoint => 0x0F00_B403,
        }
    }

    /// The BL every label carries after its keys (station and minor
    /// station labels 0x1000, geometry point labels 0x1003).
    fn label_flag(self) -> u32 {
        match self {
            Self::Station | Self::MinorStation => 0x1000,
            Self::GeomPoint => 0x1003,
        }
    }

    /// The key ids a label lists, in the samples' order (the label set's
    /// fields: a station group labels stations and the begin/end marks, a
    /// minor group only stations, a geometry point group the point's
    /// property, index and mark keys).
    fn label_ids(self) -> &'static [u32] {
        match self {
            Self::Station => &[0x0200_D020, 0x0300_D622, 0x0300_D623],
            Self::MinorStation => &[0x0200_D020],
            Self::GeomPoint => &[0x0100_D068, 0x0100_D61E, 0x0100_D621, 0x0200_D61D],
        }
    }
}

/// The typed value of a label's key.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LabelKeyValue {
    /// A BD (`0x02..` keys: the station 0x0200D020, the intervals
    /// 0x0200D61F major and 0x0200D620 minor, the geometry point mark
    /// 0x0200D61D).
    Double(f64),
    /// A BL (`0x01..` keys: the geometry point property 0x0100D068 and
    /// index 0x0100D61E).
    Long(u32),
    /// One byte (`0x03..` keys: the begin/end marks 0x0300D622/23).
    Byte(u8),
}

/// The value of a label override's property, by the property's 7-bit
/// type (`labels.rs` reads these five).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum LabelOverrideValue {
    /// Type 3 or 26: one byte.
    Byte(u8),
    /// Type 2: a BD.
    Double(f64),
    /// Types 8 and 9: three BD (a point, a dragged offset).
    Point([f64; 3]),
    /// Types 13 and 25: a BL.
    Long(u32),
    /// Type 16: the property names a reference; no value.
    None,
}

impl LabelOverrideValue {
    fn type_bits(&self) -> u32 {
        match self {
            Self::Double(_) => 2,
            Self::Byte(_) => 3,
            Self::Point(_) => 9,
            Self::Long(_) => 13,
            Self::None => 16,
        }
    }
}

/// One property of a label's override: the property set `set` (bytes C0
/// 0C for the label's own overrides), the property's `id` in it, and the
/// typed value.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LabelOverrideProperty {
    pub id: u8,
    pub set: u16,
    pub value: LabelOverrideValue,
}

/// One label of the group: its keys with their values, in stored order
/// (the first key's value sits at the label's start), the key ids it
/// lists, and its overrides (one per dragged or flipped state; an empty
/// list leaves the label in place).
#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlignmentGroupLabel {
    pub keys: Vec<(u32, LabelKeyValue)>,
    /// The key ids the label lists; the group's kind gives the sample
    /// order when empty.
    pub ids: Vec<u32>,
    /// The label's overrides, each a list of properties.
    #[cfg_attr(feature = "serde", serde(default))]
    pub overrides: Vec<Vec<LabelOverrideProperty>>,
}

/// An alignment label group's record, by its fields.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlignmentLabelGroup {
    /// Which labels the group places.
    pub kind: AlignmentLabelGroupKind,
    /// The labels, in stored order.
    pub labels: Vec<AlignmentGroupLabel>,
    /// The two words of the settings block (`69, 1` in the samples).
    pub words: (u32, u32),
    /// The five doubles of the settings block (text heights as fractions
    /// of the drawing scale in the samples).
    pub doubles: [f64; 5],
    /// The band range: flag bits, start station, end station (`7, 0, 0`
    /// — an automatic range — in the samples).
    pub range: (u32, f64, f64),
    /// The station increment (station and minor station groups only).
    pub increment: Option<f64>,
    /// The geometry point kinds the settings list (geometry point groups
    /// only). The recorded words: the corpus writes each as
    /// `0x0100 | kind` — the low byte is the kind the reader surfaces.
    /// The samples write each word as a full 32-bit long, which the
    /// writer reproduces.
    pub point_kinds: Vec<u32>,
    /// The 16 bytes the record carries after its class tag. The
    /// application does not keep them: across the corpus they are zeros,
    /// UTF-16 string fragments and heap addresses — residual bytes the
    /// reader skips. Zeros are what a new group gets; a group rebuilt
    /// from a recorded drawing carries the recorded bytes verbatim.
    #[cfg_attr(feature = "serde", serde(default))]
    pub guid: [u8; 16],
}

impl AlignmentLabelGroup {
    /// The labels with the group kind's flag and key ids applied to
    /// labels that leave the ids unset (a decoded group carries its ids
    /// on every label; a hand-built one may rely on the kind).
    fn resolved_labels(&self) -> impl Iterator<Item = (&AlignmentGroupLabel, u32, &[u32])> {
        let flag = self.kind.label_flag();
        let ids: &[u32] = self.kind.label_ids();
        self.labels.iter().map(move |l| (l, flag, if l.ids.is_empty() { ids } else { &l.ids }))
    }
}

/// The class header every label record opens with (the samples' bit run:
/// `BL 80, 0, 80, 80, 0, 0x10025F04`, the 6-bit class tag `0b010010`,
/// the record's 16 residual bytes, `BL 80, 80, 0x04BE0000, 0xFFFFFF`,
/// 3 bits).
fn header(w: &mut crate::io::dwg::dwg_stream_writers::merged_writer::DwgMergedWriter, guid: &[u8; 16]) {
    for v in [80u32, 0, 80, 80, 0, 0x1002_5F04] {
        w.write_bit_long(v as i32);
    }
    bits(w, "010010");
    w.write_bytes(guid);
    for v in [80u32, 80, 0x04BE_0000, 0x00FF_FFFF] {
        w.write_bit_long(v as i32);
    }
    bits(w, "010");
}

fn key_value(w: &mut crate::io::dwg::dwg_stream_writers::merged_writer::DwgMergedWriter, value: &LabelKeyValue) {
    match value {
        LabelKeyValue::Double(v) => w.write_bit_double(*v),
        LabelKeyValue::Long(v) => w.write_bit_long(*v as i32),
        LabelKeyValue::Byte(v) => bits(w, &format!("{v:08b}")),
    }
}

fn override_property(w: &mut crate::io::dwg::dwg_stream_writers::merged_writer::DwgMergedWriter, p: &LabelOverrideProperty, last: bool) {
    w.write_byte(p.id);
    let set = p.set.to_le_bytes();
    w.write_byte(set[0]);
    w.write_byte(set[1]);
    bits(w, &format!("{:07b}", (&p.value).type_bits()));
    match &p.value {
        LabelOverrideValue::Byte(v) => w.write_byte(*v),
        LabelOverrideValue::Double(v) => w.write_bit_double(*v),
        LabelOverrideValue::Point(v) => {
            for c in v {
                w.write_bit_double(*c);
            }
        }
        LabelOverrideValue::Long(v) => w.write_bit_long(*v as i32),
        LabelOverrideValue::None => {}
    }
    bits(w, if last { "010" } else { "000" });
}

/// Writes an alignment label group of `alignment` labeled with `style`
/// (the group's label style), on `layer` (the drawing's default when
/// `None`). The record is [`AlignmentLabelGroup`]; the references are
/// the samples': soft 0, soft 0, the alignment (soft), the style (hard),
/// then one alignment soft pointer per label.
///
/// Refuses an unknown layer, an alignment that is not an `AECC_ALIGNMENT`
/// and a range or increment outside the form.
pub fn add_alignment_label_group(
    doc: &mut CadDocument,
    alignment: Handle,
    style: Handle,
    layer: Option<&str>,
    group: &AlignmentLabelGroup,
) -> Result<Handle, DxfError> {
    let is_alignment = arch_of(doc, alignment).is_some_and(|name| crate::classes::civil::kind_of(&name) == Some(crate::classes::civil::CivilKind::Alignment));
    if !is_alignment {
        return Err(invalid("the label group's alignment is not an AECC_ALIGNMENT"));
    }
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let (range_bits, range_start, range_end) = group.range;
    if range_bits > 7 || group.labels.is_empty() {
        return Err(invalid("alignment label group out of the recorded form"));
    }
    let tailed = match group.kind {
        AlignmentLabelGroupKind::GeomPoint => group.increment.is_none(),
        _ => group.increment.is_some() && group.point_kinds.is_empty(),
    };
    if !tailed {
        // A geometry point group lists kinds; a station group has an
        // increment and no kinds. Exactly one of the two.
        return Err(invalid("the label group's kind does not match its tail"));
    }
    let owner = doc.header.model_space_block_handle;
    let handle = doc.allocate_handle();
    let common = EntityCommon {
        handle,
        owner_handle: owner,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    let mut refs = vec![soft_pointer(0), soft_pointer(0), soft_pointer(alignment.value()), hard_pointer(style.value())];
    for _ in &group.labels {
        refs.push(hard_pointer(alignment.value()));
    }
    let strings = vec![group.kind.class_text().to_string(), String::new(), String::new(), String::new()];
    put_entity(
        doc,
        group.kind.dxf(),
        common,
        |w| {
            header(w, &group.guid);
            w.write_bit_long(group.labels.len() as i32);
            for (label, flag, ids) in group.resolved_labels() {
                w.write_bit_long(label.keys.len() as i32);
                for (k, v) in &label.keys {
                    w.write_bit_long(*k as i32);
                    key_value(w, v);
                }
                bits(w, "00");
                w.write_byte(1);
                w.write_bit_long(flag as i32);
                w.write_bit_long(ids.len() as i32);
                for id in ids {
                    w.write_bit_long(*id as i32);
                }
                w.write_bit_long(0);
                w.write_bit_long(label.overrides.len() as i32);
                for override_props in &label.overrides {
                    w.write_bit_long(1);
                    w.write_bit_long(0);
                    w.write_bit_long(0);
                    bits(w, "00");
                    w.write_byte((override_props.len() * 8) as u8);
                    for (i, p) in override_props.iter().enumerate() {
                        override_property(w, p, i + 1 == override_props.len());
                    }
                }
            }
            // The settings: one bare key (the label set), two words, five
            // doubles.
            w.write_bit_long(1);
            w.write_bit_long(group.kind.set_key() as i32);
            w.write_bit_long(group.words.0 as i32);
            w.write_bit_long(group.words.1 as i32);
            for d in &group.doubles {
                w.write_bit_double(*d);
            }
            // The range: 3 flag bits, start, end.
            bits(w, &format!("{range_bits:03b}"));
            w.write_bit_double(range_start);
            w.write_bit_double(range_end);
            // The kind's tail: a station increment, or the kinds (each a
            // full 32-bit long, as the samples write them).
            if let Some(inc) = group.increment {
                w.write_bit_double(inc);
            } else {
                w.write_bit_long(group.point_kinds.len() as i32);
                for k in &group.point_kinds {
                    // The samples write each kind as a full BL (mode 00,
                    // 32 raw bits) even for one-digit kinds.
                    w.write_2bits(0);
                    w.write_raw_long(*k as i32);
                }
            }
            // Close: B, BS 0x04BE.
            w.write_bit(false);
            w.write_bit_short(0x04BE);
        },
        Body { refs, strings },
    )?;
    Ok(handle)
}

/// The dxf name of the entity at `handle`, when it is an Arch entity.
fn arch_of(doc: &CadDocument, handle: Handle) -> Option<String> {
    match doc.get_entity(handle) {
        Some(crate::entities::EntityType::Extended(e)) => match &e.data {
            crate::entities::ExtendedEntityData::Arch(a) => Some(a.dxf_name.clone()),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classes::civil::{add_alignment, ensure_civil};
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::{DxfVersion, Vector2};

    fn drawing() -> CadDocument {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        ensure_civil(&mut doc).unwrap();
        doc
    }

    fn roundtrip(doc: &CadDocument, tag: &str) -> CadDocument {
        let path = std::env::temp_dir().join(format!("rt_alignlabel_{tag}_{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        back
    }

    fn entity(doc: &CadDocument, h: Handle) -> crate::classes::arch::ArchEntityData {
        match doc.get_entity(h) {
            Some(crate::entities::EntityType::Extended(e)) => match &e.data {
                crate::entities::ExtendedEntityData::Arch(a) => a.clone(),
                _ => panic!("not arch"),
            },
            _ => panic!("missing"),
        }
    }

    fn refs(data: &crate::classes::arch::ArchEntityData) -> Vec<(u64, u8)> {
        data.typed_handles().iter().map(|r| (r.0, r.1)).collect()
    }

    /// The Align-2 station group's fields: 13 stations every 100, the
    /// begin and end marks around them, the settings' five doubles and
    /// the automatic range.
    fn align2_station() -> AlignmentLabelGroup {
        let station = |v: f64| LabelKeyValue::Double(v);
        let mut labels = vec![AlignmentGroupLabel {
            keys: vec![(0x0300_D622, LabelKeyValue::Byte(1)), (0x0200_D61F, station(100.0))],
            ids: vec![0x0200_D020, 0x0300_D622, 0x0300_D623],
            overrides: Vec::new(),
        }];
        for i in 1..=13u32 {
            labels.push(AlignmentGroupLabel {
                keys: vec![(0x0200_D61F, station(100.0)), (0x0200_D020, station(i as f64 * 100.0))],
                ids: vec![0x0200_D020, 0x0300_D622, 0x0300_D623],
                overrides: Vec::new(),
            });
        }
        labels.push(AlignmentGroupLabel {
            keys: vec![(0x0200_D61F, station(100.0)), (0x0300_D623, LabelKeyValue::Byte(1))],
            ids: vec![0x0200_D020, 0x0300_D622, 0x0300_D623],
            overrides: Vec::new(),
        });
        AlignmentLabelGroup {
            kind: AlignmentLabelGroupKind::Station,
            labels,
            words: (69, 1),
            doubles: [0.0, 0.0, 0.0, 1.0 / 12.0, 1.0 / 6.0],
            range: (7, 0.0, 0.0),
            increment: Some(100.0),
            point_kinds: Vec::new(),
            guid: [0; 16],
        }
    }

    /// The Align-2 geometry point group's fields: four labels (the
    /// alignment's beginning, one curve point pair and its end), the
    /// kinds list 1..=13.
    fn align2_geompoint() -> AlignmentLabelGroup {
        let label = |mark: f64, property: u32, index: u32| AlignmentGroupLabel {
            keys: vec![(0x0200_D61D, LabelKeyValue::Double(mark)), (0x0100_D068, LabelKeyValue::Long(property)), (0x0100_D61E, LabelKeyValue::Long(index))],
            ids: vec![0x0100_D068, 0x0100_D61E, 0x0100_D621, 0x0200_D61D],
            overrides: Vec::new(),
        };
        AlignmentLabelGroup {
            kind: AlignmentLabelGroupKind::GeomPoint,
            labels: vec![
                label(0.0, 0x0200_D004, 1),
                label(1.0, 0x0400_D052, 1),
                label(1.0, 0x0400_D052, 2),
                label(1.0, 0x0200_D005, 3),
            ],
            words: (69, 1),
            doubles: [0.0, 0.0, 0.0, 1.0 / 12.0, 1.0 / 6.0],
            range: (7, 0.0, 0.0),
            increment: None,
            point_kinds: (1..=13).map(|k| 0x100 | k).collect(),
            guid: [0; 16],
        }
    }

    /// The Align-2 minor station group's fields: 13 stations every 100
    /// from 50, major 100, minor 50, increment 50.
    fn align2_minor() -> AlignmentLabelGroup {
        let mut labels = Vec::new();
        for i in 0..13u32 {
            labels.push(AlignmentGroupLabel {
                keys: vec![(0x0200_D61F, LabelKeyValue::Double(100.0)), (0x0200_D620, LabelKeyValue::Double(50.0)), (0x0200_D020, LabelKeyValue::Double(50.0 + i as f64 * 100.0))],
                ids: vec![0x0200_D020],
                overrides: Vec::new(),
            });
        }
        AlignmentLabelGroup {
            kind: AlignmentLabelGroupKind::MinorStation,
            labels,
            words: (69, 1),
            doubles: [0.0, 0.0, 0.0, 1.0 / 12.0, 1.0 / 6.0],
            range: (7, 0.0, 0.0),
            increment: Some(50.0),
            point_kinds: Vec::new(),
            guid: [0; 16],
        }
    }

    #[test]
    fn station_group_round_trips_by_its_fields() {
        let mut doc = drawing();
        let al = add_alignment(&mut doc, "AL-L", &[Vector2::new(0.0, 0.0), Vector2::new(1400.0, 0.0)], None).unwrap();
        let style = crate::classes::civil::add_point_label_style(&mut doc, "STATION LABEL", |_| {}).unwrap();
        let group = align2_station();
        let h = add_alignment_label_group(&mut doc, al, style, None, &group).unwrap();
        let back = roundtrip(&doc, "station");
        let e = entity(&back, h);
        assert_eq!(e.dxf_name, "AECC_ALIGNMENT_STATION_LABEL_GROUP");
        assert_eq!(e.cpp_class_name, "AeccDbAlignmentStationLabeling");
        // The stations decode to the fixture's, the markers' labels
        // excepted.
        assert_eq!(e.label_stations(), (1..=13).map(|i| i as f64 * 100.0).collect::<Vec<_>>());
        assert_eq!(e.label_anchor(), None);
        // References: soft 0, soft 0, alignment, style, one alignment per
        // label. Kinds: 2 soft pointer, 5 hard pointer (decode.rs).
        let r = refs(&e);
        assert_eq!(r[0], (0, 4));
        assert_eq!(r[1], (0, 4));
        assert_eq!(r[2], (al.value(), 4));
        assert_eq!(r[3], (style.value(), 5));
        assert_eq!(r.len(), 4 + group.labels.len());
        assert!(r[4..].iter().all(|r| *r == (al.value(), 5)));
    }

    #[test]
    fn minor_and_geompoint_groups_round_trip_by_their_fields() {
        let mut doc = drawing();
        let al = add_alignment(&mut doc, "AL-M", &[Vector2::new(0.0, 0.0), Vector2::new(1400.0, 0.0)], None).unwrap();
        let style = crate::classes::civil::add_point_label_style(&mut doc, "GROUP LABEL", |_| {}).unwrap();
        let minor = align2_minor();
        let points = align2_geompoint();
        let hm = add_alignment_label_group(&mut doc, al, style, None, &minor).unwrap();
        let hp = add_alignment_label_group(&mut doc, al, style, None, &points).unwrap();
        let back = roundtrip(&doc, "kinds");
        let m = entity(&back, hm);
        assert_eq!(m.dxf_name, "AECC_ALIGNMENT_MINOR_STATION_LABEL_GROUP");
        assert_eq!(m.label_stations(), (0..13).map(|i| 50.0 + i as f64 * 100.0).collect::<Vec<_>>());
        let p = entity(&back, hp);
        assert_eq!(p.dxf_name, "AECC_ALIGNMENT_GEOMPOINT_LABEL_GROUP");
        assert_eq!(p.label_stations(), Vec::<f64>::new());
        assert_eq!(p.label_geometry_point_kinds(), (1..=13).collect::<Vec<_>>());
    }

    #[test]
    fn overrides_and_layers_round_trip() {
        let mut doc = drawing();
        doc.layers.add(crate::tables::Layer::new("C-ROAD-TEXT"));
        let al = add_alignment(&mut doc, "AL-O", &[Vector2::new(0.0, 0.0), Vector2::new(600.0, 0.0)], None).unwrap();
        let style = crate::classes::civil::add_point_label_style(&mut doc, "GROUP LABEL", |_| {}).unwrap();
        // The Parcel-1A form: the end label flipped (a type 3 override in
        // the label's own property set C0 0C) and the group on a named
        // layer.
        let mut group = align2_station();
        group.labels[0].overrides = vec![vec![LabelOverrideProperty { id: 12, set: 0x0CC0, value: LabelOverrideValue::Byte(0) }]];
        let h = add_alignment_label_group(&mut doc, al, style, Some("C-ROAD-TEXT"), &group).unwrap();
        let back = roundtrip(&doc, "overrides");
        let e = entity(&back, h);
        assert_eq!(e.label_stations(), (1..=13).map(|i| i as f64 * 100.0).collect::<Vec<_>>());
        let common = match back.get_entity(h) {
            Some(crate::entities::EntityType::Extended(e)) => e.common.clone(),
            _ => panic!("missing"),
        };
        assert_eq!(common.layer, "C-ROAD-TEXT");
        // The dragged offset stays None (the override's value is a byte,
        // not a point).
        assert_eq!(e.label_dragged_offset(), None);
    }

    #[test]
    fn the_writer_refuses_out_of_form_groups() {
        let mut doc = drawing();
        let al = add_alignment(&mut doc, "AL-R", &[Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0)], None).unwrap();
        let style = crate::classes::civil::add_point_label_style(&mut doc, "GROUP LABEL", |_| {}).unwrap();
        // A geometry point group with an increment (or a station group
        // with kinds) does not match the kind's tail.
        let mut group = align2_geompoint();
        group.increment = Some(10.0);
        assert!(add_alignment_label_group(&mut doc, al, style, None, &group).is_err());
        let mut group = align2_station();
        group.point_kinds = vec![1, 2];
        assert!(add_alignment_label_group(&mut doc, al, style, None, &group).is_err());
        // A range wider than 3 bits and an unknown layer refuse too.
        let mut group = align2_station();
        group.range = (8, 0.0, 0.0);
        assert!(add_alignment_label_group(&mut doc, al, style, None, &group).is_err());
        let mut group = align2_station();
        group.range = (7, 0.0, 0.0);
        assert!(add_alignment_label_group(&mut doc, al, style, Some("NO-SUCH"), &group).is_err());
        // An empty group refuses.
        let mut group = align2_station();
        group.labels.clear();
        assert!(add_alignment_label_group(&mut doc, al, style, None, &group).is_err());
    }
}
