//! Sites (`AECC_PARCEL` records named `AeccImpFace` without a site of
//! their own), their containers and the alignments on them.

use super::arrangement::{build, Arrangement, Input};
use super::alignment::{default_style as alignment_style, elements, solve, write_alignment, PathPoint, VertexCurve, ARC_EDGE};
use super::groups::bucket_insert;
use super::support::*;
use super::default_styles::{ensure_site_styles, KEY_FEATURE_LINE_STYLES, KEY_PARCEL_STYLES};
use super::tree::{ensure_alignment_tree, DEFAULT_STYLE_ID, KEY_STYLES};
use crate::entities::{EntityCommon, EntityType, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::types::{Handle, Vector2};
use crate::CadDocument;

/// Root key of the node that lists the sites.
const KEY_SITES: i32 = 2;
/// First text id given to a written site (kept apart from the other
/// writers' ids).
const FIRST_SITE_TAG: u32 = 0x8004_0001;

/// The default style of the style tree's collection `key`: its child with
/// the default style id (30), or its only child.
pub(super) fn default_style_of(doc: &CadDocument, key: i32) -> Option<u64> {
    let tree = read_node(doc, root_node(doc, KEY_STYLES)?)?;
    let head = tree.children.iter().find(|c| c.0 == key)?.1;
    let o = object(doc, Handle::new(head))?;
    let (data, start, end) = fields_of(o)?;
    let mut b = super::super::bits::Bits::new(data, start, end);
    b.bit_long()?;
    let n = usize::try_from(b.bit_long()?).ok()?;
    let keys = (0..n).map(|_| b.bit_long()).collect::<Option<Vec<_>>>()?;
    let at = keys.iter().position(|k| *k == DEFAULT_STYLE_ID).or((n == 1).then_some(0))?;
    o.decode().handles.get(at).copied().filter(|h| *h != 0)
}

/// The node of the sites:
///
/// ```text
/// BS 0x04BE, BL 2, BL 0 (no children), BL n, n x BL site text id,
/// BL 32, 3 bits 001
/// references: n x site (soft pointer), soft pointer 0, then the node's
///   own (hard pointers: the default parcel area and segment label styles)
/// texts: "", the site names
/// ```
pub(super) struct SiteNode {
    pub sites: Vec<(i32, u64, String)>,
    pub own: Vec<Ref>,
}

pub(super) fn read_site_node(doc: &CadDocument, node: Handle) -> Option<SiteNode> {
    let o = object(doc, node)?;
    let (data, start, end) = fields_of(o)?;
    let mut b = super::super::bits::Bits::new(data, start, end);
    (b.bit_long()? == KEY_SITES && b.bit_long()? == 0).then_some(())?;
    let n = usize::try_from(b.bit_long()?).ok()?;
    let keys = (0..n).map(|_| b.bit_long()).collect::<Option<Vec<_>>>()?;
    (b.bit_long()? == 32 && b.bits_value(3)? == 1 && b.pos == end).then_some(())?;
    let refs = o.typed_handles();
    let strings = o.decode().strings;
    (refs.len() > n && strings.len() == n + 1).then_some(())?;
    let kind = |k: u8| if k == 4 { DwgReferenceType::SoftPointer } else { DwgReferenceType::HardPointer };
    Some(SiteNode {
        sites: keys.into_iter().zip(refs.iter().map(|r| r.0)).zip(strings[1..].iter().cloned()).map(|((k, h), s)| (k, h, s)).collect(),
        own: refs[n + 1..].iter().map(|(h, k)| Ref(kind(*k), *h)).collect(),
    })
}

pub(super) fn write_site_node(doc: &mut CadDocument, node: Handle, owner: Handle, n: &SiteNode) -> Result<(), DxfError> {
    let mut refs: Vec<Ref> = n.sites.iter().map(|s| soft_pointer(s.1)).collect();
    refs.push(soft_pointer(0));
    refs.extend(n.own.iter().copied());
    let mut strings = vec![String::new()];
    strings.extend(n.sites.iter().map(|s| s.2.clone()));
    put_object(
        doc,
        "AECC_PARCEL_NODE",
        node,
        owner,
        Vec::new(),
        |w| {
            w.write_bit_long(KEY_SITES);
            w.write_bit_long(0);
            w.write_bit_long(n.sites.len() as i32);
            for s in &n.sites {
                w.write_bit_long(s.0);
            }
            w.write_bit_long(32);
            bits(w, "001");
        },
        Body { refs, strings },
    )
}

/// The site record:
///
/// ```text
/// BL 80, BL 0, BL 80, BL 80, BL 0, BL 0x10025F04, B 0, BL site text id,
/// 3 bits 101
/// RC 6 keys: 0x10008018, 0x10008017 (layers), 0x0D00800A BL 0,
///   0x10008016 (layer), 0x02008022 BD 0.1, 0x0200800B BD 5.0
/// BL 0 (one-byte form), 121 zero bits
/// 6 x BD 0, BL 0, BL 99, BL 0, BL 0, BL 1, BL 1, BL 1, B 0, BS 0x04BE
/// references: the three layers (hard pointers), soft pointer 0, the node
///   of the sites (hard), the container (hard ownership), the parcel style
///   (hard), hard pointer 0, the parcel style twice (hard)
/// texts: "AeccImpFace", "", name, description
/// ```
///
/// The run after the keys is the application's cache of what the site
/// holds: once an alignment joins the site it saves other values there,
/// but it opens a site written with the empty-site values whatever its
/// container holds (E-T1) and saves its own.
// ponytail: the 121 zero bits and the cache run are written as the
// application writes them for an empty site (WT01, kavsak); name their
// fields once a writer needs them.
pub(super) fn write_site(doc: &mut CadDocument, common: EntityCommon, tag: u32, name: &str, description: &str, layer: u64, node: Handle, container: Handle, style: u64) -> Result<Handle, DxfError> {
    let byte = |w: &mut crate::io::dwg::dwg_stream_writers::DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    let refs = vec![
        hard_pointer(layer),
        hard_pointer(layer),
        hard_pointer(layer),
        soft_pointer(0),
        hard_pointer(node.value()),
        Ref(DwgReferenceType::HardOwnership, container.value()),
        hard_pointer(style),
        hard_pointer(0),
        hard_pointer(style),
        hard_pointer(style),
    ];
    let strings = vec!["AeccImpFace".to_string(), String::new(), name.to_string(), description.to_string()];
    put_entity(
        doc,
        "AECC_PARCEL",
        common,
        |w| {
            for v in [80, 0, 80, 80, 0, 0x1002_5F04] {
                w.write_bit_long(v);
            }
            w.write_bit(false);
            w.write_bit_long(tag as i32);
            bits(w, "101");
            byte(w, 6);
            w.write_bit_long(0x1000_8018);
            w.write_bit_long(0x1000_8017);
            w.write_bit_long(0x0D00_800A);
            w.write_bit_long(0);
            w.write_bit_long(0x1000_8016);
            w.write_bit_long(0x0200_8022);
            w.write_bit_double(0.1);
            w.write_bit_long(0x0200_800B);
            w.write_bit_double(5.0);
            bl_byte(w, 0);
            bits(w, &"0".repeat(121));
            for _ in 0..6 {
                w.write_bit_double(0.0);
            }
            for v in [0, 99, 0, 0, 1, 1, 1] {
                w.write_bit_long(v);
            }
            w.write_bit(false);
            w.write_bit_short(VERSION);
        },
        Body { refs, strings },
    )
}

/// A member of a site: an alignment (its solved path, elevations 0), a
/// feature line (its points with their elevations) or a lot line (a
/// parcel segment, open or closed).
pub(super) struct Member {
    handle: u64,
    path: Vec<PathPoint>,
    elevations: Vec<f64>,
    /// Which points are elevation points (never the ends).
    elevation_points: Vec<bool>,
    kind: MemberKind,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum MemberKind {
    Alignment,
    FeatureLine,
    /// A parcel segment; a closed one joins its last point to the first.
    LotLine { closed: bool },
}

impl Member {
    fn closed(&self) -> bool {
        self.kind == MemberKind::LotLine { closed: true }
    }

    /// The path the member takes, closed back to its first point for a
    /// closed lot line.
    fn outline(&self) -> Vec<PathPoint> {
        let mut path = self.path.clone();
        if self.closed() {
            path.extend(self.path.first().map(|p| (p.0, None)));
        }
        path
    }
}

/// Edge flags of an alignment and of a feature line in a container.
const ALIGNMENT_EDGE: i32 = 0x30;
const FEATURE_LINE_EDGE: i32 = 0x1_0030;
/// Edge flags of a parcel segment; bit 1 marks a closed one.
const LOT_LINE_EDGE: i32 = 0x30;
/// Edge flag of an edge another member's crossing added a node to.
const CROSSED_EDGE: i32 = 0x80;
/// Point flag of a point on a node an earlier member made (WP08).
const SHARED_NODE: i32 = 0x80_0000;

/// The site's container (`AECC_FEATURE_STYLE` owned by the site): the
/// topology of its members, each an edge through its own nodes, and the
/// faces closed lot lines make.
///
/// ```text
/// BS 0x04BE, BL 0, BL 0 (no children), BL 0, BL 32, 7 bits 0111010,
/// elevation points: BL next free key, BL k, k x {BL key, BL 21, BD the
///   point's index in its feature line, 17 bits 10000000000101010,
///   BL key, BL 0, BL 0, BD elevation, B 0, BL 0} (keys 1..k in edge
///   order), BL 1, BL 0,
/// BL 0x80000001,
/// nodes: BL next free id, BL n, BL first id, n x {3BD (z the
///   elevation), 3 bits 010, BL next id} (ids per member: its ends, then
///   its inner points, a closed one's in path order; the records in hash
///   order: eight buckets by id mod 8, each in id order, WP15 and WP16
///   with 30 and 50 nodes; formerly read as seven buckets by
///   (id - 1) mod 7, the newest first in a bucket)
/// edges: BL next free id, BL m slots, per member BL 0, BL id (1..m),
///   BL flags (0x30 alignment, 0x10030 feature line, 0x31 closed lot
///   line), BL count, BL 0,
///   count x {BL kind (0 line, 1 arc, 2 last),
///   BL flags, BL node, BL left face, BL right face (0, 0 on the last
///   point), an arc: BD bulge, BL 0}, BL j, the keys of the
///   edge's j elevation points in order, BL 0 x 3 (a closed edge has no
///   last point: its last one starts the segment back to the first)
/// faces: BL next free id, BL count (0 without members), per face {BL id,
///   BL id, BL flags, BL loops, per loop {BL entry node, BL entry index}}
///   (see [`super::arrangement`])
/// BL 1, BL 1, BL p + 1 (p parcels), 10 x BL 1, BL p, BL 1 .. p,
///   5 x BL 0
/// references: hard pointer 0 per elevation point, per member its record
///   (soft pointer) and soft pointer 0, then (with members) the site and
///   soft pointer 0, per parcel its record and soft pointer 0, then the
///   default feature line style (hard)
/// texts: ""
/// ```
///
/// The points run as one chain through the members in edge order: a
/// point's flags are 2 (0x400400 for an elevation point), with 0x10 added
/// when another segment follows it (in its edge or a later one) and 0x20
/// when that is so or its edge is closed, 0x80 on the first point of an
/// edge after the first, 0x010F0000 for an arc; an open edge's last
/// point's are 10 (WT07, WT10 to WT13, WF01 to WF06; written for their
/// members, the containers equal the samples' bit for bit).
///
/// Members that meet (lot lines only) are split where they meet: the
/// nodes another member's crossing adds to an edge have flags 0x400
/// instead of 2 and mark the edge with 0x80; a point on an earlier node
/// carries 0x800000, and 0x80 on a first point only marks an edge that
/// starts off every earlier line. The faces, their entries and flags are
/// those [`build`] gives (WP03 to WP20: written for their segments the
/// containers equal the samples' but for edge flag 0x800, which the
/// application sets on an edge read from a file when another member joins,
/// and the newest member's point flags 0x10 and 0x20 once it meets
/// another).
pub(super) fn write_container(doc: &mut CadDocument, handle: Handle, site: Handle, members: &[Member], parcels: &[u64], style: u64) -> Result<Arrangement, DxfError> {
    let inputs: Vec<Input> = members.iter().map(|m| Input { path: &m.path, elevations: &m.elevations, closed: m.closed() }).collect();
    let arrangement = build(&inputs).map_err(invalid)?;
    let elevation_points = members.iter().map(|m| m.elevation_points.iter().filter(|x| **x).count()).sum();
    let mut refs: Vec<Ref> = vec![hard_pointer(0); elevation_points];
    refs.extend(members.iter().flat_map(|m| [soft_pointer(m.handle), soft_pointer(0)]));
    if !members.is_empty() {
        refs.extend([soft_pointer(site.value()), soft_pointer(0)]);
    }
    refs.extend(parcels.iter().flat_map(|p| [soft_pointer(*p), soft_pointer(0)]));
    refs.push(hard_pointer(style));
    let a = &arrangement;
    put_object(
        doc,
        "AECC_FEATURE_STYLE",
        handle,
        site,
        Vec::new(),
        |w| {
            for v in [0, 0, 0, 32] {
                w.write_bit_long(v);
            }
            bits(w, "0111010");
            let mut key = 0;
            let mut table = Vec::new();
            for m in members {
                for (i, _) in m.elevation_points.iter().enumerate().filter(|e| *e.1) {
                    key += 1;
                    table.push((key, i, m.elevations[i]));
                }
            }
            w.write_bit_long(key + 1);
            w.write_bit_long(key);
            for &(key, index, z) in &table {
                w.write_bit_long(key);
                w.write_bit_long(21);
                w.write_bit_double(index as f64);
                bits(w, "10000000000101010");
                for v in [key, 0, 0] {
                    w.write_bit_long(v);
                }
                w.write_bit_double(z);
                w.write_bit(false);
                w.write_bit_long(0);
            }
            w.write_bit_long(1);
            w.write_bit_long(0);
            w.write_bit_long(0x8000_0001_u32 as i32);
            // The node records in the application's hash order: eight
            // buckets by id mod 8, each in id order.
            let nodes = a.nodes.len();
            let mut ids: Vec<i32> = (1..=nodes as i32).collect();
            ids.sort_by_key(|id| (id % 8, *id));
            w.write_bit_long(nodes as i32 + 1);
            w.write_bit_long(nodes as i32);
            w.write_bit_long(ids.first().copied().unwrap_or(0));
            for (k, id) in ids.iter().enumerate() {
                let (p, z) = a.nodes[*id as usize - 1];
                for v in [p.x, p.y, z] {
                    w.write_bit_double(v);
                }
                bits(w, "010");
                w.write_bit_long(ids.get(k + 1).copied().unwrap_or(0));
            }
            w.write_bit_long(members.len() as i32 + 1);
            w.write_bit_long(members.len() as i32);
            let mut next_key = 1;
            for (e, (m, edge)) in members.iter().zip(&a.edges).enumerate() {
                let n = edge.vertices.len();
                let closed = m.closed();
                let flags = match m.kind {
                    MemberKind::Alignment => ALIGNMENT_EDGE,
                    MemberKind::FeatureLine => FEATURE_LINE_EDGE,
                    MemberKind::LotLine { closed } => LOT_LINE_EDGE | closed as i32,
                } | if edge.crossed { CROSSED_EDGE } else { 0 };
                for v in [0, e as i32 + 1, flags, n as i32, 0] {
                    w.write_bit_long(v);
                }
                let last_member = e + 1 == members.len();
                let count = m.elevation_points.iter().filter(|x| **x).count() as i32;
                let keys: Vec<i32> = (next_key..next_key + count).collect();
                next_key += count;
                let mut own = 0;
                for (i, v) in edge.vertices.iter().enumerate() {
                    let elevation_point = !v.inserted && m.elevation_points[own];
                    own += !v.inserted as usize;
                    let last = i + 1 == n && !closed;
                    let arc = v.bulge.filter(|_| !last);
                    let follows = i + 2 < n || (closed && i + 1 < n) || !last_member;
                    let shared = if v.shared { SHARED_NODE } else { 0 };
                    let flags = if last {
                        10 | shared
                    } else {
                        (if elevation_point { 0x40_0400 } else if v.inserted { 0x400 } else { 2 }) | if follows { 0x30 } else if closed { 0x20 } else { 0 } | if i == 0 && edge.loose_start { 0x80 } else { 0 } | if arc.is_some() { ARC_EDGE } else { 0 } | shared
                    };
                    let (left, right) = v.faces;
                    for x in [if last { 2 } else if arc.is_some() { 1 } else { 0 }, flags, v.node, left, right] {
                        w.write_bit_long(x);
                    }
                    if let Some(bulge) = arc {
                        w.write_bit_double(bulge);
                        w.write_bit_long(0);
                    }
                }
                w.write_bit_long(keys.len() as i32);
                for v in keys.iter().copied().chain([0, 0, 0]) {
                    w.write_bit_long(v);
                }
            }
            let p = a.faces.len().saturating_sub(1) as i32;
            w.write_bit_long(p + 2);
            w.write_bit_long(a.faces.len() as i32);
            for (i, f) in a.faces.iter().enumerate() {
                // The outside face's flags lose 8 once a member has an
                // elevation point.
                let flags = if i == 0 && members.iter().any(|m| m.elevation_points.contains(&true)) { 6 } else { f.flags };
                for v in [i as i32 + 1, i as i32 + 1, flags, f.entries.len() as i32] {
                    w.write_bit_long(v);
                }
                for &(node, k) in &f.entries {
                    w.write_bit_long(node);
                    w.write_bit_long(k);
                }
            }
            let tail = [1, 1, p + 1].into_iter().chain([1; 10]).chain([p]).chain(1..=p);
            for v in tail.chain([0; 5]) {
                w.write_bit_long(v);
            }
        },
        Body { refs, strings: vec![String::new()] },
    )?;
    Ok(arrangement)
}

/// A site's container, the members it lists and the parcels of its faces
/// (face 2 on), when the container is one this crate writes: writing it
/// from what is read gives back its record.
fn members_of(doc: &mut CadDocument, site: Handle) -> Result<(Handle, Vec<Member>, Vec<u64>, u64), DxfError> {
    let refused = || invalid("the site is not one this crate writes");
    let Some(EntityType::Extended(x)) = doc.get_entity(site) else { return Err(invalid("site not found")) };
    let ExtendedEntityData::Arch(a) = &x.data else { return Err(refused()) };
    if a.dxf_name != "AECC_PARCEL" || a.face_name().is_none() || a.parcel_site().is_some() {
        return Err(invalid("the record is not a site"));
    }
    let container = Handle::new(a.typed_handles().get(5).map(|r| r.0).ok_or_else(refused)?);
    let o = object(doc, container).ok_or_else(refused)?.clone();
    let refs = o.typed_handles();
    let style = refs.last().map(|r| r.0).ok_or_else(refused)?;
    let edges = if refs.len() == 1 { Vec::new() } else { o.topology().ok_or_else(refused)?.edges };
    let elevation_points = edges.iter().flatten().filter(|v| v.is_elevation_point()).count();
    let handles: Vec<u64> = refs.iter().skip(elevation_points).step_by(2).take(edges.len()).map(|r| r.0).collect();
    // The parcels follow the members and the site, in face order.
    let first_parcel = elevation_points + 2 * edges.len() + 2;
    let parcels: Vec<u64> = refs.iter().skip(first_parcel).step_by(2).take(refs.len().saturating_sub(first_parcel + 1) / 2).map(|r| r.0).collect();
    let dxf = |h: u64| match doc.get_entity(Handle::new(h)) {
        Some(EntityType::Extended(x)) => match &x.data {
            ExtendedEntityData::Arch(a) => Some(a.dxf_name.clone()),
            _ => None,
        },
        _ => None,
    };
    let members: Vec<Member> = edges
        .iter()
        .zip(&handles)
        .map(|(run, h)| {
            // A closed edge is read with a copy of its first vertex last;
            // an open edge's last vertex carries flag 8.
            let closed = run.last().is_some_and(|v| v.flags & 8 == 0);
            let kind = match dxf(*h)?.as_str() {
                "AECC_ALIGNMENT" => MemberKind::Alignment,
                "AECC_FEATURE_LINE" => MemberKind::FeatureLine,
                "AECC_PARCEL_SEGMENT" => MemberKind::LotLine { closed },
                _ => return None,
            };
            let run = if closed { &run[..run.len() - 1] } else { &run[..] };
            // Nodes other members' crossings added are not the member's.
            let own: Vec<_> = run.iter().filter(|v| v.flags & (0x400 | 2) != 0x400 || v.is_elevation_point()).collect();
            Some(Member {
                handle: *h,
                path: own.iter().map(|v| (v.point, (v.bulge != 0.0).then_some(v.bulge))).collect(),
                elevations: own.iter().map(|v| v.z).collect(),
                elevation_points: own.iter().map(|v| v.is_elevation_point()).collect(),
                kind,
            })
        })
        .collect::<Option<_>>()
        .ok_or_else(refused)?;
    let bits_of = |doc: &CadDocument| object(doc, container).and_then(|o| o.main_stream().map(|(d, s, e)| (s..e).map(|i| d[i / 8] >> (7 - i % 8) & 1).collect::<Vec<u8>>()));
    let before = bits_of(doc);
    let rewritten = write_container(doc, container, site, &members, &parcels, style);
    let faces = rewritten.as_ref().map_or(0, |a| a.faces.len());
    if rewritten.is_err() || bits_of(doc) != before || parcels.len() != faces.saturating_sub(1) {
        doc.objects.insert(container, crate::objects::ObjectType::Arch(o));
        return Err(refused());
    }
    Ok((container, members, parcels, style))
}

/// Adds an empty site named `name` with `description`. It takes the
/// drawing's default parcel style and default feature line style (the
/// default styles of the style tree's parcel and feature line style
/// collections, id 30; a drawing without them gets the default ones) and
/// joins the node of the sites, which a drawing without one gets (with no
/// label styles).
pub fn add_site(doc: &mut CadDocument, name: &str, description: &str) -> Result<Handle, DxfError> {
    if name.is_empty() {
        return Err(invalid("a site needs a name"));
    }
    let layer = doc.layers.get("0").map(|l| l.handle).ok_or_else(|| invalid("drawing has no layer 0"))?;
    let (node, mut list) = match root_node(doc, KEY_SITES) {
        Some(node) => (node, read_site_node(doc, node).ok_or_else(|| invalid("the node of sites cannot be read"))?),
        None => (doc.allocate_handle(), SiteNode { sites: Vec::new(), own: vec![hard_pointer(0); 3] }),
    };
    if list.sites.iter().any(|s| s.2.eq_ignore_ascii_case(name)) {
        return Err(invalid("a site of that name exists"));
    }
    ensure_site_styles(doc)?;
    let style = default_style_of(doc, KEY_PARCEL_STYLES).unwrap_or(0);
    let line_style = default_style_of(doc, KEY_FEATURE_LINE_STYLES).unwrap_or(0);
    let root = ensure_civil(doc)?;
    let tag = (FIRST_SITE_TAG..).find(|t| list.sites.iter().all(|s| s.0 as u32 != *t)).unwrap_or(FIRST_SITE_TAG);
    let common = EntityCommon {
        handle: doc.allocate_handle(),
        owner_handle: doc.header.model_space_block_handle,
        layer: "0".into(),
        layer_handle: Some(layer),
        ..EntityCommon::default()
    };
    let site = common.handle;
    let container = doc.allocate_handle();
    write_container(doc, container, site, &[], &[], line_style)?;
    write_site(doc, common, tag, name, description, layer.value(), node, container, style)?;
    bucket_insert(&mut list.sites, (tag as i32, site.value(), name.to_string()), |s| s.0);
    let owner = object(doc, node).map_or(root, |o| o.owner);
    write_site_node(doc, node, owner, &list)?;
    if root_node(doc, KEY_SITES).is_none() {
        set_root_entry(doc, KEY_SITES, node)?;
    }
    Ok(site)
}

/// Whether two solved paths cross or touch (arcs taken by their chords
/// and their middle points).
fn paths_meet(a: &[PathPoint], b: &[PathPoint]) -> bool {
    let points = |p: &[PathPoint]| -> Vec<Vector2> {
        let mut out = Vec::new();
        for w in p.windows(2) {
            out.push(w[0].0);
            if let Some(bulge) = w[0].1 {
                // The arc's middle point: off the chord by its sagitta.
                let (c, d) = (w[0].0, w[1].0);
                let chord = d - c;
                let normal = Vector2::new(-chord.y, chord.x) * (1.0 / chord.length().max(1e-300));
                out.push((c + d) * 0.5 - normal * (bulge * chord.length() / 2.0));
            }
        }
        out.extend(p.last().map(|q| q.0));
        out
    };
    let (pa, pb) = (points(a), points(b));
    let cross = |o: Vector2, p: Vector2, q: Vector2| (p - o).cross(&(q - o));
    let on = |p: Vector2, s: &[Vector2]| p.x >= s[0].x.min(s[1].x) - 1e-9 && p.x <= s[0].x.max(s[1].x) + 1e-9 && p.y >= s[0].y.min(s[1].y) - 1e-9 && p.y <= s[0].y.max(s[1].y) + 1e-9;
    pa.windows(2).any(|s| {
        pb.windows(2).any(|t| {
            let (d1, d2) = (cross(t[0], t[1], s[0]), cross(t[0], t[1], s[1]));
            let (d3, d4) = (cross(s[0], s[1], t[0]), cross(s[0], s[1], t[1]));
            let zero = |d: f64| d.abs() < 1e-9;
            (d1 * d2 < 0.0 && d3 * d4 < 0.0) || (zero(d1) && on(s[0], t)) || (zero(d2) && on(s[1], t)) || (zero(d3) && on(t[0], s)) || (zero(d4) && on(t[1], s))
        })
    })
}

/// Adds an alignment like [`super::add_alignment_with_curves`] on `site`
/// instead of siteless: its edge joins the site's container. The
/// alignment must not cross or touch the site's other members (the site's
/// parcels and the splits at crossings are not written), and the site must
/// be one this crate writes.
pub fn add_alignment_on_site(doc: &mut CadDocument, site: Handle, name: &str, vertices: &[Vector2], curves: &[VertexCurve], layer: Option<&str>) -> Result<Handle, DxfError> {
    if vertices.len() < 2 || vertices.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) || vertices.windows(2).any(|w| w[0] == w[1]) {
        return Err(invalid("an alignment needs two or more distinct finite vertices"));
    }
    if curves.len() + 2 != vertices.len() {
        return Err(invalid("an alignment needs one curve per inner vertex"));
    }
    let elements = elements(vertices, curves);
    let path = solve(vertices, curves, &elements).ok_or_else(|| invalid("a curve has no turn or is not positive, or the curves do not fit on the lines"))?;
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let (container, mut members, parcels, line_style) = members_of(doc, site)?;
    if members.iter().any(|m| paths_meet(&m.outline(), &path)) {
        return Err(invalid("the alignment crosses or touches another member of the site"));
    }
    if alignment_style(doc).is_none() {
        ensure_alignment_tree(doc)?;
    }
    let style = alignment_style(doc).ok_or_else(|| invalid("drawing has no alignment styles"))?;
    let common = EntityCommon {
        handle: doc.allocate_handle(),
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    members.push(Member { handle: common.handle.value(), elevations: vec![0.0; path.len()], elevation_points: vec![false; path.len()], path, kind: MemberKind::Alignment });
    write_container(doc, container, site, &members, &parcels, line_style)?;
    write_alignment(doc, common, name, &elements, style, container.value(), members.len() as i32, None, &[], &[], &[])
}

/// The feature line record (`AECC_FEATURE_LINE`; its points are its edge
/// in the site's container):
///
/// ```text
/// BL 80, BL 0, BL 80, BL 80, BL 0, BL 0x10025F04, 6 bits 010010,
/// 128 zero bits, BL 0, BL edge id, 7 bits 1101000, BS 0x04BE
/// references: soft pointer 0, the container twice (hard), soft pointer 0,
///   the feature line style (hard)
/// texts: "AeccImpFeatureLine", "", "", name
/// ```
// ponytail: the 128 zero bits and the 7-bit run are the same in every
// sample (WF01 to WF06); name them once a feature line varies them.
fn write_feature_line(doc: &mut CadDocument, common: EntityCommon, name: &str, edge: i32, container: Handle, style: u64) -> Result<Handle, DxfError> {
    let refs = vec![soft_pointer(0), hard_pointer(container.value()), hard_pointer(container.value()), soft_pointer(0), hard_pointer(style)];
    let strings = vec!["AeccImpFeatureLine".to_string(), String::new(), String::new(), name.to_string()];
    put_entity(
        doc,
        "AECC_FEATURE_LINE",
        common,
        |w| {
            for v in [80, 0, 80, 80, 0, 0x1002_5F04] {
                w.write_bit_long(v);
            }
            bits(w, "010010");
            bits(w, &"0".repeat(128));
            w.write_bit_long(0);
            w.write_bit_long(edge);
            bits(w, "1101000");
            w.write_bit_short(VERSION);
        },
        Body { refs, strings },
    )
}

/// A point of a feature line: a PI or an elevation point (a point that
/// only sets an elevation between two PIs), with the bulge of the segment
/// that starts there (0 for a straight segment; positive counterclockwise,
/// the tangent of a quarter of the arc's angle, as a polyline stores it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FeatureLineVertex {
    pub point: crate::types::Vector3,
    pub bulge: f64,
    pub elevation_point: bool,
}

/// Adds a feature line through `points` (x, y, elevation; straight
/// segments, every point a PI) on `site`; see [`add_feature_line_vertices`].
pub fn add_feature_line(doc: &mut CadDocument, site: Handle, name: &str, points: &[crate::types::Vector3], layer: Option<&str>) -> Result<Handle, DxfError> {
    let vertices: Vec<FeatureLineVertex> = points.iter().map(|&point| FeatureLineVertex { point, bulge: 0.0, elevation_point: false }).collect();
    add_feature_line_vertices(doc, site, name, &vertices, layer)
}

/// Adds a feature line through `vertices` on `site`: its edge joins the
/// site's container. Like an alignment on a site it must not cross or
/// touch the site's other members, and the site must be one this crate
/// writes. The ends are PIs; an elevation point on an arc splits it, so
/// the bulges on its two sides are those of the two parts. It takes the
/// feature line style the site's container uses.
pub fn add_feature_line_vertices(doc: &mut CadDocument, site: Handle, name: &str, vertices: &[FeatureLineVertex], layer: Option<&str>) -> Result<Handle, DxfError> {
    let n = vertices.len();
    if n < 2 || vertices.iter().any(|v| !v.point.x.is_finite() || !v.point.y.is_finite() || !v.point.z.is_finite() || !v.bulge.is_finite()) || vertices.windows(2).any(|w| w[0].point.x == w[1].point.x && w[0].point.y == w[1].point.y) {
        return Err(invalid("a feature line needs two or more finite points, apart in plan"));
    }
    if vertices[0].elevation_point || vertices[n - 1].elevation_point {
        return Err(invalid("the ends of a feature line are PIs"));
    }
    if name.is_empty() {
        return Err(invalid("a feature line needs a name"));
    }
    let path: Vec<PathPoint> = vertices.iter().enumerate().map(|(i, v)| (Vector2::new(v.point.x, v.point.y), Some(v.bulge).filter(|b| *b != 0.0 && i + 1 < n))).collect();
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let (container, mut members, parcels, line_style) = members_of(doc, site)?;
    if members.iter().any(|m| paths_meet(&m.outline(), &path)) {
        return Err(invalid("the feature line crosses or touches another member of the site"));
    }
    let common = EntityCommon {
        handle: doc.allocate_handle(),
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    members.push(Member {
        handle: common.handle.value(),
        path,
        elevations: vertices.iter().map(|v| v.point.z).collect(),
        elevation_points: vertices.iter().map(|v| v.elevation_point).collect(),
        kind: MemberKind::FeatureLine,
    });
    write_container(doc, container, site, &members, &parcels, line_style)?;
    write_feature_line(doc, common, name, members.len() as i32, container, line_style)
}

/// Label style tree path (under the label node) of the default parcel
/// area label style.
const PARCEL_AREA_LABELS: [i32; 3] = [2, 0x0F00_C032, DEFAULT_STYLE_ID];

/// The parcel segment record (`AECC_PARCEL_SEGMENT`, a lot line; its
/// points are its edge in the site's container):
///
/// ```text
/// BL 80, BL 0, BL 80, BL 80, BL 0, BL 0x10025F04, 6 bits 010010,
/// 128 zero bits, BL 0, BL edge id, 10 bits 1101010100, BS 0x04BE
/// references: soft pointer 0, the container twice (hard), soft pointer 0,
///   hard pointer 0
/// texts: "AeccImpLotLine", "", ""
/// ```
// ponytail: the 128 zero bits and the 7-bit run are the same in every
// sample (WP03 to WP14); name them once a segment varies them.
fn write_lot_line(doc: &mut CadDocument, common: EntityCommon, edge: i32, container: Handle) -> Result<Handle, DxfError> {
    let refs = vec![soft_pointer(0), hard_pointer(container.value()), hard_pointer(container.value()), soft_pointer(0), hard_pointer(0)];
    let strings = vec!["AeccImpLotLine".to_string(), String::new(), String::new()];
    put_entity(
        doc,
        "AECC_PARCEL_SEGMENT",
        common,
        |w| {
            for v in [80, 0, 80, 80, 0, 0x1002_5F04] {
                w.write_bit_long(v);
            }
            bits(w, "010010");
            bits(w, &"0".repeat(128));
            w.write_bit_long(0);
            w.write_bit_long(edge);
            bits(w, "1101010100");
            w.write_bit_short(VERSION);
        },
        Body { refs, strings },
    )
}

/// The parcel record (`AECC_PARCEL` named `AeccImpFace` with a site):
///
/// ```text
/// BL 80, BL 0, BL 80, BL 80, BL 0, BL 0x10025F04, B 0, BL 0, 3 bits 101
/// RC 3 keys: 0x0D00800A BL 0, 0x02008022 BD 0.1, 0x0200800B BD 1/24
/// BL 0 (one-byte form), 121 zero bits
/// BL number, BL 0, BL 0, BD 0, 3BD label location, BL 80, BL 1,
/// 3BD label location, BL 2, BL 1, BL face, BL 0, B 0, BS 0x04BE
/// references: soft pointer 0, the site (hard), hard ownership 0, the
///   parcel style (hard), the area label style (hard)
/// texts: "AeccImpFace", "", "" (the application names the parcel from
///   its style and number)
/// ```
///
/// The label location is a point inside the parcel (see `label_point`);
/// a parcel's area and
/// boundary come from its face in the site's container (WP03 to WP14:
/// parcels of other areas have the same record).
fn write_parcel(doc: &mut CadDocument, common: EntityCommon, site: Handle, number: i32, face: i32, label: Vector2, style: u64, label_style: u64) -> Result<Handle, DxfError> {
    let byte = |w: &mut crate::io::dwg::dwg_stream_writers::DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    let refs = vec![soft_pointer(0), hard_pointer(site.value()), Ref(DwgReferenceType::HardOwnership, 0), hard_pointer(style), hard_pointer(label_style)];
    let strings = vec!["AeccImpFace".to_string(), String::new(), String::new()];
    put_entity(
        doc,
        "AECC_PARCEL",
        common,
        |w| {
            for v in [80, 0, 80, 80, 0, 0x1002_5F04] {
                w.write_bit_long(v);
            }
            w.write_bit(false);
            w.write_bit_long(0);
            bits(w, "101");
            byte(w, 3);
            w.write_bit_long(0x0D00_800A);
            w.write_bit_long(0);
            w.write_bit_long(0x0200_8022);
            w.write_bit_double(0.1);
            w.write_bit_long(0x0200_800B);
            w.write_bit_double(1.0 / 24.0);
            bl_byte(w, 0);
            bits(w, &"0".repeat(121));
            for v in [number, 0, 0] {
                w.write_bit_long(v);
            }
            w.write_bit_double(0.0);
            for round in 0..2 {
                for v in [label.x, label.y, 0.0] {
                    w.write_bit_double(v);
                }
                for v in if round == 0 { [80, 1] } else { [2, 1] } {
                    w.write_bit_long(v);
                }
            }
            w.write_bit_long(face);
            w.write_bit_long(0);
            w.write_bit(false);
            w.write_bit_short(VERSION);
        },
        Body { refs, strings },
    )
}

/// Adds a lot line (a parcel segment) through `vertices` on `site`, closed
/// back to the first vertex when `closed`, and writes the parcels of the
/// faces it makes. With `meet`, the line may cross or touch the site's
/// other lot lines: each is split where they meet, the faces they cut are
/// split and the parcels of the parts are written (every parcel's label
/// moves into what is left of its face). Returns the segment and the new
/// parcels.
fn add_lot_line_on(doc: &mut CadDocument, site: Handle, points: &[(Vector2, f64)], closed: bool, meet: bool, layer: Option<&str>) -> Result<(Handle, Vec<Handle>), DxfError> {
    let n = points.len();
    let vertices: Vec<Vector2> = points.iter().map(|p| p.0).collect();
    let path: Vec<PathPoint> = points.iter().enumerate().map(|(i, p)| (p.0, Some(p.1).filter(|b| *b != 0.0 && (closed || i + 1 < n)))).collect();
    if points.iter().any(|p| !p.1.is_finite()) {
        return Err(invalid("a bulge is not finite"));
    }
    let area2: f64 = (0..n).map(|i| vertices[i].cross(&vertices[(i + 1) % n])).sum();
    if vertices.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) || (closed && (n < 3 || area2.abs() < 1e-9)) || n < 2 {
        return Err(invalid(if closed { "a closed lot line needs three or more finite vertices enclosing an area" } else { "a lot line needs two or more finite vertices" }));
    }
    let mut outline = path.clone();
    if closed {
        outline.push((vertices[0], None));
    }
    let edges: Vec<[Vector2; 2]> = outline.windows(2).map(|w| [w[0].0, w[1].0]).collect();
    let crossing = edges.iter().enumerate().any(|(i, a)| {
        edges.iter().enumerate().skip(i + 2).any(|(j, b)| !(closed && i == 0 && j + 1 == edges.len()) && paths_meet(&[(a[0], None), (a[1], None)], &[(b[0], None), (b[1], None)]))
    });
    if crossing || edges.iter().any(|e| e[0] == e[1]) {
        return Err(invalid("a lot line crosses or touches itself"));
    }
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let (container, mut members, mut parcels, line_style) = members_of(doc, site)?;
    let met = members.iter().filter(|m| paths_meet(&m.outline(), &outline)).map(|m| m.kind).collect::<Vec<_>>();
    let arcs = |p: &[PathPoint]| p.iter().any(|q| q.1.is_some());
    let met_arcs = members.iter().any(|m| arcs(&m.path) && paths_meet(&m.outline(), &outline));
    if met.iter().any(|k| !meet || !matches!(k, MemberKind::LotLine { .. })) || (!met.is_empty() && (arcs(&path) || met_arcs)) {
        return Err(invalid("the lot line crosses or touches a member it may not meet (or meets with an arc)"));
    }
    let style = match doc.get_entity(site) {
        Some(EntityType::Extended(x)) => match &x.data {
            ExtendedEntityData::Arch(a) => a.typed_handles().get(6).map_or(0, |r| r.0),
            _ => 0,
        },
        _ => 0,
    };
    let label_style = super::labels::default_label_style(doc, PARCEL_AREA_LABELS).unwrap_or(0);
    let common = |doc: &mut CadDocument| EntityCommon {
        handle: doc.allocate_handle(),
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    let line = common(doc);
    members.push(Member { handle: line.handle.value(), path, elevations: vec![0.0; n], elevation_points: vec![false; n], kind: MemberKind::LotLine { closed } });
    let inputs: Vec<Input> = members.iter().map(|m| Input { path: &m.path, elevations: &m.elevations, closed: m.closed() }).collect();
    let faces = build(&inputs).map_err(invalid)?.faces.len();
    let old = parcels.len();
    let mut made = Vec::new();
    for _ in old + 1..faces {
        let c = common(doc);
        parcels.push(c.handle.value());
        made.push(c);
    }
    let arrangement = write_container(doc, container, site, &members, &parcels, line_style)?;
    let segment = write_lot_line(doc, line, members.len() as i32, container)?;
    // The earlier parcels keep their records; their labels follow their faces.
    if !met.is_empty() {
        for (i, p) in parcels[..old].iter().enumerate() {
            let Some(EntityType::Extended(x)) = doc.get_entity(Handle::new(*p)) else { continue };
            let ExtendedEntityData::Arch(a) = &x.data else { continue };
            let refs = a.typed_handles();
            let (style, label_style) = (refs.get(3).map_or(style, |r| r.0), refs.get(4).map_or(label_style, |r| r.0));
            let common = x.common.clone();
            write_parcel(doc, common, site, i as i32 + 1, i as i32 + 2, arrangement.faces[i + 1].label, style, label_style)?;
        }
    }
    let mut out = Vec::new();
    for (k, c) in made.into_iter().enumerate() {
        let face = old + k + 2;
        out.push(write_parcel(doc, c, site, face as i32 - 1, face as i32, arrangement.faces[face - 1].label, style, label_style)?);
    }
    Ok((segment, out))
}

/// Adds a parcel on `site` bounded by a closed lot line (a parcel segment)
/// through `vertices` (straight segments; the line closes back to the
/// first vertex), and returns the parcel. The segment joins the site's
/// container and its inside becomes a new face. It must not cross or
/// touch the site's members (see [`add_lot_line`] for lines that do), and
/// the site must be one this crate writes. The parcel takes the site's
/// parcel style and the drawing's default parcel area label style (none
/// without one); the application names it from the style and its number.
pub fn add_parcel(doc: &mut CadDocument, site: Handle, vertices: &[Vector2], layer: Option<&str>) -> Result<Handle, DxfError> {
    let points: Vec<(Vector2, f64)> = vertices.iter().map(|p| (*p, 0.0)).collect();
    let (_, parcels) = add_lot_line_on(doc, site, &points, true, false, layer)?;
    parcels.into_iter().next().ok_or_else(|| invalid("the lot line closes no face"))
}

/// Adds a lot line (a parcel segment) through `vertices` on `site`, each
/// with the bulge of the segment that starts there (0 for a straight one;
/// positive counterclockwise, the tangent of a quarter of the arc's angle;
/// a closed line's last bulge is the closing segment's), closed back to
/// the first vertex when `closed`, and returns it with the parcels it
/// makes. It may cross and touch the site's other lot lines (not its
/// alignments or feature lines, and not where either has an arc): the lines are
/// split where they meet, as the application splits them, every face the
/// line closes or cuts off becomes a parcel (numbered in the order the
/// faces are made) and the parcel of a face it cuts keeps the part the
/// application gives it. Parcels take the site's parcel style and the
/// drawing's default parcel area label style.
pub fn add_lot_line(doc: &mut CadDocument, site: Handle, vertices: &[(Vector2, f64)], closed: bool, layer: Option<&str>) -> Result<(Handle, Vec<Handle>), DxfError> {
    add_lot_line_on(doc, site, vertices, closed, true, layer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::io::dwg::{DwgReader, DwgWriter};
    use crate::types::DxfVersion;

    /// A drawing with an alignment (for the style tree and alignment
    /// styles) and stand-in nodes: the parcel and feature line style
    /// collections with a default style each, and the node of sites.
    fn drawing() -> CadDocument {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        crate::classes::civil::add_alignment(&mut doc, "AL0", &[Vector2::new(0.0, -50.0), Vector2::new(100.0, -50.0)], None).unwrap();
        let root = ensure_civil(&mut doc).unwrap();
        let styles = root_node(&doc, KEY_STYLES).unwrap();
        let [parcels, parcel, lines, line, sites] = [(); 5].map(|_| doc.allocate_handle());
        let node = |doc: &mut CadDocument, handle: Handle, owner: Handle, key: i32, children: &[(i32, Handle)]| {
            let refs = children.iter().map(|c| soft_owner(c.1.value())).collect();
            put_object(doc, "AECC_TREE_NODE", handle, owner, Vec::new(), |w| {
                w.write_bit_long(key);
                w.write_bit_long(children.len() as i32);
                for c in children {
                    w.write_bit_long(c.0);
                }
                w.write_bit_long(0);
                bits(w, "010");
            }, Body { refs, strings: vec![String::new()] })
            .unwrap();
        };
        node(&mut doc, parcels, styles, KEY_PARCEL_STYLES, &[(DEFAULT_STYLE_ID, parcel)]);
        node(&mut doc, parcel, parcels, DEFAULT_STYLE_ID, &[]);
        node(&mut doc, lines, styles, KEY_FEATURE_LINE_STYLES, &[(0x8000_0001_u32 as i32, Handle::new(0)), (DEFAULT_STYLE_ID, line)]);
        node(&mut doc, line, lines, DEFAULT_STYLE_ID, &[]);
        add_node_child(&mut doc, styles, KEY_PARCEL_STYLES, parcels, "").unwrap();
        add_node_child(&mut doc, styles, KEY_FEATURE_LINE_STYLES, lines, "").unwrap();
        write_site_node(&mut doc, sites, root, &SiteNode { sites: Vec::new(), own: vec![hard_pointer(0); 3] }).unwrap();
        set_root_entry(&mut doc, KEY_SITES, sites).unwrap();
        doc
    }

    fn arch(doc: &CadDocument, h: Handle) -> crate::classes::arch::ArchEntityData {
        match doc.get_entity(h) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => a.clone(),
                _ => panic!(),
            },
            _ => panic!(),
        }
    }

    #[test]
    fn sites_and_their_alignments_round_trip() {
        let v = Vector2::new;
        let mut doc = drawing();
        let s1 = add_site(&mut doc, "Site 1", "first").unwrap();
        let s2 = add_site(&mut doc, "Second", "").unwrap();
        assert!(add_site(&mut doc, "site 1", "").is_err());
        let a1 = add_alignment_on_site(&mut doc, s1, "AL1", &[v(0.0, 0.0), v(100.0, 0.0), v(200.0, 50.0)], &[VertexCurve::Arc(80.0)], None).unwrap();
        let a2 = add_alignment_on_site(&mut doc, s1, "AL2", &[v(0.0, 50.0), v(100.0, 80.0)], &[], None).unwrap();
        // Crossing AL2 is refused.
        assert!(add_alignment_on_site(&mut doc, s1, "AL3", &[v(50.0, 40.0), v(50.0, 100.0)], &[], None).is_err());
        let path = std::env::temp_dir().join(format!("civil-site-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(arch(&back, s1).face_name(), Some(("Site 1".into(), "first".into())));
        assert_eq!(arch(&back, s2).face_name(), Some(("Second".into(), String::new())));
        let container = arch(&back, s1).typed_handles()[5].0;
        let c = object(&back, Handle::new(container)).unwrap();
        assert_eq!(c.site_members(), vec![a1.value(), a2.value()]);
        let edges = c.topology().unwrap().edges;
        assert_eq!(edges.len(), 2);
        assert!(edges[0].iter().any(|p| p.bulge > 0.0));
        assert_eq!(edges[1].iter().map(|p| p.point).collect::<Vec<_>>(), vec![v(0.0, 50.0), v(100.0, 80.0)]);
        assert_eq!(arch(&back, a1).alignment_container(), Some(container));
        assert_eq!(arch(&back, a2).alignment_name(), Some("AL2".into()));
        let node = read_site_node(&back, root_node(&back, KEY_SITES).unwrap()).unwrap();
        assert_eq!(node.sites.iter().map(|s| (s.1, s.2.as_str())).collect::<Vec<_>>(), vec![(s1.value(), "Site 1"), (s2.value(), "Second")]);
        // The second site is empty; an alignment joins it after a write.
        let mut doc = back;
        assert!(add_alignment_on_site(&mut doc, s2, "AL4", &[v(0.0, 200.0), v(100.0, 200.0)], &[], None).is_ok());
        assert!(add_alignment_on_site(&mut doc, a1, "AL5", &[v(0.0, 300.0), v(100.0, 300.0)], &[], None).is_err());
    }

    #[test]
    fn a_new_drawing_gets_the_node_of_sites() {
        let mut doc = CadDocument::new();
        doc.version = DxfVersion::AC1032;
        let s = add_site(&mut doc, "Site 1", "").unwrap();
        let a = add_alignment_on_site(&mut doc, s, "AL1", &[Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0)], &[], None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-site-new-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let node = read_site_node(&back, root_node(&back, KEY_SITES).unwrap()).unwrap();
        assert_eq!(node.sites.len(), 1);
        // A new drawing gets the default parcel and feature line styles.
        let parcel = default_style_of(&back, KEY_PARCEL_STYLES).unwrap();
        assert_eq!(object(&back, Handle::new(parcel)).unwrap().dxf_name, "AECC_PARCEL_STYLE");
        assert_eq!(arch(&back, s).typed_handles()[6].0, parcel);
        let line = default_style_of(&back, KEY_FEATURE_LINE_STYLES).unwrap();
        assert_eq!(object(&back, Handle::new(line)).unwrap().dxf_name, "AECC_FEATURELINE_STYLE");
        assert_eq!(arch(&back, a).alignment_container(), Some(arch(&back, s).typed_handles()[5].0));
    }

    #[test]
    fn feature_lines_join_their_site() {
        let v = Vector2::new;
        let p = crate::types::Vector3::new;
        let mut doc = drawing();
        let s = add_site(&mut doc, "Site 1", "").unwrap();
        let f1 = add_feature_line(&mut doc, s, "FL1", &[p(0.0, 0.0, 10.0), p(50.0, 0.0, 12.0), p(100.0, 30.0, 11.0)], None).unwrap();
        let a = add_alignment_on_site(&mut doc, s, "AL1", &[v(0.0, 100.0), v(100.0, 100.0), v(150.0, 120.0)], &[VertexCurve::None], None).unwrap();
        let f2 = add_feature_line(&mut doc, s, "FL2", &[p(0.0, 50.0, 5.0), p(100.0, 50.0, 6.0)], None).unwrap();
        assert!(add_feature_line(&mut doc, s, "FL3", &[p(50.0, -10.0, 0.0), p(50.0, 10.0, 0.0)], None).is_err());
        let path = std::env::temp_dir().join(format!("civil-feature-line-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let container = arch(&back, s).typed_handles()[5].0;
        let c = object(&back, Handle::new(container)).unwrap();
        assert_eq!(c.site_members(), vec![f1.value(), a.value(), f2.value()]);
        let edges = c.topology().unwrap().edges;
        assert_eq!(edges[0].iter().map(|v| v.z).collect::<Vec<_>>(), vec![10.0, 12.0, 11.0]);
        assert_eq!(edges[2].iter().map(|v| (v.point, v.z)).collect::<Vec<_>>(), vec![(v(0.0, 50.0), 5.0), (v(100.0, 50.0), 6.0)]);
        assert_eq!(arch(&back, f2).decode().strings.last().map(String::as_str), Some("FL2"));
        // The alignment keeps its member number (2) as its edge id.
        let (w, _) = super::super::alignment::rewritable(&back, a, |_| None).unwrap();
        assert_eq!(w.edge, 2);
    }

    #[test]
    fn feature_lines_keep_their_arcs_and_elevation_points() {
        let v = |x: f64, y: f64, z: f64, bulge: f64, elevation_point: bool| FeatureLineVertex { point: crate::types::Vector3::new(x, y, z), bulge, elevation_point };
        let mut doc = drawing();
        let s = add_site(&mut doc, "Site 1", "").unwrap();
        let bad = [v(0.0, 0.0, 10.0, 0.0, true), v(50.0, 0.0, 10.0, 0.0, false)];
        assert!(add_feature_line_vertices(&mut doc, s, "X", &bad, None).is_err());
        let a = [v(0.0, 0.0, 10.0, 0.0, false), v(25.0, 0.0, 11.0, 0.0, true), v(50.0, 0.0, 10.0, 0.5, false), v(100.0, 0.0, 14.0, -0.3, false), v(150.0, 20.0, 12.0, 0.0, false)];
        let fa = add_feature_line_vertices(&mut doc, s, "FLA", &a, None).unwrap();
        let b = [v(0.0, 50.0, 5.0, 0.0, false), v(30.0, 50.0, 6.0, 0.0, true), v(60.0, 50.0, 7.0, 0.0, true), v(100.0, 50.0, 6.0, 0.0, false)];
        let fb = add_feature_line_vertices(&mut doc, s, "FLB", &b, None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-fl-arcs-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let mut back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let (_, members, _, _) = members_of(&mut back, s).unwrap();
        for (h, want) in [(fa, &a[..]), (fb, &b[..])] {
            let m = members.iter().find(|m| m.handle == h.value()).unwrap();
            assert_eq!(m.path.iter().map(|p| p.1.unwrap_or(0.0)).collect::<Vec<_>>(), want.iter().map(|v| v.bulge).collect::<Vec<_>>());
            assert_eq!(m.elevations, want.iter().map(|v| v.point.z).collect::<Vec<_>>());
            assert_eq!(m.elevation_points, want.iter().map(|v| v.elevation_point).collect::<Vec<_>>());
        }
    }


    #[test]
    fn parcels_take_their_faces() {
        let v = Vector2::new;
        let mut doc = drawing();
        let s = add_site(&mut doc, "Site 1", "").unwrap();
        let a = add_alignment_on_site(&mut doc, s, "AL1", &[v(0.0, -20.0), v(100.0, -20.0)], &[], None).unwrap();
        let p1 = add_parcel(&mut doc, s, &[v(0.0, 0.0), v(100.0, 0.0), v(100.0, 50.0), v(0.0, 50.0)], None).unwrap();
        // Clockwise, its extents' middle on the boundary.
        let p2 = add_parcel(&mut doc, s, &[v(200.0, 0.0), v(200.0, 50.0), v(300.0, 50.0)], None).unwrap();
        assert!(add_parcel(&mut doc, s, &[v(50.0, -40.0), v(60.0, -40.0), v(55.0, 10.0)], None).is_err());
        assert!(add_parcel(&mut doc, s, &[v(0.0, 100.0), v(10.0, 110.0), v(10.0, 100.0), v(0.0, 110.0)], None).is_err());
        assert!(add_parcel(&mut doc, s, &[v(0.0, 100.0), v(10.0, 100.0)], None).is_err());
        let path = std::env::temp_dir().join(format!("civil-parcels-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let mut back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let container = arch(&back, s).typed_handles()[5].0;
        let topology = object(&back, Handle::new(container)).unwrap().topology().unwrap();
        let area = |run: &Vec<Vector2>| run.windows(2).map(|w| w[0].cross(&w[1])).sum::<f64>() / 2.0;
        for (p, number, want) in [(p1, 1, 5000.0), (p2, 2, 2500.0)] {
            let parcel = arch(&back, p);
            assert_eq!(parcel.parcel_site(), Some(s.value()));
            let (n, face) = parcel.parcel_face().unwrap();
            assert_eq!(n, number);
            let loops = topology.face_polylines(face);
            assert_eq!(loops.len(), 1);
            assert!((area(&loops[0]) - want).abs() < 1e-9);
        }
        // The site still takes members after its parcels.
        let f = add_feature_line(&mut back, s, "FL1", &[crate::types::Vector3::new(0.0, 80.0, 5.0), crate::types::Vector3::new(100.0, 80.0, 6.0)], None).unwrap();
        let refs: Vec<u64> = object(&back, Handle::new(container)).unwrap().typed_handles().iter().map(|r| r.0).collect();
        let lines: Vec<u64> = refs.iter().step_by(2).take(4).copied().collect();
        assert_eq!((lines[0], lines[3]), (a.value(), f.value()));
        assert_eq!(&refs[10..14], &[p1.value(), 0, p2.value(), 0]);
    }
    #[test]
    fn crossing_lot_lines_split_the_parcels() {
        let v = Vector2::new;
        let w = |x: f64, y: f64| (Vector2::new(x, y), 0.0);
        let mut doc = drawing();
        let s = add_site(&mut doc, "Site 1", "").unwrap();
        let a = add_alignment_on_site(&mut doc, s, "AL1", &[v(0.0, -50.0), v(100.0, -50.0)], &[], None).unwrap();
        let (_, first) = add_lot_line(&mut doc, s, &[w(0.0, 0.0), w(100.0, 0.0), w(100.0, 50.0), w(0.0, 50.0)], true, None).unwrap();
        // A T junction, then a line crossing it: four parcels.
        let (_, second) = add_lot_line(&mut doc, s, &[w(50.0, 0.0), w(50.0, 50.0)], false, None).unwrap();
        let (_, third) = add_lot_line(&mut doc, s, &[w(0.0, 25.0), w(100.0, 25.0)], false, None).unwrap();
        assert_eq!((first.len(), second.len(), third.len()), (1, 1, 2));
        // Neither the alignment nor a plain parcel may be met.
        assert!(add_lot_line(&mut doc, s, &[w(10.0, -60.0), w(10.0, -40.0)], false, None).is_err());
        assert!(add_parcel(&mut doc, s, &[v(90.0, 40.0), v(110.0, 40.0), v(110.0, 60.0)], None).is_err());
        let path = std::env::temp_dir().join(format!("civil-lot-lines-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let mut back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let container = arch(&back, s).typed_handles()[5].0;
        let topology = object(&back, Handle::new(container)).unwrap().topology().unwrap();
        // The rectangle got the four nodes the other lines meet it at.
        assert_eq!(topology.edges[1].len(), 9);
        assert!(topology.edges[1].iter().filter(|p| p.flags & 0x400 != 0).count() == 4);
        let area = |run: &Vec<Vector2>| run.windows(2).map(|w| w[0].cross(&w[1])).sum::<f64>() / 2.0;
        let parcels: Vec<Handle> = first.iter().chain(&second).chain(&third).copied().collect();
        for (i, p) in parcels.iter().enumerate() {
            let (number, face) = arch(&back, *p).parcel_face().unwrap();
            assert_eq!((number, face), (i as i32 + 1, i as i32 + 2));
            let loops = topology.face_polylines(face);
            assert_eq!(loops.len(), 1);
            assert!((area(&loops[0]).abs() - 1250.0).abs() < 1e-9);
        }
        // The site still takes members; the alignment keeps its edge.
        assert!(add_lot_line(&mut back, s, &[w(200.0, 0.0), w(300.0, 0.0), w(300.0, 50.0)], true, None).is_ok());
        assert_eq!(object(&back, Handle::new(container)).unwrap().site_members()[0], a.value());
    }
}
