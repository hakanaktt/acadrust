//! A polyline-flagged hatch path that holds no vertices (a DWG can store one)
//! must still write its 72 / 73 / 93 header in DXF. Without it the next code
//! after 92 is 97, which a strict DXF reader refuses.
use acadrust::entities::hatch::{BoundaryPath, BoundaryPathFlags};
use acadrust::entities::{EntityType, Hatch};
use acadrust::types::DxfVersion;
use acadrust::{CadDocument, DxfWriter};

/// Group code / value pairs of the first HATCH entity.
fn hatch_pairs(dxf: &str) -> Vec<(String, String)> {
    let lines: Vec<&str> = dxf.lines().map(str::trim).collect();
    let pairs: Vec<(String, String)> = lines
        .chunks(2)
        .filter(|c| c.len() == 2)
        .map(|c| (c[0].to_string(), c[1].to_string()))
        .collect();
    let start = pairs
        .iter()
        .position(|(c, v)| c == "0" && v == "HATCH")
        .expect("HATCH");
    let end = pairs[start + 1..]
        .iter()
        .position(|(c, _)| c == "0")
        .map_or(pairs.len(), |i| start + 1 + i);
    pairs[start..end].to_vec()
}

#[test]
fn empty_polyline_path_keeps_its_header() {
    let mut doc = CadDocument::with_version(DxfVersion::AC1032);
    let mut hatch = Hatch::solid();
    hatch.add_path(BoundaryPath::with_flags(
        BoundaryPathFlags::from_bits(3), // external + polyline
    ));
    doc.add_entity(EntityType::Hatch(Box::new(hatch))).unwrap();

    let bytes = DxfWriter::new(&doc).write_to_vec().unwrap();
    let pairs = hatch_pairs(&String::from_utf8_lossy(&bytes));
    let at = pairs.iter().position(|(c, _)| c == "92").expect("92");
    let next: Vec<(&str, &str)> = pairs[at + 1..at + 5]
        .iter()
        .map(|(c, v)| (c.as_str(), v.as_str()))
        .collect();
    assert_eq!(next, [("72", "0"), ("73", "0"), ("93", "0"), ("97", "0")]);
}
