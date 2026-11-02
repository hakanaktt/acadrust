//! DXF output that AutoCAD rejects (or repairs on open) unless groups follow
//! the layout AutoCAD itself writes. Expected layouts were taken from
//! AutoCAD 2027 DXFOUT of the same drawings at each version.

use acadrust::entities::*;
use acadrust::objects::{
    BlockEvalValue, BlockGrip, BlockOrientedGrip, BlockVisibilityParameter, DynamicBlockData,
    DynamicBlockObject, ObjectType, ProxyObject, ProxyObjectReference, ProxyPayload,
    ProxyReferenceKind, Scale,
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
        (
            DxfVersion::AC1018,
            31,
            &[90, 91, 95, 70, 93, 310, 340, 330, 94],
        ),
        (
            DxfVersion::AC1024,
            23,
            &[90, 91, 95, 70, 161, 310, 340, 330, 94],
        ),
        (
            DxfVersion::AC1024,
            31,
            &[90, 91, 95, 70, 162, 161, 310, 340, 330, 94],
        ),
        (
            DxfVersion::AC1032,
            31,
            &[90, 91, 71, 97, 70, 162, 161, 310, 340, 330, 94],
        ),
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
        [
            ProxyReferenceKind::HardPointer,
            ProxyReferenceKind::SoftPointer
        ]
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
    let handle = doc
        .add_entity(EntityType::Extended(Box::new(entity)))
        .unwrap();
    let rec = record(&write_pairs(&doc), handle);
    let entity_part: Vec<i32> = {
        let start = rec
            .iter()
            .position(|(c, v)| *c == 100 && v == "AcDbEntity")
            .unwrap();
        let end = rec
            .iter()
            .position(|(c, v)| *c == 100 && v == "AcDbProxyEntity")
            .unwrap();
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
    let handle = doc
        .add_entity(EntityType::Extended(Box::new(entity)))
        .unwrap();
    let rec = record(&write_pairs(&doc), handle);
    assert!(
        rec.iter().any(|(c, v)| *c == 100 && v == "RText"),
        "{rec:?}"
    );
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
    for (a, b) in [
        ((0.0, 0.0), (1.0, 0.0)),
        ((1.0, 0.0), (1.0, 1.0)),
        ((1.0, 1.0), (0.0, 0.0)),
    ] {
        path.edges.push(BoundaryEdge::Line(LineEdge {
            start: Vector2::new(a.0, a.1),
            end: Vector2::new(b.0, b.1),
        }));
    }
    hatch.paths.push(path);
    hatch.pixel_size = 0.0725;
    let handle = doc.add_entity(EntityType::Hatch(Box::new(hatch))).unwrap();
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
        .add_paper_space_entity(EntityType::Viewport(Box::new(Viewport::new())))
        .unwrap();
    let second = doc
        .add_paper_space_entity(EntityType::Viewport(Box::new(Viewport::new())))
        .unwrap();
    let pairs = write_pairs(&doc);
    for (handle, id) in [(first, "1"), (second, "2")] {
        let rec = record(&pairs, handle);
        let get = |code| {
            rec.iter()
                .find(|(c, _)| *c == code)
                .map(|(_, v)| v.trim().to_string())
        };
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
        has_file_to_extension_dictionary: true,
        uses_legacy_mleader_context_base: false,
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
    doc.objects
        .insert(handle, ObjectType::ObjectContextData(context));
    let pairs = write_pairs(&doc);
    let rec = record(&pairs, handle);
    assert!(
        !rec.iter().any(|(_, v)| v == "AcDbMTextObjectContextData"),
        "AutoCAD rejects the marker: {rec:?}"
    );
    let value = |code| {
        rec.iter()
            .find(|(c, _)| *c == code)
            .map(|(_, v)| v.trim().to_string())
    };
    assert_eq!(
        value(10).as_deref(),
        Some("1.0"),
        "10 is the x-axis direction"
    );
    assert_eq!(
        value(11).as_deref(),
        Some("29798.0"),
        "11 is the insertion point"
    );

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
fn text_context_data_has_no_subclass_marker() {
    use acadrust::objects::{ObjectContextData, ObjectContextKind};
    let mut doc = CadDocument::new();
    let handle = doc.allocate_handle();
    let context = ObjectContextData {
        handle,
        owner_handle: Handle::NULL,
        reactors: Vec::new(),
        xdictionary_handle: None,
        class_version: 4,
        has_file_to_extension_dictionary: true,
        uses_legacy_mleader_context_base: false,
        is_default: true,
        scale: Handle::NULL,
        kind: ObjectContextKind::Text {
            horizontal_mode: 1,
            rotation: 0.0,
            insertion: Vector2::new(12.0, -30.0),
            alignment: Vector2::new(40.0, -30.0),
        },
    };
    doc.objects
        .insert(handle, ObjectType::ObjectContextData(context));
    let pairs = write_pairs(&doc);
    let rec = record(&pairs, handle);
    assert!(
        !rec.iter().any(|(_, v)| v == "AcDbTextObjectContextData"),
        "AutoCAD rejects the marker: {rec:?}"
    );
    assert_eq!(
        codes_after(&rec, "AcDbAnnotScaleObjectContextData"),
        vec![340, 70, 50, 10, 20, 11, 21],
        "AutoCAD's layout"
    );

    let path = std::env::temp_dir().join("acadrust_text_context_rt.dxf");
    std::fs::write(&path, DxfWriter::new(&doc).write_to_vec().unwrap()).unwrap();
    let loaded = DxfReader::from_file(&path).unwrap().read().unwrap();
    let _ = std::fs::remove_file(&path);
    let Some(ObjectType::ObjectContextData(read)) = loaded.objects.get(&handle) else {
        panic!("context data not read back");
    };
    let ObjectContextKind::Text {
        horizontal_mode,
        insertion,
        alignment,
        ..
    } = &read.kind
    else {
        panic!("wrong kind")
    };
    assert_eq!(*horizontal_mode, 1);
    assert_eq!(*insertion, Vector2::new(12.0, -30.0));
    assert_eq!(*alignment, Vector2::new(40.0, -30.0));
}

/// Writes one context-data leaf, checks AutoCAD's layout (no own subclass
/// marker, `codes` straight after the annotation scale groups) and returns
/// the leaf read back.
fn context_leaf_roundtrip(
    kind: acadrust::objects::ObjectContextKind,
    marker: &str,
    codes: &[i32],
) -> acadrust::objects::ObjectContextKind {
    use acadrust::objects::ObjectContextData;
    let mut doc = CadDocument::new();
    let handle = doc.allocate_handle();
    let context = ObjectContextData {
        handle,
        owner_handle: Handle::NULL,
        reactors: Vec::new(),
        xdictionary_handle: None,
        class_version: 4,
        has_file_to_extension_dictionary: true,
        uses_legacy_mleader_context_base: false,
        is_default: true,
        scale: Handle::NULL,
        kind,
    };
    doc.objects
        .insert(handle, ObjectType::ObjectContextData(context));
    let rec = record(&write_pairs(&doc), handle);
    assert!(
        !rec.iter().any(|(_, v)| v == marker),
        "AutoCAD rejects the marker: {rec:?}"
    );
    assert_eq!(
        codes_after(&rec, "AcDbAnnotScaleObjectContextData"),
        codes,
        "AutoCAD's layout"
    );

    let bytes = DxfWriter::new(&doc).write_to_vec().unwrap();
    let loaded = DxfReader::from_reader(std::io::Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap();
    let Some(ObjectType::ObjectContextData(read)) = loaded.objects.get(&handle) else {
        panic!("context data not read back");
    };
    read.kind.clone()
}

#[test]
fn blkref_context_data_has_no_subclass_marker() {
    use acadrust::objects::ObjectContextKind;
    let read = context_leaf_roundtrip(
        ObjectContextKind::BlkRef {
            rotation: 0.5,
            insertion: Vector3::new(1.0, 2.0, 0.0),
            scale_factor: Vector3::new(2.0, 2.0, 2.0),
        },
        "AcDbBlkRefObjectContextData",
        &[340, 50, 10, 20, 30, 41, 42, 43],
    );
    let ObjectContextKind::BlkRef {
        rotation,
        insertion,
        scale_factor,
    } = read
    else {
        panic!("wrong kind")
    };
    assert_eq!(rotation, 0.5);
    assert_eq!(insertion, Vector3::new(1.0, 2.0, 0.0));
    assert_eq!(scale_factor, Vector3::new(2.0, 2.0, 2.0));
}

#[test]
fn leader_context_data_has_no_subclass_marker() {
    use acadrust::objects::{LeaderContext, ObjectContextKind};
    let leader = LeaderContext {
        points: vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 5.0, 0.0),
            Vector3::new(15.0, 5.0, 0.0),
        ],
        x_direction: Vector3::new(1.0, 0.0, 0.0),
        annotation_enabled: true,
        insertion_offset: Vector3::new(0.5, 0.0, 0.0),
        endpoint_projection: Vector3::new(0.0, 0.25, 0.0),
    };
    let read = context_leaf_roundtrip(
        ObjectContextKind::Leader(leader.clone()),
        "AcDbLeaderObjectContextData",
        &[
            340, 70, 10, 20, 30, 10, 20, 30, 10, 20, 30, 11, 21, 31, 290, 12, 22, 32, 13, 23, 33,
        ],
    );
    let ObjectContextKind::Leader(read) = read else {
        panic!("wrong kind")
    };
    assert_eq!(read, leader);
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

fn visibility_parameter(
    doc: &mut CadDocument,
    eval_value_code: i16,
    eval_value: BlockEvalValue,
) -> Handle {
    let handle = doc.allocate_handle();
    let object = BlockVisibilityParameter {
        handle,
        owner: Handle::NULL,
        eval_major: 33,
        eval_minor: 378,
        eval_value_code,
        eval_value,
        eval_node_id: 1,
        element_name: "Visibility State".to_string(),
        element_major: 33,
        element_minor: 378,
        name: "Visibility1".to_string(),
        ..Default::default()
    };
    doc.objects
        .insert(handle, ObjectType::BlockVisibilityParameter(object));
    handle
}

#[test]
fn visibility_eval_expr_without_value_writes_nothing_after_node_ids() {
    // AutoCAD DXFOUT (2018) of a visibility-bearing dynamic block ends
    // AcDbEvalExpr after 90/98/99; the shared reader treats an absent 70 as
    // the -9999 "no value" sentinel. Emitting "70 -9999" makes AutoCAD reject
    // the object with "Unexpected DXF group code: 70".
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let handle = visibility_parameter(&mut doc, -9999, BlockEvalValue::None);
    let codes = codes_after(&record(&write_pairs(&doc), handle), "AcDbEvalExpr");
    assert_eq!(&codes[..4], &[90, 98, 99, 100], "{codes:?}");
}

#[test]
fn visibility_eval_expr_with_value_matches_parameter_layout() {
    // AcDbEvalExpr with a value carries an empty expression string, the value
    // type, then the value - the layout every other parameter writes and the
    // shared reader accepts (a real value is group 140, not 40).
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let handle = visibility_parameter(&mut doc, 40, BlockEvalValue::Real(6.5));
    let codes = codes_after(&record(&write_pairs(&doc), handle), "AcDbEvalExpr");
    assert_eq!(&codes[..7], &[90, 98, 99, 1, 70, 140, 100], "{codes:?}");
}

fn alignment_grip(doc: &mut CadDocument) -> Handle {
    let handle = doc.allocate_handle();
    let object = DynamicBlockObject {
        handle,
        owner: Handle::NULL,
        dxf_name: "BLOCKALIGNMENTGRIP".to_string(),
        cpp_class_name: "AcDbBlockAlignmentGrip".to_string(),
        data: DynamicBlockData::AlignmentGrip(BlockOrientedGrip {
            grip: BlockGrip::default(),
            orientation: Vector3::new(1.0, 0.0, 0.0),
        }),
        ..Default::default()
    };
    doc.objects.insert(handle, ObjectType::DynamicBlock(object));
    handle
}

#[test]
fn alignment_grip_orientation_uses_sequential_group_codes() {
    // The reference writer emits AcDbBlockAlignmentGrip's orientation as the
    // sequential trio 140/141/142. Deriving y/z as base+10/base+20 (the
    // ordinary point convention) emits 140/150/160 instead, which a conforming
    // reader rejects with "DXF read error" inside BLOCKALIGNMENTGRIP and then
    // discards the whole drawing.
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let handle = alignment_grip(&mut doc);
    let codes = codes_after(
        &record(&write_pairs(&doc), handle),
        "AcDbBlockAlignmentGrip",
    );
    assert_eq!(&codes[..3], &[140, 141, 142], "{codes:?}");
}

#[test]
fn wipeout_polygonal_clip_boundary_writes_a_closed_ring() {
    // The reference writer closes a polygonal clip boundary in DXF: a
    // four-vertex ring is written as five points with the first repeated, so
    // 91 reads 5. Writing the unclosed ring leaves the reader short of a point,
    // which surfaces as "Xdata wasn't read" in WIPEOUT and a discarded drawing.
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut wipeout = Wipeout::new();
    wipeout.clip_type = WipeoutClipType::Polygonal;
    wipeout.clip_boundary_vertices = vec![
        Vector2::new(-0.5, -0.5),
        Vector2::new(0.5, -0.5),
        Vector2::new(0.5, -0.394),
        Vector2::new(-0.5, -0.394),
    ];
    let handle = doc.add_entity(EntityType::Wipeout(wipeout)).unwrap();
    let rec = record(&write_pairs(&doc), handle);
    let count = rec
        .iter()
        .position(|(code, _)| *code == 91)
        .expect("clip boundary count written");
    assert_eq!(
        rec[count].1.trim(),
        "5",
        "polygonal clip boundary must be closed: {rec:?}"
    );
    let xs: Vec<&str> = rec
        .iter()
        .filter(|(code, _)| *code == 14)
        .map(|(_, value)| value.trim())
        .collect();
    let ys: Vec<&str> = rec
        .iter()
        .filter(|(code, _)| *code == 24)
        .map(|(_, value)| value.trim())
        .collect();
    assert_eq!(xs.len(), 5, "{xs:?}");
    assert_eq!((xs[4], ys[4]), (xs[0], ys[0]), "ring must close: {xs:?}");
}
