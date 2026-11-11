//! Horizontal alignment geometry.
//!
//! An alignment body stores design elements and their constraints, not the
//! solved geometry. Each element record opens with
//!
//! ```text
//! BL type (257 line, 258 arc, 259 spiral, 260 spiral-curve-spiral,
//!          261 spiral-line-spiral, 266 spiral-spiral-curve, 269 two
//!          spiral-curve-spirals)
//! BS 2 (0 in some Civil 3D 2013 alignments), B
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
//! | 257/9 | — | line tangent to the curves before and after it |
//! | 257/10 | BD, BS, BS, 2BD, BD, BD, 2BD first point, BL, 3 x BS, 2BD second point | line through two points |
//! | 258/0 | 2RD centre, BD radius | whole circle |
//! | 258/1 | 2RD centre, 2RD start | whole circle |
//! | 258/2 | 2RD start, mid, end | fixed arc |
//! | 258/21 | BD radius, BD length, B clockwise | arc continuing the previous element |
//! | 258/13 | BD radius, 2RD end | arc from the previous line, through the end point |
//! | 258/15 | BD radius, 2RD start | arc from the start point onto the next line |
//! | 258/29, 258/30 | BD radius | arc fitted between two lines, or a line and a fixed arc |
//! | 259/0 | BL type, B cw, 2RD start, 2RD direction, BD length, BD radius | spiral from a stored point |
//! | 259/1 | BL type, B cw, BD radius, BD length | spiral from the previous curvature to zero |
//! | 260/0 | BL, BD spiral in, BD radius, BD spiral out | spiral-curve-spiral fitted between two lines |
//! | 260/6 | BL, BD spiral, BD radius, BD arc length, B cw | spiral from the previous curvature, then arc |
//! | 260/7 | BL, BD radius, BD arc length, BD spiral, B cw | arc then spiral ending at the next line's first point |
//! | 260/14 | — | spiral from the previous curvature to zero, tangent to the next line |
//! | 261/5 | — | spiral-line-spiral fitted between the previous and next curves |
//! | 267/3 | BL n, n x (BL, BL count, count x 2RD) | points joined in order |
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
    /// Line tangent to the fixed arcs before and after it.
    FreeLine,
    /// Arc through start, mid, and end points.
    FixedArc { start: Vector2, mid: Vector2, end: Vector2 },
    /// Arc continuing the previous element.
    FloatingArc { radius: f64, length: f64, clockwise: bool },
    /// Arc fitted between the previous and next elements: two lines, or a
    /// line and a fixed arc either way round.
    FreeArc { radius: f64 },
    /// Arc continuing the previous line, ending at a stored point.
    FloatingArcToPoint { radius: f64, end: Vector2 },
    /// Arc from a stored point, ending tangent to the next line.
    FloatingArcFromPoint { radius: f64, start: Vector2 },
    /// Circle by its centre; `start` when the record stores one. A whole
    /// circle when `closed`, otherwise the part between where the lines
    /// next to it touch it, travelled clockwise or not.
    Circle {
        center: Vector2,
        radius: f64,
        start: Option<Vector2>,
        closed: bool,
        clockwise: bool,
    },
    /// Line from a stored point, tangent to the next element.
    FloatingLineFrom { start: Vector2 },
    /// Widening transition of an offset alignment between the element
    /// before and the one after: a curve turning towards the other offset,
    /// then a line and a curve back (`second` set), or a reverse curve whose
    /// radius is solved (`second` unset).
    WideningCurves { first: f64, second: Option<f64> },
    /// Three arcs turning the same way from the element before's end to
    /// the next element's direction: `(radius, length)` each, a middle
    /// length of 0 taking the rest of the turn.
    ThreeCenteredArc { arcs: [(f64, f64); 3] },
    /// Widening transition: two curves the same way, then a reverse curve.
    WideningCompoundReverse { radii: [f64; 3] },
    /// An offset alignment's spiral: the parent spiral, given by its
    /// straight end (its end when it runs out to a straight, its start
    /// otherwise) and the direction of travel there, moved sideways by
    /// `offset` (negative to the left), over the part `from..to` of its
    /// length (fractions).
    OffsetSpiral {
        point: Vector2,
        direction: Vector2,
        start_radius: f64,
        length: f64,
        end_radius: f64,
        clockwise: bool,
        offset: f64,
        from: f64,
        to: f64,
    },
    /// Line from a stored point, then a spiral into the next curve.
    LineSpiralInto { start: Vector2, spiral: f64 },
    /// Spiral from a stored point and direction, curvature rising to `1/radius`.
    SpiralIn {
        start: Vector2,
        direction: Vector2,
        length: f64,
        radius: f64,
        clockwise: bool,
        shape: SpiralShape,
    },
    /// Spiral continuing the previous element, curvature falling to zero.
    SpiralOut {
        radius: f64,
        length: f64,
        clockwise: bool,
        shape: SpiralShape,
    },
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
    /// Spiral out of the previous curve and spiral into the next one, with
    /// `A1 / A2 = ratio`.
    FreeSpiralSpiralCurves { ratio: f64 },
    /// Spiral out of the previous curve, a line, spiral into the next
    /// curve: by both spiral lengths, or by the line's length with equal
    /// `A` on both spirals.
    FreeSpiralLineSpiralCurves { spirals: Option<(f64, f64)>, line: Option<f64> },
    /// Spiral of `length` between the previous and next lines, curvature
    /// rising from the first line (`incurve`) or falling onto the second.
    FreeSpiralBetweenLines { length: f64, incurve: bool },
    /// Two spirals meeting without a curve between the previous and next
    /// lines.
    FreeSpiralSpiralLines { spiral_in: f64, spiral_out: f64 },
    /// Spiral from the previous line into an arc that ends at `end`.
    FloatingSpiralCurveTo { spiral: f64, radius: f64, end: Vector2 },
    /// Spiral out of the previous curve, then a line.
    FloatingSpiralLine { spiral: f64, line: f64 },
    /// Spiral from a stored point and direction, curvature running from
    /// `1/start_radius` to `1/end_radius`.
    CompoundSpiral {
        start: Vector2,
        direction: Vector2,
        start_radius: f64,
        length: f64,
        end_radius: f64,
        clockwise: bool,
    },
    /// Spiral continuing the previous element, curvature running to
    /// `1/end_radius` (zero when it is not finite).
    PlacedSpiral {
        end_radius: f64,
        length: f64,
        clockwise: bool,
        shape: SpiralShape,
    },
    /// Spiral out of the previous curve, reverse spiral into an arc
    /// through `through` that ends at `end`; the arc's radius is solved.
    FloatingReverseSpiralsThrough {
        spiral_out: f64,
        spiral_in: f64,
        through: Vector2,
        end: Vector2,
    },
    /// Line from the previous element's end to `end`.
    FloatingLineTo { end: Vector2 },
    /// Arc fitted between the previous and next lines through a point.
    FreeArcThrough { through: Vector2 },
    /// Arc continuing the previous element, ending at `end`.
    FloatingArcThrough { end: Vector2 },
    /// Arc leaving the previous line, ending at `end` along `direction`.
    FloatingArcToDirection { end: Vector2, direction: Vector2 },
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
    /// Spiral, curve, spiral, curve, spiral from a stored TS on the
    /// previous line to the next line: `spirals` and `radii` as stored, the
    /// curves' lengths solved. `reverse`: the second curve turns the other
    /// way, after a spiral out of the first and one into it.
    SpiralCurveGroup {
        start: Vector2,
        spirals: Vec<f64>,
        radii: [f64; 2],
        first_left: bool,
        reverse: bool,
    },
    /// Points the record stores, joined in order.
    Points { points: Vec<Vector2> },
    /// Element kind or constraint this crate does not decode.
    Other { element_type: i32, constraint: Option<i32> },
}

/// How a spiral's curvature runs from zero to `1/radius` over its length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SpiralShape {
    /// Linear in arc length (type 256), and types not decoded.
    #[default]
    Clothoid,
    /// `(1 - cos(pi x/X)) / 2` against the tangent offset (257).
    SineHalfWave,
    /// `y = x^3 / (6 R X)`, with `X + X^3 / (40 R^2) = L` (258).
    JapaneseCubic,
    /// `3t^2 - 2t^3` of arc length (259).
    Bloss,
    /// `t - sin(2 pi t) / (2 pi)` of arc length (261).
    Sinusoidal,
    /// `y = x^3 / (6 R L)` to arc length `L` (260).
    CubicParabola,
    /// `2t^2`, then `1 - 2(1-t)^2`, of arc length (262).
    Biquadratic,
}

impl SpiralShape {
    fn from_code(code: i32) -> Self {
        match code {
            257 => Self::SineHalfWave,
            258 => Self::JapaneseCubic,
            259 => Self::Bloss,
            260 => Self::CubicParabola,
            261 => Self::Sinusoidal,
            262 => Self::Biquadratic,
            _ => Self::Clothoid,
        }
    }
}

/// Design elements found in a main stream, keyed by their own ids.
pub(super) fn elements(data: &[u8], start: usize, end: usize) -> Vec<AlignmentElement> {
    let mut out: Vec<AlignmentElement> = Vec::new();
    for pos in start..end {
        if !is_header(data, pos, end) {
            continue;
        }
        let member = member_body(data, pos, end);
        let after_type = type_end(data, pos, end);
        if let Some(element) = read_element(&mut Bits::new(data, pos, end), member, after_type) {
            if element.id > 0 && !out.iter().any(|e| e.id == element.id) {
                out.push(element);
            }
        }
    }
    out
}

/// `BL type` (raw, 257..=280) followed by `BS 2`, or `BS 0` in some
/// Civil 3D 2013 alignments, or the header of a group member.
fn is_header(data: &[u8], pos: usize, end: usize) -> bool {
    let mut bits = Bits::new(data, pos, end);
    let Some(element_type) = raw_bit_long(&mut bits) else {
        return false;
    };
    (257..=280).contains(&element_type)
        && (matches!(bits.bit_short(), Some(0 | 2)) || header_type(data, pos, end).is_some_and(|t| matches!(t, 0x04 | 0x09 | 0x0D)))
}

/// The header's type byte. The flag and type bytes start at bit 29 of the
/// record: a flag byte with the parameter lock (0x02) comes first when the
/// lock is on, then the type byte.
fn header_type(data: &[u8], pos: usize, end: usize) -> Option<u8> {
    let mut bits = Bits::new(data, pos + 29, end);
    let first = bits.raw_short_byte()?;
    if first & 0x02 != 0 {
        bits.raw_short_byte()
    } else {
        Some(first)
    }
}

/// Where the parameters of a group member begin. Type bit 0x08 marks the
/// first spiral of a group kept as separate elements; the group's start
/// point (2 RD) follows the type byte.
fn member_body(data: &[u8], pos: usize, end: usize) -> Option<usize> {
    let kind = header_type(data, pos, end)?;
    let body = type_end(data, pos, end)? + 128;
    (kind & 0x08 != 0 && body <= end).then_some(body)
}

/// The bit after the header's type byte.
fn type_end(data: &[u8], pos: usize, end: usize) -> Option<usize> {
    let flagged = Bits::new(data, pos + 29, end).raw_short_byte()? & 0x02 != 0;
    Some(pos + 37 + if flagged { 8 } else { 0 })
}

fn raw_bit_long(bits: &mut Bits) -> Option<i32> {
    // Only the full-width BL form: a type written compactly is not a header.
    (bits.bit()? == false && bits.bit()? == false).then_some(())?;
    bits.raw_long()
}

fn read_element(bits: &mut Bits, member: Option<usize>, after_type: Option<usize>) -> Option<AlignmentElement> {
    let element_type = raw_bit_long(bits)?;
    let mut closed = false;
    // `BS 0 / 2` covers the usual flag and type bytes up to the type's
    // last bit; other flag bytes are skipped by their layout.
    if !matches!(bits.bit_short()?, 0 | 2) {
        bits.pos = after_type? - 1;
    }
    // Lines and widening transitions carry no parameter block.
    let constraint = if element_type == 257 || (273..=275).contains(&element_type) {
        bits.bit()?;
        Some(i32::from(bits.bit_short()?))
    } else {
        match member {
            Some(body) => bits.pos = body,
            None => {
                bits.bit()?;
            }
        }
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
        let constraint = (!matches!(element_type, 268 | 269))
            .then(|| bits.bit_long())
            .flatten()
            .map(|c| c & 0xFFFF);
        if element_type == 258 {
            closed = bits.bit()?;
        }
        constraint
    };
    let id = bits.bit_short()?;
    let previous = bits.bit_short()?;
    let next = bits.bit_short()?;
    let point = |bits: &mut Bits| Some(Vector2::new(bits.raw_double()?, bits.raw_double()?));
    let long = if matches!(element_type, 268 | 269) {
        long_group(bits, element_type == 269)
    } else {
        None
    };
    let kind = match (element_type, constraint) {
        (257, Some(0)) => AlignmentElementKind::FixedLine {
            start: point(bits)?,
            end: point(bits)?,
        },
        (257, Some(9)) => AlignmentElementKind::FreeLine,
        (257, Some(4)) => AlignmentElementKind::FloatingLineTo { end: point(bits)? },
        (257, Some(3)) => AlignmentElementKind::FloatingLineFrom { start: point(bits)? },
        (257, Some(5)) => AlignmentElementKind::FloatingLine {
            length: bits.bit_double()?,
        },
        (257, Some(10)) => {
            let bd_point = |bits: &mut Bits| Some(Vector2::new(bits.bit_double()?, bits.bit_double()?));
            bits.bit_double()?;
            bits.bit_short()?;
            bits.bit_short()?;
            bd_point(bits)?;
            bits.bit_double()?;
            bits.bit_double()?;
            let start = bd_point(bits)?;
            bits.bit_long()?;
            for _ in 0..3 {
                bits.bit_short()?;
            }
            AlignmentElementKind::FixedLine {
                start,
                end: bd_point(bits)?,
            }
        }
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
        (258, Some(0)) => AlignmentElementKind::Circle {
            center: point(bits)?,
            radius: bits.bit_double()?,
            start: None,
            closed,
            clockwise: bits.bit()?,
        },
        (258, Some(1)) => {
            let center = point(bits)?;
            let start = point(bits)?;
            AlignmentElementKind::Circle {
                center,
                radius: center.distance(&start),
                start: Some(start),
                closed,
                clockwise: bits.bit()?,
            }
        }
        (258, Some(15)) => AlignmentElementKind::FloatingArcFromPoint {
            radius: bits.bit_double()?,
            start: point(bits)?,
        },
        (258, Some(13)) => AlignmentElementKind::FloatingArcToPoint {
            radius: bits.bit_double()?,
            end: point(bits)?,
        },
        (258, Some(28)) => AlignmentElementKind::FreeArcThrough {
            through: point(bits)?,
        },
        (258, Some(8)) => AlignmentElementKind::FloatingArcThrough { end: point(bits)? },
        (258, Some(11)) => AlignmentElementKind::FloatingArcToDirection {
            end: point(bits)?,
            direction: point(bits)?,
        },
        (258, Some(29 | 30)) => AlignmentElementKind::FreeArc {
            radius: bits.bit_double()?,
        },
        (259, Some(5)) => {
            bits.bit_long()?;
            let clockwise = bits.bit()?;
            AlignmentElementKind::OffsetSpiral {
                point: point(bits)?,
                direction: point(bits)?,
                start_radius: bits.bit_double()?,
                length: bits.bit_double()?,
                end_radius: bits.bit_double()?,
                clockwise,
                offset: bits.bit_double()?,
                from: bits.bit_double()?,
                to: bits.bit_double()?,
            }
        }
        (259, Some(code @ (0 | 1 | 2 | 6 | 8))) => {
            let shape = SpiralShape::from_code(bits.bit_long()?);
            let clockwise = bits.bit()?;
            if code == 2 {
                AlignmentElementKind::CompoundSpiral {
                    start: point(bits)?,
                    direction: point(bits)?,
                    start_radius: bits.bit_double()?,
                    length: bits.bit_double()?,
                    end_radius: bits.bit_double()?,
                    clockwise,
                }
            } else if code >= 6 {
                AlignmentElementKind::PlacedSpiral {
                    end_radius: bits.bit_double()?,
                    length: bits.bit_double()?,
                    clockwise,
                    shape,
                }
            } else if code == 0 {
                AlignmentElementKind::SpiralIn {
                    start: point(bits)?,
                    direction: point(bits)?,
                    length: bits.bit_double()?,
                    radius: bits.bit_double()?,
                    clockwise,
                    shape,
                }
            } else {
                AlignmentElementKind::SpiralOut {
                    radius: bits.bit_double()?,
                    length: bits.bit_double()?,
                    clockwise,
                    shape,
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
        (260, Some(code @ (10 | 12))) => {
            bits.bit_long()?;
            AlignmentElementKind::FreeSpiralBetweenLines {
                length: bits.bit_double()?,
                incurve: code == 10,
            }
        }
        (260, Some(8)) => {
            bits.bit_long()?;
            AlignmentElementKind::FreeSpiralSpiralLines {
                spiral_in: bits.bit_double()?,
                spiral_out: bits.bit_double()?,
            }
        }
        (260, Some(2)) => {
            bits.bit_long()?;
            AlignmentElementKind::FloatingSpiralCurveTo {
                spiral: bits.bit_double()?,
                radius: bits.bit_double()?,
                end: point(bits)?,
            }
        }
        // Civil 3D 2013 writes code 5 where later releases write 4.
        (261, Some(4 | 5)) => {
            bits.bit_long()?;
            AlignmentElementKind::FreeSpiralSpiralCurves {
                ratio: bits.bit_double()?,
            }
        }
        (261, Some(0)) => {
            bits.bit_long()?;
            AlignmentElementKind::FreeSpiralLineSpiralCurves {
                spirals: Some((bits.bit_double()?, bits.bit_double()?)),
                line: None,
            }
        }
        (261, Some(1)) => {
            bits.bit_long()?;
            AlignmentElementKind::FreeSpiralLineSpiralCurves {
                spirals: None,
                line: Some(bits.bit_double()?),
            }
        }
        (261, Some(161)) => {
            bits.bit_long()?;
            AlignmentElementKind::FloatingSpiralLine {
                spiral: bits.bit_double()?,
                line: bits.bit_double()?,
            }
        }
        (267, Some(1)) => {
            ((bits.bit_long()?, bits.bit_long()?, bits.bit_long()?) == (0, 1, 3)).then_some(())?;
            let mut arc = || Some((bits.bit_double()?, bits.bit_double()?));
            AlignmentElementKind::ThreeCenteredArc {
                arcs: [arc()?, arc()?, arc()?],
            }
        }
        (273, Some(0)) => AlignmentElementKind::WideningCurves {
            first: bits.bit_double()?,
            second: Some(bits.bit_double()?),
        },
        (274, Some(0)) => AlignmentElementKind::WideningCurves {
            first: bits.bit_double()?,
            second: None,
        },
        (275, Some(1)) => AlignmentElementKind::WideningCompoundReverse {
            radii: [bits.bit_double()?, bits.bit_double()?, bits.bit_double()?],
        },
        (261, Some(162)) => {
            bits.bit_long()?;
            AlignmentElementKind::LineSpiralInto {
                start: point(bits)?,
                spiral: bits.bit_double()?,
            }
        }
        (266, Some(2)) => {
            bits.bit_long()?;
            AlignmentElementKind::FloatingReverseSpiralsThrough {
                spiral_out: bits.bit_double()?,
                spiral_in: bits.bit_double()?,
                through: point(bits)?,
                end: point(bits)?,
            }
        }
        (266, Some(1)) => {
            bits.bit_long()?;
            AlignmentElementKind::FloatingSpiralSpiralCurve {
                spiral_out: bits.bit_double()?,
                spiral_in: bits.bit_double()?,
                radius: bits.bit_double()?,
                end: point(bits)?,
            }
        }
        (267, Some(3)) => {
            let mut points = Vec::new();
            let sets = bits.bit_long()?;
            if !(1..=64).contains(&sets) {
                return None;
            }
            for _ in 0..sets {
                bits.bit_long()?;
                let count = bits.bit_long()?;
                if !(1..=4096).contains(&count) {
                    return None;
                }
                for _ in 0..count {
                    let p = point(bits)?;
                    // Sets share their joining point.
                    if points.last().is_none_or(|q: &Vector2| q.distance(&p) > 1e-9) {
                        points.push(p);
                    }
                }
            }
            AlignmentElementKind::Points { points }
        }
        // Long spiral groups from a stored TS (later releases): `BL n,
        // B first curve left, B`, the spiral lengths and radii, `RD TS`.
        (268 | 269, None) if long.is_some() => long?,
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
        AlignmentElementKind::FloatingArcToPoint { radius, end: p }
        | AlignmentElementKind::FloatingArcFromPoint { radius, start: p } => {
            positive(*radius) && p.x.is_finite() && p.y.is_finite()
        }
        AlignmentElementKind::Circle { center, radius, .. } => {
            positive(*radius) && center.x.is_finite() && center.y.is_finite()
        }
        AlignmentElementKind::Points { points } => {
            points.len() > 1 && points.iter().all(|p| p.x.is_finite() && p.y.is_finite())
        }
        AlignmentElementKind::FixedLine { start, end } => {
            [start.x, start.y, end.x, end.y].iter().all(|v| v.is_finite())
        }
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

/// A long spiral group's TS layout; the reader is left after it.
fn long_group(bits: &mut Bits, reverse: bool) -> Option<AlignmentElementKind> {
    let from = bits.pos;
    let parsed = (|| {
        bits.bit_long()?;
        bits.bit_long()?;
        let first_left = bits.bit()?;
        bits.bit()?;
        let mut v = Vec::new();
        for _ in 0..if reverse { 6 } else { 5 } {
            v.push(bits.bit_double().filter(|x| x.is_finite() && *x > 0.0 && *x < 1e9)?);
        }
        let start = Vector2::new(bits.raw_double()?, bits.raw_double()?);
        let plausible = |x: f64| (x.is_normal() || x == 0.0) && x.abs() < 1e9;
        (plausible(start.x) && plausible(start.y)).then_some(())?;
        Some(if reverse {
            AlignmentElementKind::SpiralCurveGroup {
                start,
                spirals: vec![v[0], v[2], v[3], v[5]],
                radii: [v[1], v[4]],
                first_left,
                reverse,
            }
        } else {
            AlignmentElementKind::SpiralCurveGroup {
                start,
                spirals: vec![v[0], v[2], v[4]],
                radii: [v[1], v[3]],
                first_left,
                reverse,
            }
        })
    })();
    if parsed.is_none() {
        bits.pos = from;
    }
    parsed
}

/// An open circle's signed offset of its centre to the left of travel.
fn open_circle(element: Option<&&AlignmentElement>) -> Option<(Vector2, f64)> {
    match element.map(|e| &e.kind) {
        Some(AlignmentElementKind::Circle {
            center,
            radius,
            closed: false,
            clockwise,
            ..
        }) => Some((*center, if *clockwise { -radius } else { *radius })),
        _ => None,
    }
}

/// The line leaving `(c1, s1)` and arriving at `(c2, s2)` tangentially,
/// each a centre with its signed offset to the left of travel (0 for a
/// point).
fn tangent_line(c1: Vector2, s1: f64, c2: Vector2, s2: f64) -> Option<(Vector2, Vector2)> {
    let d = c2 - c1;
    let length = d.length();
    let ratio = (s2 - s1) / length;
    if length < 1e-12 || ratio.abs() > 1.0 {
        return None;
    }
    let u = direction(heading(d) - ratio.asin());
    let n = left(u);
    Some((c1 - n * s1, c2 - n * s2))
}

/// Lines next to an open circle, placed from the circle: a free line
/// between two open circles is their common tangent, a line to or from a
/// stored point the tangent through that point.
fn lines_at_open_circles(run: &[&AlignmentElement]) -> Vec<AlignmentElement> {
    run.iter()
        .enumerate()
        .map(|(i, e)| {
            let before = i.checked_sub(1).and_then(|j| run.get(j));
            let after = run.get(i + 1);
            let line = match &e.kind {
                AlignmentElementKind::FreeLine => open_circle(before)
                    .zip(open_circle(after))
                    .and_then(|((c1, s1), (c2, s2))| tangent_line(c1, s1, c2, s2)),
                AlignmentElementKind::FloatingLineTo { end } => {
                    open_circle(before).and_then(|(c, s)| tangent_line(c, s, *end, 0.0))
                }
                AlignmentElementKind::FloatingLineFrom { start } => {
                    open_circle(after).and_then(|(c, s)| tangent_line(*start, 0.0, c, s))
                }
                _ => None,
            };
            match line {
                Some((start, end)) => AlignmentElement {
                    kind: AlignmentElementKind::FixedLine { start, end },
                    ..(*e).clone()
                },
                None => (*e).clone(),
            }
        })
        .collect()
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
    let heading_at = |s: f64| h + k0 * s + (k1 - k0) * s * s / (2.0 * length);
    for i in 0..n {
        let (s0, s1) = (i as f64 * ds, (i + 1) as f64 * ds);
        // The chord of a step is shorter than the step by sinc of half its
        // turn: exact on an arc, close on a spiral.
        let half = (heading_at(s1) - heading_at(s0)) / 2.0;
        let chord = if half.abs() < 1e-12 { ds } else { ds * half.sin() / half };
        p = p + direction(heading_at((s0 + s1) / 2.0)) * chord;
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
            // Later parts repeat their first point: that marks the join.
            let skip = usize::from(points.len() == 1);
            points.extend(q.into_iter().skip(skip));
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
    /// `BP`, `PI`, `TS`, `SC`, `CS`, `ST`, `PC`, `PT`, `PCC`, `PRC`, `SPI`,
    /// `EP`.
    pub name: &'static str,
}

/// Kind of a solved alignment piece, the way an alignment style draws it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum AlignmentPiece {
    Line,
    Curve,
    Spiral,
}

use AlignmentPiece as Piece;

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

/// `(start, end, kind)` index ranges of the pieces of a solved run. Every
/// solved piece repeats its first point at the join.
fn split(raw: &[Vector2]) -> Vec<(usize, usize, Piece)> {
    let mut joins = vec![0];
    for i in 1..raw.len() {
        if raw[i].distance(&raw[i - 1]) < 1e-9 {
            joins.push(i);
        }
    }
    joins.push(raw.len());
    joins
        .windows(2)
        .filter(|w| w[1] > w[0] + 1)
        .map(|w| (w[0], w[1], piece_kind(&raw[w[0]..w[1]])))
        .collect()
}

/// Solved pieces of every run with their kind.
pub(super) fn pieces_by_kind(elements: &[AlignmentElement]) -> Vec<(Piece, Vec<Vector2>)> {
    raw_polylines(elements)
        .iter()
        .flat_map(|raw| {
            split(raw)
                .into_iter()
                .map(|(a, b, kind)| (kind, raw[a..b].to_vec()))
                .collect::<Vec<_>>()
        })
        .collect()
}

/// Geometry points along the first solved run.
pub(super) fn geometry_points(elements: &[AlignmentElement]) -> Vec<GeometryPoint> {
    let Some(raw) = raw_polylines(elements).into_iter().next() else {
        return Vec::new();
    };
    let pieces = split(&raw);
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
        push(first.0, "BP");
    }
    // Curvature at either end of a piece, from points a metre apart so a
    // tiny closing segment does not count.
    let at_end = |p: &[Vector2]| -> f64 {
        let mut picked = vec![p[0]];
        for q in &p[1..] {
            if picked.len() == 3 {
                break;
            }
            if q.distance(picked.last().unwrap()) >= 1.0 {
                picked.push(*q);
            }
        }
        if picked.len() < 3 {
            return 0.0;
        }
        curvature_at(picked[0], picked[1], picked[2])
    };
    let ends = |(a, b, _): (usize, usize, Piece)| {
        let p = &raw[a..b];
        let reversed: Vec<Vector2> = p.iter().rev().copied().collect();
        (at_end(p), -at_end(&reversed))
    };
    for pair in pieces.windows(2) {
        let (k0, k1) = (ends(pair[0]).1, ends(pair[1]).0);
        let name = match (pair[0].2, pair[1].2) {
            (Piece::Line, Piece::Spiral) => "TS",
            (Piece::Spiral, Piece::Curve) => "SC",
            (Piece::Curve, Piece::Spiral) => "CS",
            (Piece::Spiral, Piece::Line) => "ST",
            (Piece::Line, Piece::Curve) => "PC",
            (Piece::Curve, Piece::Line) => "PT",
            (Piece::Curve, Piece::Curve) if k0 * k1 < 0.0 => "PRC",
            (Piece::Curve, Piece::Curve) => "PCC",
            (Piece::Spiral, Piece::Spiral) => "SPI",
            (Piece::Line, Piece::Line) => "PI",
        };
        push(pair[1].0, name);
    }
    if let Some(last) = pieces.last() {
        push(last.1 - 1, "EP");
    }
    out
}

fn raw_polylines(elements: &[AlignmentElement]) -> Vec<Vec<Vector2>> {
    let mut out = Vec::new();
    for run in chains(elements) {
        let placed = lines_at_open_circles(&run);
        let run: Vec<&AlignmentElement> = placed.iter().collect();
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
        // A line to a stored point starts where the element before ends.
        let from_end;
        let kind = match &element.kind {
            AlignmentElementKind::FloatingLineTo { end } => {
                attach(&mut points, &mut pending, &mut state);
                match state {
                    Some(s) => {
                        from_end = AlignmentElementKind::FixedLine { start: s.point, end: *end };
                        &from_end
                    }
                    None => &element.kind,
                }
            }
            kind => kind,
        };
        let solved: Option<(Vec<Vector2>, State)> = match kind {
            AlignmentElementKind::FixedLine { start, end } => {
                let Some(u) = unit(*end - *start) else {
                    continue;
                };
                if let Some(line) = pending.take() {
                    // Lines that already meet join where they meet; only lines
                    // apart join at their tangent intersection.
                    let joint = if line.end.distance(start) <= TOLERANCE {
                        line.end
                    } else {
                        intersect(line.start, line.direction, *start, u)
                            .filter(|x| x.distance(&line.end) < 1e4)
                            .unwrap_or(line.end)
                    };
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
                shape,
            } => {
                attach(&mut points, &mut pending, &mut state);
                let k = curvature(*radius, *clockwise);
                state.map(|s| spiral(s, k, false, *length, *shape))
            }
            AlignmentElementKind::SpiralIn {
                start,
                direction: d,
                length,
                radius,
                clockwise,
                shape,
            } => {
                close(&mut points, &mut pending, *start);
                let begin = State {
                    point: *start,
                    heading: heading(*d),
                    curvature: 0.0,
                };
                Some(spiral(begin, curvature(*radius, *clockwise), true, *length, *shape))
            }
            AlignmentElementKind::FixedArc { start, mid, end } => {
                close(&mut points, &mut pending, *start);
                // An arc fitted between this arc and the line after it ends
                // this arc where it leaves.
                let fillet = match next.map(|n| &n.kind) {
                    Some(AlignmentElementKind::FreeArc { radius }) => run
                        .get(index + 1)
                        .and_then(|after| line_carrier(after))
                        .and_then(|(b, v)| arc_then_fillet(*start, *mid, *end, *radius, b, v))
                        .map(|solved| (solved, 1)),
                    // A free line to the next fixed arc ends this arc where
                    // the line leaves and starts that arc where it arrives.
                    Some(AlignmentElementKind::FreeLine) => match run.get(index + 1).map(|after| &after.kind) {
                        Some(AlignmentElementKind::FixedArc { start: a2, mid: m2, end: e2 }) => {
                            arc_line_arc((*start, *mid, *end), (*a2, *m2, *e2)).map(|solved| (solved, 2))
                        }
                        _ => None,
                    },
                    // A group from this arc to the fixed arc after it leaves this
                    // arc where it is solved to.
                    Some(
                        group @ (AlignmentElementKind::FreeSpiralSpiralCurves { .. }
                        | AlignmentElementKind::FreeSpiralLineSpiralCurves { .. }),
                    ) => match run.get(index + 1).map(|after| &after.kind) {
                        Some(AlignmentElementKind::FixedArc { start: a2, mid: m2, end: e2 }) => {
                            let parts = |k1: f64, k2: f64, value: f64| -> Option<Vec<(f64, f64, f64)>> {
                                Some(match group {
                                    AlignmentElementKind::FreeSpiralSpiralCurves { ratio } => {
                                        let a2 = value / ratio;
                                        vec![(k1, 0.0, value * value * k1.abs()), (0.0, k2, a2 * a2 * k2.abs())]
                                    }
                                    AlignmentElementKind::FreeSpiralLineSpiralCurves {
                                        spirals: Some((l1, l2)),
                                        ..
                                    } => vec![(k1, 0.0, *l1), (0.0, 0.0, value), (0.0, k2, *l2)],
                                    AlignmentElementKind::FreeSpiralLineSpiralCurves { line: Some(line), .. } => {
                                        let a = value * value;
                                        vec![(k1, 0.0, a * k1.abs()), (0.0, 0.0, *line), (0.0, k2, a * k2.abs())]
                                    }
                                    _ => return None,
                                })
                            };
                            let seeds: &[f64] = match group {
                                AlignmentElementKind::FreeSpiralLineSpiralCurves { spirals: Some(_), .. } => {
                                    &[0.0, 10.0, 100.0, 1000.0, 5000.0]
                                }
                                _ => &[10.0, 30.0, 100.0, 300.0, 1000.0, 3000.0],
                            };
                            between_circles((*start, *mid, *end), (*a2, *m2, *e2), parts, seeds).and_then(
                                |(from, q, arrive)| {
                                    let (mut all, ..) = arc_to_point(arc_start_state(*start, *mid, *end), from.point)?;
                                    all.extend(q);
                                    let (rest, last, _) = arc_to_point(arrive, *e2)?;
                                    all.extend(rest);
                                    Some(((all, last), 2))
                                },
                            )
                        }
                        _ => None,
                    },
                    // Reverse spirals leaving this arc where they are solved to.
                    Some(
                        group @ (AlignmentElementKind::FloatingSpiralSpiralCurve { .. }
                        | AlignmentElementKind::FloatingReverseSpiralsThrough { .. }),
                    ) => {
                        let solved = match group {
                            AlignmentElementKind::FloatingSpiralSpiralCurve {
                                spiral_out,
                                spiral_in,
                                radius,
                                end: to,
                            } => reverse_from_circle((*start, *mid, *end), *spiral_out, *spiral_in, Some(*radius), *to, *to),
                            AlignmentElementKind::FloatingReverseSpiralsThrough {
                                spiral_out,
                                spiral_in,
                                through,
                                end: to,
                            } => reverse_from_circle((*start, *mid, *end), *spiral_out, *spiral_in, None, *through, *to),
                            _ => None,
                        };
                        solved.and_then(|(from, q, last)| {
                            let (mut all, ..) = arc_to_point(arc_start_state(*start, *mid, *end), from.point)?;
                            all.extend(q);
                            Some(((all, last), 1))
                        })
                    }
                    _ => None,
                };
                match fillet {
                    Some((solved, skip)) => {
                        index += skip;
                        Some(solved)
                    }
                    None => three_point_arc(*start, *mid, *end),
                }
            }
            AlignmentElementKind::FreeArc { radius } => {
                // Line, this arc, then a fixed arc: the fixed arc starts
                // where this one meets it.
                let fixed = next.and_then(|n| match &n.kind {
                    AlignmentElementKind::FixedArc { start, mid, end } => Some((*start, *mid, *end)),
                    _ => None,
                });
                match (pending.take(), fixed) {
                    (Some(line), Some((a, m, c))) => fillet_then_arc(&line, *radius, a, m, c).map(|(q, end)| {
                        index += 1;
                        points.push(line.start);
                        (q, end)
                    }),
                    (line, _) => {
                        pending = line;
                        fitted_curve(&mut points, &mut pending, state, next, 0.0, *radius, 0.0)
                    }
                }
            }
            AlignmentElementKind::FloatingArcToPoint { radius, end } => {
                let carrier = match pending.as_ref() {
                    Some(line) => Some((line.start, line.direction)),
                    None => state.filter(|s| s.curvature.abs() < 1e-12).map(|s| (s.point, direction(s.heading))),
                };
                carrier.and_then(|(p, u)| arc_from_line_to_point(p, u, *radius, *end)).map(|(touch, q, end)| {
                    close(&mut points, &mut pending, touch);
                    (q, end)
                })
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
            AlignmentElementKind::FreeSpiralSpiralCurves { .. }
            | AlignmentElementKind::FreeSpiralLineSpiralCurves { .. } => {
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
                                sls.extend(q);
                                (sls, end)
                            }
                            None => (sls, sls_end),
                        })
                    }
                    _ => None,
                }
            }
            AlignmentElementKind::FreeSpiralBetweenLines { length, incurve } => {
                slide_between_lines(&mut points, &mut pending, next, |turn| {
                    let k = 2.0 * turn / length;
                    vec![if *incurve { (0.0, k, *length) } else { (k, 0.0, *length) }]
                })
            }
            AlignmentElementKind::FreeSpiralSpiralLines { spiral_in, spiral_out } => {
                slide_between_lines(&mut points, &mut pending, next, |turn| {
                    let k = 2.0 * turn / (spiral_in + spiral_out);
                    vec![(0.0, k, *spiral_in), (k, 0.0, *spiral_out)]
                })
            }
            AlignmentElementKind::FloatingSpiralCurveTo { spiral, radius, end } => line_from(&pending, state)
                .and_then(|(p, u)| spiral_curve_to(p, u, *spiral, *radius, *end))
                .map(|(start, q, end)| {
                    close(&mut points, &mut pending, start);
                    (q, end)
                }),
            AlignmentElementKind::FloatingSpiralLine { spiral, line } => {
                attach(&mut points, &mut pending, &mut state);
                state.map(|s| pieces(s, &[(s.curvature, 0.0, *spiral), (0.0, 0.0, *line)]))
            }
            AlignmentElementKind::CompoundSpiral {
                start,
                direction: d,
                start_radius,
                length,
                end_radius,
                clockwise,
            } => {
                close(&mut points, &mut pending, *start);
                let (k0, k1) = (curvature(*start_radius, *clockwise), curvature(*end_radius, *clockwise));
                let begin = State {
                    point: *start,
                    heading: heading(*d),
                    curvature: k0,
                };
                Some(pieces(begin, &[(k0, k1, *length)]))
            }
            AlignmentElementKind::PlacedSpiral {
                end_radius,
                length,
                clockwise,
                shape,
            } => {
                attach(&mut points, &mut pending, &mut state);
                let k1 = if end_radius.is_finite() && *end_radius > 0.0 && *end_radius < 1e12 {
                    curvature(*end_radius, *clockwise)
                } else {
                    0.0
                };
                state.map(|s| {
                    if s.curvature.abs() < 1e-12 {
                        spiral(s, k1, true, *length, *shape)
                    } else if k1 == 0.0 {
                        spiral(s, s.curvature, false, *length, *shape)
                    } else {
                        pieces(s, &[(s.curvature, k1, *length)])
                    }
                })
            }
            AlignmentElementKind::FreeArcThrough { through } => {
                let radius = pending
                    .as_ref()
                    .zip(next.and_then(line_carrier))
                    .and_then(|(line, (b, v))| fillet_radius_through(line.start, line.direction, b, v, *through));
                radius.and_then(|r| fitted_curve(&mut points, &mut pending, state, next, 0.0, r, 0.0))
            }
            AlignmentElementKind::FloatingArcThrough { end } => {
                attach(&mut points, &mut pending, &mut state);
                state.and_then(|s| {
                    let w = *end - s.point;
                    let k = 2.0 * cross(direction(s.heading), w) / w.dot(&w);
                    arc_to_point(State { curvature: k, ..s }, *end).map(|(q, end, _)| (q, end))
                })
            }
            AlignmentElementKind::FloatingArcToDirection { end, direction: d } => line_from(&pending, state)
                .and_then(|(p, u)| arc_to_direction(p, u, *end, unit(*d)?))
                .and_then(|(touch, at)| {
                    close(&mut points, &mut pending, touch);
                    arc_to_point(at, *end).map(|(q, end, _)| (q, end))
                }),
            AlignmentElementKind::SpiralCurveGroup {
                start,
                spirals,
                radii,
                first_left,
                reverse,
            } => {
                let u = pending.as_ref().map(|line| line.direction);
                u.zip(next.and_then(line_carrier)).and_then(|(u, (b, v))| {
                    let k1 = if *first_left { 1.0 } else { -1.0 } / radii[0];
                    let k2 = if *reverse { -k1.signum() } else { k1.signum() } / radii[1];
                    let parts = |c1: f64, c2: f64| -> Vec<(f64, f64, f64)> {
                        if *reverse {
                            vec![
                                (0.0, k1, spirals[0]),
                                (k1, k1, c1),
                                (k1, 0.0, spirals[1]),
                                (0.0, k2, spirals[2]),
                                (k2, k2, c2),
                                (k2, 0.0, spirals[3]),
                            ]
                        } else {
                            vec![
                                (0.0, k1, spirals[0]),
                                (k1, k1, c1),
                                (k1, k2, spirals[1]),
                                (k2, k2, c2),
                                (k2, 0.0, spirals[2]),
                            ]
                        }
                    };
                    let begin = State {
                        point: *start,
                        heading: heading(u),
                        curvature: 0.0,
                    };
                    let seeds: Vec<(f64, f64)> = [1.0, 30.0, 300.0, 3000.0]
                        .iter()
                        .flat_map(|a| [1.0, 30.0, 300.0, 3000.0].map(|b| (*a, b)))
                        .collect();
                    let (c1, c2) = newton2(
                        |c1, c2| {
                            let end = end_of(begin, &parts(c1, c2));
                            Some((normalize(end.heading - heading(v)), cross(end.point - b, v)))
                        },
                        &seeds,
                    )
                    .into_iter()
                    .filter(|(c1, c2)| *c1 >= -1e-9 && *c2 >= -1e-9)
                    .min_by(|a, b| (a.0 + a.1).total_cmp(&(b.0 + b.1)))?;
                    close(&mut points, &mut pending, *start);
                    Some(pieces(begin, &parts(c1.max(0.0), c2.max(0.0))))
                })
            }
            AlignmentElementKind::LineSpiralInto { start, spiral } => {
                // The line meets the curve shifted out by the spiral's
                // offset; the spiral starts its tangent offset before that.
                let circle = open_circle(next.as_ref()).or_else(|| match next.map(|n| &n.kind) {
                    Some(AlignmentElementKind::FixedArc { start: a, mid: m, end: c }) => {
                        let (center, k, _) = arc_circle(*a, *m, *c)?;
                        Some((center, 1.0 / k))
                    }
                    _ => None,
                });
                circle.and_then(|(center, signed)| {
                    let r = signed.abs();
                    let local = end_of(
                        State {
                            point: Vector2::new(0.0, 0.0),
                            heading: 0.0,
                            curvature: 0.0,
                        },
                        &[(0.0, 1.0 / r, *spiral)],
                    );
                    let theta = spiral / (2.0 * r);
                    let shift = local.point.y - r * (1.0 - theta.cos());
                    let along = local.point.x - r * theta.sin();
                    let (_, foot) = tangent_line(*start, 0.0, center, signed.signum() * (r + shift))?;
                    let u = unit(foot - *start)?;
                    let ts = foot - u * along;
                    close(&mut points, &mut pending, *start);
                    let begin = State {
                        point: ts,
                        heading: heading(u),
                        curvature: 0.0,
                    };
                    let (mut q, end) = pieces(begin, &[(0.0, 1.0 / signed, *spiral)]);
                    q.insert(0, *start);
                    Some((q, end))
                })
            }
            AlignmentElementKind::OffsetSpiral {
                point,
                direction: d,
                start_radius,
                length,
                end_radius,
                clockwise,
                offset,
                from,
                to,
            } => parallel_spiral(*point, *d, *start_radius, *length, *end_radius, *clockwise, *offset, (*from, *to))
                .map(|(q, end)| {
                    close(&mut points, &mut pending, q[0]);
                    (q, end)
                }),
            AlignmentElementKind::WideningCurves { first, second } => {
                attach(&mut points, &mut pending, &mut state);
                state.zip(start_of(next)).and_then(|(s, (to, h))| {
                    widening_transition(s, to, h, |sign, turn, a, b| {
                        // The curve back turns by the rest of the heading change.
                        let back = a - sign * turn;
                        Some(match second {
                            Some(r2) => vec![(sign / first, sign / first, first * a), (0.0, 0.0, b), (-sign / r2, -sign / r2, r2 * back)],
                            None => {
                                (b > 0.0).then_some(())?;
                                vec![(sign / first, sign / first, first * a), (-sign / b, -sign / b, b * back)]
                            }
                        })
                    })
                })
            }
            AlignmentElementKind::ThreeCenteredArc { arcs } => {
                attach(&mut points, &mut pending, &mut state);
                state.zip(start_of(next)).and_then(|(s, (to, h))| {
                    let (u, v) = (direction(s.heading), direction(h));
                    let across = cross(u, v);
                    // The way the direction turns, or towards the next start
                    // when the directions are parallel.
                    let sign = if across.abs() > 1e-12 { across.signum() } else { cross(u, to - s.point).signum() };
                    let mut total = sign * across.atan2(u.dot(&v));
                    if total <= 0.0 {
                        total += TAU;
                    }
                    let [(r1, l1), (r2, l2), (r3, l3)] = *arcs;
                    let middle = if l2 > 0.0 { l2 } else { r2 * (total - l1 / r1 - l3 / r3) };
                    (middle >= 0.0 && r1 > 0.0 && r2 > 0.0 && r3 > 0.0).then(|| {
                        pieces(
                            s,
                            &[(sign / r1, sign / r1, l1), (sign / r2, sign / r2, middle), (sign / r3, sign / r3, l3)],
                        )
                    })
                })
            }
            AlignmentElementKind::WideningCompoundReverse { radii: [r1, r2, r3] } => {
                attach(&mut points, &mut pending, &mut state);
                state.zip(start_of(next)).and_then(|(s, (to, h))| {
                    widening_transition(s, to, h, |sign, turn, a, b| {
                        let back = a + b / r2 - sign * turn;
                        Some(vec![(sign / r1, sign / r1, r1 * a), (sign / r2, sign / r2, b), (-sign / r3, -sign / r3, r3 * back)])
                    })
                })
            }
            AlignmentElementKind::FloatingReverseSpiralsThrough { .. }
            | AlignmentElementKind::FloatingLineTo { .. }
            | AlignmentElementKind::FloatingLineFrom { .. } => None,
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
                            all.extend(in_points);
                            all.extend(arc);
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
            AlignmentElementKind::FloatingArcFromPoint { radius, start } => {
                close(&mut points, &mut pending, *start);
                // Solved backwards: an arc leaving the next line against its
                // direction and reaching the start point, then reversed.
                next.and_then(line_carrier).and_then(|(b, v)| {
                    let back = Vector2::new(-v.x, -v.y);
                    let (touch, mut q, _) = arc_from_line_to_point_any(b, back, *radius, *start)?;
                    q.reverse();
                    Some((
                        q,
                        State {
                            point: touch,
                            heading: heading(v),
                            curvature: 0.0,
                        },
                    ))
                })
            }
            AlignmentElementKind::Circle {
                center,
                radius,
                closed: false,
                clockwise,
                ..
            } => {
                // From where the line before leaves off to where the line after
                // starts; a half circle on a side with no line.
                let into = pending.as_ref().map(|line| line.end).or(state.map(|s| s.point));
                let out = next.and_then(line_carrier).map(|(p, _)| p);
                let half = |p: Vector2| *center * 2.0 - p;
                into.or(out.map(half)).and_then(|from| {
                    let to = out.unwrap_or_else(|| half(from));
                    close(&mut points, &mut pending, from);
                    let k = curvature(*radius, *clockwise);
                    let a = heading(from - *center) + if *clockwise { -PI / 2.0 } else { PI / 2.0 };
                    let at = State {
                        point: from,
                        heading: a,
                        curvature: k,
                    };
                    arc_to_point(at, to).map(|(q, end, _)| (q, end))
                })
            }
            AlignmentElementKind::Circle { center, radius, start, .. } => {
                // ponytail: a circle stored by centre and radius keeps no
                // start; it is drawn from its west point, which only moves
                // where its stations begin.
                let from = start.unwrap_or(Vector2::new(center.x - radius, center.y));
                close(&mut points, &mut pending, from);
                let a0 = (from.y - center.y).atan2(from.x - center.x);
                let n = ((TAU * radius / STEP).ceil() as usize).clamp(16, 20000);
                let q: Vec<Vector2> = (0..=n)
                    .map(|j| *center + direction(a0 + TAU * j as f64 / n as f64) * *radius)
                    .collect();
                Some((
                    q,
                    State {
                        point: from,
                        heading: a0 + PI / 2.0,
                        curvature: 1.0 / radius,
                    },
                ))
            }
            AlignmentElementKind::Points { points: q } => {
                close(&mut points, &mut pending, q[0]);
                let n = q.len();
                Some((
                    q.clone(),
                    State {
                        point: q[n - 1],
                        heading: heading(q[n - 1] - q[n - 2]),
                        curvature: 0.0,
                    },
                ))
            }
            // Solved with the fixed arc before it; on its own it has no
            // position.
            AlignmentElementKind::FreeLine | AlignmentElementKind::Other { .. } => None,
        };
        match solved {
            Some((q, end)) => {
                // A piece that starts off the previous end still joins it.
                if let (Some(&last), Some(&first)) = (points.last(), q.first()) {
                    if last.distance(&first) >= 1e-9 {
                        points.push(first);
                    }
                }
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

/// A spiral of `shape` from `s`: curvature rising from zero to `k`
/// (`inward`), or falling from `k` to zero.
fn spiral(s: State, k: f64, inward: bool, length: f64, shape: SpiralShape) -> (Vec<Vector2>, State) {
    use std::f64::consts::PI;
    // Integral over [0, t] of the curvature fraction, for shapes defined
    // along the arc.
    let along: Option<fn(f64) -> f64> = match shape {
        SpiralShape::Clothoid => None,
        SpiralShape::Bloss => Some(|t| t.powi(3) - t.powi(4) / 2.0),
        SpiralShape::Sinusoidal => Some(|t| t * t / 2.0 + ((2.0 * PI * t).cos() - 1.0) / (4.0 * PI * PI)),
        SpiralShape::Biquadratic => Some(|t| {
            if t <= 0.5 {
                2.0 * t.powi(3) / 3.0
            } else {
                1.0 / 12.0 + (t - 0.5) + 2.0 / 3.0 * ((1.0 - t).powi(3) - 0.125)
            }
        }),
        _ => {
            return offset_spiral(s, k, inward, length, shape);
        }
    };
    let Some(integral) = along else {
        let (k0, k1) = if inward { (s.curvature, k) } else { (k, 0.0) };
        return pieces(s, &[(k0, k1, length)]);
    };
    // The shapes are symmetric about mid-length, so falling curvature is
    // `k (1 - f(t))`.
    let heading_at = |d: f64| {
        let t = d / length;
        if inward {
            s.heading + k * length * integral(t)
        } else {
            s.heading + k * d - k * length * integral(t)
        }
    };
    let n = ((length / STEP).ceil() as usize).clamp(2, 20000);
    let ds = length / n as f64;
    let mut points = Vec::with_capacity(n + 1);
    let mut p = s.point;
    points.push(p);
    for i in 0..n {
        let (h0, h1) = (heading_at(i as f64 * ds), heading_at((i + 1) as f64 * ds));
        let half = (h1 - h0) / 2.0;
        let chord = if half.abs() < 1e-12 { ds } else { ds * half.sin() / half };
        p = p + direction(heading_at((i as f64 + 0.5) * ds)) * chord;
        points.push(p);
    }
    let end = State {
        point: p,
        heading: heading_at(length),
        curvature: if inward { k } else { 0.0 },
    };
    (points, end)
}

/// Spirals given as a tangent offset `y(x)` from the tangent at their
/// zero-curvature end, out to `x = X`.
fn offset_spiral(s: State, k: f64, inward: bool, length: f64, shape: SpiralShape) -> (Vec<Vector2>, State) {
    use std::f64::consts::PI;
    let r = 1.0 / k.abs();
    let arc = |slope: &dyn Fn(f64) -> f64, x_end: f64| {
        let n = 2000;
        (0..n)
            .map(|i| {
                let x = (i as f64 + 0.5) * x_end / n as f64;
                (1.0 + slope(x).powi(2)).sqrt() * x_end / n as f64
            })
            .sum::<f64>()
    };
    // `X` from the shape's rule; `y` and its slope.
    let (x_end, offset, slope): (f64, Box<dyn Fn(f64) -> f64>, Box<dyn Fn(f64) -> f64>) = match shape {
        SpiralShape::JapaneseCubic => {
            // X + X^3 / (40 R^2) = L
            let mut x = length;
            for _ in 0..50 {
                x -= (x + x.powi(3) / (40.0 * r * r) - length) / (1.0 + 3.0 * x * x / (40.0 * r * r));
            }
            (x, Box::new(move |t| t.powi(3) / (6.0 * r * x)), Box::new(move |t| t * t / (2.0 * r * x)))
        }
        SpiralShape::CubicParabola => {
            let slope = move |t: f64| t * t / (2.0 * r * length);
            let (mut lo, mut hi) = (0.5 * length, length);
            for _ in 0..60 {
                let mid = (lo + hi) / 2.0;
                if arc(&slope, mid) < length { lo = mid } else { hi = mid }
            }
            (lo, Box::new(move |t| t.powi(3) / (6.0 * r * length)), Box::new(slope))
        }
        _ => {
            // Sine half-wave: curvature (1 - cos(pi x/X)) / (2R) against x.
            // ponytail: X is taken so the arc is `L` long; one sample puts
            // Civil 3D's X 0.2% shorter, so its end can sit ~0.4 off on a
            // 200-long spiral. Pin the rule down with more samples.
            let curve = move |x_end: f64| {
                move |t: f64| (t - x_end / PI * (PI * t / x_end).sin()) / (2.0 * r)
            };
            let (mut lo, mut hi) = (0.5 * length, length);
            for _ in 0..60 {
                let mid = (lo + hi) / 2.0;
                if arc(&curve(mid), mid) < length { lo = mid } else { hi = mid }
            }
            let x = lo;
            (
                x,
                Box::new(move |t| (t * t / 2.0 - x * x / (PI * PI) * (1.0 - (PI * t / x).cos())) / (2.0 * r)),
                Box::new(curve(x)),
            )
        }
    };
    let turn = k.signum();
    let n = ((length / STEP).ceil() as usize).clamp(2, 20000);
    // Local points from the zero-curvature end, turning `turn`.
    let local: Vec<Vector2> = (0..=n)
        .map(|i| {
            let x = x_end * i as f64 / n as f64;
            Vector2::new(x, turn * offset(x))
        })
        .collect();
    let end_angle = turn * slope(x_end).atan();
    let place = |points: Vec<Vector2>, origin: Vector2, from: Vector2, rotate: f64| -> Vec<Vector2> {
        let (c, si) = (rotate.cos(), rotate.sin());
        points
            .into_iter()
            .map(|q| {
                let d = q - from;
                origin + Vector2::new(d.x * c - d.y * si, d.x * si + d.y * c)
            })
            .collect()
    };
    if inward {
        let points = place(local, s.point, Vector2::new(0.0, 0.0), s.heading);
        let end = State {
            point: *points.last().unwrap(),
            heading: s.heading + end_angle,
            curvature: k,
        };
        (points, end)
    } else {
        // Travelled backwards from its curved end: the same curve turning
        // the other way, reversed.
        let mirrored: Vec<Vector2> = local.iter().rev().map(|q| Vector2::new(q.x, -q.y)).collect();
        let from = mirrored[0];
        // Direction of travel at the curved end, leaving towards x = 0:
        // back along the mirrored curve's tangent there.
        let leave = std::f64::consts::PI - end_angle;
        let points = place(mirrored, s.point, from, s.heading - leave);
        let end = State {
            point: *points.last().unwrap(),
            heading: s.heading - leave + std::f64::consts::PI,
            curvature: 0.0,
        };
        (points, end)
    }
}

/// Two fixed arcs joined by a line tangent to both, each travelled in its
/// own direction: the first up to where the line leaves it, the line, then
/// the second from where the line arrives.
fn arc_line_arc(first: (Vector2, Vector2, Vector2), second: (Vector2, Vector2, Vector2)) -> Option<(Vec<Vector2>, State)> {
    let (c1, r1, ccw1) = circle(first.0, first.1, first.2)?;
    let (c2, r2, ccw2) = circle(second.0, second.1, second.2)?;
    let side = |ccw: bool| if ccw { 1.0 } else { -1.0 };
    let (s1, s2) = (side(ccw1), side(ccw2));
    // Each centre sits `s r` along the line's left normal n from its
    // tangent point, so n . (c2 - c1) = s2 r2 - s1 r1, and the line runs
    // from the first arc towards the second.
    let d = c2 - c1;
    let length = d.x.hypot(d.y);
    let u = unit(d)?;
    let c = (s2 * r2 - s1 * r1) / length;
    if c.abs() > 1.0 {
        return None;
    }
    let n = u * c + left(u) * (1.0 - c * c).sqrt();
    let leave = c1 - n * (s1 * r1);
    let arrive = c2 - n * (s2 * r2);
    let (mut q, at_leave, miss) = arc_to_point(arc_start_state(first.0, first.1, first.2), leave)?;
    (miss < TOLERANCE).then_some(())?;
    q.extend([leave, arrive]);
    let k2 = s2 / r2;
    let line_heading = heading(arrive - leave);
    let (rest, end, miss) = arc_to_point(
        State {
            point: arrive,
            heading: line_heading,
            curvature: k2,
        },
        second.2,
    )?;
    (miss < TOLERANCE && normalize(line_heading - at_leave.heading).abs() < 1e-6).then_some(())?;
    // The join repeats its point, as between any two elements.
    q.extend(rest);
    Some((q, end))
}

/// Left normal of a unit direction.
fn left(u: Vector2) -> Vector2 {
    Vector2::new(-u.y, u.x)
}

/// Arc of `radius` leaving the line `(p, u)` at or after `p` and passing
/// through `end`; the shorter of the arcs that do. Returns where it leaves
/// the line, its points and its end state.
fn arc_from_line_to_point(p: Vector2, u: Vector2, radius: f64, end: Vector2) -> Option<(Vector2, Vec<Vector2>, State)> {
    arc_leaving_line(p, u, radius, end, true)
}

/// As [`arc_from_line_to_point`], leaving the line anywhere along it.
fn arc_from_line_to_point_any(p: Vector2, u: Vector2, radius: f64, end: Vector2) -> Option<(Vector2, Vec<Vector2>, State)> {
    arc_leaving_line(p, u, radius, end, false)
}

fn arc_leaving_line(p: Vector2, u: Vector2, radius: f64, end: Vector2, after_p: bool) -> Option<(Vector2, Vec<Vector2>, State)> {
    let mut best: Option<(f64, Vector2, f64)> = None;
    for sign in [1.0, -1.0] {
        let n = left(u) * sign;
        // |p + t u + r n - end| = r
        let a = p + n * radius - end;
        let b = a.x * u.x + a.y * u.y;
        let disc = b * b - (a.x * a.x + a.y * a.y - radius * radius);
        if disc < 0.0 {
            continue;
        }
        for t in [-b - disc.sqrt(), -b + disc.sqrt()] {
            if after_p && t < -TOLERANCE {
                continue;
            }
            let touch = p + u * t;
            let center = touch + n * radius;
            let a0 = (touch.y - center.y).atan2(touch.x - center.x);
            let a1 = (end.y - center.y).atan2(end.x - center.x);
            let sweep = if sign > 0.0 { (a1 - a0).rem_euclid(TAU) } else { (a0 - a1).rem_euclid(TAU) };
            if best.is_none_or(|(s, _, _)| sweep < s) {
                best = Some((sweep, touch, sign));
            }
        }
    }
    let (sweep, touch, sign) = best?;
    let k = sign / radius;
    let (q, end) = pieces(
        State {
            point: touch,
            heading: heading(u),
            curvature: k,
        },
        &[(k, k, sweep * radius)],
    );
    Some((touch, q, end))
}

/// Arc of `radius` from the line `(p, u)` onto the circle `(center, r)`
/// travelled `ccw`, arriving along it. Of the arcs that do, the one whose
/// ends lie nearest `near_line` and `near_circle`. Returns where it leaves
/// the line, where it meets the circle, and its curvature.
fn fillet_line_circle(
    p: Vector2,
    u: Vector2,
    center: Vector2,
    r: f64,
    ccw: bool,
    radius: f64,
    near_line: Vector2,
    near_circle: Vector2,
) -> Option<(Vector2, Vector2, f64)> {
    let mut best: Option<(f64, Vector2, Vector2, f64)> = None;
    let along = |x: Vector2, c: Vector2, turn: f64| left((x - c) * (1.0 / x.distance(&c))) * turn;
    for sign in [1.0, -1.0] {
        let n = left(u) * sign;
        for d in [r + radius, (r - radius).abs()] {
            // |p + r n + t u - center| = d
            let a = p + n * radius - center;
            let b = a.x * u.x + a.y * u.y;
            let disc = b * b - (a.x * a.x + a.y * a.y - d * d);
            if disc < 0.0 || d < 1e-12 {
                continue;
            }
            for t in [-b - disc.sqrt(), -b + disc.sqrt()] {
                let fc = p + n * radius + u * t;
                let off = fc - center;
                let len = off.x.hypot(off.y);
                if len < 1e-12 {
                    continue;
                }
                let outward = off * (1.0 / len);
                // The tangent point lies on the circle, on the fillet's side
                // unless the fillet holds the circle inside it.
                let meet = if radius > r && (len - (radius - r)).abs() < 1e-6 {
                    center - outward * r
                } else {
                    center + outward * r
                };
                let fillet = along(meet, fc, sign);
                let circle = along(meet, center, if ccw { 1.0 } else { -1.0 });
                if fillet.x * circle.x + fillet.y * circle.y < 1.0 - 1e-6 {
                    continue;
                }
                let touch = p + u * t;
                let miss = touch.distance(&near_line) + meet.distance(&near_circle);
                if best.is_none_or(|(m, ..)| miss < m) {
                    best = Some((miss, touch, meet, sign / radius));
                }
            }
        }
    }
    best.map(|(_, touch, meet, k)| (touch, meet, k))
}

/// A line, an arc fitted to a fixed arc `a, m, c`, then that arc from where
/// the two meet. The points start where the fitted arc leaves the line.
fn fillet_then_arc(line: &PendingLine, radius: f64, a: Vector2, m: Vector2, c: Vector2) -> Option<(Vec<Vector2>, State)> {
    let (center, r, ccw) = circle(a, m, c)?;
    let (touch, meet, k) = fillet_line_circle(line.start, line.direction, center, r, ccw, radius, line.end, a)?;
    let (mut q, at_meet, miss) = arc_to_point(
        State {
            point: touch,
            heading: heading(line.direction),
            curvature: k,
        },
        meet,
    )?;
    (miss < TOLERANCE).then_some(())?;
    let k2 = if ccw { 1.0 / r } else { -1.0 / r };
    let (rest, end, miss) = arc_to_point(State { curvature: k2, ..at_meet }, c)?;
    (miss < TOLERANCE).then_some(())?;
    // The join repeats its point, as between any two elements.
    q.extend(rest);
    Some((q, end))
}

/// A fixed arc `a, m, c` up to where an arc of `radius` leaves it, then
/// that arc onto the line `(b, v)`.
fn arc_then_fillet(a: Vector2, m: Vector2, c: Vector2, radius: f64, b: Vector2, v: Vector2) -> Option<(Vec<Vector2>, State)> {
    let (center, r, ccw) = circle(a, m, c)?;
    // Solved backwards: from the line, against its direction, onto the
    // circle travelled the other way.
    let back = Vector2::new(-v.x, -v.y);
    let (touch, meet, k) = fillet_line_circle(b, back, center, r, !ccw, radius, b, c)?;
    let (mut q, at_meet, miss) = arc_to_point(arc_start_state(a, m, c), meet)?;
    (miss < TOLERANCE).then_some(())?;
    let (rest, end, miss) = arc_to_point(State { curvature: -k, ..at_meet }, touch)?;
    (miss < TOLERANCE).then_some(())?;
    // The join repeats its point, as between any two elements.
    q.extend(rest);
    Some((q, State { heading: heading(v), curvature: 0.0, ..end }))
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

/// A station equation: past `raw` (the station the alignment would have
/// there) stations run on from `ahead`, up or down.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StationEquation {
    pub raw: f64,
    /// Station back, as stored (derived from the equation before).
    pub back: f64,
    pub ahead: f64,
    pub increasing: bool,
}

/// A design speed from `station` on.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DesignSpeed {
    pub station: f64,
    pub speed: f64,
}

/// The station Civil 3D shows at a raw station, after the equations.
pub fn display_station(equations: &[StationEquation], raw: f64) -> f64 {
    equations
        .iter()
        .rev()
        .find(|e| raw >= e.raw)
        .map_or(raw, |e| if e.increasing { e.ahead + (raw - e.raw) } else { e.ahead - (raw - e.raw) })
}

/// The station equations and design speeds after an alignment's
/// intervals: `BL k`, `k x { '00' 0xBE 0x04, BD raw, GUID, BD back,
/// BD ahead, B increasing }`, then `BL 1, BL n`,
/// `n x { BL 1, BD station, BD speed, GUID }`. Stations lie within
/// `first..=last`.
// ponytail: found by probing for a position where both lists read; read
// the interval block before them once its layout is mapped here.
pub(super) fn equations_and_speeds(
    data: &[u8],
    start: usize,
    end: usize,
    first: f64,
    last: f64,
) -> (Vec<StationEquation>, Vec<DesignSpeed>) {
    let slack = 1e-3 * (last - first).abs().max(1.0);
    let fits = |v: f64| v.is_finite() && v >= first - slack && v <= last + slack;
    let read = |pos: usize| -> Option<(Vec<StationEquation>, Vec<DesignSpeed>)> {
        let mut bits = Bits::new(data, pos, end);
        let k = bits.bit_long()?;
        if !(0..=64).contains(&k) {
            return None;
        }
        let mut equations = Vec::new();
        for _ in 0..k {
            (bits.bits_value(2)? == 0 && bits.raw_short_byte()? == 0xBE && bits.raw_short_byte()? == 0x04).then_some(())?;
            let raw = bits.bit_double().filter(|v| fits(*v))?;
            bits.pos += 128;
            let back = bits.bit_double().filter(|v| v.is_finite() && v.abs() < 1e8)?;
            let ahead = bits.bit_double().filter(|v| v.is_finite() && v.abs() < 1e8)?;
            let increasing = bits.bit()?;
            equations.push(StationEquation {
                raw,
                back,
                ahead,
                increasing,
            });
        }
        (bits.bit_long()? == 1).then_some(())?;
        let n = bits.bit_long()?;
        if !(0..=256).contains(&n) || (k == 0 && n == 0) {
            return None;
        }
        let mut speeds = Vec::new();
        for _ in 0..n {
            (bits.bit_long()? == 1).then_some(())?;
            let station = bits.bit_double().filter(|v| fits(*v))?;
            let speed = bits.bit_double().filter(|v| v.is_normal() && *v >= 1.0 && *v < 1000.0)?;
            bits.pos += 128;
            if bits.pos > end {
                return None;
            }
            speeds.push(DesignSpeed { station, speed });
        }
        let ordered = |a: &[f64]| a.windows(2).all(|w| w[0] <= w[1]);
        (ordered(&equations.iter().map(|e| e.raw).collect::<Vec<_>>())
            && ordered(&speeds.iter().map(|s| s.station).collect::<Vec<_>>()))
        .then_some((equations, speeds))
    };
    (start..end).find_map(read).unwrap_or_default()
}

/// Bits that follow each mask range's stations.
const MASK_TAIL_BITS: usize = 257;

/// Station ranges an alignment hides (its masking): `BL n` four times, then
/// `n` x `{ BD from, BD to, 257 bits }`. Ranges lie within `first..=last`.
// ponytail: found by probing bit offsets for a list whose every range fits
// the alignment; parse the records before it once their layout is known.
pub(super) fn masked_ranges(data: &[u8], start: usize, end: usize, first: f64, last: f64) -> Vec<(f64, f64)> {
    let slack = 1e-3 * (last - first).abs().max(1.0);
    let fits = |v: f64| v.is_finite() && v >= first - slack && v <= last + slack;
    let read = |pos: usize| -> Option<Vec<(f64, f64)>> {
        let mut bits = Bits::new(data, pos, end);
        for _ in 0..3 {
            (bits.bit_long()? == 2).then_some(())?;
        }
        let n = usize::try_from(bits.bit_long()?).ok()?;
        if n == 0 || n > 64 {
            return None;
        }
        let mut out = Vec::with_capacity(n);
        // A station other than the alignment's ends is written in full; the
        // two-bit 0.0 / 1.0 forms would match random bits too easily.
        let station = |bits: &mut Bits| -> Option<f64> {
            let at = bits.pos;
            let v = bits.bit_double()?;
            let full = bits.pos - at == 66;
            let at_end = (v - first).abs() <= slack || (v - last).abs() <= slack;
            (fits(v) && (full || at_end)).then_some(v)
        };
        for _ in 0..n {
            let from = station(&mut bits)?;
            let to = station(&mut bits)?;
            (to - from > slack).then_some(())?;
            out.push((from, to));
            if bits.remaining() < MASK_TAIL_BITS {
                return None;
            }
            bits.pos += MASK_TAIL_BITS;
        }
        Some(out)
    };
    // The list sits near the end of the body; the last one that parses wins.
    (start..end).rev().find_map(read).unwrap_or_default()
}

/// End of `parts` walked from `state`, integrated without keeping points
/// (Simpson's rule on each piece): what the solvers evaluate.
fn end_of(state: State, parts: &[(f64, f64, f64)]) -> State {
    let mut s = state;
    for &(k0, k1, length) in parts {
        if length <= 1e-9 {
            continue;
        }
        let turn = k0.abs() * length + (k1 - k0).abs() * length / 2.0;
        let n = (((turn / 0.02).ceil() as usize) * 2).clamp(16, 4000);
        let h = |t: f64| s.heading + k0 * t + (k1 - k0) * t * t / (2.0 * length);
        let ds = length / n as f64;
        let (mut x, mut y) = (0.0, 0.0);
        for i in 0..=n {
            let w = if i == 0 || i == n {
                1.0
            } else if i % 2 == 1 {
                4.0
            } else {
                2.0
            };
            let a = h(i as f64 * ds);
            x += w * a.cos();
            y += w * a.sin();
        }
        s = State {
            point: s.point + Vector2::new(x, y) * (ds / 3.0),
            heading: h(length),
            curvature: k1,
        };
    }
    s
}

/// Points of an offset alignment's spiral and its end state.
#[allow(clippy::too_many_arguments)]
fn parallel_spiral(
    point: Vector2,
    d: Vector2,
    start_radius: f64,
    length: f64,
    end_radius: f64,
    clockwise: bool,
    offset: f64,
    (from, to): (f64, f64),
) -> Option<(Vec<Vector2>, State)> {
    let k_of = |r: f64| if r.is_finite() && r > 0.0 && r < 1e12 { curvature(r, clockwise) } else { 0.0 };
    let (k0, k1) = (k_of(start_radius), k_of(end_radius));
    let h = heading(unit(d)?);
    // The parent's start: the stored point, or run back from it when it is
    // the straight end of a spiral running out.
    let start = if k1 == 0.0 && k0 != 0.0 {
        let back = end_of(
            State {
                point,
                heading: h + PI,
                curvature: 0.0,
            },
            &[(0.0, -k0, length)],
        );
        State {
            point: back.point,
            heading: back.heading - PI,
            curvature: k0,
        }
    } else {
        State {
            point,
            heading: h,
            curvature: k0,
        }
    };
    let k_at = |f: f64| k0 + (k1 - k0) * f;
    let begin = end_of(start, &[(k0, k_at(from), from * length)]);
    let (q, end) = pieces(begin, &[(k_at(from), k_at(to), (to - from) * length)]);
    // Moved sideways along the normal at each point.
    let side = -offset;
    let n = q.len();
    let moved: Vec<Vector2> = (0..n)
        .map(|i| {
            let (a, b) = (q[i.saturating_sub(1)], q[(i + 1).min(n - 1)]);
            let u = unit(b - a).unwrap_or(direction(end.heading));
            q[i] + left(u) * side
        })
        .collect();
    let last = *moved.last()?;
    Some((
        moved,
        State {
            point: last,
            heading: end.heading,
            curvature: if end.curvature == 0.0 { 0.0 } else { 1.0 / (1.0 / end.curvature - side) },
        },
    ))
}

/// Where the next element starts and its heading there.
fn start_of(next: Option<&AlignmentElement>) -> Option<(Vector2, f64)> {
    match next.map(|n| &n.kind) {
        Some(AlignmentElementKind::FixedArc { start, mid, end }) => {
            let s = arc_start_state(*start, *mid, *end);
            Some((s.point, s.heading))
        }
        _ => next.and_then(line_carrier).map(|(p, u)| (p, heading(u))),
    }
}

/// A widening transition from `begin` to `to` arriving along `to_heading`:
/// `parts(sign, a, b)` lays its curves for the first curve's turn `sign`
/// and two free values; the shortest solution.
fn widening_transition(
    begin: State,
    to: Vector2,
    to_heading: f64,
    parts: impl Fn(f64, f64, f64, f64) -> Option<Vec<(f64, f64, f64)>>,
) -> Option<(Vec<Vector2>, State)> {
    let turn = normalize(to_heading - begin.heading);
    let mut best: Option<(f64, Vec<(f64, f64, f64)>)> = None;
    for sign in [1.0, -1.0] {
        let seeds: Vec<(f64, f64)> = [0.05, 0.2, 0.5, 1.0]
            .iter()
            .flat_map(|a| [1.0, 10.0, 50.0, 200.0].map(|b| (*a, b)))
            .collect();
        for (a, b) in newton2(
            |a, b| {
                let end = end_of(begin, &parts(sign, turn, a, b)?);
                Some((end.point.x - to.x, end.point.y - to.y))
            },
            &seeds,
        ) {
            let Some(laid) = parts(sign, turn, a, b) else { continue };
            if laid.iter().any(|p| p.2 < -1e-9) {
                continue;
            }
            let length: f64 = laid.iter().map(|p| p.2).sum();
            if best.as_ref().is_none_or(|(l, _)| length < *l) {
                best = Some((length, laid));
            }
        }
    }
    let (_, laid) = best?;
    Some(pieces(begin, &laid))
}

/// Pieces laid along the pending line's direction and slid along that line
/// until they end on the next line; the pending line stops where they
/// start, and the next line starts where they end.
fn slide_between_lines(
    points: &mut Vec<Vector2>,
    pending: &mut Option<PendingLine>,
    next: Option<&AlignmentElement>,
    parts_for: impl Fn(f64) -> Vec<(f64, f64, f64)>,
) -> Option<(Vec<Vector2>, State)> {
    let line = pending.as_ref()?;
    let (b, v) = next.and_then(line_carrier)?;
    let u = line.direction;
    let den = cross(u, v);
    if den.abs() < 1e-12 {
        return None;
    }
    let parts = parts_for(normalize(heading(v) - heading(u)));
    let begin = State {
        point: line.start,
        heading: heading(u),
        curvature: parts.first()?.0,
    };
    let (q, end) = pieces(begin, &parts);
    let shift = u * (-cross(end.point - b, v) / den);
    close(points, pending, line.start + shift);
    Some((
        q.into_iter().map(|p| p + shift).collect(),
        State {
            point: end.point + shift,
            ..end
        },
    ))
}

/// Where the pending line, or a straight end state, points from.
fn line_from(pending: &Option<PendingLine>, state: Option<State>) -> Option<(Vector2, Vector2)> {
    match pending.as_ref() {
        Some(line) => Some((line.start, line.direction)),
        None => state
            .filter(|s| s.curvature.abs() < 1e-12)
            .map(|s| (s.point, direction(s.heading))),
    }
}

/// Spiral from the line `(p, u)` into an arc of `radius` that ends at
/// `end`, slid along the line until the arc reaches `end`; of the
/// positions that do, the one with the shorter arc. Returns where the
/// spiral leaves the line.
fn spiral_curve_to(p: Vector2, u: Vector2, spiral: f64, radius: f64, end: Vector2) -> Option<(Vector2, Vec<Vector2>, State)> {
    let mut best: Option<(f64, Vector2, Vec<Vector2>, State)> = None;
    for sign in [1.0, -1.0] {
        let k = sign / radius;
        let begin = State {
            point: p,
            heading: heading(u),
            curvature: 0.0,
        };
        let (q, sc) = pieces(begin, &[(0.0, k, spiral)]);
        let center = sc.point + left(direction(sc.heading)) * (1.0 / k);
        let w = end - center;
        let along = w.dot(&u);
        let disc = along * along - (w.dot(&w) - radius * radius);
        if disc < 0.0 {
            continue;
        }
        for t in [along - disc.sqrt(), along + disc.sqrt()] {
            let shift = u * t;
            let at = State {
                point: sc.point + shift,
                ..sc
            };
            let Some((arc, arc_end, miss)) = arc_to_point(at, end) else {
                continue;
            };
            let length = arc.len() as f64;
            if miss < TOLERANCE && best.as_ref().is_none_or(|b| length < b.0) {
                let mut all: Vec<Vector2> = q.iter().map(|x| *x + shift).collect();
                all.extend(arc);
                best = Some((length, p + shift, all, arc_end));
            }
        }
    }
    best.map(|(_, start, q, end)| (start, q, end))
}

/// Fillet between the lines `(p, u)` and `(b, v)` through `x`: the radius
/// of the circle tangent to both whose arc between them passes `x`.
fn fillet_radius_through(p: Vector2, u: Vector2, b: Vector2, v: Vector2, x: Vector2) -> Option<f64> {
    let pi = intersect(p, u, b, v)?;
    let bisector = unit(v - u)?;
    let delta = cross(u, v).atan2(u.dot(&v)).abs();
    let (sh, ch) = ((delta / 2.0).sin(), (delta / 2.0).cos());
    let w = x - pi;
    let (qa, qb, qc) = (sh * sh, -2.0 * bisector.dot(&w), w.dot(&w));
    let disc = qb * qb - 4.0 * qa * qc;
    if qa <= 1e-15 || disc < 0.0 {
        return None;
    }
    [(-qb - disc.sqrt()) / (2.0 * qa), (-qb + disc.sqrt()) / (2.0 * qa)]
        .into_iter()
        .filter(|k| *k > 0.0)
        .find(|k| {
            // `x` on the arc: on the PI's side of the chord between the
            // tangent points.
            let t = k * sh;
            let (t1, t2) = (pi - u * t, pi + v * t);
            cross(t2 - t1, x - t1) * cross(t2 - t1, pi - t1) >= 0.0
        })
        .map(|k| k * ch)
}

/// Arc leaving the line `(p, u)` and ending at `end` along `d`: where it
/// leaves the line and its state there.
fn arc_to_direction(p: Vector2, u: Vector2, end: Vector2, d: Vector2) -> Option<(Vector2, State)> {
    let offset = cross(u, end - p);
    let den = 1.0 - u.dot(&d);
    if offset.abs() < 1e-12 || den.abs() < 1e-12 {
        return None;
    }
    let sign = offset.signum();
    let radius = offset / (sign * den);
    if !(radius > 0.0) {
        return None;
    }
    let center = end + left(d) * (sign * radius);
    let touch = p + u * (center - p).dot(&u);
    Some((
        touch,
        State {
            point: touch,
            heading: heading(u),
            curvature: sign / radius,
        },
    ))
}

/// Circle of a fixed arc: centre, signed curvature, angle of its end.
fn arc_circle(a: Vector2, m: Vector2, c: Vector2) -> Option<(Vector2, f64, f64)> {
    let (center, radius, ccw) = circle(a, m, c)?;
    let k = if ccw { 1.0 / radius } else { -1.0 / radius };
    Some((center, k, (c.y - center.y).atan2(c.x - center.x)))
}

/// State on the circle `(center, k)` at `angle`.
fn on_circle(center: Vector2, k: f64, angle: f64) -> State {
    State {
        point: center + direction(angle) * (1.0 / k.abs()),
        heading: angle + if k > 0.0 { PI / 2.0 } else { -PI / 2.0 },
        curvature: k,
    }
}

/// Centre of the circle a state is on.
fn center_of(s: State) -> Vector2 {
    s.point + left(direction(s.heading)) * (1.0 / s.curvature)
}

/// Roots of two equations in two unknowns by Newton's method from each
/// seed.
fn newton2(f: impl Fn(f64, f64) -> Option<(f64, f64)>, seeds: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut roots: Vec<(f64, f64)> = Vec::new();
    for &(mut x, mut y) in seeds {
        for _ in 0..60 {
            let Some((f0, g0)) = f(x, y) else { break };
            let (hx, hy) = (1e-7 * (1.0 + x.abs()), 1e-7 * (1.0 + y.abs()));
            let (Some((fx, gx)), Some((fy, gy))) = (f(x + hx, y), f(x, y + hy)) else {
                break;
            };
            let (a, b, c, d) = ((fx - f0) / hx, (fy - f0) / hy, (gx - g0) / hx, (gy - g0) / hy);
            let det = a * d - b * c;
            if det.abs() < 1e-14 {
                break;
            }
            let (dx, dy) = ((d * f0 - b * g0) / det, (a * g0 - c * f0) / det);
            x -= dx;
            y -= dy;
            if !(x.is_finite() && y.is_finite()) {
                break;
            }
            if dx.abs() < 1e-12 * (1.0 + x.abs()) && dy.abs() < 1e-12 * (1.0 + y.abs()) {
                break;
            }
        }
        if f(x, y).is_some_and(|(f, g)| f.hypot(g) < 1e-6)
            && !roots.iter().any(|r| (r.0 - x).abs() < 1e-6 && (r.1 - y).abs() < 1e-6)
        {
            roots.push((x, y));
        }
    }
    roots
}

/// A group between two fixed arcs: it leaves the first circle at an
/// angle solved together with one free value, and arrives on the second
/// circle. `parts(k1, k2, value)` lays the group's pieces. Of the
/// solutions, the one leaving nearest the first arc's stored end. Returns
/// the departure angle and the solved pieces.
fn between_circles(
    first: (Vector2, Vector2, Vector2),
    second: (Vector2, Vector2, Vector2),
    parts: impl Fn(f64, f64, f64) -> Option<Vec<(f64, f64, f64)>>,
    values: &[f64],
) -> Option<(State, Vec<Vector2>, State)> {
    let (c1, k1, end_angle) = arc_circle(first.0, first.1, first.2)?;
    let (c2, k2, _) = arc_circle(second.0, second.1, second.2)?;
    let lay = |angle: f64, value: f64| -> Option<(State, Vec<Vector2>, State)> {
        let from = on_circle(c1, k1, angle);
        let (q, end) = pieces(from, &parts(k1, k2, value)?);
        Some((from, q, end))
    };
    let residual = |angle: f64, value: f64| {
        let parts = parts(k1, k2, value)?;
        // No spiral group turns more than twice round.
        if parts.iter().map(|(a, b, l)| a.abs().max(b.abs()) * l).sum::<f64>() > 4.0 * TAU {
            return None;
        }
        let end = end_of(on_circle(c1, k1, angle), &parts);
        let miss = center_of(end) - c2;
        Some((miss.x, miss.y))
    };
    let seeds: Vec<(f64, f64)> = (-12..=12)
        .flat_map(|i| values.iter().map(move |v| (end_angle + 0.1 * f64::from(i), *v)))
        .collect();
    newton2(residual, &seeds)
        .into_iter()
        .filter(|(angle, value)| parts(k1, k2, *value).is_some_and(|p| p.iter().all(|x| x.2 >= -1e-9)) && angle.is_finite())
        .min_by(|a, b| normalize(a.0 - end_angle).abs().total_cmp(&normalize(b.0 - end_angle).abs()))
        .and_then(|(angle, value)| lay(angle, value))
}

/// Spiral out of a fixed arc and a reverse spiral into a curve through
/// `through` (or of `radius`) ending at `end`: the departure on the arc
/// (and the radius) solved; of the solutions, the shortest curve to `end`.
fn reverse_from_circle(
    arc: (Vector2, Vector2, Vector2),
    spiral_out: f64,
    spiral_in: f64,
    radius: Option<f64>,
    through: Vector2,
    end: Vector2,
) -> Option<(State, Vec<Vector2>, State)> {
    let (c1, k1, end_angle) = arc_circle(arc.0, arc.1, arc.2)?;
    let parts = |r: f64| [(k1, 0.0, spiral_out), (0.0, -k1.signum() / r, spiral_in)];
    let lay = |angle: f64, r: f64| -> Option<(State, Vec<Vector2>, State)> {
        if !(r > 0.0) {
            return None;
        }
        let from = on_circle(c1, k1, angle);
        let (q, sc) = pieces(from, &parts(r));
        Some((from, q, sc))
    };
    let sc_at = |angle: f64, r: f64| (r > 0.0).then(|| end_of(on_circle(c1, k1, angle), &parts(r)));
    let seeds: Vec<(f64, f64)> = (-30..=30)
        .flat_map(|i| {
            [10.0, 50.0, 200.0, 1000.0, 5000.0]
                .into_iter()
                .map(move |r| (end_angle + 0.1 * f64::from(i), radius.unwrap_or(r)))
        })
        .collect();
    let roots = match radius {
        // One unknown: the second equation pins the radius.
        Some(r) => newton2(
            |angle, rr| {
                let sc = sc_at(angle, r)?;
                Some((end.distance(&center_of(sc)) - r, rr - r))
            },
            &seeds,
        ),
        None => newton2(
            |angle, r| {
                let c = center_of(sc_at(angle, r)?);
                Some((end.distance(&c) - r, through.distance(&c) - r))
            },
            &seeds,
        ),
    };
    roots
        .into_iter()
        .filter_map(|(angle, r)| {
            let (from, mut q, sc) = lay(angle, r)?;
            let (arc, arc_end, miss) = arc_to_point(sc, end)?;
            (miss < TOLERANCE).then(|| {
                let length = arc.len();
                q.extend(arc);
                (length, from, q, arc_end)
            })
        })
        .min_by_key(|(length, ..)| *length)
        .map(|(_, from, q, end)| (from, q, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(x: f64, y: f64) -> Vector2 {
        Vector2::new(x, y)
    }

    #[test]
    fn arc_leaves_line_and_reaches_point() {
        // Line along +x; a quarter circle of radius 10 from (5, 0) ends at (15, 10).
        let (touch, q, end) = arc_from_line_to_point(v(0.0, 0.0), v(1.0, 0.0), 10.0, v(15.0, 10.0)).unwrap();
        assert!(touch.distance(&v(5.0, 0.0)) < 1e-9);
        assert!(q.last().unwrap().distance(&v(15.0, 10.0)) < TOLERANCE);
        assert!((end.heading - PI / 2.0).abs() < 1e-6);
    }

    #[test]
    fn japanese_cubic_ends_where_its_offset_says() {
        // X + X^3 / (40 R^2) = L, y(X) = X^2 / (6 R), slope X / (2 R).
        let (r, length) = (200.0, 200.0);
        let start = State { point: v(0.0, 0.0), heading: 0.0, curvature: 0.0 };
        let (q, end) = spiral(start, 1.0 / r, true, length, SpiralShape::JapaneseCubic);
        let x: f64 = 195.341_3;
        assert!((x + x.powi(3) / (40.0 * r * r) - length).abs() < 1e-3);
        assert!(q.last().unwrap().distance(&v(x, x * x / (6.0 * r))) < 1e-3);
        assert!((end.heading - (x / (2.0 * r)).atan()).abs() < 1e-6);
    }

    #[test]
    fn outward_spiral_retraces_the_inward_one() {
        // Leaving the inward spiral's curved end backwards, with its
        // curvature reversed, an outward spiral of the same shape comes
        // back to where the inward one began.
        for shape in [
            SpiralShape::Clothoid,
            SpiralShape::Bloss,
            SpiralShape::Sinusoidal,
            SpiralShape::Biquadratic,
            SpiralShape::JapaneseCubic,
            SpiralShape::CubicParabola,
            SpiralShape::SineHalfWave,
        ] {
            let start = State { point: v(10.0, 20.0), heading: 0.3, curvature: 0.0 };
            let (_, end) = spiral(start, -1.0 / 150.0, true, 120.0, shape);
            let back = State { heading: end.heading + PI, curvature: 1.0 / 150.0, ..end };
            let (_, home) = spiral(back, 1.0 / 150.0, false, 120.0, shape);
            assert!(home.point.distance(&start.point) < 0.05, "{shape:?}: {:?}", home.point);
            assert!(normalize(home.heading - (start.heading + PI)).abs() < 1e-3, "{shape:?}");
        }
    }

    #[test]
    fn fillet_joins_line_to_fixed_arc() {
        // Line along +x on y = 0; a fixed arc clockwise around (0, 20),
        // radius 10, from its bottom over the left to its top. A fillet of
        // radius 5 around (0, 5) leaves the line at the origin and meets
        // the arc at its bottom.
        let line = PendingLine {
            start: v(-100.0, 0.0),
            direction: v(1.0, 0.0),
            end: v(-1.0, 0.0),
        };
        let (q, end) = fillet_then_arc(&line, 5.0, v(0.0, 10.0), v(-10.0, 20.0), v(0.0, 30.0)).unwrap();
        assert!(q[0].distance(&v(0.0, 0.0)) < 1e-9);
        assert!(q.iter().any(|p| p.distance(&v(0.0, 10.0)) < TOLERANCE));
        assert!(q.iter().any(|p| p.distance(&v(-10.0, 20.0)) < 0.1));
        assert!(end.point.distance(&v(0.0, 30.0)) < TOLERANCE);
    }

    #[test]
    fn fixed_arc_hands_over_to_fillet_and_line() {
        // The same join travelled backwards: the arc from its top over the
        // left to its bottom, the fillet, then the line along -x.
        let (q, end) = arc_then_fillet(v(0.0, 30.0), v(-10.0, 20.0), v(0.0, 10.0), 5.0, v(-1.0, 0.0), v(-1.0, 0.0)).unwrap();
        assert!(q[0].distance(&v(0.0, 30.0)) < 1e-9);
        assert!(end.point.distance(&v(0.0, 0.0)) < TOLERANCE);
        assert!((normalize(end.heading - PI)).abs() < 1e-9);
    }
}
