//! An MTEXT with columns keeps its rotation through DXF.
//!
//! Group 50 is the rotation in the entity, and ACadSharp and ezdxf both keep the
//! last one they see. Writing the height of each column as group 50 too makes a
//! zero rotation (never written) come back as the first column height. The
//! columns belong where AutoCAD keeps them: an embedded object in R2018, ACAD
//! extended data from R2007 to R2013, nowhere below. The embedded object is
//! already written; the other two are what these tests are about.

use std::io::Cursor;

use acadrust::entities::{EntityType, MText};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};

fn document(version: DxfVersion, rotation: f64) -> CadDocument {
    let mut m = MText::with_value("two\nlines", Vector3::new(1.0, 2.0, 0.0));
    m.rotation = rotation;
    m.column_data.column_type = 2;
    m.column_data.column_count = 1;
    m.column_data.auto_height = false;
    m.column_data.width = 15.0;
    m.column_data.gutter = 110.0;
    m.column_data.heights = vec![-2.9];
    let mut doc = CadDocument::with_version(version);
    doc.add_entity(EntityType::MText(Box::new(m))).unwrap();
    doc
}

/// The (code, value) pairs of the first MTEXT, from its `0 MTEXT` to the next group 0.
fn mtext_pairs(doc: &CadDocument) -> Vec<(i32, String)> {
    let text = String::from_utf8(DxfWriter::new(doc).write_to_vec().unwrap()).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    let mut pairs = Vec::new();
    let mut inside = false;
    for c in lines.chunks(2).filter(|c| c.len() == 2) {
        let code: i32 = c[0].trim().parse().unwrap();
        let value = c[1].trim().to_string();
        if code == 0 && value == "MTEXT" {
            inside = true;
        } else if code == 0 && inside {
            break;
        } else if inside {
            pairs.push((code, value));
        }
    }
    pairs
}

fn count(pairs: &[(i32, String)], code: i32) -> usize {
    pairs.iter().filter(|(c, _)| *c == code).count()
}

fn read_back(doc: &CadDocument) -> MText {
    let bytes = DxfWriter::new(doc).write_to_vec().unwrap();
    let doc = DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap();
    let found = doc.entities().find_map(|e| match e {
        EntityType::MText(m) => Some(m.clone()),
        _ => None,
    });
    *found.expect("the MTEXT survived")
}

#[test]
fn r2007_to_r2013_columns_are_acad_extended_data() {
    let p = mtext_pairs(&document(DxfVersion::AC1027, 0.0));
    assert_eq!(count(&p, 50), 0);
    assert!(p.contains(&(1001, "ACAD".to_string())));
    assert!(p.contains(&(1000, "ACAD_MTEXT_COLUMN_INFO_BEGIN".to_string())));
    assert!(p.contains(&(1000, "ACAD_MTEXT_COLUMN_INFO_END".to_string())));
    assert!(p
        .iter()
        .any(|(c, v)| *c == 1040 && v.parse::<f64>().unwrap() == -2.9));
    assert_eq!(count(&p, 101), 0);
}

#[test]
fn below_r2007_there_is_no_column_data() {
    let p = mtext_pairs(&document(DxfVersion::AC1015, 0.0));
    for code in [50, 75, 76, 78, 79, 101, 1001] {
        assert_eq!(count(&p, code), 0, "group {code}: {p:?}");
    }
}

#[test]
fn rotation_and_columns_survive_an_r2013_round_trip() {
    for rotation in [0.0, 0.5] {
        let m = read_back(&document(DxfVersion::AC1027, rotation));
        assert!(
            (m.rotation - rotation).abs() < 1e-9,
            "rotation {rotation} came back as {}",
            m.rotation
        );
        assert_eq!(m.column_data.column_type, 2);
        assert_eq!(m.column_data.column_count, 1);
        assert_eq!(m.column_data.width, 15.0);
        assert_eq!(m.column_data.gutter, 110.0);
        assert_eq!(m.column_data.heights, vec![-2.9]);
    }
}
