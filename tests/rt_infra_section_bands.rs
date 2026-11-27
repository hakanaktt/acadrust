//! The section view data band label groups of the corpus's
//! Sections-Project-Objects drawing, read by their fields.
//!
//! All 19 of the drawing's `AECC_SECTIONDATABAND_LABELGROUP` records
//! store one form: the band's two edge marks, its offsets from −290 to
//! 290 every 10 (0 unlabeled), every label naming band 1 and listing the
//! group's four keys, the standard settings, an automatic range and the
//! band number. The reader refuses a record that stores any other form;
//! the drawing carries none.

use acadrust::classes::civil::SectionDataBandGroup;
use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::io::dwg::DwgReader;

fn oracle() -> Option<acadrust::CadDocument> {
    let dir = std::env::var_os("ACADRUST_CIVIL_TUTORIALS_DIR")?;
    let path = std::path::Path::new(&dir).join("Sections-Project-Objects.dwg");
    if !path.exists() {
        return None;
    }
    Some(DwgReader::from_file(&path).unwrap().read().unwrap())
}

fn groups(doc: &acadrust::CadDocument) -> Vec<SectionDataBandGroup> {
    doc.entities()
        .filter_map(|e| match e {
            EntityType::Extended(x) => match &x.data {
                ExtendedEntityData::Arch(a) => a.section_data_band_group(),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[test]
fn the_drawing_data_bands_read_by_their_fields() {
    let Some(doc) = oracle() else { return };
    let groups = groups(&doc);
    assert_eq!(groups.len(), 19);
    let offsets: Vec<f64> = (-29..=-1).chain(1..=29).map(|i| i as f64 * 10.0).collect();
    for g in &groups {
        assert_eq!(g.labels.len(), 61);
        // The band's edges: the start mark, then the offsets, then the
        // end mark; the offsets step 10 over the full width, 0 unlabeled.
        assert_eq!(g.labels[0].keys[0], (0x0302_C108, acadrust::classes::civil::BandKeyValue::Byte(1)));
        assert_eq!(g.labels[60].keys[0], (0x0302_C109, acadrust::classes::civil::BandKeyValue::Byte(1)));
        assert_eq!(g.offsets(), offsets);
        // The offset-0 label between the negative and positive offsets:
        // the band number alone, run 0xA003, its one key id.
        assert_eq!(g.labels[30].keys, vec![(0x0101_5013, acadrust::classes::civil::BandKeyValue::Long(1))]);
        assert_eq!(g.labels[30].run, 0xA003);
        assert_eq!(g.labels[30].ids, vec![0x0101_5013]);
        assert!(g.labels.iter().enumerate().all(|(i, l)| i == 30 || (l.run == 0xA001 && l.ids == [0x0101_5013, 0x0202_C012, 0x0302_C108, 0x0302_C109])));
        // Every label names the view's band 1 and lists its keys.
        assert_eq!(g.band, 1);
        assert_eq!(g.band_numbers(), vec![1; 61]);
        // The standard settings and an automatic range.
        assert_eq!(g.words, (69, 1));
        assert_eq!(g.doubles, [0.0, 0.0, 0.0, 1.0 / 6.0]);
        assert_eq!(g.stagger, 0);
        assert_eq!(g.range, (7, 0.0, 0.0));
    }
}

#[test]
fn other_label_records_do_not_read_as_data_bands() {
    let Some(doc) = oracle() else { return };
    // The drawing's sample line labels and alignment profiles are label
    // records of other families: none reads as a section data band.
    let sample_line_labels = doc
        .entities()
        .filter(|e| matches!(e, EntityType::Extended(x) if matches!(&x.data, ExtendedEntityData::Arch(a) if a.dxf_name == "AECC_SAMPLELINE_LABELING")))
        .count();
    assert_eq!(sample_line_labels, 2);
    let wrong = doc
        .entities()
        .filter_map(|e| match e {
            EntityType::Extended(x) => match &x.data {
                ExtendedEntityData::Arch(a) if a.dxf_name != "AECC_SECTIONDATABAND_LABELGROUP" => a.section_data_band_group(),
                _ => None,
            },
            _ => None,
        })
        .count();
    assert_eq!(wrong, 0);
}
