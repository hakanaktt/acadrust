//! Family 7 — feature lines on a site: arcs, elevation points and
//! crossings (civil-write.md §3g "Feature lines that cross", samples
//! WF20/WF21/WF24/WF29, FX00–FX03, E-FE1/E-FE2). READY-WITH-LIMITS: the
//! region-closing crossing case is refused by the writer (the application
//! would add a grading for the closed face) and is out of scope, as are
//! the application's own marks on files it edited (an edge's flag 0x800
//! with its last point's 8, the newest member's 0x10/0x20 while it is the
//! newest). Everything asserted below is the writer's own documented
//! output, exact: node ids, elevations, arc bulges, elevation-point keys,
//! the open edge's last-point flags 10, the crossing node shared by both
//! lines with the new line's elevation and the crossed edge's 0x80 mark
//! and 0x400 crossing points.

#[path = "rt_data/mod.rs"]
mod rt_data;

use acadrust::classes::civil::{add_feature_line_vertices, add_site, FeatureLineVertex};
use acadrust::types::{Handle, Vector2, Vector3};

fn site_of(back: &acadrust::CadDocument, name: &str) -> Handle {
    for (h, d) in rt_data::rt_data_entities(back, "AECC_PARCEL") {
        if let Some((n, _)) = d.face_name() {
            if n == name {
                return h;
            }
        }
    }
    panic!("site {name} missing");
}

fn container_of(back: &acadrust::CadDocument, site: Handle) -> acadrust::classes::arch::ArchObject {
    rt_data::rt_data_objects(back, "AECC_FEATURE_STYLE")
        .into_iter()
        .find(|c| c.owner == site)
        .unwrap_or_else(|| panic!("no container owned by the site"))
}

const ARC_EDGE: i32 = 0x010F_0000;
const ELEVATION_POINT: i32 = 0x40_0400;

fn vertex(point: (f64, f64, f64), bulge: f64, elevation_point: bool) -> FeatureLineVertex {
    FeatureLineVertex { point: Vector3::new(point.0, point.1, point.2), bulge, elevation_point }
}

#[test]
fn arcs_and_elevation_points_read_back_exact() {
    let mut doc = rt_data::rt_data_drawing();
    let site = add_site(&mut doc, "S1", "lines").unwrap();
    // FL1: an elevation point between two PIs (straight segments).
    let fl1 = add_feature_line_vertices(
        &mut doc,
        site,
        "FL1",
        &[vertex((0.0, 0.0, 10.0), 0.0, false), vertex((50.0, 0.0, 11.25), 0.0, true), vertex((100.0, 0.0, 13.0), 0.0, false)],
        None,
    )
    .unwrap();
    // FL2: an arc of bulge 0.5 out, an arc of -0.3 back.
    let fl2 = add_feature_line_vertices(
        &mut doc,
        site,
        "FL2",
        &[vertex((200.0, 0.0, 8.0), 0.5, false), vertex((250.0, 50.0, 9.0), -0.3, false), vertex((300.0, 0.0, 8.5), 0.0, false)],
        None,
    )
    .unwrap();

    let back = rt_data::rt_data_roundtrip(&doc);
    let c = container_of(&back, site_of(&back, "S1"));
    assert_eq!(c.site_members(), vec![fl1.value(), fl2.value()], "both lines in edge order");

    let t = c.topology().expect("topology");
    assert_eq!(t.edges.len(), 2);
    assert_eq!(t.edge_flags, vec![0x1_0030, 0x1_0030], "feature line edges carry 0x10030");

    // FL1: the ends take ids 1 and 2, the elevation point 3; the
    // positions and elevations bit-exact.
    let e1 = &t.edges[0];
    assert_eq!(e1.iter().map(|v| v.node).collect::<Vec<_>>(), vec![1, 3, 2]);
    assert_eq!(e1[0].point, Vector2::new(0.0, 0.0));
    assert_eq!(e1[1].point, Vector2::new(50.0, 0.0));
    assert_eq!(e1[2].point, Vector2::new(100.0, 0.0));
    assert_eq!((e1[0].z, e1[1].z, e1[2].z), (10.0, 11.25, 13.0));
    // The elevation point: its flag, its key (the writer keys the
    // elevation points 1..k through the members in edge order); no
    // arcs on this line.
    assert!(e1[1].is_elevation_point());
    assert!(!e1[0].is_elevation_point() && !e1[2].is_elevation_point());
    assert_eq!(e1[1].key, 1);
    assert_eq!(e1[0].key, 0);
    assert_eq!(e1[2].key, 0);
    assert_eq!(e1[1].flags & ELEVATION_POINT, ELEVATION_POINT, "the elevation point's flag");
    assert_eq!(e1[1].flags & ARC_EDGE, 0);
    for v in e1 {
        assert_eq!(v.bulge, 0.0);
    }
    // An open edge's last point's flags are 10.
    assert_eq!(e1[2].flags, 10);
    assert_eq!(t.next_key, 2, "one elevation point");
    assert_eq!(t.next_node, 7, "FL2's three nodes took 4, 5 and 6 when it joined");

    // FL2's nodes: 4 and 5 the ends, 6 the inner PI; the first point of
    // the edge after the first carries 0x80.
    let e2 = &t.edges[1];
    assert_eq!(e2.iter().map(|v| v.node).collect::<Vec<_>>(), vec![4, 6, 5]);
    assert_eq!((e2[0].z, e2[1].z, e2[2].z), (8.0, 9.0, 8.5));
    // The arc bulges are stored as written, on the segment that starts
    // there, with the arc flag on its start point.
    assert_eq!(e2[0].bulge, 0.5);
    assert_eq!(e2[1].bulge, -0.3);
    assert_eq!(e2[2].bulge, 0.0);
    assert_eq!(e2[0].flags & ARC_EDGE, ARC_EDGE, "the first arc's flag on its start point");
    assert_eq!(e2[1].flags & ARC_EDGE, ARC_EDGE, "the second arc's flag");
    assert_eq!(e2[2].flags & ARC_EDGE, 0, "no segment starts at the last point");
    assert_eq!(e2[0].flags & 0x80, 0x80, "the first point of the edge after the first");
    assert_eq!(e2[2].flags, 10, "the open edge's last point");
    // An elevation point exists, so the outside face loses flag 8: 6.
    assert_eq!(t.outside_face_flags, 6);
}

#[test]
fn a_crossing_takes_the_new_line_s_elevation_and_node() {
    let mut doc = rt_data::rt_data_drawing();
    let site = add_site(&mut doc, "S1", "crossing").unwrap();
    // FX00's FL1 and FX03's crossing line: FL1 is crossed between its
    // PIs, at (25, 0), where the new line is at elevation 8.5.
    add_feature_line_vertices(
        &mut doc,
        site,
        "FL1",
        &[vertex((0.0, 0.0, 10.0), 0.0, false), vertex((50.0, 0.0, 12.0), 0.0, false), vertex((100.0, 30.0, 11.0), 0.0, false)],
        None,
    )
    .unwrap();
    add_feature_line_vertices(&mut doc, site, "FL2", &[vertex((25.0, -20.0, 8.0), 0.0, false), vertex((25.0, 20.0, 9.0), 0.0, false)], None).unwrap();

    let back = rt_data::rt_data_roundtrip(&doc);
    let t = container_of(&back, site_of(&back, "S1")).topology().expect("topology");
    assert_eq!(t.edges.len(), 2);

    // FL1 is split at the crossing: four points now.
    let e1 = &t.edges[0];
    assert_eq!(e1.len(), 4);
    let e2 = &t.edges[1];
    assert_eq!(e2.len(), 3);

    // The crossing node: one node both edges share, at the crossing
    // point, with the new line's elevation there (8.5 on both lines, not
    // FL1's interpolated 11).
    let cross1 = e1.iter().find(|v| v.point == Vector2::new(25.0, 0.0)).expect("FL1 runs through the crossing point");
    let cross2 = e2.iter().find(|v| v.point == Vector2::new(25.0, 0.0)).expect("the new line's middle node");
    assert_eq!(cross1.z, 8.5, "the crossing takes the new line's elevation");
    assert_eq!(cross2.z, 8.5);
    assert_eq!(cross1.node, cross2.node, "one shared node");

    // Node ids: FL1's ends 1 and 2, its inner PI 3 (from its first
    // write); the new line's own points 4 and 5, the crossing node 6.
    assert_eq!(e1.iter().map(|v| v.node).collect::<Vec<_>>(), vec![1, 6, 3, 2]);
    assert_eq!(e2.iter().map(|v| v.node).collect::<Vec<_>>(), vec![4, 6, 5]);
    assert_eq!(t.next_node, 7);

    // FL1's own elevations survive; FL2's ends keep theirs.
    assert_eq!((e1[0].z, e1[2].z, e1[3].z), (10.0, 12.0, 11.0));
    assert_eq!((e2[0].z, e2[2].z), (8.0, 9.0));

    // The crossing points are marked as inserted (0x400 instead of 2),
    // with the 0x30 of a following segment; the open edges' last points
    // are 10.
    assert_eq!(cross1.flags, 0x400 | 0x30);
    assert_eq!(cross2.flags, 0x400 | 0x30);
    assert_eq!(e1[3].flags, 10);
    assert_eq!(e2[2].flags, 10);
    for v in e1.iter().chain(e2) {
        assert!(!v.is_elevation_point());
        assert_eq!(v.key, 0);
        assert_eq!(v.bulge, 0.0);
    }
    // The crossed edge carries the 0x80 mark; both are feature lines.
    assert_eq!(t.edge_flags[0], 0x1_0030 | 0x80, "the crossed edge's mark");
    assert_eq!(t.edge_flags[1] & 0x1_0030, 0x1_0030);
    // The new line ran into the outside face, so it loses flag 8: 6.
    assert_eq!(t.outside_face_flags, 6);
    assert_eq!(t.next_key, 1);
}

#[test]
fn the_feature_line_section_of_a_container_this_crate_writes_is_its_first() {
    let mut doc = rt_data::rt_data_drawing();
    let site = add_site(&mut doc, "S2", "lines").unwrap();
    let fl1 = add_feature_line_vertices(
        &mut doc,
        site,
        "FL1",
        &[vertex((0.0, 0.0, 4.0), 0.0, false), vertex((30.0, 0.0, 5.0), 0.0, false)],
        None,
    )
    .unwrap();
    let fl2 = add_feature_line_vertices(
        &mut doc,
        site,
        "FL2",
        &[vertex((0.0, 10.0, 6.0), 0.0, false), vertex((30.0, 10.0, 7.0), 0.0, false)],
        None,
    )
    .unwrap();

    let back = rt_data::rt_data_roundtrip(&doc);
    let c = container_of(&back, site_of(&back, "S2"));
    // A container the crate writes keeps no second section: the feature
    // lines read through the first one, whole.
    let t = c.topology().expect("topology");
    let ft = c.feature_line_topology().expect("feature-line topology");
    assert_eq!(ft.edges, t.edges);
    assert_eq!(ft.edge_ids, t.edge_ids);
    assert_eq!(ft.next_node, t.next_node);
    let verts = acadrust::classes::civil::feature_line_vertices(&back, fl1).expect("FL1's vertices");
    assert_eq!(verts.iter().map(|v| (v.point.x, v.point.y, v.point.z)).collect::<Vec<_>>(), vec![(0.0, 0.0, 4.0), (30.0, 0.0, 5.0)]);
    let verts = acadrust::classes::civil::feature_line_vertices(&back, fl2).expect("FL2's vertices");
    assert_eq!(verts.iter().map(|v| (v.point.x, v.point.y, v.point.z)).collect::<Vec<_>>(), vec![(0.0, 10.0, 6.0), (30.0, 10.0, 7.0)]);
}

/// The application's own site container for the controlled crossing pair
/// (C3D 2027, captured 2026-11-26): site "OR_S" with feature lines
/// "OR_FL1" (100, 100)–(500, 100) and "OR_FL2", built in one command each.
/// `OR_FL2` runs (150, 200)–(450, 200) beside the first line in the first
/// record and (150, 50)–(450, 200) across it (one crossing, at x = 250) in
/// the second; the container is the only record that differs. The records
/// are pinned bit for bit: the container handle 0x72D1 (owner the site,
/// 0x72D0) and its 40-bit record header's handle bits as captured.
const APP_PLAIN_PAIR: &str = "7c809cb46a2f812a481d20320300200010082a08808000000000000b2800000000000002ca049020000000000101fd000000000000005940920600000000006031200000000000000d28124100000000000207c400000000000001a50252068148086000020081526480a0280a0485205555204180000802054c120680a02812148255520480a0280a0c8120320722c80a0280a0280a0280a0280a0280a0280a03555010060109cb4d0109cb550109cb4101414148822d48821148820d4881f54882394882294881f948822548821d414882354881f14882054881fd4881ed48821548823148820148822148288bf";
const APP_CROSSED_PAIR: &str = "7c809cb46a2f812a481d2032030020001008320a808000000000000b2800000000000002ca049020000000000101fd000000000000005940920600000000006031200000000000000928124100000000000207c400000000000001a5024828000000000080de800000000000002ca04a40d029012c000040103a4c90140501830040000415014050242902aaa9022c000040103a6c90340501830040000415014050242904aaa409014050141901406459014050140501405014050140501406aaa0200c0213969a021396aa0213968202828291045a91042291041a9103ea9104729104529103f291044a91043a8291046a9103e291040a9103fa9103da91042a910462910402910442905117";

fn app_container(hex: &str, handle_bits: i64) -> acadrust::classes::arch::ArchObject {
    let mut o = acadrust::classes::arch::ArchObject::new("AECC_FEATURE_STYLE", Handle::new(0x72D1), Handle::new(0x72D0));
    o.raw_dwg_data = Some((0..hex.len() / 2).map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).expect("hex")).collect());
    o.raw_dwg_handle_bits = handle_bits;
    o
}

/// The application's two records read through the strict head path, field
/// for field: the second line's crossing splits both edges at one shared
/// node (the crossing point) and marks them, and nothing else moves — the
/// elevation-point table stays empty (the crossing takes no key) and the
/// edges' key/reference tails stay empty in both records. This is the
/// one-crossing diff the second-section review asked for (#671): in the
/// application's own form for a fresh site the crossing is recorded in the
/// topology's fields (the slot's vertex count, the shared node, the edge's
/// 0x80 and the crossing vertices' 0x430), not in inter-edge words.
#[test]
fn the_application_s_crossing_pair_reads_by_the_head_path() {
    let plain = app_container(APP_PLAIN_PAIR, 590).topology().expect("the plain pair's container reads");
    assert_eq!(plain.next_node, 5);
    assert_eq!(plain.next_key, 1, "no elevation points: no keys allocated");
    assert_eq!(plain.edge_ids, vec![1, 2]);
    assert_eq!(plain.edge_flags, vec![0x1_0030, 0x1_0030]);
    let e1 = &plain.edges[0];
    assert_eq!(e1.iter().map(|v| (v.node, v.flags, v.faces)).collect::<Vec<_>>(), vec![(1, 0x32, (1, 1)), (2, 10, (0, 0))]);
    assert_eq!((e1[0].point, e1[1].point), (Vector2::new(100.0, 100.0), Vector2::new(500.0, 100.0)));
    let e2 = &plain.edges[1];
    assert_eq!(e2.iter().map(|v| (v.node, v.flags, v.faces)).collect::<Vec<_>>(), vec![(3, 0x82, (1, 1)), (4, 10, (0, 0))]);
    assert_eq!(plain.face_ids, vec![1]);
    assert_eq!(plain.outside_face_flags, 6);
    assert_eq!(plain.outside_entries, vec![(1, 0), (3, 0)], "the two loops of the disjoint pair");

    let crossed = app_container(APP_CROSSED_PAIR, 587).topology().expect("the crossed pair's container reads");
    assert_eq!(crossed.next_node, 6, "the crossing node took id 5");
    assert_eq!(crossed.next_key, 1, "still no keys: the crossing takes none");
    assert_eq!(crossed.edge_flags, vec![0x1_0030 | 0x80, 0x1_0030 | 0x80], "both edges crossed");
    let e1 = &crossed.edges[0];
    assert_eq!(e1.iter().map(|v| v.node).collect::<Vec<_>>(), vec![1, 5, 2], "FL1 split at the crossing");
    assert_eq!((e1[1].point, e1[1].flags, e1[1].faces), (Vector2::new(250.0, 100.0), 0x430, (1, 1)));
    assert_eq!((e1[0].flags, e1[2].flags), (0x32, 10));
    let e2 = &crossed.edges[1];
    assert_eq!(e2.iter().map(|v| v.node).collect::<Vec<_>>(), vec![3, 5, 4]);
    assert_eq!((e2[0].point, e2[0].flags), (Vector2::new(150.0, 50.0), 0xb2), "the first point carries the 0x30 run");
    assert_eq!((e2[1].point, e2[1].flags, e2[1].faces), (Vector2::new(250.0, 100.0), 0x430, (1, 1)), "the same shared node");
    assert_eq!(crossed.face_ids, vec![1]);
    assert_eq!(crossed.outside_face_flags, 6);
    assert_eq!(crossed.outside_entries, vec![(1, 0)], "one loop: the graph joined");
    for v in e1.iter().chain(e2) {
        assert!(!v.is_elevation_point(), "no elevation points in either record");
        assert_eq!(v.key, 0);
        assert_eq!(v.bulge, 0.0);
        assert_eq!(v.z, 0.0);
    }
}

/// The crate writes the application's form: the same two workloads built
/// with the civil writers decode to the recorded pair's every topology
/// field — the nodes with their coordinates, the edge ids, flags and
/// vertices, the crossing's shared node and marks, the face loops. One
/// field stays apart: the application's fresh pair keeps the outside
/// face's flags at 6 (flag 8 lost) in BOTH states, also where its second
/// line joins the site beside the first (no crossing, two loops); the
/// crate loses flag 8 only where a member crosses or an elevation point
/// exists, so the disjoint pair's container keeps 14. The flag-8 rule for
/// feature-line-only sites is not pinned (the captured lot-line samples
/// agree with the crate); the pair's two records are the first captures
/// that part the two cases.
#[test]
fn the_writers_crossing_form_matches_the_application_s_pair() {
    for (crossed, app_flags) in [(false, 6), (true, 6)] {
        let mut doc = rt_data::rt_data_drawing();
        let site = add_site(&mut doc, "OR_S", "lines").unwrap();
        add_feature_line_vertices(&mut doc, site, "OR_FL1", &[vertex((100.0, 100.0, 0.0), 0.0, false), vertex((500.0, 100.0, 0.0), 0.0, false)], None).unwrap();
        let fl2 = if crossed {
            add_feature_line_vertices(&mut doc, site, "OR_FL2", &[vertex((150.0, 50.0, 0.0), 0.0, false), vertex((450.0, 200.0, 0.0), 0.0, false)], None).unwrap()
        } else {
            add_feature_line_vertices(&mut doc, site, "OR_FL2", &[vertex((150.0, 200.0, 0.0), 0.0, false), vertex((450.0, 200.0, 0.0), 0.0, false)], None).unwrap()
        };
        let back = rt_data::rt_data_roundtrip(&doc);
        let t = container_of(&back, site_of(&back, "OR_S")).topology().expect("topology");
        let other = acadrust::classes::civil::feature_line_vertices(&back, fl2).expect("FL2 reads back");
        if crossed {
            assert_eq!(t.next_node, 6);
            assert_eq!(t.edge_flags, vec![0x1_0030 | 0x80, 0x1_0030 | 0x80], "the application's crossed marks");
            let e1 = &t.edges[0];
            assert_eq!(e1.iter().map(|v| (v.node, v.flags, v.faces)).collect::<Vec<_>>(), vec![(1, 0x32, (1, 1)), (5, 0x430, (1, 1)), (2, 10, (0, 0))]);
            assert_eq!((e1[1].point, e1[1].z), (Vector2::new(250.0, 100.0), 0.0));
            let e2 = &t.edges[1];
            assert_eq!(e2.iter().map(|v| (v.node, v.flags)).collect::<Vec<_>>(), vec![(3, 0xb2), (5, 0x430), (4, 10)]);
            assert_eq!((e2[0].point, e2[1].point), (Vector2::new(150.0, 50.0), Vector2::new(250.0, 100.0)));
            assert_eq!(t.outside_entries, vec![(1, 0)]);
            assert_eq!(other.iter().map(|v| (v.point.x, v.point.y)).collect::<Vec<_>>(), vec![(150.0, 50.0), (250.0, 100.0), (450.0, 200.0)], "the crossing joins FL2's points");
        } else {
            assert_eq!(t.next_node, 5);
            assert_eq!(t.edge_flags, vec![0x1_0030, 0x1_0030]);
            let e1 = &t.edges[0];
            assert_eq!(e1.iter().map(|v| (v.node, v.flags)).collect::<Vec<_>>(), vec![(1, 0x32), (2, 10)]);
            let e2 = &t.edges[1];
            assert_eq!(e2.iter().map(|v| (v.node, v.flags)).collect::<Vec<_>>(), vec![(3, 0x82), (4, 10)]);
            assert_eq!(t.outside_entries, vec![(1, 0), (3, 0)], "the two loops of the disjoint pair");
            assert_eq!(other.iter().map(|v| (v.point.x, v.point.y)).collect::<Vec<_>>(), vec![(150.0, 200.0), (450.0, 200.0)]);
        }
        assert_eq!(t.next_key, 1, "no elevation points: no keys, like the application's pair");
        assert_eq!(t.edges.iter().flatten().all(|v| v.key == 0 && v.z == 0.0), true);
        // The one field apart (see the test comment): the application's
        // pair keeps 6 in both states, the crate's disjoint case keeps 14.
        if !crossed {
            assert_eq!(t.outside_face_flags, 14, "the crate's disjoint case keeps flag 8; the application's fresh pair recorded {app_flags}");
        } else {
            assert_eq!(t.outside_face_flags, app_flags, "the crossing loses flag 8, as the application's pair does");
        }
    }
}
