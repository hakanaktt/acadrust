
use crate::document::CadDocument;
use crate::entities::{EntityType, Insert};
use crate::types::{Handle, Vector3};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq)]
pub struct BlockInstance {
    pub handle: Handle,
    pub name: String,
    pub layer: String,
    pub scale: [f64; 3],
    pub mirrored: bool,
    pub duplicate_of: Option<Handle>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CountKey {
    pub layer: Option<String>,
    pub scale: Option<[f64; 3]>,
    pub mirror_state: Option<bool>,
}

impl CountKey {
    pub fn of(instance: &BlockInstance, layer: bool, scale: bool, mirror: bool) -> Self {
        Self {
            layer: layer.then(|| instance.layer.clone()),
            scale: scale.then_some(instance.scale),
            mirror_state: mirror.then_some(instance.mirrored),
        }
    }

    pub fn matches(&self, instance: &BlockInstance) -> bool {
        self.layer.as_ref().is_none_or(|l| l.eq_ignore_ascii_case(&instance.layer))
            && self.scale.is_none_or(|s| same_scale(s, instance.scale))
            && self.mirror_state.is_none_or(|m| m == instance.mirrored)
    }

    pub fn json(&self) -> String {
        let mut parts = Vec::new();
        if let Some(layer) = &self.layer {
            parts.push(format!("\"layer\":{}", json_string(layer)));
        }
        if let Some(m) = self.mirror_state {
            parts.push(format!("\"mirrorState\":{m}"));
        }
        if let Some([x, y, z]) = self.scale {
            parts.push(format!("\"scale\":{{\"x\":{x:?},\"y\":{y:?},\"z\":{z:?}}}"));
        }
        format!("{{{}}}", parts.join(","))
    }

    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        if let Some(layer) = &self.layer {
            parts.push(layer.clone());
        }
        if let Some([x, y, z]) = self.scale {
            if same_scale([x, x, x], [x, y, z]) {
                parts.push(format!("{x:.4}"));
            } else {
                parts.push(format!("{x:.4},{y:.4},{z:.4}"));
            }
        }
        if let Some(m) = self.mirror_state {
            parts.push(if m { "Mirrored" } else { "Default" }.to_string());
        }
        parts.join("_")
    }
}

fn same_scale(a: [f64; 3], b: [f64; 3]) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0))
}

pub fn block_instances(doc: &CadDocument, area: Option<&[[f64; 2]]>) -> Vec<BlockInstance> {
    let mut seen: HashMap<(String, [i64; 10]), Handle> = HashMap::new();
    let mut out = Vec::new();
    for entity in doc.model_space_entities() {
        let EntityType::Insert(insert) = entity else {
            continue;
        };
        let Some(record) = doc.block_records.get(&insert.block_name) else {
            continue;
        };
        if record.name.starts_with('*')
            || record.flags.anonymous
            || record.flags.is_xref
            || record.flags.is_xref_overlay
            || record.flags.is_external
        {
            continue;
        }
        if let Some(polygon) = area {
            if !inside_polygon(&insert_outline(doc, insert, 0), polygon) {
                continue;
            }
        }
        let (sx, sy, sz) = (insert.x_scale(), insert.y_scale(), insert.z_scale());
        // ponytail: duplicates are exact overlaps (same transform), rounded to 1e-6.
        let r = |v: f64| (v * 1e6).round() as i64;
        let p = insert.insert_point;
        let n = insert.normal;
        let key = (
            record.name.to_ascii_uppercase(),
            [r(p.x), r(p.y), r(p.z), r(insert.rotation), r(sx), r(sy), r(sz), r(n.x), r(n.y), r(n.z)],
        );
        let handle = insert.common.handle;
        let duplicate_of = match seen.get(&key) {
            Some(first) => Some(*first),
            None => {
                seen.insert(key, handle);
                None
            }
        };
        out.push(BlockInstance {
            handle,
            name: record.name.clone(),
            layer: insert.common.layer.clone(),
            scale: [sx.abs(), sy.abs(), sz.abs()],
            mirrored: sx * sy * sz < 0.0,
            duplicate_of,
        });
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct BlockCount {
    pub name: String,
    pub counted: Vec<Handle>,
    pub duplicates: Vec<Handle>,
}

pub fn block_counts(instances: &[BlockInstance]) -> Vec<BlockCount> {
    let mut map: HashMap<String, BlockCount> = HashMap::new();
    for i in instances {
        let entry = map.entry(i.name.to_ascii_uppercase()).or_insert_with(|| BlockCount {
            name: i.name.clone(),
            counted: Vec::new(),
            duplicates: Vec::new(),
        });
        if i.duplicate_of.is_some() {
            entry.duplicates.push(i.handle);
        } else {
            entry.counted.push(i.handle);
        }
    }
    let mut out: Vec<BlockCount> = map.into_values().collect();
    out.sort_by_key(|c| c.name.to_ascii_uppercase());
    out
}

pub fn count_of(instances: &[BlockInstance], name: &str, key: &CountKey) -> usize {
    instances
        .iter()
        .filter(|i| i.duplicate_of.is_none() && i.name.eq_ignore_ascii_case(name) && key.matches(i))
        .count()
}

pub fn expanded_counts(
    instances: &[BlockInstance],
    name: &str,
    layer: bool,
    scale: bool,
    mirror: bool,
) -> Vec<(CountKey, usize)> {
    let mut out: Vec<(CountKey, usize)> = Vec::new();
    for i in instances.iter().filter(|i| i.duplicate_of.is_none() && i.name.eq_ignore_ascii_case(name)) {
        let key = CountKey::of(i, layer, scale, mirror);
        match out.iter_mut().find(|(k, _)| k.matches(i)) {
            Some((_, n)) => *n += 1,
            None => out.push((key, 1)),
        }
    }
    out
}

pub fn block_json(name: &str, key: &CountKey) -> String {
    format!("{{\"key\":{},\"name\":{},\"type\":\"block\"}}", key.json(), json_string(name))
}

pub fn area_json(name: &str, key: &CountKey, boundary: Handle) -> String {
    format!(
        "{{\"boundaryObjectHandle\":\"{:X}\",\"evaluatorId\":\"AcCount2\",\"key\":{},\"name\":{},\"type\":\"block\"}}",
        boundary.value(),
        key.json(),
        json_string(name)
    )
}

pub fn single_json(targets: &[Handle]) -> String {
    let targets: Vec<String> = targets.iter().map(|h| format!("\"{:X}\"", h.value())).collect();
    format!(
        "{{\"groupCondition\":{{\"attributes\":[],\"parameters\":[],\"properties\":[]}},\"targets\":[{}],\"type\":\"single\"}}",
        targets.join(",")
    )
}

pub fn similar_matches(doc: &CadDocument, target: &EntityType, area: Option<&[[f64; 2]]>) -> Vec<Vec<Handle>> {
    let duplicates: Vec<Handle> =
        block_instances(doc, None).into_iter().filter(|i| i.duplicate_of.is_some()).map(|i| i.handle).collect();
    let matches = |e: &EntityType| similar(target, e);
    let mut out = Vec::new();
    for e in doc.model_space_entities() {
        let top = e.common().handle;
        if duplicates.contains(&top) {
            continue;
        }
        let mut found = Vec::new();
        nested_matches(doc, e, &matches, &mut Vec::new(), 0, &mut found);
        for outline in found {
            if area.is_none_or(|polygon| inside_polygon(&outline, polygon)) {
                out.push(vec![top]);
            }
        }
    }
    out
}

fn nested_matches(
    doc: &CadDocument,
    e: &EntityType,
    matches: &dyn Fn(&EntityType) -> bool,
    chain: &mut Vec<Insert>,
    depth: usize,
    found: &mut Vec<Vec<Piece>>,
) {
    // A table's drawing (its `*T` block) counts like a reference's; a
    // dimension's does not.
    let placed = match e {
        EntityType::Insert(ins) => Some(ins.clone()),
        EntityType::Table(t) => {
            // DWG tables may name their block only by its record.
            let name = t
                .block_record_handle
                .and_then(|h| doc.block_records.iter().find(|r| r.handle == h))
                .map_or_else(|| t.block_name.clone(), |r| r.name.clone());
            let mut ins = Insert::new(name, t.insertion_point);
            ins.rotation = t.horizontal_direction.y.atan2(t.horizontal_direction.x);
            ins.normal = t.normal;
            Some(ins)
        }
        _ => None,
    };
    if let Some(ins) = placed {
        let Some(record) = doc.block_records.get(&ins.block_name) else {
            return;
        };
        if depth >= 8 {
            return;
        }
        chain.push(ins);
        for &h in &record.entity_handles {
            if let Some(child) = doc.get_entity(h) {
                nested_matches(doc, child, matches, chain, depth + 1, found);
            }
        }
        chain.pop();
        return;
    }
    if e.common().invisible || !matches(e) {
        return;
    }
    let mut outline = entity_outline(doc, e);
    for ins in chain.iter().rev() {
        let base = doc.block_records.get(&ins.block_name).map(|r| r.base_point).unwrap_or_default();
        let t = ins.get_transform();
        outline = outline.into_iter().map(|p| p.placed(&t, base)).collect();
    }
    found.push(outline);
}

fn polyline_shape(e: &EntityType) -> Option<Vec<[f64; 3]>> {
    let EntityType::LwPolyline(pl) = e else {
        return None;
    };
    let n = pl.vertices.len();
    let segs = if pl.is_closed { n } else { n.saturating_sub(1) };
    let dir = |i: usize| {
        let (a, b) = (pl.vertices[i % n].location, pl.vertices[(i + 1) % n].location);
        (b.x - a.x, b.y - a.y)
    };
    let total: f64 = (0..segs).map(|i| dir(i).0.hypot(dir(i).1)).sum();
    if total <= 1e-12 {
        return None;
    }
    Some(
        (0..segs)
            .map(|i| {
                let (dx, dy) = dir(i);
                let turn = if pl.is_closed || i + 1 < segs {
                    let (ex, ey) = dir(i + 1);
                    (dx * ey - dy * ex).atan2(dx * ex + dy * ey)
                } else {
                    0.0
                };
                [dx.hypot(dy) / total, turn, pl.vertices[i].bulge]
            })
            .collect(),
    )
}

fn same_shape(a: Option<&[[f64; 3]]>, b: Option<&[[f64; 3]]>, closed: bool) -> bool {
    let (Some(a), Some(b)) = (a, b) else {
        return false;
    };
    let close = |x: &[f64; 3], y: &[f64; 3]| x.iter().zip(y).all(|(p, q)| (p - q).abs() < 1e-6);
    let shifts = if closed { a.len() } else { 1 };
    // A mirrored copy turns the other way.
    let mirrored: Vec<[f64; 3]> = b.iter().map(|x| [x[0], -x[1], -x[2]]).collect();
    a.len() == b.len()
        && [b, mirrored.as_slice()]
            .iter()
            .any(|b| (0..shifts).any(|k| a.iter().enumerate().all(|(i, x)| close(x, &b[(i + k) % b.len()]))))
}

fn similar(target: &EntityType, e: &EntityType) -> bool {
    let eq = |a: f64, b: f64| (a - b).abs() < 1e-6;
    let sweep = |a: f64, b: f64| (b - a).rem_euclid(std::f64::consts::TAU);
    match (target, e) {
        (EntityType::Line(_), EntityType::Line(_)) => true,
        (EntityType::Arc(a), EntityType::Arc(b)) => eq(sweep(a.start_angle, a.end_angle), sweep(b.start_angle, b.end_angle)),
        (EntityType::Ellipse(a), EntityType::Ellipse(b)) => {
            eq(a.minor_axis_ratio, b.minor_axis_ratio)
                && eq(sweep(a.start_parameter, a.end_parameter), sweep(b.start_parameter, b.end_parameter))
        }
        (EntityType::LwPolyline(a), EntityType::LwPolyline(b)) => {
            a.is_closed == b.is_closed && same_shape(polyline_shape(target).as_deref(), polyline_shape(e).as_deref(), a.is_closed)
        }
        // Texts and multiline texts match one another by their string.
        (EntityType::Text(_) | EntityType::MText(_), EntityType::Text(_) | EntityType::MText(_)) => {
            let raw = |e: &EntityType| match e {
                EntityType::Text(t) => t.value.clone(),
                EntityType::MText(t) => t.value.clone(),
                _ => String::new(),
            };
            raw(target) == raw(e)
        }
        (EntityType::Spline(a), EntityType::Spline(b)) => {
            // Splines kept by their fit points compare by those.
            let pts = |s: &crate::entities::Spline| {
                normalized(if s.control_points.is_empty() { &s.fit_points } else { &s.control_points })
            };
            let (pa, pb) = (pts(a), pts(b));
            a.degree == b.degree
                && pa.len() == pb.len()
                && !pa.is_empty()
                && pa.iter().zip(&pb).all(|(p, q)| eq(p[0], q[0]) && eq(p[1], q[1]))
        }
        (EntityType::Hatch(a), EntityType::Hatch(b)) => same_hatch(a, b),
        _ => std::mem::discriminant(e) == std::mem::discriminant(target),
    }
}

fn normalized(points: &[Vector3]) -> Vec<[f64; 2]> {
    let Some(first) = points.first() else {
        return Vec::new();
    };
    let Some(d) = points.iter().map(|p| *p - *first).find(|d| d.x.hypot(d.y) > 1e-12) else {
        return Vec::new();
    };
    let (len, ang) = (d.x.hypot(d.y), d.y.atan2(d.x));
    let (c, s) = (ang.cos(), ang.sin());
    points
        .iter()
        .map(|p| {
            let (x, y) = (p.x - first.x, p.y - first.y);
            [(x * c + y * s) / len, (y * c - x * s) / len]
        })
        .collect()
}

pub fn group_matches(doc: &CadDocument, targets: &[Handle], area: Option<&[[f64; 2]]>) -> Vec<Vec<Handle>> {
    if let [one] = targets {
        match doc.get_entity(*one) {
            Some(EntityType::Insert(_)) => {}
            Some(e) => return similar_matches(doc, e, area),
            None => return Vec::new(),
        }
    }
    let r = |v: f64| (v * 1e6).round() as i64;
    let key = |sig: &str, p: Vector3| (sig.to_string(), [r(p.x), r(p.y), r(p.z)]);
    let duplicates: HashSet<Handle> = block_instances(doc, None)
        .into_iter()
        .filter(|i| i.duplicate_of.is_some())
        .map(|i| i.handle)
        .collect();
    let mut index: HashMap<(String, [i64; 3]), Vec<Handle>> = HashMap::new();
    let mut order: Vec<(Handle, String, Vector3)> = Vec::new();
    for e in doc.model_space_entities() {
        let h = e.common().handle;
        if duplicates.contains(&h) {
            continue;
        }
        if let Some((sig, anchor)) = shape_signature(e) {
            index.entry(key(&sig, anchor)).or_default().push(h);
            order.push((h, sig, anchor));
        }
    }
    let mut wanted = Vec::new();
    for t in targets {
        match doc.get_entity(*t).and_then(shape_signature) {
            Some(s) => wanted.push(s),
            // ponytail: an object without a comparable shape only matches itself.
            None if targets.iter().all(|t| doc.get_entity(*t).is_some()) => return vec![targets.to_vec()],
            None => return Vec::new(),
        }
    }
    let Some((sig0, anchor0)) = wanted.first().cloned() else {
        return Vec::new();
    };
    let mut out: Vec<Vec<Handle>> = Vec::new();
    // Sorted member sets already in `out`, so a group found from another of
    // its members is skipped in O(1).
    let mut seen: HashSet<Vec<u64>> = HashSet::new();
    for (h, sig, anchor) in &order {
        if *sig != sig0 {
            continue;
        }
        let d = *anchor - anchor0;
        let mut members = vec![*h];
        for (s, a) in &wanted[1..] {
            let Some(m) = index.get(&key(s, *a + d)).and_then(|hs| hs.iter().find(|m| !members.contains(m))) else {
                break;
            };
            members.push(*m);
        }
        if members.len() != wanted.len() {
            continue;
        }
        if let Some(polygon) = area {
            let inside = members
                .iter()
                .all(|m| doc.get_entity(*m).is_some_and(|e| inside_polygon(&entity_outline(doc, e), polygon)));
            if !inside {
                continue;
            }
        }
        let mut sorted: Vec<u64> = members.iter().map(|h| h.value()).collect();
        sorted.sort_unstable();
        if seen.insert(sorted) {
            out.push(members);
        }
    }
    out
}

fn shape_signature(entity: &EntityType) -> Option<(String, Vector3)> {
    let f = |v: f64| format!("{:.6}", v + 0.0);
    let v = |p: Vector3| format!("{},{},{}", f(p.x), f(p.y), f(p.z));
    Some(match entity {
        EntityType::Insert(i) => (
            format!(
                "INSERT|{}|{}|{}|{}|{}|{}",
                i.block_name.to_ascii_uppercase(),
                f(i.rotation),
                f(i.x_scale()),
                f(i.y_scale()),
                f(i.z_scale()),
                v(i.normal)
            ),
            i.insert_point,
        ),
        EntityType::Line(l) => (format!("LINE|{}", v(l.end - l.start)), l.start),
        EntityType::Circle(c) => (format!("CIRCLE|{}|{}", f(c.radius), v(c.normal)), c.center),
        EntityType::Arc(a) => (
            format!("ARC|{}|{}|{}|{}", f(a.radius), f(a.start_angle), f(a.end_angle), v(a.normal)),
            a.center,
        ),
        EntityType::LwPolyline(pl) => {
            let first = pl.vertices.first()?.location;
            let shape: Vec<String> = pl
                .vertices
                .iter()
                .map(|p| format!("{},{},{}", f(p.location.x - first.x), f(p.location.y - first.y), f(p.bulge)))
                .collect();
            (
                format!("LWPOLYLINE|{}|{}|{}", pl.is_closed, v(pl.normal), shape.join(";")),
                Vector3::new(first.x, first.y, pl.elevation),
            )
        }
        _ => return None,
    })
}

pub fn evaluate(doc: &CadDocument, code: &str) -> String {
    let mut code = code.trim();
    if let Some(inner) = code.strip_prefix("%<").and_then(|c| c.strip_suffix(">%")) {
        code = inner.trim();
    }
    let body = code.trim_start_matches('\\');
    let json = body.split_once(char::is_whitespace).map(|(_, j)| j.trim()).unwrap_or("");
    if json.is_empty() {
        return "####".into();
    }
    evaluate_json(doc, json).unwrap_or(0).to_string()
}

fn evaluate_json(doc: &CadDocument, json: &str) -> Option<usize> {
    let value = Json::parse(json)?;
    let obj = value.object()?;
    match get(obj, "type")?.str()? {
        "single" => {
            let targets: Vec<Handle> = get(obj, "targets")?
                .array()?
                .iter()
                .filter_map(|t| u64::from_str_radix(t.str()?, 16).ok().map(Handle::new))
                .collect();
            match targets.as_slice() {
                [one] => {
                    let Some(EntityType::Insert(insert)) = doc.get_entity(*one) else {
                        return Some(group_matches(doc, &targets, None).len());
                    };
                    let instances = block_instances(doc, None);
                    Some(count_of(&instances, &insert.block_name, &CountKey::default()))
                }
                [] => Some(0),
                _ => Some(group_matches(doc, &targets, None).len()),
            }
        }
        "block" => {
            let name = get(obj, "name")?.str()?;
            let key = parse_key(get(obj, "key"))?;
            let polygon = get(obj, "boundaryObjectHandle")
                .and_then(|h| h.str())
                .and_then(|h| u64::from_str_radix(h, 16).ok())
                .and_then(|h| boundary_polygon(doc, Handle::new(h)));
            let instances = block_instances(doc, polygon.as_deref());
            Some(count_of(&instances, name, &key))
        }
        _ => Some(0),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CountQuery {
    Block { name: String, key: CountKey, boundary: Option<Handle> },
    Single(Vec<Handle>),
}

pub fn parse_query(json: &str) -> Option<CountQuery> {
    let value = Json::parse(json.trim())?;
    let obj = value.object()?;
    match get(obj, "type")?.str()? {
        "single" => Some(CountQuery::Single(
            get(obj, "targets")?
                .array()?
                .iter()
                .filter_map(|t| u64::from_str_radix(t.str()?, 16).ok().map(Handle::new))
                .collect(),
        )),
        "block" => Some(CountQuery::Block {
            name: get(obj, "name")?.str()?.to_string(),
            key: parse_key(get(obj, "key"))?,
            boundary: get(obj, "boundaryObjectHandle")
                .and_then(|h| h.str())
                .and_then(|h| u64::from_str_radix(h, 16).ok())
                .map(Handle::new),
        }),
        _ => None,
    }
}

fn parse_key(key: Option<&Json>) -> Option<CountKey> {
    let Some(key) = key else {
        return Some(CountKey::default());
    };
    let obj = key.object()?;
    let scale = match get(obj, "scale") {
        Some(s) => {
            let s = s.object()?;
            Some([get(s, "x")?.num()?, get(s, "y")?.num()?, get(s, "z")?.num()?])
        }
        None => None,
    };
    Some(CountKey {
        layer: get(obj, "layer").and_then(|l| l.str()).map(str::to_string),
        scale,
        mirror_state: get(obj, "mirrorState").and_then(|m| match m {
            Json::Bool(b) => Some(*b),
            _ => None,
        }),
    })
}

pub fn boundary_polygon(doc: &CadDocument, handle: Handle) -> Option<Vec<[f64; 2]>> {
    match doc.get_entity(handle)? {
        EntityType::LwPolyline(pl) if valid_boundary(pl) => {
            Some(pl.vertices.iter().map(|v| [v.location.x, v.location.y]).collect())
        }
        _ => None,
    }
}

pub fn valid_boundary(pl: &crate::entities::LwPolyline) -> bool {
    let ring: Vec<[f64; 2]> = pl.vertices.iter().map(|v| [v.location.x, v.location.y]).collect();
    pl.is_closed && ring.len() >= 3 && pl.vertices.iter().all(|v| v.bulge.abs() < 1e-12) && !self_intersects(&ring)
}

pub fn self_intersects(ring: &[[f64; 2]]) -> bool {
    let n = ring.len();
    (0..n).any(|i| {
        ((i + 2)..n).any(|j| {
            // Neighbouring edges share a vertex; the last edge meets the first.
            !(i == 0 && j == n - 1) && segments_cross(ring[i], ring[(i + 1) % n], ring[j], ring[(j + 1) % n])
        })
    })
}

fn segments_cross(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let orient = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| (q[0] - p[0]) * (r[1] - p[1]) - (q[1] - p[1]) * (r[0] - p[0]);
    let on = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        r[0] >= p[0].min(q[0]) && r[0] <= p[0].max(q[0]) && r[1] >= p[1].min(q[1]) && r[1] <= p[1].max(q[1])
    };
    let (d1, d2, d3, d4) = (orient(c, d, a), orient(c, d, b), orient(a, b, c), orient(a, b, d));
    (d1 * d2 < 0.0 && d3 * d4 < 0.0)
        || (d1 == 0.0 && on(c, d, a))
        || (d2 == 0.0 && on(c, d, b))
        || (d3 == 0.0 && on(a, b, c))
        || (d4 == 0.0 && on(a, b, d))
}

pub fn point_in_polygon(p: [f64; 2], polygon: &[[f64; 2]]) -> bool {
    let mut inside = false;
    let n = polygon.len();
    for i in 0..n {
        let (a, b) = (polygon[i], polygon[(i + n - 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
    }
    inside
}

#[derive(Debug, Clone, PartialEq)]
pub enum Piece {
    Segment(Vector3, Vector3),
    Conic { c: Vector3, u: Vector3, v: Vector3, t0: f64, t1: f64 },
    Bezier(Vec<[f64; 4]>),
}

fn dehomog(p: &[f64; 4]) -> Vector3 {
    Vector3::new(p[0] / p[3], p[1] / p[3], p[2] / p[3])
}

fn bezier_split(cp: &[[f64; 4]], t: f64) -> (Vec<[f64; 4]>, Vec<[f64; 4]>) {
    let mut work = cp.to_vec();
    let n = work.len();
    let mut left = vec![work[0]];
    let mut right = vec![work[n - 1]];
    for r in 1..n {
        for j in 0..n - r {
            for k in 0..4 {
                work[j][k] = (1.0 - t) * work[j][k] + t * work[j + 1][k];
            }
        }
        left.push(work[0]);
        right.push(work[n - 1 - r]);
    }
    right.reverse();
    (left, right)
}

fn bezier_meets(cp: &[[f64; 4]], a: [f64; 2], b: [f64; 2], tol: f64, depth: usize) -> bool {
    let pts: Vec<Vector3> = cp.iter().map(dehomog).collect();
    let d = [b[0] - a[0], b[1] - a[1]];
    let len = d[0].hypot(d[1]);
    if len < 1e-15 {
        return false;
    }
    let side = |p: &Vector3| (d[0] * (p.y - a[1]) - d[1] * (p.x - a[0])) / len;
    let along = |p: &Vector3| ((p.x - a[0]) * d[0] + (p.y - a[1]) * d[1]) / (len * len);
    let (smin, smax) = pts.iter().map(side).fold((f64::MAX, f64::MIN), |(lo, hi), s| (lo.min(s), hi.max(s)));
    let (amin, amax) = pts.iter().map(along).fold((f64::MAX, f64::MIN), |(lo, hi), s| (lo.min(s), hi.max(s)));
    if smin > 0.0 || smax < 0.0 || amax < 0.0 || amin > 1.0 {
        return false;
    }
    let size = pts.iter().fold(0.0f64, |m, p| m.max((*p - pts[0]).x.hypot((*p - pts[0]).y)));
    if size <= tol || depth > 60 {
        // Flat enough: the chord decides.
        let (p, q) = (pts[0], pts[pts.len() - 1]);
        return segments_cross([p.x, p.y], [q.x, q.y], a, b) || smin.abs() <= tol || smax.abs() <= tol;
    }
    let (l, r) = bezier_split(cp, 0.5);
    bezier_meets(&l, a, b, tol, depth + 1) || bezier_meets(&r, a, b, tol, depth + 1)
}

fn spline_beziers(degree: usize, knots: &[f64], points: &[Vector3], weights: &[f64]) -> Vec<Piece> {
    let p = degree.max(1);
    if points.len() <= p || knots.len() != points.len() + p + 1 {
        return Vec::new();
    }
    let w = |i: usize| weights.get(i).copied().filter(|w| *w > 0.0).unwrap_or(1.0);
    let mut cp: Vec<[f64; 4]> = points.iter().enumerate().map(|(i, q)| [q.x * w(i), q.y * w(i), q.z * w(i), w(i)]).collect();
    let mut kv = knots.to_vec();
    let (a, b) = (kv[p], kv[cp.len()]);
    let mut distinct: Vec<f64> = kv.iter().copied().filter(|u| *u >= a && *u <= b).collect();
    distinct.dedup_by(|x, y| (*x - *y).abs() < 1e-14);
    for &u in &distinct {
        loop {
            let mult = kv.iter().filter(|k| (**k - u).abs() < 1e-14).count();
            if mult >= p {
                break;
            }
            // Boehm: insert u once.
            let k = kv.iter().rposition(|x| *x <= u + 1e-14).unwrap_or(p).min(cp.len() - 1).max(p);
            let mut next = Vec::with_capacity(cp.len() + 1);
            for i in 0..=cp.len() {
                if i <= k - p {
                    next.push(cp[i]);
                } else if i > k {
                    next.push(cp[i - 1]);
                } else {
                    let den = kv[i + p] - kv[i];
                    let al = if den.abs() < 1e-15 { 0.0 } else { (u - kv[i]) / den };
                    let mut q = [0.0; 4];
                    for c in 0..4 {
                        q[c] = al * cp[i][c] + (1.0 - al) * cp[i - 1][c];
                    }
                    next.push(q);
                }
            }
            kv.insert(k + 1, u);
            cp = next;
        }
    }
    let mut out = Vec::new();
    for k in p..cp.len() {
        if kv[k + 1] - kv[k] > 1e-14 && kv[k] >= a - 1e-14 && kv[k + 1] <= b + 1e-14 {
            out.push(Piece::Bezier(cp[k - p..=k].to_vec()));
        }
    }
    out
}

impl Piece {
    fn start(&self) -> Vector3 {
        match self {
            Piece::Segment(a, _) => *a,
            Piece::Conic { c, u, v, t0, .. } => *c + *u * t0.cos() + *v * t0.sin(),
            Piece::Bezier(cp) => dehomog(&cp[0]),
        }
    }

    fn placed(self, t: &crate::types::Transform, base: Vector3) -> Piece {
        let at = |p: Vector3| t.apply(p - base);
        match self {
            Piece::Segment(a, b) => Piece::Segment(at(a), at(b)),
            Piece::Conic { c, u, v, t0, t1 } => {
                let c2 = at(c);
                Piece::Conic { c: c2, u: at(c + u) - c2, v: at(c + v) - c2, t0, t1 }
            }
            Piece::Bezier(cp) => Piece::Bezier(
                cp.iter()
                    .map(|h| {
                        let q = at(dehomog(h));
                        [q.x * h[3], q.y * h[3], q.z * h[3], h[3]]
                    })
                    .collect(),
            ),
        }
    }

    pub fn points(&self) -> Vec<Vector3> {
        match self {
            Piece::Segment(a, b) => vec![*a, *b],
            Piece::Conic { c, u, v, t0, t1 } => {
                let n = (((t1 - t0) / 5f64.to_radians()).ceil() as usize).max(2);
                (0..=n)
                    .map(|k| {
                        let t = t0 + (t1 - t0) * k as f64 / n as f64;
                        *c + *u * t.cos() + *v * t.sin()
                    })
                    .collect()
            }
            Piece::Bezier(cp) => (0..=16).map(|k| dehomog(&bezier_split(cp, k as f64 / 16.0).0.last().copied().unwrap_or(cp[0]))).collect(),
        }
    }

    fn meets(&self, a: [f64; 2], b: [f64; 2]) -> bool {
        match self {
            Piece::Segment(p, q) => segments_cross([p.x, p.y], [q.x, q.y], a, b),
            Piece::Conic { c, u, v, t0, t1 } => {
                // n·(p(t) − a) = k + α cos t + β sin t = 0 along the edge's line.
                let d = [b[0] - a[0], b[1] - a[1]];
                let n = [-d[1], d[0]];
                let dot = |x: f64, y: f64| n[0] * x + n[1] * y;
                let (k, al, be) = (dot(c.x - a[0], c.y - a[1]), dot(u.x, u.y), dot(v.x, v.y));
                let r = al.hypot(be);
                if r < 1e-15 || k.abs() > r {
                    return false;
                }
                let phi = be.atan2(al);
                let w = (-k / r).clamp(-1.0, 1.0).acos();
                [phi + w, phi - w].iter().any(|&t| {
                    let t = t0 + (t - t0).rem_euclid(std::f64::consts::TAU);
                    if t > t1 + 1e-12 {
                        return false;
                    }
                    let p = *c + *u * t.cos() + *v * t.sin();
                    let s = ((p.x - a[0]) * d[0] + (p.y - a[1]) * d[1]) / (d[0] * d[0] + d[1] * d[1]);
                    (-1e-12..=1.0 + 1e-12).contains(&s)
                })
            }
            Piece::Bezier(cp) => {
                let pts: Vec<Vector3> = cp.iter().map(dehomog).collect();
                let size = pts.iter().fold(0.0f64, |m, p| m.max((*p - pts[0]).x.hypot((*p - pts[0]).y)));
                bezier_meets(cp, a, b, (size * 1e-12).max(1e-12), 0)
            }
        }
    }
}

pub fn inside_polygon(pieces: &[Piece], polygon: &[[f64; 2]]) -> bool {
    let n = polygon.len();
    !pieces.is_empty()
        && pieces.iter().all(|piece| {
            let s = piece.start();
            point_in_polygon([s.x, s.y], polygon) && (0..n).all(|k| !piece.meets(polygon[k], polygon[(k + 1) % n]))
        })
}

pub fn insert_corners(doc: &CadDocument, insert: &Insert, depth: usize) -> Option<Vec<Vector3>> {
    let pts: Vec<Vector3> = insert_outline(doc, insert, depth).iter().flat_map(Piece::points).collect();
    let first = *pts.first()?;
    let (lo, hi) = pts.iter().fold((first, first), |(lo, hi), p| {
        (
            Vector3::new(lo.x.min(p.x), lo.y.min(p.y), lo.z.min(p.z)),
            Vector3::new(hi.x.max(p.x), hi.y.max(p.y), hi.z.max(p.z)),
        )
    });
    let mut out = Vec::with_capacity(8);
    for x in [lo.x, hi.x] {
        for y in [lo.y, hi.y] {
            for z in [lo.z, hi.z] {
                out.push(Vector3::new(x, y, z));
            }
        }
    }
    Some(out)
}

pub fn insert_outline(doc: &CadDocument, insert: &Insert, depth: usize) -> Vec<Piece> {
    let Some(record) = doc.block_records.get(&insert.block_name) else {
        return Vec::new();
    };
    let base = record.base_point;
    let t = insert.get_transform();
    let mut out = Vec::new();
    for &h in &record.entity_handles {
        let pieces = match doc.get_entity(h) {
            Some(EntityType::Insert(nested)) if depth < 8 => insert_outline(doc, nested, depth + 1),
            Some(EntityType::Insert(_)) | Some(EntityType::AttributeDefinition(_)) | None => continue,
            Some(e) => entity_outline(doc, e),
        };
        out.extend(pieces.into_iter().map(|p| p.placed(&t, base)));
    }
    out
}

fn bulge_pieces(v: &[(f64, f64, f64)], closed: bool, elevation: f64, m: &crate::types::Matrix3) -> Vec<Piece> {
    let n = v.len();
    if n == 0 {
        return Vec::new();
    }
    let segs = if closed { n } else { n - 1 };
    let at = |x: f64, y: f64| *m * Vector3::new(x, y, elevation);
    let mut out = Vec::new();
    for i in 0..segs {
        let ((x0, y0, bulge), (x1, y1, _)) = (v[i], v[(i + 1) % n]);
        let (dx, dy) = (x1 - x0, y1 - y0);
        let chord = dx.hypot(dy);
        if bulge.abs() > 1e-12 && chord > 1e-12 {
            // bulge = tan(sweep / 4); the centre lies on the chord's bisector.
            let sweep = 4.0 * bulge.atan();
            let r = chord / (2.0 * (sweep / 2.0).sin().abs());
            let h = r * (sweep / 2.0).cos() * sweep.signum();
            let (cx, cy) = ((x0 + x1) / 2.0 - dy / chord * h, (y0 + y1) / 2.0 + dx / chord * h);
            let a0 = (y0 - cy).atan2(x0 - cx);
            let (t0, t1) = if sweep > 0.0 { (a0, a0 + sweep) } else { (a0 + sweep, a0) };
            out.push(Piece::Conic {
                c: at(cx, cy),
                u: *m * Vector3::new(r, 0.0, 0.0),
                v: *m * Vector3::new(0.0, r, 0.0),
                t0,
                t1,
            });
        } else {
            out.push(Piece::Segment(at(x0, y0), at(x1, y1)));
        }
    }
    out
}

pub fn hatch_outline(h: &crate::entities::Hatch) -> Vec<Piece> {
    use crate::entities::hatch::BoundaryEdge;
    use crate::types::Matrix3;
    let m = Matrix3::arbitrary_axis(h.normal);
    let z = h.elevation;
    let at = |x: f64, y: f64| m * Vector3::new(x, y, z);
    let span = |a0: f64, a1: f64| {
        let s = (a1 - a0).rem_euclid(std::f64::consts::TAU);
        if s < 1e-12 { std::f64::consts::TAU } else { s }
    };
    let mut out = Vec::new();
    for path in &h.paths {
        for edge in &path.edges {
            match edge {
                BoundaryEdge::Line(l) => out.push(Piece::Segment(at(l.start.x, l.start.y), at(l.end.x, l.end.y))),
                BoundaryEdge::Polyline(pl) => {
                    let v: Vec<(f64, f64, f64)> = pl.vertices.iter().map(|q| (q.x, q.y, q.z)).collect();
                    out.extend(bulge_pieces(&v, pl.is_closed, z, &m));
                }
                BoundaryEdge::CircularArc(a) => {
                    // A clockwise edge keeps its angles measured the other way.
                    let s = if a.counter_clockwise { 1.0 } else { -1.0 };
                    out.push(Piece::Conic {
                        c: at(a.center.x, a.center.y),
                        u: m * Vector3::new(a.radius, 0.0, 0.0),
                        v: m * Vector3::new(0.0, s * a.radius, 0.0),
                        t0: a.start_angle,
                        t1: a.start_angle + span(a.start_angle, a.end_angle),
                    });
                }
                BoundaryEdge::EllipticArc(e) => {
                    let s = if e.counter_clockwise { 1.0 } else { -1.0 };
                    let (mx, my) = (e.major_axis_endpoint.x, e.major_axis_endpoint.y);
                    out.push(Piece::Conic {
                        c: at(e.center.x, e.center.y),
                        u: m * Vector3::new(mx, my, 0.0),
                        v: m * Vector3::new(-my * e.minor_axis_ratio * s, mx * e.minor_axis_ratio * s, 0.0),
                        t0: e.start_angle,
                        t1: e.start_angle + span(e.start_angle, e.end_angle),
                    });
                }
                BoundaryEdge::Spline(sp) => {
                    // Rational edges keep their weight in z.
                    let pts: Vec<Vector3> = sp.control_points.iter().map(|q| at(q.x, q.y)).collect();
                    let weights: Vec<f64> = if sp.rational { sp.control_points.iter().map(|q| q.z).collect() } else { Vec::new() };
                    out.extend(spline_beziers(sp.degree.max(1) as usize, &sp.knots, &pts, &weights));
                }
            }
        }
    }
    out
}

pub fn entity_outline(doc: &CadDocument, entity: &EntityType) -> Vec<Piece> {
    use crate::types::Matrix3;
    let tau = std::f64::consts::TAU;
    let circle = |center: Vector3, r: f64, normal: Vector3, t0: f64, t1: f64| {
        let m = Matrix3::arbitrary_axis(normal);
        Piece::Conic { c: m * center, u: m * Vector3::new(r, 0.0, 0.0), v: m * Vector3::new(0.0, r, 0.0), t0, t1 }
    };
    let span = |a0: f64, a1: f64| {
        let s = (a1 - a0).rem_euclid(tau);
        if s < 1e-12 { tau } else { s }
    };
    match entity {
        EntityType::Line(l) => vec![Piece::Segment(l.start, l.end)],
        EntityType::Circle(c) => vec![circle(c.center, c.radius, c.normal, 0.0, tau)],
        EntityType::Arc(a) => vec![circle(a.center, a.radius, a.normal, a.start_angle, a.start_angle + span(a.start_angle, a.end_angle))],
        EntityType::Ellipse(e) => {
            let n = e.normal;
            let minor = Vector3::new(n.y * e.major_axis.z - n.z * e.major_axis.y, n.z * e.major_axis.x - n.x * e.major_axis.z, n.x * e.major_axis.y - n.y * e.major_axis.x) * e.minor_axis_ratio;
            vec![Piece::Conic {
                c: e.center,
                u: e.major_axis,
                v: minor,
                t0: e.start_parameter,
                t1: e.start_parameter + span(e.start_parameter, e.end_parameter),
            }]
        }
        EntityType::LwPolyline(pl) => {
            let v: Vec<(f64, f64, f64)> = pl.vertices.iter().map(|q| (q.location.x, q.location.y, q.bulge)).collect();
            bulge_pieces(&v, pl.is_closed, pl.elevation, &Matrix3::arbitrary_axis(pl.normal))
        }
        EntityType::Spline(s) if !s.control_points.is_empty() => {
            spline_beziers(s.degree.max(1) as usize, &s.knots, &s.control_points, &s.weights)
        }
        EntityType::Hatch(h) => hatch_outline(h),
        EntityType::Insert(ins) => insert_outline(doc, ins, 0),
        e => {
            let b = e.as_entity().bounding_box();
            let z = b.min.z;
            let c = [
                Vector3::new(b.min.x, b.min.y, z),
                Vector3::new(b.max.x, b.min.y, z),
                Vector3::new(b.max.x, b.max.y, z),
                Vector3::new(b.min.x, b.max.y, z),
            ];
            (0..4).map(|i| Piece::Segment(c[i], c[(i + 1) % 4])).collect()
        }
    }
}

fn hatch_loops(h: &crate::entities::Hatch) -> Vec<(Vec<Vector3>, Vec<usize>)> {
    let mut one = h.clone();
    h.paths
        .iter()
        .map(|path| {
            one.paths = vec![path.clone()];
            let mut pts = Vec::new();
            let mut starts = Vec::new();
            for piece in hatch_outline(&one) {
                starts.push(pts.len());
                match &piece {
                    Piece::Segment(a, _) => pts.push(*a),
                    Piece::Conic { c, u, v, t0, t1 } => {
                        let n = (((t1 - t0) / 1f64.to_radians()).ceil() as usize).max(2);
                        pts.extend((0..n).map(|k| {
                            let t = t0 + (t1 - t0) * k as f64 / n as f64;
                            *c + *u * t.cos() + *v * t.sin()
                        }));
                    }
                    Piece::Bezier(cp) => {
                        pts.extend((0..128).map(|k| dehomog(bezier_split(cp, k as f64 / 128.0).0.last().unwrap_or(&cp[0]))));
                    }
                }
            }
            (pts, starts)
        })
        .filter(|(pts, _)| pts.len() >= 2)
        .collect()
}

fn resampled(pts: &[Vector3], from: usize, n: usize) -> Vec<Vector3> {
    let m = pts.len();
    let ring: Vec<Vector3> = (0..=m).map(|i| pts[(from + i) % m]).collect();
    let seg: Vec<f64> = ring.windows(2).map(|w| (w[1] - w[0]).x.hypot((w[1] - w[0]).y)).collect();
    let total: f64 = seg.iter().sum();
    let mut out = Vec::with_capacity(n);
    let (mut i, mut done) = (0, 0.0);
    for k in 0..n {
        let want = total * k as f64 / n as f64;
        while i + 1 < seg.len() && done + seg[i] < want {
            done += seg[i];
            i += 1;
        }
        let f = if seg[i] > 0.0 { (want - done) / seg[i] } else { 0.0 };
        out.push(ring[i] + (ring[i + 1] - ring[i]) * f);
    }
    out
}

fn distance_to_ring(p: Vector3, pts: &[Vector3]) -> f64 {
    let m = pts.len();
    (0..m)
        .map(|i| {
            let (a, b) = (pts[i], pts[(i + 1) % m]);
            let d = b - a;
            let l2 = d.x * d.x + d.y * d.y;
            let t = if l2 > 0.0 { (((p.x - a.x) * d.x + (p.y - a.y) * d.y) / l2).clamp(0.0, 1.0) } else { 0.0 };
            let q = a + d * t;
            (p.x - q.x).hypot(p.y - q.y)
        })
        .fold(f64::MAX, f64::min)
}

#[derive(Clone, Copy)]
struct Similarity {
    mirror: bool,
    from: Vector3,
    to: Vector3,
    s: f64,
    c: f64,
    sn: f64,
}

impl Similarity {
    fn linear(&self, v: Vector3) -> Vector3 {
        let y = if self.mirror { -v.y } else { v.y };
        Vector3::new(self.s * (self.c * v.x - self.sn * y), self.s * (self.sn * v.x + self.c * y), 0.0)
    }
    fn apply(&self, p: Vector3) -> Vector3 {
        let m = |q: Vector3| if self.mirror { Vector3::new(q.x, -q.y, q.z) } else { q };
        let d = m(p) - m(self.from);
        self.to + Vector3::new(self.s * (self.c * d.x - self.sn * d.y), self.s * (self.sn * d.x + self.c * d.y), 0.0)
    }
}

fn same_hatch(a: &crate::entities::Hatch, b: &crate::entities::Hatch) -> bool {
    const N: usize = 96;
    let (la, lb) = (hatch_loops(a), hatch_loops(b));
    if la.len() != lb.len() || la.is_empty() || a.is_solid != b.is_solid || !a.pattern.name.eq_ignore_ascii_case(&b.pattern.name) {
        return false;
    }
    let ra = resampled(&la[0].0, 0, N);
    let size = ra.iter().fold(0.0f64, |m, p| m.max((*p - ra[0]).x.hypot((*p - ra[0]).y)));
    if size <= 0.0 {
        return false;
    }
    let tol = size * 1e-5;
    // A mirrored pattern hatch is not a copy (measured), a mirrored solid one is.
    let mirrors: &[bool] = if a.is_solid { &[false, true] } else { &[false] };
    for &mirror in mirrors {
        for &start in &lb[0].1 {
            let rb = resampled(&lb[0].0, start, N);
            // Two far points of each first loop fix the similarity.
            let m = |q: Vector3| if mirror { Vector3::new(q.x, -q.y, q.z) } else { q };
            let (va, vb) = (ra[N / 2] - ra[0], m(rb[N / 2]) - m(rb[0]));
            let (na, nb) = (va.x.hypot(va.y), vb.x.hypot(vb.y));
            if nb <= 0.0 || na <= 0.0 {
                continue;
            }
            let ang = va.y.atan2(va.x) - vb.y.atan2(vb.x);
            let t = Similarity { mirror, from: rb[0], to: ra[0], s: na / nb, c: ang.cos(), sn: ang.sin() };
            let fits = la.iter().zip(&lb).all(|((pa, _), (pb, _))| {
                let tb: Vec<Vector3> = pb.iter().map(|p| t.apply(*p)).collect();
                resampled(pa, 0, N).iter().all(|p| distance_to_ring(*p, &tb) <= tol)
                    && resampled(&tb, 0, N).iter().all(|p| distance_to_ring(*p, pa) <= tol)
            });
            if fits && (a.is_solid || same_pattern(a, b, &t)) {
                return true;
            }
        }
    }
    false
}

fn same_pattern(a: &crate::entities::Hatch, b: &crate::entities::Hatch, t: &Similarity) -> bool {
    let (pa, pb) = (&a.pattern.lines, &b.pattern.lines);
    if pa.len() != pb.len() {
        return false;
    }
    let near = |x: f64, y: f64, scale: f64| (x - y).abs() <= 1e-6 * scale.max(1.0);
    let integral = |v: f64| (v - v.round()).abs() < 1e-6;
    pa.iter().zip(pb).all(|(la, lb)| {
        let dir_a = Vector3::new(la.angle.cos(), la.angle.sin(), 0.0);
        let mut dir_b = t.linear(Vector3::new(lb.angle.cos(), lb.angle.sin(), 0.0));
        let len = dir_b.x.hypot(dir_b.y);
        if len <= 0.0 {
            return false;
        }
        dir_b = dir_b * (1.0 / len);
        // Directions agree up to a half turn.
        if (dir_a.x * dir_b.y - dir_a.y * dir_b.x).abs() > 1e-6 {
            return false;
        }
        let nrm = Vector3::new(-dir_a.y, dir_a.x, 0.0);
        let oa = Vector3::new(la.offset.x, la.offset.y, 0.0);
        let ob = t.linear(Vector3::new(lb.offset.x, lb.offset.y, 0.0));
        let spacing = oa.x * nrm.x + oa.y * nrm.y;
        if !near(spacing.abs(), (ob.x * nrm.x + ob.y * nrm.y).abs(), spacing.abs()) {
            return false;
        }
        let dash_a: Vec<f64> = la.dash_lengths.clone();
        let dash_b: Vec<f64> = lb.dash_lengths.iter().map(|d| d * t.s).collect();
        if dash_a.len() != dash_b.len() || dash_a.iter().zip(&dash_b).any(|(x, y)| !near(*x, *y, x.abs())) {
            return false;
        }
        let period: f64 = dash_a.iter().map(|d| d.abs()).sum();
        let base_b = t.apply(Vector3::new(lb.base_point.x, lb.base_point.y, 0.0));
        let d = base_b - Vector3::new(la.base_point.x, la.base_point.y, 0.0);
        if spacing.abs() < 1e-12 {
            return true;
        }
        let k = (d.x * nrm.x + d.y * nrm.y) / spacing;
        if !integral(k) {
            return false;
        }
        let r = d - oa * k.round();
        period < 1e-12 || integral((r.x * dir_a.x + r.y * dir_a.y) / period)
    })
}

fn json_string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

// ── a small JSON reader (count codes only) ──────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

fn get<'a>(obj: &'a [(String, Json)], name: &str) -> Option<&'a Json> {
    obj.iter().find(|(k, _)| k == name).map(|(_, v)| v)
}

impl Json {
    fn parse(s: &str) -> Option<Json> {
        let b = s.as_bytes();
        let mut i = 0;
        let v = Self::value(b, &mut i)?;
        Self::ws(b, &mut i);
        (i == b.len()).then_some(v)
    }
    fn object(&self) -> Option<&[(String, Json)]> {
        match self {
            Json::Obj(o) => Some(o),
            _ => None,
        }
    }
    fn array(&self) -> Option<&[Json]> {
        match self {
            Json::Arr(a) => Some(a),
            _ => None,
        }
    }
    fn str(&self) -> Option<&str> {
        match self {
            Json::Str(s) => Some(s),
            _ => None,
        }
    }
    fn num(&self) -> Option<f64> {
        match self {
            Json::Num(n) => Some(*n),
            _ => None,
        }
    }
    fn ws(b: &[u8], i: &mut usize) {
        while *i < b.len() && b[*i].is_ascii_whitespace() {
            *i += 1;
        }
    }
    fn value(b: &[u8], i: &mut usize) -> Option<Json> {
        Self::ws(b, i);
        match *b.get(*i)? {
            b'{' => {
                *i += 1;
                let mut out = Vec::new();
                Self::ws(b, i);
                if b.get(*i) == Some(&b'}') {
                    *i += 1;
                    return Some(Json::Obj(out));
                }
                loop {
                    Self::ws(b, i);
                    let Json::Str(k) = Self::value(b, i)? else { return None };
                    Self::ws(b, i);
                    (b.get(*i) == Some(&b':')).then_some(())?;
                    *i += 1;
                    out.push((k, Self::value(b, i)?));
                    Self::ws(b, i);
                    match b.get(*i)? {
                        b',' => *i += 1,
                        b'}' => {
                            *i += 1;
                            return Some(Json::Obj(out));
                        }
                        _ => return None,
                    }
                }
            }
            b'[' => {
                *i += 1;
                let mut out = Vec::new();
                Self::ws(b, i);
                if b.get(*i) == Some(&b']') {
                    *i += 1;
                    return Some(Json::Arr(out));
                }
                loop {
                    out.push(Self::value(b, i)?);
                    Self::ws(b, i);
                    match b.get(*i)? {
                        b',' => *i += 1,
                        b']' => {
                            *i += 1;
                            return Some(Json::Arr(out));
                        }
                        _ => return None,
                    }
                }
            }
            b'"' => {
                *i += 1;
                let mut s = String::new();
                loop {
                    let c = *b.get(*i)?;
                    *i += 1;
                    match c {
                        b'"' => return Some(Json::Str(s)),
                        b'\\' => {
                            let e = *b.get(*i)?;
                            *i += 1;
                            match e {
                                b'n' => s.push('\n'),
                                b't' => s.push('\t'),
                                b'r' => s.push('\r'),
                                b'b' => s.push('\u{8}'),
                                b'f' => s.push('\u{c}'),
                                b'u' => {
                                    let hex = std::str::from_utf8(b.get(*i..*i + 4)?).ok()?;
                                    *i += 4;
                                    s.push(char::from_u32(u32::from_str_radix(hex, 16).ok()?)?);
                                }
                                other => s.push(other as char),
                            }
                        }
                        _ => {
                            // Copy a whole UTF-8 sequence.
                            let start = *i - 1;
                            let len = match c {
                                0..=0x7F => 1,
                                0xC0..=0xDF => 2,
                                0xE0..=0xEF => 3,
                                _ => 4,
                            };
                            *i = start + len;
                            s.push_str(std::str::from_utf8(b.get(start..*i)?).ok()?);
                        }
                    }
                }
            }
            b't' if b[*i..].starts_with(b"true") => {
                *i += 4;
                Some(Json::Bool(true))
            }
            b'f' if b[*i..].starts_with(b"false") => {
                *i += 5;
                Some(Json::Bool(false))
            }
            b'n' if b[*i..].starts_with(b"null") => {
                *i += 4;
                Some(Json::Null)
            }
            _ => {
                let start = *i;
                while *i < b.len() && matches!(b[*i], b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9') {
                    *i += 1;
                }
                std::str::from_utf8(&b[start..*i]).ok()?.parse().ok().map(Json::Num)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_and_keys() {
        assert!(Json::parse("{\"a\":[1,2.5,true],\"b\":{}}").is_some());
        assert!(Json::parse("{bad}").is_none());
        let key = CountKey { layer: Some("0".into()), scale: Some([2.0; 3]), mirror_state: Some(false) };
        assert_eq!(
            block_json("CHAIR", &key),
            r#"{"key":{"layer":"0","mirrorState":false,"scale":{"x":2.0,"y":2.0,"z":2.0}},"name":"CHAIR","type":"block"}"#
        );
        assert_eq!(key.label(), "0_2.0000_Default");
        assert_eq!(parse_key(Json::parse(&key.json()).as_ref()), Some(key));
        assert_eq!(
            single_json(&[Handle::new(0x8D1A)]),
            r#"{"groupCondition":{"attributes":[],"parameters":[],"properties":[]},"targets":["8D1A"],"type":"single"}"#
        );
        assert!(point_in_polygon([1.0, 1.0], &[[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]]));
        assert!(!point_in_polygon([3.0, 1.0], &[[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]]));
    }
}
