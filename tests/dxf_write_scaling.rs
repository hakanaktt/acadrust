//! Writing a DXF takes time in proportion to the drawing: four times the blocks and objects take well under sixteen
//! times as long. A table record that names no extension dictionary has its owner looked up in every object of the
//! document (`CadDocument::extension_dictionary_handle`), and the writer asks for every block record, so a drawing of
//! many blocks (every dimension has one) and many objects is quadratic to write.

use std::time::{Duration, Instant};

use acadrust::entities::{EntityType, Line};
use acadrust::objects::{Dictionary, ObjectType};
use acadrust::tables::BlockRecord;
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DxfWriter};

/// A drawing of `n` blocks of one line each, and `n` lines in model space, each with an extension dictionary.
fn drawing(n: usize) -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    for b in 0..n {
        let mut record = BlockRecord::new(format!("B{b}"));
        record.handle = doc.allocate_handle();
        record.block_entity_handle = doc.allocate_handle();
        record.block_end_handle = doc.allocate_handle();
        let owner = record.handle;
        doc.block_records.add(record).unwrap();
        let mut line = Line::from_points(Vector3::new(0.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0));
        line.common.owner_handle = owner;
        doc.add_entity(EntityType::Line(line)).unwrap();
    }
    for i in 0..n {
        let x = i as f64;
        let line = doc
            .add_entity(EntityType::Line(Line::from_points(
                Vector3::new(x, 0.0, 0.0),
                Vector3::new(x, 1.0, 0.0),
            )))
            .unwrap();
        let mut dictionary = Dictionary::new();
        dictionary.handle = doc.allocate_handle();
        dictionary.owner = line;
        dictionary.hard_owner = true;
        let handle = dictionary.handle;
        doc.objects.insert(handle, ObjectType::Dictionary(dictionary));
        doc.get_entity_mut(line).unwrap().common_mut().xdictionary_handle = Some(handle);
    }
    doc
}

fn fastest_write(doc: &CadDocument) -> (Duration, Vec<u8>) {
    let mut best: Option<(Duration, Vec<u8>)> = None;
    for _ in 0..3 {
        let t = Instant::now();
        let bytes = DxfWriter::new(doc).write_to_vec().unwrap();
        let e = t.elapsed();
        if best.as_ref().is_none_or(|(b, _)| e < *b) {
            best = Some((e, bytes));
        }
    }
    best.unwrap()
}

#[test]
fn writing_many_blocks_and_objects_to_dxf_scales_linearly() {
    const N: usize = 3_000;
    let (small, large) = (drawing(N), drawing(N * 4));
    let (a, _) = fastest_write(&small);
    let (b, bytes) = fastest_write(&large);
    let text = String::from_utf8_lossy(&bytes);
    assert_eq!(
        text.matches("{ACAD_XDICTIONARY").count(),
        N * 4,
        "every model space line keeps its extension dictionary"
    );
    // Linear is 4x, quadratic 16x; below a few milliseconds the ratio is noise.
    let ratio = b.as_secs_f64() / a.as_secs_f64().max(0.005);
    eprintln!("{N} blocks {a:?}, {} blocks {b:?}, ratio {ratio:.1}", N * 4);
    assert!(
        ratio < 9.0,
        "4x the blocks and objects took {ratio:.1}x as long to write ({a:?} → {b:?})"
    );
}
