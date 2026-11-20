//! ACAD_PROXY_ENTITY records whose original data format is DXF (`70 1`)
//! are written back group for group.
//!
//! AutoCAD stores such an entity's own DXF groups after `70 1`
//! (`100 McDbEntity2`, `10 1.5`, `70 3`, ...) and refuses a DXF that has
//! binary data there ("Xdata wasn't read ... drawing discarded"). The
//! reader kept only binary data and let the data groups' own 70/90
//! overwrite the header; the writer then wrote `70 0` with `161 0 / 94 0`,
//! dropping the entity's data. ACAD_PROXY_OBJECT already keeps its groups
//! (ProxyObject::dxf_data); entities need the same.

use acadrust::{DxfReader, DxfWriter};
use std::io::Cursor;

/// The fixture written by AutoCAD (see `tests/datatable/README.md`).
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/datatable/point_object_id.dxf"
);

/// Two proxy entities as AutoCAD writes them when the creating application
/// is not loaded (synthetic records): `100 AcDbProxyEntity`, the `90/91/
/// 71/97` header, `70 1`, then the entity's own DXF groups. Class numbers
/// 516/517 name no class of the fixture's CLASSES section.
const PROXIES: &str = concat!(
    "0\nACAD_PROXY_ENTITY\n5\n400\n330\n70\n100\nAcDbEntity\n8\n0\n",
    "100\nAcDbProxyEntity\n90\n498\n91\n516\n71\n31\n97\n105\n70\n1\n",
    "100\nMcDbEntity2\n10\n1.5\n20\n2.5\n30\n3.5\n70\n3\n90\n42\n1\ncustom text\n",
    "0\nACAD_PROXY_ENTITY\n5\n401\n330\n70\n100\nAcDbEntity\n8\n0\n",
    "100\nAcDbProxyEntity\n90\n498\n91\n517\n71\n31\n97\n105\n70\n1\n",
    "100\nAcDbMyOther\n40\n3.25\n290\n1\n310\n0A0B0C0D\n1001\nMYAPP\n1000\nxdata value\n",
    "0\nACAD_PROXY_ENTITY\n5\n402\n330\n70\n100\nAcDbEntity\n8\n0\n",
    "100\nAcDbProxyEntity\n90\n498\n91\n516\n92\n4\n310\n01020304\n71\n31\n97\n105\n70\n1\n",
    "100\nMcDbEntity2\n10\n1.5\n",
);

fn pairs(text: &str) -> Vec<(i32, String)> {
    let lines: Vec<&str> = text.lines().collect();
    lines
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0].trim().parse().unwrap_or(-1), c[1].trim().to_string()))
        .collect()
}

/// Group code / value pairs of the record with handle `handle` after
/// `100 AcDbProxyEntity`.
fn proxy_groups(text: &str, handle: &str) -> Vec<(i32, String)> {
    let pairs = pairs(text);
    let start = pairs
        .windows(2)
        .position(|w| w[0] == (0, "ACAD_PROXY_ENTITY".into()) && w[1] == (5, handle.into()))
        .unwrap_or_else(|| panic!("proxy {handle}"));
    pairs[start + 1..]
        .iter()
        .take_while(|(code, _)| *code != 0)
        .skip_while(|p| **p != (100, "AcDbProxyEntity".into()))
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
fn dxf_format_proxy_entities_round_trip_group_for_group() {
    // Normalise line endings first: a Windows checkout has CRLF.
    let fixture = std::fs::read_to_string(FIXTURE)
        .expect("fixture")
        .replace("\r\n", "\n");
    let marker = "ENTITIES\n  0\nENDSEC";
    let at = fixture.find(marker).expect("ENTITIES end") + "ENTITIES\n".len();
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
    for (handle, subclass) in [("400", "McDbEntity2"), ("401", "AcDbMyOther")] {
        let before = proxy_groups(&source, handle);
        assert!(before.contains(&(100, subclass.into())), "fixture {handle}");
        // The data groups come back group for group; the class number
        // names the same class (here none: the fixture's CLASSES section
        // has no class 516/517).
        assert_eq!(
            data_and_class(&written, proxy_groups(&written, handle)),
            data_and_class(&source, before),
            "proxy {handle}"
        );
    }
    // Proxy graphics stay, before `70 1`, so they are not read back as data.
    let groups = proxy_groups(&written, "402");
    let graphics = groups
        .iter()
        .position(|g| *g == (310, "01020304".into()))
        .unwrap_or_else(|| panic!("graphics lost: {groups:?}"));
    let flag = groups.iter().position(|g| *g == (70, "1".into())).expect("70 1");
    assert!(graphics < flag, "{groups:?}");
    assert_eq!(groups[flag + 1..], [(100, "McDbEntity2".into()), (10, "1.5".into())]);
}

#[test]
fn binary_proxy_entities_are_written_with_dwg_data_format() {
    // A DWG read can carry `from_dxf` (the entity's original data format
    // was DXF) together with binary data. In a DXF file binary data after
    // `70 1` makes AutoCAD discard the drawing; AutoCAD itself writes
    // `70 0`.
    use acadrust::entities::{
        EntityCommon, EntityType, ExtendedEntity, ExtendedEntityData, ProxyEntityData,
    };
    use acadrust::objects::ProxyPayload;
    use acadrust::types::DxfVersion;
    use acadrust::CadDocument;
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let entity = ExtendedEntity {
        common: EntityCommon::new(),
        data: ExtendedEntityData::Proxy(ProxyEntityData {
            proxy_id: 498,
            class_id: 516,
            dxf_subclass: String::new(),
            version: 0,
            dwg_version: 31,
            maintenance_version: 105,
            from_dxf: true,
            graphics: ProxyPayload::default(),
            payload: ProxyPayload::from_bits(&[0xAB, 0xC0], 10),
            text_payload: ProxyPayload::default(),
            object_ids: Vec::new(),
            dxf_data: Vec::new(),
        }),
    };
    let handle = doc
        .add_entity(EntityType::Extended(Box::new(entity)))
        .expect("add entity");
    let written = String::from_utf8(DxfWriter::new(&doc).write_to_vec().expect("write dxf"))
        .unwrap()
        .replace("\r\n", "\n");
    let groups = proxy_groups(&written, &format!("{:X}", handle.value()));
    let flag = groups.iter().find(|(code, _)| *code == 70).expect("70");
    assert_eq!(flag.1, "0", "{groups:?}");
    assert!(groups.iter().any(|(code, _)| *code == 310), "{groups:?}");
}
