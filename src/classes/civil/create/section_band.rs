//! Section-view data band label groups, written by their fields.
//!
//! The record is the one [`section_bands.rs`](super::super::section_bands)
//! reads (there for the grammar and the corpus's form): the label
//! grammar's class header with the group's 16 residual bytes, the labels
//! with their keys, run words and key ids, the standard settings, the
//! band range, the band number, closed by `B, BS 0x04BE`. The record
//! names no handle; the section view is the entity's references:
//!
//! ```text
//! references: soft 0, soft 0, the section view (soft), hard 0, one hard
//!   pointer to the view per label
//! texts: the class text, three empty texts
//! ```
//!
//! Groups the reader refuses are not writable: the typed fields cannot
//! hold a settings label set, nested labels, overrides or non-numeric
//! key values, and the writer leaves such a band as a gap.

use super::support::*;
use crate::classes::civil::{BandKeyValue, SectionDataBandGroup};
use crate::entities::EntityCommon;
use crate::error::DxfError;
use crate::types::Handle;
use crate::CadDocument;

/// Writes a section data band's label group for `view` (the
/// `AECC_GRAPH_SECTION_VIEW` whose band the group labels), on `layer`
/// (the drawing's default when `None`).
///
/// Refuses an unknown layer, a handle that is not a section view, an
/// empty group and a range wider than the 3 recorded flag bits.
pub fn add_section_data_band_label_group(
    doc: &mut CadDocument,
    view: Handle,
    layer: Option<&str>,
    group: &SectionDataBandGroup,
) -> Result<Handle, DxfError> {
    let is_view = arch_of(doc, view).is_some_and(|name| name == "AECC_GRAPH_SECTION_VIEW");
    if !is_view {
        return Err(invalid(
            "the label group's view is not an AECC_GRAPH_SECTION_VIEW",
        ));
    }
    let layer = layer.unwrap_or("0");
    let layer_handle = doc
        .layers
        .get(layer)
        .map(|l| l.handle)
        .ok_or_else(|| invalid("layer does not exist"))?;
    if group.labels.is_empty() || group.range.0 > 7 {
        return Err(invalid("section data band group out of the recorded form"));
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
    let mut refs = vec![
        soft_pointer(0),
        soft_pointer(0),
        soft_pointer(view.value()),
        hard_pointer(0),
    ];
    for _ in &group.labels {
        refs.push(hard_pointer(view.value()));
    }
    let strings = vec![
        "AeccImpSectionDataBandLabeling".to_string(),
        String::new(),
        String::new(),
        String::new(),
    ];
    put_entity(
        doc,
        "AECC_SECTIONDATABAND_LABELGROUP",
        common,
        |w| {
            header(w, &group.guid);
            w.write_bit_long(group.labels.len() as i32);
            for label in &group.labels {
                w.write_bit_long(label.keys.len() as i32);
                for (k, v) in &label.keys {
                    w.write_bit_long(*k as i32);
                    match v {
                        BandKeyValue::Long(v) => w.write_bit_long(*v),
                        BandKeyValue::Double(v) => w.write_bit_double(*v),
                        BandKeyValue::Byte(v) => w.write_byte(*v),
                    }
                }
                // The label's run: 2 bits, the property-set byte, the
                // label's run word, then its key ids.
                w.write_2bits(0);
                w.write_byte(1);
                w.write_bit_long(label.run as i32);
                w.write_bit_long(label.ids.len() as i32);
                for id in &label.ids {
                    w.write_bit_long(*id as i32);
                }
                // No nested labels or overrides: the typed fields hold
                // neither.
                w.write_bit_long(0);
                w.write_bit_long(0);
            }
            // The settings: no keys of its own, the two words, the anchor
            // value and weeding, the stagger, the stagger line heights.
            w.write_bit_long(0);
            w.write_bit_long(group.words.0 as i32);
            w.write_bit_long(group.words.1 as i32);
            w.write_bit_double(group.doubles[0]);
            w.write_bit_double(group.doubles[1]);
            w.write_bit_long(group.stagger as i32);
            w.write_bit_double(group.doubles[2]);
            w.write_bit_double(group.doubles[3]);
            // The range: 3 flag bits, start offset, end offset.
            bits(w, &format!("{:03b}", group.range.0));
            w.write_bit_double(group.range.1);
            w.write_bit_double(group.range.2);
            // The band number, then the close.
            w.write_bit_long(group.band);
            w.write_bit(false);
            w.write_bit_short(0x04BE);
        },
        Body { refs, strings },
    )?;
    Ok(handle)
}

/// The class header every label record opens with (the samples' bit run:
/// `BL 80, 0, 80, 80, 0, 0x10025F04`, the 6-bit class tag `0b010010`,
/// the group's 16 residual bytes, `BL 80, 80, 0x04BE0000, 0xFFFFFF`,
/// 3 bits).
fn header(
    w: &mut crate::io::dwg::dwg_stream_writers::merged_writer::DwgMergedWriter,
    guid: &[u8; 16],
) {
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
    use crate::classes::civil::{
        add_alignment, add_sample_line, add_sample_line_group, add_section_view, ensure_civil,
    };
    use crate::entities::{EntityType, ExtendedEntityData};
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::{DxfVersion, Vector2};

    fn drawing() -> CadDocument {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        ensure_civil(&mut doc).unwrap();
        doc
    }

    fn roundtrip(doc: &CadDocument, tag: &str) -> CadDocument {
        let path =
            std::env::temp_dir().join(format!("rt_secband_{tag}_{}.dwg", std::process::id()));
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

    /// The corpus's group: the band's edges, the offset-0 label, the
    /// offsets from -20, the standard settings, the automatic range, the
    /// band number.
    fn group(offsets: &[f64]) -> SectionDataBandGroup {
        let edge = |mark: u32| crate::classes::civil::SectionDataBandLabel {
            keys: vec![
                (mark, BandKeyValue::Byte(1)),
                (0x0101_5013, BandKeyValue::Long(1)),
            ],
            run: 0xA001,
            ids: vec![0x0101_5013, 0x0202_C012, 0x0302_C108, 0x0302_C109],
        };
        let offset = |o: f64| crate::classes::civil::SectionDataBandLabel {
            keys: vec![
                (0x0202_C012, BandKeyValue::Double(o)),
                (0x0101_5013, BandKeyValue::Long(1)),
            ],
            run: 0xA001,
            ids: vec![0x0101_5013, 0x0202_C012, 0x0302_C108, 0x0302_C109],
        };
        let mut labels = vec![
            edge(0x0302_C108),
            crate::classes::civil::SectionDataBandLabel {
                keys: vec![(0x0101_5013, BandKeyValue::Long(1))],
                run: 0xA003,
                ids: vec![0x0101_5013],
            },
        ];
        labels.extend(offsets.iter().map(|o| offset(*o)));
        labels.push(edge(0x0302_C109));
        SectionDataBandGroup {
            labels,
            words: (69, 1),
            doubles: [0.0, 0.0, 0.0, 1.0 / 6.0],
            stagger: 0,
            range: (7, 0.0, 0.0),
            band: 1,
            guid: [0; 16],
        }
    }

    /// A drawing with one section view of an unsampled sample line, the
    /// way rt_infra_sections builds one.
    fn with_view(tag: &str) -> (CadDocument, Handle) {
        let mut doc = drawing();
        let al = add_alignment(
            &mut doc,
            &format!("AL-{tag}"),
            &[Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0)],
            None,
        )
        .unwrap();
        let g = add_sample_line_group(&mut doc, al, &format!("SLG-{tag}"), None).unwrap();
        let line =
            add_sample_line(&mut doc, g, &format!("SL-{tag}"), 20.0, 10.0, 10.0, None).unwrap();
        let view = add_section_view(
            &mut doc,
            line,
            &format!("SV-{tag}"),
            Vector2::new(500.0, 0.0),
            None,
        )
        .unwrap();
        (doc, view)
    }

    #[test]
    fn the_corpus_form_round_trips_by_its_fields() {
        let (mut doc, view) = with_view("RT");
        let group = group(&[-10.0, 10.0, 20.0]);
        let h = add_section_data_band_label_group(&mut doc, view, None, &group).unwrap();
        let back = roundtrip(&doc, "fields");
        let e = entity(&back, h);
        assert_eq!(e.dxf_name, "AECC_SECTIONDATABAND_LABELGROUP");
        assert_eq!(e.cpp_class_name, "AeccDbSectionDataBandLabeling");
        let read = e.section_data_band_group().expect("the group reads back");
        assert_eq!(read, group);
        // References: soft 0, soft 0, the view (soft), hard 0, one hard
        // pointer to the view per label.
        let r = refs(&e);
        assert_eq!(r[0], (0, 4));
        assert_eq!(r[1], (0, 4));
        assert_eq!(r[2], (view.value(), 4));
        assert_eq!(r[3], (0, 5));
        assert_eq!(r.len(), 4 + group.labels.len());
        assert!(r[4..].iter().all(|r| *r == (view.value(), 5)));
        // The view survives beside the group.
        assert!(entity(&back, view).section_view_frame().is_some());
    }

    #[test]
    fn residual_bytes_and_a_named_layer_round_trip() {
        let (mut doc, view) = with_view("GUID");
        doc.layers.add(crate::tables::Layer::new("C-SECT-BAND")).unwrap();
        let mut group = group(&[-10.0, 10.0]);
        // The corpus's first record's residual bytes: UTF-16 fragments.
        group.guid = [
            0x72, 0x00, 0x5C, 0x00, 0x44, 0x00, 0x65, 0x00, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        let h =
            add_section_data_band_label_group(&mut doc, view, Some("C-SECT-BAND"), &group).unwrap();
        let back = roundtrip(&doc, "guid");
        let e = entity(&back, h);
        let read = e.section_data_band_group().unwrap();
        assert_eq!(read.guid, group.guid);
        let common = match back.get_entity(h) {
            Some(crate::entities::EntityType::Extended(e)) => e.common.clone(),
            _ => panic!("missing"),
        };
        assert_eq!(common.layer, "C-SECT-BAND");
    }

    #[test]
    fn the_writer_refuses_out_of_form_groups() {
        let (mut doc, view) = with_view("REFUSE");
        // An empty group and a range wider than 3 bits refuse.
        let mut empty = group(&[]);
        empty.labels.clear();
        assert!(add_section_data_band_label_group(&mut doc, view, None, &empty).is_err());
        let mut wide = group(&[10.0]);
        wide.range = (8, 0.0, 0.0);
        assert!(add_section_data_band_label_group(&mut doc, view, None, &wide).is_err());
        // A handle that is not a section view refuses; so does an
        // unknown layer.
        let other = add_alignment(
            &mut doc,
            "AL-NOTAVIEW",
            &[Vector2::new(0.0, 0.0), Vector2::new(50.0, 0.0)],
            None,
        )
        .unwrap();
        assert!(add_section_data_band_label_group(&mut doc, other, None, &wide).is_err());
        assert!(add_section_data_band_label_group(&mut doc, view, Some("NO-SUCH"), &wide).is_err());
    }

    /// The drawing's 19 groups, rebuilt from their decoded fields, carry
    /// the records' class data bit for bit: every field of the form is
    /// mapped, the residual bytes included. (The class body holds no
    /// handles, so the rebuilt groups' references — one view of this
    /// drawing instead of the records' own — do not enter the compare.)
    #[test]
    fn the_drawing_bands_rebuild_bit_for_bit() {
        use crate::classes::civil::bits::Bits;
        let Some(dir) = std::env::var_os("ACADRUST_CIVIL_TUTORIALS_DIR") else {
            return;
        };
        let path = std::path::Path::new(&dir).join("Sections-Project-Objects.dwg");
        if !path.exists() {
            return;
        }
        let oracle = DwgReader::from_file(&path).unwrap().read().unwrap();
        let mut groups = Vec::new();
        for e in oracle.entities() {
            if let EntityType::Extended(x) = e {
                if let ExtendedEntityData::Arch(a) = &x.data {
                    if a.dxf_name == "AECC_SECTIONDATABAND_LABELGROUP" {
                        let g = a.section_data_band_group().expect("an oracle band decodes");
                        let (raw, start, end) = a.main_stream().expect("an oracle band's record");
                        groups.push((g, raw.to_vec(), start, end));
                    }
                }
            }
        }
        assert_eq!(groups.len(), 19);
        let (mut doc, view) = with_view("ORACLE");
        for (g, _, _, _) in &groups {
            add_section_data_band_label_group(&mut doc, view, None, g).unwrap();
        }
        let back = roundtrip(&doc, "oracle");
        let mut written = Vec::new();
        for e in back.entities() {
            if let EntityType::Extended(x) = e {
                if let ExtendedEntityData::Arch(a) = &x.data {
                    if a.dxf_name == "AECC_SECTIONDATABAND_LABELGROUP" {
                        let (raw, start, end) = a.main_stream().expect("a written band's record");
                        written.push((raw.to_vec(), start, end));
                    }
                }
            }
        }
        assert_eq!(written.len(), 19);
        for ((_, oraw, ostart, oend), (raw, start, end)) in groups.iter().zip(&written) {
            let mut a = Bits::new(oraw, *ostart, *oend);
            let mut b = Bits::new(raw, *start, *end);
            assert_eq!(a.remaining(), b.remaining(), "bit length of a rebuilt band");
            let mut byte = 0;
            while a.remaining() >= 8 {
                let (x, y) = (a.raw_short_byte(), b.raw_short_byte());
                if x != y {
                    panic!("a rebuilt band's bits differ at byte {byte}: {x:?} vs {y:?}");
                }
                byte += 1;
            }
            // The records end on a partial byte (21345 bits); its bits
            // compare too.
            let last = a.remaining();
            assert_eq!(
                a.bits_value(last),
                b.bits_value(last),
                "a rebuilt band's final bits"
            );
        }
    }
}
