//! A DIMENSION's flip-arrow flags survive a DXF write and read.
//!
//! AcDbDimension carries `280` (class version, R2010+) and `73 / 74 / 75`
//! (an always-0 flag, flip arrow 1, flip arrow 2; R2007+). The DXF writer
//! wrote none of them and the reader ignored 74/75, so flipped arrows came
//! back unflipped (the DWG path keeps them).

use acadrust::entities::{Dimension, DimensionAligned, EntityType};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};
use std::io::Cursor;

fn document() -> (CadDocument, acadrust::Handle) {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut dim = DimensionAligned::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 0.0, 0.0));
    dim.base.flip_arrow1 = true;
    dim.base.flip_arrow2 = false;
    let handle = doc
        .add_entity(EntityType::Dimension(Box::new(Dimension::Aligned(dim))))
        .unwrap();
    (doc, handle)
}

#[test]
fn dimension_flip_arrows_survive_dxf() {
    let (doc, handle) = document();
    let bytes = DxfWriter::new(&doc).write_to_vec().expect("write dxf");
    let back = DxfReader::from_reader(Cursor::new(bytes))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    match back.get_entity(handle) {
        Some(EntityType::Dimension(d)) => {
            let base = d.base();
            assert!(base.flip_arrow1, "flip arrow 1 lost");
            assert!(!base.flip_arrow2);
        }
        other => panic!("dimension lost: {other:?}"),
    }
}

#[test]
fn dimension_writes_autocads_version_and_arrow_groups() {
    let (doc, handle) = document();
    let text = String::from_utf8(DxfWriter::new(&doc).write_to_vec().unwrap()).unwrap();
    let lines: Vec<&str> = text.lines().map(str::trim).collect();
    let hex = format!("{:X}", handle.value());
    let start = (0..lines.len() - 3)
        .find(|&i| lines[i] == "0" && lines[i + 1] == "DIMENSION" && lines[i + 3] == hex)
        .expect("DIMENSION record");
    let pairs: Vec<(&str, &str)> = (start + 2..lines.len() - 1)
        .step_by(2)
        .map(|i| (lines[i], lines[i + 1]))
        .take_while(|(code, _)| *code != "0")
        .collect();
    let at = |p: (&str, &str)| pairs.iter().position(|q| *q == p);
    let marker = at(("100", "AcDbDimension")).expect("AcDbDimension");
    assert_eq!(pairs[marker + 1], ("280", "0"), "{pairs:?}");
    let measurement = pairs.iter().position(|q| q.0 == "42").expect("42");
    assert_eq!(
        &pairs[measurement + 1..measurement + 4],
        &[("73", "0"), ("74", "1"), ("75", "0")],
        "{pairs:?}"
    );
}
