//! The DXF CLASSES section counts the records actually written.
//!
//! AutoCAD drops every WIPEOUT of a DXF whose WIPEOUT class says
//! `91 0`, without an error. The writer used to copy the stored count,
//! which is 0 for a new drawing and stale after records are added or erased.

use acadrust::entities::{EntityType, Wipeout};
use acadrust::types::{DxfVersion, Handle, Vector3};
use acadrust::{CadDocument, DxfReader, DxfWriter};
use std::io::Cursor;

fn write(doc: &CadDocument) -> String {
    let bytes = DxfWriter::new(doc)
        .with_version(DxfVersion::AC1032)
        .write_to_vec()
        .expect("write dxf");
    String::from_utf8(bytes).expect("ascii dxf")
}

fn read(text: &str) -> CadDocument {
    DxfReader::from_reader(Cursor::new(text.as_bytes().to_vec()))
        .expect("open dxf")
        .read()
        .expect("parse dxf")
}

/// Line index of the `91` value of the CLASS record whose DXF name is `name`.
fn class_count_line(lines: &[&str], name: &str) -> usize {
    let mut i = 0;
    while i + 3 < lines.len() {
        if lines[i].trim() == "0" && lines[i + 1].trim() == "CLASS" {
            let is_name = lines[i + 2].trim() == "1" && lines[i + 3].trim() == name;
            let mut j = i + 2;
            while j + 1 < lines.len() && lines[j].trim() != "0" {
                if is_name && lines[j].trim() == "91" {
                    return j + 1;
                }
                j += 2;
            }
            i = j;
        } else {
            i += 2;
        }
    }
    panic!("no CLASS {name} with a 91 count");
}

fn class_count(text: &str, name: &str) -> i32 {
    let lines: Vec<&str> = text.lines().collect();
    lines[class_count_line(&lines, name)]
        .trim()
        .parse()
        .unwrap()
}

/// `text` with the 91 count of CLASS `name` replaced by `count`.
fn with_class_count(text: &str, name: &str, count: i32) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let at = class_count_line(&text.lines().collect::<Vec<_>>(), name);
    lines[at] = count.to_string();
    lines.join("\r\n") + "\r\n"
}

fn with_wipeouts(n: usize) -> (CadDocument, Vec<Handle>) {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let handles = (0..n)
        .map(|k| {
            let wipeout = Wipeout::rectangular(Vector3::new(20.0 * k as f64, 0.0, 0.0), 10.0, 10.0);
            doc.add_entity(EntityType::Wipeout(wipeout)).unwrap()
        })
        .collect();
    (doc, handles)
}

fn wipeouts(doc: &CadDocument) -> usize {
    doc.entities()
        .filter(|e| matches!(e, EntityType::Wipeout(_)))
        .count()
}

#[test]
fn new_drawing_counts_written_wipeouts() {
    let (doc, _) = with_wipeouts(2);
    assert_eq!(class_count(&write(&doc), "WIPEOUT"), 2);
}

#[test]
fn zero_count_read_from_dxf_is_recounted() {
    let (doc, _) = with_wipeouts(3);
    // The file a buggy writer produced: 3 WIPEOUTs, class count 0.
    let read_back = read(&with_class_count(&write(&doc), "WIPEOUT", 0));
    assert_eq!(wipeouts(&read_back), 3);
    assert_eq!(
        read_back
            .classes
            .get_by_name("WIPEOUT")
            .unwrap()
            .instance_count,
        0,
        "fixture must keep the file's count"
    );
    assert_eq!(class_count(&write(&read_back), "WIPEOUT"), 3);
}

#[test]
fn erased_record_lowers_the_count() {
    let (doc, _) = with_wipeouts(3);
    // A file whose count was right when it was saved.
    let mut read_back = read(&with_class_count(&write(&doc), "WIPEOUT", 3));
    assert_eq!(
        read_back
            .classes
            .get_by_name("WIPEOUT")
            .unwrap()
            .instance_count,
        3,
        "fixture must keep the file's count"
    );
    let first = read_back
        .entities()
        .find_map(|e| match e {
            EntityType::Wipeout(w) => Some(w.common.handle),
            _ => None,
        })
        .unwrap();
    read_back.remove_entity(first).expect("erase one wipeout");
    assert_eq!(wipeouts(&read_back), 2);
    assert_eq!(class_count(&write(&read_back), "WIPEOUT"), 2);
}
