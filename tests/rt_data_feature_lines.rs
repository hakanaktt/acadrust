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
