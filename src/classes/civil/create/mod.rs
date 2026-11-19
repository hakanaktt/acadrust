//! Civil objects encoded from named fields.
//!
//! Every record is written from its fields; the grammar of each record, the
//! support objects a kind needs and what the reference application does
//! with a written drawing are in `docs/civil-write.md`.

mod alignment;
mod groups;
mod points;
mod profile;
mod site;
mod support;
mod tin;
mod tree;

pub use alignment::{add_alignment, add_alignment_with_curves, VertexCurve};
pub use groups::{add_point_group, add_point_group_query, set_point_group, set_point_group_query, Comparison, PointQuery};
pub use profile::{add_profile, add_profile_with_curves, VerticalCurve};
pub use site::{add_alignment_on_site, add_feature_line, add_site};
pub use points::{add_cogo_point, cogo_points, remove_cogo_point, set_cogo_point};
pub use support::ensure_civil;
pub use tin::add_tin_surface;
