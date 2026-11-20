//! Pressure pipe networks: the network, its pipes, fittings and
//! appurtenances.
//!
//! A pressure network uses a pressure part list
//! (`AECC_PRESSURE_PART_LIST`) bound to a part catalog; the writer places
//! parts of the application's default list: 4 inch ductile iron pipes
//! (straight or curved), a 4 inch 11.25° elbow, a 4 inch tee, a 14 x 6 inch
//! cross and a 6 inch gate valve, and connects their ports. Each part
//! refers to the part definition of its size (`AECC_NETWORK_PART_DEF`,
//! shared with gravity parts) and to the style the part list gives it.

use super::assembly::{ensure_name_template, entity_common, run};
use super::network::{arch_entity, ensure_definition_with, hard_owner, run_range, texts, typed};
use super::pressure_content::*;
use super::pressure_fittings::KINDS;
use super::pressure_sizes::PIPES;
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

/// The kinds of pressure parts, in the order the network lists them.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Kind {
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
    body: Option<&Size>,
    ports: usize,
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
    ];
    // One per port (the part it connects to), then one more.
    let refs = [refs, vec![soft_pointer(0); ports + 1]].concat();
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

/// Adds a straight 4 inch ductile iron pressure pipe from `start` to `end`
/// to `network`, not connected to other parts (see
/// [`add_pressure_arc_pipe`]).
pub fn add_pressure_pipe(doc: &mut CadDocument, network: Handle, start: Vector3, end: Vector3) -> Result<Handle, DxfError> {
    add_pressure_arc_pipe(doc, network, start, end, 0.0)
}

/// Adds a 4 inch ductile iron pressure pipe (see
/// [`add_pressure_pipe_of_size`]).
pub fn add_pressure_arc_pipe(doc: &mut CadDocument, network: Handle, start: Vector3, end: Vector3, bulge: f64) -> Result<Handle, DxfError> {
    add_pressure_pipe_of_size(doc, network, PIPES[0].description, start, end, bulge)
}

/// `v` turned by `angle` (radians) about Z.
fn turned(v: Vector3, angle: f64) -> Vector3 {
    let (c, s) = (angle.cos(), angle.sin());
    Vector3::new(c * v.x - s * v.y, s * v.x + c * v.y, v.z)
}

/// Adds the pressure pipe `description` (a pipe of
/// [`pressure_part_descriptions`], 4 to 36 inch ductile iron) from `start`
/// to `end` to `network`, curved in plan by `bulge` (the tangent of a quarter of the
/// arc's angle, positive counterclockwise, at most 1 in size; 0 for a
/// straight pipe), not connected to other parts. The pipe record
/// (`AECC_PRESSUREPIPE`):
///
/// ```text
/// PIPE_HEAD (its 16-byte id zero), PIPE_PRE (an identity frame, 2 ports),
/// port 1: 3BD start, 3BD direction away from the pipe, the size's port
///   run (joint sizes, open),
/// port 2: 3BD end, 3BD direction, the size's port run,
/// 3BD start, 3BD end, BD bulge, BS 0x101 (straight) or 0x102 (curved),
/// PIPE_TAIL, BD start elevation, BD plan chord, BD end elevation, PIPE_END
/// references: network, hard 0, hard 0, part list, soft 0, hard owner 0,
///   part definition, style, the parts its start and end connect to
///   (soft), soft 0
/// texts: PIPE_STRINGS with the description and the name
///   "Pressure Pipe - (n)"
/// ```
///
/// A curved pipe's port directions are its tangents: the chord turned by
/// twice the bulge's angle, sloped by the fall over the arc.
pub fn add_pressure_pipe_of_size(doc: &mut CadDocument, network: Handle, description: &str, start: Vector3, end: Vector3, bulge: f64) -> Result<Handle, DxfError> {
    let size = PIPES.iter().find(|p| p.description == description).ok_or_else(|| invalid("no pressure pipe of that description"))?;
    if ![start.x, start.y, start.z, end.x, end.y, end.z, bulge].iter().all(|v| v.is_finite()) || start.distance(&end) < 1e-9 {
        return Err(invalid("a pressure pipe needs two distinct finite ends"));
    }
    let plan = ((end.x - start.x).powi(2) + (end.y - start.y).powi(2)).sqrt();
    if bulge != 0.0 && (plan < 1e-9 || bulge.abs() > 1.0) {
        return Err(invalid("a curved pressure pipe needs ends apart in plan and a bulge of at most 1"));
    }
    let (away_start, away_end) = if bulge == 0.0 {
        (direction(end, start), direction(start, end))
    } else {
        let chord = Vector3::new((end.x - start.x) / plan, (end.y - start.y) / plan, 0.0);
        let half = 2.0 * bulge.atan();
        let (length, rise) = (plan * half / half.sin(), end.z - start.z);
        let (level, fall) = (length / length.hypot(rise), rise / length.hypot(rise));
        let sloped = |t: Vector3, sign: f64| Vector3::new(sign * t.x * level, sign * t.y * level, sign * fall);
        // Heading into the pipe at its start, and on at its end.
        (sloped(turned(chord, -half), -1.0), sloped(turned(chord, half), 1.0))
    };
    let definition_strings = with_texts(PIPE_DEFINITION_STRINGS, size.definition_strings);
    let definition_strings: Vec<&str> = definition_strings.iter().map(String::as_str).collect();
    place(doc, network, Kind::Pipe, (size.key, size.definition, &definition_strings), None, 2, |doc, common, refs, n| {
        let mut strings = texts(PIPE_STRINGS);
        strings[2] = size.description.to_string();
        strings[3] = format!("Pressure Pipe - ({n})");
        put_entity(
            doc,
            Kind::Pipe.dxf(),
            common,
            |w| {
                run(w, PIPE_HEAD.0, PIPE_HEAD.1);
                run(w, PIPE_PRE.0, PIPE_PRE.1);
                point(w, start);
                point(w, away_start);
                run(w, size.ports[0].0, size.ports[0].1);
                point(w, end);
                point(w, away_end);
                run(w, size.ports[1].0, size.ports[1].1);
                point(w, start);
                point(w, end);
                w.write_bit_double(bulge);
                w.write_bit_short(if bulge == 0.0 { 0x101 } else { 0x102 });
                run_range(w, PIPE_TAIL.1, 18, PIPE_TAIL.0);
                for v in [start.z, plan, end.z] {
                    w.write_bit_double(v);
                }
                run(w, PIPE_END.0, PIPE_END.1);
            },
            Body { refs, strings },
        )
    })
}

/// A pipe size of the catalog: its part description, its definition's
/// key, record and texts (a base list with the size's own texts) and the
/// runs of its ports after their directions (joint sizes, open).
pub(super) struct PipeSize {
    pub(super) description: &'static str,
    pub(super) key: &'static str,
    pub(super) ports: [(usize, &'static str); 2],
    pub(super) definition: (usize, &'static str),
    pub(super) definition_strings: &'static [(usize, &'static str)],
}

/// A fitting or appurtenance kind of the catalog (elbows, tees, crosses,
/// gate valves): the runs every size shares and where a size's own data
/// goes in its definition.
pub(super) struct PartKind {
    pub(super) kind: Kind,
    /// The last word of the definition's key.
    pub(super) domain: &'static str,
    pub(super) head: (usize, &'static str),
    pub(super) end: (usize, &'static str),
    /// A size's definition record, its texts and where in them a size's
    /// own texts (size, standard, description, size id, family, family id)
    /// and bits (`windows`) go.
    pub(super) definition: (usize, &'static str),
    pub(super) definition_strings: &'static [&'static str],
    pub(super) text_at: [usize; 6],
    pub(super) windows: &'static [(usize, usize)],
    pub(super) strings: &'static [&'static str],
    pub(super) sizes: &'static [Size],
}

/// A fitting or appurtenance size: the start of its definition's key,
/// its six texts, its definition bits in the kind's windows, its ports,
/// reference point (about the insertion point), body extents (about the
/// reference point) and plan symbol.
pub(super) struct Size {
    pub(super) prefix: &'static str,
    pub(super) texts: [&'static str; 6],
    pub(super) patch: &'static str,
    pub(super) ports: &'static [Port],
    pub(super) anchor: [f64; 3],
    pub(super) extents: ([f64; 3], [f64; 3]),
    pub(super) symbol: Symbol,
}

/// A port of a size, unrotated: offset from the insertion point,
/// direction, engagement length and joint sizes (outside diameter,
/// nominal diameter, wall thickness, deflection).
pub(super) struct Port {
    pub(super) offset: [f64; 3],
    pub(super) direction: [f64; 3],
    pub(super) engagement: f64,
    pub(super) sizes: [f64; 4],
}

impl Size {
    /// The definition's key: the size's prefix, then its texts and the
    /// catalog's id with spaces, dashes and slashes as underscores, and
    /// the kind's domain.
    fn key(&self, kind: &PartKind) -> String {
        let n = |t: &str| t.replace([' ', '-', '/'], "_");
        let t = &self.texts;
        format!("{}{}{}{}{}{}{}{}{}", self.prefix, n(t[1]), self.ports.len(), n(t[2]), n(t[3]), n(t[4]), n(t[5]), n(CATALOG), kind.domain)
    }
}

/// `base` with the texts of `own` in place.
fn with_texts(base: &[&str], own: &[(usize, &str)]) -> Vec<String> {
    let mut t = texts(base);
    for (i, v) in own {
        t[*i] = v.to_string();
    }
    t
}

pub(super) enum Symbol {
    /// Center, radius, start and end angles, drawn about -Z.
    Arc([f64; 3], f64, f64, f64),
    Lines(&'static [([f64; 3], [f64; 3])]),
}

/// The two blocks a fitting's or appurtenance's definition refers to,
/// named after the size's family id: its 3D body (`<family>`) and its plan
/// symbol (`<family>_3`), in coordinates about the part's reference point
/// (the application places them there); reused when the drawing has them.
// ponytail: the application's bodies are the catalog's generated solids;
// the writer writes a box of the same extents instead.
fn body_blocks(doc: &mut CadDocument, size: &Size) -> Result<[Handle; 2], DxfError> {
    use crate::entities::{Arc, Line, Solid3D};
    use crate::entities::acis::primitives::build_box;
    let v = |p: [f64; 3]| Vector3::new(p[0], p[1], p[2]);
    let (min, max) = size.extents;
    let mut solid = Solid3D::new();
    solid.set_sat_document(&build_box([(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0, (min[2] + max[2]) / 2.0], max[0] - min[0], max[1] - min[1], max[2] - min[2]));
    let symbol = match size.symbol {
        Symbol::Arc(c, r, a, b) => {
            let mut arc = Arc::new();
            (arc.center, arc.radius, arc.start_angle, arc.end_angle, arc.normal) = (v(c), r, a, b, Vector3::new(0.0, 0.0, -1.0));
            vec![EntityType::Arc(arc)]
        }
        Symbol::Lines(lines) => lines.iter().map(|(a, b)| EntityType::Line(Line::from_points(v(*a), v(*b)))).collect(),
    };
    let mut handles = [Handle::NULL; 2];
    let family = size.texts[5];
    for (i, (name, entities)) in [(family.to_string(), vec![EntityType::Solid3D(solid)]), (format!("{family}_3"), symbol)].into_iter().enumerate() {
        let existing = doc.block_records.iter().find(|b| b.name == name).map(|b| b.handle);
        handles[i] = match existing {
            Some(h) => h,
            None => block_with(doc, &name, entities)?,
        };
    }
    Ok(handles)
}

/// A block named `name` holding `entities` on layer 0, ByBlock.
fn block_with(doc: &mut CadDocument, name: &str, entities: Vec<EntityType>) -> Result<Handle, DxfError> {
    use crate::entities::{Block, BlockEnd};
    use crate::tables::BlockRecord;
    let (record, start, end) = (doc.allocate_handle(), doc.allocate_handle(), doc.allocate_handle());
    let mut br = BlockRecord::new(name);
    (br.handle, br.block_entity_handle, br.block_end_handle) = (record, start, end);
    doc.block_records.add(br).map_err(|_| invalid("cannot add a part body's block"))?;
    let mut marker = Block::new(name, Vector3::ZERO);
    (marker.common.handle, marker.common.owner_handle) = (start, record);
    doc.add_entity(EntityType::Block(marker))?;
    for mut entity in entities {
        let c = entity.common_mut();
        c.owner_handle = record;
        c.color = crate::types::Color::ByBlock;
        c.line_weight = crate::types::LineWeight::ByBlock;
        doc.add_entity(entity)?;
    }
    let mut tail = BlockEnd::new();
    (tail.common.handle, tail.common.owner_handle) = (end, record);
    doc.add_entity(EntityType::BlockEnd(tail))?;
    Ok(record)
}

/// Writes a fitting or appurtenance at `position` turned by `rotation`
/// (radians) about Z:
///
/// ```text
/// HEAD, 3BD insertion point, 3 x 3BD axes (X, Y, Z), BL ports, 2 bits 10,
/// per port: 3BD position, 3BD direction, BD engagement length, BL -1
///   (open), 4 x BD joint sizes, then BD 1 and the next port's number in
///   8 bits, or for the last port BD 0 and 2 bits 10,
/// 3BD reference point (a point of the part's size: the body and symbol
///   blocks are placed there; the application keeps the part's extents
///   until it moves), 3 x 3BD axes again, END
/// references and texts as a pressure pipe's (name "<Kind> - (n)"), one
///   connection reference per port
/// ```
fn add_placed(doc: &mut CadDocument, network: Handle, part: &PartKind, size: &Size, position: Vector3, rotation: f64) -> Result<Handle, DxfError> {
    if ![position.x, position.y, position.z, rotation].iter().all(|v| v.is_finite()) {
        return Err(invalid("a pressure part needs a finite position and rotation"));
    }
    let (c, s) = (rotation.cos(), rotation.sin());
    let turn = |v: [f64; 3]| Vector3::new(c * v[0] - s * v[1], s * v[0] + c * v[1], v[2]);
    let at = |v: [f64; 3]| {
        let t = turn(v);
        Vector3::new(position.x + t.x, position.y + t.y, position.z + t.z)
    };
    let own: Vec<(usize, &str)> = part.text_at.iter().copied().zip(size.texts).collect();
    let definition_strings = with_texts(part.definition_strings, &own);
    let definition_strings: Vec<&str> = definition_strings.iter().map(String::as_str).collect();
    // The size's bits in the kind's windows of the definition record.
    let mut bits: Vec<char> = (0..part.definition.0).map(|i| hex_bit(part.definition.1, i)).collect();
    let mut patch = size.patch.chars();
    for &(a, b) in part.windows {
        bits.splice(a..b, patch.by_ref().take(b - a).collect::<Vec<_>>());
    }
    let definition = to_hex(&bits);
    let key = size.key(part);
    let name = match part.kind {
        Kind::Appurtenance => "Appurtenance",
        _ => "Fitting",
    };
    place(doc, network, part.kind, (&key, (bits.len(), &definition), &definition_strings), Some(size), size.ports.len(), |doc, common, refs, n| {
        let mut strings = texts(part.strings);
        strings[2] = size.texts[2].to_string();
        strings[3] = format!("{name} - ({n})");
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
                w.write_bit_long(size.ports.len() as i32);
                w.write_bit(true);
                w.write_bit(false);
                for (k, port) in size.ports.iter().enumerate() {
                    point(w, at(port.offset));
                    point(w, turn(port.direction));
                    w.write_bit_double(port.engagement);
                    w.write_bit_long(-1);
                    for v in port.sizes {
                        w.write_bit_double(v);
                    }
                    if k + 1 < size.ports.len() {
                        w.write_bit_double(1.0);
                        w.write_byte(k as u8 + 1);
                    } else {
                        w.write_bit_double(0.0);
                        w.write_bit(true);
                        w.write_bit(false);
                    }
                }
                point(w, at(size.anchor));
                axes(w);
                run(w, part.end.0, part.end.1);
            },
            Body { refs, strings },
        )
    })
}

/// The part descriptions of the catalog's sizes the writer places, as
/// the application names them ("gate valve-8 inch-push on-ductile
/// iron-200 psi-AWWA C111/C500"): pipes, then fittings and appurtenances.
pub fn pressure_part_descriptions() -> Vec<&'static str> {
    PIPES.iter().map(|p| p.description).chain(KINDS.iter().flat_map(|k| k.sizes.iter().map(|s| s.texts[2]))).collect()
}

fn placed(description: &str) -> Result<(&'static PartKind, &'static Size), DxfError> {
    KINDS
        .iter()
        .find_map(|k| k.sizes.iter().find(|s| s.texts[2] == description).map(|s| (k, s)))
        .ok_or_else(|| invalid("no fitting or appurtenance of that description"))
}

/// Bit `i` of the run `hex` as '0' or '1'.
fn hex_bit(hex: &str, i: usize) -> char {
    let v = hex.as_bytes().get(i / 4).and_then(|c| (*c as char).to_digit(16)).unwrap_or(0);
    if v >> (3 - i % 4) & 1 == 1 { '1' } else { '0' }
}

/// The bits as a constant run (hex, zero padded).
fn to_hex(bits: &[char]) -> String {
    bits.chunks(4).map(|c| {
        let v = c.iter().chain(std::iter::repeat(&'0')).take(4).fold(0, |a, b| a * 2 + (*b == '1') as u32);
        std::char::from_digit(v, 16).unwrap()
    }).collect()
}

/// Adds the fitting or appurtenance `description` (one of
/// [`pressure_part_descriptions`]) to `network` at `position`, turned by
/// `rotation` (radians) about Z, not connected to other parts. Elbows have
/// two ports (-Y, then the turned leg), tees three (the run's ends along
/// -Y and +Y, then the branch, +X), crosses four (the run, then +X and
/// -X), valves two (-X, +X).
pub fn add_pressure_part(doc: &mut CadDocument, network: Handle, description: &str, position: Vector3, rotation: f64) -> Result<Handle, DxfError> {
    let (kind, size) = placed(description)?;
    add_placed(doc, network, kind, size, position, rotation)
}

const ELBOW_4: &str = "elbow-4 in-11.25 degree-push on-ductile iron-350 psi-AWWA C111/C152";
const TEE_4: &str = "tee-4 in x 4 in- push on-ductile iron-350 psi-AWWA C111/C153";
const CROSS_14: &str = "cross-14 in x 6 in- push on-ductile iron-350 psi-AWWA C110/C111/C153";
const VALVE_6: &str = "gate valve-6 inch-push on-ductile iron-200 psi-AWWA C111/C500";

/// Adds a 4 inch 11.25° elbow (see [`add_pressure_part`]).
pub fn add_pressure_fitting(doc: &mut CadDocument, network: Handle, position: Vector3, rotation: f64) -> Result<Handle, DxfError> {
    add_pressure_part(doc, network, ELBOW_4, position, rotation)
}

/// Adds a 6 inch gate valve (see [`add_pressure_part`]).
pub fn add_pressure_appurtenance(doc: &mut CadDocument, network: Handle, position: Vector3, rotation: f64) -> Result<Handle, DxfError> {
    add_pressure_part(doc, network, VALVE_6, position, rotation)
}

/// Adds a 4 inch tee (see [`add_pressure_part`]).
pub fn add_pressure_tee(doc: &mut CadDocument, network: Handle, position: Vector3, rotation: f64) -> Result<Handle, DxfError> {
    add_pressure_part(doc, network, TEE_4, position, rotation)
}

/// Adds a 14 x 6 inch cross (see [`add_pressure_part`]).
pub fn add_pressure_cross(doc: &mut CadDocument, network: Handle, position: Vector3, rotation: f64) -> Result<Handle, DxfError> {
    add_pressure_part(doc, network, CROSS_14, position, rotation)
}

/// The first reference of a pressure part naming a connected part (port
/// `k` at `CONNECTED + k`).
const CONNECTED: usize = 8;

/// Connects port `port_a` of the pressure part `a` to port `port_b` of
/// `b` (pipes, fittings or appurtenances of one network; a pipe's ports
/// are its start, 0, and end, 1). Both ports must be open and at the same
/// place (within 1e-6); the parts are not moved or turned. Each port
/// records the index of the port it meets (`BL`, -1 when open) and the
/// part it meets is the reference `8 + port`; every other bit of the two
/// records is kept.
pub fn connect_pressure_parts(doc: &mut CadDocument, a: Handle, port_a: usize, b: Handle, port_b: usize) -> Result<(), DxfError> {
    if a == b {
        return Err(invalid("a pressure part cannot connect to itself"));
    }
    let mut at = Vec::new();
    for (part, port) in [(a, port_a), (b, port_b)] {
        let e = arch_entity(doc, part).ok_or_else(|| invalid("not a pressure part"))?;
        let ports = e.pressure_ports().ok_or_else(|| invalid("not a pressure part"))?;
        let p = *ports.get(port).ok_or_else(|| invalid("the part has no such port"))?;
        if p.connected_port >= 0 || e.typed_handles().get(CONNECTED + port).is_none_or(|r| r.0 != 0) {
            return Err(invalid("the port is connected already"));
        }
        at.push((e.typed_handles()[0].0, p.position));
    }
    if at[0].0 != at[1].0 {
        return Err(invalid("the parts are not in one network"));
    }
    if at[0].1.distance(&at[1].1) > 1e-6 {
        return Err(invalid("the ports are not at the same place"));
    }
    set_port(doc, a, port_a, b, port_b)?;
    set_port(doc, b, port_b, a, port_a)
}

/// Rewrites `part` with its port `port` connected to port `other_port` of
/// `other`.
fn set_port(doc: &mut CadDocument, part: Handle, port: usize, other: Handle, other_port: usize) -> Result<(), DxfError> {
    let refused = || invalid("the part's record is not laid out as a pressure part");
    let a = arch_entity(doc, part).ok_or_else(refused)?;
    let (from, to) = a.pressure_ports().and_then(|p| p.get(port).map(|p| p.connected_at)).ok_or_else(refused)?;
    let (data, start, end) = a.main_stream().ok_or_else(refused)?;
    let bit = |p: usize| data[p / 8] >> (7 - p % 8) & 1 == 1;
    let (head, tail): (Vec<bool>, Vec<bool>) = ((start..from).map(bit).collect(), (to..end).map(bit).collect());
    let mut refs: Vec<Ref> = a.typed_handles().into_iter().map(typed).collect();
    *refs.get_mut(CONNECTED + port).ok_or_else(refused)? = soft_pointer(other.value());
    let (dxf, strings) = (a.dxf_name.clone(), a.decode().strings);
    let common = doc.get_entity(part).ok_or_else(refused)?.common().clone();
    put_entity(
        doc,
        &dxf,
        common,
        |w| {
            head.iter().for_each(|b| w.write_bit(*b));
            w.write_bit_long(other_port as i32);
            tail.iter().for_each(|b| w.write_bit(*b));
        },
        Body { refs, strings },
    )?;
    Ok(())
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
        assert_eq!(blocks, vec![placed(ELBOW_4).unwrap().1.texts[5].to_string(), format!("{}_3", placed(ELBOW_4).unwrap().1.texts[5])]);
    }

    #[test]
    fn pressure_parts_connect_port_to_port() {
        let v = Vector3::new;
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let n = add_pressure_network(&mut doc, "PRC", None).unwrap();
        let tee = add_pressure_tee(&mut doc, n, v(200.0, 0.0, 8.0), 0.0).unwrap();
        let at = |doc: &CadDocument, h: Handle, k: usize| arch_entity(doc, h).unwrap().pressure_ports().unwrap()[k].position;
        let branch = at(&doc, tee, 2);
        assert_eq!(branch, v(200.375, 0.0, 8.0));
        let pipe = add_pressure_pipe(&mut doc, n, branch, v(300.0, 0.0, 7.0)).unwrap();
        let arc = add_pressure_arc_pipe(&mut doc, n, v(0.0, 0.0, 10.0), v(100.0, 0.0, 10.0), -0.4).unwrap();
        assert!(connect_pressure_parts(&mut doc, pipe, 1, tee, 2).is_err(), "ports apart");
        connect_pressure_parts(&mut doc, tee, 2, pipe, 0).unwrap();
        assert!(connect_pressure_parts(&mut doc, tee, 2, arc, 0).is_err(), "port taken");
        let path = std::env::temp_dir().join(format!("civil-pressure-connect-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let p = arch_entity(&back, pipe).unwrap().pressure_pipe().unwrap();
        assert_eq!((p.start_connection, p.end_connection), (Some((tee.value(), 2)), None));
        let ports = arch_entity(&back, tee).unwrap().pressure_ports().unwrap();
        assert_eq!(ports.iter().map(|p| p.connected_port).collect::<Vec<_>>(), vec![-1, -1, 0]);
        assert_eq!(arch_entity(&back, tee).unwrap().typed_handles()[CONNECTED + 2].0, pipe.value());
        let a = arch_entity(&back, arc).unwrap();
        assert_eq!(a.pressure_pipe().unwrap().bulge, -0.4);
        // The start tangent of the sample's curve, (50, 20) its middle.
        let d = a.pressure_ports().unwrap()[0].direction;
        assert!((d.x + 0.7241379310344828).abs() < 1e-12 && (d.y + 0.6896551724137931).abs() < 1e-12);
    }

    #[test]
    fn every_size_has_its_own_key_and_writes() {
        let all = pressure_part_descriptions();
        assert_eq!(all.len(), 12 + 48 + 71 + 56 + 7);
        let keys: std::collections::HashSet<String> = KINDS.iter().flat_map(|k| k.sizes.iter().map(move |s| s.key(k))).collect();
        assert_eq!(keys.len(), 182);
        // The sample's key of the 4 inch 11.25° elbow.
        let (kind, size) = placed(ELBOW_4).unwrap();
        assert!(size.key(kind).starts_with("11.25elbow24_in_x_4_in22350AWWA_C111_C1522elbow_4_in_11.25_degree"));
        assert!(size.key(kind).ends_with("611746E2_B19E_4F8E_ACF7_F12491879819Fitting_DomainElbow"));
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let n = add_pressure_network(&mut doc, "PR", None).unwrap();
        for k in KINDS {
            let s = k.sizes.last().unwrap();
            let h = add_pressure_part(&mut doc, n, s.texts[2], Vector3::new(0.0, 0.0, 0.0), 0.0).unwrap();
            assert_eq!(arch_entity(&doc, h).unwrap().pressure_ports().unwrap().len(), s.ports.len());
        }
    }
}
