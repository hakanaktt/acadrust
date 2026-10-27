//! Total decompression / allocation budget (issue #226).
//!
//! `DwgReadOptions::max_decompressed_bytes` bounds the decompressed bytes one
//! read allocates, so a crafted file cannot exhaust memory through section
//! declarations alone.

use acadrust::{DwgReadOptions, DwgReader};

const FIXTURE: &str = "tests/fixtures/issue177/annotative_2018.dwg";

#[test]
fn a_budget_below_the_declared_sections_rejects_the_read() {
    let error = DwgReader::from_file_with_options(
        FIXTURE,
        DwgReadOptions::default().with_max_decompressed_bytes(1),
    )
    .unwrap()
    .read()
    .expect_err("the read must fail when the budget cannot hold the file's sections");
    assert!(error.to_string().contains("budget"), "{error}");
}

#[test]
fn a_budget_large_enough_reads_the_drawing() {
    let document = DwgReader::from_file_with_options(
        FIXTURE,
        DwgReadOptions::default().with_max_decompressed_bytes(usize::MAX),
    )
    .unwrap()
    .read()
    .expect("an effectively unlimited budget must not reject a valid drawing");
    assert!(document.entities().next().is_some());
}
