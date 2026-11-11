//! Below R2010 a VISUALSTYLE carries the ACAD marker pair
//! `1000 AcDbSavedByObjectVersion`, `1070 0`. The pair is written as one unit
//! next to the style's existing ACAD XDATA, which is kept as it is.

use acadrust::objects::{ObjectType, VisualStyle};
use acadrust::types::DxfVersion;
use acadrust::xdata::{ExtendedDataRecord, XDataValue};
use acadrust::{CadDocument, DwgReader, DwgWriter, Handle};
use std::io::Cursor;

const MARKER: &str = "AcDbSavedByObjectVersion";

fn s(value: &str) -> XDataValue {
    XDataValue::String(value.to_string())
}

fn marker() -> Vec<XDataValue> {
    vec![s(MARKER), XDataValue::Integer16(0)]
}

/// Round-trip pairs as AutoCAD keeps them on its visual styles.
fn round_trip_pairs() -> Vec<XDataValue> {
    vec![
        s("AcRTDxfName"),
        s("VISUALSTYLE"),
        s("AcRTDxfName"),
        s("VISUALSTYLE"),
        s("AcRTDxfName"),
        s("VISUALSTYLE"),
    ]
}

/// A document of `version` with one visual style in ACAD_VISUALSTYLE whose
/// ACAD XDATA is `acad` (none when empty).
fn styled(version: DxfVersion, acad: Vec<XDataValue>) -> (CadDocument, Handle) {
    let mut doc = CadDocument::with_version(version);
    let mut style = VisualStyle::new();
    style.handle = doc.allocate_handle();
    style.owner = doc.header.acad_visualstyle_dict_handle;
    style.description = "Probe".to_string();
    let handle = style.handle;
    doc.objects.insert(handle, ObjectType::VisualStyle(style));
    if let Some(ObjectType::Dictionary(dict)) = doc
        .objects
        .get_mut(&doc.header.acad_visualstyle_dict_handle)
    {
        dict.add_entry("Probe", handle);
    }
    if !acad.is_empty() {
        let mut record = ExtendedDataRecord::new("ACAD");
        record.values = acad;
        doc.set_object_xdata_record(handle, record);
    }
    (doc, handle)
}

fn read_back(doc: &CadDocument) -> CadDocument {
    let dwg = DwgWriter::write_to_vec(doc).unwrap();
    DwgReader::from_stream(Cursor::new(dwg)).read().unwrap()
}

/// Handle of the visual style named `Probe` in `doc`.
fn probe(doc: &CadDocument) -> Handle {
    doc.objects
        .values()
        .find_map(|object| match object {
            ObjectType::VisualStyle(style) if style.description == "Probe" => Some(style.handle),
            _ => None,
        })
        .expect("visual style read back")
}

/// ACAD XDATA of the visual style named `Probe` in `doc`.
fn acad_values(doc: &CadDocument) -> Vec<XDataValue> {
    doc.object_xdata(probe(doc))
        .and_then(|data| data.get_record("ACAD"))
        .map(|record| record.values.clone())
        .unwrap_or_default()
}

fn with_marker(values: Vec<XDataValue>) -> Vec<XDataValue> {
    let mut expected = marker();
    expected.extend(values);
    expected
}

#[test]
fn marker_pair_goes_before_round_trip_pairs() {
    for version in [DxfVersion::AC1018, DxfVersion::AC1021] {
        let (doc, _) = styled(version, round_trip_pairs());
        let read = read_back(&doc);
        assert_eq!(
            acad_values(&read),
            with_marker(round_trip_pairs()),
            "{version:?}"
        );
    }
}

#[test]
fn marker_does_not_overwrite_an_existing_integer() {
    let acad = vec![
        s("AcRTDxfName"),
        s("VISUALSTYLE"),
        s("SomeSetting"),
        XDataValue::Integer16(5),
    ];
    for version in [DxfVersion::AC1018, DxfVersion::AC1021] {
        let (doc, _) = styled(version, acad.clone());
        let read = read_back(&doc);
        assert_eq!(acad_values(&read), with_marker(acad.clone()), "{version:?}");
    }
}

#[test]
fn raw_eed_from_a_newer_file_gets_the_marker_pair() {
    // Read from an R2018 file the style's ACAD XDATA is verbatim EED; saved as
    // R2007 it goes through the same merge. The decoded copy is dropped so
    // that only the verbatim block is left.
    let (doc, _) = styled(DxfVersion::AC1032, round_trip_pairs());
    let mut read = read_back(&doc);
    assert_eq!(
        acad_values(&read),
        round_trip_pairs(),
        "R2018 has no marker"
    );
    let handle = probe(&read);
    read.object_xdata.remove(&handle);
    read.version = DxfVersion::AC1021;
    let again = read_back(&read);
    assert_eq!(acad_values(&again), with_marker(round_trip_pairs()));
}

#[test]
fn existing_marker_pair_is_updated_in_place() {
    let mut acad = vec![s(MARKER), XDataValue::Integer16(3)];
    acad.extend(round_trip_pairs());
    let (doc, _) = styled(DxfVersion::AC1021, acad);
    let read = read_back(&doc);
    assert_eq!(acad_values(&read), with_marker(round_trip_pairs()));
}

#[test]
fn style_without_acad_xdata_gets_the_marker_alone() {
    let (doc, _) = styled(DxfVersion::AC1021, Vec::new());
    let read = read_back(&doc);
    assert_eq!(acad_values(&read), marker());
}

#[test]
fn r2010_and_later_write_no_marker() {
    let (doc, _) = styled(DxfVersion::AC1024, round_trip_pairs());
    let read = read_back(&doc);
    assert_eq!(acad_values(&read), round_trip_pairs());
}
