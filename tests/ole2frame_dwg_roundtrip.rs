//! OLE2FRAME through DWG: an embedded OLE object written to a DWG file of every version the
//! writer supports reads back with the same data bytes and fields (ODA DWG specification,
//! OLE2FRAME (74): flags BS, R2000+ mode BS, data length BL, the data, R2000+ one more byte),
//! and a damaged or hostile payload length neither panics nor allocates past the record.

use std::io::Cursor;

use acadrust::compound_file::{
    BinaryRecord, CompoundEntry, CompoundFile, CompoundStorage, CompoundStream,
    CompoundStreamContent,
};
use acadrust::entities::{EntityType, Ole2Frame, OleFrameEnvelope, OleObjectType};
use acadrust::io::dwg::dwg_stream_readers::bit_reader::DwgBitReader;
use acadrust::io::dwg::dwg_stream_readers::merged_reader::DwgMergedReader;
use acadrust::io::dwg::dwg_stream_readers::object_reader::entities::read_ole2frame;
use acadrust::io::dwg::dwg_stream_writers::bit_writer::DwgBitWriter;
use acadrust::io::dwg::DwgVersion;
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

const VERSIONS: [DxfVersion; 6] = [
    DxfVersion::AC1015,
    DxfVersion::AC1018,
    DxfVersion::AC1021,
    DxfVersion::AC1024,
    DxfVersion::AC1027,
    DxfVersion::AC1032,
];

/// Synthetic bytes, not a real OLE object's.
fn pattern(n: usize, seed: u32) -> Vec<u8> {
    (0..n as u32)
        .map(|i| (i.wrapping_mul(2_654_435_761).wrapping_add(seed) >> 13) as u8)
        .collect()
}

/// A frame whose data has the DWG wrapper with the frame's corners, a compound file with one
/// stream (larger than one 4096-byte record) and trailing bytes.
fn framed(object_type: OleObjectType, lock_aspect: u8) -> Ole2Frame {
    let mut ole = Ole2Frame::new();
    ole.upper_left_corner = Vector3::new(10.25, 40.5, 0.0);
    ole.lower_right_corner = Vector3::new(70.125, 5.75, 0.0);
    ole.ole_object_type = object_type;
    ole.lock_aspect = lock_aspect;
    ole.envelope = OleFrameEnvelope::Geometry {
        marker: 0x0080,
        extension_records: BinaryRecord::split(&pattern(37, 1), 4096),
    };
    let mut root = CompoundStorage::root();
    root.entries.push(CompoundEntry::Stream(CompoundStream {
        name: "CONTENTS".to_string(),
        class_id: [0; 16],
        state_bits: 0,
        creation_time: 0,
        modified_time: 0,
        content: CompoundStreamContent::Binary(BinaryRecord::split(&pattern(9000, 2), 4096)),
    }));
    ole.storage.compound_file = Some(CompoundFile {
        root,
        ..CompoundFile::default()
    });
    ole.storage.trailing_records = BinaryRecord::split(&pattern(5, 3), 4096);
    ole
}

/// A frame whose data is opaque bytes without the DWG wrapper (kept as they are).
fn opaque() -> Ole2Frame {
    let mut ole = Ole2Frame::new();
    ole.ole_object_type = OleObjectType::Static;
    ole.envelope = OleFrameEnvelope::Legacy {
        records: BinaryRecord::split(&pattern(5000, 4), 4096),
    };
    ole
}

fn frames(doc: &CadDocument) -> Vec<&Ole2Frame> {
    doc.entities()
        .filter_map(|e| match e {
            EntityType::Ole2Frame(o) => Some(o.as_ref()),
            _ => None,
        })
        .collect()
}

fn same_frame(version: DxfVersion, a: &Ole2Frame, b: &Ole2Frame) {
    assert_eq!(
        a.encoded_payload(),
        b.encoded_payload(),
        "{version:?}: the OLE data bytes"
    );
    assert_eq!(a.ole_object_type, b.ole_object_type, "{version:?}");
    assert_eq!(a.lock_aspect, b.lock_aspect, "{version:?}");
    assert_eq!(a.is_paper_space, b.is_paper_space, "{version:?}");
    assert_eq!(a.upper_left_corner, b.upper_left_corner, "{version:?}");
    assert_eq!(a.lower_right_corner, b.lower_right_corner, "{version:?}");
    assert_eq!(a.envelope, b.envelope, "{version:?}");
    assert_eq!(a.storage, b.storage, "{version:?}");
    assert_eq!(a.common.layer, b.common.layer, "{version:?}");
}

#[test]
fn ole_frames_survive_dwg_of_every_version() {
    for version in VERSIONS {
        let mut doc = CadDocument::with_version(version);
        let mut a = framed(OleObjectType::Embedded, 1);
        a.common.layer = "0".to_string();
        let b = opaque();
        let ha = doc.add_entity(EntityType::Ole2Frame(Box::new(a))).unwrap();
        let hb = doc.add_entity(EntityType::Ole2Frame(Box::new(b))).unwrap();
        let bytes = DwgWriter::write_to_vec(&doc).unwrap();
        let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        assert_eq!(frames(&read).len(), 2, "{version:?}");
        for h in [ha, hb] {
            let (EntityType::Ole2Frame(x), Some(EntityType::Ole2Frame(y))) =
                (doc.get_entity(h).unwrap(), read.get_entity(h))
            else {
                panic!("{version:?}: OLE2FRAME {h:?} missing after the DWG round trip")
            };
            same_frame(version, x, y);
        }
    }
}

#[test]
fn ole_frames_survive_dxf_to_dwg_to_dxf() {
    for version in VERSIONS {
        let mut doc = CadDocument::with_version(version);
        let h = doc
            .add_entity(EntityType::Ole2Frame(Box::new(framed(
                OleObjectType::Link,
                0,
            ))))
            .unwrap();
        let mut dxf = Vec::new();
        DxfWriter::new(&doc).write_to_writer(&mut dxf).unwrap();
        let from_dxf = DxfReader::from_reader(Cursor::new(dxf))
            .unwrap()
            .read()
            .unwrap();
        let dwg = DwgWriter::write_to_vec(&from_dxf).unwrap();
        let from_dwg = DwgReader::from_stream(Cursor::new(dwg)).read().unwrap();
        let mut back = Vec::new();
        DxfWriter::new(&from_dwg)
            .write_to_writer(&mut back)
            .unwrap();
        let again = DxfReader::from_reader(Cursor::new(back))
            .unwrap()
            .read()
            .unwrap();
        let (EntityType::Ole2Frame(x), Some(EntityType::Ole2Frame(y))) =
            (doc.get_entity(h).unwrap(), again.get_entity(h))
        else {
            panic!("{version:?}: OLE2FRAME lost on DXF → DWG → DXF")
        };
        same_frame(version, x, y);
    }
}

/// An OLE2FRAME record whose data length is `declared` followed by `actual` bytes.
fn record(dxf_version: DxfVersion, declared: i32, actual: usize) -> (Vec<u8>, i64) {
    let version = DwgVersion::from_dxf_version(dxf_version).unwrap();
    let mut w = DwgBitWriter::new(version, dxf_version);
    w.write_bit_short(2);
    w.write_bit_short(0);
    w.write_bit_long(declared);
    w.write_bytes(&pattern(actual, 5));
    w.write_byte(1);
    let end = w.position_in_bits();
    (w.into_bytes(), end)
}

/// The data bytes a read record holds (wrapper and storage).
fn payload(
    data: &acadrust::io::dwg::dwg_stream_readers::object_reader::entities::Ole2FrameData,
) -> Vec<u8> {
    let mut ole = Ole2Frame::new();
    ole.storage = data.storage.clone();
    ole.envelope = data.envelope.clone();
    ole.upper_left_corner = data.upper_left;
    ole.lower_right_corner = data.lower_right;
    ole.encoded_payload()
}

#[test]
fn hostile_ole_data_lengths_are_capped_by_the_record() {
    for dxf_version in [DxfVersion::AC1015, DxfVersion::AC1032] {
        let version = DwgVersion::from_dxf_version(dxf_version).unwrap();
        for declared in [i32::MAX, i32::MIN, -1, 1 << 30, 200] {
            let (bytes, end) = record(dxf_version, declared, 100);
            let main = DwgBitReader::new(bytes, version, dxf_version);
            let mut reader = DwgMergedReader::from_readers(main, None, None, dxf_version);
            reader.set_main_data_end(end);
            let data = read_ole2frame(&mut reader, version);
            assert!(
                reader.check_error().is_err(),
                "{dxf_version:?}: a data length of {declared} for 100 bytes must fail"
            );
            assert!(payload(&data).len() <= 100, "{dxf_version:?}, {declared}");
        }
        // The exact length reads every byte.
        let (bytes, end) = record(dxf_version, 100, 100);
        let main = DwgBitReader::new(bytes, version, dxf_version);
        let mut reader = DwgMergedReader::from_readers(main, None, None, dxf_version);
        reader.set_main_data_end(end);
        let data = read_ole2frame(&mut reader, version);
        reader.check_error().unwrap();
        assert_eq!(data.lock_aspect, 1);
        assert_eq!(payload(&data), pattern(100, 5));
    }
}

/// Damaged DXF binary chunks (odd length, not hex, not ASCII) keep their whole hex pairs and
/// never panic.
#[test]
fn damaged_dxf_ole_chunks_do_not_panic() {
    let doc = CadDocument::with_version(DxfVersion::AC1015);
    let mut dxf = Vec::new();
    DxfWriter::new(&doc).write_to_writer(&mut dxf).unwrap();
    let text = String::from_utf8(dxf).unwrap();
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let entity = [
        "0",
        "OLE2FRAME",
        "5",
        "1F0",
        "8",
        "0",
        "100",
        "AcDbOle2Frame",
        "70",
        "2",
        "10",
        "1",
        "20",
        "2",
        "30",
        "0",
        "11",
        "3",
        "21",
        "0",
        "31",
        "0",
        "71",
        "2",
        "72",
        "0",
        "90",
        "9",
        "310",
        "0A0B0",
        "310",
        "ZZ",
        "310",
        "0\u{e9}",
        "310",
        "C0FFEE",
        "1",
        "OLE",
    ]
    .join(nl);
    let marker = format!("ENTITIES{nl}");
    let at = text.find(&marker).unwrap() + marker.len();
    let hostile = format!("{}{entity}{nl}{}", &text[..at], &text[at..]);
    let read = DxfReader::from_reader(Cursor::new(hostile.into_bytes()))
        .unwrap()
        .read()
        .unwrap();
    let f = frames(&read);
    assert_eq!(f.len(), 1);
    assert_eq!(f[0].encoded_payload(), vec![0x0A, 0x0B, 0xC0, 0xFF, 0xEE]);
}

/// OLE data as another CAD wrote it: the same compound file, but with bytes the codec does
/// not re-create (here the slack after the last stream byte in its sector). Re-encoding it
/// would change the compound file while the wrapper in front still describes the source one.
fn foreign_payload() -> Vec<u8> {
    let mut raw = framed(OleObjectType::Embedded, 0).encoded_payload();
    let stream = pattern(9000, 2);
    let at = raw
        .windows(64)
        .position(|w| w == &stream[..64])
        .expect("stream bytes in the payload")
        + stream.len();
    let slack = (at
        - raw
            .windows(8)
            .position(|w| w == [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1])
            .unwrap())
        % 512;
    for b in &mut raw[at..at + (512 - slack) % 512] {
        *b = 0xAB;
    }
    raw
}

#[test]
fn source_ole_data_is_written_back_verbatim() {
    let raw = foreign_payload();
    let mut decoded = Ole2Frame::new();
    let (ul, lr) = decoded.set_payload(&raw);
    // Precondition: without the source bytes the codec would write a different compound file.
    let mut rebuilt = decoded.clone();
    rebuilt.source_storage = Default::default();
    assert_ne!(
        rebuilt.encoded_payload(),
        raw,
        "the sample must not re-encode canonically"
    );

    for version in VERSIONS {
        let mut doc = CadDocument::with_version(version);
        let mut ole = decoded.clone();
        (ole.upper_left_corner, ole.lower_right_corner) = (ul, lr);
        let kept = doc
            .add_entity(EntityType::Ole2Frame(Box::new(ole.clone())))
            .unwrap();
        // Moving the frame rewrites the corners in the wrapper, not the compound file.
        ole.upper_left_corner.x += 5.0;
        ole.lower_right_corner.x += 5.0;
        let moved = doc
            .add_entity(EntityType::Ole2Frame(Box::new(ole)))
            .unwrap();
        let bytes = DwgWriter::write_to_vec(&doc).unwrap();
        let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        let data = |h| match read.get_entity(h) {
            Some(EntityType::Ole2Frame(o)) => o.encoded_payload(),
            _ => panic!("{version:?}: OLE2FRAME missing"),
        };
        assert_eq!(data(kept), raw, "{version:?}: unchanged frame");
        let moved = data(moved);
        assert_eq!(
            moved[135..],
            raw[135..],
            "{version:?}: moved frame keeps its compound file"
        );
        assert_ne!(
            moved[..98],
            raw[..98],
            "{version:?}: moved frame has new corners"
        );
    }

    // Editing the storage falls back to the codec's own encoding.
    assert!(!decoded.rewrites_source_storage());
    let mut edited = decoded;
    edited.storage.trailing_records.clear();
    assert!(edited.rewrites_source_storage());
    assert!(!rebuilt.rewrites_source_storage() && !Ole2Frame::new().rewrites_source_storage());
    assert_eq!(edited.encoded_payload(), {
        let mut fresh = edited.clone();
        fresh.source_storage = Default::default();
        fresh.encoded_payload()
    });
}
