//! Pre-7.0 SAT written as SAB is upgraded to the ACIS 7.0 record layout
//! (issue #348).

use acadrust::entities::acis::primitives::{build_box, build_cylinder, build_torus};
use acadrust::entities::acis::{
    upgrade_records_from_v400, SabReader, SabWriter, SatDocument, SatEdge, SatLump, SatShell,
    SatVersion,
};
use acadrust::entities::Solid3D;
use acadrust::{CadDocument, DwgReader, DwgWriter, DxfVersion, EntityType};
use std::io::Cursor;

/// A 10 x 10 square region in SAT 4.0: no `@` string lengths, no entity ids,
/// no pattern pointers and no edge parameters. The third edge runs against
/// its curve and the fourth curve has a direction of length 2.
const V400_SQUARE: &str = "400 26 1 0
4 test 8 ACIS 4.0 24 Thu Jan 01 00:00:00 2026
1 9.9999999999999995e-07 1e-10
body $-1 $1 $-1 $-1 #
lump $-1 $-1 $2 $0 #
shell $-1 $-1 $-1 $3 $-1 $1 #
face $-1 $-1 $4 $2 $-1 $5 forward double out #
loop $-1 $-1 $6 $3 #
plane-surface $-1 5 5 0 0 0 1 1 0 0 forward_v I I I I #
coedge $-1 $7 $9 $-1 $10 forward $4 $-1 #
coedge $-1 $8 $6 $-1 $11 forward $4 $-1 #
coedge $-1 $9 $7 $-1 $12 forward $4 $-1 #
coedge $-1 $6 $8 $-1 $13 forward $4 $-1 #
edge $-1 $14 $15 $6 $18 forward #
edge $-1 $15 $16 $7 $19 forward #
edge $-1 $16 $17 $8 $20 reversed #
edge $-1 $17 $14 $9 $21 forward #
vertex $-1 $10 $22 #
vertex $-1 $10 $23 #
vertex $-1 $11 $24 #
vertex $-1 $12 $25 #
straight-curve $-1 5 0 0 1 0 0 I I #
straight-curve $-1 10 0 0 0 1 0 I I #
straight-curve $-1 0 10 0 1 0 0 I I #
straight-curve $-1 0 10 0 0 -2 0 I I #
point $-1 0 0 0 #
point $-1 10 0 0 #
point $-1 10 10 0 #
point $-1 0 10 0 #
End-of-ACIS-data
";

fn edge_params(doc: &SatDocument) -> Vec<(f64, f64)> {
    doc.records
        .iter()
        .filter_map(SatEdge::from_record)
        .map(|edge| (edge.start_param(), edge.end_param()))
        .collect()
}

#[test]
fn v400_edges_get_their_vertex_parameters() {
    let sab = SabWriter::write(&SatDocument::parse(V400_SQUARE).unwrap());
    let doc = SabReader::read(&sab).unwrap();
    assert_eq!(doc.header.version, SatVersion::V7_0);
    assert_eq!(doc.records.len(), 26);
    // (vertex - root) . direction / |direction|^2, negated for the reversed edge.
    assert_eq!(
        edge_params(&doc),
        [(-5.0, 5.0), (0.0, 10.0), (-10.0, 0.0), (0.0, 5.0)]
    );
    let edge = doc.records.iter().find(|r| r.entity_type == "edge").unwrap();
    assert_eq!(edge.tokens.last().and_then(|t| t.as_string()), Some("unknown"));
}

#[test]
fn v400_sat_upgrades_to_the_records_it_was_saved_from() {
    for original in [
        build_box([1.0, 2.0, 3.0], 4.0, 5.0, 6.0),
        build_cylinder([1.0, 2.0, 3.0], 2.0, 5.0),
        build_torus([0.0, 0.0, 0.0], 5.0, 1.0),
    ] {
        let mut legacy = original.clone();
        legacy.header.version = SatVersion::V4_0;
        let text = legacy.to_sat_string();
        assert!(text.starts_with("400 ") && !text.contains('@'));
        let mut upgraded = SatDocument::parse(&text).unwrap();
        upgrade_records_from_v400(&mut upgraded);
        assert_eq!(upgraded.to_sat_string(), original.to_sat_string());
    }
}

#[test]
fn v400_lump_and_shell_links_keep_their_meaning() {
    let doc = SatDocument::parse(
        "400 6 1 0
4 test 8 ACIS 4.0 24 Thu Jan 01 00:00:00 2026
1 9.9999999999999995e-07 1e-10
body $-1 $1 $-1 $-1 #
lump $-1 $2 $3 $0 #
lump $-1 $-1 $4 $0 #
shell $-1 $4 $-1 $-1 $-1 $1 #
shell $-1 $-1 $-1 $-1 $-1 $2 #
End-of-ACIS-data
",
    )
    .unwrap();
    let lump = SatLump::from_record(&doc.records[1]).unwrap();
    assert_eq!((lump.next_lump().0, lump.shell().0), (2, 3));
    let shell = SatShell::from_record(&doc.records[3]).unwrap();
    assert_eq!((shell.next_shell().0, shell.lump().0), (4, 1));
}

#[test]
fn dwg_writer_stores_v400_solids_as_v700_sab() {
    let original = build_box([0.0, 0.0, 0.0], 10.0, 10.0, 10.0);
    let mut legacy = original.clone();
    legacy.header.version = SatVersion::V4_0;
    let sat = legacy.to_sat_string();
    for version in [DxfVersion::AC1018, DxfVersion::AC1024, DxfVersion::AC1032] {
        let mut drawing = CadDocument::with_version(version);
        drawing
            .add_entity(EntityType::Solid3D(Solid3D::from_sat(&sat)))
            .unwrap();
        let bytes = DwgWriter::write_to_vec(&drawing).unwrap();
        let read = DwgReader::from_stream(Cursor::new(bytes)).read().unwrap();
        let Some(EntityType::Solid3D(solid)) = read.entities().next() else {
            panic!("missing solid");
        };
        let doc = SabReader::read(&solid.acis_data.sab_data).unwrap();
        assert_eq!(doc.header.version, SatVersion::V7_0, "{version:?}");
        assert_eq!(doc.records.len(), original.records.len(), "{version:?}");
        assert_eq!(doc.faces().len(), 6, "{version:?}");
        assert_eq!(edge_params(&doc), edge_params(&original), "{version:?}");
    }
}
