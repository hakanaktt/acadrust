//! Codec-only roundtrip test for corridors written unbuilt (§3i) and for
//! assemblies of package subassemblies (§3l).
//!
//! The corridor is written as the application writes a corridor before
//! its first rebuild: out of date, without stations, applied sections,
//! feature lines or surfaces — asserted as such after the reopen.
//! Rewriting a definition gives the same record field for field; the
//! identifiers (version-4 GUIDs), the geometry stamp and the class word
//! differ by design (§3i), so two corridors from one definition decode
//! equal while their records differ.
//!
//! Documented subset (READY-WITH-LIMITS): offset baselines and
//! boundaries along feature lines have no writer (§3i-a); target entries
//! need a subassembly with targets (stock subassemblies have no writer,
//! §3l) and are out of scope.

use acadrust::classes::arch::ArchEntityData;
use acadrust::classes::civil::pkt::{
    chain, layout, link, new_part_name, parameter_double, parameter_grade, parameter_side,
    point_delta, point_slope, read_pkt, shape, write_pkt, PktPackage, PktSide, PktValue,
    PointTargets,
};
use acadrust::classes::civil::{
    add_alignment_with_curves, add_assembly, add_corridor_with, add_pkt_subassembly,
    add_profile, ensure_civil, CorridorBaseline, CorridorBoundary, CorridorDefinition,
    CorridorRegion, CorridorSurfaceDefinition, CurveStations, ParameterValue, PktSource,
    RegionFrequency, TargetCurveStations,
};
use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::types::{DxfVersion, Handle, Vector2, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn v2(x: f64, y: f64) -> Vector2 {
    Vector2::new(x, y)
}
fn v3(x: f64, y: f64, z: f64) -> Vector3 {
    Vector3::new(x, y, z)
}

fn doc() -> CadDocument {
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1032;
    doc
}

fn roundtrip(doc: &CadDocument, tag: &str) -> CadDocument {
    let path = std::env::temp_dir().join(format!("rt_infra_{tag}_{}.dwg", std::process::id()));
    DwgWriter::write_to_file(&path, doc).expect("DWG write failed");
    let back = DwgReader::from_file(&path)
        .expect("DWG reopen failed")
        .read()
        .expect("DWG read failed");
    let _ = std::fs::remove_file(&path);
    back
}

fn entity(doc: &CadDocument, handle: Handle) -> ArchEntityData {
    let EntityType::Extended(e) = doc
        .get_entity(handle)
        .unwrap_or_else(|| panic!("entity {:#x} missing", handle.value()))
    else {
        panic!("handle {:#x} is not an extended entity", handle.value());
    };
    let ExtendedEntityData::Arch(data) = &e.data else {
        panic!("handle {:#x} is not an Arch entity", handle.value());
    };
    data.clone()
}

/// A lane package: P1 at the baseline, P2 outwards on the slope, P3 the
/// datum below P2, P4 the datum below P1.
fn lane(name: &str) -> PktPackage {
    let mut p = PktPackage::new_package(name, &new_part_name(&format!("{name} lane 1")));
    p.parameters = vec![
        parameter_side(PktSide::Right),
        parameter_double("Width", 3.5, "Lane width"),
        parameter_grade("Slope", -0.02, "Lane cross slope"),
        parameter_double("Depth", 0.3, "Pavement depth"),
    ];
    let none = PointTargets::default();
    let (flow, start) = chain(vec![
        point_delta(1, "P1", None, None, "0", "0", &["Crown"]),
        point_slope(2, "P2", "P1", Some(("L1", &["Top"])), "[Slope]", "[Width]", none, &["ETW"]),
        point_delta(3, "P3", Some("P2"), Some(("L2", &["Top"])), "0", "[-Depth]", &["ETW_Datum"]),
        point_delta(4, "P4", Some("P1"), Some(("L3", &[])), "0", "[-Depth]", &["Crown_Datum"]),
        link(5, "L4", "P4", "P1", &["Datum"]),
        shape(6, "S1", &["L1", "L2", "L3", "L4"], &["Pave"]),
    ]);
    p.flowchart = flow;
    p.start = start;
    p
}

/// A corridor with two baselines, three regions, added stations, two
/// surfaces with codes and boundaries: saved, reopened and read back
/// field for field.
///
/// The regions apply assemblies of package subassemblies: the corridor
/// reader reads an assembly's layout, and an assembly without
/// subassemblies decodes to `None` after the reopen (reader boundary;
/// `add_corridor` itself accepts an empty assembly).
#[test]
fn unbuilt_corridor_definition_survives_save_reopen() {
    let mut doc = doc();
    ensure_civil(&mut doc).unwrap();
    let al1 = add_alignment_with_curves(
        &mut doc,
        "AL1",
        &[v2(0.0, 0.0), v2(150.0, 0.0), v2(300.0, 60.0)],
        &[acadrust::classes::civil::VertexCurve::None],
        None,
    )
    .unwrap();
    let p1 = add_profile(&mut doc, al1, "PF1", &[v2(0.0, 10.0), v2(150.0, 20.0), v2(300.0, 15.0)], &[0.0], None).unwrap();
    let al2 = add_alignment_with_curves(&mut doc, "AL2", &[v2(0.0, 0.0), v2(100.0, 0.0)], &[] as &[_], None).unwrap();
    let p2 = add_profile(&mut doc, al2, "PF2", &[v2(0.0, 5.0), v2(100.0, 8.0)], &[], None).unwrap();
    let asm1 = add_assembly(&mut doc, "ASM_LANE", v3(0.0, -50.0, 0.0), None).unwrap();
    let asm2 = add_assembly(&mut doc, "ASM_TRANS", v3(0.0, -60.0, 0.0), None).unwrap();
    let source = PktSource { library: "[ \"rt_lane.pkt\" ]".into(), path: "rt_lane.pkt".into() };
    add_pkt_subassembly(&mut doc, asm1, "LaneA", &source, &lane("RT-Cor-A"), PktSide::Right, &[], None).unwrap();
    add_pkt_subassembly(&mut doc, asm2, "LaneB", &source, &lane("RT-Cor-B"), PktSide::Right, &[], None).unwrap();

    let frequent = RegionFrequency {
        along_tangents: 10.0,
        along_curves: 12.0,
        along_spirals: 15.0,
        along_profile_curves: 5.0,
        curves: CurveStations::ByCurvature,
        curve_mid_ordinate: 2.0,
        at_horizontal_geometry_points: true,
        at_superelevation_critical_points: false,
        at_profile_geometry_points: false,
        at_profile_high_low_points: true,
        at_offset_target_geometry_points: false,
        adjacent_to_offset_target_start_end: false,
        along_target_curves: 25.0,
        target_curves: TargetCurveStations::None,
        target_curve_mid_ordinate: 0.1,
    };
    let mut along_targets = RegionFrequency::default();
    along_targets.target_curves = TargetCurveStations::ByCurvature;
    along_targets.along_target_curves = 7.0;

    // The writer sorts the added stations and the surface codes and
    // closes the boundary polygon; the wanted definition below does the
    // same normalization.
    let def = CorridorDefinition {
        name: "RT-COR".into(),
        description: "roundtrip corridor".into(),
        rebuild_automatic: true,
        geometry_lock: true,
        maximum_triangle_side: 50.0,
        surfaces: vec![
            CorridorSurfaceDefinition {
                name: "CS1".into(),
                build: true,
                link_codes: vec![("Top".into(), true)],
                feature_line_codes: vec![("Daylight".into(), false)],
                overhang_correction: 1,
            },
            CorridorSurfaceDefinition {
                name: "CS2".into(),
                build: false,
                link_codes: Vec::new(),
                feature_line_codes: Vec::new(),
                overhang_correction: 0,
            },
        ],
        boundaries: vec![
            CorridorBoundary {
                surface: "CS1".into(),
                name: "B1".into(),
                description: "outside".into(),
                kind: 0x4000_0002,
                corners: vec![v3(0.0, 0.0, 0.0), v3(10.0, 0.0, 0.0), v3(10.0, 10.0, 0.0), v3(0.0, 0.0, 0.0)],
                components: 0,
            },
            CorridorBoundary {
                surface: "CS1".into(),
                name: "M1".into(),
                description: String::new(),
                kind: 0x4000_0004,
                corners: vec![v3(1.0, 1.0, 0.0), v3(9.0, 1.0, 0.0), v3(9.0, 9.0, 0.0), v3(1.0, 1.0, 0.0)],
                components: 0,
            },
        ],
        baselines: vec![
            CorridorBaseline {
                name: "BL1".into(),
                feature_line: 0,
                alignment: al1.value(),
                profile: p1.value(),
                regions: vec![
                    CorridorRegion { name: "R1".into(), assembly: asm1.value(), start: 0.0, end: 100.0, frequency: frequent, targets: Vec::new() },
                    CorridorRegion { name: "R2".into(), assembly: asm2.value(), start: 150.0, end: 300.0, frequency: RegionFrequency::default(), targets: Vec::new() },
                ],
                stations: vec![(33.3, "first".into()), (175.0, String::new())],
                built: false,
            },
            CorridorBaseline {
                name: "BL2".into(),
                feature_line: 0,
                alignment: al2.value(),
                profile: p2.value(),
                regions: vec![
                    CorridorRegion { name: "R3".into(), assembly: asm1.value(), start: 0.0, end: 100.0, frequency: along_targets, targets: Vec::new() },
                ],
                stations: Vec::new(),
                built: false,
            },
        ],
    };

    // Two corridors from one definition decode to that definition; a
    // second definition decodes to itself.
    let c1 = add_corridor_with(&mut doc, &def, None).unwrap();
    let c3 = add_corridor_with(&mut doc, &def, None).unwrap();
    let mut def2 = def.clone();
    def2.name = "RT-COR-B".into();
    def2.geometry_lock = false;
    let c2 = add_corridor_with(&mut doc, &def2, None).unwrap();

    let back = roundtrip(&doc, "corridor");

    assert_eq!(entity(&back, c1).corridor_definition(), Some(def.clone()));
    assert_eq!(entity(&back, c3).corridor_definition(), entity(&back, c1).corridor_definition());
    assert_eq!(entity(&back, c2).corridor_definition(), Some(def2));

    // Unbuilt by design: no built stations, no applied sections, no
    // feature line geometry (§3i); `built` stays false.
    for line in &entity(&back, c1).corridor_definition().unwrap().baselines {
        assert!(!line.built);
    }
    assert!(entity(&back, c1).corridor_feature_lines().is_empty());
    assert!(
        entity(&back, c1).corridor_baselines().iter().all(Vec::is_empty),
        "no built station groups per baseline"
    );
}

/// A lane package written with the pkt builders round-trips through
/// write_pkt → read_pkt with equal documents and equal bytes (§3l), lays
/// out as a lane, and an assembly built of it (right lane wide, left lane
/// with the defaults) reads back with its parameters, shape and codes
/// after a save and reopen.
#[test]
fn pkt_package_and_assembly_of_packages_survive() {
    // A simple lane: P1 at the baseline, P2..P4 outwards, links L1-L4,
    // shape S1 (the E-PK7 lane of §3l).
    let pkg = lane("RT-Lane");
    let bytes = write_pkt(&pkg).unwrap();
    // Document equality and byte equality (§3l: each package reads and
    // writes back to equal parts).
    assert_eq!(read_pkt(&bytes).unwrap(), pkg);
    assert_eq!(write_pkt(&read_pkt(&bytes).unwrap()).unwrap(), bytes);

    // The layout runs the flowchart: the lane's four points and its codes.
    let lay = layout(&pkg, PktSide::Right, &[] as &[(String, PktValue)]).unwrap();
    assert_eq!(lay.points.len(), 4);
    assert_eq!(lay.links.len(), 4);
    assert_eq!(lay.shapes.len(), 1);
    assert_eq!((lay.points[0].offset, lay.points[0].elevation), (0.0, 0.0));
    assert_eq!((lay.points[1].offset, lay.points[1].elevation), (3.5, -0.07));
    assert_eq!((lay.points[2].offset, lay.points[2].elevation), (3.5, -0.37));
    assert_eq!((lay.points[3].offset, lay.points[3].elevation), (0.0, -0.3));
    for code in ["Crown", "ETW", "ETW_Datum", "Datum", "Top", "Pave"] {
        assert!(lay.codes.iter().any(|c| c == code), "{code} among the codes");
    }

    // An assembly of the package: a wide lane on the right, the defaults
    // on the left (the E-PK7 assembly of §3l).
    let source = PktSource { library: "[ \"rt_lane.pkt\" ]".into(), path: "rt_lane.pkt".into() };
    let mut doc = doc();
    ensure_civil(&mut doc).unwrap();
    let a = add_assembly(&mut doc, "ASM-LANES", v3(0.0, -50.0, 0.0), None).unwrap();
    let wide = [
        ("Width".to_string(), PktValue::Double(4.0)),
        ("Depth".to_string(), PktValue::Double(0.25)),
        ("Slope".to_string(), PktValue::Double(-0.03)),
    ];
    let right = add_pkt_subassembly(&mut doc, a, "LaneR", &source, &pkg, PktSide::Right, &wide, None).unwrap();
    let left = add_pkt_subassembly(&mut doc, a, "LaneL", &source, &pkg, PktSide::Left, &[], None).unwrap();

    let back = roundtrip(&doc, "pkt-assembly");

    // The assembly keeps its baseline point and both subassemblies hang
    // from it (attachment parent 0, point 0).
    let lay_back = entity(&back, a).assembly_layout().unwrap();
    assert_eq!(lay_back.origin, v3(0.0, -50.0, 0.0));
    let mut subs: Vec<(u64, u64, u32)> = lay_back
        .attachments
        .iter()
        .map(|at| (at.subassembly, at.parent, at.point))
        .collect();
    subs.sort_unstable();
    let mut want = vec![(left.value(), 0, 0), (right.value(), 0, 0)];
    want.sort_unstable();
    assert_eq!(subs, want, "both lanes hang from the assembly's baseline point");

    // The right lane: its assembly, source texts, codes and the stored
    // parameter values (§3l's record: the package version, the MD5, the
    // point, link and shape codes, the parameters).
    let lane_back = entity(&back, right);
    assert_eq!(lane_back.dxf_name, "AECC_SUBASSEMBLY");
    assert_eq!(lane_back.subassembly_assembly(), Some(a.value()));
    let strings = lane_back.decode().strings;
    assert_eq!(&strings[3..5], ["[ \"rt_lane.pkt\" ]", "{\"docs_permanent_id\": \"\"}"]);
    assert_eq!(strings[7], "LaneR");
    assert_eq!(strings[9], "rt_lane.pkt");
    assert_eq!(strings[10], "Subassembly.RT-Lane");
    for code in ["Crown", "ETW", "ETW_Datum", "Datum", "Top", "Pave"] {
        assert!(strings.iter().any(|s| s == code), "{code} among the texts");
    }
    let params: Vec<(String, ParameterValue)> = lane_back
        .subassembly_parameters()
        .unwrap()
        .into_iter()
        .map(|p| (p.name, p.value))
        .collect();
    assert!(params.contains(&("Side".to_string(), ParameterValue::Integer(0))), "right side");
    assert_eq!(params.iter().find(|(n, _)| n == "Width").unwrap().1, ParameterValue::Double(4.0));
    assert_eq!(params.iter().find(|(n, _)| n == "Depth").unwrap().1, ParameterValue::Double(0.25));
    assert_eq!(params.iter().find(|(n, _)| n == "Slope").unwrap().1, ParameterValue::Double(-0.03));

    // The stored shape: the wide lane's four points and its four links
    // bounding one area (§3l, E-PK7's record).
    let shape = lane_back.subassembly_shape().unwrap();
    assert_eq!(
        shape.points.iter().map(|p| (p.x, p.y)).collect::<Vec<_>>(),
        [(0.0, 0.0), (4.0, -0.12), (4.0, -0.37), (0.0, -0.25)]
    );
    assert_eq!(shape.links.len(), 4);
    assert_eq!(shape.areas, vec![vec![0, 1, 2, 3]]);
}
