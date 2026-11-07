//! A `read_visiting` consumer only sees an entity's raw EED, because XDATA is
//! decoded after every entity is read. `eed_codec::decode_xdata` decodes it
//! once the document is finished, to the records `read` attaches.

use std::collections::HashMap;
use std::io::Cursor;

use acadrust::io::dwg::eed_codec::decode_xdata;
use acadrust::types::DxfVersion;
use acadrust::xdata::{ExtendedDataRecord, XDataValue};
use acadrust::{DwgReader, DwgWriter, DxfReader};

const DXF: &str = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1032\n0\nENDSEC\n0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nAPPID\n70\n1\n0\nAPPID\n2\nACME\n70\n0\n0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0.0\n20\n0.0\n30\n0.0\n11\n1.0\n21\n1.0\n31\n0.0\n1001\nACME\n1000\nstreamed\n1003\n0\n1040\n2.5\n1070\n7\n0\nENDSEC\n0\nEOF\n";

#[test]
fn raw_eed_from_read_visiting_decodes_to_the_records_read_attaches() {
    let mut source = DxfReader::from_reader(Cursor::new(DXF.as_bytes()))
        .unwrap()
        .read()
        .unwrap();
    // Narrow (code page) strings before R2007, UTF-16 from R2007 on.
    for version in [DxfVersion::AC1018, DxfVersion::AC1032] {
        source.version = version;
        let dwg = DwgWriter::write_to_vec(&source).unwrap();

        let mut raw = HashMap::new();
        let streamed = DwgReader::from_stream(Cursor::new(dwg.clone()))
            .read_visiting(|_, entity| {
                let common = entity.common();
                if !common.extended_data.raw_dwg_eed.is_empty() {
                    raw.insert(common.handle, common.extended_data.raw_dwg_eed.clone());
                }
                None
            })
            .unwrap();
        assert_eq!(raw.len(), 1, "{version:?}: one entity carries EED");

        let read = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
        for (handle, raw_eed) in &raw {
            let expected: Vec<ExtendedDataRecord> = read
                .get_entity(*handle)
                .unwrap()
                .common()
                .extended_data
                .records()
                .to_vec();
            assert_eq!(
                expected[0].values,
                vec![
                    XDataValue::String("streamed".into()),
                    XDataValue::LayerName("0".into()),
                    XDataValue::Real(2.5),
                    XDataValue::Integer16(7),
                ],
                "{version:?}"
            );
            assert_eq!(decode_xdata(raw_eed, &streamed), expected, "{version:?}");
        }
    }
}

#[test]
fn unknown_application_and_bad_payload_are_skipped() {
    let document = DxfReader::from_reader(Cursor::new(DXF.as_bytes()))
        .unwrap()
        .read()
        .unwrap();
    let acme = document
        .app_ids
        .iter()
        .find(|app| app.name == "ACME")
        .unwrap()
        .handle
        .value();
    let raw = vec![
        // no APPID with this handle
        (0xDEAD_BEEF, vec![70, 7, 0]),
        // 0x63 is no EED item code
        (acme, vec![0x63, 0, 0]),
        (acme, vec![70, 7, 0]),
    ];
    let records = decode_xdata(&raw, &document);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].application_name, "ACME");
    assert_eq!(records[0].values, vec![XDataValue::Integer16(7)]);
}
