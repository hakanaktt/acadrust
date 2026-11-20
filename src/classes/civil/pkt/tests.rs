use super::*;

fn common(id: i32, name: &str) -> PktCommon {
    PktCommon { activity_id: id, display_name: name.into(), show_errors: true, hint_size: Some("200,22.4".into()), view: None }
}

fn point(id: i32, name: &str, from: Option<&str>, link: Option<&str>, positioning: &str, args: &[(&str, &str, &str)], codes: &[&str], link_codes: &[&str]) -> PktActivity {
    let list = |c: &[&str]| (!c.is_empty()).then(|| c.iter().map(|s| s.to_string()).collect());
    PktActivity::Point(PktPoint {
        auxiliary: false,
        common: common(id, &match link {
            Some(l) => format!("{name}&{l}"),
            None => name.into(),
        }),
        name: name.into(),
        positioning: positioning.into(),
        from_point: from.map(Into::into),
        auto_link: link.is_some(),
        link_name: link.map(Into::into),
        apply_aor: false,
        arguments: args.iter().map(|(t, k, v)| PktArgument::new(t, k, v)).collect(),
        point_codes: list(codes),
        link_codes: list(link_codes),
    })
}

/// A lane: P1 at the origin, P2 at (width, slope), P3 `depth` below P2,
/// P4 back at the origin's offset, links L1-L4 and one shape.
pub(crate) fn lane() -> PktPackage {
    let mut p = PktPackage::new("Lane", "0123456789abcdef0123456789abcdef");
    p.creator = PktCreator { product: "Package Editor".into(), version: "Test".into(), version_number: "1.0.0.0".into() };
    p.namespaces = [("", "urn:activities"), ("asa1", "urn:library"), ("asw", "urn:engine"), ("av", "urn:presentation"), ("mc", "urn:compatibility"), ("mva", "urn:vb"), ("sads", "urn:debugger"), ("sap", "urn:designer"), ("scg", "urn:collections"), ("this", "clr-namespace:"), ("x", "urn:xaml")]
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    let arg = |n: &str, t: &str, d: &str, desc: &str| PktParameter { name: n.into(), direction: PktDirection::In, type_name: t.into(), default: Some(d.into()), attributes: vec![PktAttribute::Enabled(true), PktAttribute::DisplayName(n.into()), PktAttribute::Description(desc.into())] };
    p.parameters.push(PktParameter { name: "Side".into(), direction: PktDirection::In, type_name: "asw:EnumType".into(), default: Some("[new EnumType(0, \"Right\", \"Side\")]".into()), attributes: vec![PktAttribute::Enabled(true)] });
    p.parameters.extend([arg("Width", "x:Double", "3.5", "Lane width"), arg("Depth", "x:Double", "0.3", "Pavement depth"), arg("Slope", "asw:Grade", "[new Grade(-0.02)]", "Lane cross slope")]);
    let d = "x:Double";
    fn slope<'a>(d: &'a str, dx: &'a str) -> [(&'a str, &'a str, &'a str); 2] { [(d, "Slope2", "[Slope]"), (d, "DeltaX2", dx)] }
    let mut acts = vec![
        point(1, "P1", None, None, "DeltaXAndDeltaY", &[(d, "DeltaX1", "0"), (d, "DeltaY1", "0")], &["Crown"], &[]),
        point(2, "P2", Some("P1"), Some("L1"), "SlopeAndDeltaX", &slope(d, "[Width]"), &["ETW"], &["Top", "Pave"]),
        point(3, "P3", Some("P2"), Some("L2"), "DeltaXAndDeltaY", &[(d, "DeltaX1", "0"), (d, "DeltaY1", "[-Depth]")], &["ETW_Datum"], &[]),
        point(4, "P4", Some("P3"), Some("L3"), "SlopeAndDeltaX", &slope(d, "[-Width]"), &["Crown_Datum"], &["Datum"]),
        PktActivity::Link(PktLink { auxiliary: false, common: common(5, "L4"), name: "L4".into(), start: "P4".into(), end: "P1".into(), apply_aor: false, enabled: true, codes: None }),
        PktActivity::Shape(PktShape { common: common(6, "S1"), name: "S1".into(), links: ["L1", "L2", "L3", "L4"].map(String::from).to_vec(), codes: Some(vec!["Pave".into()]) }),
    ];
    for a in &mut acts {
        if let PktActivity::Point(x) = a {
            if x.positioning == "SlopeAndDeltaX" {
                for k in ["OffsetTarget2", "ElevationTarget2", "Superelevation2"] {
                    let t = if k.starts_with("Offset") { "asw:OffsetTarget" } else if k.starts_with("Elev") { "asw:ElevationTarget" } else { "asw:CrossSegmentType" };
                    x.arguments.push(PktArgument { direction: PktDirection::In, type_name: t.into(), key: Some(k.into()), value: None, preserve_space: false });
                }
            }
        }
    }
    let n = acts.len();
    p.flowchart = acts
        .into_iter()
        .enumerate()
        .map(|(i, a)| PktNode::Step(PktStep { reference: format!("__ReferenceID{i}"), view: None, action: a, next: (i + 1 < n).then_some(i + 1) }))
        .collect();
    p.start = Some(0);
    p
}

#[test]
fn packages_round_trip() {
    let mut p = lane();
    p.description = "A lane".into();
    p.preview_png_name = "Lane.png".into();
    p.preview_png = Some(vec![0x89, b'P', b'N', b'G', 1, 2, 3]);
    p.help_name = "Lane.html".into();
    p.help = Some(b"<html><body>help</body></html>".to_vec());
    // a decision, a switch, a sequence and a variable
    let aux = |id: i32, name: &str| {
        let mut a = point(id, name, Some("P2"), None, "DeltaXAndDeltaY", &[("x:Double", "DeltaX1", "[Width/2]"), ("x:Double", "DeltaY1", "0")], &[], &[]);
        if let PktActivity::Point(x) = &mut a {
            x.auxiliary = true;
        }
        a
    };
    let last = p.flowchart.len() - 1;
    let k = p.flowchart.len();
    if let PktNode::Step(s) = &mut p.flowchart[last] {
        s.next = Some(k);
    }
    p.flowchart.push(PktNode::Decision(PktDecision { reference: "__ReferenceID9".into(), condition: "[Slope < 0]".into(), display_name: None, hint_size: Some("70.4,87.2".into()), view: None, on_true: Some(k + 1), on_false: None }));
    p.flowchart.push(PktNode::Switch(PktSwitch { reference: "__ReferenceID10".into(), type_name: "x:String".into(), expression: "[Side.Value]".into(), display_name: None, hint_size: None, view: None, default: Some(k + 2), cases: vec![("Right".into(), k + 3)] }));
    p.flowchart.push(PktNode::Step(PktStep { reference: "__ReferenceID11".into(), view: None, action: aux(7, "AP1"), next: None }));
    p.flowchart.push(PktNode::Step(PktStep {
        reference: "__ReferenceID12".into(),
        view: Some(PktView(vec![PktViewEntry { key: "ShapeLocation".into(), value: PktViewValue::Point("50,220".into()) }])),
        action: PktActivity::Sequence(PktSequence { display_name: Some("Right side".into()), hint_size: None, view: None, activities: vec![aux(8, "AP2")] }),
        next: Some(k + 2),
    }));
    p.parameters.push(PktParameter { name: "Kind".into(), direction: PktDirection::In, type_name: "asw:EnumType".into(), default: Some("[new EnumType(301, \"A\", \"Kinds\")]".into()), attributes: vec![] });
    p.enum_groups.push(PktEnumGroup { name: "Kinds".into(), items: vec![("A".into(), "A".into(), 301), ("B".into(), "B".into(), 302)] });
    p.defined_variables.push("Used".into());
    p.variables.push(PktVariable { type_name: "x:Double".into(), name: "Used".into(), default: None, read_only: false });
    let bytes = write_pkt(&p).unwrap();
    let back = read_pkt(&bytes).unwrap();
    assert_eq!(back, p);
    assert!(same_parts(&bytes, &write_pkt(&back).unwrap()).unwrap());
    // the catalog lists the inputs with their choices
    let atc = String::from_utf8(zip::read(&bytes).unwrap().into_iter().find(|x| x.0.ends_with(".atc")).unwrap().1).unwrap();
    assert!(atc.contains(r#"<Side DataType="long" TypeInfo="16" DisplayName="Side" Description="Side">0<Enum><Left DisplayName="Left">1</Left><Right DisplayName="Right">0</Right></Enum></Side>"#));
    assert!(atc.contains(r#"<Slope DataType="double" TypeInfo="9" DisplayName="Slope" Description="Lane cross slope">-0.02</Slope>"#));
    assert!(atc.contains("<ToolTip>A lane\\nVersion: 0.1</ToolTip>"));
}

#[test]
fn foreign_parts_are_refused() {
    let p = lane();
    let mut parts = parts(&p).unwrap();
    parts.push(("extra.bin".into(), vec![1]));
    assert!(read_pkt(&zip::write(&parts).unwrap()).is_err());
    let mut q = lane();
    q.part = "short".into();
    assert!(write_pkt(&q).is_err());
}

#[test]
fn part_names_are_stable_version_4_guids() {
    let a = new_part_name("Lane");
    assert_eq!(a, new_part_name("Lane"));
    assert_ne!(a, new_part_name("Lane2"));
    assert!(a.len() == 32 && a.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    assert_eq!(&a[12..13], "4");
    assert!("89ab".contains(&a[16..17]));
    let p = PktPackage::new("Lane", &a);
    assert_eq!(p.tool_id.len(), 38);
}

/// The lane of `lane()`, built with the builders.
#[test]
fn builders_make_the_lane() {
    let mut p = PktPackage::new("Lane", "0123456789abcdef0123456789abcdef");
    let q = lane();
    p.creator = q.creator.clone();
    p.namespaces = q.namespaces.clone();
    p.parameters = vec![parameter_side(PktSide::Right), parameter_double("Width", 3.5, "Lane width"), parameter_double("Depth", 0.3, "Pavement depth"), parameter_grade("Slope", -0.02, "Lane cross slope")];
    let none = PointTargets::default();
    let (flowchart, start) = chain(vec![
        point_delta(1, "P1", None, None, "0", "0", &["Crown"]),
        point_slope(2, "P2", "P1", Some(("L1", &["Top", "Pave"])), "[Slope]", "[Width]", none, &["ETW"]),
        point_delta(3, "P3", Some("P2"), Some(("L2", &[])), "0", "[-Depth]", &["ETW_Datum"]),
        point_slope(4, "P4", "P3", Some(("L3", &["Datum"])), "[Slope]", "[-Width]", none, &["Crown_Datum"]),
        link(5, "L4", "P4", "P1", &[]),
        shape(6, "S1", &["L1", "L2", "L3", "L4"], &["Pave"]),
    ]);
    p.flowchart = flowchart;
    p.start = start;
    assert_eq!(p, q);
    let back = read_pkt(&write_pkt(&p).unwrap()).unwrap();
    assert_eq!(back, p);
    // a target and a slope parameter survive the round trip
    let mut t = p.clone();
    t.parameters.push(parameter_slope("Fill", 4.0, "Fill slope"));
    t.parameters.push(PktParameter { name: "WidthTarget".into(), direction: PktDirection::In, type_name: "asw:OffsetTarget".into(), default: None, attributes: vec![PktAttribute::Enabled(true)] });
    if let PktNode::Step(s) = &mut t.flowchart[1] {
        s.action = point_slope(2, "P2", "P1", Some(("L1", &["Top", "Pave"])), "[Slope]", "[Width]", PointTargets { offset: Some("[WidthTarget]"), ..none }, &["ETW"]);
    }
    let back = read_pkt(&write_pkt(&t).unwrap()).unwrap();
    assert_eq!(back, t);
}

#[test]
fn new_packages_carry_the_editors_settings() {
    let p = PktPackage::new_package("Lane", &new_part_name("Lane"));
    assert_eq!(p.creator.product, EDITOR_PRODUCT);
    assert_eq!(p.namespaces.len(), 13);
    assert_eq!(p.namespaces.iter().filter(|n| n.1.starts_with("clr-namespace:")).count(), 7);
    let mut q = p.clone();
    q.parameters = vec![parameter_side(PktSide::Right), parameter_double("Width", 3.5, "Lane width")];
    let (f, s) = chain(vec![point_delta(1, "P1", None, None, "0", "0", &["Crown"]), point_delta(2, "P2", Some("P1"), Some(("L1", &["Top"])), "[Width]", "0", &["ETW"])]);
    q.flowchart = f;
    q.start = s;
    let bytes = write_pkt(&q).unwrap();
    assert_eq!(read_pkt(&bytes).unwrap(), q);
    assert_eq!(bytes, write_pkt(&q).unwrap());
}
