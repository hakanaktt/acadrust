//! Pressure pipe networks: the network, its pipes, fittings and
//! appurtenances.
//!
//! A pressure network uses a pressure part list
//! (`AECC_PRESSURE_PART_LIST`) bound to a part catalog; the writer places
//! three of the parts of the application's default list: a 4 inch ductile
//! iron pipe, a 4 inch 11.25° elbow and a 6 inch gate valve. Each part
//! refers to the part definition of its size (`AECC_NETWORK_PART_DEF`,
//! shared with gravity parts) and to the style the part list gives it.

use super::assembly::{ensure_name_template, entity_common, run};
use super::network::{arch_entity, ensure_definition_with, hard_owner, texts};
use super::pressure_content::*;
use super::support::*;
use crate::entities::{EntityCommon, EntityType};
use crate::error::DxfError;
use crate::io::dwg::dwg_stream_writers::DwgMergedWriter;
use crate::objects::ObjectType;
use crate::types::{Handle, Vector3};
use crate::CadDocument;

const PIPE_TEMPLATE: (i32, &str) = (0x17B, "Pressure Pipe - (<[67112990(2449)]>)");
const FITTING_TEMPLATE: (i32, &str) = (0x17C, "Fitting - (<[67112990(2449)]>)");
const APPURTENANCE_TEMPLATE: (i32, &str) = (0x17D, "Appurtenance - (<[67112990(2449)]>)");

/// The catalog the default pressure part list is bound to.
const CATALOG: &str = "611746E2-B19E-4F8E-ACF7-F12491879819";

const PIPE_KEY: &str = "184.8standard200.44_in_x_4_in22350AWWA_C1492pipe_4_in_push_on_ductile_iron_350_psi_AWWA_C1497ea43c7c_83bb_48f4_9b18_8b9c1a5f250epipe_push_on_ductile_iron_350_psi7ea43c7c_83bb_48f4_9b18_8b9c1a5f250e611746E2_B19E_4F8E_ACF7_F12491879819Pipe_DomainPressurePipe";
const FITTING_KEY: &str = "11.25elbow24_in_x_4_in22350AWWA_C111_C1522elbow_4_in_11.25_degree_push_on_ductile_iron_350_psi_AWWA_C111_C1524ec98468_cb87_435a_89bc_cebf9c6392bbelbow_11.25_degree_push_on_ductile_iron_350_psi05a4ab2d_c9d0_428a_a506_0f8a58ee8da1611746E2_B19E_4F8E_ACF7_F12491879819Fitting_DomainElbow";
const APPURTENANCE_KEY: &str = "gate76_in_x_6_in22200AWWA_C111_C5002gate_valve_6_inch_push_on_ductile_iron_200_psi_AWWA_C111_C5002821bd9a_296d_4bc4_8fef_e0b93833dd38gate_valve_push_on_ductile_iron_200_psic0cbab87_d7f1_458f_b4df_d1052bb936eb611746E2_B19E_4F8E_ACF7_F12491879819Appurtenance_DomainValve";

/// The kinds of pressure parts, in the order the network lists them.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Pipe,
    Fitting,
    Appurtenance,
}

impl Kind {
    fn dxf(self) -> &'static str {
        match self {
            Kind::Pipe => "AECC_PRESSUREPIPE",
            Kind::Fitting => "AECC_FITTING",
            Kind::Appurtenance => "AECC_APPURTENANCE",
        }
    }
    fn style_dxf(self) -> &'static str {
        match self {
            Kind::Pipe => "AECC_PRESSURE_PIPE_STYLE",
            Kind::Fitting => "AECC_FITTING_STYLE",
            Kind::Appurtenance => "AECC_APPURTENANCE_STYLE",
        }
    }
}

/// The drawing's pressure part list bound to the default catalog.
fn part_list(doc: &CadDocument) -> Option<Handle> {
    doc.objects
        .iter()
        .filter(|(_, o)| matches!(o, ObjectType::Arch(a) if a.dxf_name == "AECC_PRESSURE_PART_LIST" && a.decode().strings.iter().any(|s| s == CATALOG)))
        .map(|(h, _)| *h)
        .min_by_key(|h| h.value())
}

/// The style the part list gives parts of `kind`: the first style of that
/// kind among its references.
fn style_of(doc: &CadDocument, list: Handle, kind: Kind) -> Option<u64> {
    let refs = object(doc, list)?.typed_handles();
    refs.iter().map(|r| r.0).find(|h| object(doc, Handle::new(*h)).is_some_and(|o| o.dxf_name == kind.style_dxf()))
}

/// The network record (`AECC_PRESSUREPIPES_NETWORK`, a model space entity):
///
/// ```text
/// NETWORK_EMPTY (no parts yet) or NETWORK_WITH_PARTS, BL pipes,
/// BL fittings, BL appurtenances, NETWORK_END
/// references: 6 x hard 0, soft 0, hard owner 0, the pipes, the fittings,
///   the appurtenances (hard), the part list (soft)
/// texts: "AeccImpPressurePipeNetwork", "", name, then the name templates
///   of appurtenances, fittings, pipes and pipe runs
/// ```
fn write_network(doc: &mut CadDocument, common: EntityCommon, name: &str, list: u64, parts: &[Vec<u64>; 3]) -> Result<Handle, DxfError> {
    let mut refs = vec![hard_pointer(0); 6];
    refs.extend([soft_pointer(0), hard_owner(0)]);
    refs.extend(parts.iter().flatten().map(|h| hard_pointer(*h)));
    refs.push(soft_pointer(list));
    let mut strings = texts(NETWORK_STRINGS);
    strings[2] = name.to_string();
    let head = if parts.iter().all(|p| p.is_empty()) { NETWORK_EMPTY } else { NETWORK_WITH_PARTS };
    put_entity(
        doc,
        "AECC_PRESSUREPIPES_NETWORK",
        common,
        |w| {
            run(w, head.0, head.1);
            for p in parts {
                w.write_bit_long(p.len() as i32);
            }
            run(w, NETWORK_END.0, NETWORK_END.1);
        },
        Body { refs, strings },
    )
}

/// A network read back: entity data, name, part list and its pipes,
/// fittings and appurtenances.
struct Network {
    common: EntityCommon,
    name: String,
    list: u64,
    parts: [Vec<u64>; 3],
}

fn read_network(doc: &CadDocument, network: Handle) -> Option<Network> {
    let a = arch_entity(doc, network).filter(|a| a.dxf_name == "AECC_PRESSUREPIPES_NETWORK")?;
    let common = doc.get_entity(network)?.common().clone();
    let refs: Vec<u64> = a.typed_handles().iter().map(|r| r.0).collect();
    let (list, parts) = refs.get(8..)?.split_last()?;
    let of = |k: Kind| parts.iter().filter(|h| arch_entity(doc, Handle::new(**h)).is_some_and(|e| e.dxf_name == k.dxf())).copied().collect();
    Some(Network { common, name: a.decode().strings.get(2)?.clone(), list: *list, parts: [of(Kind::Pipe), of(Kind::Fitting), of(Kind::Appurtenance)] })
}

/// Adds an empty pressure network named `name` on `layer` (layer "0" when
/// `None`) that uses the drawing's default pressure part list (the one
/// bound to the default catalog). A drawing without pressure part lists
/// gets the default one with the default pressure styles.
pub fn add_pressure_network(doc: &mut CadDocument, name: &str, layer: Option<&str>) -> Result<Handle, DxfError> {
    if name.is_empty() {
        return Err(invalid("a pressure network needs a name"));
    }
    if part_list(doc).is_none() && !doc.objects.values().any(|o| matches!(o, ObjectType::Arch(a) if a.dxf_name == "AECC_PRESSURE_PART_LIST")) {
        super::default_styles::ensure_pressure_network_styles(doc)?;
    }
    let list = part_list(doc).ok_or_else(|| invalid("drawing has no pressure part list of the default catalog"))?;
    let common = entity_common(doc, layer)?;
    for t in [PIPE_TEMPLATE, FITTING_TEMPLATE, APPURTENANCE_TEMPLATE] {
        ensure_name_template(doc, t)?;
    }
    write_network(doc, common, name, list.value(), &[Vec::new(), Vec::new(), Vec::new()])
}

/// Writes a part of `kind` with `write` (given its entity data, references
/// and number in the network) and adds it to the network's lists.
fn place(
    doc: &mut CadDocument,
    network: Handle,
    kind: Kind,
    definition: (&str, (usize, &str), &[&str]),
    body: Option<&Body3>,
    write: impl FnOnce(&mut CadDocument, EntityCommon, Vec<Ref>, usize) -> Result<Handle, DxfError>,
) -> Result<Handle, DxfError> {
    let mut net = read_network(doc, network).ok_or_else(|| invalid("not a pressure network"))?;
    let style = style_of(doc, Handle::new(net.list), kind).ok_or_else(|| invalid("the network's part list has no style for the part"))?;
    let definition = ensure_definition_with(doc, definition.0, definition.1, definition.2, |doc| match body {
        Some(b) => Ok(body_blocks(doc, b)?.iter().map(|h| hard_pointer(h.value())).collect()),
        None => Ok(Vec::new()),
    })?;
    let common = entity_common(doc, Some(&net.common.layer))?;
    let refs = vec![
        hard_pointer(network.value()),
        hard_pointer(0),
        hard_pointer(0),
        hard_pointer(net.list),
        soft_pointer(0),
        hard_owner(0),
        hard_pointer(definition.value()),
        hard_pointer(style),
        soft_pointer(0),
        soft_pointer(0),
        soft_pointer(0),
    ];
    let index = kind as usize;
    let handle = write(doc, common, refs, net.parts[index].len() + 1)?;
    net.parts[index].push(handle.value());
    write_network(doc, net.common, &net.name, net.list, &net.parts)?;
    Ok(handle)
}

fn point(w: &mut DwgMergedWriter, p: Vector3) {
    for v in [p.x, p.y, p.z] {
        w.write_bit_double(v);
    }
}

/// The unit vector from `a` towards `b`.
fn direction(a: Vector3, b: Vector3) -> Vector3 {
    let d = Vector3::new(b.x - a.x, b.y - a.y, b.z - a.z);
    let n = (d.x * d.x + d.y * d.y + d.z * d.z).sqrt();
    Vector3::new(d.x / n, d.y / n, d.z / n)
}

/// Adds a 4 inch ductile iron pressure pipe from `start` to `end` to
/// `network`, not connected to other parts. The pipe record
/// (`AECC_PRESSUREPIPE`):
///
/// ```text
/// PIPE_HEAD (its 16-byte id zero), PIPE_PRE (an identity frame, 2 ports),
/// port 1: 3BD start, 3BD direction away from the pipe, PIPE_PORT0 (joint
///   sizes, open),
/// port 2: 3BD end, 3BD direction, PIPE_PORT1,
/// 3BD start, 3BD end, BD bulge (0), PIPE_TAIL, BD start elevation,
/// BD plan length, BD end elevation, PIPE_END
/// references: network, hard 0, hard 0, part list, soft 0, hard owner 0,
///   part definition, style, 3 x soft 0
/// texts: PIPE_STRINGS with the name "Pressure Pipe - (n)"
/// ```
pub fn add_pressure_pipe(doc: &mut CadDocument, network: Handle, start: Vector3, end: Vector3) -> Result<Handle, DxfError> {
    if ![start.x, start.y, start.z, end.x, end.y, end.z].iter().all(|v| v.is_finite()) || start.distance(&end) < 1e-9 {
        return Err(invalid("a pressure pipe needs two distinct finite ends"));
    }
    place(doc, network, Kind::Pipe, (PIPE_KEY, PIPE_DEFINITION, PIPE_DEFINITION_STRINGS), None, |doc, common, refs, n| {
        let mut strings = texts(PIPE_STRINGS);
        strings[3] = format!("Pressure Pipe - ({n})");
        let plan = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt();
        put_entity(
            doc,
            Kind::Pipe.dxf(),
            common,
            |w| {
                run(w, PIPE_HEAD.0, PIPE_HEAD.1);
                run(w, PIPE_PRE.0, PIPE_PRE.1);
                point(w, start);
                point(w, direction(end, start));
                run(w, PIPE_PORT0.0, PIPE_PORT0.1);
                point(w, end);
                point(w, direction(start, end));
                run(w, PIPE_PORT1.0, PIPE_PORT1.1);
                point(w, start);
                point(w, end);
                w.write_bit_double(0.0);
                run(w, PIPE_TAIL.0, PIPE_TAIL.1);
                for v in [start.z, plan, end.z] {
                    w.write_bit_double(v);
                }
                run(w, PIPE_END.0, PIPE_END.1);
            },
            Body { refs, strings },
        )
    })
}

/// The constant runs and geometry of a fitting or appurtenance size.
struct Placed {
    kind: Kind,
    key: &'static str,
    definition: (usize, &'static str),
    definition_strings: &'static [&'static str],
    head: (usize, &'static str),
    pre: (usize, &'static str),
    ports: [([f64; 3], [f64; 3]); 2],
    port_runs: [(usize, &'static str); 2],
    anchor: [f64; 3],
    end: (usize, &'static str),
    strings: &'static [&'static str],
    name: &'static str,
    body: Body3,
}

/// The two blocks a fitting's or appurtenance's definition refers to,
/// named after the part family: its 3D body (`<family>`) and its plan
/// symbol (`<family>_3`), in coordinates about the part's reference point
/// (the application places them there).
// ponytail: the application's bodies are the catalog's solids (13 and
// 44 KB); the writer writes a box of the same extents instead.
struct Body3 {
    family: &'static str,
    /// Corners of the box.
    min: [f64; 3],
    max: [f64; 3],
    symbol: Symbol,
}

enum Symbol {
    /// Center, radius, start and end angles, drawn about -Z.
    Arc([f64; 3], f64, f64, f64),
    Line([f64; 3], [f64; 3]),
}

/// The body and symbol blocks of `body` (reused when the drawing has
/// them).
fn body_blocks(doc: &mut CadDocument, body: &Body3) -> Result<[Handle; 2], DxfError> {
    use crate::entities::{Arc, Line, Solid3D};
    use crate::entities::acis::primitives::build_box;
    let v = |p: [f64; 3]| Vector3::new(p[0], p[1], p[2]);
    let (min, max) = (body.min, body.max);
    let mut solid = Solid3D::new();
    solid.set_sat_document(&build_box([(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0, (min[2] + max[2]) / 2.0], max[0] - min[0], max[1] - min[1], max[2] - min[2]));
    let symbol = match body.symbol {
        Symbol::Arc(c, r, a, b) => {
            let mut arc = Arc::new();
            (arc.center, arc.radius, arc.start_angle, arc.end_angle, arc.normal) = (v(c), r, a, b, Vector3::new(0.0, 0.0, -1.0));
            EntityType::Arc(arc)
        }
        Symbol::Line(a, b) => EntityType::Line(Line::from_points(v(a), v(b))),
    };
    let mut handles = [Handle::NULL; 2];
    for (i, (name, entity)) in [(body.family.to_string(), EntityType::Solid3D(solid)), (format!("{}_3", body.family), symbol)].into_iter().enumerate() {
        let existing = doc.block_records.iter().find(|b| b.name == name).map(|b| b.handle);
        handles[i] = match existing {
            Some(h) => h,
            None => block_with(doc, &name, entity)?,
        };
    }
    Ok(handles)
}

/// A block named `name` holding `entity` on layer 0, ByBlock.
fn block_with(doc: &mut CadDocument, name: &str, mut entity: EntityType) -> Result<Handle, DxfError> {
    use crate::entities::{Block, BlockEnd};
    use crate::tables::BlockRecord;
    let (record, start, end) = (doc.allocate_handle(), doc.allocate_handle(), doc.allocate_handle());
    let mut br = BlockRecord::new(name);
    (br.handle, br.block_entity_handle, br.block_end_handle) = (record, start, end);
    doc.block_records.add(br).map_err(|_| invalid("cannot add a part body's block"))?;
    let mut marker = Block::new(name, Vector3::ZERO);
    (marker.common.handle, marker.common.owner_handle) = (start, record);
    doc.add_entity(EntityType::Block(marker))?;
    let c = entity.common_mut();
    c.owner_handle = record;
    c.color = crate::types::Color::ByBlock;
    c.line_weight = crate::types::LineWeight::ByBlock;
    doc.add_entity(entity)?;
    let mut tail = BlockEnd::new();
    (tail.common.handle, tail.common.owner_handle) = (end, record);
    doc.add_entity(EntityType::BlockEnd(tail))?;
    Ok(record)
}

const ELBOW: Placed = Placed {
    kind: Kind::Fitting,
    key: FITTING_KEY,
    definition: FITTING_DEFINITION,
    definition_strings: FITTING_DEFINITION_STRINGS,
    head: FITTING_HEAD,
    pre: FITTING_PRE,
    ports: FITTING_PORTS,
    port_runs: [FITTING_PORT0, FITTING_PORT1],
    anchor: FITTING_ANCHOR,
    end: FITTING_END,
    strings: FITTING_STRINGS,
    name: "Fitting",
    body: Body3 {
        family: "05a4ab2d-c9d0-428a-a506-0f8a58ee8da1",
        min: [-0.28916666666666, 0.0, -0.28916666666667],
        max: [0.3683121250586, 0.916404560691395, 0.28916666666667],
        symbol: Symbol::Arc([-1.692195064601477, 0.26749999999999996, 0.0], 1.692195064601477, 0.0, 0.19634954084936204),
    },
};

const GATE_VALVE: Placed = Placed {
    kind: Kind::Appurtenance,
    key: APPURTENANCE_KEY,
    definition: APPURTENANCE_DEFINITION,
    definition_strings: APPURTENANCE_DEFINITION_STRINGS,
    head: APPURTENANCE_HEAD,
    pre: APPURTENANCE_PRE,
    ports: APPURTENANCE_PORTS,
    port_runs: [APPURTENANCE_PORT0, APPURTENANCE_PORT1],
    anchor: APPURTENANCE_ANCHOR,
    end: APPURTENANCE_END,
    strings: APPURTENANCE_STRINGS,
    name: "Appurtenance",
    body: Body3 {
        family: "c0cbab87-d7f1-458f-b4df-d1052bb936eb",
        min: [-0.62833333333333, -0.355, 0.2802421927990584],
        max: [0.62833333333333, 0.355, 2.1352421927990584],
        symbol: Symbol::Line([-0.2708333333333333, 0.0, 0.6352416666666666], [0.2708333333333333, 0.0, 0.6352416666666666]),
    },
};

/// Writes a fitting or appurtenance at `position` turned by `rotation`
/// (radians) about Z:
///
/// ```text
/// HEAD, 3BD insertion point, 3 x 3BD axes (X, Y, Z), PRE (2 ports),
/// per port: 3BD position, 3BD direction, the port's constant run (joint
///   sizes, open),
/// 3BD reference point (a point of the part's size: the body and symbol
///   blocks are placed there; the application keeps the part's extents
///   until it moves), 3 x 3BD axes again, END
/// references and texts as a pressure pipe's (name "<Kind> - (n)")
/// ```
fn add_placed(doc: &mut CadDocument, network: Handle, part: &Placed, position: Vector3, rotation: f64) -> Result<Handle, DxfError> {
    if ![position.x, position.y, position.z, rotation].iter().all(|v| v.is_finite()) {
        return Err(invalid("a pressure part needs a finite position and rotation"));
    }
    let (c, s) = (rotation.cos(), rotation.sin());
    let turn = |v: [f64; 3]| Vector3::new(c * v[0] - s * v[1], s * v[0] + c * v[1], v[2]);
    let at = |v: [f64; 3]| {
        let t = turn(v);
        Vector3::new(position.x + t.x, position.y + t.y, position.z + t.z)
    };
    place(doc, network, part.kind, (part.key, part.definition, part.definition_strings), Some(&part.body), |doc, common, refs, n| {
        let mut strings = texts(part.strings);
        strings[3] = format!("{} - ({n})", part.name);
        put_entity(
            doc,
            part.kind.dxf(),
            common,
            |w| {
                let axes = |w: &mut DwgMergedWriter| {
                    for v in [c, s, 0.0, -s, c, 0.0, 0.0, 0.0, 1.0] {
                        w.write_bit_double(v);
                    }
                };
                run(w, part.head.0, part.head.1);
                point(w, position);
                axes(w);
                run(w, part.pre.0, part.pre.1);
                for ((offset, dir), runs) in part.ports.iter().zip(part.port_runs) {
                    point(w, at(*offset));
                    point(w, turn(*dir));
                    run(w, runs.0, runs.1);
                }
                point(w, at(part.anchor));
                axes(w);
                run(w, part.end.0, part.end.1);
            },
            Body { refs, strings },
        )
    })
}

/// Adds a 4 inch 11.25° elbow to `network` at `position`, turned by
/// `rotation` (radians) about Z, not connected to other parts.
pub fn add_pressure_fitting(doc: &mut CadDocument, network: Handle, position: Vector3, rotation: f64) -> Result<Handle, DxfError> {
    add_placed(doc, network, &ELBOW, position, rotation)
}

/// Adds a 6 inch gate valve to `network` at `position`, turned by
/// `rotation` (radians) about Z, not connected to other parts.
pub fn add_pressure_appurtenance(doc: &mut CadDocument, network: Handle, position: Vector3, rotation: f64) -> Result<Handle, DxfError> {
    add_placed(doc, network, &GATE_VALVE, position, rotation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::classes::DxfClass;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::DxfVersion;

    /// A stand-in pressure part list bound to the default catalog, with one
    /// style of each part kind among its references.
    fn with_part_list(doc: &mut CadDocument) -> [u64; 3] {
        let root = ensure_civil(doc).unwrap();
        for k in [Kind::Pipe, Kind::Fitting, Kind::Appurtenance] {
            doc.classes.add_or_update(DxfClass::new(k.style_dxf(), k.style_dxf()));
        }
        doc.classes.add_or_update(DxfClass::new("AECC_PRESSURE_PART_LIST", "AECC_PRESSURE_PART_LIST"));
        let styles = [Kind::Pipe, Kind::Fitting, Kind::Appurtenance].map(|k| {
            let h = doc.allocate_handle();
            put_object(doc, k.style_dxf(), h, root, Vec::new(), |_| {}, Body { refs: Vec::new(), strings: Vec::new() }).unwrap();
            h.value()
        });
        let list = doc.allocate_handle();
        put_object(doc, "AECC_PRESSURE_PART_LIST", list, root, Vec::new(), |_| {}, Body { refs: styles.iter().map(|h| hard_pointer(*h)).collect(), strings: texts(&["", CATALOG]) }).unwrap();
        styles
    }

    #[test]
    fn pressure_networks_list_their_parts_where_written() {
        let v = Vector3::new;
        // A new drawing gets the default part list and styles.
        let mut fresh = CadDocument::new();
        fresh.version = DxfVersion::AC1032;
        let n0 = add_pressure_network(&mut fresh, "PR", None).unwrap();
        let list = part_list(&fresh).unwrap();
        assert_eq!(read_network(&fresh, n0).unwrap().list, list.value());
        for k in [Kind::Pipe, Kind::Fitting, Kind::Appurtenance] {
            assert!(style_of(&fresh, list, k).is_some());
        }
        add_pressure_fitting(&mut fresh, n0, Vector3::new(0.0, 0.0, 0.0), 0.0).unwrap();
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let styles = with_part_list(&mut doc);
        assert!(add_pressure_network(&mut doc, "", None).is_err());
        let n = add_pressure_network(&mut doc, "PR1", None).unwrap();
        let p = add_pressure_pipe(&mut doc, n, v(0.0, 0.0, 10.0), v(100.0, 0.0, 9.0)).unwrap();
        assert!(add_pressure_pipe(&mut doc, n, v(1.0, 1.0, 1.0), v(1.0, 1.0, 1.0)).is_err());
        let f = add_pressure_fitting(&mut doc, n, v(200.0, 50.0, 8.0), 0.5).unwrap();
        let a = add_pressure_appurtenance(&mut doc, n, v(250.0, 0.0, 8.0), 0.0).unwrap();
        let path = std::env::temp_dir().join(format!("civil-pressure-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let net = read_network(&back, n).unwrap();
        assert_eq!((net.name.as_str(), net.parts.clone()), ("PR1", [vec![p.value()], vec![f.value()], vec![a.value()]]));
        let pipe = arch_entity(&back, p).unwrap().pressure_pipe().unwrap();
        assert_eq!((pipe.start, pipe.end, pipe.bulge), (v(0.0, 0.0, 10.0), v(100.0, 0.0, 9.0), 0.0));
        assert_eq!((pipe.start_connection, pipe.end_connection), (None, None));
        let fitting = arch_entity(&back, f).unwrap();
        assert_eq!(fitting.pressure_part_position(), Some(v(200.0, 50.0, 8.0)));
        assert!((fitting.network_part_rotation().unwrap() - 0.5).abs() < 1e-12);
        assert_eq!(fitting.decode().strings[3], "Fitting - (1)");
        assert_eq!(arch_entity(&back, a).unwrap().pressure_part_position(), Some(v(250.0, 0.0, 8.0)));
        // References: network, part list, definition and the list's style.
        let refs: Vec<u64> = arch_entity(&back, p).unwrap().typed_handles().iter().map(|r| r.0).collect();
        assert_eq!((refs[0], refs[7]), (n.value(), styles[0]));
        assert_eq!(arch_entity(&back, f).unwrap().typed_handles()[7].0, styles[1]);
        // The elbow's definition refers to its body and symbol blocks.
        let def = object(&back, Handle::new(arch_entity(&back, f).unwrap().typed_handles()[6].0)).unwrap();
        let blocks: Vec<String> = def.typed_handles().iter().filter_map(|r| back.block_records.iter().find(|b| b.handle.value() == r.0).map(|b| b.name.clone())).collect();
        assert_eq!(blocks, vec![ELBOW.body.family.to_string(), format!("{}_3", ELBOW.body.family)]);
    }
}
