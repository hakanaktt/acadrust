//! Sites (`AECC_PARCEL` records named `AeccImpFace` without a site of
//! their own), their containers and the alignments on them.

use super::stormwater::graphic_image;
use super::arrangement::{build, Arrangement, Input};
use super::alignment::{default_style as alignment_style, elements, solve, write_alignment, PathPoint, VertexCurve, ARC_EDGE};
use super::groups::bucket_insert;
use super::support::*;
use super::default_styles::{ensure_site_styles, KEY_FEATURE_LINE_STYLES, KEY_PARCEL_STYLES};
use super::tree::{ensure_alignment_tree, DEFAULT_STYLE_ID, KEY_STYLES};
use crate::classes::arch::ArchObject;
use crate::classes::civil::alignment::same_solved_point;
use crate::entities::{EntityCommon, EntityType, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::types::{Handle, Vector2, Vector3};
use crate::CadDocument;

/// Root key of the node that lists the sites.
const KEY_SITES: i32 = 2;
/// First text id given to a written site (kept apart from the other
/// writers' ids).
const FIRST_SITE_TAG: u32 = 0x8004_0001;

/// The default style of the style tree's collection `key`: its child with
/// the default style id (30), or its only child; a collection whose styles
/// are all named lists its default as the first soft pointer after them
/// (WC64base).
pub(super) fn default_style_of(doc: &CadDocument, key: i32) -> Option<u64> {
    let tree = read_node(doc, root_node(doc, KEY_STYLES)?)?;
    let head = tree.children.iter().find(|c| c.0 == key)?.1;
    let o = object(doc, Handle::new(head))?;
    let (data, start, end) = fields_of(o)?;
    let mut b = super::super::bits::Bits::new(data, start, end);
    b.bit_long()?;
    let n = usize::try_from(b.bit_long()?).ok()?;
    let keys = (0..n).map(|_| b.bit_long()).collect::<Option<Vec<_>>>()?;
    let at = keys.iter().position(|k| *k == DEFAULT_STYLE_ID).or((n == 1).then_some(0));
    let at = match at {
        Some(at) => at,
        None => n + o.typed_handles().get(n..)?.iter().position(|r| r.1 == 4)?,
    };
    o.decode().handles.get(at).copied().filter(|h| *h != 0)
}

/// The style named `name` (any case) of the style tree's collection
/// `key`, when the drawing has one.
pub(super) fn named_style(doc: &CadDocument, key: i32, name: &str) -> Option<u64> {
    let tree = read_node(doc, root_node(doc, KEY_STYLES)?)?;
    let head = tree.children.iter().find(|c| c.0 == key)?.1;
    let node = read_node(doc, Handle::new(head))?;
    let text = |k: i32| k as u32 & 0x8000_0000 != 0;
    node.children.iter().filter(|c| text(c.0)).zip(&node.names).find(|(_, s)| s.eq_ignore_ascii_case(name)).map(|(c, _)| c.1).filter(|h| *h != 0)
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
    /// Node ids and elevation point keys the container gave the points
    /// (0, or none listed, for a point that gets the next free one).
    ids: Vec<(i32, i32)>,
    /// What a container read gave the member's edge besides.
    read: Option<ReadEdge>,
}

/// What a container read gave a member's edge: its flags, each of the
/// member's points' flags and the nodes other members' crossings added to
/// it (point, id, flags). The application's marks no rule here reproduces
/// are kept: edge flag 0x800 and the last point's flag 8 with it (WF04),
/// the newest member's point flags 0x10 and 0x20 while it is the newest
/// (WP08, WP09) and those of the nodes crossings added (WP04, WP05).
pub(super) struct ReadEdge {
    flags: i32,
    points: Vec<i32>,
    inserted: Vec<(Vector2, i32, i32)>,
    /// An alignment's path solved from its record when its edge has no
    /// points (moved onto the site), for the crossing checks.
    shape: Vec<PathPoint>,
}

/// What a container read keeps when written again: its next free node id
/// and elevation point key (0 when written anew: ids and keys from 1),
/// whether its outside face has lost flag 8 (once a member has an
/// elevation point or its elevations were edited: WF11), and the outside
/// face's flags when no member has points (0 for no outside face).
#[derive(Clone, Default)]
pub(super) struct NextIds {
    node: i32,
    key: i32,
    edited: bool,
    outside: i32,
    /// The references before the default feature line style at the end
    /// (none in most containers; other feature line styles and empty
    /// slots in others: WC64base); the list after the faces starts with
    /// their count, the style's included.
    styles: Vec<Ref>,
    /// The outside face's entries read (node id, index, the node's number
    /// of half-edges then).
    outside_entries: Vec<(i32, i32, usize)>,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum MemberKind {
    Alignment,
    FeatureLine,
    /// A parcel segment; a closed one joins its last point to the first.
    LotLine { closed: bool },
    /// A profile of an alignment on the site: an edge without points
    /// (WC64, WC65).
    Profile,
}

impl Member {
    fn closed(&self) -> bool {
        self.kind == MemberKind::LotLine { closed: true }
    }

    /// The path the member takes, closed back to its first point for a
    /// closed lot line.
    fn outline(&self) -> Vec<PathPoint> {
        if self.path.is_empty() {
            return self.read.as_ref().map(|r| r.shape.clone()).unwrap_or_default();
        }
        let mut path = self.path.clone();
        if self.closed() {
            path.extend(self.path.first().map(|p| (p.0, None)));
        }
        path
    }
}

/// The bucket of a key in the application's hash order for a table of
/// `count` entries (the container's node records and elevation points,
/// each bucket in key order): linear hashing from eight buckets, one more
/// bucket split off while the table holds more than eight entries a
/// bucket; with `level` the power of two at most the bucket count and
/// `split` the buckets past it, a key goes to `key mod level`, or
/// `key mod 2 level` below `split` (WF27: 9 points in 8 buckets; WF28: 130
/// elevation points and 132 nodes in 17 buckets, 32 64 96 128 first and
/// 16 48 80 112 last).
fn hash_order(count: usize) -> impl Fn(i32) -> i32 {
    let mut buckets = 8;
    while count > 8 * buckets {
        buckets += 1;
    }
    let level = 1 << buckets.ilog2();
    let split = (buckets - level) as i32;
    let level = level as i32;
    move |key| {
        let b = key.rem_euclid(level);
        if b < split { key.rem_euclid(2 * level) } else { b }
    }
}

/// The positions of ids 1 to `count` in a table the application lists in
/// [`hash_order`] (a container's edges, its faces and its list of
/// parcels: WC64, WC65 with 12 and 20 members and 9 faces).
fn slot_order(count: usize) -> Vec<usize> {
    let bucket = hash_order(count);
    let mut order: Vec<usize> = (0..count).collect();
    order.sort_by_key(|i| (bucket(*i as i32 + 1), *i));
    order
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
/// The edge flag and point flags a container read keeps (see
/// [`ReadEdge`]).
const READ_EDGE: i32 = 0x800;
const READ_POINT: i32 = 0x30;

/// The site's container (`AECC_FEATURE_STYLE` owned by the site): the
/// topology of its members, each an edge through its own nodes, and the
/// faces closed lot lines make.
///
/// ```text
/// BS 0x04BE, BL 0, BL 0 (no children), BL 0, BL 32, 7 bits 0111010,
/// elevation points: BL next free key, BL k, k x {BL key, BL 21, BD the
///   point's index in its feature line, 17 bits 10000000000101010,
///   BL key, BL 0, BL 0, BD elevation, B 0, BL 0} (in [`hash_order`];
///   written anew, keys 1..k in edge order), BL 1, BL 0,
/// BL 0x80000001,
/// nodes: BL next free id, BL n, BL first id, n x {3BD (z the
///   elevation), 3 bits 010, BL next id} (written anew, ids per member:
///   its ends, then its inner points, a closed one's in path order; the
///   records in [`hash_order`], WP15 and WP16 with 30 and 50 nodes in
///   eight buckets)
/// A container read keeps its node ids, elevation point keys and next
/// free ones (the application does not reuse those of removed points:
/// WF22, WF25, WF26); new points take the next free ones.
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
/// Members that meet (lot lines, feature lines) are split where they meet: the
/// nodes another member's crossing adds to an edge have flags 0x400
/// instead of 2 and mark the edge with 0x80; a point on an earlier node
/// carries 0x800000, and 0x80 on a first point only marks an edge that
/// starts off every earlier line. The faces, their entries and flags are
/// those [`build`] gives (WP03 to WP20: written for their segments the
/// containers equal the samples' but for edge flag 0x800, which the
/// application sets on an edge read from a file when another member joins,
/// and the newest member's point flags 0x10 and 0x20 once it meets
/// another).
/// The members with points as the arrangement takes them.
fn member_inputs(members: &[Member]) -> Vec<Input<'_>> {
    members
        .iter()
        .filter(|m| !m.path.is_empty())
        .map(|m| Input { path: &m.path, elevations: &m.elevations, closed: m.closed(), feature_line: m.kind == MemberKind::FeatureLine, lot_line: matches!(m.kind, MemberKind::LotLine { .. }) })
        .collect()
}

/// The arrangement of `members`, with an empty edge for each member
/// without points (the outside face's `outside` flags without members
/// with points).
fn arrangement_of(members: &[Member], outside: i32) -> Result<Arrangement, DxfError> {
    let inputs = member_inputs(members);
    let mut arrangement = if inputs.is_empty() { super::arrangement::without_points(outside) } else { build(&inputs).map_err(invalid)? };
    for (i, m) in members.iter().enumerate() {
        if m.path.is_empty() {
            arrangement.edges.insert(i, super::arrangement::Edge::without_points());
        }
    }
    Ok(arrangement)
}

/// The positions of the faces with these ids in the hash order a
/// container lists them in (`slot_order` for ids 1 to n).
fn id_slot_order(ids: &[i32]) -> Vec<usize> {
    let bucket = hash_order(ids.len());
    let mut order: Vec<usize> = (0..ids.len()).collect();
    order.sort_by_key(|i| (bucket(ids[*i]), ids[*i]));
    order
}

pub(super) fn write_container(doc: &mut CadDocument, handle: Handle, site: Handle, members: &[Member], parcels: &[u64], style: u64, next: NextIds) -> Result<Arrangement, DxfError> {
    // Members moved onto the site have no points (their edges are empty)
    // until the application builds the container again; the arrangement is
    // that of the others (WT04F: a feature line added to WT04).
    let arrangement = arrangement_of(members, next.outside)?;
    // The ids of the arrangement's nodes and the keys of the members'
    // elevation points: those the container gave them, the rest the next
    // free ones in node and edge order (1.. when written anew).
    let mut node_ids = vec![0; arrangement.nodes.len()];
    let mut point_keys: Vec<Vec<i32>> = Vec::with_capacity(members.len());
    for (m, edge) in members.iter().zip(&arrangement.edges) {
        let own = edge.vertices.iter().filter(|v| !v.inserted);
        for (i, v) in own.enumerate() {
            let id = m.ids.get(i).map_or(0, |x| x.0);
            if id > 0 {
                node_ids[v.node as usize - 1] = id;
            }
        }
        // Nodes other members' crossings added keep the ids read.
        for v in edge.vertices.iter().filter(|v| v.inserted) {
            let p = arrangement.nodes[v.node as usize - 1].0;
            if let Some(&(_, id, _)) = m.read.iter().flat_map(|r| &r.inserted).find(|(q, _, _)| crate::classes::civil::alignment::same_solved_point(*q, p)) {
                node_ids[v.node as usize - 1] = id;
            }
        }
        point_keys.push((0..m.path.len()).map(|i| if m.elevation_points[i] { m.ids.get(i).map_or(0, |x| x.1) } else { 0 }).collect());
    }
    let mut next_node = next.node.max(node_ids.iter().max().map_or(1, |m| m + 1));
    for id in node_ids.iter_mut().filter(|id| **id == 0) {
        *id = next_node;
        next_node += 1;
    }
    // A feature line joining the site across lot lines makes the
    // application number every node where a feature line meets a lot line
    // again, in the lot lines' point order, after the ids the new line's
    // crossings took (PD01: crossings 7, 8 then (100, 25) 9, (0, 25) 10;
    // PD03, PD05, PD09; PD08: a second line, the first line's crossings too,
    // 16 to 19, the crossing of the two lines keeping 15; F03: the line's own
    // points on the lot line keep theirs).
    // A new feature line's marks on the lot lines (PD01 to PD09, F01 to
    // F04): every lot line edge takes 0x800, its crossings by feature
    // lines no 0x80; a lot line the line meets marks its first point 0x80
    // and the nodes feature lines put on it 0x800000; one whose face the
    // line splits marks all its points 0x800 too.
    let new_line = members.last().filter(|m| m.kind == MemberKind::FeatureLine && m.ids.is_empty()).map(|_| members.len() - 1);
    let on_feature_lines: Vec<i32> = members.iter().zip(&arrangement.edges).filter(|(m, _)| m.kind == MemberKind::FeatureLine).flat_map(|(_, e)| e.vertices.iter().map(|v| v.node)).collect();
    let splits = new_line.is_some_and(|k| arrangement_of(&members[..k], next.outside).is_ok_and(|before| before.faces.len() < arrangement.faces.len()));
    let met_by_new: Vec<bool> = arrangement.edges.iter().map(|e| new_line.is_some_and(|k| e.vertices.iter().any(|v| arrangement.edges[k].vertices.iter().any(|w| w.node == v.node)))).collect();
    if let Some((k, _)) = members.iter().enumerate().last().filter(|(_, m)| m.kind == MemberKind::FeatureLine && m.ids.is_empty()) {
        let on_feature_lines: Vec<i32> = members.iter().zip(&arrangement.edges).filter(|(m, _)| m.kind == MemberKind::FeatureLine).flat_map(|(_, e)| e.vertices.iter().filter(|v| v.inserted).map(|v| v.node)).collect();
        let lot_lines: Vec<&super::arrangement::Edge> = members.iter().zip(&arrangement.edges).filter(|(m, _)| matches!(m.kind, MemberKind::LotLine { .. })).map(|(_, e)| e).collect();
        let new_line = &arrangement.edges[k];
        if lot_lines.iter().any(|e| e.vertices.iter().any(|v| v.inserted && new_line.vertices.iter().any(|w| w.node == v.node))) {
            let mut done: Vec<i32> = Vec::new();
            for v in lot_lines.iter().flat_map(|e| &e.vertices).filter(|v| v.inserted && on_feature_lines.contains(&v.node)) {
                if !done.contains(&v.node) {
                    done.push(v.node);
                    node_ids[v.node as usize - 1] = next_node;
                    next_node += 1;
                }
            }
        }
    }
    let mut next_key = next.key.max(point_keys.iter().flatten().max().map_or(1, |m| m + 1));
    for (m, keys) in members.iter().zip(&mut point_keys) {
        for (key, _) in keys.iter_mut().zip(&m.elevation_points).filter(|(k, e)| **k == 0 && **e) {
            *key = next_key;
            next_key += 1;
        }
    }
    let id_of = |node: i32| if node > 0 { node_ids[node as usize - 1] } else { node };
    let elevation_points = members.iter().map(|m| m.elevation_points.iter().filter(|x| **x).count()).sum();
    let mut refs: Vec<Ref> = vec![hard_pointer(0); elevation_points];
    let edge_slots = slot_order(members.len());
    let face_slots = id_slot_order(&arrangement.face_ids);
    refs.extend(edge_slots.iter().flat_map(|e| [soft_pointer(members[*e].handle), soft_pointer(0)]));
    // The site is listed with the outside face, the parcels with theirs
    // (no faces once the application has opened a container of members
    // without points again: WT04R).
    refs.extend(face_slots.iter().flat_map(|f| [soft_pointer(if *f == 0 { site.value() } else { parcels.get(*f - 1).copied().unwrap_or(0) }), soft_pointer(0)]));
    refs.extend(next.styles.iter().copied());
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
            let mut table = Vec::new();
            for (m, keys) in members.iter().zip(&point_keys) {
                for (i, _) in m.elevation_points.iter().enumerate().filter(|e| *e.1) {
                    table.push((keys[i], i, m.elevations[i]));
                }
            }
            let order = hash_order(table.len());
            table.sort_by_key(|t| (order(t.0), t.0));
            w.write_bit_long(next_key);
            w.write_bit_long(table.len() as i32);
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
            let bucket = hash_order(nodes);
            let mut order: Vec<usize> = (0..nodes).collect();
            order.sort_by_key(|i| (bucket(node_ids[*i]), node_ids[*i]));
            let ids: Vec<i32> = order.iter().map(|i| node_ids[*i]).collect();
            w.write_bit_long(next_node);
            w.write_bit_long(nodes as i32);
            w.write_bit_long(ids.first().copied().unwrap_or(0));
            for (k, i) in order.iter().enumerate() {
                let (p, z) = a.nodes[*i];
                for v in [p.x, p.y, z] {
                    w.write_bit_double(v);
                }
                bits(w, "010");
                w.write_bit_long(ids.get(k + 1).copied().unwrap_or(0));
            }
            w.write_bit_long(members.len() as i32 + 1);
            w.write_bit_long(members.len() as i32);
            for &e in &edge_slots {
                let (m, edge, member_keys) = (&members[e], &a.edges[e], &point_keys[e]);
                let n = edge.vertices.len();
                let closed = m.closed();
                let flags = match m.kind {
                    MemberKind::Alignment | MemberKind::Profile => ALIGNMENT_EDGE,
                    MemberKind::FeatureLine => FEATURE_LINE_EDGE,
                    MemberKind::LotLine { closed } => LOT_LINE_EDGE | closed as i32,
                } | if edge.crossed { CROSSED_EDGE } else { 0 }
                    | m.read.as_ref().map_or(0, |r| r.flags & READ_EDGE);
                // An edge without points keeps the flags read (0x10), and so
                // does an edge the new member left as read (PD01's lot line
                // after a feature line crossed it: 0x831).
                let as_read = m.read.as_ref().is_some_and(|r| {
                    let own = edge.vertices.iter().filter(|v| !v.inserted).count();
                    own == r.points.len() && edge.vertices.iter().filter(|v| v.inserted).all(|v| r.inserted.iter().any(|(q, _, _)| q.distance(&a.nodes[v.node as usize - 1].0) <= 1e-9))
                });
                let flags = if n == 0 || as_read { m.read.as_ref().map_or(flags, |r| r.flags) } else { flags };
                let lot_line = matches!(m.kind, MemberKind::LotLine { .. });
                let flags = if new_line.is_some() && lot_line {
                    let by_others = edge.vertices.iter().any(|v| v.inserted && !on_feature_lines.contains(&v.node));
                    (flags | 0x800) & if by_others { !0 } else { !CROSSED_EDGE }
                } else {
                    flags
                };
                let marked = new_line.is_some() && lot_line && met_by_new[e];
                for v in [0, e as i32 + 1, flags, n as i32, 0] {
                    w.write_bit_long(v);
                }
                let last_member = e + 1 == members.len();
                let keys: Vec<i32> = member_keys.iter().copied().filter(|k| *k != 0).collect();
                let mut own = 0;
                // A newest feature line that crosses another member marks
                // its points as the others' are (FX01, FX03).
                let meets = m.kind == MemberKind::FeatureLine && edge.vertices.iter().any(|v| v.inserted);
                for (i, v) in edge.vertices.iter().enumerate() {
                    let read = m.read.as_ref().and_then(|r| {
                        if v.inserted {
                            let p = a.nodes[v.node as usize - 1].0;
                            r.inserted.iter().find(|(q, _, _)| q.distance(&p) <= 1e-9).map(|x| x.2)
                        } else {
                            r.points.get(own).copied()
                        }
                    });
                    // The newest member's points and the nodes crossings
                    // added keep the marks read (WP05: an inserted node of
                    // the member before the newest keeps 0x10).
                    let marks = read.filter(|_| last_member || v.inserted).map(|f| f & READ_POINT);
                    let elevation_point = !v.inserted && m.elevation_points[own];
                    own += !v.inserted as usize;
                    let last = i + 1 == n && !closed;
                    let arc = v.bulge.filter(|_| !last);
                    let follows = i + 2 < n || (closed && i + 1 < n) || !last_member || meets;
                    let shared = if v.shared { SHARED_NODE } else { 0 };
                    let flags = if last {
                        (if m.read.as_ref().is_some_and(|r| r.flags & READ_EDGE != 0) { 2 | read.unwrap_or(10) & 8 } else { 10 }) | shared
                    } else if let Some(marks) = marks {
                        (if elevation_point { 0x40_0400 } else if v.inserted { 0x400 } else { 2 }) | marks | if i == 0 && edge.loose_start { 0x80 } else { 0 } | if arc.is_some() { ARC_EDGE } else { 0 } | shared
                    } else {
                        (if elevation_point { 0x40_0400 } else if v.inserted { 0x400 } else { 2 }) | if follows { 0x30 } else if closed { 0x20 } else { 0 } | if i == 0 && edge.loose_start { 0x80 } else { 0 } | if arc.is_some() { ARC_EDGE } else { 0 } | shared
                    };
                    // A stretch along an earlier member's line starts at a
                    // point without the follow marks (WC64).
                    let flags = if v.along & 0x1000 != 0 { (flags & !0x30) | v.along } else { flags | v.along };
                    // The marks a feature line meeting lot lines set keep as
                    // read (PD01: 0x800 on the lot line's points, 0x80 on its
                    // first, 0x800000 on the crossings, 0x40 on the line's
                    // last crossing).
                    let flags = flags | read.map_or(0, |f| f & (0x800 | SHARED_NODE | 0x80 | 0x40));
                    let flags = if marked {
                        flags | if i == 0 { 0x80 } else { 0 } | if splits { 0x800 } else { 0 } | if v.inserted && on_feature_lines.contains(&v.node) { SHARED_NODE } else { 0 }
                    } else {
                        flags
                    };
                    let (left, right) = (a.face_id(v.faces.0), a.face_id(v.faces.1));
                    for x in [if last { 2 } else if arc.is_some() { 1 } else { 0 }, flags, id_of(v.node), left, right] {
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
            w.write_bit_long(a.next_face);
            w.write_bit_long(a.faces.len() as i32);
            for &i in &face_slots {
                let f = &a.faces[i];
                // The outside face's flags lose 8 once a member has an
                // elevation point or its elevations were edited.
                let flags = if i == 0 && (next.edited || members.iter().any(|m| m.elevation_points.contains(&true))) { f.flags & !8 } else { f.flags };
                for v in [a.face_ids[i], a.face_ids[i], flags, f.entries.len() as i32] {
                    w.write_bit_long(v);
                }
                // The outside face keeps the entries read while they still
                // name a half-edge of it (the application moves them by
                // rules not known: PD01, PD05).
                let kept: Option<Vec<(i32, i32)>> = if i == 0 && next.outside_entries.len() == f.entries.len() {
                    next.outside_entries
                        .iter()
                        .map(|&(id, k, degree)| {
                            let node = node_ids.iter().position(|n| *n == id)? as i32 + 1;
                            let out = a.outgoing.get(node as usize - 1)?;
                            let to = *out.get(k as usize)?;
                            (out.len() == degree && a.halves.iter().any(|h| h.0 == (node, to) && h.1 == 1)).then_some((node, k))
                        })
                        .collect()
                } else {
                    None
                };
                for &(node, k) in kept.as_ref().unwrap_or(&f.entries) {
                    w.write_bit_long(id_of(node));
                    w.write_bit_long(k);
                }
            }
            // The parcels: one over several faces counts once (PD01).
            let mut distinct: Vec<u64> = parcels.iter().copied().filter(|h| *h != 0).collect();
            distinct.sort_unstable();
            distinct.dedup();
            let p = if distinct.len() < parcels.len() { distinct.len() as i32 } else { p };
            let tail = [next.styles.len() as i32 + 1, 1, p + 1].into_iter().chain([1; 10]).chain([p]).chain(slot_order(p as usize).into_iter().map(|i| i as i32 + 1));
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
fn members_of(doc: &mut CadDocument, site: Handle) -> Result<(Handle, Vec<Member>, Vec<u64>, u64, NextIds), DxfError> {
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
    let (edges, edge_flags, edge_ids, face_ids, mut next) = if refs.len() == 1 {
        (Vec::new(), Vec::new(), Vec::new(), Vec::new(), NextIds::default())
    } else {
        let t = o.topology().ok_or_else(refused)?;
        // Members without points leave the outside face without entries;
        // its flags (6, or none) are kept as read.
        let pointless = t.edges.iter().all(|e| e.is_empty());
        let edited = !pointless && t.outside_face_flags != 0 && t.outside_face_flags & 8 == 0;
        let outside = if pointless { t.outside_face_flags } else { 0 };
        // A node's half-edges: the segments of the runs it lies on.
        let degree = |id: i32| t.edges.iter().map(|e| e.windows(2).filter(|w| w[0].node != w[1].node && (w[0].node == id || w[1].node == id)).count()).sum::<usize>();
        let outside_entries = t.outside_entries.iter().map(|&(n, k)| (n, k, degree(n))).collect();
        (t.edges, t.edge_flags, t.edge_ids, t.face_ids, NextIds { node: t.next_node, key: t.next_key, edited, outside, styles: Vec::new(), outside_entries })
    };
    let elevation_points = edges.iter().flatten().filter(|v| v.is_elevation_point()).count();
    let slots: Vec<u64> = refs.iter().skip(elevation_points).step_by(2).take(edges.len()).map(|r| r.0).collect();
    // The members in id order (the container lists them in hash order);
    // the ids must run from 1.
    let mut order: Vec<usize> = (0..edges.len()).collect();
    order.sort_by_key(|i| edge_ids[*i]);
    if order.iter().enumerate().any(|(k, i)| edge_ids[*i] != k as i32 + 1) {
        return Err(refused());
    }
    let handles: Vec<u64> = order.iter().map(|i| slots[*i]).collect();
    let edges: Vec<_> = order.iter().map(|i| edges[*i].clone()).collect();
    let edge_flags: Vec<i32> = order.iter().map(|i| edge_flags[*i]).collect();
    // The site and the parcels follow the members, by face: the site with
    // the outside face (1), each parcel with its face (2 on).
    let first_face = elevation_points + 2 * edges.len();
    let mut face_refs: Vec<(i32, u64)> = Vec::with_capacity(face_ids.len());
    for (k, f) in face_ids.iter().enumerate() {
        face_refs.push((*f, refs.get(first_face + 2 * k).map(|r| r.0).ok_or_else(refused)?));
    }
    next.styles = refs
        .get(first_face + 2 * face_ids.len()..refs.len().saturating_sub(1))
        .unwrap_or_default()
        .iter()
        .map(|&(h, code)| Ref(if code == 4 { DwgReferenceType::SoftPointer } else { DwgReferenceType::HardPointer }, h))
        .collect();
    if next.styles.iter().zip(refs.get(first_face + 2 * face_ids.len()..).unwrap_or_default()).any(|(r, x)| !matches!(x.1, 4 | 5) || r.1 != x.0) {
        return Err(refused());
    }
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
        .zip(&edge_flags)
        .map(|((run, h), &flags)| {
            // A closed edge (flag 1) is read with a copy of its first
            // vertex last.
            let closed = flags & 1 != 0;
            // A composed build's reserved lot-line slot carries no record
            // (add_parcel_without_segment; the record is the file's
            // parcel-segment set's to carry): the container's own edge
            // holds the shape, the closed flag the closure.
            let kind = match dxf(*h).as_deref() {
                Some("AECC_ALIGNMENT") => MemberKind::Alignment,
                Some("AECC_FEATURE_LINE") => MemberKind::FeatureLine,
                Some("AECC_PARCEL_SEGMENT") => MemberKind::LotLine { closed },
                Some("AECC_PROFILE") => MemberKind::Profile,
                None => MemberKind::LotLine { closed },
                _ => return None,
            };
            let run = if closed { &run[..run.len() - 1] } else { &run[..] };
            // Nodes other members' crossings added are not the member's.
            let is_own = |v: &&crate::classes::civil::TopologyVertex| v.flags & (0x400 | 2) != 0x400 || v.is_elevation_point();
            let own: Vec<_> = run.iter().filter(is_own).collect();
            // A member moved onto the site has no points: its alignment's
            // path stands in for it in the crossing checks.
            let shape = if run.is_empty() { alignment_path(doc, *h) } else { Vec::new() };
            let read = ReadEdge { flags, points: own.iter().map(|v| v.flags).collect(), inserted: run.iter().filter(|v| !is_own(v)).map(|v| (v.point, v.node, v.flags)).collect(), shape };
            Some(Member {
                handle: *h,
                path: {
                    // An arc other members' crossings split is one arc of
                    // the member: its pieces' sweeps add up.
                    // An unsplit arc keeps its bulge as stored.
                    let bulge = |pieces: &[f64]| -> Option<f64> {
                        match pieces {
                            [] => None,
                            [g] => (*g != 0.0).then_some(*g),
                            _ => {
                                let sweep: f64 = pieces.iter().map(|g| 4.0 * g.atan()).sum();
                                (sweep != 0.0).then(|| (sweep / 4.0).tan())
                            }
                        }
                    };
                    let mut path: Vec<PathPoint> = Vec::new();
                    let mut pieces: Vec<f64> = Vec::new();
                    for (i, v) in run.iter().enumerate() {
                        if is_own(&v) {
                            if let Some(last) = path.last_mut() {
                                last.1 = bulge(&pieces);
                            }
                            path.push((v.point, None));
                            pieces.clear();
                        }
                        if i + 1 < run.len() || closed {
                            pieces.push(v.bulge);
                        }
                    }
                    if let Some(last) = path.last_mut() {
                        last.1 = if closed { bulge(&pieces) } else { None };
                    }
                    path
                },
                elevations: own.iter().map(|v| v.z).collect(),
                elevation_points: own.iter().map(|v| v.is_elevation_point()).collect(),
                kind,
                ids: own.iter().map(|v| (v.node, v.key)).collect(),
                read: Some(read),
            })
        })
        .collect::<Option<_>>()
        .ok_or_else(refused)?;
    // The faces' ids need not run from 1 (a feature line renumbers the
    // faces it meets: PD01's 1, 4, 5): each names the face the
    // arrangement gives it.
    let arrangement = arrangement_of(&members, next.outside).map_err(|_| refused())?;
    let mut parcels: Vec<u64> = vec![0; face_ids.len().saturating_sub(1)];
    for (id, h) in face_refs {
        match arrangement.face_number(id) {
            Some(1) => {}
            Some(f) if (f as usize) <= face_ids.len() => parcels[f as usize - 2] = h,
            _ => return Err(refused()),
        }
    }
    let bits_of = |doc: &CadDocument| object(doc, container).and_then(|o| o.main_stream().map(|(d, s, e)| (s..e).map(|i| d[i / 8] >> (7 - i % 8) & 1).collect::<Vec<u8>>()));
    let before = bits_of(doc);
    let rewritten = write_container(doc, container, site, &members, &parcels, style, next.clone());
    let faces = rewritten.as_ref().map_or(0, |a| a.faces.len());
    // Writing what was read gives back the record: bit for bit — or, on
    // the record's solved geometry, within the solved tolerances. A
    // crossing's resplit arcs go through tan and atan chains, and a
    // platform's libm rounds those ulps its own way (macOS refused its
    // own earlier writes there), so the bits alone are not the record's
    // identity.
    if rewritten.is_err()
        || parcels.len() != faces.saturating_sub(1)
        || (bits_of(doc) != before && !rewritten_as_read(doc, container, &o))
    {
        doc.objects.insert(container, crate::objects::ObjectType::Arch(o));
        return Err(refused());
    }
    Ok((container, members, parcels, style, next))
}

/// Whether the container now in the document reads as the topology the
/// record read before had: node and key ids, flags, counts and the
/// nodes' elevations exactly; the solved points within the shared
/// solved-point tolerance and the bulges within `1e-9` of their
/// magnitude — the resplit arcs go through tan and atan chains, and a
/// platform's libm rounds those ulps its own way.
fn rewritten_as_read(doc: &CadDocument, container: Handle, before: &ArchObject) -> bool {
    let Some(before) = before.topology() else { return false };
    let Some(after) = object(doc, container).and_then(|o| o.topology()) else { return false };
    before.next_node == after.next_node
        && before.next_key == after.next_key
        && before.outside_face_flags == after.outside_face_flags
        && before.edge_flags == after.edge_flags
        && before.edges.len() == after.edges.len()
        && before.edges.iter().zip(&after.edges).all(|(x, y)| {
            x.len() == y.len()
                && x.iter().zip(y).all(|(u, v)| {
                    u.flags == v.flags
                        && u.node == v.node
                        && u.key == v.key
                        && u.faces == v.faces
                        && u.z == v.z
                        && same_solved_point(u.point, v.point)
                        && (u.bulge - v.bulge).abs() <= 1e-9 * (1.0 + u.bulge.abs())
                })
        })
}

/// The container of the site `alignment` lies on (the container its
/// record names in place of a companion), `None` for a siteless one.
pub(super) fn site_container_of(doc: &CadDocument, alignment: Handle) -> Option<Handle> {
    let Some(EntityType::Extended(x)) = doc.get_entity(alignment) else { return None };
    let ExtendedEntityData::Arch(a) = &x.data else { return None };
    let container = Handle::new(a.typed_handles().get(1)?.0);
    let o = object(doc, container).filter(|o| o.dxf_name == "AECC_FEATURE_STYLE")?;
    match doc.get_entity(o.owner) {
        Some(EntityType::Extended(s)) => matches!(&s.data, ExtendedEntityData::Arch(a) if a.dxf_name == "AECC_PARCEL").then_some(container),
        _ => None,
    }
}

/// Lists `profile` (a profile of an alignment on a site) in `container`,
/// the site's container, as a member without points, and returns its
/// edge id; a profile the container lists keeps its id. The container must
/// be one this crate writes.
pub(super) fn join_profile(doc: &mut CadDocument, container: Handle, profile: Handle) -> Result<i32, DxfError> {
    let site = object(doc, container).map(|o| o.owner).ok_or_else(|| invalid("the site's container is missing"))?;
    let (container, mut members, parcels, style, next) = members_of(doc, site)?;
    if let Some(i) = members.iter().position(|m| m.handle == profile.value()) {
        return Ok(i as i32 + 1);
    }
    // The profile record holds its edge id in one byte.
    if members.len() >= 255 {
        return Err(invalid("the site's container has too many members for a profile"));
    }
    members.push(Member { handle: profile.value(), path: Vec::new(), elevations: Vec::new(), elevation_points: Vec::new(), kind: MemberKind::Profile, ids: Vec::new(), read: None });
    write_container(doc, container, site, &members, &parcels, style, next)?;
    Ok(members.len() as i32)
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
    write_container(doc, container, site, &[], &[], line_style, NextIds::default())?;
    write_site(doc, common, tag, name, description, layer.value(), node, container, style)?;
    bucket_insert(&mut list.sites, (tag as i32, site.value(), name.to_string()), |s| s.0);
    let owner = object(doc, node).map_or(root, |o| o.owner);
    write_site_node(doc, node, owner, &list)?;
    if root_node(doc, KEY_SITES).is_none() {
        set_root_entry(doc, KEY_SITES, node)?;
    }
    Ok(site)
}

/// The solved path of an alignment record (its pieces' points, arcs as
/// chords), empty for another record.
fn alignment_path(doc: &CadDocument, alignment: u64) -> Vec<PathPoint> {
    let Some(EntityType::Extended(x)) = doc.get_entity(Handle::new(alignment)) else { return Vec::new() };
    let ExtendedEntityData::Arch(a) = &x.data else { return Vec::new() };
    let mut out: Vec<PathPoint> = Vec::new();
    for (_, _, points) in a.alignment_pieces_with_curvature() {
        let skip = usize::from(out.last().zip(points.first()).is_some_and(|(p, q)| p.0.distance(q) < 1e-9));
        out.extend(points.into_iter().skip(skip).map(|p| (p, None)));
    }
    out
}

/// Whether two solved paths cross or touch (arcs taken by their chords
/// and their middle points).
fn paths_meet(a: &[PathPoint], b: &[PathPoint]) -> bool {
    super::arrangement::paths_meet(a, b)
}

/// Adds an alignment like [`super::add_alignment_with_curves`] on `site`
/// instead of siteless: its edge joins the site's container. Like a lot
/// line it may cross and touch the site's lot lines and alignments (not
/// its feature lines; arcs included): they are split
/// where they meet and every face it closes or cuts off becomes a parcel
/// (see [`add_lot_line`]). The site must be one this crate writes.
pub fn add_alignment_on_site(doc: &mut CadDocument, site: Handle, name: &str, vertices: &[Vector2], curves: &[VertexCurve], layer: Option<&str>) -> Result<Handle, DxfError> {
    if vertices.len() < 2 || vertices.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) || vertices.windows(2).any(|w| w[0] == w[1]) {
        return Err(invalid("an alignment needs two or more distinct finite vertices"));
    }
    if curves.len() + 2 != vertices.len() {
        return Err(invalid("an alignment needs one curve per inner vertex"));
    }
    // A spanning arc replaces a segment, which the site's topology (one
    // member per segment) has no place for.
    if curves.iter().any(|c| c.spans()) {
        return Err(invalid("an alignment on a site cannot carry spanning arcs (write it siteless)"));
    }
    let elements = elements(vertices, curves);
    let path = solve(vertices, curves, &elements).ok_or_else(|| invalid("a curve has no turn or is not positive, or the curves do not fit on the lines"))?;
    if alignment_style(doc).is_none() {
        ensure_alignment_tree(doc)?;
    }
    let style = alignment_style(doc).ok_or_else(|| invalid("drawing has no alignment styles"))?;
    let elevations = vec![0.0; path.len()];
    let (alignment, _) = join_site(doc, site, path, &elevations, MemberKind::Alignment, true, layer, None, |doc, common, edge, container| write_alignment(doc, common, name, &elements, style, container.value(), edge, None, &[], &[], &[], &[], None))?;
    Ok(alignment)
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

/// The vertices of a Civil 3D feature line: the points of its edge in
/// its site container's [`Topology`](crate::classes::civil::Topology) —
/// the PIs and elevation points with their elevations and the bulge of
/// the segment starting at each (see [`FeatureLineVertex`]). `None` when
/// the record is not a feature line, its site container or its edge
/// cannot be read, or the container keeps no edge with its id.
pub fn feature_line_vertices(doc: &CadDocument, feature_line: Handle) -> Option<Vec<FeatureLineVertex>> {
    let Some(EntityType::Extended(x)) = doc.get_entity(feature_line) else { return None };
    let ExtendedEntityData::Arch(a) = &x.data else { return None };
    let container = crate::types::Handle::new(a.feature_line_container()?);
    let o = match doc.objects.get(&container) {
        Some(crate::objects::ObjectType::Arch(o)) => o,
        _ => return None,
    };
    let topology = o.feature_line_topology()?;
    let edge = a.feature_line_edge()?;
    let index = topology.edge_ids.iter().position(|id| *id == edge)?;
    Some(topology.edges[index].iter().map(|v| FeatureLineVertex {
        point: crate::types::Vector3::new(v.point.x, v.point.y, v.z),
        bulge: v.bulge,
        elevation_point: v.is_elevation_point(),
    }).collect())
}

/// Label property of a parcel style's name (the style name token in its
/// name template, `<[167804933(...)]>`).
pub const PARCEL_STYLE_NAME: u32 = 167_804_933;
/// Label property of a parcel's number (the number token in a parcel
/// style's name template, `<[16809985(...)]>`).
pub const PARCEL_NUMBER: u32 = 16_809_985;

/// Formatting option of a name template's style token: show the style's
/// name in upper case (`Parcel-2C.dwg`: "Road (Local)" reads as
/// "ROAD (LOCAL)"; 2449 keeps it as written — "Standard : 1").
const NAME_UPPER_CASE: u32 = 2450;

/// The name of a Civil 3D parcel: the string its record stores, or, when
/// the record stores none (2027 samples: `Parcel-2C.dwg`'s parcels name
/// empty where the application reads "ROAD (LOCAL): 100"), its style's
/// name template evaluated — the style's own name (upper case when the
/// template's style token asks for it) and the parcel's number. `None`
/// when the record is not a parcel, or the template, the style's name or
/// the number cannot be read (a style without a name of its own — this
/// crate's default parcel style — evaluates to no name, as the
/// application keeps no name for it in the record either).
pub fn parcel_name(doc: &CadDocument, parcel: Handle) -> Option<String> {
    let Some(EntityType::Extended(x)) = doc.get_entity(parcel) else { return None };
    let ExtendedEntityData::Arch(a) = &x.data else { return None };
    // A record that stores its name (the application's older form,
    // `Labels-Parcels.dwg` "Road : 9") reads it directly.
    let (stored, _) = a.face_name().unwrap_or_default();
    if !stored.is_empty() {
        return Some(stored);
    }
    let style = crate::types::Handle::new(a.parcel_style()?);
    let o = match doc.objects.get(&style) {
        Some(crate::objects::ObjectType::Arch(o)) if o.dxf_name == "AECC_PARCEL_STYLE" => o,
        _ => return None,
    };
    let strings = o.decode().strings;
    let template_at = strings.iter().position(|s| s.contains("<["))?;
    let style_name = strings[..template_at].iter().find(|s| !s.is_empty())?;
    let number = a.parcel_face()?.0;
    let mut name = String::new();
    for part in super::super::labels::parse_format(&strings[template_at]) {
        match part {
            super::super::labels::LabelPart::Text(text) => name.push_str(&text),
            super::super::labels::LabelPart::NewLine => name.push('\n'),
            super::super::labels::LabelPart::Field { property, options } => match property {
                PARCEL_STYLE_NAME => {
                    name.push_str(&if options.contains(&NAME_UPPER_CASE) { style_name.to_ascii_uppercase() } else { style_name.clone() });
                }
                PARCEL_NUMBER => name.push_str(&number.to_string()),
                _ => return None,
            },
        }
    }
    (!name.is_empty()).then_some(name)
}

/// Adds a feature line through `points` (x, y, elevation; straight
/// segments, every point a PI) on `site`; see [`add_feature_line_vertices`].
pub fn add_feature_line(doc: &mut CadDocument, site: Handle, name: &str, points: &[crate::types::Vector3], layer: Option<&str>) -> Result<Handle, DxfError> {
    let vertices: Vec<FeatureLineVertex> = points.iter().map(|&point| FeatureLineVertex { point, bulge: 0.0, elevation_point: false }).collect();
    add_feature_line_vertices(doc, site, name, &vertices, layer)
}

/// Adds a feature line through `vertices` on `site`: its edge joins the
/// site's container, split where it crosses or touches the site's other
/// members (straight segments; a crossing takes the new line's elevation
/// there), and the site must be one this crate writes. The ends are PIs; an elevation point on an arc splits it, so
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
    let (container, mut members, parcels, line_style, next) = members_of(doc, site)?;
    let arrange = |members: &[Member]| {
        let inputs = member_inputs(members);
        if inputs.is_empty() { Ok(None) } else { build(&inputs).map(Some).map_err(invalid) }
    };
    let before = arrange(&members)?;
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
        ids: Vec::new(),
        read: None,
    });
    // A feature line that splits a parcel's face leaves the parcel over
    // the parts (PD01, PD05, PD06, PD09: one parcel over two faces; PD08:
    // over four). One that closes or splits a region no parcel covers
    // makes the application add a grading of that face (FX04, FX07),
    // which is not written.
    let after = arrange(&members)?;
    let (Some(old), Some(new)) = (&before, &after) else {
        write_container(doc, container, site, &members, &parcels, line_style, next)?;
        return write_feature_line(doc, common, name, members.len() as i32, container, line_style);
    };
    if old.faces.len() == new.faces.len() {
        write_container(doc, container, site, &members, &parcels, line_style, next)?;
        let line = write_feature_line(doc, common, name, members.len() as i32, container, line_style)?;
        // A parcel whose face the line entered takes the face's new id and
        // label (PD03: face 3, (25, 25)).
        for (i, h) in parcels.iter().enumerate().filter(|(i, h)| **h != 0 && old.face_ids.get(i + 1) != new.face_ids.get(i + 1)) {
            let Some(EntityType::Extended(x)) = doc.get_entity(Handle::new(*h)) else { continue };
            let ExtendedEntityData::Arch(a) = &x.data else { continue };
            let refs = a.typed_handles();
            let (Some((number, _)), style, label_style) = (a.parcel_face(), refs.get(3).map_or(0, |r| r.0), refs.get(4).map_or(0, |r| r.0)) else { continue };
            let common = x.common.clone();
            write_parcel(doc, common, site, number, new.face_ids[i + 1], &new.faces[i + 1].outline(), new.faces[i + 1].label, style, label_style)?;
        }
        return Ok(line);
    }
    let owners: Vec<u64> = (1..new.faces.len())
        .map(|i| old.face_at(new.faces[i].label, 0).and_then(|f| parcels.get(f as usize - 2).copied()).filter(|h| *h != 0))
        .collect::<Option<_>>()
        .ok_or_else(|| invalid("the feature line closes or splits a region of the site no parcel covers"))?;
    let mut distinct = parcels.clone();
    distinct.sort_unstable();
    distinct.dedup();
    let earlier: Vec<(u64, i32, Vector2, u64, u64, EntityCommon)> = distinct
        .iter()
        .filter(|h| **h != 0)
        .map(|h| match doc.get_entity(Handle::new(*h)) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => {
                    let refs = a.typed_handles();
                    a.parcel_face().zip(a.parcel_label()).map(|((n, _), l)| (*h, n, l, refs.get(3).map_or(0, |r| r.0), refs.get(4).map_or(0, |r| r.0), x.common.clone()))
                }
                _ => None,
            },
            _ => None,
        })
        .collect::<Option<_>>()
        .ok_or_else(|| invalid("a parcel of the site cannot be read"))?;
    write_container(doc, container, site, &members, &owners, line_style, next)?;
    let line = write_feature_line(doc, common, name, members.len() as i32, container, line_style)?;
    for (h, number, label, style, label_style, common) in earlier {
        let faces: Vec<i32> = owners.iter().enumerate().filter(|(_, o)| **o == h).map(|(i, _)| i as i32 + 2).collect();
        // The label's elevation: the mean of the nodes on the parcel's
        // outline, the segments with its faces on one side only (PD01:
        // 11 / 6; PD08: 22 / 8).
        let mut outline: Vec<i32> = Vec::new();
        for e in &new.edges {
            for (k, v) in e.vertices.iter().enumerate() {
                let Some(next) = e.vertices.get(k + 1) else { continue };
                if faces.contains(&v.faces.0) != faces.contains(&v.faces.1) {
                    outline.extend([v.node, next.node]);
                }
            }
        }
        outline.sort_unstable();
        outline.dedup();
        let z = outline.iter().map(|n| new.nodes[*n as usize - 1].1).sum::<f64>() / outline.len().max(1) as f64;
        // Its faces by id (PD01: 4, 5; PD08: 8 to 11).
        let mut ids: Vec<i32> = faces.iter().map(|f| new.face_id(*f)).collect();
        ids.sort_unstable();
        write_parcel_over(doc, common, site, number, &ids, &[], label, z, style, label_style)?;
    }
    Ok(line)
}

/// Replaces the vertices of `feature_line` (a feature line on a site this
/// crate writes) with `vertices` (the rules of
/// [`add_feature_line_vertices`]): the site's container is written again
/// with the line's new points. A point at the place of one the line had
/// keeps its node id, and an elevation point that stays one its key; the
/// other points take the container's next free ids and keys, as the
/// application gives the points it inserts. The line must not cross or
/// touch the site's other members.
pub fn set_feature_line_vertices(doc: &mut CadDocument, feature_line: Handle, vertices: &[FeatureLineVertex]) -> Result<(), DxfError> {
    let n = vertices.len();
    if n < 2 || vertices.iter().any(|v| !v.point.x.is_finite() || !v.point.y.is_finite() || !v.point.z.is_finite() || !v.bulge.is_finite()) || vertices.windows(2).any(|w| w[0].point.x == w[1].point.x && w[0].point.y == w[1].point.y) {
        return Err(invalid("a feature line needs two or more finite points, apart in plan"));
    }
    if vertices[0].elevation_point || vertices[n - 1].elevation_point {
        return Err(invalid("the ends of a feature line are PIs"));
    }
    let Some(EntityType::Extended(x)) = doc.get_entity(feature_line) else { return Err(invalid("feature line not found")) };
    let ExtendedEntityData::Arch(a) = &x.data else { return Err(invalid("the record is not a feature line")) };
    if a.dxf_name != "AECC_FEATURE_LINE" {
        return Err(invalid("the record is not a feature line"));
    }
    let container = a.typed_handles().get(1).map(|r| Handle::new(r.0)).ok_or_else(|| invalid("the feature line has no container"))?;
    let site = object(doc, container).map(|o| o.owner).ok_or_else(|| invalid("the feature line's container is missing"))?;
    let (container, mut members, parcels, line_style, next) = members_of(doc, site)?;
    let i = members.iter().position(|m| m.handle == feature_line.value()).ok_or_else(|| invalid("the site does not list the feature line"))?;
    let path: Vec<PathPoint> = vertices.iter().enumerate().map(|(i, v)| (Vector2::new(v.point.x, v.point.y), Some(v.bulge).filter(|b| *b != 0.0 && i + 1 < n))).collect();
    if members.iter().enumerate().any(|(j, m)| j != i && paths_meet(&m.outline(), &path)) {
        return Err(invalid("the feature line crosses or touches another member of the site"));
    }
    let old = &members[i];
    let ids = vertices
        .iter()
        .map(|v| match old.path.iter().position(|p| p.0.x == v.point.x && p.0.y == v.point.y) {
            Some(k) => (old.ids.get(k).map_or(0, |x| x.0), if v.elevation_point && old.elevation_points[k] { old.ids.get(k).map_or(0, |x| x.1) } else { 0 }),
            None => (0, 0),
        })
        .collect();
    let read = old.read.as_ref().map(|r| ReadEdge { flags: r.flags, points: Vec::new(), inserted: r.inserted.clone(), shape: Vec::new() });
    members[i] = Member {
        read,
        handle: old.handle,
        path,
        elevations: vertices.iter().map(|v| v.point.z).collect(),
        elevation_points: vertices.iter().map(|v| v.elevation_point).collect(),
        kind: MemberKind::FeatureLine,
        ids,
    };
    write_container(doc, container, site, &members, &parcels, line_style, NextIds { edited: true, ..next })?;
    Ok(())
}

/// Label style tree paths (under the label node) of the default parcel
/// area label style: drawings of the older template and of the newer one
/// (WC64base).
const PARCEL_AREA_LABELS: [[i32; 3]; 2] = [[2, 0x0F00_C032, DEFAULT_STYLE_ID], [2, 0x0F00_B432, DEFAULT_STYLE_ID]];

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
fn write_lot_line(doc: &mut CadDocument, common: EntityCommon, edge: i32, container: Handle, graphic: Option<Vec<u8>>) -> Result<Handle, DxfError> {
    let mut common = common;
    common.graphic_data = graphic;
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
/// 3BD label location, BL 2, BL n, n x BL face, BL 0, B 0, BS 0x04BE
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
/// The proxy-graphics class word and kind the application writes on
/// parcel records (Labels-Parcels.dwg; `AeccDbFace (AeccLand130)`).
const PARCEL_GRAPHIC_KIND: u32 = 3;
const PARCEL_GRAPHIC_WORD: &str = "AeccDbFace (AeccLand130)";

fn write_parcel(doc: &mut CadDocument, common: EntityCommon, site: Handle, number: i32, face: i32, outline: &[Vec<Vector3>], label: Vector2, style: u64, label_style: u64) -> Result<Handle, DxfError> {
    write_parcel_over(doc, common, site, number, &[face], outline, label, 0.0, style, label_style)
}

/// [`write_parcel`] over the faces `faces` (`BL 2, BL n, n x BL face`; a
/// feature line splitting a parcel's face leaves the parcel over the
/// parts, PD01) with the label at elevation `z`.
#[allow(clippy::too_many_arguments)]
fn write_parcel_over(doc: &mut CadDocument, mut common: EntityCommon, site: Handle, number: i32, faces: &[i32], outline: &[Vec<Vector3>], label: Vector2, z: f64, style: u64, label_style: u64) -> Result<Handle, DxfError> {
    let byte = |w: &mut crate::io::dwg::dwg_stream_writers::DwgMergedWriter, v: u8| bits(w, &format!("{v:08b}"));
    // The outline the record's proxy graphics sketch (one closed piece
    // per boundary loop); a parcel over several faces has no single face
    // loop, and goes without one.
    common.graphic_data = (!outline.is_empty())
        .then(|| graphic_image(PARCEL_GRAPHIC_KIND, PARCEL_GRAPHIC_WORD, 2, Vector3::new(label.x, label.y, z), outline));
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
                for v in [label.x, label.y, z] {
                    w.write_bit_double(v);
                }
                for v in if round == 0 { [80, 1] } else { [2, faces.len() as i32] } {
                    w.write_bit_long(v);
                }
            }
            for face in faces {
                w.write_bit_long(*face);
            }
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
fn add_lot_line_on(doc: &mut CadDocument, site: Handle, points: &[(Vector2, f64)], closed: bool, meet: bool, layer: Option<&str>, member_record: Option<u64>, record: impl FnOnce(&mut CadDocument, EntityCommon, i32, Handle) -> Result<Handle, DxfError>) -> Result<(Handle, Vec<Handle>), DxfError> {
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
    let elevations = vec![0.0; n];
    join_site(doc, site, path, &elevations, MemberKind::LotLine { closed }, meet, layer, member_record, record)
}

/// The standalone parcel-segment record (a decoded lot line) on `site`'s
/// container: [`write_lot_line`]'s record form with the edge id, and —
/// when the oracle record carries a graphic-image section — that section
/// rebuilt from `geometry`: the class-name text at the label point and
/// the extents rectangle walked around its corners (#591/#595's decode;
/// the #665 outline-ring pattern). A record stored without a base curve
/// (geometry `None`) is written without one, as stored. The site's
/// container members are not rebuilt; the record is what the application
/// censuses as `AeccDbLotLine`.
pub fn add_parcel_segment_record(doc: &mut CadDocument, site: Handle, edge: i32, geometry: Option<&crate::classes::civil::ParcelSegmentGeometry>, layer: Option<&str>) -> Result<Handle, DxfError> {
    let (container, _members, _parcels, _line_style, _next) = members_of(doc, site)?;
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let common = EntityCommon {
        handle: doc.allocate_handle(),
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    let graphic = geometry.map(|g| {
        let ring: Vec<Vector3> = g.corners.iter().copied().chain(std::iter::once(g.corners[0])).map(|p| Vector3::new(p.x, p.y, 0.0)).collect();
        graphic_image(PARCEL_GRAPHIC_KIND, "AeccDbLotLine (AeccLand130)", 1, Vector3::new(g.label_point.x, g.label_point.y, 0.0), &[ring])
    });
    write_lot_line(doc, common, edge, container, graphic)
}

/// Joins a new member through `path` to `site`'s container and writes the
/// parcels of the faces it makes: with `meet`, it may cross or touch the
/// site's lot lines and alignments (arcs included): each is split
/// where they meet, the faces they cut are split and the parcels of the
/// parts are written (every parcel's label moves into what is left of its
/// face). `record` writes the member's own record (its entity, edge id and
/// the container). `member_record` gives the member's record handle when
/// the record already exists (a parcel-segment record the file's set
/// carries): the member joins under that handle — the container's edge
/// slots point at it — and `record` is not called. Returns the record and
/// the new parcels.
fn join_site(doc: &mut CadDocument, site: Handle, path: Vec<PathPoint>, elevations: &[f64], kind: MemberKind, meet: bool, layer: Option<&str>, member_record: Option<u64>, record: impl FnOnce(&mut CadDocument, EntityCommon, i32, Handle) -> Result<Handle, DxfError>) -> Result<(Handle, Vec<Handle>), DxfError> {
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let line = EntityCommon {
        handle: member_record.map_or_else(|| doc.allocate_handle(), Handle::new),
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    join_site_as(doc, site, line, path, elevations, kind, meet, record)
}

/// [`join_site`] for a member whose record is `line` (its handle and
/// layer, which the parcels it makes take too).
#[allow(clippy::too_many_arguments)]
pub(super) fn join_site_as(doc: &mut CadDocument, site: Handle, line: EntityCommon, path: Vec<PathPoint>, elevations: &[f64], kind: MemberKind, meet: bool, record: impl FnOnce(&mut CadDocument, EntityCommon, i32, Handle) -> Result<Handle, DxfError>) -> Result<(Handle, Vec<Handle>), DxfError> {
    let (layer, layer_handle) = (line.layer.clone(), line.layer_handle);
    let (container, mut members, parcels, line_style, next) = members_of(doc, site)?;
    let probe = Member { handle: 0, path: path.clone(), elevations: Vec::new(), elevation_points: Vec::new(), kind, ids: Vec::new(), read: None };
    let outline = probe.outline();
    let met = members.iter().filter(|m| paths_meet(&m.outline(), &outline)).map(|m| m.kind).collect::<Vec<_>>();
    if met.iter().any(|k| !meet || (matches!(k, MemberKind::FeatureLine) && !matches!(kind, MemberKind::LotLine { .. }))) {
        return Err(invalid("the line crosses or touches a member it may not meet"));
    }
    // New parcels take the drawing's default parcel style (WC64: the site
    // "Property", its parcels "Basic"), else the site's.
    let style = match default_style_of(doc, KEY_PARCEL_STYLES) {
        Some(s) => s,
        None => match doc.get_entity(site) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => a.typed_handles().get(6).map_or(0, |r| r.0),
                _ => 0,
            },
            _ => 0,
        },
    };
    let label_style = PARCEL_AREA_LABELS.iter().find_map(|p| super::labels::default_label_style(doc, *p)).unwrap_or(0);
    let common = |doc: &mut CadDocument| EntityCommon {
        handle: doc.allocate_handle(),
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.clone(),
        layer_handle,
        ..EntityCommon::default()
    };
    // A lot line crossing a feature line takes the feature line's
    // elevation there, and its stretches between such points and its own
    // points run straight between their elevations (PD20: (50, 25) at
    // 5.5, (50, 0) and (50, 50) at 2.444).
    let (path, elevations) = if matches!(kind, MemberKind::LotLine { .. }) { raised_by_feature_lines(&members, path, elevations) } else { (path, elevations.to_vec()) };
    let n = path.len();
    members.push(Member { handle: line.handle.value(), path, elevations, elevation_points: vec![false; n], kind, ids: Vec::new(), read: None });
    // Members without points (profiles, members moved onto the site) take
    // no part in the arrangement.
    let inputs = member_inputs(&members);
    let arrangement = build(&inputs).map_err(invalid)?;
    let faces = arrangement.faces.len();
    // A parcel lies over the faces feature lines split (PD01); a lot line
    // or alignment across such a parcel splits it along its own segments
    // only (PD20).
    let group = face_groups(&arrangement, &members);
    if (1..faces).any(|i| group[i] != i) {
        return join_groups(doc, site, container, &members, &parcels, line_style, next, &arrangement, &group, &met, line, record, style, label_style, common);
    }
    // The earlier parcels: (record, number, face, label).
    let earlier: Vec<(u64, i32, i32, Vector2)> = parcels
        .iter()
        .map(|h| match doc.get_entity(Handle::new(*h)) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => a.parcel_face().zip(a.parcel_label()).map(|((n, f), l)| (*h, n, f, l)),
                _ => None,
            },
            _ => None,
        })
        .collect::<Option<_>>()
        .ok_or_else(|| invalid("a parcel of the site cannot be read"))?;
    // A parcel keeps the face its label lies in; the faces left over get
    // new parcels, the new faces first (WP19, WP20).
    let mut owner: Vec<Option<(u64, i32)>> = vec![None; faces];
    for &(h, number, face, label) in &earlier {
        let face = arrangement.face_number(face).unwrap_or(0);
        let f = if met.is_empty() { Some(face).filter(|f| *f > 0) } else { arrangement.face_at(label, face) };
        let slot = f.and_then(|f| owner.get_mut(f as usize - 1)).filter(|s| s.is_none()).ok_or_else(|| invalid("a parcel's label is in no face of its own"))?;
        *slot = Some((h, number));
    }
    let before = earlier.len() + 1;
    let mut free: Vec<usize> = (1..faces).filter(|i| owner[*i].is_none()).collect();
    free.sort_by_key(|i| (*i < before, *i));
    let mut number = earlier.iter().map(|e| e.1).max().unwrap_or(0);
    let mut made = Vec::new();
    for i in free {
        let c = common(doc);
        number += 1;
        owner[i] = Some((c.handle.value(), number));
        made.push((c, number, i as i32 + 1));
    }
    let parcels: Vec<u64> = owner[1.min(faces)..].iter().map(|o| o.map_or(0, |o| o.0)).collect();
    let arrangement = write_container(doc, container, site, &members, &parcels, line_style, next)?;
    let segment = record(doc, line, members.len() as i32, container)?;
    // The earlier parcels keep their records and numbers; their faces and
    // labels follow the split.
    if !met.is_empty() {
        for (i, o) in owner.iter().enumerate().skip(1) {
            let Some((h, number)) = *o else { continue };
            if made.iter().any(|m| m.0.handle.value() == h) {
                continue;
            }
            let Some(EntityType::Extended(x)) = doc.get_entity(Handle::new(h)) else { continue };
            let ExtendedEntityData::Arch(a) = &x.data else { continue };
            let refs = a.typed_handles();
            let (style, label_style) = (refs.get(3).map_or(style, |r| r.0), refs.get(4).map_or(label_style, |r| r.0));
            let common = x.common.clone();
            write_parcel(doc, common, site, number, arrangement.face_ids[i], &arrangement.faces[i].outline(), arrangement.faces[i].label, style, label_style)?;
        }
    }
    let mut out = Vec::new();
    for (c, number, face) in made {
        out.push(write_parcel(doc, c, site, number, arrangement.face_id(face), &arrangement.faces[face as usize - 1].outline(), arrangement.faces[face as usize - 1].label, style, label_style)?);
    }
    Ok((segment, out))
}

/// `path` (straight segments) with a point added where it crosses a
/// feature line of `members`, at the feature line's elevation there.
// ponytail: arcs on either line are left as they are.
fn raised_by_feature_lines(members: &[Member], path: Vec<PathPoint>, elevations: &[f64]) -> (Vec<PathPoint>, Vec<f64>) {
    let straight = |p: &[PathPoint]| p.iter().all(|q| q.1.is_none_or(|b| b == 0.0));
    if !straight(&path) || elevations.len() != path.len() {
        return (path, elevations.to_vec());
    }
    let cross = |a: Vector2, b: Vector2, c: Vector2, d: Vector2| {
        let (r, s) = (b - a, d - c);
        let den = r.cross(&s);
        if den.abs() < 1e-12 {
            return None;
        }
        let (t, u) = ((c - a).cross(&s) / den, (c - a).cross(&r) / den);
        (t > 1e-9 && t < 1.0 - 1e-9 && (-1e-9..=1.0 + 1e-9).contains(&u)).then_some((t, u))
    };
    let mut out_path = Vec::new();
    let mut out_z = Vec::new();
    for k in 0..path.len() {
        out_path.push(path[k]);
        out_z.push(elevations[k]);
        let Some(next) = path.get(k + 1) else { break };
        let (a, b) = (path[k].0, next.0);
        let mut hits: Vec<(f64, f64)> = Vec::new();
        for m in members.iter().filter(|m| matches!(m.kind, MemberKind::FeatureLine) && straight(&m.path) && m.elevations.len() == m.path.len()) {
            for j in 0..m.path.len().saturating_sub(1) {
                if let Some((t, u)) = cross(a, b, m.path[j].0, m.path[j + 1].0) {
                    hits.push((t, m.elevations[j] + u * (m.elevations[j + 1] - m.elevations[j])));
                }
            }
        }
        hits.sort_by(|x, y| x.0.total_cmp(&y.0));
        for (t, z) in hits {
            out_path.push((a + (b - a) * t, None));
            out_z.push(z);
        }
    }
    (out_path, out_z)
}

/// The faces of `arrangement` joined across feature lines: the root face
/// index of each face's group (feature lines do not bound parcels).
fn face_groups(arrangement: &Arrangement, members: &[Member]) -> Vec<usize> {
    let mut root: Vec<usize> = (0..arrangement.faces.len()).collect();
    fn find(root: &mut [usize], i: usize) -> usize {
        let mut r = i;
        while root[r] != r {
            r = root[r];
        }
        root[i] = r;
        r
    }
    for (e, m) in arrangement.edges.iter().zip(members.iter().filter(|m| !m.path.is_empty())) {
        if !matches!(m.kind, MemberKind::FeatureLine) {
            continue;
        }
        for v in &e.vertices {
            let (a, b) = (v.faces.0 as usize, v.faces.1 as usize);
            // Face 1 (the outside) joins nothing.
            if a < 2 || b < 2 || a > root.len() || b > root.len() {
                continue;
            }
            let (ra, rb) = (find(&mut root, a - 1), find(&mut root, b - 1));
            if ra != rb {
                root[ra.max(rb)] = ra.min(rb);
            }
        }
    }
    (0..root.len()).map(|i| find(&mut root, i)).collect()
}

/// [`join_site`] where feature lines join faces into groups: each group is
/// one parcel over its faces (`BL 2, BL n, faces`). An earlier parcel keeps
/// the group its label lies in, the groups left over get new parcels; a
/// parcel's label goes to the middle of its outline's extents (inside it;
/// else its first face's label), at the mean elevation of the outline's
/// nodes (PD20: lot line x = 50 across PD01's parcel: two parcels of 2500,
/// labels (25, 25) and (75, 25)).
// ponytail: the application also raises the lot line's nodes to the
// feature line's elevation where they cross and interpolates its other
// nodes along it (PD20: (50, 0) and (50, 50) at 2.444); the writer keeps
// the arrangement's elevations.
#[allow(clippy::too_many_arguments)]
fn join_groups(
    doc: &mut CadDocument,
    site: Handle,
    container: Handle,
    members: &[Member],
    parcels: &[u64],
    line_style: u64,
    next: NextIds,
    arrangement: &Arrangement,
    group: &[usize],
    met: &[MemberKind],
    line: EntityCommon,
    record: impl FnOnce(&mut CadDocument, EntityCommon, i32, Handle) -> Result<Handle, DxfError>,
    style: u64,
    label_style: u64,
    common: impl Fn(&mut CadDocument) -> EntityCommon,
) -> Result<(Handle, Vec<Handle>), DxfError> {
    let faces = arrangement.faces.len();
    let mut distinct: Vec<u64> = parcels.iter().copied().filter(|h| *h != 0).collect();
    distinct.sort_unstable();
    distinct.dedup();
    let earlier: Vec<(u64, i32, i32, Vector2)> = distinct
        .iter()
        .map(|h| match doc.get_entity(Handle::new(*h)) {
            Some(EntityType::Extended(x)) => match &x.data {
                ExtendedEntityData::Arch(a) => a.parcel_face().zip(a.parcel_label()).map(|((n, f), l)| (*h, n, f, l)),
                _ => None,
            },
            _ => None,
        })
        .collect::<Option<_>>()
        .ok_or_else(|| invalid("a parcel of the site cannot be read"))?;
    let mut owner: Vec<Option<(u64, i32)>> = vec![None; faces];
    for &(h, number, face, label) in &earlier {
        let face = arrangement.face_number(face).unwrap_or(0);
        let f = if met.is_empty() { Some(face).filter(|f| *f > 0) } else { arrangement.face_at(label, face) };
        let slot = f.and_then(|f| owner.get_mut(group[f as usize - 1])).filter(|s| s.is_none()).ok_or_else(|| invalid("a parcel's label is in no face of its own"))?;
        *slot = Some((h, number));
    }
    let mut number = earlier.iter().map(|e| e.1).max().unwrap_or(0);
    let mut made: Vec<EntityCommon> = Vec::new();
    for i in 1..faces {
        if group[i] == i && owner[i].is_none() {
            let c = common(doc);
            number += 1;
            owner[i] = Some((c.handle.value(), number));
            made.push(c);
        }
    }
    let per_face: Vec<u64> = (1..faces).map(|i| owner[group[i]].map_or(0, |o| o.0)).collect();
    let arrangement = write_container(doc, container, site, members, &per_face, line_style, next)?;
    let segment = record(doc, line, members.len() as i32, container)?;
    let mut out = Vec::new();
    for root in (1..faces).filter(|i| group[*i] == *i) {
        let Some((h, number)) = owner[root] else { continue };
        let face_ids: Vec<i32> = (1..faces).filter(|i| group[*i] == root).map(|i| i as i32 + 1).collect();
        let inside = |f: i32| f >= 2 && group.get(f as usize - 1) == Some(&root);
        let mut outline: Vec<i32> = Vec::new();
        for e in &arrangement.edges {
            for (k, v) in e.vertices.iter().enumerate() {
                let Some(next) = e.vertices.get(k + 1) else { continue };
                if inside(v.faces.0) != inside(v.faces.1) {
                    outline.extend([v.node, next.node]);
                }
            }
        }
        outline.sort_unstable();
        outline.dedup();
        let nodes: Vec<(Vector2, f64)> = outline.iter().map(|n| arrangement.nodes[*n as usize - 1]).collect();
        let z = nodes.iter().map(|n| n.1).sum::<f64>() / nodes.len().max(1) as f64;
        let (lo, hi) = nodes.iter().fold((Vector2::new(f64::INFINITY, f64::INFINITY), Vector2::new(f64::NEG_INFINITY, f64::NEG_INFINITY)), |(lo, hi), (p, _)| (Vector2::new(lo.x.min(p.x), lo.y.min(p.y)), Vector2::new(hi.x.max(p.x), hi.y.max(p.y))));
        let middle = Vector2::new((lo.x + hi.x) / 2.0, (lo.y + hi.y) / 2.0);
        let mut ids: Vec<i32> = face_ids.iter().map(|f| arrangement.face_id(*f)).collect();
        ids.sort_unstable();
        let label = match arrangement.face_at(middle, face_ids[0]) {
            Some(f) if inside(f) => middle,
            _ => arrangement.faces[root].label,
        };
        match made.iter().position(|c| c.handle.value() == h) {
            Some(k) => out.push(write_parcel_over(doc, made[k].clone(), site, number, &ids, &[], label, z, style, label_style)?),
            None => {
                let Some(EntityType::Extended(x)) = doc.get_entity(Handle::new(h)) else { continue };
                let ExtendedEntityData::Arch(a) = &x.data else { continue };
                let refs = a.typed_handles();
                let (style, label_style) = (refs.get(3).map_or(style, |r| r.0), refs.get(4).map_or(label_style, |r| r.0));
                let common = x.common.clone();
                write_parcel_over(doc, common, site, number, &ids, &[], label, z, style, label_style)?;
            }
        }
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
    let (_, parcels) = add_lot_line_on(doc, site, &points, true, false, layer, None, |doc, common, edge, container| write_lot_line(doc, common, edge, container, None))?;
    parcels.into_iter().next().ok_or_else(|| invalid("the lot line closes no face"))
}

/// [`add_parcel`] for a composed build, where the file's lot-line records
/// come from its own parcel-segment set ([`add_parcel_segment_record`]):
/// the closed lot line still joins the site's container and its inside
/// still becomes the parcel's face, but this adds no `AECC_PARCEL_SEGMENT`
/// record of its own. The oracles carry exactly one record per lot line
/// and a parcel never adds a second record for a boundary the set already
/// carries (Align-2: the parcel boundary IS the file's single segment,
/// 1 parcel, 1 record; Assembly-1a: 102 records for 56 parcels whose
/// faces those 102 bound — only 3 rings are single lot lines; Parcel-2C:
/// 30 records for 11 parcels; Labels-Parcels: 55 for 54). Write the
/// parcel-segment set first: when one of its records carries this ring
/// (its extents rectangle equal to it, the created-from-object shape —
/// Align-2's), the parcel's edge slot in the container adopts that
/// record, the shape the oracle itself stores; when none does, the slot
/// keeps the boundary's reserved handle, which carries no record (the
/// application reads the faces from the container, the lot lines from
/// the records). Returns the parcel.
pub fn add_parcel_without_segment(doc: &mut CadDocument, site: Handle, vertices: &[Vector2], layer: Option<&str>) -> Result<Handle, DxfError> {
    let points: Vec<(Vector2, f64)> = vertices.iter().map(|p| (*p, 0.0)).collect();
    // The site's own container: the record a ring adopts must be one of
    // this site's lot lines.
    let (container, _, _, _, _) = members_of(doc, site)?;
    let adopting = doc.entities().find_map(|e| {
        let EntityType::Extended(x) = e else { return None };
        let ExtendedEntityData::Arch(a) = &x.data else { return None };
        if a.dxf_name != "AECC_PARCEL_SEGMENT" || !a.typed_handles().iter().any(|r| r.0 == container.value()) {
            return None;
        }
        let g = a.parcel_segment_geometry()?;
        (ring_is_the_extents(&g.corners, vertices)).then_some(x.common.handle)
    });
    let (_, parcels) = add_lot_line_on(doc, site, &points, true, false, layer, adopting.map(|h| h.value()), |_, common, _, _| Ok(common.handle))?;
    parcels.into_iter().next().ok_or_else(|| invalid("the lot line closes no face"))
}

/// Whether a closed lot line's `vertices` are, as a ring (either
/// direction, from any start), exactly the extents rectangle `corners`
/// stores (low, low-right, high, high-left).
fn ring_is_the_extents(corners: &[Vector2; 4], vertices: &[Vector2]) -> bool {
    if vertices.len() != 4 || vertices.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return false;
    }
    let eq = |a: &Vector2, b: &Vector2| a.x == b.x && a.y == b.y;
    (0..4).any(|sh| {
        (0..4).all(|k| eq(&corners[(sh + k) % 4], &vertices[k])) || (0..4).all(|k| eq(&corners[(sh + 4 - k) % 4], &vertices[k]))
    })
}

/// Adds a lot line (a parcel segment) through `vertices` on `site`, each
/// with the bulge of the segment that starts there (0 for a straight one;
/// positive counterclockwise, the tangent of a quarter of the arc's angle;
/// a closed line's last bulge is the closing segment's), closed back to
/// the first vertex when `closed`, and returns it with the parcels it
/// makes. It may cross and touch the site's other lot lines and its
/// alignments (not its feature lines; arcs included):
/// the lines are split where they meet, as the application splits them, every face the
/// line closes or cuts off becomes a parcel (numbered in the order the
/// faces are made) and the parcel of a face it cuts keeps the part the
/// application gives it. Parcels take the site's parcel style and the
/// drawing's default parcel area label style.
pub fn add_lot_line(doc: &mut CadDocument, site: Handle, vertices: &[(Vector2, f64)], closed: bool, layer: Option<&str>) -> Result<(Handle, Vec<Handle>), DxfError> {
    add_lot_line_on(doc, site, vertices, closed, true, layer, None, |doc, common, edge, container| write_lot_line(doc, common, edge, container, None))
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
        // Crossing AL1's arc is accepted (in a copy, so the drawing below
        // keeps two alignments).
        assert!(add_alignment_on_site(&mut doc.clone(), s1, "AL3", &[v(100.0, -20.0), v(100.0, 40.0)], &[], None).is_ok());
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
    fn members_without_points_keep_their_empty_edges() {
        let p = crate::types::Vector3::new;
        let mut doc = drawing();
        let s = add_site(&mut doc, "Site 1", "").unwrap();
        let a = add_alignment_on_site(&mut doc, s, "AL1", &[Vector2::new(0.0, 0.0), Vector2::new(100.0, 0.0)], &[], None).unwrap();
        // The container as the application leaves it when the alignment
        // has no points (WT04): an empty edge with flags 0x10, the outside
        // face with flags 6 and no entries.
        let (container, mut members, parcels, style, _) = members_of(&mut doc, s).unwrap();
        members[0].path.clear();
        members[0].read = Some(ReadEdge { flags: 0x10, points: Vec::new(), inserted: Vec::new(), shape: Vec::new() });
        write_container(&mut doc, container, s, &members, &parcels, style, NextIds { node: 5, outside: 6, ..NextIds::default() }).unwrap();
        let t = object(&doc, container).unwrap().topology().unwrap();
        assert_eq!((t.edges.len(), t.edges[0].len(), t.edge_flags[0], t.next_node, t.outside_face_flags), (1, 0, 0x10, 5, 6));
        // A feature line joins it: the empty edge is kept, the line's
        // nodes take the next free ids and it makes the outside face.
        add_feature_line(&mut doc, s, "FL2", &[p(0.0, 50.0, 5.0), p(100.0, 50.0, 6.0)], None).unwrap();
        let c = object(&doc, container).unwrap();
        let t = c.topology().unwrap();
        assert_eq!(t.edges.iter().map(|e| e.len()).collect::<Vec<_>>(), vec![0, 2]);
        assert_eq!(t.edges[1].iter().map(|v| v.node).collect::<Vec<_>>(), vec![5, 6]);
        assert_eq!((t.edge_flags[0], t.outside_face_flags), (0x10, 14));
        assert_eq!(c.site_members()[0], a.value());
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
    fn a_feature_line_along_another_shares_its_stretch() {
        let v = Vector2::new;
        let p = crate::types::Vector3::new;
        let mut doc = drawing();
        let s = add_site(&mut doc, "Site 1", "").unwrap();
        let f2 = add_feature_line(&mut doc, s, "FL2", &[p(0.0, 50.0, 5.0), p(100.0, 50.0, 6.0)], None).unwrap();
        let f3 = add_feature_line(&mut doc, s, "FL3", &[p(20.0, 50.0, 0.0), p(80.0, 50.0, 0.0)], None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-feature-line-along-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let c = object(&back, Handle::new(arch(&back, s).typed_handles()[5].0)).unwrap();
        assert_eq!(c.site_members(), vec![f2.value(), f3.value()]);
        // The shared stretch (20,50)-(80,50) starts with 0x2000 in the
        // earlier member and with 0x1000, without the follow marks, in the
        // later one.
        let edges = c.topology().unwrap().edges;
        let start = |mark: i32| edges.iter().flatten().find(|x| x.flags & mark != 0).map(|x| (x.point, x.flags & 0x30));
        assert_eq!(start(0x2000).map(|x| x.0), Some(v(20.0, 50.0)));
        assert_eq!(start(0x1000), Some((v(20.0, 50.0), 0)));
    }

    #[test]
    fn a_crossing_feature_line_splits_at_its_own_elevation() {
        let p = crate::types::Vector3::new;
        let mut doc = drawing();
        let s = add_site(&mut doc, "Site 1", "").unwrap();
        add_feature_line(&mut doc, s, "FL1", &[p(0.0, 0.0, 10.0), p(50.0, 0.0, 12.0), p(100.0, 30.0, 11.0)], None).unwrap();
        add_feature_line(&mut doc, s, "FLZ", &[p(25.0, -20.0, 8.0), p(25.0, 20.0, 9.0)], None).unwrap();
        // A third line closing a region with them is refused.
        assert!(add_feature_line(&mut doc, s, "FLW", &[p(10.0, 10.0, 5.0), p(90.0, 10.0, 7.0)], None).is_err());
        let container = arch(&doc, s).typed_handles()[5].0;
        let edges = object(&doc, Handle::new(container)).unwrap().topology().unwrap().edges;
        let at = |e: &Vec<crate::classes::civil::TopologyVertex>| e.iter().map(|v| (v.point.x, v.z, v.flags)).collect::<Vec<_>>();
        assert_eq!(at(&edges[0])[1], (25.0, 8.5, 0x430));
        // The new line's points are marked as the earlier lines' are.
        assert_eq!(at(&edges[1]), vec![(25.0, 8.0, 0xB2), (25.0, 8.5, 0x430), (25.0, 9.0, 0xA)]);
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
        let (_, members, _, _, _) = members_of(&mut back, s).unwrap();
        for (h, want) in [(fa, &a[..]), (fb, &b[..])] {
            let m = members.iter().find(|m| m.handle == h.value()).unwrap();
            assert_eq!(m.path.iter().map(|p| p.1.unwrap_or(0.0)).collect::<Vec<_>>(), want.iter().map(|v| v.bulge).collect::<Vec<_>>());
            assert_eq!(m.elevations, want.iter().map(|v| v.point.z).collect::<Vec<_>>());
            assert_eq!(m.elevation_points, want.iter().map(|v| v.elevation_point).collect::<Vec<_>>());
        }
    }


    #[test]
    fn feature_lines_across_lot_lines_number_the_crossings_again() {
        let p = crate::types::Vector3::new;
        let w = Vector2::new;
        let mut doc = drawing();
        let s = add_site(&mut doc, "Site 1", "").unwrap();
        let (_, made) = add_lot_line(&mut doc, s, &[(w(0.0, 0.0), 0.0), (w(100.0, 0.0), 0.0), (w(100.0, 50.0), 0.0), (w(0.0, 50.0), 0.0)], true, None).unwrap();
        add_feature_line(&mut doc, s, "FL1", &[p(-20.0, 25.0, 5.0), p(120.0, 25.0, 6.0)], None).unwrap();
        let (container, ..) = members_of(&mut doc, s).unwrap();
        let t = object(&doc, container).unwrap().topology().unwrap();
        // The crossings take 7 and 8 first, then 9 and 10 in the lot
        // line's order.
        let ids: Vec<(f64, i32)> = t.edges.iter().flatten().filter(|v| v.point.y == 25.0 && (v.point.x == 0.0 || v.point.x == 100.0)).map(|v| (v.point.x, v.node)).collect();
        assert!(ids.contains(&(100.0, 9)) && ids.contains(&(0.0, 10)), "{ids:?}");
        assert_eq!(t.next_node, 11);
        // The split takes face 3, then the parts are numbered again: 4 and
        // 5; the parcel lies over both, and the container reads back.
        let mut faces = t.face_ids.clone();
        faces.sort_unstable();
        assert_eq!(faces, vec![1, 4, 5]);
        assert_eq!(arch(&doc, made[0]).parcel_faces(), Some((1, vec![4, 5])));
        add_lot_line(&mut doc, s, &[(w(50.0, -20.0), 0.0), (w(50.0, 70.0), 0.0)], false, None).unwrap();
    }

    fn count_arch(doc: &CadDocument, dxf: &str) -> usize {
        doc.entities().filter(|e| matches!(e, EntityType::Extended(x) if matches!(&x.data, ExtendedEntityData::Arch(a) if a.dxf_name == dxf))).count()
    }

    #[test]
    fn a_parcel_records_its_boundary_and_a_silent_one_does_not() {
        let v = Vector2::new;
        let ring = [v(0.0, 0.0), v(100.0, 0.0), v(100.0, 50.0), v(0.0, 50.0)];
        // add_parcel: the created-from-object form — the boundary carries
        // exactly one parcel-segment record (Align-2's oracle: 1 parcel,
        // 1 segment whose corners are the ring).
        let mut doc = drawing();
        let s = add_site(&mut doc, "Site", "").unwrap();
        // add_site itself writes the site's own parcel record; count from it.
        let base = count_arch(&doc, "AECC_PARCEL");
        add_parcel(&mut doc, s, &ring, None).unwrap();
        assert_eq!(count_arch(&doc, "AECC_PARCEL_SEGMENT"), 1);
        assert_eq!(count_arch(&doc, "AECC_PARCEL"), base + 1);
        // add_parcel_without_segment: the composed build's form — the
        // closed lot line still joins the container and makes the face,
        // but no boundary record is written for it (the file's
        // parcel-segment set carries the records; E-1).
        let mut doc = drawing();
        let s = add_site(&mut doc, "Site", "").unwrap();
        let base = count_arch(&doc, "AECC_PARCEL");
        let parcel = add_parcel_without_segment(&mut doc, s, &ring, None).unwrap();
        assert_eq!(count_arch(&doc, "AECC_PARCEL_SEGMENT"), 0);
        assert_eq!(count_arch(&doc, "AECC_PARCEL"), base + 1);
        // No record carries the ring, so the boundary's reserved slot
        // carries no record either: this shape is the composed build's for
        // boundaries the parcel-segment set does not carry, and the site
        // does not read back member-wise until its set carries them (see
        // the composed test).
        let path = std::env::temp_dir().join(format!("civil-parcel-silent-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(count_arch(&back, "AECC_PARCEL_SEGMENT"), 0);
        assert_eq!(count_arch(&back, "AECC_PARCEL"), count_arch(&doc, "AECC_PARCEL"));
        let _ = parcel;
    }

    #[test]
    fn the_segment_record_and_the_silent_parcel_compose_to_the_oracle_census() {
        // The composed build (E-1) over Align-2's oracle shape: the
        // parcel-segment set is written first and carries the boundary
        // record (corners == the parcel ring, exactly as decoded); the
        // parcel adds none and its edge slot adopts the record.
        let v = Vector2::new;
        let ring = [v(3820.2309548999183, 2914.98799626017), v(5850.325349580031, 2914.98799626017), v(5850.325349580031, 4999.86946262978), v(3820.2309548999183, 4999.86946262978)];
        let geometry = crate::classes::civil::ParcelSegmentGeometry {
            label_point: v(3820.330954899918, 2915.08799626017),
            corners: ring,
        };
        let mut doc = drawing();
        let s = add_site(&mut doc, "Site", "").unwrap();
        let base = count_arch(&doc, "AECC_PARCEL");
        let record = add_parcel_segment_record(&mut doc, s, 1, Some(&geometry), None).unwrap();
        let parcel = add_parcel_without_segment(&mut doc, s, &ring, None).unwrap();
        assert_eq!(count_arch(&doc, "AECC_PARCEL_SEGMENT"), 1);
        assert_eq!(count_arch(&doc, "AECC_PARCEL"), base + 1);
        // The container's one edge slot points at the adopted record, and
        // the site reads back (the member kind comes from the record).
        let (_, members, parcels, _, _) = members_of(&mut doc, s).unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(parcels, vec![parcel.value()]);
        // It survives the file: one record, whose geometry reads back
        // bit-exact, and the adopted slot with it.
        let path = std::env::temp_dir().join(format!("civil-parcel-segment-e1-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let mut back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(count_arch(&back, "AECC_PARCEL_SEGMENT"), count_arch(&doc, "AECC_PARCEL_SEGMENT"));
        assert_eq!(count_arch(&back, "AECC_PARCEL"), count_arch(&doc, "AECC_PARCEL"));
        let (back_record_handle, back_record) = back.entities().find_map(|e| match e {
            EntityType::Extended(x) => match &x.data {
                ExtendedEntityData::Arch(a) if a.dxf_name == "AECC_PARCEL_SEGMENT" => Some((x.common.handle, a.clone())),
                _ => None,
            },
            _ => None,
        }).unwrap();
        assert_eq!(back_record.parcel_segment_geometry(), Some(geometry));
        assert_eq!(back_record_handle, record);
        let (_, members, parcels, _, _) = members_of(&mut back, s).unwrap();
        assert_eq!((members.len(), parcels), (1, vec![parcel.value()]));
    }

    #[test]
    fn edited_feature_lines_keep_their_node_ids_and_keys() {
        let v = |x: f64, z: f64, elevation_point: bool| FeatureLineVertex { point: crate::types::Vector3::new(x, 0.0, z), bulge: 0.0, elevation_point };
        let mut doc = drawing();
        let s = add_site(&mut doc, "Site 1", "").unwrap();
        let fl = add_feature_line_vertices(&mut doc, s, "FLB", &[v(0.0, 10.0, false), v(100.0, 10.0, false)], None).unwrap();
        set_feature_line_vertices(&mut doc, fl, &[v(0.0, 10.0, false), v(25.0, 11.0, true), v(100.0, 10.0, false)]).unwrap();
        set_feature_line_vertices(&mut doc, fl, &[v(0.0, 10.0, false), v(25.0, 11.0, true), v(60.0, 12.0, true), v(100.0, 10.0, false)]).unwrap();
        // Removing the first elevation point frees neither its id nor its key.
        set_feature_line_vertices(&mut doc, fl, &[v(0.0, 10.0, false), v(60.0, 12.0, true), v(100.0, 9.0, false)]).unwrap();
        let path = std::env::temp_dir().join(format!("civil-fl-edit-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let mut back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        let (container, members, _, _, _) = members_of(&mut back, s).unwrap();
        let t = object(&back, container).unwrap().topology().unwrap();
        assert_eq!(t.edges[0].iter().map(|v| (v.point.x, v.z, v.node, v.key)).collect::<Vec<_>>(), vec![(0.0, 10.0, 1, 0), (60.0, 12.0, 4, 2), (100.0, 9.0, 2, 0)]);
        assert_eq!((t.next_node, t.next_key, t.outside_face_flags), (5, 3, 6));
        assert_eq!(members[0].elevation_points, vec![false, true, false]);
        assert!(set_feature_line_vertices(&mut back, fl, &[v(0.0, 10.0, true), v(100.0, 10.0, false)]).is_err());
        // The newest member keeps the point marks and edge flag 0x800 a
        // container read gave it.
        let (container, mut members, parcels, style, next) = members_of(&mut back, s).unwrap();
        let read = members[0].read.as_mut().unwrap();
        read.flags |= 0x800;
        read.points[1] |= 0x20;
        write_container(&mut back, container, s, &members, &parcels, style, next).unwrap();
        let t = object(&back, container).unwrap().topology().unwrap();
        assert_eq!((t.edge_flags[0] & 0x800, t.edges[0][1].flags & 0x30, t.edges[0][2].flags & 8), (0x800, 0x20, 8));
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
    fn feature_lines_split_a_parcel_into_faces() {
        let v = Vector2::new;
        let p = crate::types::Vector3::new;
        let mut doc = drawing();
        let s = add_site(&mut doc, "Site 1", "").unwrap();
        let parcel = add_parcel(&mut doc, s, &[v(0.0, 0.0), v(100.0, 0.0), v(100.0, 50.0), v(0.0, 50.0)], None).unwrap();
        add_feature_line(&mut doc, s, "FL1", &[p(-20.0, 25.0, 5.0), p(120.0, 25.0, 6.0)], None).unwrap();
        add_feature_line(&mut doc, s, "FL2", &[p(50.0, -20.0, 5.0), p(50.0, 70.0, 6.0)], None).unwrap();
        let path = std::env::temp_dir().join(format!("civil-parcel-faces-{}.dwg", std::process::id()));
        DwgWriter::write_to_file(&path, &doc).unwrap();
        let back = DwgReader::from_file(&path).unwrap().read().unwrap();
        let _ = std::fs::remove_file(&path);
        // One parcel over the four parts, listed once per face in the
        // container; its label sits at the mean elevation of its outline
        // (corners at 0, crossings 5.143 to 5.857: 22 / 8).
        let (number, faces) = arch(&back, parcel).parcel_faces().unwrap();
        assert_eq!((number, faces.len()), (1, 4));
        let container = arch(&back, s).typed_handles()[5].0;
        let refs: Vec<u64> = object(&back, Handle::new(container)).unwrap().typed_handles().iter().map(|r| r.0).collect();
        assert_eq!(refs.iter().filter(|r| **r == parcel.value()).count(), 4);
        let record = arch(&back, parcel);
        let (data, start, end) = record.main_stream().unwrap();
        let mut b = crate::classes::civil::bits::Bits::new(data, start, end);
        crate::classes::civil::alignment::class_keys(&mut b).unwrap();
        b.pos += 121;
        for _ in 0..3 {
            b.bit_long().unwrap();
        }
        b.bit_double().unwrap();
        let label = [b.bit_double().unwrap(), b.bit_double().unwrap(), b.bit_double().unwrap()];
        assert_eq!(label[..2], [50.0, 25.0]);
        assert!((label[2] - 2.75).abs() < 1e-9);
        // A lot line across it splits it along the line only; the line
        // takes the feature lines' elevations where it crosses them.
        let mut back = back;
        let (_, made) = add_lot_line(&mut back, s, &[(v(75.0, -10.0), 0.0), (v(75.0, 60.0), 0.0)], false, None).unwrap();
        assert_eq!(made.len(), 1);
        let (_, left) = arch(&back, parcel).parcel_faces().unwrap();
        let (_, right) = arch(&back, made[0]).parcel_faces().unwrap();
        assert_eq!((left.len(), right.len()), (4, 2));
        assert_eq!(arch(&back, made[0]).parcel_label().map(|l| (l.x, l.y)), Some((87.5, 25.0)));
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
        // A plain parcel may not meet another line.
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
        // A line through a parcel's label: the parcel follows its label,
        // the faces left over get new parcels, new faces first.
        let s2 = add_site(&mut back, "Site 2", "").unwrap();
        let (_, p) = add_lot_line(&mut back, s2, &[w(0.0, 0.0), w(100.0, 0.0), w(100.0, 50.0), w(0.0, 50.0)], true, None).unwrap();
        let (_, more) = add_lot_line(&mut back, s2, &[w(30.0, -20.0), w(30.0, 70.0), w(70.0, 70.0), w(70.0, -20.0)], false, None).unwrap();
        let faces: Vec<(i32, i32)> = p.iter().chain(&more).map(|h| arch(&back, *h).parcel_face().unwrap()).collect();
        assert_eq!(faces, vec![(1, 3), (2, 4), (3, 5), (4, 2)]);
        assert_eq!(arch(&back, more[1]).parcel_label(), Some(v(85.0, 25.0)));
        // The site still takes members; the alignment keeps its edge.
        assert!(add_lot_line(&mut back, s, &[w(200.0, 0.0), w(300.0, 0.0), w(300.0, 50.0)], true, None).is_ok());
        assert_eq!(object(&back, Handle::new(container)).unwrap().site_members()[0], a.value());
    }

    #[test]
    fn alignments_and_lot_lines_split_each_other() {
        let v = Vector2::new;
        let w = |x: f64, y: f64| (Vector2::new(x, y), 0.0);
        let mut doc = drawing();
        let s = add_site(&mut doc, "Site 1", "").unwrap();
        add_alignment_on_site(&mut doc, s, "AL1", &[v(-20.0, 25.0), v(120.0, 25.0)], &[], None).unwrap();
        // A rectangle across the alignment: two parcels, the lower half
        // first (its first segment's side).
        let (_, first) = add_lot_line(&mut doc, s, &[w(0.0, 0.0), w(100.0, 0.0), w(100.0, 50.0), w(0.0, 50.0)], true, None).unwrap();
        assert_eq!(first.len(), 2);
        // An alignment across both: two more parcels, numbered on.
        add_alignment_on_site(&mut doc, s, "AL2", &[v(50.0, -20.0), v(50.0, 70.0)], &[], None).unwrap();
        // Two alignments closing a loop make a parcel of it.
        add_alignment_on_site(&mut doc, s, "AL3", &[v(200.0, 0.0), v(300.0, 0.0), v(300.0, 50.0)], &[VertexCurve::None], None).unwrap();
        add_alignment_on_site(&mut doc, s, "AL4", &[v(300.0, 50.0), v(200.0, 50.0), v(200.0, 0.0)], &[VertexCurve::None], None).unwrap();
        let mut labels: Vec<(i32, i32, Vector2)> = doc
            .entities()
            .filter_map(|e| match e {
                EntityType::Extended(x) => match &x.data {
                    ExtendedEntityData::Arch(a) if a.parcel_site().is_some() => a.parcel_face().zip(a.parcel_label()).map(|((n, f), l)| (n, f, l)),
                    _ => None,
                },
                _ => None,
            })
            .collect();
        labels.sort_by_key(|l| l.0);
        assert_eq!(labels, vec![(1, 2, v(25.0, 37.5)), (2, 3, v(25.0, 12.5)), (3, 4, v(75.0, 12.5)), (4, 5, v(75.0, 37.5)), (5, 6, v(250.0, 25.0))]);
    }

    #[test]
    fn arcs_split_where_lines_meet() {
        let v = Vector2::new;
        let w = |x: f64, y: f64| (Vector2::new(x, y), 0.0);
        let mut doc = drawing();
        let s = add_site(&mut doc, "Site 1", "").unwrap();
        // A rectangle whose first side is an arc of bulge 0.5 (down to -25).
        let (_, first) = add_lot_line(&mut doc, s, &[(v(0.0, 0.0), 0.5), w(100.0, 0.0), w(100.0, 50.0), w(0.0, 50.0)], true, None).unwrap();
        let (_, more) = add_lot_line(&mut doc, s, &[w(50.0, -40.0), w(50.0, 70.0)], false, None).unwrap();
        assert_eq!((first.len(), more.len()), (1, 1));
        let container = arch(&doc, s).typed_handles()[5].0;
        let edge = &object(&doc, Handle::new(container)).unwrap().topology().unwrap().edges[0];
        // The arc's two halves: each a quarter of the sweep.
        let half = (0.5_f64.atan() / 2.0).tan();
        assert!((edge[0].bulge - half).abs() < 1e-12 && (edge[1].bulge - half).abs() < 1e-12);
        assert!((edge[1].point - v(50.0, -25.0)).length() < 1e-9);
        // A line crossing both halves; the site still takes members (its
        // arcs read whole).
        assert!(add_lot_line(&mut doc, s, &[w(-10.0, -10.0), w(110.0, -10.0)], false, None).is_ok());
        assert!(add_lot_line(&mut doc, s, &[w(0.0, 25.0), w(100.0, 25.0)], false, None).is_ok());
    }
}
