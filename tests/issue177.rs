//! Native AutoCAD annotative context leaves (issue #177).
//!
//! `annotative_2018.dwg` was saved from AutoCAD 2027 as an R2018 drawing. It
//! contains an aligned dimension, text, MTEXT, leader, multileader, hatch, and
//! an MTEXT attribute, each annotative at 1:1 and 1:2.

use acadrust::entities::{BoundaryEdge, EntityType};
use acadrust::objects::{ObjectContextKind, ObjectType};
use acadrust::{DwgReadOptions, DwgReader};

const FIXTURE: &str = "tests/fixtures/issue177/annotative_2018.dwg";

#[test]
fn autocad_annotative_contexts_read_strictly() {
    let doc = DwgReader::from_file_with_options(FIXTURE, DwgReadOptions::default())
        .unwrap()
        .read()
        .unwrap_or_else(|error| panic!("strict read failed: {error}"));

    let mut counts = std::collections::BTreeMap::<&str, usize>::new();
    for object in doc.objects.values() {
        let ObjectType::ObjectContextData(context) = object else {
            continue;
        };
        *counts.entry(context.class_name()).or_default() += 1;
        assert!(
            context.scale.value() != 0,
            "{} has no annotation scale",
            context.class_name()
        );
        match &context.kind {
            ObjectContextKind::HatchScale(hatch) => {
                assert_eq!(hatch.pattern_lines.len(), 1);
                assert!(hatch.pattern_scale.is_finite() && hatch.pattern_scale > 0.0);
                assert_eq!(hatch.loops.len(), 1);
                let angle = hatch.pattern_lines[0].angle;
                assert!(
                    (angle - std::f64::consts::FRAC_PI_4).abs() < 1e-9
                        || (angle - 45.0_f64.to_radians()).abs() < 1e-9
                );
            }
            ObjectContextKind::Dim(dim) => {
                assert!(dim.def_pt.x.is_finite() && dim.def_pt.y.is_finite());
            }
            ObjectContextKind::Text { insertion, .. } => {
                assert!(insertion.x.is_finite() && insertion.y.is_finite());
            }
            ObjectContextKind::MText(mtext) => {
                assert!(mtext.insertion.x.is_finite());
            }
            ObjectContextKind::Leader(leader) => {
                assert!(leader.points.len() >= 2);
            }
            _ => {}
        }
    }
    for name in [
        "ACDB_ALDIMOBJECTCONTEXTDATA_CLASS",
        "ACDB_TEXTOBJECTCONTEXTDATA_CLASS",
        "ACDB_MTEXTOBJECTCONTEXTDATA_CLASS",
        "ACDB_LEADEROBJECTCONTEXTDATA_CLASS",
        "ACDB_HATCHSCALECONTEXTDATA_CLASS",
        "ACDB_MLEADEROBJECTCONTEXTDATA_CLASS",
        "ACDB_MTEXTATTRIBUTEOBJECTCONTEXTDATA_CLASS",
    ] {
        assert!(
            counts.get(name).copied().unwrap_or(0) >= 2,
            "{name}: {counts:?}"
        );
    }

    let mut elliptic = false;
    for entity in doc.entities() {
        let EntityType::Hatch(hatch) = entity else {
            continue;
        };
        for path in &hatch.paths {
            for edge in &path.edges {
                if let BoundaryEdge::EllipticArc(edge) = edge {
                    assert!((edge.end_angle - 1.107_148_717_794_090_4).abs() < 1e-12);
                    elliptic = true;
                }
            }
        }
    }
    assert!(elliptic, "the pie-slice hatch was not read");
}
