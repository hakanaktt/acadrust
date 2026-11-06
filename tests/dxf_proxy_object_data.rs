//! ACAD_PROXY_OBJECT records whose original data format is DXF (`70 1`)
//! are written back group for group.
//!
//! AutoCAD stores such an object's own DXF groups after `70 1`
//! (`100 AcDbRenderSettings`, `90 2`, `1 ...`) and refuses a DXF that has binary
//! data there ("Xdata wasn't read ... drawing discarded"). The reader kept
//! only binary data (none here) and let the data groups' own 70/90 overwrite
//! the header; the writer then wrote `162 0 / 161 0 / 94 0` after `70 1`.

use acadrust::{DxfReader, DxfWriter};
use std::io::Cursor;

/// The fixture written by AutoCAD (see `tests/datatable/README.md`).
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/datatable/point_object_id.dxf"
);

/// Two render-settings proxies as AutoCAD writes them when the render module
/// is not loaded (synthetic drawing, default presets).
const PROXIES: &str = "0\nACAD_PROXY_OBJECT\n5\n296\n102\n{ACAD_REACTORS\n330\n292\n102\n}\n330\n292\n100\nAcDbProxyObject\n90\n499\n91\n516\n71\n31\n97\n105\n70\n1\n100\nAcDbRenderSettings\n90\n2\n1\nCoffee-Break Quality\n290\n1\n290\n0\n290\n1\n290\n1\n1\n\n1\nRenders for 10 minutes\n90\n3\n100\nAcDbRapidRTRenderSettings\n90\n3\n70\n1\n90\n1\n90\n10\n70\n1\n70\n2\n40\n3.0\n40\n3.0\n290\n1\n0\nACAD_PROXY_OBJECT\n5\n295\n102\n{ACAD_REACTORS\n330\n292\n102\n}\n330\n292\n100\nAcDbProxyObject\n90\n499\n91\n516\n71\n31\n97\n105\n70\n1\n100\nAcDbRenderSettings\n90\n2\n1\nHigh\n290\n1\n290\n0\n290\n1\n290\n1\n1\n\n1\nApplies 10 rendering levels\n90\n2\n100\nAcDbRapidRTRenderSettings\n90\n3\n70\n0\n90\n10\n90\n1\n70\n2\n70\n2\n40\n3.0\n40\n3.0\n290\n1\n";

fn pairs(text: &str) -> Vec<(i32, String)> {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0].trim().parse().unwrap_or(-1), c[1].trim().to_string()))
        .collect()
}

/// Group code / value pairs of the record with handle `handle` after
/// `100 AcDbProxyObject`.
fn proxy_groups(text: &str, handle: &str) -> Vec<(i32, String)> {
    let pairs = pairs(text);
    let start = pairs
        .windows(2)
        .position(|w| w[0] == (0, "ACAD_PROXY_OBJECT".into()) && w[1] == (5, handle.into()))
        .unwrap_or_else(|| panic!("proxy {handle}"));
    pairs[start + 1..]
        .iter()
        .take_while(|(code, _)| *code != 0)
        .skip_while(|p| **p != (100, "AcDbProxyObject".into()))
        .skip(1)
        .cloned()
        .collect()
}

/// The DXF name of the class that proxy class number `number` (group 91)
/// names in the file `text`: 500 + the place of the `CLASS` record in its
/// CLASSES section. `None` — the number names no class of the file.
fn class_named(text: &str, number: &str) -> Option<String> {
    let place = number.parse::<usize>().ok()?.checked_sub(500)?;
    let pairs = pairs(text);
    let section = pairs
        .windows(2)
        .position(|w| w[0] == (0, "SECTION".into()) && w[1] == (2, "CLASSES".into()))?;
    let mut names = Vec::new();
    for (code, value) in &pairs[section + 2..] {
        match (code, value.as_str()) {
            (0, "ENDSEC") => break,
            (0, "CLASS") => names.push(String::new()),
            (1, name) => {
                if let Some(last) = names.last_mut() {
                    *last = name.to_string();
                }
            }
            _ => {}
        }
    }
    names.get(place).filter(|name| !name.is_empty()).cloned()
}

/// The groups of `groups` without the class number (91), and the class it
/// names in `text`: the writer numbers the CLASSES section it writes, so the
/// number follows the class, not the source file.
fn data_and_class(text: &str, groups: Vec<(i32, String)>) -> (Vec<(i32, String)>, Option<String>) {
    let class = groups
        .iter()
        .find(|(code, _)| *code == 91)
        .and_then(|(_, number)| class_named(text, number));
    let data = groups.into_iter().filter(|(code, _)| *code != 91).collect();
    (data, class)
}

#[test]
fn dxf_format_proxy_objects_round_trip_group_for_group() {
    // Normalise line endings first: a Windows checkout has CRLF.
    let fixture = std::fs::read_to_string(FIXTURE)
        .expect("fixture")
        .replace("\r\n", "\n");
    let at = fixture.rfind("ENDSEC").expect("OBJECTS end");
    let at = fixture[..at].rfind("\n  0\n").expect("ENDSEC pair") + 1;
    let source = format!("{}{}{}", &fixture[..at], PROXIES, &fixture[at..]);
    let doc = DxfReader::from_reader(Cursor::new(source.clone().into_bytes()))
        .expect("open dxf")
        .read()
        .expect("read dxf");
    let written = String::from_utf8(
        DxfWriter::new(&doc)
            .with_version(doc.version)
            .write_to_vec()
            .expect("write dxf"),
    )
    .unwrap();
    for handle in ["296", "295"] {
        let before = proxy_groups(&source, handle);
        assert!(
            before.contains(&(100, "AcDbRenderSettings".into())),
            "fixture {handle}"
        );
        // The data groups come back group for group; the class number
        // names the same class (here none: the fixture's CLASSES section has
        // no class 516, #329).
        assert_eq!(
            data_and_class(&written, proxy_groups(&written, handle)),
            data_and_class(&source, before),
            "proxy {handle}"
        );
    }
}

#[test]
fn binary_proxy_objects_are_written_with_dwg_data_format() {
    // A DWG read can carry `from_dxf` (the object's original data format
    // was DXF) together with binary data. In a DXF file binary data after
    // `70 1` makes AutoCAD discard the drawing; AutoCAD itself writes `70 0`.
    use acadrust::objects::{ObjectType, ProxyObject, ProxyPayload};
    use acadrust::types::DxfVersion;
    use acadrust::CadDocument;
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let handle = doc.allocate_handle();
    let object = ProxyObject {
        handle,
        proxy_id: 499,
        class_id: 516,
        dwg_version: 31,
        maintenance_version: 105,
        from_dxf: true,
        payload: ProxyPayload::from_bits(&[0xAB, 0xC0], 10),
        ..Default::default()
    };
    doc.objects.insert(handle, ObjectType::ProxyObject(object));
    let written = String::from_utf8(DxfWriter::new(&doc).write_to_vec().expect("write dxf"))
        .unwrap()
        .replace("\r\n", "\n");
    let groups = proxy_groups(&written, &format!("{:X}", handle.value()));
    let flag = groups.iter().find(|(code, _)| *code == 70).expect("70");
    assert_eq!(flag.1, "0", "{groups:?}");
    assert!(groups.iter().any(|(code, _)| *code == 310), "{groups:?}");
}
