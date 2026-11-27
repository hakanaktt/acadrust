//! Civil objects encoded from named fields.
//!
//! Every record is written from its fields; the grammar of each record, the
//! support objects a kind needs and what the reference application does
//! with a written drawing are in `docs/civil-write.md`.

mod alignment;
mod alignment_labels;
mod arrangement;
mod assembly;
mod band_styles;
mod bands;
mod assoc;
mod corridor;
mod default_styles;
mod grading;
mod grading_entity;
mod groups;
mod intersection;
mod labels;
mod label_style;
mod mass_haul;
mod network;
mod network_content;
mod network_families;
mod network_sizes;
mod network_structure_sizes;
mod points;
mod pressure;
mod pressure_content;
mod pressure_fittings;
mod pressure_sizes;
mod profile;
mod profile_view;
mod sample_lines;
mod section_band;
mod section_view;
mod site;
mod stormwater;
mod support;
mod templates;
mod tin;
mod tin_build;
mod tree;
mod user_properties;

pub use assembly::{add_assembly, add_pkt_subassembly, add_pkt_subassembly_on, PktSource};
pub use section_band::add_section_data_band_label_group;
pub use alignment::{add_curb_return, add_curb_return_with, CurbReturnCurve, add_alignment, add_alignment_with_curves, add_offset_alignment, add_offset_alignment_with_widenings, add_alignment_of_kind, add_design_speeds, add_station_equations, add_superelevation, CriticalStation, CriticalStationKind, SuperelevationCurve, AlignmentKind, VertexCurve, Widening};
pub use alignment_labels::{add_alignment_label_group, AlignmentGroupLabel, AlignmentLabelGroup, AlignmentLabelGroupKind, LabelKeyValue, LabelOverrideProperty, LabelOverrideValue};
pub use corridor::{add_corridor, add_corridor_with};
pub use intersection::{add_intersection, add_intersection_with, add_intersection_with_curb_returns, IntersectionOffsets, CURB_RETURN_MARGIN, add_intersection_with_curb_return_profiles, CurbReturnProfiles};
pub use network::{add_pipe, add_pipe_network, add_pipe_of_family, add_structure, add_structure_of_family, connect_pipe, set_network_part_name, network_part_families, add_pipe_of_size, network_pipe_sizes};
pub use network_structure_sizes::{add_structure_of_size, network_structure_size_fields, StructureSizeChoice};
pub use pressure::{add_pressure_appurtenance, add_pressure_arc_pipe, add_pressure_cross, add_pressure_fitting, add_pressure_network, add_pressure_part, add_pressure_pipe, add_pressure_pipe_of_size, add_pressure_tee, connect_pressure_parts, pressure_part_descriptions};
pub use groups::{add_point_group, add_point_group_query, set_point_group, set_point_group_draw_order, set_point_group_overrides, set_point_group_query, set_point_group_styles, Comparison, PointAttribute, PointQuery, QueryConstant, QueryExpression};
pub use grading::add_grading_group_tree;
pub use grading_entity::{add_grading_minimal, GradingSkeleton};
pub use profile::{add_profile, add_profile_entities, add_profile_with_curves, add_static_profile, add_surface_profile, ProfileEntity, VerticalCurve, add_offset_profile};
pub use profile_view::{add_profile_view, add_profile_view_with_bands, add_profile_view_with_range, add_profile_view_style, add_profile_view_with_style, add_profile_view_style_with};
pub use bands::{BandKind, BandStagger, ProfileViewBand};
pub use sample_lines::{add_sample_line, add_sample_line_through, add_sample_line_group, add_sections};
pub use mass_haul::add_mass_haul_line;
pub use section_view::{add_section_view, add_section_view_group};
pub use site::{add_alignment_on_site, add_feature_line, add_lot_line, add_parcel, add_feature_line_vertices, feature_line_vertices, parcel_name, FeatureLineVertex, add_site, set_feature_line_vertices, PARCEL_STYLE_NAME, PARCEL_NUMBER};
pub use stormwater::{add_channel, add_pond, add_underground_storage};
pub use points::{add_cogo_point, cogo_points, create_cogo_point, point_creation_layer, transformation_applied, NO_ELEVATION, remove_cogo_point, set_cogo_point, set_cogo_point_name, set_cogo_point_label_style};
pub use label_style::{add_label_style, add_point_label_style, label_style, LabelStyle};
pub use support::ensure_civil;
pub use user_properties::{add_point_choice_property, add_point_property, add_point_property_classification, point_properties, set_cogo_point_property};
pub use tin::{add_empty_tin_surface, add_tin_surface, add_tin_surface_with};
