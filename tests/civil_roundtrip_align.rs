//! Codec-only roundtrip tests for the alignment cluster (docs/civil-write.md
//! §3a, §3e): every test writes with the `create` writers, saves the DWG to a
//! scratch file, reopens it with `DwgReader::from_file` and asserts the
//! documented fields on the decoded records.
//!
//! Boundaries (per the write-surface audit):
//! - offset alignments: transitions that do not lie on one parent line are
//!   out of scope (the writer refuses them);
//! - curb returns: one-arc, three-arc and clothoid spiral-curve-spiral
//!   groups (spiral-spiral groups and 0/1 overlaps are refused, not tested);
//! - surface profiles carry no elevations by design (the application samples
//!   the surface), so their asserts are structural only.

use acadrust::classes::arch::ArchEntityData;
use acadrust::classes::civil::{
    add_alignment, add_alignment_of_kind, add_alignment_with_curves, add_curb_return, add_curb_return_with, add_design_speeds, add_offset_alignment, add_offset_alignment_with_widenings, add_offset_profile, add_profile, add_profile_entities, add_profile_with_curves, add_station_equations, add_surface_profile, add_tin_surface, ensure_civil, display_station, AlignmentElementKind, AlignmentKind, AlignmentPiece, AlignmentType, CurbReturnCurve, DesignSpeed, ProfileEntity, ProfilePiece, StationEquation, VertexCurve, VerticalCurve, Widening,
};
use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::io::dwg::{DwgReader, DwgWriter};
use acadrust::types::{DxfVersion, Handle, Vector2};
use acadrust::CadDocument;
use std::sync::atomic::{AtomicUsize, Ordering};

static SCRATCH: AtomicUsize = AtomicUsize::new(0);

/// A drawing with the civil tree the alignment and profile writers need.
fn drawing() -> CadDocument {
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1032;
    ensure_civil(&mut doc).unwrap();
    doc
}

fn arch(doc: &CadDocument, h: Handle) -> ArchEntityData {
    match doc.get_entity(h) {
        Some(EntityType::Extended(x)) => match &x.data {
            ExtendedEntityData::Arch(a) => a.clone(),
            _ => panic!("not an arch entity"),
        },
        _ => panic!("entity missing"),
    }
}

/// Save to a unique scratch file, reopen it, drop the file.
fn roundtrip(doc: &CadDocument, tag: &str) -> CadDocument {
    let n = SCRATCH.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!("rt_align_{tag}_{}_{}.dwg", std::process::id(), n));
    DwgWriter::write_to_file(&path, doc).unwrap();
    let back = DwgReader::from_file(&path).unwrap().read().unwrap();
    let _ = std::fs::remove_file(&path);
    back
}

/// 1. Alignments: siteless, with curves (arc, spiral-curve-spiral) and of
///    kind (centerline, utility, rail).
#[test]
fn rt_align_siteless_curves_and_kinds_round_trip() {
    use AlignmentElementKind::{FixedLine, FreeArc, FreeSpiralCurveSpiral};
    let mut doc = drawing();
    let v = [Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0), Vector2::new(200.0, 50.0), Vector2::new(300.0, -50.0)];
    let plain = add_alignment(&mut doc, "AL0", &v, None).unwrap();
    let arc_r = 169.4427190999918;
    let scs = VertexCurve::SpiralCurveSpiral { spiral_in: 20.0, radius: 50.0, spiral_out: 20.0 };
    let curved = add_alignment_with_curves(&mut doc, "AL1", &v, &[VertexCurve::Arc(arc_r), scs], None).unwrap();
    let center = add_alignment_of_kind(&mut doc, "ALC", &v[..2], &[], AlignmentKind::Centerline, None).unwrap();
    let utility = add_alignment_of_kind(&mut doc, "ALU", &v[..2], &[], AlignmentKind::Utility, None).unwrap();
    let rail = add_alignment_of_kind(&mut doc, "ALR", &v[..2], &[], AlignmentKind::Rail { gauge: 1.435 }, None).unwrap();
    let back = roundtrip(&doc, "kinds");

    // Siteless: name, start station, the solved polyline and the stored
    // line elements, all bit-exact.
    let a = arch(&back, plain);
    assert_eq!(a.alignment_name(), Some("AL0".into()));
    assert_eq!(a.alignment_start_station(), Some(0.0));
    assert_eq!(a.alignment_polylines(), vec![v.to_vec()]);
    assert_eq!(a.alignment_type(), Some(AlignmentType::Centerline));
    let elements = a.alignment_elements().unwrap();
    assert_eq!(elements.iter().map(|e| (e.id, e.previous, e.next)).collect::<Vec<_>>(), [(1, 0, 2), (2, 1, 3), (3, 2, 0)]);
    assert_eq!(elements[0].kind, FixedLine { start: v[0], end: v[1] });
    assert_eq!(elements[2].kind, FixedLine { start: v[2], end: v[3] });

    // Curves: the lines keep ids 1..3, the arc takes 4 and the
    // spiral-curve-spiral 5, linked along the path; the solved pieces run
    // line, arc, line, spiral, arc, spiral, line.
    let a = arch(&back, curved);
    let elements = a.alignment_elements().unwrap();
    assert_eq!(elements.iter().map(|e| (e.id, e.previous, e.next)).collect::<Vec<_>>(), [(1, 0, 4), (2, 4, 5), (3, 5, 0), (4, 1, 2), (5, 2, 3)]);
    assert_eq!(elements[3].kind, FreeArc { radius: arc_r });
    assert_eq!(elements[4].kind, FreeSpiralCurveSpiral { spiral_in: 20.0, radius: 50.0, spiral_out: 20.0 });
    let pieces = a.alignment_pieces();
    assert_eq!(pieces.iter().map(|p| p.0).collect::<Vec<_>>(), [
        AlignmentPiece::Line,
        AlignmentPiece::Curve,
        AlignmentPiece::Line,
        AlignmentPiece::Spiral,
        AlignmentPiece::Curve,
        AlignmentPiece::Spiral,
        AlignmentPiece::Line
    ]);
    // Geometry points of the line-arc-line-scs-line run.
    let names: Vec<&str> = a.alignment_geometry_points().iter().map(|g| g.name).collect();
    assert_eq!(names, ["BP", "PC", "PT", "TS", "SC", "CS", "ST", "EP"]);

    // Kinds: the record's type word (1 centerline, 4 utility, 5 rail) and
    // the rail gauge read back.
    assert_eq!(arch(&back, center).alignment_type(), Some(AlignmentType::Centerline));
    assert_eq!(arch(&back, utility).alignment_type(), Some(AlignmentType::Miscellaneous));
    let r = arch(&back, rail);
    assert_eq!(r.alignment_type(), Some(AlignmentType::Rail));
    assert_eq!(r.alignment_track_gauge(), Some(1.435));
}

/// 2. Station anchors, design speeds and station equations. A profile adds
///    a start anchor; each speed and equation ties to an anchor of kind 2
///    and 3 by a version-4 GUID.
#[test]
fn rt_align_anchors_speeds_and_equations_round_trip() {
    let mut doc = drawing();
    let v = [Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0), Vector2::new(200.0, 50.0)];
    let a = add_alignment_with_curves(&mut doc, "AL1", &v, &[VertexCurve::Arc(169.4427190999918)], None).unwrap();
    let p = add_profile(&mut doc, a, "P1", &[Vector2::new(0.0, 10.0), Vector2::new(200.0, 12.0)], &[], None).unwrap();
    add_design_speeds(&mut doc, a, &[(60.0, 30.0), (0.0, 50.0)]).unwrap();
    add_station_equations(&mut doc, a, &[(80.0, 1000.0, true)]).unwrap();
    add_station_equations(&mut doc, a, &[(150.0, 2000.0, false)]).unwrap();
    let back = roundtrip(&doc, "timing");

    let r = arch(&back, a);
    // Speeds in station order, equations in raw station order; the station
    // back is what the equations before give at the raw station (80 then
    // 1070 for 1000 ahead from 80).
    assert_eq!(r.alignment_design_speeds(), [DesignSpeed { station: 0.0, speed: 50.0 }, DesignSpeed { station: 60.0, speed: 30.0 }]);
    let equations = r.alignment_station_equations();
    assert_eq!(equations, [
        StationEquation { raw: 80.0, back: 80.0, ahead: 1000.0, increasing: true },
        StationEquation { raw: 150.0, back: 1070.0, ahead: 2000.0, increasing: false },
    ]);
    // Displayed stations between the equations add 1000 ahead.
    assert_eq!(display_station(&equations, 90.0), 1010.0);

    // The anchors: one per speed (kind 2), one per equation (kind 3), and
    // the profile's start anchor (kind 32763).
    let anchors = r.alignment_anchors().unwrap();
    let mut tagged: Vec<(i32, f64)> = anchors.iter().map(|x| (x.kind, x.station)).collect();
    tagged.sort_by(|x, y| x.0.cmp(&y.0).then(x.1.total_cmp(&y.1)));
    assert_eq!(tagged, [(2, 0.0), (2, 60.0), (3, 80.0), (3, 150.0), (32763, 0.0)]);
    // Every anchor is kept by a distinct identifier (a version-4 GUID as
    // written; the record's byte order is not asserted).
    let mut ids: Vec<[u8; 16]> = anchors.iter().map(|x| x.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), anchors.len());
    // The profile's start anchor sits at station 0 on the first line
    // element (the arc is element 3).
    let start = anchors.iter().find(|x| x.kind == 32763).unwrap();
    assert_eq!((start.element, start.previous, start.next), (1, 0, 3));
    // The first line runs to the arc's tangent point at 60.
    assert_eq!((start.along, start.value), (0.0, 0.0));
    assert!((start.remaining - start.element_start.distance(&start.element_end)).abs() < 1e-12);
    assert!((start.remaining - 60.0).abs() < 1e-9);
    assert!(start.flag);
    assert_eq!((start.element_start, start.point), (v[0], v[0]));
    assert!(start.element_end.distance(&Vector2::new(60.0, 0.0)) < 1e-9);
    // The profile reads back by its name.
    assert_eq!(arch(&back, p).profile_name().as_deref(), Some("P1"));
}

/// 3. Offset alignments, with and without widenings. Transitions on arcs
///    are out of scope (the writer refuses a transition longer than 0 that
///    does not lie on one parent line).
#[test]
fn rt_align_offset_and_widenings_round_trip() {
    let v = [Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0), Vector2::new(200.0, 50.0)];
    let arc_r = 169.4427190999918;

    // Full-range offsets both sides of a line-arc-line parent.
    let mut doc = drawing();
    let parent = add_alignment_with_curves(&mut doc, "AL1", &v, &[VertexCurve::Arc(arc_r)], None).unwrap();
    let right = add_offset_alignment(&mut doc, parent, "OR", 10.0, None, None).unwrap();
    let left = add_offset_alignment(&mut doc, parent, "OL", -10.0, None, None).unwrap();
    let back = roundtrip(&doc, "offset");
    let region_end = |h: Handle| arch(&back, h).alignment_offset_parameters().unwrap().regions[0].parent_end;
    let parent_end = region_end(right);
    for (h, offset, radius) in [(right, 10.0, arc_r + 10.0), (left, -10.0, arc_r - 10.0)] {
        let o = arch(&back, h);
        assert_eq!(o.alignment_type(), Some(AlignmentType::Offset));
        assert_eq!(o.alignment_offset_parent(), Some(parent.value()));
        let p = o.alignment_offset_parameters().unwrap();
        assert!(p.dynamic && p.left == (offset < 0.0) && p.offset == offset && p.regions.len() == 1);
        let region = &p.regions[0];
        // The writer's computed parent length (the range's end) is the same
        // value in both offsets' regions.
        assert_eq!((region.offset, region.parent_start, region.kind), (offset, 0.0, 2));
        assert_eq!(region.parent_end, parent_end);
        // Elements 2n+1..: the two lines and the concentric arc between
        // them; the map names the parent element each follows.
        assert_eq!(p.element_map, [(1, vec![7]), (2, vec![9]), (3, vec![8])]);
        assert_eq!(region.first_element, 7);
        assert_eq!(region.last_element, 9);
        assert_eq!(o.alignment_start_station(), Some(0.0));
        // The offset alignment's own start anchor (kind 2) carries its
        // design speed of 0.
        assert_eq!(o.alignment_design_speeds(), [DesignSpeed { station: 0.0, speed: 0.0 }]);
        let own = o.alignment_anchors().unwrap();
        assert_eq!(own.len(), 1);
        assert_eq!((own[0].kind, own[0].element, own[0].station), (2, 7, 0.0));
        // The solved run: line, arc, line; the arc concentric with the
        // parent's, its radius the parent's plus the offset on the outside
        // and minus it on the inside.
        let pieces = o.alignment_pieces_with_curvature();
        assert_eq!(pieces.iter().map(|p| p.0).collect::<Vec<_>>(), [AlignmentPiece::Line, AlignmentPiece::Curve, AlignmentPiece::Line]);
        let curvature = pieces[1].1;
        assert!((curvature * radius - 1.0).abs() < 1e-9, "radius {radius} vs curvature {curvature}");
    }
    // The parent holds an anchor (kind 6) at each end of each range, on
    // the element the station lies on: the first and last line here.
    let anchors = arch(&back, parent).alignment_anchors().unwrap();
    let mut tagged: Vec<(i32, u64)> = anchors.iter().map(|a| (a.kind, a.station.to_bits())).collect();
    tagged.sort();
    assert_eq!(tagged, [(6, 0.0f64.to_bits()), (6, 0.0f64.to_bits()), (6, parent_end.to_bits()), (6, parent_end.to_bits())]);
    for anchor in &anchors {
        let length = anchor.element_start.distance(&anchor.element_end);
        assert!((anchor.along + anchor.remaining - length).abs() < 1e-9);
    }

    // A widening with entry and exit transitions on a straight parent.
    let mut doc = drawing();
    let parent = add_alignment(&mut doc, "AL2", &[Vector2::new(0.0, 0.0), Vector2::new(300.0, 0.0)], None).unwrap();
    let widening = Widening { start: 100.0, end: 150.0, offset: 20.0, entry: 15.0, exit: 15.0 };
    let off = add_offset_alignment_with_widenings(&mut doc, parent, "OW", 10.0, None, &[widening], None).unwrap();
    let back = roundtrip(&doc, "widening");
    let o = arch(&back, off);
    let p = o.alignment_offset_parameters().unwrap();
    let regions: Vec<(f64, f64, f64, f64, i32)> = p.regions.iter().map(|r| (r.offset, r.parent_start, r.parent_end, r.end, r.kind)).collect();
    // Nominal, widening, nominal: the widening's full offset runs from its
    // start plus the entry to its end.
    let ramp = (15.0f64 * 15.0 + 10.0 * 10.0).sqrt();
    for (region, want) in regions.iter().zip([(10.0, 0.0, 100.0, 2.0), (20.0, 115.0, 150.0, 1.0), (10.0, 165.0, 300.0, 2.0)]) {
        assert_eq!((region.0, region.1, region.2, region.4 as f64), want);
    }
    // The offset alignment's own stations keep the parent's measure over
    // the widening and add each ramp's true length (sqrt(15² + 10²) from
    // offset 10 to 20).
    assert_eq!(regions[0].3, 100.0);
    assert!((regions[1].3 - (100.0 + ramp + 35.0)).abs() < 1e-9);
    assert!((regions[2].3 - (100.0 + 2.0 * ramp + 170.0)).abs() < 1e-9);
    // Both transitions linear, of the given length.
    assert_eq!(p.transitions.iter().map(|t| (t.kind, t.length, t.radii.is_empty())).collect::<Vec<_>>(), [(1, 15.0, true), (1, 15.0, true)]);
    // The map excludes the transitions.
    assert_eq!(p.element_map, [(1, vec![3, 4, 5])]);
    // The parent gains an anchor at every distinct region or transition
    // end (the whole range on one line, so the stations are the stores).
    let parent_anchors = arch(&back, parent).alignment_anchors().unwrap();
    let mut stations: Vec<u64> = parent_anchors.iter().map(|a| a.station.to_bits()).collect();
    stations.sort();
    assert_eq!(stations, [0.0f64, 100.0, 115.0, 150.0, 165.0, 300.0].map(|s| s.to_bits()).to_vec());
    assert!(parent_anchors.iter().all(|a| a.kind == 6));
    // The ramps run from the previous offset to the widening's and back.
    let elements = o.alignment_elements().unwrap();
    let ramp = |id: i32| match elements.iter().find(|e| e.id as i32 == id).map(|e| &e.kind) {
        Some(AlignmentElementKind::Points { points }) => points.clone(),
        other => panic!("element {id}: {other:?}"),
    };
    let ramps: Vec<Vec<Vector2>> = {
        let map_ids: Vec<i32> = p.element_map.iter().flat_map(|(_, ids)| ids).copied().collect();
        let transition_ids: Vec<i32> = p.transitions.iter().map(|t| t.element).collect();
        assert_eq!(transition_ids.len(), 2);
        assert!(transition_ids.iter().all(|t| !map_ids.contains(t)));
        transition_ids.iter().map(|t| ramp(*t)).collect()
    };
    for (points, want) in ramps.iter().zip([
        [Vector2::new(100.0, -10.0), Vector2::new(115.0, -20.0)],
        [Vector2::new(150.0, -20.0), Vector2::new(165.0, -10.0)],
    ]) {
        assert!(points.len() == 2 && points.iter().zip(want).all(|(p, w)| p.distance(&w) < 1e-9), "{points:?}");
    }
}

/// 4. Curb returns: one arc, three arcs and a clothoid
///    spiral-curve-spiral between two siteless roads.
#[test]
fn rt_align_curb_returns_round_trip() {
    let v = Vector2::new;
    let mut doc = drawing();
    let a = add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(200.0, 0.0)], None).unwrap();
    let b = add_alignment(&mut doc, "AL2", &[v(100.0, -100.0), v(100.0, 100.0)], None).unwrap();
    let one = add_curb_return(&mut doc, "CR1", a, -10.0, b, -10.0, true, 10.0, (5.0, 5.0), None).unwrap();
    let three = add_curb_return_with(&mut doc, "CR2", a, -10.0, b, -10.0, true, CurbReturnCurve::ThreeCentered { first: (25.0, 6.0), middle: 10.0, last: (25.0, 6.0) }, (5.0, 5.0), None).unwrap();
    let scs = add_curb_return_with(&mut doc, "CR3", a, -10.0, b, -10.0, true, CurbReturnCurve::SpiralCurveSpiral { spiral_in: 6.0, radius: 10.0, spiral_out: 6.0, shape: acadrust::classes::civil::SpiralShape::Clothoid }, (5.0, 5.0), None).unwrap();
    let back = roundtrip(&doc, "curb");

    // One arc: a connected alignment of line, free arc (the given radius),
    // line, joining the two roads.
    let r = arch(&back, one);
    assert_eq!(r.alignment_type(), Some(AlignmentType::CurbReturn));
    assert_eq!(r.alignment_connected_inputs(), Some([a.value(), b.value()]));
    let elements = r.alignment_elements().unwrap();
    assert_eq!(elements.iter().map(|e| e.id).collect::<Vec<_>>(), [1, 2, 3]);
    assert!(matches!(elements[1].kind, AlignmentElementKind::FreeArc { radius } if radius == 10.0));
    let AlignmentElementKind::FixedLine { start, .. } = &elements[0].kind else { panic!() };
    assert!(start.distance(&v(75.0, 10.0)) < 1e-9);

    // Three arcs: one element of three arcs turning the same way, the
    // middle length 0 (it takes the rest of the turn).
    let r = arch(&back, three);
    let elements = r.alignment_elements().unwrap();
    assert!(matches!(elements[1].kind, AlignmentElementKind::ThreeCenteredArc { arcs } if arcs == [(25.0, 6.0), (10.0, 0.0), (25.0, 6.0)]));

    // Clothoid spiral-curve-spiral: the lines keep ids 1 and 3, the spiral
    // in, the fixed arc and the spiral out take 4 to 6; the curve
    // parameters are kept in a JSON text of the record.
    let r = arch(&back, scs);
    let elements = r.alignment_elements().unwrap();
    assert_eq!(elements.iter().map(|e| e.id).collect::<Vec<_>>(), [1, 3, 4, 5, 6]);
    let kinds: Vec<AlignmentPiece> = r.alignment_pieces().iter().map(|p| p.0).collect();
    assert_eq!(kinds, [AlignmentPiece::Line, AlignmentPiece::Spiral, AlignmentPiece::Curve, AlignmentPiece::Spiral, AlignmentPiece::Line]);
    match (&elements[2].kind, &elements[4].kind) {
        (AlignmentElementKind::SpiralIn { length, radius, shape: acadrust::classes::civil::SpiralShape::Clothoid, .. }, AlignmentElementKind::SpiralOut { length: out, radius: r_out, shape: acadrust::classes::civil::SpiralShape::Clothoid, .. }) => {
            assert_eq!((length, radius), (&6.0, &10.0));
            assert_eq!((out, r_out), (&6.0, &10.0));
        }
        other => panic!("spirals: {other:?}"),
    }
    assert!(matches!(elements[3].kind, AlignmentElementKind::FixedArc { .. }));
    let json = r.alignment_layout_json().unwrap();
    for want in ["\"CurveType\": 4", "\"CurveRadius\": 10", "\"SpiralInLength\": 6", "\"SpiralOutLength\": 6", "\"SpiralType\": 256", "\"Greater180\": false"] {
        assert!(json.contains(want), "missing {want} in {json}");
    }
}

/// 5. Layout profiles: parabolas by length, K, radius and through a point,
///    circulars by radius, length and through a point, and an asymmetric
///    parabola.
#[test]
fn rt_align_layout_profiles_round_trip() {
    let v = Vector2::new;
    let mut doc = drawing();
    let al = add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(300.0, 0.0)], None).unwrap();
    let pvis3 = [v(0.0, 10.0), v(50.0, 20.0), v(100.0, 15.0)];
    let by_length = add_profile(&mut doc, al, "PA", &pvis3, &[30.0], None).unwrap();
    let by_k = add_profile_with_curves(&mut doc, al, "PB", &pvis3, &[VerticalCurve::ParabolaK(0.5)], None).unwrap();
    let by_radius = add_profile_with_curves(&mut doc, al, "PC", &pvis3, &[VerticalCurve::ParabolaRadius(50.0)], None).unwrap();
    let by_point = add_profile_with_curves(&mut doc, al, "PD", &pvis3, &[VerticalCurve::ParabolaThrough(v(50.0, 18.0))], None).unwrap();
    let circ = add_profile_with_curves(&mut doc, al, "PE", &pvis3, &[VerticalCurve::Circular(50.0)], None).unwrap();
    let circ_len = add_profile_with_curves(&mut doc, al, "PF", &pvis3, &[VerticalCurve::CircularLength(15.0)], None).unwrap();
    let circ_point = add_profile_with_curves(&mut doc, al, "PG", &pvis3, &[VerticalCurve::CircularThrough(v(50.0, 18.0))], None).unwrap();
    let pvis5 = [v(0.0, 10.0), v(50.0, 20.0), v(100.0, 15.0), v(150.0, 25.0), v(250.0, 18.0)];
    let mixed = add_profile_with_curves(&mut doc, al, "PH", &pvis5, &[VerticalCurve::Circular(50.0), VerticalCurve::Asymmetric { before: 8.0, after: 12.0 }, VerticalCurve::Parabola(20.0)], None).unwrap();
    let back = roundtrip(&doc, "profiles");

    let pieces = |h: Handle| arch(&back, h).profile_pieces();
    let curve_span = |h: Handle| {
        let curve = &pieces(h)[1].1;
        (curve[0].x, curve[curve.len() - 1].x)
    };
    let near = |a: f64, b: f64, tol: &str| assert!((a - b).abs() < 1e-6, "{tol}: {a} vs {b}");

    // The parabola of length 30 at PVI (50, 20) between the grades 0.2 and
    // -0.1: tangents meet it at 35 and 65.
    assert_eq!(pieces(by_length).iter().map(|p| p.0).collect::<Vec<_>>(), [ProfilePiece::Tangent, ProfilePiece::Parabola, ProfilePiece::Tangent]);
    let (from, to) = curve_span(by_length);
    near(from, 35.0, "parabola start");
    near(to, 65.0, "parabola end");
    // The PVIs: the joints with the curve's length at the PVI.
    let pvis = arch(&back, by_length).profile_pvis();
    assert_eq!(pvis.len(), 3);
    assert_eq!(pvis[1].point.x, 50.0);
    assert!((pvis[1].point.y - 20.0).abs() < 1e-9);
    assert_eq!(pvis[1].curve_length, 30.0);

    // By K and by radius: K 0.5 and radius 50 both give length
    // 100 * K * |grade change| = 15 (stations 42.5 to 57.5).
    for h in [by_k, by_radius] {
        let (from, to) = curve_span(h);
        near(from, 42.5, "K/radius start");
        near(to, 57.5, "K/radius end");
    }
    // Through the point (50, 18): stations 70/3 to 230/3.
    let (from, to) = curve_span(by_point);
    near(from, 70.0 / 3.0, "point start");
    near(to, 230.0 / 3.0, "point end");

    // Circular curves: of radius 50 the application starts at 42.6636; by
    // horizontal length 15 and through the point (50, 18) as sampled.
    let (from, _) = curve_span(circ);
    near(from, 42.663_583_344, "circular start");
    let (from, to) = curve_span(circ_len);
    near(from, 42.554880987, "circular length start");
    near(to, 57.554880987, "circular length end");
    let (from, to) = curve_span(circ_point);
    near(from, 23.673140622, "circular point start");
    near(to, 76.714991262, "circular point end");

    // The mixed profile: tangent, circular, tangent, asymmetric parabola
    // (8 before, 12 after its PVI), tangent, parabola, tangent.
    let kinds: Vec<ProfilePiece> = pieces(mixed).iter().map(|p| p.0).collect();
    assert_eq!(kinds, [ProfilePiece::Tangent, ProfilePiece::Circular, ProfilePiece::Tangent, ProfilePiece::AsymmetricParabola, ProfilePiece::Tangent, ProfilePiece::Parabola, ProfilePiece::Tangent]);
    let run = pieces(mixed);
    let (from, to) = (run[3].1[0].x, run[3].1[run[3].1.len() - 1].x);
    near(from, 92.0, "asymmetric start");
    near(to, 112.0, "asymmetric end");
    let pvis = arch(&back, mixed).profile_pvis();
    assert_eq!(pvis.len(), 5);
    assert_eq!(pvis[2].curve_length, 20.0);
    assert!((pvis[2].point.x - 100.0).abs() < 1e-9 && (pvis[2].point.y - 15.0).abs() < 1e-9);
    assert_eq!(pvis[3].curve_length, 20.0);
}

/// 6. Profiles laid out entity by entity: fixed tangents and parabolas,
///    floating parabolas, floating and free tangents.
#[test]
fn rt_align_profile_entities_round_trip() {
    let v = Vector2::new;
    let mut doc = drawing();
    let al = add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(200.0, 0.0)], None).unwrap();
    let t1 = ProfileEntity::Tangent { start: v(0.0, 10.0), end: v(40.0, 18.0) };
    let three = add_profile_entities(&mut doc, al, "P3", &[
        t1,
        ProfileEntity::ParabolaThreePoints { start: v(40.0, 18.0), middle: v(50.0, 19.5), end: v(60.0, 19.0) },
        ProfileEntity::Tangent { start: v(60.0, 19.0), end: v(100.0, 15.0) },
        ProfileEntity::ParabolaTwoPointsK { start: v(100.0, 15.0), end: v(150.0, 11.0), k: 5.0, crest: false },
        ProfileEntity::Tangent { start: v(150.0, 11.0), end: v(200.0, 14.0) },
    ], None).unwrap();
    let p1 = ProfileEntity::ParabolaThreePoints { start: v(0.0, 10.0), middle: v(25.0, 12.0), end: v(50.0, 11.0) };
    let p2 = ProfileEntity::ParabolaThreePoints { start: v(120.0, 15.0), middle: v(150.0, 16.0), end: v(180.0, 15.0) };
    let floating = add_profile_entities(&mut doc, al, "P4", &[p1, ProfileEntity::FloatingTangent { through: v(150.0, 14.0) }], None).unwrap();
    let free = add_profile_entities(&mut doc, al, "P5", &[p1, ProfileEntity::FreeTangent, p2], None).unwrap();
    let back = roundtrip(&doc, "entities");

    // Ids in station order, each marked true for a curve, with a fixed
    // tangent's end station.
    let ids = arch(&back, three).profile_entity_ids();
    assert_eq!(ids.len(), 5);
    assert!(ids.iter().enumerate().all(|(i, (id, _, _))| *id == i as i32 + 1));
    assert_eq!(ids.iter().map(|(_, curve, _)| *curve).collect::<Vec<_>>(), [false, true, false, true, false]);
    assert_eq!(ids.iter().map(|(_, _, end)| *end).collect::<Vec<_>>(), [Some(40.0), None, Some(100.0), None, Some(200.0)]);

    let pieces = |h: Handle| arch(&back, h).profile_pieces();
    // The fixed parabolas stand on their points.
    let p = pieces(three);
    assert_eq!(p.iter().map(|x| x.0).collect::<Vec<_>>(), [ProfilePiece::Tangent, ProfilePiece::Parabola, ProfilePiece::Tangent, ProfilePiece::Parabola, ProfilePiece::Tangent]);
    assert!(p[1].1[0].distance(&v(40.0, 18.0)) < 1e-9 && p[1].1[p[1].1.len() - 1].distance(&v(60.0, 19.0)) < 1e-9);

    // The floating tangent leaves the parabola where its grade reaches the
    // point (station 25.836).
    let p = pieces(floating);
    assert_eq!(p.iter().map(|x| x.0).collect::<Vec<_>>(), [ProfilePiece::Parabola, ProfilePiece::Tangent]);
    assert!((p[1].1[0].x - 25.836129785405483).abs() < 1e-9);

    // The free tangent touches the parabolas on both sides (22.102 to
    // 134.741).
    let p = pieces(free);
    assert_eq!(p.iter().map(|x| x.0).collect::<Vec<_>>(), [ProfilePiece::Parabola, ProfilePiece::Tangent, ProfilePiece::Parabola]);
    let tangent = &p[1].1;
    assert!((tangent[0].x - 22.102416604041473).abs() < 1e-9 && (tangent[tangent.len() - 1].x - 134.74121986472957).abs() < 1e-9);
}

/// 7. Surface profiles: structurally a layout profile with no entities —
///    no elevations are stored (the application samples the surface), so
///    only the structure is asserted.
#[test]
fn rt_align_surface_profile_round_trip() {
    let v = Vector2::new;
    let mut doc = drawing();
    let al = add_alignment(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0)], None).unwrap();
    let pts = [(0.0, -10.0), (100.0, -10.0), (0.0, 10.0), (100.0, 10.0)].map(|(x, y)| acadrust::types::Vector3::new(x, y, 5.0));
    let s = add_tin_surface(&mut doc, "S1", &pts, None).unwrap();
    let eg = add_surface_profile(&mut doc, al, s, "EG", 5.0, Some((10.0, 90.0)), None).unwrap();
    let back = roundtrip(&doc, "surface");

    let p = arch(&back, eg);
    assert_eq!(p.profile_name().as_deref(), Some("EG"));
    // No entities, so no elevations: the application samples the surface
    // when it rebuilds (the documented limitation).
    assert!(p.profile_pieces().is_empty());
    assert!(p.profile_pvis().is_empty());
    // The alignment is the reference before the surface.
    let refs = p.typed_handles();
    assert_eq!((refs[12].0, refs[13].0), (al.value(), s.value()));
    // The alignment lists the profile without an anchor.
    let a = arch(&back, al);
    assert!(a.typed_handles().iter().any(|r| r.0 == eg.value()));
    assert!(a.alignment_anchors().unwrap().is_empty());
}

/// 8. Offset profiles: the source profile copied to the offset alignment's
///    stations and raised by the slope across the offset, cut where the
///    offset alignment's elements change.
#[test]
fn rt_align_offset_profile_round_trip() {
    let v = Vector2::new;
    let mut doc = drawing();
    let al = add_alignment_with_curves(&mut doc, "AL1", &[v(0.0, 0.0), v(100.0, 0.0), v(200.0, 50.0)], &[VertexCurve::Arc(80.0)], None).unwrap();
    let fg = add_profile(&mut doc, al, "FG", &[v(0.0, 10.0), v(85.0, 15.0), v(200.0, 13.0)], &[30.0], None).unwrap();
    let off = add_offset_alignment(&mut doc, al, "OFF", -10.0, None, None).unwrap();
    let op = add_offset_profile(&mut doc, fg, off, "OP", 0.02, None).unwrap();
    let back = roundtrip(&doc, "offprofile");

    let p = arch(&back, op);
    let o = p.profile_offset().unwrap();
    assert_eq!((o.source, o.slope, o.offset), (fg.value(), 0.02, -10.0));
    // The parent's length (the writer's computed range end) ends the
    // slope's station range; it is the same value the offset alignment
    // stores as its region's end.
    let parent_end = arch(&back, off).alignment_offset_parameters().unwrap().regions[0].parent_end;
    assert_eq!(o.parent_length, parent_end);
    // The source's parabola crosses the arc's start, so it is cut in two;
    // the map follows the source's entities.
    assert_eq!(o.entity_map, [(1, vec![1]), (2, vec![4, 5]), (3, vec![2, 3])]);
    // Raised by slope * |offset|: 10 + 0.02 * 10 = 10.2 at the start.
    let pvis = p.profile_pvis();
    assert!((pvis[0].point.y - 10.2).abs() < 1e-12);
    assert!((pvis[pvis.len() - 1].point.x - 195.3635239099919).abs() < 1e-9);
    // The parent's anchors: the offset alignment's range ends (kind 6),
    // the source profile's start anchor (kind 32763), the offset
    // profile's (kind 9 at the source's start, 0 at its end station — the
    // source's stations, here 0 to 200).
    let parent_anchors = arch(&back, al).alignment_anchors().unwrap();
    let mut tagged: Vec<(i32, u64)> = parent_anchors.iter().map(|a| (a.kind, a.station.to_bits())).collect();
    tagged.sort();
    assert_eq!(tagged, [(0, 200.0f64.to_bits()), (6, 0.0f64.to_bits()), (6, parent_end.to_bits()), (9, 0.0f64.to_bits()), (32763, 0.0f64.to_bits())]);
    for (station, id) in &o.anchors {
        let at = parent_anchors.iter().find(|a| a.id == *id).unwrap();
        assert_eq!(at.station, *station);
    }
    let own = arch(&back, off).alignment_anchors().unwrap();
    assert_eq!(own.len(), 2);
    assert!(own.iter().any(|a| a.kind == 32763));
}

/// The application stores a created-by name before the alignment's own
/// (2027 samples): the reader must not take the author for the name.
/// Oracle: the application reads the three alignments of the tutorials'
/// Sections-Project-Objects.dwg as "Centerline (1)", "Left (1)" and
/// "Right (1)" (execute_civil_code `Alignment.Name`, 2026-11-23).
#[test]
fn an_app_written_alignments_name_is_not_its_created_by_name() {
    // The application's tutorial drawing, from `ACADRUST_CIVIL_TUTORIALS_DIR`.
    let Some(dir) = std::env::var_os("ACADRUST_CIVIL_TUTORIALS_DIR") else { return };
    let path = std::path::Path::new(&dir).join("Sections-Project-Objects.dwg");
    if !path.exists() {
        return;
    }
    let doc = DwgReader::from_file(&path).unwrap().read().unwrap();
    let names: Vec<String> = doc
        .entities()
        .filter_map(|e| match e {
            EntityType::Extended(x) => match &x.data {
                ExtendedEntityData::Arch(a) if a.dxf_name == "AECC_ALIGNMENT" => a.alignment_name(),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(names, vec!["Centerline (1)".to_string(), "Left (1)".to_string(), "Right (1)".to_string()]);
}
