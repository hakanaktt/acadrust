use crate::types::{Vector2, Vector3};

/// Current pre-R13 view, drafting and user-unit settings. These are independent
/// of named VPORT records. Angles use radians; bitfields retain their native bits.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct LegacyHeaderState {
    pub view_center: Vector3,
    pub view_height: f64,
    pub view_aspect_ratio: f64,
    pub view_direction: Vector3,
    pub view_target: Vector3,
    pub snap_mode: i16,
    pub snap_spacing: Vector2,
    pub snap_base: Vector2,
    pub snap_angle: f64,
    pub snap_style: i16,
    pub snap_isopair: i16,
    pub grid_mode: i16,
    pub grid_spacing: Vector2,
    pub axis_mode: i16,
    pub axis_spacing: Vector2,
    pub circle_zoom: i16,
    pub fast_zoom: i16,
    pub front_clip: f64,
    pub back_clip: f64,
    pub view_mode: i16,
    pub flatland: i16,
    pub ucs_icon: i16,
    pub unit_mode: i16,
    pub user_unit_ratios: [f64; 4],
    pub user_unit_names: [String; 4],
}

impl Default for LegacyHeaderState {
    fn default() -> Self {
        Self {
            view_center: Vector3::ZERO,
            view_height: 10.0,
            view_aspect_ratio: 1.0,
            view_direction: Vector3::UNIT_Z,
            view_target: Vector3::ZERO,
            snap_mode: 0,
            snap_spacing: Vector2::new(0.5, 0.5),
            snap_base: Vector2::ZERO,
            snap_angle: 0.0,
            snap_style: 0,
            snap_isopair: 0,
            grid_mode: 0,
            grid_spacing: Vector2::new(0.5, 0.5),
            axis_mode: 0,
            axis_spacing: Vector2::ZERO,
            circle_zoom: 100,
            fast_zoom: 1,
            front_clip: 0.0,
            back_clip: 0.0,
            view_mode: 0,
            flatland: 0,
            ucs_icon: 3,
            unit_mode: 0,
            user_unit_ratios: [1.0; 4],
            user_unit_names: std::array::from_fn(|_| String::new()),
        }
    }
}
