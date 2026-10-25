//! AutoCAD alignment-A linetype fitting.
//!
//! The pattern is stretched so a whole number of repeats covers the polyline
//! and a dash lands on each end.

use crate::types::Vector3;

/// Signed pattern element. Positive is a dash, negative a gap, zero a dot.
pub(crate) fn fit_runs(length: f64, elements: &[f64]) -> Vec<Run> {
    if length <= 1e-9 {
        return Vec::new();
    }
    let period: f64 = elements.iter().map(|element| element.abs()).sum();
    if elements.is_empty() || period <= 1e-12 {
        return vec![Run::Dash {
            start: 0.0,
            end: length,
        }];
    }
    let repeats = (length / period).round().max(1.0);
    let stretch = length / (repeats * period);
    let mut runs = Vec::new();
    let mut cursor = 0.0;
    for _ in 0..repeats as usize {
        for element in elements {
            let span = element.abs() * stretch;
            if *element > 0.0 {
                runs.push(Run::Dash {
                    start: cursor,
                    end: cursor + span,
                });
            } else if *element == 0.0 {
                runs.push(Run::Dot { at: cursor });
            }
            cursor += span;
        }
    }
    runs
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Run {
    Dash { start: f64, end: f64 },
    Dot { at: f64 },
}

/// Slice fitted runs back onto a polyline of `lengths` that sum to the
/// length the runs were fitted against.
pub(crate) fn runs_on_segments(points: &[Vector3], runs: &[Run]) -> Vec<SegmentPiece> {
    let mut pieces = Vec::new();
    if points.len() < 2 {
        return pieces;
    }
    let mut origin = 0.0;
    let mut cursor = 0usize;
    for pair in points.windows(2) {
        let start = pair[0];
        let end = pair[1];
        let delta = end - start;
        let length = delta.length();
        if length <= 1e-12 {
            origin += length;
            continue;
        }
        let seg_end = origin + length;
        while let Some(run) = runs.get(cursor) {
            let before = match *run {
                Run::Dash { end, .. } => end <= origin,
                Run::Dot { at } => at < origin - 1e-9,
            };
            if !before {
                break;
            }
            cursor += 1;
        }
        for run in &runs[cursor..] {
            let run_start = match *run {
                Run::Dash { start, .. } => start,
                Run::Dot { at } => at,
            };
            if run_start > seg_end + 1e-9 {
                break;
            }
            match *run {
                Run::Dash { start: a, end: b } => {
                    let from = a.max(origin);
                    let to = b.min(seg_end);
                    if to - from > 1e-9 {
                        pieces.push(SegmentPiece::Dash {
                            start: point_at(start, delta, (from - origin) / length),
                            end: point_at(start, delta, (to - origin) / length),
                        });
                    }
                }
                Run::Dot { at } => {
                    if (at - origin) >= -1e-9 && at < seg_end + 1e-9 {
                        let point =
                            point_at(start, delta, ((at - origin) / length).clamp(0.0, 1.0));
                        pieces.push(SegmentPiece::Dot(point));
                    }
                }
            }
        }
        origin = seg_end;
    }
    pieces
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum SegmentPiece {
    Dash { start: Vector3, end: Vector3 },
    Dot(Vector3),
}

fn point_at(start: Vector3, delta: Vector3, t: f64) -> Vector3 {
    start + delta * t
}

pub(crate) fn polyline_length(points: &[Vector3]) -> f64 {
    points
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).length())
        .sum()
}
