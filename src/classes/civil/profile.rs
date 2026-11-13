//! Profiles (vertical alignments) and profile views.
//!
//! A design profile keeps its entities in records like an alignment's
//! elements: an 11-bit entity prefix (2 tangent, 4 circular curve,
//! 6 parabola, 8 asymmetric parabola), a 23-bit header (`00`, `0x08`,
//! `0x00`, five zero bits), then `BL constraint, BL id, BL previous,
//! BL next` and the constraint's data, in station/elevation:
//!
//! ```text
//! tangent 0:   2RD start, 2RD end        4: 2RD end (from the curve before)
//!              9: nothing (between two curves)
//! parabola 0 / 2:  BD 0, BD length / K, BL 0x1000 sag (+1 crest)
//!          11: BD 0, 3 × 2RD (start, on it, end)
//!          12: BD 0, 2RD start, 2RD end, BL 0 start / 1 end, BD grade %
//!          13: BD 0, 2RD start, 2RD end, BL sag / crest, BL 0x2000 K
//!              (0x2002 radius), BD value
//!          32: BD 0, 2RD end, BL, BL parameter, BD value (hangs from
//!              the tangent before)
//!          34: BD 0, 2RD end, BL 1, BD grade % there
//! circular 0 / 9: BD length / radius     7: 2RD through point
//! asymmetric 10: BD length before, BD length after the PVI
//! ```
//!
//! A surface profile stores its sampled segments as `2RD start, 2RD end`
//! pairs that chain end to start. A profile view stores its station and
//! elevation range as three 3BD points `(st0, z0, 0) (st0, z0, 0)
//! (st1, z1, 0)`; its proxy graphics hold the frame rectangle.

use super::bits::Bits;
use crate::types::Vector2;

/// How a vertical curve is given.
#[derive(Clone, Copy)]
enum CurveDef {
    ParabolaLength(f64),
    ParabolaK(f64),
    CircularLength(f64),
    CircularRadius(f64),
    CircularThrough(Vector2),
    Asymmetric(f64, f64),
    ParabolaThreePoints([Vector2; 3]),
    /// Start, end, sag, `a` of the parabola.
    ParabolaTwoPoints(Vector2, Vector2, f64),
    /// Start, end, grade at the end (`true`) or the start.
    ParabolaTwoPointsGrade(Vector2, Vector2, bool, f64),
    /// End point and `|a|`; hangs from the tangent before.
    ParabolaFloating(Vector2, f64),
    /// End point and the grade there; hangs from the tangent before.
    ParabolaFloatingGrade(Vector2, f64),
}

#[derive(Clone, Copy)]
enum Entity {
    Tangent(Vector2, Vector2),
    /// A tangent leaving the curve before towards a point, or (`None`)
    /// touching the curves on both sides.
    Dependent(Option<Vector2>),
    Curve(CurveDef),
}

struct Record {
    /// Kind prefix and constraint code.
    code: (u32, i32),
    id: i32,
    previous: i32,
    next: i32,
    entity: Entity,
    /// Crest (`true`) or sag where the record says.
    crest: Option<bool>,
    /// A parabola's minimum K values from the list (-1 read as `None`).
    minimum_k: Option<[Option<f64>; 3]>,
    /// Where the fields this crate reads end.
    end: usize,
}

/// `a` of a parabola (elevation'' = 2a) from K or a radius (R = 100 K).
fn parabola_a(param: i32, value: f64, sag: bool) -> Option<f64> {
    let k = match param {
        0x2000 => value,
        0x2002 => value / 100.0,
        _ => return None,
    };
    (k > 0.0 && k.is_finite()).then(|| if sag { 1.0 } else { -1.0 } / (200.0 * k))
}

fn sag_or_crest(flag: i32) -> Option<bool> {
    match flag {
        0x1001 => Some(true),
        0x1000 => Some(false),
        _ => None,
    }
}

fn record_at(data: &[u8], pos: usize, end: usize) -> Option<Record> {
    if pos < 11 {
        return None;
    }
    let mut bits = Bits::new(data, pos - 11, end);
    let mut prefix = 0u32;
    for _ in 0..11 {
        prefix = (prefix << 1) | u32::from(bits.bit()?);
    }
    let mut header = 0u32;
    for _ in 0..23 {
        header = (header << 1) | u32::from(bits.bit()?);
    }
    if header != 0x08 << 13 {
        return None;
    }
    let constraint = bits.bit_long()?;
    let id = bits.bit_long()?;
    let previous = bits.bit_long()?;
    let next = bits.bit_long()?;
    let small = |v: i32| (0..100_000).contains(&v);
    if id < 1 || !small(id) || !small(previous) || !small(next) {
        return None;
    }
    let point = |bits: &mut Bits| {
        let p = Vector2::new(bits.raw_double()?, bits.raw_double()?);
        (p.x.is_finite() && p.y.is_finite() && p.x.abs() < 1e9 && p.y.abs() < 1e9).then_some(p)
    };
    let positive = |v: f64| (v.is_finite() && v > 0.0).then_some(v);
    let zero = |bits: &mut Bits| (bits.bit_double()? == 0.0).then_some(());
    let mut crest = None;
    let entity = match (prefix, constraint) {
        (2, 0) => Entity::Tangent(point(&mut bits)?, point(&mut bits)?),
        (2, 4) => Entity::Dependent(Some(point(&mut bits)?)),
        (2, 9) => Entity::Dependent(None),
        (6, 0 | 2) => {
            zero(&mut bits)?;
            let v = positive(bits.bit_double()?)?;
            Entity::Curve(if constraint == 0 { CurveDef::ParabolaLength(v) } else { CurveDef::ParabolaK(v) })
        }
        (6, 11) => {
            zero(&mut bits)?;
            Entity::Curve(CurveDef::ParabolaThreePoints([point(&mut bits)?, point(&mut bits)?, point(&mut bits)?]))
        }
        (6, 12) => {
            zero(&mut bits)?;
            let (a, b) = (point(&mut bits)?, point(&mut bits)?);
            let at_end = bits.bit_long()?;
            let grade = bits.bit_double().filter(|g| g.is_finite())? / 100.0;
            (0..=1).contains(&at_end).then_some(())?;
            Entity::Curve(CurveDef::ParabolaTwoPointsGrade(a, b, at_end == 1, grade))
        }
        (6, 13) => {
            zero(&mut bits)?;
            let (a, b) = (point(&mut bits)?, point(&mut bits)?);
            let flag = bits.bit_long()?;
            crest = sag_or_crest(flag);
            let param = bits.bit_long()?;
            let k = parabola_a(param, bits.bit_double()?, flag == 0x1000)?;
            Entity::Curve(CurveDef::ParabolaTwoPoints(a, b, k))
        }
        (6, 32) => {
            zero(&mut bits)?;
            let through = point(&mut bits)?;
            crest = sag_or_crest(bits.bit_long()?);
            let param = bits.bit_long()?;
            let a = parabola_a(param, bits.bit_double()?, true)?;
            Entity::Curve(CurveDef::ParabolaFloating(through, a))
        }
        (6, 34) => {
            zero(&mut bits)?;
            let through = point(&mut bits)?;
            (bits.bit_long()? == 1).then_some(())?;
            let grade = bits.bit_double().filter(|g| g.is_finite())? / 100.0;
            Entity::Curve(CurveDef::ParabolaFloatingGrade(through, grade))
        }
        (4, 0 | 9) => {
            let v = positive(bits.bit_double()?)?;
            Entity::Curve(if constraint == 0 { CurveDef::CircularLength(v) } else { CurveDef::CircularRadius(v) })
        }
        (4, 7) => Entity::Curve(CurveDef::CircularThrough(point(&mut bits)?)),
        (8, 10) => Entity::Curve(CurveDef::Asymmetric(positive(bits.bit_double()?)?, positive(bits.bit_double()?)?)),
        _ => return None,
    };
    Some(Record {
        code: (prefix, constraint),
        id,
        previous,
        next,
        entity,
        crest,
        minimum_k: None,
        end: bits.pos,
    })
}

/// How a vertical curve placed at its PVI runs.
#[derive(Clone, Copy)]
enum Form {
    Parabola(f64),
    Asymmetric(f64, f64),
    Circle { center: Vector2, radius: f64 },
}

/// A vertical curve placed at its PVI between the grades `g1` and `g2`.
#[derive(Clone, Copy)]
struct Placed {
    pvi: Vector2,
    g1: f64,
    g2: f64,
    form: Form,
}

fn unit_grade(g: f64) -> Vector2 {
    Vector2::new(1.0, g) * (1.0 / (1.0 + g * g).sqrt())
}

impl Placed {
    /// 1 for a sag (centre above), -1 for a crest.
    fn turn(&self) -> f64 {
        if self.g2 >= self.g1 {
            1.0
        } else {
            -1.0
        }
    }

    /// Where a circle meets the incoming and the outgoing grade.
    fn circle_ends(&self, center: Vector2, radius: f64) -> (Vector2, Vector2) {
        let s = self.turn();
        let touch = |u: Vector2| center - Vector2::new(-u.y * s, u.x * s) * radius;
        (touch(unit_grade(self.g1)), touch(unit_grade(self.g2)))
    }

    fn start(&self) -> Vector2 {
        let back = |l: f64| Vector2::new(self.pvi.x - l, self.pvi.y - self.g1 * l);
        match self.form {
            Form::Parabola(l) => back(l / 2.0),
            Form::Asymmetric(l1, _) => back(l1),
            Form::Circle { center, radius } => self.circle_ends(center, radius).0,
        }
    }

    fn end(&self) -> Vector2 {
        let on = |l: f64| Vector2::new(self.pvi.x + l, self.pvi.y + self.g2 * l);
        match self.form {
            Form::Parabola(l) => on(l / 2.0),
            Form::Asymmetric(_, l2) => on(l2),
            Form::Circle { center, radius } => self.circle_ends(center, radius).1,
        }
    }

    fn elevation_at(&self, x: f64) -> f64 {
        let a = self.g2 - self.g1;
        match self.form {
            Form::Parabola(l) => {
                let s = self.start();
                let t = x - s.x;
                s.y + self.g1 * t + a / (2.0 * l) * t * t
            }
            Form::Asymmetric(l1, l2) => {
                let e = a * l1 * l2 / (2.0 * (l1 + l2));
                if x <= self.pvi.x {
                    let s = self.start();
                    let t = x - s.x;
                    s.y + self.g1 * t + e / (l1 * l1) * t * t
                } else {
                    let s = self.end();
                    let t = s.x - x;
                    s.y - self.g2 * t + e / (l2 * l2) * t * t
                }
            }
            Form::Circle { center, radius } => {
                let dx = x - center.x;
                center.y - self.turn() * (radius * radius - dx * dx).max(0.0).sqrt()
            }
        }
    }
}

/// The circle tangent to both grades at the PVI that `def` gives; the
/// length given is the horizontal one.
fn placed_circle(pvi: Vector2, g1: f64, g2: f64, def: CurveDef) -> Option<(Vector2, f64)> {
    let s = if g2 >= g1 { 1.0 } else { -1.0 };
    let (u1, u2) = (unit_grade(g1), unit_grade(g2));
    let delta = (u1.x * u2.y - u1.y * u2.x).abs().asin();
    let t = (delta / 2.0).tan();
    // The centre per unit radius: back along the first grade by
    // tan(delta/2), then across.
    let w = Vector2::new(-t * u1.x - s * u1.y, -t * u1.y + s * u1.x);
    let radius = match def {
        CurveDef::CircularRadius(r) => r,
        CurveDef::CircularLength(l) => l / (u1.y - u2.y).abs(),
        CurveDef::CircularThrough(p) => {
            // |PVI + R w - P| = R.
            let d = p - pvi;
            let a = w.dot(&w) - 1.0;
            let b = -2.0 * w.dot(&d);
            let c = d.dot(&d);
            let disc = (b * b - 4.0 * a * c).max(0.0).sqrt();
            // The radius whose arc spans the point's station.
            [(-b + disc) / (2.0 * a), (-b - disc) / (2.0 * a)].into_iter().filter(|r| *r > 0.0).find(|r| {
                let x1 = pvi.x + r * (w.x + s * u1.y);
                let x2 = pvi.x + r * (w.x + s * u2.y);
                (x1.min(x2) - 1e-9..=x1.max(x2) + 1e-9).contains(&p.x)
            })?
        }
        _ => return None,
    };
    (radius.is_finite() && radius > 0.0).then(|| (pvi + w * radius, radius))
}

/// A resolved profile entity's shape.
#[derive(Clone, Copy)]
enum Shape {
    Line { x0: f64, y0: f64, grade: f64 },
    /// elevation = a s^2 + b s + c.
    Parabola { a: f64, b: f64, c: f64 },
    Placed(Placed),
}

impl Shape {
    fn line(p: Vector2, q: Vector2) -> Self {
        Self::Line {
            x0: p.x,
            y0: p.y,
            grade: (q.y - p.y) / (q.x - p.x),
        }
    }

    /// The parabola with `a` through `p` with grade `g` there.
    fn parabola(p: Vector2, g: f64, a: f64) -> Self {
        let b = g - 2.0 * a * p.x;
        Self::Parabola {
            a,
            b,
            c: p.y - (a * p.x + b) * p.x,
        }
    }

    fn elevation_at(&self, x: f64) -> f64 {
        match *self {
            Self::Line { x0, y0, grade } => y0 + grade * (x - x0),
            Self::Parabola { a, b, c } => (a * x + b) * x + c,
            Self::Placed(p) => p.elevation_at(x),
        }
    }

    fn grade_at(&self, x: f64) -> f64 {
        match *self {
            Self::Line { grade, .. } => grade,
            Self::Parabola { a, b, .. } => 2.0 * a * x + b,
            Self::Placed(p) => {
                let h = 1e-6;
                (p.elevation_at(x + h) - p.elevation_at(x - h)) / (2.0 * h)
            }
        }
    }

    fn at(&self, x: f64) -> Vector2 {
        Vector2::new(x, self.elevation_at(x))
    }

    fn piece(&self) -> ProfilePiece {
        match self {
            Self::Line { .. } => ProfilePiece::Tangent,
            Self::Placed(Placed {
                form: Form::Circle { .. },
                ..
            }) => ProfilePiece::Circular,
            Self::Placed(Placed {
                form: Form::Asymmetric(..),
                ..
            }) => ProfilePiece::AsymmetricParabola,
            _ => ProfilePiece::Parabola,
        }
    }
}

/// The parabola through three points.
fn parabola_through(p: [Vector2; 3]) -> Option<Shape> {
    let [p0, p1, p2] = p;
    let d = (p0.x - p1.x) * (p0.x - p2.x) * (p1.x - p2.x);
    if d == 0.0 {
        return None;
    }
    let a = (p2.x * (p1.y - p0.y) + p1.x * (p0.y - p2.y) + p0.x * (p2.y - p1.y)) / d;
    let b = (p2.x * p2.x * (p0.y - p1.y) + p1.x * p1.x * (p2.y - p0.y) + p0.x * p0.x * (p1.y - p2.y)) / d;
    Some(Shape::Parabola {
        a,
        b,
        c: p0.y - (a * p0.x + b) * p0.x,
    })
}

/// A root of `f` on `[lo, hi]`: the first sign change, bisected.
fn root(f: impl Fn(f64) -> f64, lo: f64, hi: f64) -> Option<f64> {
    let n = 400;
    let mut prev = (lo, f(lo));
    for i in 1..=n {
        let x = lo + (hi - lo) * i as f64 / n as f64;
        let cur = (x, f(x));
        if prev.1 == 0.0 {
            return Some(prev.0);
        }
        if prev.1.is_finite() && cur.1.is_finite() && prev.1.signum() != cur.1.signum() {
            let (mut a, mut b) = (prev.0, cur.0);
            for _ in 0..200 {
                let m = 0.5 * (a + b);
                let fm = f(m);
                if !fm.is_finite() {
                    break;
                }
                if (fm > 0.0) == (prev.1 > 0.0) {
                    a = m;
                } else {
                    b = m;
                }
            }
            return Some(0.5 * (a + b));
        }
        prev = cur;
    }
    None
}

/// A resolved entity from `start` to `end`.
#[derive(Clone, Copy)]
struct Segment {
    start: Vector2,
    end: Vector2,
    shape: Shape,
}

/// The entity list at `pos`: `BL n`, then n records end to end (each its
/// 11-bit kind prefix, the record, and the fields after it: 4 bits after a
/// tangent, the parabola tail below after a parabola), IDs distinct and
/// links inside the list.
fn record_list_at(data: &[u8], pos: usize, end: usize) -> Option<Vec<Record>> {
    let mut bits = Bits::new(data, pos, end);
    let n = usize::try_from(bits.bit_long()?).ok().filter(|n| (1..100_000).contains(n))?;
    let mut at = bits.pos;
    let mut out: Vec<Record> = Vec::with_capacity(n);
    // After a parabola fixed by length or K: `BL 0x1001` crest (`0x1000`
    // sag), `BL 0`, 3 bits; after the other parabolas: `BD, BD`, 1 bit.
    // Then the three minimum K values (-1 without design criteria).
    let parabola_tail = |record: &mut Record, crest_after: bool| -> Option<usize> {
        let mut b = Bits::new(data, record.end, end);
        if crest_after {
            record.crest = Some(sag_or_crest(b.bit_long()?)?);
            (b.bit_long()? == 0).then_some(())?;
            b.pos += 3;
        } else {
            b.bit_double()?;
            b.bit_double()?;
            b.pos += 1;
        }
        let mut k = [None; 3];
        for v in &mut k {
            *v = Some(b.bit_double()?).filter(|v| *v >= 0.0);
        }
        record.minimum_k = Some(k);
        Some(b.pos)
    };
    for i in 0..n {
        let mut record = record_at(data, at + 11, end)?;
        let last = i + 1 == n;
        let next = match record.code {
            (2, _) => record.end + 4,
            (6, c) => parabola_tail(&mut record, matches!(c, 0 | 2))?,
            _ if last => record.end,
            // debt: no sample has a circular or asymmetric curve before
            // another record; its tail is found by trying both forms.
            _ => [true, false]
                .into_iter()
                .find_map(|after| {
                    let mut probe = record_at(data, at + 11, end)?;
                    parabola_tail(&mut probe, after).filter(|e| record_at(data, e + 11, end).is_some())
                })?,
        };
        out.push(record);
        at = next;
    }
    let ids: Vec<i32> = out.iter().map(|r| r.id).collect();
    let known = |id: i32| id == 0 || ids.contains(&id);
    let distinct = ids.iter().enumerate().all(|(i, id)| !ids[..i].contains(id));
    (distinct && out.iter().all(|r| known(r.previous) && known(r.next))).then_some(out)
}

/// Where a profile's criteria bits and entity list sit: after the class
/// keys (see [`super::alignment::read_keys`]), `BL, BD, BD, BD, 5 x BS`,
/// 10 bits (criteria in the last three), then the entity list.
fn section(data: &[u8], start: usize, end: usize) -> Option<(usize, usize)> {
    let mut bits = Bits::new(data, start, end);
    super::alignment::read_keys(&mut bits)?;
    bits.bit_long()?;
    for _ in 0..3 {
        bits.bit_double()?;
    }
    for _ in 0..5 {
        bits.bit_short()?;
    }
    let criteria_at = bits.pos;
    bits.pos += 10;
    (bits.pos < end).then_some((criteria_at, bits.pos))
}

/// The entity records in link order, run by run: each run starts at a
/// record without a predecessor (a surface profile sampled in pieces has
/// several, and Civil 3D draws them all).
fn runs(data: &[u8], start: usize, end: usize) -> Vec<Vec<Record>> {
    let records: Vec<Record> = section(data, start, end)
        .and_then(|(_, list)| match Bits::new(data, list, end).bit_long()? {
            0 => Some(Vec::new()),
            _ => record_list_at(data, list, end),
        })
        .unwrap_or_default();
    let mut runs: Vec<Vec<usize>> = Vec::new();
    for (i, _) in records.iter().enumerate().filter(|(_, r)| r.previous == 0) {
        let mut run = vec![i];
        while run.len() <= records.len() {
            let next_id = records[*run.last().unwrap()].next;
            match records.iter().position(|r| next_id != 0 && r.id == next_id) {
                Some(j) if !run.iter().any(|c| records[*c].id == records[j].id) => run.push(j),
                _ => break,
            }
        }
        runs.push(run);
    }
    let mut records: Vec<Option<Record>> = records.into_iter().map(Some).collect();
    runs.into_iter().map(|run| run.into_iter().filter_map(|i| records[i].take()).collect()).collect()
}

/// Every entity record in link order, run after run.
fn chain(data: &[u8], start: usize, end: usize) -> Vec<Record> {
    runs(data, start, end).into_iter().flatten().collect()
}

/// Ids of a design profile's entities in link order, each marked `true`
/// for a curve, with a fixed tangent's end station.
pub(super) fn entity_ids(data: &[u8], start: usize, end: usize) -> Vec<(i32, bool, Option<f64>)> {
    chain(data, start, end)
        .iter()
        .map(|r| match r.entity {
            Entity::Tangent(_, b) => (r.id, false, Some(b.x)),
            Entity::Curve(_) => (r.id, true, None),
            _ => (r.id, false, None),
        })
        .collect()
}

/// Every run's entities resolved into drawn segments.
fn design(data: &[u8], start: usize, end: usize) -> Option<Vec<Vec<Segment>>> {
    runs(data, start, end).iter().map(|run| solve(run)).collect()
}

/// One run's entities, each resolved: fixed tangents and curves stand on
/// their points, a free curve sits at the PVI of the tangents beside it, a
/// floating curve hangs from the tangent before it, a floating tangent
/// leaves the curve before it towards its point, a free tangent touches the
/// curves on both sides.
fn solve(owned: &[Record]) -> Option<Vec<Segment>> {
    // A tangent running backwards (one closes a surface profile) ends what
    // is drawn.
    let chain: Vec<&Record> = owned
        .iter()
        .take_while(|r| !matches!(r.entity, Entity::Tangent(a, b) if b.x < a.x))
        .collect();
    let n = chain.len();
    let mut shape: Vec<Option<Shape>> = vec![None; n];
    let mut from: Vec<Option<Vector2>> = vec![None; n];
    let mut to: Vec<Option<Vector2>> = vec![None; n];
    for (i, r) in chain.iter().enumerate() {
        match r.entity {
            Entity::Tangent(a, b) => {
                // Fixed tangents in a row meet end to start; a stray header
                // in a surface profile does not.
                if let Some(Entity::Tangent(_, last)) = i.checked_sub(1).map(|j| chain[j].entity) {
                    (last.distance(&a) <= 1e-6 * (1.0 + a.x.abs())).then_some(())?;
                }
                // A step (no station length) draws nothing.
                shape[i] = Some(if b.x > a.x { Shape::line(a, b) } else { Shape::Line { x0: a.x, y0: a.y, grade: 0.0 } });
                (from[i], to[i]) = (Some(a), Some(b));
            }
            Entity::Dependent(through) => to[i] = through,
            Entity::Curve(CurveDef::ParabolaThreePoints(p)) => {
                shape[i] = Some(parabola_through(p)?);
                (from[i], to[i]) = (Some(p[0]), Some(p[2]));
            }
            Entity::Curve(CurveDef::ParabolaTwoPoints(p, q, a)) => {
                let l = q.x - p.x;
                // The chord's grade is the grade at the middle.
                shape[i] = Some(Shape::parabola(p, (q.y - p.y) / l - a * l, a));
                (from[i], to[i]) = (Some(p), Some(q));
            }
            Entity::Curve(CurveDef::ParabolaTwoPointsGrade(p, q, at_end, g)) => {
                let l = q.x - p.x;
                let chord = (q.y - p.y) / l;
                let a = if at_end { (g - chord) / l } else { (chord - g) / l };
                shape[i] = Some(Shape::parabola(p, if at_end { g - 2.0 * a * l } else { g }, a));
                (from[i], to[i]) = (Some(p), Some(q));
            }
            Entity::Curve(_) => {}
        }
    }
    for i in 0..n {
        let prev = i.checked_sub(1);
        match chain[i].entity {
            Entity::Curve(CurveDef::ParabolaFloating(through, a)) => {
                let line = shape[prev?]?;
                // Hangs from the line: line + a (s - s0)^2 through the point.
                let off = through.y - line.elevation_at(through.x);
                let a = a.abs() * off.signum();
                let s0 = through.x - (off / a).sqrt();
                let p0 = line.at(s0);
                shape[i] = Some(Shape::parabola(p0, line.grade_at(s0), a));
                (from[i], to[i]) = (Some(p0), Some(through));
                to[prev?] = Some(p0);
            }
            Entity::Curve(CurveDef::ParabolaFloatingGrade(through, gp)) => {
                let line = shape[prev?]?;
                let g = line.grade_at(through.x);
                let d = 2.0 * (through.y - line.elevation_at(through.x)) / (gp - g);
                let a = (gp - g) / (2.0 * d);
                (d > 0.0 && a.is_finite()).then_some(())?;
                let p0 = line.at(through.x - d);
                shape[i] = Some(Shape::parabola(p0, g, a));
                (from[i], to[i]) = (Some(p0), Some(through));
                to[prev?] = Some(p0);
            }
            Entity::Curve(
                def @ (CurveDef::ParabolaLength(_)
                | CurveDef::ParabolaK(_)
                | CurveDef::CircularLength(_)
                | CurveDef::CircularRadius(_)
                | CurveDef::CircularThrough(_)
                | CurveDef::Asymmetric(..)),
            ) => {
                // Between two fixed tangents, at their shared point.
                let (Entity::Tangent(a0, a1), Some(Entity::Tangent(b0, b1))) =
                    (chain[prev?].entity, chain.get(i + 1).map(|r| r.entity))
                else {
                    return None;
                };
                let (g1, g2) = ((a1.y - a0.y) / (a1.x - a0.x), (b1.y - b0.y) / (b1.x - b0.x));
                let form = match def {
                    CurveDef::ParabolaLength(l) => Form::Parabola(l),
                    CurveDef::ParabolaK(k) => Form::Parabola(k * 100.0 * (g2 - g1).abs()),
                    CurveDef::Asymmetric(l1, l2) => Form::Asymmetric(l1, l2),
                    def => {
                        let (center, radius) = placed_circle(a1, g1, g2, def)?;
                        Form::Circle { center, radius }
                    }
                };
                let placed = Placed { pvi: a1, g1, g2, form };
                shape[i] = Some(Shape::Placed(placed));
                (from[i], to[i]) = (Some(placed.start()), Some(placed.end()));
                to[prev?] = from[i];
                from[i + 1] = to[i];
            }
            Entity::Dependent(Some(p)) => {
                let before = shape[prev?]?;
                let lo = from[prev?]?.x;
                let s = root(|t| before.elevation_at(t) + before.grade_at(t) * (p.x - t) - p.y, lo, p.x)?;
                let q = before.at(s);
                shape[i] = Some(Shape::line(q, p));
                from[i] = Some(q);
                to[prev?] = Some(q);
            }
            Entity::Dependent(None) => {
                let (before, after) = (shape[prev?]?, shape.get(i + 1).copied().flatten()?);
                let (lo1, hi1) = (from[prev?]?.x, to[prev?]?.x);
                let (lo2, hi2) = (from[i + 1]?.x, to[i + 1]?.x);
                // A line touching the curve before at t1 touches the one
                // after where that curve's grade is the line's.
                let touch = |t1: f64| -> Option<(f64, f64)> {
                    let g = before.grade_at(t1);
                    let t2 = root(|t| after.grade_at(t) - g, lo2, hi2)?;
                    Some((t2, after.elevation_at(t2) - (before.elevation_at(t1) + g * (t2 - t1))))
                };
                let t1 = root(|t| touch(t).map_or(f64::NAN, |v| v.1), lo1, hi1)?;
                let t2 = touch(t1)?.0;
                let (p, q) = (before.at(t1), after.at(t2));
                shape[i] = Some(Shape::line(p, q));
                (from[i], to[i]) = (Some(p), Some(q));
                to[prev?] = Some(p);
                from[i + 1] = Some(q);
            }
            _ => {}
        }
    }
    (0..n)
        .map(|i| {
            Some(Segment {
                start: from[i]?,
                end: to[i]?,
                shape: shape[i]?,
            })
        })
        .collect()
}

/// A resolved entity's points: its ends, or the curve sampled.
fn sample(segment: &Segment) -> Vec<Vector2> {
    match segment.shape {
        Shape::Line { .. } => vec![segment.start, segment.end],
        shape => (0..=32)
            .map(|i| {
                let x = segment.start.x + (segment.end.x - segment.start.x) * f64::from(i) / 32.0;
                match i {
                    0 => segment.start,
                    32 => segment.end,
                    _ => shape.at(x),
                }
            })
            .collect(),
    }
}

/// Tangents and vertical curves of a profile, in station order.
pub(super) fn pieces(data: &[u8], start: usize, end: usize) -> Vec<(ProfilePiece, Vec<Vector2>)> {
    design(data, start, end)
        .unwrap_or_default()
        .iter()
        .flatten()
        .filter(|s| s.end.x > s.start.x + 1e-9)
        .map(|s| (s.shape.piece(), sample(s)))
        .collect()
}

/// Station/elevation polylines of a profile.
pub(super) fn polylines(data: &[u8], start: usize, end: usize) -> Vec<Vec<Vector2>> {
    let mut out: Vec<Vec<Vector2>> = Vec::new();
    for (_, run) in pieces(data, start, end) {
        match out.last_mut() {
            Some(last) if last.last().is_some_and(|p| p.distance(&run[0]) < 1e-9) => last.extend(&run[1..]),
            _ => out.push(run),
        }
    }
    out
}

/// A vertical point of intersection.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Pvi {
    /// Station and elevation.
    pub point: Vector2,
    /// Length of the vertical curve at this PVI, 0 without one.
    pub curve_length: f64,
}

/// PVIs of a profile: its start, every joint (a curve's PVI where the
/// grades on both sides of it meet), and its end.
pub(super) fn pvis(data: &[u8], start: usize, end: usize) -> Vec<Pvi> {
    design(data, start, end).unwrap_or_default().iter().flat_map(|run| run_pvis(run)).collect()
}

fn run_pvis(segments: &[Segment]) -> Vec<Pvi> {
    let Some(first) = segments.first() else {
        return Vec::new();
    };
    let at = |point: Vector2| Pvi { point, curve_length: 0.0 };
    let mut out = vec![at(first.start)];
    for (i, s) in segments.iter().enumerate() {
        match s.shape {
            Shape::Line { .. } => {
                // Two tangents meeting without a curve.
                if segments.get(i + 1).is_some_and(|n| matches!(n.shape, Shape::Line { .. })) {
                    out.push(at(s.end));
                }
            }
            Shape::Placed(p) => out.push(Pvi {
                point: p.pvi,
                curve_length: s.end.x - s.start.x,
            }),
            shape => {
                // A curve on its own: its PVI where its end grades cross.
                let (g1, g2) = (shape.grade_at(s.start.x), shape.grade_at(s.end.x));
                let x = if (g1 - g2).abs() > 1e-12 {
                    (s.end.y - s.start.y + g1 * s.start.x - g2 * s.end.x) / (g1 - g2)
                } else {
                    (s.start.x + s.end.x) / 2.0
                };
                out.push(Pvi {
                    point: Vector2::new(x, s.start.y + g1 * (x - s.start.x)),
                    curve_length: s.end.x - s.start.x,
                });
            }
        }
    }
    if let Some(last) = segments.last() {
        out.push(at(last.end));
    }
    out
}

/// Whether a profile piece is a tangent or a vertical curve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ProfilePiece {
    Tangent,
    /// Symmetric parabola.
    Parabola,
    /// Asymmetric parabola: two parabolas meeting at the PVI.
    AsymmetricParabola,
    /// Circular curve.
    Circular,
}

/// Station and elevation range of a profile view and how they map to the
/// drawing.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProfileViewFrame {
    /// Drawing position of (`station_min`, `elevation_min`).
    pub origin: Vector2,
    pub station_min: f64,
    pub station_max: f64,
    pub elevation_min: f64,
    pub elevation_max: f64,
    /// Drawing units per elevation unit.
    pub vertical_scale: f64,
}

impl ProfileViewFrame {
    /// Drawing position of a station and elevation.
    pub fn to_drawing(&self, station: f64, elevation: f64) -> Vector2 {
        Vector2::new(
            self.origin.x + station - self.station_min,
            self.origin.y + (elevation - self.elevation_min) * self.vertical_scale,
        )
    }
}

/// A profile view's frame, read field by field:
///
/// ```text
/// BL 80, 2 bits, BL, BL, BL version, then version 5: BL key count, keys
/// (see alignment::key), BL 0, 121 bits; version 4: BL 0, 128 bits;
/// 3BD origin, 9 BD (rotation), BD,
/// RC n, n x 5 bits, BL m, m x record, 1 bit,
/// 3BD origin, BL, BL, 3 x 3BD range (min, min, max), 3 x 3BD (a copy),
/// BL, BL byte count + UTF-16 title, BL 0, 7 x BD, 6 x BL,
/// 3BD origin, 4 x BD, BD vertical scale
/// record: BL 0, BL, BD, BL, BD, 258 bits, BL n, n x BL, BL n, n x BL,
///     BL index (1, 2, …), BD, BD
/// ```
// ponytail: the 121/128, 5-bit and 258-bit runs are skipped by length
// (zero or constant in every sample).
/// The class data graph objects (profile views, TIN surfaces) open with:
/// `BL 80, 2 bits, BL, BL, BL version`, then version 5: `BL key count`,
/// keys (see alignment::key), `BL 0`, 121 bits; version 4 (Civil 3D 2013):
/// `BL 0`, 128 bits.
pub(super) fn graph_head(bits: &mut Bits) -> Option<()> {
    (bits.bit_long()? == 80).then_some(())?;
    bits.pos += 2;
    bits.bit_long()?;
    bits.bit_long()?;
    match bits.bit_long()? {
        5 => {
            for _ in 0..bits.bit_long()? {
                super::alignment::key(bits)?;
            }
            (bits.bit_long()? == 0).then_some(())?;
            bits.pos += 121;
        }
        // Civil 3D 2013: no keys.
        4 => {
            (bits.bit_long()? == 0).then_some(())?;
            bits.pos += 128;
        }
        _ => return None,
    }
    Some(())
}

fn view_frame_fields(data: &[u8], start: usize, end: usize) -> Option<ProfileViewFrame> {
    let mut bits = Bits::new(data, start, end);
    graph_head(&mut bits)?;
    let point = |bits: &mut Bits| -> Option<Vector2> {
        let p = Vector2::new(bits.bit_double()?, bits.bit_double()?);
        (p.x.is_finite() && p.y.is_finite() && bits.bit_double()? == 0.0).then_some(p)
    };
    let origin = point(&mut bits)?;
    for _ in 0..10 {
        bits.bit_double()?;
    }
    let items = usize::from(bits.bits_value(8)?);
    bits.pos += 5 * items;
    for _ in 0..bits.bit_long()? {
        (bits.bit_long()? == 0).then_some(())?;
        bits.bit_long()?;
        bits.bit_double()?;
        bits.bit_long()?;
        bits.bit_double()?;
        bits.pos += 258;
        for _ in 0..2 {
            for _ in 0..bits.bit_long()? {
                bits.bit_long()?;
            }
        }
        bits.bit_long()?;
        bits.bit_double()?;
        bits.bit_double()?;
    }
    bits.pos += 1;
    (point(&mut bits)? == origin).then_some(())?;
    bits.bit_long()?;
    bits.bit_long()?;
    let (a, b, c) = (point(&mut bits)?, point(&mut bits)?, point(&mut bits)?);
    (a == b && c.x > a.x && c.y > a.y).then_some(())?;
    for _ in 0..3 {
        point(&mut bits)?;
    }
    bits.bit_long()?;
    let title = usize::try_from(bits.bit_long()?).ok()?;
    bits.pos += 8 * title;
    (bits.bit_long()? == 0).then_some(())?;
    for _ in 0..7 {
        bits.bit_double()?;
    }
    for _ in 0..6 {
        bits.bit_long()?;
    }
    (point(&mut bits)? == origin).then_some(())?;
    for _ in 0..4 {
        bits.bit_double()?;
    }
    let vertical_scale = bits.bit_double().filter(|v| v.is_finite() && *v > 0.0)?;
    Some(ProfileViewFrame {
        origin,
        station_min: a.x,
        station_max: c.x,
        elevation_min: a.y,
        elevation_max: c.y,
        vertical_scale,
    })
}

/// Frame from the body: [`view_frame_fields`].
pub(super) fn view_frame_stored(data: &[u8], start: usize, end: usize) -> Option<ProfileViewFrame> {
    view_frame_fields(data, start, end)
}

/// Profile Locking settings of a profile, from the
/// `AECC_NOTIFICATIONINFOS` object it owns.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProfileLocking {
    /// Anchor profile geometry points to alignment geometry points (false:
    /// anchor the profile to the alignment's start).
    pub anchor_to_geometry: bool,
    /// Modify affected entities (false: delete them).
    pub modify_affected: bool,
    /// Notify which entities are affected.
    pub notify: bool,
    /// Ids of the profile entities waiting to be reported as affected by
    /// an alignment edit.
    pub affected: Vec<i32>,
}

/// After the common object data: 18 bits (the same in every sample),
/// `BL n, n x BL entity id, B anchor, B modify, B notify`.
pub(super) fn locking(data: &[u8], start: usize, end: usize) -> Option<ProfileLocking> {
    let mut bits = Bits::new(data, start + 18, end);
    let n = bits.bit_long()?;
    if !(0..10_000).contains(&n) {
        return None;
    }
    let affected = (0..n).map(|_| bits.bit_long()).collect::<Option<Vec<_>>>()?;
    Some(ProfileLocking {
        anchor_to_geometry: bits.bit()?,
        modify_affected: bits.bit()?,
        notify: bits.bit()?,
        affected,
    })
}

/// Design Criteria check boxes of a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProfileCriteria {
    pub criteria_based_design: bool,
    pub design_criteria_file: bool,
    pub design_check_set: bool,
}

/// The last three of the 10 criteria bits before the entity list (see
/// [`section`]).
pub(super) fn criteria(data: &[u8], start: usize, end: usize) -> Option<ProfileCriteria> {
    let (at, _) = section(data, start, end)?;
    let mut bits = Bits::new(data, at + 7, end);
    Some(ProfileCriteria {
        criteria_based_design: bits.bit()?,
        design_criteria_file: bits.bit()?,
        design_check_set: bits.bit()?,
    })
}

/// The minimum K values design criteria give a parabolic vertical curve.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CurveMinimumK {
    /// The curve entity's id.
    pub id: i32,
    /// Crest or sag, where the record says (a floating curve does not).
    pub crest: Option<bool>,
    /// Stopping sight distance; `None` without design criteria.
    pub stopping_sight: Option<f64>,
    /// Passing sight distance.
    pub passing_sight: Option<f64>,
    /// Most likely the headlight sight distance.
    pub headlight_sight: Option<f64>,
}

/// The minimum K values of every parabola in the entity list (see
/// [`record_list_at`]).
pub(super) fn curve_minimum_k(data: &[u8], start: usize, end: usize) -> Vec<CurveMinimumK> {
    chain(data, start, end)
        .iter()
        .filter_map(|r| {
            let [stopping_sight, passing_sight, headlight_sight] = r.minimum_k?;
            Some(CurveMinimumK {
                id: r.id,
                crest: r.crest,
                stopping_sight,
                passing_sight,
                headlight_sight,
            })
        })
        .collect()
}
