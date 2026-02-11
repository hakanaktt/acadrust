//! DWG vs DXF comparison tests
//!
//! Reference samples contain identical data in both DWG and DXF formats.
//! These tests read both and verify that the parsed CadDocument contents match.
//!
//! The DWG reader is still under development, so some tests use soft assertions
//! (report mismatches without failing) while structural invariants are strict.

use acadrust::entities::EntityType;
use acadrust::io::dxf::{DxfReader, DxfReaderConfiguration};
use acadrust::io::dwg::{DwgReader, DwgReaderConfiguration};
use acadrust::CadDocument;
use std::collections::BTreeMap;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Read a DWG file in failsafe mode.
fn read_dwg(path: &str) -> CadDocument {
    let config = DwgReaderConfiguration {
        failsafe: true,
        ..Default::default()
    };
    DwgReader::from_file(path)
        .unwrap_or_else(|e| panic!("Cannot open DWG {path}: {e:?}"))
        .with_config(config)
        .read()
        .unwrap_or_else(|e| panic!("Failed to read DWG {path}: {e:?}"))
}

/// Read a DXF file in failsafe mode.
fn read_dxf(path: &str) -> CadDocument {
    let config = DxfReaderConfiguration { failsafe: true };
    DxfReader::from_file(path)
        .unwrap_or_else(|e| panic!("Cannot open DXF {path}: {e:?}"))
        .with_configuration(config)
        .read()
        .unwrap_or_else(|e| panic!("Failed to read DXF {path}: {e:?}"))
}

/// Return the DXF-style type name for an EntityType variant.
fn entity_type_name(e: &EntityType) -> &'static str {
    e.as_entity().entity_type()
}

/// Build a sorted frequency map of entity type names.
fn entity_type_histogram(doc: &CadDocument) -> BTreeMap<&'static str, usize> {
    let mut map = BTreeMap::new();
    for e in doc.entities() {
        *map.entry(entity_type_name(e)).or_insert(0) += 1;
    }
    map
}

/// Collect sorted layer names.
fn layer_names(doc: &CadDocument) -> Vec<String> {
    let mut names: Vec<_> = doc.layers.iter().map(|l| l.name.clone()).collect();
    names.sort();
    names
}

/// Collect sorted line-type names.
fn linetype_names(doc: &CadDocument) -> Vec<String> {
    let mut names: Vec<_> = doc.line_types.iter().map(|lt| lt.name.clone()).collect();
    names.sort();
    names
}

/// Collect sorted text-style names.
fn textstyle_names(doc: &CadDocument) -> Vec<String> {
    let mut names: Vec<_> = doc.text_styles.iter().map(|ts| ts.name.clone()).collect();
    names.sort();
    names
}

/// Collect sorted block-record names.
fn block_record_names(doc: &CadDocument) -> Vec<String> {
    let mut names: Vec<_> = doc.block_records.iter().map(|br| br.name.clone()).collect();
    names.sort();
    names
}

/// Approximate equality for f64 values.
fn approx_eq(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() < tol
}

/// Collect entities grouped by type name, sorted by a geometric key within each group.
fn sorted_entities_by_type(doc: &CadDocument) -> BTreeMap<&'static str, Vec<&EntityType>> {
    let mut groups: BTreeMap<&'static str, Vec<&EntityType>> = BTreeMap::new();
    for e in doc.entities() {
        groups.entry(entity_type_name(e)).or_default().push(e);
    }
    for entities in groups.values_mut() {
        entities.sort_by(|a, b| {
            entity_sort_key(a)
                .partial_cmp(&entity_sort_key(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    groups
}

/// A rough numeric sort key for an entity (based on first geometric coordinate).
fn entity_sort_key(e: &EntityType) -> (f64, f64, f64) {
    match e {
        EntityType::Line(l) => (l.start.x, l.start.y, l.start.z),
        EntityType::Circle(c) => (c.center.x, c.center.y, c.center.z),
        EntityType::Arc(a) => (a.center.x, a.center.y, a.center.z),
        EntityType::Ellipse(el) => (el.center.x, el.center.y, el.center.z),
        EntityType::Point(p) => (p.location.x, p.location.y, p.location.z),
        EntityType::Text(t) => (t.insertion_point.x, t.insertion_point.y, t.insertion_point.z),
        EntityType::MText(t) => (t.insertion_point.x, t.insertion_point.y, t.insertion_point.z),
        EntityType::LwPolyline(lw) => {
            if let Some(v) = lw.vertices.first() {
                (v.location.x, v.location.y, lw.elevation)
            } else {
                (0.0, 0.0, 0.0)
            }
        }
        EntityType::Spline(s) => {
            if let Some(cp) = s.control_points.first() {
                (cp.x, cp.y, cp.z)
            } else {
                (0.0, 0.0, 0.0)
            }
        }
        EntityType::Insert(ins) => (ins.insert_point.x, ins.insert_point.y, ins.insert_point.z),
        EntityType::Ray(r) => (r.base_point.x, r.base_point.y, r.base_point.z),
        EntityType::Solid(s) => (s.first_corner.x, s.first_corner.y, s.first_corner.z),
        EntityType::Face3D(f) => (f.first_corner.x, f.first_corner.y, f.first_corner.z),
        EntityType::Hatch(h) => (h.elevation, h.pattern_angle, h.pattern_scale),
        EntityType::Leader(l) => {
            if let Some(v) = l.vertices.first() {
                (v.x, v.y, v.z)
            } else {
                (0.0, 0.0, 0.0)
            }
        }
        EntityType::Tolerance(t) => {
            (t.insertion_point.x, t.insertion_point.y, t.insertion_point.z)
        }
        EntityType::Shape(s) => (s.insertion_point.x, s.insertion_point.y, s.insertion_point.z),
        EntityType::Viewport(vp) => (vp.center.x, vp.center.y, vp.center.z),
        EntityType::XLine(xl) => (xl.base_point.x, xl.base_point.y, xl.base_point.z),
        _ => {
            let c = e.common();
            (c.handle.value() as f64, 0.0, 0.0)
        }
    }
}

// ---------------------------------------------------------------------------
// Per-entity-type geometry comparison
// ---------------------------------------------------------------------------

const TOL: f64 = 1e-6;

/// Compare two entities of the same type. Returns a list of mismatches (empty = OK).
fn compare_entity_geometry(dwg_e: &EntityType, dxf_e: &EntityType) -> Vec<String> {
    let mut diffs = Vec::new();

    // Layer should match
    let dwg_layer = &dwg_e.common().layer;
    let dxf_layer = &dxf_e.common().layer;
    if dwg_layer != dxf_layer {
        diffs.push(format!("layer: DWG={dwg_layer:?} DXF={dxf_layer:?}"));
    }

    match (dwg_e, dxf_e) {
        (EntityType::Line(dw), EntityType::Line(dx)) => {
            check_vec3(&mut diffs, "start", &dw.start, &dx.start);
            check_vec3(&mut diffs, "end", &dw.end, &dx.end);
        }
        (EntityType::Circle(dw), EntityType::Circle(dx)) => {
            check_vec3(&mut diffs, "center", &dw.center, &dx.center);
            check_f64(&mut diffs, "radius", dw.radius, dx.radius);
        }
        (EntityType::Arc(dw), EntityType::Arc(dx)) => {
            check_vec3(&mut diffs, "center", &dw.center, &dx.center);
            check_f64(&mut diffs, "radius", dw.radius, dx.radius);
            check_f64(&mut diffs, "start_angle", dw.start_angle, dx.start_angle);
            check_f64(&mut diffs, "end_angle", dw.end_angle, dx.end_angle);
        }
        (EntityType::Ellipse(dw), EntityType::Ellipse(dx)) => {
            check_vec3(&mut diffs, "center", &dw.center, &dx.center);
            check_vec3(&mut diffs, "major_axis", &dw.major_axis, &dx.major_axis);
            check_f64(&mut diffs, "minor_axis_ratio", dw.minor_axis_ratio, dx.minor_axis_ratio);
            check_f64(&mut diffs, "start_parameter", dw.start_parameter, dx.start_parameter);
            check_f64(&mut diffs, "end_parameter", dw.end_parameter, dx.end_parameter);
        }
        (EntityType::Point(dw), EntityType::Point(dx)) => {
            check_vec3(&mut diffs, "location", &dw.location, &dx.location);
        }
        (EntityType::Text(dw), EntityType::Text(dx)) => {
            check_vec3(&mut diffs, "insertion_point", &dw.insertion_point, &dx.insertion_point);
            check_f64(&mut diffs, "height", dw.height, dx.height);
            if dw.value != dx.value {
                diffs.push(format!("value: DWG={:?} DXF={:?}", dw.value, dx.value));
            }
        }
        (EntityType::MText(dw), EntityType::MText(dx)) => {
            check_vec3(&mut diffs, "insertion_point", &dw.insertion_point, &dx.insertion_point);
            check_f64(&mut diffs, "height", dw.height, dx.height);
            if dw.value != dx.value {
                diffs.push(format!("value: DWG={:?} DXF={:?}", dw.value, dx.value));
            }
        }
        (EntityType::LwPolyline(dw), EntityType::LwPolyline(dx)) => {
            if dw.vertices.len() != dx.vertices.len() {
                diffs.push(format!(
                    "vertex_count: DWG={} DXF={}",
                    dw.vertices.len(),
                    dx.vertices.len()
                ));
            } else {
                for (i, (vd, vx)) in dw.vertices.iter().zip(dx.vertices.iter()).enumerate() {
                    if !approx_eq(vd.location.x, vx.location.x, TOL)
                        || !approx_eq(vd.location.y, vx.location.y, TOL)
                    {
                        diffs.push(format!(
                            "vertex[{i}]: DWG=({},{}) DXF=({},{})",
                            vd.location.x, vd.location.y, vx.location.x, vx.location.y
                        ));
                    }
                    if !approx_eq(vd.bulge, vx.bulge, TOL) {
                        diffs.push(format!(
                            "vertex[{i}].bulge: DWG={} DXF={}",
                            vd.bulge, vx.bulge
                        ));
                    }
                }
            }
            if dw.is_closed != dx.is_closed {
                diffs.push(format!("is_closed: DWG={} DXF={}", dw.is_closed, dx.is_closed));
            }
        }
        (EntityType::Spline(dw), EntityType::Spline(dx)) => {
            if dw.degree != dx.degree {
                diffs.push(format!("degree: DWG={} DXF={}", dw.degree, dx.degree));
            }
            if dw.control_points.len() != dx.control_points.len() {
                diffs.push(format!(
                    "control_points_count: DWG={} DXF={}",
                    dw.control_points.len(),
                    dx.control_points.len()
                ));
            } else {
                for (i, (a, b)) in
                    dw.control_points.iter().zip(dx.control_points.iter()).enumerate()
                {
                    if !approx_eq(a.x, b.x, TOL)
                        || !approx_eq(a.y, b.y, TOL)
                        || !approx_eq(a.z, b.z, TOL)
                    {
                        diffs.push(format!(
                            "control_point[{i}]: DWG=({},{},{}) DXF=({},{},{})",
                            a.x, a.y, a.z, b.x, b.y, b.z
                        ));
                    }
                }
            }
            if dw.knots.len() != dx.knots.len() {
                diffs.push(format!(
                    "knots_count: DWG={} DXF={}",
                    dw.knots.len(),
                    dx.knots.len()
                ));
            }
        }
        (EntityType::Insert(dw), EntityType::Insert(dx)) => {
            if dw.block_name != dx.block_name {
                diffs.push(format!(
                    "block_name: DWG={:?} DXF={:?}",
                    dw.block_name, dx.block_name
                ));
            }
            check_vec3(&mut diffs, "insert_point", &dw.insert_point, &dx.insert_point);
            check_f64(&mut diffs, "x_scale", dw.x_scale, dx.x_scale);
            check_f64(&mut diffs, "y_scale", dw.y_scale, dx.y_scale);
            check_f64(&mut diffs, "z_scale", dw.z_scale, dx.z_scale);
            check_f64(&mut diffs, "rotation", dw.rotation, dx.rotation);
        }
        (EntityType::Ray(dw), EntityType::Ray(dx)) => {
            check_vec3(&mut diffs, "base_point", &dw.base_point, &dx.base_point);
            check_vec3(&mut diffs, "direction", &dw.direction, &dx.direction);
        }
        (EntityType::XLine(dw), EntityType::XLine(dx)) => {
            check_vec3(&mut diffs, "base_point", &dw.base_point, &dx.base_point);
            check_vec3(&mut diffs, "direction", &dw.direction, &dx.direction);
        }
        (EntityType::Solid(dw), EntityType::Solid(dx)) => {
            check_vec3(&mut diffs, "first_corner", &dw.first_corner, &dx.first_corner);
            check_vec3(&mut diffs, "second_corner", &dw.second_corner, &dx.second_corner);
            check_vec3(&mut diffs, "third_corner", &dw.third_corner, &dx.third_corner);
            check_vec3(&mut diffs, "fourth_corner", &dw.fourth_corner, &dx.fourth_corner);
        }
        (EntityType::Face3D(dw), EntityType::Face3D(dx)) => {
            check_vec3(&mut diffs, "first_corner", &dw.first_corner, &dx.first_corner);
            check_vec3(&mut diffs, "second_corner", &dw.second_corner, &dx.second_corner);
            check_vec3(&mut diffs, "third_corner", &dw.third_corner, &dx.third_corner);
            check_vec3(&mut diffs, "fourth_corner", &dw.fourth_corner, &dx.fourth_corner);
        }
        (EntityType::Hatch(dw), EntityType::Hatch(dx)) => {
            if dw.pattern.name != dx.pattern.name {
                diffs.push(format!(
                    "pattern_name: DWG={:?} DXF={:?}",
                    dw.pattern.name, dx.pattern.name
                ));
            }
            if dw.is_solid != dx.is_solid {
                diffs.push(format!("is_solid: DWG={} DXF={}", dw.is_solid, dx.is_solid));
            }
            check_f64(&mut diffs, "pattern_scale", dw.pattern_scale, dx.pattern_scale);
        }
        (EntityType::Leader(dw), EntityType::Leader(dx)) => {
            if dw.vertices.len() != dx.vertices.len() {
                diffs.push(format!(
                    "vertex_count: DWG={} DXF={}",
                    dw.vertices.len(),
                    dx.vertices.len()
                ));
            } else {
                for (i, (a, b)) in dw.vertices.iter().zip(dx.vertices.iter()).enumerate() {
                    if !approx_eq(a.x, b.x, TOL)
                        || !approx_eq(a.y, b.y, TOL)
                        || !approx_eq(a.z, b.z, TOL)
                    {
                        diffs.push(format!(
                            "vertex[{i}]: DWG=({},{},{}) DXF=({},{},{})",
                            a.x, a.y, a.z, b.x, b.y, b.z
                        ));
                    }
                }
            }
        }
        (EntityType::Tolerance(dw), EntityType::Tolerance(dx)) => {
            check_vec3(&mut diffs, "insertion_point", &dw.insertion_point, &dx.insertion_point);
            if dw.text != dx.text {
                diffs.push(format!("text: DWG={:?} DXF={:?}", dw.text, dx.text));
            }
        }
        (EntityType::Shape(dw), EntityType::Shape(dx)) => {
            check_vec3(&mut diffs, "insertion_point", &dw.insertion_point, &dx.insertion_point);
            check_f64(&mut diffs, "size", dw.size, dx.size);
        }
        (EntityType::Viewport(dw), EntityType::Viewport(dx)) => {
            check_vec3(&mut diffs, "center", &dw.center, &dx.center);
            check_f64(&mut diffs, "width", dw.width, dx.width);
            check_f64(&mut diffs, "height", dw.height, dx.height);
        }
        _ => {}
    }
    diffs
}

fn check_vec3(diffs: &mut Vec<String>, name: &str, a: &acadrust::Vector3, b: &acadrust::Vector3) {
    if !approx_eq(a.x, b.x, TOL) || !approx_eq(a.y, b.y, TOL) || !approx_eq(a.z, b.z, TOL) {
        diffs.push(format!(
            "{name}: DWG=({},{},{}) DXF=({},{},{})",
            a.x, a.y, a.z, b.x, b.y, b.z
        ));
    }
}

fn check_f64(diffs: &mut Vec<String>, name: &str, a: f64, b: f64) {
    if !approx_eq(a, b, TOL) {
        diffs.push(format!("{name}: DWG={a} DXF={b}"));
    }
}

// ---------------------------------------------------------------------------
// Comparison report for a single version
// ---------------------------------------------------------------------------

struct ComparisonResult {
    version: String,
    dwg_entity_count: usize,
    dxf_entity_count: usize,
    dwg_layers: usize,
    dxf_layers: usize,
    dwg_linetypes: usize,
    dxf_linetypes: usize,
    dwg_blocks: usize,
    dxf_blocks: usize,
    matched_entities: usize,
}

fn compare_version(version: &str, dwg_path: &str, dxf_path: &str) -> ComparisonResult {
    let dwg = read_dwg(dwg_path);
    let dxf = read_dxf(dxf_path);

    // Geometry comparison: compare entities of the same type, matched by sort key
    let dwg_by_type = sorted_entities_by_type(&dwg);
    let dxf_by_type = sorted_entities_by_type(&dxf);

    let mut matched = 0usize;

    for (type_name, dwg_ents) in &dwg_by_type {
        if let Some(dxf_ents) = dxf_by_type.get(type_name) {
            let count = dwg_ents.len().min(dxf_ents.len());
            for i in 0..count {
                let diffs = compare_entity_geometry(dwg_ents[i], dxf_ents[i]);
                if diffs.is_empty() {
                    matched += 1;
                }
            }
        }
    }

    ComparisonResult {
        version: version.to_string(),
        dwg_entity_count: dwg.entities().count(),
        dxf_entity_count: dxf.entities().count(),
        dwg_layers: dwg.layers.len(),
        dxf_layers: dxf.layers.len(),
        dwg_linetypes: dwg.line_types.len(),
        dxf_linetypes: dxf.line_types.len(),
        dwg_blocks: dwg.block_records.len(),
        dxf_blocks: dxf.block_records.len(),
        matched_entities: matched,
    }
}

// ===========================================================================
// Per-version tests: table names (DWG tables should be subset of DXF tables)
// ===========================================================================

/// Check that every layer read from DWG also exists in DXF.
fn assert_dwg_layers_subset_of_dxf(version: &str, dwg: &CadDocument, dxf: &CadDocument) {
    let dxf_names: Vec<_> = dxf.layers.iter().map(|l| l.name.clone()).collect();
    for layer in dwg.layers.iter() {
        assert!(
            dxf_names.contains(&layer.name),
            "[{version}] DWG layer {:?} not found in DXF layers",
            layer.name
        );
    }
}

/// Check that every line type read from DWG also exists in DXF.
fn assert_dwg_linetypes_subset_of_dxf(version: &str, dwg: &CadDocument, dxf: &CadDocument) {
    let dxf_names: Vec<_> = dxf.line_types.iter().map(|lt| lt.name.clone()).collect();
    for lt in dwg.line_types.iter() {
        assert!(
            dxf_names.contains(&lt.name),
            "[{version}] DWG linetype {:?} not found in DXF linetypes",
            lt.name
        );
    }
}

/// Check that every text style read from DWG also exists in DXF.
fn assert_dwg_textstyles_subset_of_dxf(version: &str, dwg: &CadDocument, dxf: &CadDocument) {
    let dxf_names: Vec<_> = dxf.text_styles.iter().map(|ts| ts.name.clone()).collect();
    for ts in dwg.text_styles.iter() {
        assert!(
            dxf_names.contains(&ts.name),
            "[{version}] DWG textstyle {:?} not found in DXF textstyles",
            ts.name
        );
    }
}

/// Check that every entity type appearing in DWG also appears in DXF.
/// Some entity types (e.g. ATTDEF) may live inside block records in DXF
/// but appear at top-level in DWG, so we skip those known edge cases.
fn assert_dwg_entity_types_subset_of_dxf(version: &str, dwg: &CadDocument, dxf: &CadDocument) {
    // Entity types that DWG may enumerate at top-level but DXF keeps inside blocks
    const BLOCK_INTERNAL_TYPES: &[&str] = &["ATTDEF", "ATTRIB", "BLOCK", "ENDBLK", "SEQEND"];

    let dwg_hist = entity_type_histogram(dwg);
    let dxf_hist = entity_type_histogram(dxf);
    for (key, &dwg_count) in &dwg_hist {
        if BLOCK_INTERNAL_TYPES.contains(key) {
            continue;
        }
        let dxf_count = dxf_hist.get(key).copied().unwrap_or(0);
        assert!(
            dxf_count > 0,
            "[{version}] DWG has {dwg_count} {key} entities but DXF has none"
        );
    }
}

// ===========================================================================
// AC1015 (R2000)
// ===========================================================================

#[test]
fn test_dwg_vs_dxf_ac1015_tables_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1015.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1015_ascii.dxf");

    assert_dwg_layers_subset_of_dxf("AC1015", &dwg, &dxf);
    assert_dwg_linetypes_subset_of_dxf("AC1015", &dwg, &dxf);
    assert_dwg_textstyles_subset_of_dxf("AC1015", &dwg, &dxf);
}

#[test]
fn test_dwg_vs_dxf_ac1015_entity_types_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1015.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1015_ascii.dxf");
    assert_dwg_entity_types_subset_of_dxf("AC1015", &dwg, &dxf);
}

#[test]
fn test_dwg_vs_dxf_ac1015_both_have_entities() {
    let dwg = read_dwg("reference_samples/sample_AC1015.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1015_ascii.dxf");
    assert!(dwg.entities().count() > 0, "AC1015 DWG should have entities");
    assert!(dxf.entities().count() > 0, "AC1015 DXF should have entities");
}

// ===========================================================================
// AC1018 (R2004)
// ===========================================================================

#[test]
fn test_dwg_vs_dxf_ac1018_tables_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1018.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1018_ascii.dxf");

    assert_dwg_layers_subset_of_dxf("AC1018", &dwg, &dxf);
    assert_dwg_linetypes_subset_of_dxf("AC1018", &dwg, &dxf);
    assert_dwg_textstyles_subset_of_dxf("AC1018", &dwg, &dxf);
}

#[test]
fn test_dwg_vs_dxf_ac1018_entity_types_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1018.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1018_ascii.dxf");
    assert_dwg_entity_types_subset_of_dxf("AC1018", &dwg, &dxf);
}

#[test]
fn test_dwg_vs_dxf_ac1018_both_have_entities() {
    let dwg = read_dwg("reference_samples/sample_AC1018.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1018_ascii.dxf");
    assert!(dwg.entities().count() > 0, "AC1018 DWG should have entities");
    assert!(dxf.entities().count() > 0, "AC1018 DXF should have entities");
}

// ===========================================================================
// AC1024 (R2010)
// ===========================================================================

#[test]
fn test_dwg_vs_dxf_ac1024_tables_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1024.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1024_ascii.dxf");

    assert_dwg_layers_subset_of_dxf("AC1024", &dwg, &dxf);
    assert_dwg_linetypes_subset_of_dxf("AC1024", &dwg, &dxf);
    assert_dwg_textstyles_subset_of_dxf("AC1024", &dwg, &dxf);
}

#[test]
fn test_dwg_vs_dxf_ac1024_entity_types_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1024.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1024_ascii.dxf");
    assert_dwg_entity_types_subset_of_dxf("AC1024", &dwg, &dxf);
}

// ===========================================================================
// AC1027 (R2013)
// ===========================================================================

#[test]
fn test_dwg_vs_dxf_ac1027_tables_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1027.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1027_ascii.dxf");

    assert_dwg_layers_subset_of_dxf("AC1027", &dwg, &dxf);
    assert_dwg_linetypes_subset_of_dxf("AC1027", &dwg, &dxf);
    assert_dwg_textstyles_subset_of_dxf("AC1027", &dwg, &dxf);
}

#[test]
fn test_dwg_vs_dxf_ac1027_entity_types_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1027.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1027_ascii.dxf");
    assert_dwg_entity_types_subset_of_dxf("AC1027", &dwg, &dxf);
}

// ===========================================================================
// AC1032 (R2018)
// ===========================================================================

#[test]
fn test_dwg_vs_dxf_ac1032_tables_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1032.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1032_ascii.dxf");

    assert_dwg_layers_subset_of_dxf("AC1032", &dwg, &dxf);
    assert_dwg_linetypes_subset_of_dxf("AC1032", &dwg, &dxf);
    assert_dwg_textstyles_subset_of_dxf("AC1032", &dwg, &dxf);
}

#[test]
fn test_dwg_vs_dxf_ac1032_entity_types_subset() {
    let dwg = read_dwg("reference_samples/sample_AC1032.dwg");
    let dxf = read_dxf("reference_samples/sample_AC1032_ascii.dxf");
    assert_dwg_entity_types_subset_of_dxf("AC1032", &dwg, &dxf);
}

// ===========================================================================
// Full diagnostic report across all versions
// ===========================================================================

#[test]
fn test_dwg_vs_dxf_full_report() {
    let versions: &[(&str, &str, &str)] = &[
        (
            "AC1015",
            "reference_samples/sample_AC1015.dwg",
            "reference_samples/sample_AC1015_ascii.dxf",
        ),
        (
            "AC1018",
            "reference_samples/sample_AC1018.dwg",
            "reference_samples/sample_AC1018_ascii.dxf",
        ),
        // AC1021 skipped — RS encoding not fully implemented
        (
            "AC1024",
            "reference_samples/sample_AC1024.dwg",
            "reference_samples/sample_AC1024_ascii.dxf",
        ),
        (
            "AC1027",
            "reference_samples/sample_AC1027.dwg",
            "reference_samples/sample_AC1027_ascii.dxf",
        ),
        (
            "AC1032",
            "reference_samples/sample_AC1032.dwg",
            "reference_samples/sample_AC1032_ascii.dxf",
        ),
    ];

    println!("\n{:=<90}", "");
    println!("DWG vs DXF Comparison Report");
    println!("{:=<90}\n", "");

    println!(
        "{:<10} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
        "Version", "DWG Ent", "DXF Ent", "DWG Lyr", "DXF Lyr", "DWG LT", "DXF LT", "DWG Blk",
        "DXF Blk", "Matched"
    );
    println!("{:-<90}", "");

    let mut all_results = Vec::new();

    for (ver, dwg_path, dxf_path) in versions {
        let r = compare_version(ver, dwg_path, dxf_path);
        println!(
            "{:<10} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}",
            r.version,
            r.dwg_entity_count,
            r.dxf_entity_count,
            r.dwg_layers,
            r.dxf_layers,
            r.dwg_linetypes,
            r.dxf_linetypes,
            r.dwg_blocks,
            r.dxf_blocks,
            r.matched_entities,
        );
        all_results.push(r);
    }

    println!();

    // Detailed histogram per version
    for (ver, dwg_path, dxf_path) in versions {
        let dwg = read_dwg(dwg_path);
        let dxf = read_dxf(dxf_path);

        // Table name comparison
        let dwg_ly = layer_names(&dwg);
        let dxf_ly = layer_names(&dxf);
        let dwg_lt = linetype_names(&dwg);
        let dxf_lt = linetype_names(&dxf);
        let dwg_ts = textstyle_names(&dwg);
        let dxf_ts = textstyle_names(&dxf);
        let dwg_br = block_record_names(&dwg);
        let dxf_br = block_record_names(&dxf);

        println!("[{ver}] Table names:");
        println!("  Layers     match={:<5} DWG={dwg_ly:?}", dwg_ly == dxf_ly);
        println!("  LineTypes  match={:<5} DWG={dwg_lt:?}", dwg_lt == dxf_lt);
        println!("  TextStyles match={:<5} DWG={dwg_ts:?}", dwg_ts == dxf_ts);
        println!("  Blocks     match={:<5} DWG={dwg_br:?}", dwg_br == dxf_br);

        let dwg_hist = entity_type_histogram(&dwg);
        let dxf_hist = entity_type_histogram(&dxf);

        let mut all_keys: Vec<&str> = dwg_hist.keys().chain(dxf_hist.keys()).copied().collect();
        all_keys.sort();
        all_keys.dedup();

        println!("[{ver}] Entity type histogram:");
        for key in &all_keys {
            let dw = dwg_hist.get(key).copied().unwrap_or(0);
            let dx = dxf_hist.get(key).copied().unwrap_or(0);
            let marker = if dw == dx {
                "  OK"
            } else if dw == 0 {
                "  MISSING_IN_DWG"
            } else if dx == 0 {
                "  EXTRA_IN_DWG"
            } else {
                "  COUNT_DIFF"
            };
            println!("  {key:<25} DWG={dw:<4} DXF={dx:<4}{marker}");
        }
        println!();
    }

    println!("{:=<90}\n", "");

    // Structural assertions: every version should read at least some data
    for r in &all_results {
        assert!(
            r.dwg_entity_count > 0 || r.dwg_blocks >= 2,
            "[{}] DWG reader produced no entities and <2 blocks",
            r.version
        );
    }
}

// ===========================================================================
// Entity-by-entity geometry matching detail tests
// ===========================================================================

fn run_geometry_detail_test(version: &str, dwg_path: &str, dxf_path: &str) {
    let dwg = read_dwg(dwg_path);
    let dxf = read_dxf(dxf_path);

    let dwg_by_type = sorted_entities_by_type(&dwg);
    let dxf_by_type = sorted_entities_by_type(&dxf);

    let mut total_compared = 0usize;
    let mut total_matched = 0usize;
    let mut all_diffs: Vec<String> = Vec::new();

    for (type_name, dwg_ents) in &dwg_by_type {
        if let Some(dxf_ents) = dxf_by_type.get(type_name) {
            let count = dwg_ents.len().min(dxf_ents.len());
            for i in 0..count {
                let diffs = compare_entity_geometry(dwg_ents[i], dxf_ents[i]);
                total_compared += 1;
                if diffs.is_empty() {
                    total_matched += 1;
                } else {
                    for d in &diffs {
                        all_diffs.push(format!("{type_name}[{i}] {d}"));
                    }
                }
            }
        }
    }

    println!(
        "\n[{version}] Geometry comparison: {total_compared} entities compared, \
         {total_matched} fully matched, {} with diffs",
        total_compared - total_matched
    );
    for d in &all_diffs {
        println!("  {d}");
    }

    if total_compared > 0 {
        let match_rate = total_matched as f64 / total_compared as f64;
        println!("  Match rate: {:.1}%", match_rate * 100.0);
    }
}

#[test]
fn test_dwg_vs_dxf_ac1015_geometry_detail() {
    run_geometry_detail_test(
        "AC1015",
        "reference_samples/sample_AC1015.dwg",
        "reference_samples/sample_AC1015_ascii.dxf",
    );
}

#[test]
fn test_dwg_vs_dxf_ac1018_geometry_detail() {
    run_geometry_detail_test(
        "AC1018",
        "reference_samples/sample_AC1018.dwg",
        "reference_samples/sample_AC1018_ascii.dxf",
    );
}

#[test]
fn test_dwg_vs_dxf_ac1024_geometry_detail() {
    run_geometry_detail_test(
        "AC1024",
        "reference_samples/sample_AC1024.dwg",
        "reference_samples/sample_AC1024_ascii.dxf",
    );
}

#[test]
fn test_dwg_vs_dxf_ac1027_geometry_detail() {
    run_geometry_detail_test(
        "AC1027",
        "reference_samples/sample_AC1027.dwg",
        "reference_samples/sample_AC1027_ascii.dxf",
    );
}

#[test]
fn test_dwg_vs_dxf_ac1032_geometry_detail() {
    run_geometry_detail_test(
        "AC1032",
        "reference_samples/sample_AC1032.dwg",
        "reference_samples/sample_AC1032_ascii.dxf",
    );
}
