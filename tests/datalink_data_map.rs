//! An Excel data link keeps its data map (the `CUSTOMDATA` / `DATAMAP` key/value collection that
//! holds the connection string, the source date and the update options) through DXF and DWG.
//!
//! The drawing is synthetic: a DATALINK built here, with a data map in the form AutoCAD writes in
//! DXF (a count, then for each item its name and a value ending with `ACVALUE_END`).

use acadrust::objects::{ClassObject, ClassObjectData, DataLink, ObjectType};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};
use std::io::Cursor;

const CONNECTION: &str = "C:\\Data\\Quantities.xlsx!Sheet1";
/// 2026-11-01 12:00, as the 16 bytes of a SYSTEMTIME.
const SOURCE_DATE: &str = "EA070B00000001000C00000000000000";

/// The data map AutoCAD writes for an Excel link, one group code and value per entry.
fn data_map_pairs() -> Vec<(i32, String)> {
    let mut pairs = vec![(90, "3".to_string())];
    let item = |name: &str, value: Vec<(i32, &str)>| {
        let mut out = vec![(300, name.to_string()), (301, "DATAMAP_VALUE".to_string())];
        out.extend(value.into_iter().map(|(c, v)| (c, v.to_string())));
        out.extend([
            (94, "0".to_string()),
            (300, String::new()),
            (302, String::new()),
        ]);
        out.push((304, "ACVALUE_END".to_string()));
        out
    };
    pairs.extend(item(
        "ACEXCEL_CONNECTION_STRING",
        vec![(93, "2"), (90, "4"), (1, CONNECTION)],
    ));
    pairs.extend(item(
        "ACEXCEL_SOURCEDATE",
        vec![(93, "2"), (90, "8"), (92, "16"), (310, SOURCE_DATE)],
    ));
    pairs.extend(item(
        "ACEXCEL_UPDATEOPTIONS",
        vec![(93, "2"), (90, "1"), (91, "68943873")],
    ));
    pairs
}

/// A DXF with one Excel data link whose data map holds three items.
fn link_dxf() -> String {
    let mut document = CadDocument::new();
    let handle = document.allocate_handle();
    let mut object = ClassObject::new(ClassObjectData::DataLink(DataLink {
        data_adapter: "AcExcel".to_string(),
        description: "Quantities".to_string(),
        connection_string: CONNECTION.to_string(),
        option: 2,
        update_option: 68943873,
        flags: 1,
        year: 2026,
        month: 11,
        day: 1,
        hour: 12,
        ..DataLink::default()
    }));
    object.handle = handle;
    object.owner = document.header.named_objects_dict_handle;
    document
        .objects
        .insert(handle, ObjectType::ClassObject(object));

    let text = String::from_utf8(DxfWriter::new(&document).write_to_vec().expect("write DXF"))
        .expect("ASCII DXF");
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let marker = format!("DATAMAP_BEGIN{eol}");
    let at = text
        .find(&marker)
        .expect("the data link is written with its data map block")
        + marker.len();
    let map: String = data_map_pairs()
        .iter()
        .map(|(c, v)| format!("{c:>3}{eol}{v}{eol}"))
        .collect();
    format!("{}{}{}", &text[..at], map, &text[at..])
}

fn read_dxf(text: &str) -> CadDocument {
    DxfReader::from_reader(Cursor::new(text.as_bytes().to_vec()))
        .expect("create DXF reader")
        .read()
        .expect("read DXF")
}

fn write_dxf(document: &CadDocument) -> String {
    String::from_utf8_lossy(&DxfWriter::new(document).write_to_vec().expect("write DXF"))
        .into_owned()
}

/// The data map of the written DXF is the one read: three items with their names and values.
fn assert_data_map(dxf: &str) {
    for name in [
        "ACEXCEL_CONNECTION_STRING",
        "ACEXCEL_SOURCEDATE",
        "ACEXCEL_UPDATEOPTIONS",
    ] {
        assert!(dxf.contains(name), "data map item {name} is lost");
    }
    assert_eq!(
        dxf.matches("DATAMAP_VALUE").count(),
        3,
        "three data map values"
    );
    assert!(dxf.contains(SOURCE_DATE), "the source date value is lost");
}

#[test]
fn dxf_keeps_the_data_map_of_a_data_link() {
    let source = link_dxf();
    assert_data_map(&source);
    assert_data_map(&write_dxf(&read_dxf(&source)));
}

#[test]
fn dwg_keeps_the_data_map_of_a_data_link() {
    let document = read_dxf(&link_dxf());
    let dwg = DwgWriter::write_to_vec(&document).expect("write DWG");
    let back = DwgReader::from_stream(Cursor::new(dwg))
        .read()
        .expect("read DWG");
    assert_data_map(&write_dxf(&back));
}

/// The data link written by `DwgWriter` from a DXF (option 2, as AutoCAD shows it) carries bit 8
/// of its DWG option, the bit that says its data map follows; the option read back is 2 again.
#[test]
fn dwg_option_carries_the_data_map_bit_only_with_a_map() {
    let link = |document: &CadDocument| {
        document
            .objects
            .values()
            .find_map(|object| match object {
                ObjectType::ClassObject(ClassObject {
                    data: ClassObjectData::DataLink(link),
                    ..
                }) => Some(link.clone()),
                _ => None,
            })
            .expect("the data link survives")
    };
    let from_dxf = read_dxf(&link_dxf());
    assert_eq!(link(&from_dxf).option & DataLink::HAS_DATA_MAP, 0);
    let dwg = DwgWriter::write_to_vec(&from_dxf).expect("write DWG");
    let back = DwgReader::from_stream(Cursor::new(dwg))
        .read()
        .expect("read DWG");
    assert_eq!(link(&back).option, 2, "the option shows without the bit");
    assert_eq!(
        link(&back).data_map.len(),
        3,
        "the map is read because the bit is set"
    );

    // Without a map, the bit is not written and nothing follows.
    let mut empty = from_dxf.clone();
    for object in empty.objects.values_mut() {
        if let ObjectType::ClassObject(ClassObject {
            data: ClassObjectData::DataLink(link),
            ..
        }) = object
        {
            link.data_map.clear();
            link.option |= DataLink::HAS_DATA_MAP;
        }
    }
    let dwg = DwgWriter::write_to_vec(&empty).expect("write DWG");
    let back = DwgReader::from_stream(Cursor::new(dwg))
        .read()
        .expect("read DWG");
    assert_eq!(link(&back).option, 2);
    assert!(link(&back).data_map.is_empty());
}
