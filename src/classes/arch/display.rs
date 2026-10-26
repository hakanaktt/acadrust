//! Display geometry measured from native Arch records.
//!
//! The proxy-graphics preview of most AEC classes is only a bounding box.
//! These readers expose the placement frame and the measured sizes needed to
//! draw the class symbol itself instead of that box.

use super::geometry::{native_plan_frame, point_is_finite, positive};
use super::metrics::{member_frame, member_size, plan_frame, stair_size};
use super::{door_size, opening_size, window_size, ArchEntityData, ArchKind};
use crate::types::Vector3;

/// A door, window, or opening placed on a wall.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct OpeningGeometry {
    /// Jamb at the start of the opening, on the wall baseline.
    pub origin: Vector3,
    /// Unit direction from `origin` to the other jamb.
    pub along: Vector3,
    /// Unit direction across the wall.
    pub across: Vector3,
    /// Opening width along the wall.
    pub width: f64,
    /// Opening height.
    pub height: f64,
    /// Wall thickness used by the cached proxy graphics.
    pub depth: f64,
}

/// A placed straight stair.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StairGeometry {
    pub origin: Vector3,
    pub along: Vector3,
    pub across: Vector3,
    pub run: f64,
    pub width: f64,
    pub height: f64,
}

/// A placed straight railing.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct RailingGeometry {
    pub origin: Vector3,
    pub along: Vector3,
    pub across: Vector3,
    pub run: f64,
    pub height: f64,
    pub post_spacing: f64,
}

/// A straight curtain-wall layout.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CurtainWallGeometry {
    pub origin: Vector3,
    pub along: Vector3,
    pub across: Vector3,
    pub length: f64,
    pub height: f64,
    pub depth: f64,
}

/// A rectangular column or ceiling grid.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GridGeometry {
    pub origin: Vector3,
    pub along: Vector3,
    pub across: Vector3,
    pub length: f64,
    pub width: f64,
    pub spacing: f64,
}

/// A structural member with a rectangular cross-section.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MemberGeometry {
    pub origin: Vector3,
    pub along: Vector3,
    pub section_x: Vector3,
    pub section_y: Vector3,
    pub length: f64,
    pub breadth: f64,
    pub depth: f64,
}

/// Placement and size of a door, window, or opening.
pub fn opening_geometry(data: &ArchEntityData) -> Option<OpeningGeometry> {
    let size = match data.kind {
        ArchKind::Door => door_size(data)?,
        ArchKind::Window => window_size(data)?,
        ArchKind::Opening => opening_size(data)?,
        _ => return None,
    };
    let frame = plan_frame(data)?;
    if !point_is_finite(frame.origin)
        || !point_is_finite(frame.along)
        || !point_is_finite(frame.across)
        || !positive(size.width)
        || !positive(size.height)
    {
        return None;
    }
    let mut depth = projected_extent(&data.profiles, frame.across).unwrap_or(0.0);
    if !positive(depth) || depth >= size.width {
        depth = (size.width * 0.1).max(1.0);
    }
    Some(OpeningGeometry {
        origin: frame.origin,
        along: frame.along,
        across: frame.across,
        width: size.width,
        height: size.height,
        depth,
    })
}

/// Placement and dimensions of a straight stair.
pub fn stair_geometry(data: &ArchEntityData) -> Option<StairGeometry> {
    if data.kind != ArchKind::Stair {
        return None;
    }
    let size = stair_size(data)?;
    let placement = data.decode().placement?;
    if !placement.rigid || !point_is_finite(placement.origin) {
        return None;
    }
    let along = unit_plan_axis(placement.x_axis)?;
    let across = unit_plan_axis(placement.y_axis)?;
    Some(StairGeometry {
        origin: placement.origin,
        along,
        across,
        run: size.run,
        width: size.width,
        height: size.height,
    })
}

/// Placement and dimensions of a straight railing.
pub fn railing_geometry(data: &ArchEntityData) -> Option<RailingGeometry> {
    if data.kind != ArchKind::Railing {
        return None;
    }
    let placement = data.decode().placement?;
    if !placement.rigid || !point_is_finite(placement.origin) {
        return None;
    }
    let along = unit_plan_axis(placement.x_axis)?;
    let across = unit_plan_axis(placement.y_axis)?;
    let run = projected_extent(&data.profiles, along)?;
    if !positive(run) {
        return None;
    }
    let schema = data.decode().schema?;
    let mut reader = native_plan_frame(data, schema)?.body;
    let mut values = [0.0; 3];
    for value in &mut values {
        *value = reader.read_bit_double();
        if !positive(*value) {
            return None;
        }
    }
    reader.check_error().ok()?;
    values.sort_by(|left, right| right.total_cmp(left));
    let height = values[0];
    let post_spacing = values[1].min(run).max(1.0);
    Some(RailingGeometry {
        origin: placement.origin,
        along,
        across,
        run,
        height,
        post_spacing,
    })
}

/// Placement and dimensions of a straight curtain-wall layout.
pub fn curtain_wall_geometry(data: &ArchEntityData) -> Option<CurtainWallGeometry> {
    if data.kind != ArchKind::CurtainWall {
        return None;
    }
    let placement = data.decode().placement?;
    if !placement.rigid || !point_is_finite(placement.origin) {
        return None;
    }
    let along = unit_plan_axis(placement.x_axis)?;
    let across = unit_plan_axis(placement.y_axis)?;
    let length = projected_extent(&data.profiles, along)?;
    let depth = projected_extent(&data.profiles, across).unwrap_or(0.0);
    if !positive(length) {
        return None;
    }
    let height = projected_extent(&data.profiles, Vector3::UNIT_Z)
        .filter(|height| positive(*height))
        .unwrap_or(2500.0);
    Some(CurtainWallGeometry {
        origin: placement.origin,
        along,
        across,
        length,
        height,
        depth: depth.max(1.0),
    })
}

/// Placement and dimensions of a rectangular column or ceiling grid.
pub fn grid_geometry(data: &ArchEntityData) -> Option<GridGeometry> {
    if !matches!(data.kind, ArchKind::ColumnGrid | ArchKind::CeilingGrid) {
        return None;
    }
    let placement = data.decode().placement?;
    if !placement.rigid || !point_is_finite(placement.origin) {
        return None;
    }
    let along = unit_plan_axis(placement.x_axis)?;
    let across = unit_plan_axis(placement.y_axis)?;
    let length = projected_extent(&data.profiles, along)?;
    let width = projected_extent(&data.profiles, across)?;
    if !positive(length) || !positive(width) {
        return None;
    }
    let spacing = if data.kind == ArchKind::ColumnGrid {
        6000.0_f64.min(length.max(width)).max(1.0)
    } else {
        600.0_f64.min(length.max(width)).max(1.0)
    };
    Some(GridGeometry {
        origin: placement.origin,
        along,
        across,
        length,
        width,
        spacing,
    })
}

/// Placement and rectangular cross-section of a structural member.
pub fn member_geometry(data: &ArchEntityData) -> Option<MemberGeometry> {
    if data.kind != ArchKind::StructuralMember {
        return None;
    }
    let size = member_size(data)?;
    let frame = member_frame(data)?;
    Some(MemberGeometry {
        origin: frame.origin,
        along: frame.along,
        section_x: frame.section_x,
        section_y: frame.section_y,
        length: size.length,
        breadth: size.breadth,
        depth: size.depth,
    })
}

fn projected_extent(profiles: &[Vec<Vector3>], axis: Vector3) -> Option<f64> {
    let mut min = f64::INFINITY;
    let mut max = f64::NEG_INFINITY;
    let mut found = false;
    for point in profiles.iter().flatten().copied() {
        if !point_is_finite(point) {
            continue;
        }
        let value = point.dot(&axis);
        min = min.min(value);
        max = max.max(value);
        found = true;
    }
    (found && (max - min).is_finite()).then_some(max - min)
}

fn unit_plan_axis(axis: Vector3) -> Option<Vector3> {
    let axis = Vector3::new(axis.x, axis.y, 0.0);
    let length = axis.length();
    if !positive(length) {
        return None;
    }
    Some(axis * (1.0 / length))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entities::{EntityType, ExtendedEntityData};

    fn arch_data(doc: &crate::CadDocument, kind: ArchKind) -> &ArchEntityData {
        doc.entities()
            .find_map(|entity| {
                let EntityType::Extended(entity) = entity else {
                    return None;
                };
                let ExtendedEntityData::Arch(data) = &entity.data else {
                    return None;
                };
                (data.kind == kind).then_some(data)
            })
            .expect("architecture entity")
    }

    #[test]
    fn constructed_door_and_window_expose_opening_geometry() {
        let door = crate::classes::arch::door(900.0, 2100.0, None).unwrap();
        let geometry = opening_geometry(arch_data(&door, ArchKind::Door)).unwrap();
        assert_eq!(geometry.width, 900.0);
        assert_eq!(geometry.height, 2100.0);
        assert!(geometry.depth > 0.0);

        let window = crate::classes::arch::window(1200.0, 1500.0, None).unwrap();
        let geometry = opening_geometry(arch_data(&window, ArchKind::Window)).unwrap();
        assert_eq!(geometry.width, 1200.0);
        assert_eq!(geometry.height, 1500.0);
        assert!(geometry.depth > 0.0);
    }
}
