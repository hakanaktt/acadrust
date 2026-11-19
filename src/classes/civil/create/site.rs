//! Sites (`AECC_PARCEL` records named `AeccImpFace` without a site of
//! their own), their containers and the alignments on them.

use super::alignment::{default_style as alignment_style, elements, solve, write_alignment, PathPoint, VertexCurve, ARC_EDGE};
use super::groups::bucket_insert;
use super::support::*;
use super::tree::{ensure_alignment_tree, DEFAULT_STYLE_ID, KEY_STYLES};
use crate::entities::{EntityCommon, EntityType, ExtendedEntityData};
use crate::error::DxfError;
use crate::io::dwg::dwg_reference_type::DwgReferenceType;
use crate::types::{Handle, Vector2};
use crate::CadDocument;

/// Root key of the node that lists the sites.
const KEY_SITES: i32 = 2;
/// Style tree keys of the parcel style and feature line style collections.
const KEY_PARCEL_STYLES: i32 = 2;
const KEY_FEATURE_LINE_STYLES: i32 = 128;
/// First text id given to a written site (kept apart from the other
/// writers' ids).
const FIRST_SITE_TAG: u32 = 0x8004_0001;

/// The default style of the style tree's collection `key`: its child with
/// the default style id (30), or its only child.
fn default_style_of(doc: &CadDocument, key: i32) -> Option<u64> {
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

/// A member of a site: an alignment (its solved path, elevations 0) or a
/// feature line (its points with their elevations).
pub(super) struct Member {
    handle: u64,
    path: Vec<PathPoint>,
    elevations: Vec<f64>,
    feature_line: bool,
}

/// Edge flags of an alignment and of a feature line in a container.
const ALIGNMENT_EDGE: i32 = 0x30;
const FEATURE_LINE_EDGE: i32 = 0x1_0030;

/// The site's container (`AECC_FEATURE_STYLE` owned by the site): the
/// topology of its members, each an edge through its own nodes.
///
/// ```text
/// BS 0x04BE, BL 0, BL 0 (no children), BL 0, BL 32, BL 210,
/// 21 bits 000000110010000000110, BL 0x80000001,
/// nodes: BL next free id, BL n, BL first id, n x {3BD (z the
///   elevation), 3 bits 010, BL next id} (ids per member: its ends, then
///   its inner points; the records in hash order, seven buckets by
///   (id - 1) mod 7, the newest first in a bucket)
/// edges: BL next free id, BL m slots, per member BL 0, BL id (1..m),
///   BL flags (0x30 alignment, 0x10030 feature line), BL count, BL 0,
///   count x {BL kind (0 line, 1 arc, 2 last),
///   BL flags, BL node, BL 1, BL 1 (the outside face on both sides; 0, 0
///   on the last point), an arc: BD bulge, BL 0}, BL 0, BL 0 x 3
/// faces: BL 2, BL 1 (BL 0 without members), the outside face {BL 1,
///   BL 1, BL 14, BL m, per member {BL 1 +
///   the points of the members before it, BL 0}}, BL 1, 6 x {BL 1, BL 1},
///   6 x BL 0
/// references: per member its record (soft pointer) and soft pointer
///   0, then (with members) the site and soft pointer 0, then the default
///   feature line style (hard)
/// texts: ""
/// ```
///
/// The points run as one chain through the members in edge order: a
/// point's flags are 2, with 0x30 added when another segment follows it
/// (in its edge or a later one), 0x80 on the first point of an edge after
/// the first, 0x010F0000 for an arc; an edge's last point's are 10
/// (WT07, WT10 to WT13, WF01 to WF06; written for their members, the
/// containers equal the samples' bit for bit).
pub(super) fn write_container(doc: &mut CadDocument, handle: Handle, site: Handle, members: &[Member], style: u64) -> Result<(), DxfError> {
    let nodes: usize = members.iter().map(|m| m.path.len()).sum();
    let mut refs: Vec<Ref> = members.iter().flat_map(|m| [soft_pointer(m.handle), soft_pointer(0)]).collect();
    if !members.is_empty() {
        refs.extend([soft_pointer(site.value()), soft_pointer(0)]);
    }
    refs.push(hard_pointer(style));
    put_object(
        doc,
        "AECC_FEATURE_STYLE",
        handle,
        site,
        Vec::new(),
        |w| {
            for v in [0, 0, 0, 32, 210] {
                w.write_bit_long(v);
            }
            bits(w, "000000110010000000110");
            w.write_bit_long(0x8000_0001_u32 as i32);
            // Node ids: per member its ends, then its inner points. The
            // records follow the application's hash order: seven buckets
            // by (id - 1) mod 7, the newest first in a bucket.
            let mut first_id = Vec::with_capacity(members.len());
            let mut points: Vec<(i32, Vector2, f64)> = Vec::with_capacity(nodes);
            for m in members {
                first_id.push(points.len() as i32 + 1);
                let n = m.path.len();
                let mut order = vec![0, n - 1];
                order.extend(1..n - 1);
                for i in order {
                    points.push((points.len() as i32 + 1, m.path[i].0, m.elevations[i]));
                }
            }
            points.sort_by_key(|(id, ..)| ((id - 1) % 7, std::cmp::Reverse(*id)));
            w.write_bit_long(nodes as i32 + 1);
            w.write_bit_long(nodes as i32);
            w.write_bit_long(points.first().map_or(0, |p| p.0));
            for (k, (_, p, z)) in points.iter().enumerate() {
                for v in [p.x, p.y, *z] {
                    w.write_bit_double(v);
                }
                bits(w, "010");
                w.write_bit_long(points.get(k + 1).map_or(0, |q| q.0));
            }
            w.write_bit_long(members.len() as i32 + 1);
            w.write_bit_long(members.len() as i32);
            for (e, m) in members.iter().enumerate() {
                let n = m.path.len();
                let edge = if m.feature_line { FEATURE_LINE_EDGE } else { ALIGNMENT_EDGE };
                for v in [0, e as i32 + 1, edge, n as i32, 0] {
                    w.write_bit_long(v);
                }
                let last_member = e + 1 == members.len();
                for (i, (_, arc)) in m.path.iter().enumerate() {
                    let last = i + 1 == n;
                    let arc = arc.filter(|_| !last);
                    let follows = i + 2 < n || !last_member;
                    let flags = if last {
                        10
                    } else {
                        2 | if follows { 0x30 } else { 0 } | if i == 0 && e > 0 { 0x80 } else { 0 } | if arc.is_some() { ARC_EDGE } else { 0 }
                    };
                    let node = first_id[e] + if i == 0 { 0 } else if last { 1 } else { i as i32 + 1 };
                    let face = if last { 0 } else { 1 };
                    for v in [if last { 2 } else if arc.is_some() { 1 } else { 0 }, flags, node, face, face] {
                        w.write_bit_long(v);
                    }
                    if let Some(bulge) = arc {
                        w.write_bit_double(bulge);
                        w.write_bit_long(0);
                    }
                }
                for _ in 0..4 {
                    w.write_bit_long(0);
                }
            }
            w.write_bit_long(2);
            if members.is_empty() {
                w.write_bit_long(0);
            } else {
                for v in [1, 1, 1, 14, members.len() as i32] {
                    w.write_bit_long(v);
                }
                let mut before = 0;
                for m in members {
                    w.write_bit_long(before + 1);
                    w.write_bit_long(0);
                    before += m.path.len() as i32;
                }
            }
            w.write_bit_long(1);
            for _ in 0..12 {
                w.write_bit_long(1);
            }
            for _ in 0..6 {
                w.write_bit_long(0);
            }
        },
        Body { refs, strings: vec![String::new()] },
    )
}

/// A site's container and the members it lists, when the container is one
/// this crate writes: writing it from what is read gives back its record.
fn members_of(doc: &mut CadDocument, site: Handle) -> Result<(Handle, Vec<Member>, u64), DxfError> {
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
    let handles: Vec<u64> = refs.iter().step_by(2).take(edges.len()).map(|r| r.0).collect();
    let kind = |h: u64| match doc.get_entity(Handle::new(h)) {
        Some(EntityType::Extended(x)) => match &x.data {
            ExtendedEntityData::Arch(a) => Some(a.dxf_name == "AECC_FEATURE_LINE"),
            _ => None,
        },
        _ => None,
    };
    let members: Vec<Member> = edges
        .iter()
        .zip(&handles)
        .map(|(run, h)| {
            Some(Member {
                handle: *h,
                path: run.iter().map(|v| (v.point, (v.bulge != 0.0).then_some(v.bulge))).collect(),
                elevations: run.iter().map(|v| v.z).collect(),
                feature_line: kind(*h)?,
            })
        })
        .collect::<Option<_>>()
        .ok_or_else(refused)?;
    let bits_of = |doc: &CadDocument| object(doc, container).and_then(|o| o.main_stream().map(|(d, s, e)| (s..e).map(|i| d[i / 8] >> (7 - i % 8) & 1).collect::<Vec<u8>>()));
    let before = bits_of(doc);
    write_container(doc, container, site, &members, style)?;
    if bits_of(doc) != before || members.len() != (refs.len().saturating_sub(3)) / 2 {
        doc.objects.insert(container, crate::objects::ObjectType::Arch(o));
        return Err(refused());
    }
    Ok((container, members, style))
}

/// Adds an empty site named `name` with `description`. It takes the
/// drawing's default parcel style and default feature line style (the
/// default styles of the style tree's parcel and feature line style
/// collections, id 30; none when the drawing has no such collection) and
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
    write_container(doc, container, site, &[], line_style)?;
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
    let (container, mut members, line_style) = members_of(doc, site)?;
    if members.iter().any(|m| paths_meet(&m.path, &path)) {
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
    members.push(Member { handle: common.handle.value(), elevations: vec![0.0; path.len()], path, feature_line: false });
    write_container(doc, container, site, &members, line_style)?;
    write_alignment(doc, common, name, &elements, style, container.value(), members.len() as i32, None, &[])
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

/// Adds a feature line through `points` (x, y, elevation; straight
/// segments) on `site`: its edge joins the site's container. Like an
/// alignment on a site it must not cross or touch the site's other
/// members, and the site must be one this crate writes. It takes the
/// feature line style the site's container uses.
pub fn add_feature_line(doc: &mut CadDocument, site: Handle, name: &str, points: &[crate::types::Vector3], layer: Option<&str>) -> Result<Handle, DxfError> {
    if points.len() < 2 || points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite() || !p.z.is_finite()) || points.windows(2).any(|w| w[0].x == w[1].x && w[0].y == w[1].y) {
        return Err(invalid("a feature line needs two or more finite points, apart in plan"));
    }
    if name.is_empty() {
        return Err(invalid("a feature line needs a name"));
    }
    let path: Vec<PathPoint> = points.iter().map(|p| (Vector2::new(p.x, p.y), None)).collect();
    let layer = layer.unwrap_or("0");
    let layer_handle = doc.layers.get(layer).map(|l| l.handle).ok_or_else(|| invalid("layer does not exist"))?;
    let (container, mut members, line_style) = members_of(doc, site)?;
    if members.iter().any(|m| paths_meet(&m.path, &path)) {
        return Err(invalid("the feature line crosses or touches another member of the site"));
    }
    let common = EntityCommon {
        handle: doc.allocate_handle(),
        owner_handle: doc.header.model_space_block_handle,
        layer: layer.into(),
        layer_handle: Some(layer_handle),
        ..EntityCommon::default()
    };
    members.push(Member { handle: common.handle.value(), path, elevations: points.iter().map(|p| p.z).collect(), feature_line: true });
    write_container(doc, container, site, &members, line_style)?;
    write_feature_line(doc, common, name, members.len() as i32, container, line_style)
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
        // No parcel style and no feature line style in a new drawing.
        assert_eq!(arch(&back, s).typed_handles()[6].0, 0);
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
}
