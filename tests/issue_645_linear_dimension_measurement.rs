//! Issue #645: a rotated (linear) dimension measures the distance between its extension-line
//! origins projected onto the dimension-line direction, not the direct distance
//! between them. `DimensionLinear::new`/`rotated`/`vertical` cached the direct
//! distance in `actual_measurement`, and the DXF reader kept that cached value
//! when the optional group 42 is missing, so a diagonal pair of origins on a
//! horizontal or vertical dimension read back with the wrong measurement.

use acadrust::entities::{Dimension, DimensionLinear};
use acadrust::io::dxf::DxfReader;
use acadrust::types::Vector3;
use acadrust::EntityType;
use std::io::Cursor;

/// A minimal AC1015 DXF holding one rotated DIMENSION as AutoCAD writes it:
/// extension-line origins (13, 14) diagonal to each other, rotation `angle`
/// (group 50, omitted when `None`), measurement (group 42) only when given,
/// and the OCS normal (210/220/230).
fn rotated_dimension_dxf(angle: Option<f64>, measurement: Option<f64>, normal_z: f64) -> String {
    let mut s = String::from(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n0\nENDSEC\n\
         0\nSECTION\n2\nENTITIES\n\
         0\nDIMENSION\n5\n2A\n100\nAcDbEntity\n8\n0\n100\nAcDbDimension\n\
         10\n40.0\n20\n80.0\n30\n0.0\n11\n70.0\n21\n80.0\n31\n0.0\n70\n32\n",
    );
    if let Some(m) = measurement {
        s.push_str(&format!("42\n{m}\n"));
    }
    s.push_str(&format!(
        "3\nStandard\n210\n0.0\n220\n0.0\n230\n{normal_z}\n\
         100\nAcDbAlignedDimension\n\
         13\n100.0\n23\n50.0\n33\n0.0\n14\n40.0\n24\n10.0\n34\n0.0\n"
    ));
    if let Some(a) = angle {
        s.push_str(&format!("50\n{a}\n"));
    }
    s.push_str("100\nAcDbRotatedDimension\n0\nENDSEC\n0\nEOF\n");
    s
}

fn read_linear(dxf: &str) -> DimensionLinear {
    let doc = DxfReader::from_reader(Cursor::new(dxf.as_bytes().to_vec()))
        .expect("open DXF")
        .read()
        .expect("read DXF");
    let linear = doc.entities().find_map(|e| match e {
        EntityType::Dimension(d) => match d.as_ref() {
            Dimension::Linear(l) => Some(l.clone()),
            _ => None,
        },
        _ => None,
    });
    linear.expect("a linear dimension")
}

fn assert_close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "{what}: expected {expected}, got {actual}"
    );
}

// Origins (100, 50) and (40, 10): dx = 60, dy = 40, direct distance ≈ 72.111.

#[test]
fn dxf_horizontal_dimension_without_group_42_measures_along_x() {
    let dim = read_linear(&rotated_dimension_dxf(None, None, 1.0));
    assert_close(dim.base.actual_measurement, 60.0, "horizontal measurement");
    assert_close(
        dim.base.actual_measurement,
        dim.measurement(),
        "cached vs measurement()",
    );
}

#[test]
fn dxf_vertical_dimension_without_group_42_measures_along_y() {
    let dim = read_linear(&rotated_dimension_dxf(Some(90.0), None, 1.0));
    assert_close(dim.base.actual_measurement, 40.0, "vertical measurement");
    assert_close(
        dim.base.actual_measurement,
        dim.measurement(),
        "cached vs measurement()",
    );
}

#[test]
fn dxf_rotated_dimension_without_group_42_measures_in_its_ocs() {
    // Normal -Z: OCS x = -WCS x, OCS y = WCS y, so the OCS difference is
    // (60, -40) and the projection onto 30° is 60·cos30° − 40·sin30° ≈ 31.962.
    // Measured in WCS instead it would be |−60·cos30° − 40·sin30°| ≈ 71.962.
    let dim = read_linear(&rotated_dimension_dxf(Some(30.0), None, -1.0));
    let (sin, cos) = 30f64.to_radians().sin_cos();
    assert_close(
        dim.base.actual_measurement,
        60.0 * cos - 40.0 * sin,
        "OCS measurement",
    );
    assert_close(
        dim.base.actual_measurement,
        dim.measurement(),
        "cached vs measurement()",
    );
}

#[test]
fn dxf_dimension_keeps_a_stored_group_42() {
    // The file's measurement is authoritative when present, even when it
    // differs from the geometry (e.g. a dimension stretched without update).
    let dim = read_linear(&rotated_dimension_dxf(None, Some(60.5), 1.0));
    assert_close(dim.base.actual_measurement, 60.5, "stored measurement");
}

#[test]
fn linear_constructors_cache_the_projected_measurement() {
    let a = Vector3::new(0.0, 0.0, 0.0);
    let b = Vector3::new(3.0, 4.0, 0.0);

    let horizontal = DimensionLinear::new(a, b);
    assert_close(horizontal.base.actual_measurement, 3.0, "new()");

    let horizontal = DimensionLinear::horizontal(a, b);
    assert_close(horizontal.base.actual_measurement, 3.0, "horizontal()");

    let vertical = DimensionLinear::vertical(a, b);
    assert_close(vertical.base.actual_measurement, 4.0, "vertical()");

    let rotated = DimensionLinear::rotated(a, b, std::f64::consts::FRAC_PI_4);
    assert_close(
        rotated.base.actual_measurement,
        7.0 * std::f64::consts::FRAC_1_SQRT_2,
        "rotated(45°)",
    );
    assert_close(
        rotated.base.actual_measurement,
        rotated.measurement(),
        "rotated(45°) cached vs measurement()",
    );
}
