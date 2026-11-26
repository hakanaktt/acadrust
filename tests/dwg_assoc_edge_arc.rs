use std::io::Cursor;

use acadrust::objects::{
    AssocCurveValue, AssocEdgeActionParam, AssocSubcurveKind, AssociativeData, AssociativeObject,
    ObjectType,
};
use acadrust::types::{DxfVersion, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn arc() -> AssocEdgeActionParam {
    AssocEdgeActionParam {
        has_action: true,
        action_type: 11,
        subcurve_kind: AssocSubcurveKind::Arc,
        curve: vec![
            AssocCurveValue::Point(Vector3::new(2.0, 3.0, 4.0)),
            AssocCurveValue::Point(Vector3::new(0.0, 0.0, 1.0)),
            AssocCurveValue::Point(Vector3::new(1.0, 0.0, 0.0)),
            AssocCurveValue::Real(5.0),
            AssocCurveValue::Real(0.25),
            AssocCurveValue::Real(2.5),
        ],
        ..Default::default()
    }
}

fn encode(version: DxfVersion, edge: &AssocEdgeActionParam) -> Vec<u8> {
    let mut document = CadDocument::new();
    document.version = version;
    let handle = document.allocate_handle();
    document.objects.insert(
        handle,
        ObjectType::Associative(AssociativeObject {
            handle,
            owner: document.header.named_objects_dict_handle,
            dxf_name: "ASSOCEDGEACTIONPARAM".to_string(),
            cpp_class_name: "AcDbAssocEdgeActionParam".to_string(),
            data: AssociativeData::EdgeActionParam(edge.clone()),
            ..Default::default()
        }),
    );
    DwgWriter::write_to_vec(&document).expect("write synthetic edge")
}

fn round_trip(version: DxfVersion, edge: &AssocEdgeActionParam) {
    let decoded = DwgReader::from_stream(Cursor::new(encode(version, edge)))
        .read_with_stats()
        .expect("read synthetic edge");
    assert_eq!(decoded.stats.recovered_errors, 0, "{:?}", decoded.stats);
    assert_eq!(
        decoded.stats.skipped_source_records, 0,
        "{:?}",
        decoded.stats
    );
    let actual = decoded
        .document
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::Associative(AssociativeObject {
                data: AssociativeData::EdgeActionParam(value),
                ..
            }) => Some(value),
            _ => None,
        });
    assert_eq!(actual, Some(edge));
}

#[test]
fn four_scalar_arcs_preserve_nonzero_fourth_value() {
    let mut edge = arc();
    edge.curve.push(AssocCurveValue::Real(7.25));
    for version in [DxfVersion::AC1027, DxfVersion::AC1032] {
        round_trip(version, &edge);
    }
}

#[test]
fn ellipse_and_lines_keep_their_layouts() {
    for version in [DxfVersion::AC1024, DxfVersion::AC1032] {
        let mut edge = arc();
        edge.action_type = 17;
        edge.subcurve_kind = AssocSubcurveKind::Ellipse;
        edge.curve
            .extend([AssocCurveValue::Real(3.5), AssocCurveValue::Real(4.5)]);
        round_trip(version, &edge);
        for (kind, subcurve) in [
            (19, AssocSubcurveKind::Line),
            (23, AssocSubcurveKind::LineSegment3d),
        ] {
            edge.action_type = kind;
            edge.subcurve_kind = subcurve;
            edge.curve.truncate(2);
            round_trip(version, &edge);
        }
    }
}

#[test]
fn composite_arc_keeps_its_scalars_and_following_line() {
    // A nested arc has the same layout as a terminal one: three scalars
    // before AC1027, four from AC1027 on.
    for (version, fourth) in [(DxfVersion::AC1024, None), (DxfVersion::AC1032, Some(8.5))] {
        let mut edge = arc();
        let mut curve = vec![AssocCurveValue::Int(2), AssocCurveValue::Int(11)];
        curve.extend(edge.curve.clone());
        curve.extend(fourth.map(AssocCurveValue::Real));
        curve.push(AssocCurveValue::Int(19));
        curve.extend(edge.curve[..2].iter().cloned());
        edge.action_type = 47;
        edge.subcurve_kind = AssocSubcurveKind::None;
        edge.curve = curve;
        round_trip(version, &edge);
    }
}

fn decoded_edge(version: DxfVersion, edge: &AssocEdgeActionParam) -> Option<AssocEdgeActionParam> {
    let outcome = DwgReader::from_stream(Cursor::new(encode(version, edge)))
        .read_with_stats()
        .ok()?;
    outcome
        .document
        .objects
        .values()
        .find_map(|object| match object {
            ObjectType::Associative(AssociativeObject {
                data: AssociativeData::EdgeActionParam(value),
                ..
            }) => Some(value.clone()),
            _ => None,
        })
}

fn assert_rejected(version: DxfVersion, edge: &AssocEdgeActionParam, what: &str) {
    let result = DwgReader::from_stream(Cursor::new(encode(version, edge))).read_with_stats();
    // The public reader can preserve a rejected record as opaque data;
    // that must not mask acceptance as a successfully decoded edge.
    if let Ok(outcome) = result {
        assert!(
            !outcome.document.objects.values().any(|object| matches!(
                object,
                ObjectType::Associative(AssociativeObject {
                    data: AssociativeData::EdgeActionParam(_),
                    ..
                })
            )),
            "{what}: {version:?} decoded as a typed edge"
        );
        assert!(
            outcome.stats.recovered_errors > 0
                || outcome.stats.skipped_source_records > 0
                || !outcome.stats.diagnostics.is_empty(),
            "{what}: {version:?} reported no error"
        );
    }
}

fn scalar_count(edge: &AssocEdgeActionParam) -> usize {
    edge.curve
        .iter()
        .filter(|item| matches!(item, AssocCurveValue::Real(_)))
        .count()
}

/// An arc stores radius, start and end angle before AC1027; AC1027 and later
/// add a fourth scalar.
#[test]
fn arcs_before_ac1027_store_three_scalars() {
    let edge = arc();
    assert_eq!(scalar_count(&edge), 3);
    for version in [DxfVersion::AC1018, DxfVersion::AC1021, DxfVersion::AC1024] {
        round_trip(version, &edge);
        let decoded = decoded_edge(version, &edge).expect("typed edge");
        assert_eq!(scalar_count(&decoded), 3);
        assert_eq!(decoded.curve, edge.curve);
    }
}

#[test]
fn incomplete_arcs_are_not_accepted_as_typed_edges() {
    // Fewer scalars than the version stores.
    let mut two = arc();
    two.curve.pop();
    let mut one = two.clone();
    one.curve.pop();
    for version in [DxfVersion::AC1024, DxfVersion::AC1027, DxfVersion::AC1032] {
        assert_rejected(version, &two, "two scalars");
        assert_rejected(version, &one, "one scalar");
    }
    for version in [DxfVersion::AC1027, DxfVersion::AC1032] {
        assert_rejected(version, &arc(), "three scalars");
    }
}
