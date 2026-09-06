//! Comprehensive roundtrip integrity tests for DXF and DWG formats.
//!
//! These tests verify that reading and writing CAD files preserves all data
//! losslessly — both in terms of integrity (field values) and quantity
//! (entity/table/object counts).
//!
//! Strategy:
//!   1. Build a document with known entities/tables
//!   2. Write → Read → compare (single roundtrip)
//!   3. Write → Read → Write → Read → compare (double roundtrip for stability)

use std::io::Cursor;

use acadrust::entities::dimension::DimensionLinear;
use acadrust::entities::hatch::{
    BoundaryEdge, BoundaryPath, BoundaryPathFlags, CircularArcEdge, EllipticArcEdge, LineEdge,
    PolylineEdge, SplineEdge,
};
use acadrust::entities::mesh::Mesh;
use acadrust::entities::mline::MLine;
use acadrust::entities::multileader::MultiLeader;
use acadrust::entities::polyface_mesh::PolyfaceMesh;
use acadrust::entities::*;
use acadrust::tables::{LineType, LineTypeComplexContent, LineTypeElement};
use acadrust::types::{Color, DxfVersion, Handle, Vector2, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfReader, DxfWriter};

// ═══════════════════════════════════════════════════════════════════════════
//  HELPER: build a document with a rich set of entities for testing
// ═══════════════════════════════════════════════════════════════════════════

/// Builds a document populated with many different entity types.
/// Returns (doc, expected_entity_count).
fn build_rich_document(version: DxfVersion) -> (CadDocument, usize) {
    let mut doc = CadDocument::with_version(version);
    let mut count = 0usize;

    // ── Simple geometry ────────────────────────────────────────────
    doc.add_entity(EntityType::Point(Point::from_coords(50.0, 50.0, 0.0)))
        .unwrap();
    count += 1;

    doc.add_entity(EntityType::Line(Line::from_coords(
        0.0, 0.0, 0.0, 100.0, 100.0, 0.0,
    )))
    .unwrap();
    count += 1;

    doc.add_entity(EntityType::Circle(Circle::from_coords(
        50.0, 50.0, 0.0, 25.0,
    )))
    .unwrap();
    count += 1;

    doc.add_entity(EntityType::Arc(Arc::from_coords(
        50.0,
        50.0,
        0.0,
        25.0,
        0.0,
        std::f64::consts::PI,
    )))
    .unwrap();
    count += 1;

    doc.add_entity(EntityType::Ellipse(Ellipse::from_center_axes(
        Vector3::new(50.0, 50.0, 0.0),
        Vector3::new(40.0, 0.0, 0.0),
        0.5,
    )))
    .unwrap();
    count += 1;

    doc.add_entity(EntityType::Ray(Ray::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(1.0, 1.0, 0.0),
    )))
    .unwrap();
    count += 1;

    doc.add_entity(EntityType::XLine(XLine::new(
        Vector3::new(50.0, 50.0, 0.0),
        Vector3::new(1.0, 0.0, 0.0),
    )))
    .unwrap();
    count += 1;

    // ── Solids / faces ─────────────────────────────────────────────
    doc.add_entity(EntityType::Solid(Solid::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 0.0, 0.0),
        Vector3::new(10.0, 10.0, 0.0),
        Vector3::new(0.0, 10.0, 0.0),
    )))
    .unwrap();
    count += 1;

    doc.add_entity(EntityType::Face3D(Face3D::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 0.0, 0.0),
        Vector3::new(10.0, 10.0, 5.0),
        Vector3::new(0.0, 10.0, 5.0),
    )))
    .unwrap();
    count += 1;

    // ── Text ───────────────────────────────────────────────────────
    doc.add_entity(EntityType::Text(Text::with_value(
        "Hello World",
        Vector3::new(0.0, 0.0, 0.0),
    )))
    .unwrap();
    count += 1;

    doc.add_entity(EntityType::MText(MText::with_value(
        "Multi\\Pline\\PText",
        Vector3::new(0.0, 0.0, 0.0),
    )))
    .unwrap();
    count += 1;

    // ── Polylines ──────────────────────────────────────────────────
    doc.add_entity(EntityType::LwPolyline(LwPolyline::from_points(vec![
        Vector2::new(0.0, 0.0),
        Vector2::new(10.0, 0.0),
        Vector2::new(10.0, 10.0),
        Vector2::new(0.0, 10.0),
    ])))
    .unwrap();
    count += 1;

    {
        let mut pl = Polyline2D::new();
        pl.add_vertex(Vertex2D::new(Vector3::new(0.0, 0.0, 0.0)));
        pl.add_vertex(Vertex2D::new(Vector3::new(20.0, 0.0, 0.0)));
        pl.add_vertex(Vertex2D::new(Vector3::new(20.0, 20.0, 0.0)));
        doc.add_entity(EntityType::Polyline2D(pl)).unwrap();
        count += 1;
    }

    doc.add_entity(EntityType::Polyline3D(Polyline3D::from_points(vec![
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 0.0, 5.0),
        Vector3::new(20.0, 10.0, 10.0),
    ])))
    .unwrap();
    count += 1;

    doc.add_entity(EntityType::Spline(Spline::from_control_points(
        3,
        vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(5.0, 10.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(15.0, 10.0, 0.0),
        ],
    )))
    .unwrap();
    count += 1;

    // ── Annotations ────────────────────────────────────────────────
    doc.add_entity(EntityType::Leader(Leader::two_point(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 10.0, 0.0),
    )))
    .unwrap();
    count += 1;

    doc.add_entity(EntityType::Dimension(Dimension::Linear(
        DimensionLinear::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(100.0, 0.0, 0.0)),
    )))
    .unwrap();
    count += 1;

    doc.add_entity(EntityType::Tolerance(Tolerance::with_text(
        Vector3::new(10.0, 10.0, 0.0),
        "{\\Fgdt;p}%%v0.5",
    )))
    .unwrap();
    count += 1;

    doc.add_entity(EntityType::Shape(Shape::with_name(
        Vector3::new(50.0, 50.0, 0.0),
        "BOX",
        5.0,
    )))
    .unwrap();
    count += 1;

    // ── Viewport ───────────────────────────────────────────────────
    doc.add_entity(EntityType::Viewport(Viewport::new()))
        .unwrap();
    count += 1;

    // ── Insert ─────────────────────────────────────────────────────
    doc.add_entity(EntityType::Insert(Insert::new(
        "*Model_Space",
        Vector3::new(0.0, 0.0, 0.0),
    )))
    .unwrap();
    count += 1;

    // ── Hatch ──────────────────────────────────────────────────────
    {
        let mut hatch = Hatch::solid();
        let mut path = BoundaryPath::new();
        path.add_edge(BoundaryEdge::Polyline(PolylineEdge::new(
            vec![
                Vector2::new(0.0, 0.0),
                Vector2::new(100.0, 0.0),
                Vector2::new(100.0, 100.0),
                Vector2::new(0.0, 100.0),
            ],
            true,
        )));
        hatch.add_path(path);
        doc.add_entity(EntityType::Hatch(hatch)).unwrap();
        count += 1;
    }

    // ── MLine ──────────────────────────────────────────────────────
    doc.add_entity(EntityType::MLine(MLine::from_points(&[
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(50.0, 0.0, 0.0),
        Vector3::new(50.0, 50.0, 0.0),
    ])))
    .unwrap();
    count += 1;

    // ── PolyfaceMesh ───────────────────────────────────────────────
    {
        let mut pf = PolyfaceMesh::new();
        let v1 = pf.add_vertex_xyz(0.0, 0.0, 0.0);
        let v2 = pf.add_vertex_xyz(10.0, 0.0, 0.0);
        let v3 = pf.add_vertex_xyz(5.0, 10.0, 0.0);
        let v4 = pf.add_vertex_xyz(10.0, 10.0, 5.0);
        pf.add_triangle(v1, v2, v3);
        pf.add_triangle(v2, v4, v3);
        doc.add_entity(EntityType::PolyfaceMesh(pf)).unwrap();
        count += 1;
    }

    // ── MultiLeader ────────────────────────────────────────────────
    doc.add_entity(EntityType::MultiLeader(MultiLeader::with_text(
        "Label",
        Vector3::new(20.0, 20.0, 0.0),
        vec![Vector3::new(0.0, 0.0, 0.0), Vector3::new(10.0, 10.0, 0.0)],
    )))
    .unwrap();
    count += 1;

    // ── Mesh ───────────────────────────────────────────────────────
    doc.add_entity(EntityType::Mesh(Mesh::from_triangles(
        vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(5.0, 10.0, 5.0),
        ],
        &[(0, 1, 2)],
    )))
    .unwrap();
    count += 1;

    (doc, count)
}

/// Builds a minimal document with a single entity for focused testing.
fn build_minimal_document(version: DxfVersion, entity: EntityType) -> CadDocument {
    let mut doc = CadDocument::with_version(version);
    doc.add_entity(entity).unwrap();
    doc
}

// ═══════════════════════════════════════════════════════════════════════════
//  HELPER: deep comparison with detailed diagnostics
// ═══════════════════════════════════════════════════════════════════════════

/// A structured diff report between two documents.
struct DiffReport {
    differences: Vec<String>,
}

impl DiffReport {
    fn new() -> Self {
        Self {
            differences: Vec::new(),
        }
    }

    fn add(&mut self, msg: String) {
        self.differences.push(msg);
    }

    fn is_empty(&self) -> bool {
        self.differences.is_empty()
    }

    fn summary(&self) -> String {
        if self.is_empty() {
            "No differences found — PERFECT roundtrip".to_string()
        } else {
            format!(
                "{} difference(s) found:\n{}",
                self.differences.len(),
                self.differences
                    .iter()
                    .enumerate()
                    .map(|(i, d)| format!("  {}. {}", i + 1, d))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
        }
    }
}

/// Generate a human-readable diff between two Debug-format strings.
/// Shows only the lines that differ, with context.
fn generate_field_diff(orig: &str, rt: &str) -> String {
    let orig_lines: Vec<&str> = orig.lines().collect();
    let rt_lines: Vec<&str> = rt.lines().collect();
    let max_lines = orig_lines.len().max(rt_lines.len());
    let mut diffs = Vec::new();

    for i in 0..max_lines {
        let orig_line = orig_lines.get(i).unwrap_or(&"<missing>");
        let rt_line = rt_lines.get(i).unwrap_or(&"<missing>");
        if orig_line != rt_line {
            diffs.push(format!(
                "      line {}: orig: {}\n              rt:   {}",
                i + 1,
                orig_line.trim(),
                rt_line.trim()
            ));
        }
    }

    if diffs.is_empty() {
        "      (Debug repr differs but line-by-line comparison found no diff — whitespace?)"
            .to_string()
    } else if diffs.len() > 20 {
        let first10: Vec<_> = diffs[..10].to_vec();
        format!(
            "{}\n      ... and {} more differences",
            first10.join("\n"),
            diffs.len() - 10
        )
    } else {
        diffs.join("\n")
    }
}

/// Compare two documents in detail, reporting differences.
fn compare_documents(original: &CadDocument, roundtripped: &CadDocument) -> DiffReport {
    let mut report = DiffReport::new();

    // ── Version ───────────────────────────────────────────────────
    if original.version != roundtripped.version {
        report.add(format!(
            "Version mismatch: {:?} vs {:?}",
            original.version, roundtripped.version
        ));
    }

    // ── Entity counts ─────────────────────────────────────────────
    let orig_count = original.entity_count();
    let rt_count = roundtripped.entity_count();
    if orig_count != rt_count {
        report.add(format!(
            "Entity count mismatch: {} vs {}",
            orig_count, rt_count
        ));
    }

    // ── Entity type distribution ──────────────────────────────────
    let orig_types = entity_type_counts(original);
    let rt_types = entity_type_counts(roundtripped);
    if orig_types != rt_types {
        report.add(format!(
            "Entity type distribution mismatch:\n    original:    {:?}\n    roundtripped: {:?}",
            orig_types, rt_types
        ));
    }

    // ── Tables ────────────────────────────────────────────────────
    compare_table_count(
        &mut report,
        "Layer",
        original.layers.len(),
        roundtripped.layers.len(),
    );
    compare_table_count(
        &mut report,
        "LineType",
        original.line_types.len(),
        roundtripped.line_types.len(),
    );
    compare_table_count(
        &mut report,
        "TextStyle",
        original.text_styles.len(),
        roundtripped.text_styles.len(),
    );
    compare_table_count(
        &mut report,
        "BlockRecord",
        original.block_records.len(),
        roundtripped.block_records.len(),
    );
    compare_table_count(
        &mut report,
        "DimStyle",
        original.dim_styles.len(),
        roundtripped.dim_styles.len(),
    );
    compare_table_count(
        &mut report,
        "AppId",
        original.app_ids.len(),
        roundtripped.app_ids.len(),
    );
    compare_table_count(
        &mut report,
        "View",
        original.views.len(),
        roundtripped.views.len(),
    );
    compare_table_count(
        &mut report,
        "VPort",
        original.vports.len(),
        roundtripped.vports.len(),
    );
    compare_table_count(
        &mut report,
        "Ucs",
        original.ucss.len(),
        roundtripped.ucss.len(),
    );

    // ── Objects ───────────────────────────────────────────────────
    if original.objects.len() != roundtripped.objects.len() {
        report.add(format!(
            "Object count mismatch: {} vs {}",
            original.objects.len(),
            roundtripped.objects.len()
        ));
    }

    // ── Classes ───────────────────────────────────────────────────
    if original.classes.len() != roundtripped.classes.len() {
        report.add(format!(
            "Class count mismatch: {} vs {}",
            original.classes.len(),
            roundtripped.classes.len()
        ));
    }

    // ── Per-entity field comparison ───────────────────────────────
    // Match entities by type and order within each type for detailed comparison.
    compare_entities_by_type(&mut report, original, roundtripped);

    // ── Header variables (selected critical fields) ───────────────
    compare_header_variables(&mut report, &original.header, &roundtripped.header);

    report
}

fn compare_table_count(report: &mut DiffReport, name: &str, orig: usize, rt: usize) {
    if orig != rt {
        report.add(format!("{} table count mismatch: {} vs {}", name, orig, rt));
    }
}

fn entity_type_counts(doc: &CadDocument) -> std::collections::BTreeMap<String, usize> {
    let mut map = std::collections::BTreeMap::new();
    for entity in doc.entities() {
        let name = entity_variant_name(entity);
        *map.entry(name).or_insert(0) += 1;
    }
    map
}

/// Returns the Rust enum variant name for an entity, which is more precise
/// than the DXF entity type name. For example, Polyline2D, Polyline3D,
/// PolyfaceMesh, and Polyline all return "POLYLINE" from entity_type(),
/// but this function returns their distinct variant names.
fn entity_variant_name(entity: &EntityType) -> String {
    // Use Debug format to get the variant name (e.g., "Polyline2D(Polyline2D { ... })")
    // and extract just the variant prefix.
    let dbg = format!("{:?}", entity);
    if let Some(paren_pos) = dbg.find('(') {
        dbg[..paren_pos].to_string()
    } else {
        dbg
    }
}

fn compare_entities_by_type(report: &mut DiffReport, orig: &CadDocument, rt: &CadDocument) {
    // Group entities by variant name (not DXF entity type, which groups different variants)
    let mut orig_by_type: std::collections::BTreeMap<String, Vec<&EntityType>> =
        std::collections::BTreeMap::new();
    let mut rt_by_type: std::collections::BTreeMap<String, Vec<&EntityType>> =
        std::collections::BTreeMap::new();

    for e in orig.entities() {
        let name = entity_variant_name(e);
        orig_by_type.entry(name).or_default().push(e);
    }
    for e in rt.entities() {
        let name = entity_variant_name(e);
        rt_by_type.entry(name).or_default().push(e);
    }

    for (type_name, orig_entities) in &orig_by_type {
        if let Some(rt_entities) = rt_by_type.get(type_name) {
            if orig_entities.len() != rt_entities.len() {
                report.add(format!(
                    "{}: count mismatch {} vs {}",
                    type_name,
                    orig_entities.len(),
                    rt_entities.len()
                ));
                continue;
            }
            // Compare each entity pair (matched by position within type group)
            for (i, (orig_e, rt_e)) in orig_entities.iter().zip(rt_entities.iter()).enumerate() {
                compare_single_entity(report, type_name, i, orig_e, rt_e);
            }
        } else {
            report.add(format!(
                "{}: present in original ({} entities) but missing after roundtrip",
                type_name,
                orig_entities.len()
            ));
        }
    }

    // Check for extra types in roundtripped
    for type_name in rt_by_type.keys() {
        if !orig_by_type.contains_key(type_name) {
            report.add(format!(
                "{}: appeared in roundtripped but not in original",
                type_name
            ));
        }
    }
}

fn compare_single_entity(
    report: &mut DiffReport,
    type_name: &str,
    index: usize,
    orig: &EntityType,
    rt: &EntityType,
) {
    let orig_common = orig.common();
    let rt_common = rt.common();

    // Compare common entity fields (excluding handle/owner which may be reassigned)
    if orig_common.layer != rt_common.layer {
        report.add(format!(
            "{}[{}] layer: {:?} vs {:?}",
            type_name, index, orig_common.layer, rt_common.layer
        ));
    }
    if orig_common.color != rt_common.color {
        report.add(format!(
            "{}[{}] color: {:?} vs {:?}",
            type_name, index, orig_common.color, rt_common.color
        ));
    }
    if orig_common.line_weight != rt_common.line_weight {
        report.add(format!(
            "{}[{}] line_weight: {:?} vs {:?}",
            type_name, index, orig_common.line_weight, rt_common.line_weight
        ));
    }
    if orig_common.linetype != rt_common.linetype {
        report.add(format!(
            "{}[{}] linetype: {:?} vs {:?}",
            type_name, index, orig_common.linetype, rt_common.linetype
        ));
    }
    if (orig_common.linetype_scale - rt_common.linetype_scale).abs() > 1e-10 {
        report.add(format!(
            "{}[{}] linetype_scale: {} vs {}",
            type_name, index, orig_common.linetype_scale, rt_common.linetype_scale
        ));
    }
    if orig_common.invisible != rt_common.invisible {
        report.add(format!(
            "{}[{}] invisible: {} vs {}",
            type_name, index, orig_common.invisible, rt_common.invisible
        ));
    }

    // Compare type-specific geometry:
    // Normalize handles, computed fields, floats, style case, and sort edges.
    let mut orig_clone = orig.clone();
    let mut rt_clone = rt.clone();
    normalize_entity_for_comparison(&mut orig_clone);
    normalize_entity_for_comparison(&mut rt_clone);

    if orig_clone != rt_clone {
        // Generate a detailed diff by comparing Debug output line by line
        let orig_dbg = format!("{:#?}", orig_clone);
        let rt_dbg = format!("{:#?}", rt_clone);
        let diff = generate_field_diff(&orig_dbg, &rt_dbg);
        report.add(format!(
            "{}[{}] entity data differs:\n{}",
            type_name, index, diff,
        ));
    }
}

/// Normalize an EntityCommon struct by zeroing out handle-related fields.
fn normalize_entity_common(common: &mut acadrust::entities::EntityCommon) {
    common.handle = Handle::NULL;
    common.owner_handle = Handle::NULL;
    common.reactors.clear();
    common.xdictionary_handle = None;
    // entity_mode is DWG-internal and not set for programmatic documents;
    // normalize to None to avoid false differences in DWG roundtrip tests.
    common.entity_mode = None;
}

/// Comprehensive normalization for roundtrip comparison.
/// Zeros out ALL handle fields (common + entity-specific),
/// normalizes computed fields, rounds direction vectors, sorts
/// non-ordered collections, and case-normalizes style names.
fn normalize_entity_for_comparison(entity: &mut EntityType) {
    // ── Common handles ─────────────────────────────────────────
    normalize_entity_common(entity.common_mut());

    // ── Entity-specific handles & computed fields ──────────────
    match entity {
        // Polyline3D: vertex handles and layers
        EntityType::Polyline3D(p) => {
            for v in &mut p.vertices {
                v.handle = Handle::NULL;
                // Layer may be inherited from the polyline after write/read
                v.layer = String::new();
            }
        }
        // PolyfaceMesh: seqend handle + nested vertex/face EntityCommon
        EntityType::PolyfaceMesh(pf) => {
            pf.seqend_handle = None;
            for v in &mut pf.vertices {
                normalize_entity_common(&mut v.common);
            }
            for f in &mut pf.faces {
                normalize_entity_common(&mut f.common);
            }
        }
        // Hatch: boundary path handles
        EntityType::Hatch(h) => {
            for path in &mut h.paths {
                path.boundary_handles.clear();
            }
        }
        // MLine: style handle + nested style handles
        EntityType::MLine(ml) => {
            ml.style_handle = None;
        }
        // MText: this is a lossless cache of the raw DWG X-axis vector.
        // Programmatic entities leave it empty and the writer derives the
        // equivalent vector from rotation, so it is not part of semantic
        // roundtrip equality.
        EntityType::MText(m) => {
            m.dwg_x_direction = None;
        }
        // MultiLeader: many handle fields at multiple levels
        EntityType::MultiLeader(mld) => {
            mld.style_handle = None;
            mld.line_type_handle = None;
            mld.arrowhead_handle = None;
            mld.text_style_handle = None;
            mld.block_content_handle = None;
            mld.context.text_style_handle = None;
            mld.context.block_content_handle = None;
            mld.context.scale_handle = None;
            for root in &mut mld.context.leader_roots {
                for line in &mut root.lines {
                    line.line_type_handle = None;
                    line.arrowhead_handle = None;
                }
            }
            for attr in &mut mld.block_attributes {
                attr.attribute_definition_handle = None;
            }
        }
        // Tolerance: dimension style handle
        EntityType::Tolerance(t) => {
            t.dimension_style_handle = None;
        }
        // Shape: style handle
        EntityType::Shape(s) => {
            s.style_handle = None;
        }
        // Leader: annotation handle
        EntityType::Leader(l) => {
            l.annotation_handle = Handle::NULL;
        }
        // Viewport: many handle fields
        EntityType::Viewport(v) => {
            v.ucs_handle = Handle::NULL;
            v.base_ucs_handle = Handle::NULL;
            v.background_handle = Handle::NULL;
            v.shade_plot_handle = Handle::NULL;
            v.visual_style_handle = Handle::NULL;
        }
        // Dimension: block_name is generated by writer, actual_measurement is computed
        EntityType::Dimension(d) => {
            let base = d.base_mut();
            base.block_name = String::new();
            base.actual_measurement = 0.0;
        }
        // Mesh: sort edges and normalize crease None → Some(0.0)
        EntityType::Mesh(m) => {
            for edge in &mut m.edges {
                if edge.crease.is_none() {
                    edge.crease = Some(0.0);
                }
            }
            m.edges
                .sort_by(|a, b| a.start.cmp(&b.start).then(a.end.cmp(&b.end)));
        }
        _ => {}
    }

    // ── Round direction vectors (ULP-level drift from f64→text→f64) ──
    match entity {
        EntityType::Ray(r) => {
            r.direction = round_vector3(r.direction, 14);
        }
        EntityType::XLine(x) => {
            x.direction = round_vector3(x.direction, 14);
        }
        EntityType::MLine(ml) => {
            for v in &mut ml.vertices {
                v.direction = round_vector3(v.direction, 14);
                v.miter = round_vector3(v.miter, 14);
            }
        }
        _ => {}
    }

    // ── Case-normalize style names (DWG may return different case) ──
    match entity {
        EntityType::Text(t) => {
            t.style = t.style.to_uppercase();
        }
        EntityType::MText(m) => {
            m.style = m.style.to_uppercase();
        }
        EntityType::Leader(l) => {
            l.dimension_style = l.dimension_style.to_uppercase();
        }
        EntityType::Tolerance(t) => {
            t.dimension_style_name = t.dimension_style_name.to_uppercase();
        }
        _ => {}
    }
}

fn round_f64(v: f64, decimals: u32) -> f64 {
    let mult = 10f64.powi(decimals as i32);
    (v * mult).round() / mult
}

fn round_vector3(v: Vector3, decimals: u32) -> Vector3 {
    Vector3::new(
        round_f64(v.x, decimals),
        round_f64(v.y, decimals),
        round_f64(v.z, decimals),
    )
}

fn compare_header_variables(
    report: &mut DiffReport,
    orig: &acadrust::document::HeaderVariables,
    rt: &acadrust::document::HeaderVariables,
) {
    // Compare critical header fields that should survive roundtrip
    macro_rules! cmp_header {
        ($field:ident) => {
            if orig.$field != rt.$field {
                report.add(format!(
                    "Header.{}: {:?} vs {:?}",
                    stringify!($field),
                    orig.$field,
                    rt.$field
                ));
            }
        };
    }

    // Drawing mode flags
    cmp_header!(associate_dimensions);
    cmp_header!(ortho_mode);
    cmp_header!(fill_mode);
    cmp_header!(quick_text_mode);
    cmp_header!(mirror_text);
    cmp_header!(regen_mode);
    cmp_header!(limit_check);
    cmp_header!(show_model_space);
    cmp_header!(world_view);
    cmp_header!(retain_xref_visibility);
    cmp_header!(display_silhouette);

    // Unit settings
    cmp_header!(linear_unit_format);
    cmp_header!(linear_unit_precision);
    cmp_header!(angular_unit_format);
    cmp_header!(angular_unit_precision);
    cmp_header!(insertion_units);

    // Scale/size defaults
    cmp_header!(linetype_scale);
    cmp_header!(text_height);

    // Dimension variables (selected critical ones)
    cmp_header!(dim_scale);
    cmp_header!(dim_arrow_size);
    cmp_header!(dim_text_height);
    cmp_header!(dim_tolerance);
    cmp_header!(dim_limits);
    cmp_header!(dim_decimal_places);

    // Extents and limits
    cmp_header!(model_space_insertion_base);
    cmp_header!(model_space_limits_min);
    cmp_header!(model_space_limits_max);

    // Measurement
    cmp_header!(measurement);
}

// ═══════════════════════════════════════════════════════════════════════════
//  DXF ROUNDTRIP TESTS
// ═══════════════════════════════════════════════════════════════════════════

/// DXF write → read roundtrip with entity count check.
fn dxf_roundtrip(doc: CadDocument) -> CadDocument {
    let writer = DxfWriter::new(&doc);
    let bytes = writer.write_to_vec().expect("DXF write failed");
    let reader = DxfReader::from_reader(Cursor::new(bytes)).expect("DXF reader init failed");
    reader.read().expect("DXF read failed")
}

/// DWG write → read roundtrip with entity count check.
fn dwg_roundtrip(doc: &CadDocument) -> CadDocument {
    let bytes = DwgWriter::write_to_vec(doc).expect("DWG write failed");
    let mut reader = DwgReader::from_stream(Cursor::new(bytes));
    reader.read().expect("DWG read failed")
}

// ── AcDs SAB round-trip (issue 225) ───────────────────────────────────────
//
// On R2013+ a 3DSOLID/REGION/BODY's geometry is not inline — it lives as a SAB
// blob in the `AcDb:AcDsPrototype_1b` data-store section, and the entity carries
// a `has_ds_data` flag telling readers to look there. Two regressions this
// guards against: (1) the writer never set `has_ds_data`, so AutoCAD/BricsCAD
// dropped every solid as "data stream is empty"; (2) blobs were paired with
// entities positionally in handle-sorted order rather than object-stream order,
// so each solid came back with the *wrong* geometry.

/// Fabricate a minimal SAB blob the AcDs reader recognises: the ASM binary
/// magic, a distinctive body, and the `End-of-ASM-data` terminator.
fn fake_sab_blob(tag: u8, body_len: usize) -> Vec<u8> {
    let mut b = b"ASM BinaryFile".to_vec();
    b.extend(std::iter::repeat(tag).take(body_len));
    b.extend_from_slice(b"\x0E\x03End\x0E\x02of\x0E\x03ASM\x0D\x04data");
    b
}

fn solid_with_sab(sab: Vec<u8>) -> EntityType {
    let mut s = acadrust::entities::solid3d::Solid3D::new();
    s.acis_data.sab_data = sab;
    s.acis_data.is_binary = true;
    s.acis_data.version = acadrust::entities::solid3d::AcisVersion::Version2;
    EntityType::Solid3D(s)
}

#[test]
fn dwg_r2018_planar_body_solid_survives_roundtrip() {
    // A 3DSOLID built from an exact planar B-rep (build_planar_body) must save
    // to R2018 DWG and reload with its ACIS geometry intact and linked via the
    // has_ds_data flag — the full exact-export path (issue 225 + Problem 1).
    use acadrust::entities::acis::primitives::build_planar_body;
    use acadrust::entities::solid3d::Solid3D;

    let vertices = [
        [0.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [2.0, 2.0, 0.0],
        [0.0, 2.0, 0.0],
        [0.0, 0.0, 2.0],
        [2.0, 0.0, 2.0],
        [2.0, 2.0, 2.0],
        [0.0, 2.0, 2.0],
    ];
    let faces = vec![
        vec![0, 3, 2, 1],
        vec![4, 5, 6, 7],
        vec![0, 1, 5, 4],
        vec![3, 7, 6, 2],
        vec![1, 2, 6, 5],
        vec![0, 4, 7, 3],
    ];
    let sat = build_planar_body(&vertices, &faces).expect("cube builds");

    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut solid = Solid3D::new();
    solid.set_sat_document(&sat);
    assert!(
        solid.acis_data.has_data(),
        "solid must carry ACIS data pre-save"
    );
    let handle = doc.add_entity(EntityType::Solid3D(solid)).unwrap();

    let rt = dwg_roundtrip(&doc);
    let e = rt
        .get_entity(handle)
        .expect("solid present after roundtrip");
    let EntityType::Solid3D(s) = e else {
        panic!("entity is not a 3DSOLID after roundtrip");
    };
    assert!(
        s.acis_data.has_data(),
        "planar-body solid lost its ACIS geometry on DWG roundtrip"
    );
    assert!(
        s.common.has_ds_data,
        "planar-body solid was not linked to its AcDs SAB blob (would be dropped)"
    );
}

#[test]
fn dwg_r2018_acds_sab_pairs_with_correct_solid() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    // Distinct sizes so a mis-pairing (shift) fails the byte-equality assert.
    let blobs = [
        fake_sab_blob(0xA1, 300),
        fake_sab_blob(0xB2, 900),
        fake_sab_blob(0xC3, 150),
    ];
    let handles: Vec<Handle> = blobs
        .iter()
        .map(|b| doc.add_entity(solid_with_sab(b.clone())).unwrap())
        .collect();

    let rt = dwg_roundtrip(&doc);

    for (h, expected) in handles.iter().zip(blobs.iter()) {
        let e = rt.get_entity(*h).expect("solid missing after roundtrip");
        let EntityType::Solid3D(s) = e else {
            panic!("handle {h:?} is not a 3DSOLID after roundtrip");
        };
        assert_eq!(
            &s.acis_data.sab_data, expected,
            "solid {h:?} came back with the wrong SAB blob (mis-paired)"
        );
        assert!(
            s.common.has_ds_data,
            "solid {h:?} lost its has_ds_data link — other CAD apps would \
             drop it as an empty data stream"
        );
    }
}

// ── DXF: Entity count preservation ────────────────────────────────────

#[test]
fn dxf_roundtrip_entity_count_r2018() {
    let (doc, expected) = build_rich_document(DxfVersion::AC1032);
    assert_eq!(doc.entity_count(), expected, "pre-roundtrip count wrong");
    let rt = dxf_roundtrip(doc);
    assert_eq!(
        rt.entity_count(),
        expected,
        "DXF R2018 roundtrip lost entities: expected {}, got {}",
        expected,
        rt.entity_count()
    );
}

#[test]
fn dxf_roundtrip_entity_count_r2000() {
    let (doc, expected) = build_rich_document(DxfVersion::AC1015);
    let rt = dxf_roundtrip(doc);
    assert_eq!(
        rt.entity_count(),
        expected,
        "DXF R2000 roundtrip lost entities: expected {}, got {}",
        expected,
        rt.entity_count()
    );
}

// ── DXF: Deep field comparison ────────────────────────────────────────
// These tests identify real roundtrip data loss in the library.
// Known issues per format are documented with expected difference counts.
// If you fix a roundtrip issue, reduce the expected count accordingly.

#[test]
fn dxf_roundtrip_deep_r2018() {
    let (doc, _) = build_rich_document(DxfVersion::AC1032);
    let rt = dxf_roundtrip(doc.clone());
    let report = compare_documents(&doc, &rt);
    // Known DXF roundtrip issues:
    //   - Entity type distribution mismatch from Polyline type collapse (1 diff)
    //   - Polyline2D/3D/PolyfaceMesh read back as legacy Polyline (3 missing + 1 appeared = 4 diffs)
    //   - MultiLeader leader_roots not preserved in DXF (1 diff)
    let max_known = 6;
    if !report.is_empty() {
        eprintln!(
            "DXF R2018 roundtrip: {} known issue(s):\n{}",
            report.differences.len(),
            report.summary()
        );
    }
    assert!(
        report.differences.len() <= max_known,
        "DXF R2018 roundtrip REGRESSION: {} diffs (expected ≤ {}):\n{}",
        report.differences.len(),
        max_known,
        report.summary()
    );
}

#[test]
fn dxf_roundtrip_deep_r2000() {
    let (doc, _) = build_rich_document(DxfVersion::AC1015);
    let rt = dxf_roundtrip(doc.clone());
    let report = compare_documents(&doc, &rt);
    let max_known = 6; // same known issues as R2018
    if !report.is_empty() {
        eprintln!(
            "DXF R2000 roundtrip: {} known issue(s):\n{}",
            report.differences.len(),
            report.summary()
        );
    }
    assert!(
        report.differences.len() <= max_known,
        "DXF R2000 roundtrip REGRESSION: {} diffs (expected ≤ {}):\n{}",
        report.differences.len(),
        max_known,
        report.summary()
    );
}

// ── DXF: Double roundtrip stability ───────────────────────────────────

#[test]
fn dxf_double_roundtrip_stability() {
    let (doc, expected) = build_rich_document(DxfVersion::AC1032);
    let rt1 = dxf_roundtrip(doc);
    let rt2 = dxf_roundtrip(rt1.clone());

    assert_eq!(
        rt2.entity_count(),
        expected,
        "DXF double roundtrip entity count: expected {}, got {}",
        expected,
        rt2.entity_count()
    );

    // Compare rt1 vs rt2 (both already went through one DXF roundtrip).
    // Known issues: MultiLeader data drift (1), Polyline data mixing (3)
    let report = compare_documents(&rt1, &rt2);
    let max_known = 4;
    if !report.is_empty() {
        eprintln!(
            "DXF double roundtrip: {} known issue(s):\n{}",
            report.differences.len(),
            report.summary()
        );
    }
    assert!(
        report.differences.len() <= max_known,
        "DXF double roundtrip REGRESSION: {} diffs (expected ≤ {}):\n{}",
        report.differences.len(),
        max_known,
        report.summary()
    );
}

// ── DXF: Table preservation ───────────────────────────────────────────

#[test]
fn dxf_roundtrip_tables_preserved() {
    let (doc, _) = build_rich_document(DxfVersion::AC1032);
    let orig_layers = doc.layers.len();
    let orig_linetypes = doc.line_types.len();
    let orig_textstyles = doc.text_styles.len();
    let orig_dimstyles = doc.dim_styles.len();
    let orig_appids = doc.app_ids.len();

    let rt = dxf_roundtrip(doc);

    assert_eq!(rt.layers.len(), orig_layers, "Layer count changed");
    assert_eq!(
        rt.line_types.len(),
        orig_linetypes,
        "LineType count changed"
    );
    assert_eq!(
        rt.text_styles.len(),
        orig_textstyles,
        "TextStyle count changed"
    );
    assert_eq!(
        rt.dim_styles.len(),
        orig_dimstyles,
        "DimStyle count changed"
    );
    assert_eq!(rt.app_ids.len(), orig_appids, "AppId count changed");
}

// ── DXF: Individual entity type roundtrip ─────────────────────────────

macro_rules! dxf_entity_roundtrip {
    ($test_name:ident, $entity_expr:expr) => {
        #[test]
        fn $test_name() {
            let entity = $entity_expr;
            let doc = build_minimal_document(DxfVersion::AC1032, entity);
            let rt = dxf_roundtrip(doc.clone());

            assert_eq!(
                rt.entity_count(),
                doc.entity_count(),
                "Entity count changed in roundtrip"
            );

            let report = compare_documents(&doc, &rt);
            assert!(
                report.is_empty(),
                "DXF roundtrip for {} failed:\n{}",
                stringify!($test_name),
                report.summary()
            );
        }
    };
}

dxf_entity_roundtrip!(
    dxf_rt_line,
    EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 100.0, 50.0, 0.0))
);
dxf_entity_roundtrip!(
    dxf_rt_circle,
    EntityType::Circle(Circle::from_coords(50.0, 50.0, 0.0, 25.0))
);
dxf_entity_roundtrip!(
    dxf_rt_arc,
    EntityType::Arc(Arc::from_coords(
        50.0,
        50.0,
        0.0,
        25.0,
        0.0,
        std::f64::consts::PI
    ))
);
dxf_entity_roundtrip!(
    dxf_rt_ellipse,
    EntityType::Ellipse(Ellipse::from_center_axes(
        Vector3::new(50.0, 50.0, 0.0),
        Vector3::new(40.0, 0.0, 0.0),
        0.5
    ))
);
dxf_entity_roundtrip!(
    dxf_rt_point,
    EntityType::Point(Point::from_coords(10.0, 20.0, 30.0))
);
dxf_entity_roundtrip!(
    dxf_rt_ray,
    EntityType::Ray(Ray::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(1.0, 1.0, 0.0)
    ))
);
dxf_entity_roundtrip!(
    dxf_rt_xline,
    EntityType::XLine(XLine::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(1.0, 0.0, 0.0)
    ))
);
dxf_entity_roundtrip!(
    dxf_rt_solid,
    EntityType::Solid(Solid::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 0.0, 0.0),
        Vector3::new(10.0, 10.0, 0.0),
        Vector3::new(0.0, 10.0, 0.0)
    ))
);
dxf_entity_roundtrip!(
    dxf_rt_face3d,
    EntityType::Face3D(Face3D::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 0.0, 0.0),
        Vector3::new(10.0, 10.0, 5.0),
        Vector3::new(0.0, 10.0, 5.0)
    ))
);
dxf_entity_roundtrip!(
    dxf_rt_text,
    EntityType::Text(Text::with_value("Test text", Vector3::new(0.0, 0.0, 0.0)))
);
dxf_entity_roundtrip!(
    dxf_rt_mtext,
    EntityType::MText(MText::with_value(
        "Multi\\Pline test",
        Vector3::new(0.0, 0.0, 0.0)
    ))
);
dxf_entity_roundtrip!(
    dxf_rt_lwpolyline,
    EntityType::LwPolyline(LwPolyline::from_points(vec![
        Vector2::new(0.0, 0.0),
        Vector2::new(10.0, 0.0),
        Vector2::new(10.0, 10.0),
    ]))
);
dxf_entity_roundtrip!(
    dxf_rt_spline,
    EntityType::Spline(Spline::from_control_points(
        3,
        vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(5.0, 10.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(15.0, 10.0, 0.0),
        ]
    ))
);
dxf_entity_roundtrip!(
    dxf_rt_leader,
    EntityType::Leader(Leader::two_point(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 10.0, 0.0)
    ))
);
dxf_entity_roundtrip!(
    dxf_rt_dimension,
    EntityType::Dimension(Dimension::Linear(DimensionLinear::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(100.0, 0.0, 0.0)
    )))
);
dxf_entity_roundtrip!(
    dxf_rt_tolerance,
    EntityType::Tolerance(Tolerance::with_text(
        Vector3::new(10.0, 10.0, 0.0),
        "{\\Fgdt;p}%%v0.5"
    ))
);
dxf_entity_roundtrip!(dxf_rt_viewport, EntityType::Viewport(Viewport::new()));

// ═══════════════════════════════════════════════════════════════════════════
//  DWG ROUNDTRIP TESTS
// ═══════════════════════════════════════════════════════════════════════════

/// All DWG versions to test
const DWG_VERSIONS: &[(DxfVersion, &str)] = &[
    (DxfVersion::AC1015, "R2000"),
    (DxfVersion::AC1018, "R2004"),
    (DxfVersion::AC1021, "R2007"),
    (DxfVersion::AC1024, "R2010"),
    (DxfVersion::AC1027, "R2013"),
    (DxfVersion::AC1032, "R2018"),
];

// ── DWG: Entity count preservation across versions ────────────────────

#[test]
fn dwg_roundtrip_entity_count_all_versions() {
    for &(version, label) in DWG_VERSIONS {
        let (doc, expected) = build_rich_document(version);
        assert_eq!(
            doc.entity_count(),
            expected,
            "{}: pre-roundtrip count wrong",
            label
        );
        let rt = dwg_roundtrip(&doc);
        assert_eq!(
            rt.entity_count(),
            expected,
            "DWG {} roundtrip lost entities: expected {}, got {}",
            label,
            expected,
            rt.entity_count()
        );
    }
}

// ── DWG: Deep field comparison ────────────────────────────────────────

#[test]
fn dwg_roundtrip_deep_r2018() {
    let (doc, _) = build_rich_document(DxfVersion::AC1032);
    let rt = dwg_roundtrip(&doc);
    let report = compare_documents(&doc, &rt);
    // Known issues: Shape name not resolvable in DWG (1)
    let max_known = 1;
    if !report.is_empty() {
        eprintln!(
            "DWG R2018 roundtrip: {} known issue(s):\n{}",
            report.differences.len(),
            report.summary()
        );
    }
    assert!(
        report.differences.len() <= max_known,
        "DWG R2018 roundtrip REGRESSION: {} diffs (expected ≤ {}):\n{}",
        report.differences.len(),
        max_known,
        report.summary()
    );
}

#[test]
fn dwg_roundtrip_deep_r2000() {
    let (doc, _) = build_rich_document(DxfVersion::AC1015);
    let rt = dwg_roundtrip(&doc);
    let report = compare_documents(&doc, &rt);
    // Known issues: Shape name (1)
    let max_known = 1;
    if !report.is_empty() {
        eprintln!(
            "DWG R2000 roundtrip: {} known issue(s):\n{}",
            report.differences.len(),
            report.summary()
        );
    }
    assert!(
        report.differences.len() <= max_known,
        "DWG R2000 roundtrip REGRESSION: {} diffs (expected ≤ {}):\n{}",
        report.differences.len(),
        max_known,
        report.summary()
    );
}

#[test]
fn dwg_roundtrip_deep_r2013() {
    let (doc, _) = build_rich_document(DxfVersion::AC1027);
    let rt = dwg_roundtrip(&doc);
    let report = compare_documents(&doc, &rt);
    // Known issues: Shape name not resolvable in DWG (1)
    let max_known = 1;
    if !report.is_empty() {
        eprintln!(
            "DWG R2013 roundtrip: {} known issue(s):\n{}",
            report.differences.len(),
            report.summary()
        );
    }
    assert!(
        report.differences.len() <= max_known,
        "DWG R2013 roundtrip REGRESSION: {} diffs (expected ≤ {}):\n{}",
        report.differences.len(),
        max_known,
        report.summary()
    );
}

// ── DWG: Double roundtrip stability ───────────────────────────────────

#[test]
fn dwg_double_roundtrip_stability() {
    let (doc, expected) = build_rich_document(DxfVersion::AC1032);
    let rt1 = dwg_roundtrip(&doc);
    let rt2 = dwg_roundtrip(&rt1);

    assert_eq!(
        rt2.entity_count(),
        expected,
        "DWG double roundtrip entity count: expected {}, got {}",
        expected,
        rt2.entity_count()
    );

    let report = compare_documents(&rt1, &rt2);
    assert!(
        report.is_empty(),
        "DWG double roundtrip instability:\n{}",
        report.summary()
    );
}

// ── DWG: Table preservation ───────────────────────────────────────────

#[test]
fn dwg_roundtrip_tables_preserved() {
    let (doc, _) = build_rich_document(DxfVersion::AC1032);
    let orig_layers = doc.layers.len();
    let orig_linetypes = doc.line_types.len();
    let orig_textstyles = doc.text_styles.len();
    let orig_dimstyles = doc.dim_styles.len();
    let orig_appids = doc.app_ids.len();

    let rt = dwg_roundtrip(&doc);

    assert_eq!(rt.layers.len(), orig_layers, "Layer count changed");
    assert_eq!(
        rt.line_types.len(),
        orig_linetypes,
        "LineType count changed"
    );
    assert_eq!(
        rt.text_styles.len(),
        orig_textstyles,
        "TextStyle count changed"
    );
    assert_eq!(
        rt.dim_styles.len(),
        orig_dimstyles,
        "DimStyle count changed"
    );
    assert_eq!(rt.app_ids.len(), orig_appids, "AppId count changed");
}

// ── DWG: Individual entity type roundtrip ─────────────────────────────

macro_rules! dwg_entity_roundtrip {
    ($test_name:ident, $entity_expr:expr) => {
        #[test]
        fn $test_name() {
            let entity = $entity_expr;
            let doc = build_minimal_document(DxfVersion::AC1032, entity);
            let rt = dwg_roundtrip(&doc);

            assert_eq!(
                rt.entity_count(),
                doc.entity_count(),
                "Entity count changed in DWG roundtrip"
            );

            let report = compare_documents(&doc, &rt);
            assert!(
                report.is_empty(),
                "DWG roundtrip for {} failed:\n{}",
                stringify!($test_name),
                report.summary()
            );
        }
    };
}

dwg_entity_roundtrip!(
    dwg_rt_line,
    EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 100.0, 50.0, 0.0))
);
dwg_entity_roundtrip!(
    dwg_rt_circle,
    EntityType::Circle(Circle::from_coords(50.0, 50.0, 0.0, 25.0))
);
dwg_entity_roundtrip!(
    dwg_rt_arc,
    EntityType::Arc(Arc::from_coords(
        50.0,
        50.0,
        0.0,
        25.0,
        0.0,
        std::f64::consts::PI
    ))
);
dwg_entity_roundtrip!(
    dwg_rt_ellipse,
    EntityType::Ellipse(Ellipse::from_center_axes(
        Vector3::new(50.0, 50.0, 0.0),
        Vector3::new(40.0, 0.0, 0.0),
        0.5
    ))
);
dwg_entity_roundtrip!(
    dwg_rt_point,
    EntityType::Point(Point::from_coords(10.0, 20.0, 30.0))
);
dwg_entity_roundtrip!(
    dwg_rt_ray,
    EntityType::Ray(Ray::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(1.0, 1.0, 0.0)
    ))
);
dwg_entity_roundtrip!(
    dwg_rt_xline,
    EntityType::XLine(XLine::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(1.0, 0.0, 0.0)
    ))
);
dwg_entity_roundtrip!(
    dwg_rt_solid,
    EntityType::Solid(Solid::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 0.0, 0.0),
        Vector3::new(10.0, 10.0, 0.0),
        Vector3::new(0.0, 10.0, 0.0)
    ))
);
dwg_entity_roundtrip!(
    dwg_rt_face3d,
    EntityType::Face3D(Face3D::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 0.0, 0.0),
        Vector3::new(10.0, 10.0, 5.0),
        Vector3::new(0.0, 10.0, 5.0)
    ))
);
dwg_entity_roundtrip!(
    dwg_rt_text,
    EntityType::Text(Text::with_value("Test text", Vector3::new(0.0, 0.0, 0.0)))
);
dwg_entity_roundtrip!(
    dwg_rt_mtext,
    EntityType::MText(MText::with_value(
        "Multi\\Pline test",
        Vector3::new(0.0, 0.0, 0.0)
    ))
);
dwg_entity_roundtrip!(
    dwg_rt_lwpolyline,
    EntityType::LwPolyline(LwPolyline::from_points(vec![
        Vector2::new(0.0, 0.0),
        Vector2::new(10.0, 0.0),
        Vector2::new(10.0, 10.0),
    ]))
);
dwg_entity_roundtrip!(
    dwg_rt_spline,
    EntityType::Spline(Spline::from_control_points(
        3,
        vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(5.0, 10.0, 0.0),
            Vector3::new(10.0, 0.0, 0.0),
            Vector3::new(15.0, 10.0, 0.0),
        ]
    ))
);
dwg_entity_roundtrip!(
    dwg_rt_leader,
    EntityType::Leader(Leader::two_point(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(10.0, 10.0, 0.0)
    ))
);
dwg_entity_roundtrip!(
    dwg_rt_dimension,
    EntityType::Dimension(Dimension::Linear(DimensionLinear::new(
        Vector3::new(0.0, 0.0, 0.0),
        Vector3::new(100.0, 0.0, 0.0)
    )))
);
dwg_entity_roundtrip!(
    dwg_rt_tolerance,
    EntityType::Tolerance(Tolerance::with_text(
        Vector3::new(10.0, 10.0, 0.0),
        "{\\Fgdt;p}%%v0.5"
    ))
);
dwg_entity_roundtrip!(dwg_rt_viewport, EntityType::Viewport(Viewport::new()));

// ═══════════════════════════════════════════════════════════════════════════
//  CROSS-FORMAT ROUNDTRIP TESTS
// ═══════════════════════════════════════════════════════════════════════════

/// DXF → DWG → DXF roundtrip: write DXF, read, write DWG, read, compare.
#[test]
fn cross_format_dxf_to_dwg_to_dxf() {
    let (orig_doc, expected) = build_rich_document(DxfVersion::AC1032);

    // Write DXF → Read back
    let dxf_bytes = DxfWriter::new(&orig_doc).write_to_vec().unwrap();
    let doc_from_dxf = DxfReader::from_reader(Cursor::new(dxf_bytes))
        .unwrap()
        .read()
        .unwrap();
    assert_eq!(
        doc_from_dxf.entity_count(),
        expected,
        "DXF read lost entities"
    );

    // Write DWG → Read back
    let dwg_bytes = DwgWriter::write_to_vec(&doc_from_dxf).unwrap();
    let doc_from_dwg = DwgReader::from_stream(Cursor::new(dwg_bytes))
        .read()
        .unwrap();

    // Known: DXF reader converts Polyline2D/3D/PolyfaceMesh to legacy Polyline,
    // which DWG writer doesn't support → 3 entities lost.
    let max_entity_loss = 3;
    let actual_loss = expected as i64 - doc_from_dwg.entity_count() as i64;
    if actual_loss > 0 {
        let orig_types = entity_type_counts(&doc_from_dxf);
        let rt_types = entity_type_counts(&doc_from_dwg);
        eprintln!(
            "DXF→DWG: {} entities lost (known: ≤{})\n  before: {:?}\n  after:  {:?}",
            actual_loss, max_entity_loss, orig_types, rt_types
        );
    }
    assert!(
        actual_loss <= max_entity_loss as i64,
        "DXF→DWG REGRESSION: lost {} entities (expected ≤ {})\n  before: {:?}\n  after:  {:?}",
        actual_loss,
        max_entity_loss,
        entity_type_counts(&doc_from_dxf),
        entity_type_counts(&doc_from_dwg)
    );

    // Write DXF again → Read back
    let remaining = doc_from_dwg.entity_count();
    let dxf_bytes2 = DxfWriter::new(&doc_from_dwg).write_to_vec().unwrap();
    let final_doc = DxfReader::from_reader(Cursor::new(dxf_bytes2))
        .unwrap()
        .read()
        .unwrap();
    assert_eq!(
        final_doc.entity_count(),
        remaining,
        "DXF→DWG→DXF: further entity loss in final DXF write: {} → {}",
        remaining,
        final_doc.entity_count()
    );
}

/// DWG → DXF → DWG roundtrip.
#[test]
fn cross_format_dwg_to_dxf_to_dwg() {
    let (orig_doc, expected) = build_rich_document(DxfVersion::AC1032);

    // Write DWG → Read back
    let dwg_bytes = DwgWriter::write_to_vec(&orig_doc).unwrap();
    let doc_from_dwg = DwgReader::from_stream(Cursor::new(dwg_bytes))
        .read()
        .unwrap();
    assert_eq!(
        doc_from_dwg.entity_count(),
        expected,
        "DWG read lost entities"
    );

    // Write DXF → Read back
    let dxf_bytes = DxfWriter::new(&doc_from_dwg).write_to_vec().unwrap();
    let doc_from_dxf = DxfReader::from_reader(Cursor::new(dxf_bytes))
        .unwrap()
        .read()
        .unwrap();
    assert_eq!(
        doc_from_dxf.entity_count(),
        expected,
        "DWG→DXF lost entities: {} → {}",
        expected,
        doc_from_dxf.entity_count()
    );

    // Write DWG again → Read back
    // Known: DXF reader converts Polyline2D/3D/PolyfaceMesh to legacy Polyline,
    // which DWG writer doesn't support → 3 entities lost.
    let dwg_bytes2 = DwgWriter::write_to_vec(&doc_from_dxf).unwrap();
    let final_doc = DwgReader::from_stream(Cursor::new(dwg_bytes2))
        .read()
        .unwrap();
    let max_entity_loss = 3;
    let actual_loss = expected as i64 - final_doc.entity_count() as i64;
    if actual_loss > 0 {
        eprintln!(
            "DWG→DXF→DWG: {} entities lost (known: ≤{})\n  final types: {:?}",
            actual_loss,
            max_entity_loss,
            entity_type_counts(&final_doc)
        );
    }
    assert!(
        actual_loss <= max_entity_loss as i64,
        "DWG→DXF→DWG REGRESSION: lost {} entities (expected ≤ {})",
        actual_loss,
        max_entity_loss
    );
}

// ═══════════════════════════════════════════════════════════════════════════
//  ENTITY WITH CUSTOM PROPERTIES ROUNDTRIP
// ═══════════════════════════════════════════════════════════════════════════

/// Test that entity properties (layer, color, lineweight) survive roundtrip.
#[test]
fn dxf_roundtrip_entity_properties() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);

    let mut line = Line::from_coords(0.0, 0.0, 0.0, 100.0, 50.0, 0.0);
    line.common.layer = "TestLayer".to_string();
    line.common.color = Color::from_index(5); // Blue
    line.common.linetype_scale = 2.5;
    doc.add_entity(EntityType::Line(line)).unwrap();

    let rt = dxf_roundtrip(doc);
    let entity = rt.entities().next().expect("no entities after roundtrip");
    let common = entity.common();

    assert_eq!(common.layer, "TestLayer", "Layer not preserved");
    assert_eq!(common.color, Color::from_index(5), "Color not preserved");
    assert!(
        (common.linetype_scale - 2.5).abs() < 1e-10,
        "Linetype scale not preserved: {}",
        common.linetype_scale
    );
}

#[test]
fn dwg_roundtrip_entity_properties() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);

    // DWG requires the layer to exist in the layer table WITH a valid handle
    let mut test_layer = acadrust::Layer::new("TestLayer");
    test_layer.handle = doc.allocate_handle();
    doc.layers.add(test_layer).unwrap();

    let mut line = Line::from_coords(0.0, 0.0, 0.0, 100.0, 50.0, 0.0);
    line.common.layer = "TestLayer".to_string();
    line.common.color = Color::from_index(5); // Blue
    line.common.linetype_scale = 2.5;
    doc.add_entity(EntityType::Line(line)).unwrap();

    let rt = dwg_roundtrip(&doc);
    let entity = rt.entities().next().expect("no entities after roundtrip");
    let common = entity.common();

    assert_eq!(common.layer, "TestLayer", "Layer not preserved");
    assert_eq!(common.color, Color::from_index(5), "Color not preserved");
    assert!(
        (common.linetype_scale - 2.5).abs() < 1e-10,
        "Linetype scale not preserved: {}",
        common.linetype_scale
    );
}

// ═══════════════════════════════════════════════════════════════════════════
//  HEADER VARIABLE ROUNDTRIP
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn dxf_roundtrip_header_variables() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.header.fill_mode = false;
    doc.header.ortho_mode = true;
    doc.header.linetype_scale = 3.14;
    doc.header.text_height = 5.0;
    doc.header.linear_unit_format = 2;
    doc.header.insertion_units = 4;
    doc.header.dim_scale = 2.0;
    doc.header.dim_arrow_size = 0.25;

    let rt = dxf_roundtrip(doc);

    assert_eq!(rt.header.fill_mode, false, "fill_mode not preserved");
    assert_eq!(rt.header.ortho_mode, true, "ortho_mode not preserved");
    assert!(
        (rt.header.linetype_scale - 3.14).abs() < 1e-10,
        "linetype_scale not preserved"
    );
    assert!(
        (rt.header.text_height - 5.0).abs() < 1e-10,
        "text_height not preserved"
    );
    assert_eq!(
        rt.header.linear_unit_format, 2,
        "linear_unit_format not preserved"
    );
    assert_eq!(
        rt.header.insertion_units, 4,
        "insertion_units not preserved"
    );
    assert!(
        (rt.header.dim_scale - 2.0).abs() < 1e-10,
        "dim_scale not preserved"
    );
    assert!(
        (rt.header.dim_arrow_size - 0.25).abs() < 1e-10,
        "dim_arrow_size not preserved"
    );
}

#[test]
fn dwg_roundtrip_header_variables() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.header.fill_mode = false;
    doc.header.ortho_mode = true;
    doc.header.linetype_scale = 3.14;
    doc.header.text_height = 5.0;
    doc.header.linear_unit_format = 2;
    doc.header.insertion_units = 4;
    doc.header.dim_scale = 2.0;
    doc.header.dim_arrow_size = 0.25;

    let rt = dwg_roundtrip(&doc);

    assert_eq!(rt.header.fill_mode, false, "fill_mode not preserved");
    assert_eq!(rt.header.ortho_mode, true, "ortho_mode not preserved");
    assert!(
        (rt.header.linetype_scale - 3.14).abs() < 1e-10,
        "linetype_scale not preserved"
    );
    assert!(
        (rt.header.text_height - 5.0).abs() < 1e-10,
        "text_height not preserved"
    );
    assert_eq!(
        rt.header.linear_unit_format, 2,
        "linear_unit_format not preserved"
    );
    assert_eq!(
        rt.header.insertion_units, 4,
        "insertion_units not preserved"
    );
    assert!(
        (rt.header.dim_scale - 2.0).abs() < 1e-10,
        "dim_scale not preserved"
    );
    assert!(
        (rt.header.dim_arrow_size - 0.25).abs() < 1e-10,
        "dim_arrow_size not preserved"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
//  BINARY DXF ROUNDTRIP
// ═══════════════════════════════════════════════════════════════════════════

fn binary_dxf_roundtrip(doc: CadDocument) -> CadDocument {
    let writer = DxfWriter::new_binary(&doc);
    let bytes = writer.write_to_vec().expect("Binary DXF write failed");
    let reader = DxfReader::from_reader(Cursor::new(bytes)).expect("Binary DXF reader init failed");
    reader.read().expect("Binary DXF read failed")
}

#[test]
fn binary_dxf_roundtrip_entity_count() {
    let (doc, expected) = build_rich_document(DxfVersion::AC1032);
    let rt = binary_dxf_roundtrip(doc);
    assert_eq!(
        rt.entity_count(),
        expected,
        "Binary DXF roundtrip lost entities"
    );
}

#[test]
fn binary_dxf_roundtrip_deep() {
    let (doc, _) = build_rich_document(DxfVersion::AC1032);
    let rt = binary_dxf_roundtrip(doc.clone());
    let report = compare_documents(&doc, &rt);
    // Same known issues as ASCII DXF (Polyline type, Hatch, MultiLeader, etc.)
    let max_known = 7;
    if !report.is_empty() {
        eprintln!(
            "Binary DXF roundtrip: {} known issue(s):\n{}",
            report.differences.len(),
            report.summary()
        );
    }
    assert!(
        report.differences.len() <= max_known,
        "Binary DXF roundtrip REGRESSION: {} diffs (expected ≤ {}):\n{}",
        report.differences.len(),
        max_known,
        report.summary()
    );
}

// ═══════════════════════════════════════════════════════════════════════════
//  REAL-FILE ROUNDTRIP (using .dwg files in workspace root)
// ═══════════════════════════════════════════════════════════════════════════

/// Test roundtrip of actual DWG files included in the repository.
macro_rules! real_dwg_roundtrip {
    ($test_name:ident, $file:expr) => {
        #[test]
        fn $test_name() {
            let path = concat!(env!("CARGO_MANIFEST_DIR"), "/", $file);
            if !std::path::Path::new(path).exists() {
                eprintln!("Skipping {}: file not found", $file);
                return;
            }

            let mut reader = DwgReader::from_file(path).expect("Failed to open DWG file");
            let doc = reader.read().expect("Failed to read DWG file");
            let orig_entity_count = doc.entity_count();
            let orig_layer_count = doc.layers.len();
            let orig_linetype_count = doc.line_types.len();
            let orig_object_count = doc.objects.len();

            // Write it back out and read again
            let bytes = DwgWriter::write_to_vec(&doc).expect("DWG write failed");
            let mut reader2 = DwgReader::from_stream(Cursor::new(bytes));
            let rt = reader2.read().expect("DWG re-read failed");

            assert_eq!(
                rt.entity_count(),
                orig_entity_count,
                "{}: entity count changed {} → {}",
                $file,
                orig_entity_count,
                rt.entity_count()
            );
            assert_eq!(
                rt.layers.len(),
                orig_layer_count,
                "{}: layer count changed",
                $file
            );
            assert_eq!(
                rt.line_types.len(),
                orig_linetype_count,
                "{}: linetype count changed",
                $file
            );
            assert_eq!(
                rt.objects.len(),
                orig_object_count,
                "{}: object count changed {} → {}",
                $file,
                orig_object_count,
                rt.objects.len()
            );
        }
    };
}

real_dwg_roundtrip!(real_dwg_cylinder_r2013, "cylinder_r2013.dwg");
real_dwg_roundtrip!(real_dwg_cylinder_r2000, "cylinder_r2000.dwg");
real_dwg_roundtrip!(real_dwg_box_r2013, "box_r2013.dwg");
real_dwg_roundtrip!(real_dwg_pyramid_r2013, "pyramid_r2013.dwg");
real_dwg_roundtrip!(real_dwg_wedge_r2013, "wedge_r2013.dwg");
real_dwg_roundtrip!(real_dwg_solid3d_r2013, "solid3d_r2013.dwg");
real_dwg_roundtrip!(real_dwg_solid3d_r2004, "solid3d_r2004.dwg");
real_dwg_roundtrip!(real_dwg_solid3d_r2000, "solid3d_r2000.dwg");
real_dwg_roundtrip!(real_dwg_morki_general, "acadrust_morki/General.dwg");

// ═══════════════════════════════════════════════════════════════════════════
//  EMPTY DOCUMENT ROUNDTRIP
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn dxf_roundtrip_empty_document() {
    let doc = CadDocument::with_version(DxfVersion::AC1032);
    let rt = dxf_roundtrip(doc.clone());
    assert_eq!(rt.entity_count(), 0, "Empty doc should have 0 entities");
    assert_eq!(
        rt.layers.len(),
        doc.layers.len(),
        "Layer count changed for empty doc"
    );
}

#[test]
fn dwg_roundtrip_empty_document() {
    let doc = CadDocument::with_version(DxfVersion::AC1032);
    let rt = dwg_roundtrip(&doc);
    assert_eq!(rt.entity_count(), 0, "Empty doc should have 0 entities");
    assert_eq!(
        rt.layers.len(),
        doc.layers.len(),
        "Layer count changed for empty doc"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
//  MULTI-VERSION DWG INDIVIDUAL ENTITY ROUNDTRIP MATRIX
// ═══════════════════════════════════════════════════════════════════════════

/// Tests individual entity types across multiple DWG versions.
#[test]
fn dwg_version_matrix_line() {
    for &(version, label) in DWG_VERSIONS {
        let doc = build_minimal_document(
            version,
            EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 100.0, 50.0, 0.0)),
        );
        let rt = dwg_roundtrip(&doc);
        assert_eq!(
            rt.entity_count(),
            1,
            "DWG {} Line: entity count changed",
            label
        );
    }
}

#[test]
fn dwg_version_matrix_circle() {
    for &(version, label) in DWG_VERSIONS {
        let doc = build_minimal_document(
            version,
            EntityType::Circle(Circle::from_coords(50.0, 50.0, 0.0, 25.0)),
        );
        let rt = dwg_roundtrip(&doc);
        assert_eq!(
            rt.entity_count(),
            1,
            "DWG {} Circle: entity count changed",
            label
        );
    }
}

#[test]
fn dwg_version_matrix_text() {
    for &(version, label) in DWG_VERSIONS {
        let doc = build_minimal_document(
            version,
            EntityType::Text(Text::with_value("Hello", Vector3::new(0.0, 0.0, 0.0))),
        );
        let rt = dwg_roundtrip(&doc);
        assert_eq!(
            rt.entity_count(),
            1,
            "DWG {} Text: entity count changed",
            label
        );
    }
}

#[test]
fn dwg_version_matrix_mtext() {
    for &(version, label) in DWG_VERSIONS {
        let doc = build_minimal_document(
            version,
            EntityType::MText(MText::with_value("Multi", Vector3::new(0.0, 0.0, 0.0))),
        );
        let rt = dwg_roundtrip(&doc);
        assert_eq!(
            rt.entity_count(),
            1,
            "DWG {} MText: entity count changed",
            label
        );
    }
}

// ── DWG: MTEXT background fill + columns roundtrip ─────────────────────
//
// These lock in the conditional background-fill block (BD scale / CMC color /
// BL transparency, gated on flag 0x01 or the R2018+ text-frame bit 0x10) and
// the R2018+ non-annotative column block. A regression in either desyncs the
// object stream, so the entity must survive byte-for-byte.

/// Round-trip `mtext` through DWG at `version` and return the recovered MTEXT.
/// Extracts the entity directly rather than asserting on `entity_count()`,
/// which also counts the model/paper-space block markers.
fn dwg_roundtrip_mtext(version: DxfVersion, mtext: MText) -> MText {
    let doc = build_minimal_document(version, EntityType::MText(mtext));
    let rt = dwg_roundtrip(&doc);
    let found = rt.entities().find_map(|e| match e {
        EntityType::MText(m) => Some(m.clone()),
        _ => None,
    });
    found.expect("MTEXT missing after DWG roundtrip")
}

#[test]
fn dwg_mtext_background_fill_r2018() {
    let mut mtext = MText::with_value("Background fill", Vector3::new(1.0, 2.0, 0.0));
    mtext.height = 2.5;
    mtext.rectangle_width = 20.0;
    // UseBackgroundFillColor
    mtext.background_fill_flags = 0x01;
    mtext.background_scale = 1.75;
    mtext.background_color = Color::from_index(1); // red, indexed (round-trips via CMC)
    mtext.background_transparency = 0;

    let rt = dwg_roundtrip_mtext(DxfVersion::AC1032, mtext.clone());
    assert_eq!(rt.background_fill_flags, 0x01, "R2018 background flags");
    assert_eq!(rt.background_scale, 1.75, "R2018 background scale");
    assert_eq!(
        rt.background_color,
        Color::from_index(1),
        "R2018 background color"
    );
    assert_eq!(
        rt.background_transparency, 0,
        "R2018 background transparency"
    );
    assert_eq!(rt.value, "Background fill", "R2018 value desynced");
    assert_eq!(rt.height, 2.5, "R2018 height desynced");
}

#[test]
fn dwg_mtext_background_fill_r2004() {
    let mut mtext = MText::with_value("BG R2004", Vector3::new(0.0, 0.0, 0.0));
    mtext.background_fill_flags = 0x01;
    mtext.background_scale = 1.5;
    mtext.background_color = Color::from_index(3); // green

    let rt = dwg_roundtrip_mtext(DxfVersion::AC1018, mtext.clone());
    assert_eq!(rt.background_fill_flags, 0x01, "R2004 background flags");
    assert_eq!(rt.background_scale, 1.5, "R2004 background scale");
    assert_eq!(
        rt.background_color,
        Color::from_index(3),
        "R2004 background color"
    );
    assert_eq!(rt.value, "BG R2004", "R2004 value desynced");
}

#[test]
fn dwg_mtext_background_fill_byblock_r2018() {
    // ByBlock is encoded as a distinct CMC method; make sure it survives the
    // background-color round-trip rather than degrading to an indexed color.
    let mut mtext = MText::with_value("BG byblock", Vector3::new(0.0, 0.0, 0.0));
    mtext.background_fill_flags = 0x01;
    mtext.background_scale = 1.5;
    mtext.background_color = Color::ByBlock;

    let rt = dwg_roundtrip_mtext(DxfVersion::AC1032, mtext.clone());
    assert_eq!(rt.background_fill_flags, 0x01, "byblock flags");
    assert_eq!(
        rt.background_color,
        Color::ByBlock,
        "byblock color desynced"
    );
    assert_eq!(rt.value, "BG byblock", "byblock value desynced");
}

#[test]
fn dwg_mtext_text_frame_r2018() {
    // The text-frame bit (0x10) alone triggers the fill block only for R2018+.
    let mut mtext = MText::with_value("Framed", Vector3::new(0.0, 0.0, 0.0));
    mtext.background_fill_flags = 0x10; // TextFrame, no fill color bit
    mtext.background_scale = 2.0;
    mtext.background_color = Color::from_index(5); // blue

    let rt = dwg_roundtrip_mtext(DxfVersion::AC1032, mtext.clone());
    assert_eq!(rt.background_fill_flags, 0x10, "R2018 text-frame flag");
    assert_eq!(rt.background_scale, 2.0, "R2018 text-frame scale");
    assert_eq!(
        rt.background_color,
        Color::from_index(5),
        "R2018 text-frame color"
    );
    assert_eq!(rt.value, "Framed", "R2018 text-frame value desynced");
}

#[test]
fn dwg_mtext_text_frame_r2004_not_stored() {
    // At R2004 the text-frame bit (0x10) does NOT trigger the fill block, so the
    // flag survives but the scale/color are not persisted (stay at defaults).
    // This locks in the version-gated read/write condition.
    let mut mtext = MText::with_value("Framed04", Vector3::new(0.0, 0.0, 0.0));
    mtext.background_fill_flags = 0x10; // TextFrame only
    mtext.background_scale = 9.0; // would be lost: no block written at R2004
    mtext.background_color = Color::from_index(5);

    let rt = dwg_roundtrip_mtext(DxfVersion::AC1018, mtext.clone());
    assert_eq!(
        rt.background_fill_flags, 0x10,
        "R2004 text-frame flag survives"
    );
    // No fill block at R2004 → scale/color come back as the reader defaults.
    assert_eq!(
        rt.background_scale, 1.5,
        "R2004 text-frame scale not stored"
    );
    assert_eq!(
        rt.background_color,
        Color::ByLayer,
        "R2004 text-frame color not stored"
    );
    assert_eq!(rt.value, "Framed04", "R2004 text-frame value desynced");
}

#[test]
fn dwg_mtext_dynamic_columns_r2018() {
    let mut mtext = MText::with_value("Columns", Vector3::new(0.0, 0.0, 0.0));
    mtext.is_annotative = false;
    mtext.column_data = MTextColumnData {
        column_type: 2, // dynamic
        column_count: 2,
        flow_reversed: true,
        auto_height: false,
        width: 50.0,
        gutter: 5.0,
        heights: vec![30.0, 40.0],
    };

    let rt = dwg_roundtrip_mtext(DxfVersion::AC1032, mtext.clone());
    assert!(!rt.is_annotative, "R2018 annotative flag desynced");
    assert_eq!(
        rt.column_data, mtext.column_data,
        "R2018 column data desynced"
    );
    assert_eq!(rt.value, "Columns", "R2018 columns value desynced");
}

#[test]
fn dwg_mtext_static_columns_r2018() {
    let mut mtext = MText::with_value("Static cols", Vector3::new(0.0, 0.0, 0.0));
    mtext.is_annotative = false;
    mtext.column_data = MTextColumnData {
        column_type: 1, // static
        column_count: 3,
        flow_reversed: false,
        auto_height: true,
        width: 25.0,
        gutter: 2.5,
        heights: vec![], // static columns store no per-column heights
    };

    let rt = dwg_roundtrip_mtext(DxfVersion::AC1032, mtext.clone());
    assert!(!rt.is_annotative, "R2018 static annotative flag desynced");
    assert_eq!(
        rt.column_data, mtext.column_data,
        "R2018 static column data desynced"
    );
}

#[test]
fn dwg_mtext_background_no_regression_all_versions() {
    // A plain MTEXT (no fill) must still round-trip its core data on every
    // supported version after the background/column changes.
    for &(version, label) in DWG_VERSIONS {
        let mut mtext = MText::with_value("Plain", Vector3::new(7.0, 8.0, 0.0));
        mtext.height = 3.0;
        let rt = dwg_roundtrip_mtext(version, mtext);
        assert_eq!(
            rt.value, "Plain",
            "DWG {} plain MTEXT value desynced",
            label
        );
        assert_eq!(rt.height, 3.0, "DWG {} plain MTEXT height desynced", label);
        assert_eq!(
            rt.background_fill_flags, 0,
            "DWG {} plain MTEXT spurious flags",
            label
        );
        // A plain (default) MTEXT is non-annotative on every version: R2018+
        // round-trips the false inline bit, and older versions carry no bit at
        // all. Annotativeness comes from the context/style, not a fresh entity.
        assert!(
            !rt.is_annotative,
            "DWG {} plain MTEXT should be non-annotative",
            label
        );
    }
}

#[test]
fn dwg_version_matrix_lwpolyline() {
    for &(version, label) in DWG_VERSIONS {
        let doc = build_minimal_document(
            version,
            EntityType::LwPolyline(LwPolyline::from_points(vec![
                Vector2::new(0.0, 0.0),
                Vector2::new(10.0, 0.0),
                Vector2::new(10.0, 10.0),
            ])),
        );
        let rt = dwg_roundtrip(&doc);
        assert_eq!(
            rt.entity_count(),
            1,
            "DWG {} LwPolyline: entity count changed",
            label
        );
    }
}

#[test]
fn dwg_version_matrix_spline() {
    for &(version, label) in DWG_VERSIONS {
        let doc = build_minimal_document(
            version,
            EntityType::Spline(Spline::from_control_points(
                3,
                vec![
                    Vector3::new(0.0, 0.0, 0.0),
                    Vector3::new(5.0, 10.0, 0.0),
                    Vector3::new(10.0, 0.0, 0.0),
                    Vector3::new(15.0, 10.0, 0.0),
                ],
            )),
        );
        let rt = dwg_roundtrip(&doc);
        assert_eq!(
            rt.entity_count(),
            1,
            "DWG {} Spline: entity count changed",
            label
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  HATCH EDGE ROUNDTRIP TESTS
// ═══════════════════════════════════════════════════════════════════════════

/// Helper: create a doc with a single hatch containing one boundary path with given edges.
fn build_hatch_doc(edges: Vec<BoundaryEdge>, flags: BoundaryPathFlags) -> CadDocument {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut hatch = Hatch::solid();
    let mut path = BoundaryPath::with_flags(flags);
    for e in edges {
        path.edges.push(e);
    }
    hatch.add_path(path);
    doc.add_entity(EntityType::Hatch(hatch)).unwrap();
    doc
}

/// Helper: extract first hatch's first boundary path from a document.
fn extract_hatch_path(doc: &CadDocument) -> &BoundaryPath {
    for entity in doc.entities() {
        if let EntityType::Hatch(h) = entity {
            return &h.paths[0];
        }
    }
    panic!("No hatch found in document");
}

#[test]
fn hatch_line_edge_roundtrip() {
    let edge = BoundaryEdge::Line(LineEdge {
        start: Vector2::new(1.0, 2.0),
        end: Vector2::new(3.0, 4.0),
    });
    let mut flags = BoundaryPathFlags::new();
    flags.set_external(true);
    let doc = build_hatch_doc(
        vec![
            edge.clone(),
            BoundaryEdge::Line(LineEdge {
                start: Vector2::new(3.0, 4.0),
                end: Vector2::new(1.0, 2.0),
            }),
        ],
        flags,
    );
    let rt = dxf_roundtrip(doc);
    let path = extract_hatch_path(&rt);
    assert!(path.flags.is_external(), "external flag lost");
    assert_eq!(path.edges.len(), 2, "edge count");
    if let BoundaryEdge::Line(e) = &path.edges[0] {
        assert!((e.start.x - 1.0).abs() < 1e-6);
        assert!((e.start.y - 2.0).abs() < 1e-6);
        assert!((e.end.x - 3.0).abs() < 1e-6);
        assert!((e.end.y - 4.0).abs() < 1e-6);
    } else {
        panic!("Expected Line edge");
    }
}

#[test]
fn hatch_circular_arc_edge_roundtrip() {
    let edge = BoundaryEdge::CircularArc(CircularArcEdge {
        center: Vector2::new(5.0, 5.0),
        radius: 10.0,
        start_angle: 0.0,
        end_angle: std::f64::consts::FRAC_PI_2,
        counter_clockwise: true,
    });
    let mut flags = BoundaryPathFlags::new();
    flags.set_external(true);
    let doc = build_hatch_doc(vec![edge], flags);
    let rt = dxf_roundtrip(doc);
    let path = extract_hatch_path(&rt);
    assert_eq!(path.edges.len(), 1);
    if let BoundaryEdge::CircularArc(a) = &path.edges[0] {
        assert!((a.center.x - 5.0).abs() < 1e-6);
        assert!((a.center.y - 5.0).abs() < 1e-6);
        assert!((a.radius - 10.0).abs() < 1e-6);
        assert!((a.start_angle - 0.0).abs() < 1e-4);
        assert!((a.end_angle - std::f64::consts::FRAC_PI_2).abs() < 1e-4);
        assert!(a.counter_clockwise);
    } else {
        panic!("Expected CircularArc edge");
    }
}

#[test]
fn hatch_elliptic_arc_edge_roundtrip() {
    let edge = BoundaryEdge::EllipticArc(EllipticArcEdge {
        center: Vector2::new(10.0, 20.0),
        major_axis_endpoint: Vector2::new(15.0, 0.0),
        minor_axis_ratio: 0.5,
        start_angle: 0.0,
        end_angle: std::f64::consts::PI,
        counter_clockwise: true,
    });
    let mut flags = BoundaryPathFlags::new();
    flags.set_external(true);
    let doc = build_hatch_doc(vec![edge], flags);
    let rt = dxf_roundtrip(doc);
    let path = extract_hatch_path(&rt);
    assert_eq!(path.edges.len(), 1);
    if let BoundaryEdge::EllipticArc(e) = &path.edges[0] {
        assert!((e.center.x - 10.0).abs() < 1e-6);
        assert!((e.center.y - 20.0).abs() < 1e-6);
        assert!((e.major_axis_endpoint.x - 15.0).abs() < 1e-6);
        assert!((e.minor_axis_ratio - 0.5).abs() < 1e-6);
        assert!((e.start_angle - 0.0).abs() < 1e-4);
        assert!((e.end_angle - std::f64::consts::PI).abs() < 1e-4);
        assert!(e.counter_clockwise);
    } else {
        panic!("Expected EllipticArc edge");
    }
}

#[test]
fn hatch_spline_edge_roundtrip() {
    let edge = BoundaryEdge::Spline(SplineEdge {
        degree: 3,
        rational: false,
        periodic: false,
        knots: vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
        control_points: vec![
            Vector3::new(0.0, 0.0, 1.0),
            Vector3::new(5.0, 10.0, 1.0),
            Vector3::new(10.0, 10.0, 1.0),
            Vector3::new(15.0, 0.0, 1.0),
        ],
        fit_points: Vec::new(),
        start_tangent: Vector2::new(0.0, 0.0),
        end_tangent: Vector2::new(0.0, 0.0),
    });
    let mut flags = BoundaryPathFlags::new();
    flags.set_external(true);
    let doc = build_hatch_doc(vec![edge], flags);
    let rt = dxf_roundtrip(doc);
    let path = extract_hatch_path(&rt);
    assert_eq!(path.edges.len(), 1);
    if let BoundaryEdge::Spline(s) = &path.edges[0] {
        assert_eq!(s.degree, 3);
        assert_eq!(s.knots.len(), 8);
        assert_eq!(s.control_points.len(), 4);
        assert!((s.control_points[1].x - 5.0).abs() < 1e-6);
        assert!((s.control_points[1].y - 10.0).abs() < 1e-6);
    } else {
        panic!("Expected Spline edge");
    }
}

#[test]
fn hatch_polyline_edge_roundtrip() {
    let edge = BoundaryEdge::Polyline(PolylineEdge::new(
        vec![
            Vector2::new(0.0, 0.0),
            Vector2::new(100.0, 0.0),
            Vector2::new(100.0, 100.0),
            Vector2::new(0.0, 100.0),
        ],
        true,
    ));
    let mut flags = BoundaryPathFlags::new();
    flags.set_external(true);
    flags.set_polyline(true);
    let doc = build_hatch_doc(vec![edge], flags);
    let rt = dxf_roundtrip(doc);
    let path = extract_hatch_path(&rt);
    assert!(path.flags.is_polyline(), "polyline flag lost");
    assert_eq!(path.edges.len(), 1);
    if let BoundaryEdge::Polyline(p) = &path.edges[0] {
        assert_eq!(p.vertices.len(), 4);
        assert!(p.is_closed);
        assert!((p.vertices[0].x - 0.0).abs() < 1e-6);
        assert!((p.vertices[1].x - 100.0).abs() < 1e-6);
        assert!((p.vertices[2].y - 100.0).abs() < 1e-6);
    } else {
        panic!("Expected Polyline edge");
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  VPORT render mode (visual style) — DXF code 281 / DWG RC 281
// ═══════════════════════════════════════════════════════════════════════════

/// Distinct per-tile visual styles on duplicate `*Active` VPORT entries must
/// survive both DXF and DWG roundtrips.
#[test]
fn roundtrip_vport_render_mode() {
    use acadrust::entities::ViewportRenderMode as M;
    use acadrust::tables::VPort;

    let (mut doc, _) = build_rich_document(DxfVersion::AC1032);
    // Replace the vport table with two tiled *Active entries carrying
    // distinct visual styles (the model-tile scenario).
    doc.vports.clear();
    let mut a = VPort::new("*Active");
    a.view_height = 100.0;
    a.render_mode = M::FlatShaded;
    a.handle = doc.allocate_handle();
    let mut b = VPort::new("*Active");
    b.view_height = 200.0;
    b.render_mode = M::GouraudShadedWithEdges;
    b.handle = doc.allocate_handle();
    doc.vports.add_allow_duplicate(a);
    doc.vports.add_allow_duplicate(b);

    let modes = |d: &CadDocument| -> Vec<M> {
        d.vports
            .iter()
            .filter(|v| v.name == "*Active")
            .map(|v| v.render_mode)
            .collect()
    };

    let dxf = modes(&dxf_roundtrip(doc.clone()));
    assert!(dxf.contains(&M::FlatShaded), "DXF lost FlatShaded: {dxf:?}");
    assert!(
        dxf.contains(&M::GouraudShadedWithEdges),
        "DXF lost GouraudShadedWithEdges: {dxf:?}"
    );

    let dwg = modes(&dwg_roundtrip(&doc));
    assert!(dwg.contains(&M::FlatShaded), "DWG lost FlatShaded: {dwg:?}");
    assert!(
        dwg.contains(&M::GouraudShadedWithEdges),
        "DWG lost GouraudShadedWithEdges: {dwg:?}"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
//  Annotative flag round-trip
//
//  Per the DXF/DWG standard the flag lives in different places per record:
//   • MLEADERSTYLE — native attribute (DXF group 296, DWG bit).
//   • STYLE / DIMSTYLE / TABLESTYLE — XDATA under the `AcadAnnotative`
//     application: `AnnotativeData { 1 <flag> }`.
// ═══════════════════════════════════════════════════════════════════════════

fn build_annotative_document() -> CadDocument {
    use acadrust::objects::{MultiLeaderStyle, ObjectType, TableStyle};
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);

    if let Some(s) = doc.text_styles.get_mut("Standard") {
        s.annotative = true;
    }
    if let Some(d) = doc.dim_styles.get_mut("Standard") {
        d.annotative = true;
    }

    let mut mls = MultiLeaderStyle::new("AnnoML");
    mls.handle = doc.allocate_handle();
    mls.is_annotative = true;
    doc.objects
        .insert(mls.handle, ObjectType::MultiLeaderStyle(mls));

    let mut tbs = TableStyle::new("AnnoTS");
    tbs.handle = doc.allocate_handle();
    tbs.annotative = true;
    doc.objects.insert(tbs.handle, ObjectType::TableStyle(tbs));

    doc
}

fn mleader_is_annotative(doc: &CadDocument) -> bool {
    use acadrust::objects::ObjectType;
    // Order-independent: the objects map is a HashMap, so a find_map here
    // raced between the default "Standard" style (not annotative) and the
    // test's annotative one (issue #51 class of bugs).
    doc.objects
        .values()
        .any(|o| matches!(o, ObjectType::MultiLeaderStyle(s) if s.is_annotative))
}

fn table_is_annotative(doc: &CadDocument) -> bool {
    use acadrust::objects::ObjectType;
    doc.objects
        .values()
        .any(|o| matches!(o, ObjectType::TableStyle(s) if s.annotative))
}

#[test]
fn dxf_roundtrip_annotative_styles() {
    let rt = dxf_roundtrip(build_annotative_document());
    assert!(
        rt.text_styles
            .get("Standard")
            .map(|s| s.annotative)
            .unwrap_or(false),
        "DXF: text style annotative lost"
    );
    assert!(
        rt.dim_styles
            .get("Standard")
            .map(|d| d.annotative)
            .unwrap_or(false),
        "DXF: dim style annotative lost"
    );
    assert!(
        mleader_is_annotative(&rt),
        "DXF: mleader style annotative lost"
    );
    assert!(table_is_annotative(&rt), "DXF: table style annotative lost");
}

#[test]
fn dwg_roundtrip_annotative_styles() {
    // MLEADERSTYLE: native DWG bit. STYLE/DIMSTYLE: AcadAnnotative EED.
    // (TABLESTYLE is not yet serialized to DWG — tracked separately.)
    let rt = dwg_roundtrip(&build_annotative_document());
    assert!(
        rt.text_styles
            .get("Standard")
            .map(|s| s.annotative)
            .unwrap_or(false),
        "DWG: text style annotative lost"
    );
    assert!(
        rt.dim_styles
            .get("Standard")
            .map(|d| d.annotative)
            .unwrap_or(false),
        "DWG: dim style annotative lost"
    );
    assert!(
        mleader_is_annotative(&rt),
        "DWG: mleader style annotative lost"
    );
}

#[test]
fn dxf_roundtrip_complex_linetype_shape() {
    use acadrust::tables::LineTypeComplexData;
    let mut doc = build_minimal_document(DxfVersion::AC1032, EntityType::Point(Point::new()));

    let mut lt = LineType::new("SHAPELT");
    let mut dash = LineTypeElement::dash(5.0);
    dash.complex = Some(LineTypeComplexData {
        content: LineTypeComplexContent::Shape { shape_number: 42 },
        style_handle: Handle::new(0x80),
        scale: 2.0,
        rotation: 30.0,
        absolute_rotation: false,
        offset: [1.0, 0.5],
    });
    let mut space = LineTypeElement::space(2.0);
    space.complex = Some(LineTypeComplexData::default());
    lt.elements.push(dash);
    lt.elements.push(space);
    lt.pattern_length = 7.0;

    doc.line_types.add(lt).ok();

    let rt = dxf_roundtrip(doc);
    let rt_lt = rt.line_types.get("SHAPELT").expect("linetype lost");
    assert!(rt_lt.is_complex());
    assert_eq!(rt_lt.elements.len(), 2);
    let c0 = rt_lt.elements[0].complex.as_ref().expect("complex lost");
    assert!(c0.is_shape());
    assert_eq!(c0.shape_number(), Some(42));
    assert!((c0.scale - 2.0).abs() < 1e-6);
    assert!((c0.rotation - 30.0).abs() < 1e-6);
    assert!(!c0.is_absolute_rotation());
}

#[test]
fn dxf_roundtrip_complex_linetype_text() {
    use acadrust::tables::LineTypeComplexData;
    let mut doc = build_minimal_document(DxfVersion::AC1032, EntityType::Point(Point::new()));

    let mut lt = LineType::new("TEXTLT");
    let mut dash = LineTypeElement::dash(3.0);
    dash.complex = Some(LineTypeComplexData {
        content: LineTypeComplexContent::Text {
            text: "X".to_string(),
        },
        style_handle: Handle::new(0x80),
        scale: 1.5,
        rotation: 45.0,
        absolute_rotation: true,
        offset: [0.0, 0.25],
    });
    lt.elements.push(dash);
    lt.pattern_length = 3.0;

    doc.line_types.add(lt).ok();

    let rt = dxf_roundtrip(doc);
    let rt_lt = rt.line_types.get("TEXTLT").expect("linetype lost");
    assert!(rt_lt.is_complex());
    let c0 = rt_lt.elements[0].complex.as_ref().expect("complex lost");
    assert!(c0.is_text());
    assert_eq!(c0.text(), Some("X"));
    assert!(c0.is_absolute_rotation());
}

#[test]
fn dwg_roundtrip_complex_linetype_shape() {
    use acadrust::tables::LineTypeComplexData;
    let mut doc = build_minimal_document(DxfVersion::AC1032, EntityType::Point(Point::new()));

    let mut lt = LineType::new("SHAPELT");
    // Assign a valid handle so the DWG writer can reference it correctly.
    // User-created linetypes with Handle::NULL may not survive DWG roundtrip
    // without additional handle pre-allocation logic.
    lt.handle = Handle::new(0xF0);
    let mut dash = LineTypeElement::dash(4.0);
    dash.complex = Some(LineTypeComplexData {
        content: LineTypeComplexContent::Shape { shape_number: 10 },
        style_handle: Handle::NULL,
        scale: 1.0,
        rotation: 0.0,
        absolute_rotation: false,
        offset: [0.0, 0.0],
    });
    lt.elements.push(dash);
    lt.pattern_length = 4.0;

    doc.line_types.add(lt).ok();

    let rt = dwg_roundtrip(&doc);
    let rt_lt = rt.line_types.get("SHAPELT").expect("linetype lost");
    assert!(rt_lt.is_complex());
    let c0 = rt_lt.elements[0].complex.as_ref().expect("complex lost");
    assert!(c0.is_shape());
    assert_eq!(c0.shape_number(), Some(10));
}

// ── DWG: structured XDATA (records) round-trip (issue 249) ─────────────
// A plugin attaches XDATA to an entity via `write_record`, which lands in
// `ExtendedData::records`. The DWG writer used to serialize only verbatim
// `raw_dwg_eed` and drop `records`, so the data vanished on the first save.
// These assert the values survive a full write → read cycle in both the
// R2007+ UTF-16 string branch and the pre-R2007 codepage branch.
fn xdata_record_survives_dwg_roundtrip(version: DxfVersion) {
    use acadrust::tables::AppId;
    use acadrust::xdata::{ExtendedDataRecord, XDataValue};

    let mut doc = CadDocument::with_version(version);
    let mut app = AppId::new("DEMO_SURVEY");
    app.handle = doc.allocate_handle();
    let _ = doc.app_ids.add(app);
    let h = doc
        .add_entity(EntityType::Point(Point::from_coords(5.0, 6.0, 0.0)))
        .expect("add point");

    let mut rec = ExtendedDataRecord::new("DEMO_SURVEY");
    rec.add_value(XDataValue::String("PNT-1".to_string()));
    rec.add_value(XDataValue::Integer16(7));
    rec.add_value(XDataValue::Integer32(123456));
    rec.add_value(XDataValue::Real(1.25));
    rec.add_value(XDataValue::Point3D(Vector3::new(9.0, 8.0, 7.0)));
    doc.get_entity_mut(h)
        .expect("entity")
        .common_mut()
        .extended_data
        .add_record(rec);

    let rt = dwg_roundtrip(&doc);
    let got = rt
        .entities()
        .find_map(|e| match e {
            EntityType::Point(_) => e.common().extended_data.get_record("DEMO_SURVEY"),
            _ => None,
        })
        .expect("record survived save+reopen");
    assert_eq!(
        got.values,
        vec![
            XDataValue::String("PNT-1".to_string()),
            XDataValue::Integer16(7),
            XDataValue::Integer32(123456),
            XDataValue::Real(1.25),
            XDataValue::Point3D(Vector3::new(9.0, 8.0, 7.0)),
        ],
        "XDATA values changed across DWG roundtrip ({version:?})"
    );
    // The application must stay registered so other CAD apps accept the EED.
    assert!(rt.app_ids.get("DEMO_SURVEY").is_some(), "APPID lost");
}

#[test]
fn xdata_record_survives_dwg_roundtrip_r2018() {
    xdata_record_survives_dwg_roundtrip(DxfVersion::AC1032);
}

#[test]
fn xdata_record_survives_dwg_roundtrip_r2004() {
    xdata_record_survives_dwg_roundtrip(DxfVersion::AC1018);
}
