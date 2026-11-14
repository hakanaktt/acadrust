//! Table records with an empty name in a DWG save (#370).
//!
//! An empty name is legal in DWG only on a shape-file STYLE. The unnamed
//! ordinary STYLE that linetype shape segments draw from is that shape file
//! stored without its flag (#311), so the writer writes it with the flag. Any
//! other unnamed record is written as it is, under its own handle.

use std::io::Cursor;

use acadrust::tables::linetype::LineTypeComplexContent;
use acadrust::tables::{
    AppId, BlockRecord, DimStyle, Layer, LineType, LineTypeComplexData, LineTypeElement,
    TableEntry, TextStyle,
};
use acadrust::types::{DxfVersion, Handle};
use acadrust::{CadDocument, DwgReader, DwgWriter};

const VERSIONS: [DxfVersion; 6] = [
    DxfVersion::AC1015,
    DxfVersion::AC1018,
    DxfVersion::AC1021,
    DxfVersion::AC1024,
    DxfVersion::AC1027,
    DxfVersion::AC1032,
];

fn read_back(document: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(document).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

/// An ordinary STYLE named `name` and a linetype whose one segment draws
/// `content` from it.
fn document_with_segment_style(
    version: DxfVersion,
    name: &str,
    content: LineTypeComplexContent,
) -> (CadDocument, Handle) {
    let mut document = CadDocument::with_version(version);
    let mut style = TextStyle::new(name);
    style.font_file = "ltypeshp.shx".into();
    style.is_shape_file = false;
    style.handle = document.allocate_handle();
    let handle = style.handle;
    document.text_styles.add_allow_duplicate(style);

    let mut linetype = LineType::new("ZIGZAG");
    linetype.handle = document.allocate_handle();
    let mut dash = LineTypeElement::dash(0.0);
    dash.complex = Some(LineTypeComplexData {
        content,
        style_handle: handle,
        ..LineTypeComplexData::default()
    });
    linetype.elements.push(dash);
    linetype.elements.push(LineTypeElement::space(1.0));
    linetype.pattern_length = 1.0;
    document.line_types.add(linetype).unwrap();
    (document, handle)
}

fn style_by_handle(document: &CadDocument, handle: Handle) -> &TextStyle {
    document
        .text_styles
        .iter()
        .find(|style| style.handle == handle)
        .unwrap_or_else(|| panic!("STYLE {:#X} is missing", handle.value()))
}

fn assert_shape_style_written_as_shape_file(version: DxfVersion) {
    let (document, handle) = document_with_segment_style(
        version,
        "",
        LineTypeComplexContent::Shape { shape_number: 131 },
    );
    let loaded = read_back(&document);
    let style = style_by_handle(&loaded, handle);
    assert!(
        style.is_shape_file,
        "{version:?}: the unnamed STYLE {:#X} behind shape segments is written without the shape-file flag",
        handle.value()
    );
    assert_eq!(style.name, "", "{version:?}");
    assert_eq!(style.font_file, "ltypeshp.shx", "{version:?}");
    let linetype = loaded.line_types.get("ZIGZAG").unwrap();
    let complex = linetype.elements[0].complex.as_ref().unwrap();
    assert_eq!(complex.style_handle, handle, "{version:?}");
    // The flag is a choice of the writer: the document keeps the record as it was.
    assert!(!style_by_handle(&document, handle).is_shape_file);
}

#[test]
fn unnamed_style_behind_shape_segments_is_a_shape_file_r2000() {
    assert_shape_style_written_as_shape_file(DxfVersion::AC1015);
}

#[test]
fn unnamed_style_behind_shape_segments_is_a_shape_file_r2004() {
    assert_shape_style_written_as_shape_file(DxfVersion::AC1018);
}

#[test]
fn unnamed_style_behind_shape_segments_is_a_shape_file_r2007() {
    assert_shape_style_written_as_shape_file(DxfVersion::AC1021);
}

#[test]
fn unnamed_style_behind_shape_segments_is_a_shape_file_r2010() {
    assert_shape_style_written_as_shape_file(DxfVersion::AC1024);
}

#[test]
fn unnamed_style_behind_shape_segments_is_a_shape_file_r2013() {
    assert_shape_style_written_as_shape_file(DxfVersion::AC1027);
}

#[test]
fn unnamed_style_behind_shape_segments_is_a_shape_file_r2018() {
    assert_shape_style_written_as_shape_file(DxfVersion::AC1032);
}

/// Only an unnamed STYLE that shape segments use takes the flag: one that
/// text segments use, and a named one behind shape segments, keep theirs.
#[test]
fn other_segment_styles_keep_their_flags() {
    for version in VERSIONS {
        let cases = [
            ("", LineTypeComplexContent::Text { text: "GAS".into() }),
            (
                "SHAPES",
                LineTypeComplexContent::Shape { shape_number: 131 },
            ),
        ];
        for (name, content) in cases {
            let (document, handle) = document_with_segment_style(version, name, content);
            let loaded = read_back(&document);
            let style = style_by_handle(&loaded, handle);
            assert!(!style.is_shape_file, "{version:?}: STYLE {name:?}");
            assert_eq!(style.name, name, "{version:?}");
        }
    }
}

/// One unnamed record in each of LAYER, LTYPE, STYLE (no segment uses it),
/// DIMSTYLE, BLOCK_RECORD and APPID, with the table and the handle of each.
fn document_with_unnamed_records(
    version: DxfVersion,
) -> (CadDocument, Vec<(&'static str, Handle)>) {
    let mut document = CadDocument::with_version(version);
    let mut records = Vec::new();

    let mut layer = Layer::new("");
    layer.handle = document.allocate_handle();
    records.push(("LAYER", layer.handle));
    document.layers.add_allow_duplicate(layer);

    let mut linetype = LineType::new("");
    linetype.handle = document.allocate_handle();
    records.push(("LTYPE", linetype.handle));
    document.line_types.add_allow_duplicate(linetype);

    let mut style = TextStyle::new("");
    style.font_file = "txt.shx".into();
    style.handle = document.allocate_handle();
    records.push(("STYLE", style.handle));
    document.text_styles.add_allow_duplicate(style);

    let mut dimstyle = DimStyle::new("");
    dimstyle.handle = document.allocate_handle();
    records.push(("DIMSTYLE", dimstyle.handle));
    document.dim_styles.add_allow_duplicate(dimstyle);

    let mut block = BlockRecord::new("");
    block.handle = document.allocate_handle();
    records.push(("BLOCK_RECORD", block.handle));
    document.block_records.add_allow_duplicate(block);

    let mut appid = AppId::new("");
    appid.handle = document.allocate_handle();
    records.push(("APPID", appid.handle));
    document.app_ids.add_allow_duplicate(appid);

    (document, records)
}

fn name_by_handle(document: &CadDocument, table: &str, handle: Handle) -> Option<String> {
    fn find<'a, T: TableEntry + 'a>(
        mut entries: impl Iterator<Item = &'a T>,
        handle: Handle,
    ) -> Option<String> {
        entries
            .find(|entry| entry.handle() == handle)
            .map(|entry| entry.name().to_string())
    }
    match table {
        "LAYER" => find(document.layers.iter(), handle),
        "LTYPE" => find(document.line_types.iter(), handle),
        "STYLE" => find(document.text_styles.iter(), handle),
        "DIMSTYLE" => find(document.dim_styles.iter(), handle),
        "BLOCK_RECORD" => find(document.block_records.iter(), handle),
        "APPID" => find(document.app_ids.iter(), handle),
        _ => unreachable!("{table}"),
    }
}

/// The writer does not invent a name and does not drop the record.
#[test]
fn unnamed_records_are_written_unchanged() {
    for version in VERSIONS {
        let (document, records) = document_with_unnamed_records(version);
        let loaded = read_back(&document);
        for (table, handle) in records {
            assert_eq!(
                name_by_handle(&loaded, table, handle).as_deref(),
                Some(""),
                "{version:?}: {table} {:#X}",
                handle.value()
            );
        }
        let style = loaded
            .text_styles
            .iter()
            .find(|style| style.name.is_empty())
            .unwrap();
        assert!(!style.is_shape_file, "{version:?}");
    }
}
