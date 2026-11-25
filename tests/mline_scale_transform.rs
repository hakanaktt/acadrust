//! A scale of a multiline also scales its scale factor and the element and
//! area fill parameters of its vertices, which are lengths in drawing units.
//! Without them the parallel lines keep their old distance from the centre
//! line.

use acadrust::entities::{Entity, MLine, MLineSegment};
use acadrust::types::{Transform, Vector3};

const TOLERANCE: f64 = 1e-12;

fn mline() -> MLine {
    let mut mline = MLine::from_points(&[
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 0.0, 0.0),
        Vector3::new(10.0, 10.0, 0.0),
    ]);
    mline.scale_factor = 2.0;
    for vertex in &mut mline.vertices {
        vertex.segments = vec![
            MLineSegment {
                parameters: vec![1.0, 0.0],
                area_fill_parameters: vec![0.5, 3.0],
            },
            MLineSegment {
                parameters: vec![-1.0, 0.0],
                area_fill_parameters: Vec::new(),
            },
        ];
    }
    mline
}

fn assert_close(actual: &[f64], expected: &[f64]) {
    assert_eq!(actual.len(), expected.len(), "{actual:?} != {expected:?}");
    for (a, e) in actual.iter().zip(expected) {
        assert!((a - e).abs() < TOLERANCE, "{actual:?} != {expected:?}");
    }
}

fn assert_segments_scaled(mline: &MLine, source: &MLine, factor: f64) {
    for (vertex, original) in mline.vertices.iter().zip(&source.vertices) {
        for (segment, before) in vertex.segments.iter().zip(&original.segments) {
            let scaled = |values: &[f64]| values.iter().map(|v| v * factor).collect::<Vec<_>>();
            assert_close(&segment.parameters, &scaled(&before.parameters));
            assert_close(
                &segment.area_fill_parameters,
                &scaled(&before.area_fill_parameters),
            );
        }
    }
}

#[test]
fn a_scaled_multiline_scales_its_scale_factor_and_segment_parameters() {
    let source = mline();
    let mut mline = source.clone();
    mline.apply_transform(&Transform::from_scaling_with_origin(
        Vector3::new(2.5, 2.5, 2.5),
        Vector3::new(1.0, 1.0, 0.0),
    ));

    assert!(
        (mline.scale_factor - 5.0).abs() < TOLERANCE,
        "{}",
        mline.scale_factor
    );
    assert_segments_scaled(&mline, &source, 2.5);
    assert!((mline.vertices[1].position - Vector3::new(23.5, -1.5, 0.0)).length() < TOLERANCE);
}

#[test]
fn a_mirror_with_a_scale_keeps_the_mirror_sign_and_scales() {
    let source = mline();
    let mut mline = source.clone();
    let mirror_and_scale = Transform::from_scale(3.0) * Transform::from_mirror_x();
    mline.apply_mirror(&mirror_and_scale);

    assert!(
        (mline.scale_factor + 6.0).abs() < TOLERANCE,
        "{}",
        mline.scale_factor
    );
    assert_segments_scaled(&mline, &source, 3.0);
}

#[test]
fn a_rotated_multiline_keeps_its_scale_factor_and_segment_parameters() {
    let source = mline();
    let mut mline = source.clone();
    mline.apply_transform(&Transform::from_rotation(Vector3::UNIT_Z, 0.6));

    assert!(
        (mline.scale_factor - 2.0).abs() < TOLERANCE,
        "{}",
        mline.scale_factor
    );
    assert_segments_scaled(&mline, &source, 1.0);
}
