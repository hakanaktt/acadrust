//! Issue #215 follow-up: a header's dimension text style and linetypes are
//! names in DXF and handles in a DWG. The readers already expose the names;
//! these tests keep them when the document is written again.

use std::io::Cursor;

use acadrust::tables::{LineType, TextStyle};
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, TableEntry};

/// A drawing that defines the styles the dimension header names, the way a
/// file that uses them does.
fn document(version: DxfVersion) -> CadDocument {
    let mut doc = CadDocument::with_version(version);

    let mut text = TextStyle::new("MyDimText");
    text.set_handle(doc.allocate_handle());
    doc.text_styles.add(text).unwrap();

    for name in ["BATTING", "TRACKS", "HIDDEN2"] {
        let mut linetype = LineType::new(name);
        linetype.set_handle(doc.allocate_handle());
        doc.line_types.add(linetype).unwrap();
    }
    doc
}

/// The dimension header variables as a DXF spells them: `$DIMTXSTY` names a
/// text style (group 7) and `$DIMLTYPE`/`$DIMLTEX1`/`$DIMLTEX2` name linetypes
/// (group 6).
fn dimension_header_variables() -> &'static str {
    "  9\n$DIMTXSTY\n  7\nMyDimText\n  9\n$DIMLTYPE\n  6\nBATTING\n\
  9\n$DIMLTEX1\n  6\nTRACKS\n  9\n$DIMLTEX2\n  6\nHIDDEN2\n"
}

/// A real DXF with a tabulated drawing and the four variables in its header.
///
/// The variables are appended after the writer's own header values so this
/// input does not depend on the writer under test: whatever it emits, the
/// last value of a variable is the one the reader keeps.
fn dxf_with_dimension_names(version: DxfVersion) -> Vec<u8> {
    let text = String::from_utf8(DxfWriter::new(&document(version)).write_to_vec().unwrap())
        .unwrap()
        .replace("\r\n", "\n");
    let header = text.find("2\nHEADER\n").expect("a header section") + "2\nHEADER\n".len();
    let end = text[header..]
        .find("\n  0\nENDSEC\n")
        .expect("the end of the header section")
        + header
        + 1;
    format!(
        "{}{}{}",
        &text[..end],
        dimension_header_variables(),
        &text[end..]
    )
    .into_bytes()
}

fn read_dxf(bytes: Vec<u8>) -> CadDocument {
    DxfReader::from_reader(Cursor::new(bytes))
        .expect("open dxf")
        .read()
        .expect("read dxf")
}

fn read_dwg(bytes: Vec<u8>) -> CadDocument {
    DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("read dwg")
}

fn assert_names(document: &CadDocument, version: DxfVersion, label: &str) {
    let h = &document.header;
    assert_eq!(
        h.dim_text_style_name, "MyDimText",
        "{version:?} {label} $DIMTXSTY"
    );
    // A DWG header has held the dimension linetype handles only since R2007.
    if label.contains("dwg") && version < DxfVersion::AC1021 {
        return;
    }
    assert_eq!(
        h.dim_linetype_name, "BATTING",
        "{version:?} {label} $DIMLTYPE"
    );
    assert_eq!(
        h.dim_linetype1_name, "TRACKS",
        "{version:?} {label} $DIMLTEX1"
    );
    assert_eq!(
        h.dim_linetype2_name, "HIDDEN2",
        "{version:?} {label} $DIMLTEX2"
    );
}

#[test]
fn a_dxf_read_keeps_the_names_when_written_again() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1032] {
        let source = read_dxf(dxf_with_dimension_names(version));
        assert_names(&source, version, "source");

        let written = DxfWriter::new(&source).write_to_vec().expect("write dxf");
        let text = String::from_utf8_lossy(&written);
        for variable in ["$DIMTXSTY", "$DIMLTYPE", "$DIMLTEX1", "$DIMLTEX2"] {
            assert!(
                text.contains(variable),
                "{version:?} written DXF lost {variable}"
            );
        }

        assert_names(&read_dxf(written), version, "dxf roundtrip");
    }
}

#[test]
fn a_dxf_read_keeps_the_names_through_a_dwg() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1021, DxfVersion::AC1032] {
        let source = read_dxf(dxf_with_dimension_names(version));
        let written = DwgWriter::write_to_vec(&source).expect("write dwg");
        assert_names(&read_dwg(written), version, "dwg roundtrip");
    }
}

#[test]
fn a_dwg_read_keeps_the_names_through_a_dxf() {
    let mut source = document(DxfVersion::AC1032);
    let style = source.text_styles.get("MyDimText").unwrap().handle;
    source.header.dim_text_style_handle = style;
    for (slot, name) in ["BATTING", "TRACKS", "HIDDEN2"].iter().enumerate() {
        let handle = source.line_types.get(name).unwrap().handle;
        match slot {
            0 => source.header.dim_linetype_handle = handle,
            1 => source.header.dim_linetype1_handle = handle,
            _ => source.header.dim_linetype2_handle = handle,
        }
    }

    let dwg = read_dwg(DwgWriter::write_to_vec(&source).expect("write dwg"));
    assert_names(&dwg, DxfVersion::AC1032, "dwg source");

    let dxf = DxfWriter::new(&dwg).write_to_vec().expect("write dxf");
    assert_names(&read_dxf(dxf), DxfVersion::AC1032, "dxf roundtrip from dwg");
}

#[test]
fn a_dwg_roundtrip_keeps_the_names() {
    let mut source = document(DxfVersion::AC1032);
    let style = source.text_styles.get("MyDimText").unwrap().handle;
    source.header.dim_text_style_handle = style;
    source.header.dim_linetype_handle = source.line_types.get("BATTING").unwrap().handle;
    source.header.dim_linetype1_handle = source.line_types.get("TRACKS").unwrap().handle;
    source.header.dim_linetype2_handle = source.line_types.get("HIDDEN2").unwrap().handle;

    let written = DwgWriter::write_to_vec(&source).expect("write dwg");
    assert_names(&read_dwg(written), DxfVersion::AC1032, "dwg roundtrip");
}

#[test]
fn a_name_only_header_resolves_to_the_named_entries_in_a_dwg() {
    for version in [DxfVersion::AC1015, DxfVersion::AC1021, DxfVersion::AC1032] {
        // A DXF header names the entries and carries no handles.
        let mut source = document(version);
        source.header.dim_text_style_name = "MyDimText".into();
        source.header.dim_linetype_name = "BATTING".into();
        source.header.dim_linetype1_name = "TRACKS".into();
        source.header.dim_linetype2_name = "HIDDEN2".into();
        source.header.dim_text_style_handle = acadrust::types::Handle::NULL;
        source.header.dim_linetype_handle = acadrust::types::Handle::NULL;
        source.header.dim_linetype1_handle = acadrust::types::Handle::NULL;
        source.header.dim_linetype2_handle = acadrust::types::Handle::NULL;

        let written = DwgWriter::write_to_vec(&source).expect("write dwg");
        assert_names(&read_dwg(written), version, "dwg from names");
    }
}

#[test]
fn a_new_drawing_writes_the_autocad_default_names() {
    let text = String::from_utf8(
        DxfWriter::new(&CadDocument::with_version(DxfVersion::AC1032))
            .write_to_vec()
            .expect("write dxf"),
    )
    .unwrap()
    .replace("\r\n", "\n");

    for (variable, code, name) in [
        ("$DIMTXSTY", "  7", "Standard"),
        ("$DIMLTYPE", "  6", "ByBlock"),
        ("$DIMLTEX1", "  6", "ByBlock"),
        ("$DIMLTEX2", "  6", "ByBlock"),
    ] {
        let expected = format!("  9\n{variable}\n{code}\n{name}\n");
        assert!(text.contains(&expected), "missing {variable} = {name}");
    }
}
