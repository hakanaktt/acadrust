//! Convex polygon clipping for insert boundaries (XCLIP).

use crate::types::{Vector2, Vector3};

pub(crate) fn rectangle(min: Vector2, max: Vector2) -> Vec<Vector2> {
    let min_x = min.x.min(max.x);
    let max_x = min.x.max(max.x);
    let min_y = min.y.min(max.y);
    let max_y = min.y.max(max.y);
    vec![
        Vector2::new(min_x, min_y),
        Vector2::new(max_x, min_y),
        Vector2::new(max_x, max_y),
        Vector2::new(min_x, max_y),
    ]
}

pub(crate) fn clip_segment(
    start: Vector3,
    end: Vector3,
    polygon: &[Vector2],
) -> Option<(Vector3, Vector3)> {
    if polygon.len() < 3 {
        return Some((start, end));
    }
    let mut from = start;
    let mut to = end;
    for index in 0..polygon.len() {
        let edge_start = polygon[index];
        let edge_end = polygon[(index + 1) % polygon.len()];
        let from_inside = inside(from, edge_start, edge_end);
        let to_inside = inside(to, edge_start, edge_end);
        match (from_inside, to_inside) {
            (true, true) => {}
            (true, false) => to = intersection(from, to, edge_start, edge_end),
            (false, true) => from = intersection(from, to, edge_start, edge_end),
            (false, false) => return None,
        }
    }
    if (to - from).length_squared() <= 1e-18 {
        None
    } else {
        Some((from, to))
    }
}

pub(crate) fn clip_polygon(subject: &[Vector3], polygon: &[Vector2]) -> Vec<Vector3> {
    if polygon.len() < 3 || subject.len() < 3 {
        return subject.to_vec();
    }
    let mut output = subject.to_vec();
    for index in 0..polygon.len() {
        if output.is_empty() {
            return output;
        }
        let edge_start = polygon[index];
        let edge_end = polygon[(index + 1) % polygon.len()];
        let input = output;
        output = Vec::new();
        let mut previous = *input.last().unwrap();
        for &current in &input {
            let previous_inside = inside(previous, edge_start, edge_end);
            let current_inside = inside(current, edge_start, edge_end);
            if current_inside {
                if !previous_inside {
                    output.push(intersection(previous, current, edge_start, edge_end));
                }
                output.push(current);
            } else if previous_inside {
                output.push(intersection(previous, current, edge_start, edge_end));
            }
            previous = current;
        }
    }
    output
}

fn inside(point: Vector3, start: Vector2, end: Vector2) -> bool {
    let cross = (end.x - start.x) * (point.y - start.y) - (end.y - start.y) * (point.x - start.x);
    cross >= -1e-8
}

fn intersection(start: Vector3, end: Vector3, edge_start: Vector2, edge_end: Vector2) -> Vector3 {
    let segment = end - start;
    let edge = Vector2::new(edge_end.x - edge_start.x, edge_end.y - edge_start.y);
    let denom = segment.x * edge.y - segment.y * edge.x;
    let t = if denom.abs() <= 1e-12 {
        0.0
    } else {
        ((edge_start.x - start.x) * edge.y - (edge_start.y - start.y) * edge.x) / denom
    };
    let t = t.clamp(0.0, 1.0);
    start + segment * t
}
