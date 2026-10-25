use super::*;
use crate::tables::DimStyle;

pub(super) fn format_dimension_value(dim: &Dimension, style: &DimStyle) -> String {
    let is_angular = dim.base().is_angular();
    let mut value = dim.measurement();
    if !is_angular {
        value *= style.dimlfac;
    }
    if style.dimrnd.is_finite() && style.dimrnd > 0.0 {
        value = (value / style.dimrnd).round() * style.dimrnd;
    }
    let decimals = if is_angular {
        style.dimadec.clamp(0, 12) as usize
    } else {
        style.dimdec.clamp(0, 12) as usize
    };
    let mut text = format!("{value:.decimals$}");
    if style.dimdsep == 44 {
        text = text.replace('.', ",");
    }
    let zero_flags = if is_angular {
        style.dimazin
    } else {
        style.dimzin
    };
    if zero_flags & 1 != 0 {
        if let Some(index) = text.find("0.") {
            if index == 0 {
                text.remove(0);
            } else if index == 1 && text.starts_with('-') {
                text.remove(1);
            }
        }
        if let Some(index) = text.find("0,") {
            if index == 0 {
                text.remove(0);
            } else if index == 1 && text.starts_with('-') {
                text.remove(1);
            }
        }
    }
    if zero_flags & 2 != 0 {
        let separator = if style.dimdsep == 44 { ',' } else { '.' };
        if let Some(index) = text.find(separator) {
            let trimmed = text.trim_end_matches('0');
            text = trimmed.trim_end_matches(separator).to_owned();
            if text.is_empty() || text == "-" {
                text = "0".to_owned();
            } else if index >= text.len() && text.ends_with(separator) {
                text.pop();
            }
        }
    }
    let postfix = &style.dimpost;
    if postfix.is_empty() {
        return text;
    }
    if postfix.contains("<>") {
        postfix.replace("<>", &text)
    } else {
        format!("{text}{postfix}")
    }
}

pub(super) fn dimension_text_value(dim: &Dimension, style: Option<&DimStyle>) -> Option<String> {
    let base = dim.base();
    let Some(override_text) = base.text_override() else {
        return Some(
            style
                .map(|style| format_dimension_value(dim, style))
                .unwrap_or_else(|| format!("{:.4}", dim.measurement())),
        );
    };
    // A single space is ACAD's documented text suppression convention.
    if override_text == " " {
        return None;
    }
    let formatted = style
        .map(|style| format_dimension_value(dim, style))
        .unwrap_or_else(|| format!("{:.4}", dim.measurement()));
    Some(override_text.replace("<>", &formatted))
}

pub(super) fn dimension_extension_points(
    measured: Vector3,
    projected: Vector3,
    style: &DimStyle,
) -> (Vector3, Vector3) {
    let delta = projected - measured;
    let direction = if delta.length_squared() > 1.0e-20 {
        delta.normalize()
    } else {
        Vector3::ZERO
    };
    let scale = if style.dimscale.is_finite() {
        style.dimscale.abs()
    } else {
        1.0
    };
    let start = measured + direction * (style.dimexo.max(0.0) * scale);
    let end = if style.dimfxlon {
        start + direction * (style.dimfxl.max(0.0) * scale)
    } else {
        projected + direction * (style.dimexe.max(0.0) * scale)
    };
    (start, end)
}

pub(super) fn styled_dimension_line_segments(
    first: Vector3,
    second: Vector3,
    text_middle: Vector3,
    arrow_size: f64,
    gap: f64,
) -> Vec<(Vector3, Vector3)> {
    let axis = second - first;
    let length = axis.length();
    if length <= 1.0e-20 {
        return Vec::new();
    }
    let direction = axis / length;
    let projection = (text_middle - first).dot(&direction);
    let first_limit = first + direction * arrow_size.min(length * 0.5);
    let second_limit = second - direction * arrow_size.min(length * 0.5);
    let mut result = Vec::new();
    if projection > 0.0 && projection < length {
        let center = first + direction * projection;
        let half_gap = gap.max(0.0);
        let left = center - direction * half_gap;
        let right = center + direction * half_gap;
        if first_limit.distance(&left) > 1.0e-12 {
            result.push((first_limit, left));
        }
        if right.distance(&second_limit) > 1.0e-12 {
            result.push((right, second_limit));
        }
    } else if projection >= length {
        if first_limit.distance(&second_limit) > 1.0e-12 {
            result.push((first_limit, second_limit));
        }
        let outside_end = second + direction * arrow_size;
        if second.distance(&outside_end) > 1.0e-12 {
            result.push((second, outside_end));
        }
    } else {
        if first_limit.distance(&second_limit) > 1.0e-12 {
            result.push((first_limit, second_limit));
        }
        let outside_start = first - direction * arrow_size;
        if outside_start.distance(&first) > 1.0e-12 {
            result.push((outside_start, first));
        }
    }
    result
}

fn midpoint(first: Vector3, second: Vector3) -> Vector3 {
    (first + second) * 0.5
}

fn angular_text_point(vertex: Vector3, first: Vector3, second: Vector3) -> Vector3 {
    let first_direction = first - vertex;
    let second_direction = second - vertex;
    let first_length = first_direction.length();
    let second_length = second_direction.length();
    if first_length <= 1.0e-12 || second_length <= 1.0e-12 {
        return midpoint(first, second);
    }
    let bisector = first_direction / first_length + second_direction / second_length;
    let bisector_length = bisector.length();
    if bisector_length <= 1.0e-12 {
        return midpoint(first, second);
    }
    let radius = first_length.min(second_length);
    vertex + bisector / bisector_length * radius
}

fn default_dimension_text_point(dim: &Dimension) -> Vector3 {
    match dim {
        Dimension::Linear(d) => {
            let direction = Matrix3::arbitrary_axis(d.base.normal)
                * Vector3::new(d.rotation.cos(), d.rotation.sin(), 0.0);
            let direction = direction.normalize();
            if direction.length_squared() <= 1.0e-20 {
                midpoint(d.first_point, d.second_point)
            } else {
                let project = |point: Vector3| {
                    d.definition_point + direction * (point - d.definition_point).dot(&direction)
                };
                midpoint(project(d.first_point), project(d.second_point))
            }
        }
        Dimension::Aligned(d) => {
            let direction = (d.second_point - d.first_point).normalize();
            if direction.length_squared() <= 1.0e-20 {
                midpoint(d.first_point, d.second_point)
            } else {
                let project = |point: Vector3| {
                    d.definition_point + direction * (point - d.definition_point).dot(&direction)
                };
                midpoint(project(d.first_point), project(d.second_point))
            }
        }
        Dimension::Radius(d) => midpoint(d.angle_vertex, d.definition_point),
        Dimension::Diameter(d) => midpoint(d.angle_vertex, d.definition_point),
        Dimension::Angular2Ln(d) => {
            angular_text_point(d.angle_vertex, d.second_point, d.definition_point)
        }
        Dimension::Angular3Pt(d) => {
            angular_text_point(d.angle_vertex, d.first_point, d.second_point)
        }
        Dimension::Ordinate(d) => d.leader_endpoint,
        Dimension::Arc(d) => {
            let first = d.first_extension_point - d.center_point;
            let second = d.second_extension_point - d.center_point;
            let radius = (first.length() + second.length()) * 0.5;
            let bisector = first.normalize() + second.normalize();
            if radius > 1.0e-12 && bisector.length_squared() > 1.0e-20 {
                d.center_point + bisector.normalize() * radius
            } else {
                midpoint(d.first_extension_point, d.second_extension_point)
            }
        }
        Dimension::LargeRadial(d) => midpoint(d.jog_point, d.chord_point),
    }
}

/// Explode a dimension with an optional DIMSTYLE table entry.
///
/// The detached [`EntityType::explode`] API has no document/table context and
/// therefore retains its historical defaults. [`CadDocument::explode_entity`]
/// supplies the resolved style through this function so offsets, precision,
/// suppression flags, and text height follow the drawing's DIMSTYLE.
pub(crate) fn explode_dimension_with_style(
    dim: &Dimension,
    style: Option<&DimStyle>,
) -> Vec<EntityType> {
    // Simplified decomposition: produce extension lines, dimension line,
    // and a text entity with the measurement value.
    let base = dim.base();
    let common = &base.common;

    let text_value = dimension_text_value(dim, style);
    let style_scale = style
        .map(|style| {
            if style.dimscale.is_finite() {
                style.dimscale.abs()
            } else {
                1.0
            }
        })
        .unwrap_or(1.0);
    let arrow_size = style
        .map(|style| (style.dimasz * style_scale).abs())
        .filter(|size| size.is_finite() && *size > 0.0)
        .unwrap_or(2.5);
    let dimension_gap = style
        .map(|style| (style.dimgap * style_scale).abs())
        .filter(|gap| gap.is_finite())
        .unwrap_or(0.0);

    let mut result = Vec::new();
    let mut arrows = Vec::new();
    let mut text_middle_point = base.text_middle_point;
    if !base.text_user_positioned {
        text_middle_point = default_dimension_text_point(dim);
    }

    // Dimension geometry is defined by two measured points and a point on the
    // dimension line.  Project each measured point onto that line so the
    // exploded extension and dimension lines meet at their endpoints.
    let projected_pair =
        |first: Vector3, second: Vector3, on_line: Vector3, line_direction: Vector3| {
            let direction = line_direction.normalize();
            if direction.length_squared() <= 1e-20 {
                return (first, second);
            }
            let project = |point: Vector3| {
                let offset = point - on_line;
                on_line + direction * offset.dot(&direction)
            };
            (project(first), project(second))
        };

    match dim {
        Dimension::Linear(d) => {
            let direction = Matrix3::arbitrary_axis(base.normal)
                * Vector3::new(d.rotation.cos(), d.rotation.sin(), 0.0);
            let (first_on_line, second_on_line) =
                projected_pair(d.first_point, d.second_point, d.definition_point, direction);
            if let Some(style) = style {
                if !style.dimse1 {
                    let (start, end) =
                        dimension_extension_points(d.first_point, first_on_line, style);
                    result.push(line_entity(start, end, 0.0, base.normal, common));
                }
                if !style.dimse2 {
                    let (start, end) =
                        dimension_extension_points(d.second_point, second_on_line, style);
                    result.push(line_entity(start, end, 0.0, base.normal, common));
                }
                if !style.dimsd1 || !style.dimsd2 {
                    for (start, end) in styled_dimension_line_segments(
                        first_on_line,
                        second_on_line,
                        text_middle_point,
                        arrow_size,
                        dimension_gap,
                    ) {
                        result.push(line_entity(start, end, 0.0, base.normal, common));
                    }
                }
            } else {
                result.push(line_entity(
                    d.first_point,
                    first_on_line,
                    0.0,
                    base.normal,
                    common,
                ));
                result.push(line_entity(
                    d.second_point,
                    second_on_line,
                    0.0,
                    base.normal,
                    common,
                ));
                // ACAD leaves a text gap in the dimension line.  Emit the two
                // visible portions around the text location.
                let axis = second_on_line - first_on_line;
                let t = if axis.length_squared() > 1e-20 {
                    ((text_middle_point - first_on_line).dot(&axis) / axis.length_squared())
                        .clamp(0.0, 1.0)
                } else {
                    0.5
                };
                let split = first_on_line + axis * t;
                result.push(line_entity(first_on_line, split, 0.0, base.normal, common));
                result.push(line_entity(split, second_on_line, 0.0, base.normal, common));
            }
            arrows.push((first_on_line, second_on_line));
            arrows.push((second_on_line, first_on_line));
        }
        Dimension::Aligned(d) => {
            let (first_on_line, second_on_line) = projected_pair(
                d.first_point,
                d.second_point,
                d.definition_point,
                d.second_point - d.first_point,
            );
            if let Some(style) = style {
                if !style.dimse1 {
                    let (start, end) =
                        dimension_extension_points(d.first_point, first_on_line, style);
                    result.push(line_entity(start, end, 0.0, base.normal, common));
                }
                if !style.dimse2 {
                    let (start, end) =
                        dimension_extension_points(d.second_point, second_on_line, style);
                    result.push(line_entity(start, end, 0.0, base.normal, common));
                }
                if !style.dimsd1 || !style.dimsd2 {
                    for (start, end) in styled_dimension_line_segments(
                        first_on_line,
                        second_on_line,
                        text_middle_point,
                        arrow_size,
                        dimension_gap,
                    ) {
                        result.push(line_entity(start, end, 0.0, base.normal, common));
                    }
                }
            } else {
                result.push(line_entity(
                    d.first_point,
                    first_on_line,
                    0.0,
                    base.normal,
                    common,
                ));
                result.push(line_entity(
                    d.second_point,
                    second_on_line,
                    0.0,
                    base.normal,
                    common,
                ));
                // Keep the measured alignment and split the dimension line at
                // the text location, matching ACAD's five LINE result.
                result.push(line_entity(
                    d.first_point,
                    d.second_point,
                    0.0,
                    base.normal,
                    common,
                ));
                let axis = second_on_line - first_on_line;
                let t = if axis.length_squared() > 1e-20 {
                    ((text_middle_point - first_on_line).dot(&axis) / axis.length_squared())
                        .clamp(0.0, 1.0)
                } else {
                    0.5
                };
                let split = first_on_line + axis * t;
                result.push(line_entity(first_on_line, split, 0.0, base.normal, common));
                result.push(line_entity(split, second_on_line, 0.0, base.normal, common));
            }
            arrows.push((first_on_line, second_on_line));
            arrows.push((second_on_line, first_on_line));
        }
        Dimension::Radius(d) => {
            result.push(line_entity(
                d.angle_vertex,
                d.definition_point,
                0.0,
                base.normal,
                common,
            ));
        }
        Dimension::Diameter(d) => {
            result.push(line_entity(
                d.angle_vertex,
                d.definition_point,
                0.0,
                base.normal,
                common,
            ));
        }
        Dimension::Angular2Ln(d) => {
            result.push(line_entity(
                d.angle_vertex,
                d.first_point,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.angle_vertex,
                d.second_point,
                0.0,
                base.normal,
                common,
            ));
        }
        Dimension::Angular3Pt(d) => {
            result.push(line_entity(
                d.angle_vertex,
                d.first_point,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.angle_vertex,
                d.second_point,
                0.0,
                base.normal,
                common,
            ));
        }
        Dimension::Ordinate(d) => {
            let points = d.leader_polyline(0.0, 0.0, None);
            for pair in points.windows(2) {
                if (pair[1] - pair[0]).length() > 1e-12 {
                    result.push(line_entity(pair[0], pair[1], 0.0, base.normal, common));
                }
            }
        }
        Dimension::Arc(d) => {
            result.push(line_entity(
                d.center_point,
                d.first_extension_point,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.center_point,
                d.second_extension_point,
                0.0,
                base.normal,
                common,
            ));
            if d.has_leader {
                result.push(line_entity(
                    d.first_leader_point,
                    d.second_leader_point,
                    0.0,
                    base.normal,
                    common,
                ));
            }
        }
        Dimension::LargeRadial(d) => {
            result.push(line_entity(
                d.definition_point,
                d.jog_point,
                0.0,
                base.normal,
                common,
            ));
            result.push(line_entity(
                d.jog_point,
                d.chord_point,
                0.0,
                base.normal,
                common,
            ));
        }
    }

    for (index, (tip, toward)) in arrows.into_iter().enumerate() {
        // A named _NONE arrow block suppresses the corresponding arrowhead;
        // custom block geometry still requires document block context and is
        // handled by the regular default arrow fallback here.
        let arrow_suppressed = style.is_some_and(|style| {
            (index == 0 && style.dimblk1_name.eq_ignore_ascii_case("_none"))
                || (index == 1 && style.dimblk2_name.eq_ignore_ascii_case("_none"))
        });
        if !arrow_suppressed {
            result.push(dimension_style_arrow(
                tip,
                toward,
                arrow_size,
                common,
                base.normal,
            ));
        }
    }
    if let Some(text_value) = text_value {
        result.push(dimension_mtext(base, text_value, style, text_middle_point));
    }

    result
}

pub(super) fn explode_dimension(dim: &Dimension) -> Vec<EntityType> {
    explode_dimension_with_style(dim, None)
}
