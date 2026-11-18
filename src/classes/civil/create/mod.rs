//! Civil objects encoded from named fields.
//!
//! Every record is written from its fields; the grammar of each record, the
//! support objects a kind needs and what the reference application does
//! with a written drawing are in `docs/civil-write.md`.

mod points;
mod support;

pub use points::{add_cogo_point, cogo_points, remove_cogo_point, set_cogo_point};
pub use support::ensure_civil;
