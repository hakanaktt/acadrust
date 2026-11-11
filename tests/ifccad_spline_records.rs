use std::io::Cursor;

use acadrust::entities::{BoundaryEdge, BoundaryPath, EntityType, Hatch, Spline, SplineEdge};
use acadrust::types::{DxfVersion, Vector2, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

fn roundtrip_dwg(document: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(document).unwrap();
    DwgReader::from_stream(Cursor::new(bytes)).read().unwrap()
}

fn roundtrip_dxf(document: &CadDocument) -> CadDocument {
    let mut bytes = Vec::new();
    DxfWriter::new(document)
        .write_to_writer(&mut bytes)
        .unwrap();
    DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap()
}

fn spline(document: &CadDocument) -> &Spline {
    document
        .entities()
        .find_map(|entity| match entity {
            EntityType::Spline(spline) => Some(spline),
            _ => None,
        })
        .expect("standalone spline")
}

fn hatch_edge(document: &CadDocument) -> &SplineEdge {
    document
        .entities()
        .find_map(|entity| match entity {
            EntityType::Hatch(hatch) => match &hatch.paths[0].edges[0] {
                BoundaryEdge::Spline(edge) => Some(edge),
                _ => None,
            },
            _ => None,
        })
        .expect("hatch spline edge")
}

#[test]
fn hatch_spline_record_retains_fit_data_from_r2010() {
    let edge = SplineEdge {
        degree: 2,
        rational: true,
        periodic: false,
        knots: vec![0., 0., 0., 1., 1., 1.],
        control_points: vec![
            Vector3::new(0., 0., 1.),
            Vector3::new(2., 3., 0.75),
            Vector3::new(0., 0., 1.),
        ],
        fit_points: vec![Vector2::new(0., 0.), Vector2::new(0.75, 1.125)],
        start_tangent: Vector2::new(2., 3.),
        end_tangent: Vector2::new(-2., -3.),
    };
    for version in [DxfVersion::AC1021, DxfVersion::AC1024, DxfVersion::AC1032] {
        let mut hatch = Hatch::new();
        let mut path = BoundaryPath::external();
        path.add_edge(BoundaryEdge::Spline(edge.clone()));
        hatch.add_path(path);
        let mut document = CadDocument::with_version(version);
        document
            .add_entity(EntityType::Hatch(Box::new(hatch)))
            .unwrap();

        for read in [roundtrip_dwg(&document), roundtrip_dxf(&document)] {
            let observed = hatch_edge(&read);
            assert_eq!(observed.degree, edge.degree);
            assert!(observed.rational);
            assert_eq!(observed.knots, edge.knots);
            assert_eq!(observed.control_points, edge.control_points);
            if version >= DxfVersion::AC1024 {
                assert_eq!(observed, &edge);
            } else {
                assert!(observed.fit_points.is_empty());
            }
        }
    }
}

#[test]
fn dwg_preserves_compact_nonuniform_weighted_periodic_spline() {
    let mut original = Spline::new();
    original.degree = 3;
    original.flags.closed = true;
    original.flags.periodic = true;
    original.flags.rational = true;
    original.control_points = vec![
        Vector3::new(0., 0., 0.),
        Vector3::new(3., -2., 1.),
        Vector3::new(5., 4., 0.),
        Vector3::new(-1., 2., -1.),
    ];
    original.knots = vec![2., 2.5, 4., 7., 9.];
    original.weights = vec![1., 2., 0.75, 1.25];
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    document
        .add_entity(EntityType::Spline(original.clone()))
        .unwrap();

    let read = roundtrip_dwg(&document);
    let observed = spline(&read);
    assert!(observed.flags.periodic && observed.flags.closed && observed.flags.rational);
    assert_eq!(observed.knots, original.knots);
    assert_eq!(observed.control_points, original.control_points);
    assert_eq!(observed.weights, original.weights);
}

#[test]
fn dxf_preserves_an_already_clamped_periodic_definition() {
    let source = DwgReader::from_file("tests/fixtures/issue234/poly_self_intersection.dwg")
        .unwrap()
        .read()
        .unwrap();
    let generated = roundtrip_dxf(&source);
    let clamped = generated
        .entities()
        .find_map(|entity| match entity {
            EntityType::Spline(spline) if spline.flags.periodic => Some(spline.clone()),
            _ => None,
        })
        .expect("periodic definition in physical DXF");
    assert_eq!(
        clamped.knots.len(),
        clamped.control_points.len() + clamped.degree as usize + 1
    );
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    document
        .add_entity(EntityType::Spline(clamped.clone()))
        .unwrap();

    let read = roundtrip_dxf(&document);
    let observed = spline(&read);
    assert!(observed.flags.closed && observed.flags.periodic);
    assert_eq!(observed.knots, clamped.knots);
    assert_eq!(observed.control_points, clamped.control_points);
    assert_eq!(observed.weights, clamped.weights);
}

#[test]
fn dxf_clamping_preserves_common_weight_scale() {
    let source = DwgReader::from_file("tests/fixtures/issue234/poly_self_intersection.dwg")
        .unwrap()
        .read()
        .unwrap();
    let mut original = source
        .entities()
        .find_map(|entity| match entity {
            EntityType::Spline(spline) if spline.flags.periodic => Some(spline.clone()),
            _ => None,
        })
        .expect("compact periodic spline in physical DWG");
    original.flags.rational = true;
    original.weights = vec![2.; original.control_points.len()];
    let mut document = CadDocument::with_version(DxfVersion::AC1032);
    document.add_entity(EntityType::Spline(original)).unwrap();
    let read = roundtrip_dxf(&document);
    let observed = spline(&read);

    // A common weight scale is independent of the curve and survives exact
    // clamping. Defaulting only the extra control points changes the curve.
    assert_eq!(observed.weights, vec![2.; observed.control_points.len()]);
}
