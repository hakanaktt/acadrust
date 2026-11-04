//! Literal DXF flags and a DWG -> DXF -> DWG chain independently guard
//! fit-spline parameterization; matching two lossy codec chains is insufficient.
use std::io::Cursor;

use acadrust::entities::{Helix, Spline};
use acadrust::{
    CadDocument, DwgReader, DwgWriter, DxfReader, DxfVersion, DxfWriter, EntityType, Vector3,
};

fn read_dxf(bytes: Vec<u8>) -> CadDocument {
    DxfReader::from_reader(Cursor::new(bytes))
        .unwrap()
        .read()
        .unwrap()
}

fn first_spline(doc: &CadDocument) -> &Spline {
    doc.entities()
        .find_map(|e| match e {
            EntityType::Spline(s) => Some(s),
            _ => None,
        })
        .expect("spline")
}

fn literal_spline(flags: i16) -> Vec<u8> {
    // A literal fit-only cubic with endpoint derivatives. No writer under test
    // constructs these flags or their expected meanings.
    format!("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1032\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n0\nSPLINE\n5\n10\n100\nAcDbEntity\n8\n0\n100\nAcDbSpline\n70\n{flags}\n71\n3\n72\n0\n73\n0\n74\n2\n44\n1e-10\n12\n12\n22\n32\n32\n0\n13\n-0.5\n23\n29\n33\n0\n11\n0\n21\n0\n31\n0\n11\n8\n21\n10\n31\n0\n0\nENDSEC\n0\nEOF\n").into_bytes()
}

fn written_flags(bytes: &[u8], kind: &str) -> i16 {
    let text = std::str::from_utf8(bytes).unwrap();
    let lines: Vec<_> = text.lines().map(str::trim).collect();
    let mut in_entity = false;
    for pair in lines.chunks_exact(2) {
        if pair[0] == "0" {
            in_entity = pair[1] == kind;
        }
        if in_entity && pair[0] == "70" {
            return pair[1].parse().unwrap();
        }
    }
    panic!("missing {kind} flags");
}

fn fit_spline(parameterization: i32) -> Spline {
    // Same two-fit-point/endpoint-derivative profile as the reported AC1032
    // failure, with independent small coordinates for a self-contained fixture.
    let mut spline = Spline::from_fit_points(vec![Vector3::ZERO, Vector3::new(8.0, 10.0, 0.0)]);
    spline.knot_parameterization = parameterization;
    spline.begin_tangent = Vector3::new(12.0, 32.0, 0.0);
    spline.end_tangent = Vector3::new(-0.5, 29.0, 0.0);
    spline.fit_tolerance = 1e-10;
    spline.dwg_flags1 = 9;
    spline
}

#[test]
fn literal_dxf_reads_parameterization_independently_of_creation_method() {
    for (flags, parameterization, method) in [
        (32, 0, 0),
        (64, 1, 0),
        (128, 2, 0),
        (256, 15, 0),
        (1056, 0, 1),
        (1088, 1, 1),
        (1152, 2, 1),
        (1280, 15, 1),
        (16512, 2, 0),
    ] {
        let doc = read_dxf(literal_spline(flags));
        let s = first_spline(&doc);
        assert_eq!(s.knot_parameterization, parameterization, "flags {flags}");
        assert_eq!(s.dwg_flags1 & 1, method, "flag 32 is Chord, not fit method");
        assert_eq!(s.dxf_flags, flags);
    }
}

#[test]
fn binary_dxf_preserves_fit_parameterization_and_endpoint_derivatives() {
    for parameterization in [0, 1, 2, 15] {
        let expected = fit_spline(parameterization);
        let mut doc = CadDocument::with_version(DxfVersion::AC1032);
        doc.add_entity(EntityType::Spline(expected.clone()))
            .unwrap();
        let read = read_dxf(DxfWriter::new_binary(&doc).write_to_vec().unwrap());
        let s = first_spline(&read);
        assert_eq!(s.knot_parameterization, parameterization);
        assert_eq!(s.fit_points, expected.fit_points);
        assert_eq!(s.begin_tangent, expected.begin_tangent);
        assert_eq!(s.end_tangent, expected.end_tangent);
    }
}

#[test]
fn literal_dxf_reads_frame_visibility_and_fit_storage_context() {
    let doc = read_dxf(literal_spline(1665)); // closed | Uniform | frame | fit method
    let s = first_spline(&doc);
    assert!(s.cv_frame_visible);
    assert!(s.flags.closed);
    assert_eq!(s.knot_parameterization, 2);
    assert_eq!(s.dwg_flags1 & 15, 15);
}

#[test]
fn dxf_writer_uses_typed_parameterization_instead_of_stale_raw_flags() {
    for (parameterization, flags) in [(0, 1056), (1, 1088), (2, 1152), (15, 1280)] {
        let mut s = fit_spline(parameterization);
        s.dxf_flags = 16384 | 256 | 512; // unrelated high bit plus stale Custom/frame
        s.cv_frame_visible = false;
        let mut doc = CadDocument::with_version(DxfVersion::AC1032);
        doc.add_entity(EntityType::Spline(s)).unwrap();
        let bytes = DxfWriter::new(&doc).write_to_vec().unwrap();
        assert_eq!(written_flags(&bytes, "SPLINE"), 16384 | flags);
    }
}

#[test]
fn constructed_fit_only_spline_writes_fit_method_and_frame_flags() {
    let mut s = fit_spline(2);
    s.dwg_flags1 = 0; // constructed from fit points rather than read from DWG
    s.cv_frame_visible = true;
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.add_entity(EntityType::Spline(s)).unwrap();
    let bytes = DxfWriter::new(&doc).write_to_vec().unwrap();
    assert_eq!(written_flags(&bytes, "SPLINE"), 1664);
    let read = read_dxf(bytes);
    assert!(first_spline(&read).cv_frame_visible);
}

#[test]
fn chord_flag_does_not_turn_a_control_spline_into_fit_method() {
    let mut s = Spline::from_control_points(
        2,
        vec![
            Vector3::ZERO,
            Vector3::new(1.0, 2.0, 0.0),
            Vector3::new(3.0, 0.0, 0.0),
        ],
    );
    s.dxf_flags = 32;
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.add_entity(EntityType::Spline(s)).unwrap();
    let read = read_dxf(DxfWriter::new(&doc).write_to_vec().unwrap());
    assert_eq!(first_spline(&read).dwg_flags1 & 1, 0);
}

#[test]
fn uniform_and_centripetal_fit_splines_survive_dwg_dxf_dwg() {
    for (parameterization, flags) in [(0, 1056), (1, 1088), (2, 1152)] {
        for target in [DxfVersion::AC1015, DxfVersion::AC1027, DxfVersion::AC1032] {
            let expected = fit_spline(parameterization);
            let mut doc = CadDocument::with_version(DxfVersion::AC1032);
            doc.add_entity(EntityType::Spline(expected.clone()))
                .unwrap();
            let mut dwg =
                DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&doc).unwrap()))
                    .read()
                    .unwrap();
            assert_eq!(first_spline(&dwg).knot_parameterization, parameterization);
            dwg.version = target;
            let bytes = DxfWriter::new(&dwg).write_to_vec().unwrap();
            assert_eq!(written_flags(&bytes, "SPLINE"), flags, "{target:?}");
            let mut dxf = read_dxf(bytes);
            let s = first_spline(&dxf);
            assert_eq!(s.knot_parameterization, parameterization, "{target:?}");
            assert_eq!(s.fit_points, expected.fit_points);
            assert_eq!(s.begin_tangent, expected.begin_tangent);
            assert_eq!(s.end_tangent, expected.end_tangent);
            assert_eq!(s.fit_tolerance, expected.fit_tolerance);
            dxf.version = DxfVersion::AC1032;
            let reread =
                DwgReader::from_stream(Cursor::new(DwgWriter::write_to_vec(&dxf).unwrap()))
                    .read()
                    .unwrap();
            let s = first_spline(&reread);
            assert_eq!(s.knot_parameterization, parameterization);
            assert_eq!(s.fit_points, expected.fit_points);
            assert_eq!(s.begin_tangent, expected.begin_tangent);
            assert_eq!(s.end_tangent, expected.end_tangent);
        }
    }
}

#[test]
fn helix_embedded_spline_uses_the_same_extended_flag_mapping() {
    let mut helix = Helix::new();
    helix.spline = fit_spline(2);
    helix.spline.cv_frame_visible = true;
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.add_entity(EntityType::Helix(Box::new(helix))).unwrap();
    let bytes = DxfWriter::new(&doc).write_to_vec().unwrap();
    assert_eq!(written_flags(&bytes, "HELIX"), 1664);
    let read = read_dxf(bytes);
    let spline = read
        .entities()
        .find_map(|e| match e {
            EntityType::Helix(h) => Some(&h.spline),
            _ => None,
        })
        .unwrap();
    assert_eq!(spline.knot_parameterization, 2);
    assert!(spline.cv_frame_visible);
}

/// A control-point spline read from DWG carries a parameterization (Custom),
/// but its DXF flags keep only the curve bits, as other writers do (#149).
#[test]
fn control_point_spline_from_dwg_writes_no_parameterization_bits() {
    let mut spline = Spline::new();
    spline.degree = 2;
    spline.control_points = vec![
        Vector3::ZERO,
        Vector3::new(1.0, 1.0, 0.0),
        Vector3::new(2.0, 0.0, 0.0),
    ];
    spline.knots = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0];
    spline.flags.closed = true;
    spline.flags.periodic = true;
    spline.knot_parameterization = 15;
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    doc.add_entity(EntityType::Spline(spline)).unwrap();
    let bytes = DxfWriter::new(&doc).write_to_vec().unwrap();
    assert_eq!(written_flags(&bytes, "SPLINE"), 3);
}
