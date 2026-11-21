//! Family 6 — sites and alignments on a site (civil-write.md §3g). READY:
//! the site's container topology reads back with its nodes, edges and
//! faces exactly: node ids (a member's ends, then its inner points),
//! point flags, faces on the segment sides, the alignment's edge flags,
//! and the arc's bulge and tangent points (the solver's own geometry,
//! compared within 1e-6; every other number bit-exact).

#[path = "rt_data/mod.rs"]
mod rt_data;

use acadrust::classes::civil::{add_alignment_on_site, add_site};
use acadrust::types::{Handle, Vector2};

/// The site entity named `name` (an `AECC_PARCEL` whose first texts are
/// "AeccImpFace", the name and the description).
fn site_named(back: &acadrust::CadDocument, name: &str) -> (Handle, String, String) {
    for (h, d) in rt_data::rt_data_entities(back, "AECC_PARCEL") {
        if let Some((n, desc)) = d.face_name() {
            if n == name {
                return (h, n, desc);
            }
        }
    }
    panic!("site {name} missing");
}

/// The site's container record: the `AECC_FEATURE_STYLE` the site owns.
fn container_of(back: &acadrust::CadDocument, site: Handle) -> acadrust::classes::arch::ArchObject {
    rt_data::rt_data_objects(back, "AECC_FEATURE_STYLE")
        .into_iter()
        .find(|c| c.owner == site)
        .unwrap_or_else(|| panic!("no container owned by the site"))
}

fn alignment_named(back: &acadrust::CadDocument, name: &str) -> Handle {
    for (h, d) in rt_data::rt_data_entities(back, "AECC_ALIGNMENT") {
        if d.alignment_name().as_deref() == Some(name) {
            return h;
        }
    }
    panic!("alignment {name} missing");
}

#[test]
fn a_site_and_a_straight_alignment_read_back_exact() {
    let mut doc = rt_data::rt_data_drawing();
    let site = add_site(&mut doc, "S1", "site one").unwrap();
    let v = [(0.0, 0.0), (100.0, 0.0), (200.0, 80.0)];
    let verts: Vec<acadrust::types::Vector2> = v.iter().map(|(x, y)| Vector2::new(*x, *y)).collect();
    add_alignment_on_site(&mut doc, site, "AL1", &verts, &[acadrust::classes::civil::VertexCurve::None], None).unwrap();

    let back = rt_data::rt_data_roundtrip(&doc);
    let (site_h, name, description) = site_named(&back, "S1");
    assert_eq!((name.as_str(), description.as_str()), ("S1", "site one"));

    let container = container_of(&back, site_h);
    // The alignment is the container's only member.
    let al_h = alignment_named(&back, "AL1");
    assert_eq!(container.site_members(), vec![al_h.value()], "the alignment is a member of the site");

    let topology = container.topology().expect("the container's topology reads back");
    // One edge for the one member; an alignment edge has flags 0x30.
    assert_eq!(topology.edges.len(), 1);
    assert_eq!(topology.edge_flags, vec![0x30]);
    // Nodes: the ends first, then the inner points (ids 1, 2, 3); the
    // positions bit-exact, the elevation 0 on an alignment.
    let e = &topology.edges[0];
    assert_eq!(e.iter().map(|v| v.node).collect::<Vec<_>>(), vec![1, 3, 2]);
    let want = [Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0), Vector2::new(200.0, 80.0)];
    for (v, w) in e.iter().zip(want) {
        assert_eq!(v.point, w, "node position bit-exact");
        assert_eq!(v.z, 0.0, "alignment nodes lie at elevation 0");
        assert_eq!(v.key, 0, "no elevation points on an alignment");
        assert_eq!(v.bulge, 0.0, "straight segments");
    }
    // Point flags: 2 with 0x30 while another segment follows the
    // segment that starts there; the last point of the open edge is 10.
    // No faces but the outside face.
    assert_eq!(e[0].flags, 0x32);
    assert_eq!(e[1].flags, 2, "the last segment starts here, none follows it");
    assert_eq!(e[2].flags, 10);
    assert_eq!(e[0].faces, (1, 1), "the outside face on both sides");
    assert_eq!(e[1].faces, (1, 1));
    assert_eq!(e[2].faces, (0, 0), "the last vertex keeps no faces");
    assert_eq!(topology.next_node, 4, "three nodes, the next free id 4");
    assert_eq!(topology.next_key, 1, "no elevation points, the next free key 1");
    assert_eq!(topology.outside_face_flags, 14, "the outside face untouched");
}

#[test]
fn an_alignment_with_a_free_arc_carries_its_bulge() {
    let mut doc = rt_data::rt_data_drawing();
    let site = add_site(&mut doc, "S2", "").unwrap();
    let verts = [Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0), Vector2::new(100.0, 50.0)];
    add_alignment_on_site(&mut doc, site, "AL2", &verts, &[acadrust::classes::civil::VertexCurve::Arc(30.0)], None).unwrap();

    let back = rt_data::rt_data_roundtrip(&doc);
    let (site_h, _, _) = site_named(&back, "S2");
    let topology = container_of(&back, site_h).topology().expect("topology");
    let e = &topology.edges[0];
    // The path: line to the arc's start, the arc, the line out.
    assert_eq!(e.len(), 4);
    assert_eq!(e.iter().map(|v| v.node).collect::<Vec<_>>(), vec![1, 3, 4, 2], "the ends take ids 1 and 2, the arc's tangent points 3 and 4");
    for v in e.iter() {
        assert_eq!(v.z, 0.0);
        assert_eq!(v.key, 0);
    }
    // The tangent points lie on the two legs; the radius-30 arc fits
    // between them. The writer's solver computes these, so they are
    // compared within 1e-6; ids, flags and faces stay exact.
    let a = Vector2::new(0.0, 0.0);
    let b = Vector2::new(100.0, 50.0);
    let t1 = e[1].point;
    assert!(t1.y.abs() < 1e-6 && t1.x > 0.0 && t1.x < 100.0, "arc start on the first leg, at {t1:?}");
    let t2 = e[2].point;
    assert!((t2.x - 100.0).abs() < 1e-6 && t2.y > 0.0 && t2.y < 50.0, "arc end on the second leg, at {t2:?}");
    let bulge = e[1].bulge;
    // A left turn: a positive bulge of tan(sweep / 4), sweep = PI / 2.
    assert!(bulge > 0.0);
    let want = (std::f64::consts::PI / 8.0).tan();
    assert!((bulge - want).abs() < 1e-6, "bulge {bulge} against tan(sweep/4) = {want}");
    assert_eq!(e[0].bulge, 0.0);
    assert_eq!(e[2].bulge, 0.0);
    // The chord between the tangent points fits the radius: the sweep from
    // the bulge and the chord agree.
    let chord = (Vector2::new(t2.x - t1.x, t2.y - t1.y)).length();
    let sweep = 4.0 * bulge.atan();
    let radius = chord / (2.0 * (sweep / 2.0).sin());
    assert!((radius - 30.0).abs() < 1e-4, "the arc keeps its radius, got {radius}");
    // The legs are bit-exact.
    assert_eq!(e[0].point, a);
    assert_eq!(e[3].point, b);
}
