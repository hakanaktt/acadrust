//! `DwgReadOptions::skip_roundtrip_data` reads without the verbatim bytes an
//! unedited entity is re-saved from (`raw_record`, the raw EED of decoded
//! XDATA). The read is otherwise the same, and a save re-encodes.

use std::io::Cursor;

use acadrust::types::DxfVersion;
use acadrust::xdata::XDataValue;
use acadrust::{CadDocument, DwgReadOptions, DwgReader, DwgWriter, DxfReader};

const DXF: &str = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1032\n0\nENDSEC\n0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nAPPID\n70\n1\n0\nAPPID\n2\nACME\n70\n0\n0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nLINE\n8\n0\n10\n0.0\n20\n0.0\n30\n0.0\n11\n1.0\n21\n1.0\n31\n0.0\n1001\nACME\n1000\nkept\n1070\n7\n0\nCIRCLE\n8\n0\n10\n5.0\n20\n5.0\n30\n0.0\n40\n2.0\n0\nENDSEC\n0\nEOF\n";

fn read(dwg: &[u8], options: DwgReadOptions) -> CadDocument {
    DwgReader::from_stream_with_options(Cursor::new(dwg.to_vec()), options)
        .read()
        .unwrap()
}

#[test]
fn skip_roundtrip_data_keeps_the_model_and_drops_the_verbatim_bytes() {
    let mut source = DxfReader::from_reader(Cursor::new(DXF.as_bytes()))
        .unwrap()
        .read()
        .unwrap();
    for version in [DxfVersion::AC1018, DxfVersion::AC1032] {
        source.version = version;
        let dwg = DwgWriter::write_to_vec(&source).unwrap();

        let full = read(&dwg, DwgReadOptions::default());
        let lean = read(&dwg, DwgReadOptions::default().without_roundtrip_data());

        // The default read keeps what a byte-exact re-save needs.
        assert!(full.entities().any(|e| e.common().raw_record.is_some()), "{version:?}");
        assert!(
            full.entities().any(|e| !e.common().extended_data.raw_dwg_eed.is_empty()),
            "{version:?}"
        );

        assert_eq!(lean.entities().count(), full.entities().count(), "{version:?}");
        for entity in lean.entities() {
            let common = entity.common();
            assert!(common.raw_record.is_none(), "{version:?}");
            assert!(common.extended_data.raw_dwg_eed.is_empty(), "{version:?}");
            let original = full.get_entity(common.handle).unwrap();
            assert_eq!(
                common.extended_data.records(),
                original.common().extended_data.records(),
                "{version:?}"
            );
        }

        // A save re-encodes every entity and still carries the XDATA.
        let resaved = read(&DwgWriter::write_to_vec(&lean).unwrap(), DwgReadOptions::default());
        assert_eq!(resaved.entities().count(), full.entities().count(), "{version:?}");
        let values: Vec<_> = resaved
            .entities()
            .flat_map(|e| e.common().extended_data.records().to_vec())
            .map(|record| (record.application_name, record.values))
            .collect();
        assert_eq!(
            values,
            vec![(
                "ACME".to_string(),
                vec![XDataValue::String("kept".into()), XDataValue::Integer16(7)]
            )],
            "{version:?}"
        );
    }
}
