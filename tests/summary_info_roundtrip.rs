
use std::io::Cursor;

use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn document_with_properties(version: DxfVersion) -> CadDocument {
    let mut document = CadDocument::with_version(version);
    let info = &mut document.summary_info;
    info.title = "Kanalbau Hauptstra\u{df}e".into();
    info.subject = "Ausf\u{fc}hrungsplanung".into();
    info.author = "Planungsb\u{fc}ro".into();
    info.keywords = "Kanal; Stra\u{df}enbau".into();
    info.comments = "Stand 06.10.2026".into();
    info.last_saved_by = "Bauleitung".into();
    info.revision_number = "3".into();
    info.hyperlink_base = "https://example.com/plaene/".into();
    info.custom_properties = vec![
        ("Plansatz".into(), "Ausf\u{fc}hrung".into()),
        ("H\u{f6}henbezug".into(), "m NHN".into()),
        ("Leer".into(), String::new()),
    ];
    document
}

#[test]
fn summary_info_round_trips_through_dwg() {
    for version in [
        DxfVersion::AC1018,
        DxfVersion::AC1021,
        DxfVersion::AC1024,
        DxfVersion::AC1027,
        DxfVersion::AC1032,
    ] {
        let document = document_with_properties(version);
        let bytes = DwgWriter::write_to_vec(&document).expect("DWG write");
        let read = DwgReader::from_stream(Cursor::new(bytes))
            .read()
            .expect("DWG read");
        assert_eq!(read.summary_info, document.summary_info, "{version:?}");
    }
}

#[test]
fn empty_summary_info_stays_empty() {
    let document = CadDocument::new();
    let bytes = DwgWriter::write_to_vec(&document).expect("DWG write");
    let read = DwgReader::from_stream(Cursor::new(bytes))
        .read()
        .expect("DWG read");
    assert_eq!(read.summary_info, document.summary_info);
}
