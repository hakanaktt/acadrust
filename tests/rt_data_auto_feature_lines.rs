//! The corridor-generated feature line (`AECC_AUTO_FEATURE_LINE`, class
//! 0x12f8): the record pinned bit for bit from Civil 3D 2027, captured
//! 2026-11-28 from Corridor-2a.dwg (Civil Tutorials Drawings; the corpus's
//! only two records — one in Corridor-2a, one in Corridor-2b — are
//! byte-identical, md5 a128246d5199bcfddcf66a98b595bb2a, 877 bytes). The
//! record references its site container twice (handle 0x1306, the
//! "Corridor Targets" parcel 0x1305's container) and names the container
//! edge that carries its points (edge 3), exactly the regular feature
//! line's shape; its display wireframe rides in the proxy-graphics blob
//! the shared envelope already decodes. The site container's own edges
//! head is the corridor round's named residual (`topology()` refuses at
//! the `BL 1, BL 1` head), so the chain from this record to points stops
//! at the edge id — pinned as refused, not silently dropped.

use acadrust::classes::arch::ArchEntityData;
use acadrust::types::{DxfVersion, Handle};

/// The application's auto feature line "Feature 1" (creator
/// "Administrator", layer C-TOPO-GRAD-FLIN), handle 0x12f8, owner 0x1f;
/// entity type code 897, handle bits 132 as captured.
const AUTO_FL: &str = "a040c084be2ab802b802000008000000b0000000240000009aa3a5ef0dc9b140d11aef7ce354b040\
    000000000000000000000000000000000000000000000000000000000000f03f000000000000f03f\
    000000000000000000000000000000009a9999999999c93f000000000000f03f0000000000000000\
    4100650063006300440062004100750074006f0046006500610074007500720065004c0069006e00\
    65002000280041006500630063004c0061006e006400310033003000290000008400000006000000\
    05000000000a0c56f4c8b140378155e3c954b040000000000000000000e632de7babb440378155e3\
    c954b040000000000000000000e632de7babb44000cc14065926b3400000000000000000000a0c56\
    f4c8b14000cc14065926b3400000000000000000000a0c56f4c8b140378155e3c954b04000000000\
    0000000084000000060000000500000000e632de7babb44000cc14065926b340004080ce389e8340\
    00e632de7babb440378155e3c954b040004080ce389e8340000a0c56f4c8b140378155e3c954b040\
    004080ce389e8340000a0c56f4c8b14000cc14065926b340004080ce389e834000e632de7babb440\
    00cc14065926b340004080ce389e83403c000000060000000200000000e632de7babb44000cc1406\
    5926b340004080ce389e834000e632de7babb44000cc14065926b34000000000000000003c000000\
    060000000200000000e632de7babb440378155e3c954b040004080ce389e834000e632de7babb440\
    378155e3c954b04000000000000000003c0000000600000002000000000a0c56f4c8b140378155e3\
    c954b040004080ce389e8340000a0c56f4c8b140378155e3c954b04000000000000000003c000000\
    0600000002000000000a0c56f4c8b14000cc14065926b340004080ce389e8340000a0c56f4c8b140\
    00cc14065926b34000000000000000000800000015000000ab400010eaa12a0a84022f81082a0408\
    08000203e20002c61c32637fad190028bc0108013c016000bc0104013001300240f487b14ae47e17\
    a843f2485f0222c8200ca00c600c6009200da00e0008200ea00e800de008c00ca00c200e800ea00e\
    400ca009800d200dc00ca01486a0803200368034803700348039803a00390030803a003780390021\
    28c00ca00c200e800ea00e400ca0040006201c4055212f6405213065213064052082d5050f";

fn record(dxf: &str) -> ArchEntityData {
    let bytes: Vec<u8> = (0..AUTO_FL.len() / 2)
        .map(|i| u8::from_str_radix(&AUTO_FL[2 * i..2 * i + 2], 16).expect("hex"))
        .collect();
    ArchEntityData {
        dxf_name: dxf.to_string(),
        cpp_class_name: "AeccDbAutoFeatureLine".to_string(),
        kind: acadrust::classes::arch::ArchKind::Other,
        profiles: Vec::new(),
        dwg_type_code: 897,
        dwg_handle_bits: 132,
        dwg_source_version: Some(DxfVersion::AC1032),
        raw_dwg_data: Some(bytes),
        raw_dxf_codes: None,
        fields: Vec::new(),
        preview: Vec::new(),
    }
}

/// The keyed-form head reads through: the container handle (0x1306) and
/// the edge id (3, the one edge the corridor site container's graph
/// holds), and the name/creator strings from the text stream.
#[test]
fn the_corridor_record_reads_container_edge_and_name() {
    let r = record("AECC_AUTO_FEATURE_LINE");
    assert_eq!(r.auto_feature_line_container(), Some(0x1306));
    assert_eq!(r.auto_feature_line_edge(), Some(3));
    assert_eq!(r.auto_feature_line_name(), Some(("Feature 1".into(), "Administrator".into())));
    // The envelope the readers build on, pinned: class tag string, the
    // container twice, then the style handle.
    let d = r.decode();
    assert_eq!(d.strings[0], "AeccImpAutoFeatureLine");
    assert_eq!(d.handles, vec![0, 0x1306, 0x1306, 0, 0x82D, 0, 0]);
}

/// A regular feature line is not an auto feature line: the class gate
/// refuses, and a truncated record refuses gracefully rather than
/// guessing.
#[test]
fn the_gates_refuse() {
    let regular = record("AECC_FEATURE_LINE");
    assert_eq!(regular.auto_feature_line_container(), None);
    assert_eq!(regular.auto_feature_line_edge(), None);
    assert_eq!(regular.auto_feature_line_name(), None);

    let mut truncated = record("AECC_AUTO_FEATURE_LINE");
    truncated.raw_dwg_data.as_mut().unwrap().truncate(60);
    assert_eq!(truncated.auto_feature_line_edge(), None, "a record cut before the class head refuses");
}

/// The "Corridor Targets" site container (0x1306, owner the parcel
/// 0x1305) the record points at, pinned bit for bit from the same file
/// (206 bytes, md5 003f727d6cec1ef5cad5b28ce9a5dc1b; object handle bits
/// 75 as captured). Its head, table, elements, gate and node list are the
/// shipped grammar's; the edges head `BL 1, BL 1` is the corridor round's
/// named residual — `topology()` refuses, so this pins the refusal the
/// auto feature line's edge id 3 dead-ends in.
const CORRIDOR_CONTAINER: &str = "650084c1aa2f812a481d20320300200010083a0a80801484a8373556480003d0e0e917ad82049020\
    000ae85be272cd0000a6ea18a6f6b14092060073196f3dd5da2000198280cb24d6680002040671c4\
    f41a02410000a0c56f4c8b14000041febc1d56c1024828001d6e93b3b76280005a5666b22f58204a\
    405019030c020040105a4c9034050193240901405010c8003c0501405011d0ab34ee8b16defe9024\
    15014050240904aaa40901405014190140e9015910159101591015910159101592aaa0200c021097\
    c20210982a07";

#[test]
fn the_corridor_site_container_s_edges_head_stays_refused() {
    let mut o = acadrust::classes::arch::ArchObject::new(
        "AECC_FEATURE_STYLE",
        Handle::new(0x1306),
        Handle::new(0x1305),
    );
    o.raw_dwg_data = Some(
        (0..CORRIDOR_CONTAINER.len() / 2)
            .map(|i| u8::from_str_radix(&CORRIDOR_CONTAINER[2 * i..2 * i + 2], 16).expect("hex"))
            .collect(),
    );
    o.raw_dwg_handle_bits = 75;
    assert!(o.topology().is_none(), "the BL 1, BL 1 edges head refuses (the corridor round's named residual)");
}

/// The corridor feature line's points: the live-API harvest (D010 bridge
/// round, 2026-11-28, oracle Corridor-2a.dwg opened readOnly) read
/// Feature 1's `GetPoints(FeatureLinePointType.AllPoints)` as exactly
/// five PIs — the container edge's five nodes (the edge is arc + 3
/// tangents, the arc's bulge on its third vertex). The record keeps no
/// geometry; the read routes through the referenced site container's
/// edge (id 3), whose `BL 1, BL 1` edges head the general topology
/// reader refuses — this is the auto feature line's own route, keyed by
/// the line's edge id and validated to the record end. Values below are
/// the live PIs bit for bit.
#[test]
fn the_corridor_feature_line_s_vertices_are_the_container_edge_s_nodes() {
    let mut doc = acadrust::CadDocument::new();
    let mut common = acadrust::entities::EntityCommon::default();
    common.handle = doc.allocate_handle();
    let fl = doc
        .add_entity(acadrust::entities::EntityType::Extended(Box::new(acadrust::entities::ExtendedEntity {
            common,
            data: acadrust::entities::ExtendedEntityData::Arch(record("AECC_AUTO_FEATURE_LINE")),
        })))
        .expect("the auto feature line entity");
    let mut o = acadrust::classes::arch::ArchObject::new(
        "AECC_FEATURE_STYLE",
        Handle::new(0x1306),
        Handle::new(0x1305),
    );
    o.raw_dwg_data = Some(
        (0..CORRIDOR_CONTAINER.len() / 2)
            .map(|i| u8::from_str_radix(&CORRIDOR_CONTAINER[2 * i..2 * i + 2], 16).expect("hex"))
            .collect(),
    );
    o.raw_dwg_handle_bits = 75;
    doc.objects.insert(Handle::new(0x1306), acadrust::objects::ObjectType::Arch(o));

    let verts = acadrust::classes::civil::auto_feature_line_vertices(&doc, fl)
        .expect("the corridor feature line's vertices via its container edge");
    let expected: [( (f64, f64, f64), f64); 5] = [
        ((5291.483859234955, 4902.347749042325, 627.7777376193553), 0.0),
        ((5020.969096780289, 4598.648817697074, 0.0), 0.0),
        ((4778.723650314845, 4341.133256702218, 0.0), -0.09285940132137133),
        ((4571.848781052511, 4190.393763345666, 0.0), 0.0),
        ((4552.954437973443, 4181.030021611601, 0.0), 0.0),
    ];
    assert_eq!(verts.len(), expected.len());
    for (v, (p, bulge)) in verts.iter().zip(expected) {
        assert_eq!((v.point.x, v.point.y, v.point.z), p, "PI does not match the live GetPoints value");
        assert_eq!(v.bulge, bulge);
        assert!(!v.elevation_point);
    }
}

/// The read is graceful: without the referenced container in the
/// document, or for a record that is not an auto feature line, it is
/// `None` — never a guess.
#[test]
fn the_vertices_read_refuses_without_the_container_or_the_class() {
    let mut doc = acadrust::CadDocument::new();
    let mut common = acadrust::entities::EntityCommon::default();
    common.handle = doc.allocate_handle();
    let fl = doc
        .add_entity(acadrust::entities::EntityType::Extended(Box::new(acadrust::entities::ExtendedEntity {
            common,
            data: acadrust::entities::ExtendedEntityData::Arch(record("AECC_AUTO_FEATURE_LINE")),
        })))
        .expect("the auto feature line entity");
    assert!(acadrust::classes::civil::auto_feature_line_vertices(&doc, fl).is_none());

    let mut common = acadrust::entities::EntityCommon::default();
    common.handle = doc.allocate_handle();
    let not_auto = doc
        .add_entity(acadrust::entities::EntityType::Extended(Box::new(acadrust::entities::ExtendedEntity {
            common,
            data: acadrust::entities::ExtendedEntityData::Arch(record("AECC_FEATURE_LINE")),
        })))
        .expect("the regular feature line entity");
    let mut o = acadrust::classes::arch::ArchObject::new(
        "AECC_FEATURE_STYLE",
        Handle::new(0x1306),
        Handle::new(0x1305),
    );
    o.raw_dwg_data = Some(
        (0..CORRIDOR_CONTAINER.len() / 2)
            .map(|i| u8::from_str_radix(&CORRIDOR_CONTAINER[2 * i..2 * i + 2], 16).expect("hex"))
            .collect(),
    );
    o.raw_dwg_handle_bits = 75;
    doc.objects.insert(Handle::new(0x1306), acadrust::objects::ObjectType::Arch(o));
    assert!(acadrust::classes::civil::auto_feature_line_vertices(&doc, not_auto).is_none());
}

/// A container edge id the record does not name refuses: the single slot
/// must carry the referencing line's own edge id.
#[test]
fn the_corridor_edge_read_refuses_another_edge_id() {
    let mut o = acadrust::classes::arch::ArchObject::new(
        "AECC_FEATURE_STYLE",
        Handle::new(0x1306),
        Handle::new(0x1305),
    );
    o.raw_dwg_data = Some(
        (0..CORRIDOR_CONTAINER.len() / 2)
            .map(|i| u8::from_str_radix(&CORRIDOR_CONTAINER[2 * i..2 * i + 2], 16).expect("hex"))
            .collect(),
    );
    o.raw_dwg_handle_bits = 75;
    assert!(o.corridor_edge_vertices(4).is_none(), "no slot with edge id 4");
}
