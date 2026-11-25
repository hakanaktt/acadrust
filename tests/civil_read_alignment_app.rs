//! The alignment records of drawings the application wrote whose class
//! keys close on the alignment's site edge and whose site-edge double
//! packs 1.0/0.0 (the D016 packet-1 read gap W1-1: `alignment_elements`
//! decoded no elements from them).
//!
//! The reader treated both places as fixed zeros; the application's
//! records keep the same 138-bit span with the site's data in it.

use acadrust::classes::civil::AlignmentElementKind;
use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::types::Vector2;
use acadrust::DwgReader;

/// The application's tutorial drawing, from `ACADRUST_CIVIL_TUTORIALS_DIR`.
fn app_drawing(name: &str) -> Option<acadrust::CadDocument> {
    let dir = std::env::var_os("ACADRUST_CIVIL_TUTORIALS_DIR")?;
    let path = std::path::Path::new(&dir).join(name);
    if !path.exists() {
        return None;
    }
    Some(DwgReader::from_file(&path).unwrap().read().unwrap())
}

fn alignments(doc: &acadrust::CadDocument) -> Vec<(u64, String, Vec<AlignmentElementKind>)> {
    let mut out = Vec::new();
    for e in doc.entities() {
        if let EntityType::Extended(x) = e {
            if let ExtendedEntityData::Arch(a) = &x.data {
                if a.dxf_name == "AECC_ALIGNMENT" {
                    let elements = a.alignment_elements().unwrap_or_default();
                    out.push((x.common.handle.value(), a.alignment_name().unwrap_or_default(), elements.into_iter().map(|e| e.kind).collect()));
                }
            }
        }
    }
    out
}

fn kind_of(elements: &[AlignmentElementKind], index: usize) -> &AlignmentElementKind {
    &elements[index]
}

/// Pipe Networks-3B.dwg: the corridor baseline ROAD1 (its site edge in
/// the keys' closing long: 16) and the catchment alignment XC_STORM
/// (32). Geometry exactly as the application reads it
/// (execute_civil_code `AlignmentLine/AlignmentArc/AlignmentSpiral`,
/// 2026-11-25).
#[test]
fn corridor_baseline_and_catchment_alignments_decode() {
    let Some(doc) = app_drawing("Pipe Networks-3B.dwg") else { return };
    let mut als = alignments(&doc);
    als.sort_by_key(|(h, _, _)| *h);
    let road1 = als.iter().find(|(_, name, _)| name == "ROAD1").expect("ROAD1");
    assert_eq!(road1.2.len(), 9);
    // the stored line geometry, exact to the last digit
    assert_eq!(
        kind_of(&road1.2, 0),
        &AlignmentElementKind::FixedLine { start: Vector2::new(4012.14, 3595.31), end: Vector2::new(4466.31450725, 3952.80401514) }
    );
    assert_eq!(
        kind_of(&road1.2, 8),
        &AlignmentElementKind::FixedLine { start: Vector2::new(5212.87236926, 4286.96267977), end: Vector2::new(5287.68, 4673.63) }
    );
    // the clothoids and the arcs between them
    assert!(matches!(kind_of(&road1.2, 1), AlignmentElementKind::SpiralIn { length, radius, .. } if *length == 50.0 && *radius == 200.0));
    assert!(matches!(kind_of(&road1.2, 3), AlignmentElementKind::SpiralOut { length: 50.0, radius: 200.0, .. }));
    assert!(matches!(kind_of(&road1.2, 2), AlignmentElementKind::FixedArc { start, end, .. }
        if *start == Vector2::new(4506.82918112, 3982.04595498) && *end == Vector2::new(4546.38258284, 4000.31561444)));
    // the solved pieces chain without gaps (the application's arc radius
    // 200.00000000247874 against the solved 200.00000000025)
    let road1_entity = doc.entities().find_map(|e| match e {
        EntityType::Extended(x) if x.common.handle.value() == road1.0 => Some(x.data.clone()),
        _ => None,
    });
    let pieces = road1_entity.and_then(|data| match &data {
        ExtendedEntityData::Arch(a) => Some(a.alignment_pieces()),
        _ => None,
    }).unwrap_or_default();
    assert_eq!(pieces.len(), 9);
    for window in pieces.windows(2) {
        let end = window[0].1.last().expect("piece points");
        let start = window[1].1.first().expect("piece points");
        assert!((end.x - start.x).abs() < 1e-6 && (end.y - start.y).abs() < 1e-6, "pieces chain");
    }

    let xc = als.iter().find(|(_, name, _)| name == "XC_STORM").expect("XC_STORM");
    assert_eq!(
        xc.2,
        vec![
            AlignmentElementKind::FixedLine { start: Vector2::new(4669.9453522461135, 4055.922695668984), end: Vector2::new(4765.325575950574, 4272.661778468604) },
            AlignmentElementKind::FixedLine { start: Vector2::new(4765.325575950574, 4272.661778468604), end: Vector2::new(4761.312414915001, 4630.104633896095) },
        ]
    );
}

/// The basic alignment the alignment creation tools make (Align-2) and
/// the assembly drawings' baselines decode again; the drawings whose
/// alignments always decoded keep their counts.
#[test]
fn the_other_site_forms_decode_and_the_working_forms_hold() {
    let Some(doc) = app_drawing("Align-2.dwg") else { return };
    let als = alignments(&doc);
    assert_eq!(als.len(), 1);
    assert_eq!(als[0].2.len(), 3, "Align-2's line-arc-line");

    for (name, counts) in [
        ("Pipe Networks-3B.dwg", vec![9, 2]),
        ("Sections-Project-Objects.dwg", vec![3, 3, 3]),
        ("Corridor-2a.dwg", vec![3, 3]),
        ("Align-Superelevation-1.dwg", vec![12]),
    ] {
        let Some(doc) = app_drawing(name) else { continue };
        let counts = {
            let mut c = counts;
            c.sort();
            c
        };
        let mut got: Vec<usize> = alignments(&doc).iter().map(|(_, _, e)| e.len()).collect();
        got.sort();
        assert_eq!(got, counts, "{name}");
    }
}
