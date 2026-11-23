//! Family 8 — parcels and lot lines (civil-write.md §3g "Parcels",
//! samples WP03/WP04, checks E-P2). READY-WITH-LIMITS: the documented
//! subset is asserted — the writer's own node ids, positions, faces,
//! edge flags and the recomputed label locations are exact. The
//! application's own marks are out of scope: the edge flag 0x800 it sets
//! on an edge read from a file when another member joins, the newest
//! member's point marks 0x10/0x20, and lot lines that meet an arc or an
//! alignment or feature line (the writer refuses them).

#[path = "rt_data/mod.rs"]
mod rt_data;

use acadrust::classes::civil::{add_lot_line, add_parcel, add_site, Topology};
use acadrust::types::{Handle, Vector2};

/// The site entity named `name`.
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

/// The parcels (not the site): the `AECC_PARCEL` entities that name a
/// site as their second reference, with number, face and label.
fn parcels_of(back: &acadrust::CadDocument, site: Handle) -> Vec<(i32, i32, Vector2)> {
    let mut out: Vec<(i32, i32, Vector2)> = rt_data::rt_data_entities(back, "AECC_PARCEL")
        .into_iter()
        .filter(|(_, d)| d.parcel_site() == Some(site.value()))
        .map(|(_, d)| {
            let (number, face) = d.parcel_face().expect("a parcel's number and face");
            let label = d.parcel_label().expect("a parcel's label location");
            (number, face, label)
        })
        .collect();
    out.sort_by_key(|p| p.1);
    out
}

/// The signed area of the first boundary loop of `face` (positive
/// counterclockwise).
fn face_area(t: &Topology, face: i32) -> f64 {
    let loops = t.face_polylines(face);
    assert_eq!(loops.len(), 1, "face {face} is one loop");
    let p = &loops[0];
    let mut a = 0.0;
    for i in 0..p.len() {
        let (q, r) = (p[i], p[(i + 1) % p.len()]);
        a += q.x * r.y - r.x * q.y;
    }
    a / 2.0
}

#[test]
fn a_parcel_and_its_face_read_back_exact() {
    let mut doc = rt_data::rt_data_drawing();
    let site = add_site(&mut doc, "P1", "parcels").unwrap();
    let rect = [Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0), Vector2::new(100.0, 50.0), Vector2::new(0.0, 50.0)];
    add_parcel(&mut doc, site, &rect, None).unwrap();

    let back = rt_data::rt_data_roundtrip(&doc);
    let site_h = site_of(&back, "P1");

    // One parcel: number 1 over face 2 (face 1 is the outside face), its
    // label in the middle of the rectangle (the middle of a rectangle
    // stays where it is).
    assert_eq!(parcels_of(&back, site_h), vec![(1, 2, Vector2::new(50.0, 25.0))]);

    let t = container_of(&back, site_h).topology().expect("topology");
    // One member: the closed lot line, edge flags 0x31.
    assert_eq!(t.edges.len(), 1);
    assert_eq!(t.edge_flags, vec![0x31]);
    let e = &t.edges[0];
    // The nodes in path order, ids 1..4 — the record closes with a copy
    // of the first vertex — the written points bit-exact at elevation 0.
    assert_eq!(e.iter().map(|v| v.node).collect::<Vec<_>>(), vec![1, 2, 3, 4, 1]);
    for (v, w) in e[..4].iter().zip(rect) {
        assert_eq!(v.point, w, "node position bit-exact");
        assert_eq!(v.z, 0.0);
        assert_eq!(v.bulge, 0.0);
        assert_eq!(v.key, 0);
        assert!(!v.is_elevation_point());
    }
    // The counter-clockwise rectangle keeps the parcel face (2) on the
    // left of every segment, the outside face (1) on the right — on the
    // last point too, which starts the segment back to the first.
    for v in e[..4].iter() {
        assert_eq!(v.faces, (2, 1), "faces on the sides of the segment starting at {:?}", v.point);
    }
    // The closed edge's last point: 0x22 on the last member.
    assert_eq!(e[3].flags, 0x22);
    assert_eq!(t.next_node, 5);
    assert_eq!(t.next_key, 1, "no elevation points");
    assert_eq!(t.outside_face_flags, 14);

    // The parcel face is the rectangle, exactly.
    assert_eq!(face_area(&t, 2), 5000.0, "the parcel face's area");
    let loop2 = t.face_boundary(2);
    assert_eq!(loop2.len(), 1, "one loop");
}

#[test]
fn a_lot_line_across_a_parcel_splits_face_and_parcel() {
    let mut doc = rt_data::rt_data_drawing();
    let site = add_site(&mut doc, "P1", "split").unwrap();
    let rect = [Vector2::new(0.0, 0.0), Vector2::new(200.0, 0.0), Vector2::new(200.0, 100.0), Vector2::new(0.0, 100.0)];
    add_parcel(&mut doc, site, &rect, None).unwrap();
    let (_line, made) = add_lot_line(
        &mut doc,
        site,
        &[(Vector2::new(50.0, -10.0), 0.0), (Vector2::new(50.0, 110.0), 0.0)],
        false,
        None,
    )
    .unwrap();
    assert_eq!(made.len(), 1, "the crossing makes one new parcel");

    let back = rt_data::rt_data_roundtrip(&doc);
    let site_h = site_of(&back, "P1");

    // Two parcels over faces 2 and 3. The first parcel keeps its number
    // and takes the part its label lies in: the right part (x > 50),
    // middle (125, 50); the left part (x < 50) becomes a new parcel, number 2,
    // label (25, 50).
    let got = parcels_of(&back, site_h);
    assert_eq!(got.len(), 2);
    assert!(got.contains(&(1, 2, Vector2::new(125.0, 50.0))) || got.contains(&(1, 3, Vector2::new(125.0, 50.0))), "parcel 1 over the right part: {got:?}");
    assert!(got.iter().any(|(n, _, l)| *n == 2 && *l == Vector2::new(25.0, 50.0)), "the left part is parcel 2: {got:?}");
    let faces: Vec<i32> = got.iter().map(|p| p.1).collect();
    assert!(faces.contains(&2) && faces.contains(&3), "faces 2 and 3: {faces:?}");

    let t = container_of(&back, site_h).topology().expect("topology");
    assert_eq!(t.edges.len(), 2);
    // The closed rectangle edge keeps flag 1 and takes the 0x80 mark of
    // an edge another member's crossing added nodes to; the open lot line
    // is 0x30 (its own crossings do not close it).
    let closed_at = t.edge_flags.iter().position(|f| f & 1 != 0).expect("the rectangle's closed edge");
    assert_eq!(t.edge_flags[closed_at] & 0x30, 0x30);
    assert_eq!(t.edge_flags[closed_at] & 0x80, 0x80, "the crossed edge's mark");
    assert_eq!(t.edge_flags[1 - closed_at] & 0x30, 0x30, "the open lot line's edge");

    // The rectangle's edge runs through the two crossing nodes; the lot
    // line's ends are its own. All positions bit-exact, elevation 0.
    let (rect_e, line_e) = if closed_at == 0 { (&t.edges[0], &t.edges[1]) } else { (&t.edges[1], &t.edges[0]) };
    assert_eq!(rect_e.len(), 7, "four corners, two crossing nodes, and the closing copy of the first");
    let crossings: Vec<&acadrust::classes::civil::TopologyVertex> = rect_e[..6].iter().filter(|v| v.point != rect[0] && v.point != rect[1] && v.point != rect[2] && v.point != rect[3]).collect();
    assert_eq!(crossings.len(), 2);
    for c in &crossings {
        assert!(c.point == Vector2::new(50.0, 0.0) || c.point == Vector2::new(50.0, 100.0), "crossing at {:?}", c.point);
        assert_eq!(c.z, 0.0);
        assert_eq!(c.flags & 0x400, 0x400, "a crossing node is marked 0x400");
        // The lot line's edge passes through the same node.
        assert!(line_e.iter().any(|v| v.node == c.node), "the crossing node is shared");
    }
    assert_eq!(line_e.len(), 4, "the lot line's two ends and the two shared crossings between them");
    assert_eq!(line_e[0].point, Vector2::new(50.0, -10.0));
    assert_eq!(line_e[3].point, Vector2::new(50.0, 110.0));
    assert_eq!(line_e[1].point, Vector2::new(50.0, 0.0));
    assert_eq!(line_e[2].point, Vector2::new(50.0, 100.0));
    for v in rect_e[..6].iter().chain(line_e.iter()) {
        assert_eq!(v.z, 0.0);
        assert_eq!(v.bulge, 0.0);
    }

    // Three faces: the outside face and the two parcel halves, of the
    // areas the split gives.
    let mut areas: Vec<f64> = [2, 3].iter().map(|f| face_area(&t, *f)).collect();
    areas.sort_by(|a, b| a.total_cmp(b));
    assert_eq!(areas, vec![5000.0, 15000.0], "the two parcel halves' areas");
    assert_eq!(t.outside_face_flags, 6, "a later member split the outside face's face: flag 8 lost");
}
