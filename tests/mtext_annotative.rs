//! A plain MTEXT must not come back flagged annotative after a DXF round-trip.
//!
//! DXF carries MTEXT annotativeness via the annotation context (an
//! ObjectContextData / XDATA association), not an entity-level flag, so the
//! reader never sets `is_annotative` on the entity. A freshly-created,
//! non-annotative MTEXT must therefore read back with `is_annotative == false`
//! — regressing the old struct default of `true` would mark every imported DXF
//! MTEXT annotative and over-scale it in annotation-scaled viewports.

use std::io::Cursor;

use acadrust::entities::{EntityType, MText, MultiLeader};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};

fn dxf_roundtrip(doc: &CadDocument) -> CadDocument {
    let bytes = DxfWriter::new(doc)
        .write_to_vec()
        .expect("DXF write failed");
    DxfReader::from_reader(Cursor::new(bytes))
        .expect("DXF reader init failed")
        .read()
        .expect("DXF read failed")
}

#[test]
fn plain_mtext_not_annotative_after_dxf_roundtrip() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1027);
    doc.add_entity(EntityType::MText(MText::with_value(
        "Plain",
        Vector3::new(1.0, 2.0, 0.0),
    )))
    .unwrap();

    let rt = dxf_roundtrip(&doc);
    let mtext = rt
        .entities()
        .find_map(|e| {
            if let EntityType::MText(t) = e {
                Some(t)
            } else {
                None
            }
        })
        .expect("MTEXT survived DXF round-trip");

    assert!(
        !mtext.is_annotative,
        "a plain MTEXT must read back non-annotative from DXF"
    );
}

#[test]
fn multileader_annotation_scale_survives_dxf_roundtrip() {
    // A plain (default) MULTILEADER must stay non-annotative, and an annotative
    // one must round-trip via DXF group code 293 — the reader previously ignored
    // 293, so every imported MULTILEADER inherited the old `true` default.
    for enabled in [false, true] {
        let mut doc = CadDocument::with_version(DxfVersion::AC1027);
        let mut ml = MultiLeader::with_text(
            "Label",
            Vector3::new(20.0, 20.0, 0.0),
            vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 10.0, 0.0)],
        );
        ml.enable_annotation_scale = enabled;
        doc.add_entity(EntityType::MultiLeader(ml)).unwrap();

        let rt = dxf_roundtrip(&doc);
        let got = rt
            .entities()
            .find_map(|e| {
                if let EntityType::MultiLeader(m) = e {
                    Some(m.enable_annotation_scale)
                } else {
                    None
                }
            })
            .expect("MULTILEADER survived DXF round-trip");
        assert_eq!(
            got, enabled,
            "MULTILEADER enable_annotation_scale should round-trip {enabled} via DXF"
        );
    }
}
