//! Horizontal alignment geometry.
//!
//! An alignment body stores design elements and their constraints, not the
//! solved geometry. Each element record opens with
//!
//! ```text
//! BL type (257 line, 258 arc, 259 spiral, 260 spiral-curve-spiral,
//!          261 spiral-line-spiral, 266 spiral-spiral-curve, 269 two
//!          spiral-curve-spirals)
//! BS 2, B
//! line:  BS constraint
//! other: B, BD design speed, BD, BS n, n x BD, BL constraint (none for
//!        269), B (arcs only)
//! BS id, BS previous id, BS next id
//! ```
//!
//! followed by the constraint's parameters:
//!
//! | type/constraint | parameters | geometry |
//! |---|---|---|
//! | 257/0 | 2RD start, 2RD end | fixed line |
//! | 257/5 | BD length | line continuing the previous element |
//! | 258/2 | 2RD start, mid, end | fixed arc |
//! | 258/21 | BD radius, BD length, B clockwise | arc continuing the previous element |
//! | 258/29 | BD radius | arc fitted between two lines |
//! | 259/0 | BL, B cw, 2RD start, 2RD direction, BD length, BD radius | spiral from a stored point |
//! | 259/1 | BL, B cw, BD radius, BD length | spiral from the previous curvature to zero |
//! | 260/0 | BL, BD spiral in, BD radius, BD spiral out | spiral-curve-spiral fitted between two lines |
//! | 260/6 | BL, BD spiral, BD radius, BD arc length, B cw | spiral from the previous curvature, then arc |
//! | 260/7 | BL, BD radius, BD arc length, BD spiral, B cw | arc then spiral ending at the next line's first point |
//! | 260/14 | — | spiral from the previous curvature to zero, tangent to the next line |
//! | 261/5 | — | spiral-line-spiral fitted between the previous and next curves |
//! | 266/1 | BL, BD spiral out, BD spiral in, BD radius, 2RD end | spiral to zero, reverse spiral, arc ending at the stored point |
//! | 269 | BL, BS, BD, 3 x BD, 3 x BD, BD first arc | reverse spiral-curve-spirals from the previous line to the next |
//!
//! Turn flags are 0 for left, 1 for right; where a record has none the turn
//! follows from the geometry that has to be met.

use super::bits::Bits;
use crate::types::Vector2;
use std::f64::consts::{PI, TAU};

/// One design element of an alignment.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AlignmentElement {
    pub id: i16,
    /// Previous element id, 0 at the start.
    pub previous: i16,
    /// Next element id, 0 at the end.
    pub next: i16,
    pub kind: AlignmentElementKind,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AlignmentElementKind {
    /// Line through two points.
    FixedLine { start: Vector2, end: Vector2 },
    /// Line continuing the previous element.
    FloatingLine { length: f64 },
    /// Arc through start, mid, and end points.
    FixedArc { start: Vector2, mid: Vector2, end: Vector2 },
    /// Arc continuing the previous element.
    FloatingArc { radius: f64, length: f64, clockwise: bool },
    /// Arc fitted between the previous and next lines.
    FreeArc { radius: f64 },
    /// Spiral from a stored point and direction, curvature rising to `1/radius`.
    SpiralIn {
        start: Vector2,
        direction: Vector2,
        length: f64,
        radius: f64,
        clockwise: bool,
    },
    /// Spiral continuing the previous element, curvature falling to zero.
    SpiralOut { radius: f64, length: f64, clockwise: bool },
    /// Spiral-curve-spiral fitted between the previous and next lines.
    FreeSpiralCurveSpiral {
        spiral_in: f64,
        radius: f64,
        spiral_out: f64,
    },
    /// Spiral from the previous curvature to `1/radius`, then an arc.
    FloatingSpiralCurve {
        spiral: f64,
        radius: f64,
        arc_length: f64,
        clockwise: bool,
    },
    /// Arc then spiral to zero curvature, ending at the next line's first point.
    FloatingCurveSpiral {
        radius: f64,
        arc_length: f64,
        spiral: f64,
        clockwise: bool,
    },
    /// Spiral from the previous curvature to zero, tangent to the next line.
    FreeSpiral,
    /// Spiral-line-spiral joining the previous and next curves.
    FreeSpiralLineSpiral,
    /// Spiral to zero, reverse spiral to `1/radius`, then an arc ending at `end`.
    FloatingSpiralSpiralCurve {
        spiral_out: f64,
        spiral_in: f64,
        radius: f64,
        end: Vector2,
    },
    /// Spiral-curve-spiral and a reverse spiral-curve-spiral from the
    /// previous line's end to the next line; the second arc takes up the
    /// rest of the turn.
    FloatingReverseCurves {
        first: [f64; 3],
        second: [f64; 3],
        first_arc: f64,
    },
    /// Element kind or constraint this crate does not decode.
    Other { element_type: i32, constraint: Option<i32> },
}

/// Design elements found in a main stream, keyed by their own ids.
pub(super) fn elements(data: &[u8], start: usize, end: usize) -> Vec<AlignmentElement> {
    let mut out: Vec<AlignmentElement> = Vec::new();
    for pos in start..end {
        if !is_header(data, pos, end) {
            continue;
        }
        if let Some(element) = read_element(&mut Bits::new(data, pos, end)) {
            if element.id > 0 && !out.iter().any(|e| e.id == element.id) {
                out.push(element);
            }
        }
    }
    out
}

/// `BL type` (raw, 257..=280) followed by `BS 2`.
fn is_header(data: &[u8], pos: usize, end: usize) -> bool {
    let mut bits = Bits::new(data, pos, end);
    let Some(element_type) = raw_bit_long(&mut bits) else {
        return false;
    };
    (257..=280).contains(&element_type) && bits.compact_short() == Some(2)
}

fn raw_bit_long(bits: &mut Bits) -> Option<i32> {
    // Only the full-width BL form: a type written compactly is not a header.
    (bits.bit()? == false && bits.bit()? == false).then_some(())?;
    bits.raw_long()
}

fn read_element(bits: &mut Bits) -> Option<AlignmentElement> {
    let element_type = raw_bit_long(bits)?;
    bits.bit_short()?;
    let constraint = if element_type == 257 {
        bits.bit()?;
        Some(i32::from(bits.bit_short()?))
    } else {
        bits.bit()?;
        bits.bit()?;
        bits.bit_double()?;
        bits.bit_double()?;
        let count = bits.bit_short()?;
        if !(0..=64).contains(&count) {
            return None;
        }
        for _ in 0..count {
            bits.bit_double()?;
        }
        // The low word is the constraint; the high word carries flags.
        let constraint = (element_type != 269)
            .then(|| bits.bit_long())
            .flatten()
            .map(|c| c & 0xFFFF);
        if element_type == 258 {
            bits.bit()?;
        }
        constraint
    };
    let id = bits.bit_short()?;
    let previous = bits.bit_short()?;
    let next = bits.bit_short()?;
    let point = |bits: &mut Bits| Some(Vector2::new(bits.raw_double()?, bits.raw_double()?));
    let kind = match (element_type, constraint) {
        (257, Some(0)) => AlignmentElementKind::FixedLine {
            start: point(bits)?,
            end: point(bits)?,
        },
        (257, Some(5)) => AlignmentElementKind::FloatingLine {
            length: bits.bit_double()?,
        },
        (258, Some(2)) => AlignmentElementKind::FixedArc {
            start: point(bits)?,
            mid: point(bits)?,
            end: point(bits)?,
        },
        (258, Some(21)) => AlignmentElementKind::FloatingArc {
            radius: bits.bit_double()?,
            length: bits.bit_double()?,
            clockwise: bits.bit()?,
        },
        (258, Some(29)) => AlignmentElementKind::FreeArc {
            radius: bits.bit_double()?,
        },
        (259, Some(code @ (0 | 1))) => {
            bits.bit_long()?;
            let clockwise = bits.bit()?;
            if code == 0 {
                AlignmentElementKind::SpiralIn {
                    start: point(bits)?,
                    direction: point(bits)?,
                    length: bits.bit_double()?,
                    radius: bits.bit_double()?,
                    clockwise,
                }
            } else {
                AlignmentElementKind::SpiralOut {
                    radius: bits.bit_double()?,
                    length: bits.bit_double()?,
                    clockwise,
                }
            }
        }
        (260, Some(0)) => {
            bits.bit_long()?;
            AlignmentElementKind::FreeSpiralCurveSpiral {
                spiral_in: bits.bit_double()?,
                radius: bits.bit_double()?,
                spiral_out: bits.bit_double()?,
            }
        }
        (260, Some(6)) => {
            bits.bit_long()?;
            AlignmentElementKind::FloatingSpiralCurve {
                spiral: bits.bit_double()?,
                radius: bits.bit_double()?,
                arc_length: bits.bit_double()?,
                clockwise: bits.bit()?,
            }
        }
        (260, Some(7)) => {
            bits.bit_long()?;
            AlignmentElementKind::FloatingCurveSpiral {
                radius: bits.bit_double()?,
                arc_length: bits.bit_double()?,
                spiral: bits.bit_double()?,
                clockwise: bits.bit()?,
            }
        }
        (260, Some(14)) => AlignmentElementKind::FreeSpiral,
        (261, Some(5)) => AlignmentElementKind::FreeSpiralLineSpiral,
        (266, Some(1)) => {
            bits.bit_long()?;
            AlignmentElementKind::FloatingSpiralSpiralCurve {
                spiral_out: bits.bit_double()?,
                spiral_in: bits.bit_double()?,
                radius: bits.bit_double()?,
                end: point(bits)?,
            }
        }
        (269, None) => {
            bits.bit_long()?;
            bits.bit_short()?;
            bits.bit_double()?;
            let mut values = [0.0; 6];
            for value in &mut values {
                *value = bits.bit_double()?;
            }
            AlignmentElementKind::FloatingReverseCurves {
                first: [values[0], values[1], values[2]],
                second: [values[3], values[4], values[5]],
                first_arc: bits.bit_double()?,
            }
        }
        _ => AlignmentElementKind::Other {
            element_type,
            constraint,
        },
    };
    let positive = |v: f64| v.is_finite() && v > 0.0;
    let valid = match &kind {
        AlignmentElementKind::FloatingLine { length } => positive(*length),
        AlignmentElementKind::FloatingArc { radius, length, .. } => {
            positive(*radius) && positive(*length)
        }
        AlignmentElementKind::FreeArc { radius } => positive(*radius),
        AlignmentElementKind::SpiralIn { length, radius, .. }
        | AlignmentElementKind::SpiralOut { radius, length, .. } => {
            positive(*radius) && positive(*length)
        }
        AlignmentElementKind::FreeSpiralCurveSpiral {
            spiral_in,
            radius,
            spiral_out,
        } => positive(*radius) && *spiral_in >= 0.0 && *spiral_out >= 0.0,
        AlignmentElementKind::FloatingSpiralCurve {
            spiral,
            radius,
            arc_length,
            ..
        }
        | AlignmentElementKind::FloatingCurveSpiral {
            radius,
            arc_length,
            spiral,
            ..
        } => positive(*radius) && *spiral >= 0.0 && *arc_length >= 0.0,
        AlignmentElementKind::FloatingSpiralSpiralCurve {
            spiral_out,
            spiral_in,
            radius,
            end,
        } => {
            positive(*radius)
                && *spiral_out >= 0.0
                && *spiral_in >= 0.0
                && end.x.is_finite()
                && end.y.is_finite()
        }
        AlignmentElementKind::FloatingReverseCurves {
            first,
            second,
            first_arc,
        } => positive(first[1]) && positive(second[1]) && *first_arc >= 0.0,
        _ => true,
    };
    valid.then_some(AlignmentElement {
        id,
        previous,
        next,
        kind,
    })
}

/// Elements in station order, one run per start element.
fn chains(elements: &[AlignmentElement]) -> Vec<Vec<&AlignmentElement>> {
    let by_id = |id: i16| elements.iter().find(|e| e.id == id);
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    // A run also starts after a previous element that was not decoded.
    let starts = elements
        .iter()
        .filter(|e| e.previous == 0 || by_id(e.previous).is_none());
    for first in starts {
        let mut run = Vec::new();
        let mut current = Some(first);
        while let Some(element) = current {
            if !seen.insert(element.id) {
                break;
            }
            run.push(element);
            current = by_id(element.next);
        }
        out.push(run);
    }
    out
}

const STEP: f64 = 1.0;
/// How far a solved element may miss the geometry it has to meet.
const TOLERANCE: f64 = 0.01;

fn cross(a: Vector2, b: Vector2) -> f64 {
    a.x * b.y - a.y * b.x
}

fn unit(v: Vector2) -> Option<Vector2> {
    let length = v.x.hypot(v.y);
    (length > 1e-12).then(|| Vector2::new(v.x / length, v.y / length))
}

fn heading(v: Vector2) -> f64 {
    v.y.atan2(v.x)
}

fn direction(h: f64) -> Vector2 {
    Vector2::new(h.cos(), h.sin())
}

fn normalize(angle: f64) -> f64 {
    (angle + PI).rem_euclid(TAU) - PI
}

fn intersect(p: Vector2, u: Vector2, q: Vector2, v: Vector2) -> Option<Vector2> {
    let den = cross(u, v);
    if den.abs() < 1e-12 {
        return None;
    }
    let t = cross(q - p, v) / den;
    Some(p + u * t)
}

/// Points along a curve whose curvature changes linearly from `k0` to `k1`.
fn walk(start: Vector2, h: f64, k0: f64, k1: f64, length: f64) -> (Vec<Vector2>, f64) {
    let n = ((length / STEP).ceil() as usize).clamp(2, 20000);
    let ds = length / n as f64;
    let mut points = Vec::with_capacity(n + 1);
    points.push(start);
    let mut p = start;
    for i in 0..n {
        let s = (i as f64 + 0.5) * ds;
        let angle = h + k0 * s + (k1 - k0) * s * s / (2.0 * length);
        p = p + direction(angle) * ds;
        points.push(p);
    }
    (points, h + k0 * length + (k1 - k0) * length / 2.0)
}

fn curvature(radius: f64, clockwise: bool) -> f64 {
    if clockwise {
        -1.0 / radius
    } else {
        1.0 / radius
    }
}

/// Position, heading and curvature at the end of the solved geometry.
#[derive(Clone, Copy)]
struct State {
    point: Vector2,
    heading: f64,
    curvature: f64,
}

/// A line waiting for the element after it to say where it ends.
struct PendingLine {
    start: Vector2,
    direction: Vector2,
    end: Vector2,
}

/// Point and unit direction an element can aim at.
fn line_carrier(element: &AlignmentElement) -> Option<(Vector2, Vector2)> {
    match &element.kind {
        AlignmentElementKind::FixedLine { start, end } => Some((*start, unit(*end - *start)?)),
        AlignmentElementKind::SpiralIn {
            start, direction, ..
        } => Some((*start, unit(*direction)?)),
        _ => None,
    }
}

/// Pieces with curvature changing linearly, walked from `state`.
fn pieces(state: State, parts: &[(f64, f64, f64)]) -> (Vec<Vector2>, State) {
    let mut points = vec![state.point];
    let mut h = state.heading;
    let mut k = state.curvature;
    for &(k0, k1, length) in parts {
        if length > 1e-9 {
            let (q, end) = walk(*points.last().unwrap(), h, k0, k1, length);
            h = end;
            points.extend(q.into_iter().skip(1));
            k = k1;
        }
    }
    let point = *points.last().unwrap();
    (points, State { point, heading: h, curvature: k })
}

/// Solved polylines; a run breaks where an element cannot be solved.
pub(super) fn polylines(elements: &[AlignmentElement]) -> Vec<Vec<Vector2>> {
    let mut out = raw_polylines(elements);
    for points in &mut out {
        points.dedup_by(|a, b| a.distance(b) < 1e-9);
    }
    out
}

/// A point where one kind of piece hands over to another.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GeometryPoint {
    pub point: Vector2,
    /// Distance from the start of the first solved run.
    pub distance: f64,
    /// `POB`, `TS`, `SC`, `CS`, `ST`, `PC`, `PT`, `PCC`, `SS`, `POE`.
    pub name: &'static str,
}

#[derive(Clone, Copy, PartialEq)]
enum Piece {
    Line,
    Curve,
    Spiral,
}

/// Signed curvature at the middle vertex of three.
fn curvature_at(a: Vector2, b: Vector2, c: Vector2) -> f64 {
    let (ab, bc, ca) = (a.distance(&b), b.distance(&c), c.distance(&a));
    let area2 = cross(b - a, c - a);
    if ab * bc * ca < 1e-12 {
        0.0
    } else {
        2.0 * area2 / (ab * bc * ca)
    }
}

fn piece_kind(points: &[Vector2]) -> Piece {
    let n = points.len();
    if n < 4 {
        return Piece::Line;
    }
    let k0 = curvature_at(points[0], points[1], points[2]);
    let k1 = curvature_at(points[n - 3], points[n - 2], points[n - 1]);
    if k0.abs() < 1e-6 && k1.abs() < 1e-6 {
        Piece::Line
    } else if (k0 - k1).abs() <= 0.02 * k0.abs().max(k1.abs()) {
        Piece::Curve
    } else {
        Piece::Spiral
    }
}

/// Geometry points along the first solved run.
pub(super) fn geometry_points(elements: &[AlignmentElement]) -> Vec<GeometryPoint> {
    let Some(raw) = raw_polylines(elements).into_iter().next() else {
        return Vec::new();
    };
    // Every solved piece repeats its first point at the join.
    let mut joins = vec![0];
    for i in 1..raw.len() {
        if raw[i].distance(&raw[i - 1]) < 1e-9 {
            joins.push(i);
        }
    }
    joins.push(raw.len());
    let pieces: Vec<(usize, usize, Piece)> = joins
        .windows(2)
        .filter(|w| w[1] > w[0] + 1)
        .map(|w| (w[0], w[1], piece_kind(&raw[w[0]..w[1]])))
        .collect();
    let mut distance_to = vec![0.0; raw.len()];
    for i in 1..raw.len() {
        distance_to[i] = distance_to[i - 1] + raw[i].distance(&raw[i - 1]);
    }
    let mut out = Vec::new();
    let mut push = |index: usize, name| {
        let index = index.min(raw.len() - 1);
        out.push(GeometryPoint {
            point: raw[index],
            distance: distance_to[index],
            name,
        })
    };
    if let Some(first) = pieces.first() {
        push(first.0, "POB");
    }
    for pair in pieces.windows(2) {
        let name = match (pair[0].2, pair[1].2) {
            (Piece::Line, Piece::Spiral) => "TS",
            (Piece::Spiral, Piece::Curve) => "SC",
            (Piece::Curve, Piece::Spiral) => "CS",
            (Piece::Spiral, Piece::Line) => "ST",
            (Piece::Line, Piece::Curve) => "PC",
            (Piece::Curve, Piece::Line) => "PT",
            (Piece::Curve, Piece::Curve) => "PCC",
            (Piece::Spiral, Piece::Spiral) => "SS",
            (Piece::Line, Piece::Line) => continue,
        };
        push(pair[1].0, name);
    }
    if let Some(last) = pieces.last() {
        push(last.1 - 1, "POE");
    }
    out
}

fn raw_polylines(elements: &[AlignmentElement]) -> Vec<Vec<Vector2>> {
    let mut out = Vec::new();
    for run in chains(elements) {
        solve_run(&run, &mut out);
    }
    out.retain(|points| points.len() > 1);
    out
}

/// Solve one chain of elements. Where an element cannot be solved exactly
/// the polyline ends and a new one starts at the next element that carries
/// its own position.
fn solve_run(run: &[&AlignmentElement], out: &mut Vec<Vec<Vector2>>) {
    let mut points: Vec<Vector2> = Vec::new();
    let mut state: Option<State> = None;
    let mut pending: Option<PendingLine> = None;
    let mut index = 0;
    while index < run.len() {
        let element = run[index];
        let next = run.get(index + 1).copied();
        index += 1;
        // Floating elements attach to a fixed line at its second point.
        let attach = |points: &mut Vec<Vector2>,
                      pending: &mut Option<PendingLine>,
                      state: &mut Option<State>| {
            if let Some(line) = pending.take() {
                points.push(line.start);
                points.push(line.end);
                *state = Some(State {
                    point: line.end,
                    heading: heading(line.direction),
                    curvature: 0.0,
                });
            }
        };
        let solved: Option<(Vec<Vector2>, State)> = match &element.kind {
            AlignmentElementKind::FixedLine { start, end } => {
                let Some(u) = unit(*end - *start) else {
                    continue;
                };
                if let Some(line) = pending.take() {
                    let joint = intersect(line.start, line.direction, *start, u)
                        .filter(|x| x.distance(&line.end) < 1e4)
                        .unwrap_or(line.end);
                    points.push(line.start);
                    points.push(joint);
                    state = Some(State {
                        point: joint,
                        heading: heading(u),
                        curvature: 0.0,
                    });
                }
                let from = state.map_or(*start, |s| s.point);
                pending = Some(PendingLine {
                    start: from,
                    direction: u,
                    end: *end,
                });
                state = Some(State {
                    point: from,
                    heading: heading(u),
                    curvature: 0.0,
                });
                if next.is_none() {
                    points.push(from);
                    points.push(*end);
                    pending = None;
                }
                continue;
            }
            AlignmentElementKind::FloatingLine { length } => {
                attach(&mut points, &mut pending, &mut state);
                state.map(|s| pieces(s, &[(0.0, 0.0, *length)]))
            }
            AlignmentElementKind::FloatingArc {
                radius,
                length,
                clockwise,
            } => {
                attach(&mut points, &mut pending, &mut state);
                let k = curvature(*radius, *clockwise);
                state.map(|s| pieces(s, &[(k, k, *length)]))
            }
            AlignmentElementKind::SpiralOut {
                radius,
                length,
                clockwise,
            } => {
                attach(&mut points, &mut pending, &mut state);
                let k = curvature(*radius, *clockwise);
                state.map(|s| pieces(s, &[(k, 0.0, *length)]))
            }
            AlignmentElementKind::SpiralIn {
                start,
                direction: d,
                length,
                radius,
                clockwise,
            } => {
                close(&mut points, &mut pending, *start);
                let begin = State {
                    point: *start,
                    heading: heading(*d),
                    curvature: 0.0,
                };
                Some(pieces(begin, &[(0.0, curvature(*radius, *clockwise), *length)]))
            }
            AlignmentElementKind::FixedArc { start, mid, end } => {
                close(&mut points, &mut pending, *start);
                three_point_arc(*start, *mid, *end)
            }
            AlignmentElementKind::FreeArc { radius } => {
                fitted_curve(&mut points, &mut pending, state, next, 0.0, *radius, 0.0)
            }
            AlignmentElementKind::FreeSpiralCurveSpiral {
                spiral_in,
                radius,
                spiral_out,
            } => fitted_curve(
                &mut points,
                &mut pending,
                state,
                next,
                *spiral_in,
                *radius,
                *spiral_out,
            ),
            AlignmentElementKind::FloatingSpiralCurve {
                spiral,
                radius,
                arc_length,
                clockwise,
            } => {
                attach(&mut points, &mut pending, &mut state);
                let k = curvature(*radius, *clockwise);
                state.map(|s| pieces(s, &[(s.curvature, k, *spiral), (k, k, *arc_length)]))
            }
            AlignmentElementKind::FloatingCurveSpiral { .. } => {
                attach(&mut points, &mut pending, &mut state);
                curve_spiral_to_next(element, next)
                    .filter(|(_, start, _)| {
                        state.is_some_and(|s| s.point.distance(&start.point) < TOLERANCE)
                    })
                    .map(|(q, _, end)| (q, end))
            }
            AlignmentElementKind::FreeSpiral => {
                attach(&mut points, &mut pending, &mut state);
                state.zip(next.and_then(line_carrier)).and_then(|(s, (b, v))| {
                    if s.curvature.abs() < 1e-12 {
                        return None;
                    }
                    let length = 2.0 * normalize(heading(v) - s.heading) / s.curvature;
                    (length > 0.0)
                        .then(|| pieces(s, &[(s.curvature, 0.0, length)]))
                        .filter(|(_, end)| cross(end.point - b, v).abs() < TOLERANCE)
                })
            }
            AlignmentElementKind::FreeSpiralLineSpiral => {
                attach(&mut points, &mut pending, &mut state);
                // What the spiral-line-spiral has to arrive at: the start of a
                // curve-spiral built back from the line after it (solved
                // together with it), or the start of a fixed arc.
                let target = next.and_then(|n| match &n.kind {
                    AlignmentElementKind::FloatingCurveSpiral { .. } => {
                        let after = run.get(index + 1).copied();
                        curve_spiral_to_next(n, after).map(|(q, start, end)| (start, Some((q, end))))
                    }
                    AlignmentElementKind::FixedArc { start, mid, end } => {
                        Some((arc_start_state(*start, *mid, *end), None))
                    }
                    _ => None,
                });
                match (state, target) {
                    (Some(s), Some((begin, then))) => {
                        spiral_line_spiral(s, begin).map(|(mut sls, sls_end)| match then {
                            Some((q, end)) => {
                                index += 1;
                                sls.extend(q.into_iter().skip(1));
                                (sls, end)
                            }
                            None => (sls, sls_end),
                        })
                    }
                    _ => None,
                }
            }
            AlignmentElementKind::FloatingSpiralSpiralCurve {
                spiral_out,
                spiral_in,
                radius,
                end,
            } => {
                attach(&mut points, &mut pending, &mut state);
                state.and_then(|s| {
                    let (out_points, at_zero) = pieces(s, &[(s.curvature, 0.0, *spiral_out)]);
                    // The reverse curve's turn is the one that reaches `end`.
                    [1.0, -1.0]
                        .into_iter()
                        .filter_map(|sign| {
                            let k = sign / radius;
                            let (in_points, at_arc) = pieces(at_zero, &[(0.0, k, *spiral_in)]);
                            let (arc, arc_end, miss) = arc_to_point(at_arc, *end)?;
                            let mut all = out_points.clone();
                            all.extend(in_points.into_iter().skip(1));
                            all.extend(arc.into_iter().skip(1));
                            Some((miss, all, arc_end))
                        })
                        .min_by(|a, b| a.0.total_cmp(&b.0))
                        .filter(|(miss, _, _)| *miss < TOLERANCE)
                        .map(|(_, all, end)| (all, end))
                })
            }
            AlignmentElementKind::FloatingReverseCurves {
                first,
                second,
                first_arc,
            } => {
                attach(&mut points, &mut pending, &mut state);
                state
                    .zip(next.and_then(line_carrier))
                    .and_then(|(s, carrier)| reverse_curves(s, carrier, *first, *second, *first_arc))
            }
            AlignmentElementKind::Other { .. } => None,
        };
        match solved {
            Some((q, end)) => {
                points.extend(q);
                state = Some(end);
            }
            None => {
                // Not solvable here: end this polyline, restart at the next
                // element that carries its own position.
                if let Some(line) = pending.take() {
                    points.push(line.start);
                    points.push(line.end);
                }
                out.push(std::mem::take(&mut points));
                state = None;
            }
        }
    }
    if let Some(line) = pending.take() {
        points.push(line.start);
        points.push(line.end);
    }
    out.push(points);
}

fn close(points: &mut Vec<Vector2>, pending: &mut Option<PendingLine>, end: Vector2) {
    if let Some(line) = pending.take() {
        points.push(line.start);
        points.push(end);
    }
}

fn circle(a: Vector2, b: Vector2, c: Vector2) -> Option<(Vector2, f64, bool)> {
    let d = 2.0 * (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y));
    if d.abs() < 1e-12 {
        return None;
    }
    let sq = |p: Vector2| p.x * p.x + p.y * p.y;
    let center = Vector2::new(
        (sq(a) * (b.y - c.y) + sq(b) * (c.y - a.y) + sq(c) * (a.y - b.y)) / d,
        (sq(a) * (c.x - b.x) + sq(b) * (a.x - c.x) + sq(c) * (b.x - a.x)) / d,
    );
    Some((center, a.distance(&center), cross(b - a, c - b) > 0.0))
}

fn arc_start_state(a: Vector2, b: Vector2, c: Vector2) -> State {
    let (center, radius, ccw) = circle(a, b, c).unwrap_or((a, 1.0, true));
    let angle = (a.y - center.y).atan2(a.x - center.x);
    State {
        point: a,
        heading: angle + if ccw { PI / 2.0 } else { -PI / 2.0 },
        curvature: if ccw { 1.0 / radius } else { -1.0 / radius },
    }
}

fn three_point_arc(a: Vector2, b: Vector2, c: Vector2) -> Option<(Vec<Vector2>, State)> {
    let (center, radius, ccw) = circle(a, b, c)?;
    let a0 = (a.y - center.y).atan2(a.x - center.x);
    let a1 = (c.y - center.y).atan2(c.x - center.x);
    let sweep = if ccw {
        (a1 - a0).rem_euclid(TAU)
    } else {
        -(a0 - a1).rem_euclid(TAU)
    };
    let n = ((sweep.abs() * radius / STEP).ceil() as usize).clamp(2, 20000);
    let points: Vec<Vector2> = (0..=n)
        .map(|j| {
            let angle = a0 + sweep * j as f64 / n as f64;
            center + direction(angle) * radius
        })
        .collect();
    let end = State {
        point: *points.last().unwrap(),
        heading: a0 + sweep + if ccw { PI / 2.0 } else { -PI / 2.0 },
        curvature: if ccw { 1.0 / radius } else { -1.0 / radius },
    };
    Some((points, end))
}

/// Arc continuing `state` to `end`, with how far `end` lies off its circle.
fn arc_to_point(state: State, end: Vector2) -> Option<(Vec<Vector2>, State, f64)> {
    let k = state.curvature;
    if k.abs() < 1e-12 {
        return None;
    }
    let radius = 1.0 / k.abs();
    let normal = direction(state.heading + if k > 0.0 { PI / 2.0 } else { -PI / 2.0 });
    let center = state.point + normal * radius;
    let miss = (end.distance(&center) - radius).abs();
    let a0 = (state.point.y - center.y).atan2(state.point.x - center.x);
    let a1 = (end.y - center.y).atan2(end.x - center.x);
    let sweep = if k > 0.0 {
        (a1 - a0).rem_euclid(TAU)
    } else {
        (a0 - a1).rem_euclid(TAU)
    };
    let (points, end_state) = pieces(state, &[(k, k, sweep * radius)]);
    Some((points, end_state, miss))
}

/// Backward construction of a curve-spiral ending at the first point of the
/// element after it; returns its forward points, start state and end state.
fn curve_spiral_to_next(
    element: &AlignmentElement,
    after: Option<&AlignmentElement>,
) -> Option<(Vec<Vector2>, State, State)> {
    let AlignmentElementKind::FloatingCurveSpiral {
        radius,
        arc_length,
        spiral,
        clockwise,
    } = &element.kind
    else {
        return None;
    };
    let (anchor, v) = line_carrier(after?)?;
    let k = curvature(*radius, *clockwise);
    // Walking backwards mirrors the turn.
    let back = State {
        point: anchor,
        heading: heading(v) + PI,
        curvature: 0.0,
    };
    let (mut q, back_end) = pieces(back, &[(0.0, -k, *spiral), (-k, -k, *arc_length)]);
    q.reverse();
    let start = State {
        point: back_end.point,
        heading: back_end.heading + PI,
        curvature: k,
    };
    let end = State {
        point: anchor,
        heading: heading(v),
        curvature: 0.0,
    };
    Some((q, start, end))
}

/// Spiral-line-spiral from `from` (curvature to zero), a line, and a spiral
/// up to `to`'s curvature arriving at `to`.
fn spiral_line_spiral(from: State, to: State) -> Option<(Vec<Vector2>, State)> {
    let (k1, k2) = (from.curvature, to.curvature);
    if k2.abs() < 1e-12 {
        return None;
    }
    let turn = normalize(to.heading - from.heading);
    // Heading: k1 a / 2 + k2 b / 2 = turn.
    let b_of = |a: f64| (turn - k1 * a / 2.0) * 2.0 / k2;
    let end_of = |a: f64, l: f64| {
        pieces(from, &[(k1, 0.0, a), (0.0, 0.0, l), (0.0, k2, b_of(a))]).1.point
    };
    let mut best: Option<(f64, f64, f64)> = None;
    for &a0 in &[50.0, 100.0, 250.0, 500.0, 1000.0] {
        for &l0 in &[0.0, 50.0, 200.0, 1000.0] {
            let (mut a, mut l) = (a0, l0);
            for _ in 0..50 {
                let f = end_of(a, l) - to.point;
                let e = 1e-4;
                let fa = (end_of(a + e, l) - to.point - f) * (1.0 / e);
                let fl = (end_of(a, l + e) - to.point - f) * (1.0 / e);
                let det = fa.x * fl.y - fl.x * fa.y;
                if det.abs() < 1e-12 {
                    break;
                }
                let da = (fl.y * f.x - fl.x * f.y) / det;
                let dl = (-fa.y * f.x + fa.x * f.y) / det;
                a -= da;
                l -= dl;
                if da.abs() + dl.abs() < 1e-9 {
                    break;
                }
            }
            let miss = end_of(a, l).distance(&to.point);
            let b = b_of(a);
            if a >= -1e-6 && l >= -1e-6 && b >= -1e-6 && miss < TOLERANCE
                && best.map_or(true, |(_, _, m)| miss < m)
            {
                best = Some((a.max(0.0), l.max(0.0), miss));
            }
        }
    }
    let (a, l, _) = best?;
    Some(pieces(from, &[(k1, 0.0, a), (0.0, 0.0, l), (0.0, k2, b_of(a))]))
}

/// Spiral-curve-spiral fitted between the pending line (or the solved end)
/// and the next element's line.
fn fitted_curve(
    points: &mut Vec<Vector2>,
    pending: &mut Option<PendingLine>,
    state: Option<State>,
    next: Option<&AlignmentElement>,
    spiral_in: f64,
    radius: f64,
    spiral_out: f64,
) -> Option<(Vec<Vector2>, State)> {
    let (b, v) = line_carrier(next?)?;
    if let Some(line) = pending.as_ref() {
        // Between two lines: slide along the first until the curve ends on
        // the second.
        let u = line.direction;
        let pi = intersect(line.start, u, b, v)?;
        let delta = normalize(heading(v) - heading(u));
        let k = delta.signum() / radius;
        let arc = radius * (delta.abs() - (spiral_in + spiral_out) / (2.0 * radius));
        if arc < -1e-9 {
            return None;
        }
        let origin = State {
            point: Vector2::new(0.0, 0.0),
            heading: heading(u),
            curvature: 0.0,
        };
        let (q, end) = pieces(origin, &[(0.0, k, spiral_in), (k, k, arc.max(0.0)), (k, 0.0, spiral_out)]);
        let slide = cross(pi + end.point - b, v) / cross(u, v);
        let ts = pi - u * slide;
        let line = pending.take()?;
        points.push(line.start);
        points.push(ts);
        let q: Vec<Vector2> = q.into_iter().map(|p| ts + p).collect();
        let end = State {
            point: *q.last().unwrap(),
            ..end
        };
        return Some((q, end));
    }
    // After a curve: compound spiral from its curvature, arc, spiral out;
    // the arc takes up the remaining turn and must end on the line.
    let s = state?;
    let turn = normalize(heading(v) - s.heading);
    [1.0, -1.0]
        .into_iter()
        .filter_map(|sign| {
            let k = sign / radius;
            let spirals = (s.curvature + k) * spiral_in / 2.0 + k * spiral_out / 2.0;
            let arc = (turn - spirals) / k;
            (arc >= -1e-9).then(|| pieces(s, &[(s.curvature, k, spiral_in), (k, k, arc.max(0.0)), (k, 0.0, spiral_out)]))
        })
        .find(|(_, end)| cross(end.point - b, v).abs() < TOLERANCE)
}

/// Spiral-curve-spiral and a reverse one from `s` to the line `(b, v)`.
fn reverse_curves(
    s: State,
    (b, v): (Vector2, Vector2),
    first: [f64; 3],
    second: [f64; 3],
    first_arc: f64,
) -> Option<(Vec<Vector2>, State)> {
    let turn = normalize(heading(v) - s.heading);
    let mut best: Option<(f64, (Vec<Vector2>, State))> = None;
    for s1 in [1.0, -1.0] {
        for s2 in [1.0, -1.0] {
            let k1 = s1 / first[1];
            let k2 = s2 / second[1];
            let spirals = k1 * (first[0] + first[2]) / 2.0 + k2 * (second[0] + second[2]) / 2.0;
            let second_arc = (turn - spirals - k1 * first_arc) / k2;
            if second_arc < -1e-9 {
                continue;
            }
            let solved = pieces(
                s,
                &[
                    (0.0, k1, first[0]),
                    (k1, k1, first_arc),
                    (k1, 0.0, first[2]),
                    (0.0, k2, second[0]),
                    (k2, k2, second_arc.max(0.0)),
                    (k2, 0.0, second[2]),
                ],
            );
            let miss = cross(solved.1.point - b, v).abs();
            if miss < TOLERANCE && best.as_ref().map_or(true, |(m, _)| miss < *m) {
                best = Some((miss, solved));
            }
        }
    }
    best.map(|(_, solved)| solved)
}

/// Start station: the BD after the header's `2.25` marker, 16 bits and three
/// flags (`1 1 0`) later. Other flag layouts start at station 0.
pub(super) fn start_station(data: &[u8], start: usize, end: usize) -> f64 {
    let window = end.min(start + 4096);
    for pos in start..window {
        let mut bits = Bits::new(data, pos, end);
        if bits.bit_double() != Some(2.25) || bits.pos != pos + 66 {
            continue;
        }
        bits.pos += 16;
        let flags = (bits.bit(), bits.bit(), bits.bit());
        if flags == (Some(true), Some(true), Some(false)) {
            if let Some(station) = bits.bit_double().filter(|v| v.is_finite() && v.abs() < 1e7) {
                return station;
            }
        }
        return 0.0;
    }
    0.0
}
