//! A dynamic block definition's purge preventer survives a save.
//!
//! The extension dictionary of a dynamic block's BLOCK_RECORD holds an
//! `AcDbDynamicBlockRoundTripPurgePreventer` entry that points at an
//! `ACDB_DYNAMICBLOCKPURGEPREVENTER_VERSION` object. In DXF that object has
//! the subclass marker `AcDbDynamicBlockPurgePreventer` and a version (70),
//! and nothing else. In DWG it also has a hard pointer to the BLOCK_RECORD
//! that owns the dictionary.

use acadrust::objects::{DynamicBlockData, DynamicBlockObject, ObjectType};
use acadrust::tables::BlockRecord;
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter, Handle};
use std::io::Cursor;

const ENTRY: &str = "AcDbDynamicBlockRoundTripPurgePreventer";
const TYPE: &str = "ACDB_DYNAMICBLOCKPURGEPREVENTER_VERSION";

struct Source {
    dxf: Vec<u8>,
    block: Handle,
    dictionary: Handle,
    preventer: Handle,
}

fn pairs(bytes: &[u8]) -> Vec<(String, String)> {
    let text = String::from_utf8_lossy(bytes).replace('\r', "");
    let lines: Vec<&str> = text.split('\n').collect();
    lines
        .chunks(2)
        .filter(|pair| pair.len() == 2)
        .map(|pair| (pair[0].trim().to_string(), pair[1].trim().to_string()))
        .collect()
}

fn join(pairs: &[(String, String)]) -> Vec<u8> {
    let mut out = String::new();
    for (code, value) in pairs {
        out.push_str(&format!("{code:>3}\n{value}\n"));
    }
    out.into_bytes()
}

fn pair(code: &str, value: impl Into<String>) -> (String, String) {
    (code.to_string(), value.into())
}

fn hex(handle: Handle) -> String {
    format!("{:X}", handle.value())
}

/// A drawing with one block whose extension dictionary holds the purge
/// preventer, written the way a DXF from the block's authoring application
/// has it.
fn source() -> Source {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut record = BlockRecord::new("PP_BLOCK");
    record.handle = doc.allocate_handle();
    record.block_entity_handle = doc.allocate_handle();
    record.block_end_handle = doc.allocate_handle();
    let block = record.handle;
    doc.block_records.add(record).unwrap();
    let dictionary = doc.allocate_handle();
    let preventer = doc.allocate_handle();

    let mut groups = pairs(&DxfWriter::new(&doc).write_to_vec().unwrap());

    // The block record names its extension dictionary.
    let at = groups
        .windows(2)
        .position(|w| w[0] == pair("0", "BLOCK_RECORD") && w[1] == pair("5", hex(block)))
        .expect("block record written");
    groups.splice(
        at + 2..at + 2,
        [
            pair("102", "{ACAD_XDICTIONARY"),
            pair("360", hex(dictionary)),
            pair("102", "}"),
        ],
    );

    // The dictionary and the purge preventer, at the end of OBJECTS.
    let objects = groups
        .iter()
        .position(|g| *g == pair("2", "OBJECTS"))
        .expect("objects section");
    let end = objects
        + groups[objects..]
            .iter()
            .position(|g| *g == pair("0", "ENDSEC"))
            .unwrap();
    groups.splice(
        end..end,
        [
            pair("0", "DICTIONARY"),
            pair("5", hex(dictionary)),
            pair("330", hex(block)),
            pair("100", "AcDbDictionary"),
            pair("280", "1"),
            pair("281", "1"),
            pair("3", ENTRY),
            pair("360", hex(preventer)),
            pair("0", TYPE),
            pair("5", hex(preventer)),
            pair("102", "{ACAD_REACTORS"),
            pair("330", hex(dictionary)),
            pair("102", "}"),
            pair("330", hex(dictionary)),
            pair("100", "AcDbDynamicBlockPurgePreventer"),
            pair("70", "1"),
        ],
    );

    Source {
        dxf: join(&groups),
        block,
        dictionary,
        preventer,
    }
}

fn read_dxf(bytes: &[u8]) -> CadDocument {
    DxfReader::from_reader(Cursor::new(bytes.to_vec()))
        .unwrap()
        .read()
        .unwrap()
}

fn read_dwg(bytes: Vec<u8>) -> CadDocument {
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn preventer(doc: &CadDocument, handle: Handle) -> &DynamicBlockObject {
    match doc.objects.get(&handle) {
        Some(ObjectType::DynamicBlock(object)) if object.dxf_name == TYPE => object,
        other => panic!("purge preventer {handle:?} lost, read {other:?}"),
    }
}

fn entry(doc: &CadDocument, dictionary: Handle) -> Option<Handle> {
    match doc.objects.get(&dictionary) {
        Some(ObjectType::Dictionary(d)) => d
            .entries
            .iter()
            .find(|(key, _)| key == ENTRY)
            .map(|(_, handle)| *handle),
        other => panic!("dictionary {dictionary:?} lost, read {other:?}"),
    }
}

fn version(object: &DynamicBlockObject) -> i16 {
    match &object.data {
        DynamicBlockData::Representation(data) => data.flags,
        other => panic!("purge preventer read as {other:?}"),
    }
}

fn block(object: &DynamicBlockObject) -> Handle {
    match &object.data {
        DynamicBlockData::Representation(data) => data.block,
        other => panic!("purge preventer read as {other:?}"),
    }
}

/// The groups of the first record of `type_name` after its `0` line.
fn record(bytes: &[u8], type_name: &str) -> Option<Vec<(String, String)>> {
    let groups = pairs(bytes);
    let at = groups.iter().position(|g| *g == pair("0", type_name))?;
    Some(
        groups[at + 1..]
            .iter()
            .take_while(|g| g.0 != "0")
            .cloned()
            .collect(),
    )
}

#[test]
fn the_fixture_is_read() {
    let source = source();
    let doc = read_dxf(&source.dxf);
    assert_eq!(version(preventer(&doc, source.preventer)), 1);
    assert_eq!(entry(&doc, source.dictionary), Some(source.preventer));
}

#[test]
fn dxf_save_keeps_the_purge_preventer_in_its_own_form() {
    let source = source();
    let written = DxfWriter::new(&read_dxf(&source.dxf))
        .write_to_vec()
        .unwrap();

    assert_eq!(
        record(&written, TYPE),
        Some(vec![
            pair("5", hex(source.preventer)),
            pair("102", "{ACAD_REACTORS"),
            pair("330", hex(source.dictionary)),
            pair("102", "}"),
            pair("330", hex(source.dictionary)),
            pair("100", "AcDbDynamicBlockPurgePreventer"),
            pair("70", "1"),
        ]),
        "the purge preventer record"
    );

    let back = read_dxf(&written);
    assert_eq!(version(preventer(&back, source.preventer)), 1);
    assert_eq!(entry(&back, source.dictionary), Some(source.preventer));
}

#[test]
fn dwg_save_of_a_dxf_purge_preventer_points_at_its_block() {
    let source = source();
    let dwg = DwgWriter::write_to_vec(&read_dxf(&source.dxf)).unwrap();
    let back = read_dwg(dwg);
    let object = preventer(&back, source.preventer);
    assert_eq!(version(object), 1);
    assert_eq!(block(object), source.block, "the block it guards");
    assert_eq!(entry(&back, source.dictionary), Some(source.preventer));
}

#[test]
fn dxf_save_after_dwg_keeps_the_purge_preventer() {
    let source = source();
    let dwg = DwgWriter::write_to_vec(&read_dxf(&source.dxf)).unwrap();
    let written = DxfWriter::new(&read_dwg(dwg)).write_to_vec().unwrap();
    let groups = record(&written, TYPE).expect("the purge preventer record");
    assert!(groups.contains(&pair("100", "AcDbDynamicBlockPurgePreventer")));
    assert!(groups.contains(&pair("70", "1")));
    assert!(!groups.iter().any(|g| g.0 == "340"), "no block in DXF");
    let back = read_dxf(&written);
    assert_eq!(version(preventer(&back, source.preventer)), 1);
    assert_eq!(entry(&back, source.dictionary), Some(source.preventer));
}
