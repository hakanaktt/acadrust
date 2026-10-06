//! DXF output that AutoCAD rejects (or repairs on open) unless groups follow
//! the layout AutoCAD itself writes. Expected layouts were taken from
//! AutoCAD 2027 DXFOUT of the same drawings at each version.

use acadrust::entities::*;
use acadrust::objects::{
    ObjectType, ProxyObject, ProxyObjectReference, ProxyPayload, ProxyReferenceKind, Scale,
};
use acadrust::types::{DxfVersion, Handle, Vector2, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

type Pairs = Vec<(i32, String)>;

fn write_pairs(doc: &CadDocument) -> Pairs {
    let bytes = DxfWriter::new(doc).write_to_vec().expect("write dxf");
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let lines: Vec<&str> = text.lines().collect();
    lines
        .chunks(2)
        .filter(|chunk| chunk.len() == 2)
        .map(|chunk| {
            (
                chunk[0].trim().parse().unwrap_or(-1),
                chunk[1].trim_end().to_string(),
            )
        })
        .collect()
}

/// Groups of the record with handle `handle` (from its 5 group to the next 0 group).
fn record(pairs: &Pairs, handle: Handle) -> Pairs {
    let hex = format!("{:X}", handle.value());
    let start = pairs
        .iter()
        .position(|(code, value)| *code == 5 && value.trim() == hex)
        .unwrap_or_else(|| panic!("record {hex} not written"));
    pairs[start..]
        .iter()
        .take_while(|(code, _)| *code != 0)
        .cloned()
        .collect()
}

/// Group codes after the given subclass marker, with consecutive 310 runs collapsed.
fn codes_after(rec: &Pairs, subclass: &str) -> Vec<i32> {
    let start = rec
        .iter()
        .position(|(code, value)| *code == 100 && value == subclass)
        .unwrap_or_else(|| panic!("subclass {subclass} missing"));
    let mut out: Vec<i32> = Vec::new();
    for (code, _) in &rec[start + 1..] {
        if *code == 310 && out.last() == Some(&310) {
            continue;
        }
        out.push(*code);
    }
    out
}

fn proxy_object(doc: &mut CadDocument, dwg_version: i32) -> Handle {
    let handle = doc.allocate_handle();
    let object = ProxyObject {
        handle,
        owner: Handle::NULL,
        proxy_id: 499,
        class_id: 501,
        dwg_version,
        maintenance_version: 106,
        payload: ProxyPayload::from_bits(&[0xAB, 0xC0], 10),
        object_ids: vec![
            ProxyObjectReference {
                handle: Handle::new(0x2A),
                kind: ProxyReferenceKind::HardPointer,
            },
            ProxyObjectReference {
                handle: Handle::new(0x2B),
                kind: ProxyReferenceKind::SoftPointer,
            },
        ],
        ..Default::default()
    };
    doc.objects.insert(handle, ObjectType::ProxyObject(object));
    handle
}

#[test]
fn proxy_object_layout_matches_autocad_per_version() {
    // (DXF version, proxied object's own format version, expected groups)
    let cases: [(DxfVersion, i32, &[i32]); 4] = [
        (DxfVersion::AC1018, 31, &[90, 91, 95, 70, 93, 310, 340, 330, 94]),
        (DxfVersion::AC1024, 23, &[90, 91, 95, 70, 161, 310, 340, 330, 94]),
        (DxfVersion::AC1024, 31, &[90, 91, 95, 70, 162, 161, 310, 340, 330, 94]),
        (DxfVersion::AC1032, 31, &[90, 91, 71, 97, 70, 162, 161, 310, 340, 330, 94]),
    ];
    for (version, object_version, expected) in cases {
        let mut doc = CadDocument::with_version(version);
        let handle = proxy_object(&mut doc, object_version);
        let rec = record(&write_pairs(&doc), handle);
        assert_eq!(
            codes_after(&rec, "AcDbProxyObject"),
            expected,
            "{version:?}, object format {object_version}"
        );
    }
}

#[test]
fn proxy_object_references_and_payload_read_back() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let handle = proxy_object(&mut doc, 31);
    let bytes = DxfWriter::new(&doc).write_to_vec().unwrap();
    let path = std::env::temp_dir().join("acadrust_proxy_object_rt.dxf");
    std::fs::write(&path, bytes).unwrap();
    let loaded = DxfReader::from_file(&path).unwrap().read().unwrap();
    let _ = std::fs::remove_file(&path);
    let Some(ObjectType::ProxyObject(object)) = loaded.objects.get(&handle) else {
        panic!("proxy object not read back");
    };
    assert_eq!(object.payload.bit_count, 10);
    assert_eq!(object.payload.data(), vec![0xAB, 0xC0]);
    let kinds: Vec<_> = object.object_ids.iter().map(|r| r.kind).collect();
    assert_eq!(
        kinds,
        [ProxyReferenceKind::HardPointer, ProxyReferenceKind::SoftPointer]
    );
}

#[test]
fn proxy_entity_graphics_are_written_once_inside_the_proxy_subclass() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let graphics = vec![0x10, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
    let mut common = EntityCommon::new();
    common.graphic_data = Some(graphics.clone());
    let entity = ExtendedEntity {
        common,
        data: ExtendedEntityData::Proxy(ProxyEntityData {
            proxy_id: 498,
            class_id: 500,
            dxf_subclass: String::new(),
            version: 0,
            dwg_version: 31,
            maintenance_version: 106,
            from_dxf: false,
            graphics: ProxyPayload::from_bytes(&graphics),
            payload: ProxyPayload::from_bits(&[0x40], 2),
            text_payload: ProxyPayload::default(),
            object_ids: Vec::new(),
        }),
    };
    let handle = doc.add_entity(EntityType::Extended(Box::new(entity))).unwrap();
    let rec = record(&write_pairs(&doc), handle);
    let entity_part: Vec<i32> = {
        let start = rec.iter().position(|(c, v)| *c == 100 && v == "AcDbEntity").unwrap();
        let end = rec.iter().position(|(c, v)| *c == 100 && v == "AcDbProxyEntity").unwrap();
        rec[start..end].iter().map(|(c, _)| *c).collect()
    };
    assert!(
        !entity_part.contains(&160) && !entity_part.contains(&310),
        "graphics leaked into AcDbEntity: {entity_part:?}"
    );
    assert_eq!(
        codes_after(&rec, "AcDbProxyEntity"),
        [90, 91, 71, 97, 70, 160, 310, 162, 161, 310, 94]
    );
}

#[test]
fn remote_text_uses_the_rtext_subclass_marker() {
    let mut doc = CadDocument::new();
    let entity = ExtendedEntity {
        common: EntityCommon::new(),
        data: ExtendedEntityData::RemoteText(RemoteTextData {
            position: Vector3::new(1.0, 2.0, 0.0),
            normal: Vector3::new(0.0, 0.0, 1.0),
            rotation: 0.0,
            height: 2.5,
            style_handle: Handle::NULL,
            style_name: "Standard".to_string(),
            flags: 1,
            text: "$(getvar, \"dwgname\")".to_string(),
        }),
    };
    let handle = doc.add_entity(EntityType::Extended(Box::new(entity))).unwrap();
    let rec = record(&write_pairs(&doc), handle);
    assert!(rec.iter().any(|(c, v)| *c == 100 && v == "RText"), "{rec:?}");
    assert!(!rec.iter().any(|(_, v)| v == "AcDbRText"));
}

#[test]
fn scale_writes_its_flag_word_before_the_name() {
    let mut doc = CadDocument::new();
    let mut scale = Scale::new("1:50", 1.0, 50.0);
    scale.handle = doc.allocate_handle();
    let handle = scale.handle;
    doc.objects.insert(handle, ObjectType::Scale(scale));
    let rec = record(&write_pairs(&doc), handle);
    let codes = codes_after(&rec, "AcDbScale");
    assert_eq!(&codes[..2], &[70, 300], "{codes:?}");
}

#[test]
fn hatch_with_a_derived_boundary_writes_pixel_size_before_seed_points() {
    let mut doc = CadDocument::new();
    let mut hatch = Hatch::new();
    let mut flags = BoundaryPathFlags::new();
    flags.set_derived(true);
    let mut path = BoundaryPath::with_flags(flags);
    for (a, b) in [((0.0, 0.0), (1.0, 0.0)), ((1.0, 0.0), (1.0, 1.0)), ((1.0, 1.0), (0.0, 0.0))] {
        path.edges.push(BoundaryEdge::Line(LineEdge {
            start: Vector2::new(a.0, a.1),
            end: Vector2::new(b.0, b.1),
        }));
    }
    hatch.paths.push(path);
    hatch.pixel_size = 0.0725;
    let handle = doc.add_entity(EntityType::Hatch(hatch)).unwrap();
    let rec = record(&write_pairs(&doc), handle);
    let p47 = rec.iter().position(|(c, _)| *c == 47).expect("47 written");
    let p98 = rec.iter().position(|(c, _)| *c == 98).expect("98 written");
    assert!(p47 < p98);
    assert_eq!(rec[p47].1.trim(), "0.0725");
}

#[test]
fn active_layout_viewports_get_ids_starting_at_one() {
    let mut doc = CadDocument::new();
    let first = doc
        .add_paper_space_entity(EntityType::Viewport(Viewport::new()))
        .unwrap();
    let second = doc
        .add_paper_space_entity(EntityType::Viewport(Viewport::new()))
        .unwrap();
    let pairs = write_pairs(&doc);
    for (handle, id) in [(first, "1"), (second, "2")] {
        let rec = record(&pairs, handle);
        let get = |code| rec.iter().find(|(c, _)| *c == code).map(|(_, v)| v.trim().to_string());
        assert_eq!(get(69).as_deref(), Some(id), "viewport id");
        assert_eq!(get(68).as_deref(), Some(id), "viewport status");
    }
}

#[test]
fn mtext_context_data_has_no_subclass_marker_and_autocad_point_codes() {
    use acadrust::objects::{MTextContext, ObjectContextData, ObjectContextKind};
    let mut doc = CadDocument::new();
    let handle = doc.allocate_handle();
    let context = ObjectContextData {
        handle,
        owner_handle: Handle::NULL,
        reactors: Vec::new(),
        xdictionary_handle: None,
        class_version: 4,
        is_default: true,
        scale: Handle::NULL,
        kind: ObjectContextKind::MText(MTextContext {
            attachment: 1,
            x_axis_dir: Vector3::new(1.0, 0.0, 0.0),
            insertion: Vector3::new(29798.0, 9841.0, 0.0),
            rect_width: 392.0,
            rect_height: 0.0,
            extents_width: 175.0,
            extents_height: 177.0,
            column_type: 0,
            columns: None,
        }),
    };
    doc.objects.insert(handle, ObjectType::ObjectContextData(context));
    let pairs = write_pairs(&doc);
    let rec = record(&pairs, handle);
    assert!(
        !rec.iter().any(|(_, v)| v == "AcDbMTextObjectContextData"),
        "AutoCAD rejects the marker: {rec:?}"
    );
    let value = |code| rec.iter().find(|(c, _)| *c == code).map(|(_, v)| v.trim().to_string());
    assert_eq!(value(10).as_deref(), Some("1.0"), "10 is the x-axis direction");
    assert_eq!(value(11).as_deref(), Some("29798.0"), "11 is the insertion point");

    let path = std::env::temp_dir().join("acadrust_mtext_context_rt.dxf");
    std::fs::write(&path, DxfWriter::new(&doc).write_to_vec().unwrap()).unwrap();
    let loaded = DxfReader::from_file(&path).unwrap().read().unwrap();
    let _ = std::fs::remove_file(&path);
    let Some(ObjectType::ObjectContextData(read)) = loaded.objects.get(&handle) else {
        panic!("context data not read back");
    };
    let ObjectContextKind::MText(m) = &read.kind else {
        panic!("wrong kind")
    };
    assert_eq!(m.insertion, Vector3::new(29798.0, 9841.0, 0.0));
    assert_eq!(m.x_axis_dir, Vector3::new(1.0, 0.0, 0.0));
}

#[test]
fn r2007_table_records_do_not_claim_the_referenced_flag() {
    // R2007+ DWG no longer stores the "referenced" (64) bit. Reading must not
    // invent it, or DXF output carries 70 = 64 and AutoCAD rejects "*Multiple"
    // VPORT records.
    let doc = CadDocument::with_version(DxfVersion::AC1032);
    let bytes = DwgWriter::write_to_vec(&doc).unwrap();
    let loaded = DwgReader::from_stream(std::io::Cursor::new(bytes))
        .read()
        .unwrap();
    for vport in loaded.vports.iter() {
        assert!(!vport.xref_reference, "{} claims the 64 flag", vport.name);
    }
}
