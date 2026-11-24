//! Codec-only roundtrip tests for the pipe network cluster: gravity
//! networks (network, structures, pipes of a family, connections) and
//! pressure networks (parts, ports, connections, arc pipes).
//!
//! Each test writes with the create-module writers, saves the DWG to a
//! scratch file, reopens it with `DwgReader::from_file` and asserts the
//! fields docs/civil-write.md documents (§3m, §3n).
//!
//! Documented subset (READY-WITH-LIMITS, §3m):
//! - the catalog body solids of gravity parts are not written (a box or
//!   cylinder of the size's extents is), so nothing asserts them;
//! - a rotated structure's cached plan outline is not decoded and the
//!   writer keeps the unrotated outline (§3m), so only the insertion
//!   point and the rotation itself are asserted for a turned structure.

use acadrust::classes::arch::ArchEntityData;
use acadrust::classes::civil::{
    add_pipe, add_pipe_network, add_pipe_of_family, add_pressure_appurtenance,
    add_pressure_arc_pipe, add_pressure_cross, add_pressure_fitting, add_pressure_network,
    add_pressure_pipe, add_pressure_pipe_of_size, add_pressure_tee,
    add_structure, add_structure_of_family, add_structure_of_size, connect_pipe, connect_pressure_parts, ensure_civil,
    network_part_families, network_structure_size_fields, pressure_part_descriptions, PressurePipe,
    StructureSizeChoice,
};
use acadrust::entities::{EntityType, ExtendedEntityData};
use acadrust::objects::ObjectType;
use acadrust::types::{DxfVersion, Handle, Vector3};
use acadrust::{CadDocument, DwgReader, DwgWriter};

fn v(x: f64, y: f64, z: f64) -> Vector3 {
    Vector3::new(x, y, z)
}

fn doc() -> CadDocument {
    let mut doc = CadDocument::new();
    doc.version = DxfVersion::AC1032;
    doc
}

/// Saves `doc` to a unique scratch file, reopens it and deletes the file.
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

/// Every Arch entity of one DXF class name, with its handle.
fn entities_of(doc: &CadDocument, dxf_name: &str) -> Vec<(Handle, ArchEntityData)> {
    doc.entities()
        .filter_map(|e| {
            let EntityType::Extended(e) = e else { return None };
            let ExtendedEntityData::Arch(data) = &e.data else { return None };
            (data.dxf_name == dxf_name).then(|| (e.common.handle, data.clone()))
        })
        .collect()
}

/// The Arch entity data of the entity at `handle`.
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

/// Reference handle values of an entity's class body, in stream order.
fn refs(data: &ArchEntityData) -> Vec<u64> {
    data.typed_handles().iter().map(|r| r.0).collect()
}

/// The gravity network roundtrip: a network with structures and pipes of
/// two families, one pipe connected to both structures and one connected
/// at its end only (§3m's E-N50/E-N51 shapes).
#[test]
fn gravity_network_survives_save_reopen_with_connections() {
    let mut doc = doc();
    ensure_civil(&mut doc).unwrap();

    let net = add_pipe_network(&mut doc, "RT-NET", None).unwrap();
    let s1 = add_structure(&mut doc, net, v(10.0, 20.0, 3.0), 0.0).unwrap();
    let s2 = add_structure(&mut doc, net, v(100.0, 20.0, 2.0), 0.0).unwrap();
    let p1 = add_pipe(&mut doc, net, v(10.0, 20.0, -2.0), v(100.0, 20.0, -3.0)).unwrap();
    let p2 = add_pipe(&mut doc, net, v(100.0, 20.0, -3.0), v(150.0, 20.0, -4.0)).unwrap();
    connect_pipe(&mut doc, p1, Some(s1), Some(s2)).unwrap();
    connect_pipe(&mut doc, p2, None, Some(s2)).unwrap();
    // Other families of the default parts list (§3m "Other families"):
    // the null structure plus 21 structure and 11 pipe families, one
    // size each.
    let families = network_part_families();
    assert_eq!(families.len(), 1 + 21 + 11);
    let headwall = *families
        .iter()
        .find(|f| f.contains("Headwall"))
        .expect("a headwall family in the default parts list");
    let s3 = add_structure_of_family(&mut doc, net, headwall, v(200.0, 0.0, -6.0), 0.75).unwrap();
    let p3 = add_pipe_of_family(&mut doc, net, "Concrete Pipe", v(0.0, 0.0, -5.0), v(90.0, 0.0, -6.0)).unwrap();

    let back = roundtrip(&doc, "gravity-network");

    // The network lists its pipes then its structures (the record's last
    // references, §3m).
    let net_refs = refs(&entity(&back, net));
    assert_eq!(entity(&back, net).dxf_name, "AECC_NETWORK");
    assert_eq!(&net_refs[net_refs.len() - 6..net_refs.len() - 3], &[p1.value(), p2.value(), p3.value()]);
    assert_eq!(&net_refs[net_refs.len() - 3..], &[s1.value(), s2.value(), s3.value()]);

    // Structures: insertion points and rotations as written.
    let st1 = entity(&back, s1);
    assert_eq!(st1.dxf_name, "AECC_STRUCTURE");
    assert_eq!(st1.structure_position(), Some(v(10.0, 20.0, 3.0)));
    assert_eq!(st1.network_part_rotation(), Some(0.0));
    assert_eq!(entity(&back, s2).structure_position(), Some(v(100.0, 20.0, 2.0)));
    assert_eq!(entity(&back, s3).structure_position(), Some(v(200.0, 0.0, -6.0)));
    assert_eq!(entity(&back, s3).network_part_rotation(), Some(0.75));

    // Pipes: ends as written, part definition and network among the refs.
    let pipe1 = entity(&back, p1);
    assert_eq!(pipe1.dxf_name, "AECC_PIPE");
    assert_eq!(pipe1.pipe_ends(), Some((v(10.0, 20.0, -2.0), v(100.0, 20.0, -3.0))));
    assert_eq!(entity(&back, p2).pipe_ends(), Some((v(100.0, 20.0, -3.0), v(150.0, 20.0, -4.0))));
    assert_eq!(entity(&back, p3).pipe_ends(), Some((v(0.0, 0.0, -5.0), v(90.0, 0.0, -6.0))));
    let pr = refs(&pipe1);
    // hard 0, hard 0, soft 0, hard owner 0, part definition, style, two
    // soft 0, network, ... (§3m; the indexes of the in-module writer).
    assert!(pr[4] != 0 && pr[5] != 0, "part definition and style written");
    assert_eq!(pr[8], net.value(), "the pipe names its network");

    // Connections are recorded on both sides: a two-way connected pipe
    // carries two connectors, a structure one per pipe in connection
    // order; the end-only pipe's start is open (§3m). Connectors pair
    // through their third reference (soft 0, hard owner 0, the paired
    // connector, the part).
    let paired = |doc: &CadDocument, connector: Handle| refs(&entity(doc, connector))[2];
    let p1_connectors = connectors_of(&back, p1);
    assert_eq!(p1_connectors.len(), 2, "start and end connector");
    let s1_connectors = connectors_of(&back, s1);
    assert_eq!(s1_connectors.len(), 1, "one pipe meets the structure");
    assert_eq!(
        paired(&back, s1_connectors[0]),
        p1_connectors[0].value(),
        "the structure's connector is paired with the pipe's start"
    );
    assert_eq!(
        paired(&back, p1_connectors[0]),
        s1_connectors[0].value(),
        "the pairing is recorded on both sides"
    );
    let s2_connectors = connectors_of(&back, s2);
    assert_eq!(s2_connectors.len(), 2, "two pipes meet the second structure");
    assert_eq!(paired(&back, s2_connectors[0]), p1_connectors[1].value());
    assert_eq!(
        paired(&back, s2_connectors[1]),
        connectors_of(&back, p2)[1].value()
    );
    // The parts the connectors belong to.
    assert_eq!(connector_part(&back, p1_connectors[0]), p1.value());
    assert_eq!(connector_part(&back, s1_connectors[0]), s1.value());
    // The end-only pipe: two connectors, its start's partner is 0.
    let p2_connectors = connectors_of(&back, p2);
    assert_eq!(p2_connectors.len(), 2);
    assert_eq!(paired(&back, p2_connectors[0]), 0, "open start");
    assert_eq!(paired(&back, p2_connectors[1]), s2_connectors[1].value(), "connected end");

    // One part definition per distinct size, keyed by family and size.
    let defs = back
        .named_objects_dictionary()
        .unwrap()
        .get("AECC_NETWORK_PART_DEFS")
        .expect("the part definitions dictionary");
    let ObjectType::Dictionary(d) = back.objects.get(&defs).unwrap() else {
        panic!("AECC_NETWORK_PART_DEFS is not a dictionary");
    };
    assert!(d.len() >= 3, "a definition per distinct size: {}", d.len());

    // The parts keep their names (the record's identifier text).
    assert!(entity(&back, s1).decode().strings[6].starts_with("Structure - ("));
    assert!(entity(&back, p1).decode().strings[6].starts_with("Pipe - ("));
}

/// The pressure network roundtrip: straight and curved pipes, an elbow, a
/// tee, a cross and a valve, ports connected port to port (§3n). The
/// audit table calls positions, ports, diameters and deflections
/// bit-exact, so they are compared with `==`.
#[test]
fn pressure_network_ports_and_connections_survive() {
    let mut doc = doc();
    ensure_civil(&mut doc).unwrap();

    let net = add_pressure_network(&mut doc, "RT-PRES", None).unwrap();
    let straight = add_pressure_pipe(&mut doc, net, v(0.0, 0.0, 10.0), v(100.0, 0.0, 9.0)).unwrap();
    // The sample's curved pipe: bulge -0.4 flat, start tangent
    // (-0.7241..., -0.6897...) (§3n, sample P08).
    let arc = add_pressure_arc_pipe(&mut doc, net, v(0.0, 0.0, 10.0), v(100.0, 0.0, 10.0), -0.4).unwrap();
    let elbow = add_pressure_fitting(&mut doc, net, v(200.0, 0.0, 8.0), 0.5).unwrap();
    let tee = add_pressure_tee(&mut doc, net, v(250.0, 0.0, 8.0), 0.0).unwrap();
    let cross = add_pressure_cross(&mut doc, net, v(300.0, 0.0, 8.0), 0.0).unwrap();
    let valve = add_pressure_appurtenance(&mut doc, net, v(350.0, 0.0, 8.0), 0.0).unwrap();
    // A pipe of a named catalog size, straight.
    let sized = add_pressure_pipe_of_size(
        &mut doc,
        net,
        &pressure_part_descriptions()[0],
        v(400.0, 0.0, 8.0),
        v(500.0, 0.0, 8.0),
        0.0,
    )
    .unwrap();
    // A connection through the tee's branch port: the ports sit at the
    // same place already, so the records change only the connected
    // indices (§3n).
    let branch = entity(&doc, tee).pressure_ports().unwrap()[2].position;
    let branch_pipe = add_pressure_pipe(&mut doc, net, branch, v(250.0, 100.0, 7.0)).unwrap();
    connect_pressure_parts(&mut doc, tee, 2, branch_pipe, 0).unwrap();

    let back = roundtrip(&doc, "pressure-network");

    // The straight pipe: ends and joint sizes bit-exact, both ends open.
    let pipe: PressurePipe = entity(&back, straight).pressure_pipe().unwrap();
    assert_eq!(pipe.start, v(0.0, 0.0, 10.0));
    assert_eq!(pipe.end, v(100.0, 0.0, 9.0));
    assert_eq!(pipe.bulge, 0.0);
    assert_eq!(pipe.start_connection, None);
    assert_eq!(pipe.end_connection, None);

    // The arc pipe: bulge and port tangent directions bit-exact (§3n).
    let arc_back = entity(&back, arc);
    assert_eq!(arc_back.pressure_pipe().unwrap().bulge, -0.4);
    let start_direction = arc_back.pressure_ports().unwrap()[0].direction;
    assert_eq!(start_direction.x, -0.7241379310344828);
    assert_eq!(start_direction.y, -0.6896551724137931);

    // The elbow: position, rotation and both ports bit-exact against the
    // values read before the save.
    let elbow_before = entity(&doc, elbow);
    let elbow_back = entity(&back, elbow);
    assert_eq!(elbow_back.pressure_part_position(), Some(v(200.0, 0.0, 8.0)));
    assert_eq!(elbow_back.network_part_rotation(), Some(0.5));
    let ports_before = elbow_before.pressure_ports().unwrap();
    let ports_back = elbow_back.pressure_ports().unwrap();
    assert!(!ports_before.is_empty());
    assert_eq!(ports_back.len(), ports_before.len());
    for (port, before) in ports_back.iter().zip(ports_before) {
        assert_eq!(port.position, before.position, "port position bit-exact");
        assert_eq!(port.direction, before.direction, "port direction bit-exact");
        assert_eq!(port.outside_diameter, before.outside_diameter);
        assert_eq!(port.nominal_diameter, before.nominal_diameter);
        assert_eq!(port.wall_thickness, before.wall_thickness);
        assert_eq!(port.deflection, before.deflection);
        assert_eq!(port.connected_port, -1, "open");
    }

    // Ports per part kind: 2 elbow and valve, 3 tee, 4 cross (§3n).
    assert_eq!(entity(&back, tee).pressure_ports().unwrap().len(), 3);
    assert_eq!(entity(&back, cross).pressure_ports().unwrap().len(), 4);
    assert_eq!(entity(&back, valve).pressure_ports().unwrap().len(), 2);
    // The sized pipe keeps its part description among its texts.
    assert!(entity(&back, sized)
        .decode()
        .strings
        .iter()
        .any(|s| s == &pressure_part_descriptions()[0]));

    // The recorded connection: each side holds the other's port index and
    // names the other part (§3n).
    let tee_back = entity(&back, tee);
    assert_eq!(tee_back.pressure_ports().unwrap()[2].connected_port, 0, "the tee's branch meets the pipe's start");
    let branch_back = entity(&back, branch_pipe).pressure_pipe().unwrap();
    assert_eq!(branch_back.start_connection, Some((tee.value(), 2)));
    assert_eq!(branch_back.end_connection, None);
    // The pipe is the soft reference of the tee's third port (8 + port).
    assert_eq!(refs(&tee_back)[8 + 2], branch_pipe.value());

    // The network lists its pipes, fittings and appurtenances.
    let net_back = entity(&back, net);
    assert_eq!(net_back.dxf_name, "AECC_PRESSUREPIPES_NETWORK");
    assert_eq!(net_back.decode().strings[2], "RT-PRES");
    let net_refs = refs(&net_back);
    for part in [straight, arc, sized, elbow, tee, cross, valve, branch_pipe] {
        assert!(net_refs.contains(&part.value()), "network lists {:#x}", part.value());
    }
}

/// The connectors (AECC_NETWORK_PART_CONNECTOR entities) that belong to
/// `part`: their fourth class-body reference names it (soft 0, hard owner
/// 0, the paired connector, the part — §3m).
fn connectors_of(doc: &CadDocument, part: Handle) -> Vec<Handle> {
    entities_of(doc, "AECC_NETWORK_PART_CONNECTOR")
        .into_iter()
        .filter(|(_, c)| refs(c).get(3) == Some(&part.value()))
        .map(|(h, _)| h)
        .collect()
}

/// The part a connector belongs to (its fourth reference).
fn connector_part(doc: &CadDocument, connector: Handle) -> u64 {
    refs(&entity(doc, connector))[3]
}

/// A structure of a size the default parts list lacks: its part definition
/// holds the chosen values, the fields derived from them and the size
/// name, keyed by the key fields' values (§3m "Structure sizes").
#[test]
fn structure_of_a_catalog_size_carries_its_fields() {
    let family = "Cylindrical Junction Structure NF";
    let fields = network_structure_size_fields(family);
    assert_eq!(fields[0], ("SID", StructureSizeChoice::List(&[48.0, 60.0, 72.0, 96.0])));
    assert!(matches!(fields[1], ("SRS", StructureSizeChoice::Range(..))));

    let mut doc = doc();
    ensure_civil(&mut doc).unwrap();
    let net = add_pipe_network(&mut doc, "RT-SIZES", None).unwrap();
    let s = add_structure_of_size(&mut doc, net, family, &[("SID", 72.0), ("SRS", 96.0)], v(5.0, 6.0, 0.0), 0.0).unwrap();
    assert!(add_structure_of_size(&mut doc, net, family, &[("SID", 50.0)], v(0.0, 0.0, 0.0), 0.0).is_err());
    assert!(add_structure_of_size(&mut doc, net, family, &[("SFD", 18.0)], v(0.0, 0.0, 0.0), 0.0).is_err());

    let back = roundtrip(&doc, "structure-size");
    let st = entity(&back, s);
    assert_eq!(st.structure_position(), Some(v(5.0, 6.0, 0.0)));
    let definition = Handle::new(refs(&st)[4]);
    let Some(ObjectType::Arch(def)) = back.objects.get(&definition) else { panic!("part definition missing") };
    let value = |code: &str| def.network_part_fields().iter().find(|f| f.code == code).map(|f| f.value);
    assert_eq!(value("SID"), Some(72.0));
    assert_eq!(value("SRS"), Some(96.0));
    // Derived: diameter = inner diameter + 2 walls, height = rim to sump +
    // floor.
    assert_eq!(value("SBSD"), Some(80.0));
    assert_eq!(value("SBSH"), Some(102.0));
    assert!(def.decode().strings.iter().any(|t| t == "72 inch Cylindrical Structure"));
    let defs = back.named_objects_dictionary().and_then(|d| d.get("AECC_NETWORK_PART_DEFS")).unwrap();
    let Some(ObjectType::Dictionary(defs)) = back.objects.get(&defs) else { panic!("no part definitions") };
    assert_eq!(defs.get("1D412393942C400EAC7DCECB4D23AEE4SID72SRS96"), Some(definition));
}
