//! Civil objects encoded from named fields.
//!
//! Every record is written from its fields; the grammar of each record, the
//! support objects a kind needs and what the reference application does
//! with a written drawing are in `docs/civil-write.md`.

mod alignment;
mod corridor;
mod default_styles;
mod groups;
mod intersection;
mod labels;
mod points;
mod profile;
mod profile_view;
mod sample_lines;
mod section_view;
mod site;
mod support;
mod templates;
mod tin;
mod tree;

pub use alignment::{add_alignment, add_alignment_with_curves, VertexCurve};
pub use corridor::add_corridor;
pub use intersection::add_intersection;
pub use groups::{add_point_group, add_point_group_query, set_point_group, set_point_group_query, Comparison, PointQuery};
pub use profile::{add_profile, add_profile_with_curves, VerticalCurve};
pub use profile_view::{add_profile_view, add_profile_view_with_range};
pub use sample_lines::{add_sample_line, add_sample_line_group, add_sections};
pub use section_view::add_section_view;
pub use site::{add_alignment_on_site, add_feature_line, add_feature_line_vertices, FeatureLineVertex, add_site};
pub use points::{add_cogo_point, cogo_points, remove_cogo_point, set_cogo_point};
pub use support::ensure_civil;
pub use tin::add_tin_surface;
