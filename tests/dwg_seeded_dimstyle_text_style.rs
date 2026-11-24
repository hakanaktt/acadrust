//! The DWG reader seeds a Standard dimension style before it reads the file.
//! When the file has none of its own, that seeded style survives, and its text
//! style must be the file's Standard text style - not whatever record the file
//! keeps at the handle the seed was wired to (the DXF reader's fix for #64).

use std::io::Cursor;

use acadrust::tables::DimStyle;
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter};

/// A drawing as other CAD programs save it: the Standard text style at #11,
/// the ByBlock linetype at #14 and ISO-25 as its only dimension style.
fn iso_only_dwg() -> Vec<u8> {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.text_styles.get_mut("Standard").unwrap().handle = Handle::new(0x11);
    doc.line_types.get_mut("ByBlock").unwrap().handle = Handle::new(0x14);
    doc.dim_styles.remove("Standard").unwrap();
    let mut iso = DimStyle::new("ISO-25");
    iso.handle = doc.allocate_handle();
    iso.dimtxsty_handle = Handle::new(0x11);
    iso.dimtxsty = "Standard".to_string();
    doc.header.current_dimstyle_handle = iso.handle;
    doc.header.current_dimstyle_name = "ISO-25".to_string();
    doc.header.dim_text_style_handle = Handle::new(0x11);
    doc.dim_styles.add(iso).unwrap();
    DwgWriter::write_to_vec(&doc).expect("write dwg")
}

fn read(bytes: Vec<u8>) -> CadDocument {
    DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read dwg")
}

/// Every dimension style's DIMTXSTY names a text style of the document.
fn assert_dim_styles_name_text_styles(doc: &CadDocument) {
    let text_styles: Vec<Handle> = doc.text_styles.iter().map(|s| s.handle).collect();
    for style in doc.dim_styles.iter() {
        assert!(
            text_styles.contains(&style.dimtxsty_handle),
            "dimension style {} names {:?} as its text style, which is no text style \
             (ByBlock linetype = {:?})",
            style.name,
            style.dimtxsty_handle,
            doc.line_types.get("ByBlock").map(|l| l.handle),
        );
    }
}

#[test]
fn a_seeded_standard_dim_style_points_at_the_files_text_style() {
    let loaded = read(iso_only_dwg());
    assert!(
        loaded.dim_styles.get("Standard").is_some(),
        "the reader keeps its seeded Standard dimension style"
    );
    assert_dim_styles_name_text_styles(&loaded);
    assert_eq!(
        loaded.dim_styles.get("Standard").unwrap().dimtxsty_handle,
        loaded.text_styles.get("Standard").unwrap().handle,
        "the seeded style uses the file's Standard text style"
    );
}

/// What a consumer sees: the drawing read and written again carries a
/// Standard dimension style whose DIMTXSTY is a text style.
#[test]
fn a_dwg_written_again_names_a_text_style_in_every_dim_style() {
    let loaded = read(iso_only_dwg());
    let again = read(DwgWriter::write_to_vec(&loaded).expect("write dwg again"));
    assert_dim_styles_name_text_styles(&again);
    assert_eq!(
        again.dim_styles.get("ISO-25").unwrap().dimtxsty_handle,
        again.text_styles.get("Standard").unwrap().handle,
        "the file's own style keeps its text style"
    );
}

/// The repair only re-points what names no text style: a file's own Standard
/// dimension style keeps the text style it names, even one not called Standard.
#[test]
fn a_files_own_standard_dim_style_keeps_its_text_style() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut romans = acadrust::tables::TextStyle::new("ROMANS");
    romans.handle = doc.allocate_handle();
    let romans_handle = romans.handle;
    doc.text_styles.add(romans).unwrap();
    let standard = doc.dim_styles.get_mut("Standard").unwrap();
    standard.dimtxsty_handle = romans_handle;
    standard.dimtxsty = "ROMANS".to_string();
    let loaded = read(DwgWriter::write_to_vec(&doc).expect("write dwg"));
    assert_eq!(
        loaded.dim_styles.get("Standard").unwrap().dimtxsty_handle,
        romans_handle
    );
}
