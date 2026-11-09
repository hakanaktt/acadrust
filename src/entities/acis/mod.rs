//! ACIS/SAT solid modeler data parser and writer.
//!
//! This module provides support for parsing and generating ACIS SAT (Save And
//! Restore) format data, which is the text-based geometry representation used
//! by 3DSOLID, BODY, and REGION entities in DXF/DWG files.
//!
//! # Supported Versions
//!
//! - **ACIS 1.5–6.0** (SAT versions 400–600): Legacy format without explicit
//!   record indices.
//! - **ACIS 7.0** (SAT version 700): Adds explicit negative record indices,
//!   `asmheader` entity, and `@`-prefixed counted strings.
//! - **ACIS 21.0+** (SAT version 21800): Modern format with extended entity types.
//!
//! # Format Overview
//!
//! A SAT file consists of:
//! 1. A **header** (3–4 lines): version, product info, tolerances
//! 2. **Entity records**: each terminated by `#`, representing the B-rep
//!    topology (body → lump → shell → face → loop → coedge → edge → vertex)
//!    and associated geometry (surfaces, curves, points, transforms).
//!
//! # Example
//!
//! ```rust
//! use acadrust::entities::acis::{SatDocument, SatVersion};
//!
//! // Parse SAT text from a 3DSOLID entity
//! let sat_text = "700 0 1 0\n\
//!     @12 Spatial Corp @7 ACIS 7.0 @24 Thu Jan 01 00:00:00 2023\n\
//!     1e-06 9.9999999999999995e-07\n\
//!     -0 asmheader $-1 -1 @12 700 7 0 0 @5 ACIS @3 7.0 @24 Thu Jan 01 00:00:00 2023 #\n\
//!     -1 body $-1 $-1 $-1 $-1 #\n\
//!     End-of-ACIS-data\n";
//!
//! let doc = SatDocument::parse(sat_text).unwrap();
//! assert_eq!(doc.header.version, SatVersion::new(7, 0, 0));
//! assert_eq!(doc.records.len(), 2);
//!
//! // Write back to SAT text
//! let output = doc.to_sat_string();
//! assert!(output.contains("700"));
//! ```

pub mod parser;
pub mod primitives;
pub mod sab;
pub mod types;
pub mod writer;

pub use parser::SatParser;
pub use sab::{SabReader, SabWriter};
pub use types::*;
pub use writer::{SatWriter, SplineNet};

/// Downgrade ACIS v600+ record token layouts to v400 format.
///
/// ACIS 6.0 added extra fields to some entity types (most notably `edge`).
/// When downgrading SAT output for DXF compatibility, these extra fields
/// must be stripped.
///
/// Detection is based on actual token count rather than version header,
/// because some SAT data has a v400 header but v600 record layouts
/// (e.g., "ACIS 6.00" builder with version=400 header).
///
/// **edge** layout change (after sentinel normalization):
/// - v400 (6 tokens): `$sentinel $sv $ev $coedge $curve sense`
/// - v600 (9 tokens): `$sentinel $sv start_param $ev end_param $coedge $curve sense "unknown"`
///
/// The v600 "unknown" string becomes `7 unknown` in legacy SAT (counted string),
/// which BricsCAD's v400 parser misinterprets as extra tokens.
pub fn downgrade_records_to_v400(records: &mut Vec<SatRecord>) {
    for record in records.iter_mut() {
        if record.entity_type == "edge" {
            // v600 edge tokens (after sentinel normalization):
            //   [0] sentinel $-1
            //   [1] $sv
            //   [2] start_param (Float)      ← remove
            //   [3] $ev
            //   [4] end_param (Float)        ← remove
            //   [5] $coedge
            //   [6] $curve
            //   [7] sense (Enum)
            //   [8] "unknown" (String)       ← remove
            // Result: [0]sentinel [1]$sv [2]$ev [3]$coedge [4]$curve [5]sense
            let len = record.tokens.len();
            if len >= 9 {
                // Remove from end first to preserve indices
                record.tokens.truncate(len - 1); // remove "unknown" string
                record.tokens.remove(4); // remove end_param (was index 4)
                record.tokens.remove(2); // remove start_param (was index 2)
            }
        }
    }
}

/// Upgrade a pre-7.0 document to the ACIS 7.0 record layout, the inverse of
/// [`downgrade_records_to_v400`].
///
/// The parsers already give pre-7.0 records the 7.0 token slots: the pattern
/// pointer, and zero placeholders for the edge vertex parameters 4.0 does
/// not store. A 7.0 reader still needs:
/// - the version itself: a 4.0 header tells it not to expect the pattern
///   pointer and entity id every record now carries;
/// - real edge parameters: a 4.0 reader projects the vertices onto the
///   curve, a 7.0 one reads them, in the edge's own sense;
/// - the convexity string a 7.0 edge ends with.
pub fn upgrade_records_from_v400(doc: &mut SatDocument) {
    if doc.header.version.major >= 7 {
        return;
    }
    let params: Vec<Option<(f64, f64)>> = doc
        .records
        .iter()
        .map(|record| v400_edge_params(doc, record))
        .collect();
    for (record, params) in doc.records.iter_mut().zip(params) {
        if base_entity_type(&record.entity_type) != "edge" {
            continue;
        }
        if let Some((start, end)) = params {
            record.tokens[2] = SatToken::Float(start);
            record.tokens[4] = SatToken::Float(end);
        }
        if record.tokens.len() == 8 {
            record.tokens.push(SatToken::String("unknown".to_string()));
        }
        record.raw_text = None;
    }
    doc.header.version = SatVersion::V7_0;
}

/// Parameters of an edge's vertices on its curve, in the edge's sense, for
/// an edge whose parameter slots are still the parser's zero placeholders.
///
/// ponytail: straight, ellipse and intcurve curves only; any other curve
/// keeps the zeros, project onto it here when one shows up.
fn v400_edge_params(doc: &SatDocument, record: &SatRecord) -> Option<(f64, f64)> {
    let edge = SatEdge::from_record(record)?;
    if record.tokens.len() < 8 || edge.start_param() != 0.0 || edge.end_param() != 0.0 {
        return None;
    }
    let point = |vertex: SatPointer| -> Option<[f64; 3]> {
        let vertex = SatVertex::from_record(doc.resolve(vertex)?)?;
        let (x, y, z) = SatPoint::from_record(doc.resolve(vertex.point())?)?.position();
        Some([x, y, z])
    };
    let start = point(edge.start_vertex())?;
    let end = point(edge.end_vertex())?;
    let closed = edge.start_vertex() == edge.end_vertex();
    let curve = doc.resolve(edge.curve())?;
    // A reversed edge stores the negated curve parameters.
    let sign = if edge.sense() == Sense::Reversed {
        -1.0
    } else {
        1.0
    };
    let sub = |a: [f64; 3], b: (f64, f64, f64)| [a[0] - b.0, a[1] - b.1, a[2] - b.2];
    let dot = |a: [f64; 3], b: (f64, f64, f64)| a[0] * b.0 + a[1] * b.1 + a[2] * b.2;

    if let Some(line) = SatStraightCurve::from_record(curve) {
        let (root, direction) = (line.root_point(), line.direction());
        let length = dot([direction.0, direction.1, direction.2], direction);
        let t = |p: [f64; 3]| dot(sub(p, root), direction) / length;
        return Some((sign * t(start), sign * t(end)));
    }

    if let Some(ellipse) = SatEllipseCurve::from_record(curve) {
        // p = center + major cos t + ratio (normal x major) sin t
        let (center, (nx, ny, nz), major) =
            (ellipse.center(), ellipse.normal(), ellipse.major_axis());
        let norm = (nx * nx + ny * ny + nz * nz).sqrt();
        let (nx, ny, nz) = (nx / norm, ny / norm, nz / norm);
        let minor = (
            ny * major.2 - nz * major.1,
            nz * major.0 - nx * major.2,
            nx * major.1 - ny * major.0,
        );
        let ratio = ellipse.ratio();
        let angle = |p: [f64; 3]| {
            let q = sub(p, center);
            (dot(q, minor) / ratio).atan2(dot(q, major))
        };
        let from = sign * angle(start);
        let mut to = sign * angle(end);
        if closed || to <= from {
            to += std::f64::consts::TAU;
        }
        return Some((from, to));
    }

    let spline = SatIntCurve::from_record(curve)?;
    let (degree, knots, _) = spline.bspline()?;
    let (lo, hi) = (knots[degree], knots[knots.len() - degree - 1]);
    // An intcurve that runs against its spline negates the spline parameter.
    let reversed = curve
        .tokens
        .iter()
        .take_while(|token| token.as_ident() != Some("{"))
        .any(|token| token.as_ident() == Some("reversed"));
    let sign = if reversed { -sign } else { sign };
    if closed {
        return Some(if sign < 0.0 { (-hi, -lo) } else { (lo, hi) });
    }
    let nearest = |p: [f64; 3]| -> Option<f64> {
        const SEGS: usize = 32;
        let (mut a, mut b, mut best) = (lo, hi, lo);
        for _ in 0..8 {
            let step = (b - a) / SEGS as f64;
            let samples = spline.sample_range(a, b, SEGS);
            let index = (0..samples.len()).min_by(|&i, &j| {
                let distance = |k: usize| {
                    let (x, y, z) = samples[k];
                    let q = sub(p, (x, y, z));
                    q[0] * q[0] + q[1] * q[1] + q[2] * q[2]
                };
                distance(i).total_cmp(&distance(j))
            })?;
            best = a + step * index as f64;
            (a, b) = ((best - step).max(lo), (best + step).min(hi));
        }
        Some(best)
    };
    Some((sign * nearest(start)?, sign * nearest(end)?))
}
