//! DXF round-trip of the current table / multileader style header variables
//! ($CTABLESTYLE / $CMLEADERSTYLE), plus the existing text/dim/mline ones.

use acadrust::{CadDocument, DxfReader, DxfWriter};

#[test]
fn dxf_roundtrips_current_style_header_vars() {
    let mut doc = CadDocument::new();
    doc.header.current_text_style_name = "MyText".to_string();
    doc.header.current_dimstyle_name = "MyDim".to_string();
    doc.header.multiline_style = "MyMline".to_string();
    doc.header.current_table_style_name = "MyTable".to_string();
    doc.header.current_mleader_style_name = "MyLeader".to_string();

    let path = std::env::temp_dir().join("acadrust_current_style_roundtrip.dxf");
    DxfWriter::new(&doc)
        .write_to_file(&path)
        .expect("write dxf");
    let loaded = DxfReader::from_file(&path)
        .expect("open")
        .read()
        .expect("read");
    let _ = std::fs::remove_file(&path);

    assert_eq!(loaded.header.current_text_style_name, "MyText");
    assert_eq!(loaded.header.current_dimstyle_name, "MyDim");
    assert_eq!(loaded.header.multiline_style, "MyMline");
    assert_eq!(loaded.header.current_table_style_name, "MyTable");
    assert_eq!(loaded.header.current_mleader_style_name, "MyLeader");
}
