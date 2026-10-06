//! Writing the same document repeatedly must produce identical bytes.

use acadrust::objects::{Dictionary, ObjectType};
use acadrust::{CadDocument, DwgWriter, DxfWriter};

// Exercise orphaned objects in both writers.
const ORPHANED_DICTIONARIES: usize = 32;

fn document_with_orphaned_objects() -> CadDocument {
    let mut document = CadDocument::new();

    for index in 0..ORPHANED_DICTIONARIES {
        let handle = document.allocate_handle();
        let mut dictionary = Dictionary::new();
        dictionary.handle = handle;
        dictionary.add_entry(format!("ENTRY_{index}"), handle);
        document
            .objects
            .insert(handle, ObjectType::Dictionary(dictionary));
    }

    document
}

fn assert_three_runs_are_byte_identical(label: &str, write: impl Fn() -> Vec<u8>) {
    let first = write();

    for run in 2..=3 {
        let again = write();
        assert_eq!(
            first.len(),
            again.len(),
            "{label}: run {run} has a different length"
        );

        if let Some(offset) = first.iter().zip(&again).position(|(a, b)| a != b) {
            panic!("{label}: run {run} differs at byte offset {offset}");
        }
    }
}

#[test]
fn dxf_output_is_byte_identical_across_runs() {
    assert_three_runs_are_byte_identical("DXF", || {
        let document = document_with_orphaned_objects();
        DxfWriter::new(&document).write_to_vec().expect("DXF write")
    });
}

#[test]
fn dwg_output_is_byte_identical_across_runs() {
    assert_three_runs_are_byte_identical("DWG", || {
        let document = document_with_orphaned_objects();
        DwgWriter::write_to_vec(&document).expect("DWG write")
    });
}
