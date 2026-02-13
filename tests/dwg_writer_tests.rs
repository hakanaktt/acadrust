//! Master DWG writer test file — scaffolded per-phase.
//!
//! Each phase of DWG_GAP_FIX_PLAN.md has its own submodule here so
//! that tests can be run selectively:
//!
//!   cargo test --test dwg_writer_tests
//!   cargo test --test dwg_writer_tests phase0

#[allow(dead_code)]
mod common;

// ===========================================================================
// Phase 0 — test infrastructure self-tests
// ===========================================================================

mod phase0_infrastructure {
    use super::common;
    use super::common::comparison;

    #[test]
    fn test_common_sample_dwg_path_resolution() {
        for ver in &common::DWG_SAMPLE_VERSIONS {
            let p = common::sample_dwg_path(ver);
            assert!(
                p.exists(),
                "DWG sample not found: {}",
                p.display()
            );
        }
    }

    #[test]
    fn test_common_sample_dxf_path_resolution() {
        for ver in &common::DXF_SAMPLE_VERSIONS {
            let ascii = common::sample_dxf_path(ver, "ascii");
            let binary = common::sample_dxf_path(ver, "binary");
            assert!(
                ascii.exists(),
                "DXF ascii sample not found: {}",
                ascii.display()
            );
            assert!(
                binary.exists(),
                "DXF binary sample not found: {}",
                binary.display()
            );
        }
    }

    #[test]
    fn test_common_read_all_sample_dwgs() {
        for ver in &common::DWG_SAMPLE_VERSIONS {
            // AC1021 (R2007) uses RS encoding not yet fully supported
            if *ver == "AC1021" {
                continue;
            }
            let doc = common::read_sample_dwg(ver);
            let n = common::entity_count(&doc);
            assert!(n > 0, "{ver}: expected >0 entities, got {n}");
        }
    }

    #[test]
    fn test_common_read_all_sample_dxfs() {
        for ver in &common::DXF_SAMPLE_VERSIONS {
            let doc = common::read_sample_dxf(ver, "ascii");
            let n = common::entity_count(&doc);
            assert!(n > 0, "{ver} ascii: expected >0 entities, got {n}");
        }
    }

    #[test]
    fn test_common_create_test_document_has_defaults() {
        let doc = common::builders::create_all_entities_document();
        // Must have at least 30 entities (we put 41+)
        assert!(
            doc.entity_count() >= 30,
            "Expected >=30 entities, got {}",
            doc.entity_count()
        );
        // Default tables present
        assert!(!doc.layers.is_empty(), "Should have default layer");
        assert!(!doc.line_types.is_empty(), "Should have default linetypes");
        assert!(
            !doc.block_records.is_empty(),
            "Should have default block records"
        );
    }

    #[test]
    fn test_common_roundtrip_helper_minimal() {
        use acadrust::types::DxfVersion;
        let mut doc = acadrust::CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let line =
            acadrust::entities::Line::from_coords(0.0, 0.0, 0.0, 10.0, 10.0, 0.0);
        doc.add_entity(acadrust::entities::EntityType::Line(line))
            .unwrap();

        let rdoc = common::roundtrip_dxf(&doc, "minimal_test");
        assert_eq!(common::entity_count(&rdoc), 1);
    }

    #[test]
    fn test_common_entity_count_helper() {
        let doc = common::builders::create_all_entities_document();
        let counts = common::entity_type_counts(&doc);
        // Should have multiple distinct entity types
        assert!(counts.len() >= 15, "Expected >=15 types, got {}", counts.len());
    }

    #[test]
    fn test_common_f64_tolerance_pass() {
        comparison::assert_f64_eq(1.0, 1.0 + 1e-10, 1e-6);
        comparison::assert_f64_eq(0.0, 0.0, 1e-6);
        comparison::assert_f64_eq(-5.5, -5.5 + 1e-8, 1e-6);
    }

    #[test]
    #[should_panic(expected = "f64 mismatch")]
    fn test_common_f64_tolerance_fail() {
        comparison::assert_f64_eq(1.0, 1.01, 1e-6);
    }

    #[test]
    fn test_common_vec3_tolerance_pass() {
        use acadrust::Vector3;
        let a = Vector3::new(1.0, 2.0, 3.0);
        let b = Vector3::new(1.0 + 1e-10, 2.0 - 1e-10, 3.0 + 1e-10);
        comparison::assert_vec3_eq(&a, &b, 1e-6);
    }

    #[test]
    #[should_panic(expected = "Vector3 mismatch")]
    fn test_common_vec3_tolerance_fail() {
        use acadrust::Vector3;
        let a = Vector3::new(1.0, 2.0, 3.0);
        let b = Vector3::new(1.0, 2.01, 3.0);
        comparison::assert_vec3_eq(&a, &b, 1e-6);
    }

    #[test]
    fn test_common_comparison_identical_entities() {
        use acadrust::entities::{EntityType, Line};
        let line = EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 10.0, 10.0, 0.0));
        let diffs = comparison::compare_entity_geometry(&line, &line);
        assert!(diffs.is_empty(), "Identical entities should have no diffs: {diffs:?}");
    }

    #[test]
    fn test_common_comparison_different_entities() {
        use acadrust::entities::{EntityType, Line};
        let a = EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 10.0, 10.0, 0.0));
        let b = EntityType::Line(Line::from_coords(0.0, 0.0, 0.0, 99.0, 99.0, 0.0));
        let diffs = comparison::compare_entity_geometry(&a, &b);
        assert!(!diffs.is_empty(), "Different entities should have diffs");
    }

    #[test]
    fn test_common_entity_sort_key() {
        use acadrust::entities::{EntityType, Line, Circle};

        let line = EntityType::Line(Line::from_coords(5.0, 10.0, 0.0, 20.0, 20.0, 0.0));
        let circle = EntityType::Circle(Circle::from_coords(5.0, 10.0, 0.0, 3.0));

        let (lx, ly, _) = comparison::entity_sort_key(&line);
        let (cx, cy, _) = comparison::entity_sort_key(&circle);

        // Both should sort by their primary point
        assert!((lx - 5.0).abs() < 1e-10);
        assert!((ly - 10.0).abs() < 1e-10);
        assert!((cx - 5.0).abs() < 1e-10);
        assert!((cy - 10.0).abs() < 1e-10);
    }
}

// ===========================================================================
// Future phases — stubs for easy scaffolding
// ===========================================================================

// mod phase1_polylines;
// mod phase2_attributes;
// mod phase3_dimensions;
// mod phase4_hatch;
// mod phase5_complex_entities;
// mod phase6_multileader_images;
// mod phase7_critical_objects;
// mod phase8_remaining_objects;
// mod phase9_tables_sections;
// mod phase10_reader_gaps;
