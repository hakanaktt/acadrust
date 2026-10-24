//! Paper space, viewport windows, and sheet lineweights.

use crate::document::CadDocument;
use crate::entities::Viewport;
use crate::objects::ObjectType;
use crate::types::{Matrix3, Matrix4, Transform, Vector2, Vector3};

/// Drawing units per millimetre of displayed lineweight.
///
/// `MEASUREMENT` 0 is English, so a millimetre of lineweight is `1/25.4`
/// drawing units. Any other value is metric, where one unit is one millimetre.
pub(crate) fn units_per_mm(measurement: i16) -> f64 {
    if measurement == 0 {
        1.0 / 25.4
    } else {
        1.0
    }
}

pub(crate) fn is_sheet_block(name: &str) -> bool {
    !name.eq_ignore_ascii_case("*Model_Space")
}

/// Block record that owns a layout tab. `"Model"` is model space.
pub(crate) fn layout_block_name(doc: &CadDocument, name: &str) -> Option<String> {
    if name.eq_ignore_ascii_case("model") {
        return Some("*Model_Space".to_string());
    }
    let block_handle = doc.objects.values().find_map(|object| match object {
        ObjectType::Layout(layout) if layout.name.eq_ignore_ascii_case(name) => {
            Some(layout.block_record)
        }
        _ => None,
    })?;
    doc.block_records
        .iter()
        .find(|record| record.handle == block_handle)
        .map(|record| record.name.clone())
}

/// Paper units per model unit. `view_height` wins. `custom_scale` is the fallback.
pub(crate) fn viewport_scale(viewport: &Viewport) -> f64 {
    if viewport.view_height.abs() > 1e-10 {
        let scale = viewport.height / viewport.view_height;
        if scale.abs() > 1e-12 {
            return scale;
        }
    }
    if viewport.custom_scale.abs() > 1e-12 {
        viewport.custom_scale
    } else {
        1.0
    }
}

/// Model windows are on, have a size, and are not the paperspace viewport (`id` 1).
pub(crate) fn viewport_shows_model(viewport: &Viewport) -> bool {
    if viewport.id == 1 || viewport.off_screen || viewport.status.is_off || !viewport.status.is_on {
        return false;
    }
    if matches!(viewport.dxf_status, Some(status) if status <= 0) {
        return false;
    }
    viewport.width.abs() > 1e-9
        && viewport.height.abs() > 1e-9
        && viewport_scale(viewport).abs() > 1e-12
}

pub(crate) fn viewport_rectangle(viewport: &Viewport) -> Vec<Vector2> {
    let half_width = viewport.width.abs() * 0.5;
    let half_height = viewport.height.abs() * 0.5;
    super::clip::rectangle(
        Vector2::new(
            viewport.center.x - half_width,
            viewport.center.y - half_height,
        ),
        Vector2::new(
            viewport.center.x + half_width,
            viewport.center.y + half_height,
        ),
    )
}

/// Map a paper-space polygon through `transform` and keep it counter-clockwise.
pub(crate) fn map_polygon(transform: &Transform, polygon: &[Vector2]) -> Vec<Vector2> {
    let mut mapped: Vec<Vector2> = polygon
        .iter()
        .map(|point| {
            let world = transform.apply(Vector3::new(point.x, point.y, 0.0));
            Vector2::new(world.x, world.y)
        })
        .collect();
    if signed_area(&mapped) < 0.0 {
        mapped.reverse();
    }
    mapped
}

fn signed_area(polygon: &[Vector2]) -> f64 {
    let mut area = 0.0;
    for index in 0..polygon.len() {
        let start = polygon[index];
        let end = polygon[(index + 1) % polygon.len()];
        area += start.x * end.y - end.x * start.y;
    }
    area
}

/// Model space onto the sheet.
///
/// `view_direction` points from the target toward the camera. Positive twist
/// rotates the picture clockwise about that direction. `view_center` is the
/// display-coordinate point that lands on the viewport center. The projection
/// is orthographic: a perspective flag does not foreshorten the window.
pub(crate) fn model_to_paper(viewport: &Viewport) -> Transform {
    let scale = viewport_scale(viewport);
    let direction = if viewport.view_direction.length_squared() > 1e-18 {
        viewport.view_direction
    } else {
        Vector3::UNIT_Z
    };
    let to_view = Matrix4::from_matrix3(Matrix3::arbitrary_axis(direction).transpose());
    Transform::from_translation(-viewport.view_target)
        .then(&Transform::from_matrix(to_view))
        .then(&Transform::from_matrix(Matrix4::rotation_z(
            -viewport.twist_angle,
        )))
        .then(&Transform::from_translation(Vector3::new(
            -viewport.view_center.x,
            -viewport.view_center.y,
            0.0,
        )))
        .then(&Transform::from_scaling(Vector3::new(scale, scale, scale)))
        .then(&Transform::from_translation(viewport.center))
}
